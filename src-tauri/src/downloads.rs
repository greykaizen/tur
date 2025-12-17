#[path = "downloads/core.rs"]
pub mod core;
#[path = "downloads/manager.rs"]
pub mod manager;

use reqwest::Client;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tauri::{Emitter, Manager};
use url::Url;
use uuid::Uuid;

use crate::database;
use crate::settings;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum DownloadRequest {
    /// New downloads from external sources (browser extension, manual add, drag & drop)
    New(Vec<Url>),
    /// Resume existing downloads from history
    Resume(Vec<Uuid>),
    /// Deep link URLs (cold start, app fetches headers)
    DeepLink(Vec<Url>),
}

/// Handle deep link URL parsing and create download request
pub fn parse_deep_link_url(url_str: &str) -> Option<(Url, Option<String>, Option<u64>)> {
    let parsed = Url::parse(url_str).ok()?;
    
    let src_url_str = parsed.query_pairs().find(|(k, _)| k == "url")?.1.to_string();
    let src_url = Url::parse(&src_url_str).ok()?;
    
    let filename = parsed
        .query_pairs()
        .find(|(k, _)| k == "filename")
        .map(|(_, v)| v.to_string());
    let size_opt = parsed
        .query_pairs()
        .find(|(k, _)| k == "size")
        .and_then(|(_, v)| v.parse::<u64>().ok());
    
    Some((src_url, filename, size_opt))
}

/// Create optimized HTTP client with settings-based configuration
fn create_http_client(settings: &settings::config::AppSettings) -> Result<Client, String> {
    let client = Client::builder()
        // Timeouts based on settings or sensible defaults
        .timeout(Duration::from_secs(300)) // 5min total timeout
        .connect_timeout(Duration::from_secs(15)) // Slightly longer connection timeout
        // Connection pooling for better performance
        .pool_max_idle_per_host(settings.thread.total_connections as usize)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        // Compression is enabled by default in reqwest
        // User agent and redirects
        .user_agent("tur/1.0 (Download Manager)")
        .redirect(reqwest::redirect::Policy::limited(10))
        // Security settings
        .danger_accept_invalid_certs(false)
        .https_only(false) // Allow HTTP for compatibility
        // HTTP/2 support
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(Some(Duration::from_secs(30)))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    Ok(client)
}

// Helper functions for extracting download metadata
fn extract_filename_from_headers(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(|cd| {
            // Parse Content-Disposition header for filename
            cd.split(';').find_map(|part| {
                let part = part.trim();
                if part.starts_with("filename=") {
                    Some(part[9..].trim_matches('"').to_string())
                } else if part.starts_with("filename*=") {
                    // Handle RFC 5987 encoded filenames
                    part[10..].split('\'').nth(2).map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
}

fn extract_filename_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|s| s.split('?').next()) // Remove query parameters
        .and_then(|s| s.split('#').next()) // Remove fragments
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_string()
}

fn extract_content_length(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

fn extract_etag(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string()) // Remove quotes if present
}

fn extract_last_modified(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn extract_resume_support(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get(reqwest::header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("bytes"))
        .unwrap_or(false)
}

// for new instances
// creating instance of Download push it's handle to DMan
#[tauri::command]
pub async fn handle_download_request(
    app: tauri::AppHandle,
    request: DownloadRequest,
) -> Result<(), String> {
    // Load fresh settings state
    let settings = settings::load_or_create(&app);
    
    // Create HTTP client
    let client = match create_http_client(&settings) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create HTTP client: {}", e);
            return Err(e);
        }
    };

    match request {
        DownloadRequest::New(urls) => {
            // Get database instance
            let db = database::Database::initialize(&app).map_err(|e| e.to_string())?;
            
            // Process each URL from browser extension
            for url in urls {
                let url_str = url.as_str();
                
                // Fetch headers from server
                let response = client.head(url_str).send().await.map_err(|e| e.to_string())?;
                let headers = response.headers();
                
                let filename = extract_filename_from_headers(headers)
                    .unwrap_or_else(|| extract_filename_from_url(url_str));
                let size = extract_content_length(headers).map(|s| s as i64);
                let etag = extract_etag(headers);
                let last_modified = extract_last_modified(headers);
                let resume_supported = extract_resume_support(headers);

                // Generate unique ID for this download
                let id = Uuid::now_v7();

                // Determine destination path (use downloads directory + filename)
                let downloads_dir = app.path().download_dir()
                    .map_err(|e| format!("Failed to get downloads directory: {}", e))?;
                let destination = downloads_dir.join(&filename).to_string_lossy().to_string();

                // Store to database
                db.insert_download(
                    &id,
                    url_str,
                    &filename,
                    &destination,
                    size,
                    headers.get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok()),
                    etag.as_deref(),
                    last_modified.as_deref(),
                    resume_supported,
                ).map_err(|e| e.to_string())?;

                // Emit download info to frontend
                let payload = json!({
                    "id": id,
                    "url": url_str,
                    "filename": filename,
                    "size": size,
                    "destination": destination,
                    "resume_supported": resume_supported,
                    "etag": etag,
                    "last_modified": last_modified,
                    "status": "queued",
                    "type": "external"
                });
                
                if let Err(e) = app.emit("queue_download", payload) {
                    eprintln!("Failed to emit queue_download event: {}", e);
                }

                // TODO: Start download work through download manager
                // 1. Create Download instance with settings
                // 2. Add to download manager
                // 3. Start download process
            }

            Ok(())
        }
        DownloadRequest::Resume(uuids) => {

            // instructions
            // resume variant takes uuid, which are to be checked from the database that they exist or not and then we retreive their respective information that is present in the db, we use client to fetch headers and match the information for any mismatch of information e.g last-modified/etag change may mean remove the old downloaded data and start download from zero again, the resume support might have dropped, file destination might have change, or server got file updated or something, you get the idea.update the headers, save in db and start the download.
            // you can take a look down at my personal comments as well how i was thinking about it would go. you are not allowed to edit my personal comments (they have architecture guides for me).
            // comments below this area are not part of instructions.

            // Get database instance
            let db = database::Database::initialize(&app).map_err(|e| e.to_string())?;

            // Get resume info for all requested UUIDs from database
            let uuid_refs: Vec<&Uuid> = uuids.iter().collect();
            let downloads = db.get_resume_info(uuid_refs).map_err(|e| e.to_string())?;

            for download in downloads {
                // 1st .emit("queue_work") - emit initial download info
                let payload = json!({
                    "id": download.id,
                    "url": download.url,
                    "filename": download.filename,
                    "size": download.size,
                    "type": "resume_check"
                });
                
                if let Err(e) = app.emit("queue_download", payload) {
                    eprintln!("Failed to emit queue_download event: {}", e);
                    continue;
                }

                // Check file existence on destination
                let file_path = Path::new(&download.destination);
                let file_exists = file_path.exists();
                let current_file_size = if file_exists {
                    std::fs::metadata(file_path).ok().map(|m| m.len() as i64).unwrap_or(0)
                } else {
                    0
                };

                // Fetch current headers from server to check for changes
                let response = match client.head(&download.url).send().await {
                    Ok(resp) => resp,
                    Err(e) => {
                        eprintln!("Failed to fetch headers for {}: {}", download.url, e);
                        continue;
                    }
                };

                let headers = response.headers();
                let server_etag = extract_etag(headers);
                let server_last_modified = extract_last_modified(headers);
                let server_size = extract_content_length(headers).map(|s| s as i64);
                let resume_supported = extract_resume_support(headers);

                // Check for mismatches that require restart from scratch
                let needs_restart = !file_exists ||
                    (download.etag.is_some() && server_etag != download.etag) ||
                    (download.last_modified.is_some() && server_last_modified != download.last_modified) ||
                    (download.size.is_some() && server_size != download.size);

                if needs_restart {
                    // Update headers in database and reset progress
                    if let Err(e) = db.update_headers(
                        &download.id,
                        server_size,
                        headers.get(reqwest::header::CONTENT_TYPE)
                            .and_then(|v| v.to_str().ok()),
                        server_etag.as_deref(),
                        server_last_modified.as_deref(),
                        resume_supported,
                    ) {
                        eprintln!("Failed to update headers: {}", e);
                        continue;
                    }
                    
                    // Reset progress to 0
                    if let Err(e) = db.update_progress(&download.id, 0) {
                        eprintln!("Failed to reset progress: {}", e);
                        continue;
                    }
                } else {
                    // Update progress to current file size
                    if let Err(e) = db.update_progress(&download.id, current_file_size) {
                        eprintln!("Failed to update progress: {}", e);
                        continue;
                    }
                }

                // 2nd .emit("queue_work") - emit resume info with updated headers
                let resume_payload = json!({
                    "id": download.id,
                    "url": download.url,
                    "filename": download.filename,
                    "size": server_size,
                    "bytes_received": if needs_restart { 0 } else { current_file_size },
                    "resume_supported": resume_supported,
                    "etag": server_etag,
                    "last_modified": server_last_modified,
                    "needs_restart": needs_restart,
                    "type": "resume_ready"
                });
                
                if let Err(e) = app.emit("queue_download", resume_payload) {
                    eprintln!("Failed to emit resume_ready event: {}", e);
                }

                // TODO: Start download work through download manager
                // DMAN store to db
                // Download() starts
            }

            // --- resume_instance (for resuming old instances, headers aren't available)
            // History shows
            // uuid(not shown but there), Name, Status, Date,
            // so frontend sends back uuid

            // frontend sends Instance Config for settings

            // get from db via uuid
            // uuid (came from frontend)
            // filename
            // size
            // url
            // Etag (consume & drop)
            // Last-Modified (consume & drop)
            // dest. location

            // check file existance on dest. location, if not there start from scratch via Download::new(id, size, num_conn)

            // prep asyn client

            // 1st .emit("queue_work") (as req. came we check dest. & metadata then emit and start client)
            // uuid
            // filename
            // size
            // url

            // await client, get header match

            // 2nd .emit("queue_work") (client.await is completed, we check etag/last modified, emit resume, client starts working)
            // resume
            // Etag
            // Last-Modified

            // DMAN store to db
            // Download() starts

            // ---
            // received vec

            // loop isn't right we gotta do these in async as well
            // load existing record via uuid, match if right for use else redo connection (InstanceTarget::new)
            // for _uuid in &uuids {
                // using download_manager.db we get the whole record via get_resume_info(uuid)
                // destination check for the file existance, if show window "download file missing" with restart again button
                // etag, last-mod for check & match, if mismatch then drop old etag/last-mod and retrieve new headers, parse new info from it then update_download.

                // 206 support from response

                // .get().send() (inside arm)
                // let response = client.get(&url).send();

                // await client for work on between
                // let client = response.await?;

                // load headers via client
                // let headers = client.headers();
                // let filename = headers
                //     .get(reqwest::header::CONTENT_DISPOSITION)
                //     .and_then(|v| v.to_str().ok())
                //     .and_then(|cd| {
                //         cd.split(';').find_map(|p| {
                //             p.trim()
                //                 .strip_prefix("filename=")
                //                 .map(|s| s.trim_matches('"').to_string())
                //         })
                //     })
                //     .unwrap_or_else(|| {
                //         url.rsplit('/')
                //             .next()
                //             .and_then(|s| s.split('?').next())
                //             .unwrap_or("download")
                //             .to_string()
                //     });
                // let size = headers
                //     .get(reqwest::header::CONTENT_LENGTH)
                //     .and_then(|v| v.to_str().ok())
                //     .and_then(|s| s.parse::<u64>().ok());
                // let resume = headers
                //     .get(reqwest::header::ACCEPT_RANGES)
                //     .and_then(|v| v.to_str().ok())
                //     .map(|s| s.eq_ignore_ascii_case("bytes"))
                //     .unwrap_or(false);
                // let etag = headers
                //     .get(reqwest::header::ETAG)
                //     .and_then(|v| v.to_str().ok())
                //     .map(|s| s.to_string());
                // let last_modified = headers
                //     .get(reqwest::header::LAST_MODIFIED)
                //     .and_then(|v| v.to_str().ok())
                //     .map(|s| s.to_string());

                // // generate ID
                // let id = Uuid::now_v7();

                // // emit full
                // let payload = json!({
                //     "id": id,
                //     "url": url, // might not be needed
                //     "filename": filename,
                //     "size": size,
                //     "resume": resume,
                //     "etag": etag,
                //     "last-modified": last_modified
                // });
                // if let Err(e) = app.emit("queue_download", payload) {
                //     eprintln!("failed to emit event: {}", e);
                // }
            // }
            // await client
            // via client load headers
            // emit twice
            // start work
            Ok(())
        }
        DownloadRequest::DeepLink(urls) => {
            // Get database instance
            let db = database::Database::initialize(&app).map_err(|e| e.to_string())?;
            
            // Process each URL from deep link
            for url in urls {
                let url_str = url.as_str();
                
                // Fetch headers from server
                let response = client.head(url_str).send().await.map_err(|e| e.to_string())?;
                let headers = response.headers();
                
                let filename = extract_filename_from_headers(headers)
                    .unwrap_or_else(|| extract_filename_from_url(url_str));
                let size = extract_content_length(headers).map(|s| s as i64);
                let etag = extract_etag(headers);
                let last_modified = extract_last_modified(headers);
                let resume_supported = extract_resume_support(headers);

                // Generate unique ID for this download
                let id = Uuid::now_v7();

                // Determine destination path (use downloads directory + filename)
                let downloads_dir = app.path().download_dir()
                    .map_err(|e| format!("Failed to get downloads directory: {}", e))?;
                let destination = downloads_dir.join(&filename).to_string_lossy().to_string();

                // Store to database
                db.insert_download(
                    &id,
                    url_str,
                    &filename,
                    &destination,
                    size,
                    headers.get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok()),
                    etag.as_deref(),
                    last_modified.as_deref(),
                    resume_supported,
                ).map_err(|e| e.to_string())?;

                // Emit download info to frontend
                let payload = json!({
                    "id": id,
                    "url": url_str,
                    "filename": filename,
                    "size": size,
                    "destination": destination,
                    "resume_supported": resume_supported,
                    "etag": etag,
                    "last_modified": last_modified,
                    "status": "queued",
                    "type": "deep_link"
                });
                
                if let Err(e) = app.emit("queue_download", payload) {
                    eprintln!("Failed to emit queue_download event: {}", e);
                }

                // TODO: Start download work through download manager
            }

            Ok(())
        }
    }
}
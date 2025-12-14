use reqwest::Client;
use serde_json::json;
// use url::Url;
use std::time::Duration;
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_deep_link::DeepLinkExt;
use uuid::Uuid;

// use crate::download_manager::DownloadManager;
pub mod args;
pub mod db;
// pub mod download;
// pub mod download_manager;
pub mod settings;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let parsed_args = args::AppArgs::parse_from_vec(&args);
            
            // Handle deep link if present
            if let Some(url_str) = &parsed_args.deep_link {
                handle_deep_link(app, url_str);
            }

            // Show window unless minimized
            if let Some(window) = app.get_webview_window("main") {
                if !parsed_args.minimized {
                    let _ = window.show();
                    let _ = window.set_focus();
                } else {
                    let _ = window.hide();
                }
            }
        }))
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::update_settings,
            settings::update_setting,
            get_autostart,
            set_autostart,
        ])
        .setup(|app| {
            // Parse command line arguments
            let args = args::AppArgs::parse();
            
            // Handle deep links from startup
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    handle_deep_link(app.handle(), url.as_str());
                }
            }
            
            // Handle deep link from command line
            if let Some(url) = &args.deep_link {
                handle_deep_link(app.handle(), url);
            }
            
            // Handle minimized startup
            if args.minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    
    if enabled {
        autostart.enable().map_err(|e| e.to_string())
    } else {
        autostart.disable().map_err(|e| e.to_string())
    }
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

fn handle_deep_link(app: &tauri::AppHandle, url_str: &str) {
    let parsed = match url::Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return,
    };

    // TODO reading other header etag, last-modified, partial-content etc
    let src_url = match parsed.query_pairs().find(|(k, _)| k == "url") {
        Some((_, v)) => v.to_string(),
        _ => return,
    };
    let filename = parsed
        .query_pairs()
        .find(|(k, _)| k == "filename")
        .map(|(_, v)| v.to_string());
    let size_opt = parsed
        .query_pairs()
        .find(|(k, _)| k == "size")
        .and_then(|(_, v)| v.parse::<u64>().ok());

    // Read settings from store
    let _settings = settings::load_or_create(app);

    let id = uuid::Uuid::now_v7();

    // Start work with settings from store
    // Create download request for deep link
    let _download_request = DownloadRequest::DeepLink(vec![src_url.clone()]);
    
    // TODO: Process download request through download manager
    let _ = app.emit(
        "download-request",
        json!({
            "id": id,
            "url": src_url,
            "filename": filename,
            "size": size_opt,
            "type": "deep_link"
        }),
    );
}

// ------------------------------------------

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DownloadInfo {
    pub url: String,
    pub filename: Option<String>,
    pub size: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub resume_supported: Option<bool>,
    pub headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", content = "data")]
enum DownloadRequest {
    /// New downloads with pre-fetched headers (from browser extension, manual add, drag & drop)
    New(Vec<DownloadInfo>),
    /// Resume existing downloads from history
    Resume(Vec<Uuid>),
    /// Deep link URLs (cold start, app fetches headers)
    DeepLink(Vec<String>),
}


// for new instances
// creating instance of Download push it's handle to DMan
#[tauri::command]
async fn handle_download_request(
    app: tauri::AppHandle,
    request: DownloadRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load fresh settings state
    let settings = settings::load_or_create(&app);

    // Create optimized HTTP client with settings-based configuration
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

    match request {
        DownloadRequest::New(download_infos) => {
            // Process each download with pre-fetched information
            for download_info in download_infos {
                let url = &download_info.url;
                
                // Use provided info or fetch from server
                let (filename, size, etag, last_modified, resume_supported) = 
                    if download_info.filename.is_some() && download_info.size.is_some() {
                        // Use pre-fetched data from browser extension
                        (
                            download_info.filename.clone().unwrap_or_else(|| extract_filename_from_url(url)),
                            download_info.size,
                            download_info.etag.clone(),
                            download_info.last_modified.clone(),
                            download_info.resume_supported.unwrap_or(false),
                        )
                    } else {
                        // Fetch headers from server (fallback)
                        let response = client.head(url).send().await?;
                        let headers = response.headers();
                        
                        let filename = extract_filename_from_headers(headers)
                            .unwrap_or_else(|| extract_filename_from_url(url));
                        let size = extract_content_length(headers);
                        let etag = extract_etag(headers);
                        let last_modified = extract_last_modified(headers);
                        let resume_supported = extract_resume_support(headers);
                        
                        (filename, size, etag, last_modified, resume_supported)
                    };

                // Generate unique ID for this download
                let id = Uuid::now_v7();

                // TODO: Store to database here
                // download_manager.db.insert_download(id, url, filename, size, etc.)

                // Emit download info to frontend
                let payload = json!({
                    "id": id,
                    "url": url,
                    "filename": filename,
                    "size": size,
                    "resume_supported": resume_supported,
                    "etag": etag,
                    "last_modified": last_modified,
                    "status": "queued"
                });
                
                if let Err(e) = app.emit("queue_download", payload) {
                    eprintln!("Failed to emit queue_download event: {}", e);
                }

                // TODO: Start download work
                // 1. Create Download instance with settings
                // 2. Store to database
                // 3. Add to download manager
                // 4. Start download process
            }

            Ok(())
        }
        DownloadRequest::Resume(uuids) => {
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
            for _uuid in &uuids {
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
            }
            // await client
            // via client load headers
            // emit twice
            // start work
            Ok(())
        }
        DownloadRequest::DeepLink(_urls) => {
            // received vec

            // // loop vec
            // decode urls + headers(for emit only)
            // .get().send() (inside arm)

            // emit full
            // await client
            // start work
            Ok(())
        }
    }
}

// TODO removal after impl. the uuid to emit and listen for events
// for instances that are already in history
// #[tauri::command]
// fn instance_action(id: Vec<usize>, action: u8) {
//     // actions: cancel(0), start(1), pause(2)  (assuming item is already in DM)
//     match action {
//         // 0 => engine::DownloadManager::pause_instance(id),
//         // 1 => engine::DownloadManager::start_instance(id),
//         // 2 => engine::DownloadManager::cancel_instance(id),
//         _ => unreachable!(),
//     }
// }

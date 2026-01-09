//! Download manager - handles active downloads and control commands

use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::task::JoinHandle;
use url::Url;
use uuid::Uuid;

#[cfg(unix)]
use tokio::signal::{self, unix::SignalKind};

use super::download::Download;
use super::headers;
use super::workers::run_download;
use crate::database::Database;
use crate::downloads::client;
use crate::settings::{self, config::AppSettings};

/// Resolves destination path conflicts by adding numeric suffix like (1), (2), etc.
/// Returns the resolved path and potentially modified filename.
fn resolve_destination_conflict(downloads_dir: &Path, filename: &str) -> (PathBuf, String) {
    let original_path = downloads_dir.join(filename);

    if !original_path.exists() {
        return (original_path, filename.to_string());
    }

    // Split filename into stem and extension
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or(filename);
    let extension = path.extension().and_then(OsStr::to_str);

    // Try incrementing numbers until we find an available filename
    for i in 1..1000 {
        let new_filename = match extension {
            Some(ext) => format!("{} ({}).{}", stem, i, ext),
            None => format!("{} ({})", stem, i),
        };
        let new_path = downloads_dir.join(&new_filename);
        if !new_path.exists() {
            return (new_path, new_filename);
        }
    }

    // Fallback: use UUID suffix if all numbers exhausted (unlikely)
    let uuid_suffix = Uuid::now_v7()
        .to_string()
        .split('-')
        .next()
        .unwrap()
        .to_string();
    let new_filename = match extension {
        Some(ext) => format!("{}_{}.{}", stem, uuid_suffix, ext),
        None => format!("{}_{}", stem, uuid_suffix),
    };
    (downloads_dir.join(&new_filename), new_filename)
}

/// Control commands for active downloads (from frontend)
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "cmd")]
pub enum ControlCommand {
    Pause,
    Resume,
    Cancel,
    SpeedLimit { bytes_per_sec: u64 },
}

/// A single download item for new downloads
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NewDownloadItem {
    pub url: Url,
    /// Optional filename override - if None, extract from headers/URL
    pub filename: Option<String>,
    /// Optional queue ID to assign
    pub queue_id: Option<Uuid>,
}

/// Download request types from frontend
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum DownloadRequest {
    New(Vec<NewDownloadItem>),
    Resume(Vec<Uuid>),
}

pub struct DownloadManager {
    instances: Mutex<HashMap<Uuid, Vec<JoinHandle<()>>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(HashMap::new()),
        }
    }

    /// Handle incoming download requests
    pub async fn handle_request(
        &self,
        app: &AppHandle,
        request: DownloadRequest,
    ) -> Result<(), String> {
        let settings = settings::load_or_create(app);
        let client = client::create(&settings)?;
        let db = Database::initialize(app).map_err(|e| e.to_string())?;

        match request {
            DownloadRequest::New(items) => {
                self.handle_new_downloads(app, &db, &client, &settings, items)
                    .await
            }
            DownloadRequest::Resume(uuids) => {
                self.handle_resume_downloads(app, &db, &client, &settings, uuids)
                    .await
            }
        }
    }

    /// Handle new download requests
    async fn handle_new_downloads(
        &self,
        app: &AppHandle,
        db: &Database,
        client: &reqwest::Client,
        settings: &AppSettings,
        items: Vec<NewDownloadItem>,
    ) -> Result<(), String> {
        for item in items {
            let url = item.url;
            let custom_filename = item.filename;
            // Check max_concurrent limit (0 = unlimited)
            let max_concurrent = settings.download.max_concurrent;
            if max_concurrent > 0 && self.active_count() >= max_concurrent as usize {
                return Err(format!(
                    "Max concurrent downloads ({}) reached",
                    max_concurrent
                ));
            }

            let url_str = url.as_str();

            // Fetch headers
            eprintln!("  🔍 HEAD request to: {}", url_str);
            let response = client.head(url_str).send().await.map_err(|e| {
                eprintln!("  ❌ HEAD request failed: {}", e);
                e.to_string()
            })?;
            eprintln!("  ✅ HEAD response status: {}", response.status());
            let hdrs = response.headers();

            // Use custom filename if provided, otherwise extract from headers or URL
            let filename = custom_filename.unwrap_or_else(|| {
                headers::extract_filename(hdrs)
                    .unwrap_or_else(|| headers::extract_filename_from_url(url_str))
            });
            let size = headers::extract_content_length(hdrs).map(|s| s as i64);
            let etag = headers::extract_etag(hdrs);
            let last_modified = headers::extract_last_modified(hdrs);
            let resume_supported = headers::supports_resume(hdrs);

            let id = Uuid::now_v7();
            // Use configured download location, fallback to system downloads dir
            let downloads_dir = if settings.download.download_location.is_empty() {
                app.path()
                    .download_dir()
                    .map_err(|e| format!("Failed to get downloads directory: {}", e))?
            } else {
                PathBuf::from(&settings.download.download_location)
            };

            // Resolve destination conflict (auto-rename if file exists)
            let (resolved_path, filename) = resolve_destination_conflict(&downloads_dir, &filename);
            let destination = resolved_path.to_string_lossy().to_string();

            // Store to database
            db.insert_download(
                &id,
                url_str,
                &filename,
                &destination,
                size,
                hdrs.get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok()),
                etag.as_deref(),
                last_modified.as_deref(),
                resume_supported,
                item.queue_id.as_ref(),
            )
            .map_err(|e| {
                eprintln!("  ❌ DB insert failed: {}", e);
                e.to_string()
            })?;
            eprintln!("  ✅ DB insert success, id: {}", id);

            // Emit to frontend
            eprintln!("  📡 Emitting queue_download event for: {}", filename);
            let num_connections = if resume_supported {
                settings.download.num_threads
            } else {
                1
            };
            let _ = app.emit(
                "queue_download",
                json!({
                    "id": id.to_string(),
                    "url": url_str,
                    "filename": filename,
                    "size": size,
                    "destination": destination,
                    "resume_supported": resume_supported,
                    "num_connections": num_connections,
                    "status": "queued",
                    "queue_id": item.queue_id.map(|q| q.to_string()),
                }),
            );

            // Create and run download
            eprintln!("  🚀 Starting download, size: {:?}", size);
            let download = Download::new(size.unwrap_or(0) as usize, settings.download.num_threads);
            if let Err(e) = download.save(app, &id) {
                eprintln!("Failed to save download state: {}", e);
            }

            let handles = run_download(
                download,
                id,
                url_str.to_string(),
                destination,
                size.unwrap_or(0) as usize,
                app,
                settings,
            );
            eprintln!("  ✅ Download started with {} handles", handles.len());
            self.add_instance(id, handles);
        }
        Ok(())
    }

    /// Handle resume download requests
    async fn handle_resume_downloads(
        &self,
        app: &AppHandle,
        db: &Database,
        client: &reqwest::Client,
        settings: &AppSettings,
        uuids: Vec<Uuid>,
    ) -> Result<(), String> {
        let uuid_refs: Vec<&Uuid> = uuids.iter().collect();
        let downloads = db.get_resume_info(uuid_refs).map_err(|e| e.to_string())?;

        for download in downloads {
            let file_path = Path::new(&download.destination);
            let file_exists = file_path.exists();
            let current_file_size = if file_exists {
                std::fs::metadata(file_path)
                    .ok()
                    .map(|m| m.len() as i64)
                    .unwrap_or(0)
            } else {
                0
            };

            // Fetch current headers
            let response = match client.head(&download.url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("Failed to fetch headers for {}: {}", download.url, e);
                    continue;
                }
            };

            let hdrs = response.headers();
            let server_etag = headers::extract_etag(hdrs);
            let server_last_modified = headers::extract_last_modified(hdrs);
            let server_size = headers::extract_content_length(hdrs).map(|s| s as i64);
            let resume_supported = headers::supports_resume(hdrs);

            let needs_restart = !file_exists
                || (download.etag.is_some() && server_etag != download.etag)
                || (download.last_modified.is_some()
                    && server_last_modified != download.last_modified)
                || (download.size.is_some() && server_size != download.size);

            if needs_restart {
                let _ = db.update_headers(
                    &download.id,
                    server_size,
                    hdrs.get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok()),
                    server_etag.as_deref(),
                    server_last_modified.as_deref(),
                    resume_supported,
                );
                let _ = db.update_progress(&download.id, 0);
            } else {
                let _ = db.update_progress(&download.id, current_file_size);
            }

            let num_connections = if resume_supported {
                settings.download.num_threads
            } else {
                1
            };
            let _ = app.emit(
                "queue_download",
                json!({
                    "id": download.id,
                    "url": download.url,
                    "filename": download.filename,
                    "size": server_size,
                    "destination": download.destination,
                    "bytes_received": if needs_restart { 0 } else { current_file_size },
                    "resume_supported": resume_supported,
                    "num_connections": num_connections,
                    "status": "resuming",
                }),
            );

            let download_instance = match Download::load(app, &download.id) {
                Ok(instance) => instance,
                Err(e) => {
                    eprintln!(
                        "Failed to load download instance for {}: {}",
                        download.id, e
                    );
                    continue;
                }
            };

            let handles = run_download(
                download_instance,
                download.id,
                download.url.clone(),
                download.destination.clone(),
                server_size.unwrap_or(0) as usize,
                app,
                settings,
            );
            self.add_instance(download.id, handles);
        }
        Ok(())
    }

    pub fn add_instance(&self, id: Uuid, handles: Vec<JoinHandle<()>>) {
        self.instances.lock().unwrap().insert(id, handles);
    }

    /// Pause a download
    pub fn pause_instance(&self, id: &Uuid, app: &AppHandle) -> bool {
        if let Some(handles) = self.instances.lock().unwrap().remove(id) {
            for handle in handles {
                handle.abort();
            }
            let _ = app.emit(
                &format!("download_paused_{}", id),
                json!({"id": id.to_string()}),
            );
            return true;
        }
        false
    }

    /// Cancel a download
    pub fn cancel_instance(&self, id: &Uuid, app: &AppHandle) -> bool {
        if self.pause_instance(id, app) {
            let meta_path = Download::meta_path(app, id);
            let _ = std::fs::remove_file(meta_path);
            let _ = app.emit(
                &format!("download_cancelled_{}", id),
                json!({"id": id.to_string()}),
            );
            return true;
        }
        false
    }

    /// Check if download is active
    pub fn is_active(&self, id: &Uuid) -> bool {
        self.instances.lock().unwrap().contains_key(id)
    }

    /// Get count of active downloads
    pub fn active_count(&self) -> usize {
        self.instances.lock().unwrap().len()
    }

    /// Shutdown all active downloads
    pub fn shutdown_all(&self) {
        let mut instances = self.instances.lock().unwrap();
        for (_, handles) in instances.drain() {
            for handle in handles {
                handle.abort();
            }
        }
    }

    /// Start signal handler for graceful shutdown
    pub async fn start_signal_handler(&self) {
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(SignalKind::terminate())
                .expect("Failed to create SIGTERM handler");
            let mut sigint = signal::unix::signal(SignalKind::interrupt())
                .expect("Failed to create SIGINT handler");

            tokio::select! {
                _ = signal::ctrl_c() => {
                    eprintln!("Received Ctrl+C, shutting down...");
                    self.shutdown_all();
                },
                _ = sigterm.recv() => {
                    eprintln!("Received SIGTERM, shutting down...");
                    self.shutdown_all();
                },
                _ = sigint.recv() => {
                    eprintln!("Received SIGINT, shutting down...");
                    self.shutdown_all();
                },
            }
        }

        #[cfg(not(unix))]
        {
            if let Err(e) = signal::ctrl_c().await {
                eprintln!("Failed to listen for Ctrl+C: {}", e);
                return;
            }
            eprintln!("Received Ctrl+C, shutting down...");
            self.shutdown_all();
        }
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Tauri command wrapper for download requests
#[tauri::command]
pub async fn handle_download_request(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    request: DownloadRequest,
) -> Result<(), String> {
    eprintln!("📥 handle_download_request called: {:?}", request);
    let result = manager.handle_request(&app, request).await;
    if let Err(ref e) = result {
        eprintln!("❌ handle_download_request error: {}", e);
    }
    result
}

/// Tauri command for pausing a download
#[tauri::command]
pub fn pause_download(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    id: Uuid,
) -> bool {
    manager.pause_instance(&id, &app)
}

/// Tauri command for cancelling a download
#[tauri::command]
pub fn cancel_download(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    id: Uuid,
) -> bool {
    manager.cancel_instance(&id, &app)
}

/// Tauri command for checking if download is active
#[tauri::command]
pub fn is_download_active(manager: tauri::State<'_, DownloadManager>, id: Uuid) -> bool {
    manager.is_active(&id)
}

/// Tauri command for getting active download count
#[tauri::command]
pub fn active_download_count(manager: tauri::State<'_, DownloadManager>) -> usize {
    manager.active_count()
}

/// Tauri command for getting download history from database
#[tauri::command]
pub fn get_download_history(app: AppHandle) -> Result<Vec<crate::database::Download>, String> {
    let db = crate::database::Database::initialize(&app).map_err(|e| e.to_string())?;
    db.get_downloads().map_err(|e| e.to_string())
}

/// Tauri command for graceful shutdown - stops all downloads and exits
#[tauri::command]
pub async fn request_shutdown(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
) -> Result<(), String> {
    // 1. Stop all active downloads
    manager.shutdown_all();

    // 2. TODO: Save download state to metadata files for resume on next launch
    // This will be implemented with the .tur.meta file system

    // 3. Exit the application completely (bypasses window close event handler)
    app.exit(0);

    Ok(())
}

/// Tauri command for deleting a download from history
#[tauri::command]
pub fn delete_download_history(app: AppHandle, id: Uuid) -> Result<(), String> {
    let db = crate::database::Database::initialize(&app).map_err(|e| e.to_string())?;
    db.delete_download(&id).map_err(|e| e.to_string())
}

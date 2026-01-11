//! Download manager - handles active downloads and control commands

use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex}; // Added Arc here, Mutex was already present
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc; // Added this import
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken; // Added this import
use url::Url;
use uuid::Uuid;

#[cfg(unix)]
use tokio::signal::{self, unix::SignalKind};

use super::download::Download;
use super::headers;
use super::workers::run_download;
use crate::database::Database;
use crate::downloads::client;
use crate::settings::{self, AppSettings}; // Modified this import (removed `config::`)

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
use super::index::Index; // Need Index

pub struct DownloadInstance {
    pub handles: Vec<JoinHandle<()>>,
    pub cancel_token: CancellationToken,
    pub indices: Arc<Mutex<Vec<Arc<Index>>>>,
    pub queue_id: Option<Uuid>,
    pub bytes_downloaded: Arc<AtomicUsize>,
}

pub struct DownloadManager {
    instances: Mutex<HashMap<Uuid, DownloadInstance>>,
    completion_tx: mpsc::Sender<Uuid>,
    completion_rx: Mutex<Option<mpsc::Receiver<Uuid>>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            instances: Mutex::new(HashMap::new()),
            completion_tx: tx,
            completion_rx: Mutex::new(Some(rx)),
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

            // Probe request (warm up & metadata fetch)
            // Use GET without Range to mimic browser initial navigation
            tracing::debug!("  🔍 Probing (GET) to: {}", url_str);
            let response = client.get(url_str).send().await.map_err(|e| {
                tracing::error!("  ❌ Probe request failed: {}", e);
                e.to_string()
            })?;

            let status = response.status();
            tracing::debug!("  ✅ Probe response status: {}", status);

            if !status.is_success() {
                return Err(format!("Server returned error status: {}", status));
            }

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
                tracing::error!("  ❌ DB insert failed: {}", e);
                e.to_string()
            })?;
            tracing::debug!("  ✅ DB insert success, id: {}", id);

            // Emit to frontend
            tracing::info!("  📡 Emitting queue_download event for: {}", filename);
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
            tracing::info!("  🚀 Starting download, size: {:?}", size);
            let download = Download::new(size.unwrap_or(0) as usize, settings.download.num_threads);
            if let Err(e) = download.save(app, &id) {
                tracing::error!("Failed to save download state: {}", e);
            }

            let (handles, cancel_token, indices, bytes_downloaded) = run_download(
                download,
                id,
                url_str.to_string(),
                destination,
                size.unwrap_or(0) as usize,
                0, // New download starts at 0 bytes
                app,
                settings,
                self.completion_tx.clone(),
            );
            tracing::info!("  ✅ Download started with {} handles", handles.len());
            self.add_instance(
                id,
                handles,
                cancel_token,
                indices,
                item.queue_id,
                bytes_downloaded,
            );
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
                    tracing::error!("Failed to fetch headers for {}: {}", download.url, e);
                    continue;
                }
            };

            let hdrs = response.headers();
            let server_etag = headers::extract_etag(hdrs);
            let server_last_modified = headers::extract_last_modified(hdrs);
            let server_size = headers::extract_content_length(hdrs).map(|s| s as i64);
            let resume_supported = headers::supports_resume(hdrs);

            let needs_restart = headers::should_restart_download(
                file_exists,
                download.etag.as_deref(),
                server_etag.as_deref(),
                download.last_modified.as_deref(),
                server_last_modified.as_deref(),
                download.size,
                server_size,
            );

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
                    tracing::error!(
                        "Failed to load download instance for {}: {}",
                        download.id,
                        e
                    );
                    continue;
                }
            };

            let (handles, cancel_token, indices, bytes_downloaded) = run_download(
                download_instance,
                download.id,
                download.url.clone(),
                download.destination.clone(),
                server_size.unwrap_or(0) as usize,
                download.bytes_received as usize, // Resume from saved progress
                app,
                settings,
                self.completion_tx.clone(),
            );

            // Update status to in-progress (NULL)
            let _ = db.update_status(&download.id, None);

            self.add_instance(
                download.id,
                handles,
                cancel_token,
                indices,
                download.queue_id,
                bytes_downloaded,
            );
        }
        Ok(())
    }

    pub fn add_instance(
        &self,
        id: Uuid,
        handles: Vec<JoinHandle<()>>,
        cancel_token: CancellationToken,
        indices: Arc<Mutex<Vec<Arc<Index>>>>,
        queue_id: Option<Uuid>,
        bytes_downloaded: Arc<AtomicUsize>,
    ) {
        self.instances.lock().unwrap().insert(
            id,
            DownloadInstance {
                handles,
                cancel_token,
                indices,
                queue_id,
                bytes_downloaded,
            },
        );
    }

    /// Get the current in-memory bytes downloaded for a specific instance
    pub fn get_bytes_downloaded(&self, id: &Uuid) -> Option<usize> {
        self.instances.lock().unwrap().get(id).map(|instance| {
            instance
                .bytes_downloaded
                .load(std::sync::atomic::Ordering::Relaxed)
        })
    }

    /// Pause a download
    pub fn pause_instance(&self, id: &Uuid, app: &AppHandle, db: &Database) -> bool {
        // 1. Check if active (avoid work if not needed)
        if !self.is_active(id) {
            return false;
        }

        // 2. Get info from DB for reconstruction
        let download_info = match db.get_download_by_id(id) {
            Ok(Some(info)) => info,
            _ => return false,
        };

        // 3. Create State from LIVE progress (not stale DB value)
        let total_size = download_info.size.unwrap_or(0) as usize;
        // Use live bytes_downloaded from instance, fallback to DB if somehow missing
        let bytes_rec = self
            .get_bytes_downloaded(id)
            .unwrap_or(download_info.bytes_received as usize);

        // Load settings to get num_threads preference
        let settings = settings::load_or_create(app);
        let num_threads = settings.download.num_threads;

        // 4. Cancel and Abort Workers FIRST, then save their live state
        if let Some(instance) = self.instances.lock().unwrap().remove(id) {
            // Signal cancellation first
            instance.cancel_token.cancel();

            // Save LIVE state from the running instance (indices + worker states)
            // This preserves the exact progress including partial units
            {
                let indices_guard = instance.indices.lock().unwrap();
                let range: Vec<Arc<Index>> = indices_guard.clone();
                drop(indices_guard);

                // We need to reconstruct worker_states - but we don't have direct access
                // The progress emitter saves state periodically, so the meta file should be recent
                // For now, create a Download with the live indices
                let max_index = Download::get_index(total_size >> 23).unwrap_or(0);
                // Set steal_exhausted = false so workers can steal from existing indices on resume
                let coordinator = super::coordinator::Coordinator::from_parts(
                    max_index, max_index, 2, false, total_size
                );

                let download_state = Download {
                    coordinator,
                    range,
                    worker_states: (0..num_threads).map(|_| std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0))).collect(),
                };

                if let Err(e) = download_state.save(app, id) {
                    tracing::error!("Failed to save state on pause for {}: {}", id, e);
                }
            }

            // Abort handles as backup
            for handle in instance.handles {
                handle.abort();
            }

            // 5. Update DB status and progress
            if let Err(e) = db.update_status(id, Some("paused")) {
                tracing::error!("Failed to update DB status on pause for {}: {}", id, e);
            }
            // Also update bytes_received so DB reflects actual progress
            let _ = db.update_progress(id, bytes_rec as i64);

            // 6. Emit Event
            let _ = app.emit(
                &format!("download_paused_{}", id),
                json!({"id": id.to_string()}),
            );
            return true;
        }
        false
    }

    /// Cancel a download (stop workers, keep files for later restart)
    pub fn cancel_instance(&self, id: &Uuid, app: &AppHandle, db: &Database) -> bool {
        // 1. Get download info for state saving
        let download_info = match db.get_download_by_id(id) {
            Ok(Some(info)) => info,
            _ => return false,
        };

        let total_size = download_info.size.unwrap_or(0) as usize;
        let settings = settings::load_or_create(app);

        // 2. Stop workers and save LIVE state
        if let Some(instance) = self.instances.lock().unwrap().remove(id) {
            instance.cancel_token.cancel();

            // Save LIVE state from the running instance
            let bytes_rec = instance
                .bytes_downloaded
                .load(std::sync::atomic::Ordering::Relaxed);

            {
                let indices_guard = instance.indices.lock().unwrap();
                let range: Vec<Arc<Index>> = indices_guard.clone();
                drop(indices_guard);

                let max_index = Download::get_index(total_size >> 23).unwrap_or(0);
                // Set steal_exhausted = false so workers can steal from existing indices on resume
                let coordinator = super::coordinator::Coordinator::from_parts(
                    max_index, max_index, 2, false, total_size
                );

                let state = Download {
                    coordinator,
                    range,
                    worker_states: (0..settings.download.num_threads)
                        .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0)))
                        .collect(),
                };

                if let Err(e) = state.save(app, id) {
                    tracing::error!("Failed to save state on cancel for {}: {}", id, e);
                }
            }

            for handle in instance.handles {
                handle.abort();
            }

            // 3. Update DB status to 'cancelled' (keeps metadata + partial file)
            let _ = db.update_status(id, Some("cancelled"));
            let _ = db.update_progress(id, bytes_rec as i64);

            // 4. Emit event
            let _ = app.emit(
                &format!("download_cancelled_{}", id),
                json!({"id": id.to_string()}),
            );
            return true;
        }

        false
    }

    /// Delete a download completely (remove files, metadata, and DB record)
    pub fn delete_instance(&self, id: &Uuid, app: &AppHandle, db: &Database) -> bool {
        // 1. Stop workers if active
        if let Some(instance) = self.instances.lock().unwrap().remove(id) {
            instance.cancel_token.cancel();
            for handle in instance.handles {
                handle.abort();
            }
        }

        // 2. Delete metadata file
        let meta_path = Download::meta_path(app, id);
        if meta_path.exists() {
            let _ = std::fs::remove_file(meta_path);
        }

        // 3. Delete partial download file
        if let Ok(Some(info)) = db.get_download_by_id(id) {
            let path = PathBuf::from(&info.destination);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }

        // 4. Delete DB record
        let _ = db.delete_download(id);

        // 5. Emit event
        let _ = app.emit(
            &format!("download_deleted_{}", id),
            json!({"id": id.to_string()}),
        );
        true
    }

    /// Check if download is active
    pub fn is_active(&self, id: &Uuid) -> bool {
        self.instances.lock().unwrap().contains_key(id)
    }

    /// Get count of active downloads
    pub fn active_count(&self) -> usize {
        self.instances.lock().unwrap().len()
    }

    /// Shutdown all active downloads gracefully (save state, update DB, abort)
    pub fn shutdown_all_graceful(&self, app: &AppHandle, db: &Database) {
        let mut instances = self.instances.lock().unwrap();
        tracing::info!(
            "[tur] Shutting down {} active downloads...",
            instances.len()
        );

        for (id, instance) in instances.drain() {
            // 1. Save LIVE state from instance
            let live_bytes = instance
                .bytes_downloaded
                .load(std::sync::atomic::Ordering::Relaxed);

            if let Ok(Some(info)) = db.get_download_by_id(&id) {
                let total_size = info.size.unwrap_or(0) as usize;
                let settings = settings::load_or_create(app);

                // Save live indices
                let indices_guard = instance.indices.lock().unwrap();
                let range: Vec<Arc<Index>> = indices_guard.clone();
                drop(indices_guard);

                let max_index = Download::get_index(total_size >> 23).unwrap_or(0);
                // Set steal_exhausted = false so workers can steal from existing indices on resume
                let coordinator = super::coordinator::Coordinator::from_parts(
                    max_index, max_index, 2, false, total_size
                );

                let state = Download {
                    coordinator,
                    range,
                    worker_states: (0..settings.download.num_threads)
                        .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0)))
                        .collect(),
                };

                if let Err(e) = state.save(app, &id) {
                    tracing::error!("Failed to save state during shutdown for {}: {}", id, e);
                }

                if let Err(e) = db.update_status(&id, Some("paused")) {
                    tracing::error!("Failed to update status during shutdown for {}: {}", id, e);
                }
                let _ = db.update_progress(&id, live_bytes as i64);
            }

            // 2. Abort workers
            instance.cancel_token.cancel();
            for handle in instance.handles {
                handle.abort();
            }
        }
    }

    /// Basic shutdown (abort only)
    pub fn shutdown_all(&self) {
        let mut instances = self.instances.lock().unwrap();
        for (_, instance) in instances.drain() {
            instance.cancel_token.cancel();
            for handle in instance.handles {
                handle.abort();
            }
        }
    }

    /// Start signal handler for graceful shutdown
    pub async fn start_signal_handler(&self, app: AppHandle) {
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(SignalKind::terminate())
                .expect("Failed to create SIGTERM handler");
            let mut sigint = signal::unix::signal(SignalKind::interrupt())
                .expect("Failed to create SIGINT handler");

            tokio::select! {
                _ = signal::ctrl_c() => {
                    tracing::info!("Received Ctrl+C, shutting down...");
                },
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, shutting down...");
                },
                _ = sigint.recv() => {
                    tracing::info!("Received SIGINT, shutting down...");
                },
            }
        }

        #[cfg(not(unix))]
        {
            if let Err(e) = signal::ctrl_c().await {
                tracing::error!("Failed to listen for Ctrl+C: {}", e);
            } else {
                tracing::info!("Received Ctrl+C, shutting down...");
            }
        }

        // Execute graceful shutdown
        if let Ok(db) = Database::initialize(&app) {
            self.shutdown_all_graceful(&app, &db);
        } else {
            self.shutdown_all();
        }
        app.exit(0);
    }

    /// Start background task to process queue upon completion
    pub fn start_background_task(&self, app: AppHandle) {
        let mut rx_opt = self.completion_rx.lock().unwrap();
        if let Some(mut rx) = rx_opt.take() {
            let app_clone = app.clone();
            tokio::spawn(async move {
                tracing::info!("🔄 Queue processor started");
                // Initial check on startup
                if let Err(e) = Self::check_and_start_next(&app_clone).await {
                    tracing::error!("Failed to perform initial queue check: {}", e);
                }
                while let Some(completed_id) = rx.recv().await {
                    tracing::info!("✅ Download completed signal received: {}", completed_id);
                    // Trigger check for next download
                    if let Err(e) = Self::check_and_start_next(&app_clone).await {
                        tracing::error!("Failed to process queue: {}", e);
                    }
                }
            });
        }
    }

    pub fn get_active_download_indices(&self, id: &Uuid) -> Option<Vec<(usize, usize)>> {
        let instances = self.instances.lock().unwrap();
        if let Some(instance) = instances.get(id) {
            let indices = instance.indices.lock().unwrap();
            Some(
                indices
                    .iter()
                    .map(|idx| {
                        (
                            idx.start.load(std::sync::atomic::Ordering::Relaxed),
                            idx.end.load(std::sync::atomic::Ordering::Relaxed),
                        )
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    /// Check if queue logic needs to run (e.g. on startup or completion)
    async fn check_and_start_next(app: &AppHandle) -> Result<(), String> {
        let manager = app.state::<DownloadManager>();
        let db = Database::initialize(app).map_err(|e| e.to_string())?;

        // Load settings
        let settings = settings::load_or_create(app);

        let max_concurrent = settings.download.max_concurrent as usize;
        let active = manager.active_count();

        if max_concurrent > 0 && active >= max_concurrent {
            tracing::debug!(
                "Queue check: Max concurrent reached ({}/{})",
                active,
                max_concurrent
            );
            return Ok(());
        }

        let slots_available = if max_concurrent == 0 {
            5
        } else {
            max_concurrent - active
        };
        if slots_available == 0 {
            return Ok(());
        }

        // Find queued downloads (status = 'queued' or 'pending')
        // We need a DB query for this.
        let pending = db
            .get_queued_downloads(slots_available)
            .map_err(|e| e.to_string())?;

        if pending.is_empty() {
            tracing::debug!("Queue check: No pending downloads");
            return Ok(());
        }

        tracing::info!("🚀 Auto-starting {} pending downloads", pending.len());

        // Start them (logic similar to resume)
        // We can reuse handle_resume_downloads logic if we just pass IDs?
        // But 'queued' items might be 'New' (never started) or 'Resume' (interrupted).
        // Database `Download` struct handles both.
        // `handle_resume_downloads` loads from DB.

        let uuids: Vec<Uuid> = pending.iter().map(|d| d.id).collect();
        // Since we are inside async task, we can call handle_resume_downloads?
        // But handle_resume_downloads is on `&self`. `manager` is `State`.

        // We need HTTP client.
        let client = client::create(&settings)?;

        manager
            .handle_resume_downloads(app, &db, &client, &settings, uuids)
            .await?;

        Ok(())
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
    tracing::debug!("📥 handle_download_request called: {:?}", request);
    let result = manager.handle_request(&app, request).await;
    if let Err(ref e) = result {
        tracing::error!("❌ handle_download_request error: {}", e);
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
    let db = match Database::initialize(&app) {
        Ok(db) => db,
        Err(_) => return false,
    };
    manager.pause_instance(&id, &app, &db)
}

/// Tauri command for cancelling a download
#[tauri::command]
pub fn cancel_download(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    id: Uuid,
) -> bool {
    let db = match Database::initialize(&app) {
        Ok(db) => db,
        Err(_) => return false,
    };
    manager.cancel_instance(&id, &app, &db)
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
    // 1. Stop all active downloads gracefully
    if let Ok(db) = Database::initialize(&app) {
        manager.shutdown_all_graceful(&app, &db);
    } else {
        manager.shutdown_all();
    }

    // 2. Exit the application completely
    app.exit(0);

    Ok(())
}

/// Tauri command for deleting a download completely (files + DB record)
#[tauri::command]
pub fn delete_download(
    app: AppHandle,
    manager: tauri::State<'_, DownloadManager>,
    id: Uuid,
) -> bool {
    let db = match Database::initialize(&app) {
        Ok(db) => db,
        Err(_) => return false,
    };
    manager.delete_instance(&id, &app, &db)
}

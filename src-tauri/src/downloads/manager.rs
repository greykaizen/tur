//! Download manager - handles active downloads and control commands

use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc; // Added this import
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken; // Added this import
use url::Url;
use uuid::Uuid;

use super::download::Download;
use super::headers;
use super::index::Index;
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
/// Command types for the Manager Actor
pub enum ManagerCommand {
    Start {
        request: DownloadRequest,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    Pause {
        id: Uuid,
        reply_tx: oneshot::Sender<bool>,
    },
    Cancel {
        id: Uuid,
        reply_tx: oneshot::Sender<bool>,
    },
    Delete {
        id: Uuid,
        reply_tx: oneshot::Sender<bool>,
    },
    IsActive {
        id: Uuid,
        reply_tx: oneshot::Sender<bool>,
    },
    ActiveCount {
        reply_tx: oneshot::Sender<usize>,
    },
    DownloadCompleted {
        id: Uuid,
    },
    Shutdown {
        reply_tx: oneshot::Sender<()>,
    },
    CheckQueue,
}

#[derive(Debug)]
pub struct ActiveDownload {
    pub handles: Vec<JoinHandle<()>>,
    pub cancel_token: CancellationToken,
    pub indices: Arc<Vec<Arc<Index>>>,
    pub queue_id: Option<Uuid>,
    pub bytes_downloaded: Arc<AtomicUsize>,
}

impl ActiveDownload {
    pub fn bytes_downloaded(&self) -> usize {
        self.bytes_downloaded
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.cancel_token.cancel();
        for handle in &self.handles {
            handle.abort();
        }
    }

    pub fn snapshot_and_save(
        &self,
        id: &Uuid,
        app: &AppHandle,
        total_size: usize,
    ) -> Result<(), String> {
        // Create snapshot of indices for persistence
        let indices_snapshot: Vec<Arc<Index>> = self
            .indices
            .iter()
            .map(|idx| {
                let i = Index::new();
                i.start.store(
                    idx.start.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                i.end.store(
                    idx.end.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                i.state.store(
                    idx.state.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                Arc::new(i)
            })
            .collect();

        let max_index = Download::get_index(total_size >> 23).unwrap_or(0);
        // Set steal_exhausted = false so workers can steal from existing indices on resume
        let coordinator =
            super::coordinator::Coordinator::from_parts(max_index, max_index, 2, false, total_size);

        let download_state = Download {
            coordinator,
            indices: indices_snapshot,
        };

        download_state.save(app, id).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct ManagerHandle {
    tx: mpsc::Sender<ManagerCommand>,
}

impl ManagerHandle {
    pub async fn start(&self, request: DownloadRequest) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::Start { request, reply_tx })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn pause(&self, id: Uuid) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .send(ManagerCommand::Pause { id, reply_tx })
            .await
            .is_err()
        {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn cancel(&self, id: Uuid) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .send(ManagerCommand::Cancel { id, reply_tx })
            .await
            .is_err()
        {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn delete(&self, id: Uuid) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .send(ManagerCommand::Delete { id, reply_tx })
            .await
            .is_err()
        {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn is_active(&self, id: Uuid) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .send(ManagerCommand::IsActive { id, reply_tx })
            .await
            .is_err()
        {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn active_count(&self) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .send(ManagerCommand::ActiveCount { reply_tx })
            .await
            .is_err()
        {
            return 0;
        }
        reply_rx.await.unwrap_or(0)
    }

    pub async fn shutdown(&self) {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .tx
            .send(ManagerCommand::Shutdown { reply_tx })
            .await
            .is_ok()
        {
            let _ = reply_rx.await;
        }
    }

    pub fn download_completed(&self, id: Uuid) {
        let _ = self.tx.try_send(ManagerCommand::DownloadCompleted { id });
    }

    pub async fn check_queue(&self) {
        let _ = self.tx.send(ManagerCommand::CheckQueue).await;
    }

    pub async fn start_signal_handler(&self, app: AppHandle) {
        use tokio::signal;
        #[cfg(unix)]
        use tokio::signal::unix::SignalKind;

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

        self.shutdown().await;
        app.exit(0);
    }
}

struct ManagerActor {
    instances: HashMap<Uuid, ActiveDownload>,
    cmd_rx: mpsc::Receiver<ManagerCommand>,
    app: AppHandle,
    self_handle: ManagerHandle,
}

impl ManagerActor {
    async fn run(&mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                ManagerCommand::Start { request, reply_tx } => {
                    // Start logic is complex, handles it in run struct for now or separate method
                    // For now, let's keep it deferred or call a placeholder
                    // We need to move handle_request logic here.
                    // Let's implement handle_start separately.
                    let res = self.handle_start(request).await;
                    let _ = reply_tx.send(res);
                }
                ManagerCommand::Pause { id, reply_tx } => {
                    let _ = reply_tx.send(self.handle_pause(id));
                }
                ManagerCommand::Cancel { id, reply_tx } => {
                    let _ = reply_tx.send(self.handle_cancel(id));
                }
                ManagerCommand::Delete { id, reply_tx } => {
                    let _ = reply_tx.send(self.handle_delete(id));
                }
                ManagerCommand::IsActive { id, reply_tx } => {
                    let _ = reply_tx.send(self.instances.contains_key(&id));
                }
                ManagerCommand::ActiveCount { reply_tx } => {
                    let _ = reply_tx.send(self.instances.len());
                }
                ManagerCommand::DownloadCompleted { id } => {
                    if let Some(_instance) = self.instances.remove(&id) {
                        // Download finished, remove active instance.
                        // No need to stop or save, workers are done.
                        // Just cleanup entry.
                        tracing::info!("⬇️ Download {} finished, removed from manager", id);
                    }
                    // Trigger queue check
                    self.handle_check_queue().await;
                }
                ManagerCommand::CheckQueue => {
                    self.handle_check_queue().await;
                }
                ManagerCommand::Shutdown { reply_tx } => {
                    self.shutdown_all();
                    let _ = reply_tx.send(());
                    break;
                }
            }
        }
    }

    async fn handle_start(&mut self, request: DownloadRequest) -> Result<(), String> {
        let settings = settings::load_or_create(&self.app);
        let client = client::create(&settings)?;
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        match request {
            DownloadRequest::New(items) => {
                self.handle_new_downloads(&db, &client, &settings, items)
                    .await
            }
            DownloadRequest::Resume(uuids) => {
                self.handle_resume_downloads(&db, &client, &settings, uuids)
                    .await
            }
        }
    }

    async fn handle_new_downloads(
        &mut self,
        db: &Database,
        client: &reqwest::Client,
        settings: &AppSettings,
        items: Vec<NewDownloadItem>,
    ) -> Result<(), String> {
        for item in items {
            let url = item.url;
            let custom_filename = item.filename;
            let max_concurrent = settings.download.max_concurrent;
            if max_concurrent > 0 && self.instances.len() >= max_concurrent as usize {
                return Err(format!(
                    "Max concurrent downloads ({}) reached",
                    max_concurrent
                ));
            }

            let url_str = url.as_str();

            tracing::debug!("  🔍 Probing (GET) to: {}", url_str);
            let response = client.get(url_str).send().await.map_err(|e| {
                tracing::error!("  ❌ Probe request failed: {}", e);
                e.to_string()
            })?;

            let status = response.status();
            if !status.is_success() {
                return Err(format!("Server returned error status: {}", status));
            }

            let hdrs = response.headers();
            let filename = custom_filename.unwrap_or_else(|| {
                headers::extract_filename(hdrs)
                    .unwrap_or_else(|| headers::extract_filename_from_url(url_str))
            });
            let size = headers::extract_content_length(hdrs).map(|s| s as i64);
            let etag = headers::extract_etag(hdrs);
            let last_modified = headers::extract_last_modified(hdrs);
            let resume_supported = headers::supports_resume(hdrs);

            let id = Uuid::now_v7();
            let downloads_dir = if settings.download.download_location.is_empty() {
                self.app.path().download_dir().map_err(|e| e.to_string())?
            } else {
                PathBuf::from(&settings.download.download_location)
            };

            let (resolved_path, filename) = resolve_destination_conflict(&downloads_dir, &filename);
            let destination = resolved_path.to_string_lossy().to_string();

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
            .map_err(|e| e.to_string())?;

            let num_connections = if resume_supported {
                settings.download.num_threads
            } else {
                1
            };
            let _ = self.app.emit(
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

            let download = Download::new(size.unwrap_or(0) as usize, settings.download.num_threads);
            if let Err(e) = download.save(&self.app, &id) {
                tracing::error!("Failed to save download state: {}", e);
            }

            let (handles, cancel_token, indices, bytes_downloaded) = run_download(
                download,
                id,
                url_str.to_string(),
                destination,
                size.unwrap_or(0) as usize,
                0,
                &self.app,
                settings,
                self.self_handle.clone(),
            );

            self.instances.insert(
                id,
                ActiveDownload {
                    handles,
                    cancel_token,
                    indices,
                    queue_id: item.queue_id,
                    bytes_downloaded,
                },
            );
        }
        Ok(())
    }

    async fn handle_resume_downloads(
        &mut self,
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
                    .map(|m| m.len() as i64)
                    .unwrap_or(0)
            } else {
                0
            };

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
            let _ = self.app.emit(
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

            let download_instance = match Download::load(&self.app, &download.id) {
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
                download.bytes_received as usize,
                &self.app,
                settings,
                self.self_handle.clone(),
            );

            let _ = db.update_status(&download.id, None);

            self.instances.insert(
                download.id,
                ActiveDownload {
                    handles,
                    cancel_token,
                    indices,
                    queue_id: download.queue_id,
                    bytes_downloaded,
                },
            );
        }
        Ok(())
    }

    fn handle_pause(&mut self, id: Uuid) -> bool {
        if !self.instances.contains_key(&id) {
            return false;
        }

        let db = match Database::initialize(&self.app) {
            Ok(db) => db,
            Err(_) => return false,
        };

        let download_info = match db.get_download_by_id(&id) {
            Ok(Some(info)) => info,
            _ => return false,
        };

        if let Some(instance) = self.instances.remove(&id) {
            let total_size = download_info.size.unwrap_or(0) as usize;
            let bytes_rec = instance.bytes_downloaded();

            // Snapshot and Save
            if let Err(e) = instance.snapshot_and_save(&id, &self.app, total_size) {
                tracing::error!("Failed to save state on pause for {}: {}", id, e);
            }

            // Stop workers
            instance.stop();

            // Update DB
            let _ = db.update_status(&id, Some("paused"));
            let _ = db.update_progress(&id, bytes_rec as i64);

            // Emit Event
            let _ = self.app.emit(
                &format!("download_paused_{}", id),
                json!({"id": id.to_string()}),
            );
            return true;
        }
        false
    }

    fn handle_cancel(&mut self, id: Uuid) -> bool {
        if !self.instances.contains_key(&id) {
            return false;
        }

        let db = match Database::initialize(&self.app) {
            Ok(db) => db,
            Err(_) => return false,
        };

        let download_info = match db.get_download_by_id(&id) {
            Ok(Some(info)) => info,
            _ => return false,
        };

        if let Some(instance) = self.instances.remove(&id) {
            let total_size = download_info.size.unwrap_or(0) as usize;
            let bytes_rec = instance.bytes_downloaded();

            // Snapshot and Save
            if let Err(e) = instance.snapshot_and_save(&id, &self.app, total_size) {
                tracing::error!("Failed to save state on cancel for {}: {}", id, e);
            }

            // Stop workers
            instance.stop();

            // Update DB
            let _ = db.update_status(&id, Some("cancelled"));
            let _ = db.update_progress(&id, bytes_rec as i64);

            // Emit Event
            let _ = self.app.emit(
                &format!("download_cancelled_{}", id),
                json!({"id": id.to_string()}),
            );
            return true;
        }
        false
    }

    async fn handle_check_queue(&mut self) {
        let settings = settings::load_or_create(&self.app);
        let max_concurrent = settings.download.max_concurrent as usize;
        let active = self.instances.len();

        if max_concurrent > 0 && active >= max_concurrent {
            tracing::debug!(
                "Queue check: Max concurrent reached ({}/{})",
                active,
                max_concurrent
            );
            return;
        }

        let slots_available = if max_concurrent == 0 {
            5
        } else {
            max_concurrent - active
        };

        if slots_available == 0 {
            return;
        }

        let db = match Database::initialize(&self.app) {
            Ok(db) => db,
            Err(_) => return,
        };

        match db.get_queued_downloads(slots_available) {
            Ok(pending) => {
                if !pending.is_empty() {
                    tracing::info!("🚀 Auto-starting {} pending downloads", pending.len());
                    let uuids: Vec<Uuid> = pending.iter().map(|d| d.id).collect();

                    let client = match client::create(&settings) {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::error!("Failed to create client: {}", e);
                            return;
                        }
                    };

                    if let Err(e) = self
                        .handle_resume_downloads(&db, &client, &settings, uuids)
                        .await
                    {
                        tracing::error!("Failed to auto-resume downloads: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Queue check failed: {}", e);
            }
        }
    }

    fn handle_delete(&mut self, id: Uuid) -> bool {
        // Stop workers if active
        if let Some(instance) = self.instances.remove(&id) {
            instance.stop();
        }

        let db = match Database::initialize(&self.app) {
            Ok(db) => db,
            Err(_) => return false,
        };

        // Delete metadata file
        let meta_path = Download::meta_path(&self.app, &id);
        if meta_path.exists() {
            let _ = std::fs::remove_file(meta_path);
        }

        // Delete partial download file
        if let Ok(Some(info)) = db.get_download_by_id(&id) {
            let path = PathBuf::from(&info.destination);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }

        // Delete DB record
        let _ = db.delete_download(&id);

        // Emit event
        let _ = self.app.emit(
            &format!("download_deleted_{}", id),
            json!({"id": id.to_string()}),
        );
        true
    }

    fn shutdown_all(&mut self) {
        tracing::info!(
            "[tur] Shutting down {} active downloads...",
            self.instances.len()
        );

        let db = Database::initialize(&self.app).ok();

        for (id, instance) in self.instances.drain() {
            let live_bytes = instance.bytes_downloaded();

            if let Some(db) = &db {
                if let Ok(Some(info)) = db.get_download_by_id(&id) {
                    let total_size = info.size.unwrap_or(0) as usize;
                    if let Err(e) = instance.snapshot_and_save(&id, &self.app, total_size) {
                        tracing::error!("Failed to save state during shutdown for {}: {}", id, e);
                    }

                    let _ = db.update_status(&id, Some("paused"));
                    let _ = db.update_progress(&id, live_bytes as i64);
                }
            }

            instance.stop();
        }
    }
}

/// Spawn the ManagerActor and return a handle
pub fn spawn_manager(app: AppHandle) -> ManagerHandle {
    let (tx, rx) = mpsc::channel(100);
    let handle = ManagerHandle { tx: tx.clone() };

    let mut actor = ManagerActor {
        instances: HashMap::new(),
        cmd_rx: rx,
        app: app.clone(),
        self_handle: handle.clone(),
    };

    tauri::async_runtime::spawn(async move {
        actor.run().await;
    });

    handle
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Tauri command wrapper for download requests
#[tauri::command]
pub async fn handle_download_request(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    request: DownloadRequest,
) -> Result<(), String> {
    tracing::debug!("📥 handle_download_request called: {:?}", request);
    manager.start(request).await
}

/// Tauri command for pausing a download
#[tauri::command]
pub async fn pause_download(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.pause(id).await)
}

/// Tauri command for cancelling a download
#[tauri::command]
pub async fn cancel_download(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.cancel(id).await)
}

/// Tauri command for checking if download is active
#[tauri::command]
pub async fn is_download_active(
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.is_active(id).await)
}

/// Tauri command for getting active download count
#[tauri::command]
pub async fn active_download_count(
    manager: tauri::State<'_, ManagerHandle>,
) -> Result<usize, String> {
    Ok(manager.active_count().await)
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
    manager: tauri::State<'_, ManagerHandle>,
) -> Result<(), String> {
    // 1. Send shutdown command
    manager.shutdown().await;

    // 2. Exit the application completely
    app.exit(0);

    Ok(())
}

/// Tauri command for deleting a download completely (files + DB record)
#[tauri::command]
pub async fn delete_download(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.delete(id).await)
}

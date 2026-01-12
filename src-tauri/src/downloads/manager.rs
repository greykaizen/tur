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
use crate::database::{Database, Queue};
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

    // Queue Commands
    ReorderQueue {
        queue_id: Uuid,
        new_order: Vec<Uuid>,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    PauseQueue {
        queue_id: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    ResumeQueue {
        queue_id: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    CancelQueue {
        queue_id: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    AddToQueue {
        download_id: Uuid,
        queue_id: Uuid,
        position: Option<i32>,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    RemoveFromQueue {
        download_id: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    // Scheduling Commands
    SetSchedule {
        download_id: Uuid,
        scheduled_at: chrono::DateTime<chrono::Utc>,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    ClearSchedule {
        download_id: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    StartNow {
        download_id: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    SetQueueDependency {
        queue_id: Uuid,
        depends_on: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    ClearQueueDependency {
        queue_id: Uuid,
        reply_tx: oneshot::Sender<Result<(), String>>,
    },
    EvaluateSchedules,
    ApplyBandwidthLimit,
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

    // Queue Methods
    pub async fn reorder_queue(&self, queue_id: Uuid, new_order: Vec<Uuid>) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::ReorderQueue {
                queue_id,
                new_order,
                reply_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn pause_queue(&self, queue_id: Uuid) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::PauseQueue { queue_id, reply_tx })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn resume_queue(&self, queue_id: Uuid) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::ResumeQueue { queue_id, reply_tx })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn cancel_queue(&self, queue_id: Uuid) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::CancelQueue { queue_id, reply_tx })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn add_to_queue(
        &self,
        download_id: Uuid,
        queue_id: Uuid,
        position: Option<i32>,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::AddToQueue {
                download_id,
                queue_id,
                position,
                reply_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn remove_from_queue(&self, download_id: Uuid) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::RemoveFromQueue {
                download_id,
                reply_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    // Scheduling Methods
    pub async fn set_schedule(
        &self,
        download_id: Uuid,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::SetSchedule {
                download_id,
                scheduled_at,
                reply_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn clear_schedule(&self, download_id: Uuid) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::ClearSchedule {
                download_id,
                reply_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn start_now(&self, download_id: Uuid) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::StartNow {
                download_id,
                reply_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn set_queue_dependency(
        &self,
        queue_id: Uuid,
        depends_on: Uuid,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::SetQueueDependency {
                queue_id,
                depends_on,
                reply_tx,
            })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub async fn clear_queue_dependency(&self, queue_id: Uuid) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ManagerCommand::ClearQueueDependency { queue_id, reply_tx })
            .await
            .map_err(|e| e.to_string())?;
        reply_rx.await.map_err(|e| e.to_string())?
    }

    pub fn apply_bandwidth_limit(&self) {
        let _ = self.tx.try_send(ManagerCommand::ApplyBandwidthLimit);
    }

    pub fn trigger_schedule_evaluation(&self) {
        let _ = self.tx.try_send(ManagerCommand::EvaluateSchedules);
    }
}

struct ManagerActor {
    instances: HashMap<Uuid, ActiveDownload>,
    cmd_rx: mpsc::Receiver<ManagerCommand>,
    app: AppHandle,
    self_handle: ManagerHandle,
    boundary_timer_handle: Option<JoinHandle<()>>,
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
                    // Also reconcile queue if it was in one
                    let _ = self.handle_download_completed_hook(id).await;
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
                // Queue Commands
                ManagerCommand::ReorderQueue {
                    queue_id,
                    new_order,
                    reply_tx,
                } => {
                    let _ = reply_tx.send(self.handle_reorder_queue(queue_id, new_order).await);
                }
                ManagerCommand::PauseQueue { queue_id, reply_tx } => {
                    let _ = reply_tx.send(self.handle_pause_queue(queue_id).await);
                }
                ManagerCommand::ResumeQueue { queue_id, reply_tx } => {
                    let _ = reply_tx.send(self.handle_resume_queue(queue_id).await);
                }
                ManagerCommand::CancelQueue { queue_id, reply_tx } => {
                    let _ = reply_tx.send(self.handle_cancel_queue(queue_id).await);
                }
                ManagerCommand::AddToQueue {
                    download_id,
                    queue_id,
                    position,
                    reply_tx,
                } => {
                    let _ = reply_tx.send(
                        self.handle_add_to_queue(download_id, queue_id, position)
                            .await,
                    );
                }
                ManagerCommand::RemoveFromQueue {
                    download_id,
                    reply_tx,
                } => {
                    let _ = reply_tx.send(self.handle_remove_from_queue(download_id).await);
                }

                // Scheduling Commands
                ManagerCommand::SetSchedule {
                    download_id,
                    scheduled_at,
                    reply_tx,
                } => {
                    let db = Database::initialize(&self.app).map_err(|e| e.to_string());
                    match db {
                        Ok(db) => {
                            let res = db
                                .set_download_schedule(&download_id, &scheduled_at)
                                .map_err(|e| e.to_string());
                            if res.is_ok() {
                                // Trigger evaluation
                                self.evaluate_schedules().await.ok();
                            }
                            let _ = reply_tx.send(res);
                        }
                        Err(e) => {
                            let _ = reply_tx.send(Err(e));
                        }
                    }
                }
                ManagerCommand::ClearSchedule {
                    download_id,
                    reply_tx,
                } => {
                    let db = Database::initialize(&self.app).map_err(|e| e.to_string());
                    match db {
                        Ok(db) => {
                            let res = db
                                .clear_download_schedule(&download_id)
                                .map_err(|e| e.to_string());
                            if res.is_ok() {
                                self.evaluate_schedules().await.ok();
                            }
                            let _ = reply_tx.send(res);
                        }
                        Err(e) => {
                            let _ = reply_tx.send(Err(e));
                        }
                    }
                }
                ManagerCommand::StartNow {
                    download_id,
                    reply_tx,
                } => {
                    let _ = reply_tx.send(self.handle_start_now(download_id).await);
                }
                ManagerCommand::SetQueueDependency {
                    queue_id,
                    depends_on,
                    reply_tx,
                } => {
                    let _ =
                        reply_tx.send(self.handle_set_queue_dependency(queue_id, depends_on).await);
                }
                ManagerCommand::ClearQueueDependency { queue_id, reply_tx } => {
                    let db = Database::initialize(&self.app).map_err(|e| e.to_string());
                    match db {
                        Ok(db) => {
                            let res = db
                                .clear_queue_dependency(&queue_id)
                                .map_err(|e| e.to_string());
                            if res.is_ok() {
                                self.reconcile_queue(&queue_id).await.ok();
                            }
                            let _ = reply_tx.send(res);
                        }
                        Err(e) => {
                            let _ = reply_tx.send(Err(e));
                        }
                    }
                }
                ManagerCommand::EvaluateSchedules => {
                    if let Err(e) = self.evaluate_schedules().await {
                        tracing::error!("Error evaluating schedules: {}", e);
                    }
                }
                ManagerCommand::ApplyBandwidthLimit => {
                    let settings = settings::load_or_create(&self.app);
                    self.apply_bandwidth_limit(&settings).await;
                }
            }
        }
    }

    // --- Core Queue Logic ---

    fn get_active_zone_size(&self, queue: &Queue, queue_length: usize) -> usize {
        match queue.mode.as_str() {
            "sequential" => 1,
            "parallel" => queue.parallel_count as usize,
            "all_at_once" => queue_length,
            _ => 1, // default to sequential
        }
    }

    async fn reconcile_queue(&mut self, queue_id: &Uuid) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Get queue info
        let queue = db
            .get_queue_by_id(queue_id)
            .map_err(|e| e.to_string())?
            .ok_or("Queue not found")?;

        // If queue is paused, don't start anything
        if queue.status == "paused" {
            return Ok(());
        }

        // Get downloads in queue ordered by position
        let downloads = db
            .get_queue_downloads_ordered(queue_id)
            .map_err(|e| e.to_string())?;

        if downloads.is_empty() {
            // Queue is empty, mark as completed and delete
            db.delete_queue(queue_id).map_err(|e| e.to_string())?;
            let _ = self
                .app
                .emit("queue_completed", json!({"id": queue_id.to_string()}));
            return Ok(());
        }

        // Check if all downloads are completed

        // Spec says: "WHEN all downloads in a queue complete, THE Queue SHALL be automatically deleted"
        // Let's stick to spec, but maybe cancelled ones should count towards completion?
        // If all are completed or cancelled, queue is done.
        let all_done = downloads.iter().all(|d| {
            let s = d.status.as_deref();
            s == Some("completed") || s == Some("cancelled") || s == Some("failed")
        });

        if all_done {
            db.delete_queue(queue_id).map_err(|e| e.to_string())?;
            let _ = self
                .app
                .emit("queue_completed", json!({"id": queue_id.to_string()}));
            return Ok(());
        }

        let active_zone_size = self.get_active_zone_size(&queue, downloads.len());
        let settings = settings::load_or_create(&self.app);
        let client = client::create(&settings)?;

        for (index, download) in downloads.iter().enumerate() {
            let position = index + 1; // 1-based
            let should_be_active = position <= active_zone_size;
            let is_active = self.instances.contains_key(&download.id);
            let is_terminal = matches!(
                download.status.as_deref(),
                Some("completed") | Some("cancelled") | Some("failed")
            );

            if is_terminal {
                continue;
            }

            if should_be_active && !is_active {
                // Start this download
                self.start_single_download(&db, &client, &settings, download.id)
                    .await?;
            } else if !should_be_active && is_active {
                // Pause this download
                self.handle_pause(download.id);
            }
        }

        Ok(())
    }

    async fn start_single_download(
        &mut self,
        db: &Database,
        client: &reqwest::Client,
        settings: &AppSettings,
        download_id: Uuid,
    ) -> Result<(), String> {
        self.handle_resume_downloads(db, client, settings, vec![download_id])
            .await
    }

    // --- Scheduling System Logic ---

    async fn evaluate_schedules(&mut self) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;
        let now = chrono::Utc::now();
        let settings = settings::load_or_create(&self.app);

        // 1. Check download schedules
        let scheduled_downloads = db.get_scheduled_downloads().map_err(|e| e.to_string())?;

        for download in scheduled_downloads {
            if let Some(scheduled_at) = download.scheduled_at {
                if scheduled_at <= now {
                    // Schedule time reached or passed
                    if download.status.as_deref() == Some("scheduled") {
                        // Check if this is a missed schedule (app was closed)
                        // If it's more than 1 minute past, it's missed
                        let is_missed = scheduled_at < now - chrono::Duration::minutes(1);

                        if is_missed && settings.scheduler.missed_schedule_action == "ask_user" {
                            // Emit event for UI to handle
                            let _ = self.app.emit(
                                "missed_schedule",
                                json!({
                                    "download_id": download.id.to_string(),
                                    "scheduled_at": scheduled_at.to_rfc3339(),
                                    "filename": download.filename,
                                }),
                            );
                        } else {
                            // Start immediately
                            self.start_scheduled_download(&db, download.id).await?;
                        }
                    }
                }
            }
        }

        // 2. Check queue dependencies
        let queues_with_deps = db
            .get_queues_with_dependencies()
            .map_err(|e| e.to_string())?;

        for queue in queues_with_deps {
            if let Some(depends_on) = queue.depends_on {
                // Check if dependency is complete
                let dep_queue = db.get_queue_by_id(&depends_on).map_err(|e| e.to_string())?;

                match dep_queue {
                    None => {
                        // Dependency queue was deleted, clear dependency and start
                        db.clear_queue_dependency(&queue.id)
                            .map_err(|e| e.to_string())?;
                        self.reconcile_queue(&queue.id).await?;
                    }
                    Some(dep) => {
                        if dep.status == "completed" {
                            // Dependency complete, clear and start
                            db.clear_queue_dependency(&queue.id)
                                .map_err(|e| e.to_string())?;
                            self.reconcile_queue(&queue.id).await?;
                        }
                    }
                }
            }
        }

        // 3. Apply current bandwidth window
        self.apply_bandwidth_limit(&settings).await;

        // 4. Set next boundary timer
        self.set_next_boundary_timer(&db, &settings).await;

        Ok(())
    }

    async fn start_scheduled_download(
        &mut self,
        db: &Database,
        download_id: Uuid,
    ) -> Result<(), String> {
        // Clear the schedule
        db.clear_download_schedule(&download_id)
            .map_err(|e| e.to_string())?;

        // Update status from "scheduled" to "queued"
        db.update_download_status(&download_id, "queued")
            .map_err(|e| e.to_string())?;

        // Check if download is in a queue
        let download = db
            .get_download_by_id(&download_id)
            .map_err(|e| e.to_string())?
            .ok_or("Download not found")?;

        if let Some(queue_id) = download.queue_id {
            // Let queue reconciliation handle it
            self.reconcile_queue(&queue_id).await?;
        } else {
            // Start directly
            let settings = settings::load_or_create(&self.app);
            let client = client::create(&settings)?;
            self.handle_resume_downloads(db, &client, &settings, vec![download_id])
                .await?;
        }

        let _ = self.app.emit(
            "schedule_triggered",
            json!({
                "download_id": download_id.to_string(),
            }),
        );

        Ok(())
    }

    async fn set_next_boundary_timer(&mut self, db: &Database, settings: &AppSettings) {
        // Cancel existing timer
        if let Some(handle) = self.boundary_timer_handle.take() {
            handle.abort();
        }

        let now = chrono::Utc::now();
        let mut next_boundary: Option<chrono::DateTime<chrono::Utc>> = None;

        // 1. Find next scheduled download time
        if let Ok(scheduled) = db.get_scheduled_downloads() {
            for download in scheduled {
                if let Some(scheduled_at) = download.scheduled_at {
                    if scheduled_at > now {
                        next_boundary = Some(match next_boundary {
                            None => scheduled_at,
                            Some(existing) => existing.min(scheduled_at),
                        });
                    }
                }
            }
        }

        // 2. Find next bandwidth window boundary
        for window in &settings.scheduler.bandwidth_windows {
            if !window.enabled {
                continue;
            }

            let today = now.date_naive();

            // Parse window times (HH:MM format)
            if let (Ok(start), Ok(end)) = (
                chrono::NaiveTime::parse_from_str(&window.start_time, "%H:%M"),
                chrono::NaiveTime::parse_from_str(&window.end_time, "%H:%M"),
            ) {
                let start_dt = today.and_time(start).and_utc();
                let end_dt = today.and_time(end).and_utc();

                // Check if boundaries are in the future
                for boundary in [start_dt, end_dt] {
                    if boundary > now {
                        next_boundary = Some(match next_boundary {
                            None => boundary,
                            Some(existing) => existing.min(boundary),
                        });
                    }
                }

                // Also check tomorrow's start
                let tomorrow_start = (today + chrono::Duration::days(1))
                    .and_time(start)
                    .and_utc();
                next_boundary = Some(match next_boundary {
                    None => tomorrow_start,
                    Some(existing) => existing.min(tomorrow_start),
                });
            }
        }

        // Set timer if we have a next boundary
        if let Some(boundary) = next_boundary {
            let duration = (boundary - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(60));
            let tx = self.self_handle.tx.clone();

            // Log for debug
            tracing::debug!("Next schedule boundary in {:?}", duration);

            self.boundary_timer_handle = Some(tokio::spawn(async move {
                tokio::time::sleep(duration).await;
                let _ = tx.try_send(ManagerCommand::EvaluateSchedules);
            }));
        }
    }

    async fn apply_bandwidth_limit(&mut self, settings: &AppSettings) {
        let now = chrono::Utc::now();
        let current_time = now.time();

        let mut effective_limit = settings.download.speed_limit; // Global default

        for window in &settings.scheduler.bandwidth_windows {
            if !window.enabled {
                continue;
            }

            if let (Ok(start), Ok(end)) = (
                chrono::NaiveTime::parse_from_str(&window.start_time, "%H:%M"),
                chrono::NaiveTime::parse_from_str(&window.end_time, "%H:%M"),
            ) {
                let in_window = if start <= end {
                    // Normal window (e.g., 09:00 - 17:00)
                    current_time >= start && current_time < end
                } else {
                    // Overnight window (e.g., 22:00 - 06:00)
                    current_time >= start || current_time < end
                };

                if in_window && window.speed_limit > 0 {
                    // Use most restrictive (lowest non-zero)
                    if effective_limit == 0 || window.speed_limit < effective_limit {
                        effective_limit = window.speed_limit;
                    }
                }
            }
        }

        // Apply to all active downloads
        let _ = self.app.emit(
            "speed_limit_changed",
            json!({
                "limit": effective_limit,
            }),
        );

        tracing::debug!("Applied bandwidth limit: {} bytes/sec", effective_limit);
    }

    fn would_create_cycle(
        &self,
        db: &Database,
        queue_id: Uuid,
        depends_on: Uuid,
    ) -> Result<bool, String> {
        let mut visited = std::collections::HashSet::new();
        let mut current = depends_on;

        // Limit depth to avoid infinite loops in bad data
        let mut safe_guard = 0;

        while !visited.contains(&current) && safe_guard < 100 {
            safe_guard += 1;
            visited.insert(current);

            if current == queue_id {
                return Ok(true); // Cycle detected
            }

            // Get the dependency of current
            let queue = db.get_queue_by_id(&current).map_err(|e| e.to_string())?;
            match queue.and_then(|q| q.depends_on) {
                Some(next) => current = next,
                None => break,
            }
        }

        Ok(false)
    }

    // --- Command Handlers ---

    async fn handle_reorder_queue(
        &mut self,
        queue_id: Uuid,
        new_order: Vec<Uuid>,
    ) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Update positions in database
        for (index, download_id) in new_order.iter().enumerate() {
            let position = (index + 1) as i32;
            db.update_download_position(download_id, position)
                .map_err(|e| e.to_string())?;
        }

        // Reconcile to start/pause based on new positions
        self.reconcile_queue(&queue_id).await
    }

    async fn handle_pause_queue(&mut self, queue_id: Uuid) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Get all downloads in queue
        let downloads = db
            .get_queue_downloads_ordered(&queue_id)
            .map_err(|e| e.to_string())?;

        // Pause all active downloads in this queue
        for download in downloads {
            if self.instances.contains_key(&download.id) {
                self.handle_pause(download.id);
            }
        }

        // Update queue status
        db.update_queue_status(&queue_id, "paused")
            .map_err(|e| e.to_string())?;

        let _ = self
            .app
            .emit("queue_paused", json!({"id": queue_id.to_string()}));
        Ok(())
    }

    async fn handle_resume_queue(&mut self, queue_id: Uuid) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Update queue status first
        db.update_queue_status(&queue_id, "active")
            .map_err(|e| e.to_string())?;

        // Reconcile will start downloads in active zone
        self.reconcile_queue(&queue_id).await?;

        let _ = self
            .app
            .emit("queue_resumed", json!({"id": queue_id.to_string()}));
        Ok(())
    }

    async fn handle_cancel_queue(&mut self, queue_id: Uuid) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Get all downloads in queue
        let downloads = db
            .get_queue_downloads_ordered(&queue_id)
            .map_err(|e| e.to_string())?;

        // Cancel all active downloads
        for download in downloads {
            self.handle_cancel(download.id); // Cancel Logic handles DB update and event for download
                                             // Ensure it's removed from queue in DB too
            db.remove_download_from_queue(&download.id)
                .map_err(|e| e.to_string())?;
        }

        // Delete the queue
        db.delete_queue(&queue_id).map_err(|e| e.to_string())?;

        let _ = self
            .app
            .emit("queue_cancelled", json!({"id": queue_id.to_string()}));
        Ok(())
    }

    async fn handle_add_to_queue(
        &mut self,
        download_id: Uuid,
        queue_id: Uuid,
        position: Option<i32>,
    ) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Add to queue at position (or append)
        db.add_download_to_queue(&download_id, &queue_id, position)
            .map_err(|e| e.to_string())?;

        // Reconcile to handle if this affects active zone
        self.reconcile_queue(&queue_id).await
    }

    async fn handle_remove_from_queue(&mut self, download_id: Uuid) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Get queue_id before removing
        let download = db
            .get_download_by_id(&download_id)
            .map_err(|e| e.to_string())?
            .ok_or("Download not found")?;

        let queue_id = download.queue_id;

        // Remove from queue
        db.remove_download_from_queue(&download_id)
            .map_err(|e| e.to_string())?;

        // Reconcile the queue if it existed (to potentially start next item)
        if let Some(qid) = queue_id {
            self.reconcile_queue(&qid).await?;
        }

        Ok(())
    }

    async fn handle_start_now(&mut self, download_id: Uuid) -> Result<(), String> {
        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Clear any schedule
        db.clear_download_schedule(&download_id)
            .map_err(|e| e.to_string())?;

        // Get download info
        let _download = db
            .get_download_by_id(&download_id)
            .map_err(|e| e.to_string())?
            .ok_or("Download not found")?;

        // If in a queue with dependency, we still start it (override)
        // Update status to queued
        db.update_download_status(&download_id, "queued")
            .map_err(|e| e.to_string())?;

        // Start the download directly, bypassing queue checks
        let settings = settings::load_or_create(&self.app);
        let client = client::create(&settings)?;
        self.handle_resume_downloads(&db, &client, &settings, vec![download_id])
            .await?;

        Ok(())
    }

    async fn handle_set_queue_dependency(
        &mut self,
        queue_id: Uuid,
        depends_on: Uuid,
    ) -> Result<(), String> {
        if queue_id == depends_on {
            return Err("Queue cannot depend on itself".to_string());
        }

        let db = Database::initialize(&self.app).map_err(|e| e.to_string())?;

        // Check for circular dependency
        if self.would_create_cycle(&db, queue_id, depends_on)? {
            return Err("Circular dependency detected".to_string());
        }

        db.set_queue_dependency(&queue_id, &depends_on)
            .map_err(|e| e.to_string())?;

        let _ = self.app.emit(
            "queue_dependency_set",
            json!({
                "queue_id": queue_id.to_string(),
                "depends_on": depends_on.to_string(),
            }),
        );

        Ok(())
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

    async fn handle_download_completed_hook(&mut self, id: Uuid) {
        // We might not have the instance anymore, so check DB
        // Use match to ensure Result is consumed and dropped
        let db = match Database::initialize(&self.app) {
            Ok(db) => db,
            Err(_) => return,
        };

        let queue_id = match db.get_download_by_id(&id) {
            Ok(Some(download)) => download.queue_id,
            _ => None,
        };

        if let Some(qid) = queue_id {
            if let Err(e) = self.reconcile_queue(&qid).await {
                tracing::error!("Failed to reconcile queue after completion: {}", e);
            }
        }

        // Check for dependencies that might now be runnable
        if let Err(e) = self.evaluate_schedules().await {
            tracing::error!("Error evaluating schedules after completion: {}", e);
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
        boundary_timer_handle: None,
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

/// Tauri command for reordering a queue
#[tauri::command]
pub async fn reorder_queue(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
    new_order: Vec<Uuid>,
) -> Result<(), String> {
    manager.reorder_queue(queue_id, new_order).await
}

/// Tauri command for pausing a queue
#[tauri::command]
pub async fn pause_queue(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.pause_queue(queue_id).await
}

/// Tauri command for resuming a queue
#[tauri::command]
pub async fn resume_queue(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.resume_queue(queue_id).await
}

/// Tauri command for cancelling a queue
#[tauri::command]
pub async fn cancel_queue(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.cancel_queue(queue_id).await
}

/// Tauri command for adding a download to a queue (via manager for reconciliation)
#[tauri::command]
pub async fn manager_add_to_queue(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
    queue_id: Uuid,
    position: Option<i32>,
) -> Result<(), String> {
    manager.add_to_queue(download_id, queue_id, position).await
}

/// Tauri command for removing a download from a queue (via manager for reconciliation)
#[tauri::command]
pub async fn manager_remove_from_queue(
    _app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
) -> Result<(), String> {
    manager.remove_from_queue(download_id).await
}

/// Tauri command for scheduling a download
#[tauri::command]
pub async fn set_download_schedule(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
    scheduled_at: String, // RFC3339 string
) -> Result<(), String> {
    let dt = chrono::DateTime::parse_from_rfc3339(&scheduled_at)
        .map_err(|e| e.to_string())?
        .with_timezone(&chrono::Utc);
    manager.set_schedule(download_id, dt).await
}

/// Tauri command for clearing a schedule
#[tauri::command]
pub async fn clear_download_schedule(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
) -> Result<(), String> {
    manager.clear_schedule(download_id).await
}

/// Tauri command for manual start override
#[tauri::command]
pub async fn start_download_now(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
) -> Result<(), String> {
    manager.start_now(download_id).await
}

/// Tauri command for setting queue dependency
#[tauri::command]
pub async fn set_queue_dependency(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
    depends_on: Uuid,
) -> Result<(), String> {
    manager.set_queue_dependency(queue_id, depends_on).await
}

/// Tauri command for clearing queue dependency
#[tauri::command]
pub async fn clear_queue_dependency(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.clear_queue_dependency(queue_id).await
}

/// Tauri command for getting bandwidth windows
#[tauri::command]
pub fn get_bandwidth_windows(
    app: AppHandle,
) -> Result<Vec<crate::settings::config::BandwidthWindow>, String> {
    let settings = crate::settings::load_or_create(&app);
    Ok(settings.scheduler.bandwidth_windows)
}

/// Tauri command for setting bandwidth windows
#[tauri::command]
pub async fn set_bandwidth_windows(
    app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    windows: Vec<crate::settings::config::BandwidthWindow>,
) -> Result<(), String> {
    let mut settings = crate::settings::load_or_create(&app);
    settings.scheduler.bandwidth_windows = windows;
    crate::settings::save(&app, &settings).map_err(|e| e.to_string())?;

    // Trigger immediate re-evaluation of limits
    manager.apply_bandwidth_limit();

    Ok(())
}

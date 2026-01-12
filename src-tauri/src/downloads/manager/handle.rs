use chrono::{DateTime, Utc};
use tauri::{AppHandle, Listener};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::commands::{DownloadRequest, ManagerCommand};

/// Handle for communicating with the Manager Actor
#[derive(Clone)]
pub struct ManagerHandle {
    pub tx: mpsc::Sender<ManagerCommand>,
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
        if let Err(_) = self.tx.send(ManagerCommand::Pause { id, reply_tx }).await {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn cancel(&self, id: Uuid) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(_) = self.tx.send(ManagerCommand::Cancel { id, reply_tx }).await {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn delete(&self, id: Uuid) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(_) = self.tx.send(ManagerCommand::Delete { id, reply_tx }).await {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn is_active(&self, id: Uuid) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(_) = self
            .tx
            .send(ManagerCommand::IsActive { id, reply_tx })
            .await
        {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn active_count(&self) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(_) = self.tx.send(ManagerCommand::ActiveCount { reply_tx }).await {
            return 0;
        }
        reply_rx.await.unwrap_or(0)
    }

    pub async fn check_queue(&self) {
        let _ = self.tx.send(ManagerCommand::CheckQueue).await;
    }

    pub async fn shutdown(&self) {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Ok(_) = self.tx.send(ManagerCommand::Shutdown { reply_tx }).await {
            let _ = reply_rx.await;
        }
    }

    // Queue Operations

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

    // Scheduling Operations

    pub async fn set_schedule(
        &self,
        download_id: Uuid,
        scheduled_at: DateTime<Utc>,
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

    pub fn download_completed(&self, id: Uuid) {
        let _ = self.tx.try_send(ManagerCommand::DownloadCompleted { id });
    }

    pub async fn start_signal_handler(&self, app: AppHandle) {
        use tokio::signal;
        #[cfg(unix)]
        use tokio::signal::unix::SignalKind;

        let handle = self.clone();
        // Also listen for internal Tauri event if needed
        let _id = app.listen("request-shutdown", move |_| {
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                handle.shutdown().await;
            });
        });

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

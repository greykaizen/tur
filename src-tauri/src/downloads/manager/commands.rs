use chrono::{DateTime, Utc};
use tokio::sync::oneshot;
use url::Url;
use uuid::Uuid;

/// Control commands for active downloads (from frontend)
#[derive(Debug, Clone, serde::Deserialize)]
pub enum ControlCommand {
    Pause,
    Resume,
    Cancel,
}

/// Request for a new download item
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NewDownloadItem {
    pub url: Url,
    pub filename: Option<String>,
    pub queue_id: Option<Uuid>,
}

/// Enum representing the initial request to the manager
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum DownloadRequest {
    New(Vec<NewDownloadItem>),
    Resume(Vec<Uuid>),
}

/// Commands for the Manager Actor
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
    CheckQueue,
    Shutdown {
        reply_tx: oneshot::Sender<()>,
    },
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
    // Schedule Commands
    SetSchedule {
        download_id: Uuid,
        scheduled_at: DateTime<Utc>,
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

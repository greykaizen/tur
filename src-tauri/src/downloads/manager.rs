use tauri::AppHandle;
use uuid::Uuid;

pub mod actor;
pub mod commands;
pub mod handle;
pub mod state;
pub mod utils;

pub use self::actor::ManagerActor;
pub use self::commands::{ControlCommand, DownloadRequest, ManagerCommand, NewDownloadItem};
pub use self::handle::ManagerHandle;

use crate::settings;

/// Spawns the download manager actor
pub fn spawn_manager(app: &AppHandle) -> ManagerHandle {
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    let handle = ManagerHandle { tx: tx.clone() };
    let app_handle = app.clone();
    let self_handle = handle.clone();

    tokio::spawn(async move {
        let mut actor = ManagerActor {
            instances: std::collections::HashMap::new(),
            cmd_rx: rx,
            app: app_handle,
            self_handle,
            boundary_timer_handle: None,
        };
        actor.run().await;
    });

    handle
}

// ============================================================================
// Tauri Commands - Wrapper functions called by frontend
// ============================================================================

/// Tauri command for adding a download request
#[tauri::command]
pub async fn handle_download_request(
    manager: tauri::State<'_, ManagerHandle>,
    request: DownloadRequest,
) -> Result<(), String> {
    manager.start(request).await
}

/// Tauri command: pause download
#[tauri::command]
pub async fn pause_download(
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.pause(id).await)
}

/// Tauri command: cancel download
#[tauri::command]
pub async fn cancel_download(
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.cancel(id).await)
}

/// Tauri command: delete download
#[tauri::command]
pub async fn delete_download(
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.delete(id).await)
}

/// Tauri command: is active
#[tauri::command]
pub async fn is_download_active(
    manager: tauri::State<'_, ManagerHandle>,
    id: Uuid,
) -> Result<bool, String> {
    Ok(manager.is_active(id).await)
}

/// Tauri command: active count
#[tauri::command]
pub async fn active_download_count(
    manager: tauri::State<'_, ManagerHandle>,
) -> Result<usize, String> {
    Ok(manager.active_count().await)
}

/// Tauri command: history
#[tauri::command]
pub async fn get_download_history(
    app: AppHandle,
) -> Result<Vec<crate::database::Download>, String> {
    use crate::database::Database;
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    db.get_downloads().map_err(|e| e.to_string())
}

/// Tauri command: shutdown
#[tauri::command]
pub async fn request_shutdown(manager: tauri::State<'_, ManagerHandle>) -> Result<(), String> {
    manager.shutdown().await;
    Ok(())
}

/// Tauri command for adding a download (NewDownloadItem version)
#[tauri::command]
pub async fn manager_add_download(
    manager: tauri::State<'_, ManagerHandle>,
    items: Vec<NewDownloadItem>,
) -> Result<(), String> {
    manager.start(DownloadRequest::New(items)).await
}

// Queue Commands

#[tauri::command]
pub async fn reorder_queue(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
    new_order: Vec<Uuid>,
) -> Result<(), String> {
    manager.reorder_queue(queue_id, new_order).await
}

#[tauri::command]
pub async fn pause_queue(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.pause_queue(queue_id).await
}

#[tauri::command]
pub async fn resume_queue(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.resume_queue(queue_id).await
}

#[tauri::command]
pub async fn cancel_queue(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.cancel_queue(queue_id).await
}

// Manager-based helpers for queue items (add/remove)
// These replace direct DB manipulation from frontend if we want manager to know immediately.

#[tauri::command]
pub async fn manager_add_to_queue(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
    queue_id: Uuid,
    position: Option<i32>,
) -> Result<(), String> {
    manager.add_to_queue(download_id, queue_id, position).await
}

#[tauri::command]
pub async fn manager_remove_from_queue(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
) -> Result<(), String> {
    manager.remove_from_queue(download_id).await
}

// Scheduling Commands

#[tauri::command]
pub async fn set_download_schedule(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
    scheduled_at: String,
) -> Result<(), String> {
    let dt = chrono::DateTime::parse_from_rfc3339(&scheduled_at)
        .map_err(|e| format!("Invalid date format: {}", e))?
        .with_timezone(&chrono::Utc);
    manager.set_schedule(download_id, dt).await
}

#[tauri::command]
pub async fn clear_download_schedule(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
) -> Result<(), String> {
    manager.clear_schedule(download_id).await
}

#[tauri::command]
pub async fn start_download_now(
    manager: tauri::State<'_, ManagerHandle>,
    download_id: Uuid,
) -> Result<(), String> {
    manager.start_now(download_id).await
}

// Queue Dependency Commands

#[tauri::command]
pub async fn set_queue_dependency(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
    depends_on: Uuid,
) -> Result<(), String> {
    manager.set_queue_dependency(queue_id, depends_on).await
}

#[tauri::command]
pub async fn clear_queue_dependency(
    manager: tauri::State<'_, ManagerHandle>,
    queue_id: Uuid,
) -> Result<(), String> {
    manager.clear_queue_dependency(queue_id).await
}

// Bandwidth Windows

#[tauri::command]
pub async fn get_bandwidth_windows(
    app: AppHandle,
) -> Result<Vec<crate::settings::config::BandwidthWindow>, String> {
    let settings = settings::load_or_create(&app);
    Ok(settings.scheduler.bandwidth_windows)
}

#[tauri::command]
pub async fn set_bandwidth_windows(
    app: AppHandle,
    manager: tauri::State<'_, ManagerHandle>,
    windows: Vec<crate::settings::config::BandwidthWindow>,
) -> Result<(), String> {
    let mut settings = settings::load_or_create(&app);
    settings.scheduler.bandwidth_windows = windows;
    crate::settings::save(&app, &settings).map_err(|e| e.to_string())?;

    // Trigger re-evaluation of bandwidth limits immediately
    manager.apply_bandwidth_limit();
    // Also re-evaluate boundary timer
    manager.trigger_schedule_evaluation();

    Ok(())
}

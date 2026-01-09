use crate::database::{Database, Queue};
use tauri::AppHandle;
use uuid::Uuid;

#[tauri::command]
pub async fn create_queue(
    app: AppHandle,
    name: String,
    color: Option<String>,
    mode: String,
    parallel_count: i32,
) -> Result<Queue, String> {
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    db.create_queue(&name, color.as_deref(), &mode, parallel_count)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_queues(app: AppHandle) -> Result<Vec<Queue>, String> {
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    db.get_queues().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_queue(app: AppHandle, id: String) -> Result<(), String> {
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    db.delete_queue(&uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_queue(
    app: AppHandle,
    download_id: String,
    queue_id: String,
    position: Option<i32>,
) -> Result<(), String> {
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    let d_id = Uuid::parse_str(&download_id).map_err(|e| e.to_string())?;
    let q_id = Uuid::parse_str(&queue_id).map_err(|e| e.to_string())?;
    db.add_download_to_queue(&d_id, &q_id, position)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_queue(app: AppHandle, download_id: String) -> Result<(), String> {
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    let d_id = Uuid::parse_str(&download_id).map_err(|e| e.to_string())?;
    db.remove_download_from_queue(&d_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_queue_status(
    app: AppHandle,
    queue_id: String,
    status: String,
) -> Result<(), String> {
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    let uuid = Uuid::parse_str(&queue_id).map_err(|e| e.to_string())?;
    db.update_queue_status(&uuid, &status)
        .map_err(|e| e.to_string())
}

/// Check queue progression after a download completes
/// Returns IDs of downloads that should be started, and marks queue complete if done
#[tauri::command]
pub async fn check_queue_progression(
    app: AppHandle,
    queue_id: String,
) -> Result<Vec<String>, String> {
    let db = Database::initialize(&app).map_err(|e| e.to_string())?;
    let uuid = Uuid::parse_str(&queue_id).map_err(|e| e.to_string())?;

    // Get queue settings
    let queue = db
        .get_queue_by_id(&uuid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Queue not found".to_string())?;

    // Check if queue is already completed
    if queue.status == "completed" {
        return Ok(vec![]);
    }

    // Check if all downloads in queue are complete
    let all_complete = db.is_queue_complete(&uuid).map_err(|e| e.to_string())?;
    if all_complete {
        // Mark queue as completed
        db.update_queue_status(&uuid, "completed")
            .map_err(|e| e.to_string())?;
        return Ok(vec![]);
    }

    // Count currently active downloads in queue
    let active_count = db.count_active_in_queue(&uuid).map_err(|e| e.to_string())?;

    // Determine how many more to start based on mode
    let slots_available = match queue.mode.as_str() {
        "concurrent" => (queue.parallel_count - active_count).max(0) as usize,
        "sequential" | _ => {
            if active_count == 0 {
                1
            } else {
                0
            }
        }
    };

    if slots_available == 0 {
        return Ok(vec![]);
    }

    // Get pending downloads in queue
    let pending = db
        .get_pending_queue_downloads(&uuid)
        .map_err(|e| e.to_string())?;

    // Return IDs that should be started (up to slots_available)
    let to_start: Vec<String> = pending
        .into_iter()
        .take(slots_available)
        .map(|id| id.to_string())
        .collect();

    Ok(to_start)
}

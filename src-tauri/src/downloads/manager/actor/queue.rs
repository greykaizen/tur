use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::database::{Database, Queue};
use crate::downloads::client;
use crate::settings::{self, AppSettings};

use super::super::handle::ManagerHandle;
use super::super::state::ActiveDownload;
use super::actions;

fn get_active_zone_size(queue: &Queue, queue_length: usize) -> usize {
    match queue.mode.as_str() {
        "sequential" => 1,
        "parallel" => queue.parallel_count as usize,
        "all_at_once" => queue_length,
        _ => 1,
    }
}

pub async fn reconcile_queue(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    queue_id: &Uuid,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    let queue = db
        .get_queue_by_id(queue_id)
        .map_err(|e| e.to_string())?
        .ok_or("Queue not found")?;

    if queue.status == "paused" {
        return Ok(());
    }

    let downloads = db
        .get_queue_downloads_ordered(queue_id)
        .map_err(|e| e.to_string())?;

    if downloads.is_empty() {
        db.delete_queue(queue_id).map_err(|e| e.to_string())?;
        let _ = app.emit(
            "queue_completed",
            serde_json::json!({"id": queue_id.to_string()}),
        );
        return Ok(());
    }

    let all_done = downloads.iter().all(|d| {
        let s = d.status.as_deref();
        s == Some("completed") || s == Some("cancelled") || s == Some("failed")
    });

    if all_done {
        db.delete_queue(queue_id).map_err(|e| e.to_string())?;
        let _ = app.emit(
            "queue_completed",
            serde_json::json!({"id": queue_id.to_string()}),
        );
        return Ok(());
    }

    let active_zone_size = get_active_zone_size(&queue, downloads.len());
    let client = client::create(settings)?;

    for (index, download) in downloads.iter().enumerate() {
        let position = index + 1;
        let should_be_active = position <= active_zone_size;
        let is_active = instances.contains_key(&download.id);
        let is_terminal = matches!(
            download.status.as_deref(),
            Some("completed") | Some("cancelled") | Some("failed")
        );

        if is_terminal {
            continue;
        }

        if should_be_active && !is_active {
            // Start download
            actions::start_downloads(
                instances,
                db,
                &client,
                settings,
                vec![download.id],
                app,
                manager_handle.clone(),
            )
            .await?;
        } else if !should_be_active && is_active {
            // Pause download
            actions::pause_download(instances, download.id, app);
        }
    }

    Ok(())
}

pub fn would_create_cycle(db: &Database, queue_id: Uuid, depends_on: Uuid) -> Result<bool, String> {
    let mut visited = HashSet::new();
    let mut current = depends_on;
    let mut safe_guard = 0;

    while !visited.contains(&current) && safe_guard < 100 {
        safe_guard += 1;
        visited.insert(current);
        if current == queue_id {
            return Ok(true);
        }
        let queue = db.get_queue_by_id(&current).map_err(|e| e.to_string())?;
        match queue.and_then(|q| q.depends_on) {
            Some(next) => current = next,
            None => break,
        }
    }
    Ok(false)
}

// Queue Command Handlers

pub async fn handle_reorder_queue(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    queue_id: Uuid,
    new_order: Vec<Uuid>,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    for (index, download_id) in new_order.iter().enumerate() {
        let position = (index + 1) as i32;
        db.update_download_position(download_id, position)
            .map_err(|e| e.to_string())?;
    }
    reconcile_queue(instances, db, settings, &queue_id, app, manager_handle).await
}

pub async fn handle_pause_queue(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    queue_id: Uuid,
    app: &AppHandle,
) -> Result<(), String> {
    let downloads = db
        .get_queue_downloads_ordered(&queue_id)
        .map_err(|e| e.to_string())?;
    for download in downloads {
        if instances.contains_key(&download.id) {
            actions::pause_download(instances, download.id, app);
        }
    }
    db.update_queue_status(&queue_id, "paused")
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        "queue_paused",
        serde_json::json!({"id": queue_id.to_string()}),
    );
    Ok(())
}

pub async fn handle_resume_queue(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    queue_id: Uuid,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    db.update_queue_status(&queue_id, "active")
        .map_err(|e| e.to_string())?;
    reconcile_queue(instances, db, settings, &queue_id, app, manager_handle).await?;
    let _ = app.emit(
        "queue_resumed",
        serde_json::json!({"id": queue_id.to_string()}),
    );
    Ok(())
}

pub async fn handle_cancel_queue(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    queue_id: Uuid,
    app: &AppHandle,
) -> Result<(), String> {
    let downloads = db
        .get_queue_downloads_ordered(&queue_id)
        .map_err(|e| e.to_string())?;
    for download in downloads {
        actions::cancel_download(instances, download.id, app);
        db.remove_download_from_queue(&download.id)
            .map_err(|e| e.to_string())?;
    }
    db.delete_queue(&queue_id).map_err(|e| e.to_string())?;
    let _ = app.emit(
        "queue_cancelled",
        serde_json::json!({"id": queue_id.to_string()}),
    );
    Ok(())
}

pub async fn handle_add_to_queue(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    download_id: Uuid,
    queue_id: Uuid,
    position: Option<i32>,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    db.add_download_to_queue(&download_id, &queue_id, position)
        .map_err(|e| e.to_string())?;
    reconcile_queue(instances, db, settings, &queue_id, app, manager_handle).await
}

pub async fn handle_remove_from_queue(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    download_id: Uuid,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    let download = db
        .get_download_by_id(&download_id)
        .map_err(|e| e.to_string())?
        .ok_or("Download not found")?;
    let queue_id = download.queue_id;
    db.remove_download_from_queue(&download_id)
        .map_err(|e| e.to_string())?;
    if let Some(qid) = queue_id {
        reconcile_queue(instances, db, settings, &qid, app, manager_handle).await?;
    }
    Ok(())
}

pub async fn handle_set_queue_dependency(
    db: &Database,
    queue_id: Uuid,
    depends_on: Uuid,
    app: &AppHandle,
) -> Result<(), String> {
    if queue_id == depends_on {
        return Err("Queue cannot depend on itself".to_string());
    }
    if would_create_cycle(db, queue_id, depends_on)? {
        return Err("Circular dependency detected".to_string());
    }
    db.set_queue_dependency(&queue_id, &depends_on)
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        "queue_dependency_set",
        serde_json::json!({
            "queue_id": queue_id.to_string(),
            "depends_on": depends_on.to_string(),
        }),
    );
    Ok(())
}

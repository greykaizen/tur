use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use super::super::commands::{DownloadRequest, NewDownloadItem};
use super::super::handle::ManagerHandle;
use super::super::state::ActiveDownload;
use super::super::utils::resolve_destination_conflict;
use crate::database::{Database, Queue};
use crate::downloads::client;
use crate::downloads::download::Download;
use crate::downloads::headers;
use crate::downloads::workers::run_download;
use crate::settings::{self, AppSettings};

// Circular import resolution:
// We might need to import queue logic if handle_new_downloads calls it.
// For now, let's inject a callback or handle it in actor?
// No, let's try direct import if possible.
// use super::queue; // This might fail if queue doesn't exist yet.
// We will come back to handle_new_downloads's queue reconciliation call later
// or assume actor handles it.
// Actually allow handle_new_downloads to return a "Task" or "Effect"?
// Or just let it call queue::reconcile directly (cyclic deps are allowed).
// I will implement start/pause/cancel first.

pub async fn start_downloads(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    client: &reqwest::Client,
    settings: &AppSettings,
    uuids: Vec<Uuid>,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    let max_concurrent = settings.download.max_concurrent;
    for id in uuids {
        if instances.contains_key(&id) {
            continue;
        }
        if max_concurrent > 0 && instances.len() >= max_concurrent as usize {
            continue;
        }
        let download_info = db
            .get_download_by_id(&id)
            .map_err(|e| e.to_string())?
            .ok_or("Download not found")?;

        let path = PathBuf::from(&download_info.destination);
        let meta_path = Download::meta_path(app, &id);
        let total_size = download_info.size.unwrap_or(0) as usize;

        // Check if meta exists to resume, otherwise start fresh
        let download_state = if meta_path.exists() {
            match Download::load(app, &id) {
                Ok(d) => d,
                Err(_) => Download::new(total_size, 2),
            }
        } else {
            Download::new(total_size, 2)
        };

        // Initial bytes: 0 for new run, but if we have meta, we have indices.
        // run_download takes "initial_bytes".
        // If resuming, we rely on updated DB/Meta.
        // In original code: `run_download(..., 0, ...)` was used in `handle_resume_downloads`.
        // Wait, original passed 0?
        // Line 760: `0` passed as initial_bytes.
        // Yes. `Display` might look wrong initially?
        // `ActiveDownload` counts from that base?
        // Indices state tracks progress. `bytes_downloaded` atom accounts for *session* progress?
        // Or total?
        // `run_download` docs say "initial_bytes: usize".
        // Usage in `run_download`: `let bytes_downloaded = Arc::new(AtomicUsize::new(initial_bytes));`
        // If we pass 0, it starts at 0.
        // The *total* percentage is calculated via indices or explicit total?
        // Re-reading `run_download`: `multi::spawn_progress_emitter` aggregates from shared counter.
        // If we restart, shared counter is 0.
        // If we don't pass previous progress, UI starts at 0%?
        // `spawn_progress_emitter` reads `bytes_downloaded`.
        // If `indices` are partially done, `bytes_downloaded` should reflect that?
        // Original code passed 0. So I will pass 0 for now to match behavior.

        let (handles, cancel_token, indices, bytes_downloaded) = run_download(
            download_state,
            id,
            download_info.url,
            path.to_string_lossy().to_string(),
            total_size,
            0,
            app,
            settings,
            manager_handle.clone(),
        );

        let active = ActiveDownload {
            handles,
            cancel_token,
            indices,
            queue_id: download_info.queue_id,
            bytes_downloaded,
        };
        instances.insert(id, active);

        db.update_download_status(&id, "downloading")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn pause_download(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    id: Uuid,
    app: &AppHandle,
) -> bool {
    if let Some(instance) = instances.remove(&id) {
        let total_bytes = instance.bytes_downloaded();
        let _ = instance.snapshot_and_save(&id, app, total_bytes); // Save progress
        instance.stop();
        // Update DB status to paused
        if let Ok(db) = Database::initialize(app) {
            let _ = db.update_download_status(&id, "paused");
        }
        let _ = app.emit("download_paused", serde_json::json!({"id": id.to_string()}));
        true
    } else {
        false
    }
}

pub fn cancel_download(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    id: Uuid,
    app: &AppHandle,
) -> bool {
    if let Some(instance) = instances.remove(&id) {
        instance.stop();
        if let Ok(db) = Database::initialize(app) {
            let _ = db.update_download_status(&id, "cancelled");
        }
        let _ = app.emit(
            "download_cancelled",
            serde_json::json!({"id": id.to_string()}),
        );
        true
    } else {
        if let Ok(db) = Database::initialize(app) {
            let _ = db.update_download_status(&id, "cancelled");
        }
        false
    }
}

pub fn delete_download(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    id: Uuid,
    app: &AppHandle,
) -> bool {
    if let Some(instance) = instances.remove(&id) {
        instance.stop();
    }
    let db = match Database::initialize(app) {
        Ok(db) => db,
        Err(_) => return false,
    };
    let meta_path = Download::meta_path(app, &id);
    if meta_path.exists() {
        let _ = std::fs::remove_file(meta_path);
    }
    if let Ok(Some(info)) = db.get_download_by_id(&id) {
        let path = PathBuf::from(&info.destination);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        let _ = db.delete_download(&id);
    }
    let _ = app.emit(
        "download_deleted",
        serde_json::json!({"id": id.to_string()}),
    );
    true
}

// NOTE: handle_new_downloads requires queue logic.
// We will implement simpler new_download logic here that DOES NOT reconcile queue automatically.
// The caller (Actor) must handle queue reconciliation.
// Or we separate "add_download" from "start_download".

pub async fn add_new_downloads(
    db: &Database,
    client: &reqwest::Client,
    settings: &AppSettings,
    items: Vec<NewDownloadItem>,
    app: &AppHandle,
) -> Result<Vec<Uuid>, String> {
    let mut added_ids = Vec::new();

    for item in items {
        let url_str = item.url.as_str();

        // Fetch Metadata
        let response = client
            .get(url_str)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("Server returned error status: {}", status));
        }

        let hdrs = response.headers();
        let filename = item.filename.clone().unwrap_or_else(|| {
            headers::extract_filename(hdrs)
                .unwrap_or_else(|| headers::extract_filename_from_url(url_str))
        });
        let size = headers::extract_content_length(hdrs).map(|s| s as i64);
        let etag = headers::extract_etag(hdrs);
        let last_modified = headers::extract_last_modified(hdrs);
        let resume_supported = headers::supports_resume(hdrs);

        let id = Uuid::now_v7();
        let downloads_dir = if settings.download.download_location.is_empty() {
            app.path().download_dir().map_err(|e| e.to_string())?
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
            None,
            etag.as_deref(),
            last_modified.as_deref(),
            resume_supported,
            None,
        )
        .map_err(|e| e.to_string())?;

        if let Some(queue_id) = item.queue_id {
            db.add_download_to_queue(&id, &queue_id, None)
                .map_err(|e| e.to_string())?;
            let _ = app.emit(
                "download_created",
                serde_json::json!({
                    "id": id.to_string(),
                    "filename": filename,
                    "status": "queued",
                }),
            );
            // Caller must reconcile queue
        } else {
            let _ = app.emit(
                "download_created",
                serde_json::json!({
                    "id": id.to_string(),
                    "filename": filename,
                    "status": "pending",
                }),
            );
        }
        added_ids.push(id);
    }
    Ok(added_ids)
}

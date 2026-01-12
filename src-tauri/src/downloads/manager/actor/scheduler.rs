use std::collections::HashMap;
use tauri::{AppHandle, Emitter};
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::super::commands::ManagerCommand;
use super::super::handle::ManagerHandle;
use super::super::state::ActiveDownload;
use super::actions;
use super::queue;
use crate::database::Database;
use crate::settings::{self, AppSettings};

pub async fn evaluate_schedules(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    app: &AppHandle,
    manager_handle: ManagerHandle,
    boundary_timer_handle: &mut Option<JoinHandle<()>>,
) -> Result<(), String> {
    let now = chrono::Utc::now();
    let scheduled_downloads = db.get_scheduled_downloads().map_err(|e| e.to_string())?;

    for download in scheduled_downloads {
        if let Some(scheduled_at) = download.scheduled_at {
            if scheduled_at <= now {
                if download.status.as_deref() == Some("scheduled") {
                    let is_missed = scheduled_at < now - chrono::Duration::minutes(1);

                    if is_missed && settings.scheduler.missed_schedule_action == "ask_user" {
                        let _ = app.emit(
                            "missed_schedule",
                            serde_json::json!({
                                "download_id": download.id.to_string(),
                                "scheduled_at": scheduled_at.to_rfc3339(),
                                "filename": download.filename,
                            }),
                        );
                    } else {
                        start_scheduled_download(
                            instances,
                            db,
                            settings,
                            download.id,
                            app,
                            manager_handle.clone(),
                        )
                        .await?;
                    }
                }
            }
        }
    }

    let queues_with_deps = db
        .get_queues_with_dependencies()
        .map_err(|e| e.to_string())?;

    for queue_data in queues_with_deps {
        if let Some(depends_on) = queue_data.depends_on {
            let dep_queue = db.get_queue_by_id(&depends_on).map_err(|e| e.to_string())?;

            match dep_queue {
                None => {
                    db.clear_queue_dependency(&queue_data.id)
                        .map_err(|e| e.to_string())?;
                    queue::reconcile_queue(
                        instances,
                        db,
                        settings,
                        &queue_data.id,
                        app,
                        manager_handle.clone(),
                    )
                    .await?;
                }
                Some(dep) => {
                    if dep.status == "completed" {
                        db.clear_queue_dependency(&queue_data.id)
                            .map_err(|e| e.to_string())?;
                        queue::reconcile_queue(
                            instances,
                            db,
                            settings,
                            &queue_data.id,
                            app,
                            manager_handle.clone(),
                        )
                        .await?;
                    }
                }
            }
        }
    }

    apply_bandwidth_limit(app, settings).await;
    set_next_boundary_timer(db, settings, manager_handle.clone(), boundary_timer_handle).await;

    Ok(())
}

async fn start_scheduled_download(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    download_id: Uuid,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    db.clear_download_schedule(&download_id)
        .map_err(|e| e.to_string())?;
    db.update_download_status(&download_id, "queued")
        .map_err(|e| e.to_string())?;

    let download = db
        .get_download_by_id(&download_id)
        .map_err(|e| e.to_string())?
        .ok_or("Download not found")?;

    if let Some(queue_id) = download.queue_id {
        queue::reconcile_queue(
            instances,
            db,
            settings,
            &queue_id,
            app,
            manager_handle.clone(),
        )
        .await?;
    } else {
        use crate::downloads::client;
        let client = client::create(settings)?;
        actions::start_downloads(
            instances,
            db,
            &client,
            settings,
            vec![download_id],
            app,
            manager_handle.clone(),
        )
        .await?;
    }

    let _ = app.emit(
        "schedule_triggered",
        serde_json::json!({
            "download_id": download_id.to_string(),
        }),
    );
    Ok(())
}

pub async fn set_next_boundary_timer(
    db: &Database,
    settings: &AppSettings,
    manager_handle: ManagerHandle,
    boundary_timer_handle: &mut Option<JoinHandle<()>>,
) {
    if let Some(handle) = boundary_timer_handle.take() {
        handle.abort();
    }

    let now = chrono::Utc::now();
    let mut next_boundary: Option<chrono::DateTime<chrono::Utc>> = None;

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

    for window in &settings.scheduler.bandwidth_windows {
        if !window.enabled {
            continue;
        }
        let today = now.date_naive();
        if let (Ok(start), Ok(end)) = (
            chrono::NaiveTime::parse_from_str(&window.start_time, "%H:%M"),
            chrono::NaiveTime::parse_from_str(&window.end_time, "%H:%M"),
        ) {
            let start_dt = today.and_time(start).and_utc();
            let end_dt = today.and_time(end).and_utc();
            for boundary in [start_dt, end_dt] {
                if boundary > now {
                    next_boundary = Some(match next_boundary {
                        None => boundary,
                        Some(existing) => existing.min(boundary),
                    });
                }
            }
            let tomorrow_start = (today + chrono::Duration::days(1))
                .and_time(start)
                .and_utc();
            next_boundary = Some(match next_boundary {
                None => tomorrow_start,
                Some(existing) => existing.min(tomorrow_start),
            });
        }
    }

    if let Some(boundary) = next_boundary {
        let duration = (boundary - now)
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(60));
        let tx = manager_handle.tx.clone();
        tracing::debug!("Next schedule boundary in {:?}", duration);
        *boundary_timer_handle = Some(tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let _ = tx.try_send(ManagerCommand::EvaluateSchedules);
        }));
    }
}

pub async fn apply_bandwidth_limit(app: &AppHandle, settings: &AppSettings) {
    let now = chrono::Utc::now();
    let current_time = now.time();
    let mut effective_limit = settings.download.speed_limit;

    for window in &settings.scheduler.bandwidth_windows {
        if !window.enabled {
            continue;
        }
        if let (Ok(start), Ok(end)) = (
            chrono::NaiveTime::parse_from_str(&window.start_time, "%H:%M"),
            chrono::NaiveTime::parse_from_str(&window.end_time, "%H:%M"),
        ) {
            let in_window = if start <= end {
                current_time >= start && current_time < end
            } else {
                current_time >= start || current_time < end
            };

            if in_window && window.speed_limit > 0 {
                if effective_limit == 0 || window.speed_limit < effective_limit {
                    effective_limit = window.speed_limit;
                }
            }
        }
    }

    let _ = app.emit(
        "speed_limit_changed",
        serde_json::json!({
            "limit": effective_limit,
        }),
    );
    tracing::debug!("Applied bandwidth limit: {} bytes/sec", effective_limit);
}

pub async fn handle_start_now(
    instances: &mut HashMap<Uuid, ActiveDownload>,
    db: &Database,
    settings: &AppSettings,
    download_id: Uuid,
    app: &AppHandle,
    manager_handle: ManagerHandle,
) -> Result<(), String> {
    db.clear_download_schedule(&download_id)
        .map_err(|e| e.to_string())?;
    let _download = db
        .get_download_by_id(&download_id)
        .map_err(|e| e.to_string())?
        .ok_or("Download not found")?;
    db.update_download_status(&download_id, "queued")
        .map_err(|e| e.to_string())?;

    use crate::downloads::client;
    let client = client::create(settings)?;
    actions::start_downloads(
        instances,
        db,
        &client,
        settings,
        vec![download_id],
        app,
        manager_handle.clone(),
    )
    .await?;
    Ok(())
}

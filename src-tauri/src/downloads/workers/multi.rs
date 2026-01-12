use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::context::UnitCompletion;
use super::unit::download_unit;
use crate::downloads::coordinator::Coordinator;
use crate::downloads::download::Download;
use crate::downloads::index::Index;
use crate::downloads::manager::ManagerHandle;
use crate::downloads::work_request::{WorkRequest, WorkResponse};

pub fn run_multi_threaded<R: tauri::Runtime>(
    mut coordinator: Coordinator,
    indices: Arc<Vec<Arc<Index>>>,
    id: Uuid,
    url: String,
    destination: String,
    bytes_downloaded: Arc<AtomicUsize>,
    client: Arc<reqwest::Client>,
    num_threads: u8,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
    cancel_token: CancellationToken,
    handle: tauri::AppHandle<R>,
    unit_tx: mpsc::Sender<UnitCompletion>,
) -> Vec<JoinHandle<()>> {
    // Channel for worker -> coordinator
    let (tx, mut rx) = mpsc::channel::<WorkRequest>(num_threads as usize * 2);

    let mut handles = Vec::new();

    // Spawn coordinator task
    let coord_indices = indices.clone();
    handles.push(tokio::spawn(async move {
        // Coordinator logic is now single-threaded here, but verifies against atomic indices
        // handle_request does NOT modify indices directly, only reads states and potentially CAS for stealing
        // For new ranges, it updates its own range_byte state.
        while let Some(req) = rx.recv().await {
            let response = coordinator.handle_request(req.worker_id, &coord_indices);

            if let Err(_) = req.reply_tx.send(response) {
                tracing::warn!(
                    "Coordinator failed to send reply to worker {}",
                    req.worker_id
                );
            }
        }
    }));

    let per_worker_limit = if speed_limit > 0 {
        speed_limit / num_threads as u64
    } else {
        0
    };

    for worker_id in 0..(num_threads as usize) {
        let worker_tx = tx.clone();
        let worker_url = url.clone();
        let worker_dest = destination.clone();
        let worker_bytes = bytes_downloaded.clone();
        let worker_client = client.clone();
        let worker_token = cancel_token.clone();
        let worker_indices = indices.clone();
        let worker_handle = handle.clone();
        let worker_download_id = id.to_string();
        let worker_unit_tx = unit_tx.clone();

        handles.push(tokio::spawn(async move {
            // Open file handle once per worker for reuse
            let file_handle = std::fs::OpenOptions::new()
                .write(true)
                .open(&worker_dest)
                .ok();

            loop {
                // Check cancellation
                if worker_token.is_cancelled() {
                    break;
                }

                let (reply_tx, reply_rx) = oneshot::channel();

                // Send work request
                let request = WorkRequest {
                    worker_id,
                    reply_tx,
                };

                if worker_tx.send(request).await.is_err() {
                    break;
                }

                match reply_rx.await {
                    Ok(WorkResponse::Range(start, end)) => {
                        if worker_id < worker_indices.len() {
                            let my_index = &worker_indices[worker_id];
                            my_index.set_range(start, end);

                            process_assigned_work(
                                worker_id,
                                my_index.clone(),
                                file_handle.as_ref(),
                                &worker_client,
                                &worker_url,
                                &worker_bytes,
                                per_worker_limit,
                                retry_count,
                                retry_delay_ms,
                                &worker_token,
                                &worker_handle,
                                &worker_download_id,
                                &worker_unit_tx,
                                None,
                            )
                            .await;
                        }
                    }
                    Ok(WorkResponse::Stolen(start, end, victim_id)) => {
                        if worker_id < worker_indices.len() {
                            let my_index = &worker_indices[worker_id];
                            my_index.set_range(start, end);

                            process_assigned_work(
                                worker_id,
                                my_index.clone(),
                                file_handle.as_ref(),
                                &worker_client,
                                &worker_url,
                                &worker_bytes,
                                per_worker_limit,
                                retry_count,
                                retry_delay_ms,
                                &worker_token,
                                &worker_handle,
                                &worker_download_id,
                                &worker_unit_tx,
                                Some(victim_id),
                            )
                            .await;
                        }
                    }
                    Ok(WorkResponse::None) => {
                        break;
                    }
                    Err(_) => break,
                }
            } // End Loop

            // Emit worker completion event
            let _ = worker_handle.emit(
                "worker_complete",
                serde_json::json!({
                    "id": worker_download_id,
                    "worker_id": worker_id
                }),
            );
        }));
    }
    handles
}

async fn process_assigned_work<R: tauri::Runtime>(
    worker_id: usize,
    index: Arc<Index>,
    file_handle: Option<&std::fs::File>,
    client: &reqwest::Client,
    url: &str,
    bytes_counter: &Arc<AtomicUsize>,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
    cancel_token: &CancellationToken,
    handle: &tauri::AppHandle<R>,
    download_id: &str,
    unit_tx: &mpsc::Sender<UnitCompletion>,
    stealing_from: Option<usize>,
) {
    let mut current_unit = index.start.load(Ordering::Relaxed);

    while current_unit < index.end.load(Ordering::Relaxed) {
        if cancel_token.is_cancelled() {
            break;
        }

        if let Some(f) = file_handle {
            if let Ok(f_clone) = f.try_clone() {
                use crate::downloads::worker_context::WorkerContext;

                let context = WorkerContext::new(worker_id, f_clone, index.clone(), stealing_from);

                let success = download_unit(
                    client,
                    url,
                    context,
                    bytes_counter,
                    speed_limit,
                    retry_count,
                    retry_delay_ms,
                    cancel_token.clone(),
                    handle.clone(),
                    download_id.to_string(),
                    unit_tx.clone(),
                )
                .await;

                if !success {
                    break;
                }

                current_unit = index.start.load(Ordering::Relaxed);
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

/// Production-quality progress emitter with variance-adaptive EWMA
pub fn spawn_progress_emitter<R: tauri::Runtime>(
    indices: Arc<Vec<Arc<Index>>>, // Fixed size, read-only Vec of Shared Atomics
    total_size: usize,
    id: Uuid,
    destination: String,
    bytes_downloaded: Arc<AtomicUsize>,
    handle: tauri::AppHandle<R>,
    cancel_token: CancellationToken,
    completion_tx: ManagerHandle,
    mut unit_rx: mpsc::Receiver<UnitCompletion>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        const EMIT_INTERVAL_MS: u64 = 250;
        const SAVE_INTERVAL_S: u64 = 5; // Save every 5 seconds
        const WARM_UP_SAMPLES: usize = 3;
        const STALL_HOLD_MS: u64 = 800;
        const STALL_DECAY: f64 = 0.85;
        const ALPHA_MIN: f64 = 0.15;
        const ALPHA_MAX: f64 = 0.45;

        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(EMIT_INTERVAL_MS));
        let mut last_save = std::time::Instant::now();
        let mut last_bytes = 0usize;
        let mut smoothed_speed: f64 = 0.0;
        let mut stall_start: Option<std::time::Instant> = None;
        let mut last_tick = std::time::Instant::now();
        let mut speed_history: [f64; 5] = [0.0; 5];
        let mut history_idx = 0usize;
        let mut sample_count = 0usize;
        let mut pending_unit_save = false; // Flag for unit completion save

        // Helper closure to save state
        let save_state = |indices: &Arc<Vec<Arc<Index>>>,
                          handle: &tauri::AppHandle<R>,
                          id: &Uuid,
                          total: usize| {
            // Construct snapshot with Arc<Index>
            let snapshot_indices: Vec<Arc<Index>> = indices
                .iter()
                .map(|idx| {
                    let i = Index::new();
                    i.start
                        .store(idx.start.load(Ordering::Relaxed), Ordering::Relaxed);
                    i.end
                        .store(idx.end.load(Ordering::Relaxed), Ordering::Relaxed);
                    i.state
                        .store(idx.state.load(Ordering::Relaxed), Ordering::Relaxed);
                    Arc::new(i)
                })
                .collect();

            let max_index = Download::get_index(total >> 23).unwrap_or(0);
            let coordinator = Coordinator::from_parts(max_index, max_index, 2, true, total);

            let download = Download {
                coordinator,
                indices: snapshot_indices,
            };

            if let Err(e) = download.save(handle, id) {
                tracing::warn!("Failed to save download state: {}", e);
            } else {
                tracing::debug!("Meta file saved for download {}", id);
            }
        };

        loop {
            // Check cancellation first
            if cancel_token.is_cancelled() {
                break;
            }

            // Select on cancellation, interval, and unit completions
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                _ = interval.tick() => {}
                Some(unit_completion) = unit_rx.recv() => {
                    // Unit completed - mark for immediate save
                    // We can also use this to catch up the bytes_downloaded counter if needed
                    // but aggregating from atomic sum is safer.
                    pending_unit_save = true;
                    tracing::debug!(
                        "Worker {} completed unit {}",
                        unit_completion.worker_id,
                        unit_completion.unit_index
                    );
                }
            }

            // Handle unit completion save (immediate, not waiting for interval)
            if pending_unit_save {
                save_state(&indices, &handle, &id, total_size);
                last_save = std::time::Instant::now();
                pending_unit_save = false;
            }

            // Aggregate total bytes from all workers via indices
            // This is arguably more accurate than the shared counter which updates on stream chunks
            // But the shared counter is Atomic so it's fine.
            // Let's stick to the shared counter `bytes_downloaded` for speed calculation,
            // but we could cross-check with indices if we wanted.
            // Requirement 10.2 says "Aggregate bytes from all workers" for progress emitter.
            // Requirement 5.1 says "Sum bytes_downloaded from all WorkerContext" (deprecated).
            // The Architecture says "The progress emitter SHALL aggregate total bytes from shared AtomicUsize counter." (Req 8 in new specs)
            let downloaded = bytes_downloaded.load(Ordering::Relaxed);

            // Periodic Save
            if last_save.elapsed().as_secs() >= SAVE_INTERVAL_S {
                save_state(&indices, &handle, &id, total_size);
                // Also update DB progress for crash recovery
                if let Ok(db) = crate::database::Database::initialize(&handle) {
                    let _ = db.update_progress(&id, downloaded as i64);
                }
                last_save = std::time::Instant::now();
            }

            let now = std::time::Instant::now();
            let byte_delta = downloaded.saturating_sub(last_bytes);

            let elapsed_secs = last_tick.elapsed().as_secs_f64();
            last_tick = now;

            if byte_delta > 0 && elapsed_secs > 0.05 {
                let raw_speed = byte_delta as f64 / elapsed_secs;
                speed_history[history_idx] = raw_speed;
                history_idx = (history_idx + 1) % speed_history.len();
                sample_count = (sample_count + 1).min(speed_history.len());

                let alpha = if sample_count >= 2 {
                    let mean: f64 =
                        speed_history[..sample_count].iter().sum::<f64>() / sample_count as f64;
                    // Simplified variation calc
                    let variance: f64 = speed_history[..sample_count]
                        .iter()
                        .map(|s| (s - mean).powi(2))
                        .sum::<f64>()
                        / sample_count as f64;
                    let cv = if mean > 0.0 {
                        variance.sqrt() / mean
                    } else {
                        0.0
                    };
                    let cv = cv.clamp(0.0, 1.0);
                    ALPHA_MAX - (cv * (ALPHA_MAX - ALPHA_MIN))
                } else {
                    0.35
                };

                if sample_count <= 1 {
                    smoothed_speed = raw_speed;
                } else {
                    smoothed_speed = alpha * raw_speed + (1.0 - alpha) * smoothed_speed;
                }

                last_bytes = downloaded;
                stall_start = None;
            } else {
                let stall_duration = stall_start.get_or_insert(now).elapsed().as_millis() as u64;
                if stall_duration > STALL_HOLD_MS {
                    smoothed_speed *= STALL_DECAY;
                    if smoothed_speed < 100.0 {
                        smoothed_speed = 0.0;
                    }
                }
            }

            let display_speed = if sample_count >= WARM_UP_SAMPLES {
                smoothed_speed as usize
            } else {
                0
            };
            let percentage = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            let _ = handle.emit(
                "download_progress",
                serde_json::json!({
                    "id": id.to_string(),
                    "downloaded": downloaded,
                    "progress": percentage,
                    "speed": display_speed,
                }),
            );

            if downloaded >= total_size && total_size > 0 {
                // Final cleanup
                if let Ok(db) = crate::database::Database::initialize(&handle) {
                    let _ = db.mark_completed(&id);
                }
                let meta_path = Download::meta_path(&handle, &id);
                let _ = std::fs::remove_file(meta_path);

                // Trigger Check Queue via channel
                completion_tx.download_completed(id);

                let _ = handle.emit(
                    "download_complete",
                    serde_json::json!({
                        "id": id.to_string(),
                        "destination": destination,
                        "status": "completed",
                    }),
                );
                break;
            }
        }
    })
}

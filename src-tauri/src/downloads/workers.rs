//! Worker tasks and download execution logic

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::constants::RANGE;
use super::coordinator::Coordinator;
use super::download::Download;
use super::index::Index;
use super::manager::ManagerHandle;
use super::work_request::{WorkRequest, WorkResponse};
use crate::downloads::client;
use crate::settings::config::AppSettings;

/// Minimum units to steal from a worker

/// Unit completion signal for triggering meta saves
#[derive(Debug, Clone)]
pub struct UnitCompletion {
    pub worker_id: usize,
    pub unit_index: usize,
}

/// Start download execution - returns handles, cancellation token, and indices for tracking
pub fn run_download<R: tauri::Runtime>(
    download: Download,
    id: Uuid,
    url: String,
    destination: String,
    total_size: usize,
    initial_bytes: usize, // For resume: pass DB bytes_received; for new: pass 0
    handle: &tauri::AppHandle<R>,
    config: &AppSettings,
    completion_tx: ManagerHandle,
) -> (
    Vec<JoinHandle<()>>,
    CancellationToken,
    Arc<Vec<Arc<Index>>>,
    Arc<AtomicUsize>,
) {
    let mut handles = Vec::new();
    let cancel_token = CancellationToken::new();

    // Initialize indices list from download state (live progress tracking)
    // Fixed size Arc<Vec<Arc<Index>>> - extra Arc allows sharing individual indices
    let indices = Arc::new(download.indices);

    // Channel for unit completion signals (workers -> progress emitter for meta saves)
    let (unit_tx, unit_rx) = mpsc::channel::<UnitCompletion>(64);

    // Pre-allocate file
    if let Err(e) = preallocate_file(&destination, total_size) {
        tracing::error!("Failed to pre-allocate file: {}", e);
    }

    // Create shared HTTP client
    let shared_client = match client::create(config) {
        Ok(c) => Arc::new(c),
        Err(e) => {
            tracing::error!("Failed to create HTTP client: {}", e);
            let bytes = Arc::new(AtomicUsize::new(0));
            return (handles, cancel_token, indices, bytes);
        }
    };

    // Shared bytes counter - use the provided initial_bytes value
    // For new downloads: 0
    // For resume: DB bytes_received (accurate value saved on pause)
    let bytes_downloaded = Arc::new(AtomicUsize::new(initial_bytes));

    // Settings
    let speed_limit = config.download.speed_limit;
    let retry_count = config.network.retry_count;
    let retry_delay_ms = config.network.retry_delay_ms;
    let num_threads = config.download.num_threads;

    // Spawn progress emitter - pass shared references for live state
    handles.push(spawn_progress_emitter(
        indices.clone(),
        total_size,
        id,
        destination.clone(),
        bytes_downloaded.clone(),
        handle.clone(),
        cancel_token.clone(),
        completion_tx,
        unit_rx,
    ));

    // Check mode based on file size
    if total_size > RANGE[2].end << 20 {
        // Multi-threaded: coordinator task manages range distribution
        handles.extend(run_multi_threaded(
            download.coordinator,
            indices.clone(),
            id,
            url,
            destination,
            bytes_downloaded.clone(),
            shared_client,
            num_threads,
            speed_limit,
            retry_count,
            retry_delay_ms,
            cancel_token.clone(),
            handle.clone(),
            unit_tx,
        ));
    } else {
        // Single-threaded: simple streaming
        // Ensure at least one index exists (should allow for small files)
        if indices.is_empty() {
            // This case should be rare/impossible with fixed initialization unless 0 threads
            tracing::warn!("Download indices empty for single threaded run");
        }

        handles.push(run_single_threaded(
            url,
            destination,
            bytes_downloaded.clone(),
            shared_client,
            speed_limit,
            retry_count,
            retry_delay_ms,
            cancel_token.clone(),
            indices.clone(),
            handle.clone(),
            id.to_string(),
            unit_tx,
        ));
    }

    (handles, cancel_token, indices, bytes_downloaded)
}

/// Production-quality progress emitter with variance-adaptive EWMA
fn spawn_progress_emitter<R: tauri::Runtime>(
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

        let mut interval = tokio::time::interval(Duration::from_millis(EMIT_INTERVAL_MS));
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

/// Multi-threaded download with coordinator task and channel communication
fn run_multi_threaded<R: tauri::Runtime>(
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

fn run_single_threaded<R: tauri::Runtime>(
    url: String,
    destination: String,
    bytes_downloaded: Arc<AtomicUsize>,
    client: Arc<reqwest::Client>,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
    cancel_token: CancellationToken,
    indices: Arc<Vec<Arc<Index>>>,
    handle: tauri::AppHandle<R>,
    id: String,
    unit_tx: mpsc::Sender<UnitCompletion>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Single threaded: just take index 0
        if let Some(idx) = indices.first() {
            let limit = idx.end.load(Ordering::Relaxed);
            let mut current = idx.start.load(Ordering::Relaxed);

            // Single threaded loop units
            let file_handle = std::fs::OpenOptions::new()
                .write(true)
                .open(&destination)
                .ok();

            while current < limit {
                if cancel_token.is_cancelled() {
                    break;
                }

                if let Some(ref f) = file_handle {
                    if let Ok(f_clone) = f.try_clone() {
                        use crate::downloads::worker_context::WorkerContext;

                        let context = WorkerContext::new(0, f_clone, idx.clone(), None);

                        let success = download_unit(
                            &client,
                            &url,
                            context,
                            &bytes_downloaded,
                            speed_limit,
                            retry_count,
                            retry_delay_ms,
                            cancel_token.clone(),
                            handle.clone(),
                            id.clone(),
                            unit_tx.clone(),
                        )
                        .await;

                        if !success {
                            break;
                        }

                        current = idx.start.load(Ordering::Relaxed);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    })
}

async fn download_unit<R: tauri::Runtime>(
    client: &reqwest::Client,
    url: &str,
    context: super::worker_context::WorkerContext, // Takes ownership (one unit lifecycle)
    bytes_counter: &Arc<AtomicUsize>,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
    cancel_token: CancellationToken,
    handle: tauri::AppHandle<R>,
    download_id: String,                   // For events
    unit_tx: mpsc::Sender<UnitCompletion>, // Signal unit completion for meta save
) -> bool {
    let mut retries = 0u8;
    let unit_index = context.index.start.load(Ordering::Relaxed);
    let start_byte = unit_index << 23;
    let end_byte = start_byte + (1 << 23) - 1;
    let worker_id = context.worker_id;

    // Resume Logic: Find first unset bit to determine resume position
    // trailing_ones() gives us the first gap - if bits are 00001011, we resume at bit 2
    // This is correct because we download sequentially within a unit
    let state_bits = context.index.state.load(Ordering::Relaxed);
    let first_unset_bit = state_bits.trailing_ones() as usize;
    let resume_offset = first_unset_bit << 20; // MB to Bytes

    // Update context to reflect skipped bytes
    context
        .bytes_in_unit
        .store(resume_offset, Ordering::Relaxed);

    // If fully complete (or somehow overshot), finish immediately
    if state_bits == 0xFF || resume_offset >= (1 << 23) {
        context.reset_unit();
        // Signal unit completion
        let _ = unit_tx.try_send(UnitCompletion {
            worker_id,
            unit_index,
        });
        return true;
    }

    let actual_start_byte = start_byte + resume_offset;
    // Safety clamp (though logic above handles it)
    if actual_start_byte > end_byte {
        context.reset_unit();
        return true;
    }

    loop {
        if cancel_token.is_cancelled() {
            return false;
        }

        let mut req = client.get(url);

        // Auto-Referer: Use origin as referer to bypass hotlink protection
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                let scheme = parsed.scheme();
                let referer = format!("{}://{}/", scheme, host);
                req = req.header("Referer", referer);
                // Adjust Sec-Fetch-Site since we are faking internal nav
                req = req.header("Sec-Fetch-Site", "same-origin");
            }
        }

        // Resume from actual_start_byte
        req = req.header("Range", format!("bytes={}-{}", actual_start_byte, end_byte));

        // Select for cancellation during request
        let response_future = req.send();
        let response = tokio::select! {
            _ = cancel_token.cancelled() => return false,
            res = response_future => match res {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Request failed: {}", e);
                    if retries < retry_count {
                        retries += 1;
                        tokio::time::sleep(Duration::from_millis(exponential_backoff(retries, retry_delay_ms))).await;
                        continue;
                    }
                    return false;
                }
            }
        };

        let status = response.status();
        let headers = response.headers();

        tracing::debug!(
            "Worker {} req {} response: {} (Range: {}-{})",
            context.worker_id,
            retries,
            status,
            actual_start_byte,
            end_byte
        );

        if status == reqwest::StatusCode::OK {
            // Server ignored Range header
            if actual_start_byte > 0 {
                // We requested a partial range but got the whole file.
                // We can't efficienty resume or split.
                tracing::error!(
                     "Worker {} requested range {}-{} but server sent 200 OK (ignored range). Aborting unit.",
                     context.worker_id,
                     actual_start_byte,
                     end_byte
                 );
                return false;
            }
            // If actual_start_byte == 0, we can accept it (it's arguably the "first" chunk),
            // but we must be careful not to read past end_byte if we only wanted a slice.
            // However, Response::bytes_stream() will give us everything.
            // Ideally we should limit the stream, but for now we'll accept it and let the loop below
            // just read what it needs. A "Take" adapter would be better.
            tracing::info!(
                "Worker {} got 200 OK for start=0. Proceeding.",
                context.worker_id
            );
        } else if status == reqwest::StatusCode::PARTIAL_CONTENT {
            // Validate Content-Range
            if let Some(cr) = headers
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
            {
                // Expected format: "bytes <start>-<end>/<total>"
                // We just check if it starts with "bytes <actual_start_byte>-"
                let expected_prefix = format!("bytes {}-", actual_start_byte);
                if !cr.starts_with(&expected_prefix) {
                    tracing::error!(
                        "Worker {} Content-Range mismatch. Requested start {}, got {}",
                        context.worker_id,
                        actual_start_byte,
                        cr
                    );
                    return false;
                }
            }
        } else if status == reqwest::StatusCode::FORBIDDEN
            || status == reqwest::StatusCode::UNAUTHORIZED
        {
            // 403/401: Fatal Auth Error
            tracing::error!("Worker {} Fatal Auth Error: {}", context.worker_id, status);
            let _ = handle.emit(
                "download_error",
                serde_json::json!({
                    "id": download_id,
                    "worker_id": context.worker_id,
                    "code": status.as_u16(),
                    "message": format!("Authentication failed: {}", status),
                    "fatal": true
                }),
            );
            return false;
        } else if status == reqwest::StatusCode::SERVICE_UNAVAILABLE
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            // 503/429: Server busy, retry
            tracing::warn!("Worker {} Server Busy: {}", context.worker_id, status);
            // Let retry logic handle it
        } else {
            tracing::error!("Unexpected status: {}", status);
            // Treat as retryable generic error unless retries exhausted
        }

        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            if retries < retry_count {
                retries += 1;
                tokio::time::sleep(Duration::from_millis(exponential_backoff(
                    retries,
                    retry_delay_ms,
                )))
                .await;
                continue;
            } else {
                // Retries exhausted
                let _ = handle.emit(
                    "download_error",
                    serde_json::json!({
                        "id": download_id,
                        "worker_id": context.worker_id,
                        "code": status.as_u16(),
                        "message": format!("Request failed after retries: {}", status),
                        "fatal": true
                    }),
                );
                return false;
            }
        }

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let mut offset = actual_start_byte; // Resume from actual position

        let mut last_throttle = std::time::Instant::now();
        let mut bytes_this_second = 0u64;

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => return false,
                chunk_opt = stream.next() => {
                    match chunk_opt {
                        Some(Ok(bytes)) => {
                            let bytes_len = bytes.len();
                            let write_offset = offset as u64;
                            let bytes_clone = bytes.to_vec();

                            // WorkerContext owns the file handle. Clone it for blocking task.
                            // But File is not cloneable easily without try_clone, and context owns it.
                            // We construct context by passing File.
                            // context.file.try_clone().
                            let file_handle_op = context.file.try_clone().ok();

                            let _ = tokio::task::spawn_blocking(move || {
                                use std::io::{Seek, Write};
                                if let Some(mut f) = file_handle_op {
                                    if let Err(e) = f.seek(std::io::SeekFrom::Start(write_offset)) {
                                        tracing::error!("File seek error: {}", e);
                                    }
                                    if let Err(e) = f.write_all(&bytes_clone) {
                                        tracing::error!("File write error: {}", e);
                                    }
                                    // fsync every 1MB (when we're about to flip a bit)
                                    // This ensures data is on disk before we mark it complete
                                    if (write_offset as usize + bytes_clone.len()) >> 20 > (write_offset as usize) >> 20 {
                                        if let Err(e) = f.sync_data() {
                                            tracing::warn!("File sync error: {}", e);
                                        }
                                    }
                                }
                            }).await;

                            offset += bytes_len;
                            bytes_counter.fetch_add(bytes_len, Ordering::Relaxed);

                            // 1MB Bit tracking and event emission
                            let stored = context.bytes_in_unit.fetch_add(bytes_len, Ordering::Relaxed);
                            let new_total = stored + bytes_len;
                            let current_mb_index = new_total >> 20;
                            let prev_mb_index = stored >> 20;

                            if current_mb_index > prev_mb_index {
                                for bit in prev_mb_index..current_mb_index {
                                    if bit < 8 {
                                        context.flip_bit(bit as u8);
                                        // Emit progress event
                                        let stealing_val = context.stealing_from.load(Ordering::Relaxed);
                                        let stealing_opt = if stealing_val == usize::MAX { None } else { Some(stealing_val) };

                                        let _ = handle.emit("worker_progress", serde_json::json!({
                                           "download_id": download_id,
                                           "worker_id": context.worker_id,
                                           "state_bits": context.index.state.load(Ordering::Relaxed),
                                           "current_unit": unit_index,
                                           "unit_start_offset": start_byte,
                                           "unit_end_offset": end_byte + 1,
                                           "index_start": context.index.start.load(Ordering::Relaxed),
                                           "index_end": context.index.end.load(Ordering::Relaxed),
                                           "stealing_from": stealing_opt
                                        }));
                                    }
                                }
                            }

                            if speed_limit > 0 {
                                bytes_this_second += bytes_len as u64;
                                if bytes_this_second >= speed_limit {
                                     let elapsed = last_throttle.elapsed();
                                     if elapsed < Duration::from_secs(1) {
                                         tokio::time::sleep(Duration::from_secs(1) - elapsed).await;
                                     }
                                     last_throttle = std::time::Instant::now();
                                     bytes_this_second = 0;
                                }
                            }
                        },
                        Some(Err(e)) => {
                             tracing::error!("Stream error: {}", e);
                             break;
                        },
                        None => {
                             // Unit success
                             // Reset state for next unit
                             context.index.state.store(0, Ordering::Relaxed);
                             context.bytes_in_unit.store(0, Ordering::Relaxed);

                             // Signal unit completion for meta save
                             let _ = unit_tx.try_send(UnitCompletion { worker_id, unit_index });

                             // Log completion of unit
                             tracing::debug!(
                                 "Worker {} completed unit {} (offset {})",
                                 worker_id,
                                 unit_index,
                                 start_byte
                             );

                             return true;
                        }
                    }
                }
            }
        }

        if retries >= retry_count {
            return false;
        }
        retries += 1;
        tokio::time::sleep(Duration::from_millis(exponential_backoff(
            retries,
            retry_delay_ms,
        )))
        .await;
    }
}

fn exponential_backoff(retry: u8, base_delay_ms: u32) -> u64 {
    (base_delay_ms as u64) * 2u64.pow(retry.saturating_sub(1) as u32)
}

fn preallocate_file(path: &str, size: usize) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    file.set_len(size as u64)?;
    // No zeroing needed if FS supports sparse files or we trust set_len
    Ok(())
}

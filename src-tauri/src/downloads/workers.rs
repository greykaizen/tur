//! Worker tasks and download execution logic

use std::ops::Range;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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
use crate::downloads::client;
use crate::settings::config::AppSettings;

/// Minimum units to steal from a worker
const MIN_STEAL_UNITS: usize = 2; // 2 units minimum

/// Start download execution - returns handles, cancellation token, and indices for tracking
pub fn run_download<R: tauri::Runtime>(
    download: Download,
    id: Uuid,
    url: String,
    destination: String,
    total_size: usize,
    handle: &tauri::AppHandle<R>,
    config: &AppSettings,
    completion_tx: mpsc::Sender<Uuid>,
) -> (
    Vec<JoinHandle<()>>,
    CancellationToken,
    Arc<Mutex<Vec<Arc<Index>>>>,
    Arc<AtomicUsize>,
) {
    let mut handles = Vec::new();
    let cancel_token = CancellationToken::new();

    // Initialize indices list from download state (live progress tracking)
    // For multi-threaded: Coordinator will add to this.
    // For single-threaded: We'll add one index.
    let indices = Arc::new(Mutex::new(download.range.clone()));

    // Wrap worker_states in Arc for sharing with progress emitter
    let worker_states = Arc::new(download.worker_states);

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

    // Shared bytes counter
    let bytes_downloaded = Arc::new(AtomicUsize::new(0));

    // Settings
    let speed_limit = config.download.speed_limit;
    let retry_count = config.network.retry_count;
    let retry_delay_ms = config.network.retry_delay_ms;
    let num_threads = config.download.num_threads;

    // Spawn progress emitter - pass shared references for live state
    handles.push(spawn_progress_emitter(
        indices.clone(),
        worker_states.clone(),
        total_size,
        id,
        destination.clone(),
        bytes_downloaded.clone(),
        handle.clone(),
        cancel_token.clone(),
        completion_tx,
    ));

    // Check mode based on file size
    if total_size > RANGE[2].end << 20 {
        // Multi-threaded: coordinator owns mutable state directly
        // Convert Arc<Vec> back to Vec for workers (they each get Arc<AtomicU8>)
        let worker_states_vec: Vec<Arc<AtomicU8>> = worker_states.iter().cloned().collect();
        handles.extend(run_multi_threaded(
            download.coordinator,
            indices.clone(),
            worker_states_vec,
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
        ));
    } else {
        // Single-threaded: simple streaming
        // Add a single index for tracking if empty
        {
            let mut idx_guard = indices.lock().unwrap();
            if idx_guard.is_empty() {
                let total_units = (total_size + (1 << 23) - 1) >> 23;
                idx_guard.push(Arc::new(Index {
                    start: AtomicUsize::new(0),
                    end: AtomicUsize::new(total_units),
                }));
            }
        }

        let worker_states_vec: Vec<Arc<AtomicU8>> = worker_states.iter().cloned().collect();
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
            worker_states_vec,
            handle.clone(),
            id.to_string(),
        ));
    }

    (handles, cancel_token, indices, bytes_downloaded)
}

/// Production-quality progress emitter with variance-adaptive EWMA
fn spawn_progress_emitter<R: tauri::Runtime>(
    indices: Arc<Mutex<Vec<Arc<Index>>>>,
    worker_states: Arc<Vec<Arc<AtomicU8>>>,
    total_size: usize,
    id: Uuid,
    destination: String,
    bytes_downloaded: Arc<AtomicUsize>,
    handle: tauri::AppHandle<R>,
    cancel_token: CancellationToken,
    completion_tx: mpsc::Sender<Uuid>,
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

        loop {
            // Check cancellation first
            if cancel_token.is_cancelled() {
                break;
            }

            // Select on cancellation and interval ticking
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                _ = interval.tick() => {}
            }

            // Periodic Save - snapshot live state
            if last_save.elapsed().as_secs() >= SAVE_INTERVAL_S {
                let indices_clone = indices.clone();
                let states_clone = worker_states.clone();
                let handle_clone = handle.clone();
                let id_clone = id;
                let total = total_size;

                let _ = tokio::task::spawn_blocking(move || {
                    // Build a Download struct from live state for saving
                    let indices_guard = indices_clone.lock().unwrap();
                    let range: Vec<Arc<Index>> = indices_guard.clone();
                    drop(indices_guard);

                    let ws: Vec<Arc<AtomicU8>> = states_clone.iter().cloned().collect();

                    // Create minimal coordinator (we don't have live coordinator state here)
                    // This is a limitation - coordinator state won't be perfectly accurate
                    // But indices and worker_states ARE accurate
                    let max_index = Download::get_index(total >> 23).unwrap_or(0);
                    let coordinator = Coordinator::from_parts(max_index, max_index, 2, true, total);

                    let download = Download {
                        coordinator,
                        range,
                        worker_states: ws,
                    };

                    if let Err(e) = download.save(&handle_clone, &id_clone) {
                        tracing::warn!("Failed to auto-save download state: {}", e);
                    }
                })
                .await;

                last_save = std::time::Instant::now();
            }

            let downloaded = bytes_downloaded.load(Ordering::Relaxed);
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
                if let Ok(db) = crate::database::Database::initialize(&handle) {
                    let _ = db.mark_completed(&id);
                }
                let meta_path = Download::meta_path(&handle, &id);
                let _ = std::fs::remove_file(meta_path);

                // Trigger Check Queue via channel
                if let Err(e) = completion_tx.send(id).await {
                    tracing::error!("Failed to signal completion: {}", e);
                }

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

/// Multi-threaded download with coordinator owning state directly (no Mutex)
fn run_multi_threaded<R: tauri::Runtime>(
    mut coordinator: Coordinator,
    indices: Arc<Mutex<Vec<Arc<Index>>>>,
    worker_states: Vec<Arc<std::sync::atomic::AtomicU8>>,
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
) -> Vec<JoinHandle<()>> {
    // Channel for worker -> coordinator
    type WorkResponse = Option<(Arc<Index>, Range<usize>, Option<usize>)>;
    let (tx, mut rx) = mpsc::channel::<oneshot::Sender<WorkResponse>>(num_threads as usize * 2);

    let mut handles = Vec::new();

    // Spawn coordinator task
    handles.push(tokio::spawn(async move {
        // Access shared indices via lock inside the coordinator loop
        while let Some(reply_tx) = rx.recv().await {
            // Lock and pass mutable reference to request_work
            let mut indices_guard = indices.lock().unwrap();
            let result = coordinator.request_work(&mut indices_guard, MIN_STEAL_UNITS);
            let _ = reply_tx.send(result);
        }
    }));

    let per_worker_limit = if speed_limit > 0 {
        speed_limit / num_threads as u64
    } else {
        0
    };

    for (worker_id, _) in (0..num_threads).enumerate() {
        let worker_tx = tx.clone();
        let worker_url = url.clone();
        let worker_dest = destination.clone();
        let worker_bytes = bytes_downloaded.clone();
        let worker_client = client.clone();
        let worker_token = cancel_token.clone();
        let worker_state = worker_states[worker_id].clone();
        let worker_handle = handle.clone();
        let worker_download_id = id.to_string();

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
                if worker_tx.send(reply_tx).await.is_err() {
                    break;
                }

                match reply_rx.await {
                    Ok(Some((index, _unit_range, stealing_from))) => {
                        let mut current_unit = index.start.load(Ordering::Relaxed);

                        while current_unit < index.end.load(Ordering::Relaxed) {
                            if worker_token.is_cancelled() {
                                break;
                            }

                            if let Some(ref f) = file_handle {
                                if let Ok(f_clone) = f.try_clone() {
                                    use crate::downloads::worker_context::WorkerContext;
                                    let context = WorkerContext::new(
                                        worker_id,
                                        f_clone,
                                        worker_state.clone(),
                                        index.clone(),
                                        stealing_from,
                                    );

                                    let success = download_unit(
                                        &worker_client,
                                        &worker_url,
                                        context,
                                        &worker_bytes,
                                        per_worker_limit,
                                        retry_count,
                                        retry_delay_ms,
                                        worker_token.clone(),
                                        worker_handle.clone(),
                                        worker_download_id.clone(),
                                    )
                                    .await;

                                    if !success {
                                        break;
                                    }

                                    index.start.fetch_add(1, Ordering::Relaxed);
                                    current_unit += 1;
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    Ok(None) => break,
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

fn run_single_threaded<R: tauri::Runtime>(
    url: String,
    destination: String,
    bytes_downloaded: Arc<AtomicUsize>,
    client: Arc<reqwest::Client>,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
    cancel_token: CancellationToken,
    indices: Arc<Mutex<Vec<Arc<Index>>>>,
    worker_states: Vec<Arc<std::sync::atomic::AtomicU8>>,
    handle: tauri::AppHandle<R>,
    id: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Calculate total units based on total file size?
        // We can get it from the index (0..end)
        let index = {
            let guard = indices.lock().unwrap();
            guard.first().cloned()
        };

        if let Some(idx) = index {
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

                // Use worker 0 state
                if let Some(ref f) = file_handle {
                    if let Ok(f_clone) = f.try_clone() {
                        use crate::downloads::worker_context::WorkerContext;
                        // Use state 0 if available, else new
                        let state = if !worker_states.is_empty() {
                            worker_states[0].clone()
                        } else {
                            Arc::new(std::sync::atomic::AtomicU8::new(0))
                        };

                        let context = WorkerContext::new(0, f_clone, state, idx.clone(), None);

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
                        )
                        .await;

                        if !success {
                            break;
                        }

                        idx.start.fetch_add(1, Ordering::Relaxed);
                        current += 1;
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
    download_id: String, // For events
) -> bool {
    let mut retries = 0u8;
    let unit_index = context.index.start.load(Ordering::Relaxed);
    let start_byte = unit_index << 23;
    let end_byte = start_byte + (1 << 23) - 1;

    // Resume Logic: Find first unset bit to determine resume position
    // trailing_ones() gives us the first gap - if bits are 00001011, we resume at bit 2
    // This is correct because we download sequentially within a unit
    let state_bits = context.state.load(Ordering::Relaxed);
    let first_unset_bit = state_bits.trailing_ones() as usize;
    let resume_offset = first_unset_bit << 20; // MB to Bytes

    // Update context to reflect skipped bytes
    context
        .bytes_in_unit
        .store(resume_offset, Ordering::Relaxed);

    // If fully complete (or somehow overshot), finish immediately
    if state_bits == 0xFF || resume_offset >= (1 << 23) {
        context.reset_unit();
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
                                           "state_bits": context.state.load(Ordering::Relaxed),
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
                             context.state.store(0, Ordering::Relaxed);
                             context.bytes_in_unit.store(0, Ordering::Relaxed);

                             // Log completion of unit
                             tracing::debug!(
                                 "Worker {} completed unit {} (offset {})",
                                 context.worker_id,
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

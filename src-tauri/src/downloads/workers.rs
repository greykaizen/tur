//! Worker tasks and download execution logic

use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::constants::RANGE;
use super::coordinator::Coordinator;
use super::download::Download;
use super::index::Index;
use crate::downloads::client;
use crate::settings::config::AppSettings;

/// Minimum bytes to steal from a worker
const MIN_STEAL_BYTES: usize = 1024 * 1024; // 1 MB

/// Start download execution - takes ownership of Download, returns handles
/// Parameters passed in for minimal memory footprint
pub fn run_download<R: tauri::Runtime>(
    download: Download,
    id: Uuid,
    url: String,
    destination: String,
    total_size: usize,
    handle: &tauri::AppHandle<R>,
    config: &AppSettings,
) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();

    // Pre-allocate file
    if let Err(e) = preallocate_file(&destination, total_size) {
        eprintln!("Failed to pre-allocate file: {}", e);
    }

    // Create shared HTTP client
    let shared_client = match client::create(config) {
        Ok(c) => Arc::new(c),
        Err(e) => {
            eprintln!("Failed to create HTTP client: {}", e);
            return handles;
        }
    };

    // Shared bytes counter
    let bytes_downloaded = Arc::new(AtomicUsize::new(0));

    // Settings
    let speed_limit = config.download.speed_limit;
    let retry_count = config.network.retry_count;
    let retry_delay_ms = config.network.retry_delay_ms;
    let num_threads = config.download.num_threads;

    // Spawn progress emitter
    handles.push(spawn_progress_emitter(
        id,
        destination.clone(),
        total_size,
        bytes_downloaded.clone(),
        handle.clone(),
    ));

    // Check mode based on file size
    if total_size > RANGE[2].end << 23 {
        // Multi-threaded: coordinator owns mutable state directly
        handles.extend(run_multi_threaded(
            download.coordinator,
            url,
            destination,
            bytes_downloaded,
            shared_client,
            num_threads,
            speed_limit,
            retry_count,
            retry_delay_ms,
        ));
    } else {
        // Single-threaded: simple streaming
        handles.push(run_single_threaded(
            url,
            destination,
            bytes_downloaded,
            shared_client,
            speed_limit,
            retry_count,
            retry_delay_ms,
        ));
    }

    handles
}

/// Production-quality progress emitter with variance-adaptive EWMA
/// Works correctly for fast fiber, slow mobile, and laggy VPN connections
fn spawn_progress_emitter<R: tauri::Runtime>(
    id: Uuid,
    destination: String,
    total_size: usize,
    bytes_downloaded: Arc<AtomicUsize>,
    handle: tauri::AppHandle<R>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        use std::time::Instant;

        // Configuration
        const EMIT_INTERVAL_MS: u64 = 250;
        const WARM_UP_SAMPLES: usize = 3; // Collect samples before showing speed
        const STALL_HOLD_MS: u64 = 800; // Hold speed during brief stalls
        const STALL_DECAY: f64 = 0.85; // Decay rate per tick during extended stalls

        // EWMA bounds - variance controls alpha within this range
        const ALPHA_MIN: f64 = 0.15; // Very smooth when unstable
        const ALPHA_MAX: f64 = 0.45; // Responsive when stable

        let mut interval = tokio::time::interval(Duration::from_millis(EMIT_INTERVAL_MS));

        // State
        let mut last_bytes = 0usize;
        let mut smoothed_speed: f64 = 0.0;
        let mut stall_start: Option<Instant> = None;
        let mut last_tick = Instant::now();

        // Variance tracking (ring buffer of last N speed samples)
        let mut speed_history: [f64; 5] = [0.0; 5];
        let mut history_idx = 0usize;
        let mut sample_count = 0usize;

        loop {
            interval.tick().await;

            let downloaded = bytes_downloaded.load(Ordering::Relaxed);
            let now = Instant::now();
            let byte_delta = downloaded.saturating_sub(last_bytes);

            // Use actual elapsed time since last tick for accurate speed
            let elapsed_secs = last_tick.elapsed().as_secs_f64();
            last_tick = now;

            if byte_delta > 0 && elapsed_secs > 0.05 {
                // We received bytes - calculate speed from this interval
                let raw_speed = byte_delta as f64 / elapsed_secs;

                // Update history for variance calculation
                speed_history[history_idx] = raw_speed;
                history_idx = (history_idx + 1) % speed_history.len();
                sample_count = (sample_count + 1).min(speed_history.len());

                // Calculate variance-adaptive alpha
                let alpha = if sample_count >= 2 {
                    let mean: f64 =
                        speed_history[..sample_count].iter().sum::<f64>() / sample_count as f64;
                    let variance: f64 = speed_history[..sample_count]
                        .iter()
                        .map(|s| (s - mean).powi(2))
                        .sum::<f64>()
                        / sample_count as f64;

                    // Coefficient of variation (normalized variance)
                    let cv = if mean > 0.0 {
                        variance.sqrt() / mean
                    } else {
                        0.0
                    };

                    // High CV (unstable) → low alpha (smooth)
                    // Low CV (stable) → high alpha (responsive)
                    let cv_clamped = cv.clamp(0.0, 1.0);
                    ALPHA_MAX - (cv_clamped * (ALPHA_MAX - ALPHA_MIN))
                } else {
                    0.35 // Default alpha during warm-up
                };

                // Apply EWMA - no clamping, just smooth transition
                if sample_count <= 1 {
                    smoothed_speed = raw_speed;
                } else {
                    smoothed_speed = alpha * raw_speed + (1.0 - alpha) * smoothed_speed;
                }

                // Reset stall tracking
                last_bytes = downloaded;
                stall_start = None;
            } else {
                // No new bytes - handle stall
                let stall_duration = stall_start.get_or_insert(now).elapsed().as_millis() as u64;

                if stall_duration > STALL_HOLD_MS {
                    // Extended stall: decay speed
                    smoothed_speed *= STALL_DECAY;
                    if smoothed_speed < 100.0 {
                        smoothed_speed = 0.0;
                    }
                }
                // Brief stall: hold current speed (do nothing)
            }

            // Calculate display values
            let display_speed = if sample_count >= WARM_UP_SAMPLES {
                smoothed_speed as usize
            } else {
                0 // Don't show speed until we have enough samples
            };

            let percentage = if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };

            // Emit progress
            let _ = handle.emit(
                "download_progress",
                serde_json::json!({
                    "id": id.to_string(),
                    "downloaded": downloaded,
                    "progress": percentage,
                    "speed": display_speed,
                }),
            );

            // Check completion
            if downloaded >= total_size && total_size > 0 {
                if let Ok(db) = crate::database::Database::initialize(&handle) {
                    let _ = db.mark_completed(&id);
                }
                let meta_path = Download::meta_path(&handle, &id);
                let _ = std::fs::remove_file(meta_path);

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
fn run_multi_threaded(
    mut coordinator: Coordinator,
    url: String,
    destination: String,
    bytes_downloaded: Arc<AtomicUsize>,
    client: Arc<reqwest::Client>,
    num_threads: u8,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
) -> Vec<JoinHandle<()>> {
    // Channel for worker -> coordinator
    type WorkResponse = Option<(Arc<Index>, Range<usize>)>;
    let (tx, mut rx) = mpsc::channel::<oneshot::Sender<WorkResponse>>(num_threads as usize * 2);

    // Coordinator owns range Vec directly - no Arc, no Mutex!
    let mut range: Vec<Arc<Index>> = Vec::with_capacity(num_threads as usize);

    let mut handles = Vec::new();

    // Spawn coordinator task - owns coordinator and range directly
    handles.push(tokio::spawn(async move {
        while let Some(reply_tx) = rx.recv().await {
            let result = coordinator.request_work(&mut range, MIN_STEAL_BYTES);
            let _ = reply_tx.send(result);
        }
    }));

    // Per-worker speed limit
    let per_worker_limit = if speed_limit > 0 {
        speed_limit / num_threads as u64
    } else {
        0
    };

    // Spawn worker tasks
    for _ in 0..num_threads {
        let worker_tx = tx.clone();
        let worker_url = url.clone();
        let worker_dest = destination.clone();
        let worker_bytes = bytes_downloaded.clone();
        let worker_client = client.clone();

        handles.push(tokio::spawn(async move {
            loop {
                let (reply_tx, reply_rx) = oneshot::channel();
                if worker_tx.send(reply_tx).await.is_err() {
                    break;
                }

                match reply_rx.await {
                    Ok(Some((index, byte_range))) => {
                        let _ = stream_range(
                            &worker_client,
                            &worker_url,
                            &worker_dest,
                            Some((byte_range, index)),
                            &worker_bytes,
                            per_worker_limit,
                            retry_count,
                            retry_delay_ms,
                        )
                        .await;
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }));
    }

    handles
}

/// Single-threaded download
fn run_single_threaded(
    url: String,
    destination: String,
    bytes_downloaded: Arc<AtomicUsize>,
    client: Arc<reqwest::Client>,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = stream_range(
            &client,
            &url,
            &destination,
            None, // Full file, no range
            &bytes_downloaded,
            speed_limit,
            retry_count,
            retry_delay_ms,
        )
        .await;
    })
}

/// Common streaming logic - handles both full file and range requests
async fn stream_range(
    client: &reqwest::Client,
    url: &str,
    destination: &str,
    range_info: Option<(Range<usize>, Arc<Index>)>,
    bytes_counter: &Arc<AtomicUsize>,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
) -> bool {
    let mut retries = 0u8;

    loop {
        // Build request
        let mut req = client.get(url);
        let start_offset = if let Some((ref range, _)) = range_info {
            req = req.header(
                "Range",
                format!("bytes={}-{}", range.start, range.end.saturating_sub(1)),
            );
            range.start
        } else {
            0
        };

        let response = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Request failed: {}", e);
                if retries < retry_count {
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(exponential_backoff(
                        retries,
                        retry_delay_ms,
                    )))
                    .await;
                    continue;
                }
                return false;
            }
        };

        let status = response.status();
        if status != reqwest::StatusCode::OK && status != reqwest::StatusCode::PARTIAL_CONTENT {
            eprintln!("Unexpected status: {}", status);
            if retries < retry_count {
                retries += 1;
                tokio::time::sleep(Duration::from_millis(exponential_backoff(
                    retries,
                    retry_delay_ms,
                )))
                .await;
                continue;
            }
            return false;
        }

        // Stream to file
        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let mut offset = start_offset;
        let mut last_throttle = std::time::Instant::now();
        let mut bytes_this_second = 0u64;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    let bytes_len = bytes.len();
                    let write_offset = offset as u64;
                    let bytes_clone = bytes.to_vec();
                    let dest = destination.to_string();

                    let _ = tokio::task::spawn_blocking(move || {
                        use std::io::{Seek, Write};
                        if let Ok(mut f) = std::fs::OpenOptions::new()
                            .write(true)
                            .create(true)
                            .open(&dest)
                        {
                            let _ = f.seek(std::io::SeekFrom::Start(write_offset));
                            let _ = f.write_all(&bytes_clone);
                        }
                    })
                    .await;

                    offset += bytes_len;

                    // Update Index if range download
                    if let Some((_, ref index)) = range_info {
                        index.start.store(offset, Ordering::Relaxed);
                    }

                    bytes_counter.fetch_add(bytes_len, Ordering::Relaxed);

                    // Speed limiting
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
                }
                Err(e) => {
                    eprintln!("Stream error: {}", e);
                    if retries < retry_count {
                        retries += 1;
                        tokio::time::sleep(Duration::from_millis(exponential_backoff(
                            retries,
                            retry_delay_ms,
                        )))
                        .await;
                        break;
                    }
                    return false;
                }
            }
        }

        // Check completion
        if let Some((ref range, ref index)) = range_info {
            if index.start.load(Ordering::Relaxed) >= range.end {
                return true;
            }
            if retries >= retry_count {
                return false;
            }
            retries += 1;
        } else {
            return true; // Single-threaded completed
        }
    }
}

fn exponential_backoff(retry: u8, base_delay_ms: u32) -> u64 {
    (base_delay_ms as u64) * 2u64.pow(retry.saturating_sub(1) as u32)
}

fn preallocate_file(path: &str, size: usize) -> std::io::Result<()> {
    use std::io::Write;
    let file = std::fs::File::create(path)?;
    file.set_len(size as u64)?;
    let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
    std::io::Seek::seek(&mut file, std::io::SeekFrom::End(-1))?;
    file.write_all(&[0])?;
    Ok(())
}

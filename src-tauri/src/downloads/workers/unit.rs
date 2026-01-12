use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::context::UnitCompletion;
use super::utils::exponential_backoff;
use crate::downloads::worker_context::WorkerContext;

pub async fn download_unit<R: tauri::Runtime>(
    client: &reqwest::Client,
    url: &str,
    context: WorkerContext, // Takes ownership (one unit lifecycle)
    bytes_counter: &Arc<AtomicUsize>,
    speed_limit: u64,
    retry_count: u8,
    retry_delay_ms: u32,
    cancel_token: CancellationToken,
    handle: AppHandle<R>,
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

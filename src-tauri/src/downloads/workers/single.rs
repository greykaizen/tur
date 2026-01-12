use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::context::UnitCompletion;
use super::unit::download_unit;
use crate::downloads::index::Index;

pub fn run_single_threaded<R: tauri::Runtime>(
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

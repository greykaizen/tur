//! Worker tasks and download execution logic

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::constants::RANGE;
use super::download::Download;
use super::index::Index;
use super::manager::ManagerHandle;
use crate::downloads::client;
use crate::settings::config::AppSettings;

// Declare submodules
pub mod context;
pub mod multi;
pub mod single;
pub mod unit;
pub mod utils;

use context::UnitCompletion;

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
    if let Err(e) = utils::preallocate_file(&destination, total_size) {
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
    handles.push(multi::spawn_progress_emitter(
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
        handles.extend(multi::run_multi_threaded(
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

        handles.push(single::run_single_threaded(
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

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::downloads::coordinator::Coordinator;
use crate::downloads::download::Download;
use crate::downloads::index::Index;

#[derive(Debug)]
pub struct ActiveDownload {
    pub handles: Vec<JoinHandle<()>>,
    pub cancel_token: CancellationToken,
    pub indices: Arc<Vec<Arc<Index>>>,
    pub queue_id: Option<Uuid>,
    pub bytes_downloaded: Arc<AtomicUsize>,
}

impl ActiveDownload {
    pub fn bytes_downloaded(&self) -> usize {
        self.bytes_downloaded
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.cancel_token.cancel();
        for handle in &self.handles {
            handle.abort();
        }
    }

    pub fn snapshot_and_save(
        &self,
        id: &Uuid,
        app: &AppHandle,
        total_size: usize,
    ) -> Result<(), String> {
        let indices_snapshot: Vec<Arc<Index>> = self
            .indices
            .iter()
            .map(|idx| {
                let i = Index::new();
                i.start.store(
                    idx.start.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                i.end.store(
                    idx.end.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                i.state.store(
                    idx.state.load(std::sync::atomic::Ordering::Relaxed),
                    std::sync::atomic::Ordering::Relaxed,
                );
                Arc::new(i)
            })
            .collect();

        let max_index = Download::get_index(total_size >> 23).unwrap_or(0);
        let coordinator = Coordinator::from_parts(max_index, max_index, 2, false, total_size);

        let download_state = Download {
            coordinator,
            indices: indices_snapshot,
        };

        download_state.save(app, id).map_err(|e| e.to_string())
    }
}

use std::sync::Mutex;
use tauri::Manager;
use tokio::task::JoinSet;

#[cfg(unix)]
use tokio::signal::{self, unix::SignalKind};

use crate::db::DownloadDb;
enum _ControlCommand {
    Resume,
    Pause,
    Cancel,
    SpeedLimit(usize),
}

//  TODO tauri store read to memory and push new changes design
pub struct DownloadManager {
    db: DownloadDb,
    instances: Mutex<JoinSet<()>>, // uuid ain't needed if joinset auto drop on finish
}

impl DownloadManager {
    pub fn new(app_handle: &tauri::AppHandle) -> anyhow::Result<Self> {
        let db_path = app_handle.path().app_data_dir()?.join("tur.db");
        Ok(Self {
            db: DownloadDb::new(&db_path)?,
            instances: Mutex::new(JoinSet::new()),
        })
    }

    // replace shutdown_all() with Drop trait
    async fn _start_signal_handler(&self) {
        #[cfg(unix)]
        {
            let mut _sigterm = signal::unix::signal(SignalKind::terminate()).unwrap();
            // let mut sigint = signal::unix::signal(SignalKind::interrupt()).unwrap();

            // TODO select! issue
            // tokio::select! {
            //     _ = signal::ctrl_c() => {
            //         self.shutdown_all();
            //     },
            //     _ = sigterm.recv() => {
            //         self.shutdown_all();
            //     },
            // }
        }

        #[cfg(not(unix))]
        {
            signal::ctrl_c().await?;
            tracing::info!("Ctrl-C");
            self.shutdown_all().await;
        }
    }

    // TODO: remove shutdown after shutdown logic is done
    fn _shutdown_all(&self) {} // cancel each instance, not .abort()
}

// impl Drop for DownloadManager {
//     fn drop(&self) {
//         // cancel all, so they save progress and close db conn
//     }
// }

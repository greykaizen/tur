use tokio::task::JoinHandle;

#[cfg(unix)]
use tokio::signal::{self, unix::SignalKind};
enum _ControlCommand {
    Resume,
    Pause,
    Cancel,
    SpeedLimit(usize),
}

//  TODO tauri store read to memory and push new changes design
pub struct DownloadManager<R: tauri::Runtime> {
    // db_conn: Connection,
    pub app_handle: tauri::AppHandle<R>,
    pub instances: Vec<JoinHandle<()>>
}

impl<R: tauri::Runtime> DownloadManager<R> {
    pub fn new(app_handle: tauri::AppHandle<R>) -> Self {
        DownloadManager {
            app_handle,
            instances: Vec::new(),
        }
    }

    fn _save_record() {
        // use db to store record
    }

    // if id exist in DM, watch signal send to 0/1/2
    // on instances that are already in history
    // pub fn exec_instance_action(id: Vec<usize>, action: u8) {
        // actions: cancel(0), start(1), pause(2)  (assuming item is already in DM)
        // id checked in DM, call necessary action 0/1/2
        // if id is not in the DM and action called do nothing except for start action where we init_instance
    // }

    // this line up don't make sense as well
    // pub fn cancel_instance(id: Vec<usize>) {}
    // pub fn start_instance(id: Vec<usize>) {}
    // pub fn pause_instance(id: Vec<usize>) {}

    // replace shutdown_all() with Drop trait
    async fn _start_signal_handler(&self) {
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(SignalKind::terminate()).unwrap();
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
    fn shutdown_all(&self) {} // cancel each instance, not .abort()
}

// impl Drop for DownloadManager {
//     fn drop(&self) {
//         // cancel all, so they save progress and close db conn
//     }
// }

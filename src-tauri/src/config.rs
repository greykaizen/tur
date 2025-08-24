// use std::path::PathBuf;
use serde::{Deserialize, Serialize};
// use tauri::path::BaseDirectory::Download as download_dir;

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceConfig {
    pub download: DownloadConfig,
    pub thread: ThreadConfig,
    pub session: SessionConfig,
}
// impl From<&AppConfig> for InstanceConfig {
//     fn from(app_cfg: &AppConfig) -> Self {
//         Self {
//             download: app_cfg.download.clone(),
//             thread: app_cfg.thread.clone(),
//             session: app_cfg.session.clone(),
//         }
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// pub struct AppConfig {
//     pub app: WindowConfig,
//     pub download: DownloadConfig,
//     pub thread: ThreadConfig,
//     pub session: SessionConfig,
// }
// impl Default for AppConfig {
//     fn default() -> Self {
//         AppConfig {
//             app: WindowConfig::default(),
//             download: DownloadConfig::default(),
//             thread: ThreadConfig::default(),
//             session: SessionConfig::default(),
//         }
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// pub struct WindowConfig {
//     pub show_tray_icon: bool,
//     pub quit_on_close: bool,
//     pub side_bar: String,
//     pub theme: String,
//     // pub single_instance : bool
// }
// impl Default for WindowConfig {
//     fn default() -> Self {
//         WindowConfig {
//             show_tray_icon: true,
//             quit_on_close: false,
//             side_bar: String::from("left"),
//             theme: String::from("system"),
//         }
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DownloadConfig {
    pub num_threads: u8,
    pub chunk_size: usize,
    pub socket_buffer_size: usize,
    pub speed_limit: usize,
    // pub download_location: String,
}
// impl Default for DownloadConfig {
//     fn default() -> Self {
//         DownloadConfig {
//             num_threads: 8,
//             chunk_size: 16,
//             socket_buffer_size: 0,
//             speed_limit: 0,
//             // download_location: String::new()
//         }
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThreadConfig {
    pub total_connections: u8,
    pub per_task_connections: u8,
}
// impl Default for ThreadConfig {
//     fn default() -> Self {
//         ThreadConfig {
//             total_connections: 1,
//             per_task_connections: 1,
//         }
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionConfig {
    pub history: bool,
    pub metadata: bool,
}
// impl Default for SessionConfig {
//     fn default() -> Self {
//         SessionConfig {
//             history: false,
//             metadata: false,
//         }
//     }
// }

// use std::path::PathBuf;
use serde::{Deserialize, Serialize};
// use tauri::path::BaseDirectory::Download as download_dir;

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceConfig {
    pub download: DownloadConfig,
    pub thread: ThreadConfig,
    pub session: SessionConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DownloadConfig {
    pub num_threads: u8,
    pub chunk_size: usize,
    pub socket_buffer_size: usize,
    pub speed_limit: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThreadConfig {
    pub total_connections: u8,
    pub per_task_connections: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionConfig {
    pub history: bool,
    pub metadata: bool,
}

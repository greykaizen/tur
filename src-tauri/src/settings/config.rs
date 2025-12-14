use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub app: AppConfig,
    pub shortcuts: ShortcutConfig,
    pub download: DownloadConfig,
    pub thread: ThreadConfig,
    pub session: SessionConfig,
    pub send_anonymous_metrics: bool,
    pub show_notifications: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub show_tray_icon: bool,
    pub quit_on_close: bool,
    pub sidebar: String,
    pub theme: String,
    pub button_label: String,
    pub show_download_progress: bool,
    pub show_segment_progress: bool,
    pub autostart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub go_home: String,
    pub open_settings: String,
    pub add_download: String,
    pub open_details: String,
    pub open_history: String,
    pub toggle_sidebar: String,
    pub cancel_download: String,
    pub quit_app: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub download_location: String,
    pub num_threads: u8,
    pub chunk_size: u32,
    pub socket_buffer_size: u32,
    pub speed_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadConfig {
    pub total_connections: u8,
    pub per_task_connections: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub history: bool,
    pub metadata: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app: AppConfig::default(),
            shortcuts: ShortcutConfig::default(),
            download: DownloadConfig::default(),
            thread: ThreadConfig::default(),
            session: SessionConfig::default(),
            send_anonymous_metrics: false,
            show_notifications: true,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            show_tray_icon: true,
            quit_on_close: false,
            sidebar: "left".into(),
            theme: "system".into(),
            button_label: "both".into(),
            show_download_progress: true,
            show_segment_progress: true,
            autostart: false,
        }
    }
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            go_home: "Ctrl+K".into(),
            open_settings: "Ctrl+P".into(),
            add_download: "Ctrl+N".into(),
            open_details: "Ctrl+D".into(),
            open_history: "Ctrl+H".into(),
            toggle_sidebar: "Ctrl+L".into(),
            cancel_download: "Ctrl+C".into(),
            quit_app: "Ctrl+Q".into(),
        }
    }
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            download_location: get_default_download_dir(),
            num_threads: 8,
            chunk_size: 16,
            socket_buffer_size: 0,
            speed_limit: 0,
        }
    }
}

impl Default for ThreadConfig {
    fn default() -> Self {
        Self {
            total_connections: 1,
            per_task_connections: 1,
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            history: false,
            metadata: false,
        }
    }
}

fn get_default_download_dir() -> String {
    dirs::download_dir()
        .and_then(|path| path.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            // Fallback if dirs crate fails
            dirs::home_dir()
                .and_then(|home| {
                    let downloads = home.join("Downloads");
                    downloads.to_str().map(|s| s.to_string())
                })
                .unwrap_or_else(|| "./downloads".to_string())
        })
}
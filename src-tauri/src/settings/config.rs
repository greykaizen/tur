use serde::{Deserialize, Serialize};

/// Main application settings container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub app: AppConfig,
    pub shortcuts: ShortcutConfig,
    pub download: DownloadConfig,
    pub network: NetworkConfig,
    pub session: SessionConfig,
    pub send_anonymous_metrics: bool,
    pub show_notifications: bool,
    pub notification_sound: bool,
    pub dependencies: DependencyConfig,
    pub scheduler: SchedulerConfig,
}

/// Scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Behavior when a schedule is missed: "start_immediately" or "ask_user"
    pub missed_schedule_action: String,
    /// Bandwidth windows
    pub bandwidth_windows: Vec<BandwidthWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthWindow {
    /// Start time in HH:MM format (24-hour)
    pub start_time: String,
    /// End time in HH:MM format (24-hour)
    pub end_time: String,
    /// Speed limit in bytes/sec (0 = unlimited)
    pub speed_limit: u64,
    /// Optional name for the window
    pub name: Option<String>,
    /// Whether this window is enabled
    pub enabled: bool,
}

/// General application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub show_tray_icon: bool,
    pub quit_on_close: bool,
    pub sidebar: String,
    pub theme: String,
    pub button_label: String,
    pub show_download_progress: bool,
    pub show_segment_progress: bool,
    pub show_welcome_screen: bool,
    pub autostart: bool,
    /// Resume incomplete downloads when app starts
    pub auto_resume: bool,
    /// Page to show on startup: "home", "details", "history"
    pub startup_page: String,
    /// Action on new download: "go_to_home", "stay_on_page"
    pub new_download_action: String,
    /// Seconds to display completion state before dismissing (0-30)
    pub completion_display_duration: u8,
    /// Action after completion: "popup", "notification", "none"
    pub completion_action: String,
}

/// Keyboard shortcut bindings
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

/// Download behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    /// Default download directory
    pub download_location: String,
    /// Number of connections per download (1-64)
    pub num_threads: u8,
    /// Maximum concurrent downloads (0 = unlimited)
    pub max_concurrent: u8,
    /// Global speed limit in bytes/sec (0 = unlimited)
    pub speed_limit: u64,
    /// How to handle filename conflicts: "rename", "overwrite", "skip", "ask"
    pub conflict_action: String,
}

/// Network and HTTP client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// User agent preset: "chrome", "firefox", "edge", "safari", "custom"
    pub user_agent: String,
    /// Custom user agent string (used when user_agent == "custom")
    pub custom_user_agent: String,
    /// Connection timeout in seconds (1-300)
    pub connect_timeout_secs: u16,
    /// Read timeout in seconds per chunk (1-300)
    pub read_timeout_secs: u16,
    /// Number of retry attempts on failure (0-10)
    pub retry_count: u8,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u32,
    /// Allow invalid/self-signed SSL certificates
    pub allow_insecure: bool,
    /// Proxy configuration
    pub proxy: ProxyConfig,
}

/// Proxy server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Enable proxy
    pub enabled: bool,
    /// Proxy type: "http", "https", "socks5"
    pub proxy_type: String,
    /// Proxy host
    pub host: String,
    /// Proxy port
    pub port: u16,
    /// Enable proxy authentication
    pub auth_enabled: bool,
    /// Proxy username
    pub username: String,
    /// Proxy password
    pub password: String,
}

/// Session and data retention settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Store download history in database (false = don't record new downloads)
    pub history: bool,
    /// Save metadata on pause/cancel for resume (false = no resume capability)
    pub metadata: bool,
}

/// Dependency path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyConfig {
    /// Custom path to yt-dlp binary (empty = auto-detect)
    pub ytdlp_path: String,
    /// Custom path to ffmpeg binary (empty = auto-detect)
    pub ffmpeg_path: String,
}

// ============================================================================
// Default implementations
// ============================================================================

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app: AppConfig::default(),
            shortcuts: ShortcutConfig::default(),
            download: DownloadConfig::default(),
            network: NetworkConfig::default(),
            session: SessionConfig::default(),
            send_anonymous_metrics: false,
            show_notifications: true,
            notification_sound: true,
            dependencies: DependencyConfig::default(),
            scheduler: SchedulerConfig::default(),
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            missed_schedule_action: "start_immediately".into(),
            bandwidth_windows: Vec::new(),
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
            show_welcome_screen: true,
            autostart: false,
            auto_resume: false,
            startup_page: "home".into(),
            new_download_action: "go_to_home".into(),
            completion_display_duration: 5,
            completion_action: "popup".into(),
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
            max_concurrent: 0, // 0 = unlimited
            speed_limit: 0,    // 0 = unlimited
            conflict_action: "ask".into(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            user_agent: "chrome".into(),
            custom_user_agent: String::new(),
            connect_timeout_secs: 15,
            read_timeout_secs: 30,
            retry_count: 3,
            retry_delay_ms: 1000,
            allow_insecure: false,
            proxy: ProxyConfig::default(),
        }
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: "http".into(),
            host: String::new(),
            port: 8080,
            auth_enabled: false,
            username: String::new(),
            password: String::new(),
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            history: true,
            metadata: true,
        }
    }
}

impl Default for DependencyConfig {
    fn default() -> Self {
        Self {
            ytdlp_path: String::new(),
            ffmpeg_path: String::new(),
        }
    }
}

// ============================================================================
// Validation
// ============================================================================

impl AppSettings {
    /// Validate and clamp settings to valid ranges
    pub fn validate(&mut self) {
        self.download.validate();
        self.network.validate();
    }
}

impl DownloadConfig {
    pub fn validate(&mut self) {
        // num_threads: 1-64
        self.num_threads = self.num_threads.clamp(1, 64);
        // max_concurrent: 0-32 (0 = unlimited)
        self.max_concurrent = self.max_concurrent.min(32);
        // conflict_action must be valid
        if !["rename", "overwrite", "skip", "ask"].contains(&self.conflict_action.as_str()) {
            self.conflict_action = "ask".into();
        }
    }
}

impl NetworkConfig {
    pub fn validate(&mut self) {
        // Timeouts: 1-300 seconds
        self.connect_timeout_secs = self.connect_timeout_secs.clamp(1, 300);
        self.read_timeout_secs = self.read_timeout_secs.clamp(1, 300);
        // Retry count: 0-10
        self.retry_count = self.retry_count.min(10);
        // User agent must be valid preset or "custom"
        if !["chrome", "firefox", "edge", "safari", "custom"].contains(&self.user_agent.as_str()) {
            self.user_agent = "chrome".into();
        }
        // Proxy type must be valid
        if !["http", "https", "socks5"].contains(&self.proxy.proxy_type.as_str()) {
            self.proxy.proxy_type = "http".into();
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn get_default_download_dir() -> String {
    dirs::download_dir()
        .and_then(|path| path.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .and_then(|home| {
                    let downloads = home.join("Downloads");
                    downloads.to_str().map(|s| s.to_string())
                })
                .unwrap_or_else(|| "./downloads".to_string())
        })
}

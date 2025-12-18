//! HTTP client configuration and creation

use reqwest::Client;
use std::time::Duration;

use crate::settings::config::AppSettings;

/// Chrome user agent (latest stable)
const UA_CHROME: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Firefox user agent (latest stable)
const UA_FIREFOX: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0";

/// Edge user agent (latest stable)
const UA_EDGE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0";

/// Safari user agent (latest stable)
const UA_SAFARI: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_2) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15";

/// Get user agent string based on settings preset
fn get_user_agent(settings: &AppSettings) -> &str {
    match settings.network.user_agent.as_str() {
        "chrome" => UA_CHROME,
        "firefox" => UA_FIREFOX,
        "edge" => UA_EDGE,
        "safari" => UA_SAFARI,
        "custom" => &settings.network.custom_user_agent,
        _ => UA_CHROME, // Default fallback
    }
}

/// Create optimized HTTP client with settings-based configuration
pub fn create(settings: &AppSettings) -> Result<Client, String> {
    let network = &settings.network;

    let mut builder = Client::builder()
        // Timeouts from settings
        .timeout(Duration::from_secs(300)) // Overall request timeout
        .connect_timeout(Duration::from_secs(network.connect_timeout_secs as u64))
        .read_timeout(Duration::from_secs(network.read_timeout_secs as u64))
        // Connection pooling for better performance
        .pool_max_idle_per_host(settings.thread.total_connections as usize)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        // User agent from settings
        .user_agent(get_user_agent(settings))
        // Redirect policy
        .redirect(reqwest::redirect::Policy::limited(10))
        // Security settings from settings
        .danger_accept_invalid_certs(network.allow_insecure)
        .https_only(false) // Allow HTTP for compatibility
        // HTTP/2 support
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(Some(Duration::from_secs(30)));

    // Configure proxy if enabled
    if network.proxy.enabled && !network.proxy.host.is_empty() {
        let proxy_url = format!(
            "{}://{}:{}",
            network.proxy.proxy_type, network.proxy.host, network.proxy.port
        );

        let proxy = match network.proxy.proxy_type.as_str() {
            "socks5" => reqwest::Proxy::all(&proxy_url),
            _ => reqwest::Proxy::all(&proxy_url), // HTTP/HTTPS use same method
        }
        .map_err(|e| format!("Invalid proxy URL: {}", e))?;

        // Add authentication if enabled
        let proxy = if network.proxy.auth_enabled && !network.proxy.username.is_empty() {
            proxy.basic_auth(&network.proxy.username, &network.proxy.password)
        } else {
            proxy
        };

        builder = builder.proxy(proxy);
    }

    builder
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))
}

/// Get retry configuration from settings
pub fn retry_config(settings: &AppSettings) -> (u8, Duration) {
    (
        settings.network.retry_count,
        Duration::from_millis(settings.network.retry_delay_ms as u64),
    )
}

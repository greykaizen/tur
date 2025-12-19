use super::config::{
    AppConfig, AppSettings, DownloadConfig, NetworkConfig, ProxyConfig, SessionConfig,
    ShortcutConfig,
};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "settings.json";
const SETTINGS_KEY: &str = "settings";

/// Load settings from store or create defaults
pub fn load_or_create(app: &AppHandle) -> AppSettings {
    match load_existing(app) {
        Ok(mut settings) => {
            // Validate after loading (clamps to valid ranges)
            settings.validate();
            settings
        }
        Err(_) => {
            let default_settings = AppSettings::default();
            if let Err(e) = save(app, &default_settings) {
                eprintln!("Warning: Failed to save default settings: {}", e);
            }
            default_settings
        }
    }
}

fn load_existing(app: &AppHandle) -> Result<AppSettings, String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;

    match store.get(SETTINGS_KEY) {
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|e| format!("Failed to deserialize settings: {}", e)),
        None => Err("Settings key not found in store".to_string()),
    }
}

pub fn save(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;

    let value = serde_json::to_value(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    store.set(SETTINGS_KEY, value);
    store.save().map_err(|e| e.to_string())?;

    Ok(())
}

/// Update a single field by dot-notation key
pub fn update_field(app: &AppHandle, key: &str, value: serde_json::Value) -> Result<(), String> {
    let mut settings = load_or_create(app);

    let parts: Vec<&str> = key.split('.').collect();

    match parts.as_slice() {
        // App settings
        ["app", field] => update_app_field(&mut settings.app, field, value)?,

        // Shortcuts
        ["shortcuts", field] => update_shortcuts_field(&mut settings.shortcuts, field, value)?,

        // Download settings
        ["download", field] => update_download_field(&mut settings.download, field, value)?,

        // Network settings
        ["network", field] => update_network_field(&mut settings.network, field, value)?,

        // Proxy settings (nested under network)
        ["network", "proxy", field] => {
            update_proxy_field(&mut settings.network.proxy, field, value)?
        }

        // Session settings
        ["session", field] => update_session_field(&mut settings.session, field, value)?,

        // Top-level flags
        ["send_anonymous_metrics"] => {
            settings.send_anonymous_metrics = value.as_bool().unwrap_or(false);
        }
        ["show_notifications"] => {
            settings.show_notifications = value.as_bool().unwrap_or(true);
        }
        ["notification_sound"] => {
            settings.notification_sound = value.as_bool().unwrap_or(true);
        }

        _ => return Err(format!("Unknown setting key: {}", key)),
    }

    // Validate before saving
    settings.validate();
    save(app, &settings)
}

fn update_app_field(
    config: &mut AppConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "show_tray_icon" => config.show_tray_icon = value.as_bool().unwrap_or(true),
        "quit_on_close" => config.quit_on_close = value.as_bool().unwrap_or(false),
        "sidebar" => config.sidebar = value.as_str().unwrap_or("left").to_string(),
        "theme" => config.theme = value.as_str().unwrap_or("system").to_string(),
        "button_label" => config.button_label = value.as_str().unwrap_or("both").to_string(),
        "show_download_progress" => config.show_download_progress = value.as_bool().unwrap_or(true),
        "show_segment_progress" => config.show_segment_progress = value.as_bool().unwrap_or(true),
        "autostart" => config.autostart = value.as_bool().unwrap_or(false),
        "auto_resume" => config.auto_resume = value.as_bool().unwrap_or(false),
        _ => return Err(format!("Unknown app field: {}", field)),
    }
    Ok(())
}

fn update_shortcuts_field(
    config: &mut ShortcutConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let shortcut = value.as_str().unwrap_or("").to_string();
    match field {
        "go_home" => config.go_home = shortcut,
        "open_settings" => config.open_settings = shortcut,
        "add_download" => config.add_download = shortcut,
        "open_details" => config.open_details = shortcut,
        "open_history" => config.open_history = shortcut,
        "toggle_sidebar" => config.toggle_sidebar = shortcut,
        "cancel_download" => config.cancel_download = shortcut,
        "quit_app" => config.quit_app = shortcut,
        _ => return Err(format!("Unknown shortcuts field: {}", field)),
    }
    Ok(())
}

fn update_download_field(
    config: &mut DownloadConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "download_location" => config.download_location = value.as_str().unwrap_or("").to_string(),
        "num_threads" => config.num_threads = value.as_u64().unwrap_or(8) as u8,
        "max_concurrent" => config.max_concurrent = value.as_u64().unwrap_or(0) as u8,
        "speed_limit" => config.speed_limit = value.as_u64().unwrap_or(0),
        "conflict_action" => config.conflict_action = value.as_str().unwrap_or("ask").to_string(),
        _ => return Err(format!("Unknown download field: {}", field)),
    }
    Ok(())
}

fn update_network_field(
    config: &mut NetworkConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "user_agent" => config.user_agent = value.as_str().unwrap_or("chrome").to_string(),
        "custom_user_agent" => config.custom_user_agent = value.as_str().unwrap_or("").to_string(),
        "connect_timeout_secs" => config.connect_timeout_secs = value.as_u64().unwrap_or(15) as u16,
        "read_timeout_secs" => config.read_timeout_secs = value.as_u64().unwrap_or(30) as u16,
        "retry_count" => config.retry_count = value.as_u64().unwrap_or(3) as u8,
        "retry_delay_ms" => config.retry_delay_ms = value.as_u64().unwrap_or(1000) as u32,
        "allow_insecure" => config.allow_insecure = value.as_bool().unwrap_or(false),
        _ => return Err(format!("Unknown network field: {}", field)),
    }
    Ok(())
}

fn update_proxy_field(
    config: &mut ProxyConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "enabled" => config.enabled = value.as_bool().unwrap_or(false),
        "proxy_type" => config.proxy_type = value.as_str().unwrap_or("http").to_string(),
        "host" => config.host = value.as_str().unwrap_or("").to_string(),
        "port" => config.port = value.as_u64().unwrap_or(8080) as u16,
        "auth_enabled" => config.auth_enabled = value.as_bool().unwrap_or(false),
        "username" => config.username = value.as_str().unwrap_or("").to_string(),
        "password" => config.password = value.as_str().unwrap_or("").to_string(),
        _ => return Err(format!("Unknown proxy field: {}", field)),
    }
    Ok(())
}

fn update_session_field(
    config: &mut SessionConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "history" => config.history = value.as_bool().unwrap_or(true),
        "metadata" => config.metadata = value.as_bool().unwrap_or(true),
        _ => return Err(format!("Unknown session field: {}", field)),
    }
    Ok(())
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    load_or_create(&app)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, mut settings: AppSettings) -> Result<(), String> {
    settings.validate();
    save(&app, &settings)
}

#[tauri::command]
pub fn update_setting(app: AppHandle, key: String, value: serde_json::Value) -> Result<(), String> {
    update_field(&app, &key, value)
}

use super::config::AppSettings;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "settings.json";
const SETTINGS_KEY: &str = "settings";

pub fn load_or_create(app: &AppHandle) -> AppSettings {
    match load_existing(app) {
        Ok(settings) => settings,
        Err(_) => {
            // Store doesn't exist or is corrupted, create with defaults
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
    
    // Check if store exists and has our settings key
    match store.get(SETTINGS_KEY) {
        Some(value) => {
            serde_json::from_value(value.clone())
                .map_err(|e| format!("Failed to deserialize settings: {}", e))
        }
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

pub fn update_field(app: &AppHandle, key: &str, value: serde_json::Value) -> Result<(), String> {
    let mut settings = load_or_create(app);
    
    let parts: Vec<&str> = key.split('.').collect();
    
    match parts.as_slice() {
        ["app", field] => {
            update_app_field(&mut settings.app, field, value)?;
        }
        ["shortcuts", field] => {
            update_shortcuts_field(&mut settings.shortcuts, field, value)?;
        }
        ["download", field] => {
            update_download_field(&mut settings.download, field, value)?;
        }
        ["thread", field] => {
            update_thread_field(&mut settings.thread, field, value)?;
        }
        ["session", field] => {
            update_session_field(&mut settings.session, field, value)?;
        }
        ["send_anonymous_metrics"] => {
            settings.send_anonymous_metrics = value.as_bool().unwrap_or(false);
        }
        ["show_notifications"] => {
            settings.show_notifications = value.as_bool().unwrap_or(true);
        }
        _ => return Err(format!("Unknown setting key: {}", key)),
    }
    
    save(app, &settings)
}

fn update_app_field(
    config: &mut super::config::AppConfig,
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
        _ => return Err(format!("Unknown app field: {}", field)),
    }
    Ok(())
}

fn update_shortcuts_field(
    config: &mut super::config::ShortcutConfig,
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
    config: &mut super::config::DownloadConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "download_location" => config.download_location = value.as_str().unwrap_or("").to_string(),
        "num_threads" => config.num_threads = value.as_u64().unwrap_or(8) as u8,
        "chunk_size" => config.chunk_size = value.as_u64().unwrap_or(16) as u32,
        "socket_buffer_size" => config.socket_buffer_size = value.as_u64().unwrap_or(0) as u32,
        "speed_limit" => config.speed_limit = value.as_u64().unwrap_or(0),
        _ => return Err(format!("Unknown download field: {}", field)),
    }
    Ok(())
}

fn update_thread_field(
    config: &mut super::config::ThreadConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "total_connections" => config.total_connections = value.as_u64().unwrap_or(1) as u8,
        "per_task_connections" => config.per_task_connections = value.as_u64().unwrap_or(1) as u8,
        _ => return Err(format!("Unknown thread field: {}", field)),
    }
    Ok(())
}

fn update_session_field(
    config: &mut super::config::SessionConfig,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    match field {
        "history" => config.history = value.as_bool().unwrap_or(false),
        "metadata" => config.metadata = value.as_bool().unwrap_or(false),
        _ => return Err(format!("Unknown session field: {}", field)),
    }
    Ok(())
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    load_or_create(&app)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    save(&app, &settings)
}

#[tauri::command]
pub fn update_setting(app: AppHandle, key: String, value: serde_json::Value) -> Result<(), String> {
    update_field(&app, &key, value)
}
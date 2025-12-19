use serde_json::json;

use tauri::{Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_deep_link::DeepLinkExt;

// use crate::download_manager::DownloadManager;
pub mod args;
pub mod database;
pub mod downloads;
pub mod settings;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let parsed_args = args::AppArgs::parse_from_vec(&args);

            // Handle deep link if present
            if let Some(url_str) = &parsed_args.deep_link {
                if let Some((url, _filename, _size_opt)) = downloads::parse_deep_link_url(url_str) {
                    // Emit event to frontend to handle deep link
                    let _ = app.emit(
                        "deep-link-received",
                        json!({
                            "url": url.as_str(),
                            "type": "startup"
                        }),
                    );
                }
            }

            // Show window unless minimized
            if let Some(window) = app.get_webview_window("main") {
                if !parsed_args.minimized {
                    let _ = window.show();
                    let _ = window.set_focus();
                } else {
                    let _ = window.hide();
                }
            }
        }))
        .invoke_handler(tauri::generate_handler![
            settings::get_settings,
            settings::update_settings,
            settings::update_setting,
            get_autostart,
            set_autostart,
            downloads::manager::handle_download_request,
            downloads::manager::pause_download,
            downloads::manager::cancel_download,
            downloads::manager::is_download_active,
            downloads::manager::active_download_count,
        ])
        .setup(|app| {
            // Initialize and manage DownloadManager
            let download_manager = downloads::DownloadManager::new();
            app.manage(download_manager);

            // Parse command line arguments
            let args = args::AppArgs::parse();

            // Handle deep links from startup
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    if let Some((parsed_url, _filename, _size_opt)) =
                        downloads::parse_deep_link_url(url.as_str())
                    {
                        let _ = app.emit(
                            "deep-link-received",
                            json!({
                                "url": parsed_url.as_str(),
                                "type": "startup"
                            }),
                        );
                    }
                }
            }

            // Handle deep link from command line
            if let Some(url) = &args.deep_link {
                if let Some((parsed_url, _filename, _size_opt)) =
                    downloads::parse_deep_link_url(url)
                {
                    let _ = app.emit(
                        "deep-link-received",
                        json!({
                            "url": parsed_url.as_str(),
                            "type": "command_line"
                        }),
                    );
                }
            }

            // Handle minimized startup
            if args.minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();

    if enabled {
        autostart.enable().map_err(|e| e.to_string())
    } else {
        autostart.disable().map_err(|e| e.to_string())
    }
}

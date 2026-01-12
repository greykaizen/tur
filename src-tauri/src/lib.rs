use serde_json::json;

use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_deep_link::DeepLinkExt;

pub mod args;
pub mod database;
pub mod downloads;
pub mod menu;
pub mod native_host;
pub mod queue;
pub mod settings;
pub mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Conditional tracing configuration
    #[cfg(debug_assertions)]
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    #[cfg(not(debug_assertions))]
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let parsed_args = args::AppArgs::parse_from_vec(&args);

            // Handle download URL from CLI/extension
            if let Some(url) = &parsed_args.download_url {
                tracing::info!("[tur] Single-instance: received download URL: {}", url);
                open_download_window_internal(
                    app.clone(),
                    url.clone(),
                    parsed_args.format_id.clone(),
                    parsed_args.audio_id.clone(),
                    parsed_args.title.clone(),
                    parsed_args.filesize,
                    parsed_args.ext.clone(),
                    parsed_args.video_stream_url.clone(),
                    parsed_args.audio_stream_url.clone(),
                );
                return; // Don't show main window
            }

            // Handle deep link if present
            if let Some(url_str) = &parsed_args.deep_link {
                if let Some((url, _filename, _size_opt)) = downloads::parse_deep_link_url(url_str) {
                    let _ = app.emit(
                        "deep-link-received",
                        json!({
                            "url": url.as_str(),
                            "type": "startup"
                        }),
                    );
                }
            }

            // Show window unless minimized - this wakes app from background
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
            get_default_download_path,
            close_download_window,
            open_download_window,
            open_download_options_window,
            start_ytdlp_download,
            downloads::manager::handle_download_request,
            downloads::manager::pause_download,
            downloads::manager::cancel_download,
            downloads::manager::is_download_active,
            downloads::manager::active_download_count,
            downloads::manager::get_download_history,
            downloads::manager::request_shutdown,
            downloads::manager::delete_download,
            delete_download_file,
            open_path,
            queue::create_queue,
            queue::get_queues,
            queue::delete_queue,
            queue::check_queue_progression,
            downloads::manager::reorder_queue,
            downloads::manager::pause_queue,
            downloads::manager::resume_queue,
            downloads::manager::cancel_queue,
            downloads::manager::manager_add_to_queue,
            downloads::manager::manager_remove_from_queue,
            downloads::manager::set_download_schedule,
            downloads::manager::clear_download_schedule,
            downloads::manager::start_download_now,
            downloads::manager::set_queue_dependency,
            downloads::manager::clear_queue_dependency,
            downloads::manager::get_bandwidth_windows,
            downloads::manager::set_bandwidth_windows,
            queue::update_queue_status,
            dependencies::check_dependency,
            dependencies::install_dependency,
            dependencies::update_dependency,
        ])
        .on_window_event(|window, event| {
            // Handle window close request based on settings
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Only handle close prevention for main window
                // Download windows should close normally
                if window.label() == "main" {
                    let app = window.app_handle();
                    let should_prevent = tray::handle_close_requested(app);

                    if should_prevent {
                        // Prevent actual close, window is hidden
                        api.prevent_close();
                    }
                }
                // Download windows and other windows close without intervention
            }
        })
        .on_menu_event(|app, event| {
            menu::handle_menu_event(app, event.id().as_ref());
        })
        .setup(|app| {
            // Initialize and manage Manager
            let manager = downloads::manager::spawn_manager(app.handle().clone());
            app.manage(manager);

            // Start background queue processor / auto resume
            let handle_for_bg = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Initial short delay to allow app to settle
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let manager = handle_for_bg.state::<downloads::manager::ManagerHandle>();
                manager.check_queue().await;
                // Explicitly check schedules on startup
                manager.trigger_schedule_evaluation();
            });

            // Start graceful shutdown signal handler
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let manager = handle.state::<downloads::manager::ManagerHandle>();
                manager.start_signal_handler(handle.clone()).await;
            });

            // Set up global menu
            if let Ok(menu) = menu::create_menu(app.handle()) {
                let _ = app.set_menu(menu);
            }

            // Set up system tray based on settings
            if let Err(e) = tray::setup_tray(app.handle()) {
                tracing::warn!("Failed to set up system tray: {}", e);
            }

            // Parse command line arguments
            let args = args::AppArgs::parse();

            // Detect native messaging mode by checking argv[1]
            // Chrome passes "chrome-extension://EXTENSION_ID/" as the first argument
            // This is the reliable, production-grade way to detect native messaging
            let is_native_messaging = std::env::args()
                .nth(1)
                .map(|arg| arg.starts_with("chrome-extension://"))
                .unwrap_or(false);

            if is_native_messaging || args.native_messaging {
                tracing::info!("[tur] Starting in native messaging mode");
                let rx = native_host::start_native_messaging_thread(app.handle().clone());
                let app_handle = app.handle().clone();

                // Spawn thread to handle download actions from native messaging
                std::thread::spawn(move || {
                    tracing::info!("[tur] Download action handler thread started");
                    while let Ok(action) = rx.recv() {
                        tracing::debug!("[tur] Received action from native messaging");
                        match action {
                            native_host::HostAction::OpenDownload {
                                url,
                                format_id,
                                title,
                                filesize,
                                ext,
                                video_stream_url,
                                audio_stream_url,
                            } => {
                                tracing::info!("[tur] OpenDownload action received, url: {}", url);
                                tracing::debug!("[tur] Calling open_download_window_internal...");
                                open_download_window_internal(
                                    app_handle.clone(),
                                    url,
                                    format_id,
                                    None, // audio_id
                                    title,
                                    filesize,
                                    ext,
                                    video_stream_url,
                                    audio_stream_url,
                                );
                                tracing::debug!("[tur] open_download_window_internal returned");
                            }
                        }
                    }
                    tracing::info!("[tur] Download action handler thread ended");
                });
            }

            // Handle deep links from startup - open download window
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    if let Some((parsed_url, _filename, _size_opt)) =
                        downloads::parse_deep_link_url(url.as_str())
                    {
                        open_download_window_internal(
                            app.handle().clone(),
                            parsed_url.to_string(),
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        );
                    }
                }
            }

            // Handle deep link from command line - open download window
            if let Some(url) = &args.deep_link {
                if let Some((parsed_url, _filename, _size_opt)) =
                    downloads::parse_deep_link_url(url)
                {
                    open_download_window_internal(
                        app.handle().clone(),
                        parsed_url.to_string(),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    );
                }
            }

            // Handle direct download URL from CLI (from extension)
            if let Some(url) = &args.download_url {
                open_download_window_internal(
                    app.handle().clone(),
                    url.clone(),
                    args.format_id.clone(),
                    args.audio_id.clone(),
                    args.title.clone(),
                    args.filesize,
                    args.ext.clone(),
                    args.video_stream_url.clone(),
                    args.audio_stream_url.clone(),
                );
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

/// Get the default OS download path
#[tauri::command]
fn get_default_download_path(app: tauri::AppHandle) -> Result<String, String> {
    app.path()
        .download_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Close a download window by its label
#[tauri::command]
fn close_download_window(app: tauri::AppHandle, label: String) -> Result<(), String> {
    use tauri::Manager;
    tracing::info!("[tur] Closing download window: {}", label);

    if let Some(window) = app.get_webview_window(&label) {
        window.close().map_err(|e| e.to_string())
    } else {
        Err(format!("Window '{}' not found", label))
    }
}

/// Start a yt-dlp download with format selection
#[tauri::command]
async fn start_ytdlp_download(
    app: tauri::AppHandle,
    url: String,
    format_id: String,
    audio_id: Option<String>,
    output_path: String,
    filename: String,
) -> Result<String, String> {
    use crate::dependencies::{DependencyKind, DependencyManager};
    use std::process::Command;

    tracing::info!(
        "[tur] Starting yt-dlp download: {} format: {} audio: {:?} -> {}/{}",
        url,
        format_id,
        audio_id,
        output_path,
        filename
    );

    // Find yt-dlp binary using DependencyManager
    let settings = crate::settings::store::load_or_create(&app);
    let mut manager = DependencyManager::new(&app, &settings.dependencies);
    let ytdlp_path = manager
        .find(DependencyKind::YtDlp)
        .ok_or_else(|| "yt-dlp not found. Please install yt-dlp.".to_string())?;
    let ytdlp = ytdlp_path.to_string_lossy().to_string();

    // Build format string: video+audio or just video
    let format_str = if let Some(audio) = audio_id {
        format!("{}+{}", format_id, audio)
    } else {
        format_id.clone()
    };

    // Build output template
    let output_template = format!("{}/{}.%(ext)s", output_path, filename.replace('.', "_"));

    // Spawn yt-dlp in background
    let child = Command::new(&ytdlp)
        .args([
            "-f",
            &format_str,
            "-o",
            &output_template,
            "--no-warnings",
            "--progress",
            &url,
        ])
        .spawn()
        .map_err(|e| format!("Failed to start yt-dlp: {}", e))?;

    tracing::info!("[tur] yt-dlp started with PID: {:?}", child.id());

    Ok(format!("Download started: {}", filename))
}

/// Internal helper to open download window (sync, for setup context)
fn open_download_window_internal(
    app: tauri::AppHandle,
    url: String,
    format_id: Option<String>,
    audio_id: Option<String>,
    title: Option<String>,
    filesize: Option<u64>,
    ext: Option<String>,
    video_stream_url: Option<String>,
    audio_stream_url: Option<String>,
) {
    tracing::debug!(
        "[tur] open_download_window_internal called with url: {}",
        url
    );

    // Build query string
    let mut query = format!("url={}", urlencoding::encode(&url));
    if let Some(f) = format_id {
        query.push_str(&format!("&format={}", urlencoding::encode(&f)));
    }
    if let Some(a) = audio_id {
        query.push_str(&format!("&audio={}", urlencoding::encode(&a)));
    }
    if let Some(t) = title {
        query.push_str(&format!("&title={}", urlencoding::encode(&t)));
    }
    if let Some(s) = filesize {
        query.push_str(&format!("&filesize={}", s));
    }
    if let Some(e) = ext {
        query.push_str(&format!("&ext={}", urlencoding::encode(&e)));
    }
    // Stream URLs for direct download
    if let Some(vs) = video_stream_url {
        query.push_str(&format!("&videoStreamUrl={}", urlencoding::encode(&vs)));
    }
    if let Some(as_) = audio_stream_url {
        query.push_str(&format!("&audioStreamUrl={}", urlencoding::encode(&as_)));
    }

    tracing::debug!("[tur] Query string: {}", query);

    // Create unique window label for each download
    let window_label = format!(
        "download_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    tracing::info!("[tur] Creating download window: {}", window_label);
    let webview_url = tauri::WebviewUrl::App(format!("/download?{}", query).into());

    if let Err(e) = tauri::WebviewWindowBuilder::new(&app, &window_label, webview_url)
        .title("Download")
        .inner_size(728.0, 428.0)
        .decorations(true) // Native title bar for drag/close
        .resizable(false)
        .center()
        .build()
    {
        tracing::error!("[tur] Failed to create download window: {}", e);
    } else {
        tracing::info!("[tur] Download window created successfully");
    }
}

/// Open the download options window with URLs
#[tauri::command]
async fn open_download_options_window(
    app: tauri::AppHandle,
    urls: Vec<String>,
) -> Result<(), String> {
    let window_label = format!(
        "download_options_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let json_urls = serde_json::to_string(&urls).map_err(|e| e.to_string())?;
    let encoded_urls = urlencoding::encode(&json_urls);
    let url = format!("/download-options?urls={}", encoded_urls);

    let webview_url = tauri::WebviewUrl::App(url.into());

    let _window = tauri::WebviewWindowBuilder::new(&app, &window_label, webview_url)
        .title("Download Options")
        .inner_size(520.0, 600.0)
        .decorations(false)
        .transparent(true)
        .center()
        .always_on_top(true)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Open the download dialog window with URL parameters
#[tauri::command]
async fn open_download_window(
    app: tauri::AppHandle,
    url: String,
    format_id: Option<String>,
    audio_id: Option<String>,
) -> Result<(), String> {
    // Build query string
    let mut query = format!("url={}", urlencoding::encode(&url));
    if let Some(f) = format_id {
        query.push_str(&format!("&format={}", urlencoding::encode(&f)));
    }
    if let Some(a) = audio_id {
        query.push_str(&format!("&audio={}", urlencoding::encode(&a)));
    }

    // Get or create download window
    if let Some(win) = app.get_webview_window("download") {
        // Navigate to URL with params and show
        let nav_url = format!("/download?{}", query);
        let _ = win.eval(&format!("window.location.href = '{}'", nav_url));
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
    } else {
        // Create new window
        let url = tauri::WebviewUrl::App(format!("/download?{}", query).into());
        tauri::WebviewWindowBuilder::new(&app, "download", url)
            .title("Download")
            .inner_size(728.0, 428.0)
            .build()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Open a path in system explorer or default app
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Delete a file from disk
#[tauri::command]
fn delete_download_file(file_path: String) -> Result<(), String> {
    use std::fs;
    use std::path::Path;

    let path = Path::new(&file_path);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
        tracing::info!("[tur] Deleted file: {}", file_path);
        Ok(())
    } else {
        Err("File not found".to_string())
    }
}
pub mod dependencies;

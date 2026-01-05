//! System tray management
//!
//! Handles system tray icon, menu, and window close behavior based on settings:
//! - `show_tray_icon: true` → Show tray icon, minimize to tray on close
//! - `show_tray_icon: false` → No tray, minimize to background on close
//! - `quit_on_close: true` → Actually exit the app on window close
//!
//! The single-instance plugin handles waking the app when user double-clicks binary again.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::settings::store;

/// Set up the system tray based on current settings
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings = store::load_or_create(app);

    if settings.app.show_tray_icon {
        create_tray(app)?;
    }

    Ok(())
}

/// Create the system tray with menu
fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    // Build menu
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    // Build tray icon
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("tur - Download Manager")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    // Use the shutdown mechanism
                    let manager = app.state::<crate::downloads::DownloadManager>();
                    manager.shutdown_all();
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            // Left click shows window
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Handle window close request based on settings
/// Returns true if the close should be prevented (minimize instead)
/// Returns false if the app should actually close
pub fn handle_close_requested(app: &AppHandle) -> bool {
    let settings = store::load_or_create(app);

    // If quit_on_close is true, allow the close (don't prevent)
    if settings.app.quit_on_close {
        // Stop all downloads first
        let manager = app.state::<crate::downloads::DownloadManager>();
        manager.shutdown_all();
        return false; // Allow close
    }

    // Otherwise, hide the window (minimize to tray/background)
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    true // Prevent actual close (window is just hidden)
}

/// Update tray visibility based on settings change
pub fn update_tray_visibility(app: &AppHandle, show_tray: bool) {
    // In Tauri 2, we need to manage tray creation/destruction
    // For now, we'll handle this through app restart or just create if needed
    if show_tray {
        let _ = create_tray(app);
    }
    // Note: Removing tray at runtime requires storing the TrayIcon handle
    // For production, we'd store the handle in app state and call .destroy()
}

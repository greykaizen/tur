// Global Menu for tur download manager
// Provides File, Downloads, View, Settings, Help menus
// Cross-platform: macOS (native bar), Windows/Linux (in-window or native)

use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Emitter, Runtime,
};

/// Create the application menu
pub fn create_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    // File menu
    let file_menu = SubmenuBuilder::new(app, "File")
        .item(
            &MenuItemBuilder::with_id("add_url", "Add URL...")
                .accelerator("CmdOrCtrl+N")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("add_clipboard", "Add from Clipboard")
                .accelerator("CmdOrCtrl+Shift+V")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("import_links", "Import Links...").build(app)?)
        .item(&MenuItemBuilder::with_id("export_list", "Export Download List...").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Quit"))?)
        .build()?;

    // Downloads menu
    let downloads_menu = SubmenuBuilder::new(app, "Downloads")
        .item(&MenuItemBuilder::with_id("start_all", "Start All").build(app)?)
        .item(&MenuItemBuilder::with_id("pause_all", "Pause All").build(app)?)
        .item(&MenuItemBuilder::with_id("resume_all", "Resume All").build(app)?)
        .item(&MenuItemBuilder::with_id("cancel_all", "Cancel All").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("create_queue", "Create Queue...")
                .accelerator("CmdOrCtrl+Shift+Q")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("manage_queues", "Manage Queues...").build(app)?)
        .build()?;

    // View menu
    let view_menu = SubmenuBuilder::new(app, "View")
        .item(
            &MenuItemBuilder::with_id("view_home", "Home")
                .accelerator("CmdOrCtrl+K")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("view_details", "Details")
                .accelerator("CmdOrCtrl+D")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("view_history", "History")
                .accelerator("CmdOrCtrl+H")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("toggle_sidebar", "Toggle Sidebar")
                .accelerator("CmdOrCtrl+L")
                .build(app)?,
        )
        .build()?;

    // Help menu
    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id("documentation", "Documentation").build(app)?)
        .item(&MenuItemBuilder::with_id("shortcuts", "Keyboard Shortcuts").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("check_updates", "Check for Updates").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("about", "About tur").build(app)?)
        .build()?;

    // Build full menu
    let menu = MenuBuilder::new(app)
        .item(&file_menu)
        .item(&downloads_menu)
        .item(&view_menu)
        .item(&help_menu)
        .build()?;

    Ok(menu)
}

/// Handle menu item clicks
pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        // File menu
        "add_url" => {
            let _ = app.emit("menu-action", "add_url");
        }
        "add_clipboard" => {
            let _ = app.emit("menu-action", "add_clipboard");
        }
        "import_links" => {
            let _ = app.emit("menu-action", "import_links");
        }
        "export_list" => {
            let _ = app.emit("menu-action", "export_list");
        }

        // Downloads menu
        "start_all" => {
            let _ = app.emit("menu-action", "start_all");
        }
        "pause_all" => {
            let _ = app.emit("menu-action", "pause_all");
        }
        "resume_all" => {
            let _ = app.emit("menu-action", "resume_all");
        }
        "cancel_all" => {
            let _ = app.emit("menu-action", "cancel_all");
        }
        "create_queue" => {
            let _ = app.emit("menu-action", "create_queue");
        }
        "manage_queues" => {
            let _ = app.emit("menu-action", "manage_queues");
        }

        // View menu
        "view_home" => {
            let _ = app.emit("menu-action", "view_home");
        }
        "view_details" => {
            let _ = app.emit("menu-action", "view_details");
        }
        "view_history" => {
            let _ = app.emit("menu-action", "view_history");
        }
        "toggle_sidebar" => {
            let _ = app.emit("menu-action", "toggle_sidebar");
        }

        // Help menu
        "documentation" => {
            let _ = app.emit("menu-action", "documentation");
        }
        "shortcuts" => {
            let _ = app.emit("menu-action", "shortcuts");
        }
        "check_updates" => {
            let _ = app.emit("menu-action", "check_updates");
        }
        "about" => {
            let _ = app.emit("menu-action", "about");
        }

        _ => {}
    }
}

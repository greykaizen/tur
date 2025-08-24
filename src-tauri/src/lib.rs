use tauri::{AppHandle, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use uuid::Uuid;

use crate::config::InstanceConfig;

pub mod config;
pub mod db;
pub mod download;
pub mod download_manager;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

// #[tauri::command]
// fn greet(name: &str) -> String {
//     format!("Hello, {}! You've been greeted from Rust!", name)
// }

pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            let _ = app
                .get_webview_window("main")
                .expect("no main window")
                .set_focus();
        }));
    }

    builder
        .plugin(tauri_plugin_deep_link::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// #[cfg_attr(mobile, tauri::mobile_entry_point)]
// pub fn run() {
//     tauri::Builder::default()
//     .plugin(tauri_plugin_single_instance::init())
//     // .plugin(tauri::Builder::default().build())
//     .setup(|app| Ok(()))
//     .invoke_handler(tauri::generate_handler![handle_instance])
//     .run(tauri::generate_context!())
//     .expect("run");
// }

// #[cfg_attr(mobile, tauri::mobile_entry_point)]
// pub fn run() {
//     tauri::Builder::default()
// .plugin(tauri_plugin_deep_link::init())
// .plugin(tauri_plugin_store::Builder::new().build())
//         .plugin(tauri_plugin_opener::init())
//         .invoke_handler(tauri::generate_handler![greet])
//         .run(tauri::generate_context!())
//         .expect("error while running tauri application");
// }

// ------------------------------------------

// #[tauri::command]
// fn get_config(
//     store: State<tauri_plugin_store::Store<tauri::Wry>>,
//     key: String,
// ) -> Result<serde_json::Value, String> {
//     store.get(&key).ok_or("Key not found".to_string())
// }

// #[tauri::command]
// fn set_config(
//     store: State<tauri_plugin_store::Store<tauri::Wry>>,
//     key: String,
//     value: serde_json::Value,
// ) -> Result<(), String> {
//     store.set(&key, value);
//     Ok(())
// }
// #[derive(Default)]
// struct MyState {
//   s: std::sync::Mutex<String>,
//   t: std::sync::Mutex<std::collections::HashMap<String, String>>,
// }
// // remember to call `.manage(MyState::default())`
// #[tauri::command]
// async fn command_name(state: tauri::State<'_, MyState>) -> Result<(), String> {
//   *state.s.lock().unwrap() = "new string".into();
//   state.t.lock().unwrap().insert("key".into(), "value".into());
//   Ok(())
// }

#[derive(serde::Deserialize)]
#[serde(tag = "type", content = "data")]
enum InstanceTarget {
    Urls(Vec<String>),
    Uuids(Vec<Uuid>),
}

// store configs
// let instance_cfg = InstanceConfig::from(&app_config);
// for new instances
#[tauri::command]
async fn handle_instance(instance_cfg: InstanceConfig, target: InstanceTarget) {
    match target {
        InstanceTarget::Urls(urls) => {
            // --- new_instance
            // all headers comes in
            // save info to db, get from tauri store and send info to start instance
        }
        InstanceTarget::Uuids(uuids) => {
            // --- resume_instance (for resuming old instances)
            // History shows
            // uuid(not shown but there), Name, Status, Date,
            // so frontend sends back uuid

            // from store
            // conns

            // get from db via uuid
            // uuid (came from frontend)
            // filename
            // size
            // url
            // Etag (consume & drop)
            // Last-Modified (consume & drop)
            // dest. location

            // check file existance on dest. location, if not there start from scratch via Download::new(id, size, num_conn)

            // what's need to start work

            // Download::load(apphandle, uuid)

            // first emit (as req. came we check dest. & metadata then emit and start client)
            // uuid
            // filename
            // size
            // url

            // second emit (client.await is completed, we check etag/last modified, emit resume, client starts working)
            // resume
            // Etag
            // Last-Modified
        }
    }
}

// for instances that are already in history
#[tauri::command]
fn instance_action(id: Vec<usize>, action: u8) {
    // actions: cancel(0), start(1), pause(2)  (assuming item is already in DM)
    match action {
        // 0 => engine::DownloadManager::pause_instance(id),
        // 1 => engine::DownloadManager::start_instance(id),
        // 2 => engine::DownloadManager::cancel_instance(id),
        _ => unreachable!(),
    }
}

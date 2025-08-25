use serde_json::json;
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use uuid::Uuid;

use crate::config::InstanceConfig;

pub mod config;
pub mod db;
pub mod download;
pub mod download_manager;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Handle deep link when a second instance is launched (warm start)
            if let Some(url_str) = args.iter().find(|arg| arg.starts_with("tur://")) {
                handle_deep_link(app, url_str);
            }

            // Always focus main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    handle_deep_link(app.handle(), url.as_str());
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_deep_link(app: &tauri::AppHandle, url_str: &str) {
    let parsed = match url::Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return,
    };

    // TODO reading other header etag, last-modified, partial-content etc
    let src_url = match parsed.query_pairs().find(|(k, _)| k == "url") {
        Some((_, v)) => v.to_string(),
        _ => return,
    };
    let filename = parsed
        .query_pairs()
        .find(|(k, _)| k == "filename")
        .map(|(_, v)| v.to_string());
    let size_opt = parsed
        .query_pairs()
        .find(|(k, _)| k == "size")
        .and_then(|(_, v)| v.parse::<u64>().ok());

    // Read settings from store
    let store = tauri_plugin_store::StoreExt::store(app, "settings.json").unwrap();

    let instance_cfg = InstanceConfig {
        download: store
            .get("download")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap(),
        thread: store
            .get("threads")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap(),
        session: store
            .get("session")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap(),
    };

    let id = uuid::Uuid::now_v7();

    // Start work with settings from store
    let target = InstanceTarget::Urls(vec![src_url.clone()]);
    handle_instance(instance_cfg, target); // Call your existing function


    let _ = app.emit(
        "download-request", // show UI for start and cancel
        json!({
            "id": id,
            "url": src_url, // might not be needed
            "filename": filename,
            "size": size_opt
        }),
    );
}

// ------------------------------------------

#[derive(serde::Deserialize)]
#[serde(tag = "type", content = "data")]
enum InstanceTarget {
    Urls(Vec<String>),
    Uuids(Vec<Uuid>),
    // new, (via new button/dragNdrop, non-encoded url, prep client and fetch headers)
    // deep-link (encoded url, prep client and start)
    // resume, (from frontend via history, via uuid, prep client) if sends url back then it could be workz
}

// for new instances
// creating instance of Download push it's handle to DMan
#[tauri::command]
fn handle_instance(instance_cfg: InstanceConfig, target: InstanceTarget) {
    match target {
        InstanceTarget::Urls(urls) => {
            // --- new_instance
            //    url comes in only via new button
            // OR all headers comes in deep link browser extension


            // save info to db, get from tauri store and send info to start instance
        }
        InstanceTarget::Uuids(uuids) => {
            // --- resume_instance (for resuming old instances, headers aren't available)
            // History shows
            // uuid(not shown but there), Name, Status, Date,
            // so frontend sends back uuid

            // frontend sends Instance Config for settings

            // get from db via uuid
            // uuid (came from frontend)
            // filename
            // size
            // url
            // Etag (consume & drop)
            // Last-Modified (consume & drop)
            // dest. location

            // check file existance on dest. location, if not there start from scratch via Download::new(id, size, num_conn)

            // prep asyn client

            // 1st .emit("queue_work") (as req. came we check dest. & metadata then emit and start client)
            // uuid
            // filename
            // size
            // url

            // await client, get header match

            // 2nd .emit("queue_work") (client.await is completed, we check etag/last modified, emit resume, client starts working)
            // resume
            // Etag
            // Last-Modified

            // Download() starts
        }
    }
}

// TODO removal after impl. the uuid to emit and listen for events
// for instances that are already in history
// #[tauri::command]
// fn instance_action(id: Vec<usize>, action: u8) {
//     // actions: cancel(0), start(1), pause(2)  (assuming item is already in DM)
//     match action {
//         // 0 => engine::DownloadManager::pause_instance(id),
//         // 1 => engine::DownloadManager::start_instance(id),
//         // 2 => engine::DownloadManager::cancel_instance(id),
//         _ => unreachable!(),
//     }
// }

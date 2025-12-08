use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use uuid::Uuid;

use crate::{config::InstanceConfig, download_manager::DownloadManager};

pub mod config;
pub mod db;
pub mod download;
pub mod download_manager;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

pub fn run() {
    tauri::Builder::default()
        // .manage(DownloadManager::new())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // Handle deep link when a second instance is launched (warm start)
            if let Some(url_str) = args.iter().find(|arg| arg.starts_with("tur://")) {
                // TODO needs fixing here to Instancetarget::deep_link()
                handle_deep_link(app, url_str);
            }

            // Always focus main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            // Initialize WorkManager/DownloadManager here
            // let download_manager =
            //     DownloadManager::new(app.handle()).expect("Failed to initialize DownloadManager");
            // app.manage(download_manager);

            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    // TODO needs fixing here to Instancetarget::deep_link()
                    // TODO change the url.as_str() to normal url being passes
                    handle_deep_link(app.handle(), url.as_str());
                }
            }

            // let DMan object then app.manage()
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
    let target = InstanceTarget::deep_link(vec![src_url.clone()]);
    // handle_instance(instance_cfg, target); // Call your existing function

    let _ = app.emit(
        "download-request", // show UI for start and cancel
        json!({
            "id": id,
            "url": src_url, // might not be needed
            "filename": filename,
            "size": size_opt
            // partial content resume YES/NO
        }),
    );
}

// ------------------------------------------

#[derive(serde::Deserialize)]
#[serde(tag = "type", content = "data")]
enum InstanceTarget {
    new(Vec<String>),
    resume(Vec<Uuid>),
    deep_link(Vec<String>),
}

// for new instances
// creating instance of Download push it's handle to DMan
#[tauri::command]
async fn handle_instance(
    app: tauri::AppHandle,
    download_manager: tauri::State<'_, DownloadManager>,
    instance_cfg: InstanceConfig,
    target: InstanceTarget,
) -> Result<(), Box<dyn std::error::Error>> {
    // do prep client (outside match arms)
    let client = Client::builder() // Connection & Performance
        .timeout(Duration::from_secs(300)) // 5min total timeout (vs 30s default)
        .connect_timeout(Duration::from_secs(10)) // Connection establishment timeout
        .pool_max_idle_per_host(10) // Keep more connections alive (default: varies)
        .pool_idle_timeout(Duration::from_secs(90)) // Keep connections alive longer
        .tcp_keepalive(Duration::from_secs(60)) // Keep TCP connections alive
        // Download-specific
        // .gzip(true) // Auto decompress (default: true)
        // .brotli(true) // Brotli compression support (default: true)
        // .deflate(true) // Deflate compression (default: true)
        // Headers & User Agent
        .user_agent("MyDownloader/1.0") // Custom user agent (default: reqwest/version)
        // Redirects
        .redirect(reqwest::redirect::Policy::limited(10)) // Max redirects (default: 10)
        // Security (if needed)
        .danger_accept_invalid_certs(false) // Only if you need it (default: false)
        .https_only(false) // Allow HTTP (default: false)
        // HTTP/2
        .http2_adaptive_window(true) // Use HTTP/2 if server supports
        .build()
        .expect("client builder failed");

    match target {
        InstanceTarget::new(urls) => {
            // received vec
            for url in urls {
                // .get().send() (inside arm)
                let response = client.get(&url).send();

                // await client for work on between
                let client = response.await?;
                // load headers via client
                let headers = client.headers();
                let filename = headers
                    .get(reqwest::header::CONTENT_DISPOSITION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|cd| {
                        cd.split(';').find_map(|p| {
                            p.trim()
                                .strip_prefix("filename=")
                                .map(|s| s.trim_matches('"').to_string())
                        })
                    })
                    .unwrap_or_else(|| {
                        url.rsplit('/')
                            .next()
                            .and_then(|s| s.split('?').next())
                            .unwrap_or("download")
                            .to_string()
                    });
                let size = headers
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                let resume = headers
                    .get(reqwest::header::ACCEPT_RANGES)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.eq_ignore_ascii_case("bytes"))
                    .unwrap_or(false);
                let etag = headers
                    .get(reqwest::header::ETAG)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let last_modified = headers
                    .get(reqwest::header::LAST_MODIFIED)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                // generate ID
                let id = Uuid::now_v7();

                // store to db here

                // emit full
                let payload = json!({
                    "id": id,
                    "url": url, // might not be needed
                    "filename": filename,
                    "size": size,
                    "resume": resume, // TODO resume with 206
                    "etag": etag,
                    "last-modified": last_modified
                });
                if let Err(e) = app.emit("queue_download", payload) {
                    eprintln!("failed to emit event: {}", e);
                }

                // start work
                // DMan create Download instance then db store, push to vec and call it's run_instance
            }

            Ok(())
        }
        InstanceTarget::resume(uuids) => {
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

            // DMAN store to db
            // Download() starts

            // ---
            // received vec

            // loop isn't right we gotta do these in async as well
            // load existing record via uuid, match if right for use else redo connection (InstanceTarget::new)
            for uuid in uuids {
                // using download_manager.db we get the whole record via get_resume_info(uuid)
                // destination check for the file existance, if show window "download file missing" with restart again button
                // etag, last-mod for check & match, if mismatch then drop old etag/last-mod and retrieve new headers, parse new info from it then update_download.

                // 206 support from response

                // .get().send() (inside arm)
                // let response = client.get(&url).send();

                // await client for work on between
                // let client = response.await?;

                // load headers via client
                // let headers = client.headers();
                // let filename = headers
                //     .get(reqwest::header::CONTENT_DISPOSITION)
                //     .and_then(|v| v.to_str().ok())
                //     .and_then(|cd| {
                //         cd.split(';').find_map(|p| {
                //             p.trim()
                //                 .strip_prefix("filename=")
                //                 .map(|s| s.trim_matches('"').to_string())
                //         })
                //     })
                //     .unwrap_or_else(|| {
                //         url.rsplit('/')
                //             .next()
                //             .and_then(|s| s.split('?').next())
                //             .unwrap_or("download")
                //             .to_string()
                //     });
                // let size = headers
                //     .get(reqwest::header::CONTENT_LENGTH)
                //     .and_then(|v| v.to_str().ok())
                //     .and_then(|s| s.parse::<u64>().ok());
                // let resume = headers
                //     .get(reqwest::header::ACCEPT_RANGES)
                //     .and_then(|v| v.to_str().ok())
                //     .map(|s| s.eq_ignore_ascii_case("bytes"))
                //     .unwrap_or(false);
                // let etag = headers
                //     .get(reqwest::header::ETAG)
                //     .and_then(|v| v.to_str().ok())
                //     .map(|s| s.to_string());
                // let last_modified = headers
                //     .get(reqwest::header::LAST_MODIFIED)
                //     .and_then(|v| v.to_str().ok())
                //     .map(|s| s.to_string());

                // // generate ID
                // let id = Uuid::now_v7();

                // // emit full
                // let payload = json!({
                //     "id": id,
                //     "url": url, // might not be needed
                //     "filename": filename,
                //     "size": size,
                //     "resume": resume,
                //     "etag": etag,
                //     "last-modified": last_modified
                // });
                // if let Err(e) = app.emit("queue_download", payload) {
                //     eprintln!("failed to emit event: {}", e);
                // }
            }
            // await client
            // via client load headers
            // emit twice
            // start work
            Ok(())
        }
        InstanceTarget::deep_link(encoded_urls) => {
            // received vec

            // // loop vec
            // decode urls + headers(for emit only)
            // .get().send() (inside arm)

            // emit full
            // await client
            // start work
            Ok(())
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

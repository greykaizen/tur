use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{Emitter, Listener, Manager};

use tur_lib::database::Database;
use tur_lib::downloads::manager::NewDownloadItem;
use tur_lib::downloads::{DownloadManager, DownloadRequest};
use url::Url;

#[derive(Clone, Debug, serde::Deserialize)]
struct WorkerEvent {
    download_id: String,
    worker_id: u8,
    state_bits: u8,
    current_unit: usize,
    unit_start_offset: u64,
    unit_end_offset: u64,
}

struct WorkerInfo {
    last_update: Instant,
    current_unit: usize,
    start_offset: u64,
    end_offset: u64,
    state_bits: u8,
    speed: f64, // MB/s
}

#[tokio::main]
async fn main() {
    println!("Initializing Test Harness...");

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                run_test_logic(app_handle).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn run_test_logic(app: tauri::AppHandle) {
    // 2. Init DB
    let db = Database::initialize(&app).expect("Failed to init DB");

    // 3. Init Manager
    let manager = DownloadManager::new();
    app.manage(manager);
    let manager = app.state::<DownloadManager>();

    // Shared Worker State
    let worker_infos: Arc<Mutex<HashMap<u8, WorkerInfo>>> = Arc::new(Mutex::new(HashMap::new()));
    let worker_infos_clone = worker_infos.clone();

    // Event Listener
    let _ = app.listen("worker_progress", move |event| {
        if let Ok(payload) = serde_json::from_str::<WorkerEvent>(event.payload()) {
            let mut guard = worker_infos_clone.lock().unwrap();
            let now = Instant::now();

            let speed = if let Some(old) = guard.get(&payload.worker_id) {
                // Calculate speed based on elapsed time between events (approx 1MB chunks)
                let elapsed = now.duration_since(old.last_update).as_secs_f64();
                if elapsed > 0.0 {
                    1.0 / elapsed // 1 MB / elapsed seconds = MB/s
                } else {
                    old.speed
                }
            } else {
                0.0
            };

            guard.insert(
                payload.worker_id,
                WorkerInfo {
                    last_update: now,
                    current_unit: payload.current_unit,
                    start_offset: payload.unit_start_offset,
                    end_offset: payload.unit_end_offset,
                    state_bits: payload.state_bits,
                    speed,
                },
            );
        }
    });

    // 4. Input Handler
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(10);
    std::thread::spawn(move || {
        let mut buffer = String::new();
        loop {
            buffer.clear();
            if std::io::stdin().read_line(&mut buffer).is_ok() {
                let _ = tx.blocking_send(buffer.trim().to_string());
            }
        }
    });

    println!("Starting download (VLC Player)...");
    let url = Url::parse(
        "https://mirror.bom2.albony.in/videolan-ftp/vlc/3.0.23/win32/vlc-3.0.23-win32.exe",
    )
    .unwrap();
    let item = NewDownloadItem {
        url: url.clone(),
        filename: Some("vlc-3.0.23-win32.exe".to_string()),
        queue_id: None,
    };

    manager
        .handle_request(&app, DownloadRequest::New(vec![item]))
        .await
        .expect("Failed to start");

    // 5. MultiProgress Setup
    let mp = indicatif::MultiProgress::new();
    let pb_main = mp.add(indicatif::ProgressBar::new(100));
    pb_main.set_style(indicatif::ProgressStyle::default_bar()
        .template("{spinner:.green} [MAIN] [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
        .unwrap()
        .progress_chars("#>-"));

    // Map to hold worker progress bars
    let mut worker_bars: HashMap<u8, indicatif::ProgressBar> = HashMap::new();

    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut initialized_size = false;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Update Main Bar
                if let Ok(downloads) = db.get_downloads() {
                     if let Some(d) = downloads.first() {
                         let status = d.status.as_deref().unwrap_or("run");
                         let real_received = manager.get_bytes_downloaded(&d.id).map(|b| b as u64).unwrap_or(d.bytes_received as u64);
                         let size = d.size.unwrap_or(0) as u64;

                         if !initialized_size && size > 0 {
                             pb_main.set_length(size);
                             initialized_size = true;
                         }

                         pb_main.set_position(real_received);
                         pb_main.set_message(format!("[{}]", status.to_uppercase()));

                         if status == "completed" {
                             pb_main.finish_with_message("✅ Download Completed!");
                             // Clean up worker bars
                             for bar in worker_bars.values() {
                                 bar.finish_and_clear();
                             }
                             app.exit(0);
                             break;
                         }
                         if status == "failed" {
                             pb_main.abandon_with_message("❌ Download Failed");
                              for bar in worker_bars.values() {
                                 bar.finish_and_clear();
                             }
                             break;
                         }
                     }
                }

                // Update Worker Bars
                let infos = {
                    let guard = worker_infos.lock().unwrap();
                    guard.iter().map(|(k, v)| (*k, v.current_unit, v.start_offset, v.end_offset, v.speed, v.state_bits)).collect::<Vec<_>>()
                };

                for (wid, unit, start, end, speed_mbs, bits) in infos {
                    let bar = worker_bars.entry(wid).or_insert_with(|| {
                        let pb = mp.add(indicatif::ProgressBar::new(100));
                        // Text-only template: "WorkerID | Details"
                        pb.set_style(indicatif::ProgressStyle::default_bar().template("   [W{len}] {msg}").unwrap());
                        pb.set_length(wid as u64); // Storing ID in length field for template access if needed, or just use set_message
                        pb
                    });

                    // Format: "128-136MB | 5.2 MB/s | [00010111]"
                    let start_mb = start >> 20;
                    let end_mb = end >> 20;

                    // Bits visualization
                    let mut bits_str = String::with_capacity(8);
                    for i in 0..8 {
                        if (bits >> i) & 1 == 1 {
                             bits_str.push('█');
                        } else {
                             bits_str.push('.');
                        }
                    }

                    bar.set_message(format!("{}-{}MB | {:.1} MB/s | [{}] Unit: {}", start_mb, end_mb, speed_mbs, bits_str, unit));
                    bar.tick();
                }
            }
            Some(cmd) = rx.recv() => {
                 let active = if let Ok(list) = db.get_downloads() { list.first().map(|d| d.id) } else { None };
                 if let Some(id) = active {
                     match cmd.as_str() {
                         "p" => { manager.pause_instance(&id, &app, &db); },
                         "r" => { manager.handle_request(&app, DownloadRequest::Resume(vec![id])).await.unwrap(); },
                         "c" => { manager.cancel_instance(&id, &app, &db); },
                         "q" => {
                            manager.shutdown_all_graceful(&app, &db);
                            app.exit(0);
                            break;
                         },
                         _ => {},
                     }
                 }
            }
        }
    }
}

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{Listener, Manager};

use tur_lib::database::Database;
use tur_lib::downloads::manager::NewDownloadItem;
use tur_lib::downloads::{spawn_manager, DownloadRequest};
use url::Url;

#[derive(Clone, Debug, serde::Deserialize)]
struct WorkerEvent {
    #[allow(dead_code)]
    download_id: String,
    worker_id: u8,
    state_bits: u8,
    current_unit: usize,
    unit_start_offset: u64,
    unit_end_offset: u64,
    index_start: usize,
    index_end: usize,
    stealing_from: Option<usize>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct ErrorEvent {
    #[allow(dead_code)]
    download_id: String,
    worker_id: u8,
    code: u16,
    message: String,
    fatal: bool,
}

struct WorkerInfo {
    last_update: Instant,
    current_unit: usize,
    start_offset: u64,
    end_offset: u64,
    state_bits: u8,
    speed: f64, // MB/s
    index_start: usize,
    index_end: usize,
    stealing_from: Option<usize>,
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
    let manager = spawn_manager(app.clone());
    app.manage(manager.clone());

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
                    index_start: payload.index_start,
                    index_end: payload.index_end,
                    stealing_from: payload.stealing_from,
                },
            );
        }
    });

    let _ = app.listen("download_error", move |event| {
        if let Ok(payload) = serde_json::from_str::<ErrorEvent>(event.payload()) {
            println!(
                "\n⚠️  Worker {} Error (Fatal: {}): [{}] {}\n",
                payload.worker_id, payload.fatal, payload.code, payload.message
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

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    TUR Download Test Harness                  ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Controls:                                                    ║");
    println!("║    [p] Pause    [r] Resume    [c] Cancel    [q] Quit          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Starting download (Debian 13.3.0 Netinst)...");
    // "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.3.0-amd64-netinst.iso",
    // "https://mirror.bom2.albony.in/videolan-ftp/vlc/3.0.23/win32/vlc-3.0.23-win32.exe",
    let url = Url::parse(
        "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-13.3.0-amd64-netinst.iso",
    )
    .unwrap();
    let item = NewDownloadItem {
        url: url.clone(),
        filename: Some("debian-13.3.0-netinst.iso".to_string()),
        queue_id: None,
    };

    manager
        .start(DownloadRequest::New(vec![item]))
        .await
        .expect("Failed to start");

    // Get the download ID we just started (most recent in-progress download)
    let download_id = db
        .get_downloads_by_status(None)
        .ok()
        .and_then(|list| list.first().map(|d| d.id))
        .expect("Failed to get download ID");
    println!("Download ID: {}", download_id);

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
                // Update Main Bar - query by specific download ID
                if let Ok(Some(d)) = db.get_download_by_id(&download_id) {
                         let status = d.status.as_deref().unwrap_or("run");
                         let real_received = d.bytes_received as u64;
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

                // Update Worker Bars
                let infos = {
                    let guard = worker_infos.lock().unwrap();
                    guard.iter().map(|(k, v)| (*k, v.current_unit, v.start_offset, v.end_offset, v.speed, v.state_bits, v.index_start, v.index_end, v.stealing_from)).collect::<Vec<_>>()
                };

                fn format_speed(speed_mbs: f64) -> String {
                     let bytes_per_sec = speed_mbs * 1_000_000.0;
                     if bytes_per_sec >= 1_000_000_000.0 {
                         format!("{:.2} GB/s", bytes_per_sec / 1_000_000_000.0)
                     } else if bytes_per_sec >= 1_000_000.0 {
                         format!("{:.2} MB/s", bytes_per_sec / 1_000_000.0)
                     } else if bytes_per_sec >= 1_000.0 {
                         format!("{:.2} KB/s", bytes_per_sec / 1_000.0)
                     } else {
                         format!("{:.0} B/s", bytes_per_sec)
                     }
                }

                for (wid, unit, start, end, speed_mbs, bits, idx_start, idx_end, stealing_from) in infos {
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

                    let steal_tag = if let Some(victim) = stealing_from {
                         format!(" | Stealing from W{}", victim)
                    } else {
                         String::new()
                    };
                    bar.set_message(format!("{}..{} | {}-{}MB | Unit: {} | {} | [{}] {}", idx_start, idx_end, start_mb, end_mb, unit, format_speed(speed_mbs), bits_str, steal_tag));
                    bar.tick();
                }
            }
            Some(cmd) = rx.recv() => {
                 // Use the tracked download_id, but check if it's still active for pause/cancel
                 let id = download_id;
                     match cmd.as_str() {
                         "p" => {
                            if manager.pause(id).await {
                                pb_main.println("⏸️  Download paused. Press [r] to resume.");
                            }
                         },
                         "r" => {
                            pb_main.println("▶️  Resuming download...");
                            if let Err(e) = manager.start(DownloadRequest::Resume(vec![id])).await {
                                pb_main.println(format!("❌ Resume failed: {}", e));
                            }
                         },
                         "c" => {
                            if manager.cancel(id).await {
                                pb_main.println("🛑 Download cancelled. State saved for later resume.");
                            }
                         },
                         "q" => {
                            pb_main.println("👋 Shutting down gracefully...");
                            manager.shutdown().await;
                            app.exit(0);
                            break;
                         },
                         "h" | "?" => {
                            pb_main.println("Controls: [p]ause | [r]esume | [c]ancel | [q]uit | [h]elp");
                         },
                         _ => {
                            pb_main.println("Unknown command. Press [h] for help.");
                         },
                     }
            }
        }
    }
}

//! CLI download runner - standalone download execution for terminal mode

use console::{style, Term};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::args::AppArgs;

/// Download result
pub struct DownloadResult {
    pub url: String,
    pub filename: String,
    pub size: u64,
    pub success: bool,
    pub error: Option<String>,
}

/// Progress bar style for downloads
fn download_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.green} {msg}\n  [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
    )
    .unwrap()
    .progress_chars("█▓▒░")
}

/// Spinner style for unknown size
fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.green} {msg} {bytes} ({bytes_per_sec})").unwrap()
}

/// Print controls hint
fn print_controls() {
    println!("{}", style("Controls: [C]ancel  [G]UI  [Q]uit").dim());
    println!();
}

/// Clear screen and print header
pub fn clear_and_header() {
    let _ = Term::stdout().clear_screen();
    println!();
    println!(
        "🚀 {} v{}",
        style("tur").blue().bold(),
        env!("CARGO_PKG_VERSION")
    );
    println!();
    print_controls();
}

/// Run downloads in CLI mode with keyboard controls
pub async fn run_downloads(args: &AppArgs, urls: Vec<String>) -> Vec<DownloadResult> {
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .connect_timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0")
        .build()
        .expect("Failed to create HTTP client");

    let mp = MultiProgress::new();

    // Cancel flag shared across all downloads
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let open_gui_flag = Arc::new(AtomicBool::new(false));

    // Determine output directory
    let output_dir = args
        .output
        .clone()
        .unwrap_or_else(|| dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")));

    // Speed limit per download
    let speed_limit = args.parse_speed_limit().unwrap_or(0);
    let per_download_limit = if speed_limit > 0 && !urls.is_empty() {
        speed_limit / urls.len() as u64
    } else {
        0
    };

    // Spawn keyboard listener
    let cancel_clone = cancel_flag.clone();
    let gui_clone = open_gui_flag.clone();
    let kb_handle = std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(KeyEvent {
                    code, modifiers, ..
                })) = event::read()
                {
                    match code {
                        // Ctrl+C or 'c' or 'C' to cancel
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            if modifiers.contains(KeyModifiers::CONTROL) || modifiers.is_empty() {
                                cancel_clone.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                        // 'q' or 'Q' to quit
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            cancel_clone.store(true, Ordering::SeqCst);
                            break;
                        }
                        // 'g' or 'G' to open GUI
                        KeyCode::Char('g') | KeyCode::Char('G') => {
                            gui_clone.store(true, Ordering::SeqCst);
                            cancel_clone.store(true, Ordering::SeqCst);
                            break;
                        }
                        KeyCode::Esc => {
                            cancel_clone.store(true, Ordering::SeqCst);
                            break;
                        }
                        _ => {}
                    }
                }
            }

            // Check if downloads are done
            if cancel_clone.load(Ordering::SeqCst) {
                break;
            }
        }
    });

    let mut handles = Vec::new();
    let mut results = Vec::new();

    for url in urls {
        let client = client.clone();
        let mp = mp.clone();
        let output_dir = output_dir.clone();
        let quiet = args.quiet;
        let cancel = cancel_flag.clone();

        let handle = tokio::spawn(async move {
            download_file(
                &client,
                &url,
                &output_dir,
                &mp,
                quiet,
                per_download_limit,
                cancel,
            )
            .await
        });
        handles.push(handle);
    }

    // Wait for all downloads
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => results.push(DownloadResult {
                url: String::new(),
                filename: String::new(),
                size: 0,
                success: false,
                error: Some(format!("Task failed: {}", e)),
            }),
        }
    }

    // Stop keyboard listener
    cancel_flag.store(true, Ordering::SeqCst);
    let _ = kb_handle.join();

    // Check if user wants to open GUI
    if open_gui_flag.load(Ordering::SeqCst) {
        println!();
        println!("🖥️  Opening GUI...");
        // This will be handled by main.rs
    }

    results
}

/// Download a single file with progress bar
async fn download_file(
    client: &Client,
    url: &str,
    output_dir: &Path,
    mp: &MultiProgress,
    quiet: bool,
    speed_limit: u64,
    cancel_flag: Arc<AtomicBool>,
) -> DownloadResult {
    // Extract filename from URL
    let filename = url
        .split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .unwrap_or("download")
        .to_string();

    let filepath = output_dir.join(&filename);

    // Start request
    let response = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            return DownloadResult {
                url: url.to_string(),
                filename,
                size: 0,
                success: false,
                error: Some(format!("Request failed: {}", e)),
            };
        }
    };

    if !response.status().is_success() {
        return DownloadResult {
            url: url.to_string(),
            filename,
            size: 0,
            success: false,
            error: Some(format!("HTTP {}", response.status())),
        };
    }

    let total_size = response.content_length();

    // Create progress bar
    let pb = if !quiet {
        let pb = match total_size {
            Some(size) => {
                let pb = mp.add(ProgressBar::new(size));
                pb.set_style(download_style());
                pb
            }
            None => {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(spinner_style());
                pb
            }
        };
        pb.set_message(format!("📥 {}", filename));
        pb.enable_steady_tick(Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    // Create output file
    let mut file = match File::create(&filepath) {
        Ok(f) => f,
        Err(e) => {
            if let Some(pb) = pb {
                pb.abandon_with_message(format!("❌ {} - Failed to create file", filename));
            }
            return DownloadResult {
                url: url.to_string(),
                filename,
                size: 0,
                success: false,
                error: Some(format!("Failed to create file: {}", e)),
            };
        }
    };

    // Stream response
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_throttle = std::time::Instant::now();
    let mut bytes_this_second: u64 = 0;

    while let Some(chunk) = stream.next().await {
        // Check for cancel
        if cancel_flag.load(Ordering::SeqCst) {
            if let Some(pb) = pb {
                pb.abandon_with_message(format!("⏹️  {} - Cancelled", filename));
            }
            return DownloadResult {
                url: url.to_string(),
                filename,
                size: downloaded,
                success: false,
                error: Some("Cancelled by user".to_string()),
            };
        }

        match chunk {
            Ok(bytes) => {
                if file.write_all(&bytes).is_err() {
                    if let Some(pb) = pb {
                        pb.abandon_with_message(format!("❌ {} - Write failed", filename));
                    }
                    return DownloadResult {
                        url: url.to_string(),
                        filename,
                        size: downloaded,
                        success: false,
                        error: Some("Write failed".to_string()),
                    };
                }

                downloaded += bytes.len() as u64;
                bytes_this_second += bytes.len() as u64;

                if let Some(ref pb) = pb {
                    pb.set_position(downloaded);
                }

                // Speed limiting
                if speed_limit > 0 {
                    if bytes_this_second >= speed_limit {
                        let elapsed = last_throttle.elapsed();
                        if elapsed < Duration::from_secs(1) {
                            tokio::time::sleep(Duration::from_secs(1) - elapsed).await;
                        }
                        bytes_this_second = 0;
                        last_throttle = std::time::Instant::now();
                    }
                }
            }
            Err(e) => {
                if let Some(pb) = pb {
                    pb.abandon_with_message(format!("❌ {} - {}", filename, e));
                }
                return DownloadResult {
                    url: url.to_string(),
                    filename,
                    size: downloaded,
                    success: false,
                    error: Some(e.to_string()),
                };
            }
        }
    }

    if let Some(pb) = pb {
        pb.finish_with_message(format!("✅ {} ({})", filename, format_size(downloaded)));
    }

    DownloadResult {
        url: url.to_string(),
        filename,
        size: downloaded,
        success: true,
        error: None,
    }
}

/// Format bytes to human-readable size
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

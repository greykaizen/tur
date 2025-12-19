//! TUI progress display using indicatif
//!
//! Provides terminal progress bars for downloads

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;

/// Style for download progress bars
fn download_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
    )
    .unwrap()
    .progress_chars("█▓▒░")
}

/// Style for indeterminate progress (unknown size)
fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] {msg} {bytes} ({bytes_per_sec})",
    )
    .unwrap()
}

/// Create a new multi-progress container for multiple downloads
pub fn create_multi_progress() -> MultiProgress {
    MultiProgress::new()
}

/// Create a progress bar for a single download
pub fn create_download_bar(
    mp: &MultiProgress,
    filename: &str,
    total_size: Option<u64>,
) -> ProgressBar {
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
    pb.set_message(filename.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// Status emojis for download states
pub mod status {
    pub const DOWNLOADING: &str = "📥";
    pub const COMPLETE: &str = "✅";
    pub const FAILED: &str = "❌";
    pub const PAUSED: &str = "⏸️";
    pub const QUEUED: &str = "⏳";
    pub const CONNECTING: &str = "🔗";
}

/// Print a status line with emoji
pub fn print_status(emoji: &str, message: &str) {
    println!("{} {}", emoji, message);
}

/// Print download start
pub fn print_download_start(filename: &str, size: Option<u64>) {
    let size_str = size
        .map(|s| format_size(s))
        .unwrap_or_else(|| "unknown size".to_string());
    print_status(
        status::DOWNLOADING,
        &format!("Starting: {} ({})", filename, size_str),
    );
}

/// Print download complete
pub fn print_download_complete(filename: &str, duration_secs: u64) {
    print_status(
        status::COMPLETE,
        &format!("Complete: {} in {}s", filename, duration_secs),
    );
}

/// Print download error
pub fn print_download_error(filename: &str, error: &str) {
    print_status(status::FAILED, &format!("Failed: {} - {}", filename, error));
}

/// Format byte size to human readable
pub fn format_size(bytes: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }
}

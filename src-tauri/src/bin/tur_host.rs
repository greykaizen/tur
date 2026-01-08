//! tur Native Messaging Host
//!
//! Standalone binary for Chrome/Firefox Native Messaging.
//! Reads messages from stdin, processes them, writes responses to stdout.
//!
//! Protocol: 4-byte little-endian length prefix + JSON payload
//!
//! @author greykaizen

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::process::Command;

// ============================================================================
// Message Types
// ============================================================================

/// Incoming message from browser extension
#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
pub enum IncomingMessage {
    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "get_formats")]
    GetFormats { url: String },

    #[serde(rename = "download")]
    Download {
        url: String,
        format_id: Option<String>,
        title: Option<String>,
        filesize: Option<u64>,
        ext: Option<String>,
        video_stream_url: Option<String>,
        audio_stream_url: Option<String>,
    },
}

/// Outgoing message to browser extension
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum OutgoingMessage {
    #[serde(rename = "pong")]
    Pong { version: String },

    #[serde(rename = "formats")]
    Formats {
        url: String,
        videos: Vec<VideoFormat>,
        audios: Vec<AudioFormat>,
        subtitles: Vec<SubtitleTrack>,
    },

    #[serde(rename = "download_started")]
    DownloadStarted { id: String },

    #[serde(rename = "error")]
    Error { message: String },
}

/// Video format info
#[derive(Debug, Serialize, Clone)]
pub struct VideoFormat {
    pub format_id: String,
    pub ext: String,
    pub quality: String,
    pub filesize: Option<u64>,
    pub has_audio: bool,
    pub url: Option<String>,
}

/// Audio-only format
#[derive(Debug, Serialize, Clone)]
pub struct AudioFormat {
    pub format_id: String,
    pub ext: String,
    pub quality: String, // bitrate or description
    pub filesize: Option<u64>,
    pub language: Option<String>,
    pub url: Option<String>,
}

/// Subtitle track
#[derive(Debug, Serialize, Clone)]
pub struct SubtitleTrack {
    pub lang: String,
    pub name: String,
    pub ext: String,
}

// ============================================================================
// yt-dlp Integration
// ============================================================================

/// yt-dlp JSON output format
#[derive(Debug, Deserialize)]
struct YtDlpOutput {
    formats: Option<Vec<YtDlpFormat>>,
    subtitles: Option<std::collections::HashMap<String, Vec<YtDlpSubtitle>>>,
}

#[derive(Debug, Deserialize)]
struct YtDlpFormat {
    format_id: String,
    ext: String,
    #[serde(default)]
    format_note: Option<String>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    filesize: Option<u64>,
    #[serde(default)]
    filesize_approx: Option<u64>,
    #[serde(default)]
    vcodec: Option<String>,
    #[serde(default)]
    acodec: Option<String>,
    #[serde(default)]
    abr: Option<f64>, // audio bitrate
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YtDlpSubtitle {
    ext: String,
    #[serde(default)]
    name: Option<String>,
}

/// Run yt-dlp and get available formats
fn get_formats_ytdlp(url: &str) -> OutgoingMessage {
    eprintln!("[tur-host] Running yt-dlp for: {}", url);

    let ytdlp_path = find_ytdlp();
    if ytdlp_path.is_none() {
        return OutgoingMessage::Error {
            message: "yt-dlp not found. Please install yt-dlp.".to_string(),
        };
    }

    let ytdlp = ytdlp_path.unwrap();
    eprintln!("[tur-host] Using yt-dlp: {}", ytdlp);

    // Run yt-dlp with speed optimizations
    let output = Command::new(&ytdlp)
        .args([
            "--dump-json",
            "--no-warnings",
            "--no-playlist",
            "--socket-timeout",
            "10",
            url,
        ])
        .output();

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("[tur-host] yt-dlp error: {}", stderr);
                return OutgoingMessage::Error {
                    message: format!(
                        "yt-dlp failed: {}",
                        stderr.lines().next().unwrap_or("Unknown error")
                    ),
                };
            }

            match serde_json::from_slice::<YtDlpOutput>(&output.stdout) {
                Ok(data) => {
                    let (videos, audios) = parse_ytdlp_formats(&data);
                    let subtitles = parse_ytdlp_subtitles(&data);

                    eprintln!(
                        "[tur-host] Found {} videos, {} audios, {} subtitles",
                        videos.len(),
                        audios.len(),
                        subtitles.len()
                    );

                    OutgoingMessage::Formats {
                        url: url.to_string(),
                        videos,
                        audios,
                        subtitles,
                    }
                }
                Err(e) => {
                    eprintln!("[tur-host] Failed to parse yt-dlp output: {}", e);
                    OutgoingMessage::Error {
                        message: "Failed to parse video info".to_string(),
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[tur-host] Failed to run yt-dlp: {}", e);
            OutgoingMessage::Error {
                message: format!("Failed to run yt-dlp: {}", e),
            }
        }
    }
}

/// Find yt-dlp binary location, auto-download if not found
fn find_ytdlp() -> Option<String> {
    // First check app data directory
    let app_data_path = get_app_data_ytdlp_path();
    if let Some(ref path) = app_data_path {
        if std::path::Path::new(path).exists() {
            eprintln!("[tur-host] Using bundled yt-dlp: {}", path);
            return Some(path.clone());
        }
    }

    // Check system paths
    let system_paths = ["yt-dlp", "/usr/bin/yt-dlp", "/usr/local/bin/yt-dlp"];

    for path in system_paths {
        if Command::new(path)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            eprintln!("[tur-host] Using system yt-dlp: {}", path);
            return Some(path.to_string());
        }
    }

    // Not found - try to download
    eprintln!("[tur-host] yt-dlp not found, attempting download...");
    if let Some(path) = app_data_path {
        if download_ytdlp(&path) {
            return Some(path);
        }
    }

    None
}

/// Find the main tur app executable
fn find_tur_app() -> Option<String> {
    // Check development/debug build first (for testing)
    let dev_paths = [
        "/home/kaizen/Repo/Rust🦀/tur/src-tauri/target/debug/tur",
        "./target/debug/tur",
    ];

    for path in dev_paths {
        if std::path::Path::new(path).exists() {
            eprintln!("[tur-host] Using dev build: {}", path);
            return Some(path.to_string());
        }
    }

    // Check common installed locations
    let paths = ["tur", "/usr/bin/tur", "/usr/local/bin/tur"];

    for path in paths {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // Check app data directory (for development/portable installs)
    if let Some(data_dir) = dirs::data_dir() {
        let app_path = data_dir.join("tur").join("tur");
        if app_path.exists() {
            return Some(app_path.to_string_lossy().to_string());
        }
    }

    // Try to find via which command on Unix
    #[cfg(unix)]
    {
        if let Ok(output) = Command::new("which").arg("tur").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }

    None
}

/// Get the path where yt-dlp should be stored in app data
fn get_app_data_ytdlp_path() -> Option<String> {
    let data_dir = dirs::data_dir()?;
    let tur_dir = data_dir.join("tur").join("bin");

    // Create directory if needed
    std::fs::create_dir_all(&tur_dir).ok()?;

    #[cfg(target_os = "windows")]
    let binary_name = "yt-dlp.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "yt-dlp";

    Some(tur_dir.join(binary_name).to_string_lossy().to_string())
}

/// Download yt-dlp from GitHub releases
fn download_ytdlp(dest_path: &str) -> bool {
    // Determine download URL based on OS
    let download_url = get_ytdlp_download_url();

    if download_url.is_none() {
        eprintln!("[tur-host] Unsupported OS for yt-dlp auto-download");
        return false;
    }

    let url = download_url.unwrap();
    eprintln!("[tur-host] Downloading yt-dlp from: {}", url);

    // Use curl/wget to download (simpler than adding HTTP client dependency)
    let result = Command::new("curl")
        .args(["-L", "-o", dest_path, &url])
        .output();

    if let Ok(output) = result {
        if output.status.success() {
            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mut perms) =
                    std::fs::metadata(dest_path).and_then(|m| Ok(m.permissions()))
                {
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(dest_path, perms);
                }
            }

            eprintln!(
                "[tur-host] Successfully downloaded yt-dlp to: {}",
                dest_path
            );
            return true;
        }
    }

    // Try wget as fallback
    let result = Command::new("wget").args(["-O", dest_path, &url]).output();

    if let Ok(output) = result {
        if output.status.success() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mut perms) =
                    std::fs::metadata(dest_path).and_then(|m| Ok(m.permissions()))
                {
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(dest_path, perms);
                }
            }

            eprintln!(
                "[tur-host] Successfully downloaded yt-dlp to: {}",
                dest_path
            );
            return true;
        }
    }

    eprintln!("[tur-host] Failed to download yt-dlp");
    false
}

/// Get the yt-dlp download URL for current OS
fn get_ytdlp_download_url() -> Option<String> {
    let base = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Some(format!("{}/yt-dlp_linux", base));

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Some(format!("{}/yt-dlp_linux_aarch64", base));

    #[cfg(target_os = "macos")]
    return Some(format!("{}/yt-dlp_macos", base));

    #[cfg(target_os = "windows")]
    return Some(format!("{}/yt-dlp.exe", base));

    #[allow(unreachable_code)]
    None
}

/// Parse yt-dlp formats into separate video and audio lists
fn parse_ytdlp_formats(data: &YtDlpOutput) -> (Vec<VideoFormat>, Vec<AudioFormat>) {
    let Some(formats) = &data.formats else {
        return (vec![], vec![]);
    };

    let mut videos: Vec<VideoFormat> = Vec::new();
    let mut audios: Vec<AudioFormat> = Vec::new();

    for f in formats {
        let has_video =
            f.vcodec.as_ref().map(|v| v != "none").unwrap_or(false) || f.height.is_some();
        let has_audio = f.acodec.as_ref().map(|a| a != "none").unwrap_or(false);

        if has_video {
            // Video format (may also have audio)
            let quality = if let Some(h) = f.height {
                format!("{}p", h)
            } else if let Some(note) = &f.format_note {
                note.clone()
            } else {
                "unknown".to_string()
            };

            videos.push(VideoFormat {
                format_id: f.format_id.clone(),
                ext: f.ext.clone(),
                quality,
                filesize: f.filesize.or(f.filesize_approx),
                has_audio,
                url: f.url.clone(),
            });
        } else if has_audio {
            // Audio-only format
            let quality = if let Some(abr) = f.abr {
                format!("{}kbps", abr as u32)
            } else if let Some(note) = &f.format_note {
                note.clone()
            } else {
                "audio".to_string()
            };

            audios.push(AudioFormat {
                format_id: f.format_id.clone(),
                ext: f.ext.clone(),
                quality,
                filesize: f.filesize.or(f.filesize_approx),
                language: f.language.clone(),
                url: f.url.clone(),
            });
        }
    }

    // Sort videos by resolution (highest first)
    videos.sort_by(|a, b| {
        let a_height: u32 = a.quality.trim_end_matches('p').parse().unwrap_or(0);
        let b_height: u32 = b.quality.trim_end_matches('p').parse().unwrap_or(0);
        b_height.cmp(&a_height)
    });
    // Show all formats - no truncation

    // Sort audios by bitrate (highest first)
    audios.sort_by(|a, b| {
        let a_br: u32 = a.quality.trim_end_matches("kbps").parse().unwrap_or(0);
        let b_br: u32 = b.quality.trim_end_matches("kbps").parse().unwrap_or(0);
        b_br.cmp(&a_br)
    });
    // Show all formats - no truncation

    (videos, audios)
}

/// Parse subtitles from yt-dlp output
fn parse_ytdlp_subtitles(data: &YtDlpOutput) -> Vec<SubtitleTrack> {
    let Some(subs) = &data.subtitles else {
        return vec![];
    };

    let mut result: Vec<SubtitleTrack> = subs
        .iter()
        .filter_map(|(lang, tracks)| {
            let track = tracks.first()?;
            Some(SubtitleTrack {
                lang: lang.clone(),
                name: track.name.clone().unwrap_or_else(|| lang.clone()),
                ext: track.ext.clone(),
            })
        })
        .collect();

    // Sort alphabetically by language
    result.sort_by(|a, b| a.lang.cmp(&b.lang));
    result.truncate(20);
    result
}

// ============================================================================
// Native Messaging Protocol
// ============================================================================

fn read_message() -> io::Result<IncomingMessage> {
    let mut stdin = io::stdin().lock();

    let mut len_bytes = [0u8; 4];
    stdin.read_exact(&mut len_bytes)?;
    let len = u32::from_ne_bytes(len_bytes) as usize;

    if len > 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Message too large",
        ));
    }

    let mut buffer = vec![0u8; len];
    stdin.read_exact(&mut buffer)?;

    serde_json::from_slice(&buffer).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_message(msg: &OutgoingMessage) -> io::Result<()> {
    let json =
        serde_json::to_vec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let len = json.len() as u32;

    let mut stdout = io::stdout().lock();
    stdout.write_all(&len.to_ne_bytes())?;
    stdout.write_all(&json)?;
    stdout.flush()
}

// ============================================================================
// Message Handling
// ============================================================================

fn handle_message(msg: IncomingMessage) -> OutgoingMessage {
    eprintln!("[tur-host] Received: {:?}", msg);

    match msg {
        IncomingMessage::Ping => OutgoingMessage::Pong {
            version: env!("CARGO_PKG_VERSION").to_string(),
        },

        IncomingMessage::GetFormats { url } => {
            // Use real yt-dlp
            get_formats_ytdlp(&url)
        }

        IncomingMessage::Download {
            url,
            format_id,
            title,
            filesize,
            ext,
            video_stream_url,
            audio_stream_url,
        } => {
            eprintln!(
                "[tur-host] Download request: {} format: {:?} title: {:?} size: {:?} ext: {:?} stream: {:?}",
                url, format_id, title, filesize, ext, video_stream_url
            );

            // Launch main Tauri app with download args
            let tur_path = find_tur_app();
            if let Some(app_path) = tur_path {
                let mut cmd = std::process::Command::new(&app_path);
                cmd.arg("--download-url").arg(&url);
                if let Some(fmt) = &format_id {
                    cmd.arg("--format").arg(fmt);
                }
                if let Some(t) = &title {
                    cmd.arg("--title").arg(t);
                }
                if let Some(s) = filesize {
                    cmd.arg("--filesize").arg(s.to_string());
                }
                if let Some(e) = &ext {
                    cmd.arg("--ext").arg(e);
                }
                // Pass actual stream URLs for direct download
                if let Some(vs) = &video_stream_url {
                    cmd.arg("--video-stream-url").arg(vs);
                }
                if let Some(as_) = &audio_stream_url {
                    cmd.arg("--audio-stream-url").arg(as_);
                }

                match cmd.spawn() {
                    Ok(_) => {
                        eprintln!("[tur-host] Launched tur app for download");
                    }
                    Err(e) => {
                        eprintln!("[tur-host] Failed to launch tur: {}", e);
                    }
                }
            } else {
                eprintln!("[tur-host] Could not find tur app executable");
            }

            OutgoingMessage::DownloadStarted {
                id: format!(
                    "dl_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                ),
            }
        }
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    eprintln!("[tur-host] Native messaging host started");

    loop {
        match read_message() {
            Ok(msg) => {
                let response = handle_message(msg);
                if let Err(e) = write_message(&response) {
                    eprintln!("[tur-host] Write error: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("[tur-host] Read error: {}", e);
                break;
            }
        }
    }

    eprintln!("[tur-host] Exiting");
}

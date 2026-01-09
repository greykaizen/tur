//! Native Messaging Host for Chrome/Firefox extension
//!
//! Runs in a separate thread when --native-messaging flag is passed.
//! Handles stdin/stdout for native messaging protocol.
//!
//! @author greykaizen

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::process::Command;
use std::sync::mpsc;
use std::thread;

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

#[derive(Debug, Serialize, Clone)]
pub struct VideoFormat {
    pub format_id: String,
    pub ext: String,
    pub quality: String,
    pub filesize: Option<u64>,
    pub has_audio: bool,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AudioFormat {
    pub format_id: String,
    pub ext: String,
    pub quality: String,
    pub filesize: Option<u64>,
    pub language: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SubtitleTrack {
    pub lang: String,
    pub name: String,
    pub ext: String,
}

// ============================================================================
// yt-dlp Types
// ============================================================================

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
    abr: Option<f64>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    url: Option<String>,
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
// yt-dlp Integration (FAST command)
// ============================================================================

// ============================================================================
// Message Handler
// ============================================================================

pub enum HostAction {
    OpenDownload {
        url: String,
        format_id: Option<String>,
        title: Option<String>,
        filesize: Option<u64>,
        ext: Option<String>,
        video_stream_url: Option<String>,
        audio_stream_url: Option<String>,
    },
}

fn handle_message(
    msg: IncomingMessage,
    ytdlp_path: &Option<String>,
) -> (OutgoingMessage, Option<HostAction>) {
    match msg {
        IncomingMessage::Ping => (
            OutgoingMessage::Pong {
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            None,
        ),
        IncomingMessage::GetFormats { url } => (get_formats_ytdlp(&url, ytdlp_path), None),
        IncomingMessage::Download {
            url,
            format_id,
            title,
            filesize,
            ext,
            video_stream_url,
            audio_stream_url,
        } => {
            let id = format!(
                "dl_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            );

            (
                OutgoingMessage::DownloadStarted { id },
                Some(HostAction::OpenDownload {
                    url,
                    format_id,
                    title,
                    filesize,
                    ext,
                    video_stream_url,
                    audio_stream_url,
                }),
            )
        }
    }
}

// ============================================================================
// Main Loop (runs in separate thread)
// ============================================================================

/// Start native messaging listener in a separate thread
/// Returns a receiver for download actions
pub fn start_native_messaging_thread() -> mpsc::Receiver<HostAction> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        eprintln!("[tur] Native messaging thread started");

        // Optimization: Cache yt-dlp path once
        let ytdlp_path = find_ytdlp();
        if ytdlp_path.is_none() {
            eprintln!("[tur] Warning: yt-dlp not found in PATH or app dir");
        } else {
            eprintln!("[tur] Using yt-dlp at: {}", ytdlp_path.as_ref().unwrap());
        }

        loop {
            match read_message() {
                Ok(msg) => {
                    let (response, action) = handle_message(msg, &ytdlp_path);

                    if let Err(e) = write_message(&response) {
                        eprintln!("[tur] Failed to write response: {}", e);
                        break;
                    }

                    if let Some(action) = action {
                        let _ = tx.send(action);
                    }
                }
                Err(e) => {
                    // EOF or error - extension disconnected
                    eprintln!("[tur] Native messaging ended: {}", e);
                    break;
                }
            }
        }
    });

    rx
}

// ============================================================================
// yt-dlp Integration (FAST command)
// ============================================================================

fn get_formats_ytdlp(url: &str, ytdlp_path: &Option<String>) -> OutgoingMessage {
    eprintln!("[tur] Running yt-dlp for: {}", url);

    if ytdlp_path.is_none() {
        return OutgoingMessage::Error {
            message: "yt-dlp not found. Please install yt-dlp.".to_string(),
        };
    }

    let ytdlp = ytdlp_path.as_ref().unwrap();

    // FAST command: -O outputs only formats, extractor-args skips parsing
    let output = Command::new(ytdlp)
        .args([
            "-O",
            "%(formats)#j",
            "--no-warnings",
            "--extractor-args",
            "youtube:skip=dash,hls,translated_subs",
            url,
        ])
        .output();

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return OutgoingMessage::Error {
                    message: format!(
                        "yt-dlp failed: {}",
                        stderr.lines().next().unwrap_or("Unknown error")
                    ),
                };
            }

            // Parse formats array directly
            match serde_json::from_slice::<Vec<YtDlpFormat>>(&output.stdout) {
                Ok(formats) => {
                    let (videos, audios) = parse_formats(&formats);
                    eprintln!(
                        "[tur] Found {} videos, {} audios",
                        videos.len(),
                        audios.len()
                    );

                    OutgoingMessage::Formats {
                        url: url.to_string(),
                        videos,
                        audios,
                        subtitles: vec![], // Not fetched with -O command
                    }
                }
                Err(e) => {
                    eprintln!("[tur] Failed to parse yt-dlp output: {}", e);
                    OutgoingMessage::Error {
                        message: "Failed to parse video info".to_string(),
                    }
                }
            }
        }
        Err(e) => OutgoingMessage::Error {
            message: format!("Failed to run yt-dlp: {}", e),
        },
    }
}

fn parse_formats(formats: &[YtDlpFormat]) -> (Vec<VideoFormat>, Vec<AudioFormat>) {
    let mut videos = Vec::new();
    let mut audios = Vec::new();

    for f in formats {
        let has_video = f.vcodec.as_ref().map(|v| v != "none").unwrap_or(false);
        let has_audio = f.acodec.as_ref().map(|a| a != "none").unwrap_or(false);

        if has_video {
            let quality = f
                .height
                .map(|h| format!("{}p", h))
                .or_else(|| f.format_note.clone())
                .unwrap_or_else(|| "unknown".to_string());

            videos.push(VideoFormat {
                format_id: f.format_id.clone(),
                ext: f.ext.clone(),
                quality,
                filesize: f.filesize.or(f.filesize_approx),
                has_audio,
                url: f.url.clone(),
            });
        } else if has_audio {
            let quality = f
                .abr
                .map(|abr| format!("{}kbps", abr as u32))
                .or_else(|| f.format_note.clone())
                .unwrap_or_else(|| "audio".to_string());

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

    // Sort by quality (highest first)
    videos.sort_by(|a, b| {
        let a_height: u32 = a.quality.trim_end_matches('p').parse().unwrap_or(0);
        let b_height: u32 = b.quality.trim_end_matches('p').parse().unwrap_or(0);
        b_height.cmp(&a_height)
    });

    audios.sort_by(|a, b| {
        let a_br: u32 = a.quality.trim_end_matches("kbps").parse().unwrap_or(0);
        let b_br: u32 = b.quality.trim_end_matches("kbps").parse().unwrap_or(0);
        b_br.cmp(&a_br)
    });

    (videos, audios)
}

fn find_ytdlp() -> Option<String> {
    // Check system paths
    let paths = ["yt-dlp", "/usr/bin/yt-dlp", "/usr/local/bin/yt-dlp"];
    for path in paths {
        if Command::new(path)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(path.to_string());
        }
    }

    // Check app data directory
    if let Some(data_dir) = dirs::data_dir() {
        let app_path = data_dir.join("tur").join("bin").join("yt-dlp");
        if app_path.exists() {
            return Some(app_path.to_string_lossy().to_string());
        }
    }

    None
}

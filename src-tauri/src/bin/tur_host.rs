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
        formats: Vec<VideoFormat>,
    },

    #[serde(rename = "download_started")]
    DownloadStarted { id: String },

    #[serde(rename = "error")]
    Error { message: String },
}

/// Video format info
#[derive(Debug, Serialize)]
pub struct VideoFormat {
    pub format_id: String,
    pub ext: String,
    pub quality: String,
    pub filesize: Option<u64>,
    pub has_video: bool,
    pub has_audio: bool,
}

// ============================================================================
// Native Messaging Protocol
// ============================================================================

/// Read a message from stdin
fn read_message() -> io::Result<IncomingMessage> {
    let mut stdin = io::stdin().lock();

    // Read 4-byte length prefix (native byte order)
    let mut len_bytes = [0u8; 4];
    stdin.read_exact(&mut len_bytes)?;
    let len = u32::from_ne_bytes(len_bytes) as usize;

    // Validate length (max 1MB)
    if len > 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Message too large",
        ));
    }

    // Read JSON payload
    let mut buffer = vec![0u8; len];
    stdin.read_exact(&mut buffer)?;

    serde_json::from_slice(&buffer).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write a message to stdout
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
            // TODO: Replace with real yt-dlp call
            get_formats_dummy(&url)
        }

        IncomingMessage::Download { url, format_id } => {
            // TODO: Send to main app via IPC
            eprintln!(
                "[tur-host] Download request: {} format: {:?}",
                url, format_id
            );
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

/// Dummy format response (TODO: replace with yt-dlp)
fn get_formats_dummy(url: &str) -> OutgoingMessage {
    OutgoingMessage::Formats {
        url: url.to_string(),
        formats: vec![
            VideoFormat {
                format_id: "22".into(),
                ext: "mp4".into(),
                quality: "720p".into(),
                filesize: Some(50_000_000),
                has_video: true,
                has_audio: true,
            },
            VideoFormat {
                format_id: "18".into(),
                ext: "mp4".into(),
                quality: "360p".into(),
                filesize: Some(20_000_000),
                has_video: true,
                has_audio: true,
            },
        ],
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
                // EOF or error - extension disconnected
                eprintln!("[tur-host] Read error: {}", e);
                break;
            }
        }
    }

    eprintln!("[tur-host] Exiting");
}

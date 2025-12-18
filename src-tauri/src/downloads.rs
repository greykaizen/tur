//! Downloads module - manages download instances and requests
//!
//! Submodules:
//! - `client`: HTTP client configuration
//! - `constants`: PHI and RANGE constants
//! - `coordinator`: Range distribution and work stealing  
//! - `download`: Download struct and persistence
//! - `headers`: Header extraction utilities
//! - `index`: Atomic byte range tracking
//! - `manager`: Download lifecycle management and commands
//! - `workers`: Download execution tasks

pub mod client;
pub mod constants;
pub mod coordinator;
pub mod download;
pub mod headers;
pub mod index;
pub mod manager;
pub mod workers;

// Re-export main types for convenient access
pub use download::Download;
pub use headers::parse_deep_link as parse_deep_link_url;
pub use manager::{
    active_download_count, cancel_download, handle_download_request, is_download_active,
    pause_download, ControlCommand, DownloadManager, DownloadRequest,
};

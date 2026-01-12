use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};

use crate::settings::config::DependencyConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyKind {
    YtDlp,
    Ffmpeg,
}

impl DependencyKind {
    pub fn binary_name(&self) -> &'static str {
        match self {
            Self::YtDlp => {
                if cfg!(windows) {
                    "yt-dlp.exe"
                } else {
                    "yt-dlp"
                }
            }
            Self::Ffmpeg => {
                if cfg!(windows) {
                    "ffmpeg.exe"
                } else {
                    "ffmpeg"
                }
            }
        }
    }

    pub fn version_flag(&self) -> &'static str {
        match self {
            Self::YtDlp => "--version",
            Self::Ffmpeg => "-version",
        }
    }

    pub fn download_url(&self) -> String {
        match self {
            Self::YtDlp => {
                #[cfg(target_os = "linux")]
                {
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp".into()
                }
                #[cfg(target_os = "macos")]
                {
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos".into()
                }
                #[cfg(target_os = "windows")]
                {
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe".into()
                }
                // Fallback or other OS
                #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
                {
                    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp".into()
                }
            }
            Self::Ffmpeg => {
                // Placeholder: Ffmpeg download URLs are more complex (archives)
                // For now we might leave this empty or point to a known static build
                String::new()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DependencyInfo {
    pub kind: DependencyKind,
    pub path: PathBuf,
    pub version: Option<String>,
    pub installed_by_app: bool,
    pub age_days: Option<i64>,
}

pub struct DependencyManager {
    app_bin_dir: PathBuf,
    settings: DependencyConfig,
    cache: HashMap<DependencyKind, Option<PathBuf>>,
}

impl DependencyManager {
    pub fn new<R: tauri::Runtime>(app: &AppHandle<R>, settings: &DependencyConfig) -> Self {
        let app_bin_dir = app
            .path()
            .app_data_dir()
            .expect("Failed to get app data dir")
            .join("bin");

        Self {
            app_bin_dir,
            settings: settings.clone(),
            cache: HashMap::new(),
        }
    }

    /// Find dependency, checking: custom path → app bin → system PATH
    pub fn find(&mut self, kind: DependencyKind) -> Option<PathBuf> {
        // Check cache first
        if let Some(cached) = self.cache.get(&kind) {
            return cached.clone();
        }

        let result = self.find_uncached(kind);
        self.cache.insert(kind, result.clone());
        result
    }

    fn find_uncached(&self, kind: DependencyKind) -> Option<PathBuf> {
        // 1. Check custom path from settings
        let custom_path = match kind {
            DependencyKind::YtDlp => &self.settings.ytdlp_path,
            DependencyKind::Ffmpeg => &self.settings.ffmpeg_path,
        };

        if !custom_path.is_empty() {
            let path = PathBuf::from(custom_path);
            if self.is_valid_binary(&path, kind) {
                return Some(path);
            }
        }

        // 2. Check app bin directory
        let app_path = self.app_bin_dir.join(kind.binary_name());
        if self.is_valid_binary(&app_path, kind) {
            return Some(app_path);
        }

        // 3. Check system PATH
        // We use "which" command equivalent by trying to execute the binary name
        if let Ok(output) = Command::new(kind.binary_name())
            .arg(kind.version_flag())
            .output()
        {
            if output.status.success() {
                // Return just the name to let the OS resolve it, or verify full path if needed.
                // For now, PathBuf::from(binary_name) implies it's in PATH.
                return Some(PathBuf::from(kind.binary_name()));
            }
        }

        None
    }

    fn is_valid_binary(&self, path: &Path, kind: DependencyKind) -> bool {
        // If it's just a filename (system PATH), we can't check .exists() easily without "which"
        // But Command::new works for both absolute paths and PATH lookups.
        if path.is_absolute() && !path.exists() {
            return false;
        }

        Command::new(path)
            .arg(kind.version_flag())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if dependency was installed by this app
    pub fn is_app_installed(&self, kind: DependencyKind) -> bool {
        let app_path = self.app_bin_dir.join(kind.binary_name());
        app_path.exists()
    }

    /// Get version string from binary
    pub fn get_version(&mut self, kind: DependencyKind) -> Option<String> {
        let path = self.find(kind)?;

        let output = Command::new(&path).arg(kind.version_flag()).output().ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse version from first line
            Some(stdout.lines().next()?.trim().to_string())
        } else {
            None
        }
    }

    /// Check if version is older than 6 months (approximate)
    pub fn is_outdated(&mut self, kind: DependencyKind) -> bool {
        let version = match self.get_version(kind) {
            Some(v) => v,
            None => return false,
        };

        // yt-dlp versions are dates: 2024.01.15
        // Parse and compare to current date
        if let Some(date_str) = version.split_whitespace().next() {
            // yt-dlp version might look like "2024.01.15" or "2024.01.15.232"
            // We take the first 3 parts
            let parts: Vec<&str> = date_str.split('.').collect();
            if parts.len() >= 3 {
                let date_only = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
                if let Ok(version_date) = chrono::NaiveDate::parse_from_str(&date_only, "%Y.%m.%d")
                {
                    let today = chrono::Utc::now().date_naive();
                    let age = today.signed_duration_since(version_date);
                    return age.num_days() > 180; // 6 months
                }
            }
        }

        false
    }

    /// Install dependency to app bin directory
    pub async fn install(&self, kind: DependencyKind) -> Result<PathBuf, String> {
        // Ensure bin directory exists
        tokio::fs::create_dir_all(&self.app_bin_dir)
            .await
            .map_err(|e| format!("Failed to create bin directory: {}", e))?;

        let dest_path = self.app_bin_dir.join(kind.binary_name());
        let url = kind.download_url();

        if url.is_empty() {
            return Err("No download URL available for this dependency".to_string());
        }

        // Download binary
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Download failed: HTTP {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Write to file
        tokio::fs::write(&dest_path, &bytes)
            .await
            .map_err(|e| format!("Failed to write binary: {}", e))?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest_path)
                .map_err(|e| e.to_string())?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest_path, perms)
                .map_err(|e| format!("Failed to set permissions: {}", e))?;
        }

        // Verify installation
        if !self.is_valid_binary(&dest_path, kind) {
            tokio::fs::remove_file(&dest_path).await.ok();
            return Err("Downloaded binary is not valid".to_string());
        }

        Ok(dest_path)
    }

    /// Update dependency (only if app-installed)
    pub async fn update(&mut self, kind: DependencyKind) -> Result<PathBuf, String> {
        if !self.is_app_installed(kind) {
            return Err("Cannot update: dependency not installed by app".to_string());
        }

        // Remove old binary
        let old_path = self.app_bin_dir.join(kind.binary_name());
        tokio::fs::remove_file(&old_path).await.ok();

        // Install fresh
        let new_path = self.install(kind).await?;

        // Clear cache
        self.cache.remove(&kind);

        Ok(new_path)
    }

    /// Clear cached paths (call after settings change)
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn check_dependency(app: AppHandle, kind: String) -> Result<Option<DependencyInfo>, String> {
    let settings = crate::settings::store::load_or_create(&app);
    let mut manager = DependencyManager::new(&app, &settings.dependencies);

    let kind = match kind.as_str() {
        "ytdlp" | "yt-dlp" => DependencyKind::YtDlp,
        "ffmpeg" => DependencyKind::Ffmpeg,
        _ => return Err("Unknown dependency kind".to_string()),
    };

    match manager.find(kind) {
        Some(path) => Ok(Some(DependencyInfo {
            kind,
            path: path.clone(),
            version: manager.get_version(kind),
            installed_by_app: manager.is_app_installed(kind),
            age_days: None, // Logic for age calculation is in is_outdated, not returned here directly yet
        })),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn install_dependency(app: AppHandle, kind: String) -> Result<String, String> {
    let settings = crate::settings::store::load_or_create(&app);
    let manager = DependencyManager::new(&app, &settings.dependencies);

    let kind = match kind.as_str() {
        "ytdlp" | "yt-dlp" => DependencyKind::YtDlp,
        "ffmpeg" => DependencyKind::Ffmpeg,
        _ => return Err("Unknown dependency kind".to_string()),
    };

    let path = manager.install(kind).await?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn update_dependency(app: AppHandle, kind: String) -> Result<String, String> {
    let settings = crate::settings::store::load_or_create(&app);
    let mut manager = DependencyManager::new(&app, &settings.dependencies);

    let kind = match kind.as_str() {
        "ytdlp" | "yt-dlp" => DependencyKind::YtDlp,
        "ffmpeg" => DependencyKind::Ffmpeg,
        _ => return Err("Unknown dependency kind".to_string()),
    };

    let path = manager.update(kind).await?;
    Ok(path.to_string_lossy().to_string())
}

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Resolves destination path conflicts by adding numeric suffix like (1), (2), etc.
/// Returns the resolved path and potentially modified filename.
pub fn resolve_destination_conflict(downloads_dir: &Path, filename: &str) -> (PathBuf, String) {
    let original_path = downloads_dir.join(filename);

    if !original_path.exists() {
        return (original_path, filename.to_string());
    }

    let path = Path::new(filename);
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or(filename);
    let extension = path.extension().and_then(OsStr::to_str);

    for i in 1..1000 {
        let new_filename = match extension {
            Some(ext) => format!("{} ({}).{}", stem, i, ext),
            None => format!("{} ({})", stem, i),
        };
        let new_path = downloads_dir.join(&new_filename);
        if !new_path.exists() {
            return (new_path, new_filename);
        }
    }

    // Fallback: use UUID suffix if all numbers exhausted (unlikely)
    let uuid_suffix = Uuid::now_v7()
        .to_string()
        .split('-')
        .next()
        .unwrap()
        .to_string();
    let new_filename = match extension {
        Some(ext) => format!("{}_{}.{}", stem, uuid_suffix, ext),
        None => format!("{}_{}", stem, uuid_suffix),
    };
    (downloads_dir.join(&new_filename), new_filename)
}

//! HTTP response header extraction utilities

use reqwest::header::HeaderMap;
use url::Url;

/// Extract filename from Content-Disposition header
pub fn extract_filename(headers: &HeaderMap) -> Option<String> {
    headers
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(|cd| {
            // Parse Content-Disposition header for filename
            cd.split(';').find_map(|part| {
                let part = part.trim();
                if part.starts_with("filename=") {
                    Some(part[9..].trim_matches('"').to_string())
                } else if part.starts_with("filename*=") {
                    // Handle RFC 5987 encoded filenames
                    part[10..].split('\'').nth(2).map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
}

/// Extract filename from URL path as fallback
pub fn extract_filename_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|s| s.split('?').next()) // Remove query parameters
        .and_then(|s| s.split('#').next()) // Remove fragments
        .filter(|s| !s.is_empty())
        .unwrap_or("download")
        .to_string()
}

/// Extract Content-Length header
pub fn extract_content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

/// Extract ETag header (with quotes removed)
pub fn extract_etag(headers: &HeaderMap) -> Option<String> {
    headers
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string())
}

/// Extract Last-Modified header
pub fn extract_last_modified(headers: &HeaderMap) -> Option<String> {
    headers
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Check if server supports range requests
pub fn supports_resume(headers: &HeaderMap) -> bool {
    headers
        .get(reqwest::header::ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("bytes"))
        .unwrap_or(false)
}

/// Parse deep link URL and extract download info
pub fn parse_deep_link(url_str: &str) -> Option<(Url, Option<String>, Option<u64>)> {
    let parsed = Url::parse(url_str).ok()?;

    let src_url_str = parsed
        .query_pairs()
        .find(|(k, _)| k == "url")?
        .1
        .to_string();
    let src_url = Url::parse(&src_url_str).ok()?;

    let filename = parsed
        .query_pairs()
        .find(|(k, _)| k == "filename")
        .map(|(_, v)| v.to_string());
    let size_opt = parsed
        .query_pairs()
        .find(|(k, _)| k == "size")
        .and_then(|(_, v)| v.parse::<u64>().ok());

    Some((src_url, filename, size_opt))
}

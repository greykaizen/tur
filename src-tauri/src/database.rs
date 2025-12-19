use rusqlite::{params, Connection, Result};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use tauri::Manager;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Download {
    pub id: Uuid,
    pub filename: String,
    pub status: Option<String>, // None = in-progress, Some("completed"|"paused"|"failed")
    pub size: Option<i64>,
    pub bytes_received: i64,
    pub url: String,
    pub etag: Option<String>,
    pub content_type: Option<String>,
    pub last_modified: Option<String>,
    pub destination: String,
    pub accept_ranges: bool,
    pub updated_at: i64,
}

impl Download {
    /// Get the created_at timestamp from the UUID v7
    pub fn created_at(&self) -> Option<i64> {
        extract_timestamp_from_uuid_v7(&self.id)
    }

    /// Check if download is completed
    pub fn is_completed(&self) -> bool {
        self.status.as_deref() == Some("completed")
    }

    /// Check if download is in progress
    pub fn is_in_progress(&self) -> bool {
        self.status.is_none()
    }

    /// Get download progress as percentage (0.0 to 1.0)
    pub fn progress(&self) -> Option<f64> {
        self.size.map(|total| {
            if total > 0 {
                self.bytes_received as f64 / total as f64
            } else {
                0.0
            }
        })
    }
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode for better concurrent access
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "cache_size", 10000)?;
        conn.pragma_update(None, "temp_store", "memory")?;

        // Create table with improved schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS downloads (
                id             BLOB PRIMARY KEY,
                filename       TEXT NOT NULL,
                status         TEXT CHECK (status IN ('completed', 'paused', 'failed')),
                size           INTEGER,
                bytes_received INTEGER NOT NULL DEFAULT 0,
                url            TEXT NOT NULL,
                etag           TEXT,
                content_type   TEXT,
                last_modified  TEXT,
                destination    TEXT NOT NULL,
                accept_ranges  INTEGER NOT NULL DEFAULT 0,
                updated_at     INTEGER NOT NULL DEFAULT (unixepoch())
            )",
            [],
        )?;

        // Create indexes for better performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_downloads_updated_at ON downloads(updated_at)",
            [],
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Initialize database with proper app data directory path
    pub fn initialize<R: tauri::Runtime>(
        app_handle: &tauri::AppHandle<R>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data directory: {}", e))?;

        // Ensure directory exists
        std::fs::create_dir_all(&app_data_dir)
            .map_err(|e| format!("Failed to create app data directory: {}", e))?;

        let db_path = app_data_dir.join("tur.db");
        Self::new(&db_path).map_err(|e| format!("Failed to initialize database: {}", e).into())
    }

    /// Check if database exists and create if it doesn't
    pub fn ensure_exists(db_path: &Path) -> Result<Self> {
        Self::new(db_path)
    }

    /// Insert a new download record
    pub fn insert_download(
        &self,
        id: &Uuid,
        url: &str,
        filename: &str,
        destination: &str,
        size: Option<i64>,
        content_type: Option<&str>,
        etag: Option<&str>,
        last_modified: Option<&str>,
        accept_ranges: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (
                id, url, filename, destination, size, content_type, 
                etag, last_modified, accept_ranges, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, unixepoch())",
            params![
                id.as_bytes(),
                url,
                filename,
                destination,
                size,
                content_type,
                etag,
                last_modified,
                accept_ranges as i32
            ],
        )?;
        Ok(())
    }

    /// Update headers for an existing download
    pub fn update_headers(
        &self,
        id: &Uuid,
        size: Option<i64>,
        content_type: Option<&str>,
        etag: Option<&str>,
        last_modified: Option<&str>,
        accept_ranges: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET 
                size = ?2, content_type = ?3, etag = ?4, 
                last_modified = ?5, accept_ranges = ?6, updated_at = unixepoch() 
             WHERE id = ?1",
            params![
                id.as_bytes(),
                size,
                content_type,
                etag,
                last_modified,
                accept_ranges as i32
            ],
        )?;
        Ok(())
    }

    /// Get resume information for multiple downloads
    pub fn get_resume_info(&self, ids: Vec<&Uuid>) -> Result<Vec<Download>> {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();

        for id in ids {
            if let Some(download) = self.get_download_by_id_internal(&conn, id)? {
                results.push(download);
            }
        }

        Ok(results)
    }

    /// Mark a download as completed
    pub fn mark_completed(&self, id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET status = 'completed', updated_at = unixepoch() WHERE id = ?1",
            params![id.as_bytes()],
        )?;
        Ok(())
    }

    /// Get all incomplete downloads (status is NULL)
    pub fn get_incomplete(&self) -> Result<Vec<(Uuid, String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, url, bytes_received FROM downloads WHERE status IS NULL")?;

        let downloads = stmt.query_map([], |row| {
            let id_bytes: Vec<u8> = row.get(0)?;
            let uuid = Uuid::from_slice(&id_bytes).unwrap();
            Ok((uuid, row.get(1)?, row.get(2)?))
        })?;

        downloads.collect()
    }

    /// Get all downloads for history page
    pub fn get_downloads(&self) -> Result<Vec<Download>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, status, size, bytes_received, url, etag, 
                    content_type, last_modified, destination, accept_ranges, updated_at
             FROM downloads ORDER BY updated_at DESC",
        )?;

        let downloads = stmt.query_map([], |row| self.row_to_download(row))?;

        downloads.collect()
    }

    /// Delete a single download record
    pub fn delete_download(&self, id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM downloads WHERE id = ?1",
            params![id.as_bytes()],
        )?;
        Ok(())
    }

    /// Purge all records from database
    pub fn purge(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM downloads", [])?;
        Ok(())
    }

    /// Get a single download by ID
    pub fn get_download_by_id(&self, id: &Uuid) -> Result<Option<Download>> {
        let conn = self.conn.lock().unwrap();
        self.get_download_by_id_internal(&conn, id)
    }

    /// Internal helper for getting download by ID (reusable with existing connection)
    fn get_download_by_id_internal(
        &self,
        conn: &Connection,
        id: &Uuid,
    ) -> Result<Option<Download>> {
        let mut stmt = conn.prepare(
            "SELECT id, filename, status, size, bytes_received, url, etag, 
                    content_type, last_modified, destination, accept_ranges, updated_at
             FROM downloads WHERE id = ?1",
        )?;

        let result = stmt.query_row(params![id.as_bytes()], |row| self.row_to_download(row));

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update download progress (bytes_received)
    pub fn update_progress(&self, id: &Uuid, bytes_received: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET bytes_received = ?2, updated_at = unixepoch() WHERE id = ?1",
            params![id.as_bytes(), bytes_received],
        )?;
        Ok(())
    }

    /// Get downloads filtered by status
    pub fn get_downloads_by_status(&self, status: Option<&str>) -> Result<Vec<Download>> {
        let conn = self.conn.lock().unwrap();

        match status {
            Some(s) => {
                let mut stmt = conn.prepare(
                    "SELECT id, filename, status, size, bytes_received, url, etag, 
                            content_type, last_modified, destination, accept_ranges, updated_at
                     FROM downloads WHERE status = ?1 ORDER BY updated_at DESC",
                )?;
                let downloads = stmt.query_map([s], |row| self.row_to_download(row))?;
                downloads.collect()
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, filename, status, size, bytes_received, url, etag, 
                            content_type, last_modified, destination, accept_ranges, updated_at
                     FROM downloads WHERE status IS NULL ORDER BY updated_at DESC",
                )?;
                let downloads = stmt.query_map([], |row| self.row_to_download(row))?;
                downloads.collect()
            }
        }
    }

    /// Update download status (completed, paused, failed)
    pub fn update_status(&self, id: &Uuid, status: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET status = ?2, updated_at = unixepoch() WHERE id = ?1",
            params![id.as_bytes(), status],
        )?;
        Ok(())
    }

    /// Helper to convert database row to Download struct
    fn row_to_download(&self, row: &rusqlite::Row) -> rusqlite::Result<Download> {
        let id_bytes: Vec<u8> = row.get(0)?;
        let uuid = Uuid::from_slice(&id_bytes).unwrap();

        Ok(Download {
            id: uuid,
            filename: row.get(1)?,
            status: row.get(2)?,
            size: row.get(3)?,
            bytes_received: row.get(4)?,
            url: row.get(5)?,
            etag: row.get(6)?,
            content_type: row.get(7)?,
            last_modified: row.get(8)?,
            destination: row.get(9)?,
            accept_ranges: row.get::<_, i32>(10)? != 0,
            updated_at: row.get(11)?,
        })
    }
}

/// Extract created_at timestamp from UUID v7
pub fn extract_timestamp_from_uuid_v7(id: &Uuid) -> Option<i64> {
    // UUID v7 has timestamp in first 48 bits (6 bytes)
    let bytes = id.as_bytes();
    if bytes.len() >= 6 {
        let timestamp_ms = u64::from_be_bytes([
            0, 0, // pad with zeros
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5],
        ]);
        Some(timestamp_ms as i64 / 1000) // convert to seconds
    } else {
        None
    }
}

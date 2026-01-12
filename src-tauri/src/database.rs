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
    pub queue_id: Option<Uuid>,
    pub scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
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

#[derive(Debug, Clone, Serialize)]
pub struct Queue {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
    pub mode: String,        // 'sequential' or 'concurrent'
    pub parallel_count: i32, // for concurrent mode
    pub status: String,      // 'active', 'paused', 'completed'
    pub created_at: i64,
    pub depends_on: Option<Uuid>,
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
                status         TEXT CHECK (status IN ('completed', 'paused', 'failed', 'cancelled')),
                size           INTEGER,
                bytes_received INTEGER NOT NULL DEFAULT 0,
                url            TEXT NOT NULL,
                etag           TEXT,
                content_type   TEXT,
                last_modified  TEXT,
                destination    TEXT NOT NULL,
                accept_ranges  INTEGER NOT NULL DEFAULT 0,
                updated_at     INTEGER NOT NULL DEFAULT (unixepoch()),
                queue_id       BLOB
            )",
            [],
        )?;

        // Create queues table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS queues (
                id             BLOB PRIMARY KEY,
                name           TEXT NOT NULL,
                color          TEXT,
                mode           TEXT NOT NULL DEFAULT 'sequential',
                parallel_count INTEGER NOT NULL DEFAULT 2,
                status         TEXT NOT NULL DEFAULT 'active',
                created_at     INTEGER NOT NULL DEFAULT (unixepoch())
            )",
            [],
        )?;

        // Migrations for existing tables
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN queue_id BLOB", []);
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN queue_position INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE queues ADD COLUMN mode TEXT NOT NULL DEFAULT 'sequential'",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE queues ADD COLUMN parallel_count INTEGER NOT NULL DEFAULT 2",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE queues ADD COLUMN status TEXT NOT NULL DEFAULT 'active'",
            [],
        );

        // Scheduling System Migrations
        let _ = conn.execute("ALTER TABLE downloads ADD COLUMN scheduled_at TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN schedule_cleared INTEGER DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE queues ADD COLUMN depends_on BLOB", []);

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
        queue_id: Option<&Uuid>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (
                id, url, filename, destination, size, content_type, 
                etag, last_modified, accept_ranges, updated_at, queue_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, unixepoch(), ?10)",
            params![
                id.as_bytes(),
                url,
                filename,
                destination,
                size,
                content_type,
                etag,
                last_modified,
                accept_ranges as i32,
                queue_id.map(|u| u.as_bytes())
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
                    content_type, last_modified, destination, accept_ranges, updated_at, queue_id, scheduled_at
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

    /// Get pending downloads for auto-start (status NULL or 'paused')
    pub fn get_queued_downloads(&self, limit: usize) -> Result<Vec<Download>> {
        let conn = self.conn.lock().unwrap();
        // efficient query for pending items
        let mut stmt = conn.prepare(
            "SELECT id, filename, status, size, bytes_received, url, etag, 
                    content_type, last_modified, destination, accept_ranges, updated_at, queue_id
             FROM downloads 
             WHERE status IS NULL OR status = 'paused'
             ORDER BY updated_at ASC
             LIMIT ?1",
        )?;

        let downloads = stmt.query_map([limit], |row| self.row_to_download(row))?;
        downloads.collect()
    }

    /// Puritan purge all records from database
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
                    content_type, last_modified, destination, accept_ranges, updated_at, queue_id, scheduled_at
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
                            content_type, last_modified, destination, accept_ranges, updated_at, queue_id, scheduled_at
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
            queue_id: row
                .get::<_, Option<Vec<u8>>>(12)?
                .map(|b| Uuid::from_slice(&b).unwrap()),
            scheduled_at: row.get::<_, Option<String>>(13)?.map(
                |s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now())
                }, // Fallback or handle error better?
            ), // Actually scheduled_at is TEXT ISO8601
        })
    }

    // --- Queue Management ---

    /// Create a new queue
    pub fn create_queue(
        &self,
        name: &str,
        color: Option<&str>,
        mode: &str,
        parallel_count: i32,
    ) -> Result<Queue> {
        let id = Uuid::now_v7();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO queues (id, name, color, mode, parallel_count, status, created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', unixepoch())",
            params![id.as_bytes(), name, color, mode, parallel_count],
        )?;

        Ok(Queue {
            id,
            name: name.to_string(),
            color: color.map(|s| s.to_string()),
            mode: mode.to_string(),
            parallel_count,
            status: "active".to_string(),
            created_at: extract_timestamp_from_uuid_v7(&id).unwrap_or(0),
            depends_on: None,
        })
    }

    /// Get all queues
    pub fn get_queues(&self) -> Result<Vec<Queue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, color, mode, parallel_count, status, created_at, depends_on
             FROM queues ORDER BY created_at ASC",
        )?;

        let queues = stmt.query_map([], |row| {
            let id_bytes: Vec<u8> = row.get(0)?;
            Ok(Queue {
                id: Uuid::from_slice(&id_bytes).unwrap(),
                name: row.get(1)?,
                color: row.get(2)?,
                mode: row.get(3)?,
                parallel_count: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                depends_on: row
                    .get::<_, Option<Vec<u8>>>(7)?
                    .map(|b| Uuid::from_slice(&b).unwrap()),
            })
        })?;

        queues.collect()
    }

    /// Get a single queue by ID
    pub fn get_queue_by_id(&self, queue_id: &Uuid) -> Result<Option<Queue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, color, mode, parallel_count, status, created_at, depends_on
             FROM queues WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![queue_id.as_bytes()])?;
        if let Some(row) = rows.next()? {
            let id_bytes: Vec<u8> = row.get(0)?;
            Ok(Some(Queue {
                id: Uuid::from_slice(&id_bytes).unwrap(),
                name: row.get(1)?,
                color: row.get(2)?,
                mode: row.get(3)?,
                parallel_count: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                depends_on: row
                    .get::<_, Option<Vec<u8>>>(7)?
                    .map(|b| Uuid::from_slice(&b).unwrap()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get download IDs in a queue that are pending (not yet started or queued status)
    /// Returns them ordered by queue_position
    pub fn get_pending_queue_downloads(&self, queue_id: &Uuid) -> Result<Vec<Uuid>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id FROM downloads 
             WHERE queue_id = ?1 AND (status IS NULL OR status = 'paused')
             ORDER BY queue_position ASC, updated_at ASC",
        )?;

        let ids: Vec<Uuid> = stmt
            .query_map(params![queue_id.as_bytes()], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                Ok(Uuid::from_slice(&id_bytes).unwrap())
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(ids)
    }

    /// Count currently active (non-completed, non-paused) downloads in a queue
    pub fn count_active_in_queue(&self, queue_id: &Uuid) -> Result<i32> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM downloads 
             WHERE queue_id = ?1 AND status IS NULL",
            params![queue_id.as_bytes()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Update queue status
    pub fn update_queue_status(&self, queue_id: &Uuid, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE queues SET status = ?2 WHERE id = ?1",
            params![queue_id.as_bytes(), status],
        )?;
        Ok(())
    }

    /// Check if all downloads in a queue are completed
    pub fn is_queue_complete(&self, queue_id: &Uuid) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        // Count non-completed downloads in queue
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM downloads 
             WHERE queue_id = ?1 AND (status IS NULL OR status != 'completed')",
            params![queue_id.as_bytes()],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }

    /// Add a download to a queue with position
    pub fn add_download_to_queue(
        &self,
        download_id: &Uuid,
        queue_id: &Uuid,
        position: Option<i32>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let pos = position.unwrap_or_else(|| {
            // Get next available position
            conn.query_row(
                "SELECT COALESCE(MAX(queue_position), 0) + 1 FROM downloads WHERE queue_id = ?1",
                params![queue_id.as_bytes()],
                |row| row.get(0),
            )
            .unwrap_or(1)
        });
        conn.execute(
            "UPDATE downloads SET queue_id = ?2, queue_position = ?3 WHERE id = ?1",
            params![download_id.as_bytes(), queue_id.as_bytes(), pos],
        )?;
        Ok(())
    }

    /// Remove a download from its queue
    pub fn remove_download_from_queue(&self, download_id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET queue_id = NULL, queue_position = NULL WHERE id = ?1",
            params![download_id.as_bytes()],
        )?;
        Ok(())
    }

    /// Delete a queue (and unset queue_id in downloads)
    pub fn delete_queue(&self, id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // Unset queue_id for downloads in this queue
        conn.execute(
            "UPDATE downloads SET queue_id = NULL WHERE queue_id = ?1",
            params![id.as_bytes()],
        )?;
        // Delete the queue
        conn.execute("DELETE FROM queues WHERE id = ?1", params![id.as_bytes()])?;
        Ok(())
    }
    /// Get downloads in a queue ordered by position
    pub fn get_queue_downloads_ordered(&self, queue_id: &Uuid) -> Result<Vec<Download>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, status, size, bytes_received, url, etag, 
                    content_type, last_modified, destination, accept_ranges, 
                    updated_at, queue_id, queue_position, scheduled_at
             FROM downloads 
             WHERE queue_id = ?1
             ORDER BY queue_position ASC, updated_at ASC",
        )?;

        let downloads = stmt.query_map(params![queue_id.as_bytes()], |row| {
            self.row_to_download(row)
        })?;

        downloads.collect()
    }

    /// Update a download's position in queue
    pub fn update_download_position(&self, download_id: &Uuid, position: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET queue_position = ?2 WHERE id = ?1",
            params![download_id.as_bytes(), position],
        )?;
        Ok(())
    }

    /// Check if queue name already exists
    pub fn queue_name_exists(&self, name: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM queues WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // --- Scheduling System ---

    /// Get all downloads with schedules
    pub fn get_scheduled_downloads(&self) -> Result<Vec<Download>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, status, size, bytes_received, url, etag, 
                    content_type, last_modified, destination, accept_ranges, updated_at, queue_id
             FROM downloads WHERE scheduled_at IS NOT NULL AND status = 'scheduled'",
        )?;

        let downloads = stmt.query_map([], |row| self.row_to_download(row))?;
        downloads.collect()
    }

    /// Set a download's schedule
    pub fn set_download_schedule(
        &self,
        download_id: &Uuid,
        scheduled_at: &chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET scheduled_at = ?2, status = 'scheduled' WHERE id = ?1",
            params![download_id.as_bytes(), scheduled_at.to_rfc3339()],
        )?;
        Ok(())
    }

    /// Clear a download's schedule
    pub fn clear_download_schedule(&self, download_id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET scheduled_at = NULL WHERE id = ?1",
            params![download_id.as_bytes()],
        )?;
        Ok(())
    }

    /// Update download status wrapper (already exists as update_status, aliasing for clarity if needed or just use update_status)
    pub fn update_download_status(&self, download_id: &Uuid, status: &str) -> Result<()> {
        self.update_status(download_id, Some(status))
    }

    /// Get queues with dependencies
    pub fn get_queues_with_dependencies(&self) -> Result<Vec<Queue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, color, mode, parallel_count, status, created_at, depends_on
             FROM queues WHERE depends_on IS NOT NULL",
        )?;

        let queues = stmt.query_map([], |row| {
            let id_bytes: Vec<u8> = row.get(0)?;
            let depends_on_bytes: Option<Vec<u8>> = row.get(7)?;

            Ok(Queue {
                id: Uuid::from_slice(&id_bytes).unwrap(),
                name: row.get(1)?,
                color: row.get(2)?,
                mode: row.get(3)?,
                parallel_count: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                depends_on: depends_on_bytes.map(|b| Uuid::from_slice(&b).unwrap()),
            })
        })?;

        queues.collect()
    }

    /// Set queue dependency
    pub fn set_queue_dependency(&self, queue_id: &Uuid, depends_on: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE queues SET depends_on = ?2 WHERE id = ?1",
            params![queue_id.as_bytes(), depends_on.as_bytes()],
        )?;
        Ok(())
    }

    /// Clear queue dependency
    pub fn clear_queue_dependency(&self, queue_id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE queues SET depends_on = NULL WHERE id = ?1",
            params![queue_id.as_bytes()],
        )?;
        Ok(())
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

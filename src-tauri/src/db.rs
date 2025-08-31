use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

// schema
// CREATE TABLE IF NOT EXISTS downloads (
//     id             BLOB PRIMARY KEY,           -- 16-byte UUIDv7
//     url            TEXT NOT NULL,
//     filename       TEXT NOT NULL,
//     status         TEXT CHECK (status = 'completed'),  -- NULL = not completed
//     size           INTEGER,
//     bytes_received INTEGER NOT NULL DEFAULT 0,
//     content_type   TEXT,
//     etag           TEXT,
//     last_modified  TEXT,
//     accept_ranges  INTEGER,
//     destination    TEXT NOT NULL,
//     updated_at     INTEGER NOT NULL DEFAULT (unixepoch())
// );
// CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);
pub struct DownloadDb {
    conn: Mutex<Connection>,
}

impl DownloadDb {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Enable WAL for concurrent reads
        conn.execute("PRAGMA journal_mode=WAL", [])?;

        // Create table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS downloads (
                id             BLOB PRIMARY KEY,
                url            TEXT NOT NULL,
                filename       TEXT NOT NULL,
                status         TEXT CHECK (status = 'completed'),
                size           INTEGER,
                bytes_received INTEGER NOT NULL DEFAULT 0,
                content_type   TEXT,
                etag           TEXT,
                last_modified  TEXT,
                accept_ranges  INTEGER,
                destination    TEXT NOT NULL,
                updated_at     INTEGER NOT NULL DEFAULT (unixepoch())
            )",
            [],
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_download(
        &self,
        id: &Uuid,
        url: &str,
        filename: &str,
        destination: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (id, url, filename, destination, updated_at) 
             VALUES (?1, ?2, ?3, ?4, unixepoch())",
            params![id.as_bytes(), url, filename, destination],
        )?;
        Ok(())
    }

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
            "UPDATE downloads SET size = ?2, content_type = ?3, etag = ?4, 
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

    // mark completed via id
    pub fn mark_completed(&self, id: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET status = 'completed', updated_at = unixepoch() WHERE id = ?1",
            params![id.as_bytes()],
        )?;
        Ok(())
    }


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
}

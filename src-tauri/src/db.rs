use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ResumeInfo {
    pub id: Uuid,
    pub url: String,
    pub filename: String,
    pub size: Option<i64>,
    pub bytes_received: i64,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub destination: String,
}

// help me out in some db methods like, we need some correction in some old methods and need to write a method or two. only write these methods not the whole db.rs file

// 1) pub fn insert_download(
//         &self,
//         id: &Uuid,
//         url: &str,
//         filename: &str,
//         destination: &str,
//     )
// - we need to fix this method to take in all necessary parameter for record insertion

// 2)  pub fn update_headers(
//         &self,
//         id: &Uuid,
//         size: Option<i64>,
//         content_type: Option<&str>,
//         etag: Option<&str>,
//         last_modified: Option<&str>,
//         accept_ranges: bool,
//     )
// - update_at updates as well on headers updates.

// 3) pub fn get_resume_info(&self, ids: Vec<&Uuid>) {
//         // return record related to that id except the following entries
//         // !status, !accept_range, !updated_at
//         // as in paramater we pass vec of uuid in return we need vec of records
//     }
// - we need to write this method as well

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

    pub fn update_download(){}

    // new loads them all at once
    // deeplink decodes them all at once
    // resume has twice emit delay
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
            "INSERT INTO downloads (id, url, filename, destination, size, content_type, etag, last_modified, accept_ranges, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, unixepoch())",
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

    pub fn get_resume_info(&self, ids: Vec<&Uuid>) -> Result<Vec<ResumeInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();
        
        for id in ids {
            let mut stmt = conn.prepare(
                "SELECT id, url, filename, size, bytes_received, content_type, etag, last_modified, destination 
                 FROM downloads WHERE id = ?1"
            )?;
            
            let resume_info = stmt.query_row(params![id.as_bytes()], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let uuid = Uuid::from_slice(&id_bytes).unwrap();
                
                Ok(ResumeInfo {
                    id: uuid,
                    url: row.get(1)?,
                    filename: row.get(2)?,
                    size: row.get(3)?,
                    bytes_received: row.get(4)?,
                    content_type: row.get(5)?,
                    etag: row.get(6)?,
                    last_modified: row.get(7)?,
                    destination: row.get(8)?,
                })
            });
            
            match resume_info {
                Ok(info) => results.push(info),
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Skip missing records
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        Ok(results)
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

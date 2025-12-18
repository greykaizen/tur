//! Download struct and persistence

use bincode::{config, error::DecodeError, error::EncodeError, Decode, Encode};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::constants::RANGE;
use super::coordinator::Coordinator;
use super::index::Index;

/// Minimal Download struct - only holds what's needed for coordination
/// Other info (url, destination, size) passed as parameters to run_instance
pub struct Download {
    pub coordinator: Coordinator,
    pub range: Vec<Arc<Index>>,
}

impl Encode for Download {
    fn encode<E: bincode::enc::Encoder>(&self, e: &mut E) -> Result<(), EncodeError> {
        // Encode Coordinator state
        self.coordinator.range_byte.start.encode(e)?;
        self.coordinator.range_byte.end.encode(e)?;
        self.coordinator.steal_ptr.encode(e)?;
        self.coordinator.steal_exhausted.encode(e)?;

        // Encode only incomplete ranges (start < end)
        let incomplete: Vec<_> = self
            .range
            .iter()
            .filter(|idx| idx.start.load(Ordering::Relaxed) < idx.end.load(Ordering::Relaxed))
            .collect();

        incomplete.len().encode(e)?;
        for index in incomplete {
            index.encode(e)?;
        }
        Ok(())
    }
}

impl<Context> Decode<Context> for Download {
    fn decode<D: bincode::de::Decoder<Context = Context>>(d: &mut D) -> Result<Self, DecodeError> {
        let current = u8::decode(d)?;
        let max_index = u8::decode(d)?;
        let steal_ptr = u8::decode(d)?;
        let steal_exhausted = bool::decode(d)?;

        let mut coordinator =
            Coordinator::from_parts(current, max_index, steal_ptr, steal_exhausted);

        let len = usize::decode(d)?;
        let mut range = Vec::with_capacity(len);
        for _ in 0..len {
            range.push(Arc::new(Index::decode(d)?));
        }

        // Restore steal_ptr to valid position after loading cleaned Vec
        if !range.is_empty() {
            coordinator.steal_ptr = coordinator.steal_ptr.min((range.len() - 1) as u8);
            if coordinator.steal_ptr < 2 && range.len() >= 3 {
                coordinator.steal_ptr = 2;
            }
        }

        Ok(Download { coordinator, range })
    }
}

impl Download {
    /// Create new Download instance
    /// - size: file size in bytes (used to calculate max range index)
    /// - num_conn: number of worker threads
    pub fn new(size: usize, num_conn: u8) -> Self {
        let max_index = Self::get_index(size >> 23).unwrap_or(0);
        Download {
            coordinator: Coordinator::new(max_index),
            range: Vec::with_capacity(num_conn as usize),
        }
    }

    /// Binary search to find RANGE index for given file size
    /// Pass value as (value >> 23) i.e. (value/2^20/8)
    pub fn get_index(v: usize) -> Option<u8> {
        if v == 0 {
            return Some(0);
        }

        let mut lo = if v <= RANGE[13].start { 0 } else { 13 };
        let mut hi = if v <= RANGE[13].start { 12 } else { 59 };

        while lo < hi {
            let mid = (lo + hi) >> 1;
            if RANGE[mid].start < v {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        (lo < RANGE.len()).then_some(lo as u8)
    }

    /// Load Download state from disk (for resume)
    pub fn load<R: tauri::Runtime>(
        handle: &tauri::AppHandle<R>,
        id: &uuid::Uuid,
    ) -> Result<Self, DecodeError> {
        let path = Self::meta_path(handle, id);
        let mut file = std::fs::File::open(&path).map_err(|e| DecodeError::Io {
            inner: e,
            additional: 0,
        })?;
        bincode::decode_from_std_read(&mut file, config::standard())
    }

    /// Get metadata file path
    pub fn meta_path<R: tauri::Runtime>(
        handle: &tauri::AppHandle<R>,
        id: &uuid::Uuid,
    ) -> std::path::PathBuf {
        use tauri::path::BaseDirectory;
        use tauri::Manager;

        let mut path = handle
            .path()
            .resolve("metadata", BaseDirectory::AppData)
            .expect("cannot resolve AppData/metadata");
        std::fs::create_dir_all(&path).ok();
        path.push(format!("{}.tur", id.as_simple()));
        path
    }

    /// Save Download state to disk
    pub fn save<R: tauri::Runtime>(
        &self,
        handle: &tauri::AppHandle<R>,
        id: &uuid::Uuid,
    ) -> Result<(), EncodeError> {
        let path = Self::meta_path(handle, id);
        let mut file =
            std::fs::File::create(&path).map_err(|e| EncodeError::Io { inner: e, index: 0 })?;
        bincode::encode_into_std_write(self, &mut file, config::standard()).map(|_| ())
    }

    /// Calculate total bytes still remaining to download
    pub fn bytes_remaining(&self) -> usize {
        self.range
            .iter()
            .map(|idx| {
                let start = idx.start.load(Ordering::Relaxed);
                let end = idx.end.load(Ordering::Relaxed);
                end.saturating_sub(start)
            })
            .sum()
    }
}

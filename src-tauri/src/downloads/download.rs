//! Download struct and persistence

use bincode::{config, error::DecodeError, error::EncodeError, Decode, Encode};
use std::sync::{atomic::Ordering, Arc};

use super::constants::RANGE;
use super::coordinator::Coordinator;
use super::index::Index;

/// Minimal Download struct - only holds what's needed for coordination
/// Other info (url, destination, size) passed as parameters to run_instance
pub struct Download {
    pub coordinator: Coordinator,
    pub indices: Vec<Arc<Index>>, // Fixed size = num_connections
}

impl Encode for Download {
    fn encode<E: bincode::enc::Encoder>(&self, e: &mut E) -> Result<(), EncodeError> {
        // Encode Coordinator state
        self.coordinator.range_byte.start.encode(e)?;
        self.coordinator.range_byte.end.encode(e)?;
        self.coordinator.steal_ptr.encode(e)?;
        self.coordinator.steal_exhausted.encode(e)?;
        self.coordinator.total_size.encode(e)?;

        // Encode indices snapshot
        // We only save incomplete indices or significant state
        // For simplicity and correctness with the new design, we snapshot all indices
        // but only those active are truly needed. However, to maintain fixed structure on load:
        let snapshot = self.snapshot_indices();
        snapshot.len().encode(e)?;
        for (start, end, state) in snapshot {
            start.encode(e)?;
            end.encode(e)?;
            state.encode(e)?;
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
        let total_size = usize::decode(d)?;

        let mut coordinator =
            Coordinator::from_parts(current, max_index, steal_ptr, steal_exhausted, total_size);

        let len = usize::decode(d)?;
        let mut indices = Vec::with_capacity(len);
        for _ in 0..len {
            let start = usize::decode(d)?;
            let end = usize::decode(d)?;
            let state = u8::decode(d)?;

            let idx = Index::new();
            idx.start.store(start, Ordering::Relaxed);
            idx.end.store(end, Ordering::Relaxed);
            idx.state.store(state, Ordering::Relaxed);
            indices.push(Arc::new(idx));
        }

        // Restore steal_ptr to valid position after loading cleaned Vec
        if !indices.is_empty() {
            coordinator.steal_ptr = coordinator.steal_ptr.min((indices.len() - 1) as u8);
            if coordinator.steal_ptr < 2 && indices.len() >= 3 {
                coordinator.steal_ptr = 2;
            }
        }

        Ok(Download {
            coordinator,
            indices,
        })
    }
}

impl Download {
    /// Create new Download instance
    /// - size: file size in bytes (used to calculate max range index)
    /// - num_conn: number of worker threads
    pub fn new(size: usize, num_conn: u8) -> Self {
        let max_index = Self::get_index(size >> 23).unwrap_or(0);

        let indices: Vec<Arc<Index>> = (0..num_conn).map(|_| Arc::new(Index::new())).collect();

        Download {
            coordinator: Coordinator::new(max_index, size),
            indices,
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

    /// Create Download for resume from a known byte offset
    /// Converts byte offset to unit index and creates appropriate range
    /// The Coordinator is initialized as "exhausted" so workers only use stealing
    pub fn from_offset(offset: usize, total_size: usize, num_conn: u8) -> Self {
        let max_index = Self::get_index(total_size >> 23).unwrap_or(0);

        // Initialize coordinator as if all standard ranges are depleted
        // current=max_index ensures range_byte is empty
        let coordinator = Coordinator::from_parts(
            max_index, // current
            max_index, // end
            2,         // steal_ptr
            false,     // steal_exhausted
            total_size,
        );

        let mut indices = Vec::with_capacity(num_conn as usize);

        // Populate with empty indices initially
        for _ in 0..num_conn {
            indices.push(Arc::new(Index::new()));
        }

        // Convert byte offset to unit index (8MB units)
        // Unit = bytes >> 23 (divide by 8MB)
        let start_unit = offset >> 23;
        let total_units = (total_size + (1 << 23) - 1) >> 23; // ceil division

        // Use the first worker for the remaining units
        if !indices.is_empty() && start_unit < total_units {
            indices[0].start.store(start_unit, Ordering::Relaxed);
            indices[0].end.store(total_units, Ordering::Relaxed);
            indices[0].state.store(0, Ordering::Relaxed);
        }

        Download {
            coordinator,
            indices,
        }
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

    /// Calculate total units still remaining to download
    /// Each unit is 8MB (1 << 23 bytes)
    pub fn units_remaining(&self) -> usize {
        self.indices.iter().map(|idx| idx.remaining_units()).sum()
    }

    /// Calculate total bytes still remaining to download (approximate)
    /// Note: This is an approximation since we track at unit granularity (8MB)
    pub fn bytes_remaining(&self) -> usize {
        self.units_remaining() << 23
    }

    /// Snapshot indices for serialization (atomic reads only)
    pub fn snapshot_indices(&self) -> Vec<(usize, usize, u8)> {
        self.indices
            .iter()
            .map(|idx| {
                (
                    idx.start.load(Ordering::Relaxed),
                    idx.end.load(Ordering::Relaxed),
                    idx.state.load(Ordering::Relaxed),
                )
            })
            .collect()
    }
}

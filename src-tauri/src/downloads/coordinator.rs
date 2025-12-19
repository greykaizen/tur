//! Coordinator for download range distribution and work stealing

use bincode::{Decode, Encode};
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::constants::RANGE;
use super::index::Index;

// Original design comments preserved:
// Coordinator distributes RANGE[0], RANGE[1], ... up to RANGE[max_index]
// Vec is naturally sorted since we push in Fibonacci order
// On save: retain incomplete ranges, adjust steal_ptr by counting removals

/// Coordinator manages range distribution and work stealing
/// Accessed via mpsc channel - single-threaded access, no atomics needed
#[derive(Encode, Decode, Clone)]
pub struct Coordinator {
    /// Current range index (start) and max index (end) based on file size
    pub range_byte: Range<u8>,
    /// Current steal target index, starts at 2
    pub steal_ptr: u8,
    /// Full circle completed, no more stealing possible
    pub steal_exhausted: bool,
    /// Total file size in bytes (for clamping ranges)
    pub total_size: usize,
}

impl Coordinator {
    pub fn new(max_index: u8, total_size: usize) -> Self {
        Coordinator {
            range_byte: 0..max_index,
            steal_ptr: 2, // Starts from index 2 as per arch
            steal_exhausted: false,
            total_size,
        }
    }

    /// Reconstruct coordinator from deserialized parts
    pub fn from_parts(
        current: u8,
        max_index: u8,
        steal_ptr: u8,
        steal_exhausted: bool,
        total_size: usize,
    ) -> Self {
        Coordinator {
            range_byte: current..max_index,
            steal_ptr,
            steal_exhausted,
            total_size,
        }
    }

    /// Request a new range from the coordinator
    /// Creates Index, pushes to Vec, returns the byte range
    /// Returns Some((Arc<Index>, Range)) if available, None if exhausted
    pub fn new_range(
        &mut self,
        range_vec: &mut Vec<Arc<Index>>,
    ) -> Option<(Arc<Index>, Range<usize>)> {
        if self.range_byte.start < self.range_byte.end {
            let idx = self.range_byte.start as usize;
            self.range_byte.start += 1;

            let byte_range = RANGE[idx].clone();
            // Convert from 8MB units to bytes, clamp to total_size
            let start_bytes = byte_range.start << 23; // * 8MB
            let end_bytes = (byte_range.end << 23).min(self.total_size);

            let index = Arc::new(Index {
                start: AtomicUsize::new(start_bytes),
                end: AtomicUsize::new(end_bytes),
            });

            range_vec.push(index.clone());
            Some((index, start_bytes..end_bytes))
        } else {
            None
        }
    }

    /// Request work: tries new range first, then steal
    /// Returns Some((Arc<Index>, Range)) or None if no work available
    pub fn request_work(
        &mut self,
        range_vec: &mut Vec<Arc<Index>>,
        min_steal_bytes: usize,
    ) -> Option<(Arc<Index>, Range<usize>)> {
        // 1. Try to get new range
        if let Some(result) = self.new_range(range_vec) {
            return Some(result);
        }

        // 2. Try to steal from existing workers
        self.steal_range(range_vec, min_steal_bytes)
    }

    /// Attempt to steal a range from a target worker's Index
    /// Uses 38.2% golden ratio (1 - PHI^-1), rounded high
    /// Starts from steal_ptr (index 2), wraps around
    /// Returns None if full circle completed (steal_exhausted set)
    pub fn steal_range(
        &mut self,
        indices: &mut Vec<Arc<Index>>,
        min_steal_bytes: usize,
    ) -> Option<(Arc<Index>, Range<usize>)> {
        if self.steal_exhausted || indices.len() < 3 {
            return None;
        }

        let num_indices = indices.len();
        let start_ptr = self.steal_ptr as usize;

        // Try each index once (full circle detection)
        for attempt in 0..num_indices {
            let target = (start_ptr + attempt) % num_indices;

            // Skip indices 0 and 1 as per architecture
            if target < 2 {
                continue;
            }

            let index = &indices[target];
            let current_start = index.start.load(Ordering::Relaxed);
            let current_end = index.end.load(Ordering::Relaxed);
            let remaining = current_end.saturating_sub(current_start);

            // Skip completed or too-small ranges
            if remaining <= min_steal_bytes {
                continue;
            }

            // Steal 38.2% (1 - PHI^-1) from the top, rounded high
            let steal_amount = ((remaining as f32) * 0.382).ceil() as usize;
            let new_end = current_end - steal_amount;

            // CAS to atomically shrink the victim's range
            if index
                .end
                .compare_exchange(current_end, new_end, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                // Create new Index for stolen portion
                let stolen_index = Arc::new(Index {
                    start: AtomicUsize::new(new_end),
                    end: AtomicUsize::new(current_end),
                });

                // Push stolen index to Vec
                indices.push(stolen_index.clone());

                // Update steal_ptr for next attempt
                self.steal_ptr = ((target + 1) % num_indices) as u8;

                return Some((stolen_index, new_end..current_end));
            }
        }

        // Full circle completed, no more stealing possible
        self.steal_exhausted = true;
        None
    }

    /// Reset steal_exhausted flag (call when a worker finishes, freeing opportunities)
    pub fn reset_steal(&mut self) {
        self.steal_exhausted = false;
    }

    /// Check if coordinator can provide more work (new ranges or stealing)
    pub fn has_work(&self) -> bool {
        self.range_byte.start < self.range_byte.end || !self.steal_exhausted
    }

    /// Prepare for save: retain incomplete indices, adjust steal_ptr
    /// Returns the value steal_ptr was pointing to (for re-finding after load)
    pub fn prepare_save(&self, indices: &[Arc<Index>]) -> Option<usize> {
        if (self.steal_ptr as usize) < indices.len() {
            let idx = &indices[self.steal_ptr as usize];
            Some(idx.start.load(Ordering::Relaxed))
        } else {
            None
        }
    }

    /// After loading and cleaning Vec, find new steal_ptr position
    pub fn restore_steal_ptr(&mut self, indices: &[Arc<Index>], saved_start: Option<usize>) {
        match saved_start {
            Some(start_val) => {
                // Find index with this start value
                for (i, idx) in indices.iter().enumerate() {
                    if idx.start.load(Ordering::Relaxed) == start_val {
                        self.steal_ptr = i as u8;
                        return;
                    }
                }
                // Not found, reset to 2
                self.steal_ptr = 2.min(indices.len().saturating_sub(1) as u8);
            }
            None => {
                self.steal_ptr = 2.min(indices.len().saturating_sub(1) as u8);
            }
        }
    }
}

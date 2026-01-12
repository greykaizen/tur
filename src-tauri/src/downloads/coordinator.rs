//! Coordinator for download range distribution and work stealing

use bincode::{Decode, Encode};
use std::ops::Range;
use std::sync::atomic::Ordering;
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

use super::work_request::WorkResponse;

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

    /// Handle work request from a worker
    /// Returns a WorkResponse to be sent back via channel
    /// Does NOT modify indices directly
    pub fn handle_request(&mut self, worker_id: usize, indices: &[Arc<Index>]) -> WorkResponse {
        // 1. Try to get new range
        if let Some((start, end)) = self.new_range() {
            return WorkResponse::Range(start, end);
        }

        // 2. Try to steal from existing workers
        if let Some((start, end, victim)) = self.try_steal(indices, worker_id) {
            return WorkResponse::Stolen(start, end, victim);
        }

        WorkResponse::None
    }

    /// Request a new range from the coordinator
    /// Returns (start_unit, end_unit) if available
    fn new_range(&mut self) -> Option<(usize, usize)> {
        if self.range_byte.start < self.range_byte.end {
            let idx = self.range_byte.start as usize;
            self.range_byte.start += 1;

            let unit_range = RANGE[idx].clone();

            // Calculate total units (ceil(total_size / 8MB))
            let total_units = (self.total_size + (1 << 23) - 1) >> 23;

            // Clamp end to total_units
            let start_unit = unit_range.start;
            let end_unit = unit_range.end.min(total_units);

            Some((start_unit, end_unit))
        } else {
            None
        }
    }

    /// Attempt to steal a range directly using CAS
    /// Returns (new_start, new_end, victim_id) for the requester
    fn try_steal(
        &mut self,
        indices: &[Arc<Index>],
        requester: usize,
    ) -> Option<(usize, usize, usize)> {
        if self.steal_exhausted {
            return None;
        }

        let num_workers = indices.len();
        if num_workers == 0 {
            return None;
        }

        let start_ptr = self.steal_ptr as usize;

        // Try each index once (full circle detection)
        for attempt in 0..num_workers {
            let victim = (start_ptr + attempt) % num_workers;

            // Don't steal from self
            if victim == requester {
                continue;
            }

            let victim_idx = &indices[victim];
            let current_start = victim_idx.start.load(Ordering::Relaxed);
            let current_end = victim_idx.end.load(Ordering::Relaxed);
            let remaining = current_end.saturating_sub(current_start);

            // Need at least 2 units to steal
            if remaining < 2 {
                continue;
            }

            // Steal 38.2% (golden ratio)
            let steal_amount = ((remaining as f32) * 0.382).ceil() as usize;
            if steal_amount == 0 {
                continue;
            }

            let new_end = current_end - steal_amount;

            // CAS to atomically shrink the victim's range
            // We use SeqCst for success to ensure total order of steals, Relaxed for failure
            if victim_idx
                .end
                .compare_exchange(current_end, new_end, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                // Update steal_ptr for next attempt
                self.steal_ptr = ((victim + 1) % num_workers) as u8;

                // Return the stolen range (from new_end to old_end)
                // The requester will take ownership of [new_end, current_end)
                return Some((new_end, current_end, victim));
            }
        }

        // Full circle completed, no more stealing possible
        self.steal_exhausted = true;
        None
    }

    /// Reset steal_exhausted flag
    pub fn reset_steal(&mut self) {
        self.steal_exhausted = false;
    }

    /// Check if work is potentially available
    pub fn has_work(&self) -> bool {
        self.range_byte.start < self.range_byte.end || !self.steal_exhausted
    }
}

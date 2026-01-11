use super::index::Index;
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

pub struct WorkerContext {
    pub worker_id: usize,
    pub file: File,
    pub state: Arc<AtomicU8>,
    pub index: Arc<Index>,
    pub bytes_in_unit: AtomicUsize,
    pub speed_bps: AtomicUsize,
    pub is_stealing: AtomicBool,
}

impl WorkerContext {
    pub fn new(worker_id: usize, file: File, state: Arc<AtomicU8>, index: Arc<Index>) -> Self {
        Self {
            worker_id,
            file,
            state,
            index,
            bytes_in_unit: AtomicUsize::new(0),
            speed_bps: AtomicUsize::new(0),
            is_stealing: AtomicBool::new(false),
        }
    }

    pub fn flip_bit(&self, bit: u8) {
        if bit < 8 {
            self.state.fetch_or(1 << bit, Ordering::Relaxed);
        }
    }

    pub fn is_unit_complete(&self) -> bool {
        self.state.load(Ordering::Relaxed) == 0xFF
    }

    pub fn reset_unit(&self) {
        self.state.store(0, Ordering::Relaxed);
        self.index.start.fetch_add(1, Ordering::Relaxed);
        self.bytes_in_unit.store(0, Ordering::Relaxed);
    }

    pub fn current_byte_offset(&self) -> usize {
        let unit = self.index.start.load(Ordering::Relaxed);
        (unit << 23) + self.bytes_in_unit.load(Ordering::Relaxed)
    }
}

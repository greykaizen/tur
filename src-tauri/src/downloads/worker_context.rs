use super::index::Index;
use std::fs::File;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct WorkerContext {
    pub worker_id: usize,
    pub file: File,
    // state is now inside Index
    pub index: Arc<Index>,
    pub bytes_in_unit: AtomicUsize,
    pub speed_bps: AtomicUsize,
    pub stealing_from: AtomicUsize,
}

impl WorkerContext {
    pub fn new(
        worker_id: usize,
        file: File,
        index: Arc<Index>,
        is_stealing: Option<usize>,
    ) -> Self {
        Self {
            worker_id,
            file,
            index,
            bytes_in_unit: AtomicUsize::new(0),
            speed_bps: AtomicUsize::new(0),
            stealing_from: AtomicUsize::new(is_stealing.unwrap_or(usize::MAX)),
        }
    }

    pub fn flip_bit(&self, bit: u8) {
        self.index.flip_bit(bit);
    }

    pub fn is_unit_complete(&self) -> bool {
        self.index.is_unit_complete()
    }

    pub fn reset_unit(&self) {
        self.index.advance_unit();
        self.bytes_in_unit.store(0, Ordering::Relaxed);
    }

    pub fn current_byte_offset(&self) -> usize {
        let unit = self.index.start.load(Ordering::Relaxed);
        (unit << 23) + self.bytes_in_unit.load(Ordering::Relaxed)
    }
}

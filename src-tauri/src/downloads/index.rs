//! Index struct for tracking download range progress

use bincode::{error::DecodeError, error::EncodeError, Decode, Encode};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// Represents a byte range being downloaded (unit based)
/// Uses AtomicUsize for thread-safe progress tracking and work stealing
#[derive(Debug)]
pub struct Index {
    pub start: AtomicUsize,
    pub end: AtomicUsize,
    /// 8 bits = 8MB progress within current unit
    pub state: AtomicU8,
}

impl Encode for Index {
    fn encode<E: bincode::enc::Encoder>(&self, e: &mut E) -> Result<(), EncodeError> {
        self.start.load(Ordering::Relaxed).encode(e)?;
        self.end.load(Ordering::Relaxed).encode(e)?;
        self.state.load(Ordering::Relaxed).encode(e)
    }
}

impl<Context> Decode<Context> for Index {
    fn decode<D: bincode::de::Decoder<Context = Context>>(d: &mut D) -> Result<Self, DecodeError> {
        Ok(Index {
            start: AtomicUsize::new(usize::decode(d)?),
            end: AtomicUsize::new(usize::decode(d)?),
            state: AtomicU8::new(u8::decode(d)?),
        })
    }
}

impl Index {
    pub fn new() -> Self {
        Index {
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
            state: AtomicU8::new(0),
        }
    }

    pub fn unit_to_bytes(unit: usize) -> usize {
        unit << 23
    }

    pub fn bytes_to_unit(bytes: usize) -> usize {
        bytes >> 23
    }

    pub fn remaining_units(&self) -> usize {
        let start = self.start.load(Ordering::Relaxed);
        let end = self.end.load(Ordering::Relaxed);
        end.saturating_sub(start)
    }

    /// Set range (called by worker after receiving work)
    pub fn set_range(&self, start: usize, end: usize) {
        self.start.store(start, Ordering::Relaxed);
        self.end.store(end, Ordering::Relaxed);
        self.state.store(0, Ordering::Relaxed);
    }

    /// Flip bit for 1MB completion
    pub fn flip_bit(&self, bit: u8) {
        self.state.fetch_or(1 << bit, Ordering::Relaxed);
    }

    /// Check if unit complete (all 8 bits set)
    pub fn is_unit_complete(&self) -> bool {
        self.state.load(Ordering::Relaxed) == 0xFF
    }

    /// Reset state and advance to next unit
    pub fn advance_unit(&self) {
        self.state.store(0, Ordering::Relaxed);
        self.start.fetch_add(1, Ordering::Relaxed);
    }

    /// Check if range exhausted
    pub fn is_exhausted(&self) -> bool {
        self.start.load(Ordering::Relaxed) >= self.end.load(Ordering::Relaxed)
    }
}

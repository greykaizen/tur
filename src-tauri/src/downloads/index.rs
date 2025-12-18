//! Index struct for tracking download range progress

use bincode::{error::DecodeError, error::EncodeError, Decode, Encode};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Represents a byte range being downloaded
/// Uses AtomicUsize for thread-safe progress tracking and work stealing
pub struct Index {
    pub start: AtomicUsize,
    pub end: AtomicUsize,
}

impl Encode for Index {
    fn encode<E: bincode::enc::Encoder>(&self, e: &mut E) -> Result<(), EncodeError> {
        self.start.load(Ordering::Relaxed).encode(e)?;
        self.end.load(Ordering::Relaxed).encode(e)
    }
}

impl<Context> Decode<Context> for Index {
    fn decode<D: bincode::de::Decoder<Context = Context>>(d: &mut D) -> Result<Self, DecodeError> {
        Ok(Index {
            start: AtomicUsize::new(usize::decode(d)?),
            end: AtomicUsize::new(usize::decode(d)?),
        })
    }
}

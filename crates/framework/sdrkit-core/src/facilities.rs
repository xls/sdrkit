//! What the host supplies once and every receiver, service and driver uses: files, the wall clock, randomness and the
//! audio codec plug-ins.

use crate::codec::CodecSet;
use crate::random::Random;
use crate::storage::{MemStorage, Storage};
use crate::time::Clock;
use std::sync::Arc;

/// The host's facilities, passed to a receiver with [`Receiver::set_facilities`](crate::Receiver::set_facilities).
#[derive(Clone)]
pub struct Facilities {
    /// Files and folders.
    pub storage: Arc<dyn Storage>,
    /// The wall clock.
    pub clock: Clock,
    /// The random source.
    pub random: Random,
    /// The audio codec plug-ins the host registered.
    pub codecs: Arc<CodecSet>,
}

impl std::fmt::Debug for Facilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Facilities")
            .field("clock", &self.clock)
            .finish_non_exhaustive()
    }
}

impl Default for Facilities {
    /// For tests and a receiver that runs without a host: empty storage in memory, a clock fixed at 0, the random source
    /// of seed 0 and no codecs.
    fn default() -> Self {
        Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(0.0),
            random: Random::fixed(0),
            codecs: Arc::new(CodecSet::default()),
        }
    }
}

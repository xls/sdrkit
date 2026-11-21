//! The random source that the host provides.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The host's random source. A library crate reads random bytes only through this type.
#[derive(Clone)]
pub struct Random(Arc<FillFn>);

type FillFn = dyn Fn(&mut [u8]) + Send + Sync;

impl Random {
    /// A source that calls `fill` to fill a buffer with random bytes.
    pub fn new(fill: impl Fn(&mut [u8]) + Send + Sync + 'static) -> Self {
        Random(Arc::new(fill))
    }

    /// A source that produces the same sequence of bytes for the same `seed`. Clones share the sequence.
    pub fn fixed(seed: u64) -> Self {
        let state = AtomicU64::new(seed);
        Random::new(move |out| {
            for chunk in out.chunks_mut(8) {
                let x = splitmix64(state.fetch_add(GOLDEN, Ordering::Relaxed));
                chunk.copy_from_slice(&x.to_le_bytes()[..chunk.len()]);
            }
        })
    }

    /// Fills `out` with random bytes.
    pub fn fill(&self, out: &mut [u8]) {
        (self.0)(out);
    }

    /// A random 64-bit number.
    pub fn u64(&self) -> u64 {
        let mut b = [0u8; 8];
        self.fill(&mut b);
        u64::from_le_bytes(b)
    }
}

impl fmt::Debug for Random {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Random")
    }
}

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// The output step of SplitMix64 for the state after one increment.
fn splitmix64(state: u64) -> u64 {
    let mut z = state.wrapping_add(GOLDEN);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_source_repeats_for_one_seed() {
        let a = Random::fixed(7);
        let b = Random::fixed(7);
        let first = a.u64();
        assert_eq!(first, b.u64());
        assert_ne!(first, a.u64());
        assert_ne!(Random::fixed(8).u64(), first);
        let mut odd = [0u8; 11];
        Random::fixed(7).fill(&mut odd);
        assert_eq!(odd[..8], first.to_le_bytes());
        let host = Random::new(|out| out.fill(0xAB));
        assert_eq!(host.u64(), 0xABAB_ABAB_ABAB_ABAB);
    }
}

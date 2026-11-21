//! The decoders of a signalling burst on each carrier of the multi-carrier receiver: one decoder per channel key, fed the
//! flat voice band of the blocks that pass the squelch gate, reset across a closed gate, and dropped when the carrier has
//! gone.

use crate::channels::{VoiceBlock, AUDIO_RATE, BLOCK};
use std::collections::BTreeMap;

/// A burst decoder that a gap in the audio interrupts.
pub trait BlockDecoder {
    /// Forgets the burst in progress.
    fn reset(&mut self);
}

struct Entry<D> {
    dec: D,
    /// The time of the last block it took, s of the input.
    last: f64,
}

/// One decoder `D` for each channel key.
pub struct CarrierDecoders<D> {
    map: BTreeMap<i64, Entry<D>>,
}

impl<D> Default for CarrierDecoders<D> {
    fn default() -> Self {
        CarrierDecoders {
            map: BTreeMap::new(),
        }
    }
}

impl<D: BlockDecoder> CarrierDecoders<D> {
    /// Forgets every decoder, as when the block times start again with the input.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Runs `run` on each block of `blocks` with open audio, with the decoder of its channel. `make` makes the decoder of
    /// a new channel; a channel it gives `None` for is skipped. A block given a second time is skipped, and a decoder is
    /// reset when the gate was closed since its last block. A channel that gave no block for 10 s is dropped.
    pub fn feed(
        &mut self,
        blocks: &[VoiceBlock],
        mut make: impl FnMut() -> Option<D>,
        mut run: impl FnMut(&mut D, &VoiceBlock),
    ) {
        let block_s = BLOCK as f64 / AUDIO_RATE;
        for b in blocks {
            if b.flat.is_empty() {
                continue;
            }
            let e = match self.map.entry(b.key) {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => {
                    let Some(dec) = make() else {
                        continue;
                    };
                    e.insert(Entry {
                        dec,
                        last: b.time - block_s,
                    })
                }
            };
            // A receiver that starts again on a channel gives the stored span a second time.
            if b.time <= e.last + 0.5 * block_s {
                continue;
            }
            if b.time > e.last + 1.5 * block_s {
                e.dec.reset();
            }
            e.last = b.time;
            run(&mut e.dec, b);
        }
        let newest = self.map.values().map(|e| e.last).fold(0.0, f64::max);
        self.map.retain(|_, e| newest - e.last < 10.0);
    }
}

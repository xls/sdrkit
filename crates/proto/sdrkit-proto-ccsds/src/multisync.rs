//! Frame synchronisation on links whose bits arrive with an ambiguity that only the sync word settles: several
//! [`WordSync`] synchronisers run side by side, one for each reading of the symbols, and the first one that holds a lock is
//! used alone.
//!
//! - [`SplitPhaseSync`]: split-phase chips. A bit is two chips, high then low for a 1. Which chip starts a bit is not known,
//!   so both pairings are read. The pairing whose synchroniser has a confirmed lock first is used until that lock ends.
//! - [`QuadSync`]: uncoded QPSK. The two bits of a symbol (I then Q) are one of four streams up to a complement: (I, Q),
//!   (not Q, I), (Q, I), (not I, Q). Each stream has its own synchroniser, which also accepts the complement. A stream is
//!   chosen when its synchroniser has a confirmed lock and has found 3 sync words since the last search began; the others
//!   are then reset. Until then every synchroniser hands on its frames.
//!
//! Both take soft values as LLRs (positive means a 0 bit, or a low chip).
//!
//! ```
//! use sdrkit_proto_ccsds::multisync::QuadSync;
//! use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
//! let cfg = WordSyncConfig { word: 0x1ACF_FC1D, word_bits: 32, frame_bits: 64, acquire_errors: 0, track_errors: 2, complement: true, max_misses: 3 };
//! let mut q = QuadSync::new(cfg).unwrap();
//! let frame = vec![0x1Au8, 0xCF, 0xFC, 0x1D, 1, 2, 3, 4];
//! let bits: Vec<i8> = frame.iter().cycle().take(8 * 8).flat_map(|b| (0..8).rev().map(move |k| if b >> k & 1 == 1 { -64 } else { 64 })).collect();
//! let mut n = 0;
//! q.push_llrs(&bits, |stream, f| {
//!     assert_eq!((stream, f.bytes), (0, &frame[..]));
//!     n += 1;
//! });
//! assert_eq!(q.chosen(), Some(0));
//! assert_eq!(n, 8);
//! ```

use crate::wordsync::{WordFrame, WordSync, WordSyncConfig};
use crate::Result;

/// A change of the stream in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    /// This stream is used from now on.
    Chosen(usize),
    /// The lock of the stream in use ended: every stream searches again.
    Lost,
}

/// Split-phase chips to frames. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct SplitPhaseSync {
    prev: Option<i8>,
    chips: u64,
    bits: [Vec<u8>; 2],
    sync: [WordSync; 2],
    chosen: Option<usize>,
    events: Vec<StreamEvent>,
}

impl SplitPhaseSync {
    /// Two synchronisers with the rules `cfg`, which count bits.
    ///
    /// # Errors
    ///
    /// The error of [`WordSync::new`].
    pub fn new(cfg: WordSyncConfig) -> Result<Self> {
        let s = WordSync::new(cfg)?;
        Ok(SplitPhaseSync {
            prev: None,
            chips: 0,
            bits: [Vec::new(), Vec::new()],
            sync: [s.clone(), s],
            chosen: None,
            events: Vec::new(),
        })
    }

    /// Ends every lock and forgets the last chip.
    pub fn drop_lock(&mut self) {
        self.prev = None;
        self.chosen = None;
        for s in &mut self.sync {
            s.drop_lock();
        }
    }

    /// The pairing in use: 0 or 1.
    pub fn chosen(&self) -> Option<usize> {
        self.chosen
    }

    /// The synchroniser of pairing `w`.
    pub fn sync(&self, w: usize) -> &WordSync {
        &self.sync[w.min(1)]
    }

    /// The stream in use is inverted.
    pub fn inverted(&self) -> bool {
        self.chosen.is_some_and(|c| self.sync[c].inverted())
    }

    /// Sync words found by both synchronisers.
    pub fn sync_hits(&self) -> u64 {
        self.sync.iter().map(|s| s.stats().sync_hits).sum()
    }

    /// The changes since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<StreamEvent> {
        std::mem::take(&mut self.events)
    }

    /// Takes soft chips and calls `frame` with the pairing and each frame.
    pub fn push_llrs(&mut self, chips: &[i8], mut frame: impl FnMut(usize, WordFrame<'_>)) {
        self.bits[0].clear();
        self.bits[1].clear();
        for &c in chips {
            if let Some(p) = self.prev {
                // A positive LLR is a low chip: high then low is a 1.
                let bit = u8::from(i16::from(p) < i16::from(c));
                self.bits[usize::from(self.chips.is_multiple_of(2))].push(bit);
            }
            self.prev = Some(c);
            self.chips += 1;
        }
        for w in 0..2 {
            if self.chosen.is_some_and(|c| c != w) {
                continue;
            }
            self.sync[w].push_bits(&self.bits[w], |f| frame(w, f));
            let locked = self.sync[w].locked();
            if self.chosen.is_none() && locked {
                self.chosen = Some(w);
                self.events.push(StreamEvent::Chosen(w));
            } else if self.chosen == Some(w) && !locked {
                self.chosen = None;
                self.events.push(StreamEvent::Lost);
            }
            if self.chosen == Some(w) {
                // The other pairing starts again when this lock ends.
                self.sync[1 - w].drop_lock();
                break;
            }
        }
    }
}

/// The four bit streams of uncoded QPSK to frames. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct QuadSync {
    pending: Option<u8>,
    bits: [Vec<u8>; 4],
    sync: [WordSync; 4],
    base_hits: [u64; 4],
    chosen: Option<usize>,
    events: Vec<StreamEvent>,
}

/// Sync words a stream must have found, its lock confirmed, before it is chosen.
const CHOOSE_AFTER: u64 = 3;

impl QuadSync {
    /// Four synchronisers with the rules `cfg`, which count bits.
    ///
    /// # Errors
    ///
    /// The error of [`WordSync::new`].
    pub fn new(cfg: WordSyncConfig) -> Result<Self> {
        let s = WordSync::new(cfg)?;
        Ok(QuadSync {
            pending: None,
            bits: Default::default(),
            sync: [s.clone(), s.clone(), s.clone(), s],
            base_hits: [0; 4],
            chosen: None,
            events: Vec::new(),
        })
    }

    /// Ends every lock and forgets a half symbol.
    pub fn drop_lock(&mut self) {
        self.pending = None;
        self.reset_all();
    }

    fn reset_all(&mut self) {
        self.chosen = None;
        for (s, b) in self.sync.iter_mut().zip(&mut self.base_hits) {
            s.drop_lock();
            *b = s.stats().sync_hits;
        }
    }

    /// The stream in use: 0 (I, Q), 1 (not Q, I), 2 (Q, I) or 3 (not I, Q).
    pub fn chosen(&self) -> Option<usize> {
        self.chosen
    }

    /// Some synchroniser holds a confirmed lock.
    pub fn locked(&self) -> bool {
        self.sync.iter().any(WordSync::locked)
    }

    /// The stream in use is inverted.
    pub fn inverted(&self) -> bool {
        self.chosen.is_some_and(|c| self.sync[c].inverted())
    }

    /// Sync words found by all four synchronisers.
    pub fn sync_hits(&self) -> u64 {
        self.sync.iter().map(|s| s.stats().sync_hits).sum()
    }

    /// The changes since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<StreamEvent> {
        std::mem::take(&mut self.events)
    }

    /// Takes soft values, I then Q of each symbol, and calls `frame` with the stream and each frame.
    pub fn push_llrs(&mut self, llrs: &[i8], mut frame: impl FnMut(usize, WordFrame<'_>)) {
        for b in &mut self.bits {
            b.clear();
        }
        for &l in llrs {
            let bit = u8::from(l < 0);
            let Some(i) = self.pending.take() else {
                self.pending = Some(bit);
                continue;
            };
            let q = bit;
            for (s, pair) in self
                .bits
                .iter_mut()
                .zip([[i, q], [q ^ 1, i], [q, i], [i ^ 1, q]])
            {
                s.extend_from_slice(&pair);
            }
        }
        for w in 0..4 {
            if self.chosen.is_some_and(|c| c != w) {
                continue;
            }
            self.sync[w].push_bits(&self.bits[w], |f| frame(w, f));
            let s = &self.sync[w];
            if self.chosen.is_none()
                && s.locked()
                && s.stats().sync_hits - self.base_hits[w] >= CHOOSE_AFTER
            {
                self.chosen = Some(w);
                self.events.push(StreamEvent::Chosen(w));
                for o in (0..4).filter(|&o| o != w) {
                    self.sync[o].drop_lock();
                    self.base_hits[o] = self.sync[o].stats().sync_hits;
                }
            } else if self.chosen == Some(w) && !s.locked() {
                self.events.push(StreamEvent::Lost);
                self.reset_all();
            }
        }
    }
}

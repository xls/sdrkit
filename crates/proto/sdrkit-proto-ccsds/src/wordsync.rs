//! Frame synchronisation on any fixed sync word, for links whose frames are not CCSDS channel frames: a sync word of 8 to 64
//! bits, a frame length in bits that need not be a whole number of bytes, and a choice whether the complement of the word is
//! accepted. [`WordSync`] takes hard bits and passes each frame to a callback.
//!
//! A new lock is confirmed by the sync word of the next frame. The first frame is held until then and dropped when the
//! confirmation fails, so random bits do not give frames on a link that has no check behind its sync word.
//!
//! ```
//! use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig};
//! // Three frames of 20 bits behind the word 0xB5: one byte and four more bits each.
//! let cfg = WordSyncConfig { word: 0xB5, word_bits: 8, frame_bits: 20, acquire_errors: 0, track_errors: 1, complement: false, max_misses: 3 };
//! let mut sync = WordSync::new(cfg).unwrap();
//! let frame = [1u8, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1];
//! let bits: Vec<u8> = frame.iter().cycle().take(60).copied().collect();
//! let mut n = 0;
//! sync.push_bits(&bits, |f| {
//!     assert_eq!(f.bytes, &[0xB5, 0x30, 0x90]);
//!     n += 1;
//! });
//! assert_eq!(n, 3); // The first frame was held until the second sync word confirmed the lock.
//! ```

use crate::{Error, Param, Result};

/// The rules of a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordSyncConfig {
    /// The sync word, first bit most significant, in the low `word_bits` bits.
    pub word: u64,
    /// Bits of the sync word, 8 to 64.
    pub word_bits: u32,
    /// Bits of a frame, the sync word included.
    pub frame_bits: usize,
    /// Wrong bits accepted when the search finds a sync word, and when the next one confirms it.
    pub acquire_errors: u32,
    /// Wrong bits accepted for the sync word of a confirmed lock.
    pub track_errors: u32,
    /// The complement of the sync word is accepted: the bits after it are then inverted.
    pub complement: bool,
    /// The lock ends after more than this many missed sync words in a row.
    pub max_misses: u32,
}

/// A frame found by [`WordSync`]. It borrows the synchroniser's buffer until the callback returns.
#[derive(Debug)]
pub struct WordFrame<'a> {
    /// The frame from its sync word on, most significant bit first, with zero bits up to a whole byte. The sync word is the
    /// nominal one. The other bits have their polarity corrected.
    pub bytes: &'a [u8],
    /// The frame arrived inverted.
    pub inverted: bool,
    /// Wrong bits in the sync word as received.
    pub word_errors: u32,
    /// The sync word did not match. The frame was taken at the expected place.
    pub missed: bool,
}

/// Counters of [`WordSync`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WordSyncStats {
    /// Sync words found: by the search, by a confirmation or while tracking.
    pub sync_hits: u64,
    /// Frames given out.
    pub frames: u64,
    /// Locks that the next sync word confirmed.
    pub confirmed: u64,
    /// Locks dropped because the next sync word did not confirm them.
    pub unconfirmed: u64,
    /// Sync words missed while locked.
    pub misses: u64,
    /// Confirmed locks lost.
    pub losses: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Search,
    /// The first frame of a new lock, then the sync word after it.
    Confirm,
    Track,
}

/// The streaming synchroniser. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct WordSync {
    cfg: WordSyncConfig,
    /// The search: the newest bits, the newest in bit 0, and how many of them count (up to the length of the word).
    reg: u64,
    seen: u32,
    /// The bits of the word.
    mask: u64,
    state: State,
    inverted: bool,
    /// The bits of the frame in progress, as received, from its sync word on.
    bits: Vec<u8>,
    /// The first frame of a lock that waits for its confirmation.
    held: Vec<u8>,
    /// The word read for a confirmation that failed, while it is searched again.
    tail: Vec<u8>,
    held_errors: u32,
    word_errors: u32,
    missed: bool,
    misses: u32,
    out: Vec<u8>,
    stats: WordSyncStats,
}

impl WordSync {
    /// A synchroniser with these rules.
    ///
    /// # Errors
    ///
    /// [`Error::BadParameter`] if the word has fewer than 8 or more than 64 bits, the frame is not longer than the word or is
    /// longer than 2^23 bits, or an error limit is half the word or more.
    pub fn new(cfg: WordSyncConfig) -> Result<Self> {
        if !(8..=64).contains(&cfg.word_bits)
            || cfg.frame_bits <= cfg.word_bits as usize
            || cfg.frame_bits > 1 << 23
        {
            return Err(Error::BadParameter(Param::FrameBytes));
        }
        if 2 * cfg.acquire_errors >= cfg.word_bits || 2 * cfg.track_errors >= cfg.word_bits {
            return Err(Error::BadParameter(Param::MarkerErrors));
        }
        Ok(WordSync {
            cfg,
            reg: 0,
            seen: 0,
            mask: u64::MAX >> (64 - cfg.word_bits),
            state: State::Search,
            inverted: false,
            bits: Vec::with_capacity(cfg.frame_bits),
            held: Vec::with_capacity(cfg.frame_bits),
            tail: Vec::with_capacity(cfg.word_bits as usize),
            held_errors: 0,
            word_errors: 0,
            missed: false,
            misses: 0,
            out: vec![0; cfg.frame_bits.div_ceil(8)],
            stats: WordSyncStats::default(),
        })
    }

    /// The rules.
    pub fn config(&self) -> &WordSyncConfig {
        &self.cfg
    }

    /// The counters.
    pub fn stats(&self) -> &WordSyncStats {
        &self.stats
    }

    /// A confirmed lock is held.
    pub fn locked(&self) -> bool {
        self.state == State::Track
    }

    /// The stream of the present lock is inverted.
    pub fn inverted(&self) -> bool {
        self.inverted
    }

    /// Ends the lock and searches again.
    pub fn drop_lock(&mut self) {
        if self.state == State::Track {
            self.stats.losses += 1;
        }
        self.search_again();
    }

    fn search_again(&mut self) {
        self.state = State::Search;
        self.reg = 0;
        self.seen = 0;
        self.bits.clear();
        self.held.clear();
        self.misses = 0;
    }

    /// Wrong bits of the first word of `bits` against the sync word in the upright polarity.
    fn distance(&self, bits: &[u8]) -> u32 {
        let n = self.cfg.word_bits as usize;
        let got = bits[..n]
            .iter()
            .fold(0u64, |v, &b| (v << 1) | u64::from(b & 1));
        ((got ^ self.cfg.word) & self.mask).count_ones()
    }

    /// Packs `bits` (a whole frame as received, each 0 or 1) into the output buffer and calls `frame`.
    fn give(
        cfg: &WordSyncConfig,
        out: &mut [u8],
        bits: &[u8],
        inverted: bool,
        word_errors: u32,
        missed: bool,
        frame: &mut impl FnMut(WordFrame<'_>),
    ) {
        // Eight bits to a byte with one product: bit `i` of the eight goes to bit `7 - i` of the top byte.
        const GATHER: u64 = 0x8040_2010_0804_0201;
        let flip = if inverted { 0xFF } else { 0 };
        let (whole, rest) = bits.as_chunks::<8>();
        for (o, c) in out.iter_mut().zip(whole) {
            *o = (u64::from_le_bytes(*c).wrapping_mul(GATHER) >> 56) as u8 ^ flip;
        }
        if let Some(o) = out.get_mut(whole.len()) {
            *o = 0;
            for (i, &b) in rest.iter().enumerate() {
                *o |= (b ^ (flip & 1)) << (7 - i);
            }
        }
        // The sync word as its nominal bits.
        let n = cfg.word_bits as usize;
        for i in 0..n {
            let b = ((cfg.word >> (n - 1 - i)) & 1) as u8;
            out[i / 8] = (out[i / 8] & !(0x80 >> (i % 8))) | (b << (7 - i % 8));
        }
        frame(WordFrame {
            bytes: out,
            inverted,
            word_errors,
            missed,
        });
    }

    /// Takes hard bits (bit 0 of each value) and calls `frame` for each frame.
    pub fn push_bits(&mut self, bits: &[u8], mut frame: impl FnMut(WordFrame<'_>)) {
        self.take(bits, &mut frame);
    }

    /// The bits of [`push_bits`](Self::push_bits): the search bit by bit, a frame in progress in pieces up to its next
    /// sync word or its end.
    fn take(&mut self, mut bits: &[u8], frame: &mut impl FnMut(WordFrame<'_>)) {
        let word = self.cfg.word_bits as usize;
        let total = self.cfg.frame_bits;
        while !bits.is_empty() {
            if self.state == State::Search {
                bits = self.search(bits);
                continue;
            }
            // The frame in progress up to the end of its sync word, then up to its end.
            let end = if self.bits.len() < word { word } else { total };
            let (now, later) = bits.split_at((end - self.bits.len()).min(bits.len()));
            self.bits.extend(now.iter().map(|&b| b & 1));
            bits = later;
            if self.bits.len() < end {
                break;
            }
            match self.state {
                State::Confirm if end == total => {
                    std::mem::swap(&mut self.held, &mut self.bits);
                    self.bits.clear();
                }
                State::Confirm => self.confirm(frame),
                State::Track if end == word => self.track_word(),
                _ => {
                    self.stats.frames += 1;
                    Self::give(
                        &self.cfg,
                        &mut self.out,
                        &self.bits,
                        self.inverted,
                        self.word_errors,
                        self.missed,
                        frame,
                    );
                    self.bits.clear();
                }
            }
        }
    }

    /// Searches `bits` for the sync word, in the complement too when the rules accept it; a word is found when as many
    /// bits as it has were seen since the search began. Returns the bits after the word, or none.
    fn search<'a>(&mut self, bits: &'a [u8]) -> &'a [u8] {
        let (word, max) = (self.cfg.word_bits, self.cfg.acquire_errors);
        let (mut reg, mut seen) = (self.reg, self.seen);
        let mut found = None;
        for (i, &b) in bits.iter().enumerate() {
            reg = (reg << 1) | u64::from(b & 1);
            if seen < word {
                seen += 1;
                if seen < word {
                    continue;
                }
            }
            let d = ((reg ^ self.cfg.word) & self.mask).count_ones();
            if d <= max {
                found = Some((i, d, false));
                break;
            }
            if self.cfg.complement && word - d <= max {
                found = Some((i, word - d, true));
                break;
            }
        }
        (self.reg, self.seen) = (reg, seen);
        let Some((i, errors, inverted)) = found else {
            return &[];
        };
        self.stats.sync_hits += 1;
        self.inverted = inverted;
        self.held_errors = errors;
        // The word is written as its nominal bits when the frame is given out.
        self.bits.clear();
        self.bits.resize(word as usize, 0);
        self.state = State::Confirm;
        &bits[i + 1..]
    }

    /// The sync word after the first frame of a lock: it confirms the lock and the frame is given out, or the frame is
    /// dropped and the word is searched again.
    fn confirm(&mut self, frame: &mut impl FnMut(WordFrame<'_>)) {
        let d = self.distance(&self.bits);
        let d = if self.inverted {
            self.cfg.word_bits - d
        } else {
            d
        };
        if d <= self.cfg.acquire_errors {
            self.stats.sync_hits += 1;
            self.stats.confirmed += 1;
            self.stats.frames += 1;
            Self::give(
                &self.cfg,
                &mut self.out,
                &self.held,
                self.inverted,
                self.held_errors,
                false,
                frame,
            );
            self.held.clear();
            self.word_errors = d;
            self.missed = false;
            self.misses = 0;
            self.state = State::Track;
        } else {
            self.stats.unconfirmed += 1;
            // A true word may end inside the bits just read: search them again.
            let mut tail = std::mem::take(&mut self.tail);
            std::mem::swap(&mut tail, &mut self.bits);
            self.search_again();
            self.take(&tail, frame);
            tail.clear();
            self.tail = tail;
        }
    }

    /// The sync word of a frame of a confirmed lock.
    fn track_word(&mut self) {
        let d = self.distance(&self.bits);
        let di = self.cfg.word_bits - d;
        let (best, inv) = if di < d { (di, true) } else { (d, false) };
        if best <= self.cfg.track_errors && (self.cfg.complement || !inv) {
            // The polarity follows the better match: a half-turn slip of the carrier is followed.
            self.inverted = inv;
            self.word_errors = best;
            self.missed = false;
            self.misses = 0;
            self.stats.sync_hits += 1;
        } else {
            self.word_errors = if self.inverted { di } else { d };
            self.missed = true;
            self.misses += 1;
            self.stats.misses += 1;
            if self.misses > self.cfg.max_misses {
                self.stats.losses += 1;
                self.search_again();
            }
        }
    }
}

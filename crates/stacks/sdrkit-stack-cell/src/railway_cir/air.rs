//! The air frame of the railway CIR data channel: the 32-bit sync `0x0DD4259F`, then 26-bit words of two frame bytes and 10
//! check bits of the shortened cyclic (26,16) code of `x^10 + x^8 + x^7 + x^5 + x^4 + x^3 + 1`. The (26,16) code cannot
//! correct every double error, so the [`Framer`] keeps every reading of each word with up to two errors and lets the frame
//! checks choose.

use super::frame::{self, LEN_RANGE};
use crate::lbj::message::Locomotives;
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_algo_fec::syndromelist::SyndromeList;
use std::sync::{Arc, OnceLock};

/// The sync word, first bit sent in the most significant place.
pub const SYNC: u32 = 0x0DD4_259F;
/// The generator of the word code.
pub const GENERATOR: u64 = 0x5B9;
/// Sync bits that may differ.
pub const SYNC_ERRORS: u32 = 2;
/// Readings kept at most while a frame is assembled.
pub const MAX_PENDING: usize = 64;
/// The largest frame stored, bytes.
pub const MAX_BYTES: usize = 258;

/// The word code and its readings of up to two errors.
pub struct WordCode {
    /// The code.
    pub code: SmallCode,
    /// The readings.
    pub list: SyndromeList,
}

/// The shared word code.
pub fn word_code() -> Arc<WordCode> {
    static CODE: OnceLock<Arc<WordCode>> = OnceLock::new();
    CODE.get_or_init(|| {
        let code = SmallCode::cyclic(26, GENERATOR).expect("a (26,16) code");
        let list = SyndromeList::new(&code, 2);
        Arc::new(WordCode { code, list })
    })
    .clone()
}

/// The air bits of `frame` (0 or 1, first sent first): the sync and the words; an odd frame gets a pad byte 0.
pub fn encode(frame: &[u8]) -> Vec<u8> {
    let c = word_code();
    let mut bits: Vec<u8> = (0..32).rev().map(|i| (SYNC >> i & 1) as u8).collect();
    for pair in frame.chunks(2) {
        let data = u128::from(pair[0]) << 8 | u128::from(*pair.get(1).unwrap_or(&0));
        let w = c.code.encode(data);
        bits.extend((0..26).rev().map(|i| (w >> i & 1) as u8));
    }
    bits
}

/// What the framer found at the end of a word.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    /// One distinct frame passed its checks: its bytes, the bits corrected, and whether the bits were inverted.
    Frame(Vec<u8>, u32, bool),
    /// Two or more distinct frames passed: none is taken.
    Ambiguous,
    /// No reading could continue.
    Rejected,
}

/// Counts of the framer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Sync words found.
    pub syncs: u64,
    /// Readings that reached their length and failed the checks.
    pub crc_failures: u64,
    /// Frames given up.
    pub rejections: u64,
    /// Frames with two readings that passed.
    pub ambiguous: u64,
}

/// Searches the sync and assembles the frame word by word.
pub struct Framer {
    code: Arc<WordCode>,
    reg: u32,
    seen: u32,
    inverted: Option<bool>,
    word: u32,
    nbits: u32,
    pending: Vec<(Vec<u8>, u32)>,
    /// The counts.
    pub counts: Counts,
}

impl Default for Framer {
    fn default() -> Self {
        Framer {
            code: word_code(),
            reg: 0,
            seen: 0,
            inverted: None,
            word: 0,
            nbits: 0,
            pending: Vec::new(),
            counts: Counts::default(),
        }
    }
}

impl Framer {
    /// Back to the sync search.
    pub fn reset(&mut self) {
        self.reg = 0;
        self.seen = 0;
        self.inverted = None;
        self.pending.clear();
    }

    /// Takes one bit (0 or 1). Returns what a finished frame gave.
    pub fn push(&mut self, bit: u8) -> Option<Found> {
        let bit = u32::from(bit & 1);
        let Some(inv) = self.inverted else {
            self.reg = self.reg << 1 | bit;
            self.seen = (self.seen + 1).min(32);
            if self.seen < 32 {
                return None;
            }
            let d = (self.reg ^ SYNC).count_ones();
            if d <= SYNC_ERRORS || d >= 32 - SYNC_ERRORS {
                self.inverted = Some(d > SYNC_ERRORS);
                self.counts.syncs += 1;
                self.word = 0;
                self.nbits = 0;
                self.pending.clear();
                self.pending.push((Vec::new(), 0));
            }
            return None;
        };
        self.word = self.word << 1 | (bit ^ u32::from(inv));
        self.nbits += 1;
        if self.nbits < 26 {
            return None;
        }
        self.nbits = 0;
        let found = self.take_word(u128::from(self.word), inv);
        if found.is_some() {
            self.reset();
        }
        found
    }

    fn take_word(&mut self, word: u128, inverted: bool) -> Option<Found> {
        let code = self.code.clone();
        let mut next: Vec<(Vec<u8>, u32)> = Vec::new();
        let mut done: Vec<(Vec<u8>, u32)> = Vec::new();
        for (bytes, fixed) in &self.pending {
            for (cw, w) in code.list.readings(&code.code, word) {
                let data = code.code.info(cw);
                let mut b = bytes.clone();
                b.push((data >> 8) as u8);
                b.push(data as u8);
                b.truncate(MAX_BYTES);
                if !frame::prefix_ok(&b) {
                    continue;
                }
                let fixed = fixed + u32::from(w);
                let want = frame::stated_len(b[1]).clamp(LEN_RANGE.0, LEN_RANGE.1);
                if b.len() >= want {
                    b.truncate(want);
                    // The check needs no locomotive class names.
                    if frame::parse(&b, &Locomotives::default()).is_some() {
                        merge(&mut done, b, fixed);
                    } else {
                        self.counts.crc_failures += 1;
                    }
                } else {
                    merge(&mut next, b, fixed);
                }
            }
        }
        match done.len() {
            1 => {
                let (b, fixed) = done.pop().expect("one frame");
                return Some(Found::Frame(b, fixed, inverted));
            }
            0 => {}
            _ => {
                self.counts.ambiguous += 1;
                return Some(Found::Ambiguous);
            }
        }
        if next.is_empty() || next.len() > MAX_PENDING {
            self.counts.rejections += 1;
            return Some(Found::Rejected);
        }
        self.pending = next;
        None
    }
}

/// Adds `b` to `list`, or keeps the fewer corrections of an equal string.
fn merge(list: &mut Vec<(Vec<u8>, u32)>, b: Vec<u8>, fixed: u32) {
    match list.iter_mut().find(|(x, _)| *x == b) {
        Some(e) => e.1 = e.1.min(fixed),
        None => list.push((b, fixed)),
    }
}

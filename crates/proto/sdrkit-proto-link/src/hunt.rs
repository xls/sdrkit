//! Sync word hunting on a stream of soft bits, for burst links whose frames each start with a sync word and carry their own
//! check. Every match of the word (or of its complement) opens a candidate; several candidates may be open at once. A
//! [`Framing`] reads the header of a candidate, which may drop it at once or give its real length, and decodes it when all its
//! bits are in. After a good frame, the matches that start inside it are dropped.
//!
//! Soft bits are `f32` with a positive value for a 0 bit, as the LLRs of the other crates. A candidate found on the complement
//! of the word gets every bit negated before its framing sees it.
//!
//! ```
//! use sdrkit_proto_link::hunt::{Framing, HuntConfig, Hunter};
//! // Frames of 8 bits behind the sync word B5, good when the byte is 0x42.
//! struct Byte;
//! impl Framing for Byte {
//!     type Frame = u8;
//!     fn frame_bits(&mut self, _header: &[f32]) -> Option<usize> { Some(8) }
//!     fn decode(&mut self, bits: &[f32]) -> Option<u8> {
//!         let b = bits.iter().fold(0u8, |a, &s| (a << 1) | u8::from(s < 0.0));
//!         (b == 0x42).then_some(b)
//!     }
//! }
//! let cfg = HuntConfig { word: 0xB5, word_bits: 8, max_errors: 0, complement: true, ..HuntConfig::default() };
//! let mut h = Hunter::new(cfg, Byte).unwrap();
//! let mut found = Vec::new();
//! for byte in [0x00u8, !0xB5, !0x42, 0x00] {
//!     for k in (0..8).rev() {
//!         h.push(if (byte >> k) & 1 == 1 { -1.0 } else { 1.0 }, &mut found);
//!     }
//! }
//! assert_eq!(found.len(), 1);
//! assert_eq!((found[0].frame, found[0].inverted), (0x42, true));
//! ```

use crate::{Error, Param, Result};
use sdrkit_util_bits::sync::SyncWord;

/// The rules of a hunter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HuntConfig {
    /// The sync word, first bit sent most significant, in the low `word_bits` bits.
    pub word: u128,
    /// Bits of the sync word, 8 to 128.
    pub word_bits: u32,
    /// Wrong bits a match may have.
    pub max_errors: u32,
    /// The complement of the word is a match too (an inverted signal).
    pub complement: bool,
    /// Candidates open at once; a match beyond this is dropped.
    pub max_open: usize,
    /// The longest frame after the sync word, bits. Sets the soft bit store.
    pub max_frame_bits: usize,
}

impl Default for HuntConfig {
    fn default() -> Self {
        HuntConfig {
            word: 0,
            word_bits: 32,
            max_errors: 0,
            complement: true,
            max_open: 64,
            max_frame_bits: 8192,
        }
    }
}

/// What a framing does with a candidate.
pub trait Framing {
    /// A decoded frame.
    type Frame;
    /// Bits after the sync word that [`Framing::frame_bits`] needs to see. 0 for a frame of fixed length.
    fn header_bits(&self) -> usize {
        0
    }
    /// The bits of the whole frame after the sync word, read from the first [`Framing::header_bits`] bits. `None` drops the
    /// candidate (the header is not valid).
    fn frame_bits(&mut self, header: &[f32]) -> Option<usize>;
    /// Decodes the frame from its bits after the sync word. `None` when its checks fail.
    fn decode(&mut self, bits: &[f32]) -> Option<Self::Frame>;
}

/// A frame the hunter found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found<T> {
    /// The decoded frame.
    pub frame: T,
    /// The sync word was found complemented.
    pub inverted: bool,
    /// Wrong bits of the sync word.
    pub sync_errors: u32,
    /// The stream index of the first bit after the sync word.
    pub start: u64,
    /// The stream index just past the last bit of the frame.
    pub end: u64,
}

/// The counts of a hunter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HuntCounts {
    /// Sync word matches.
    pub sync_hits: u64,
    /// Candidates whose header or decode failed.
    pub failed: u64,
    /// Frames decoded.
    pub frames: u64,
}

#[derive(Clone, Copy, Debug)]
struct Open {
    start: u64,
    inverted: bool,
    errors: u32,
    need: usize,
    header_done: bool,
}

/// The sync word hunter. See the [module documentation](self).
pub struct Hunter<F: Framing> {
    cfg: HuntConfig,
    sync: SyncWord,
    framing: F,
    ring: Vec<f32>,
    mask: usize,
    n: u64,
    open: Vec<Open>,
    scratch: Vec<f32>,
    quiet_until: u64,
    counts: HuntCounts,
}

impl<F: Framing> Hunter<F> {
    /// A hunter with these rules for `framing`.
    ///
    /// # Errors
    ///
    /// [`Error::BadParameter`] when the word length is outside 8 to 128 bits or the frame length is 0.
    pub fn new(cfg: HuntConfig, framing: F) -> Result<Self> {
        if !(8..=128).contains(&cfg.word_bits) || cfg.max_frame_bits == 0 {
            return Err(Error::BadParameter(Param::SyncWord));
        }
        let sync = SyncWord::new(cfg.word, cfg.word_bits, cfg.max_errors)
            .map_err(|_| Error::BadParameter(Param::SyncWord))?
            .with_inverted(cfg.complement);
        let size = (cfg.max_frame_bits + 64).next_power_of_two();
        Ok(Hunter {
            cfg,
            sync,
            framing,
            ring: vec![0.0; size],
            mask: size - 1,
            n: 0,
            open: Vec::with_capacity(cfg.max_open),
            scratch: Vec::with_capacity(cfg.max_frame_bits),
            quiet_until: 0,
            counts: HuntCounts::default(),
        })
    }

    /// The framing.
    pub fn framing(&self) -> &F {
        &self.framing
    }

    /// The framing, to change its settings.
    pub fn framing_mut(&mut self) -> &mut F {
        &mut self.framing
    }

    /// The counts so far.
    pub fn counts(&self) -> HuntCounts {
        self.counts
    }

    /// Bits taken so far.
    pub fn position(&self) -> u64 {
        self.n
    }

    /// Drops the open candidates and the sync word history.
    pub fn reset(&mut self) {
        self.open.clear();
        self.sync.reset();
        self.quiet_until = 0;
    }

    /// Copies bits `[from, from + len)` of the stream into the scratch, negated when `invert`.
    fn copy_out(&mut self, from: u64, len: usize, invert: bool) {
        self.scratch.clear();
        let sign = if invert { -1.0 } else { 1.0 };
        for i in from..from + len as u64 {
            self.scratch.push(sign * self.ring[i as usize & self.mask]);
        }
    }

    /// Takes one soft bit and appends the frames that end on it to `out`.
    pub fn push(&mut self, soft: f32, out: &mut Vec<Found<F::Frame>>) {
        self.ring[self.n as usize & self.mask] = soft;
        self.n += 1;
        let bit = u8::from(soft < 0.0);
        if let Some(hit) = self.sync.push(bit) {
            self.counts.sync_hits += 1;
            if self.n >= self.quiet_until && self.open.len() < self.cfg.max_open {
                let header = self.framing.header_bits();
                self.open.push(Open {
                    start: self.n,
                    inverted: hit.inverted(),
                    errors: hit.errors(),
                    need: header,
                    header_done: header == 0,
                });
                if header == 0 {
                    self.read_header(self.open.len() - 1);
                }
            }
        }
        let mut i = 0;
        while i < self.open.len() {
            let c = self.open[i];
            if c.need == 0 || self.n - c.start < c.need as u64 {
                i += 1;
                continue;
            }
            if !c.header_done {
                if self.read_header(i) {
                    i += 1;
                } else {
                    self.open.swap_remove(i);
                    self.counts.failed += 1;
                }
                continue;
            }
            self.open.swap_remove(i);
            self.copy_out(c.start, c.need, c.inverted);
            match self.framing.decode(&self.scratch) {
                Some(frame) => {
                    self.counts.frames += 1;
                    let end = c.start + c.need as u64;
                    self.quiet_until = end;
                    self.open.retain(|o| o.start <= c.start || o.start >= end);
                    out.push(Found {
                        frame,
                        inverted: c.inverted,
                        sync_errors: c.errors,
                        start: c.start,
                        end,
                    });
                    i = 0;
                }
                None => self.counts.failed += 1,
            }
        }
    }

    /// Reads the header of candidate `i` when its bits are in; returns false when the candidate is dropped.
    fn read_header(&mut self, i: usize) -> bool {
        let c = self.open[i];
        let header = self.framing.header_bits();
        self.copy_out(c.start, header, c.inverted);
        match self.framing.frame_bits(&self.scratch) {
            Some(total) if total > 0 && total <= self.cfg.max_frame_bits => {
                let o = &mut self.open[i];
                o.need = total.max(header);
                o.header_done = true;
                true
            }
            _ => {
                if header == 0 {
                    // A fixed-length framing that refuses: drop it here.
                    self.open.swap_remove(i);
                    self.counts.failed += 1;
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of a length byte and that many bytes, good when the bytes sum to 0 modulo 256.
    struct Sized;
    impl Framing for Sized {
        type Frame = Vec<u8>;
        fn header_bits(&self) -> usize {
            8
        }
        fn frame_bits(&mut self, header: &[f32]) -> Option<usize> {
            let n = bytes(header)[0] as usize;
            (n > 0).then_some(8 + 8 * n)
        }
        fn decode(&mut self, bits: &[f32]) -> Option<Vec<u8>> {
            let b = bytes(bits);
            (b[1..].iter().fold(0u8, |a, &x| a.wrapping_add(x)) == 0).then_some(b)
        }
    }

    fn bytes(bits: &[f32]) -> Vec<u8> {
        bits.chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &s| (a << 1) | u8::from(s < 0.0)))
            .collect()
    }

    fn soft(bytes: &[u8], out: &mut Vec<f32>) {
        for b in bytes {
            for k in (0..8).rev() {
                out.push(if (b >> k) & 1 == 1 { -1.0 } else { 1.0 });
            }
        }
    }

    #[test]
    fn frames_back_to_back_both_polarities_and_a_bad_one() {
        let cfg = HuntConfig {
            word: 0x1ACF_FC1D,
            word_bits: 32,
            max_errors: 2,
            ..HuntConfig::default()
        };
        let mut h = Hunter::new(cfg, Sized).unwrap();
        let mut s = Vec::new();
        soft(&[0x55; 4], &mut s);
        soft(&[0x1A, 0xCF, 0xFC, 0x1D, 3, 1, 2, 0xFD], &mut s);
        // Complemented, with one wrong sync bit.
        soft(&[!0x1A, !0xCF ^ 0x10, !0xFC, !0x1D, !2, !7, !0xF9], &mut s);
        // A bad sum.
        soft(&[0x1A, 0xCF, 0xFC, 0x1D, 2, 7, 7], &mut s);
        soft(&[0x55; 4], &mut s);
        let mut out = Vec::new();
        for &v in &s {
            h.push(v, &mut out);
        }
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].frame, [3, 1, 2, 0xFD]);
        assert!(!out[0].inverted);
        assert_eq!(out[1].frame, [2, 7, 0xF9]);
        assert!(out[1].inverted && out[1].sync_errors == 1);
        let c = h.counts();
        assert_eq!((c.sync_hits, c.frames, c.failed), (3, 2, 1));
        assert!(Hunter::new(
            HuntConfig {
                word_bits: 4,
                ..cfg
            },
            Sized
        )
        .is_err());
    }
}

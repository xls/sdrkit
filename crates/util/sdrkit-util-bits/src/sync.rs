//! Finding sync words in bit streams.
//!
//! - [`SyncWord`] matches the newest bits of a hard-bit stream against a pattern of up to 128 bits, allowing up to `max_err`
//!   differing bits, in normal and (optionally) inverted polarity. One XOR and one popcount per bit.
//! - [`SoftSync`] correlates soft bits (LLRs, positive means 0) against a pattern and reports peaks above a threshold.
//! - [`byte_sync_search`] finds the bit phase of a byte sync that repeats at a fixed spacing (MPEG-TS 0x47 every 188 bytes, the
//!   CCSDS ASM in bytes).
//!
//! ```
//! use sdrkit_util_bits::sync::{Hit, SyncWord};
//! // The CCSDS attached sync marker 0x1ACFFC1D with up to 2 bit errors, either polarity.
//! let mut s = SyncWord::new(0x1ACF_FC1D, 32, 2).unwrap().with_inverted(true);
//! let mut bits = vec![0u8; 5];
//! sdrkit_util_bits::pack::push_u64(&mut bits, !0x1ACF_FC1Du64 ^ 0b101, 32); // inverted, 2 errors
//! let mut hits = Vec::new();
//! s.scan(&bits, &mut hits);
//! assert_eq!(hits, [(37, Hit::Inverted { errors: 2 })]);
//! ```

use crate::Param;
use crate::{Error, Result, Soft};

/// A sync word match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// The pattern, with this many differing bits.
    Normal {
        /// Differing bits.
        errors: u32,
    },
    /// The inverted pattern (the bits after it are inverted too), with this many differing bits.
    Inverted {
        /// Differing bits.
        errors: u32,
    },
}

impl Hit {
    /// The number of differing bits.
    pub fn errors(&self) -> u32 {
        match *self {
            Hit::Normal { errors } | Hit::Inverted { errors } => errors,
        }
    }

    /// Whether the stream is inverted.
    pub fn inverted(&self) -> bool {
        matches!(self, Hit::Inverted { .. })
    }
}

/// A hard-bit sync word matcher. See the [module documentation](self).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncWord {
    reg: u128,
    seen: u32,
    pat: u128,
    mask: u128,
    len: u32,
    max_err: u32,
    inverted_ok: bool,
}

impl SyncWord {
    /// A matcher for the `len` low bits of `pat` (first bit most significant), 1 to 128 bits, allowing `max_err` differing bits
    /// (less than half of `len`, so normal and inverted matches cannot both hold).
    pub const fn new(pat: u128, len: u32, max_err: u32) -> Result<Self> {
        if len == 0 || len > 128 || 2 * max_err >= len {
            return Err(Error::BadParameter(Param::Sync));
        }
        let mask = if len == 128 {
            u128::MAX
        } else {
            (1u128 << len) - 1
        };
        Ok(SyncWord {
            reg: 0,
            seen: 0,
            pat: pat & mask,
            mask,
            len,
            max_err,
            inverted_ok: false,
        })
    }

    /// Also report matches of the inverted pattern.
    pub const fn with_inverted(mut self, yes: bool) -> Self {
        self.inverted_ok = yes;
        self
    }

    /// Only bits under `mask` (aligned with the pattern) are compared; the others are "don't care".
    pub const fn with_mask(mut self, mask: u128) -> Self {
        self.mask &= mask;
        self
    }

    /// The pattern length.
    pub const fn len(&self) -> u32 {
        self.len
    }

    /// Whether the pattern is empty (never: `new` refuses it).
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The number of differing bits between the newest `len` bits and the pattern.
    #[inline]
    pub fn distance(&self) -> u32 {
        ((self.reg ^ self.pat) & self.mask).count_ones()
    }

    /// Takes one hard bit; reports a match ending at it.
    #[inline]
    pub fn push(&mut self, bit: u8) -> Option<Hit> {
        self.reg = (self.reg << 1) | (bit & 1) as u128;
        if self.seen < self.len {
            self.seen += 1;
            if self.seen < self.len {
                return None;
            }
        }
        let d = ((self.reg ^ self.pat) & self.mask).count_ones();
        if d <= self.max_err {
            return Some(Hit::Normal { errors: d });
        }
        if self.inverted_ok {
            let di = self.mask.count_ones() - d;
            if di <= self.max_err {
                return Some(Hit::Inverted { errors: di });
            }
        }
        None
    }

    /// Runs over `bits` and appends `(index after the last bit of the match, hit)` for every match to `hits`, so the index is
    /// where the data after the sync word starts.
    pub fn scan(&mut self, bits: &[u8], hits: &mut Vec<(usize, Hit)>) {
        for (i, &b) in bits.iter().enumerate() {
            if let Some(h) = self.push(b) {
                hits.push((i + 1, h));
            }
        }
    }

    /// Forgets the bits seen.
    pub fn reset(&mut self) {
        self.reg = 0;
        self.seen = 0;
    }
}

/// A soft sync correlation peak.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoftHit {
    /// The correlation normalised to -1 .. 1: the sum of the LLRs signed by the pattern, divided by the sum of their magnitudes.
    /// 1 is a perfect match; negative values are the inverted pattern.
    pub score: f32,
    /// The unnormalised correlation (in LLR units).
    pub metric: f32,
}

/// A soft correlator: keeps the newest `len` soft bits and correlates them with a pattern. A positive LLR means 0, so a pattern
/// bit 0 counts `+L` and a 1 counts `-L`. Allocates its window at `new`.
#[derive(Clone, Debug)]
pub struct SoftSync {
    sign: Vec<f32>,
    win: Vec<f32>,
    at: usize,
    seen: usize,
    threshold: f32,
    inverted_ok: bool,
}

impl SoftSync {
    /// A correlator for the `len` low bits of `pat` (first bit most significant, at most 128) reporting normalised scores of at
    /// least `threshold` (0 to 1).
    pub fn new(pat: u128, len: u32, threshold: f32) -> Result<Self> {
        if len == 0 || len > 128 || !(0.0..=1.0).contains(&threshold) {
            return Err(Error::BadParameter(Param::SoftSync));
        }
        let sign = (0..len)
            .map(|k| {
                if (pat >> (len - 1 - k)) & 1 == 1 {
                    -1.0
                } else {
                    1.0
                }
            })
            .collect();
        Ok(SoftSync {
            sign,
            win: vec![0.0; len as usize],
            at: 0,
            seen: 0,
            threshold,
            inverted_ok: false,
        })
    }

    /// Also report the inverted pattern (as a negative score).
    pub fn with_inverted(mut self, yes: bool) -> Self {
        self.inverted_ok = yes;
        self
    }

    /// Takes one soft bit (`i8` or `f32` LLR, or a hard bit taken at full confidence); reports a peak ending at it.
    pub fn push<T: Soft>(&mut self, x: T) -> Option<SoftHit> {
        let n = self.win.len();
        self.win[self.at] = x.llr();
        self.at = if self.at + 1 == n { 0 } else { self.at + 1 };
        if self.seen < n {
            self.seen += 1;
            if self.seen < n {
                return None;
            }
        }
        // The oldest value is at `at`.
        let (mut m, mut mag) = (0.0f32, 0.0f32);
        let (old, new) = self.win.split_at(self.at);
        for (v, s) in new.iter().chain(old).zip(&self.sign) {
            m += v * s;
            mag += v.abs();
        }
        if mag <= 0.0 {
            return None;
        }
        let score = m / mag;
        if score >= self.threshold || (self.inverted_ok && -score >= self.threshold) {
            Some(SoftHit { score, metric: m })
        } else {
            None
        }
    }

    /// Runs over `xs` and appends `(index after the match, hit)` for each peak to `hits`.
    pub fn scan<T: Soft>(&mut self, xs: &[T], hits: &mut Vec<(usize, SoftHit)>) {
        for (i, &x) in xs.iter().enumerate() {
            if let Some(h) = self.push(x) {
                hits.push((i + 1, h));
            }
        }
    }

    /// Forgets the values seen.
    pub fn reset(&mut self) {
        self.win.fill(0.0);
        self.at = 0;
        self.seen = 0;
    }
}

/// Where [`byte_sync_search`] found the sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteSync {
    /// The bit index of the first sync byte.
    pub start: usize,
    /// Whether the stream is inverted.
    pub inverted: bool,
    /// Sync bytes found at their places.
    pub hits: usize,
    /// Places looked at.
    pub places: usize,
}

/// Searches hard bits for a byte sync that repeats every `spacing` bytes (MPEG-TS: 0x47 every 188 bytes; DVB also accepts the
/// inverted 0xB8 that starts every eighth packet, so `words` is `[0x47, 0xB8]`). Every bit phase in the first `spacing` bytes is
/// tried; the phase with the most places holding one of `words` (MSB first) wins, if it has at least `min_hits`. With `inverted`,
/// the complemented stream is tried too (when `words` holds a byte and its complement, as `[0x47, 0xB8]` does, the polarity
/// cannot be told this way: search with one of them).
pub fn byte_sync_search(
    bits: &[u8],
    words: &[u8],
    spacing: usize,
    min_hits: usize,
    inverted: bool,
) -> Option<ByteSync> {
    if spacing == 0 || words.is_empty() {
        return None;
    }
    let step = spacing * 8;
    let mut best: Option<ByteSync> = None;
    for start in 0..step.min(bits.len()) {
        let (mut hits, mut hits_inv, mut places) = (0usize, 0usize, 0usize);
        let mut at = start;
        while at + 8 <= bits.len() {
            let mut a = [0u8; 8];
            a.copy_from_slice(&bits[at..at + 8]);
            let b = crate::pack::gather8(a, crate::BitOrder::Msb);
            hits += words.contains(&b) as usize;
            hits_inv += words.contains(&!b) as usize;
            places += 1;
            at += step;
        }
        for (h, inv) in [(hits, false), (hits_inv, true)] {
            if inv && !inverted {
                continue;
            }
            if h >= min_hits && best.is_none_or(|b| h > b.hits) {
                best = Some(ByteSync {
                    start,
                    inverted: inv,
                    hits: h,
                    places,
                });
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::push_u64;

    #[test]
    fn hard_matches_with_errors_and_polarity() {
        let mut bits = vec![0u8, 1, 1];
        push_u64(&mut bits, 0x7CD2_15D8, 32); // POCSAG sync
        push_u64(&mut bits, 0x7CD2_15D8 ^ 0x0001_0010, 32);
        push_u64(&mut bits, !0x7CD2_15D8u64, 32);
        let mut s = SyncWord::new(0x7CD2_15D8, 32, 2).unwrap();
        let mut hits = Vec::new();
        s.scan(&bits, &mut hits);
        assert_eq!(
            hits,
            [
                (35, Hit::Normal { errors: 0 }),
                (67, Hit::Normal { errors: 2 })
            ]
        );
        let mut s = s.with_inverted(true);
        s.reset();
        hits.clear();
        s.scan(&bits, &mut hits);
        assert_eq!(hits.len(), 3);
        assert!(hits[2].1.inverted() && hits[2].1.errors() == 0);
        assert!(SyncWord::new(0, 0, 0).is_err());
        assert!(SyncWord::new(0, 8, 4).is_err());
    }

    #[test]
    fn long_and_masked_patterns() {
        let pat: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210;
        let mut bits = Vec::new();
        push_u64(&mut bits, (pat >> 64) as u64, 64);
        push_u64(&mut bits, pat as u64, 64);
        let mut s = SyncWord::new(pat, 128, 3).unwrap();
        let mut hits = Vec::new();
        s.scan(&bits, &mut hits);
        assert_eq!(hits, [(128, Hit::Normal { errors: 0 })]);
        // Don't-care bits: only the top 16 bits of a 32-bit pattern.
        let mut m = SyncWord::new(0xC4D7_0000, 32, 0)
            .unwrap()
            .with_mask(0xFFFF_0000);
        let mut b = Vec::new();
        push_u64(&mut b, 0xC4D7_1234, 32);
        hits.clear();
        m.scan(&b, &mut hits);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn soft_correlator() {
        let pat = 0b1011_0011u128;
        let mut s = SoftSync::new(pat, 8, 0.8).unwrap().with_inverted(true);
        let mut x: Vec<i8> = vec![5, -5, 5];
        for k in (0..8).rev() {
            x.push(if (pat >> k) & 1 == 1 { -60 } else { 60 });
        }
        x[5] = 10; // one weak wrong bit
        for k in (0..8).rev() {
            x.push(if (pat >> k) & 1 == 1 { 40 } else { -40 }); // inverted
        }
        let mut hits = Vec::new();
        s.scan(&x, &mut hits);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].0, 11);
        assert!(hits[0].1.score > 0.8 && hits[0].1.score < 1.0);
        assert_eq!(hits[1].0, 19);
        assert!((hits[1].1.score + 1.0).abs() < 1e-6);
        let mut f = SoftSync::new(pat, 8, 0.99).unwrap();
        let y: Vec<f32> = x.iter().map(|&v| v as f32 * 0.01).collect();
        hits.clear();
        f.scan(&y, &mut hits);
        assert!(hits.is_empty());
        assert!(SoftSync::new(1, 0, 0.5).is_err());
        f.reset();
    }

    #[test]
    fn transport_stream_phase() {
        let mut bits = vec![1u8; 5]; // 5 bits of junk before the first sync byte
        for p in 0..10u8 {
            let mut pkt = vec![p.wrapping_mul(17); 188];
            pkt[0] = if p % 8 == 0 { 0xB8 } else { 0x47 };
            crate::pack::unpack(&pkt, crate::BitOrder::Msb, &mut bits);
        }
        let s = byte_sync_search(&bits, &[0x47, 0xB8], 188, 8, true).unwrap();
        assert_eq!((s.start, s.inverted, s.hits), (5, false, 10));
        let inv: Vec<u8> = bits.iter().map(|b| b ^ 1).collect();
        // 0x47 and 0xB8 are complements, so a search for 0x47 alone also finds the polarity.
        let s = byte_sync_search(&inv, &[0x47], 188, 8, true).unwrap();
        assert_eq!((s.start, s.inverted, s.hits), (5, true, 8));
        assert!(byte_sync_search(&inv, &[0x47], 188, 8, false).is_none());
    }
}

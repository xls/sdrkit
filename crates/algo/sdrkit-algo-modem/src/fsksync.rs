//! Sync search over the discriminator output of a 2-FSK burst: a bit pattern compared at fractional steps of a symbol, with the
//! slicing threshold taken from the pattern itself, so a carrier offset (a constant added to the discriminator) does not change
//! it.
//!
//! At each trial position the value of a symbol is the mean of the discriminator over the samples of that symbol. The threshold
//! is the midpoint between the mean value of the pattern's 1 symbols and that of its 0 symbols, and a pattern symbol on the
//! wrong side of it is a bit error. A 1 is the higher frequency; with [`Polarity::Either`] the pattern with every bit inverted
//! is accepted as well. Of the positions with at most the allowed errors, the one with the best score
//! `|mean(1) - mean(0)| × (bits - errors)` wins. The bits after the pattern are sliced with the same threshold.
//!
//! ```
//! use sdrkit_algo_modem::fsksync::{BitSync, Polarity};
//! let pattern = [1u8, 0, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0];
//! let data = [0u8, 1, 1, 0];
//! // 8 samples a bit, a carrier offset of 0.3 and levels of ±1.
//! let d: Vec<f32> = std::iter::repeat_n(0u8, 5)
//!     .chain(pattern.iter().copied())
//!     .chain(data.iter().copied())
//!     .flat_map(|b| std::iter::repeat_n(if b == 1 { 1.3f32 } else { -0.7 }, 8))
//!     .collect();
//! let mut s = BitSync::new();
//! let hit = s.find(&d, 8.0, &pattern, 0, Polarity::Normal, 8).unwrap();
//! assert_eq!((hit.at.round() as usize, hit.errors), (40, 0));
//! assert!((hit.threshold - 0.3).abs() < 1e-6);
//! let mut bits = Vec::new();
//! s.bits(&d, &hit, 4, &mut bits);
//! assert_eq!(bits, data);
//! ```

/// Which polarities of the pattern are accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Polarity {
    /// A 1 is the higher frequency.
    Normal,
    /// Either; a hit says which.
    Either,
}

/// Where a pattern was found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncHit {
    /// The sample position of the start of the pattern's first symbol (fractional).
    pub at: f64,
    /// Samples per symbol.
    pub sps: f64,
    /// The slicing threshold (the carrier offset, in discriminator units).
    pub threshold: f32,
    /// The pattern was found with every bit inverted.
    pub inverted: bool,
    /// Pattern symbols on the wrong side of the threshold.
    pub errors: u32,
    /// `|mean(1) - mean(0)| × (bits - errors)`.
    pub score: f32,
    /// Bits of the pattern.
    pub len: usize,
}

/// The search, with its buffer of running sums (module documentation). It allocates only when a longer input than before
/// comes.
#[derive(Clone, Debug, Default)]
pub struct BitSync {
    cum: Vec<f64>,
}

impl BitSync {
    /// A search with no buffer yet.
    pub fn new() -> BitSync {
        BitSync::default()
    }

    fn sums(&mut self, d: &[f32]) {
        self.cum.clear();
        self.cum.push(0.0);
        let mut a = 0f64;
        for &v in d {
            a += f64::from(if v.is_finite() { v } else { 0.0 });
            self.cum.push(a);
        }
    }

    /// The mean of the samples from `pos` (rounded) to `pos + sps` (rounded), or `None` past the end.
    #[inline]
    fn value(&self, pos: f64, sps: f64) -> Option<f32> {
        let a = pos.round().max(0.0) as usize;
        let b = (pos + sps).round() as usize;
        if b >= self.cum.len() || b <= a {
            return None;
        }
        Some(((self.cum[b] - self.cum[a]) / (b - a) as f64) as f32)
    }

    /// Searches `d` (discriminator values, `sps` samples a symbol, at least 2) for `pattern` (one bit a byte, the first sent
    /// first) at `steps` positions a symbol, with at most `max_errors` bit errors. `None` when no position qualifies.
    pub fn find(
        &mut self,
        d: &[f32],
        sps: f64,
        pattern: &[u8],
        max_errors: u32,
        polarity: Polarity,
        steps: usize,
    ) -> Option<SyncHit> {
        let len = pattern.len();
        let ones = pattern.iter().filter(|&&b| b & 1 == 1).count();
        if !(sps >= 2.0 && sps.is_finite()) || len == 0 || ones == 0 || ones == len {
            return None;
        }
        self.sums(d);
        let span = len as f64 * sps;
        let step = sps / steps.max(1) as f64;
        let mut best: Option<SyncHit> = None;
        let mut k = 0usize;
        loop {
            let at = k as f64 * step;
            k += 1;
            if at + span + 1.0 > d.len() as f64 {
                break;
            }
            let (mut s1, mut s0) = (0f32, 0f32);
            for (i, &b) in pattern.iter().enumerate() {
                let Some(v) = self.value(at + i as f64 * sps, sps) else {
                    break;
                };
                if b & 1 == 1 {
                    s1 += v;
                } else {
                    s0 += v;
                }
            }
            let m1 = s1 / ones as f32;
            let m0 = s0 / (len - ones) as f32;
            let inverted = m1 < m0;
            if inverted && polarity == Polarity::Normal {
                continue;
            }
            let thr = 0.5 * (m1 + m0);
            let mut errors = 0u32;
            for (i, &b) in pattern.iter().enumerate() {
                let v = self.value(at + i as f64 * sps, sps).unwrap_or(thr);
                let one = (v > thr) != inverted;
                if one != (b & 1 == 1) {
                    errors += 1;
                    if errors > max_errors {
                        break;
                    }
                }
            }
            if errors > max_errors {
                continue;
            }
            let score = (m1 - m0).abs() * (len as u32 - errors) as f32;
            if best.is_none_or(|b| score > b.score) {
                best = Some(SyncHit {
                    at,
                    sps,
                    threshold: thr,
                    inverted,
                    errors,
                    score,
                    len,
                });
            }
        }
        best
    }

    /// Slices up to `n` symbols after the pattern of `hit` from the same `d` that [`find`](Self::find) searched, into `out`
    /// (cleared first, one bit a byte). Returns how many were read.
    pub fn bits(&self, d: &[f32], hit: &SyncHit, n: usize, out: &mut Vec<u8>) -> usize {
        out.clear();
        if self.cum.len() != d.len() + 1 {
            return 0;
        }
        for i in 0..n {
            let pos = hit.at + (hit.len + i) as f64 * hit.sps;
            let Some(v) = self.value(pos, hit.sps) else {
                break;
            };
            out.push(u8::from((v > hit.threshold) != hit.inverted));
        }
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels(bits: &[u8], sps: usize, hi: f32, lo: f32) -> Vec<f32> {
        bits.iter()
            .flat_map(|&b| std::iter::repeat_n(if b == 1 { hi } else { lo }, sps))
            .collect()
    }

    #[test]
    fn the_inverted_pattern_is_found_only_when_asked_for() {
        let p = [1u8, 1, 0, 1, 0, 0, 1, 0, 1, 1, 1, 0, 0, 0, 1, 0];
        let inv: Vec<u8> = p.iter().map(|b| b ^ 1).collect();
        let mut bits = vec![0u8, 1, 0, 1];
        bits.extend(&inv);
        bits.extend([1, 1, 0, 0, 1, 0, 1, 0]);
        let d = levels(&bits, 6, 0.8, -0.8);
        let mut s = BitSync::new();
        assert!(s.find(&d, 6.0, &p, 1, Polarity::Normal, 8).is_none());
        let h = s.find(&d, 6.0, &p, 1, Polarity::Either, 8).unwrap();
        assert!(h.inverted && h.errors == 0 && (h.at - 24.0).abs() < 1.0);
        let mut out = Vec::new();
        assert_eq!(s.bits(&d, &h, 8, &mut out), 8);
        assert_eq!(out, [0, 0, 1, 1, 0, 1, 0, 1]);
    }

    #[test]
    fn one_bit_error_is_allowed_and_two_are_not() {
        let p = [1u8, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 1, 0, 1, 0, 1];
        let mut bits = vec![0u8; 3];
        let mut q = p;
        q[9] ^= 1;
        bits.extend(q);
        bits.extend([0u8; 4]);
        let d = levels(&bits, 8, 1.0, -1.0);
        let mut s = BitSync::new();
        assert_eq!(s.find(&d, 8.0, &p, 1, Polarity::Normal, 8).map(|h| h.errors), Some(1));
        q[2] ^= 1;
        let mut bits = vec![0u8; 3];
        bits.extend(q);
        bits.extend([0u8; 4]);
        let d = levels(&bits, 8, 1.0, -1.0);
        assert!(s.find(&d, 8.0, &p, 1, Polarity::Normal, 8).is_none());
    }
}

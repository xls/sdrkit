//! Soft bits: the `i8` fast-path LLR, its hard decision, and the one shared [`Quantizer`] that turns `f32` LLRs into `i8`.
//!
//! The convention is the crate's: `L = ln P(b=0) / P(b=1)`, so **positive means 0**, and an erased or punctured bit is 0. `i8`
//! LLRs saturate at ±127; −128 is never produced (so negating is always exact). Demappers give true LLRs (they take the noise
//! variance), and the quantizer only scales them into the `i8` range.
//!
//! ```
//! use sdrkit_algo_fec::llr::{hard, Quantizer};
//! let soft = [3.2f32, -0.4, 0.0, -9.0, 1.1];
//! let q = Quantizer::default();
//! let scale = q.scale_for(&soft);
//! let mut out = [0i8; 5];
//! q.quantize(&soft, scale, &mut out).unwrap();
//! assert_eq!(out.map(hard), [0, 1, 0, 1, 0]);
//! assert_eq!(out[2], 0); // an erasure stays an erasure
//! ```

use crate::error::{FecError, Param, Result};

/// An `i8` log-likelihood ratio: positive means 0, 0 means erased, saturated at ±127.
pub type Llr8 = i8;

/// The hard decision of an `i8` LLR: its sign bit (0 for positive or erased, 1 for negative).
#[inline]
pub const fn hard(l: Llr8) -> u8 {
    (l as u8) >> 7
}

/// The hard decision of an `f32` LLR (0 for positive, zero or NaN).
#[inline]
pub fn hard_f32(l: f32) -> u8 {
    (l < 0.0) as u8
}

/// The LLR of a hard bit at full confidence: +127 for 0, −127 for 1.
#[inline]
pub const fn from_hard(bit: u8) -> Llr8 {
    if bit & 1 == 0 {
        127
    } else {
        -127
    }
}

/// Adds two `i8` LLRs of the same bit (independent observations), saturating at ±127.
#[inline]
pub const fn add(a: Llr8, b: Llr8) -> Llr8 {
    let s = a as i16 + b as i16;
    if s > 127 {
        127
    } else if s < -127 {
        -127
    } else {
        s as i8
    }
}

/// Scales `f32` LLRs into `i8` ones.
///
/// The scale is chosen so that the mean magnitude of the non-zero inputs lands on `target` (out of 127): enough headroom that
/// strong bits are not all clipped to the same value, enough resolution that weak ones are not rounded to 0. For Viterbi decoding a
/// target near 20 to 30 works well; for min-sum LDPC near 16 to 24. The quantizer itself never changes the sign of a value, so the
/// hard decisions are kept exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantizer {
    /// Mean magnitude the non-zero inputs are scaled to.
    pub target: f32,
    /// Largest magnitude written (1 to 127).
    pub limit: i8,
    /// Largest scale [`Quantizer::scale_for`] returns (for nearly all-zero input).
    pub max_scale: f32,
}

impl Default for Quantizer {
    /// Target 20, limit 127, maximum scale 1000.
    fn default() -> Self {
        Quantizer {
            target: 20.0,
            limit: 127,
            max_scale: 1000.0,
        }
    }
}

impl Quantizer {
    /// The scale that maps the mean magnitude of the finite, non-zero values of `x` to `target`, at most `max_scale` (and
    /// `max_scale` for input with no such values). The magnitudes are added in eight interleaved partial sums, which are then
    /// added in a fixed order.
    pub fn scale_for(&self, x: &[f32]) -> f32 {
        let (mut sums, mut counts) = ([0f64; 8], [0u64; 8]);
        let (bulk, tail) = x.as_chunks::<8>();
        for xs in bulk {
            for k in 0..8 {
                let a = xs[k].abs();
                // False for an infinity and for NaN.
                let finite = a < f32::INFINITY;
                sums[k] += if finite { a as f64 } else { 0.0 };
                counts[k] += (finite & (a != 0.0)) as u64;
            }
        }
        let mut sum = ((sums[0] + sums[4]) + (sums[2] + sums[6]))
            + ((sums[1] + sums[5]) + (sums[3] + sums[7]));
        let mut n = counts.iter().sum::<u64>() as usize;
        for &v in tail {
            if v != 0.0 && v.is_finite() {
                sum += v.abs() as f64;
                n += 1;
            }
        }
        if n == 0 || sum == 0.0 {
            return self.max_scale;
        }
        ((self.target as f64 * n as f64 / sum) as f32).min(self.max_scale)
    }

    /// Writes `round(x * scale)` limited to ±`limit` into `out` (which must be at least as long as `x`). The bulk is done 16 at a
    /// time in a loop the compiler vectorises; the tail one by one; both give the same values. A NaN is written as 0 (erased) and
    /// reported as `Err(NotFinite)` after the whole input has been written; infinities saturate (an infinity times a scale of 0
    /// is a NaN product and is written as 0).
    pub fn quantize(&self, x: &[f32], scale: f32, out: &mut [Llr8]) -> Result<()> {
        if out.len() < x.len() {
            return Err(FecError::OutputTooShort { need: x.len() });
        }
        if !(scale.is_finite() && scale >= 0.0) {
            return Param::Scale.degenerate();
        }
        Param::Limit.check(self.limit, 1, 127)?;
        let lim = self.limit as i32;
        // `as i32` maps NaN to 0 and saturates, and the integer clamp keeps the result in range.
        let one = |v: f32| ((v * scale).round() as i32).clamp(-lim, lim) as i8;
        let n = x.len();
        let bulk = n - n % 16;
        let mut nan = false;
        for (xs, os) in x[..bulk]
            .as_chunks::<16>()
            .0
            .iter()
            .zip(out[..bulk].as_chunks_mut::<16>().0)
        {
            for k in 0..16 {
                os[k] = one(xs[k]);
                nan |= xs[k].is_nan();
            }
        }
        for (v, o) in x[bulk..].iter().zip(out[bulk..n].iter_mut()) {
            *o = one(*v);
            nan |= v.is_nan();
        }
        if nan {
            if let Some(at) = x.iter().position(|v| v.is_nan()) {
                return Err(FecError::NotFinite { at });
            }
        }
        Ok(())
    }

    /// [`Quantizer::scale_for`] then [`Quantizer::quantize`]; returns the scale used.
    pub fn quantize_auto(&self, x: &[f32], out: &mut [Llr8]) -> Result<f32> {
        let s = self.scale_for(x);
        self.quantize(x, s, out)?;
        Ok(s)
    }
}

/// Checks that every value is finite; `Err(NotFinite)` names the first that is not.
pub fn check_finite(x: &[f32]) -> Result<()> {
    match x.iter().position(|v| !v.is_finite()) {
        Some(at) => Err(FecError::NotFinite { at }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_is_the_sign_bit() {
        assert_eq!(hard(5), 0);
        assert_eq!(hard(0), 0);
        assert_eq!(hard(-1), 1);
        assert_eq!(hard(-127), 1);
        assert_eq!(hard_f32(-0.1), 1);
        assert_eq!(hard_f32(f32::NAN), 0);
        assert_eq!(from_hard(1), -127);
        assert_eq!(add(100, 100), 127);
        assert_eq!(add(-100, -100), -127);
    }

    #[test]
    fn quantize_bulk_and_tail_agree_and_clip() {
        let q = Quantizer {
            target: 20.0,
            limit: 31,
            max_scale: 100.0,
        };
        for len in [5usize, 16, 33, 200] {
            let x: Vec<f32> = (0..len).map(|i| (i as f32 * 7.3 - 200.0) * 0.5).collect();
            let mut a = vec![0i8; len];
            q.quantize(&x, 1.0, &mut a).unwrap();
            for (v, &o) in x.iter().zip(&a) {
                assert_eq!(o as f32, v.round().clamp(-31.0, 31.0));
            }
        }
    }

    #[test]
    fn scale_hits_the_target_and_nan_is_reported() {
        let q = Quantizer::default();
        let x = [2.0f32, -2.0, 0.0, 2.0];
        assert!((q.scale_for(&x) - 10.0).abs() < 1e-6);
        let mut out = [0i8; 4];
        assert_eq!(
            q.quantize(&[1.0, f32::NAN, -1.0, 0.0], 10.0, &mut out),
            Err(FecError::NotFinite { at: 1 })
        );
        assert_eq!(out, [10, 0, -10, 0]);
        assert_eq!(q.scale_for(&[0.0; 3]), 1000.0);
        // Zeros, infinities and NaN do not count, in the bulk and in the tail.
        let mut y = vec![0.0f32; 37];
        for (i, v) in y.iter_mut().enumerate() {
            *v = match i % 6 {
                0 => 4.0,
                1 => -4.0,
                2 => 0.0,
                3 => f32::NAN,
                4 => f32::INFINITY,
                _ => -0.0,
            };
        }
        assert_eq!(q.scale_for(&y), 5.0);
        let mut out = [1i8; 37];
        assert_eq!(
            q.quantize(&y, 2.0, &mut out),
            Err(FecError::NotFinite { at: 3 })
        );
        for (v, &o) in y.iter().zip(&out) {
            let want = if v.is_nan() {
                0.0
            } else {
                (v * 2.0).clamp(-127.0, 127.0)
            };
            assert_eq!(o as f32, want);
        }
        // An infinity times a scale of 0 is not a number: an erasure.
        q.quantize(&[f32::INFINITY; 20], 0.0, &mut out).unwrap();
        assert_eq!(out[..20], [0i8; 20]);
        assert!(q.quantize(&x, 1.0, &mut [0i8; 2]).is_err());
    }
}

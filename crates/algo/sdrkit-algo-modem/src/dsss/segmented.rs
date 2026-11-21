//! Code phase search of a long complex code: coherent over short segments of the code, non-coherent across them, with carrier
//! hypotheses as shifts of one spectrum. A segment short against the carrier offset keeps the coherent loss small, so the
//! search covers a wide carrier range with few hypotheses, and every code phase keeps its own best hypothesis (several
//! transmitters of one code at different offsets and carriers, as the pilots of cdma2000 base stations).
//!
//! The input holds two code periods. Segment `s` of the code (samples `s L .. (s + 1) L` of one period, the rest zero) is
//! correlated with the input at every lag `0 .. period` by one FFT product; the correlation of segment `s` at lag `d` is
//! `sum_n x(d + n) conj(c(n))` over the samples of the segment. A carrier hypothesis `h` moves the input spectrum by `h` steps
//! of `step_bins` bins. The power of each segment adds into its hypothesis, and each lag keeps the best hypothesis.
//!
//! ```
//! use sdrkit_algo_modem::dsss::segmented::SegmentedSearch;
//! use sdrkit_algo_modem::Complex32;
//! // A code of 256 random complex chips, the input the code delayed by 40 samples.
//! let mut s = 7u32;
//! let code: Vec<Complex32> = (0..256)
//!     .map(|_| {
//!         s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
//!         Complex32::new(if s & 0x10000 == 0 { 1.0 } else { -1.0 }, if s & 0x20000 == 0 { 1.0 } else { -1.0 })
//!     })
//!     .collect();
//! let x: Vec<Complex32> = (0..512).map(|n| code[(n + 256 - 40) % 256]).collect();
//! let mut search = SegmentedSearch::new(&code, 64).unwrap();
//! let p = search.search(&x, 4, 1).unwrap();
//! let best = (0..256).max_by(|&a, &b| p.power[a].total_cmp(&p.power[b])).unwrap();
//! assert_eq!(best, 40);
//! assert_eq!(p.hypothesis[best], 0);
//! ```

use super::acquire::shift_product;
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;

/// The best power of every code phase over the carrier hypotheses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SegmentPowers {
    /// The summed segment powers at the best hypothesis, one per lag `0 .. period`.
    pub power: Vec<f32>,
    /// The best hypothesis of each lag, `-half .. half` (steps of `step_bins` bins).
    pub hypothesis: Vec<i16>,
}

/// The search of one code (module documentation). It keeps its FFT plans and buffers, so a repeated search allocates
/// nothing.
pub struct SegmentedSearch {
    code: Vec<Complex32>,
    seg_len: usize,
    fwd: Fft,
    inv: Fft,
    spec: Vec<Complex32>,
    seg: Vec<Complex32>,
    prod: Vec<Complex32>,
    acc: Vec<f32>,
    out: SegmentPowers,
}

impl SegmentedSearch {
    /// A search of the code `code` (one period, one sample per chip or any fixed rate) in segments of `seg_len` samples.
    ///
    /// # Errors
    ///
    /// [`ModemError`] when the code is shorter than 8 samples or `seg_len` is 0 or does not divide the period.
    pub fn new(code: &[Complex32], seg_len: usize) -> Result<Self, ModemError> {
        let p = code.len();
        Param::Period.check_count(p, 8, 1 << 22)?;
        Param::Window.check_count(seg_len, 1, p)?;
        if !p.is_multiple_of(seg_len) {
            return Param::Window.conflict(Param::Period);
        }
        let n = 2 * p;
        Ok(SegmentedSearch {
            code: code.to_vec(),
            seg_len,
            fwd: Fft::forward(n),
            inv: Fft::inverse(n),
            spec: vec![Complex32::default(); n],
            seg: vec![Complex32::default(); n],
            prod: vec![Complex32::default(); n],
            acc: Vec::new(),
            out: SegmentPowers::default(),
        })
    }

    /// The code period, samples.
    pub fn period(&self) -> usize {
        self.code.len()
    }

    /// The carrier frequency of hypothesis `h`, cycles per sample, for a search with `step_bins`.
    pub fn freq(&self, h: i32, step_bins: usize) -> f64 {
        f64::from(h) * step_bins as f64 / (2 * self.code.len()) as f64
    }

    /// Searches the first two periods of `x` over the hypotheses `-half ..= half`, each `step_bins` bins (of `1 / (2 period)`
    /// cycles per sample) apart. The powers are scaled so that an input equal to the code, aligned and without noise, gives 1 at its
    /// lag.
    ///
    /// # Errors
    ///
    /// [`ModemError`] when `x` holds less than two periods or the hypotheses reach half the sample rate.
    pub fn search(
        &mut self,
        x: &[Complex32],
        step_bins: usize,
        half: usize,
    ) -> Result<&SegmentPowers, ModemError> {
        let p = self.code.len();
        let n = 2 * p;
        if x.len() < n {
            return Err(ModemError::Length {
                param: Param::Input,
                expected: n,
                got: x.len(),
            });
        }
        let hyps = 2 * half + 1;
        Param::Window.check_count(step_bins * half, 0, p - 1)?;
        self.spec.copy_from_slice(&x[..n]);
        self.fwd.process(&mut self.spec);
        self.acc.clear();
        self.acc.resize(hyps * p, 0.0);
        let segs = p / self.seg_len;
        let pc = self.code.iter().map(|c| c.norm_sqr()).sum::<f32>() / p as f32;
        let scale = 1.0
            / (segs as f32 * (self.seg_len * self.seg_len) as f32 * n as f32 * n as f32 * pc * pc).max(f32::MIN_POSITIVE);
        for s in 0..segs {
            self.seg.fill(Complex32::default());
            let r = s * self.seg_len..(s + 1) * self.seg_len;
            self.seg[r.clone()].copy_from_slice(&self.code[r]);
            self.fwd.process(&mut self.seg);
            self.seg.iter_mut().for_each(|c| *c = c.conj());
            for h in 0..hyps {
                let shift = ((h as i64 - half as i64) * step_bins as i64).rem_euclid(n as i64) as usize;
                shift_product(&self.spec, shift, &self.seg, &mut self.prod, false);
                self.inv.process(&mut self.prod);
                let acc = &mut self.acc[h * p..(h + 1) * p];
                for (a, y) in acc.iter_mut().zip(&self.prod[..p]) {
                    *a += y.norm_sqr() * scale;
                }
            }
        }
        self.out.power.clear();
        self.out.power.resize(p, 0.0);
        self.out.hypothesis.clear();
        self.out.hypothesis.resize(p, 0);
        for h in 0..hyps {
            let acc = &self.acc[h * p..(h + 1) * p];
            let hv = (h as i64 - half as i64) as i16;
            for ((best, bh), &v) in self
                .out
                .power
                .iter_mut()
                .zip(self.out.hypothesis.iter_mut())
                .zip(acc)
            {
                if v > *best {
                    *best = v;
                    *bh = hv;
                }
            }
        }
        Ok(&self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn finds_two_codes_at_two_carriers() {
        let p = 4096;
        let mut s = 11u64;
        let code: Vec<Complex32> = (0..p)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                Complex32::new(
                    if s & 1 == 0 { 1.0 } else { -1.0 },
                    if s & 2 == 0 { 1.0 } else { -1.0 },
                )
            })
            .collect();
        // Two copies: lag 100 at +3 steps, lag 2000 at -2 steps (step 8 bins of 1/8192).
        let f1 = 3.0 * 8.0 / 8192.0;
        let f2 = -2.0 * 8.0 / 8192.0;
        let x: Vec<Complex32> = (0..2 * p)
            .map(|n| {
                let a = code[(n + p - 100) % p] * Complex32::from_polar(1.0, (TAU * f1 * n as f64) as f32);
                let b = code[(n + p - 2000) % p] * Complex32::from_polar(0.5, (TAU * f2 * n as f64) as f32);
                a + b
            })
            .collect();
        let mut search = SegmentedSearch::new(&code, 256).unwrap();
        let r = search.search(&x, 8, 4).unwrap();
        assert_eq!(r.hypothesis[100], 3);
        assert_eq!(r.hypothesis[2000], -2);
        assert!(r.power[100] > 0.8, "{}", r.power[100]);
        assert!(r.power[2000] > 0.2 && r.power[2000] < 0.3, "{}", r.power[2000]);
        let floor = r.power[500];
        assert!(floor < 0.05, "{floor}");
    }
}

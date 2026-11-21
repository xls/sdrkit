//! Soft demapping of square QAM whose axes are reflected-Gray PAM, in a few operations per bit at any size (16- to 4096-QAM
//! and above): the label bits alternate between the axes (`y_0` real, `y_1` imaginary, ...); on each axis the first bit is the
//! sign (1 negative) and the others are the reflected Gray code of the distance from the outermost level. This is
//! `QamMap::DVB` of `sdrkit_algo_dsp::constellation` (DVB-C2, DVB-T2, DVB-S2X and others).
//!
//! The LLR of axis bit `k` is `4 a^2 w t_k` with `t_0 = u`, `t_k = |t_(k-1)| - 2^(b-k)` for the axis value `u` in units of the
//! half step `a` (levels at odd `u`) and the cell weight `w = |H|^2 / sigma^2`. It equals the max-log LLR between the two levels
//! next to each decision boundary and grows linearly beyond them, where the true max-log grows faster; the sign is always the
//! max-log sign. Positive means 0.

use crate::{Complex32, ModemError, Param};

/// The demapper of one square Gray QAM at unit mean power.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrayQam {
    /// Bits per axis.
    bits: usize,
    /// 1 / a, the levels in units of the half step.
    inv_a: f32,
    /// 4 a^2.
    four_a2: f32,
}

impl GrayQam {
    /// The demapper of square QAM of `points` points (4, 16, 64, ... 2^24). Errors for another number.
    pub fn new(points: usize) -> Result<GrayQam, ModemError> {
        let bits = points.trailing_zeros() as usize;
        if !points.is_power_of_two() || !bits.is_multiple_of(2) || !(2..=24).contains(&bits) {
            return Param::Points.not_allowed(points as f64);
        }
        // Unit mean power: levels a (2j + 1 - M) on each axis of M = 2^(bits / 2) levels.
        let a = (1.5 / (points as f64 - 1.0)).sqrt();
        Ok(GrayQam {
            bits: bits / 2,
            inv_a: (1.0 / a) as f32,
            four_a2: (4.0 * a * a) as f32,
        })
    }

    /// Bits per cell.
    pub fn bits(&self) -> usize {
        2 * self.bits
    }

    /// The LLRs of the cells `z` with weights `w` into `out` (`bits()` per cell, `y_0` first). Errors on short inputs or output.
    pub fn demap_block(&self, z: &[Complex32], w: &[f32], out: &mut [f32]) -> Result<(), ModemError> {
        let m = self.bits();
        if w.len() < z.len() {
            return Err(ModemError::InputTooShort { need: z.len() });
        }
        if out.len() < z.len() * m {
            return Err(ModemError::OutputTooShort { need: z.len() * m });
        }
        let b = self.bits;
        let top = (1u32 << (b - 1)) as f32;
        for ((c, &wt), o) in z.iter().zip(w).zip(out.chunks_exact_mut(m)) {
            let s = self.four_a2 * wt;
            let (mut tr, mut ti) = (c.re * self.inv_a, c.im * self.inv_a);
            o[0] = s * tr;
            o[1] = s * ti;
            let mut half = top;
            for k in 1..b {
                tr = tr.abs() - half;
                ti = ti.abs() - half;
                // A bit is 0 on the outer side of its boundary.
                o[2 * k] = s * tr;
                o[2 * k + 1] = s * ti;
                half *= 0.5;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Constellation, Demap};
    use sdrkit_algo_dsp::constellation::{PointSet, QamMap};

    #[test]
    fn signs_match_max_log_and_boundaries_match_its_values() {
        for points in [16usize, 64, 256, 1024, 4096] {
            let set = PointSet::qam(points, QamMap::DVB).unwrap();
            let c = Constellation::new(set.clone()).unwrap();
            let g = GrayQam::new(points).unwrap();
            let m = g.bits();
            let mut fast = vec![0f32; m];
            let mut exact = vec![0f32; m];
            // Every point, nudged towards the centre, and points between neighbours.
            for (label, p) in set.points().iter().enumerate() {
                let z = *p * 0.999 + Complex32::new(1e-4, -2e-4);
                g.demap_block(&[z], &[50.0], &mut fast).unwrap();
                c.demap_weighted(z, 50.0, Demap::MaxLog, &mut exact).unwrap();
                for k in 0..m {
                    let bit = (label >> (m - 1 - k)) & 1;
                    assert_eq!(fast[k] < 0.0, bit == 1, "{points} label {label} bit {k}");
                    assert_eq!(fast[k] < 0.0, exact[k] < 0.0);
                    // The weakest bit sits next to its boundary: the values agree.
                    if k >= m - 2 {
                        assert!((fast[k] - exact[k]).abs() <= 1e-3 * exact[k].abs().max(1.0), "{points} {label} {k}");
                    }
                }
            }
        }
    }
}

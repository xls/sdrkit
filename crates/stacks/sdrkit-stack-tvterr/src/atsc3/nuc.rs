//! The constellations of 1024 and 4096 points: one non-uniform PAM, the same on both axes, mapped, decided and demapped
//! per axis.
//!
//! Standard: ATSC A/322:2026-06 clause 6.3.4.3, tables C.1.8 to C.1.11 and C.3.1 to C.3.4.

use super::tables::{LABELS1024, LABELS4096, NUC1024, NUC4096};
use sdrkit_core::Complex32;

/// A level far from every magnitude: the side of a bit value that has no magnitude there.
const FAR: f32 = 1e15;

/// The constellation of 1024 or 4096 points of one code rate. Bit `y0` of a cell word comes first; the real part takes the
/// odd bits `y1, y3, ...` and the imaginary part the even bits `y0, y2, ...`. The first bit of each axis is the sign (1:
/// negative) and the others select the magnitude `u_k` by table C.3. The levels are those printed, whose mean power is 1
/// within 1e-4.
///
/// The max-log LLRs of one cell cost per axis a binary search among the 16 or 32 magnitudes (4 or 5 comparisons), four
/// squared distances per magnitude bit and one more for the sign bit: 17 squared distances per axis for 1024 points and
/// 21 for 4096 points, against 1024 or 4096 for a search over every point.
#[derive(Clone, Debug)]
pub struct AxisPam {
    /// Bits per axis: 5 or 6.
    axis_bits: usize,
    /// `u_k` by `k`.
    levels: &'static [f32],
    /// The `k` of each magnitude bit value.
    by_label: Vec<u8>,
    /// The magnitudes in increasing order.
    sorted: Vec<f32>,
    /// Per gap `g` (above `g` sorted magnitudes) and magnitude bit `j` at `g * (axis_bits - 1) + j`: for bit value `v`,
    /// the nearest magnitude with that value below the gap and above it, [`FAR`] away when there is none.
    near: Vec<[[f32; 2]; 2]>,
}

impl AxisPam {
    /// The constellation of `bits` bits per cell (10 or 12) at code rate `rate_fifteenths / 15` (2 to 13). `None` for
    /// another size or rate.
    pub fn new(bits: usize, rate_fifteenths: u8) -> Option<AxisPam> {
        let r = usize::from(rate_fifteenths).checked_sub(2).filter(|&r| r < 12)?;
        let (levels, labels): (&'static [f32], &'static [u8]) = match bits {
            10 => (&NUC1024[r], &LABELS1024),
            12 => (&NUC4096[r], &LABELS4096),
            _ => return None,
        };
        let axis_bits = bits / 2;
        let mut by_label = vec![0u8; levels.len()];
        for (k, &l) in labels.iter().enumerate() {
            by_label[usize::from(l)] = k as u8;
        }
        let mut sorted = levels.to_vec();
        sorted.sort_by(f32::total_cmp);
        let mbits = axis_bits - 1;
        let mut near = Vec::with_capacity((sorted.len() + 1) * mbits);
        for g in 0..=sorted.len() {
            for j in 0..mbits {
                let shift = mbits - 1 - j;
                let mut sides = [[-FAR, FAR]; 2];
                for (k, (&u, &l)) in levels.iter().zip(labels).enumerate() {
                    let v = usize::from(l >> shift & 1);
                    // Equal magnitudes keep their sorted order: the place of `u_k` among them is its rank by `k`.
                    let rank = sorted.partition_point(|&s| s < u)
                        + levels[..k].iter().filter(|&&x| x == u).count();
                    if rank < g {
                        sides[v][0] = sides[v][0].max(u);
                    } else {
                        sides[v][1] = sides[v][1].min(u);
                    }
                }
                near.push(sides);
            }
        }
        Some(AxisPam {
            axis_bits,
            levels,
            by_label,
            sorted,
            near,
        })
    }

    /// Bits per cell.
    pub fn bits(&self) -> usize {
        2 * self.axis_bits
    }

    /// The level of one axis for its bits `b` (sign first).
    fn level(&self, b: impl Iterator<Item = u8>) -> f32 {
        let mut word = 0usize;
        for x in b {
            word = word << 1 | usize::from(x & 1);
        }
        let mbits = self.axis_bits - 1;
        let u = self.levels[usize::from(self.by_label[word & ((1 << mbits) - 1)])];
        if word >> mbits & 1 == 1 {
            -u
        } else {
            u
        }
    }

    /// Maps hard bits (0 or 1 per byte, whole cell words) to cells, appending to `out`. False when the bits do not fill
    /// whole cells.
    pub fn map_bits(&self, bits: &[u8], out: &mut Vec<Complex32>) -> bool {
        let m = self.bits();
        if !bits.len().is_multiple_of(m) {
            return false;
        }
        out.extend(bits.chunks_exact(m).map(|y| {
            Complex32::new(
                self.level(y.iter().skip(1).step_by(2).copied()),
                self.level(y.iter().step_by(2).copied()),
            )
        }));
        true
    }

    /// The nearest level of one axis to `x`.
    fn nearest_level(&self, x: f32) -> f32 {
        let t = x.abs();
        let g = self.sorted.partition_point(|&s| s < t);
        let below = g.checked_sub(1).map(|i| self.sorted[i]);
        let above = self.sorted.get(g).copied();
        let u = match (below, above) {
            (Some(b), Some(a)) if t - b <= a - t => b,
            (_, Some(a)) => a,
            (Some(b), None) => b,
            (None, None) => 0.0,
        };
        if x < 0.0 {
            -u
        } else {
            u
        }
    }

    /// The point nearest `z`.
    pub fn nearest(&self, z: Complex32) -> Complex32 {
        Complex32::new(self.nearest_level(z.re), self.nearest_level(z.im))
    }

    /// The max-log LLRs of the bits of one axis at `x` with weight `w` into `out[first]`, `out[first + 2]`, and so on
    /// (positive means 0).
    #[inline]
    fn axis_llr(&self, x: f32, w: f32, out: &mut [f32], first: usize) {
        let t = x.abs();
        let g = self.sorted.partition_point(|&s| s < t);
        let mbits = self.axis_bits - 1;
        let sides = &self.near[g * mbits..(g + 1) * mbits];
        let mut nearest = f32::INFINITY;
        for (j, s) in sides.iter().enumerate() {
            let d = |v: usize| {
                let (a, b) = (t - s[v][0], t - s[v][1]);
                (a * a).min(b * b)
            };
            let (d0, d1) = (d(0), d(1));
            nearest = nearest.min(d0.min(d1));
            out[first + 2 * (j + 1)] = (d1 - d0) * w;
        }
        // The sign: the nearest level of the sign of `x`, and the smallest magnitude of the other sign.
        let e = t + self.sorted[0];
        let flip = (e * e - nearest) * w;
        out[first] = if x < 0.0 { -flip } else { flip };
    }

    /// Demaps equalised cells, each with its weight `|H|^2 / sigma^2`, into `out` (`bits` LLRs per cell, `y0` first,
    /// positive means 0). False when a length is wrong or a value is not finite.
    pub fn demap(&self, cells: &[Complex32], weights: &[f32], out: &mut [f32]) -> bool {
        let m = self.bits();
        if weights.len() != cells.len() || out.len() != cells.len() * m {
            return false;
        }
        for ((z, &w), o) in cells.iter().zip(weights).zip(out.chunks_exact_mut(m)) {
            if !(z.re.is_finite() && z.im.is_finite() && w.is_finite() && w >= 0.0) {
                return false;
            }
            self.axis_llr(z.re, w, o, 1);
            self.axis_llr(z.im, w, o, 0);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every level of every rate, its power and the worked example of clause 6.3.4.3.1.
    #[test]
    fn the_levels_and_labels_are_those_printed() {
        for bits in [10, 12] {
            for rate in 2..=13 {
                let c = AxisPam::new(bits, rate).unwrap();
                let n = c.levels.len() as f64;
                let power = 2.0 * c.levels.iter().map(|&u| f64::from(u * u)).sum::<f64>() / n;
                assert!((power - 1.0).abs() < 1e-4, "{bits} bits {rate}/15: {power}");
                // The labels of neighbouring magnitudes by `k` differ in one bit, and each label is used once.
                let labels: &[u8] = if bits == 10 { &LABELS1024 } else { &LABELS4096 };
                assert!(labels.windows(2).all(|w| (w[0] ^ w[1]).count_ones() == 1));
                let mut seen = labels.to_vec();
                seen.sort_unstable();
                assert!(seen.iter().enumerate().all(|(k, &l)| usize::from(l) == k));
            }
        }
        assert!(AxisPam::new(8, 6).is_none() && AxisPam::new(10, 14).is_none() && AxisPam::new(12, 1).is_none());
        // 1024 points 6/15, cell word 0010011100: the imaginary part 01010 is u3, the real part 00110 is u11.
        let c = AxisPam::new(10, 6).unwrap();
        let mut out = Vec::new();
        assert!(c.map_bits(&[0, 0, 1, 0, 0, 1, 1, 1, 0, 0], &mut out));
        assert_eq!(out, [Complex32::new(0.7196, 0.1295)]);
        // A sign bit of 1 is a negative level.
        out.clear();
        assert!(c.map_bits(&[1, 1, 1, 0, 0, 1, 1, 1, 0, 0], &mut out));
        assert_eq!(out, [Complex32::new(-0.7196, -0.1295)]);
        assert!(!c.map_bits(&[0; 9], &mut out));
    }

    /// The max-log LLRs per axis equal those of a search over every point, and the hard decisions give the bits back.
    #[test]
    fn the_axis_llrs_are_those_of_every_point() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
        for (bits, rate) in [(10, 2), (10, 9), (12, 5), (12, 13)] {
            let c = AxisPam::new(bits, rate).unwrap();
            let words: Vec<u8> = (0..bits << bits).map(|i| ((i / bits) >> (bits - 1 - i % bits) & 1) as u8).collect();
            let mut points = Vec::new();
            assert!(c.map_bits(&words, &mut points));
            for trial in 0..200 {
                let z = points[trial * 37 % points.len()] + rng.complex_normal(0.01);
                let w = 3.0;
                let mut llr = vec![0f32; bits];
                assert!(c.demap(&[z], &[w], &mut llr));
                for (b, &l) in llr.iter().enumerate() {
                    let side = |v: u8| {
                        points
                            .iter()
                            .enumerate()
                            .filter(|&(p, _)| words[p * bits + b] == v)
                            .map(|(_, p)| (z - p).norm_sqr())
                            .fold(f32::INFINITY, f32::min)
                    };
                    let want = (side(1) - side(0)) * w;
                    assert!((l - want).abs() <= 1e-3 * want.abs().max(1.0), "{bits} {rate} bit {b}: {l} {want}");
                }
                let p = c.nearest(z);
                let best = points.iter().map(|q| (z - q).norm_sqr()).fold(f32::INFINITY, f32::min);
                assert!(((z - p).norm_sqr() - best).abs() < 1e-6);
            }
        }
    }
}

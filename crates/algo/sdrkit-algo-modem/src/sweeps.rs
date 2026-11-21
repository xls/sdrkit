//! Preambles of linear frequency sweeps: a sweep added to a buffer with raised-cosine edges, and the search for a group of
//! rising and falling sweeps in a stream, which gives the start of the group and the carrier offset together.
//!
//! A carrier offset of `df` Hz moves the correlation peak of a sweep of slope `k` Hz/s by `-df rate / k` samples, so rising and
//! falling sweeps of one slope move the opposite ways. [`SweepSearch::sums`] correlates the stream with every sweep of a group
//! (each alone, start phase 0, through [`TemplateBank`]) and adds the correlation powers, each over its template's energy, at
//! the sweep's offset from the group start: `A[p]` of the rising sweeps and `B[q]` of the falling ones. [`pair_max`] then
//! takes `T[x] = max over s of A[x - s] + B[x + s]`; the peaks `p` of `A` and `q` of `B` near the best pair give the start
//! `(p + q) / 2` and the offset `(q - p) / 2 · |k| / rate` ([`SweepSearch::offset_hz`]). The powers add without phase, so the
//! search holds on a channel that fades between the sweeps.
//!
//! ```
//! use sdrkit_algo_modem::sweeps::{pair_max, peak_near, Sweep, SweepSearch};
//! use sdrkit_algo_modem::Complex32;
//! let rate = 2400.0;
//! let group = [
//!     Sweep { start: 0, len: 2400, start_hz: -1200.0, slope_hz_s: 1600.0, phase: 0.0 },
//!     Sweep { start: 600, len: 2400, start_hz: 1200.0, slope_hz_s: -1600.0, phase: 1.0 },
//! ];
//! let mut x = vec![Complex32::new(0.0, 0.0); 6000];
//! for s in &group {
//!     s.add_to(rate, 10, &mut x[1000..]);
//! }
//! let mut search = SweepSearch::new(rate, &group, 10).unwrap();
//! let (a, b) = search.sums(&x);
//! let t = pair_max(&a, &b, 20);
//! let best = (0..t.len()).max_by(|&i, &j| t[i].0.total_cmp(&t[j].0)).unwrap();
//! assert_eq!(best + 20, 1000);
//! let p = peak_near(&a, (best + 20) as i64 - i64::from(t[best].1), 4).unwrap();
//! assert!((p - 1000.0).abs() < 0.1);
//! ```

use crate::template_bank::TemplateBank;
use crate::{Complex32, ModemError, Param};
use std::f64::consts::{PI, TAU};

/// One linear sweep of a preamble, in samples of the stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    /// The first sample, from the start of the group.
    pub start: usize,
    /// The length, samples.
    pub len: usize,
    /// The frequency at the first sample, Hz from the centre.
    pub start_hz: f64,
    /// The slope, Hz a second; negative for a falling sweep, 0 for a tone.
    pub slope_hz_s: f64,
    /// The phase at the first sample, radians.
    pub phase: f64,
}

impl Sweep {
    /// The amplitude of sample `i` of a sweep of `len` samples with raised-cosine edges of `ramp` samples:
    /// `0.5 (1 - cos(pi i / ramp))` for the first `ramp` samples, the mirror of it for the last `ramp`, else 1.
    pub fn edge(i: usize, len: usize, ramp: usize) -> f32 {
        let from_end = len - 1 - i;
        if i < ramp {
            (0.5 * (1.0 - (PI * i as f64 / ramp as f64).cos())) as f32
        } else if from_end < ramp {
            (0.5 * (1.0 - (PI * from_end as f64 / ramp as f64).cos())) as f32
        } else {
            1.0
        }
    }

    /// Adds the sweep at unit amplitude to `out` (whose sample 0 is the start of the group) at `rate`, with edges of `ramp`
    /// samples: sample `start + i` gains `edge(i) exp(j (phase + 2 pi (start_hz t + slope t^2 / 2)))`, `t = i / rate`, the phase
    /// computed in `f64`. Samples past the end of `out` are left out.
    pub fn add_to(&self, rate: f64, ramp: usize, out: &mut [Complex32]) {
        for i in 0..self.len {
            let Some(o) = out.get_mut(self.start + i) else {
                break;
            };
            *o += self.sample(i, rate, ramp, self.phase);
        }
    }

    fn sample(&self, i: usize, rate: f64, ramp: usize, phase: f64) -> Complex32 {
        let t = i as f64 / rate;
        let ph = phase + TAU * (self.start_hz * t + 0.5 * self.slope_hz_s * t * t);
        Complex32::new(ph.cos() as f32, ph.sin() as f32) * Self::edge(i, self.len, ramp)
    }
}

/// The search for one group of sweeps (module documentation). It keeps the correlator.
pub struct SweepSearch {
    rate: f64,
    bank: TemplateBank,
    /// The start of each sweep in the group.
    offsets: Vec<usize>,
    /// The sweep rises.
    rising: Vec<bool>,
    /// The energy of each template.
    energy: Vec<f32>,
    span: usize,
    slope: f64,
}

impl SweepSearch {
    /// A search for `group` at `rate`, the templates with edges of `ramp` samples. Every sweep of the group has the same
    /// slope magnitude; at least one rises and one falls.
    ///
    /// # Errors
    ///
    /// [`ModemError`] for a rate that is not positive, a group without a rising or a falling sweep, a sweep of no samples, or
    /// sweeps of different slope magnitudes.
    pub fn new(rate: f64, group: &[Sweep], ramp: usize) -> Result<Self, ModemError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        let slope = group.first().map_or(0.0, |s| s.slope_hz_s.abs());
        if slope <= 0.0
            || group.iter().any(|s| s.len == 0 || (s.slope_hz_s.abs() - slope).abs() > 1e-9 * slope)
            || !group.iter().any(|s| s.slope_hz_s > 0.0)
            || !group.iter().any(|s| s.slope_hz_s < 0.0)
        {
            return Param::Template.degenerate();
        }
        let templates: Vec<Vec<Complex32>> =
            group.iter().map(|s| (0..s.len).map(|i| s.sample(i, rate, ramp, 0.0)).collect()).collect();
        let longest = templates.iter().map(Vec::len).max().unwrap_or(1);
        let energy = templates.iter().map(|t| t.iter().map(|v| v.norm_sqr()).sum::<f32>().max(1e-12)).collect();
        let fft = (4 * longest).next_power_of_two();
        Ok(SweepSearch {
            rate,
            bank: TemplateBank::new(&templates, fft)?,
            offsets: group.iter().map(|s| s.start).collect(),
            rising: group.iter().map(|s| s.slope_hz_s > 0.0).collect(),
            energy,
            span: group.iter().map(|s| s.start + s.len).max().unwrap_or(0).max(longest),
            slope,
        })
    }

    /// Samples from a group start to the end of its last sweep: [`SweepSearch::sums`] gives a value for every start whose
    /// group ends inside the input.
    pub fn span(&self) -> usize {
        self.span
    }

    /// Samples that a carrier offset of 1 Hz moves a peak by: `rate / |slope|`.
    pub fn samples_per_hz(&self) -> f64 {
        self.rate / self.slope
    }

    /// The carrier offset, Hz, of a rising peak at `p` and a falling peak at `q` of one group.
    pub fn offset_hz(&self, p: f64, q: f64) -> f64 {
        (q - p) / 2.0 / self.samples_per_hz()
    }

    /// The rising sums `A` and the falling sums `B` of `x` for the group starts `0 ..= x.len() - span()` (empty when `x` is
    /// shorter): the correlation power of each sweep at its offset from the start, over the template's energy, added up.
    /// For white noise of variance `N` a sum of `n` sweeps has the mean `n N`.
    pub fn sums(&mut self, x: &[Complex32]) -> (Vec<f32>, Vec<f32>) {
        if x.len() < self.span {
            return (Vec::new(), Vec::new());
        }
        let starts = x.len() - self.span + 1;
        let (mut a, mut b) = (vec![0f32; starts], vec![0f32; starts]);
        let (offsets, rising, energy) = (&self.offsets, &self.rising, &self.energy);
        self.bank.process(x, |t, first, c| {
            let sum = if rising[t] { &mut a } else { &mut b };
            let o = offsets[t];
            for (i, v) in c.iter().enumerate() {
                // The correlation at stream position first + i belongs to the group start first + i - o.
                let Some(p) = (first + i).checked_sub(o) else {
                    continue;
                };
                if p < starts {
                    sum[p] += v.norm_sqr() / energy[t];
                }
            }
        });
        (a, b)
    }
}

/// The best pair for each start: `T[x] = max over |s| <= max_shift of A[x - s] + B[x + s]` and its `s`, for `x` from
/// `max_shift` to `len - 1 - max_shift` (entry `i` is start `max_shift + i`). Empty when the sums are shorter than
/// `2 max_shift + 1`.
pub fn pair_max(a: &[f32], b: &[f32], max_shift: usize) -> Vec<(f32, i32)> {
    let n = a.len().min(b.len());
    if n < 2 * max_shift + 1 {
        return Vec::new();
    }
    let m = max_shift as i64;
    (max_shift..n - max_shift)
        .map(|x| {
            let mut best = (f32::NEG_INFINITY, 0i32);
            for s in -m..=m {
                let v = a[(x as i64 - s) as usize] + b[(x as i64 + s) as usize];
                if v > best.0 {
                    best = (v, s as i32);
                }
            }
            best
        })
        .collect()
}

/// The peak of `v` within `search` samples of `at`, refined by a parabola through the square roots of the peak and its
/// neighbours. `None` when no sample of the range lies inside `v`.
pub fn peak_near(v: &[f32], at: i64, search: usize) -> Option<f64> {
    let lo = (at - search as i64).max(0);
    let hi = (at + search as i64).min(v.len() as i64 - 1);
    if lo > hi {
        return None;
    }
    let i = (lo..=hi).max_by(|&i, &j| v[i as usize].total_cmp(&v[j as usize]))? as usize;
    if i == 0 || i + 1 >= v.len() {
        return Some(i as f64);
    }
    let (l, c, r) = (f64::from(v[i - 1]).sqrt(), f64::from(v[i]).sqrt(), f64::from(v[i + 1]).sqrt());
    let d = l - 2.0 * c + r;
    let f = if d < 0.0 { (0.5 * (l - r) / d).clamp(-0.5, 0.5) } else { 0.0 };
    Some(i as f64 + f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group() -> Vec<Sweep> {
        (0..4)
            .flat_map(|k| {
                [
                    Sweep { start: 1200 * k, len: 2400, start_hz: -1200.0 + 266.0 * k as f64, slope_hz_s: 1600.0, phase: 0.3 * k as f64 },
                    Sweep { start: 600 + 1200 * k, len: 2400, start_hz: 1200.0 - 266.0 * k as f64, slope_hz_s: -1600.0, phase: 1.1 },
                ]
            })
            .collect()
    }

    #[test]
    fn start_and_offset_of_a_shifted_group() {
        let rate = 2400.0;
        let g = group();
        let mut search = SweepSearch::new(rate, &g, 10).unwrap();
        for df in [-80.0f64, 0.0, 37.5] {
            let mut x = vec![Complex32::new(0.0, 0.0); 14_000];
            let at = 2345;
            for s in &g {
                s.add_to(rate, 10, &mut x[at..]);
            }
            for (i, v) in x.iter_mut().enumerate() {
                let ph = TAU * df * i as f64 / rate;
                *v *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
            }
            let (a, b) = search.sums(&x);
            let shift = 160;
            let t = pair_max(&a, &b, shift);
            let i = (0..t.len()).max_by(|&i, &j| t[i].0.total_cmp(&t[j].0)).unwrap();
            let (xx, s) = ((i + shift) as i64, i64::from(t[i].1));
            let p = peak_near(&a, xx - s, 4).unwrap();
            let q = peak_near(&b, xx + s, 4).unwrap();
            let start = (p + q) / 2.0;
            assert!((start - at as f64).abs() < 0.3, "{df} Hz: start {start}");
            assert!((search.offset_hz(p, q) - df).abs() < 0.5, "{df} Hz: offset {}", search.offset_hz(p, q));
            if df == 0.0 {
                // Four sweeps of nearly 2400 samples at their peaks.
                let v = a[(xx - s) as usize];
                assert!((v - 4.0 * 2390.0).abs() < 600.0, "{v}");
            }
        }
    }

    #[test]
    fn edges_and_bad_groups() {
        assert_eq!(Sweep::edge(0, 100, 10), 0.0);
        assert_eq!(Sweep::edge(99, 100, 10), 0.0);
        assert_eq!(Sweep::edge(50, 100, 10), 1.0);
        let up = Sweep { start: 0, len: 10, start_hz: 0.0, slope_hz_s: 100.0, phase: 0.0 };
        assert!(SweepSearch::new(2400.0, &[up], 2).is_err());
        assert!(SweepSearch::new(2400.0, &[up, Sweep { slope_hz_s: -50.0, ..up }], 2).is_err());
        assert!(SweepSearch::new(0.0, &[up, Sweep { slope_hz_s: -100.0, ..up }], 2).is_err());
        let mut s = SweepSearch::new(2400.0, &[up, Sweep { slope_hz_s: -100.0, ..up }], 2).unwrap();
        assert_eq!(s.sums(&[Complex32::new(1.0, 0.0); 5]).0.len(), 0);
        assert!(pair_max(&[1.0; 3], &[1.0; 3], 2).is_empty());
        assert_eq!(peak_near(&[], 0, 3), None);
    }
}

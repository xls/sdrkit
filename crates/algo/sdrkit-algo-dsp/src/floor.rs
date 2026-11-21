//! Noise-floor estimators: the mean noise power from values that are mostly noise with some signal among them.
//!
//! - [`BinMedian`]: a quantile (the median by default) of the bins of a power spectrum, divided by the quantile-to-mean ratio of the
//!   bins' distribution, so it reads the **mean** noise power per bin. A bin of complex Gaussian noise is exponential (a chi-square
//!   with 2 degrees of freedom); the average of `K` frames is a chi-square with `2K` degrees of freedom scaled to the same mean, whose
//!   median is `0.693` of the mean for `K = 1` and `0.959` for `K = 8` ([`quantile_to_mean`]). Signals occupying fewer than half the
//!   bins move the median little.
//! - [`ChunkPercentile`]: the same idea on a stream of power values: a quantile of each chunk of `n` values, corrected the same way
//!   and optionally smoothed from chunk to chunk.
//! - [`MinFollower`]: follows the minimum: falls quickly to a lower value, rises slowly (a fixed number of dB per update). It reads
//!   below the mean noise (by how much depends on the spread of the values and the rise rate), so it suits a squelch threshold or a
//!   display floor rather than an SNR.
//!
//! [`FloorTracker`] holds any of the three behind one interface.
//!
//! Every estimator returns at least a minimum power, [`DEFAULT_MIN_FLOOR`] (1e-12 of full-scale power, -120 dB) unless the
//! caller sets another (`set_min`). Digital silence (zeros from an 8-bit source, padding, a filled gap) thus gives a small
//! positive floor: a squelch that needs a floor above zero still opens, and an SNR over the floor stays finite. The stored
//! state is held at the minimum too, so a [`MinFollower`] rises from it.
//!
//! ```
//! use sdrkit_algo_dsp::floor::quantile_to_mean;
//! assert!((quantile_to_mean(1, 0.5).unwrap() - std::f64::consts::LN_2).abs() < 1e-12);
//! ```

use crate::{DspError, Param};

/// The smallest floor an estimator returns unless the caller sets another: 1e-12 of full-scale power (-120 dB).
pub const DEFAULT_MIN_FLOOR: f32 = 1e-12;

/// Errors unless `min` is finite and at least 0.
fn check_min(min: f32) -> Result<(), DspError> {
    if !(min >= 0.0 && min.is_finite()) {
        return Param::Values.not_finite();
    }
    Ok(())
}

/// The `p` quantile of the mean of `frames` independent unit-mean exponential values, i.e. of a chi-square with `2 frames` degrees
/// of freedom divided by `2 frames`: the factor by which the `p` quantile of averaged noise bins lies below their mean. Computed by
/// bisection on the exact CDF `P(K, K x) = 1 - e^(-Kx) sum_{j<K} (Kx)^j / j!` (summed in the log domain, so large `K` works).
/// Errors unless `1 <= frames <= 100000` and `0 < p < 1`.
pub fn quantile_to_mean(frames: u32, p: f64) -> Result<f64, DspError> {
    if !(1..=100_000).contains(&frames) {
        return Param::Frames.range(1.0, 100_000.0, frames as f64);
    }
    if !(p > 0.0 && p < 1.0) {
        return Param::Quantile.range(0.0, 1.0, p);
    }
    let k = frames as f64;
    // Upper tail Q(K, y) = e^-y sum_{j<K} y^j / j!, y = K x.
    let upper = |x: f64| -> f64 {
        let y = k * x;
        if y <= 0.0 {
            return 1.0;
        }
        let ly = y.ln();
        // The log of term j is -y + j ln y - ln j!; find the largest, then sum relative to it.
        let mut lt = -y;
        let mut top = lt;
        for j in 1..frames {
            lt += ly - (j as f64).ln();
            top = top.max(lt);
        }
        lt = -y;
        let mut s = (lt - top).exp();
        for j in 1..frames {
            lt += ly - (j as f64).ln();
            s += (lt - top).exp();
        }
        (top + s.ln()).exp().min(1.0)
    };
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    while 1.0 - upper(hi) < p {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if 1.0 - upper(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < 1e-15 * hi {
            break;
        }
    }
    Ok(0.5 * (lo + hi))
}

/// The expected value of the `r`-th smallest (1-based) of `n` independent unit-mean exponential values: `H(n) - H(n - r)`, with `H`
/// the harmonic numbers. The `r`-th smallest bin of `n` divided by this value is an unbiased mean (an order-statistic floor). Errors
/// unless `1 <= r <= n`.
pub fn exp_order_mean(n: usize, r: usize) -> Result<f64, DspError> {
    if !(1..=n).contains(&r) {
        return Param::Rank.range(1.0, n as f64, r as f64);
    }
    Ok(((n - r + 1)..=n).map(|j| 1.0 / j as f64).sum())
}

/// The `p` quantile of the values, by selection in `scratch` (no allocation once it has grown). `None` when there are none.
fn quantile_in(scratch: &mut [f32], p: f64) -> Option<f32> {
    if scratch.is_empty() {
        return None;
    }
    let k = ((p * (scratch.len() - 1) as f64).round() as usize).min(scratch.len() - 1);
    let (_, v, _) = scratch.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    Some(*v)
}

/// The mean noise power per bin from a quantile of spectrum bins (module docs).
#[derive(Clone, Debug)]
pub struct BinMedian {
    p: f64,
    ratio: f64,
    min: f32,
    scratch: Vec<f32>,
}

impl BinMedian {
    /// For bins that are averages of `frames` power spectra (1 for a single periodogram), reading the median.
    pub fn new(frames: u32) -> Result<Self, DspError> {
        Self::with_quantile(frames, 0.5)
    }

    /// The same reading quantile `p` (a lower quantile, such as 0.25, resists more signal).
    pub fn with_quantile(frames: u32, p: f64) -> Result<Self, DspError> {
        Ok(BinMedian {
            p,
            ratio: quantile_to_mean(frames, p)?,
            min: DEFAULT_MIN_FLOOR,
            scratch: Vec::new(),
        })
    }

    /// Sets the smallest estimate returned (module docs). Errors unless `min` is finite and at least 0.
    pub fn set_min(&mut self, min: f32) -> Result<(), DspError> {
        check_min(min)?;
        self.min = min;
        Ok(())
    }

    /// The smallest estimate returned.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// The quantile-to-mean ratio used.
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Reserves room for `n` bins (so [`estimate`](Self::estimate) never allocates).
    pub fn reserve(&mut self, n: usize) {
        self.scratch.reserve(n.saturating_sub(self.scratch.len()));
    }

    /// The mean noise power per bin of the (linear power) bins given, at least [`min`](Self::min); pass a filtered iterator to
    /// leave out a passband. NaN bins are skipped. `None` when no bin is given.
    pub fn estimate(&mut self, bins: impl IntoIterator<Item = f32>) -> Option<f32> {
        self.scratch.clear();
        self.scratch
            .extend(bins.into_iter().filter(|v| !v.is_nan()));
        let min = self.min;
        quantile_in(&mut self.scratch, self.p).map(|q| ((q as f64 / self.ratio) as f32).max(min))
    }
}

/// A quantile of each chunk of a stream of power values, corrected to a mean and optionally smoothed (module docs).
#[derive(Clone, Debug)]
pub struct ChunkPercentile {
    n: usize,
    p: f64,
    ratio: f64,
    alpha: f64,
    min: f64,
    buf: Vec<f32>,
    floor: Option<f64>,
}

impl ChunkPercentile {
    /// Chunks of `n` values (at least 1), quantile `p`, for values that are averages of `frames` exponential ones (1 for raw power
    /// samples or single bins), smoothed across chunks by `y += alpha (new - y)` (`alpha` in `(0, 1]`; 1 for none).
    pub fn new(n: usize, p: f64, frames: u32, alpha: f64) -> Result<Self, DspError> {
        if n == 0 {
            return Param::BlockLen.at_least(1.0, 0.0);
        }
        if !(alpha > 0.0 && alpha <= 1.0) {
            return Param::Smoothing.range(0.0, 1.0, alpha);
        }
        Ok(ChunkPercentile {
            n,
            p,
            ratio: quantile_to_mean(frames, p)?,
            alpha,
            min: DEFAULT_MIN_FLOOR as f64,
            buf: Vec::with_capacity(n),
            floor: None,
        })
    }

    /// Sets the smallest estimate returned (module docs); a held estimate below it is raised. Errors unless `min` is finite and
    /// at least 0.
    pub fn set_min(&mut self, min: f32) -> Result<(), DspError> {
        check_min(min)?;
        self.min = min as f64;
        if let Some(f) = &mut self.floor {
            *f = f.max(self.min);
        }
        Ok(())
    }

    /// The smallest estimate returned.
    pub fn min(&self) -> f32 {
        self.min as f32
    }

    /// The current estimate.
    pub fn floor(&self) -> Option<f32> {
        self.floor.map(|v| v as f32)
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.floor = None;
    }

    /// Takes one value; returns the new estimate when a chunk completes. NaN values are skipped.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<f32> {
        if x.is_nan() {
            return None;
        }
        self.buf.push(x);
        if self.buf.len() < self.n {
            return None;
        }
        let q = quantile_in(&mut self.buf, self.p).unwrap_or(0.0) as f64 / self.ratio;
        self.buf.clear();
        let f = match self.floor {
            Some(y) => y + self.alpha * (q - y),
            None => q,
        }
        .max(self.min);
        self.floor = Some(f);
        Some(f as f32)
    }
}

/// A minimum follower: drops towards lower values by `attack` of the gap per update (1 jumps at once) and rises by at most
/// `rise_db` per update, never above the value (module docs).
#[derive(Clone, Debug)]
pub struct MinFollower {
    attack: f32,
    rise: f32,
    min: f32,
    floor: Option<f32>,
}

impl MinFollower {
    /// Per-update parameters: `attack` in `(0, 1]`, `rise_db >= 0`.
    pub fn new(attack: f32, rise_db: f32) -> Result<Self, DspError> {
        if !(attack > 0.0 && attack <= 1.0) {
            return Param::Attack.range(0.0, 1.0, attack as f64);
        }
        if !(rise_db >= 0.0 && rise_db.is_finite()) {
            return Param::Rise.at_least(0.0, rise_db as f64);
        }
        Ok(MinFollower {
            attack,
            rise: 10f32.powf(rise_db / 10.0),
            min: DEFAULT_MIN_FLOOR,
            floor: None,
        })
    }

    /// Sets the smallest floor (module docs); a held floor below it is raised. Errors unless `min` is finite and at least 0.
    pub fn set_min(&mut self, min: f32) -> Result<(), DspError> {
        check_min(min)?;
        self.min = min;
        if let Some(f) = &mut self.floor {
            *f = f.max(min);
        }
        Ok(())
    }

    /// The smallest floor.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// From times: `updates_per_s` values a second, falling with time constant `fall_s` (0 for at once), rising at `rise_db_per_s`.
    pub fn with_times(
        updates_per_s: f64,
        fall_s: f64,
        rise_db_per_s: f64,
    ) -> Result<Self, DspError> {
        if updates_per_s <= 0.0 || updates_per_s.is_nan() {
            return Param::Rate.positive(updates_per_s);
        }
        if fall_s < 0.0 || fall_s.is_nan() {
            return Param::FallTime.at_least(0.0, fall_s);
        }
        let attack = if fall_s == 0.0 {
            1.0
        } else {
            1.0 - (-1.0 / (fall_s * updates_per_s)).exp()
        };
        Self::new(attack as f32, (rise_db_per_s / updates_per_s) as f32)
    }

    /// The current floor.
    pub fn floor(&self) -> Option<f32> {
        self.floor
    }

    /// Forgets the floor.
    pub fn reset(&mut self) {
        self.floor = None;
    }

    /// Takes one power value (the first one sets the floor); returns the floor, at least [`min`](Self::min). NaN is ignored.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<f32> {
        if x.is_nan() {
            return self.floor;
        }
        let f = match self.floor {
            None => x,
            Some(f) if x < f => f + self.attack * (x - f),
            Some(f) => (f * self.rise).min(x),
        }
        .max(self.min);
        self.floor = Some(f);
        self.floor
    }
}

/// Any of the three estimators behind one interface.
#[derive(Clone, Debug)]
pub enum FloorTracker {
    /// See [`MinFollower`].
    MinFollower(MinFollower),
    /// See [`ChunkPercentile`].
    ChunkPercentile(ChunkPercentile),
    /// See [`BinMedian`]: each [`update`](FloorTracker::update) is one spectrum.
    BinMedian(BinMedian, Option<f32>),
}

impl FloorTracker {
    /// Feeds values: for the streaming forms, power values in time order; for [`FloorTracker::BinMedian`], the bins of one spectrum.
    /// Returns the current estimate.
    pub fn update(&mut self, values: &[f32]) -> Option<f32> {
        match self {
            FloorTracker::MinFollower(m) => {
                for &v in values {
                    m.push(v);
                }
                m.floor()
            }
            FloorTracker::ChunkPercentile(c) => {
                for &v in values {
                    c.push(v);
                }
                c.floor()
            }
            FloorTracker::BinMedian(b, last) => {
                if let Some(v) = b.estimate(values.iter().copied()) {
                    *last = Some(v);
                }
                *last
            }
        }
    }

    /// Sets the smallest estimate of the estimator held (module docs). Errors unless `min` is finite and at least 0.
    pub fn set_min(&mut self, min: f32) -> Result<(), DspError> {
        match self {
            FloorTracker::MinFollower(m) => m.set_min(min),
            FloorTracker::ChunkPercentile(c) => c.set_min(min),
            FloorTracker::BinMedian(b, last) => {
                b.set_min(min)?;
                if let Some(v) = last {
                    *v = v.max(min);
                }
                Ok(())
            }
        }
    }

    /// The current estimate.
    pub fn floor(&self) -> Option<f32> {
        match self {
            FloorTracker::MinFollower(m) => m.floor(),
            FloorTracker::ChunkPercentile(c) => c.floor(),
            FloorTracker::BinMedian(_, last) => *last,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Rng;

    #[test]
    fn quantile_ratios_match_chi_square_tables() {
        // Exponential (K = 1): the p quantile is -ln(1 - p).
        for p in [0.1, 0.25, 0.5, 0.9] {
            let want = -(1.0f64 - p).ln();
            assert!(
                (quantile_to_mean(1, p).unwrap() - want).abs() < 1e-12,
                "{p}"
            );
        }
        // Median of chi-square with 16 degrees of freedom is 15.3385 (published tables), so the value for K = 8 is 15.3385 / 16 = 0.95866.
        assert!((quantile_to_mean(8, 0.5).unwrap() - 15.3385 / 16.0).abs() < 1e-5);
        // Chi-square 20 dof: 5 % point 10.851, 95 % point 31.410.
        assert!((quantile_to_mean(10, 0.05).unwrap() * 20.0 - 10.851).abs() < 1e-3);
        assert!((quantile_to_mean(10, 0.95).unwrap() * 20.0 - 31.410).abs() < 1e-3);
        // Large K tends to 1 (and does not overflow).
        let big = quantile_to_mean(20_000, 0.5).unwrap();
        assert!((big - 1.0).abs() < 1e-4, "{big}");
        assert!(quantile_to_mean(0, 0.5).is_err());
        assert!(quantile_to_mean(4, 1.0).is_err());
        assert!((exp_order_mean(4, 1).unwrap() - 0.25).abs() < 1e-15);
        assert!((exp_order_mean(3, 3).unwrap() - (1.0 + 0.5 + 1.0 / 3.0)).abs() < 1e-15);
        assert!(exp_order_mean(3, 4).is_err());
    }

    /// Power of complex Gaussian noise of mean `p` (exponential).
    fn noise_power(rng: &mut Rng, p: f64) -> f32 {
        let (a, b) = (rng.gauss(), rng.gauss());
        (p * (a * a + b * b) / 2.0) as f32
    }

    #[test]
    fn bin_median_reads_the_mean_noise() {
        let mut rng = Rng(12345);
        for frames in [1u32, 8] {
            let mut bins = vec![0f32; 16_384];
            for b in bins.iter_mut() {
                let s: f32 = (0..frames).map(|_| noise_power(&mut rng, 2.0)).sum();
                *b = s / frames as f32;
            }
            for p in [0.5, 0.25] {
                let mut m = BinMedian::with_quantile(frames, p).unwrap();
                m.reserve(bins.len());
                let est = m.estimate(bins.iter().copied()).unwrap();
                assert!((est / 2.0 - 1.0).abs() < 0.03, "{frames} {p}: {est}");
            }
            // With 10 % of the bins holding strong signals, averaged bins (narrow distribution) still read within 5 %, and the lower
            // quantile resists better than the median.
            let mut with = bins.clone();
            for b in with.iter_mut().step_by(10) {
                *b += 1000.0;
            }
            let med = BinMedian::new(frames)
                .unwrap()
                .estimate(with.iter().copied())
                .unwrap();
            let low = BinMedian::with_quantile(frames, 0.25)
                .unwrap()
                .estimate(with.iter().copied())
                .unwrap();
            assert!(low / 2.0 - 1.0 < med / 2.0 - 1.0);
            if frames == 8 {
                assert!((low / 2.0 - 1.0).abs() < 0.05, "{low}");
            }
        }
        assert_eq!(
            BinMedian::new(1).unwrap().estimate(std::iter::empty()),
            None
        );
    }

    #[test]
    fn chunk_percentile_and_min_follower() {
        let mut rng = Rng(777);
        let mut c = ChunkPercentile::new(1000, 0.5, 1, 1.0).unwrap();
        let mut last = None;
        for _ in 0..20_000 {
            if let Some(v) = c.push(noise_power(&mut rng, 0.5)) {
                last = Some(v);
            }
        }
        let v = last.unwrap();
        assert!((v / 0.5 - 1.0).abs() < 0.1, "{v}");
        // The min follower drops at once with attack 1 and rises 1 dB per update.
        let mut m = MinFollower::new(1.0, 1.0).unwrap();
        assert_eq!(m.push(10.0), Some(10.0));
        assert_eq!(m.push(1.0), Some(1.0));
        let up = m.push(100.0).unwrap();
        assert!((up - 10f32.powf(0.1)).abs() < 1e-6);
        assert_eq!(m.push(1.1), Some(1.1));
        assert_eq!(m.push(f32::NAN), Some(1.1));
        assert!(MinFollower::new(0.0, 1.0).is_err());
        let t = MinFollower::with_times(100.0, 0.0, 10.0).unwrap();
        assert!((t.rise - 10f32.powf(0.01)).abs() < 1e-6);
        // Behind the common interface.
        let mut f = FloorTracker::BinMedian(BinMedian::new(1).unwrap(), None);
        assert!(f.update(&[1.0, 2.0, 3.0]).is_some());
        let mut f = FloorTracker::MinFollower(MinFollower::new(1.0, 0.0).unwrap());
        assert_eq!(f.update(&[3.0, 2.0, 5.0]), Some(2.0));
    }

    #[test]
    fn digital_silence_gives_the_minimum_floor() {
        let zeros = [0f32; 4096];
        let mut b = BinMedian::new(1).unwrap();
        assert_eq!(b.estimate(zeros.iter().copied()), Some(DEFAULT_MIN_FLOOR));
        let mut c = ChunkPercentile::new(256, 0.5, 1, 0.1).unwrap();
        let mut last = None;
        for &z in &zeros {
            last = c.push(z).or(last);
        }
        assert_eq!(last, Some(DEFAULT_MIN_FLOOR));
        let mut m = MinFollower::new(1.0, 1.0).unwrap();
        for &z in &zeros {
            m.push(z);
        }
        assert_eq!(m.floor(), Some(DEFAULT_MIN_FLOOR));
        // The caller's minimum (1e-10 for a stack with its own rule) and the tracker interface.
        for mut f in [
            FloorTracker::MinFollower(MinFollower::new(0.5, 0.5).unwrap()),
            FloorTracker::ChunkPercentile(ChunkPercentile::new(64, 0.25, 1, 1.0).unwrap()),
            FloorTracker::BinMedian(BinMedian::new(8).unwrap(), None),
        ] {
            assert_eq!(f.update(&zeros), Some(DEFAULT_MIN_FLOOR));
            f.set_min(1e-10).unwrap();
            assert_eq!(f.floor(), Some(1e-10));
            assert_eq!(f.update(&zeros), Some(1e-10));
            assert!(f.set_min(f32::NAN).is_err());
        }
    }

    #[test]
    fn zeros_then_a_burst_open_a_squelch_at_the_burst() {
        // A squelch that opens at 10 dB over a floor that must be positive. A floor of zeros is the minimum, so the squelch opens at
        // the burst, and the minimum follower rises from it.
        let mut m = MinFollower::with_times(1000.0, 0.0, 3.0).unwrap();
        let mut rng = Rng(99);
        let mut opened = None;
        for i in 0..20_000 {
            let x = if i < 10_000 {
                0.0
            } else {
                noise_power(&mut rng, 1e-6)
            };
            let f = m.push(x).unwrap();
            if opened.is_none() && f > 0.0 && x > 10.0 * f {
                opened = Some(i);
            }
        }
        let at = opened.expect("the squelch opens");
        assert!((10_000..10_010).contains(&at), "{at}");
        // The floor rises from the minimum towards the burst's level at 3 dB a second (3 dB per 1000 updates).
        let f = m.floor().unwrap();
        assert!(f > DEFAULT_MIN_FLOOR * 1.5 && f < 1e-6, "{f}");
    }
}

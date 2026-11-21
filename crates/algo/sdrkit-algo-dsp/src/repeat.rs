//! Repetition of events in time: the steady interval of events from one source, and the repetition rate of impulses from all
//! their pair spacings.
//!
//! - [`repeat_period`]: the intervals between consecutive event times; their median is the period when most intervals agree
//!   with it. Events that a receiver missed or added break the rule, so it suits a source whose events are all seen.
//! - [`PairSpacing`]: a histogram of the spacings of every pair of events within a span, smoothed with weights 1, 2, 1. The
//!   strongest bin, or the smallest local maximum below it that holds most of its count (the fundamental), gives the rate.
//!   Missing events and added random events leave the pairs of the steady part in place, so the rate holds where consecutive
//!   intervals fail.

/// A steady interval of events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Period {
    /// The median interval, s.
    pub period_s: f64,
    /// The intervals that agree with it.
    pub good: usize,
    /// All intervals.
    pub intervals: usize,
    /// The RMS of (good interval - period) over the good intervals, divided by the period.
    pub spread: f64,
}

/// The period of the event times `starts` (s, any order): the median `M` of the intervals between consecutive sorted times. It
/// is a period when `M > min_period_s`, and the intervals within `tolerance M` of `M` are at least `min_good` and at least the
/// share `min_share` of all intervals.
pub fn repeat_period(
    starts: &[f64],
    tolerance: f64,
    min_good: usize,
    min_share: f64,
    min_period_s: f64,
) -> Option<Period> {
    if starts.len() < 2 {
        return None;
    }
    let mut t = starts.to_vec();
    t.sort_by(f64::total_cmp);
    let mut iv: Vec<f64> = t.windows(2).map(|w| w[1] - w[0]).collect();
    let n = iv.len();
    let mut sorted = iv.clone();
    sorted.sort_by(f64::total_cmp);
    let m = sorted[n / 2];
    if m.is_nan() || m <= min_period_s {
        return None;
    }
    iv.retain(|&d| (d - m).abs() <= tolerance * m);
    let good = iv.len();
    if good < min_good || (good as f64) < min_share * n as f64 {
        return None;
    }
    let spread = (iv.iter().map(|d| (d - m) * (d - m)).sum::<f64>() / good as f64).sqrt() / m;
    Some(Period {
        period_s: m,
        good,
        intervals: n,
        spread,
    })
}

/// The rules of a [`PairSpacing`] search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingConfig {
    /// The width of a histogram bin, s.
    pub bin_s: f64,
    /// The least spacing counted, s.
    pub min_s: f64,
    /// The span of the histogram: spacings below it are counted, s.
    pub max_s: f64,
    /// The fundamental is searched from this spacing up, s.
    pub fundamental_from_s: f64,
    /// A local maximum is the fundamental when it holds this share of the strongest bin.
    pub fundamental_share: f64,
    /// The strongest bin must hold this many times the mean bin ...
    pub min_ratio: f64,
    /// ... and at least this many pairs.
    pub min_pairs: f64,
    /// The rate is refined from the spacings within this share of the chosen bin's centre.
    pub refine_share: f64,
}

impl SpacingConfig {
    /// Bins of 0.1 ms over 0 to 60 ms counting from 1.5 ms, the fundamental from 3 ms at 60 % of the strongest, the strongest
    /// at 4 times the mean and 12 pairs, refined within 4 %: rates from about 17 to 330 Hz.
    pub const fn impulses() -> Self {
        SpacingConfig {
            bin_s: 1e-4,
            min_s: 1.5e-3,
            max_s: 60e-3,
            fundamental_from_s: 3e-3,
            fundamental_share: 0.6,
            min_ratio: 4.0,
            min_pairs: 12.0,
            refine_share: 0.04,
        }
    }
}

/// A repetition rate found by [`PairSpacing`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Repetition {
    /// The rate, Hz: one over the mean of the spacings near the chosen bin.
    pub rate_hz: f64,
    /// The pairs whose spacing lies near the chosen bin.
    pub pairs: usize,
    /// `pairs / events`, at most 1.
    pub strength: f64,
}

/// The pair-spacing repetition search (module docs).
#[derive(Clone, Debug)]
pub struct PairSpacing {
    cfg: SpacingConfig,
    hist: Vec<f64>,
    smooth: Vec<f64>,
    spacings: Vec<f64>,
}

impl PairSpacing {
    /// A search with the rules `cfg`.
    pub fn new(cfg: SpacingConfig) -> Self {
        let bins = (cfg.max_s / cfg.bin_s).round().max(3.0) as usize;
        PairSpacing {
            cfg,
            hist: vec![0.0; bins],
            smooth: vec![0.0; bins],
            spacings: Vec::new(),
        }
    }

    /// The rules.
    pub fn config(&self) -> &SpacingConfig {
        &self.cfg
    }

    /// The repetition rate of the event times `times` (s, sorted, oldest first), or `None` when no bin stands out.
    pub fn find(&mut self, times: &[f64]) -> Option<Repetition> {
        let c = self.cfg;
        let bins = self.hist.len();
        self.hist.fill(0.0);
        self.spacings.clear();
        for (i, &a) in times.iter().enumerate() {
            for &b in &times[i + 1..] {
                let d = b - a;
                if d >= c.max_s {
                    break;
                }
                if d >= c.min_s {
                    let k = (d / c.bin_s) as usize;
                    if k < bins {
                        self.hist[k] += 1.0;
                        self.spacings.push(d);
                    }
                }
            }
        }
        let pairs = self.spacings.len();
        if pairs == 0 {
            return None;
        }
        for k in 0..bins {
            let l = if k > 0 { self.hist[k - 1] } else { 0.0 };
            let r = if k + 1 < bins { self.hist[k + 1] } else { 0.0 };
            self.smooth[k] = (l + 2.0 * self.hist[k] + r) / 4.0;
        }
        let from = ((c.min_s / c.bin_s).floor() as usize).min(bins - 1);
        let (strongest, &top) = self.smooth[from..]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(&a.0)))
            .map(|(k, v)| (k + from, v))?;
        let mean = pairs as f64 / bins as f64;
        if !(top > c.min_ratio * mean && top >= c.min_pairs) {
            return None;
        }
        let f0 = ((c.fundamental_from_s / c.bin_s).floor() as usize).max(1);
        let s = &self.smooth;
        let chosen = (f0..strongest)
            .find(|&k| {
                k + 1 < bins
                    && s[k] >= s[k - 1]
                    && s[k] >= s[k + 1]
                    && s[k] >= c.fundamental_share * top
            })
            .unwrap_or(strongest);
        let centre = (chosen as f64 + 0.5) * c.bin_s;
        let (mut sum, mut n) = (0.0, 0usize);
        for &d in &self.spacings {
            if (d - centre).abs() <= c.refine_share * centre {
                sum += d;
                n += 1;
            }
        }
        if n == 0 {
            return None;
        }
        Some(Repetition {
            rate_hz: n as f64 / sum,
            pairs: n,
            strength: (n as f64 / times.len().max(1) as f64).min(1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-bit LCG: uniform values in [0, 1).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    #[test]
    fn starts_every_quarter_second_give_the_period_and_scattered_starts_give_none() {
        let mut r = Lcg(5);
        let t: Vec<f64> = (0..24)
            .map(|k| 0.1 + 0.25 * k as f64 * (1.0 + 0.001 * (2.0 * r.next() - 1.0)))
            .collect();
        let p = repeat_period(&t, 0.05, 3, 0.7, 0.002).unwrap();
        assert!((p.period_s - 0.25).abs() < 0.001, "{p:?}");
        assert_eq!((p.good, p.intervals), (23, 23));
        let sensor = [0.7, 1.1, 1.9, 2.0, 3.3, 3.7, 4.4, 5.0, 5.4];
        assert_eq!(repeat_period(&sensor, 0.05, 3, 0.7, 0.002), None);
    }

    fn impulses(rate: f64, drop: f64, secs: f64, seed: u64) -> Vec<f64> {
        let mut r = Lcg(seed);
        let mut t = Vec::new();
        for k in 0..(rate * secs) as usize {
            let at = k as f64 / rate + 60e-6 * (r.next() - 0.5) * 3.4;
            if r.next() >= drop {
                t.push(at);
            }
        }
        t
    }

    #[test]
    fn impulses_at_100_per_second_give_100_hz_and_at_50_give_50_not_100() {
        let mut s = PairSpacing::new(SpacingConfig::impulses());
        for seed in 1..=4 {
            let r = s.find(&impulses(100.0, 0.1, 10.0, seed)).unwrap();
            assert!((r.rate_hz - 100.0).abs() < 0.3, "{r:?}");
            let r = s.find(&impulses(50.0, 0.1, 10.0, seed)).unwrap();
            assert!((r.rate_hz - 50.0).abs() < 0.2, "{r:?}");
            let r = s.find(&impulses(173.0, 0.1, 10.0, seed)).unwrap();
            assert!((r.rate_hz - 173.0).abs() < 0.6, "{r:?}");
        }
    }

    #[test]
    fn poisson_events_give_no_rate() {
        let mut s = PairSpacing::new(SpacingConfig::impulses());
        for seed in 1..=8 {
            let mut r = Lcg(seed);
            let mut t = 0.0;
            let mut times = Vec::new();
            while t < 10.0 {
                t += -(1.0 - r.next()).ln() / 100.0;
                times.push(t);
            }
            assert_eq!(s.find(&times), None, "seed {seed}");
        }
    }
}

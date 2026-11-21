//! Pulse trains: pulses over a running noise level in a stream of complex samples, the repetition intervals and widths of a
//! train of pulses, and the beam crossings of a rotating emitter with its scan period.
//!
//! - [`PulseDetector`]: the noise is the mean of `|x|²` over the first samples, then an exponential mean over the samples that
//!   lie below a gate and outside pulses. A pulse starts at the first sample above a start level and ends at the first sample
//!   below an end level, or at a longest length.
//! - [`PulseTrain`]: the start-to-start intervals within limits and the widths of the newest pulses; a histogram of the
//!   intervals in fixed bins gives one repetition frequency, or two for a staggered train ([`Prf`]); the median width.
//! - [`BeamScan`]: the peak pulse level in fixed time buckets. A crossing is a run of buckets near the largest level of a recent
//!   window ([`Crossing`]); the scan period is the median interval between crossings when most intervals agree with it.
//!
//! Every number is a parameter. Times are absolute sample indices (the detector) or seconds (the train and the scan), so a
//! stream fed in pieces of any length gives the same output as one piece.

use crate::meter::db_to_power;
use crate::stats::quantile_f64;
use crate::{Complex32, DspError, Param};
use std::collections::VecDeque;

/// The levels and lengths of a [`PulseDetector`]. Levels are factors of the noise power; lengths are in samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PulseConfig {
    /// The samples whose mean `|x|²` is the first noise, at least 1. No pulse starts before they are in.
    pub init: usize,
    /// The time constant of the exponential mean of the noise after the first samples, samples (more than 0).
    pub tau: f64,
    /// A sample outside pulses with `|x|²` below `gate` × noise enters the noise mean.
    pub gate: f64,
    /// A pulse starts at the first sample with `|x|²` above `start` × noise.
    pub start: f64,
    /// A pulse ends at the first sample with `|x|²` below `end` × noise.
    pub end: f64,
    /// A pulse also ends after this many samples, at least 1.
    pub max_len: usize,
    /// The least noise power, so that input of exact zeros has a noise and pulses end.
    pub min_noise: f64,
}

impl PulseConfig {
    /// The detector at `rate` (Hz) with the times in seconds: the first noise over `init_s`, the time constant `tau_s`, the
    /// gate, start and end levels in dB over the noise, and the longest pulse `max_s`. Lengths are rounded to whole samples (at
    /// least 1); the least noise is 1e-20.
    pub fn at_rate(
        rate: f64,
        init_s: f64,
        tau_s: f64,
        gate_db: f64,
        start_db: f64,
        end_db: f64,
        max_s: f64,
    ) -> Self {
        PulseConfig {
            init: samples(rate * init_s),
            tau: (rate * tau_s).max(1.0),
            gate: db_to_power(gate_db),
            start: db_to_power(start_db),
            end: db_to_power(end_db),
            max_len: samples(rate * max_s),
            min_noise: 1e-20,
        }
    }
}

/// `v` rounded to a whole number of samples, at least 1.
fn samples(v: f64) -> usize {
    if v.is_finite() && v > 1.0 {
        v.round() as usize
    } else {
        1
    }
}

/// One pulse found by a [`PulseDetector`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pulse {
    /// The index of its first sample.
    pub start: u64,
    /// Its length, samples (at least 1).
    pub len: u64,
    /// The largest `|x|²` in it.
    pub peak: f64,
    /// The noise power at its start.
    pub noise: f64,
    /// It ended at [`PulseConfig::max_len`], not below the end level.
    pub capped: bool,
}

impl Pulse {
    /// The peak over the noise, dB.
    pub fn peak_db(&self) -> f64 {
        10.0 * (self.peak / self.noise).log10()
    }
}

/// Pulses over a running noise level (module docs).
///
/// ```
/// use sdrkit_algo_dsp::pulsetrain::{PulseConfig, PulseDetector};
/// use sdrkit_algo_dsp::Complex32;
/// let cfg = PulseConfig { init: 100, tau: 1000.0, gate: 4.0, start: 20.0, end: 5.0, max_len: 50, min_noise: 1e-20 };
/// let mut d = PulseDetector::new(cfg).unwrap();
/// let mut x = vec![Complex32::new(0.1, 0.0); 1000];
/// x[500..508].fill(Complex32::new(3.0, 0.0));
/// let mut out = Vec::new();
/// d.process(&x, 0, &mut out);
/// assert_eq!((out.len(), out[0].start, out[0].len), (1, 500, 8));
/// ```
#[derive(Clone, Debug)]
pub struct PulseDetector {
    cfg: PulseConfig,
    alpha: f64,
    init_sum: f64,
    init_n: usize,
    noise: f64,
    gate_level: f64,
    start_level: f64,
    end_level: f64,
    active: Option<Pulse>,
    next: u64,
}

impl PulseDetector {
    /// A detector with the levels and lengths of `cfg`.
    ///
    /// # Errors
    ///
    /// [`DspError`] for `init` or `max_len` of 0, a time constant that is not a positive number, levels that are not positive
    /// numbers, or a least noise that is not a positive number.
    pub fn new(cfg: PulseConfig) -> Result<Self, DspError> {
        if cfg.init == 0 || cfg.max_len == 0 {
            return Param::Values.at_least(1.0, 0.0);
        }
        if !(cfg.tau.is_finite() && cfg.tau > 0.0) {
            return Param::Values.positive(cfg.tau);
        }
        for v in [cfg.gate, cfg.start, cfg.end, cfg.min_noise] {
            if !(v.is_finite() && v > 0.0) {
                return Param::Power.positive(v);
            }
        }
        let mut d = PulseDetector {
            cfg,
            alpha: 1.0 / cfg.tau,
            init_sum: 0.0,
            init_n: 0,
            noise: 0.0,
            gate_level: 0.0,
            start_level: f64::INFINITY,
            end_level: 0.0,
            active: None,
            next: 0,
        };
        d.reset();
        Ok(d)
    }

    /// The levels and lengths.
    pub fn config(&self) -> &PulseConfig {
        &self.cfg
    }

    /// The noise power, once the first samples are in.
    pub fn noise(&self) -> Option<f64> {
        (self.init_n >= self.cfg.init).then_some(self.noise)
    }

    /// The index of the first sample of the pulse being measured.
    pub fn active_start(&self) -> Option<u64> {
        self.active.map(|p| p.start)
    }

    /// The index of the next sample expected.
    pub fn position(&self) -> u64 {
        self.next
    }

    /// Forgets the noise and a pulse being measured.
    pub fn reset(&mut self) {
        self.init_sum = 0.0;
        self.init_n = 0;
        self.noise = 0.0;
        self.start_level = f64::INFINITY;
        self.gate_level = 0.0;
        self.end_level = 0.0;
        self.active = None;
    }

    fn set_noise(&mut self, noise: f64) {
        self.noise = noise.max(self.cfg.min_noise);
        self.gate_level = self.cfg.gate * self.noise;
        self.start_level = self.cfg.start * self.noise;
        self.end_level = self.cfg.end * self.noise;
    }

    /// Takes `x`, whose first sample has the absolute index `first`, and appends each pulse that ends in it to `out`, in order.
    pub fn process(&mut self, x: &[Complex32], first: u64, out: &mut Vec<Pulse>) {
        let max_len = self.cfg.max_len as u64;
        for (i, v) in x.iter().enumerate() {
            let n = first + i as u64;
            let p = f64::from(v.norm_sqr());
            if self.init_n < self.cfg.init {
                self.init_sum += if p.is_finite() { p } else { 0.0 };
                self.init_n += 1;
                if self.init_n == self.cfg.init {
                    self.set_noise(self.init_sum / self.cfg.init as f64);
                }
                continue;
            }
            if let Some(a) = &mut self.active {
                if p < self.end_level {
                    a.len = n - a.start;
                    out.push(*a);
                    self.active = None;
                } else {
                    if p > a.peak {
                        a.peak = p;
                    }
                    if n + 1 - a.start >= max_len {
                        a.len = max_len;
                        a.capped = true;
                        out.push(*a);
                        self.active = None;
                    }
                    continue;
                }
            }
            if p > self.start_level {
                self.active = Some(Pulse {
                    start: n,
                    len: 1,
                    peak: p,
                    noise: self.noise,
                    capped: false,
                });
                if max_len == 1 {
                    out.push(Pulse {
                        capped: true,
                        ..self.active.take().unwrap_or_default()
                    });
                }
            } else if p < self.gate_level {
                self.set_noise(self.noise + (p - self.noise) * self.alpha);
            }
        }
        self.next = first + x.len() as u64;
    }
}

/// The limits and bins of a [`PulseTrain`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainConfig {
    /// An interval is kept when it is longer than this, s.
    pub min_interval_s: f64,
    /// An interval is kept when it is shorter than this, s.
    pub max_interval_s: f64,
    /// The newest intervals kept, at least 1.
    pub intervals: usize,
    /// The newest widths kept, at least 1.
    pub widths: usize,
    /// The width of a histogram bin, s.
    pub bin_s: f64,
    /// A second repetition frequency is found when its bin holds more than this share of the fullest bin.
    pub second_share: f64,
    /// ... and lies more than this many bins from the fullest bin.
    pub second_bins: usize,
}

/// The repetition of a train: the frequency of the fullest bin of the intervals and, for a staggered train, the second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prf {
    /// The repetition frequency of the fullest bin, Hz.
    pub hz: f64,
    /// The intervals in the fullest bin.
    pub count: usize,
    /// The repetition frequency of the second bin of a staggered train, Hz.
    pub second_hz: Option<f64>,
}

impl Prf {
    /// The higher and the lower frequency of a staggered train.
    pub fn dual(&self) -> Option<(f64, f64)> {
        self.second_hz
            .map(|s| (self.hz.max(s), self.hz.min(s)))
    }
}

/// Repetition intervals and widths of a train of pulses (module docs).
#[derive(Clone, Debug)]
pub struct PulseTrain {
    cfg: TrainConfig,
    last_start: Option<f64>,
    intervals: VecDeque<f64>,
    widths: VecDeque<f64>,
    scratch: Vec<f64>,
}

impl PulseTrain {
    /// A train with the limits and bins of `cfg`.
    ///
    /// # Errors
    ///
    /// [`DspError`] for a count of 0 or a bin width that is not a positive number.
    pub fn new(cfg: TrainConfig) -> Result<Self, DspError> {
        if cfg.intervals == 0 || cfg.widths == 0 {
            return Param::Values.at_least(1.0, 0.0);
        }
        if !(cfg.bin_s.is_finite() && cfg.bin_s > 0.0) {
            return Param::Values.positive(cfg.bin_s);
        }
        Ok(PulseTrain {
            cfg,
            last_start: None,
            intervals: VecDeque::with_capacity(cfg.intervals),
            widths: VecDeque::with_capacity(cfg.widths),
            scratch: Vec::new(),
        })
    }

    /// Adds a pulse that starts at `start_s` and is `width_s` long. Pulses come in the order of their starts.
    pub fn push(&mut self, start_s: f64, width_s: f64) {
        if let Some(last) = self.last_start {
            let d = start_s - last;
            if d > self.cfg.min_interval_s && d < self.cfg.max_interval_s {
                if self.intervals.len() == self.cfg.intervals {
                    self.intervals.pop_front();
                }
                self.intervals.push_back(d);
            }
        }
        self.last_start = Some(start_s);
        if self.widths.len() == self.cfg.widths {
            self.widths.pop_front();
        }
        self.widths.push_back(width_s);
    }

    /// The intervals kept, oldest first, s.
    pub fn intervals(&self) -> impl Iterator<Item = f64> + '_ {
        self.intervals.iter().copied()
    }

    /// The median of the widths kept (the upper middle of an even count), s.
    pub fn median_width(&mut self) -> Option<f64> {
        self.widths.make_contiguous();
        quantile_f64(self.widths.as_slices().0, 0.5, &mut self.scratch)
    }

    /// The repetition: the intervals in bins of `round(interval / bin)`; the fullest bin (the shorter interval on a tie) gives
    /// the frequency, 1 / the mean of the intervals in it and its two neighbours. The fullest bin more than
    /// [`TrainConfig::second_bins`] away gives the second frequency when it holds more than [`TrainConfig::second_share`] of the
    /// first. `None` without intervals.
    pub fn prf(&self) -> Option<Prf> {
        if self.intervals.is_empty() {
            return None;
        }
        let bin = |d: f64| (d / self.cfg.bin_s).round() as i64;
        let mut bins: Vec<(i64, usize)> = Vec::new();
        let mut keys: Vec<i64> = self.intervals.iter().map(|&d| bin(d)).collect();
        keys.sort_unstable();
        for k in keys {
            match bins.last_mut() {
                Some((b, n)) if *b == k => *n += 1,
                _ => bins.push((k, 1)),
            }
        }
        let fullest = |skip: &dyn Fn(i64) -> bool| {
            bins.iter()
                .filter(|(b, _)| !skip(*b))
                .fold(None, |best: Option<(i64, usize)>, &(b, n)| match best {
                    Some((_, m)) if m >= n => best,
                    _ => Some((b, n)),
                })
        };
        let (b1, n1) = fullest(&|_| false)?;
        let hz = |b: i64| {
            let (sum, n) = self
                .intervals
                .iter()
                .filter(|&&d| (bin(d) - b).abs() <= 1)
                .fold((0.0, 0usize), |(s, n), &d| (s + d, n + 1));
            n as f64 / sum
        };
        let far = self.cfg.second_bins as i64;
        let second = fullest(&|b| (b - b1).abs() <= far)
            .filter(|&(_, n2)| n2 as f64 > self.cfg.second_share * n1 as f64)
            .map(|(b2, _)| hz(b2));
        Some(Prf {
            hz: hz(b1),
            count: n1,
            second_hz: second,
        })
    }

    /// Forgets the intervals, the widths and the last start.
    pub fn clear(&mut self) {
        self.last_start = None;
        self.intervals.clear();
        self.widths.clear();
    }
}

/// The buckets, levels and limits of a [`BeamScan`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScanConfig {
    /// The length of a bucket, s.
    pub bucket_s: f64,
    /// The newest buckets whose largest level sets the level of a crossing, at least 1.
    pub window: usize,
    /// No crossing while that largest level is below this, dB; an open crossing closes.
    pub min_level_db: f64,
    /// A bucket at the largest level less this or more opens a crossing or keeps it open, dB.
    pub within_db: f64,
    /// An open crossing closes after more than this many buckets below in a row.
    pub close_after: usize,
    /// The newest crossings kept, at least 2.
    pub crossings: usize,
    /// The least crossings for a period.
    pub min_crossings: usize,
    /// The shortest and the longest period, s.
    pub period_s: (f64, f64),
    /// An interval agrees with the median when it is within this share of it.
    pub agree: f64,
    /// The share of the intervals that must agree.
    pub agree_share: f64,
}

/// One passing of the beam.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Crossing {
    /// The power-weighted mean time of the centres of its buckets at or above the level, s.
    pub time_s: f64,
    /// From the start of its first bucket to the end of its last bucket at or above the level, s.
    pub width_s: f64,
    /// Its largest bucket level, dB.
    pub peak_db: f64,
}

#[derive(Clone, Copy, Debug)]
struct Open {
    first: u64,
    last: u64,
    below: usize,
    weight: f64,
    weighted_t: f64,
    peak: f64,
}

/// Beam crossings and the scan period (module docs).
#[derive(Clone, Debug)]
pub struct BeamScan {
    cfg: ScanConfig,
    /// Levels of the buckets from `next` on that are not closed yet.
    pending: VecDeque<f64>,
    next: u64,
    /// The buckets of the window that can still be its largest: (index, level), levels falling.
    peaks: VecDeque<(u64, f64)>,
    open: Option<Open>,
    crossings: VecDeque<Crossing>,
    scratch: Vec<f64>,
    gaps: Vec<f64>,
}

impl BeamScan {
    /// A scan with the buckets, levels and limits of `cfg`.
    ///
    /// # Errors
    ///
    /// [`DspError`] for a bucket length that is not a positive number, a window of 0 or fewer than 2 crossings kept.
    pub fn new(cfg: ScanConfig) -> Result<Self, DspError> {
        if !(cfg.bucket_s.is_finite() && cfg.bucket_s > 0.0) {
            return Param::Values.positive(cfg.bucket_s);
        }
        if cfg.window == 0 || cfg.crossings < 2 {
            return Param::Values.at_least(2.0, cfg.crossings as f64);
        }
        Ok(BeamScan {
            cfg,
            pending: VecDeque::new(),
            next: 0,
            peaks: VecDeque::new(),
            open: None,
            crossings: VecDeque::with_capacity(cfg.crossings),
            scratch: Vec::new(),
            gaps: Vec::new(),
        })
    }

    /// The bucket of time `t_s`.
    fn bucket(&self, t_s: f64) -> u64 {
        (t_s / self.cfg.bucket_s).floor().max(0.0) as u64
    }

    /// Adds a pulse at `t_s` with the level `level_db`. A pulse in a bucket that is closed already goes into the oldest open one.
    pub fn add(&mut self, t_s: f64, level_db: f64) {
        let i = self.bucket(t_s).saturating_sub(self.next) as usize;
        if self.pending.len() <= i {
            self.pending.resize(i + 1, f64::NEG_INFINITY);
        }
        if level_db > self.pending[i] {
            self.pending[i] = level_db;
        }
    }

    /// Closes every bucket that ends at or before `t_s` and appends each crossing that closes to `out`, in order.
    pub fn advance(&mut self, t_s: f64, out: &mut Vec<Crossing>) {
        let end = ((t_s / self.cfg.bucket_s).floor().max(0.0)) as u64;
        while self.next < end {
            let level = self.pending.pop_front().unwrap_or(f64::NEG_INFINITY);
            let b = self.next;
            self.next += 1;
            self.close_bucket(b, level, out);
        }
    }

    fn close_bucket(&mut self, b: u64, level: f64, out: &mut Vec<Crossing>) {
        let w = self.cfg.window as u64;
        while self.peaks.front().is_some_and(|&(i, _)| i + w <= b) {
            self.peaks.pop_front();
        }
        while self.peaks.back().is_some_and(|&(_, l)| l <= level) {
            self.peaks.pop_back();
        }
        self.peaks.push_back((b, level));
        let mx = self.peaks.front().map_or(f64::NEG_INFINITY, |p| p.1);
        if mx < self.cfg.min_level_db {
            self.close(out);
            return;
        }
        let near = level >= mx - self.cfg.within_db;
        match &mut self.open {
            Some(o) if near => {
                o.last = b;
                o.below = 0;
                let p = 10f64.powf(level / 10.0);
                o.weight += p;
                o.weighted_t += p * (b as f64 + 0.5);
                o.peak = o.peak.max(level);
            }
            Some(o) => {
                o.below += 1;
                if o.below > self.cfg.close_after {
                    self.close(out);
                }
            }
            None if near => {
                let p = 10f64.powf(level / 10.0);
                self.open = Some(Open {
                    first: b,
                    last: b,
                    below: 0,
                    weight: p,
                    weighted_t: p * (b as f64 + 0.5),
                    peak: level,
                });
            }
            None => {}
        }
    }

    fn close(&mut self, out: &mut Vec<Crossing>) {
        let Some(o) = self.open.take() else {
            return;
        };
        let c = Crossing {
            time_s: o.weighted_t / o.weight * self.cfg.bucket_s,
            width_s: (o.last - o.first + 1) as f64 * self.cfg.bucket_s,
            peak_db: o.peak,
        };
        if self.crossings.len() == self.cfg.crossings {
            self.crossings.pop_front();
        }
        self.crossings.push_back(c);
        out.push(c);
    }

    /// The crossings kept, oldest first.
    pub fn crossings(&self) -> impl Iterator<Item = &Crossing> {
        self.crossings.iter()
    }

    /// The scan period: with [`ScanConfig::min_crossings`] or more crossings, the median m of the intervals between them (the
    /// upper middle of an even count), when m is within [`ScanConfig::period_s`] and at least [`ScanConfig::agree_share`] of
    /// the intervals lie within [`ScanConfig::agree`] × m of it.
    pub fn period(&mut self) -> Option<f64> {
        if self.crossings.len() < self.cfg.min_crossings.max(2) {
            return None;
        }
        self.gaps.clear();
        let times: Vec<f64> = self.crossings.iter().map(|c| c.time_s).collect();
        self.gaps.extend(times.windows(2).map(|w| w[1] - w[0]));
        let m = quantile_f64(&self.gaps, 0.5, &mut self.scratch)?;
        if m < self.cfg.period_s.0 || m > self.cfg.period_s.1 {
            return None;
        }
        let agree = self
            .gaps
            .iter()
            .filter(|&&g| (g - m).abs() <= self.cfg.agree * m)
            .count();
        (agree as f64 >= self.cfg.agree_share * self.gaps.len() as f64).then_some(m)
    }

    /// Forgets the crossings and an open crossing; the buckets and the window go on.
    pub fn clear(&mut self) {
        self.open = None;
        self.crossings.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> PulseDetector {
        PulseDetector::new(PulseConfig::at_rate(1e6, 2e-3, 0.2, 6.02, 13.01, 6.99, 200e-6)).unwrap()
    }

    /// Complex noise of power 1 from a small linear congruential source with a Box-Muller pair.
    fn noise(n: usize, seed: u64) -> Vec<Complex32> {
        let mut s = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mut u = || {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        (0..n)
            .map(|_| {
                let r = (-u().ln()).sqrt();
                let a = std::f64::consts::TAU * u();
                Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn pulses_are_found_with_their_start_width_and_peak() {
        let mut x = noise(200_000, 1);
        for k in 0..50 {
            let s = 5000 + k * 1000;
            for v in &mut x[s..s + 3] {
                *v = Complex32::new(31.6, 0.0);
            }
        }
        let mut d = detector();
        let mut out = Vec::new();
        d.process(&x, 0, &mut out);
        assert_eq!(out.len(), 50, "{out:?}");
        for (k, p) in out.iter().enumerate() {
            assert_eq!(p.start, 5000 + k as u64 * 1000);
            assert_eq!(p.len, 3);
            assert!((p.peak_db() - 30.0).abs() < 1.5, "{p:?}");
        }
        // The gate keeps the noise near the power of the noise (the mean below the gate is about 0.93 of it).
        let n = d.noise().unwrap();
        assert!((0.85..1.0).contains(&n), "{n}");
    }

    #[test]
    fn a_long_pulse_ends_at_the_longest_length() {
        let mut x = noise(10_000, 2);
        x[5000..5500].fill(Complex32::new(10.0, 0.0));
        let mut out = Vec::new();
        detector().process(&x, 0, &mut out);
        assert_eq!(out[0].start, 5000);
        assert_eq!(out[0].len, 200);
        assert!(out[0].capped);
        assert_eq!(out[1].start, 5200);
        assert!(out.len() == 3 && out[2].len == 100 && !out[2].capped, "{out:?}");
    }

    #[test]
    fn noise_alone_gives_almost_no_pulse() {
        let x = noise(2_000_000, 3);
        let mut out = Vec::new();
        detector().process(&x, 0, &mut out);
        // P(|x|^2 > 20 x 0.93) = exp(-18.6) per sample.
        assert!(out.len() <= 2, "{out:?}");
    }

    #[test]
    fn pieces_of_any_size_give_the_same_pulses() {
        let mut x = noise(60_000, 4);
        for s in (3000..60_000).step_by(777) {
            x[s] = Complex32::new(8.0, 0.0);
        }
        let mut whole = Vec::new();
        detector().process(&x, 0, &mut whole);
        for piece in [1usize, 97, 4096] {
            let mut d = detector();
            let mut out = Vec::new();
            let mut first = 0u64;
            for c in x.chunks(piece) {
                d.process(c, first, &mut out);
                first += c.len() as u64;
            }
            assert_eq!(out, whole, "piece {piece}");
        }
        assert!(whole.len() >= 70);
    }

    fn train() -> PulseTrain {
        PulseTrain::new(TrainConfig {
            min_interval_s: 50e-6,
            max_interval_s: 20e-3,
            intervals: 400,
            widths: 200,
            bin_s: 5e-6,
            second_share: 1.0 / 3.0,
            second_bins: 3,
        })
        .unwrap()
    }

    #[test]
    fn a_single_train_with_jitter_gives_its_frequency() {
        let mut t = train();
        let mut s = 0.0;
        for k in 0..300 {
            // Starts on a 0.5 us grid: intervals of 999.5, 1000 and 1000.5 us.
            let jitter = [0.0, 0.5e-6, -0.5e-6][k % 3];
            t.push(s + jitter, 0.8e-6 + 0.1e-6 * (k % 2) as f64);
            s += 1e-3;
        }
        let p = t.prf().unwrap();
        assert!((p.hz - 1000.0).abs() < 0.5, "{p:?}");
        assert_eq!(p.second_hz, None);
        assert!((t.median_width().unwrap() - 0.9e-6).abs() < 1e-12);
    }

    #[test]
    fn a_staggered_train_gives_two_frequencies() {
        let mut t = train();
        let mut s = 0.0;
        for k in 0..400 {
            t.push(s, 2e-6);
            s += if k % 2 == 0 { 1.0 / 1200.0 } else { 1.0 / 900.0 };
        }
        let (hi, lo) = t.prf().unwrap().dual().unwrap();
        assert!((hi - 1200.0).abs() < 1.0 && (lo - 900.0).abs() < 1.0, "{hi} {lo}");
        // Long gaps are not intervals; a second bin of a quarter of the first is not a second frequency.
        let mut u = train();
        let mut s = 0.0;
        for k in 0..400 {
            u.push(s, 1e-6);
            s += if k % 5 == 0 { 1.25e-3 } else { 1e-3 };
            if k % 50 == 49 {
                s += 2.0;
            }
        }
        assert_eq!(u.prf().unwrap().second_hz, None);
        assert_eq!(u.intervals().count(), 392);
    }

    fn scan() -> BeamScan {
        BeamScan::new(ScanConfig {
            bucket_s: 2e-3,
            window: 3500,
            min_level_db: 13.0,
            within_db: 6.0,
            close_after: 6,
            crossings: 80,
            min_crossings: 3,
            period_s: (0.5, 60.0),
            agree: 0.2,
            agree_share: 0.5,
        })
        .unwrap()
    }

    /// Pulses every 1 ms with a beam of `beam_s` (the time of the beam's 12 dB per (dt / beam)^2 fall) every `period` s.
    fn beam(period: f64, beam_s: f64, seconds: f64, peak: f64, piece_s: f64) -> (Vec<Crossing>, BeamScan) {
        let mut sc = scan();
        let mut out = Vec::new();
        let mut t = 0.0;
        let mut next_advance = piece_s;
        while t < seconds {
            let dt = ((t - 0.3) / period - ((t - 0.3) / period).round()) * period;
            let level = peak - 12.0 * (dt / beam_s).powi(2);
            if level >= 13.0 {
                sc.add(t, level);
            }
            t += 1e-3;
            if t >= next_advance {
                sc.advance(t, &mut out);
                next_advance += piece_s;
            }
        }
        sc.advance(seconds, &mut out);
        (out, sc)
    }

    #[test]
    fn crossings_and_the_period_of_a_rotating_beam() {
        let (out, mut sc) = beam(2.5, 0.014, 13.0, 30.0, 0.05);
        assert_eq!(out.len(), 6, "{out:?}");
        for (k, c) in out.iter().enumerate() {
            assert!((c.time_s - (0.3 + 2.5 * k as f64)).abs() < 1.5e-3, "{c:?}");
            // At 6 dB below the peak: dt = beam x sqrt(0.5), a width of about 2 x 9.9 ms rounded to buckets. The first is
            // wider: the largest level of the window still rises while it opens.
            let most = if k == 0 { 0.03 } else { 0.024 };
            assert!((0.016..=most).contains(&c.width_s), "{c:?}");
            assert!((c.peak_db - 30.0).abs() < 0.2);
        }
        let p = sc.period().unwrap();
        assert!((p - 2.5).abs() < 0.01, "{p}");
        let (again, _) = beam(2.5, 0.014, 13.0, 30.0, 0.0071);
        assert_eq!(out, again);
    }

    #[test]
    fn no_period_from_two_crossings_or_from_weak_pulses() {
        let (out, mut sc) = beam(2.5, 0.014, 4.0, 30.0, 0.05);
        assert_eq!(out.len(), 2);
        assert_eq!(sc.period(), None);
        let (out, mut sc) = beam(2.5, 0.014, 13.0, 12.5, 0.05);
        assert!(out.is_empty());
        assert_eq!(sc.period(), None);
    }

    #[test]
    fn bad_configurations_are_refused() {
        let mut c = PulseConfig::at_rate(1e6, 2e-3, 0.2, 6.0, 13.0, 7.0, 200e-6);
        c.init = 0;
        assert!(PulseDetector::new(c).is_err());
        let mut s = scan().cfg;
        s.bucket_s = 0.0;
        assert!(BeamScan::new(s).is_err());
    }
}

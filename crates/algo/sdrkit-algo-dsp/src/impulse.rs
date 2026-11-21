//! Impulses over a noise floor: a detector of impulses in a stream of complex samples, the measure of the leading edge of one
//! impulse, and a recogniser of impulses that fall on the grid of a known period (the impulses of mains equipment).
//!
//! - [`ImpulseDetector`]: the moving average of `|x|²` over a short window against a floor, the median of the means of the
//!   last groups of samples. An impulse starts above one level, ends after a run of samples below a lower level, and can join
//!   the impulse before it.
//! - [`leading_edge`]: the half-peak crossing of the envelope of one impulse, interpolated between samples, with its rise
//!   time, width, mean frequency and 1-sigma error.
//! - [`PeriodicTrain`]: marks the impulses whose starts lie a whole number of known periods after recent starts, and says
//!   when most recent impulses do.
//!
//! Times are absolute sample indices (the detector) or seconds (the recogniser), so a stream fed in pieces of any length gives
//! the same output as one piece.

use crate::ring::SampleRing;
use crate::stats::{quantile_f64, WindowSum};
use crate::{Complex32, DspError, Param};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The lengths and levels of an [`ImpulseDetector`]. Every length is in samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpulseConfig {
    /// The length of the moving average of `|x|²` (the smoothed power), at least 1.
    pub smooth: usize,
    /// The samples of one group whose mean enters the floor, at least 1.
    pub group: usize,
    /// The newest group means whose median is the floor, at least 1.
    pub floor_groups: usize,
    /// Once the floor is valid, it is computed again every this many groups, at least 1.
    pub floor_every: usize,
    /// The groups before the floor has a first value.
    pub floor_first: usize,
    /// The groups before the floor is valid. No impulse starts before.
    pub valid_after: usize,
    /// The group means are of the smoothed power (`true`) or of `|x|²` (`false`).
    pub floor_of_smoothed: bool,
    /// An impulse starts at the first sample whose smoothed power is above the floor by this much, dB.
    pub start_db: f64,
    /// An impulse ends at the first of `hold` samples in a row whose smoothed power is below the floor plus this much, dB.
    pub end_db: f64,
    /// The run of samples below the end level that ends an impulse, at least 1.
    pub hold: usize,
    /// An impulse longer than this (end minus start) is marked too long.
    pub max_len: usize,
    /// An impulse that starts less than this after the end of the impulse before it joins that impulse; 0 joins none. An
    /// impulse is given out once this time has passed after its end.
    pub merge: usize,
    /// The samples kept before the start of an impulse when `capture` is on.
    pub pre: usize,
    /// Each impulse carries its samples, from `pre` before its start to the end of its run below the end level.
    pub capture: bool,
    /// The least floor (power). Input of exact zeros then has a floor, and impulses end.
    pub min_floor: f64,
}

impl ImpulseConfig {
    /// The detector that counts impulses at `rate` (Hz): a smoothed power over `max(16, rate × 25 µs)` samples, groups of
    /// `max(1, rate × 1 ms)`, the median of the last 4000 group means (recomputed every 100 groups, first after 20, valid
    /// after 500), start at `threshold_db`, end 3 dB lower after `max(1, rate × 100 µs)` samples, too long above 5 ms, and
    /// joins over `merge_s` seconds.
    pub fn counter(rate: f64, threshold_db: f64, merge_s: f64) -> Self {
        ImpulseConfig {
            smooth: samples(rate * 25e-6).max(16),
            group: samples(rate * 1e-3).max(1),
            floor_groups: 4000,
            floor_every: 100,
            floor_first: 20,
            valid_after: 500,
            floor_of_smoothed: true,
            start_db: threshold_db,
            end_db: threshold_db - 3.0,
            hold: samples(rate * 100e-6).max(1),
            max_len: samples(rate * 5e-3).max(1),
            merge: samples(rate * merge_s),
            pre: 0,
            capture: false,
            min_floor: 1e-12,
        }
    }

    /// The detector that times impulses at `rate` (Hz): a smoothed power over `max(2, rate × 10 µs)` samples, groups of
    /// `max(16, rate × 1 ms)` samples of `|x|²`, the median of the last 2000 group means (recomputed every 100 groups, valid
    /// after 300), start at `threshold_db`, end 3 dB lower after `max(2, rate × 100 µs)` samples, too long above
    /// `max(64, rate × 5 ms)`, no joining, and the samples captured from `max(4, smooth + rate × 20 µs)` before the start.
    pub fn timer(rate: f64, threshold_db: f64) -> Self {
        let smooth = samples(rate * 10e-6).max(2);
        ImpulseConfig {
            smooth,
            group: samples(rate * 1e-3).max(16),
            floor_groups: 2000,
            floor_every: 100,
            floor_first: 1,
            valid_after: 300,
            floor_of_smoothed: false,
            start_db: threshold_db,
            end_db: threshold_db - 3.0,
            hold: samples(rate * 100e-6).max(2),
            max_len: samples(rate * 5e-3).max(64),
            merge: 0,
            pre: (smooth + samples(rate * 20e-6)).max(4),
            capture: true,
            min_floor: 1e-12,
        }
    }
}

/// `v` rounded to a whole number of samples (0 for a value that is not a positive number).
fn samples(v: f64) -> usize {
    if v.is_finite() && v > 0.0 {
        v.round() as usize
    } else {
        0
    }
}

/// One impulse found by an [`ImpulseDetector`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Impulse {
    /// The index of its first sample above the start level.
    pub start: u64,
    /// The index of the first sample of the run below the end level that ended it (one after its last sample).
    pub end: u64,
    /// The largest smoothed power in it.
    pub peak: f64,
    /// The index of that largest smoothed power.
    pub peak_at: u64,
    /// The floor (power) at its start.
    pub floor: f64,
    /// It is longer than [`ImpulseConfig::max_len`].
    pub too_long: bool,
    /// The impulses that joined it.
    pub joined: u32,
    /// The index of the sample at which it was given out.
    pub given_at: u64,
    /// Its samples with [`ImpulseConfig::capture`], from [`Impulse::samples_from`]; empty without capture, or when it is too
    /// long.
    pub samples: Vec<Complex32>,
    /// The index of the first of [`Impulse::samples`].
    pub samples_from: u64,
}

impl Impulse {
    /// Its length, samples.
    pub fn width(&self) -> u64 {
        self.end - self.start
    }

    /// Its peak over the floor: `10 log10(peak / floor)`, dB.
    pub fn peak_db(&self) -> f64 {
        10.0 * (self.peak / self.floor).log10()
    }
}

/// The impulse being measured.
#[derive(Clone, Debug)]
struct Active {
    imp: Impulse,
    below: usize,
    below_from: u64,
}

/// A detector of impulses over the floor of a stream of complex samples (see [`ImpulseConfig`]).
///
/// Each sample's smoothed power is the mean of `|x|²` over the newest [`ImpulseConfig::smooth`] samples. The floor is the
/// median (the upper middle of an even count) of the newest group means, computed after every group from the first value
/// until it is valid and then every [`ImpulseConfig::floor_every`] groups; every group enters it, those of impulses too. An
/// impulse is given out at the sample where its end is known, or, with joining, once [`ImpulseConfig::merge`] samples have
/// passed after its end. Nothing is allocated per sample unless samples are captured.
///
/// ```
/// use sdrkit_algo_dsp::impulse::{ImpulseConfig, ImpulseDetector};
/// use sdrkit_algo_dsp::Complex32;
/// let rate = 100e3;
/// let mut d = ImpulseDetector::new(ImpulseConfig::counter(rate, 10.0, 0.5e-3)).unwrap();
/// // 1 s of a weak constant level, then a strong impulse of 100 samples.
/// let mut x = vec![Complex32::new(0.01, 0.0); 120_000];
/// for v in &mut x[100_000..100_100] {
///     *v = Complex32::new(1.0, 0.0);
/// }
/// let mut found = Vec::new();
/// d.process(&x, 0, &mut found);
/// d.flush(&mut found);
/// assert_eq!(found.len(), 1);
/// assert!(found[0].start >= 100_000 && found[0].start < 100_016);
/// ```
#[derive(Clone, Debug)]
pub struct ImpulseDetector {
    cfg: ImpulseConfig,
    win: WindowSum<f64>,
    acc: f64,
    acc_n: usize,
    means: Vec<f64>,
    pos: usize,
    groups: u64,
    floor: Option<f64>,
    scratch: Vec<f64>,
    start_ratio: f64,
    end_ratio: f64,
    active: Option<Active>,
    pending: Option<Impulse>,
    ring: Option<SampleRing<Complex32>>,
    cap: Vec<Complex32>,
    cap_full: bool,
    next: u64,
}

impl ImpulseDetector {
    /// A detector with the lengths and levels of `cfg`.
    ///
    /// # Errors
    ///
    /// [`DspError`] for a length of 0 where at least 1 is needed (`smooth`, `group`, `floor_groups`, `floor_every`, `hold`),
    /// or levels or a least floor that are not finite.
    pub fn new(cfg: ImpulseConfig) -> Result<Self, DspError> {
        if cfg.group == 0 || cfg.floor_groups == 0 || cfg.floor_every == 0 || cfg.hold == 0 {
            return Param::Values.at_least(1.0, 0.0);
        }
        if !(cfg.start_db.is_finite() && cfg.end_db.is_finite()) {
            return Param::Power.not_finite();
        }
        if !(cfg.min_floor.is_finite() && cfg.min_floor > 0.0) {
            return Param::Power.positive(cfg.min_floor);
        }
        let ring = if cfg.capture {
            Some(SampleRing::new(cfg.pre + 1)?)
        } else {
            None
        };
        Ok(ImpulseDetector {
            win: WindowSum::new(cfg.smooth)?,
            acc: 0.0,
            acc_n: 0,
            means: vec![0.0; cfg.floor_groups],
            pos: 0,
            groups: 0,
            floor: None,
            scratch: Vec::with_capacity(cfg.floor_groups),
            start_ratio: 10f64.powf(cfg.start_db / 10.0),
            end_ratio: 10f64.powf(cfg.end_db / 10.0),
            active: None,
            pending: None,
            ring,
            cap: Vec::new(),
            cap_full: false,
            next: 0,
            cfg,
        })
    }

    /// The lengths and levels.
    pub fn config(&self) -> &ImpulseConfig {
        &self.cfg
    }

    /// Sets the start level to `start_db` and the end level to `end_db` over the floor, from the next sample on.
    pub fn set_levels(&mut self, start_db: f64, end_db: f64) {
        self.cfg.start_db = start_db;
        self.cfg.end_db = end_db;
        self.start_ratio = 10f64.powf(start_db / 10.0);
        self.end_ratio = 10f64.powf(end_db / 10.0);
    }

    /// Sets the joining time to `merge` samples, from the next impulse on.
    pub fn set_merge(&mut self, merge: usize) {
        self.cfg.merge = merge;
    }

    /// The floor (mean power of the noise), once it has a first value.
    pub fn floor(&self) -> Option<f64> {
        self.floor
    }

    /// Whether the floor is valid, so that impulses can start.
    pub fn is_valid(&self) -> bool {
        self.floor.is_some() && self.groups >= self.cfg.valid_after as u64
    }

    /// The index of the next sample expected.
    pub fn position(&self) -> u64 {
        self.next
    }

    /// Forgets everything: the floor is learned again, and an impulse being measured or held is dropped.
    pub fn reset(&mut self) {
        self.win.reset();
        self.acc = 0.0;
        self.acc_n = 0;
        self.means.fill(0.0);
        self.pos = 0;
        self.groups = 0;
        self.floor = None;
        self.active = None;
        self.pending = None;
        if let Some(r) = &mut self.ring {
            r.reset();
        }
        self.cap.clear();
        self.cap_full = false;
    }

    /// Takes `x`, whose first sample has the absolute index `first`, and appends each impulse given out to `out`, in order.
    pub fn process(&mut self, x: &[Complex32], first: u64, out: &mut Vec<Impulse>) {
        let inv = 1.0 / self.cfg.smooth as f64;
        for (i, v) in x.iter().enumerate() {
            let n = first + i as u64;
            let p = f64::from(v.norm_sqr());
            let sp = self.win.push(p) * inv;
            self.take_group(if self.cfg.floor_of_smoothed { sp } else { p });
            if self.cfg.capture {
                self.capture(*v);
            }
            if self.active.is_none() {
                if let Some(pnd) = &self.pending {
                    if n >= pnd.end + self.cfg.merge as u64 {
                        self.give(n, out);
                    }
                }
            }
            let Some(floor) = self
                .floor
                .filter(|_| self.groups >= self.cfg.valid_after as u64)
            else {
                continue;
            };
            match &mut self.active {
                None => {
                    if sp > floor * self.start_ratio {
                        self.begin(n, sp, floor);
                    }
                }
                Some(a) => {
                    if sp > a.imp.peak {
                        a.imp.peak = sp;
                        a.imp.peak_at = n;
                    }
                    if sp < floor * self.end_ratio {
                        if a.below == 0 {
                            a.below_from = n;
                        }
                        a.below += 1;
                        if a.below >= self.cfg.hold {
                            let end = a.below_from;
                            self.finish(end, n, out);
                        }
                    } else {
                        a.below = 0;
                    }
                }
            }
        }
        self.next = first + x.len() as u64;
    }

    /// The input ended: ends an impulse being measured at the last sample (or at the start of its run below the end level) and
    /// gives out every impulse still held.
    pub fn flush(&mut self, out: &mut Vec<Impulse>) {
        let n = self.next;
        if let Some(a) = &self.active {
            let end = if a.below > 0 { a.below_from } else { n };
            self.finish(end, n, out);
        }
        if self.pending.is_some() {
            self.give(n, out);
        }
    }

    fn take_group(&mut self, v: f64) {
        self.acc += v;
        self.acc_n += 1;
        if self.acc_n < self.cfg.group {
            return;
        }
        self.means[self.pos] = self.acc / self.acc_n as f64;
        self.pos = (self.pos + 1) % self.means.len();
        self.acc = 0.0;
        self.acc_n = 0;
        self.groups += 1;
        let first = (self.cfg.floor_first.max(1)) as u64;
        let valid = self.cfg.valid_after as u64;
        if self.groups >= first
            && (self.groups <= valid || self.groups.is_multiple_of(self.cfg.floor_every as u64))
        {
            let filled = (self.groups as usize).min(self.means.len());
            if let Some(m) = quantile_f64(&self.means[..filled], 0.5, &mut self.scratch) {
                self.floor = Some(m.max(self.cfg.min_floor));
            }
        }
    }

    fn capture(&mut self, v: Complex32) {
        if let Some(r) = &mut self.ring {
            r.push(v);
        }
        if (self.active.is_some() || self.pending.is_some()) && !self.cap_full {
            let limit = self.cfg.pre + self.cfg.max_len + self.cfg.hold + self.cfg.merge + 1;
            if self.cap.len() < limit {
                self.cap.push(v);
            } else {
                self.cap_full = true;
            }
        }
    }

    fn begin(&mut self, n: u64, sp: f64, floor: f64) {
        let mut imp = Impulse {
            start: n,
            end: n,
            peak: sp,
            peak_at: n,
            floor,
            ..Impulse::default()
        };
        if self.pending.is_none() {
            if let Some(r) = &mut self.ring {
                // The ring holds the sample `n` and up to `pre` samples before it.
                let held = r.last(usize::try_from(r.end()).unwrap_or(usize::MAX));
                imp.samples_from = n + 1 - held.len() as u64;
                self.cap.clear();
                self.cap.extend_from_slice(held);
                self.cap_full = false;
            }
        }
        self.active = Some(Active {
            imp,
            below: 0,
            below_from: n,
        });
    }

    /// Ends the active impulse at `end`, at sample `n`: it joins the held one, or is held, or is given out.
    fn finish(&mut self, end: u64, n: u64, out: &mut Vec<Impulse>) {
        let Some(a) = self.active.take() else {
            return;
        };
        let mut imp = a.imp;
        imp.end = end.max(imp.start + 1);
        if let Some(p) = &mut self.pending {
            if imp.start < p.end + self.cfg.merge as u64 {
                p.end = imp.end;
                if imp.peak > p.peak {
                    p.peak = imp.peak;
                    p.peak_at = imp.peak_at;
                }
                p.joined += 1 + imp.joined;
                if self.cfg.merge == 0 {
                    self.give(n, out);
                }
                return;
            }
            self.give(n, out);
        }
        self.pending = Some(imp);
        if self.cfg.merge == 0 {
            self.give(n, out);
        }
    }

    /// Gives out the held impulse at sample `n`.
    fn give(&mut self, n: u64, out: &mut Vec<Impulse>) {
        let Some(mut imp) = self.pending.take() else {
            return;
        };
        imp.given_at = n;
        imp.too_long = imp.width() > self.cfg.max_len as u64;
        if self.cfg.capture {
            let mut s = std::mem::take(&mut self.cap);
            if imp.too_long || self.cap_full {
                s.clear();
            } else {
                let keep =
                    (imp.end + self.cfg.hold as u64).saturating_sub(imp.samples_from) as usize;
                s.truncate(keep);
            }
            imp.samples = s;
            self.cap_full = false;
        }
        out.push(imp);
    }
}

/// The leading edge of one impulse (see [`leading_edge`]). Positions are fractional sample indices into the slice measured;
/// lengths are in samples and the frequency in cycles per sample (multiply by the rate for hertz).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    /// The last crossing of half the peak envelope before the peak, interpolated between samples.
    pub edge: f64,
    /// From the 10 % to the 90 % crossing of the peak envelope before the peak.
    pub rise: f64,
    /// The samples around the peak whose envelope is at least half the peak.
    pub width: f64,
    /// The mean frequency over the half-peak span: the angle of `Σ x[k+1] conj(x[k])` over `2π`.
    pub frequency: f64,
    /// The 1-sigma error of [`Edge::edge`]: the envelope noise over the slope at the edge, clamped to 0.02 to 5 samples.
    pub sigma: f64,
    /// The peak envelope over the floor: `10 log10(peak² / floor)`, dB.
    pub peak_db: f64,
    /// The peak envelope (an amplitude).
    pub amplitude: f64,
    /// The index of the peak envelope.
    pub peak_at: usize,
}

/// The leading edge of the impulse in `x` over the noise power `floor`.
///
/// The envelope is `a[k] = √(mean |x|² over k − 1, k, k + 1)`, of the samples that exist. Each crossing is the last one of its
/// level before the peak, interpolated linearly between the two samples around it. The edge noise is `σa = √(floor / 2) / √1.5`
/// and the slope is the envelope step across the half-peak crossing. Returns `None` for fewer than 3 samples, a floor that is
/// not a positive number, or an envelope that does not cross half its peak before the peak. `env` holds the envelope, so
/// nothing is allocated once it has grown.
///
/// ```
/// use sdrkit_algo_dsp::impulse::leading_edge;
/// use sdrkit_algo_dsp::Complex32;
/// // A ramp from 0 to 1 over samples 10 to 20, then a slow fall.
/// let x: Vec<Complex32> = (0..60)
///     .map(|k| {
///         let a = if k < 10 { 0.0 } else if k <= 20 { (k - 10) as f32 / 10.0 } else { (-(k as f32 - 20.0) / 15.0).exp() };
///         Complex32::new(a, 0.0)
///     })
///     .collect();
/// let e = leading_edge(&x, 1e-6, &mut Vec::new()).unwrap();
/// assert!((e.edge - 15.0).abs() < 0.5, "{}", e.edge);
/// assert!(e.rise > 7.0 && e.rise < 9.0);
/// ```
pub fn leading_edge(x: &[Complex32], floor: f64, env: &mut Vec<f64>) -> Option<Edge> {
    let n = x.len();
    if n < 3 || !(floor.is_finite() && floor > 0.0) {
        return None;
    }
    env.clear();
    for k in 0..n {
        let lo = k.saturating_sub(1);
        let hi = (k + 1).min(n - 1);
        let s: f64 = x[lo..=hi].iter().map(|v| f64::from(v.norm_sqr())).sum();
        env.push((s / (hi - lo + 1) as f64).sqrt());
    }
    let (peak_at, &peak) = env.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    if peak.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    let half = 0.5 * peak;
    let (edge, k_half) = crossing(env, peak_at, half)?;
    let rise = match (
        crossing(env, peak_at, 0.1 * peak),
        crossing(env, peak_at, 0.9 * peak),
    ) {
        (Some((t10, _)), Some((t90, _))) => (t90 - t10).max(0.0),
        _ => 0.0,
    };
    let mut lo = peak_at;
    while lo > 0 && env[lo - 1] >= half {
        lo -= 1;
    }
    let mut hi = peak_at;
    while hi + 1 < n && env[hi + 1] >= half {
        hi += 1;
    }
    let mut acc = (0.0f64, 0.0f64);
    for k in lo..hi {
        let c = x[k + 1] * x[k].conj();
        acc.0 += f64::from(c.re);
        acc.1 += f64::from(c.im);
    }
    let frequency = if hi > lo {
        acc.1.atan2(acc.0) / TAU
    } else {
        0.0
    };
    let slope = (env[k_half + 1] - env[k_half]).abs();
    let sa = (floor / 2.0).sqrt() / 1.5f64.sqrt();
    let sigma = if slope > 0.0 {
        (sa / slope).clamp(0.02, 5.0)
    } else {
        5.0
    };
    Some(Edge {
        edge,
        rise,
        width: (hi - lo + 1) as f64,
        frequency,
        sigma,
        peak_db: 10.0 * (peak * peak / floor).log10(),
        amplitude: peak,
        peak_at,
    })
}

/// The last upward crossing of `level` by `env` before `peak_at`: the fractional position and the index of the sample below.
fn crossing(env: &[f64], peak_at: usize, level: f64) -> Option<(f64, usize)> {
    let mut k = peak_at;
    while k > 0 {
        k -= 1;
        if env[k] < level {
            let (a, b) = (env[k], env[k + 1]);
            let f = if b > a { (level - a) / (b - a) } else { 0.0 };
            return Some((k as f64 + f, k));
        }
    }
    None
}

/// The periods and tolerances of a [`PeriodicTrain`]. Times are in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct TrainConfig {
    /// The periods a train can have.
    pub periods: Vec<f64>,
    /// The most whole periods between two impulses that still count.
    pub max_multiple: u32,
    /// The longest gap to a recent start that counts.
    pub max_gap: f64,
    /// The tolerance of `k` periods is `max(tolerance, relative × k × period)`.
    pub tolerance: f64,
    /// See [`TrainConfig::tolerance`].
    pub relative: f64,
    /// The recent starts an impulse is compared with.
    pub compare: usize,
    /// While the train is on and its period known: the tolerance of an impulse against the grid of the last hit.
    pub grid_tolerance: f64,
    /// The recent impulses whose hits are counted.
    pub window: usize,
    /// The train is on when at least this many of the recent impulses are hits.
    pub on_hits: usize,
    /// The train goes off when fewer than this many of the recent impulses are hits; its period is then forgotten.
    pub off_hits: usize,
}

impl TrainConfig {
    /// Impulses at the half periods of 50 Hz and 60 Hz mains (10 ms and 1/120 s): 1 to 5 periods up to 50 ms apart, within
    /// `max(0.3 ms, 4 %)`, against the last 4 starts; the grid within 0.5 ms; on at 24 of 32 hits, off below 12.
    pub fn mains() -> Self {
        TrainConfig {
            periods: vec![0.01, 1.0 / 120.0],
            max_multiple: 5,
            max_gap: 0.05,
            tolerance: 0.3e-3,
            relative: 0.04,
            compare: 4,
            grid_tolerance: 0.5e-3,
            window: 32,
            on_hits: 24,
            off_hits: 12,
        }
    }
}

/// A recogniser of a train of impulses on the grid of a known period (see [`TrainConfig`]).
///
/// An impulse is a hit when its start lies `dt` after one of the recent starts with `0 < dt <= max_gap` and `dt` within the
/// tolerance of `k` periods, for a period of the set and `k` from 1 to `max_multiple`. When `dt` to the start just before fits
/// one period, that period becomes the grid period. While the train is on and the grid period is known, an impulse is a hit
/// only when it lies within the grid tolerance of a whole number (at least 1) of grid periods after the last hit: a train
/// falls on its grid, other impulses fall anywhere.
///
/// ```
/// use sdrkit_algo_dsp::impulse::{PeriodicTrain, TrainConfig};
/// let mut t = PeriodicTrain::new(TrainConfig::mains());
/// for i in 0..40 {
///     t.push(1.0 + i as f64 * 0.01);
/// }
/// assert!(t.is_on());
/// assert!(!t.push(1.4 + 0.0043)); // off the grid
/// assert!(t.push(1.42));
/// ```
#[derive(Clone, Debug)]
pub struct PeriodicTrain {
    cfg: TrainConfig,
    starts: VecDeque<f64>,
    hits: VecDeque<bool>,
    hit_count: usize,
    on: bool,
    period: Option<f64>,
    last_hit: Option<f64>,
}

impl PeriodicTrain {
    /// A recogniser with the periods and tolerances of `cfg`.
    pub fn new(cfg: TrainConfig) -> Self {
        PeriodicTrain {
            starts: VecDeque::with_capacity(cfg.compare + 1),
            hits: VecDeque::with_capacity(cfg.window + 1),
            hit_count: 0,
            on: false,
            period: None,
            last_hit: None,
            cfg,
        }
    }

    /// Takes the start `t` (s) of the next impulse, in order, and returns whether it is a hit.
    pub fn push(&mut self, t: f64) -> bool {
        let c = &self.cfg;
        let within = |dt: f64, p: f64, k: u32| {
            (dt - f64::from(k) * p).abs() <= c.tolerance.max(c.relative * f64::from(k) * p)
        };
        let fits = |dt: f64| {
            dt > 0.0
                && dt <= c.max_gap
                && c.periods
                    .iter()
                    .any(|&p| (1..=c.max_multiple).any(|k| within(dt, p, k)))
        };
        let hit = match (self.on, self.period, self.last_hit) {
            (true, Some(p), Some(last)) => {
                let d = t - last;
                let k = (d / p).round();
                k >= 1.0 && (d - k * p).abs() <= c.grid_tolerance
            }
            _ => self.starts.iter().any(|&s| fits(t - s)),
        };
        if let Some(&prev) = self.starts.back() {
            let dt = t - prev;
            if dt > 0.0 {
                if let Some(&p) = c.periods.iter().find(|&&p| within(dt, p, 1)) {
                    self.period = Some(p);
                }
            }
        }
        if hit {
            self.last_hit = Some(t);
        }
        self.starts.push_back(t);
        while self.starts.len() > c.compare {
            self.starts.pop_front();
        }
        self.hits.push_back(hit);
        self.hit_count += usize::from(hit);
        while self.hits.len() > c.window {
            if self.hits.pop_front() == Some(true) {
                self.hit_count -= 1;
            }
        }
        if self.hit_count >= c.on_hits {
            self.on = true;
        } else if self.hit_count < c.off_hits {
            self.on = false;
            self.period = None;
        }
        hit
    }

    /// Whether the train is on: enough recent impulses are hits.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// The grid period, s, once one is known.
    pub fn period(&self) -> Option<f64> {
        self.period
    }

    /// Forgets every impulse.
    pub fn reset(&mut self) {
        self.starts.clear();
        self.hits.clear();
        self.hit_count = 0;
        self.on = false;
        self.period = None;
        self.last_hit = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Rng;

    fn noise(n: usize, rms: f64, seed: u64) -> Vec<Complex32> {
        let mut r = Rng(seed);
        let s = rms / 2f64.sqrt();
        (0..n)
            .map(|_| Complex32::new((s * r.gauss()) as f32, (s * r.gauss()) as f32))
            .collect()
    }

    #[test]
    fn the_median_of_an_even_count_is_the_upper_middle() {
        let mut s = Vec::new();
        assert_eq!(quantile_f64(&[4.0, 1.0, 3.0, 2.0], 0.5, &mut s), Some(3.0));
    }

    #[test]
    fn the_floor_reads_the_noise_power() {
        let rate = 200e3;
        let mut d = ImpulseDetector::new(ImpulseConfig::counter(rate, 10.0, 0.5e-3)).unwrap();
        let x = noise(200_000, 0.1, 7);
        let mut out = Vec::new();
        d.process(&x, 0, &mut out);
        assert!(d.is_valid());
        let db = 10.0 * d.floor().unwrap().log10();
        assert!((db + 20.0).abs() < 0.5, "{db}");
        assert!(out.is_empty());
    }

    #[test]
    fn nothing_starts_before_the_floor_is_valid() {
        let rate = 100e3;
        let mut d = ImpulseDetector::new(ImpulseConfig::counter(rate, 10.0, 0.5e-3)).unwrap();
        let mut x = noise(100_000, 0.01, 3);
        for v in &mut x[20_000..20_050] {
            *v = Complex32::new(1.0, 0.0);
        }
        let mut out = Vec::new();
        d.process(&x, 0, &mut out);
        d.flush(&mut out);
        assert!(out.is_empty());
    }

    fn with_pulses(n: usize, at: &[(usize, usize)], seed: u64) -> Vec<Complex32> {
        let mut x = noise(n, 0.01, seed);
        for &(s, len) in at {
            for v in &mut x[s..s + len] {
                *v += Complex32::new(0.5, 0.0);
            }
        }
        x
    }

    #[test]
    fn close_impulses_join_and_long_ones_are_marked() {
        let rate = 100e3;
        let cfg = ImpulseConfig::counter(rate, 10.0, 0.5e-3);
        let mut d = ImpulseDetector::new(cfg).unwrap();
        // Two pulses 30 samples apart (end to start, under the 50-sample joining time), one apart, and one of 20 ms.
        let x = with_pulses(
            200_000,
            &[(60_000, 20), (60_070, 20), (90_000, 20), (120_000, 2000)],
            5,
        );
        let mut out = Vec::new();
        d.process(&x, 0, &mut out);
        d.flush(&mut out);
        assert_eq!(out.len(), 3, "{out:?}");
        assert_eq!(out[0].joined, 1);
        assert!(!out[0].too_long && !out[1].too_long && out[2].too_long);
        assert!(out[0].peak_db() > 30.0);
    }

    #[test]
    fn pieces_of_any_size_give_the_same_impulses() {
        let rate = 100e3;
        let mut cfg = ImpulseConfig::counter(rate, 10.0, 0.5e-3);
        cfg.capture = true;
        cfg.pre = 12;
        let x = with_pulses(150_000, &[(60_000, 20), (60_070, 20), (90_000, 40)], 9);
        let run = |piece: usize| {
            let mut d = ImpulseDetector::new(cfg).unwrap();
            let mut out = Vec::new();
            let mut first = 0u64;
            for p in x.chunks(piece) {
                d.process(p, first, &mut out);
                first += p.len() as u64;
            }
            d.flush(&mut out);
            out
        };
        let one = run(x.len());
        assert_eq!(one.len(), 2);
        assert_eq!(one[1].samples_from, one[1].start - 12);
        assert_eq!(
            one[1].samples.len() as u64,
            one[1].end + cfg.hold as u64 - one[1].samples_from
        );
        for piece in [1, 7, 1000, 4096] {
            assert_eq!(run(piece), one, "piece {piece}");
        }
    }

    #[test]
    fn the_timer_captures_the_samples_before_the_start() {
        let rate = 500e3;
        let cfg = ImpulseConfig::timer(rate, 12.0);
        assert_eq!(cfg.smooth, 5);
        assert_eq!(cfg.pre, 15);
        let mut d = ImpulseDetector::new(cfg).unwrap();
        let mut x = noise(400_000, 0.004, 11);
        for k in 0..400 {
            let a = (-(k as f64) / 30.0).exp() as f32 * 0.5;
            x[300_000 + k] += Complex32::new(a, 0.0);
        }
        let mut out = Vec::new();
        d.process(&x, 0, &mut out);
        d.flush(&mut out);
        assert_eq!(out.len(), 1);
        let imp = &out[0];
        assert_eq!(imp.samples_from, imp.start - 15);
        let e = leading_edge(&imp.samples, imp.floor, &mut Vec::new()).unwrap();
        let onset = 300_000.0 - imp.samples_from as f64;
        assert!((e.edge - onset).abs() < 1.5, "{} {}", e.edge, onset);
        assert!(e.peak_db > 35.0);
    }

    #[test]
    fn the_edge_frequency_and_width_of_a_tone_burst() {
        let f = 0.05;
        let x: Vec<Complex32> = (0..200)
            .map(|k| {
                let a = if (50..150).contains(&k) { 1.0 } else { 0.0 };
                Complex32::from_polar(a, (TAU * f * k as f64) as f32)
            })
            .collect();
        let e = leading_edge(&x, 1e-4, &mut Vec::new()).unwrap();
        assert!((e.frequency - f).abs() < 1e-4);
        assert!((e.width - 100.0).abs() <= 2.0, "{}", e.width);
        assert!((e.edge - 49.0).abs() < 1.0, "{}", e.edge);
        assert!((0.02..=5.0).contains(&e.sigma));
    }

    #[test]
    fn a_train_goes_on_and_off() {
        let mut t = PeriodicTrain::new(TrainConfig::mains());
        let mut hits = 0;
        for i in 0..40 {
            hits += usize::from(t.push(i as f64 / 120.0));
        }
        assert_eq!(hits, 39);
        assert!(t.is_on());
        assert!((t.period().unwrap() - 1.0 / 120.0).abs() < 1e-12);
        // Impulses far apart: few hits by chance on the grid, and the train goes off.
        let mut chance = 0;
        for i in 0..60 {
            chance += usize::from(t.push(1.0037 + i as f64 * 0.37));
        }
        assert!(chance < 15, "{chance}");
        assert!(!t.is_on());
        assert!(t.period().is_none());
    }
}

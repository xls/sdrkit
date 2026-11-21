//! Pulse detection: complex samples to [`Train`]s of marks and spaces, OOK and FSK.

use crate::train::{quantile, Modulation, Pulse, Train};
use crate::{Error, Param};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::kmeans::kmeans_1d_fixed;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::stats::centred_mean;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::fsk::TonePair;
use sdrkit_algo_modem::ook::{AskSlicer, OokConfig, OokSlicer};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// Settings of [`PulseDetector`] (module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetectorConfig {
    /// The sample rate, Hz.
    pub rate_hz: f64,
    /// `Some` to make trains of one modulation only, `None` to tell OOK from FSK for each pulse.
    pub modulation: Option<Modulation>,
    /// A pulse opens this far above the noise floor, dB.
    pub open_db: f32,
    /// An open pulse closes below this level above the floor, dB (at most `open_db`).
    pub close_db: f32,
    /// The power average, µs.
    pub smooth_us: f32,
    /// Samples the power must stay below the close level before a pulse ends.
    pub hold: usize,
    /// Power values per noise floor estimate.
    pub floor_chunk: usize,
    /// An OOK train ends when no pulse starts for this long, µs.
    pub reset_us: f32,
    /// Trains with fewer pulses are dropped.
    pub min_pulses: usize,
    /// A train that reaches this many pulses leaves at once, and a new train starts.
    pub max_pulses: usize,
    /// The shortest carrier tested as FSK, µs.
    pub min_fsk_us: f32,
    /// The longest carrier tested as FSK, µs (this sets the history the detector keeps).
    pub max_fsk_us: f32,
    /// The smallest FSK deviation, Hz.
    pub min_deviation_hz: f64,
    /// Fold slivers before a train leaves.
    pub fold: bool,
}

impl DetectorConfig {
    /// Settings for `rate_hz`: OOK and FSK, open at 10 dB and close at 6 dB, an 8 µs average, a hold of 2 samples, the floor from
    /// chunks of 256 values, a 10 ms reset, 3 to 1024 pulses, FSK carriers from 200 µs to 100 ms with at least 4 kHz deviation,
    /// slivers folded.
    pub fn new(rate_hz: f64) -> Self {
        DetectorConfig {
            rate_hz,
            modulation: None,
            open_db: 10.0,
            close_db: 6.0,
            smooth_us: 8.0,
            hold: 2,
            floor_chunk: 256,
            reset_us: 10_000.0,
            min_pulses: 3,
            max_pulses: 1024,
            min_fsk_us: 200.0,
            max_fsk_us: 100_000.0,
            min_deviation_hz: 4000.0,
            fold: true,
        }
    }
}

/// Re-sum the boxcar exactly after this many samples, so `f64` rounding never builds up.
const RESUM: u32 = 4096;
/// The fewest samples a sliver limit can be.
const MIN_SLIVER: f32 = 2.5;

/// Detects pulse trains in complex samples. The detector keeps its state between calls, so the trains do not depend on how the
/// input is split across calls.
///
/// The power, averaged over `smooth_us`, opens a pulse `open_db` above the noise floor and closes it below `close_db`. Each
/// edge moves to the crossing half way between the floor and the mean power of the pulse. A pulse of at least `min_fsk_us`
/// with two tones at least `min_deviation_hz` apart produces one FSK train. Other pulses join the OOK train, which ends when no
/// pulse starts for `reset_us`. Each train carries its start, its level, its SNR, and for FSK the tone midpoint and deviation.
///
/// ```
/// use sdrkit_proto_ookdev::{detect::{DetectorConfig, PulseDetector}, Modulation};
/// use sdrkit_core::Complex32;
/// // 1 Msps: quiet, then three 500 µs pulses with 1000 µs spaces, then quiet.
/// let mut x = vec![Complex32::new(0.001, 0.0); 20_000];
/// for k in 0..3 {
///    let a = 5000 + k * 1500;
///    for v in &mut x[a..a + 500] {
///        *v = Complex32::new(0.5, 0.0);
///    }
/// }
/// let mut d = PulseDetector::new(DetectorConfig::new(1e6)).unwrap();
/// let mut trains = Vec::new();
/// d.process(&x, |t| trains.push(t.clone()));
/// d.flush(|t| trains.push(t.clone()));
/// assert_eq!(trains.len(), 1);
/// assert_eq!(trains[0].modulation, Modulation::Ook);
/// assert!((trains[0].us(trains[0].pulses[0].mark) - 500.0).abs() < 2.0);
/// assert!((trains[0].us(trains[0].pulses[0].space) - 1000.0).abs() < 2.0);
/// ```
#[derive(Clone, Debug)]
pub struct PulseDetector {
    cfg: DetectorConfig,
    ook: OokSlicer,
    disc: Discriminator,
    // Power average.
    boxcar: Vec<f32>,
    box_at: usize,
    box_sum: f64,
    since_resum: u32,
    delay: f64,
    // History of smoothed power and raw frequency (radians per sample), by stream index.
    ring_p: Vec<f32>,
    ring_f: Vec<f32>,
    mask: u64,
    n: u64,
    // The OOK train being built.
    train: Train,
    last_end: f64,
    pow_sum: f64,
    mark_sum: f64,
    floor_sum: f64,
    // The FSK train and scratch.
    fsk: Train,
    fa: Vec<f32>,
    fb: Vec<f32>,
    sorted: Vec<f32>,
    fold: Vec<f32>,
    reset_samples: f64,
    min_fsk: f64,
    // Counters.
    trains: u64,
    dropped: u64,
}

impl PulseDetector {
    /// A detector with these settings. Errors for a rate that is not positive, thresholds out of order, or lengths that are not
    /// positive.
    pub fn new(cfg: DetectorConfig) -> Result<Self, Error> {
        let pos = |v: f32| v.is_finite() && v > 0.0;
        if !(cfg.rate_hz.is_finite() && cfg.rate_hz > 0.0)
            || !pos(cfg.smooth_us)
            || !pos(cfg.reset_us)
            || !pos(cfg.max_fsk_us)
            || !(cfg.min_fsk_us.is_finite() && cfg.min_fsk_us >= 0.0)
            || !(cfg.min_deviation_hz.is_finite() && cfg.min_deviation_hz >= 0.0)
            || cfg.max_pulses < 2
        {
            return Err(Error::BadParameter(Param::Detector));
        }
        if cfg.close_db > cfg.open_db {
            return Err(Error::BadParameter(Param::Thresholds));
        }
        let ook = OokSlicer::new(OokConfig {
            open_db: cfg.open_db,
            close_db: cfg.close_db,
            hold: cfg.hold,
            floor_chunk: cfg.floor_chunk.max(1),
            ..OokConfig::default()
        })
        .map_err(Error::Modem)?;
        let us = cfg.rate_hz * 1e-6;
        let len = ((cfg.smooth_us as f64 * us).round() as usize).max(1);
        let fsk_len = (cfg.max_fsk_us as f64 * us).ceil() as usize;
        let cap = (fsk_len + len + 16).next_power_of_two().max(1024);
        let mk = |m: Modulation| {
            let mut t = Train::new(m, cfg.rate_hz);
            t.pulses.reserve(cfg.max_pulses);
            t
        };
        Ok(PulseDetector {
            ook,
            disc: Discriminator::new(1.0, 1.0 / TAU).map_err(Error::Dsp)?,
            boxcar: vec![0.0; len],
            box_at: 0,
            box_sum: 0.0,
            since_resum: 0,
            delay: (len as f64 - 1.0) / 2.0,
            ring_p: vec![0.0; cap],
            ring_f: vec![0.0; cap],
            mask: cap as u64 - 1,
            n: 0,
            train: mk(Modulation::Ook),
            last_end: 0.0,
            pow_sum: 0.0,
            mark_sum: 0.0,
            floor_sum: 0.0,
            fsk: mk(Modulation::Fsk),
            fa: Vec::with_capacity(cap),
            fb: Vec::with_capacity(cap),
            sorted: Vec::with_capacity(cap),
            fold: Vec::with_capacity(2 * cfg.max_pulses.max(cap / 2)),
            reset_samples: cfg.reset_us as f64 * us,
            min_fsk: (cfg.min_fsk_us as f64 * us).max(16.0),
            trains: 0,
            dropped: 0,
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &DetectorConfig {
        &self.cfg
    }

    /// The noise floor, mean power per sample.
    pub fn floor(&self) -> f32 {
        self.ook.floor()
    }

    /// Sets the noise floor (mean power per sample), for example from the quiet samples before a stored burst.
    pub fn set_floor(&mut self, power: f32) {
        self.ook.set_floor(power);
    }

    /// Samples taken since the start or the last [`reset`](Self::reset).
    pub fn samples_seen(&self) -> u64 {
        self.n
    }

    /// The number of trains output since the start.
    pub fn trains(&self) -> u64 {
        self.trains
    }

    /// Trains dropped for having fewer than `min_pulses` pulses.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Forgets the signal, the floor and any train being built. Stream indices start again at 0.
    pub fn reset(&mut self) {
        self.ook.reset();
        self.disc.reset();
        self.boxcar.fill(0.0);
        self.box_at = 0;
        self.box_sum = 0.0;
        self.since_resum = 0;
        self.n = 0;
        self.train.pulses.clear();
    }

    /// Takes samples and calls `f` with every train that ends in them.
    pub fn process(&mut self, x: &[Complex32], mut f: impl FnMut(&Train)) {
        let fsk = self.cfg.modulation != Some(Modulation::Ook);
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            let at = (self.n & self.mask) as usize;
            self.ring_f[at] = if fsk {
                self.disc.process_sample(v)
            } else {
                0.0
            };
            let p = self.smooth(v.norm_sqr());
            self.ring_p[at] = p;
            if let Some(q) = self.ook.push(p) {
                self.on_pulse(q.start, q.width, q.power, q.floor, &mut f);
            }
            self.n += 1;
            if !self.train.pulses.is_empty()
                && !self.ook.is_on()
                && self.n as f64 - self.delay - self.last_end > self.reset_samples
            {
                self.emit_ook(&mut f);
            }
        }
    }

    /// Ends the train being built (the end of a stream or of a stored burst) and calls `f` with it.
    pub fn flush(&mut self, mut f: impl FnMut(&Train)) {
        if !self.train.pulses.is_empty() {
            self.emit_ook(&mut f);
        }
    }

    /// The burst form: forgets the stream, sets the floor to `floor` (mean power per sample, from the samples before the burst),
    /// runs `x` and ends the train. Stream indices count from `x[0]`.
    pub fn run_burst(&mut self, x: &[Complex32], floor: f32, mut f: impl FnMut(&Train)) {
        self.reset();
        if floor.is_finite() && floor > 0.0 {
            self.set_floor(floor);
        }
        self.process(x, &mut f);
        // The tail of the burst: let an open pulse close.
        let tail = self.cfg.hold + self.boxcar.len() + 2;
        for _ in 0..tail {
            self.process(&[Complex32::default()], &mut f);
        }
        self.flush(&mut f);
    }

    #[inline]
    fn smooth(&mut self, p: f32) -> f32 {
        let n = self.boxcar.len();
        self.box_sum += p as f64 - self.boxcar[self.box_at] as f64;
        self.boxcar[self.box_at] = p;
        self.box_at = if self.box_at + 1 == n {
            0
        } else {
            self.box_at + 1
        };
        self.since_resum += 1;
        if self.since_resum >= RESUM {
            self.since_resum = 0;
            self.box_sum = self.boxcar.iter().map(|&v| v as f64).sum();
        }
        (self.box_sum / n as f64).max(0.0) as f32
    }

    /// The smoothed power at stream index `k`, when the history still holds it.
    #[inline]
    fn p_at(&self, k: i64) -> Option<f32> {
        let oldest = self.n as i64 - self.ring_p.len() as i64;
        (k >= 0.max(oldest) && k < self.n as i64)
            .then(|| self.ring_p[(k as u64 & self.mask) as usize])
    }

    /// Moves the edges of a pulse to the half-amplitude crossings (module documentation, step 3).
    fn refine(&self, start: f64, width: f64, power: f32, floor: f32) -> (f64, f64) {
        // The boxcar turns a step of power into a straight ramp, so half the power is the middle of the ramp.
        let mid = 0.5 * (floor.max(0.0) + power.max(0.0));
        let reach = self.boxcar.len() as i64 + self.cfg.hold as i64 + 3;
        let mut s = start;
        let k0 = start.floor() as i64 - 1;
        for k in k0..k0 + reach {
            match (self.p_at(k - 1), self.p_at(k)) {
                (Some(a), Some(b)) if b >= mid => {
                    if a < mid {
                        s = (k - 1) as f64 + ((mid - a) / (b - a)) as f64;
                    }
                    break;
                }
                (_, None) => break,
                _ => {}
            }
        }
        let mut e = start + width;
        let e0 = (start + width).ceil() as i64 + 1;
        let mut k = e0;
        while k > e0 - reach {
            if let (Some(a), Some(b)) = (self.p_at(k), self.p_at(k + 1)) {
                if a >= mid {
                    if b < mid {
                        e = k as f64 + ((a - mid) / (a - b)) as f64;
                    }
                    break;
                }
            }
            k -= 1;
        }
        if e <= s {
            return (start, start + width);
        }
        (s, e)
    }

    fn on_pulse(
        &mut self,
        start: f64,
        width: f64,
        power: f32,
        floor: f32,
        f: &mut impl FnMut(&Train),
    ) {
        let (s, e) = self.refine(start, width, power, floor);
        let (s, e) = (s - self.delay, e - self.delay);
        let fsk_ok = self.cfg.modulation != Some(Modulation::Ook)
            && e - s >= self.min_fsk
            && s >= self.n as f64 - self.ring_f.len() as f64 + 2.0;
        if fsk_ok && self.try_fsk(s, e, power, floor) {
            if !self.train.pulses.is_empty() {
                self.emit_ook(f);
            }
            self.emit_fsk(f);
            return;
        }
        if self.cfg.modulation == Some(Modulation::Fsk) {
            return;
        }
        if self.train.pulses.len() >= self.cfg.max_pulses {
            self.emit_ook(f);
        }
        if let Some(last) = self.train.pulses.last_mut() {
            last.space = (s - self.last_end).max(0.0) as f32;
        } else {
            self.train.start = s;
            self.pow_sum = 0.0;
            self.mark_sum = 0.0;
            self.floor_sum = 0.0;
        }
        let w = e - s;
        self.train.pulses.push(Pulse::new(w as f32, 0.0));
        self.last_end = e;
        self.pow_sum += power as f64 * w;
        self.mark_sum += w;
        self.floor_sum += floor as f64 * w;
    }

    fn finish_levels(t: &mut Train, pow: f64, floor: f64) {
        t.level_dbfs = (10.0 * pow.max(1e-30).log10()) as f32;
        t.snr_db = (10.0 * (pow / floor.max(1e-30)).max(1e-30).log10()) as f32;
    }

    fn emit_ook(&mut self, f: &mut impl FnMut(&Train)) {
        let gap = (self.n as f64 - self.delay - self.last_end).max(0.0) as f32;
        if let Some(last) = self.train.pulses.last_mut() {
            last.space = gap;
        }
        if self.cfg.fold {
            self.train.fold_slivers(MIN_SLIVER, &mut self.fold);
        }
        if self.train.pulses.len() >= self.cfg.min_pulses {
            let m = self.mark_sum.max(1e-12);
            Self::finish_levels(&mut self.train, self.pow_sum / m, self.floor_sum / m);
            self.train.offset_hz = 0.0;
            self.train.deviation_hz = 0.0;
            self.trains += 1;
            f(&self.train);
        } else {
            self.dropped += 1;
        }
        self.train.pulses.clear();
    }

    fn emit_fsk(&mut self, f: &mut impl FnMut(&Train)) {
        if self.cfg.fold {
            self.fsk.fold_slivers(MIN_SLIVER, &mut self.fold);
        }
        if self.fsk.pulses.len() >= self.cfg.min_pulses {
            self.trains += 1;
            f(&self.fsk);
        } else {
            self.dropped += 1;
        }
    }

    /// The FSK test and slicing of one carrier from `s` to `e` (module documentation, step 4). Fills `self.fsk` and returns
    /// true when the carrier is FSK.
    fn try_fsk(&mut self, s: f64, e: f64, power: f32, floor: f32) -> bool {
        // Two samples in from each edge: the phase steps where the carrier rises and falls are not tones.
        let a = s.ceil().max(0.0) as u64 + 2;
        let b = (e.floor().max(0.0) as u64).saturating_sub(2).min(self.n);
        if b <= a + 8 {
            return false;
        }
        self.fa.clear();
        for k in a..b {
            self.fa.push(self.ring_f[(k & self.mask) as usize]);
        }
        // Median of three removes discriminator clicks.
        self.fb.clear();
        self.fb.extend_from_slice(&self.fa);
        for k in 1..self.fa.len() - 1 {
            let (x, y, z) = (self.fa[k - 1], self.fa[k], self.fa[k + 1]);
            self.fb[k] = x.max(y).min(x.min(y).max(z));
        }
        let mut w = 3usize;
        let mut tones = None;
        for pass in 0..2 {
            centred_mean(&self.fb, w / 2, &mut self.fa);
            let (lo, hi, share) = two_means(&self.fa, &mut self.sorted);
            let dev = (hi - lo) as f64 / 2.0 * self.cfg.rate_hz / TAU;
            // Both tones must hold a tenth of the carrier: a few clicks at the edges are not a tone.
            if !(dev >= self.cfg.min_deviation_hz && dev > 0.0 && share >= 0.1) {
                break;
            }
            let mid = (lo + hi) / 2.0;
            tones = Some((mid, dev));
            if pass == 1 {
                break;
            }
            // The chip from the tone runs.
            self.sorted.clear();
            let mut cur = self.fa[0] > mid;
            let mut len = 0f32;
            for &v in &self.fa {
                if (v > mid) == cur {
                    len += 1.0;
                } else {
                    self.sorted.push(len);
                    cur = v > mid;
                    len = 1.0;
                }
            }
            if self.sorted.len() < 8 {
                break;
            }
            let m = quantile(&mut self.sorted, 0.5);
            self.sorted.retain(|&r| r >= m / 10.0);
            let chip = quantile(&mut self.sorted, 0.25);
            let nw = ((chip / 3.0).round() as usize).clamp(1, 64);
            if nw == w {
                break;
            }
            w = nw;
        }
        let Some((mid, dev)) = tones else {
            return false;
        };
        // `fa` holds the average of the last pass that found the tones.
        let t = &mut self.fsk;
        t.pulses.clear();
        let Some(first) = self.fa.iter().position(|&v| v > mid) else {
            return false;
        };
        let n = self.fa.len();
        let mut k = first;
        while k < n && t.pulses.len() < self.cfg.max_pulses {
            let m0 = k;
            while k < n && self.fa[k] > mid {
                k += 1;
            }
            let g0 = k;
            while k < n && self.fa[k] <= mid {
                k += 1;
            }
            t.pulses.push(Pulse::new((g0 - m0) as f32, (k - g0) as f32));
        }
        t.start = a as f64 + first as f64;
        t.offset_hz = mid as f64 * self.cfg.rate_hz / TAU;
        t.deviation_hz = dev;
        Self::finish_levels(t, power as f64, floor as f64);
        true
    }
}

/// The share of the gap between the two levels, either side of the half-way threshold, that an edge of [`level_train`] must
/// cross: on at 55 %, off at 45 %.
const LEVEL_HYSTERESIS: f32 = 0.05;
/// The high level of [`level_train`] must be this many times the low level.
const LEVEL_MIN_RATIO: f32 = 1.6;

/// The OOK train of a stored burst from its magnitude envelope `env` at `rate_hz`, sliced between its two levels and not at a
/// distance from the noise floor: at a low signal-to-noise ratio the samples before a burst are not known to be quiet (the
/// burst detector can open late), and the floor of a narrow envelope is known only roughly; the two levels are not.
///
/// The levels are the two means of the envelope's values. A two-level slicer (`sdrkit_algo_modem::ook::AskSlicer`) that starts
/// with them decides each value with a hysteresis of 5 % of the gap; runs under 2 values and slivers are folded. `None` when
/// the smaller group holds under 5 % of the values, the high level is under 1.6 times the low level, or fewer than 3 pulses
/// are found. Noise alone also has two means: the caller keeps a train only when a protocol decodes it.
pub fn level_train(env: &[f32], rate_hz: f64) -> Option<Train> {
    if env.len() < 16 {
        return None;
    }
    let (low, high, share) = two_means(env, &mut Vec::new());
    if !(share >= 0.05 && high > LEVEL_MIN_RATIO * low && low >= 0.0) {
        return None;
    }
    // The levels follow the envelope over about 4 ms.
    let alpha = (1.0 / (4e-3 * rate_hz)).clamp(1e-6, 0.5) as f32;
    let mut slicer = AskSlicer::new(low, high, alpha, LEVEL_HYSTERESIS).ok()?;
    let mut t = Train::new(Modulation::Ook, rate_hz);
    let mut on = false;
    let mut edge = 0usize;
    let mut started = false;
    for (k, &v) in env.iter().enumerate() {
        let now = slicer.push(v).0 == 1;
        if now == on {
            continue;
        }
        let run = (k - edge) as f32;
        if now {
            match t.pulses.last_mut() {
                Some(p) => p.space = run,
                None => t.start = k as f64,
            }
            started = true;
        } else if started {
            t.pulses.push(Pulse::new(run, 0.0));
        }
        on = now;
        edge = k;
    }
    if on && started {
        t.pulses.push(Pulse::new((env.len() - edge) as f32, 0.0));
    } else if let Some(p) = t.pulses.last_mut() {
        p.space = (env.len() - edge) as f32;
    }
    t.despike(2.0);
    t.fold_slivers(MIN_SLIVER, &mut Vec::new());
    if t.pulses.len() < 3 {
        return None;
    }
    t.level_dbfs = 20.0 * high.max(1e-15).log10();
    t.snr_db = 20.0 * (high / low.max(1e-15)).log10();
    Some(t)
}

/// The second tone of [`burst_tones`] must hold this share of the first one's power over the noise.
const TONE_MIN_SHARE: f32 = 0.25;

/// The two tones of an FSK burst `x` at `rate_hz` from its averaged spectrum (Hann frames of about 256 µs, half overlapped):
/// the strongest bin and the strongest bin at least `min_deviation_hz` to either side of it that is a local maximum, each
/// moved to the power centroid of its five bins over the noise (the median bin). Returns the midpoint and the deviation, Hz.
/// `None` when the burst is shorter than a frame, or the second tone holds under a quarter of the first one's power over the
/// noise.
pub fn burst_tones(x: &[Complex32], rate_hz: f64, min_deviation_hz: f64) -> Option<(f64, f64)> {
    let n = ((256e-6 * rate_hz) as usize).next_power_of_two().max(16);
    let mut p = vec![0f32; n];
    welch(x, n, Window::Hann, 0.5, Scaling::Power, &mut p).ok()?;
    let mut sorted = p.clone();
    let noise = quantile(&mut sorted, 0.5);
    let over = |k: usize| (p[k] - noise).max(0.0);
    let first = (2..n - 2).max_by(|&a, &b| p[a].total_cmp(&p[b]))?;
    let gap = ((2.0 * min_deviation_hz * n as f64 / rate_hz).ceil() as usize).max(2);
    let second = (2..n - 2)
        .filter(|&k| k.abs_diff(first) >= gap && p[k] >= p[k - 1] && p[k] >= p[k + 1])
        .max_by(|&a, &b| p[a].total_cmp(&p[b]))?;
    if over(second) < TONE_MIN_SHARE * over(first) || over(second) <= 0.0 {
        return None;
    }
    let centre = |k: usize| {
        let (mut w, mut m) = (0f64, 0f64);
        for j in k - 2..=k + 2 {
            w += f64::from(over(j));
            m += f64::from(over(j)) * j as f64;
        }
        (m / w.max(1e-30) - (n / 2) as f64) * rate_hz / n as f64
    };
    let (a, b) = (centre(first), centre(second));
    Some((0.5 * (a + b), 0.5 * (a - b).abs()))
}

/// The FSK train of a stored burst `x` at `rate_hz` from the energies of its two tones (`sdrkit_algo_modem::fsk::TonePair` at
/// `mid_hz +- deviation_hz`) over a sliding window of `window` samples: non-coherent detection over about a chip, which reads
/// a signal that the discriminator cannot.
///
/// The carrier is up where the sum of the two energies is over the half-way point of its two means; the train spans the first
/// to the last such window. Inside the span a mark is a run of windows with more energy in the upper tone, a space a run with
/// more in the lower. Runs under 2 samples and slivers are folded. `None` when the tones cannot be set, the carrier's two
/// levels are under 1.6 apart, or fewer than 3 pulses are found.
pub fn tone_train(
    x: &[Complex32],
    rate_hz: f64,
    mid_hz: f64,
    deviation_hz: f64,
    window: usize,
) -> Option<Train> {
    if window == 0 || x.len() < 4 * window {
        return None;
    }
    let mut pair = TonePair::new(rate_hz, mid_hz, deviation_hz, window).ok()?;
    let n = x.len() - window + 1;
    let mut sum = Vec::with_capacity(n);
    let mut upper = Vec::with_capacity(n);
    for (k, &z) in x.iter().enumerate() {
        pair.push(z);
        if k + 1 >= window {
            let [u, l] = pair.energies(window, 0)?;
            sum.push(u + l);
            upper.push(u > l);
        }
    }
    let (low, high, share) = two_means(&sum, &mut Vec::new());
    if !(share >= 0.05 && high > LEVEL_MIN_RATIO * low) {
        return None;
    }
    let gate = 0.5 * (low + high);
    let a = sum.iter().position(|&v| v > gate)?;
    let e = sum.iter().rposition(|&v| v > gate)? + 1;
    let first = a + upper[a..e].iter().position(|&u| u)?;
    let mut t = Train::new(Modulation::Fsk, rate_hz);
    // Window `k` covers the samples `k .. k + window`.
    t.start = first as f64 + 0.5 * window as f64;
    t.offset_hz = mid_hz;
    t.deviation_hz = deviation_hz;
    let mut k = first;
    while k < e && t.pulses.len() < 4096 {
        let m0 = k;
        while k < e && upper[k] {
            k += 1;
        }
        let g0 = k;
        while k < e && !upper[k] {
            k += 1;
        }
        t.pulses.push(Pulse::new((g0 - m0) as f32, (k - g0) as f32));
    }
    t.despike(2.0);
    t.fold_slivers(MIN_SLIVER, &mut Vec::new());
    if t.pulses.len() < 3 {
        return None;
    }
    t.level_dbfs = 10.0 * (high / window as f32).max(1e-30).log10();
    t.snr_db = 10.0 * (high / low.max(1e-30)).log10();
    Some(t)
}

/// Two levels of `v` by at most 12 rounds of two-means started at the 10th and 90th percentiles, and the share of the smaller
/// of the two groups (0 to 0.5).
fn two_means(v: &[f32], scratch: &mut Vec<f32>) -> (f32, f32, f64) {
    scratch.clear();
    scratch.extend_from_slice(v);
    let lo = quantile(scratch, 0.1);
    let hi = quantile(scratch, 0.9);
    match kmeans_1d_fixed(v, [f64::from(lo), f64::from(hi)], 12) {
        Some(c) => (
            c.centres[0] as f32,
            c.centres[1] as f32,
            c.counts[0].min(c.counts[1]) as f64 / v.len() as f64,
        ),
        None => (lo, hi, 0.0),
    }
}

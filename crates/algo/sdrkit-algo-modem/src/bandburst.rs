//! Bursts anywhere in a band: short FFT frames of the whole input, a noise floor for each bin, and the bursts that rise over it
//! with their centre frequencies.
//!
//! Each frame of [`BandConfig::frame`] samples, taken every [`BandConfig::hop`] samples, is Hann-windowed and transformed. The
//! power of a bin is `|X|²`, or with [`BandConfig::smooth`] the weighted sum 1/4, 1/2, 1/4 of it and its two neighbours. A bin
//! is **lit** when its power exceeds its floor by [`BandConfig::threshold_db`]; the bins within [`BandConfig::dc_guard`] of
//! 0 Hz are never lit. The floor follows [`FloorRule`].
//!
//! [`Grouping`] says how lit bins make bursts:
//!
//! - [`Grouping::Band`]: one burst over the whole band at a time. It opens on the first lit frame and closes after
//!   [`BandConfig::close_frames`] frames without a lit bin. At the close the excess power of each bin over its floor, summed over
//!   the burst, gives up to `signals` signals: the strongest bin, its power-weighted centroid within `centroid_hz`, and the bins
//!   within `clear_hz` removed before the next; a signal under `min_share` of the first ends the list.
//! - [`Grouping::Segments`]: runs of lit bins with at most `gap_bins` unlit bins inside are segments. A segment that touches
//!   (within one bin) the bins of an open burst (those of all its segments so far) continues it, and one that touches two
//!   open bursts joins them; any other opens a new one, so bursts on several frequencies at once are kept apart. A burst
//!   closes after [`BandConfig::close_frames`] frames without a segment. Its centre is the power-weighted centroid over its
//!   frames, its width the mean width of its segments.
//!
//! A burst lit in fewer than [`BandConfig::min_frames`] frames is dropped. A burst longer than [`BandConfig::max_len`] samples
//! is handled as [`BandConfig::long`] says. Each burst is given with [`BandConfig::pre`] samples before its first frame and
//! [`BandConfig::post`] after its last lit frame, as far as the history of [`BandConfig::history`] samples holds them. Nothing is
//! allocated after [`BandDetector::new`].
//!
//! ```
//! use sdrkit_algo_modem::bandburst::{BandConfig, BandDetector};
//! use sdrkit_algo_modem::Complex32;
//! let mut d = BandDetector::new(BandConfig::new(1e6, 64)).unwrap();
//! let mut s = 1u32;
//! let mut u = move || {
//!     s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
//!     1e-2 * ((s >> 8) as f32 / (1u32 << 24) as f32 - 0.5)
//! };
//! let mut x: Vec<Complex32> = (0..60_000).map(|_| Complex32::new(u(), u())).collect();
//! for (i, v) in x[30_000..32_000].iter_mut().enumerate() {
//!     let p = std::f32::consts::TAU * 0.25 * i as f32;
//!     *v += Complex32::new(p.cos(), p.sin());
//! }
//! let mut found = Vec::new();
//! d.process(&x, |b| found.push(b.offset_hz));
//! assert_eq!(found.len(), 1);
//! assert!((found[0] - 250e3).abs() < 20e3);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::window::Window;

/// How the floor of a bin starts and follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorRule {
    /// Frames over which the floor learns; nothing is detected in them (at least 1).
    pub learn_frames: u32,
    /// The weight of a new frame while the floor learns (after the first, which sets it).
    pub learn_alpha: f32,
    /// The first frame sets every bin to the median power of that frame, not to the bin's own power.
    pub start_median: bool,
    /// The weight of a new frame for a bin that is not lit (and, with `hold_while_open`, while no burst is open).
    pub idle_alpha: f32,
    /// The rule for a lit bin (and, with `hold_while_open`, for every bin while a burst is open).
    pub busy: BusyFloor,
    /// While a burst is open, every bin follows `busy` ([`Grouping::Band`]), or the bins of each open burst and their
    /// neighbours do ([`Grouping::Segments`]).
    pub hold_while_open: bool,
}

/// How the floor of a busy bin moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BusyFloor {
    /// Towards the power with this weight.
    Follow(f32),
    /// Up by this factor each frame, so that a carrier that stays becomes floor.
    Grow(f32),
}

/// How lit bins make bursts (module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grouping {
    /// One burst over the band, with up to `signals` signals found at the close.
    Band {
        /// The most signals of one burst.
        signals: usize,
        /// The centroid of a signal spans this far from its strongest bin, Hz.
        centroid_hz: f64,
        /// Bins this near a signal are taken by it, Hz.
        clear_hz: f64,
        /// A signal with less than this share of the first one's excess ends the list.
        min_share: f32,
    },
    /// Each run of lit bins is a burst of its own.
    Segments {
        /// Unlit bins that a segment may hold inside.
        gap_bins: usize,
        /// The most bursts open at once.
        max_open: usize,
    },
}

/// What happens to a burst longer than [`BandConfig::max_len`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongBurst {
    /// It closes there and is given like any other.
    Close,
    /// It is dropped, and its lit bins take their power as floor (a carrier that stays).
    Absorb,
}

/// The settings of a [`BandDetector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandConfig {
    /// The sample rate, Hz.
    pub rate_hz: f64,
    /// Samples per FFT frame: a power of two from 16 to 8192.
    pub frame: usize,
    /// Samples from one frame to the next: `frame` or `frame / 2`.
    pub hop: usize,
    /// The power of a bin is smoothed with its neighbours (1/4, 1/2, 1/4).
    pub smooth: bool,
    /// Bins this near 0 Hz are never lit.
    pub dc_guard: usize,
    /// How far over its floor a bin is lit, dB.
    pub threshold_db: f64,
    /// The floor.
    pub floor: FloorRule,
    /// The least floor, as a mean power per input sample: input of exact zeros, or far below any radio's noise, does not
    /// light bins.
    pub floor_min: f32,
    /// How lit bins make bursts.
    pub grouping: Grouping,
    /// Frames without the burst that close it.
    pub close_frames: u32,
    /// Bursts lit in fewer frames are dropped.
    pub min_frames: u32,
    /// The longest burst, samples from its first frame.
    pub max_len: usize,
    /// What a longer burst does.
    pub long: LongBurst,
    /// Samples given before the first frame of a burst.
    pub pre: usize,
    /// Samples given after the last lit frame of a burst.
    pub post: usize,
    /// Samples of history (at least `pre + max_len + post` and the frames to the close).
    pub history: usize,
}

impl BandConfig {
    /// Segments at `rate_hz` with frames of `frame` samples overlapping by half, smoothed power, a 9 dB threshold, a floor that
    /// starts at the median of the first frame, follows idle bins with 0.02 and grows by 0.2 % a frame under a lit bin or the
    /// bins of an open burst; a burst
    /// closes after 2 frames without its bins or at 80 ms, one-frame bursts are dropped, one frame of samples before and after,
    /// at most 16 bursts open, a least floor of 1e-12 (-120 dBFS).
    pub fn new(rate_hz: f64, frame: usize) -> BandConfig {
        let max_len = (80e-3 * rate_hz).round().max(1.0) as usize;
        BandConfig {
            rate_hz,
            frame,
            hop: (frame / 2).max(1),
            smooth: true,
            dc_guard: 0,
            threshold_db: 9.0,
            floor: FloorRule {
                learn_frames: 1,
                learn_alpha: 1.0,
                start_median: true,
                idle_alpha: 0.02,
                busy: BusyFloor::Grow(1.002),
                hold_while_open: true,
            },
            floor_min: 1e-12,
            grouping: Grouping::Segments {
                gap_bins: 1,
                max_open: 16,
            },
            close_frames: 2,
            min_frames: 2,
            max_len,
            long: LongBurst::Close,
            pre: frame,
            post: frame,
            history: max_len + 6 * frame,
        }
    }

    /// The smallest power of two from `least` to `most` whose bins are at most `bin_hz` wide at `rate_hz`.
    pub fn frame_for(rate_hz: f64, bin_hz: f64, least: usize, most: usize) -> usize {
        let mut n = least.max(16).next_power_of_two();
        while n < most && rate_hz / n as f64 > bin_hz {
            n *= 2;
        }
        n
    }
}

/// A finished burst, borrowed from the detector's history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandBurst<'a> {
    /// The samples at the input rate, signal at `offset_hz`.
    pub samples: &'a [Complex32],
    /// The stream index of `samples[0]` (counted from the first sample since `new` or `reset`).
    pub start: u64,
    /// Samples of `samples` before the first frame of the burst.
    pub lead: usize,
    /// The centre of the signal, Hz from the centre of the input.
    pub offset_hz: f64,
    /// The mean width of its lit bins, Hz (0 for [`Grouping::Band`]).
    pub width_hz: f64,
    /// The highest ratio of power to floor over the burst, dB.
    pub peak_db: f32,
    /// Frames in which it was lit.
    pub frames: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Open {
    /// Stream index of its first frame.
    start: u64,
    /// Stream index after its last lit frame.
    end: u64,
    lo: usize,
    hi: usize,
    idle: u32,
    frames: u32,
    weight: f64,
    moment: f64,
    width: f64,
    peak: f32,
    seen: bool,
}

/// The detector (module documentation).
pub struct BandDetector {
    cfg: BandConfig,
    fft: Fft,
    win: Vec<f32>,
    frame: Vec<Complex32>,
    raw: Vec<f32>,
    power: Vec<f32>,
    floor: Vec<f32>,
    lit: Vec<bool>,
    excess: Vec<f32>,
    sorted: Vec<f32>,
    open: Vec<Open>,
    signals: Vec<(f64, f32)>,
    hist: SampleRing<Complex32>,
    thr: f32,
    /// The least floor of a bin: `floor_min` times the energy of the window.
    least: f32,
    frames: u64,
    since: usize,
    bursts: u64,
    band: Option<Open>,
}

impl BandDetector {
    /// A detector with settings `cfg`. Errors for a rate that is not positive, a frame that is not a power of two from 16 to
    /// 8192, a hop that is neither the frame nor half of it, a threshold that is not finite, or no history.
    pub fn new(cfg: BandConfig) -> Result<BandDetector, ModemError> {
        Param::Rate.check_positive(cfg.rate_hz)?;
        let n = cfg.frame;
        if !n.is_power_of_two() || !(16..=8192).contains(&n) {
            return Param::FftSize.range(16.0, 8192.0, n as f64);
        }
        if cfg.hop != n && cfg.hop != n / 2 {
            return Param::Window.range((n / 2) as f64, n as f64, cfg.hop as f64);
        }
        Param::Threshold.check_finite(cfg.threshold_db)?;
        Param::MaxLen.check_count(cfg.max_len, 1, 1 << 26)?;
        let history = cfg.history.max(cfg.pre + cfg.max_len + cfg.post + 4 * n);
        let max_open = match cfg.grouping {
            Grouping::Segments { max_open, .. } => max_open.max(1),
            Grouping::Band { .. } => 1,
        };
        let signals = match cfg.grouping {
            Grouping::Band { signals, .. } => signals.min(n),
            Grouping::Segments { .. } => 0,
        };
        let win = Window::Hann.coefficients(n);
        let energy: f32 = win.iter().map(|w| w * w).sum();
        Ok(BandDetector {
            least: (cfg.floor_min.max(0.0) * energy).max(1e-30),
            fft: Fft::forward(n),
            win,
            frame: Vec::with_capacity(n),
            raw: vec![0.0; n],
            power: vec![0.0; n],
            floor: vec![1e-30; n],
            lit: vec![false; n],
            excess: vec![0.0; n],
            sorted: Vec::with_capacity(n),
            open: Vec::with_capacity(max_open),
            signals: Vec::with_capacity(signals),
            hist: SampleRing::new(history).map_err(ModemError::Dsp)?,
            thr: 10f32.powf(cfg.threshold_db as f32 / 10.0),
            frames: 0,
            since: 0,
            bursts: 0,
            band: None,
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &BandConfig {
        &self.cfg
    }

    /// The floor of each bin (FFT order, `|X|²` of a Hann-windowed frame).
    pub fn floor(&self) -> &[f32] {
        &self.floor
    }

    /// Frames analysed.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Bursts closed so far, given or not.
    pub fn bursts(&self) -> u64 {
        self.bursts
    }

    /// The stream index of the next sample.
    pub fn position(&self) -> u64 {
        self.hist.end()
    }

    /// Changes the threshold; the floor learns again.
    pub fn set_threshold_db(&mut self, db: f64) {
        self.cfg.threshold_db = db;
        self.thr = 10f32.powf(db as f32 / 10.0);
        self.frames = 0;
    }

    /// Forgets the stream, the floors and every open burst.
    pub fn reset(&mut self) {
        self.frame.clear();
        self.hist.reset();
        self.floor.fill(self.least);
        self.open.clear();
        self.band = None;
        self.frames = 0;
        self.since = 0;
    }

    /// The frequency of bin `k`, Hz.
    fn bin_hz(&self, k: usize) -> f64 {
        let n = self.cfg.frame;
        let s = if k < n / 2 { k as f64 } else { k as f64 - n as f64 };
        s * self.cfg.rate_hz / n as f64
    }

    /// The signed bin of FFT bin `k`.
    fn signed(&self, k: usize) -> i64 {
        let n = self.cfg.frame as i64;
        let k = k as i64;
        if k < n / 2 {
            k
        } else {
            k - n
        }
    }

    /// Takes `x`; each finished burst goes to `found`.
    pub fn process(&mut self, x: &[Complex32], mut found: impl FnMut(&BandBurst<'_>)) {
        let n = self.cfg.frame;
        let hop = self.cfg.hop;
        for &s in x {
            let s = if s.re.is_finite() && s.im.is_finite() {
                s
            } else {
                Complex32::default()
            };
            self.hist.push(s);
            self.since += 1;
            if self.since >= hop && self.hist.end() >= n as u64 {
                self.since = 0;
                let at = self.hist.end() - n as u64;
                self.frame.clear();
                self.frame.extend_from_slice(self.hist.last(n));
                self.analyse(at, &mut found);
            }
        }
    }

    fn spectrum(&mut self) {
        for (x, w) in self.frame.iter_mut().zip(&self.win) {
            *x *= *w;
        }
        self.fft.process(&mut self.frame);
        let n = self.cfg.frame;
        for (p, x) in self.raw.iter_mut().zip(&self.frame) {
            *p = x.norm_sqr();
        }
        if self.cfg.smooth {
            for k in 0..n {
                let a = self.raw[(k + n - 1) % n];
                let b = self.raw[(k + 1) % n];
                self.power[k] = 0.25 * a + 0.5 * self.raw[k] + 0.25 * b;
            }
        } else {
            self.power.copy_from_slice(&self.raw);
        }
    }

    fn learn(&mut self) {
        let r = self.cfg.floor;
        let least = self.least;
        if self.frames == 1 {
            if r.start_median {
                self.sorted.clear();
                self.sorted.extend_from_slice(&self.power);
                let m = self.sorted.len() / 2;
                let (_, v, _) = self.sorted.select_nth_unstable_by(m, f32::total_cmp);
                let v = v.max(least);
                self.floor.fill(v);
            } else {
                for (f, &p) in self.floor.iter_mut().zip(&self.power) {
                    *f = p.max(least);
                }
            }
        } else {
            let a = r.learn_alpha;
            for (f, &p) in self.floor.iter_mut().zip(&self.power) {
                *f = ((1.0 - a) * *f + a * p).max(least);
            }
        }
    }

    /// Bin `k` is over its floor now and outside the guard around 0 Hz.
    fn over(&self, k: usize) -> bool {
        let g = self.cfg.dc_guard as i64;
        (g == 0 || self.signed(k).abs() > g) && self.power[k] > self.floor[k] * self.thr
    }

    fn mark(&mut self) -> bool {
        let mut any = false;
        for k in 0..self.cfg.frame {
            let on = self.over(k);
            self.lit[k] = on;
            any |= on;
        }
        any
    }

    fn follow(&mut self, open: bool) {
        let r = self.cfg.floor;
        for k in 0..self.cfg.frame {
            let busy = self.lit[k] || (r.hold_while_open && open);
            let p = self.power[k];
            let f = &mut self.floor[k];
            *f = if busy {
                match r.busy {
                    BusyFloor::Follow(a) => (1.0 - a) * *f + a * p,
                    BusyFloor::Grow(g) => *f * g,
                }
            } else {
                (1.0 - r.idle_alpha) * *f + r.idle_alpha * p
            }
            .max(self.least);
        }
    }

    fn analyse(&mut self, at: u64, found: &mut impl FnMut(&BandBurst<'_>)) {
        self.spectrum();
        self.frames += 1;
        if self.frames <= u64::from(self.cfg.floor.learn_frames.max(1)) {
            self.learn();
            return;
        }
        let any = self.mark();
        match self.cfg.grouping {
            Grouping::Band { .. } => self.band_frame(at, any, found),
            Grouping::Segments { gap_bins, .. } => self.segment_frame(at, gap_bins, found),
        }
    }

    fn band_frame(&mut self, at: u64, any: bool, found: &mut impl FnMut(&BandBurst<'_>)) {
        let n = self.cfg.frame;
        if any && self.band.is_none() {
            self.band = Some(Open {
                start: at,
                ..Open::default()
            });
            self.excess.fill(0.0);
        }
        let open = self.band.is_some();
        if open {
            for k in 0..n {
                if self.lit[k] {
                    self.excess[k] += self.power[k] - self.floor[k];
                    let r = self.power[k] / self.floor[k];
                    if let Some(b) = self.band.as_mut() {
                        b.peak = b.peak.max(r);
                    }
                }
            }
        }
        self.follow(open);
        let Some(mut b) = self.band else {
            return;
        };
        if any {
            b.end = at + n as u64;
            b.idle = 0;
            b.frames += 1;
        } else {
            b.idle += 1;
        }
        self.band = Some(b);
        if (at + n as u64 - b.start) as usize > self.cfg.max_len {
            match self.cfg.long {
                LongBurst::Absorb => {
                    for k in 0..n {
                        if self.over(k) {
                            self.floor[k] = self.power[k];
                        }
                    }
                    self.band = None;
                }
                LongBurst::Close => self.close_band(found),
            }
            return;
        }
        if b.idle >= self.cfg.close_frames {
            self.close_band(found);
        }
    }

    fn close_band(&mut self, found: &mut impl FnMut(&BandBurst<'_>)) {
        let Some(b) = self.band.take() else {
            return;
        };
        self.bursts += 1;
        let Grouping::Band {
            signals,
            centroid_hz,
            clear_hz,
            min_share,
        } = self.cfg.grouping
        else {
            return;
        };
        if b.frames < self.cfg.min_frames {
            return;
        }
        let n = self.cfg.frame;
        self.signals.clear();
        let mut first = 0f32;
        for _ in 0..signals {
            let Some((kmax, emax)) = self
                .excess
                .iter()
                .copied()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(&b.1))
            else {
                break;
            };
            if emax <= 0.0 || (!self.signals.is_empty() && emax < min_share * first) {
                break;
            }
            if self.signals.is_empty() {
                first = emax;
            }
            let f0 = self.bin_hz(kmax);
            let (mut wsum, mut fsum) = (0f64, 0f64);
            for k in 0..n {
                let f = self.bin_hz(k);
                if (f - f0).abs() <= centroid_hz {
                    wsum += f64::from(self.excess[k]);
                    fsum += f64::from(self.excess[k]) * f;
                }
            }
            let fc = if wsum > 0.0 { fsum / wsum } else { f0 };
            for k in 0..n {
                if (self.bin_hz(k) - f0).abs() <= clear_hz {
                    self.excess[k] = 0.0;
                }
            }
            if self.signals.iter().any(|s| (s.0 - fc).abs() < clear_hz) {
                continue;
            }
            self.signals.push((fc, emax));
        }
        let from = b.start.saturating_sub(self.cfg.pre as u64).max(self.hist.start());
        let to = (b.end + self.cfg.post as u64).min(self.hist.end());
        if to <= from {
            return;
        }
        let Some(samples) = self.hist.window(from, (to - from) as usize) else {
            return;
        };
        for &(fc, _) in &self.signals {
            found(&BandBurst {
                samples,
                start: from,
                lead: (b.start.saturating_sub(from)) as usize,
                offset_hz: fc,
                width_hz: 0.0,
                peak_db: 10.0 * b.peak.max(1e-30).log10(),
                frames: b.frames,
            });
        }
    }

    fn segment_frame(&mut self, at: u64, gap: usize, found: &mut impl FnMut(&BandBurst<'_>)) {
        let n = self.cfg.frame;
        let max_open = match self.cfg.grouping {
            Grouping::Segments { max_open, .. } => max_open.max(1),
            Grouping::Band { .. } => 1,
        };
        for o in &mut self.open {
            o.seen = false;
        }
        // Segments in signed-bin order, from -n/2 to n/2 - 1.
        let mut k = 0usize;
        while k < n {
            let idx = (k + n / 2) % n;
            if !self.lit[idx] {
                k += 1;
                continue;
            }
            let lo = k;
            let mut hi = k;
            let mut j = k + 1;
            while j < n {
                if self.lit[(j + n / 2) % n] {
                    hi = j;
                    j += 1;
                } else if j - hi <= gap {
                    j += 1;
                } else {
                    break;
                }
            }
            self.segment(at, lo, hi, max_open);
            k = hi + 1;
        }
        if self.cfg.floor.hold_while_open {
            // The bins of an open burst count as busy, so a bin that dips under the threshold does not pull its floor up
            // towards the signal.
            for o in &self.open {
                for j in o.lo.saturating_sub(1)..=(o.hi + 1).min(n - 1) {
                    self.lit[(j + n / 2) % n] = true;
                }
            }
        }
        self.follow(false);
        let mut i = 0;
        while i < self.open.len() {
            let o = &mut self.open[i];
            if !o.seen {
                o.idle += 1;
            }
            let long = (at + n as u64 - o.start) as usize > self.cfg.max_len;
            if o.idle >= self.cfg.close_frames || long {
                let o = self.open.swap_remove(i);
                if long && self.cfg.long == LongBurst::Absorb {
                    for b in o.lo..=o.hi {
                        let idx = (b + n / 2) % n;
                        self.floor[idx] = self.floor[idx].max(self.power[idx]);
                    }
                    self.bursts += 1;
                    continue;
                }
                self.emit_segment(&o, found);
                continue;
            }
            i += 1;
        }
    }

    /// Adds the segment of shifted bins `lo..=hi` (bin `j` is FFT bin `(j + n/2) mod n`) to the open burst it touches, or
    /// opens one.
    fn segment(&mut self, at: u64, lo: usize, hi: usize, max_open: usize) {
        let n = self.cfg.frame;
        let half = n as f64 / 2.0;
        let bin = self.cfg.rate_hz / n as f64;
        let (mut w, mut m, mut peak) = (0f64, 0f64, 0f32);
        for j in lo..=hi {
            let idx = (j + n / 2) % n;
            if !self.lit[idx] {
                continue;
            }
            let e = f64::from((self.power[idx] - self.floor[idx]).max(0.0));
            w += e;
            m += e * (j as f64 - half) * bin;
            peak = peak.max(self.power[idx] / self.floor[idx]);
        }
        let touches = |o: &Open| lo <= o.hi + 1 && o.lo <= hi + 1;
        // A segment that touches two open bursts joins them: the two halves of one signal, split while a dip in its
        // spectrum stayed under the threshold.
        while let Some(first) = self.open.iter().position(touches) {
            let Some(k) = self.open[first + 1..].iter().position(touches) else {
                break;
            };
            let b = self.open.swap_remove(first + 1 + k);
            let a = &mut self.open[first];
            a.start = a.start.min(b.start);
            a.end = a.end.max(b.end);
            a.lo = a.lo.min(b.lo);
            a.hi = a.hi.max(b.hi);
            a.idle = a.idle.min(b.idle);
            a.frames = a.frames.max(b.frames);
            a.weight += b.weight;
            a.moment += b.moment;
            a.width = a.width.max(b.width);
            a.peak = a.peak.max(b.peak);
            a.seen |= b.seen;
        }
        let o = match self.open.iter().position(touches) {
            Some(i) => &mut self.open[i],
            None => {
                if self.open.len() >= max_open {
                    return;
                }
                self.open.push(Open {
                    start: at,
                    ..Open::default()
                });
                let last = self.open.len() - 1;
                &mut self.open[last]
            }
        };
        // The bins of a burst are those of all its segments so far: the two tones of an FSK signal, lit in turn, stay one
        // burst.
        if o.frames == 0 {
            (o.lo, o.hi) = (lo, hi);
        }
        o.lo = o.lo.min(lo);
        o.hi = o.hi.max(hi);
        o.width += (hi - lo + 1) as f64 * bin;
        if !o.seen {
            o.frames += 1;
        }
        o.seen = true;
        o.idle = 0;
        o.end = at + n as u64;
        o.weight += w;
        o.moment += m;
        o.peak = o.peak.max(peak);
    }

    fn emit_segment(&mut self, o: &Open, found: &mut impl FnMut(&BandBurst<'_>)) {
        self.bursts += 1;
        if o.frames < self.cfg.min_frames {
            return;
        }
        let from = o.start.saturating_sub(self.cfg.pre as u64).max(self.hist.start());
        let to = (o.end + self.cfg.post as u64).min(self.hist.end());
        if to <= from {
            return;
        }
        let Some(samples) = self.hist.window(from, (to - from) as usize) else {
            return;
        };
        let centre = if o.weight > 0.0 {
            o.moment / o.weight
        } else {
            let n = self.cfg.frame as f64;
            ((o.lo + o.hi) as f64 / 2.0 - n / 2.0) * self.cfg.rate_hz / n
        };
        found(&BandBurst {
            samples,
            start: from,
            lead: (o.start.saturating_sub(from)) as usize,
            offset_hz: centre,
            width_hz: o.width / f64::from(o.frames.max(1)),
            peak_db: 10.0 * o.peak.max(1e-30).log10(),
            frames: o.frames,
        });
    }
}

impl std::fmt::Debug for BandDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BandDetector")
            .field("cfg", &self.cfg)
            .field("frames", &self.frames)
            .field("bursts", &self.bursts)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    fn noise(n: usize, sigma: f32, seed: u64) -> Vec<Complex32> {
        let mut r = Rng::new(seed);
        (0..n)
            .map(|_| Complex32::new(sigma * r.normal_f32(), sigma * r.normal_f32()))
            .collect()
    }

    fn tone(x: &mut [Complex32], rate: f64, hz: f64, amp: f32) {
        for (i, v) in x.iter_mut().enumerate() {
            let p = std::f64::consts::TAU * hz / rate * i as f64;
            *v += Complex32::new(amp * p.cos() as f32, amp * p.sin() as f32);
        }
    }

    #[test]
    fn two_bursts_at_once_on_two_frequencies_are_kept_apart() {
        let rate = 8e6;
        let mut x = noise(400_000, 0.01, 3);
        tone(&mut x[100_000..110_000], rate, 1.5e6, 0.3);
        tone(&mut x[104_000..112_000], rate, -2.2e6, 0.3);
        let mut d = BandDetector::new(BandConfig::new(rate, 256)).unwrap();
        let mut got = Vec::new();
        d.process(&x, |b| got.push((b.offset_hz, b.start, b.samples.len())));
        got.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(got.len(), 2, "{got:?}");
        assert!((got[0].0 + 2.2e6).abs() < 40e3 && (got[1].0 - 1.5e6).abs() < 40e3, "{got:?}");
        assert!(got[1].1 <= 100_000 && got[1].1 + got[1].2 as u64 >= 110_000);
    }

    #[test]
    fn noise_alone_gives_nothing_and_a_carrier_becomes_floor() {
        let rate = 2.4e6;
        let mut x = noise(2_400_000, 0.05, 7);
        let mut d = BandDetector::new(BandConfig::new(rate, 64)).unwrap();
        let mut got = 0;
        d.process(&x, |_| got += 1);
        assert_eq!(got, 0);
        tone(&mut x, rate, 300e3, 0.5);
        let mut d = BandDetector::new(BandConfig::new(rate, 64)).unwrap();
        let mut late = 0;
        d.process(&x, |b| {
            if b.start > 1_200_000 {
                late += 1;
            }
        });
        assert_eq!(late, 0);
    }

    #[test]
    fn the_band_grouping_gives_every_signal_of_one_burst() {
        let rate = 2e6;
        let mut x = noise(300_000, 0.01, 9);
        tone(&mut x[100_000..104_000], rate, 400e3, 0.3);
        tone(&mut x[100_000..104_000], rate, -500e3, 0.3);
        let mut cfg = BandConfig::new(rate, 128);
        cfg.floor.learn_frames = 30;
        cfg.floor.learn_alpha = 0.2;
        cfg.grouping = Grouping::Band {
            signals: 8,
            centroid_hz: 60e3,
            clear_hz: 80e3,
            min_share: 0.02,
        };
        let mut d = BandDetector::new(cfg).unwrap();
        let mut got = Vec::new();
        d.process(&x, |b| got.push(b.offset_hz));
        got.sort_by(f64::total_cmp);
        assert_eq!(got.len(), 2, "{got:?}");
        assert!((got[0] + 500e3).abs() < 20e3 && (got[1] - 400e3).abs() < 20e3);
    }

    #[test]
    fn all_zero_input_opens_nothing() {
        let mut d = BandDetector::new(BandConfig::new(1e6, 64)).unwrap();
        let mut got = 0;
        d.process(&vec![Complex32::default(); 200_000], |_| got += 1);
        assert_eq!(got, 0);
    }
}

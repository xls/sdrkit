//! A wide search for a known set of tones anywhere in a channel, and the parts that read the symbols of such a signal: a
//! long averaged spectrum, the fit of a comb of tones to it, a lock that replays the stored channel and follows the tones, a
//! symbol quality gate, and a chooser among parallel readings of one symbol stream.
//!
//! | Part | What it does |
//! |---|---|
//! | [`AveragedSpectrum`] | Hann segments of `n` samples (the smallest power of two over a given time), half overlap, bin power over `n` times the window power, an exponential average whose first segments form a plain mean, a cumulative sum so the power between two frequencies costs two look-ups, the noise per bin from the median |
//! | [`fit_comb`] | every offset in bin steps: the power within a half width of each tone, less the power at guard tones beyond the ends (a comb moved by whole tones loses to the true one); the best offset with its z value against the noise and its SNR in 3 kHz |
//! | [`ToneSetSearch`] | keeps the last seconds of the channel; on the first fit that passes, locks, and hands out the stored channel mixed by the offset (so traffic before the lock is read), then every new sample mixed, as the reader pulls them; follows the offset by half of each small change, and is lost after a number of failed fits in a row |
//! | [`ToneOffset`] | the frequency error of M-FSK whose spacing is its symbol rate, from the filters beside the strongest |
//! | [`QualityGate`] | the mean over 16 symbols of `(e1 - e2) / (e1 + e2)` of the two strongest tones; open from 0.25 |
//! | [`Chooser`] | readings hold their rows until one wins; then its rows are released and only its rows pass |
//!
//! A tone of amplitude `A` sums to `A^2` over its bins, and white noise of power `s2` per sample gives `s2 / n` per bin. A
//! signal received in the lower sideband shows at negative frequencies with its tone order reversed; an evenly spaced comb
//! then fits at a negative offset, and the reading decides the order.
//!
//! ```
//! use sdrkit_algo_modem::toneset::{AveragedSpectrum, CombSpec, fit_comb};
//! use sdrkit_algo_modem::Complex32;
//! let rate = 8000.0;
//! let mut s = AveragedSpectrum::new(rate, 1.0, 1.0 / 16.0).unwrap();
//! let tones: Vec<f64> = (0..4).map(|k| 1000.0 + 100.0 * k as f64).collect();
//! let mut ph = 0.0f64;
//! for i in 0..40_000 {
//!     let f = tones[(i / 400) % 4] + 37.0;
//!     ph += std::f64::consts::TAU * f / rate;
//!     s.push(Complex32::from_polar(1.0, ph as f32));
//! }
//! let spec = CombSpec::new(&tones, 20.0, -500.0, 500.0).with_guards(100.0, 2);
//! let fit = fit_comb(&mut s, &spec).unwrap();
//! assert!(fit.found && (fit.df_hz - 37.0).abs() < 1.5);
//! ```

use crate::error::{ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The share of the rate within which the noise median is taken.
const NOISE_SPAN: f64 = 0.40;
/// The share of the rate a comb and its guards must stay within.
const COMB_SPAN: f64 = 0.44;

/// The long averaged power spectrum of a complex channel.
pub struct AveragedSpectrum {
    rate: f64,
    n: usize,
    alpha: f64,
    window: Vec<f32>,
    norm: f64,
    fft: Fft,
    ring: Vec<Complex32>,
    pos: usize,
    filled: usize,
    since: usize,
    work: Vec<Complex32>,
    power: Vec<f64>,
    cum: Vec<f64>,
    segments: u64,
    scratch: Vec<f64>,
}

impl AveragedSpectrum {
    /// A spectrum of the channel at `rate` with segments of the smallest power of two of at least `seconds` (256 to 65 536
    /// samples) and the averaging factor `alpha` (0 to 1).
    pub fn new(rate: f64, seconds: f64, alpha: f64) -> Result<Self, ModemError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        Param::Averaging.check_in(alpha, 1e-6, 1.0)?;
        let want = (rate * seconds).ceil().clamp(256.0, 65_536.0) as usize;
        let n = want.next_power_of_two();
        let window: Vec<f32> = (0..n)
            .map(|i| (0.5 - 0.5 * (TAU * (i as f64 + 0.5) / n as f64).cos()) as f32)
            .collect();
        let wsum: f64 = window.iter().map(|&w| f64::from(w) * f64::from(w)).sum();
        Ok(AveragedSpectrum {
            rate,
            n,
            alpha,
            window,
            norm: 1.0 / (n as f64 * wsum),
            fft: Fft::forward(n),
            ring: vec![Complex32::default(); n],
            pos: 0,
            filled: 0,
            since: 0,
            work: vec![Complex32::default(); n],
            power: vec![0.0; n],
            cum: vec![0.0; n + 1],
            segments: 0,
            scratch: Vec::with_capacity(n),
        })
    }

    /// The FFT length.
    pub fn fft_len(&self) -> usize {
        self.n
    }

    /// The bin width, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.n as f64
    }

    /// The rate of the channel.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The segments averaged so far.
    pub fn segments(&self) -> u64 {
        self.segments
    }

    /// Forgets the average and the samples.
    pub fn reset(&mut self) {
        self.ring.iter_mut().for_each(|v| *v = Complex32::default());
        self.pos = 0;
        self.filled = 0;
        self.since = 0;
        self.power.iter_mut().for_each(|p| *p = 0.0);
        self.cum.iter_mut().for_each(|p| *p = 0.0);
        self.segments = 0;
    }

    /// Takes one sample. Returns true when it completed a segment (every `n / 2` samples once `n` were seen).
    pub fn push(&mut self, x: Complex32) -> bool {
        self.ring[self.pos] = x;
        self.pos = (self.pos + 1) % self.n;
        self.filled = (self.filled + 1).min(self.n);
        self.since += 1;
        if self.filled < self.n || (self.segments > 0 && self.since < self.n / 2) {
            return false;
        }
        self.since = 0;
        for i in 0..self.n {
            self.work[i] = self.ring[(self.pos + i) % self.n] * self.window[i];
        }
        self.fft.process(&mut self.work);
        self.segments += 1;
        let a = self.alpha.max(1.0 / self.segments as f64);
        let half = self.n / 2;
        for i in 0..self.n {
            let q = f64::from(self.work[(i + half) % self.n].norm_sqr()) * self.norm;
            self.power[i] = (1.0 - a) * self.power[i] + a * q;
            self.cum[i + 1] = self.cum[i] + self.power[i];
        }
        true
    }

    /// The bin power in frequency order: index `n / 2` is 0 Hz.
    pub fn power(&self) -> &[f64] {
        &self.power
    }

    /// The power of the bins whose centre lies in `[lo_hz, hi_hz]`, and their count.
    pub fn power_between(&self, lo_hz: f64, hi_hz: f64) -> (f64, usize) {
        let bw = self.bin_hz();
        let half = (self.n / 2) as f64;
        let a = ((lo_hz / bw + half).ceil()).max(0.0) as usize;
        let b = ((hi_hz / bw + half).floor()).min(self.n as f64 - 1.0);
        if b < 0.0 || (b as usize) < a {
            return (0.0, 0);
        }
        let b = b as usize;
        (self.cum[b + 1] - self.cum[a], b + 1 - a)
    }

    /// The independent averages behind a bin.
    pub fn effective_averages(&self) -> f64 {
        0.7 * self.averages()
    }

    fn averages(&self) -> f64 {
        (self.segments as f64).min((2.0 - self.alpha) / self.alpha)
    }

    /// The noise per bin: the median of the bins within 0.40 of the rate of 0 Hz over the median of a gamma variable of that
    /// many averages over its mean.
    pub fn noise_per_bin(&mut self) -> f64 {
        let bw = self.bin_hz();
        let half = self.n / 2;
        let span = ((NOISE_SPAN * self.rate) / bw) as usize;
        let lo = half.saturating_sub(span);
        let hi = (half + span).min(self.n - 1);
        self.scratch.clear();
        self.scratch.extend_from_slice(&self.power[lo..=hi]);
        let m = self.scratch.len() / 2;
        let k = self.averages().max(1.0);
        let (_, med, _) = self.scratch.select_nth_unstable_by(m, f64::total_cmp);
        let c = (1.0 - 1.0 / (9.0 * k)).powi(3).max(0.5);
        *med / c
    }
}

/// A comb of tones to fit: the nominal tones and the rules of [`fit_comb`].
#[derive(Clone, Debug, PartialEq)]
pub struct CombSpec {
    /// The nominal tones, Hz.
    pub tones_hz: Vec<f64>,
    /// The half width summed around each tone, Hz.
    pub half_w_hz: f64,
    /// The step of the guard tones beyond each end, Hz.
    pub guard_step_hz: f64,
    /// The guard tones on each side.
    pub guards: usize,
    /// The least z value of a found comb.
    pub min_z: f64,
    /// The least ratio of the power in the comb over the expected noise.
    pub min_ratio: f64,
    /// The lowest offset tried, Hz.
    pub df_lo_hz: f64,
    /// The highest offset tried, Hz.
    pub df_hi_hz: f64,
    /// Segments needed before a fit.
    pub min_segments: u64,
    /// For an evenly spaced comb, the spacing: the best offset is then refined by the circular mean of the power over the
    /// comb, taken modulo the spacing.
    pub period_hz: Option<f64>,
    /// The best offset is refined by the centroid of the power over the window of each tone (for a few tones whose keyed
    /// spectra are each symmetric about their tone, such as 2-FSK).
    pub centred: bool,
}

impl CombSpec {
    /// A comb of `tones_hz` summed over `half_w_hz` each side, tried from `df_lo_hz` to `df_hi_hz`: no guards, z at least 9,
    /// ratio at least 1, 4 segments.
    pub fn new(tones_hz: &[f64], half_w_hz: f64, df_lo_hz: f64, df_hi_hz: f64) -> Self {
        CombSpec {
            tones_hz: tones_hz.to_vec(),
            half_w_hz,
            guard_step_hz: 0.0,
            guards: 0,
            min_z: 9.0,
            min_ratio: 1.0,
            df_lo_hz,
            df_hi_hz,
            min_segments: 4,
            period_hz: None,
            centred: false,
        }
    }

    /// With the refinement by the centroids of the tones.
    pub fn centred(mut self) -> Self {
        self.centred = true;
        self
    }

    /// With the refinement of an evenly spaced comb of spacing `hz`.
    pub fn periodic(mut self, hz: f64) -> Self {
        self.period_hz = Some(hz).filter(|&h| h > 0.0);
        self
    }

    /// With `guards` guard tones `step_hz` apart beyond each end.
    pub fn with_guards(mut self, step_hz: f64, guards: usize) -> Self {
        self.guard_step_hz = step_hz;
        self.guards = guards;
        self
    }

    /// With the thresholds of a found comb.
    pub fn with_thresholds(mut self, min_z: f64, min_ratio: f64) -> Self {
        self.min_z = min_z;
        self.min_ratio = min_ratio;
        self
    }

    /// With the segments needed before a fit.
    pub fn with_min_segments(mut self, n: u64) -> Self {
        self.min_segments = n;
        self
    }
}

/// The best fit of a comb.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombFit {
    /// The offset of the comb from its nominal tones, Hz.
    pub df_hz: f64,
    /// The z value of the power in the comb against the noise.
    pub z: f64,
    /// The power in the comb over the expected noise.
    pub ratio: f64,
    /// The SNR in 3 kHz, dB.
    pub snr_db: f64,
    /// The comb passes the thresholds.
    pub found: bool,
}

/// The best offset of the comb `c` on `s` (module documentation). `None` before `c.min_segments` segments or when no offset
/// fits inside the channel.
pub fn fit_comb(s: &mut AveragedSpectrum, c: &CombSpec) -> Option<CombFit> {
    if s.segments() < c.min_segments || c.tones_hz.is_empty() {
        return None;
    }
    let bw = s.bin_hz();
    let edge = COMB_SPAN * s.rate();
    let fmin = c.tones_hz.iter().copied().fold(f64::INFINITY, f64::min);
    let fmax = c.tones_hz.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let k0 = (c.df_lo_hz / bw).ceil() as i64;
    let k1 = (c.df_hi_hz / bw).floor() as i64;
    let mut best: Option<(f64, f64, usize)> = None;
    for k in k0..=k1 {
        let df = k as f64 * bw;
        if fmin + df - c.half_w_hz < -edge || fmax + df + c.half_w_hz > edge {
            continue;
        }
        let (mut inp, mut bins) = (0.0, 0usize);
        for &f in &c.tones_hz {
            let (p, b) = s.power_between(f + df - c.half_w_hz, f + df + c.half_w_hz);
            inp += p;
            bins += b;
        }
        let mut guard = 0.0;
        for g in 1..=c.guards {
            for f in [
                fmin + df - g as f64 * c.guard_step_hz,
                fmax + df + g as f64 * c.guard_step_hz,
            ] {
                if f.abs() <= edge {
                    guard += s.power_between(f - c.half_w_hz, f + c.half_w_hz).0;
                }
            }
        }
        let score = inp - guard;
        if best.is_none_or(|(sc, _, _)| score > sc) {
            best = Some((score, df, bins));
        }
    }
    let (_, mut df, mut bins) = best?;
    if c.centred {
        df = refine_centroids(s, c, df);
    }
    if let Some(p) = c.period_hz {
        df = refine_periodic(s, df, p, fmin, fmax);
        bins = c
            .tones_hz
            .iter()
            .map(|&f| {
                s.power_between(f + df - c.half_w_hz, f + df + c.half_w_hz)
                    .1
            })
            .sum();
    }
    let (inp, _) = c.tones_hz.iter().fold((0.0, 0usize), |(a, n), &f| {
        let (p, b) = s.power_between(f + df - c.half_w_hz, f + df + c.half_w_hz);
        (a + p, n + b)
    });
    let noise = s.noise_per_bin().max(1e-30);
    let expected = noise * bins.max(1) as f64;
    let ratio = inp / expected;
    let sigma = 1.0 / (s.effective_averages() * bins as f64 / 1.5).max(1.0).sqrt();
    let z = (ratio - 1.0) / sigma;
    let snr = ((inp - expected) / (noise * 3000.0 / bw)).max(1e-6);
    Some(CombFit {
        df_hz: df,
        z,
        ratio,
        snr_db: 10.0 * snr.log10(),
        found: z >= c.min_z && ratio >= c.min_ratio,
    })
}

/// The offset `df` refined within half a spacing `p`: the angle of the sum of the bin powers over the whole periods of the comb,
/// each turned by its place within the spacing. A power spectrum that is symmetric about each tone (with any line structure
/// of the keying) gives the tone centres, which a sum over windows narrower than the spacing does not. When the turned sum is
/// under a quarter of the power over the noise (two tones alternating put their lines a quarter spacing each side, and the
/// angle is then not defined), `df` is kept.
fn refine_periodic(s: &mut AveragedSpectrum, df: f64, p: f64, fmin: f64, fmax: f64) -> f64 {
    let bw = s.bin_hz();
    let half = (s.fft_len() / 2) as f64;
    let lo = ((fmin + df - p / 2.0) / bw + half).ceil().max(0.0) as usize;
    let hi = (((fmax + df + p / 2.0) / bw + half).floor() as usize).min(s.fft_len() - 1);
    if hi <= lo {
        return df;
    }
    let (mut re, mut im, mut total) = (0.0f64, 0.0f64, 0.0f64);
    for (i, &pw) in s.power().iter().enumerate().take(hi + 1).skip(lo) {
        let f = (i as f64 - half) * bw - fmin - df;
        let a = TAU * f / p;
        re += pw * a.cos();
        im += pw * a.sin();
        total += pw;
    }
    let signal = total - s.noise_per_bin() * (hi + 1 - lo) as f64;
    if signal.is_nan() || signal <= 0.0 || re.hypot(im) < 0.25 * signal {
        return df;
    }
    df + im.atan2(re) * p / TAU
}

/// The offset `df` refined three times by the mean shift of the centroids of the power over the noise within the window of
/// each tone.
fn refine_centroids(s: &mut AveragedSpectrum, c: &CombSpec, mut df: f64) -> f64 {
    let noise = s.noise_per_bin();
    let bw = s.bin_hz();
    let half = (s.fft_len() / 2) as f64;
    for _ in 0..3 {
        let mut shift = 0.0;
        let mut n = 0;
        for &f in &c.tones_hz {
            let lo = ((f + df - c.half_w_hz) / bw + half).ceil().max(0.0) as usize;
            let hi = (((f + df + c.half_w_hz) / bw + half).floor().max(0.0) as usize)
                .min(s.fft_len() - 1);
            let (mut w, mut m) = (0.0f64, 0.0f64);
            for i in lo..=hi {
                let p = (s.power()[i] - noise).max(0.0);
                w += p;
                m += p * (i as f64 - half) * bw;
            }
            if w > 0.0 {
                shift += m / w - (f + df);
                n += 1;
            }
        }
        if n == 0 {
            break;
        }
        df += shift / n as f64;
    }
    df
}

/// The rules of a [`ToneSetSearch`].
#[derive(Clone, Debug, PartialEq)]
pub struct SearchConfig {
    /// The comb.
    pub comb: CombSpec,
    /// The time a segment covers at least, seconds.
    pub spectrum_s: f64,
    /// The averaging factor of the spectrum.
    pub alpha: f64,
    /// The time kept and replayed at a lock, seconds.
    pub replay_s: f64,
    /// For an evenly spaced comb, the spacing: a change of the offset is first taken modulo it.
    pub spacing_hz: Option<f64>,
    /// The largest change of the offset that is followed, Hz.
    pub track_hz: f64,
    /// Failed fits in a row that lose the signal.
    pub misses: u32,
}

impl SearchConfig {
    /// The rules of a continuous signal: segments of 1 s, `alpha` 1/16, 16 s replayed, lost after 4 failed fits.
    pub fn new(comb: CombSpec, spacing_hz: Option<f64>, track_hz: f64) -> Self {
        SearchConfig {
            comb,
            spectrum_s: 1.0,
            alpha: 1.0 / 16.0,
            replay_s: 16.0,
            spacing_hz,
            track_hz,
            misses: 4,
        }
    }
}

/// What a [`ToneSetSearch`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SearchEvent {
    /// The tones were found: the samples given are the stored channel from its oldest sample, mixed by `df_hz`.
    Found {
        /// The offset, Hz.
        df_hz: f64,
        /// The SNR in 3 kHz, dB.
        snr_db: f64,
    },
    /// The tones are gone: the receiver finishes what is open.
    Lost {
        /// The offset when lost, Hz.
        df_hz: f64,
    },
}

/// The search, lock, replay and tracking of a tone set (module documentation).
pub struct ToneSetSearch {
    cfg: SearchConfig,
    spec: AveragedSpectrum,
    history: VecDeque<Complex32>,
    keep: usize,
    df: Option<f64>,
    misses: u32,
    segs: u64,
    phase: f64,
    last: Option<CombFit>,
    queue: VecDeque<Complex32>,
    fits: u64,
}

impl ToneSetSearch {
    /// A search of a channel at `rate`.
    pub fn new(rate: f64, cfg: SearchConfig) -> Result<Self, ModemError> {
        let spec = AveragedSpectrum::new(rate, cfg.spectrum_s, cfg.alpha)?;
        let keep = (cfg.replay_s.max(0.0) * rate) as usize;
        Ok(ToneSetSearch {
            cfg,
            spec,
            history: VecDeque::with_capacity(keep + 1),
            keep,
            df: None,
            misses: 0,
            segs: 0,
            phase: 0.0,
            last: None,
            queue: VecDeque::new(),
            fits: 0,
        })
    }

    /// The offset of the lock, Hz.
    pub fn offset_hz(&self) -> Option<f64> {
        self.df
    }

    /// The last fit.
    pub fn last_fit(&self) -> Option<CombFit> {
        self.last
    }

    /// Whether the tones are locked.
    pub fn locked(&self) -> bool {
        self.df.is_some()
    }

    /// Forgets the lock, the spectrum and the stored channel.
    pub fn reset(&mut self) {
        self.spec.reset();
        self.history.clear();
        self.df = None;
        self.misses = 0;
        self.segs = 0;
        self.phase = 0.0;
        self.last = None;
        self.queue.clear();
    }

    /// The fits made so far.
    pub fn fits(&self) -> u64 {
        self.fits
    }

    /// Locks again on the last fit, if it passed: the stored channel waits again, mixed by its offset. A reader that has
    /// published nothing yet uses it when the fits keep naming another offset than the lock (by whole tones, which the
    /// tracking does not follow).
    pub fn relock(&mut self) -> Option<SearchEvent> {
        let f = self.last.filter(|f| f.found)?;
        self.df = Some(f.df_hz);
        self.misses = 0;
        self.phase = 0.0;
        self.queue.clear();
        self.queue.extend(self.history.iter().copied());
        Some(SearchEvent::Found {
            df_hz: f.df_hz,
            snr_db: f.snr_db,
        })
    }

    /// Moves the offset of the lock by `delta_hz` (a follow-up from the symbols, [`ToneOffset`]).
    pub fn adjust(&mut self, delta_hz: f64) {
        if let Some(df) = &mut self.df {
            *df += delta_hz;
        }
    }

    /// Drops the lock and keeps the spectrum and the stored channel.
    pub fn unlock(&mut self) {
        self.df = None;
        self.misses = 0;
    }

    fn mix(&mut self, x: Complex32, df: f64) -> Complex32 {
        let r = Complex32::from_polar(1.0, (-TAU * self.phase) as f32);
        self.phase = (self.phase + df / self.spec.rate()).rem_euclid(1.0);
        x * r
    }

    /// Takes one sample of the channel. While locked, the sample waits to be read by [`ToneSetSearch::pull`]; at a lock the
    /// whole stored channel waits (and [`SearchEvent::Found`] is returned before any of it is read).
    pub fn push(&mut self, x: Complex32) -> Option<SearchEvent> {
        if self.keep > 0 {
            if self.history.len() == self.keep {
                self.history.pop_front();
            }
            self.history.push_back(x);
        }
        let mut event = None;
        if self.spec.push(x) {
            self.segs += 1;
            if self.df.is_none() || self.segs.is_multiple_of(2) {
                let fit = fit_comb(&mut self.spec, &self.cfg.comb);
                self.fits += 1;
                if let Some(f) = fit {
                    self.last = Some(f);
                }
                event = self.judge(fit);
            }
        }
        match event {
            Some(SearchEvent::Found { .. }) => {
                self.phase = 0.0;
                self.queue.clear();
                self.queue.extend(self.history.iter().copied());
                if self.keep == 0 {
                    self.queue.push_back(x);
                }
            }
            Some(SearchEvent::Lost { .. }) => self.queue.clear(),
            None if self.df.is_some() => self.queue.push_back(x),
            None => {}
        }
        event
    }

    /// Appends up to `max` waiting samples to `out`, mixed by the offset as it is now (so a change by
    /// [`ToneSetSearch::adjust`] applies from the next sample). Returns how many.
    pub fn pull(&mut self, max: usize, out: &mut Vec<Complex32>) -> usize {
        let Some(df) = self.df else {
            self.queue.clear();
            return 0;
        };
        let n = max.min(self.queue.len());
        for _ in 0..n {
            let v = self.queue.pop_front().unwrap_or_default();
            let y = self.mix(v, df);
            out.push(y);
        }
        n
    }

    fn judge(&mut self, fit: Option<CombFit>) -> Option<SearchEvent> {
        let found = fit.filter(|f| f.found);
        match (self.df, found) {
            (None, Some(f)) => {
                self.df = Some(f.df_hz);
                self.misses = 0;
                self.segs = 0;
                Some(SearchEvent::Found {
                    df_hz: f.df_hz,
                    snr_db: f.snr_db,
                })
            }
            (None, None) => None,
            (Some(df), None) => {
                self.misses += 1;
                if self.misses >= self.cfg.misses {
                    self.df = None;
                    self.misses = 0;
                    Some(SearchEvent::Lost { df_hz: df })
                } else {
                    None
                }
            }
            (Some(df), Some(f)) => {
                self.misses = 0;
                let mut d = f.df_hz - df;
                if let Some(s) = self.cfg.spacing_hz.filter(|&s| s > 0.0) {
                    d -= s * (d / s).round();
                }
                if d.abs() <= self.cfg.track_hz {
                    self.df = Some(df + d / 2.0);
                }
                None
            }
        }
    }
}

/// The frequency error of the tones of an M-FSK signal whose spacing equals its symbol rate, from the matched filters of its
/// symbols: at each symbol, with `a` and `c` the magnitudes of the filters above and below the strongest,
/// `(a - c) / (a + c)` is the error of a tone that fills the window as a share of the spacing (exactly, for an error under one
/// spacing). The mean over a number of symbols is returned as a correction. The averaged spectrum of such a signal is flat
/// over the comb apart from the lines of its keying, so the comb fit leaves an error within the spacing that this follows.
#[derive(Clone, Debug)]
pub struct ToneOffset {
    spacing_hz: f64,
    symbols: u32,
    sum: f64,
    n: u32,
}

impl ToneOffset {
    /// A follow-up of tones `spacing_hz` apart that returns a correction every `symbols` symbols.
    pub fn new(spacing_hz: f64, symbols: u32) -> Self {
        ToneOffset {
            spacing_hz,
            symbols: symbols.max(1),
            sum: 0.0,
            n: 0,
        }
    }

    /// Takes the filter energies of one symbol (in the order of frequency). Returns the mean error in hertz (positive: the
    /// tones are above the filters) once `symbols` symbols were taken.
    pub fn push(&mut self, energies: &[f32]) -> Option<f64> {
        let b = energies
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .map(|(i, _)| i)?;
        if b == 0 || b + 1 >= energies.len() {
            return None;
        }
        let a = f64::from(energies[b + 1]).sqrt();
        let c = f64::from(energies[b - 1]).sqrt();
        if a + c <= 0.0 {
            return None;
        }
        self.sum += (a - c) / (a + c);
        self.n += 1;
        if self.n < self.symbols {
            return None;
        }
        let d = self.sum / f64::from(self.n) * self.spacing_hz;
        self.sum = 0.0;
        self.n = 0;
        Some(d)
    }

    /// Forgets the symbols taken.
    pub fn reset(&mut self) {
        self.sum = 0.0;
        self.n = 0;
    }
}

/// The symbol quality gate (module documentation).
#[derive(Clone, Debug, Default)]
pub struct QualityGate {
    q: VecDeque<f32>,
    sum: f32,
    open: bool,
    closed: bool,
}

/// Symbols over which the quality is averaged.
const GATE_SYMBOLS: usize = 16;
/// The least mean quality of an open gate.
const GATE_OPEN: f32 = 0.25;

impl QualityGate {
    /// A closed gate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the tone energies of one symbol. Returns whether the gate is open.
    pub fn push(&mut self, energies: &[f32]) -> bool {
        let (mut e1, mut e2) = (0f32, 0f32);
        for &e in energies {
            if e > e1 {
                e2 = e1;
                e1 = e;
            } else if e > e2 {
                e2 = e;
            }
        }
        let q = if e1 + e2 > 0.0 {
            (e1 - e2) / (e1 + e2)
        } else {
            0.0
        };
        self.q.push_back(q);
        self.sum += q;
        if self.q.len() > GATE_SYMBOLS {
            self.sum -= self.q.pop_front().unwrap_or(0.0);
        }
        let open = self.q.len() == GATE_SYMBOLS && self.sum / GATE_SYMBOLS as f32 >= GATE_OPEN;
        if self.open && !open {
            self.closed = true;
        }
        self.open = open;
        open
    }

    /// The mean quality, once 16 symbols are in.
    pub fn quality(&self) -> Option<f32> {
        (self.q.len() == GATE_SYMBOLS).then(|| self.sum / GATE_SYMBOLS as f32)
    }

    /// Whether the gate closed since the last call.
    pub fn take_closed(&mut self) -> bool {
        std::mem::take(&mut self.closed)
    }

    /// Forgets the symbols.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Rows a reading holds before a winner is chosen.
const HELD: usize = 64;
/// Rows a losing reading holds.
const HELD_LOSER: usize = 8;

/// A chooser among parallel readings of one symbol stream (module documentation).
#[derive(Clone, Debug)]
pub struct Chooser<R> {
    held: Vec<VecDeque<R>>,
    winner: Option<usize>,
}

impl<R> Chooser<R> {
    /// A chooser of `readings` readings without a winner.
    pub fn new(readings: usize) -> Self {
        Chooser {
            held: (0..readings).map(|_| VecDeque::new()).collect(),
            winner: None,
        }
    }

    /// The winner.
    pub fn winner(&self) -> Option<usize> {
        self.winner
    }

    /// A row of `reading`: appended to `out` when the reading is the winner, else held (the oldest dropped).
    pub fn offer(&mut self, reading: usize, row: R, out: &mut Vec<R>) {
        if self.winner == Some(reading) {
            out.push(row);
            return;
        }
        let Some(h) = self.held.get_mut(reading) else {
            return;
        };
        h.push_back(row);
        let cap = if self.winner.is_some() {
            HELD_LOSER
        } else {
            HELD
        };
        while h.len() > cap {
            h.pop_front();
        }
    }

    /// Makes `reading` the winner: its held rows go to `out`, the others are dropped.
    pub fn choose(&mut self, reading: usize, out: &mut Vec<R>) {
        if reading >= self.held.len() {
            return;
        }
        self.winner = Some(reading);
        out.extend(self.held[reading].drain(..));
        for h in &mut self.held {
            h.clear();
        }
    }

    /// Forgets the winner and every held row.
    pub fn forget(&mut self) {
        self.winner = None;
        for h in &mut self.held {
            h.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tones(f: &[f64], sym: usize, rate: f64, n: usize, df: f64) -> Vec<Complex32> {
        let mut ph = 0.0f64;
        (0..n)
            .map(|i| {
                ph += TAU * (f[(i / sym * 7) % f.len()] + df) / rate;
                Complex32::from_polar(1.0, ph as f32)
            })
            .collect()
    }

    fn noise(n: usize, power: f64, seed: u64) -> Vec<Complex32> {
        let mut s = seed | 1;
        let mut u = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let sd = (power / 2.0).sqrt();
        (0..n)
            .map(|_| {
                let (a, b) = (u(), u());
                let r = (-2.0 * a.ln()).sqrt() * sd;
                Complex32::new((r * (TAU * b).cos()) as f32, (r * (TAU * b).sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn a_tone_sums_to_its_power_and_noise_to_its_floor() {
        let rate = 8000.0;
        let mut s = AveragedSpectrum::new(rate, 1.0, 1.0 / 16.0).unwrap();
        assert_eq!(s.fft_len(), 8192);
        let x = tones(&[1000.0], 1, rate, 8192 * 4, 0.0);
        x.iter().for_each(|&v| {
            s.push(v);
        });
        let (p, _) = s.power_between(990.0, 1010.0);
        assert!((p - 1.0).abs() < 0.01, "{p}");
        let mut s = AveragedSpectrum::new(rate, 1.0, 1.0 / 16.0).unwrap();
        for v in noise(8192 * 20, 2.0, 3) {
            s.push(v);
        }
        let per_bin = s.noise_per_bin();
        assert!((per_bin * 8192.0 / 2.0 - 1.0).abs() < 0.05, "{per_bin}");
    }

    #[test]
    fn the_guards_refuse_a_comb_moved_by_whole_tones() {
        let rate = 8000.0;
        let f: Vec<f64> = (0..20).map(|k| 520.0 + 40.0 * k as f64).collect();
        let mut s = AveragedSpectrum::new(rate, 1.0, 1.0 / 16.0).unwrap();
        let x = tones(&f, 800, rate, 8000 * 8, 23.0);
        let n = noise(x.len(), 10.0, 5);
        for (a, b) in x.iter().zip(&n) {
            s.push(*a + *b);
        }
        let spec = CombSpec::new(&f, 10.0, -4500.0, 4500.0)
            .with_guards(40.0, 2)
            .periodic(40.0);
        let fit = fit_comb(&mut s, &spec).unwrap();
        assert!(fit.found && (fit.df_hz - 23.0).abs() < 1.0, "{fit:?}");
        // Noise alone: no comb in 30 tries.
        for seed in 0..30 {
            let mut s = AveragedSpectrum::new(rate, 1.0, 1.0 / 16.0).unwrap();
            for v in noise(8000 * 6, 1.0, 100 + seed) {
                s.push(v);
            }
            let fit = fit_comb(&mut s, &spec).unwrap();
            assert!(!fit.found, "{seed}: {fit:?}");
        }
    }

    #[test]
    fn a_lock_replays_the_stored_channel_and_follows_the_offset() {
        let rate = 8000.0;
        let f: Vec<f64> = (0..4).map(|k| 1000.0 + 100.0 * k as f64).collect();
        let spec = CombSpec::new(&f, 20.0, -1000.0, 1000.0).with_guards(100.0, 2);
        let mut s = ToneSetSearch::new(rate, SearchConfig::new(spec, Some(100.0), 12.0)).unwrap();
        let x = tones(&f, 400, rate, 8000 * 10, 50.0);
        let mut mixed = Vec::new();
        let mut found = None;
        for (i, &v) in x.iter().enumerate() {
            if let Some(SearchEvent::Found { df_hz, .. }) = s.push(v) {
                found = Some((i, df_hz));
                assert_eq!(s.pull(usize::MAX, &mut mixed), i + 1);
            }
            s.pull(usize::MAX, &mut mixed);
        }
        let (at, df) = found.unwrap();
        assert!((df - 50.0).abs() < 1.0 && at < 8000 * 4, "{at} {df}");
        assert_eq!(mixed.len(), x.len());
        // The average forgets the tones over about 16 segments; then 4 failed fits lose them.
        let mut lost = false;
        for v in noise(8000 * 60, 1.0, 9) {
            lost |= matches!(s.push(v), Some(SearchEvent::Lost { .. }));
        }
        assert!(lost && !s.locked());
    }

    #[test]
    fn the_filters_beside_the_strongest_give_the_tone_error() {
        let rate = 8000.0;
        for err in [-12.0, -3.0, 1.0, 7.0, 15.0] {
            let f: Vec<f64> = (0..8).map(|k| (1000.0 + 40.0 * k as f64) / rate).collect();
            let mut c = crate::fsk::ToneCorrelator::new(&f, 200).unwrap();
            let mut t = ToneOffset::new(40.0, 4);
            let mut got = None;
            let mut ph = 0.0f64;
            for s in 0..8 {
                for _ in 0..200 {
                    ph += TAU * (1000.0 + 40.0 * (1 + s % 6) as f64 + err) / rate;
                    c.push(Complex32::from_polar(1.0, ph as f32));
                }
                let mut e = [0f32; 8];
                c.energies(&mut e).unwrap();
                got = t.push(&e).or(got);
            }
            assert!((got.unwrap() - err).abs() < 0.3, "{err}: {got:?}");
        }
    }

    #[test]
    fn the_centroids_place_a_keyed_pair_and_a_relock_moves_the_lock() {
        let rate = 8000.0;
        let mut s = AveragedSpectrum::new(rate, 1.0, 1.0 / 16.0).unwrap();
        // 2-FSK at 200 Bd, 1000 Hz shift, centred on 1517 Hz, random bits.
        let mut ph = 0.0f64;
        let mut seed = 5u32;
        for i in 0..8000 * 8 {
            if i % 40 == 0 {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            }
            let f = if seed >> 16 & 1 == 1 { 2017.0 } else { 1017.0 };
            ph += TAU * f / rate;
            s.push(Complex32::from_polar(1.0, ph as f32));
        }
        let spec = CombSpec::new(&[-500.0, 500.0], 200.0, -3000.0, 3000.0)
            .with_thresholds(9.0, 1.25)
            .centred();
        let fit = fit_comb(&mut s, &spec).unwrap();
        assert!(fit.found && (fit.df_hz - 1517.0).abs() < 3.0, "{fit:?}");
        let f: Vec<f64> = (0..4).map(|k| 1000.0 + 100.0 * k as f64).collect();
        let comb = CombSpec::new(&f, 20.0, -1000.0, 1000.0).with_guards(100.0, 2);
        let mut t = ToneSetSearch::new(rate, SearchConfig::new(comb, Some(100.0), 12.0)).unwrap();
        for v in tones(&f, 400, rate, 8000 * 4, 50.0) {
            t.push(v);
        }
        let df = t.offset_hz().unwrap();
        t.adjust(10.0);
        assert!((t.offset_hz().unwrap() - df - 10.0).abs() < 1e-9);
        assert!(matches!(t.relock(), Some(SearchEvent::Found { .. })));
        let mut out = Vec::new();
        assert!(t.pull(usize::MAX, &mut out) > 8000 * 3);
        assert!((t.offset_hz().unwrap() - 50.0).abs() < 1.0);
    }

    #[test]
    fn the_gate_opens_on_clear_symbols_and_the_chooser_releases_the_winner() {
        let mut g = QualityGate::new();
        for _ in 0..15 {
            assert!(!g.push(&[1.0, 0.1, 0.0]));
        }
        assert!(g.push(&[1.0, 0.1, 0.0]));
        for _ in 0..16 {
            g.push(&[1.0, 0.9, 0.8]);
        }
        assert!(g.take_closed() && !g.take_closed());
        let mut c = Chooser::new(3);
        let mut out = Vec::new();
        for i in 0..70 {
            c.offer(i % 3, i, &mut out);
        }
        assert!(out.is_empty());
        c.choose(1, &mut out);
        assert_eq!(out.len(), 23);
        assert_eq!(out[0], 1);
        out.clear();
        c.offer(1, 100, &mut out);
        c.offer(0, 101, &mut out);
        assert_eq!(out, vec![100]);
    }
}

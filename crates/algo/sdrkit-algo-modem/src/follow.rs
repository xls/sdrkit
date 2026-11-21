//! Carrier follow-up for a narrow channel filter: the carrier error of a channel measured before its narrow filter, so the
//! filter can be moved onto the carrier (or the carrier onto the filter).
//!
//! | Part | Type | Suits |
//! |---|---|---|
//! | spectral follow-up | [`SpectralFollow`] | control channels and bursty FSK: the power centroid of an averaged spectrum over the quietest quarter, in closed loop (the caller's mixer takes the correction in steps) or open loop (an estimate the caller centres its filter on); noise does not pull it to the filter centre |
//! | narrow channel centred in open loop | [`CentredChannel`], [`CentredFir`], [`centred_lowpass`] | a stream whose discriminator values keep their frequencies: a fourth-order Butterworth low-pass moved onto the estimate, or a complex FIR with a stated stop band edge moved there |
//! | discriminator-mean follow-up | [`DiscriminatorFollow`] | a slicer that runs on every sample: a mixer, two Butterworth sections and a discriminator whose mean (the carrier error of a balanced FSK) drives the mixer |
//!
//! Frequencies are in Hz at the channel's working rate. Nothing allocates per sample; the spectral follow-up runs one FFT of 128
//! to 1024 points per frame of about 4 ms.
//!
//! ```
//! use sdrkit_algo_modem::follow::{SpectralConfig, SpectralFollow};
//! use sdrkit_algo_modem::Complex32;
//! let mut f = SpectralFollow::new(SpectralConfig::open_loop(48_000.0)).unwrap();
//! assert_eq!(f.fft_len(), 256);
//! // A carrier 3 kHz above the centre.
//! let mut x: Vec<Complex32> = (0..48_000)
//!     .map(|n| Complex32::from_polar(1.0, (std::f64::consts::TAU * 3000.0 * n as f64 / 48_000.0) as f32))
//!     .collect();
//! f.process(&mut x);
//! assert!(f.settled() && (f.correction_hz() - 3000.0).abs() < 100.0);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::ComplexFir;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::iir::{BiquadCoeffs, ComplexBiquad, ComplexBiquadCoeffs, BUTTERWORTH4_Q};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;
use std::f64::consts::{PI, TAU};

/// The options of a [`SpectralFollow`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectralConfig {
    /// The working rate of the channel (Hz).
    pub rate: f64,
    /// Bins within this many Hz of the centre are searched (`S`).
    pub search_hz: f64,
    /// The correction never goes beyond this many Hz (`Fmax`).
    pub max_hz: f64,
    /// A centroid within this many Hz is centred (`D`).
    pub dead_band_hz: f64,
    /// The averaging time once the signal was found centred (`Tavg`, s).
    pub avg_s: f64,
    /// The averaging time before that (s).
    pub first_avg_s: f64,
    /// Closed loop (the caller's mixer takes the correction) or open loop (the caller moves its filter).
    pub closed_loop: bool,
    /// The closed loop moves by this share of the centroid.
    pub loop_gain: f64,
    /// Bins at this multiple of the floor or more count in the centroid.
    pub centroid_ratio: f64,
    /// A look finds a signal when the total excess over the searched bins is above this multiple of `floor x bins`.
    pub excess_ratio: f64,
}

impl SpectralConfig {
    /// The closed loop with the usual numbers: search +-9 kHz, at most +-20 kHz, dead band 500 Hz, 0.3 s then 0.8 s of averaging.
    pub fn closed_loop(rate: f64) -> Self {
        SpectralConfig {
            rate,
            search_hz: 9000.0,
            max_hz: 20_000.0,
            dead_band_hz: 500.0,
            avg_s: 0.8,
            first_avg_s: 0.3,
            closed_loop: true,
            loop_gain: 0.8,
            centroid_ratio: 1.4,
            excess_ratio: 0.25,
        }
    }

    /// The open loop with the same numbers.
    pub fn open_loop(rate: f64) -> Self {
        SpectralConfig {
            closed_loop: false,
            ..Self::closed_loop(rate)
        }
    }
}

/// What a look of a [`SpectralFollow`] found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    /// A signal stood out of the floor.
    pub found: bool,
    /// The power centroid relative to the channel centre (after the closed loop's mixer), Hz.
    pub centroid_hz: f64,
    /// The correction (closed loop) or estimate (open loop) after the look, Hz.
    pub correction_hz: f64,
    /// The look found the signal centred (closed loop) or agreeing with the estimate (open loop).
    pub settled: bool,
    /// The correction changed (closed loop: the caller's mixer must move; the averages restarted).
    pub moved: bool,
}

/// The spectral carrier follow-up (module docs).
///
/// FFT length `N`: 128, doubled while `N < rate / 250` and `N < 1024` (about 4 ms). Frames of `N` samples, not overlapping, under
/// the Hann window `0.5 - 0.5 cos(2 pi (i + 0.5) / N)`. The power spectrum is averaged by `avg += a (|X|^2 - avg)`,
/// `a = max(min(0.5, N / (T rate)), 1 / k)` for the `k`-th frame since the averages started (the first frame sets it, and the
/// first look sees a plain mean of its frames rather than one frame weighted by a third), `T` the first averaging time until a
/// look finds the signal centred, then the averaging time. A look comes every `ceil(T rate / N)` frames: the floor is the bin at rank `N / 4` of the sorted
/// average (at least 1e-30); over the bins within the search width, the excess `e = avg - floor`; the centroid is
/// `sum e f / sum e` over the bins at least `centroid_ratio x floor`; nothing is found unless that sum is positive and the total
/// excess exceeds `excess_ratio x floor x bins`.
///
/// Closed loop: a centroid beyond the dead band moves the correction by `loop_gain x centroid` (held to `+-max_hz`) and restarts
/// the averages; within it the follow-up is settled; a look that finds nothing is repeated one frame later. Open loop: the first
/// look sets the estimate; a later one within the dead band of it averages the two and settles; a farther one replaces it and
/// unsettles; every look waits a whole averaging time for the next.
///
/// In closed loop [`process`](Self::process) turns the channel by `-correction_hz()` in place (the mixer in front), and a move
/// applies from the next sample; in open loop it leaves the samples as they are.
#[derive(Debug)]
pub struct SpectralFollow {
    cfg: SpectralConfig,
    n: usize,
    fft: Fft,
    nco: Nco,
    window: Vec<f32>,
    frame: Vec<Complex32>,
    fill: usize,
    avg: Vec<f32>,
    frames: u64,
    until_look: u64,
    correction: f64,
    estimate: bool,
    settled: bool,
    centred_once: bool,
    sorted: Vec<f32>,
    last: Look,
}

impl SpectralFollow {
    /// A follow-up with these options. Errors unless the rate, search width, largest correction and averaging times are positive
    /// and the dead band is at least 0.
    pub fn new(cfg: SpectralConfig) -> Result<Self, ModemError> {
        Param::Rate.check_positive(cfg.rate)?;
        Param::Width.check_positive(cfg.search_hz)?;
        Param::MaxFreq.check_positive(cfg.max_hz)?;
        Param::TimeConstant.check_positive(cfg.avg_s)?;
        Param::TimeConstant.check_positive(cfg.first_avg_s)?;
        Param::Hysteresis.check_in(cfg.dead_band_hz, 0.0, f64::MAX)?;
        Param::Gain.check_positive(cfg.loop_gain)?;
        let mut n = 128usize;
        while (n as f64) < cfg.rate / 250.0 && n < 1024 {
            n *= 2;
        }
        let window = (0..n)
            .map(|i| (0.5 - 0.5 * (TAU * (i as f64 + 0.5) / n as f64).cos()) as f32)
            .collect();
        let mut f = SpectralFollow {
            cfg,
            n,
            fft: Fft::forward(n),
            nco: Nco::new(cfg.rate, 0.0),
            window,
            frame: vec![Complex32::default(); n],
            fill: 0,
            avg: vec![0.0; n],
            frames: 0,
            until_look: 0,
            correction: 0.0,
            estimate: false,
            settled: false,
            centred_once: false,
            sorted: Vec::with_capacity(n),
            last: Look::default(),
        };
        f.restart_averages();
        Ok(f)
    }

    /// The options.
    pub fn config(&self) -> &SpectralConfig {
        &self.cfg
    }

    /// The FFT length.
    pub fn fft_len(&self) -> usize {
        self.n
    }

    /// The correction (closed loop) or estimate (open loop) of the carrier error, Hz.
    pub fn correction_hz(&self) -> f64 {
        self.correction
    }

    /// Whether the last look found the signal centred (closed loop) or agreeing with the estimate (open loop).
    pub fn settled(&self) -> bool {
        self.settled
    }

    /// The last look.
    pub fn last_look(&self) -> Look {
        self.last
    }

    /// The averaging time in use (s).
    fn avg_time(&self) -> f64 {
        if self.centred_once {
            self.cfg.avg_s
        } else {
            self.cfg.first_avg_s
        }
    }

    /// Frames between looks.
    fn look_period(&self) -> u64 {
        (self.avg_time() * self.cfg.rate / self.n as f64)
            .ceil()
            .max(1.0) as u64
    }

    /// Restarts the averages and the frame in progress; keeps the correction (a stream interruption).
    pub fn restart_averages(&mut self) {
        self.fill = 0;
        self.frames = 0;
        self.until_look = self.look_period();
    }

    /// Clears everything, the correction too (a retune by the user).
    pub fn retune(&mut self) {
        self.correction = 0.0;
        self.nco.set_freq(0.0);
        self.estimate = false;
        self.settled = false;
        self.centred_once = false;
        self.last = Look::default();
        self.restart_averages();
    }

    /// Takes samples of the channel; in closed loop turns them by `-correction_hz()` in place first. Returns the last look made
    /// during the call, if any.
    pub fn process(&mut self, x: &mut [Complex32]) -> Option<Look> {
        let mut look = None;
        let mut i = 0;
        while i < x.len() {
            let k = (self.n - self.fill).min(x.len() - i);
            if self.cfg.closed_loop {
                self.nco.mix_in_place(&mut x[i..i + k]);
            }
            self.frame[self.fill..self.fill + k].copy_from_slice(&x[i..i + k]);
            self.fill += k;
            i += k;
            if self.fill == self.n {
                self.fill = 0;
                if let Some(l) = self.frame_done() {
                    look = Some(l);
                }
            }
        }
        look
    }

    /// The frequency of bin `i` (Hz).
    fn bin_hz(&self, i: usize) -> f64 {
        let k = if i < self.n / 2 {
            i as f64
        } else {
            i as f64 - self.n as f64
        };
        k * self.cfg.rate / self.n as f64
    }

    fn frame_done(&mut self) -> Option<Look> {
        for (v, w) in self.frame.iter_mut().zip(&self.window) {
            *v *= *w;
        }
        self.fft.process(&mut self.frame);
        let a = (self.n as f64 / (self.avg_time() * self.cfg.rate))
            .min(0.5)
            .max(1.0 / (self.frames + 1) as f64) as f32;
        if self.frames == 0 {
            for (g, x) in self.avg.iter_mut().zip(&self.frame) {
                *g = x.norm_sqr();
            }
        } else {
            for (g, x) in self.avg.iter_mut().zip(&self.frame) {
                *g += a * (x.norm_sqr() - *g);
            }
        }
        self.frames += 1;
        self.until_look = self.until_look.saturating_sub(1);
        if self.until_look > 0 {
            return None;
        }
        let l = self.look();
        self.last = l;
        Some(l)
    }

    fn look(&mut self) -> Look {
        let cfg = self.cfg;
        self.sorted.clear();
        self.sorted.extend_from_slice(&self.avg);
        let rank = self.n / 4;
        let floor = (*self.sorted.select_nth_unstable_by(rank, f32::total_cmp).1 as f64).max(1e-30);
        let (mut total, mut nb, mut se, mut sef) = (0f64, 0usize, 0f64, 0f64);
        for i in 0..self.n {
            let f = self.bin_hz(i);
            if f.abs() > cfg.search_hz {
                continue;
            }
            let p = self.avg[i] as f64;
            let e = p - floor;
            total += e;
            nb += 1;
            if p >= cfg.centroid_ratio * floor {
                se += e;
                sef += e * f;
            }
        }
        let clamp = |v: f64| v.clamp(-cfg.max_hz, cfg.max_hz);
        let mut l = Look {
            correction_hz: self.correction,
            ..Look::default()
        };
        if !(se > 0.0 && total > cfg.excess_ratio * floor * nb as f64) {
            self.until_look = if cfg.closed_loop {
                1
            } else {
                self.look_period()
            };
            l.settled = self.settled;
            return l;
        }
        let c = sef / se;
        l.found = true;
        l.centroid_hz = c;
        if cfg.closed_loop {
            if c.abs() > cfg.dead_band_hz {
                let next = clamp(self.correction + cfg.loop_gain * c);
                l.moved = next != self.correction;
                self.correction = next;
                self.settled = false;
                if l.moved {
                    self.nco.set_freq(-next);
                    self.restart_averages();
                } else {
                    self.until_look = self.look_period();
                }
            } else {
                self.settled = true;
                self.centred_once = true;
                self.until_look = self.look_period();
            }
        } else {
            if self.estimate && (c - self.correction).abs() <= cfg.dead_band_hz {
                self.correction = clamp((self.correction + c) / 2.0);
                self.settled = true;
                self.centred_once = true;
            } else {
                self.correction = clamp(c);
                self.settled = false;
            }
            self.estimate = true;
            self.until_look = self.look_period();
        }
        l.correction_hz = self.correction;
        l.settled = self.settled;
        l
    }
}

/// How far the estimate must move before [`CentredChannel`] or [`CentredFir`] moves its filter (Hz).
pub const CENTRED_STEP_HZ: f64 = 150.0;

/// The move rule of a channel centred in open loop: with no centre in use, the first settled estimate; with one, an estimate
/// more than [`CENTRED_STEP_HZ`] from it.
fn centred_moves(in_use: Option<f64>, estimate_hz: f64, settled: bool) -> bool {
    match in_use {
        None => settled,
        Some(a) => (estimate_hz - a).abs() > CENTRED_STEP_HZ,
    }
}

/// A narrow channel centred in open loop: a fourth-order Butterworth low-pass of the filter's half width (sections of Q 0.5412
/// and 1.3066, bilinear) moved onto the estimate `A`. It equals turning the stream by `-A`, low-passing I and Q and turning it
/// back by `+A`, so discriminator values keep their frequencies. Until the first settled estimate the stream passes unfiltered;
/// after that the filter moves whenever the estimate is more than [`CENTRED_STEP_HZ`] from the one in use.
#[derive(Clone, Debug)]
pub struct CentredChannel {
    rate: f64,
    half_hz: f64,
    proto: [BiquadCoeffs; 2],
    sections: [ComplexBiquad; 2],
    in_use: Option<f64>,
}

impl CentredChannel {
    /// A channel of half width `half_hz` at `rate`. Errors unless `0 < half_hz < rate / 2`.
    pub fn new(rate: f64, half_hz: f64) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        Param::Cutoff.check_open(half_hz, 0.0, rate / 2.0)?;
        let proto = BUTTERWORTH4_Q.map(|q| BiquadCoeffs::lowpass(rate, half_hz, q));
        Ok(CentredChannel {
            rate,
            half_hz,
            proto,
            sections: proto.map(ComplexBiquad::from_real),
            in_use: None,
        })
    }

    /// The half width (Hz).
    pub fn half_hz(&self) -> f64 {
        self.half_hz
    }

    /// The centre in use, or `None` while the stream passes unfiltered.
    pub fn centre_hz(&self) -> Option<f64> {
        self.in_use
    }

    /// Takes the follow-up's estimate and whether it is settled; moves the filter when the rule says so. Returns whether it moved.
    pub fn set_estimate(&mut self, estimate_hz: f64, settled: bool) -> bool {
        let go = centred_moves(self.in_use, estimate_hz, settled);
        if go {
            for (s, c) in self.sections.iter_mut().zip(&self.proto) {
                s.set_coeffs(ComplexBiquadCoeffs::shifted(*c, self.rate, estimate_hz));
            }
            self.in_use = Some(estimate_hz);
        }
        go
    }

    /// Forgets the centre: the stream passes unfiltered again.
    pub fn reset(&mut self) {
        self.in_use = None;
        for s in &mut self.sections {
            s.reset();
        }
    }

    /// Filters `buf` in place (unchanged while no centre is in use).
    pub fn process(&mut self, buf: &mut [Complex32]) {
        if self.in_use.is_none() {
            return;
        }
        let [a, b] = &mut self.sections;
        for v in buf {
            *v = b.process_sample(a.process_sample(*v));
        }
    }
}

/// The taps of a complex FIR channel filter centred on `centre_hz`: the windowed-sinc low-pass of half width `half_hz` over
/// `t = -m ..= m` (`2m + 1` taps, Blackman window `0.42 + 0.5 cos(pi t / m) + 0.08 cos(2 pi t / m)`, unit gain at its centre),
/// each tap times `e^(j 2 pi centre t / rate)`. With `taps[0]` on the newest sample (`ComplexFir`), the pass band lies on
/// `+centre_hz`. Errors unless `0 < half_hz < rate / 2`, `m >= 1` and `out` holds `2m + 1` taps.
pub fn centred_lowpass(
    rate: f64,
    half_hz: f64,
    m: usize,
    centre_hz: f64,
    out: &mut [Complex32],
) -> Result<(), ModemError> {
    Param::Rate.check_positive(rate)?;
    Param::Cutoff.check_open(half_hz, 0.0, rate / 2.0)?;
    Param::Taps.check_count(m, 1, 1 << 16)?;
    if out.len() != 2 * m + 1 {
        return Param::Taps.length(2 * m + 1, out.len());
    }
    let fc = half_hz / rate;
    let mut sum = 0.0;
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f64 - m as f64;
        let x = 2.0 * fc * t;
        let sinc = if t == 0.0 {
            1.0
        } else {
            (PI * x).sin() / (PI * x)
        };
        let h = 2.0 * fc * sinc * Window::Blackman.value_symmetric(i, 2 * m + 1);
        sum += h;
        *o = Complex32::new(h as f32, 0.0);
    }
    for (i, o) in out.iter_mut().enumerate() {
        let p = TAU * centre_hz * (i as f64 - m as f64) / rate;
        let h = o.re as f64 / sum;
        *o = Complex32::new((h * p.cos()) as f32, (h * p.sin()) as f32);
    }
    Ok(())
}

/// A narrow channel centred in open loop, FIR form: the complex low-pass of [`centred_lowpass`] with `2m + 1` taps. Until the
/// first settled estimate the filter has the half width `first_half_hz` at 0 Hz (the whole channel the estimate is searched
/// in), so the delay of `m` samples is the same before and after; then it has the half width `half_hz` on the estimate and
/// moves by the rule of [`CentredChannel`]. A move clears the history of the filter.
#[derive(Clone, Debug)]
pub struct CentredFir {
    rate: f64,
    half_hz: f64,
    first_half_hz: f64,
    m: usize,
    taps: Vec<Complex32>,
    fir: ComplexFir,
    in_use: Option<f64>,
}

impl CentredFir {
    /// A channel of half width `half_hz` (the -6 dB point: the middle of pass band edge and stop band edge) at `rate` with `m`
    /// taps each side of the centre, and of half width `first_half_hz` before the first estimate. Errors as
    /// [`centred_lowpass`] for either width.
    pub fn new(rate: f64, half_hz: f64, m: usize, first_half_hz: f64) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        Param::Cutoff.check_open(half_hz, 0.0, rate / 2.0)?;
        Param::Taps.check_count(m, 1, 1 << 16)?;
        let mut taps = vec![Complex32::new(0.0, 0.0); 2 * m + 1];
        centred_lowpass(rate, first_half_hz, m, 0.0, &mut taps)?;
        Ok(CentredFir {
            rate,
            half_hz,
            first_half_hz,
            m,
            fir: ComplexFir::new(&taps),
            taps,
            in_use: None,
        })
    }

    /// The delay of the filter, samples.
    pub fn delay(&self) -> usize {
        self.m
    }

    /// The centre of the narrow filter in use, or `None` while the first filter is in use.
    pub fn centre_hz(&self) -> Option<f64> {
        self.in_use
    }

    /// Takes the follow-up's estimate and whether it is settled; moves the filter when the rule says so. Returns whether it moved.
    pub fn set_estimate(&mut self, estimate_hz: f64, settled: bool) -> bool {
        let go = centred_moves(self.in_use, estimate_hz, settled)
            && centred_lowpass(self.rate, self.half_hz, self.m, estimate_hz, &mut self.taps).is_ok();
        if go {
            self.fir = ComplexFir::new(&self.taps);
            self.in_use = Some(estimate_hz);
        }
        go
    }

    /// Forgets the centre and the history: the first filter is in use again.
    pub fn reset(&mut self) {
        if centred_lowpass(self.rate, self.first_half_hz, self.m, 0.0, &mut self.taps).is_ok() {
            self.fir = ComplexFir::new(&self.taps);
        }
        self.in_use = None;
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        self.fir.process_in_place(buf);
    }
}

/// The options of a [`DiscriminatorFollow`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiscFollowConfig {
    /// The working rate (Hz).
    pub rate: f64,
    /// The cut-off `B` of the two Butterworth sections (the cascade is 6 dB down there), Hz.
    pub cutoff_hz: f64,
    /// The correction is held to `+-range_hz`.
    pub range_hz: f64,
    /// The time of the discriminator mean (s).
    pub mean_s: f64,
    /// The time of the integrator from the mean to the correction (s).
    pub integrate_s: f64,
}

impl DiscFollowConfig {
    /// The usual numbers: a mean over 50 ms, a 0.5 s integrator, a range of +-4 kHz.
    pub fn new(rate: f64, cutoff_hz: f64) -> Self {
        DiscFollowConfig {
            rate,
            cutoff_hz,
            range_hz: 4000.0,
            mean_s: 0.05,
            integrate_s: 0.5,
        }
    }
}

/// Samples between updates of the mixer's frequency.
const DISC_MIX_CHUNK: usize = 32;

/// The discriminator-mean follow-up (module docs): per sample, the input is turned by the correction `h` (a mixer at `-h`, which
/// takes a new `h` every 32 samples), filtered by two identical second-order Butterworth low-pass sections (Q 0.7071, bilinear,
/// cut-off `B`), and discriminated to `d` Hz. Then `mean += (d - mean) / (mean_s rate)` and
/// `h = clamp(h + mean / (integrate_s rate), -range, +range)`. The mean of a balanced FSK is the carrier error, so `h` follows it
/// and the output `d` is centred.
#[derive(Clone, Debug)]
pub struct DiscriminatorFollow {
    cfg: DiscFollowConfig,
    nco: Nco,
    sections: [ComplexBiquad; 2],
    disc: Discriminator,
    mean: f64,
    h: f64,
    chunk: [Complex32; DISC_MIX_CHUNK],
}

impl DiscriminatorFollow {
    /// A follow-up with these options. Errors unless the rate is positive, `0 < B < rate / 2`, the range is at least 0 and the
    /// times are positive.
    pub fn new(cfg: DiscFollowConfig) -> Result<Self, ModemError> {
        Param::Rate.check_positive(cfg.rate)?;
        Param::Cutoff.check_open(cfg.cutoff_hz, 0.0, cfg.rate / 2.0)?;
        Param::MaxFreq.check_in(cfg.range_hz, 0.0, f64::MAX)?;
        Param::TimeConstant.check_positive(cfg.mean_s)?;
        Param::TimeConstant.check_positive(cfg.integrate_s)?;
        let c = Self::section(cfg.rate, cfg.cutoff_hz);
        Ok(DiscriminatorFollow {
            cfg,
            nco: Nco::new(cfg.rate, 0.0),
            sections: [ComplexBiquad::from_real(c), ComplexBiquad::from_real(c)],
            disc: Discriminator::new(cfg.rate, 1.0)
                .map_err(|_| ModemError::Degenerate { param: Param::Rate })?,
            mean: 0.0,
            h: 0.0,
            chunk: [Complex32::default(); DISC_MIX_CHUNK],
        })
    }

    /// The coefficients of one section: a Butterworth low-pass (Q 1/sqrt 2) at `cutoff_hz`.
    pub fn section(rate: f64, cutoff_hz: f64) -> BiquadCoeffs {
        BiquadCoeffs::lowpass(rate, cutoff_hz, std::f64::consts::FRAC_1_SQRT_2)
    }

    /// The correction `h` (Hz): the carrier error followed.
    pub fn correction_hz(&self) -> f64 {
        self.h
    }

    /// The discriminator mean (Hz).
    pub fn mean_hz(&self) -> f64 {
        self.mean
    }

    /// Clears the correction, the mean and the filters.
    pub fn reset(&mut self) {
        self.h = 0.0;
        self.mean = 0.0;
        self.nco = Nco::new(self.cfg.rate, 0.0);
        for s in &mut self.sections {
            s.reset();
        }
        self.disc.reset();
    }

    /// Turns, filters and discriminates `x` into `out` (Hz, the same length), following the carrier. Errors when the lengths
    /// differ.
    pub fn process(&mut self, x: &[Complex32], out: &mut [f32]) -> Result<(), ModemError> {
        if out.len() != x.len() {
            return Param::Input.length(x.len(), out.len());
        }
        let km = 1.0 / (self.cfg.mean_s * self.cfg.rate);
        let ki = 1.0 / (self.cfg.integrate_s * self.cfg.rate);
        let r = self.cfg.range_hz;
        for (xs, os) in x.chunks(DISC_MIX_CHUNK).zip(out.chunks_mut(DISC_MIX_CHUNK)) {
            self.nco.set_freq(-self.h);
            let buf = &mut self.chunk[..xs.len()];
            self.nco.mix(xs, buf);
            let [a, b] = &mut self.sections;
            for (v, o) in buf.iter().zip(os) {
                let y = b.process_sample(a.process_sample(*v));
                let d = self.disc.process_sample(y);
                *o = d;
                self.mean += (d as f64 - self.mean) * km;
                self.h = (self.h + self.mean * ki).clamp(-r, r);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::iir::Butterworth4;
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;

    /// Continuous-phase 2-FSK: `bits` at `baud`, deviation `dev`, carrier `carrier`, unit power.
    fn fsk(
        rate: f64,
        baud: f64,
        dev: f64,
        carrier: f64,
        n: usize,
        rng: &mut Rng,
    ) -> (Vec<Complex32>, Vec<u8>) {
        let sps = rate / baud;
        let mut ph = 0f64;
        let mut bits = Vec::new();
        let mut x = Vec::with_capacity(n);
        let mut bit = 0u8;
        for i in 0..n {
            let k = (i as f64 / sps) as usize;
            if k >= bits.len() {
                bit = rng.bit();
                bits.push(bit);
            }
            let f = carrier + if bit == 1 { dev } else { -dev };
            ph = (ph + f / rate).rem_euclid(1.0);
            x.push(Complex32::from_polar(1.0, (TAU * ph) as f32));
        }
        (x, bits)
    }

    /// Runs a follow-up over `x` in pieces of 1000 samples. Returns the time it first settled, the correction at the end and
    /// whether it is settled then.
    fn run(cfg: SpectralConfig, x: &[Complex32]) -> (Option<f64>, f64, bool) {
        let mut f = SpectralFollow::new(cfg).unwrap();
        let mut buf = x.to_vec();
        let mut first = None;
        for (k, b) in buf.chunks_mut(1000).enumerate() {
            f.process(b);
            if f.settled() && first.is_none() {
                first = Some((k + 1) as f64 * 1000.0 / cfg.rate);
            }
        }
        (first, f.correction_hz(), f.settled())
    }

    #[test]
    fn spectral_follow_settles_on_fsk_and_not_on_noise() {
        let rate = 48_000.0;
        let n = (3.0 * rate) as usize;
        for &(off, seed) in &[(0.0, 1u64), (3100.0, 2), (-6000.0, 3)] {
            let mut rng = Rng::new(seed);
            let (mut x, _) = fsk(rate, 9600.0, 1440.0, off, n, &mut rng);
            // 10 dB SNR over the whole 48 kHz.
            Awgn::with_power(0.1, seed).unwrap().process(&mut x);
            let (t, a, s) = run(SpectralConfig::closed_loop(rate), &x);
            let t = t.expect("closed loop settles");
            assert!(
                t <= 1.5 && s && (a - off).abs() <= 500.0,
                "closed {off}: {t} s, {a} Hz"
            );
            let (t, a, s) = run(SpectralConfig::open_loop(rate), &x);
            let t = t.expect("open loop settles");
            assert!(
                t <= 1.6 && s && (a - off).abs() <= 300.0,
                "open {off}: {t} s, {a} Hz"
            );
        }
        let mut x = vec![Complex32::default(); (10.0 * rate) as usize];
        Awgn::with_power(1.0, 9).unwrap().process(&mut x);
        for cfg in [
            SpectralConfig::closed_loop(rate),
            SpectralConfig::open_loop(rate),
        ] {
            let mut f = SpectralFollow::new(cfg).unwrap();
            for piece in x.clone().chunks_mut(4096) {
                if let Some(l) = f.process(piece) {
                    assert!(!l.found && !l.settled && f.correction_hz() == 0.0, "{l:?}");
                }
            }
        }
        // FFT lengths: about 4 ms.
        let len = |r: f64| {
            SpectralFollow::new(SpectralConfig::open_loop(r))
                .unwrap()
                .fft_len()
        };
        assert_eq!(
            (len(24_000.0), len(48_000.0), len(250_000.0), len(2e6)),
            (128, 256, 1024, 1024)
        );
    }

    /// The share of the power of `x` that passes `filter`.
    fn passed(x: &[Complex32], mut filter: impl FnMut(Complex32) -> Complex32) -> f64 {
        let (mut pi, mut po) = (0f64, 0f64);
        for (i, &v) in x.iter().enumerate() {
            let y = filter(v);
            if i > 2000 {
                pi += v.norm_sqr() as f64;
                po += y.norm_sqr() as f64;
            }
        }
        po / pi
    }

    #[test]
    fn narrow_filter_passes_the_signal_only_after_the_follow_up() {
        // FFSK at 1200 bit/s, 1.5 kHz deviation, at 24 kHz, into a 2.5 kHz narrow filter.
        let rate = 24_000.0;
        for &(off, seed) in &[(3100.0, 11u64), (5500.0, 12)] {
            let mut rng = Rng::new(seed);
            let (x, _) = fsk(rate, 1200.0, 1500.0, off, (2.0 * rate) as usize, &mut rng);
            let mut f = SpectralFollow::new(SpectralConfig::open_loop(rate)).unwrap();
            f.process(&mut x.clone());
            assert!(f.settled());
            let mut ch = CentredChannel::new(rate, 2500.0).unwrap();
            assert!(ch.set_estimate(f.correction_hz(), true));
            let tail = &x[x.len() / 2..];
            let mut buf = tail.to_vec();
            ch.process(&mut buf);
            let with = passed(tail, {
                let mut i = 0;
                move |_| {
                    i += 1;
                    buf[i - 1]
                }
            });
            let mut fixed = [
                Butterworth4::lowpass(rate, 2500.0),
                Butterworth4::lowpass(rate, 2500.0),
            ];
            let without = passed(tail, |v| {
                let [re, im] = &mut fixed;
                Complex32::new(re.process_sample(v.re), im.process_sample(v.im))
            });
            assert!(with > 0.85 && without < 0.55, "{off}: {with} {without}");
            if off > 5000.0 {
                assert!(without < 0.1, "{without}");
            }
            // The FIR form centred the same way passes as much; centred on the other side it passes little.
            let mut taps = vec![Complex32::default(); 129];
            centred_lowpass(rate, 2500.0, 64, f.correction_hz(), &mut taps).unwrap();
            let mut fir = ComplexFir::new(&taps);
            let p = passed(tail, |v| fir.process_sample(v));
            centred_lowpass(rate, 2500.0, 64, -f.correction_hz(), &mut taps).unwrap();
            let mut fir = ComplexFir::new(&taps);
            let q = passed(tail, |v| fir.process_sample(v));
            assert!(p > 0.85 && q < 0.1, "{p} {q}");
        }
        // Moves only past 150 Hz once in use.
        let mut ch = CentredChannel::new(rate, 2500.0).unwrap();
        assert!(!ch.set_estimate(1000.0, false));
        assert!(ch.set_estimate(1000.0, true));
        assert!(!ch.set_estimate(1100.0, false));
        assert!(ch.set_estimate(1200.0, false));
        assert_eq!(ch.centre_hz(), Some(1200.0));
    }

    #[test]
    fn centred_fir_moves_by_the_rule_and_keeps_its_delay() {
        let rate = 48_000.0;
        let mut ch = CentredFir::new(rate, 6300.0, 36, 12_500.0).unwrap();
        assert_eq!(ch.delay(), 36);
        // The share of the power of a tone at `f` that passes, after the filter has filled.
        let passed = |ch: &mut CentredFir, f: f64| {
            let mut x: Vec<Complex32> = (0..2000)
                .map(|n| Complex32::from_polar(1.0, (TAU * f * n as f64 / rate) as f32))
                .collect();
            ch.process(&mut x);
            x[200..].iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / 1800.0
        };
        // Before the first estimate the whole first width passes.
        assert!(!ch.set_estimate(3000.0, false));
        assert_eq!(ch.centre_hz(), None);
        assert!(passed(&mut ch, 9000.0) > 0.95 && passed(&mut ch, -9000.0) > 0.95);
        // Centred on +3 kHz: the pass band of 4.5 kHz each side passes, the stop band from 8.1 kHz each side does not.
        assert!(ch.set_estimate(3000.0, true));
        assert!(passed(&mut ch, 7400.0) > 0.95 && passed(&mut ch, -1400.0) > 0.95);
        assert!(passed(&mut ch, 11_200.0) < 1e-4 && passed(&mut ch, -5200.0) < 1e-4);
        // Moves only past 150 Hz once in use.
        assert!(!ch.set_estimate(3100.0, true));
        assert!(ch.set_estimate(3200.0, false));
        assert_eq!(ch.centre_hz(), Some(3200.0));
        // An impulse comes out at the delay, before and after a move.
        for _ in 0..2 {
            let mut x = vec![Complex32::default(); 100];
            x[10] = Complex32::new(1.0, 0.0);
            ch.process(&mut x);
            let top = (0..100).max_by(|&a, &b| x[a].norm_sqr().total_cmp(&x[b].norm_sqr())).unwrap();
            assert_eq!(top, 10 + ch.delay());
            ch.reset();
            assert_eq!(ch.centre_hz(), None);
        }
        assert!(CentredFir::new(rate, 30_000.0, 36, 12_500.0).is_err());
        assert!(CentredFir::new(rate, 6300.0, 0, 12_500.0).is_err());
    }

    #[test]
    fn discriminator_follow_centres_the_slicer() {
        // The section at 24 kHz and 4.5 kHz.
        let c = DiscriminatorFollow::section(24_000.0, 4500.0);
        for (got, want) in [
            (c.b0, 0.186694),
            (c.b1, 0.373389),
            (c.b2, 0.186694),
            (c.a1, -0.462938),
            (c.a2, 0.209715),
        ] {
            assert!((got - want).abs() < 1e-6, "{got} {want}");
        }
        // Balanced 2-FSK at 1200 bit/s, +-800 Hz, carrier 3.1 kHz off, 15 dB SNR over 24 kHz.
        let rate = 24_000.0;
        let mut rng = Rng::new(21);
        let n = (4.0 * rate) as usize;
        let (mut x, bits) = fsk(rate, 1200.0, 800.0, 3100.0, n, &mut rng);
        Awgn::with_power(10f64.powf(-1.5), 22)
            .unwrap()
            .process(&mut x);
        let mut f = DiscriminatorFollow::new(DiscFollowConfig::new(rate, 4500.0)).unwrap();
        let mut d = vec![0f32; n];
        f.process(&x, &mut d).unwrap();
        assert!(
            (f.correction_hz() - 3100.0).abs() < 150.0,
            "{}",
            f.correction_hz()
        );
        // Slicing at 0 Hz over the last second, the symbol mean (shifted by the filters' delay of about 2 samples).
        let sps = 20;
        let errs = |d: &[f32]| {
            let mut e = 0;
            for k in (3 * 1200)..(4 * 1200 - 1) {
                let s: f32 = d[k * sps + 4..(k + 1) * sps + 2].iter().sum();
                e += ((s > 0.0) as u8 != bits[k]) as usize;
            }
            e
        };
        assert!(errs(&d) <= 2, "{}", errs(&d));
        // Without the follow-up (range 0) the slicer sees only the upper side: about half the bits wrong.
        let mut g = DiscriminatorFollow::new(DiscFollowConfig {
            range_hz: 0.0,
            ..DiscFollowConfig::new(rate, 4500.0)
        })
        .unwrap();
        g.process(&x, &mut d).unwrap();
        assert!(errs(&d) > 400, "{}", errs(&d));
    }
}

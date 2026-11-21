//! The spectrum analyser behind the spectrum and waterfall displays, Welch spectra, and the carrier to noise density (C/N0) of a
//! carrier from a power spectrum.

use crate::fft::{Fft, BACKENDS};
use crate::window::Window;
use crate::{Complex32, DspError, Param};

/// How bin powers are scaled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scaling {
    /// `1 / (sum w)^2`: a complex tone of magnitude 1.0 centred on a bin reads 0 dBFS with any size and window (floored at
    /// -200 dB). The default.
    #[default]
    Tone,
    /// The window is normalised to a mean square of 1 and a bin is `|X|^2 / N^2`, so the sum of all bins equals the mean power
    /// of the samples (with a periodic Hann window, a full-scale tone on a bin reads -1.76 dB and white noise
    /// of power `P` reads `P - 10 log10 N` per bin). Floored at -140 dB.
    Power,
}

/// How a [`SpectrumAnalyzer`] works.
#[derive(Clone, Debug, PartialEq)]
pub struct SpectrumConfig {
    /// FFT size: a power of two from 256 to 262144.
    pub fft_size: usize,
    /// The window (default Blackman-Harris, 4 terms).
    pub window: Window,
    /// The input sample rate.
    pub sample_rate: f64,
    /// The requested frames per second. Frames overlap when the rate is high relative to the FFT size and skip input when it is low.
    pub frames_per_second: f64,
    /// Power average over this many frames (exponential, so a frame comes out at every hop; 1 = none).
    pub average: usize,
    /// Keep the maximum of the averaged spectrum (until [`SpectrumAnalyzer::reset_peak`]).
    pub peak_hold: bool,
    /// How bins are scaled (default [`Scaling::Tone`]).
    pub scaling: Scaling,
}

impl SpectrumConfig {
    /// `fft_size` points at `sample_rate`, Blackman-Harris, 30 frames per second, no averaging, no peak hold.
    pub fn new(fft_size: usize, sample_rate: f64) -> Self {
        SpectrumConfig {
            fft_size,
            window: Window::default(),
            sample_rate,
            frames_per_second: 30.0,
            average: 1,
            peak_hold: false,
            scaling: Scaling::Tone,
        }
    }

    /// A power analyser: periodic Hann, [`Scaling::Power`].
    pub fn spec(fft_size: usize, sample_rate: f64) -> Self {
        SpectrumConfig {
            window: Window::Hann,
            scaling: Scaling::Power,
            ..Self::new(fft_size, sample_rate)
        }
    }
}

/// Windowed FFT power spectra in dBFS, bins from the lowest frequency to the highest (index `n/2` is DC, index 0 is `-rate/2`).
///
/// Scaled by `1 / (sum w)^2`, so a complex tone of magnitude 1.0 centred on a bin reads 0 dBFS with any size and window. (Between bins
/// a tone reads lower by the window's scalloping loss: up to 0.83 dB for Blackman-Harris, 1.42 dB for Hann, 3.92 dB for rectangular.)
/// White noise of power `P` dBFS per sample reads `P - 10 log10(n / enbw)` dB per bin, `enbw` being [`SpectrumAnalyzer::enbw_bins`].
#[derive(Debug)]
pub struct SpectrumAnalyzer {
    cfg: SpectrumConfig,
    fft: Fft,
    window: Vec<f32>,
    scale: f32,
    enbw: f64,
    hop: f64,
    frame: Vec<Complex32>,
    filled: usize,
    /// Absolute index of the next frame's first sample.
    next_start: f64,
    consumed: u64,
    work: Vec<Complex32>,
    avg: Vec<f32>,
    peak: Vec<f32>,
    frames: u64,
    db: Vec<f32>,
    peak_db: Vec<f32>,
    db_fresh: bool,
    floor: f32,
    scratch: Vec<f32>,
}

impl SpectrumAnalyzer {
    /// A new analyser.
    pub fn new(cfg: SpectrumConfig) -> Result<Self, DspError> {
        let n = cfg.fft_size;
        if !(n.is_power_of_two() && (256..=262_144).contains(&n)) {
            return Param::FftSize.not_allowed(n as f64);
        }
        if cfg.sample_rate <= 0.0 || cfg.sample_rate.is_nan() {
            return Param::Rate.positive(cfg.sample_rate);
        }
        if cfg.frames_per_second <= 0.0 || cfg.frames_per_second.is_nan() {
            return Param::FrameRate.positive(cfg.frames_per_second);
        }
        let window = cfg.window.coefficients(n);
        let s1: f64 = window.iter().map(|&w| w as f64).sum();
        let s2: f64 = window.iter().map(|&w| (w as f64) * (w as f64)).sum();
        let (scale, floor) = match cfg.scaling {
            Scaling::Tone => (1.0 / (s1 * s1), 1e-20),
            Scaling::Power => (1.0 / (n as f64 * s2), 1e-14),
        };
        let floor_db = (10.0 * f64::log10(floor)) as f32;
        let mut a = SpectrumAnalyzer {
            fft: Fft::forward(n),
            scale: scale as f32,
            floor: floor as f32,
            enbw: n as f64 * s2 / (s1 * s1),
            window,
            hop: 1.0,
            frame: vec![Complex32::default(); n],
            filled: 0,
            next_start: 0.0,
            consumed: 0,
            work: vec![Complex32::default(); n],
            avg: vec![0.0; n],
            peak: vec![0.0; n],
            frames: 0,
            db: vec![floor_db; n],
            peak_db: vec![floor_db; n],
            db_fresh: false,
            scratch: Vec::with_capacity(n),
            cfg,
        };
        a.set_frames_per_second(a.cfg.frames_per_second);
        a.cfg.average = a.cfg.average.max(1);
        Ok(a)
    }

    /// The configuration.
    pub fn config(&self) -> &SpectrumConfig {
        &self.cfg
    }

    /// Changes the frame rate (takes effect from the next frame).
    pub fn set_frames_per_second(&mut self, fps: f64) {
        assert!(fps > 0.0);
        self.cfg.frames_per_second = fps;
        self.hop = (self.cfg.sample_rate / fps).max(1.0);
    }

    /// Changes the averaging length.
    pub fn set_average(&mut self, frames: usize) {
        self.cfg.average = frames.max(1);
    }

    /// Turns peak hold on or off (turning it on starts from the current spectrum).
    pub fn set_peak_hold(&mut self, on: bool) {
        if on && !self.cfg.peak_hold {
            self.peak.copy_from_slice(&self.avg);
        }
        self.cfg.peak_hold = on;
    }

    /// Input samples between frame starts.
    pub fn hop(&self) -> f64 {
        self.hop
    }

    /// The FFT size.
    pub fn len(&self) -> usize {
        self.cfg.fft_size
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The window's equivalent noise bandwidth in bins (1.0 rectangular, 1.5 Hann, about 2.0 Blackman-Harris).
    pub fn enbw_bins(&self) -> f64 {
        self.enbw
    }

    /// The frequency of bin `i` relative to the centre, in Hz.
    pub fn bin_hz(&self, i: usize) -> f64 {
        (i as f64 - (self.cfg.fft_size / 2) as f64) * self.cfg.sample_rate
            / self.cfg.fft_size as f64
    }

    /// Frames computed since the start or the last [`reset`](Self::reset).
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Clears the averages and the peak (the input position is kept).
    pub fn reset(&mut self) {
        self.avg.iter_mut().for_each(|v| *v = 0.0);
        self.peak.iter_mut().for_each(|v| *v = 0.0);
        self.frames = 0;
        self.db_fresh = false;
    }

    /// Drops a frame that is partly gathered (after a retune, when its samples belong to the old frequency): the next frame starts
    /// with the next sample. Averages and the peak are kept.
    pub fn drop_partial(&mut self) {
        self.filled = 0;
        self.next_start = self.next_start.max(self.consumed as f64);
    }

    /// Input samples gathered towards the next frame.
    pub fn partial(&self) -> usize {
        self.filled
    }

    /// Clears the peak hold.
    pub fn reset_peak(&mut self) {
        self.peak.copy_from_slice(&self.avg);
        self.db_fresh = false;
    }

    /// Feeds samples; returns the number of new frames (0 when none completed).
    pub fn push(&mut self, input: &[Complex32]) -> usize {
        self.push_with(input.len(), |frame, at| {
            let n = frame.len();
            frame.copy_from_slice(&input[at..at + n]);
        })
    }

    /// Feeds real samples; returns the number of new frames. A real signal provides a spectrum that is symmetric about the
    /// centre bin, and a cosine of amplitude `A` on a bin reads `A^2 / 4` (-6 dB for `A` = 1) in each of its two bins.
    pub fn push_real(&mut self, input: &[f32]) -> usize {
        self.push_with(input.len(), |frame, at| {
            let n = frame.len();
            for (f, &x) in frame.iter_mut().zip(&input[at..at + n]) {
                *f = Complex32::new(x, 0.0);
            }
        })
    }

    /// Feeds `len` samples, which `fill` copies into the frame from a position of the input.
    fn push_with(&mut self, len: usize, mut fill: impl FnMut(&mut [Complex32], usize)) -> usize {
        let n = self.cfg.fft_size;
        let mut produced = 0;
        let mut i = 0;
        while i < len {
            let start = self.next_start as u64;
            if self.filled == 0 && self.consumed < start {
                let skip = ((start - self.consumed) as usize).min(len - i);
                i += skip;
                self.consumed += skip as u64;
                continue;
            }
            let take = (n - self.filled).min(len - i);
            fill(&mut self.frame[self.filled..self.filled + take], i);
            self.filled += take;
            i += take;
            self.consumed += take as u64;
            if self.filled == n {
                self.compute();
                produced += 1;
                self.next_start += self.hop;
                let shift = (self.next_start as u64 - start) as usize;
                if shift < n {
                    self.frame.copy_within(shift.., 0);
                    self.filled = n - shift;
                } else {
                    self.filled = 0;
                }
            }
        }
        produced
    }

    fn compute(&mut self) {
        let n = self.cfg.fft_size;
        for ((w, x), c) in self.work.iter_mut().zip(&self.frame).zip(&self.window) {
            *w = *x * *c;
        }
        self.fft.process(&mut self.work);
        self.frames += 1;
        let k = (self.frames.min(self.cfg.average as u64)) as f32;
        let wgt = 1.0 / k;
        let half = n / 2;
        let mask = n - 1;
        for (i, a) in self.avg.iter_mut().enumerate() {
            let p = self.work[(i + half) & mask].norm_sqr() * self.scale;
            *a += (p - *a) * wgt;
        }
        if self.cfg.peak_hold {
            for (p, a) in self.peak.iter_mut().zip(&self.avg) {
                *p = p.max(*a);
            }
        }
        self.db_fresh = false;
    }

    fn refresh_db(&mut self) {
        if !self.db_fresh {
            let floor = self.floor;
            for (d, p) in self.db.iter_mut().zip(&self.avg) {
                *d = 10.0 * p.max(floor).log10();
            }
            if self.cfg.peak_hold {
                for (d, p) in self.peak_db.iter_mut().zip(&self.peak) {
                    *d = 10.0 * p.max(floor).log10();
                }
            }
            self.db_fresh = true;
        }
    }

    /// The averaged spectrum as linear power (1.0 = 0 dBFS), in display order.
    pub fn power(&self) -> &[f32] {
        &self.avg
    }

    /// The averaged spectrum in dBFS, in display order (floored at -200 dB, or -140 dB with [`Scaling::Power`]).
    pub fn spectrum_db(&mut self) -> &[f32] {
        self.refresh_db();
        &self.db
    }

    /// The peak-hold spectrum in dBFS, when peak hold is on.
    pub fn peak_db(&mut self) -> Option<&[f32]> {
        if !self.cfg.peak_hold {
            return None;
        }
        self.refresh_db();
        Some(&self.peak_db)
    }

    /// The noise floor of the averaged spectrum: its 20th percentile in dB (see [`noise_floor_db`]).
    pub fn noise_floor_db(&mut self) -> f32 {
        self.refresh_db();
        percentile(&self.db, 0.2, &mut self.scratch)
    }
}

/// A Welch power spectrum of a stored buffer: the average of the windowed periodograms of frames of `n` samples overlapping by a
/// fraction, in one call. The FFT plan, window and work buffer are kept, so repeated calls allocate nothing.
///
/// Bins are linear power (not dB), ordered from the lowest frequency to the highest (index `n / 2` is DC, as in
/// [`SpectrumAnalyzer`]), and scaled by [`Scaling`]: with [`Scaling::Tone`] a complex tone of magnitude 1 on a bin reads 1.0; with
/// [`Scaling::Power`] the bins sum to the mean power of the samples (so white noise of power `P` reads `P / n` per bin).
///
/// ```
/// use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
/// use sdrkit_algo_dsp::window::Window;
/// use sdrkit_algo_dsp::Complex32;
/// let x = vec![Complex32::new(1.0, 0.0); 4096];
/// let mut w = Welch::new(256, Window::Hann, 0.5, Scaling::Tone).unwrap();
/// let mut p = vec![0.0; 256];
/// assert_eq!(w.compute(&x, &mut p).unwrap(), 31);
/// assert!((p[128] - 1.0).abs() < 1e-4);
/// ```
#[derive(Debug)]
pub struct Welch {
    n: usize,
    hop: usize,
    window: Vec<f32>,
    fft: Fft,
    work: Vec<Complex32>,
    scale: f64,
    enbw: f64,
    acc: Vec<f64>,
}

impl Welch {
    /// Frames of `n` points (2 to 2^20; any size, a power of two is fastest), `window` in its periodic form, consecutive frames
    /// overlapping by `overlap` (in `[0, 1)`; 0.5 is usual for Hann, 0.67 for Blackman-Harris), scaled by `scaling`.
    pub fn new(n: usize, window: Window, overlap: f64, scaling: Scaling) -> Result<Self, DspError> {
        if !(2..=1 << 20).contains(&n) {
            return Param::FftSize.range(2.0, (1u32 << 20) as f64, n as f64);
        }
        if !(0.0..1.0).contains(&overlap) {
            return Param::Overlap.range(0.0, 1.0, overlap);
        }
        let w = window.coefficients(n);
        let s1: f64 = w.iter().map(|&v| v as f64).sum();
        let s2: f64 = w.iter().map(|&v| (v as f64) * (v as f64)).sum();
        if s1.is_nan() || s1 <= 0.0 {
            return Param::Window.degenerate();
        }
        let scale = match scaling {
            Scaling::Tone => 1.0 / (s1 * s1),
            Scaling::Power => 1.0 / (n as f64 * s2),
        };
        Ok(Welch {
            n,
            hop: (((1.0 - overlap) * n as f64).round() as usize).max(1),
            window: w,
            fft: Fft::forward(n),
            work: vec![Complex32::default(); n],
            scale,
            enbw: n as f64 * s2 / (s1 * s1),
            acc: vec![0.0; n],
        })
    }

    /// The frame size.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Samples between frame starts.
    pub fn hop(&self) -> usize {
        self.hop
    }

    /// The window's equivalent noise bandwidth in bins.
    pub fn enbw_bins(&self) -> f64 {
        self.enbw
    }

    /// The number of frames a buffer of `len` samples produces (0 when it is shorter than a frame).
    pub fn frames_for(&self, len: usize) -> usize {
        if len < self.n {
            0
        } else {
            1 + (len - self.n) / self.hop
        }
    }

    /// The averaged spectrum of `x` into `out` (`n` bins); returns the number of frames averaged. Errors when `x` is shorter than a
    /// frame or `out` is not `n` long.
    pub fn compute(&mut self, x: &[Complex32], out: &mut [f32]) -> Result<usize, DspError> {
        let frames = self.check(x.len(), out.len())?;
        self.acc.fill(0.0);
        for f in 0..frames {
            let s = f * self.hop;
            for ((o, &v), &g) in self
                .work
                .iter_mut()
                .zip(&x[s..s + self.n])
                .zip(&self.window)
            {
                *o = v * g;
            }
            self.accumulate();
        }
        self.finish(frames, out);
        Ok(frames)
    }

    /// The same for a real signal (its spectrum is symmetric about DC, and a real cosine of amplitude `A` on a bin reads `A^2 / 4`
    /// in each of its two bins with [`Scaling::Tone`]).
    pub fn compute_real(&mut self, x: &[f32], out: &mut [f32]) -> Result<usize, DspError> {
        let frames = self.check(x.len(), out.len())?;
        self.acc.fill(0.0);
        for f in 0..frames {
            let s = f * self.hop;
            for ((o, &v), &g) in self
                .work
                .iter_mut()
                .zip(&x[s..s + self.n])
                .zip(&self.window)
            {
                *o = Complex32::new(v * g, 0.0);
            }
            self.accumulate();
        }
        self.finish(frames, out);
        Ok(frames)
    }

    fn check(&self, len: usize, out: usize) -> Result<usize, DspError> {
        if out != self.n {
            return Param::Output.length(self.n, out);
        }
        match self.frames_for(len) {
            0 => Param::Input.at_least(self.n as f64, len as f64),
            f => Ok(f),
        }
    }

    fn accumulate(&mut self) {
        self.fft.process(&mut self.work);
        for (a, v) in self.acc.iter_mut().zip(&self.work) {
            *a += v.norm_sqr() as f64;
        }
    }

    fn finish(&self, frames: usize, out: &mut [f32]) {
        let k = self.scale / frames as f64;
        let h = self.n / 2;
        // Natural order to lowest-first: bin i of the output is FFT bin (i + n - n/2) mod n.
        for (i, o) in out.iter_mut().enumerate() {
            *o = (self.acc[(i + self.n - h) % self.n] * k) as f32;
        }
    }
}

/// A Welch spectrum of `x` in one call (see [`Welch`]; this plans an FFT and allocates, so keep a [`Welch`] for repeated use).
pub fn welch(
    x: &[Complex32],
    n: usize,
    window: Window,
    overlap: f64,
    scaling: Scaling,
    out: &mut [f32],
) -> Result<usize, DspError> {
    Welch::new(n, window, overlap, scaling)?.compute(x, out)
}

/// Reduces a spectrum to `out.len()` display bins, each the maximum of the source bins it covers (so a narrow signal never disappears).
/// When `out` is longer than `src`, each display bin takes the source bin under its centre.
pub fn reduce_max(src: &[f32], out: &mut [f32]) {
    let (n, m) = (src.len(), out.len());
    if n == 0 || m == 0 {
        return;
    }
    if m >= n {
        for (j, o) in out.iter_mut().enumerate() {
            *o = src[((2 * j + 1) * n / (2 * m)).min(n - 1)];
        }
        return;
    }
    for (j, o) in out.iter_mut().enumerate() {
        let a = j * n / m;
        let b = ((j + 1) * n / m).max(a + 1);
        *o = src[a..b].iter().copied().fold(f32::NEG_INFINITY, f32::max);
    }
}

/// The carrier power and noise density from power spectra made with a periodic Hann window of `n` points. With these, a receiver
/// gets the carrier to noise density ratio (C/N0) of a carrier: `10 log10(carrier_power / noise_density)` dB-Hz. The noise is
/// measured only in the bins within a band around 0 Hz, so a filter's stop band does not pull it down. Nothing is allocated after
/// [`CarrierSpectrum::new`]. The default value is empty and measures nothing.
///
/// ```
/// use sdrkit_algo_dsp::spectrum::CarrierSpectrum;
/// let c = CarrierSpectrum::new(64, 1000.0, 800.0).unwrap();
/// // A tone of power 1 on bin k of a 64-point Hann spectrum: |X[k]|^2 = (64 / 2)^2 and |X[k +- 1]|^2 = (64 / 4)^2.
/// assert!((c.carrier_power(256.0, 1024.0, 256.0) - 1.0).abs() < 1e-6);
/// ```
#[derive(Clone, Debug, Default)]
pub struct CarrierSpectrum {
    window: Vec<f32>,
    w2sum: f64,
    rate: f64,
    /// The bins each side of 0 Hz that count for the noise.
    band: usize,
    inband: Vec<f32>,
    scratch: Vec<f32>,
}

impl CarrierSpectrum {
    /// For `n`-point spectra of samples at `rate_hz`, whose noise is measured within `band_hz` (two-sided) around 0 Hz. The band
    /// covers at least the bins either side of 0 Hz and at most all bins but the one at half the rate. Errors when `n` is below 4,
    /// the rate is not finite and above 0, or the band is not finite and at least 0.
    pub fn new(n: usize, rate_hz: f64, band_hz: f64) -> Result<Self, DspError> {
        if n < 4 {
            return Param::FftSize.at_least(4.0, n as f64);
        }
        if !(rate_hz.is_finite() && rate_hz > 0.0) {
            return Param::Rate.positive(rate_hz);
        }
        if !(band_hz.is_finite() && band_hz >= 0.0) {
            return Param::Bandwidth.at_least(0.0, band_hz);
        }
        let mut window = vec![0.0f32; n];
        Window::Hann.fill_periodic(&mut window);
        let w2sum = window.iter().map(|&w| (w as f64).powi(2)).sum();
        let band = ((band_hz / 2.0 / rate_hz * n as f64) as usize).clamp(1, n / 2 - 1);
        Ok(CarrierSpectrum {
            window,
            w2sum,
            rate: rate_hz,
            band,
            inband: Vec::with_capacity(2 * band + 1),
            scratch: Vec::with_capacity(2 * band + 1),
        })
    }

    /// The window, to multiply the samples by before the transform.
    pub fn window(&self) -> &[f32] {
        &self.window
    }

    /// The power of a carrier whose peak bin has power `p` and its neighbours `l` and `r` (powers `|X[k]|^2` of the unscaled
    /// transform). By Parseval, the three bins of the Hann main lobe hold `n sum(w^2)` times the carrier's power.
    pub fn carrier_power(&self, l: f64, p: f64, r: f64) -> f64 {
        if self.window.is_empty() {
            return 0.0;
        }
        (l + p + r) / (self.window.len() as f64 * self.w2sum)
    }

    /// The noise density (power per Hz) of a spectrum in FFT order that is the mean of `frames` power spectra, from the median
    /// of its bins within the band. The median of averaged noise bins lies below their mean by a fixed factor (their law is a
    /// chi-square with `2 frames` degrees of freedom), which this undoes. Returns `None` when `powers` does not hold `n` bins.
    pub fn noise_density(&mut self, powers: &[f32], frames: u32) -> Option<f64> {
        let n = powers.len();
        if n == 0 || n != self.window.len() {
            return None;
        }
        let b = self.band;
        self.inband.clear();
        self.inband.extend_from_slice(&powers[..=b]);
        self.inband.extend_from_slice(&powers[n - b..]);
        let median = percentile(&self.inband, 0.5, &mut self.scratch) as f64;
        let to_mean = crate::floor::quantile_to_mean(frames.clamp(1, 100_000), 0.5)
            .unwrap_or(std::f64::consts::LN_2);
        Some(median / to_mean / self.w2sum / self.rate)
    }
}

/// The `p` quantile (0..1) of `values`, using `scratch` (no allocation once it has grown).
pub fn percentile(values: &[f32], p: f64, scratch: &mut Vec<f32>) -> f32 {
    if values.is_empty() {
        return f32::NAN;
    }
    scratch.clear();
    scratch.extend_from_slice(values);
    let k = ((p.clamp(0.0, 1.0) * (values.len() - 1) as f64).round()) as usize;
    let (_, v, _) = scratch.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    *v
}

/// A noise-floor estimate of a dB spectrum: its 20th percentile. Signals rarely cover more than 80% of a band, so this lands on the
/// noise. It reads below the mean noise power: by about 6.5 dB with no averaging (the power of one noise bin is exponentially
/// distributed), and by less as averaging narrows the spread (about 0.5 dB at 100 frames).
pub fn noise_floor_db(spectrum_db: &[f32], scratch: &mut Vec<f32>) -> f32 {
    percentile(spectrum_db, 0.2, scratch)
}

/// Bridges the DC spike of a centred spectrum `s` (index `len / 2` is DC): if the centre bin exceeds 4 times the mean of bins
/// 4 to 8 on either side, the bins within 3 of the centre take that mean. A spectrum of fewer than 18 bins is left as it is.
pub fn bridge_dc(s: &mut [f64]) {
    let c = s.len() / 2;
    if c < 9 {
        return;
    }
    let m = (s[c - 8..=c - 4].iter().sum::<f64>() + s[c + 4..=c + 8].iter().sum::<f64>()) / 10.0;
    if s[c] > 4.0 * m {
        for v in &mut s[c - 3..=c + 3] {
            *v = m;
        }
    }
}

/// A forward transform that [`cyclic_lines`] keeps, with the backend selection it was made under.
struct KeptPlan {
    selected: &'static str,
    fft: Fft,
}

thread_local! {
    /// The transforms this thread used last in [`cyclic_lines`], newest first, [`KEPT_PLANS`] at most.
    static CYCLIC_PLANS: std::cell::RefCell<Vec<KeptPlan>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Sizes of transform a thread keeps for [`cyclic_lines`].
const KEPT_PLANS: usize = 4;

/// Runs `job` with a forward transform of `n` points: the one this thread kept, or a new one when it kept none of this size
/// or the selected backend has changed. The transform is kept afterwards.
fn with_kept_plan<R>(n: usize, job: impl FnOnce(&mut Fft) -> R) -> R {
    let selected = BACKENDS.selected();
    let kept = CYCLIC_PLANS.with_borrow_mut(|plans| {
        let at = plans
            .iter()
            .position(|p| p.fft.len() == n && p.selected == selected)?;
        Some(plans.remove(at))
    });
    let mut plan = kept.unwrap_or_else(|| KeptPlan {
        selected,
        fft: Fft::forward(n),
    });
    let r = job(&mut plan.fft);
    CYCLIC_PLANS.with_borrow_mut(|plans| {
        plans.insert(0, plan);
        plans.truncate(KEPT_PLANS);
    });
    r
}

/// Returns the spectral lines of the squared envelope of `x` at sample rate `rate`, the lines a linear modulation shows at its
/// symbol rate. Power spectra of the mean-removed `|x|^2` are averaged over blocks of `n` samples. Each line is a local maximum
/// with its frequency in Hz and its ratio to the mean of the bins within 256 of it, at least `min_ratio`, between `lo_hz` and
/// `hi_hz` (the aliases `rate - f` and `rate + f` count). The strongest line comes first. Returns no line if `x` is shorter than
/// `n`. The calling thread keeps the transform of its last few sizes, so a repeated call does not plan it again.
pub fn cyclic_lines(
    x: &[Complex32],
    rate: f64,
    n: usize,
    lo_hz: f64,
    hi_hz: f64,
    min_ratio: f64,
) -> Vec<(f64, f64)> {
    let blocks = x.len() / n;
    if blocks == 0 {
        return Vec::new();
    }
    let mut acc = vec![0f64; n];
    let mut buf = vec![Complex32::new(0.0, 0.0); n];
    with_kept_plan(n, |fft| {
        for b in 0..blocks {
            let seg = &x[b * n..(b + 1) * n];
            let mean = seg.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / n as f64;
            for (o, s) in buf.iter_mut().zip(seg) {
                let p = s.norm_sqr() as f64;
                *o = Complex32::new(
                    if p.is_finite() {
                        (p - mean) as f32
                    } else {
                        0.0
                    },
                    0.0,
                );
            }
            fft.process(&mut buf);
            for (a, v) in acc.iter_mut().zip(&buf) {
                *a += v.norm_sqr() as f64;
            }
        }
    });
    let bin = rate / n as f64;
    let mut out = Vec::new();
    let half = n / 2;
    for k in 2..half - 2 {
        let v = acc[k];
        if v < acc[k - 1] || v < acc[k + 1] {
            continue;
        }
        let (a, b) = (k.saturating_sub(256).max(1), (k + 257).min(half));
        let mut bg = 0.0f64;
        let mut cnt = 0;
        for (j, &p) in acc.iter().enumerate().take(b).skip(a) {
            if j + 2 < k || j > k + 2 {
                bg += p;
                cnt += 1;
            }
        }
        let ratio = v / (bg / cnt.max(1) as f64).max(1e-30);
        if ratio < min_ratio {
            continue;
        }
        // Parabolic interpolation.
        let (ym, y0, yp) = (acc[k - 1].ln(), v.ln(), acc[k + 1].ln());
        let den = ym - 2.0 * y0 + yp;
        let d = if den.abs() > 1e-12 {
            0.5 * (ym - yp) / den
        } else {
            0.0
        };
        let f = (k as f64 + d.clamp(-0.5, 0.5)) * bin;
        for cand in [f, rate - f, rate + f] {
            if cand >= lo_hz && cand <= hi_hz {
                out.push((cand, ratio));
            }
        }
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn a_real_cosine_reads_minus_6_db_in_both_bins() {
        let (n, rate) = (1024usize, 1024.0);
        let mut a = SpectrumAnalyzer::new(SpectrumConfig::new(n, rate)).unwrap();
        let k = 64.0;
        let x: Vec<f32> = (0..4 * n)
            .map(|i| (std::f64::consts::TAU * k * i as f64 / n as f64).cos() as f32)
            .collect();
        assert!(a.push_real(&x) > 0);
        let db = a.spectrum_db().to_vec();
        let (up, down) = (db[n / 2 + 64], db[n / 2 - 64]);
        assert!(
            (up + 6.02).abs() < 0.1 && (down + 6.02).abs() < 0.1,
            "{up} {down}"
        );
        assert!((a.power()[n / 2 + 64] - 0.25).abs() < 0.01);
    }

    #[test]
    fn the_carrier_spectrum_reads_carrier_power_and_noise_density() {
        assert!(CarrierSpectrum::new(2, 1000.0, 500.0).is_err());
        assert!(CarrierSpectrum::new(64, 0.0, 500.0).is_err());
        assert!(CarrierSpectrum::new(64, 1000.0, f64::NAN).is_err());
        let (n, rate) = (256usize, 8000.0);
        let mut c = CarrierSpectrum::new(n, rate, 6000.0).unwrap();
        let mut fft = Fft::forward(n);
        let w = c.window().to_vec();
        // A tone of amplitude 0.5 (power 0.25) on bin 10 and one a quarter bin off: the three bins hold its power.
        for (bin, tol) in [(10.0, 1e-4), (10.25, 0.06)] {
            let mut x: Vec<Complex32> = (0..n)
                .map(|i| {
                    Complex32::from_polar(
                        0.5,
                        (std::f64::consts::TAU * bin * i as f64 / n as f64) as f32,
                    ) * w[i]
                })
                .collect();
            fft.process(&mut x);
            let p: Vec<f64> = x.iter().map(|v| v.norm_sqr() as f64).collect();
            assert!(
                (c.carrier_power(p[9], p[10], p[11]) / 0.25 - 1.0).abs() < tol,
                "{bin}"
            );
        }
        // White noise of power 0.01, averaged over 16 frames: density 0.01 / rate.
        let mut rng = Rng(7);
        // Each lane has half the power.
        let s = (0.005f64).sqrt();
        let mut noise = || Complex32::new((s * rng.gauss()) as f32, (s * rng.gauss()) as f32);
        let frames = 16u32;
        let mut mean = vec![0.0f32; n];
        for _ in 0..frames {
            let mut x: Vec<Complex32> = (0..n).map(|i| noise() * w[i]).collect();
            fft.process(&mut x);
            for (m, v) in mean.iter_mut().zip(&x) {
                *m += v.norm_sqr() / frames as f32;
            }
        }
        let n0 = c.noise_density(&mean, frames).unwrap();
        assert!((n0 * rate / 0.01 - 1.0).abs() < 0.15, "{}", n0 * rate);
        assert_eq!(c.noise_density(&mean[..10], frames), None);
        assert_eq!(CarrierSpectrum::default().noise_density(&mean, 1), None);
    }

    #[test]
    fn welch_reads_tones_and_noise_power() {
        // A complex tone of magnitude 0.5 on bin 40 of 512 reads 0.25 with tone scaling.
        let n = 512;
        let x = ctone(20_000, 40.0 / n as f64, 0.5);
        let mut p = vec![0.0; n];
        let frames = welch(&x, n, Window::Hann, 0.5, Scaling::Tone, &mut p).unwrap();
        assert_eq!(frames, 1 + (20_000 - 512) / 256);
        assert!((p[n / 2 + 40] - 0.25).abs() < 1e-4, "{}", p[n / 2 + 40]);
        // Power scaling: the bins sum to the mean power, for noise of power 2.
        let mut rng = Rng(99);
        let noise: Vec<Complex32> = (0..65_536)
            .map(|_| Complex32::new(rng.gauss() as f32, rng.gauss() as f32))
            .collect();
        let mut w = Welch::new(256, Window::BlackmanHarris4, 0.67, Scaling::Power).unwrap();
        let mut q = vec![0.0; 256];
        w.compute(&noise, &mut q).unwrap();
        let total: f32 = q.iter().sum();
        assert!((total / 2.0 - 1.0).abs() < 0.02, "{total}");
        // A real cosine reads A^2 / 4 in each of its bins.
        let r = rtone(8192, 32.0 / 256.0, 1.0);
        w = Welch::new(256, Window::Hann, 0.5, Scaling::Tone).unwrap();
        w.compute_real(&r, &mut q).unwrap();
        assert!((q[128 + 32] - 0.25).abs() < 1e-4 && (q[128 - 32] - 0.25).abs() < 1e-4);
        assert!(w.compute(&noise[..100], &mut q).is_err());
        assert!(w.compute(&noise, &mut q[..10]).is_err());
        assert!(Welch::new(256, Window::Hann, 1.0, Scaling::Tone).is_err());
    }

    #[test]
    fn full_scale_tone_reads_zero_dbfs() {
        for window in [Window::BlackmanHarris4, Window::Hann, Window::Rectangular] {
            for n in [256usize, 4096, 65_536, 262_144] {
                let rate = 1e6;
                let mut cfg = SpectrumConfig::new(n, rate);
                cfg.window = window;
                let mut s = SpectrumAnalyzer::new(cfg).unwrap();
                let bin = n / 2 + n / 8 + 3;
                let f = s.bin_hz(bin) / rate;
                assert_eq!(s.push(&ctone(n, f, 1.0)), 1);
                let db = s.spectrum_db();
                assert!(db[bin].abs() < 0.5, "{window:?} {n}: {}", db[bin]);
                let max = db
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0;
                assert_eq!(max, bin);
            }
        }
    }

    #[test]
    fn the_spec_scaling_reads_what_the_spec_says() {
        let n = 4096;
        let rate = 1e6;
        let mut s = SpectrumAnalyzer::new(SpectrumConfig::spec(n, rate)).unwrap();
        let bin = n / 2 + 100;
        s.push(&ctone(n, s.bin_hz(bin) / rate, 1.0));
        let db = s.spectrum_db().to_vec();
        assert!((db[bin] + 1.76).abs() < 0.01, "peak {}", db[bin]);
        assert!((db[bin + 1] + 7.78).abs() < 0.01 && (db[bin - 1] + 7.78).abs() < 0.01);
        // The bins add up to the mean power (1.0).
        let sum: f64 = s.power().iter().map(|&p| p as f64).sum();
        assert!((sum - 1.0).abs() < 1e-3, "sum {sum}");
        // White noise at -30 dBFS reads -30 - 10 log10(N) per bin; silence reads the -140 dB floor.
        let mut cfg = SpectrumConfig::spec(n, rate);
        cfg.frames_per_second = rate / n as f64;
        cfg.average = 200;
        let mut s = SpectrumAnalyzer::new(cfg.clone()).unwrap();
        let sd = (1e-3f64 / 2.0).sqrt();
        let mut rng = Rng(5);
        let x: Vec<Complex32> = (0..n * 100)
            .map(|_| Complex32::new((rng.gauss() * sd) as f32, (rng.gauss() * sd) as f32))
            .collect();
        s.push(&x);
        let mean: f64 = s.power().iter().map(|&p| p as f64).sum::<f64>() / n as f64;
        let want = -30.0 - 10.0 * (n as f64).log10();
        assert!(
            (10.0 * mean.log10() - want).abs() < 0.2,
            "{} vs {want}",
            10.0 * mean.log10()
        );
        let mut quiet = SpectrumAnalyzer::new(cfg).unwrap();
        quiet.push(&vec![Complex32::default(); n]);
        assert_eq!(quiet.spectrum_db()[0], -140.0);
    }

    #[test]
    fn a_partial_frame_can_be_dropped() {
        let n = 1024;
        let mut cfg = SpectrumConfig::new(n, 1e6);
        cfg.frames_per_second = 1e6 / n as f64;
        let mut s = SpectrumAnalyzer::new(cfg).unwrap();
        assert_eq!(s.push(&ctone(700, 0.1, 1.0)), 0);
        assert_eq!(s.partial(), 700);
        s.drop_partial();
        assert_eq!(s.partial(), 0);
        // The next frame takes n new samples, all of the new signal.
        assert_eq!(s.push(&ctone(n - 1, -0.2, 1.0)), 0);
        assert_eq!(s.push(&ctone(1, -0.2, 1.0)), 1);
        let p = s.power();
        let peak = p
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!(
            (s.bin_hz(peak) + 0.2e6).abs() < 1e6 / n as f64,
            "{}",
            s.bin_hz(peak)
        );
    }

    #[test]
    fn bins_run_from_low_to_high_with_dc_in_the_middle() {
        let n = 1024;
        let rate = 1024.0;
        let mut s = SpectrumAnalyzer::new(SpectrumConfig::new(n, rate)).unwrap();
        assert_eq!(s.bin_hz(n / 2), 0.0);
        assert_eq!(s.bin_hz(0), -512.0);
        for f in [-300.0, 0.0, 100.0] {
            s.reset();
            s.push(&ctone(n, f / rate, 1.0));
            let p = s.power();
            let max = p
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0;
            assert_eq!(s.bin_hz(max), f);
        }
    }

    #[test]
    fn white_noise_floor_matches_theory() {
        let n = 1024;
        let rate = 1e6;
        let mut cfg = SpectrumConfig::new(n, rate);
        cfg.frames_per_second = rate / n as f64; // no overlap
        cfg.average = 1000;
        let mut s = SpectrumAnalyzer::new(cfg).unwrap();
        let sigma2 = 0.01; // -20 dBFS total
        let sd = (sigma2 / 2.0f64).sqrt();
        let mut rng = Rng(0x1234_5678);
        let x: Vec<Complex32> = (0..n * 200)
            .map(|_| Complex32::new((rng.gauss() * sd) as f32, (rng.gauss() * sd) as f32))
            .collect();
        assert_eq!(s.push(&x), 200);
        let mean: f64 = s.power().iter().map(|&p| p as f64).sum::<f64>() / n as f64;
        let theory = 10.0 * sigma2.log10() - 10.0 * (n as f64 / s.enbw_bins()).log10();
        assert!(
            (10.0 * mean.log10() - theory).abs() < 0.2,
            "{} vs {theory}",
            10.0 * mean.log10()
        );
        assert!((s.enbw_bins() - 2.0).abs() < 0.01);
        let nf = s.noise_floor_db() as f64;
        assert!(nf < theory && nf > theory - 1.5, "floor {nf} vs {theory}");
    }

    #[test]
    fn frame_rate_by_overlap_and_by_skipping() {
        let rate = 48_000.0;
        // 4800 hops on a 1024 FFT: input is skipped; 10 fps.
        let mut cfg = SpectrumConfig::new(1024, rate);
        cfg.frames_per_second = 10.0;
        let mut s = SpectrumAnalyzer::new(cfg).unwrap();
        let x = vec![Complex32::new(0.1, 0.0); 48_000];
        let mut frames = 0;
        for c in x.chunks(777) {
            frames += s.push(c);
        }
        assert_eq!(frames, 10);
        // 160 fps on 1024 points at 48 kHz: hop 300, overlapping frames.
        let mut cfg = SpectrumConfig::new(1024, rate);
        cfg.frames_per_second = 160.0;
        cfg.peak_hold = true;
        let mut s = SpectrumAnalyzer::new(cfg).unwrap();
        let frames: usize = x.chunks(1).map(|c| s.push(c)).sum();
        assert_eq!(frames, (48_000 - 1024) / 300 + 1);
        assert!(s.peak_db().is_some());
        assert!(SpectrumAnalyzer::new(SpectrumConfig::new(1000, rate)).is_err());
    }

    #[test]
    fn reduce_and_percentile() {
        let src: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let mut out = [0.0; 3];
        reduce_max(&src, &mut out);
        assert_eq!(out, [2.0, 5.0, 9.0]);
        let mut wide = [0.0; 20];
        reduce_max(&src, &mut wide);
        assert_eq!(wide[0], 0.0);
        assert_eq!(wide[19], 9.0);
        let mut scratch = Vec::new();
        assert_eq!(percentile(&src, 0.5, &mut scratch), 5.0);
        assert_eq!(noise_floor_db(&src, &mut scratch), 2.0);
    }
    #[test]
    fn the_dc_spike_is_bridged() {
        let mut s = vec![1.0; 64];
        s[32] = 100.0;
        bridge_dc(&mut s);
        assert_eq!(s[32], 1.0);
        let mut short = vec![1.0, 9.0, 1.0];
        bridge_dc(&mut short);
        assert_eq!(short, [1.0, 9.0, 1.0]);
    }

    #[test]
    fn the_squared_envelope_shows_the_symbol_rate() {
        // BPSK at 1/8 of the rate with rectangular pulses through a 3-tap smoother: the envelope dips at each symbol edge.
        let mut x = Vec::new();
        let mut seed = 1u32;
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let v = if seed >> 16 & 1 == 0 { 1.0 } else { -1.0 };
            x.extend(std::iter::repeat_n(Complex32::new(v, 0.0), 8));
        }
        let y: Vec<Complex32> = x.windows(3).map(|w| (w[0] + w[1] + w[2]) / 3.0).collect();
        let lines = cyclic_lines(&y, 8000.0, 4096, 500.0, 1500.0, 4.0);
        assert!((lines[0].0 - 1000.0).abs() < 2.0, "{lines:?}");
        assert!(cyclic_lines(&y[..100], 8000.0, 4096, 500.0, 1500.0, 4.0).is_empty());
    }
}

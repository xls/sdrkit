//! Frequency-shift keying: tone detectors, the generic M-FSK demodulator, soft values for non-coherent M-FSK, and the level slicer
//! for 2- and 4-level FSK after a discriminator.
//!
//! Three ways to tell the tones apart, chosen by the caller:
//!
//! | Detector | Type | Suits |
//! |---|---|---|
//! | **Correlator** | [`ToneCorrelator`]: a sliding DFT per tone over one symbol, `O(M)` per sample, re-summed exactly once per window | 2 to 64 tones at a few to a few hundred samples per symbol: RTTY, NAVTEX, AFSK 1200, DSC, ALE, buoys |
//! | **FFT bins** | [`FftTones`]: one FFT per symbol at a time the caller chooses | slow many-tone protocols: FT8, JT65, WSPR, MFSK, Olivia |
//! | **Discriminator** | `sdrkit_algo_dsp::fm::Discriminator`, then [`LevelSlicer`] on the symbol-rate levels | 2- and 4-level FSK with a Nyquist or Gaussian pulse: POCSAG, FLEX, C4FM, DMR, NXDN, dPMR, M17, GFSK |
//!
//! Real audio (AFSK on a discriminator, Bell 202, SAME, DSC) goes through [`Analytic`] first: a Hilbert transformer makes it
//! complex, so a tone correlator sees only the positive-frequency tone and no image at `-f` (the ripple at twice the tone that a
//! real input against complex tones leaves in the sum).
//!
//! [`MfskDemod`] is the streaming demodulator: a [`ToneCorrelator`] and a symbol clock that follows the instant of clearest
//! decision (the maximum of the averaged tone contrast over the symbol phases, refined by a parabola and tracked by a second-order
//! loop, so a clock error of a few hundred ppm is followed). It appends the `M` tone energies of every symbol.
//!
//! [`FskLlr`] turns energies into LLRs of the bits (positive means 0), exactly: for each tone the non-coherent (Rician) likelihood
//! `ln I0(2 A |S_t| / N)` and per bit the log-sum-exp over the tones whose label has that bit 0, minus the same over 1. The values
//! are LLRs of the bits, so repeated copies of a bit (NAVTEX and SITOR-B send every character twice) combine by adding them.
//! [`TrackedFskLlr`] estimates the amplitude and noise blind from the symbols themselves, for a streaming receiver.
//!
//! [`DiscriminatorLevels`] is the front half of every discriminator receiver (discriminator, one-symbol integrate-and-dump,
//! zero-crossing symbol clock): one level per symbol, for a [`LevelSlicer`] or for an MLSE (`crate::gmsk::GfskMlseDemod`).
//! [`LabelMap`] names the bit labels of the tones or levels (natural, Gray, binary, C4FM) for settings.
//!
//! ```
//! use sdrkit_algo_modem::fsk::{FskLlr, MfskConfig, MfskDemod};
//! use sdrkit_algo_modem::Complex32;
//! // 2-FSK, tones at +-0.05 cycles per sample, 20 samples per symbol.
//! let mut d = MfskDemod::new(MfskConfig::new(&[-0.05, 0.05], 20.0)).unwrap();
//! let mut x = Vec::new();
//! for k in 0..200 {
//!     let f = if k % 3 == 0 { 0.05 } else { -0.05 };
//!     for _ in 0..20 {
//!         let n = x.len() as f64;
//!         x.push(Complex32::from_polar(1.0, (std::f64::consts::TAU * f * n) as f32));
//!     }
//! }
//! let mut e = Vec::new();
//! d.process(&x, &mut e);
//! let llr = FskLlr::natural(2).unwrap();
//! let mut l = vec![0f32; e.len() / 2];
//! llr.llrs(&e, 20.0, 1.0, &mut l).unwrap();
//! assert!(l.len() > 150);
//! ```

use crate::timing::{SymbolSync, SymbolSyncConfig, ZeroCrossing};
use crate::{Complex32, Constellation, Demap, ModemError, Param, PointSet};
use num_complex::Complex64;
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::loopfilter::LoopFilter;
use sdrkit_algo_dsp::peak;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::stats::WindowSum;
use std::f64::consts::TAU;

/// Samples between exact re-anchors of a rotator (so `f32` rounding never builds up).
const REANCHOR: u32 = 1024;

/// `e^(-j 2 pi phase)` for a phase in cycles.
#[inline]
fn phasor_down(cycles: f64) -> Complex32 {
    let (s, c) = approx::sin_cos(-TAU * cycles);
    Complex32::new(c, s)
}

/// Real audio to an analytic (complex) signal: `x[n - D] + j H{x}[n]` with a Kaiser-windowed Hilbert transformer of `taps` taps
/// (`D = (taps - 1) / 2`). A cosine becomes a positive-frequency complex tone, so tone detectors see no image.
#[derive(Clone, Debug)]
pub struct Analytic {
    fir: Fir<f32>,
    centre: usize,
}

impl Analytic {
    /// A transformer of `taps` taps (odd, 7 to 1023; 4k+3 is best) with Kaiser `beta` (about 8 for 80 dB).
    pub fn new(taps: usize, beta: f64) -> Result<Self, ModemError> {
        Param::Taps.check_count(taps, 7, 1023)?;
        if taps.is_multiple_of(2) {
            return Param::Taps.not_allowed(taps as f64);
        }
        Param::Beta.check_in(beta, 0.0, f64::INFINITY)?;
        Ok(Analytic {
            fir: Fir::new(&fir::hilbert(taps, beta)?),
            centre: (taps - 1) / 2,
        })
    }

    /// A transformer flat within about 0.1 dB from `low_hz` to `rate - low_hz` (Nyquist side) at `rate`.
    pub fn for_band(rate: f64, low_hz: f64) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        Param::LowHz.check_open(low_hz, 0.0, rate / 4.0)?;
        let n = fir::hilbert_len(rate, low_hz, 60.0)?.clamp(7, 1023);
        Self::new(n | 1, sdrkit_algo_dsp::fir::design_beta(60.0))
    }

    /// The delay of the output in samples.
    pub fn delay(&self) -> usize {
        self.centre
    }

    /// Clears the history.
    pub fn reset(&mut self) {
        self.fir.reset();
    }

    /// One sample in, one analytic sample out.
    #[inline]
    pub fn push(&mut self, x: f32) -> Complex32 {
        self.fir.push(x);
        let q = self.fir.output();
        let i = self.fir.window()[self.centre];
        Complex32::new(i, q)
    }

    /// Converts `x` into `out` (the same length).
    pub fn process(&mut self, x: &[f32], out: &mut [Complex32]) -> Result<(), ModemError> {
        if out.len() < x.len() {
            return Err(ModemError::OutputTooShort { need: x.len() });
        }
        for (o, &v) in out.iter_mut().zip(x) {
            *o = self.push(v);
        }
        Ok(())
    }
}

/// A bank of sliding DFTs: for each tone `t`, `S_t[n] = sum_{k=n-N+1}^{n} x[k] e^(-j 2 pi f_t k)` over the last `N` samples. Each
/// tone has a rotator re-anchored exactly every 1024 samples from an `f64` phase (no drift, no phase restart, any non-integer tone
/// frequency); the running sums are recomputed exactly from the stored terms once per window, so `f32` rounding never builds up.
#[derive(Clone, Debug)]
pub struct ToneCorrelator {
    freqs: Vec<f64>,
    phase: Vec<f64>,
    rot: Vec<Complex32>,
    step: Vec<Complex32>,
    since: u32,
    ring: Vec<Complex32>,
    sums: Vec<Complex32>,
    n: usize,
    pos: usize,
    filled: usize,
}

impl ToneCorrelator {
    /// Tones at `freqs` (cycles per sample, each in `[-0.5, 0.5]`, 1 to 256 of them) over windows of `window` samples (1 to 65536).
    pub fn new(freqs: &[f64], window: usize) -> Result<Self, ModemError> {
        Param::Tones.check_count(freqs.len(), 1, 256)?;
        for &f in freqs {
            Param::Frequency.check_in(f, -0.5, 0.5)?;
        }
        if !(1..=65536).contains(&window) {
            return Param::Window.range(1.0, 65_536.0, window as f64);
        }
        let m = freqs.len();
        let mut c = ToneCorrelator {
            freqs: freqs.to_vec(),
            phase: vec![0.0; m],
            rot: vec![Complex32::new(1.0, 0.0); m],
            step: freqs.iter().map(|&f| phasor_down(f)).collect(),
            since: 0,
            ring: vec![Complex32::default(); window * m],
            sums: vec![Complex32::default(); m],
            n: window,
            pos: 0,
            filled: 0,
        };
        c.reset();
        Ok(c)
    }

    /// Number of tones.
    pub fn tones(&self) -> usize {
        self.freqs.len()
    }

    /// The window length.
    pub fn window(&self) -> usize {
        self.n
    }

    /// Clears the sums and restarts the rotators at phase 0.
    pub fn reset(&mut self) {
        self.phase.iter_mut().for_each(|p| *p = 0.0);
        self.rot
            .iter_mut()
            .for_each(|r| *r = Complex32::new(1.0, 0.0));
        self.since = 0;
        self.ring.iter_mut().for_each(|v| *v = Complex32::default());
        self.sums.iter_mut().for_each(|v| *v = Complex32::default());
        self.pos = 0;
        self.filled = 0;
    }

    /// Whether a whole window has been seen.
    pub fn ready(&self) -> bool {
        self.filled >= self.n
    }

    /// Takes one sample.
    #[inline]
    pub fn push(&mut self, x: Complex32) {
        let m = self.freqs.len();
        let row = &mut self.ring[self.pos * m..(self.pos + 1) * m];
        for (((r, sum), rot), step) in row
            .iter_mut()
            .zip(&mut self.sums)
            .zip(&mut self.rot)
            .zip(&self.step)
        {
            let y = x * *rot;
            *sum += y - *r;
            *r = y;
            *rot *= *step;
        }
        self.since += 1;
        if self.since == REANCHOR {
            self.since = 0;
            for t in 0..m {
                self.phase[t] = (self.phase[t] + self.freqs[t] * REANCHOR as f64).rem_euclid(1.0);
                self.rot[t] = phasor_down(self.phase[t]);
            }
        }
        self.pos += 1;
        if self.filled < self.n {
            self.filled += 1;
        }
        if self.pos == self.n {
            self.pos = 0;
            // Exact re-sum once per window.
            for t in 0..m {
                let mut s = Complex32::default();
                for k in 0..self.n {
                    s += self.ring[k * m + t];
                }
                self.sums[t] = s;
            }
        }
    }

    /// The complex sums `S_t` of the last window.
    pub fn sums(&self) -> &[Complex32] {
        &self.sums
    }

    /// The energies `|S_t|^2` of the last window into `out[0..tones]`.
    pub fn energies(&self, out: &mut [f32]) -> Result<(), ModemError> {
        let m = self.freqs.len();
        if out.len() < m {
            return Err(ModemError::OutputTooShort { need: m });
        }
        for (o, s) in out.iter_mut().zip(&self.sums) {
            *o = s.norm_sqr();
        }
        Ok(())
    }
}

/// Tone energies from one FFT of a symbol: `|X_k|^2` at the tone bins, after zero padding to the FFT size. With the window equal to
/// the symbol and the tone on a bin it returns the same values as [`ToneCorrelator`]. The plan and buffer are made at `new`.
#[derive(Clone, Debug)]
pub struct FftTones {
    fft: Fft,
    buf: Vec<Complex32>,
    bins: Vec<usize>,
}

impl FftTones {
    /// An `nfft`-point FFT (2 to 65536) reading the energies at `bins` (each below `nfft`; negative frequencies are the upper bins).
    pub fn new(nfft: usize, bins: &[usize]) -> Result<Self, ModemError> {
        Param::FftSize.check_count(nfft, 2, 65_536)?;
        if bins.is_empty() {
            return Param::Tones.at_least(1.0, 0.0);
        }
        if let Some(&b) = bins.iter().find(|&&b| b >= nfft) {
            return Param::Bin.range(0.0, (nfft - 1) as f64, b as f64);
        }
        Ok(FftTones {
            fft: Fft::forward(nfft),
            buf: vec![Complex32::default(); nfft],
            bins: bins.to_vec(),
        })
    }

    /// The FFT size.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Never true.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// The energies of the tones in `x` (at most `nfft` samples, zero padded) into `out[0..bins]`.
    pub fn energies(&mut self, x: &[Complex32], out: &mut [f32]) -> Result<(), ModemError> {
        let n = self.buf.len();
        if x.len() > n {
            return Param::Input.range(0.0, n as f64, x.len() as f64);
        }
        if out.len() < self.bins.len() {
            return Err(ModemError::OutputTooShort {
                need: self.bins.len(),
            });
        }
        self.buf[..x.len()].copy_from_slice(x);
        self.buf[x.len()..]
            .iter_mut()
            .for_each(|v| *v = Complex32::default());
        self.fft.process(&mut self.buf);
        for (o, &b) in out.iter_mut().zip(&self.bins) {
            *o = self.buf[b].norm_sqr();
        }
        Ok(())
    }

    /// Every bin's energy of the last call (the whole spectrum, for searches).
    pub fn spectrum(&self, out: &mut [f32]) -> Result<(), ModemError> {
        if out.len() < self.buf.len() {
            return Err(ModemError::OutputTooShort {
                need: self.buf.len(),
            });
        }
        for (o, v) in out.iter_mut().zip(&self.buf) {
            *o = v.norm_sqr();
        }
        Ok(())
    }
}

/// Settings of [`MfskDemod`].
#[derive(Clone, Debug, PartialEq)]
pub struct MfskConfig {
    /// Tone frequencies in cycles per sample, in label order of the tone index (tone 0 first).
    pub tones: Vec<f64>,
    /// Samples per symbol (any value of at least 2; fractional is fine).
    pub sps: f64,
    /// Correlation window in samples (default: `sps` rounded down; shorter windows tolerate more timing error).
    pub window: usize,
    /// Noise bandwidth of the symbol clock loop per symbol (default 0.01).
    pub bn_t: f64,
    /// Symbols over which the contrast per symbol phase is averaged (default 8).
    pub average_symbols: f64,
    /// The largest clock error the loop follows, in ppm (default 2000).
    pub max_ppm: f64,
}

impl MfskConfig {
    /// Defaults for these tones and samples per symbol.
    pub fn new(tones: &[f64], sps: f64) -> Self {
        MfskConfig {
            tones: tones.to_vec(),
            sps,
            window: sps.max(1.0).floor() as usize,
            bn_t: 0.01,
            average_symbols: 8.0,
            max_ppm: 2000.0,
        }
    }

    /// Tones given in hertz at `rate`, symbols at `baud`.
    pub fn from_hz(tones_hz: &[f64], baud: f64, rate: f64) -> Self {
        let t: Vec<f64> = tones_hz.iter().map(|f| f / rate).collect();
        Self::new(&t, rate / baud)
    }
}

/// The streaming M-FSK demodulator (module documentation): appends `M` tone energies per symbol.
#[derive(Clone, Debug)]
pub struct MfskDemod {
    corr: ToneCorrelator,
    analytic: Option<Analytic>,
    sps: f64,
    slots: Vec<f32>,
    alpha: f32,
    /// Sample count since the last strobe, as a position inside the symbol.
    phase: f64,
    lf: LoopFilter,
    max_frac: f64,
    energy: Vec<f32>,
    symbols: u64,
}

impl MfskDemod {
    /// A demodulator with these settings. Errors for fewer than 2 or more than 256 tones, `sps < 2` or above 65536, a window of 0
    /// or longer than `sps + 1`, or a bad loop setting.
    pub fn new(cfg: MfskConfig) -> Result<Self, ModemError> {
        if cfg.tones.len() < 2 {
            return Param::Tones.at_least(2.0, cfg.tones.len() as f64);
        }
        if !(cfg.sps >= 2.0 && cfg.sps <= 65536.0) {
            return Param::Sps.range(2.0, 65_536.0, cfg.sps);
        }
        if cfg.window == 0 || cfg.window as f64 > cfg.sps + 1.0 {
            return Param::Window.range(1.0, cfg.sps + 1.0, cfg.window as f64);
        }
        if cfg.average_symbols < 1.0 || cfg.average_symbols.is_nan() {
            return Param::Averaging.at_least(1.0, cfg.average_symbols);
        }
        Param::MaxPpm.check_in(cfg.max_ppm, 0.0, f64::INFINITY)?;
        let mut lf = LoopFilter::new(cfg.bn_t, 1.0)?;
        lf.set_clamp(cfg.max_ppm * 1e-6 * cfg.sps);
        let nslots = cfg.sps.ceil() as usize;
        Ok(MfskDemod {
            corr: ToneCorrelator::new(&cfg.tones, cfg.window)?,
            analytic: None,
            sps: cfg.sps,
            slots: vec![0.0; nslots],
            alpha: (1.0 / cfg.average_symbols) as f32,
            phase: 0.0,
            lf,
            max_frac: cfg.max_ppm * 1e-6,
            energy: vec![0.0; cfg.tones.len()],
            symbols: 0,
        })
    }

    /// Also accept real audio through [`process_real`](Self::process_real), made analytic by `analytic`.
    pub fn with_analytic(mut self, analytic: Analytic) -> Self {
        self.analytic = Some(analytic);
        self
    }

    /// Number of tones.
    pub fn tones(&self) -> usize {
        self.corr.tones()
    }

    /// The clock error the loop has measured, in ppm (positive: symbols longer than nominal).
    pub fn clock_ppm(&self) -> f64 {
        self.lf.integrator() / self.sps * 1e6
    }

    /// Symbols produced so far.
    pub fn symbols(&self) -> u64 {
        self.symbols
    }

    /// Forgets the signal (clock, sums, history).
    pub fn reset(&mut self) {
        self.corr.reset();
        if let Some(a) = &mut self.analytic {
            a.reset();
        }
        self.slots.iter_mut().for_each(|v| *v = 0.0);
        self.phase = 0.0;
        self.lf.reset();
        self.symbols = 0;
    }

    #[inline]
    fn sample(&mut self, x: Complex32, out: &mut Vec<f32>) {
        self.corr.push(x);
        // Contrast: the strongest tone's share of the energy, averaged per symbol phase slot.
        let mut tot = 0f32;
        let mut best = 0f32;
        for s in self.corr.sums() {
            let e = s.norm_sqr();
            tot += e;
            best = best.max(e);
        }
        let q = if tot > 0.0 { best / tot } else { 0.0 };
        let n = self.slots.len();
        let slot = (self.phase as usize).min(n - 1);
        self.slots[slot] += self.alpha * (q - self.slots[slot]);
        self.phase += 1.0;
        let period = self.sps + self.lf.integrator();
        if self.phase >= period {
            self.phase -= period;
            if self.corr.ready() {
                self.corr.energies(&mut self.energy).unwrap_or(());
                out.extend_from_slice(&self.energy);
                self.symbols += 1;
            }
            // The strobe is at slot n-1 (the end of the symbol); steer it towards the clearest slot.
            if let Some(k) = peak::argmax(&self.slots) {
                let (m, z, p) = (
                    self.slots[(k + n - 1) % n] as f64,
                    self.slots[k] as f64,
                    self.slots[(k + 1) % n] as f64,
                );
                let best = k as f64 + peak::parabolic(m, z, p).offset;
                // Circular distance from the strobe slot, in samples.
                let mut err = best - (n as f64 - 1.0);
                let half = self.sps / 2.0;
                if err > half {
                    err -= self.sps;
                } else if err < -half {
                    err += self.sps;
                }
                if self.slots[k] > 0.0 {
                    let c = self.lf.update(err);
                    // A positive error (the clearest instant comes later) delays the next strobe.
                    let corr = c - self.lf.integrator();
                    let lim = 0.5 + self.max_frac * self.sps;
                    self.phase -= corr.clamp(-lim, lim);
                }
            }
        }
    }

    /// Demodulates complex samples, appending `M` energies per symbol to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        for &v in x {
            self.sample(v, out);
        }
    }

    /// Demodulates real audio (needs [`with_analytic`](Self::with_analytic)), appending `M` energies per symbol to `out`.
    pub fn process_real(&mut self, x: &[f32], out: &mut Vec<f32>) -> Result<(), ModemError> {
        if self.analytic.is_none() {
            return Param::Analytic.conflict(Param::Input);
        }
        for &v in x {
            let z = match &mut self.analytic {
                Some(a) => a.push(v),
                None => Complex32::default(),
            };
            self.sample(z, out);
        }
        Ok(())
    }
}

/// `ln I0(x)` for `x >= 0` within about 2e-7 relative (M. Abramowitz and I. A. Stegun, *Handbook of Mathematical Functions*,
/// 9.8.1 and 9.8.2).
#[inline]
pub fn ln_i0(x: f32) -> f32 {
    let x = x.abs();
    if x < 3.75 {
        let t = (x / 3.75) * (x / 3.75);
        let i0 = 1.0
            + t * (3.515_623
                + t * (3.089_942_4
                    + t * (1.206_749_2 + t * (0.265_973_2 + t * (0.036_076_8 + t * 0.004_581_3)))));
        i0.ln()
    } else {
        let t = 3.75 / x;
        let p = 0.398_942_3
            + t * (0.013_285_92
                + t * (0.002_253_19
                    + t * (-0.001_575_65
                        + t * (0.009_162_81
                            + t * (-0.020_577_06
                                + t * (0.026_355_37 + t * (-0.016_476_33 + t * 0.003_923_77)))))));
        x + p.ln() - 0.5 * x.ln()
    }
}

/// `ln(e^a + e^b)` without overflow.
#[inline]
fn log_add(a: f32, b: f32) -> f32 {
    if a == f32::NEG_INFINITY {
        return b;
    }
    if b == f32::NEG_INFINITY {
        return a;
    }
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    hi + (lo - hi).exp().ln_1p()
}

/// Soft values of non-coherent M-FSK (module documentation): LLRs of the bits of each symbol from its `M` tone energies.
#[derive(Clone, Debug)]
pub struct FskLlr {
    labels: Vec<u16>,
    bits: usize,
}

impl FskLlr {
    /// LLRs for tone labels `labels[tone]` (a permutation of `0..M`, `M` a power of two from 2 to 4096).
    pub fn new(labels: &[u16]) -> Result<Self, ModemError> {
        let m = labels.len();
        if !(2..=4096).contains(&m) || !m.is_power_of_two() {
            return Param::Tones.not_allowed(m as f64);
        }
        let mut seen = vec![false; m];
        for &l in labels {
            if l as usize >= m || seen[l as usize] {
                return Param::Labels.not_allowed(l as f64);
            }
            seen[l as usize] = true;
        }
        Ok(FskLlr {
            labels: labels.to_vec(),
            bits: m.trailing_zeros() as usize,
        })
    }

    /// Tone `t` carries label `t`.
    pub fn natural(m: usize) -> Result<Self, ModemError> {
        let l: Vec<u16> = (0..m.min(1 << 16) as u32).map(|t| t as u16).collect();
        Self::new(&l)
    }

    /// Tone `t` carries `gray(t)`, so neighbouring tones differ in one bit (FT8, JT9, MFSK).
    pub fn gray(m: usize) -> Result<Self, ModemError> {
        let l: Vec<u16> = (0..m.min(1 << 16) as u32)
            .map(|t| sdrkit_util_bits::gray::gray(t) as u16)
            .collect();
        Self::new(&l)
    }

    /// Bits per symbol.
    pub fn bits(&self) -> usize {
        self.bits
    }

    /// Number of tones.
    pub fn tones(&self) -> usize {
        self.labels.len()
    }

    /// The amplitude `A` and noise variance `N` of a run of symbols, estimated blind: `N` the mean energy of the tones other than
    /// the strongest, `A^2` the mean strongest energy less `N` (at least 5 % of `N`). `None` for an empty input.
    pub fn estimate(energies: &[f32], m: usize) -> Option<(f32, f32)> {
        if m < 2 || energies.len() < m {
            return None;
        }
        let ns = energies.len() / m;
        let (mut pmax, mut pother) = (0f64, 0f64);
        for s in energies.chunks_exact(m) {
            let tot: f32 = s.iter().sum();
            let best = s.iter().copied().fold(0f32, f32::max);
            pmax += best as f64;
            pother += ((tot - best) / (m - 1) as f32) as f64;
        }
        let n = (pother / ns as f64).max(1e-30);
        let a2 = (pmax / ns as f64 - n).max(0.05 * n);
        Some((a2.sqrt() as f32, n as f32))
    }

    /// The LLRs of the symbols in `energies` (`M` per symbol, `|S_t|^2`) into `out` (`bits` per symbol), for tone amplitude `A`
    /// (`|S|` of the sent tone without noise) and noise variance `N` per tone (`E|noise|^2` of `S`).
    pub fn llrs(
        &self,
        energies: &[f32],
        amplitude: f32,
        noise: f32,
        out: &mut [f32],
    ) -> Result<(), ModemError> {
        let m = self.labels.len();
        if !energies.len().is_multiple_of(m) {
            return Err(ModemError::InputTooShort {
                need: energies.len().div_ceil(m) * m,
            });
        }
        let ns = energies.len() / m;
        if out.len() < ns * self.bits {
            return Err(ModemError::OutputTooShort {
                need: ns * self.bits,
            });
        }
        Param::NoiseVariance.check_positive(noise)?;
        Param::Amplitude.check_in(amplitude, 0.0, f64::INFINITY)?;
        let k = 2.0 * amplitude / noise;
        for (s, o) in energies
            .chunks_exact(m)
            .zip(out.chunks_exact_mut(self.bits))
        {
            if s.iter().any(|v| !v.is_finite()) {
                return Err(ModemError::NotFinite {
                    param: Param::Input,
                });
            }
            for (b, ob) in o.iter_mut().enumerate() {
                let shift = self.bits - 1 - b;
                let (mut l0, mut l1) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
                for (t, &e) in s.iter().enumerate() {
                    let v = ln_i0(k * e.max(0.0).sqrt());
                    if (self.labels[t] >> shift) & 1 == 0 {
                        l0 = log_add(l0, v);
                    } else {
                        l1 = log_add(l1, v);
                    }
                }
                *ob = l0 - l1;
            }
        }
        Ok(())
    }

    /// The extrinsic LLRs of the bits of each symbol from per-tone log-likelihoods and a priori bit LLRs: the iterative
    /// demapper of a coded M-ary signal. `metrics` holds `M` values per symbol (`ln p(y | tone)` up to a constant, for
    /// example `ln I0(2 A sqrt(E_t) / N)` summed over the copies of a symbol), `prior` `bits` LLRs per symbol (positive means
    /// 0) or nothing. Bit `b` of a symbol gets `ln sum_{t: b = 0} exp(m_t + a_t) - ln sum_{t: b = 1} exp(m_t + a_t)`, where
    /// `a_t` is half the sum of the priors of the other bits, each signed by the bit of tone `t`'s label.
    pub fn soft_bits(
        &self,
        metrics: &[f32],
        prior: &[f32],
        out: &mut [f32],
    ) -> Result<(), ModemError> {
        let m = self.labels.len();
        if !metrics.len().is_multiple_of(m) {
            return Err(ModemError::InputTooShort {
                need: metrics.len().div_ceil(m) * m,
            });
        }
        let ns = metrics.len() / m;
        if out.len() < ns * self.bits {
            return Err(ModemError::OutputTooShort {
                need: ns * self.bits,
            });
        }
        if !prior.is_empty() && prior.len() < ns * self.bits {
            return Err(ModemError::InputTooShort {
                need: ns * self.bits,
            });
        }
        let mut full = vec![0f32; m];
        for (s, (mt, o)) in metrics
            .chunks_exact(m)
            .zip(out.chunks_exact_mut(self.bits))
            .enumerate()
        {
            if mt.iter().any(|v| v.is_nan()) {
                return Err(ModemError::NotFinite {
                    param: Param::Input,
                });
            }
            let pr = if prior.is_empty() {
                &[][..]
            } else {
                &prior[s * self.bits..(s + 1) * self.bits]
            };
            let sign = |label: u16, b: usize| {
                if (label >> (self.bits - 1 - b)) & 1 == 0 {
                    0.5
                } else {
                    -0.5
                }
            };
            for (t, f) in full.iter_mut().enumerate() {
                *f = mt[t]
                    + pr.iter()
                        .enumerate()
                        .map(|(b, &l)| l * sign(self.labels[t], b))
                        .sum::<f32>();
            }
            for (b, ob) in o.iter_mut().enumerate() {
                let own = pr.get(b).copied().unwrap_or(0.0);
                let (mut l0, mut l1) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
                for (t, &f) in full.iter().enumerate() {
                    let sg = sign(self.labels[t], b);
                    let v = f - own * sg;
                    if sg > 0.0 {
                        l0 = log_add(l0, v);
                    } else {
                        l1 = log_add(l1, v);
                    }
                }
                *ob = l0 - l1;
            }
        }
        Ok(())
    }

    /// The hard label of each symbol (the strongest tone's label) into `out`.
    pub fn decide(&self, energies: &[f32], out: &mut [u16]) -> Result<(), ModemError> {
        let m = self.labels.len();
        let ns = energies.len() / m;
        if out.len() < ns {
            return Err(ModemError::OutputTooShort { need: ns });
        }
        for (s, o) in energies.chunks_exact(m).zip(out.iter_mut()) {
            *o = self.labels[peak::argmax(s).unwrap_or(0)];
        }
        Ok(())
    }
}

/// A slicer for multi-level FSK after a discriminator (2-level POCSAG and GFSK, 4-level C4FM, DMR, NXDN, dPMR, M17): decides each
/// symbol-rate level against levels `c + s a`, `a = -(L-1), ..., -1, +1, ..., L-1`, and returns LLRs of the label bits.
///
/// The centre `c` (the DC a frequency offset leaves) and the spacing `s` (the deviation per unit level) are fitted by least squares
/// to the decided levels, with exponential forgetting, so they follow drift (decision-directed); [`fit`](Self::fit) sets them from
/// a known training sequence (the sync word). The noise is the mean squared residual. LLRs come from the shared
/// [`Constellation`], so they are true LLRs for Gaussian noise on the level.
#[derive(Clone, Debug)]
pub struct LevelSlicer {
    c: Constellation,
    levels: usize,
    /// Label of each level from the most negative.
    labels: Vec<u16>,
    rms: f32,
    centre: f32,
    spacing: f32,
    noise: f32,
    alpha: f32,
    sums: [f32; 5],
    track: bool,
}

impl LevelSlicer {
    /// Levels labelled `labels[i]` from the most negative (2 to 256 levels, a power of two), forgetting factor `alpha` in `(0, 1)`
    /// for the fit (about 1 / the symbols to average), and a first guess of the spacing `s` (deviation per unit level, non-zero).
    pub fn new(labels: &[u16], alpha: f32, spacing: f32) -> Result<Self, ModemError> {
        let l = labels.len();
        if !(2..=256).contains(&l) || !l.is_power_of_two() {
            return Param::Levels.not_allowed(l as f64);
        }
        Param::Smoothing.check_open(alpha, 0.0, 1.0)?;
        if !(spacing.is_finite() && spacing != 0.0) {
            return Param::Spacing.not_allowed(spacing);
        }
        let mut pts = vec![Complex32::default(); l];
        let mut seen = vec![false; l];
        for (i, &lab) in labels.iter().enumerate() {
            if lab as usize >= l || seen[lab as usize] {
                return Param::Labels.not_allowed(lab as f64);
            }
            seen[lab as usize] = true;
            pts[lab as usize] = Complex32::new((2 * i) as f32 - (l - 1) as f32, 0.0);
        }
        let set = PointSet::new(pts, sdrkit_algo_dsp::constellation::Shape::Pam)?;
        Ok(LevelSlicer {
            c: Constellation::new(set)?,
            levels: l,
            labels: labels.to_vec(),
            rms: (((l * l - 1) as f32) / 3.0).sqrt(),
            centre: 0.0,
            spacing,
            noise: (0.25 * spacing * spacing).max(f32::MIN_POSITIVE),
            alpha,
            sums: [0.0; 5],
            track: true,
        })
    }

    /// Two levels: `-1` is label 1, `+1` label 0 (positive means 0, as for BPSK).
    pub fn binary(alpha: f32, spacing: f32) -> Result<Self, ModemError> {
        Self::new(&[1, 0], alpha, spacing)
    }

    /// The C4FM / 4FSK dibit map of P25 (TIA-102.BAAA-A, table 3: `01` +3, `00` +1, `10` -1, `11` -3) and DMR (ETSI TS 102 361-1,
    /// table 10.3), also used by NXDN, dPMR and M17.
    pub fn c4fm(alpha: f32, spacing: f32) -> Result<Self, ModemError> {
        Self::new(&[0b11, 0b10, 0b00, 0b01], alpha, spacing)
    }

    /// Number of levels.
    pub fn levels(&self) -> usize {
        self.levels
    }

    /// Bits per symbol.
    pub fn bits(&self) -> usize {
        self.c.bits()
    }

    /// The fitted centre.
    pub fn centre(&self) -> f32 {
        self.centre
    }

    /// The fitted spacing per unit level.
    pub fn spacing(&self) -> f32 {
        self.spacing
    }

    /// The noise variance on the level (same units as the input).
    pub fn noise_var(&self) -> f32 {
        self.noise
    }

    /// Freezes (false) or resumes (true) the decision-directed fit, for example outside a burst.
    pub fn set_tracking(&mut self, on: bool) {
        self.track = on;
    }

    /// Sets the centre and spacing (and forgets the running fit).
    pub fn set_levels(&mut self, centre: f32, spacing: f32) -> Result<(), ModemError> {
        Param::Centre.check_finite(centre)?;
        if !(spacing.is_finite() && spacing != 0.0) {
            return Param::Spacing.not_allowed(spacing);
        }
        self.centre = centre;
        self.spacing = spacing;
        self.sums = [0.0; 5];
        Ok(())
    }

    /// Least-squares centre, spacing and noise from received levels `x` of known level indices `idx` (0 = most negative), for
    /// example a sync word. Errors when the lengths differ, an index is out of range, or fewer than two distinct levels appear.
    pub fn fit(&mut self, x: &[f32], idx: &[u8]) -> Result<(), ModemError> {
        if x.len() != idx.len() {
            return Param::Labels.length(x.len(), idx.len());
        }
        if let Some(&i) = idx.iter().find(|&&i| i as usize >= self.levels) {
            return Param::Labels.range(0.0, (self.levels - 1) as f64, i as f64);
        }
        let mut s = [0f64; 5];
        for (&v, &i) in x.iter().zip(idx) {
            let a = (2 * i as i32 - (self.levels as i32 - 1)) as f64;
            s[0] += 1.0;
            s[1] += a;
            s[2] += a * a;
            s[3] += v as f64;
            s[4] += a * v as f64;
        }
        let det = s[0] * s[2] - s[1] * s[1];
        if det.is_nan() || det <= 1e-9 {
            return Param::Levels.degenerate();
        }
        let sp = (s[0] * s[4] - s[1] * s[3]) / det;
        let c = (s[3] - sp * s[1]) / s[0];
        if !(sp.is_finite() && c.is_finite()) || sp == 0.0 {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let mut r = 0.0;
        for (&v, &i) in x.iter().zip(idx) {
            let a = (2 * i as i32 - (self.levels as i32 - 1)) as f64;
            r += (v as f64 - c - sp * a).powi(2);
        }
        self.centre = c as f32;
        self.spacing = sp as f32;
        if s[0] > 2.0 {
            self.noise = ((r / (s[0] - 2.0)) as f32).max(1e-12 * (sp * sp) as f32);
        }
        self.sums = s.map(|v| v as f32);
        Ok(())
    }

    /// Decides one level `x`: returns its label and writes the LLRs of its bits (most significant first) to `llr[0..bits]`.
    pub fn slice(&mut self, x: f32, llr: &mut [f32]) -> Result<u16, ModemError> {
        if !x.is_finite() {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        let z = (x - self.centre) / self.spacing;
        let zu = Complex32::new(z / self.rms, 0.0);
        // Real noise of variance n on the level is n / s^2 in units of a; the constellation expects twice that (complex variance).
        let var =
            (2.0 * self.noise / (self.spacing * self.spacing) / (self.rms * self.rms)).max(1e-12);
        self.c.demap(zu, var, Demap::Exact, llr)?;
        let label = self.c.decide(zu);
        if self.track {
            let i = self.labels.iter().position(|&l| l == label).unwrap_or(0);
            let a = (2 * i) as f32 - (self.levels - 1) as f32;
            let w = self.alpha;
            let s = &mut self.sums;
            for (k, v) in [1.0, a, a * a, x, a * x].into_iter().enumerate() {
                s[k] += w * (v - s[k]);
            }
            let det = s[0] * s[2] - s[1] * s[1];
            if det > 1e-6 * s[0] * s[2] {
                let sp = (s[0] * s[4] - s[1] * s[3]) / det;
                if sp.is_finite() && sp * self.spacing > 0.0 {
                    self.spacing = sp;
                    self.centre = (s[3] - sp * s[1]) / s[0];
                }
            } else {
                // One level seen so far: move only the centre.
                self.centre += w * (x - (self.centre + self.spacing * a));
            }
            let e = x - (self.centre + self.spacing * a);
            self.noise += w * (e * e - self.noise);
            self.noise = self.noise.max(1e-12 * self.spacing * self.spacing);
        }
        Ok(label)
    }

    /// Slices every level in `x`, appending `bits` LLRs per level to `out`.
    pub fn process(&mut self, x: &[f32], out: &mut Vec<f32>) -> Result<(), ModemError> {
        let b = self.bits();
        let mut l = [0f32; 8];
        for &v in x {
            self.slice(v, &mut l[..b])?;
            out.extend_from_slice(&l[..b]);
        }
        Ok(())
    }
}

/// [`FskLlr`] with the tone amplitude and the noise estimated blind ([`FskLlr::estimate`]) from every symbol and smoothed over
/// about `memory` symbols, for a streaming receiver that has no training: energies in, LLRs out, one symbol at a time.
///
/// ```
/// use sdrkit_algo_modem::fsk::{FskLlr, TrackedFskLlr};
/// let mut t = TrackedFskLlr::new(FskLlr::natural(2).unwrap(), 16.0).unwrap();
/// let mut l = [0f32];
/// for _ in 0..32 {
///     t.symbol(&[100.0, 1.0], &mut l).unwrap(); // tone 0 strong: bit 0
/// }
/// assert!(l[0] > 0.0);
/// assert!(t.snr() > 10.0);
/// ```
#[derive(Clone, Debug)]
pub struct TrackedFskLlr {
    llr: FskLlr,
    weight: f32,
    a2: f32,
    noise: f32,
    started: bool,
}

impl TrackedFskLlr {
    /// LLRs by `llr`, the estimates smoothed by a one-pole average over `memory` symbols (at least 1).
    pub fn new(llr: FskLlr, memory: f32) -> Result<Self, ModemError> {
        if !(memory >= 1.0 && memory.is_finite()) {
            return Param::Memory.at_least(1.0, memory);
        }
        Ok(TrackedFskLlr {
            llr,
            weight: 1.0 / memory,
            a2: 0.0,
            noise: 0.0,
            started: false,
        })
    }

    /// The LLR maker inside.
    pub fn llr(&self) -> &FskLlr {
        &self.llr
    }

    /// The smoothed tone amplitude `A`.
    pub fn amplitude(&self) -> f32 {
        self.a2.sqrt()
    }

    /// The smoothed noise variance per tone.
    pub fn noise(&self) -> f32 {
        self.noise
    }

    /// The smoothed `A^2 / N` (0 before the first symbol).
    pub fn snr(&self) -> f32 {
        if self.started {
            self.a2 / self.noise.max(f32::MIN_POSITIVE)
        } else {
            0.0
        }
    }

    /// Forgets the estimates.
    pub fn reset(&mut self) {
        (self.a2, self.noise, self.started) = (0.0, 0.0, false);
    }

    /// Takes the `M` energies of one symbol, updates the estimates and writes the symbol's `bits` LLRs into `out`. Energies that
    /// are not finite leave the estimates alone and erase the symbol (LLRs 0, `NotFinite`).
    pub fn symbol(&mut self, energies: &[f32], out: &mut [f32]) -> Result<(), ModemError> {
        let (m, b) = (self.llr.tones(), self.llr.bits());
        if energies.len() != m {
            return Err(ModemError::InputTooShort { need: m });
        }
        if out.len() < b {
            return Err(ModemError::OutputTooShort { need: b });
        }
        if energies.iter().any(|e| !e.is_finite()) {
            out[..b].fill(0.0);
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        if let Some((amp, n)) = FskLlr::estimate(energies, m) {
            if self.started {
                self.a2 += (amp * amp - self.a2) * self.weight;
                self.noise += (n - self.noise) * self.weight;
            } else {
                (self.a2, self.noise, self.started) = (amp * amp, n, true);
            }
        }
        let n = self.noise.max(f32::MIN_POSITIVE);
        let r = self.llr.llrs(energies, self.a2.sqrt(), n, &mut out[..b]);
        if r.is_err() {
            out[..b].fill(0.0);
        }
        r
    }
}

/// The bit labels of the tones of an M-FSK signal (in the order the tones are listed) or of the levels of a multi-level FSK
/// (from the most negative), chosen by name for settings: what [`FskLlr::new`] and [`LevelSlicer::new`] take.
///
/// ```
/// use sdrkit_algo_modem::fsk::LabelMap;
/// assert_eq!(LabelMap::by_name("c4fm").unwrap().labels(4).unwrap(), [0b11, 0b10, 0b00, 0b01]);
/// assert_eq!(LabelMap::Gray.labels(4).unwrap(), [0, 1, 3, 2]);
/// assert!(LabelMap::Binary.labels(4).is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelMap {
    /// Tone or level `t` carries `t`.
    Natural,
    /// Tone or level `t` carries `gray(t)`, so neighbours differ in one bit (FT8, JT9, MFSK).
    Gray,
    /// Two tones or levels: the first (the lower) is bit 1, as [`LevelSlicer::binary`] (positive means 0, as for BPSK).
    Binary,
    /// Four levels: the C4FM / 4FSK dibits of [`LevelSlicer::c4fm`] (TIA-102.BAAA-A table 3: -3 `11`, -1 `10`, +1 `00`, +3 `01`).
    C4fm,
}

impl LabelMap {
    /// The names [`by_name`](Self::by_name) accepts, in the order of the variants.
    pub const NAMES: [&'static str; 4] = ["natural", "gray", "binary", "c4fm"];

    /// The map called `name` (one of [`NAMES`](Self::NAMES)).
    pub fn by_name(name: &str) -> Result<Self, ModemError> {
        match name {
            "natural" => Ok(LabelMap::Natural),
            "gray" => Ok(LabelMap::Gray),
            "binary" => Ok(LabelMap::Binary),
            "c4fm" => Ok(LabelMap::C4fm),
            _ => Param::LabelMap.unknown_name(),
        }
    }

    /// The name of the map.
    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    /// The labels of `m` tones or levels (`m` a power of two from 2 to 4096; `Binary` needs 2 and `C4fm` 4).
    pub fn labels(self, m: usize) -> Result<Vec<u16>, ModemError> {
        if !(2..=4096).contains(&m) || !m.is_power_of_two() {
            return Param::Tones.not_allowed(m as f64);
        }
        match self {
            LabelMap::Natural => Ok((0..m as u16).collect()),
            LabelMap::Gray => Ok((0..m as u32)
                .map(|t| sdrkit_util_bits::gray::gray(t) as u16)
                .collect()),
            LabelMap::Binary if m == 2 => Ok(vec![1, 0]),
            LabelMap::Binary => Param::LabelMap.conflict(Param::Levels),
            LabelMap::C4fm if m == 4 => Ok(vec![0b11, 0b10, 0b00, 0b01]),
            LabelMap::C4fm => Param::LabelMap.conflict(Param::Levels),
        }
    }
}

/// Settings of [`DiscriminatorLevels`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiscriminatorConfig {
    /// Input samples per symbol, 2 to 4096.
    pub sps: f64,
    /// Peak deviation in cycles per sample, in `(0, 0.5)`, for complex input; `None` for real input that is a frequency already
    /// (the output of an FM demodulator).
    pub deviation: Option<f64>,
    /// Gaussian bandwidth-time product of the transmitter (`None` for a plain or Nyquist-shaped FSK): sets the pulse the timing
    /// detector expects.
    pub bt: Option<f64>,
    /// Timing loop noise bandwidth per symbol.
    pub bn_t: f64,
    /// Clock range of the timing loop, ppm.
    pub max_ppm: f64,
}

impl DiscriminatorConfig {
    /// Complex input with peak `deviation` (cycles per sample); timing bandwidth 0.01, +-500 ppm.
    pub fn new(sps: f64, deviation: Option<f64>) -> Self {
        DiscriminatorConfig {
            sps,
            deviation,
            bt: None,
            bn_t: 0.01,
            max_ppm: 500.0,
        }
    }
}

/// Samples [`DiscriminatorLevels`] works on at a time (the size of its scratch buffer).
const LEVELS_PIECE: usize = 4096;

/// The front half of every discriminator FSK receiver: the FM discriminator (`sdrkit_algo_dsp::fm::Discriminator`, for complex
/// input), a one-symbol integrate-and-dump (a running mean, re-summed exactly every 1024 symbols so it never drifts) and the
/// zero-crossing symbol synchroniser ([`crate::timing::SymbolSync`]), giving one level per symbol at unit mean power.
///
/// The levels then go to a [`LevelSlicer`] (2- and 4-level FSK, GFSK) or to an MLSE over the pulse's intersymbol interference
/// ([`crate::gmsk::MlseStream`]). State is kept between calls, so the levels do not depend on how the input is split.
///
/// ```
/// use sdrkit_algo_modem::fsk::{DiscriminatorConfig, DiscriminatorLevels};
/// use sdrkit_algo_modem::Complex32;
/// // Binary FSK, deviation 0.05 cycles per sample, 10 samples per symbol, alternating symbols.
/// let mut d = DiscriminatorLevels::new(DiscriminatorConfig::new(10.0, Some(0.05))).unwrap();
/// let (mut x, mut ph) = (Vec::new(), 0f64);
/// for k in 0..400 {
///     let f = if k % 2 == 0 { 0.05 } else { -0.05 };
///     for _ in 0..10 {
///         ph += f;
///         x.push(Complex32::from_polar(1.0, (std::f64::consts::TAU * ph) as f32));
///     }
/// }
/// let mut levels = Vec::new();
/// d.process_iq(&x, &mut levels);
/// assert!((390..=410).contains(&levels.len()));
/// // Settled: the levels alternate in sign.
/// assert!(levels[300..].windows(2).all(|w| w[0] * w[1] < 0.0));
/// ```
#[derive(Clone, Debug)]
pub struct DiscriminatorLevels {
    disc: Option<Discriminator>,
    boxcar: Vec<f32>,
    sum: f32,
    at: usize,
    since_resum: usize,
    sync: SymbolSync<f32, ZeroCrossing, Cubic>,
    freq: Vec<f32>,
}

impl DiscriminatorLevels {
    /// Levels with these settings. Errors for `sps` outside 2 to 4096, a deviation outside `(0, 0.5)`, or a bad loop setting.
    pub fn new(cfg: DiscriminatorConfig) -> Result<Self, ModemError> {
        if !(2.0..=4096.0).contains(&cfg.sps) {
            return Param::Sps.range(2.0, 4096.0, cfg.sps);
        }
        let disc = match cfg.deviation {
            Some(d) if d > 0.0 && d < 0.5 => Some(Discriminator::new(1.0, d)?),
            Some(d) => return Param::Deviation.range(0.0, 0.5, d),
            None => None,
        };
        let mut sc = SymbolSyncConfig::new(cfg.sps);
        sc.bn_t = cfg.bn_t;
        sc.max_ppm = cfg.max_ppm;
        // The overall pulse at the detector: the frequency pulse through the one-symbol integrator, close to a raised cosine.
        sc.pulse = match cfg.bt {
            Some(bt) if bt < 0.6 => Shape::Rc { beta: 1.0 },
            _ => Shape::Rc { beta: 0.8 },
        };
        Ok(DiscriminatorLevels {
            disc,
            boxcar: vec![0.0; cfg.sps.round().max(1.0) as usize],
            sum: 0.0,
            at: 0,
            since_resum: 0,
            sync: SymbolSync::new(ZeroCrossing, Cubic, sc)?,
            freq: Vec::with_capacity(LEVELS_PIECE),
        })
    }

    /// The timing loop.
    pub fn timing(&self) -> &SymbolSync<f32, ZeroCrossing, Cubic> {
        &self.sync
    }

    /// The clock error the timing loop follows, ppm.
    pub fn clock_ppm(&self) -> f64 {
        self.sync.clock_ppm()
    }

    /// Forgets the signal.
    pub fn reset(&mut self) {
        if let Some(d) = &mut self.disc {
            d.reset();
        }
        self.boxcar.fill(0.0);
        self.sum = 0.0;
        self.at = 0;
        self.since_resum = 0;
        self.sync.reset();
    }

    /// The running one-symbol mean over `self.freq` (a sample that is not a number counts as 0), then the synchroniser.
    fn finish(&mut self, out: &mut Vec<f32>) {
        let n = self.boxcar.len();
        for f in self.freq.iter_mut() {
            let v = if f.is_finite() { *f } else { 0.0 };
            self.sum += v - self.boxcar[self.at];
            self.boxcar[self.at] = v;
            self.at = if self.at + 1 == n { 0 } else { self.at + 1 };
            self.since_resum += 1;
            if self.since_resum >= 1024 * n {
                self.since_resum = 0;
                self.sum = self.boxcar.iter().sum();
            }
            *f = self.sum / n as f32;
        }
        self.sync.process(&self.freq, out);
    }

    /// Complex input: appends one level per symbol to `out`. Without a deviation (built for real input) the samples count as 0.
    pub fn process_iq(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        for piece in x.chunks(LEVELS_PIECE) {
            self.freq.clear();
            self.freq.resize(piece.len(), 0.0);
            if let Some(d) = &mut self.disc {
                d.process_into(piece, &mut self.freq);
            }
            self.finish(out);
        }
    }

    /// Real input, a frequency already: appends one level per symbol to `out`.
    pub fn process_real(&mut self, x: &[f32], out: &mut Vec<f32>) {
        for piece in x.chunks(LEVELS_PIECE) {
            self.freq.clear();
            self.freq.extend_from_slice(piece);
            self.finish(out);
        }
    }
}

/// Samples between exact re-anchors of the rotators of a [`TonePair`].
const PAIR_REANCHOR: u32 = 256;

/// A gain under this share of the mean of the two is not believed by [`tone_gains`].
pub const TONE_GAIN_FLOOR: f32 = 0.3;

/// The tone-energy decision of 2-FSK: the symbol from the complex samples instead of the mean of a discriminator, whose clicks at
/// low SNR (whole turns of phase) count in full while the energies hardly move.
///
/// For a centre `c` (the carrier offset, which the caller updates) and a deviation `dev`, over a window of `W` samples:
/// `Eu = |sum z[n] e^(-j 2 pi (c + dev) n / rate)|^2` and `El` the same at `c - dev`. Hard decision: the upper tone when
/// `Eu > El`; soft value `(Eu / gu - El / gl) / (Eu / gu + El / gl)` in -1..1 with per-tone gains `gu`, `gl` (1 unless set, see
/// [`tone_gains`]); a stack that needs a frequency takes `c + dev x value`.
///
/// The two mixed streams are kept as prefix sums (in `f64`) over a history the caller sizes, so any window that ends in it, of
/// any length up to it (a symbol, a chip, half a chip centred on an instant), is two subtractions per tone. The rotators run from
/// an `f64` phase and are re-anchored exactly every 256 samples. A new centre or deviation applies from the next sample; a window
/// that spans the change sums both. Nothing is allocated after [`TonePair::new`].
///
/// ```
/// use sdrkit_algo_modem::fsk::TonePair;
/// use sdrkit_algo_modem::Complex32;
/// // A tone at 1000 + 2400 Hz: the upper tone of a pair centred on 1000 Hz.
/// let mut p = TonePair::new(48_000.0, 1000.0, 2400.0, 64).unwrap();
/// for n in 0..20 {
///     p.push(Complex32::from_polar(1.0, (std::f64::consts::TAU * 3400.0 * n as f64 / 48_000.0) as f32));
/// }
/// assert!(p.value(10, 0).unwrap() > 0.99);
/// ```
#[derive(Clone, Debug)]
pub struct TonePair {
    rate: f64,
    centre: f64,
    dev: f64,
    gains: [f32; 2],
    /// Upper, lower: the phase (cycles) of the next sample, the rotator, the step.
    phase: [f64; 2],
    rot: [Complex32; 2],
    step: [Complex32; 2],
    since: u32,
    acc: [Complex64; 2],
    prefix: Vec<[Complex64; 2]>,
    pos: usize,
    count: u64,
}

impl TonePair {
    /// A pair at `centre_hz +- dev_hz` for samples at `rate`, keeping `history` samples (1 to 2^20) for its windows. Errors for a
    /// rate that is not positive, a deviation that is not positive, or another history.
    pub fn new(rate: f64, centre_hz: f64, dev_hz: f64, history: usize) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        Param::Deviation.check_positive(dev_hz)?;
        Param::Frequency.check_finite(centre_hz)?;
        Param::Capacity.check_count(history, 1, 1 << 20)?;
        let mut p = TonePair {
            rate,
            centre: centre_hz,
            dev: dev_hz,
            gains: [1.0; 2],
            phase: [0.0; 2],
            rot: [Complex32::new(1.0, 0.0); 2],
            step: [Complex32::new(1.0, 0.0); 2],
            since: 0,
            acc: [Complex64::default(); 2],
            prefix: vec![[Complex64::default(); 2]; history + 1],
            pos: 0,
            count: 0,
        };
        p.set_steps();
        Ok(p)
    }

    /// The tone frequencies in cycles per sample: upper, lower.
    fn freqs(&self) -> [f64; 2] {
        [
            (self.centre + self.dev) / self.rate,
            (self.centre - self.dev) / self.rate,
        ]
    }

    fn set_steps(&mut self) {
        self.step = self.freqs().map(phasor_down);
    }

    /// The centre (Hz).
    pub fn centre(&self) -> f64 {
        self.centre
    }

    /// The deviation (Hz).
    pub fn deviation(&self) -> f64 {
        self.dev
    }

    /// Moves the centre (from the next sample).
    pub fn set_centre(&mut self, centre_hz: f64) {
        if centre_hz.is_finite() {
            self.centre = centre_hz;
            self.set_steps();
        }
    }

    /// Changes the deviation (from the next sample); a value that is not positive is ignored.
    pub fn set_deviation(&mut self, dev_hz: f64) {
        if dev_hz > 0.0 && dev_hz.is_finite() {
            self.dev = dev_hz;
            self.set_steps();
        }
    }

    /// Sets the per-tone gains (upper, lower); a gain that is not positive counts as 1.
    pub fn set_gains(&mut self, upper: f32, lower: f32) {
        let ok = |g: f32| if g > 0.0 && g.is_finite() { g } else { 1.0 };
        self.gains = [ok(upper), ok(lower)];
    }

    /// The per-tone gains (upper, lower).
    pub fn gains(&self) -> [f32; 2] {
        self.gains
    }

    /// The longest window (the history).
    pub fn history(&self) -> usize {
        self.prefix.len() - 1
    }

    /// Clears the history and restarts the rotators at phase 0; keeps the centre, deviation and gains.
    pub fn reset(&mut self) {
        self.phase = [0.0; 2];
        self.rot = [Complex32::new(1.0, 0.0); 2];
        self.since = 0;
        self.acc = [Complex64::default(); 2];
        self.prefix.fill([Complex64::default(); 2]);
        self.pos = 0;
        self.count = 0;
    }

    /// Takes one sample.
    #[inline]
    pub fn push(&mut self, z: Complex32) {
        let f = self.freqs();
        for (t, ft) in f.into_iter().enumerate() {
            let y = z * self.rot[t];
            self.acc[t] += Complex64::new(y.re as f64, y.im as f64);
            self.rot[t] *= self.step[t];
            self.phase[t] = (self.phase[t] + ft).rem_euclid(1.0);
        }
        self.since += 1;
        if self.since == PAIR_REANCHOR {
            self.since = 0;
            for t in 0..2 {
                self.rot[t] = phasor_down(self.phase[t]);
            }
        }
        self.pos += 1;
        if self.pos == self.prefix.len() {
            self.pos = 0;
        }
        self.prefix[self.pos] = self.acc;
        self.count += 1;
    }

    /// Takes samples.
    pub fn process(&mut self, x: &[Complex32]) {
        for &z in x {
            self.push(z);
        }
    }

    /// The prefix sums after the sample `back` samples before the newest.
    fn prefix_back(&self, back: usize) -> [Complex64; 2] {
        let n = self.prefix.len();
        self.prefix[(self.pos + n - back % n) % n]
    }

    /// `[Eu, El]` over the `len` samples that end `back` samples before the newest (`back = 0`: ending with the newest). `None`
    /// when `len` is 0 or the window reaches beyond the history or before the first sample.
    pub fn energies(&self, len: usize, back: usize) -> Option<[f32; 2]> {
        if len == 0 || len + back > self.history() || (len + back) as u64 > self.count {
            return None;
        }
        let (a, b) = (self.prefix_back(back), self.prefix_back(back + len));
        Some([
            (a[0] - b[0]).norm_sqr() as f32,
            (a[1] - b[1]).norm_sqr() as f32,
        ])
    }

    /// The soft value `(Eu / gu - El / gl) / (Eu / gu + El / gl)` of the window (as [`energies`](Self::energies)); 0 when both
    /// energies are 0.
    pub fn value(&self, len: usize, back: usize) -> Option<f32> {
        let [eu, el] = self.energies(len, back)?;
        let (u, l) = (eu / self.gains[0], el / self.gains[1]);
        Some(if u + l > 0.0 { (u - l) / (u + l) } else { 0.0 })
    }

    /// The hard decision of the window: `true` for the upper tone.
    pub fn upper(&self, len: usize, back: usize) -> Option<bool> {
        self.value(len, back).map(|v| v > 0.0)
    }
}

/// Per-tone gains from the mean energy of each tone on the known symbols that sent it (a sync word): `[mean_upper, mean_lower]`,
/// except that a gain under [`TONE_GAIN_FLOOR`] of the mean of the two is not believed and the mean is used for it. With a carrier
/// offset the channel filter's slope makes the two tones' energies differ; dividing by these gains evens them.
pub fn tone_gains(mean_upper: f32, mean_lower: f32) -> [f32; 2] {
    let m = 0.5 * (mean_upper + mean_lower);
    let ok = |g: f32| {
        if g.is_finite() && g >= TONE_GAIN_FLOOR * m && g > 0.0 {
            g
        } else {
            m
        }
    };
    [ok(mean_upper), ok(mean_lower)]
}

/// The slicer threshold of a discriminator from smoothed extremes, for line codes that do not let the discriminator mean settle
/// (short preambles, unbalanced start-stop framing).
///
/// `v` is the moving average of the discriminator over `L = round(0.7 rate / baud)` samples (the average the slicer decides
/// on). Per sample: if `v > max` then `max = v`, else `max -= (max - v) k`; if `v < min` then `min = v`, else
/// `min += (v - min) k`, with `k = baud / rate / 12` (about 12 symbols); the threshold is `(max + min) / 2`. Taken on `v` and not
/// on the raw discriminator, the extremes are not noise spikes at low SNR. Nothing is allocated after [`ExtremeThreshold::new`].
///
/// ```
/// use sdrkit_algo_modem::fsk::ExtremeThreshold;
/// let mut t = ExtremeThreshold::new(48_000.0, 4800.0).unwrap();
/// let mut last = (0.0, 0.0);
/// for n in 0..4710 {
///     // Mostly marks at +3 kHz, a space at -1 kHz every tenth symbol, on a 1 kHz offset; the last sample ends a space.
///     let d = if (n / 10) % 10 == 0 { -1000.0 } else { 3000.0 };
///     last = t.push(d);
/// }
/// assert!((last.1 - 1000.0).abs() < 300.0); // the middle, not the mean (2600 Hz)
/// ```
#[derive(Clone, Debug)]
pub struct ExtremeThreshold {
    avg: WindowSum<f32>,
    k: f32,
    max: f32,
    min: f32,
    started: bool,
}

impl ExtremeThreshold {
    /// For a discriminator at `rate` and symbols at `baud`. Errors unless both are positive and `baud <= rate`.
    pub fn new(rate: f64, baud: f64) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        Param::Baud.check_in(baud, f64::MIN_POSITIVE, rate)?;
        let len = ((0.7 * rate / baud).round() as usize).max(1);
        Self::with_params(len, (baud / rate / 12.0) as f32)
    }

    /// An average over `len` samples (at least 1) and a decay `k` per sample in `(0, 1]`.
    pub fn with_params(len: usize, k: f32) -> Result<Self, ModemError> {
        Param::Window.check_count(len, 1, 1 << 20)?;
        Param::Smoothing.check_in(k, f64::MIN_POSITIVE, 1.0)?;
        Ok(ExtremeThreshold {
            avg: WindowSum::new(len).map_err(|_| ModemError::Degenerate {
                param: Param::Window,
            })?,
            k,
            max: 0.0,
            min: 0.0,
            started: false,
        })
    }

    /// The length `L` of the moving average.
    pub fn len(&self) -> usize {
        self.avg.len()
    }

    /// Always false: the average has at least one sample.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The current threshold.
    pub fn threshold(&self) -> f32 {
        0.5 * (self.max + self.min)
    }

    /// The decaying maximum and minimum.
    pub fn extremes(&self) -> (f32, f32) {
        (self.max, self.min)
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.avg.reset();
        self.started = false;
        self.max = 0.0;
        self.min = 0.0;
    }

    /// Takes one discriminator value; returns the moving average `v` and the threshold.
    #[inline]
    pub fn push(&mut self, d: f32) -> (f32, f32) {
        let n = (self.avg.count() + 1).min(self.avg.len() as u64) as f64;
        let v = (self.avg.push(d) / n) as f32;
        if !self.started {
            self.started = true;
            self.max = v;
            self.min = v;
        } else {
            if v > self.max {
                self.max = v;
            } else {
                self.max -= (self.max - v) * self.k;
            }
            if v < self.min {
                self.min = v;
            } else {
                self.min += (v - self.min) * self.k;
            }
        }
        (v, self.threshold())
    }
}

pub(crate) fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::bench::measure;
    use sdrkit_core::{BenchResult, Benchmark};
    fn afsk(seconds: f64) -> BenchResult {
        // Bell 202 AFSK 1200 at 48 kHz on real audio.
        let n = 48_000;
        let x: Vec<f32> = (0..n).map(|i| ((i as f32) * 0.157).sin()).collect();
        let cfg = MfskConfig::from_hz(&[1200.0, 2200.0], 1200.0, 48_000.0);
        let mut d = MfskDemod::new(cfg)
            .expect("valid")
            .with_analytic(Analytic::for_band(48_000.0, 600.0).expect("valid"));
        let mut out = Vec::with_capacity(4096);
        measure(seconds, n, n as f64 / 48_000.0, || {
            out.clear();
            d.process_real(&x, &mut out).unwrap_or(());
        })
    }
    fn mfsk8(seconds: f64) -> BenchResult {
        // 8 tones at 12 kHz, 100 baud (an MFSK or ALE-like rate).
        let n = 12_000;
        let x: Vec<Complex32> = (0..n)
            .map(|i| Complex32::from_polar(1.0, i as f32 * 0.1))
            .collect();
        let tones: Vec<f64> = (0..8)
            .map(|k| (k as f64 - 3.5) * 100.0 / 12_000.0)
            .collect();
        let mut d = MfskDemod::new(MfskConfig::new(&tones, 120.0)).expect("valid");
        let mut out = Vec::with_capacity(4096);
        measure(seconds, n, n as f64 / 12_000.0, || {
            out.clear();
            d.process(&x, &mut out);
        })
    }
    fn levels(seconds: f64) -> BenchResult {
        // C4FM at 4800 symbols/s.
        let n = 4800;
        let x: Vec<f32> = (0..n)
            .map(|i| [3.0, 1.0, -1.0, -3.0][(i * 7) % 4] * 600.0)
            .collect();
        let mut s = LevelSlicer::c4fm(0.01, 600.0).expect("valid");
        let mut out = Vec::with_capacity(2 * n);
        measure(seconds, n, n as f64 / 4800.0, || {
            out.clear();
            s.process(&x, &mut out).unwrap_or(());
        })
    }
    fn discriminator(seconds: f64) -> BenchResult {
        // 4-level FSK on IQ at 48 kHz, 4800 baud (10 samples per symbol): discriminator, integrate-and-dump, symbol clock.
        let n = 48_000;
        let mut ph = 0f32;
        let x: Vec<Complex32> = (0..n)
            .map(|i| {
                ph += [0.03f32, 0.01, -0.01, -0.03][(i / 10 * 7) % 4] * std::f32::consts::TAU;
                Complex32::from_polar(1.0, ph)
            })
            .collect();
        let mut d =
            DiscriminatorLevels::new(DiscriminatorConfig::new(10.0, Some(0.03))).expect("valid");
        let mut out = Vec::with_capacity(n / 8);
        measure(seconds, n, n as f64 / 48_000.0, || {
            out.clear();
            d.process_iq(&x, &mut out);
        })
    }
    fn tracked_llr(seconds: f64) -> BenchResult {
        // Blind-tracked LLRs of 8-FSK (Gray) at 100 baud... counted per symbol, timed at 1200 symbols/s.
        let n = 4096;
        let e: Vec<f32> = (0..n * 8)
            .map(|i| {
                if i % 11 == 0 {
                    50.0
                } else {
                    1.0 + (i % 3) as f32
                }
            })
            .collect();
        let mut t = TrackedFskLlr::new(FskLlr::gray(8).expect("valid"), 16.0).expect("valid");
        let mut l = [0f32; 3];
        measure(seconds, n, n as f64 / 1200.0, || {
            for s in e.as_chunks::<8>().0 {
                t.symbol(s, &mut l).unwrap_or(());
            }
        })
    }
    vec![
        Benchmark {
            name: "modem.fsk4_discriminator_levels_48k",
            run: discriminator,
        },
        Benchmark {
            name: "modem.fsk8_tracked_llr_1200",
            run: tracked_llr,
        },
        Benchmark {
            name: "modem.afsk1200_48k",
            run: afsk,
        },
        Benchmark {
            name: "modem.mfsk8_12k",
            run: mfsk8,
        },
        Benchmark {
            name: "modem.c4fm_levels_4800",
            run: levels,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::ber_fsk_noncoherent;
    use sdrkit_algo_dsp::meter::db_to_power;
    use sdrkit_algo_sim::modulate::{Afsk, Cpm};
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn soft_bits_with_and_without_priors() {
        let llr = FskLlr::gray(8).unwrap();
        // Metrics that favour tone 5 (label gray(5) = 7, bits 111).
        let mut m = vec![0f32; 8];
        m[5] = 6.0;
        let mut out = [0f32; 3];
        llr.soft_bits(&m, &[], &mut out).unwrap();
        assert!(out.iter().all(|&v| v < -3.0), "{out:?}");
        // The energies form agrees with the metrics form.
        let e: Vec<f32> = (0..8).map(|t| if t == 5 { 9.0 } else { 1.0 }).collect();
        let met: Vec<f32> = e.iter().map(|&v| ln_i0(2.0 * 2.0 * v.sqrt())).collect();
        let (mut a, mut b) = ([0f32; 3], [0f32; 3]);
        llr.llrs(&e, 2.0, 1.0, &mut a).unwrap();
        llr.soft_bits(&met, &[], &mut b).unwrap();
        for (x, y) in a.iter().zip(&b) {
            assert!((x - y).abs() < 1e-3, "{a:?} {b:?}");
        }
        // Flat metrics: the output is the extrinsic part only, so a prior on bit 0 alone tells nothing of bit 0.
        llr.soft_bits(&[0.0; 8], &[5.0, 0.0, 0.0], &mut out)
            .unwrap();
        assert!(out.iter().all(|v| v.abs() < 1e-5), "{out:?}");
        assert!(llr.soft_bits(&[0.0; 7], &[], &mut out).is_err());
    }

    #[test]
    fn ln_i0_matches_the_series() {
        for &x in &[0.0f32, 0.5, 2.0, 3.7, 3.8, 10.0, 50.0] {
            // I0 by its power series in f64.
            let mut s = 1.0f64;
            let mut t = 1.0f64;
            for k in 1..400 {
                t *= (x as f64 / 2.0).powi(2) / (k * k) as f64;
                s += t;
            }
            assert!(
                (ln_i0(x) as f64 - s.ln()).abs() < 2e-6 * (1.0 + s.ln()),
                "{x}"
            );
        }
    }

    /// Orthogonal M-FSK with symbol-long correlators: BER against the closed form.
    fn mfsk_ber(m: usize, ebn0_db: f64, nsym: usize, seed: u64) -> f64 {
        let sps = 16usize;
        let tones: Vec<f64> = (0..m)
            .map(|k| (k as f64 - (m as f64 - 1.0) / 2.0) / sps as f64)
            .collect();
        let k = m.trailing_zeros() as f64;
        let mut rng = Rng::new(seed);
        let mut x = Vec::with_capacity(nsym * sps);
        let mut sent = Vec::with_capacity(nsym);
        for _ in 0..nsym {
            let t = rng.below(m as u64) as usize;
            sent.push(t);
            for _ in 0..sps {
                let n = x.len() as f64;
                x.push(Complex32::from_polar(
                    1.0,
                    (TAU * ((tones[t] * n) % 1.0)) as f32,
                ));
            }
        }
        // Es = sps (unit power), N0 = noise variance per sample.
        let es_n0 = k * db_to_power(ebn0_db);
        let mut awgn = Awgn::with_power(sps as f64 / es_n0, seed + 1).unwrap();
        awgn.process(&mut x);
        let mut c = ToneCorrelator::new(&tones, sps).unwrap();
        let llr = FskLlr::natural(m).unwrap();
        let mut e = vec![0f32; m];
        let mut lab = [0u16];
        let mut errors = 0usize;
        for (s, chunk) in x.chunks_exact(sps).enumerate() {
            for &v in chunk {
                c.push(v);
            }
            c.energies(&mut e).unwrap();
            llr.decide(&e, &mut lab).unwrap();
            errors += (lab[0] as usize ^ sent[s]).count_ones() as usize;
        }
        errors as f64 / (nsym as f64 * k)
    }

    #[test]
    fn noncoherent_mfsk_ber_follows_theory() {
        for (m, ebn0) in [(2usize, 8.0), (2, 11.0), (4, 8.0), (8, 6.0)] {
            let got = mfsk_ber(m, ebn0, 60_000, 7 + m as u64);
            let want = ber_fsk_noncoherent(m, db_to_power(ebn0));
            assert!(
                got > 0.75 * want && got < 1.3 * want,
                "{m}-FSK at {ebn0} dB: {got} vs {want}"
            );
        }
    }

    #[test]
    fn llrs_are_calibrated() {
        // 4-FSK at a low SNR: among bits whose |LLR| falls in a bin, the share in error is 1 / (1 + e^|L|).
        let m = 4;
        let sps = 16usize;
        let tones: Vec<f64> = (0..m).map(|k| (k as f64 - 1.5) / sps as f64).collect();
        let mut rng = Rng::new(3);
        let nsym = 80_000;
        let mut x = Vec::with_capacity(nsym * sps);
        let mut sent = Vec::new();
        for _ in 0..nsym {
            let t = rng.below(m as u64) as usize;
            sent.push(t);
            for _ in 0..sps {
                let n = x.len() as f64;
                x.push(Complex32::from_polar(
                    1.0,
                    (TAU * ((tones[t] * n) % 1.0)) as f32,
                ));
            }
        }
        let noise_per_sample = 2.0;
        Awgn::with_power(noise_per_sample, 9)
            .unwrap()
            .process(&mut x);
        let mut c = ToneCorrelator::new(&tones, sps).unwrap();
        let mut e = Vec::with_capacity(nsym * m);
        let mut row = vec![0f32; m];
        for chunk in x.chunks_exact(sps) {
            for &v in chunk {
                c.push(v);
            }
            c.energies(&mut row).unwrap();
            e.extend_from_slice(&row);
        }
        let llr = FskLlr::gray(m).unwrap();
        let mut l = vec![0f32; nsym * 2];
        // True parameters: amplitude sps, noise sps * N0.
        llr.llrs(
            &e,
            sps as f32,
            (sps as f64 * noise_per_sample) as f32,
            &mut l,
        )
        .unwrap();
        let mut bins = [(0usize, 0usize); 6];
        for (s, &t) in sent.iter().enumerate() {
            let label = sdrkit_util_bits::gray::gray(t as u32) as usize;
            for b in 0..2 {
                let bit = (label >> (1 - b)) & 1;
                let v = l[2 * s + b];
                let k = ((v.abs() / 1.0) as usize).min(5);
                bins[k].0 += 1;
                if (v < 0.0) as usize != bit {
                    bins[k].1 += 1;
                }
            }
        }
        for (k, &(n, err)) in bins.iter().enumerate().take(5) {
            if n < 2000 {
                continue;
            }
            let p = err as f64 / n as f64;
            // Expected over the bin [k, k+1): average of 1 / (1 + e^L).
            let lo = 1.0 / (1.0 + (k as f64 + 1.0).exp());
            let hi = 1.0 / (1.0 + (k as f64).exp());
            assert!(
                p > 0.85 * lo && p < 1.15 * hi,
                "bin {k}: {p} not in [{lo}, {hi}]"
            );
        }
        // The blind estimate is close to the truth.
        let (a, n) = FskLlr::estimate(&e, m).unwrap();
        assert!(
            (a / sps as f32 - 1.0).abs() < 0.15 && (n / 32.0 - 1.0).abs() < 0.25,
            "{a} {n}"
        );
    }

    #[test]
    fn afsk_on_audio_follows_a_clock_error() {
        // Bell 202 at 48 kHz with the clock 300 ppm slow, through the analytic step: every bit comes back.
        let rate = 48_000.0 * (1.0 + 300e-6);
        let mut a = Afsk::new(rate, 1200.0, 1200.0, 2200.0).unwrap();
        let mut rng = Rng::new(5);
        let bits: Vec<u8> = (0..3000).map(|_| rng.bit()).collect();
        let mut audio = Vec::new();
        a.push(&bits, &mut audio);
        a.flush(&mut audio);
        Awgn::with_power(0.05, 2).unwrap().process_real(&mut audio);
        let cfg = MfskConfig::from_hz(&[2200.0, 1200.0], 1200.0, 48_000.0);
        let mut d = MfskDemod::new(cfg)
            .unwrap()
            .with_analytic(Analytic::for_band(48_000.0, 600.0).unwrap());
        let mut e = Vec::new();
        // Fed in uneven pieces.
        for piece in audio.chunks(777) {
            d.process_real(piece, &mut e).unwrap();
        }
        let llr = FskLlr::natural(2).unwrap();
        let mut lab = vec![0u16; e.len() / 2];
        llr.decide(&e, &mut lab).unwrap();
        let got: Vec<u8> = lab.iter().map(|&l| l as u8).collect();
        // Find the sent bits in what came out (the first symbols are the clock settling).
        let want = &bits[200..2800];
        let found =
            (0..400).any(|o| got.len() >= o + want.len() && &got[o..o + want.len()] == want);
        assert!(found, "bits not recovered; clock {} ppm", d.clock_ppm());
        assert!((d.clock_ppm() - 300.0).abs() < 150.0, "{}", d.clock_ppm());
    }

    #[test]
    fn fft_tones_match_the_correlator() {
        let tones = [3usize, 7];
        let x: Vec<Complex32> = (0..32)
            .map(|n| Complex32::from_polar(1.0, (TAU * 7.0 * n as f64 / 32.0) as f32))
            .collect();
        let mut f = FftTones::new(32, &tones).unwrap();
        let mut e = [0f32; 2];
        f.energies(&x, &mut e).unwrap();
        let mut c = ToneCorrelator::new(&[3.0 / 32.0, 7.0 / 32.0], 32).unwrap();
        for &v in &x {
            c.push(v);
        }
        let mut e2 = [0f32; 2];
        c.energies(&mut e2).unwrap();
        assert!((e[1] - 1024.0).abs() < 0.1 && e[0] < 1e-3);
        assert!((e2[1] - e[1]).abs() < 0.5 && e2[0] < 1e-2);
        assert!(f.energies(&[Complex32::default(); 40], &mut e).is_err());
        assert!(FftTones::new(32, &[40]).is_err());
    }

    #[test]
    fn correlator_has_no_drift_over_long_runs() {
        // A non-integer tone frequency for 10 million samples: the sum stays at the window length.
        let f = 0.123_456_789;
        let mut c = ToneCorrelator::new(&[f], 100).unwrap();
        let mut p = sdrkit_algo_dsp::loopfilter::PhaseAcc::new(f);
        for _ in 0..10_000_000 {
            c.push(p.next_phasor());
        }
        let e = c.sums()[0].norm();
        assert!((e - 100.0).abs() < 0.01, "{e}");
    }

    #[test]
    fn level_slicer_c4fm_with_offset_and_noise() {
        // 4-level FSK through the CPM modulator and a discriminator would also do; levels direct keep the test about the slicer.
        let mut rng = Rng::new(11);
        let dev = 600.0f32;
        let dc = 140.0f32;
        let sigma = 120.0f32;
        let n = 20_000;
        let mut idx = Vec::new();
        let mut x = Vec::new();
        for _ in 0..n {
            let i = rng.below(4) as u8;
            idx.push(i);
            x.push(dc + dev * (2.0 * i as f32 - 3.0) + sigma * rng.normal_f32());
        }
        let mut s = LevelSlicer::c4fm(0.005, 500.0).unwrap();
        // Train on a 24-symbol sync.
        s.fit(&x[..24], &idx[..24]).unwrap();
        let mut l = [0f32; 2];
        let map = [0b11u16, 0b10, 0b00, 0b01];
        let mut errs = 0;
        for k in 24..n {
            let lab = s.slice(x[k], &mut l).unwrap();
            if lab != map[idx[k] as usize] {
                errs += 1;
            }
            assert_eq!((l[0] < 0.0) as u16 * 2 + (l[1] < 0.0) as u16, lab);
        }
        // Symbol error rate for 4-PAM with this spacing: 1.5 * 2 Q(dev / sigma) = 3 Q(5).
        let ser = errs as f64 / (n - 24) as f64;
        assert!(ser < 1e-4, "{ser}");
        assert!((s.centre() - dc).abs() < 10.0 && (s.spacing() - dev).abs() < 10.0);
        assert!(
            (s.noise_var().sqrt() - sigma).abs() < 10.0,
            "{}",
            s.noise_var().sqrt()
        );
        assert!(s.fit(&[1.0, 1.0], &[2, 2]).is_err());
        assert!(LevelSlicer::new(&[0, 1, 2], 0.1, 1.0).is_err());
    }

    #[test]
    fn binary_cpfsk_through_the_discriminator() {
        // 2-FSK h = 1 at 10 samples per symbol, discriminator then integrate-and-dump at the known instants.
        let mut m = Cpm::cpfsk(1.0, 10.0).unwrap();
        let mut rng = Rng::new(1);
        let a: Vec<f32> = (0..500)
            .map(|_| if rng.bit() == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        let mut d = sdrkit_algo_dsp::fm::Discriminator::new(1.0, 0.05).unwrap();
        let mut f = vec![0f32; x.len()];
        d.process_into(&x, &mut f);
        let mut s = LevelSlicer::binary(0.02, 1.0).unwrap();
        let mut l = [0f32];
        for (k, &want) in a.iter().enumerate().skip(1) {
            let mean: f32 = f[k * 10 + 1..k * 10 + 10].iter().sum::<f32>() / 9.0;
            let lab = s.slice(mean, &mut l).unwrap();
            assert_eq!(lab, (want < 0.0) as u16);
        }
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(ToneCorrelator::new(&[0.7], 10).is_err());
        assert!(MfskDemod::new(MfskConfig::new(&[0.1], 10.0)).is_err());
        assert!(MfskDemod::new(MfskConfig::new(&[0.1, 0.2], 1.0)).is_err());
        assert!(FskLlr::new(&[0, 0]).is_err());
        let l = FskLlr::natural(2).unwrap();
        let mut o = [0f32; 1];
        assert!(l.llrs(&[1.0, f32::NAN], 1.0, 1.0, &mut o).is_err());
        assert!(l.llrs(&[1.0, 2.0], 1.0, 0.0, &mut o).is_err());
        let mut d = MfskDemod::new(MfskConfig::new(&[0.1, 0.2], 10.0)).unwrap();
        assert!(d.process_real(&[0.0], &mut Vec::new()).is_err());
        assert!(Analytic::new(8, 5.0).is_err());
        let mut s = LevelSlicer::binary(0.1, 1.0).unwrap();
        assert!(s.slice(f32::NAN, &mut o).is_err());
        assert!(TrackedFskLlr::new(FskLlr::natural(2).unwrap(), 0.5).is_err());
        let mut t = TrackedFskLlr::new(FskLlr::natural(4).unwrap(), 16.0).unwrap();
        let mut o2 = [0f32; 2];
        assert!(t.symbol(&[1.0, 2.0], &mut o2).is_err());
        assert!(t.symbol(&[1.0, 2.0, 3.0, 4.0], &mut o).is_err());
        for c in [
            DiscriminatorConfig::new(1.5, Some(0.1)),
            DiscriminatorConfig::new(10.0, Some(0.5)),
            DiscriminatorConfig::new(10.0, Some(-0.1)),
        ] {
            assert!(DiscriminatorLevels::new(c).is_err(), "{c:?}");
        }
    }

    #[test]
    fn label_maps_by_name() {
        for (k, n) in LabelMap::NAMES.iter().enumerate() {
            let m = LabelMap::by_name(n).unwrap();
            assert_eq!((m.name(), m as usize), (*n, k));
        }
        assert!(LabelMap::by_name("grey").is_err());
        assert_eq!(
            LabelMap::Natural.labels(8).unwrap(),
            [0, 1, 2, 3, 4, 5, 6, 7]
        );
        // Reflected binary Gray code of 3 bits.
        assert_eq!(LabelMap::Gray.labels(8).unwrap(), [0, 1, 3, 2, 6, 7, 5, 4]);
        assert_eq!(LabelMap::Binary.labels(2).unwrap(), [1, 0]);
        // TIA-102.BAAA-A table 3 from the most negative level: -3 is 11, -1 is 10, +1 is 00, +3 is 01.
        assert_eq!(LabelMap::C4fm.labels(4).unwrap(), [3, 2, 0, 1]);
        assert!(LabelMap::C4fm.labels(2).is_err());
        assert!(LabelMap::Natural.labels(6).is_err());
        assert!(LabelMap::Natural.labels(1).is_err());
    }

    #[test]
    fn tracked_llrs_follow_the_level_and_skip_bad_symbols() {
        let mut t = TrackedFskLlr::new(FskLlr::natural(2).unwrap(), 16.0).unwrap();
        let mut l = [0f32];
        assert_eq!(t.snr(), 0.0);
        // The first symbol sets the estimates; later ones move them by 1/16 of the difference.
        t.symbol(&[101.0, 1.0], &mut l).unwrap();
        assert!((t.amplitude() - 10.0).abs() < 1e-4 && (t.noise() - 1.0).abs() < 1e-6);
        let want = FskLlr::natural(2).unwrap();
        let mut w = [0f32];
        want.llrs(&[101.0, 1.0], 10.0, 1.0, &mut w).unwrap();
        assert_eq!(l, w);
        t.symbol(&[1.0, 3.0], &mut l).unwrap();
        // Strongest 3, other 1: A^2 = 2, N = 1.
        assert!((t.amplitude().powi(2) - (100.0 + (2.0 - 100.0) / 16.0)).abs() < 1e-3);
        assert!(l[0] < 0.0);
        // A symbol that is not a number is erased and leaves the estimates alone.
        let (a, n) = (t.amplitude(), t.noise());
        assert_eq!(
            t.symbol(&[f32::NAN, 1.0], &mut l),
            Err(ModemError::NotFinite {
                param: Param::Input
            })
        );
        assert_eq!((l[0], t.amplitude(), t.noise()), (0.0, a, n));
        t.reset();
        assert_eq!(t.snr(), 0.0);
    }

    /// Binary FSK at `sps` samples per symbol (deviation 0.05 cycles per sample), random symbols, light noise.
    fn fsk2(n: usize, sps: f64, seed: u64) -> (Vec<f32>, Vec<Complex32>) {
        let mut rng = Rng::new(seed);
        let a: Vec<f32> = (0..n)
            .map(|_| if rng.bit() == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut m = Cpm::gfsk(0.5, 0.1 * sps, sps).unwrap();
        let mut x = Vec::new();
        m.push(&a, &mut x);
        m.flush(&mut x);
        Awgn::with_power(0.05, seed + 1).unwrap().process(&mut x);
        (a, x)
    }

    #[test]
    fn discriminator_levels_are_cut_invariant_and_decide_the_symbols() {
        let sps = 10.0;
        let (a, x) = fsk2(3000, sps, 91);
        let mut cfg = DiscriminatorConfig::new(sps, Some(0.05));
        cfg.bt = Some(0.5);
        let run = |cut: usize| {
            let mut d = DiscriminatorLevels::new(cfg).unwrap();
            let mut out = Vec::new();
            for p in x.chunks(cut) {
                d.process_iq(p, &mut out);
            }
            out
        };
        let whole = run(x.len());
        for cut in [1, 7, 4096, 5000] {
            assert_eq!(run(cut), whole, "{cut}");
        }
        // After the loop settles, every level has the sign of its symbol, a fixed few symbols late.
        let ok = (0..5).any(|d| {
            whole[500..2900]
                .iter()
                .zip(&a[500 - d..2900 - d])
                .all(|(l, s)| l * s > 0.0)
        });
        assert!(ok);
        // Real input that is a frequency already produces the same levels as the discriminator's output.
        let mut disc = sdrkit_algo_dsp::fm::Discriminator::new(1.0, 0.05).unwrap();
        let mut f = vec![0f32; x.len()];
        disc.process_into(&x, &mut f);
        let mut r = DiscriminatorLevels::new(DiscriminatorConfig {
            deviation: None,
            ..cfg
        })
        .unwrap();
        let mut out = Vec::new();
        r.process_real(&f, &mut out);
        assert_eq!(out, whole);
        r.reset();
        out.clear();
        r.process_real(&f, &mut out);
        assert_eq!(out, whole);
    }

    /// Continuous-phase 2-FSK, `sps` samples a symbol, deviation `dev` and carrier `c` in cycles per sample, unit power.
    fn cpfsk(bits: &[u8], sps: usize, dev: f64, c: f64) -> Vec<Complex32> {
        let mut ph = 0f64;
        let mut x = Vec::with_capacity(bits.len() * sps);
        for &b in bits {
            let f = c + if b == 1 { dev } else { -dev };
            for _ in 0..sps {
                ph = (ph + f).rem_euclid(1.0);
                x.push(Complex32::from_polar(1.0, (TAU * ph) as f32));
            }
        }
        x
    }

    /// The channel filter of the FSK tests (pass 7.2 kHz, stop 12 kHz at 48 kHz) on complex samples, and its delay.
    fn channel(x: &[Complex32]) -> (Vec<Complex32>, usize) {
        let taps = fir::lowpass(48_000.0, 7200.0, 12_000.0, 50.0).unwrap();
        let d = (taps.len() - 1) / 2;
        let mut f = Fir::<Complex32>::new(&taps);
        let mut y = vec![Complex32::default(); x.len()];
        f.process(x, &mut y);
        (y, d)
    }

    #[test]
    fn tone_energies_halve_the_symbol_errors_of_the_discriminator_mean() {
        // 2-FSK at h = 1, 4800 bit/s, 48 kHz, a carrier 700 Hz off, Eb/N0 = 10 dB; both detectors after the same channel filter.
        let (rate, sps) = (48_000.0, 10usize);
        let (dev, c) = (2400.0 / rate, 700.0 / rate);
        let n = 40_000;
        let mut rng = Rng::new(31);
        let bits: Vec<u8> = (0..n).map(|_| rng.bit()).collect();
        let clean = cpfsk(&bits, sps, dev, c);
        let count = |x: &[Complex32]| {
            let (y, d) = channel(x);
            let mut pair = TonePair::new(rate, 700.0, 2400.0, 64).unwrap();
            let mut disc = Discriminator::new(rate, 1.0).unwrap();
            let mut dv = vec![0f32; y.len()];
            disc.process_into(&y, &mut dv);
            let (mut ep, mut ed) = (0usize, 0usize);
            for (k, &bit) in bits[..n - 2].iter().enumerate() {
                let end = (k + 1) * sps + d;
                pair.process(&y[end - sps..end]);
                let up = pair.upper(sps, 0).unwrap();
                let mean = dv[end - sps..end].iter().sum::<f32>() / sps as f32;
                ep += (up != (bit == 1)) as usize;
                ed += ((mean > 700.0) != (bit == 1)) as usize;
            }
            (ep, ed)
        };
        // With no noise both give the sent symbols.
        assert_eq!(count(&clean), (0, 0));
        let mut x = clean.clone();
        Awgn::with_power(1.0, 32).unwrap().process(&mut x);
        let (ep, ed) = count(&x);
        // The tone energies make at most half the symbol errors of the discriminator mean.
        assert!(ep >= 20 && 2 * ep <= ed, "tones {ep}, discriminator {ed}");
    }

    #[test]
    fn tone_pair_windows_centre_and_gains() {
        let rate = 48_000.0;
        let mut p = TonePair::new(rate, 0.0, 2400.0, 32).unwrap();
        assert!(p.energies(10, 0).is_none());
        let tone =
            |hz: f64, n: usize| Complex32::from_polar(1.0, (TAU * hz * n as f64 / rate) as f32);
        for n in 0..20 {
            p.push(tone(-2400.0, n));
        }
        let [eu, el] = p.energies(10, 0).unwrap();
        assert!((el - 100.0).abs() < 1e-3 && eu < 1e-6, "{eu} {el}");
        // Half a window, and a window ending earlier.
        assert!((p.energies(5, 3).unwrap()[1] - 25.0).abs() < 1e-3);
        assert!(p.energies(30, 3).is_none());
        // The centre moves with the carrier: a tone at 1000 + 2400 Hz is the upper tone once the centre is 1000 Hz.
        p.set_centre(1000.0);
        for n in 20..40 {
            p.push(tone(3400.0, n));
        }
        assert!(p.value(10, 0).unwrap() > 0.999);
        // Gains: an upper tone at half the power of the lower one reads as strong as it once divided by its gain.
        let g = tone_gains(0.5, 1.0);
        assert_eq!(g, [0.5, 1.0]);
        p.set_gains(g[0], g[1]);
        p.reset();
        for n in 0..10 {
            let z = tone(3400.0, n) * std::f32::consts::FRAC_1_SQRT_2 + tone(-1400.0, n);
            p.push(z);
        }
        assert!(p.value(10, 0).unwrap().abs() < 1e-3);
        // A gain under 0.3 of the mean is not believed.
        assert_eq!(tone_gains(1.0, 0.1), [1.0, 0.55]);
        // Long runs: the re-anchored rotators and f64 sums keep a window exact.
        let mut q = TonePair::new(rate, 300.0, 1200.0, 16).unwrap();
        for n in 0..200_000 {
            q.push(tone(1500.0, n));
        }
        assert!((q.energies(16, 0).unwrap()[0] - 256.0).abs() < 0.05);
    }

    #[test]
    fn extremes_of_the_average_slice_an_unbalanced_code() {
        // Start-stop framing at 4800 Bd (start 0, 8 data bits, stop 1, four idle 1s), mostly-one bytes, a carrier 1.5 kHz off,
        // +-2.4 kHz, 48 kHz, after the channel filter, at Eb/N0 = 13 dB. Decisions on the moving average at the 9th sample of a bit.
        let (rate, sps) = (48_000.0, 10usize);
        let mut rng = Rng::new(41);
        let mut bits = Vec::new();
        while bits.len() < 60_000 {
            let byte = (rng.next_u32() | rng.next_u32() | rng.next_u32()) as u8;
            bits.push(0u8);
            bits.extend((0..8).map(|i| byte >> i & 1));
            bits.extend([1, 1, 1, 1, 1]);
        }
        let ones = bits.iter().filter(|&&b| b == 1).count() as f64 / bits.len() as f64;
        assert!(ones > 0.8, "{ones}");
        let mut x = cpfsk(&bits, sps, 2400.0 / rate, 1500.0 / rate);
        Awgn::with_power(10f64.powf(-1.3) * sps as f64, 42)
            .unwrap()
            .process(&mut x);
        let (y, d) = channel(&x);
        let mut disc = Discriminator::new(rate, 1.0).unwrap();
        let mut dv = vec![0f32; y.len()];
        disc.process_into(&y, &mut dv);
        let mut s3 = ExtremeThreshold::new(rate, 4800.0).unwrap();
        assert_eq!(s3.len(), 7);
        let mut raw = ExtremeThreshold::with_params(1, (4800.0 / rate / 12.0) as f32).unwrap();
        let mut mean = 0f64;
        let am = 1.0 / (64.0 * sps as f64);
        let (mut e3, mut er, mut em) = (0usize, 0usize, 0usize);
        for (i, &v) in dv.iter().enumerate() {
            let (avg, thr) = s3.push(v);
            let (_, thr_raw) = raw.push(v);
            mean += (v as f64 - mean) * am;
            if i < d + 200 * sps || (i - d) % sps != 8 {
                continue;
            }
            let b = bits[(i - d) / sps] == 1;
            e3 += ((avg > thr) != b) as usize;
            er += ((avg > thr_raw) != b) as usize;
            em += ((avg as f64 > mean) != b) as usize;
        }
        // The extremes of the average make under half the errors of the running mean and under a tenth of those of the raw
        // extremes.
        assert!(
            e3 * 2 < em && e3 * 10 < er,
            "average {e3}, raw {er}, mean {em}"
        );
    }
}

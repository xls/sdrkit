//! Carrier frequency estimators: coarse searches by FFT and fine estimators by autocorrelation, all feed-forward (for burst
//! receivers, acquisition, and the starting frequency of a carrier loop).
//!
//! | Estimator | Input | Range | Accuracy |
//! |---|---|---|---|
//! | [`PowerLine`] | any M-PSK (or a tone with `m = 1`): the line of `x^m` in an FFT, refined by Candan's three-bin formula (one frame) or a log-parabola (averaged frames) | `+-rate / (2m)` | a fraction of a bin |
//! | [`PowerLine::estimate_pair`] | MSK, OQPSK, GMSK: the two lines of `x^2` at `2 fc +- Rs / 2` (both carrier and symbol rate) | as above | as above |
//! | [`lag_phase`] | a tone: `arg R(L) / 2 pi L` at one lag | `+-1 / 2L` | |
//! | [`kay`] | a tone: weighted average of sample-to-sample phase steps (S. Kay, IEEE Trans. ASSP 37(12), 1989) | `+-1 / 2` | reaches the Cramer-Rao bound above about 6 dB SNR per sample |
//! | [`fitz`] | a tone: sum of the angles of `R(1) .. R(M)` (M. P. Fitz, IEEE Trans. Commun. 39(6), 1991) | `+-1 / 2M` | near the bound with `M = N / 2` |
//! | [`luise_reggiannini`] | a tone: the angle of `R(1) + ... + R(M)` (M. Luise and R. Reggiannini, IEEE Trans. Commun. 43(2/3/4), 1995) | `+-1 / (M + 1)` | near the bound with `M = N / 2` |
//! | [`multi_lag`] | a tone: a short lag for range, then longer lags for precision, each resolving the ambiguity of the next | `+-1 / 2 L1` | that of the longest lag |
//! | [`centroid`] | a power spectrum: power-weighted mean bin above a floor | the span given | for wide or spread signals |
//! | [`TemplateFit`] | an averaged power spectrum of a linearly modulated signal: the best shift of its raised-cosine power shape, refined by a parabola | the span given | a fraction of a bin |
//! | [`BandSearch`] | an averaged power spectrum: the strongest band of a given width, each bin capped at a multiple of the noise floor, known bad places skipped, refined by the centroid | the span given | about a bin (a coarse carrier search) |
//!
//! Frequencies are in cycles per sample (multiply by the rate for hertz). The two spectrum searches work in hertz on a power spectrum
//! ordered from the lowest frequency to the highest (index `n / 2` is DC, as `sdrkit_algo_dsp::spectrum::Welch` outputs it). Data modulation is removed first by the caller (a known
//! preamble multiplied by its conjugate) or by [`PowerLine`]'s `m`-th power. No estimator allocates per call: [`PowerLine`] holds
//! its FFT plan and buffers, the rest work on the slice given. Angles come from `sdrkit_algo_dsp::approx::atan2`.
//!
//! ```
//! use sdrkit_algo_modem::freq::{kay, PowerLine};
//! use sdrkit_algo_modem::Complex32;
//! let f = 0.0123;
//! let x: Vec<Complex32> = (0..1024).map(|n| Complex32::from_polar(1.0, (std::f64::consts::TAU * f * n as f64) as f32)).collect();
//! assert!((kay(&x).unwrap() - f).abs() < 1e-6);
//! // QPSK: the 4th-power line.
//! let q: Vec<Complex32> = x.iter().enumerate().map(|(n, z)| z * Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_2 * (n * 7 % 4) as f32 + 0.785)).collect();
//! let mut pl = PowerLine::new(4, 1024).unwrap();
//! let line = pl.estimate(&q).unwrap();
//! assert!((line.freq - f).abs() < 1e-5 && line.snr_db > 30.0);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::peak::{candan, log_parabolic};
use std::f64::consts::TAU;

/// A spectral line found by [`PowerLine`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// The carrier frequency, cycles per sample (the line's frequency divided by `m`).
    pub freq: f64,
    /// The line's power over the median noise of the bins searched, dB.
    pub snr_db: f32,
    /// The FFT bin of the peak (before refinement).
    pub bin: usize,
}

/// Two lines found by [`PowerLine::estimate_pair`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinePair {
    /// The carrier frequency, cycles per sample: the mid-point of the two lines divided by `m`.
    pub freq: f64,
    /// The measured spacing of the lines, cycles per sample (for MSK and OQPSK with `m = 2`, the symbol rate).
    pub spacing: f64,
    /// The weaker line's power over the median noise, dB.
    pub snr_db: f32,
}

/// The `m`-th power FFT line search (module documentation). Holds its FFT plan, buffers and floor estimator.
#[derive(Debug)]
pub struct PowerLine {
    m: u32,
    n: usize,
    fft: Fft,
    buf: Vec<Complex32>,
    spec: Vec<Complex32>,
    pow: Vec<f32>,
    floor: BinMedian,
    max_freq: f64,
    reject: Vec<(f64, f64)>,
}

/// The most frequency ranges a [`PowerLine`] can be told to skip.
pub const MAX_REJECT: usize = 16;

impl PowerLine {
    /// A search raising samples to the power `m` (1 to 16; 1 for a tone, 2 for BPSK, 4 for QPSK and square QAM, 8 for 8PSK) with
    /// an `n`-point FFT (4 to 2^20).
    pub fn new(m: u32, n: usize) -> Result<Self, ModemError> {
        Param::Order.check_count(m as usize, 1, 16)?;
        Param::FftSize.check_count(n, 4, 1 << 20)?;
        let mut floor = BinMedian::new(1)?;
        floor.reserve(n);
        Ok(PowerLine {
            m,
            n,
            fft: Fft::forward(n),
            buf: vec![Complex32::default(); n],
            spec: vec![Complex32::default(); n],
            pow: vec![0.0; n],
            floor,
            max_freq: 0.5 / m as f64,
            reject: Vec::with_capacity(MAX_REJECT),
        })
    }

    /// Searches only carriers within `+-max_freq` cycles per sample (at most `1 / (2m)`, the default).
    pub fn set_span(&mut self, max_freq: f64) -> Result<(), ModemError> {
        if !(max_freq > 0.0 && max_freq <= 0.5 / self.m as f64) {
            return Param::MaxFreq.range(0.0, 0.5 / self.m as f64, max_freq);
        }
        self.max_freq = max_freq;
        Ok(())
    }

    /// Skips carriers within `width` of each frequency in `list` (spurs, DC, an offset that never locked), at most [`MAX_REJECT`].
    pub fn set_reject(&mut self, list: &[f64], width: f64) -> Result<(), ModemError> {
        if list.len() > MAX_REJECT {
            return Param::Reject.range(0.0, MAX_REJECT as f64, list.len() as f64);
        }
        if list.iter().any(|f| !f.is_finite()) {
            return Param::Reject.not_finite();
        }
        if width.is_nan() || width < 0.0 {
            return Param::Width.at_least(0.0, width);
        }
        self.reject.clear();
        self.reject.extend(list.iter().map(|&f| (f, width)));
        Ok(())
    }

    /// The FFT size.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Never true.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Fills `pow` with the averaged power spectrum of `x^m` over frames of `n` (the last one zero-padded); keeps the last frame's
    /// complex bins in `spec`. Returns the number of frames.
    fn spectrum(&mut self, x: &[Complex32]) -> Result<usize, ModemError> {
        if x.len() < 4 {
            return Err(ModemError::InputTooShort { need: 4 });
        }
        self.pow.iter_mut().for_each(|p| *p = 0.0);
        let frames = x.len().div_ceil(self.n);
        for f in 0..frames {
            let chunk = &x[f * self.n..((f + 1) * self.n).min(x.len())];
            for (b, &z) in self
                .buf
                .iter_mut()
                .zip(chunk.iter().chain(std::iter::repeat(&Complex32::default())))
            {
                let mut w = if z.re.is_finite() && z.im.is_finite() {
                    z
                } else {
                    Complex32::default()
                };
                let mut acc = Complex32::new(1.0, 0.0);
                let mut e = self.m;
                // w^m by squaring.
                while e > 0 {
                    if e & 1 == 1 {
                        acc *= w;
                    }
                    w = w * w;
                    e >>= 1;
                }
                *b = acc;
            }
            self.fft.process(&mut self.buf);
            for (p, v) in self.pow.iter_mut().zip(&self.buf) {
                *p += v.norm_sqr();
            }
        }
        self.spec.copy_from_slice(&self.buf);
        Ok(frames)
    }

    /// The frequency of FFT bin `k` (cycles per sample of `x^m`, in `[-1/2, 1/2)`).
    #[inline]
    fn bin_freq(&self, k: f64) -> f64 {
        let f = k / self.n as f64;
        f - (f + 0.5).floor()
    }

    fn allowed(&self, k: usize) -> bool {
        let f = self.bin_freq(k as f64) / self.m as f64;
        f.abs() <= self.max_freq && !self.reject.iter().any(|&(r, w)| (f - r).abs() <= w)
    }

    /// The best allowed bin.
    fn peak(&self) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (k, &p) in self.pow.iter().enumerate() {
            if p.is_finite() && self.allowed(k) && best.is_none_or(|(_, b)| p > b) {
                best = Some((k, p));
            }
        }
        best.map(|b| b.0)
    }

    fn refine(&self, k: usize, frames: usize, single_full: bool) -> f64 {
        let n = self.n;
        let (km, kp) = ((k + n - 1) % n, (k + 1) % n);
        let off = if frames == 1 && single_full {
            candan(self.spec[km], self.spec[k], self.spec[kp], n)
        } else {
            let p = log_parabolic(self.pow[km] as f64, self.pow[k] as f64, self.pow[kp] as f64);
            if p.offset.is_finite() {
                p.offset.clamp(-0.5, 0.5)
            } else {
                0.0
            }
        };
        self.bin_freq(k as f64 + off)
    }

    fn snr_db(&mut self, p: f32) -> f32 {
        let floor = {
            let pow = &self.pow;
            self.floor.estimate(pow.iter().copied())
        };
        match floor {
            Some(f) if f > 0.0 => 10.0 * (p / f).log10(),
            _ => f32::INFINITY,
        }
    }

    /// The strongest line of `x^m` within the span (module documentation). `x` may be shorter than the FFT (zero-padded) or longer
    /// (power spectra of successive frames are averaged). Errors for fewer than 4 samples or when no bin is allowed.
    pub fn estimate(&mut self, x: &[Complex32]) -> Result<Line, ModemError> {
        let frames = self.spectrum(x)?;
        let k = self
            .peak()
            .ok_or(ModemError::Degenerate { param: Param::Span })?;
        let f = self.refine(k, frames, x.len() == self.n);
        let p = self.pow[k];
        Ok(Line {
            freq: f / self.m as f64,
            snr_db: self.snr_db(p),
            bin: k,
        })
    }

    /// The pair of lines of `x^m` spaced `spacing` cycles per sample apart (for MSK, OQPSK and GMSK with `m = 2`, spacing = the symbol
    /// rate): the bin pair `(k, k + spacing n)` with the most power, each refined, and the carrier at their mid-point over `m`.
    pub fn estimate_pair(&mut self, x: &[Complex32], spacing: f64) -> Result<LinePair, ModemError> {
        if !(spacing > 0.0 && spacing < 0.5) {
            return Param::Spacing.range(0.0, 0.5, spacing);
        }
        let frames = self.spectrum(x)?;
        let n = self.n;
        let d = (spacing * n as f64).round() as usize;
        if d == 0 {
            return Param::Spacing.degenerate();
        }
        let mut best: Option<(usize, f32)> = None;
        for k in 0..n {
            let k2 = (k + d) % n;
            // The carrier is the pair's mid-point.
            let mid = self.bin_freq(k as f64 + d as f64 / 2.0) / self.m as f64;
            if mid.abs() > self.max_freq || self.reject.iter().any(|&(r, w)| (mid - r).abs() <= w) {
                continue;
            }
            let p = self.pow[k] + self.pow[k2];
            if p.is_finite() && best.is_none_or(|(_, b)| p > b) {
                best = Some((k, p));
            }
        }
        let (k, _) = best.ok_or(ModemError::Degenerate { param: Param::Span })?;
        let k2 = (k + d) % n;
        let single = x.len() == n;
        let f1 = self.refine(k, frames, single);
        let mut f2 = self.refine(k2, frames, single);
        if f2 < f1 {
            f2 += 1.0;
        }
        let weak = self.pow[k].min(self.pow[k2]);
        let mid = (f1 + f2) / 2.0;
        Ok(LinePair {
            freq: (mid - (mid + 0.5).floor()) / self.m as f64,
            spacing: f2 - f1,
            snr_db: self.snr_db(weak),
        })
    }
}

/// `R(lag) = sum x[k] x[k - lag]*` over the slice.
#[inline]
fn autocorr(x: &[Complex32], lag: usize) -> Complex32 {
    let mut acc = Complex32::default();
    for (a, b) in x[lag..].iter().zip(x) {
        acc += a * b.conj();
    }
    acc
}

#[inline]
fn arg(z: Complex32) -> f64 {
    approx::atan2(z.im, z.re) as f64
}

fn check(x: &[Complex32], need: usize) -> Result<(), ModemError> {
    if x.len() < need {
        return Err(ModemError::InputTooShort { need });
    }
    if x.iter().any(|z| !(z.re.is_finite() && z.im.is_finite())) {
        return Err(ModemError::NotFinite {
            param: Param::Input,
        });
    }
    Ok(())
}

/// The frequency from the autocorrelation at one lag, `arg R(lag) / (2 pi lag)`, unambiguous within `+-1 / (2 lag)` (the cyclic
/// prefix and repeated-preamble estimators use `lag` = the repetition length).
pub fn lag_phase(x: &[Complex32], lag: usize) -> Result<f64, ModemError> {
    if lag == 0 {
        return Param::Lag.at_least(1.0, 0.0);
    }
    check(x, lag + 1)?;
    Ok(arg(autocorr(x, lag)) / (TAU * lag as f64))
}

/// Kay's weighted phase-difference estimator: `sum w[n] arg(x[n+1] x[n]*) / 2 pi` with the parabolic window `w`. Efficient at
/// high SNR, any frequency within `+-1/2`. At least 3 samples.
pub fn kay(x: &[Complex32]) -> Result<f64, ModemError> {
    check(x, 3)?;
    let n = x.len() as f64;
    let m = x.len() - 1;
    let c = 1.5 * n / (n * n - 1.0);
    let half = n / 2.0;
    let mut f = 0.0;
    for k in 0..m {
        let t = (k as f64 + 1.0 - half) / half;
        let w = c * (1.0 - t * t);
        f += w * arg(x[k + 1] * x[k].conj());
    }
    Ok(f / TAU)
}

/// Fitz's estimator: `sum_{m=1..M} arg R(m) / (pi M (M + 1))`, unambiguous within `+-1 / (2M)`. Needs more than `max_lag` samples.
pub fn fitz(x: &[Complex32], max_lag: usize) -> Result<f64, ModemError> {
    if max_lag == 0 {
        return Param::Lag.at_least(1.0, 0.0);
    }
    check(x, max_lag + 1)?;
    let mut s = 0.0;
    for m in 1..=max_lag {
        s += arg(autocorr(x, m));
    }
    let mm = max_lag as f64;
    Ok(s / (std::f64::consts::PI * mm * (mm + 1.0)))
}

/// The Luise and Reggiannini estimator: `arg(sum_{m=1..M} R(m) / (N - m)) / (pi (M + 1))`, unambiguous within `+-1 / (M + 1)`. Needs
/// more than `max_lag` samples.
pub fn luise_reggiannini(x: &[Complex32], max_lag: usize) -> Result<f64, ModemError> {
    if max_lag == 0 {
        return Param::Lag.at_least(1.0, 0.0);
    }
    check(x, max_lag + 1)?;
    let n = x.len();
    let mut s = Complex32::default();
    for m in 1..=max_lag {
        s += autocorr(x, m) / (n - m) as f32;
    }
    Ok(arg(s) / (std::f64::consts::PI * (max_lag as f64 + 1.0)))
}

/// Unambiguous refinement over increasing lags: the first lag sets the range (`+-1 / (2 lags[0])`), each longer lag refines the
/// estimate by choosing the branch of its angle nearest the previous estimate. Lags must increase, and each step must be small
/// enough that the previous estimate's error stays within half the next lag's ambiguity.
pub fn multi_lag(x: &[Complex32], lags: &[usize]) -> Result<f64, ModemError> {
    if lags.is_empty() || lags[0] == 0 || lags.windows(2).any(|w| w[1] <= w[0]) {
        return Param::Lags.not_allowed(lags.len() as f64);
    }
    let last = lags[lags.len() - 1];
    check(x, last + 1)?;
    let mut f = arg(autocorr(x, lags[0])) / (TAU * lags[0] as f64);
    for &l in &lags[1..] {
        let a = arg(autocorr(x, l)) / TAU; // in cycles, the fractional part of f l
        let whole = (f * l as f64 - a).round();
        f = (a + whole) / l as f64;
    }
    Ok(f)
}

/// The power-weighted mean bin of `power[lo..hi]` above `floor` (bins below it count 0), as a fractional bin index; `None` when
/// nothing rises above the floor or the range is empty.
pub fn centroid(power: &[f32], lo: usize, hi: usize, floor: f32) -> Option<f64> {
    let hi = hi.min(power.len());
    if lo >= hi {
        return None;
    }
    let (mut s, mut sk) = (0.0f64, 0.0f64);
    for (k, &p) in power[lo..hi].iter().enumerate() {
        let v = p - floor;
        if v > 0.0 && v.is_finite() {
            s += v as f64;
            sk += v as f64 * (lo + k) as f64;
        }
    }
    (s > 0.0).then(|| sk / s)
}

/// The fit of a raised-cosine power shape to a spectrum, found by [`TemplateFit::fit`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeFit {
    /// The centre of the signal, Hz from the centre of the spectrum.
    pub freq_hz: f64,
    /// The correlation of the spectrum with the mean-removed shape at the best shift (in the spectrum's power units times bins).
    pub score: f64,
}

/// The spectrum template fit: the shift of the ideal power shape of a raised-cosine link that best matches an averaged power
/// spectrum.
///
/// The shape is flat to `Rs (1 - a) / 2` from the centre and falls as a half cosine to zero at `Rs (1 + a) / 2` (the raised-cosine
/// spectrum of the transmit and receive filters together, `a` the roll-off). Its mean is taken off, so a flat noise floor scores 0.
/// Every whole-bin shift within the span is scored by the correlation. The best is refined by a parabola through its neighbours.
/// QPSK and OQPSK links have no line to find, so this is the coarse carrier search for them. Nothing is allocated after
/// [`TemplateFit::new`].
///
/// ```
/// use sdrkit_algo_modem::freq::TemplateFit;
/// // 1024 bins of 250 Hz, and a 72 kHz raised-cosine signal (roll-off 0.6) 3.1 kHz above the centre, on a floor of 1.
/// let (n, bin, rs, a) = (1024, 250.0, 72e3, 0.6);
/// let shape = |f: f64| -> f64 {
///     let (f1, f2) = (rs * (1.0 - a) / 2.0, rs * (1.0 + a) / 2.0);
///     let f = f.abs();
///     if f <= f1 { 1.0 } else if f <= f2 { 0.5 * (1.0 + (std::f64::consts::PI * (f - f1) / (a * rs)).cos()) } else { 0.0 }
/// };
/// let p: Vec<f32> = (0..n).map(|k| (1.0 + 20.0 * shape((k as f64 - 512.0) * bin - 3100.0)) as f32).collect();
/// let t = TemplateFit::new(bin, rs, a, n, 15e3).unwrap();
/// let got = t.fit(&p, 15e3).unwrap();
/// assert!((got.freq_hz - 3100.0).abs() < 50.0);
/// ```
#[derive(Clone, Debug)]
pub struct TemplateFit {
    bin_hz: f64,
    half: usize,
    tmpl: Vec<f64>,
}

impl TemplateFit {
    /// The template for spectra of `bins` bins of `bin_hz`, searched within `+-span_hz`, for a symbol rate `symbol_rate` (Hz)
    /// and a roll-off `roll_off` (0 < roll-off <= 1). The template spans 1.3 times the signal's width, so the shift sees the floor
    /// on both sides. A signal nearly as wide as the spectrum gets a template cut to what the spectrum holds beside the span, so
    /// [`TemplateFit::fit`] always fits a spectrum of `bins` bins. Errors for values out of range, fewer than 8 bins, or a span
    /// that leaves no room for the template.
    pub fn new(
        bin_hz: f64,
        symbol_rate: f64,
        roll_off: f64,
        bins: usize,
        span_hz: f64,
    ) -> Result<Self, ModemError> {
        Param::BinWidth.check_positive(bin_hz)?;
        Param::SymbolRate.check_positive(symbol_rate)?;
        Param::RollOff.check_above(roll_off, 0.0, 1.0)?;
        Param::Bins.check_count(bins, 8, 1 << 20)?;
        if !(span_hz >= 0.0 && span_hz.is_finite()) {
            return Param::Span.at_least(0.0, span_hz);
        }
        let (f1, f2) = (
            symbol_rate * (1.0 - roll_off) / 2.0,
            symbol_rate * (1.0 + roll_off) / 2.0,
        );
        let room = (bins / 2).saturating_sub(2);
        let shift = (span_hz / bin_hz).ceil();
        if shift >= room as f64 {
            return Param::Span.range(0.0, (room - 1) as f64 * bin_hz, span_hz);
        }
        let half = ((f2 * 1.3 / bin_hz).ceil() as usize).clamp(1, room - shift as usize);
        let shape = |f: f64| -> f64 {
            let f = f.abs();
            if f <= f1 {
                1.0
            } else if f <= f2 {
                0.5 * (1.0 + (std::f64::consts::PI * (f - f1) / (roll_off * symbol_rate)).cos())
            } else {
                0.0
            }
        };
        let mut tmpl: Vec<f64> = (0..=2 * half)
            .map(|j| shape((j as f64 - half as f64) * bin_hz))
            .collect();
        let mean = tmpl.iter().sum::<f64>() / tmpl.len() as f64;
        tmpl.iter_mut().for_each(|t| *t -= mean);
        Ok(TemplateFit { bin_hz, half, tmpl })
    }

    /// The template's half width, bins.
    pub fn half_width(&self) -> usize {
        self.half
    }

    fn score(&self, power: &[f32], centre: i64) -> f64 {
        let mut sum = 0.0;
        for (j, t) in self.tmpl.iter().enumerate() {
            let k = centre + j as i64 - self.half as i64;
            if k >= 0 && (k as usize) < power.len() {
                let p = power[k as usize];
                if p.is_finite() {
                    sum += p as f64 * t;
                }
            }
        }
        sum
    }

    /// The best centre within `+-span_hz` of the centre of `power` (linear power, lowest frequency first, DC at `n / 2`). The span
    /// is cut to what the spectrum holds with the template inside it. Errors for a spectrum shorter than the template or a span
    /// that is not finite and at least 0.
    pub fn fit(&self, power: &[f32], span_hz: f64) -> Result<ShapeFit, ModemError> {
        let n = power.len();
        if n < 2 * self.half + 3 {
            return Err(ModemError::InputTooShort {
                need: 2 * self.half + 3,
            });
        }
        if !(span_hz >= 0.0 && span_hz.is_finite()) {
            return Param::Span.at_least(0.0, span_hz);
        }
        let centre = (n / 2) as i64;
        let max_shift = ((span_hz / self.bin_hz).round() as i64)
            .min(n as i64 / 2 - self.half as i64 - 1)
            .max(0);
        let mut best = (f64::NEG_INFINITY, 0i64);
        for s in -max_shift..=max_shift {
            let v = self.score(power, centre + s);
            if v > best.0 {
                best = (v, s);
            }
        }
        let pk = sdrkit_algo_dsp::peak::parabolic(
            self.score(power, centre + best.1 - 1),
            best.0,
            self.score(power, centre + best.1 + 1),
        );
        let d = if pk.offset.is_finite() {
            pk.offset.clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Ok(ShapeFit {
            freq_hz: (best.1 as f64 + d) * self.bin_hz,
            score: best.0,
        })
    }
}

/// The strongest band found by [`BandSearch::find`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// The centre of the best window, Hz from the centre of the spectrum.
    pub centre_hz: f64,
    /// The power-weighted centre of the bins above the floor inside the best window, Hz (`None` when nothing rises above it).
    pub centroid_hz: Option<f64>,
    /// The noise floor per bin: the floor quantile of the bins searched, times the floor factor.
    pub floor: f32,
    /// The mean power per bin in the best window (after the cap) over the floor.
    pub ratio: f32,
}

/// Places a coarse carrier search must not choose: centres within `half_width_hz` of `centre_hz`.
pub const MAX_SKIPS: usize = 16;

/// The band-power search: the strongest band of a given width in an averaged power spectrum.
///
/// The noise floor is a quantile of the bins searched (the median by default) times a factor. Each bin counts at most `cap` times
/// the floor, so a narrow spur cannot win against a wide signal. A window of the band's width slides over the span in steps of one
/// bin. Window centres near a place that never locked (at most [`MAX_SKIPS`], the newest kept) are skipped. The best window's
/// power centroid above the floor refines the centre. This is the coarse carrier search of digital voice (the strongest window of
/// a channel's width, the floor from the lower quartile) and of wide geostationary captures (the strongest band of the signal's
/// width, each bin capped at 4 times the median). Nothing is allocated after [`BandSearch::new`].
///
/// ```
/// use sdrkit_algo_modem::freq::BandSearch;
/// // 512 bins of 100 Hz on a floor of 1, a 5 kHz wide signal 4 times the floor at +6 kHz, and a strong narrow spur at -10 kHz.
/// let mut p = vec![1.0f32; 512];
/// for k in 0..50 { p[256 + 35 + k] += 4.0; }
/// p[156] = 1000.0;
/// let mut s = BandSearch::new(512, 5e3, 100.0).unwrap();
/// let b = s.find(&p, -20e3, 20e3).unwrap();
/// assert!((b.centre_hz - 6e3).abs() <= 100.0);
/// ```
#[derive(Clone, Debug)]
pub struct BandSearch {
    n: usize,
    width: usize,
    bin_hz: f64,
    quantile: f64,
    factor: f32,
    cap: Option<f32>,
    skips: Vec<(f64, f64)>,
    scratch: Vec<f32>,
    capped: Vec<f32>,
}

impl BandSearch {
    /// A search over spectra of `n` bins (4 to 2^20) of `bin_hz` for a band `width_hz` wide (at least one bin, at most the
    /// spectrum), with the floor at the median and each bin capped at 4 times the floor. Errors for values out of range.
    pub fn new(n: usize, width_hz: f64, bin_hz: f64) -> Result<Self, ModemError> {
        Param::Bins.check_count(n, 4, 1 << 20)?;
        Param::BinWidth.check_positive(bin_hz)?;
        Param::Width.check_finite(width_hz)?;
        let width = (width_hz / bin_hz).round() as usize;
        if width == 0 || width > n {
            return Param::Width.range(1.0, n as f64, width as f64);
        }
        Ok(BandSearch {
            n,
            width,
            bin_hz,
            quantile: 0.5,
            factor: 1.0,
            cap: Some(4.0),
            skips: Vec::with_capacity(MAX_SKIPS),
            scratch: Vec::with_capacity(n),
            capped: vec![0.0; n],
        })
    }

    /// The floor: the `quantile` (0 to 1) of the bins searched, times `factor` (positive). Errors for values out of range.
    pub fn set_floor(&mut self, quantile: f64, factor: f32) -> Result<(), ModemError> {
        Param::Quantile.check_in(quantile, 0.0, 1.0)?;
        Param::Factor.check_positive(factor)?;
        self.quantile = quantile;
        self.factor = factor;
        Ok(())
    }

    /// The cap of each bin, in multiples of the floor (`None`: no cap). Errors for a cap that is not positive.
    pub fn set_cap(&mut self, cap: Option<f32>) -> Result<(), ModemError> {
        if let Some(c) = cap {
            if !(c > 0.0 && c.is_finite()) {
                return Param::Cap.positive(c);
            }
        }
        self.cap = cap;
        Ok(())
    }

    /// Remembers a place not to choose again: window centres within `half_width_hz` of `centre_hz`. The oldest is forgotten when
    /// [`MAX_SKIPS`] are held. A value that is not finite is ignored.
    pub fn skip(&mut self, centre_hz: f64, half_width_hz: f64) {
        if !(centre_hz.is_finite() && half_width_hz.is_finite()) {
            return;
        }
        if self.skips.len() == MAX_SKIPS {
            self.skips.remove(0);
        }
        self.skips.push((centre_hz, half_width_hz.abs()));
    }

    /// Forgets every place to skip.
    pub fn clear_skips(&mut self) {
        self.skips.clear();
    }

    /// The band width, bins.
    pub fn width_bins(&self) -> usize {
        self.width
    }

    /// The strongest band whose centre lies in `lo_hz ..= hi_hz` (Hz from the centre of `power`, linear power, lowest frequency
    /// first). The floor comes from the bins the windows cover. `None` when no window fits the span or the spectrum has the wrong
    /// length, or every window is skipped.
    pub fn find(&mut self, power: &[f32], lo_hz: f64, hi_hz: f64) -> Option<Band> {
        if power.len() != self.n || lo_hz.is_nan() || hi_hz.is_nan() || lo_hz > hi_hz {
            return None;
        }
        let n = self.n as i64;
        let w = self.width as i64;
        let c0 = n / 2;
        // Window starts whose centres lie in the span, inside the spectrum.
        let half = (w - 1) as f64 / 2.0;
        let first = ((lo_hz / self.bin_hz + c0 as f64 - half).ceil() as i64).max(0);
        let last = ((hi_hz / self.bin_hz + c0 as f64 - half).floor() as i64).min(n - w);
        if first > last {
            return None;
        }
        let (a, b) = (first as usize, (last + w) as usize);
        self.scratch.clear();
        self.scratch.extend(
            power[a..b]
                .iter()
                .map(|&p| if p.is_finite() { p } else { 0.0 }),
        );
        let k = ((self.quantile * (self.scratch.len() - 1) as f64).round() as usize)
            .min(self.scratch.len() - 1);
        let (_, q, _) = self
            .scratch
            .select_nth_unstable_by(k, |x, y| x.total_cmp(y));
        let floor = (*q * self.factor).max(1e-30);
        let cap = self.cap.map_or(f32::INFINITY, |c| c * floor);
        for (c, &p) in self.capped[a..b].iter_mut().zip(&power[a..b]) {
            *c = if p.is_finite() { p.min(cap) } else { 0.0 };
        }
        let centre_of = |start: i64| (start as f64 + half - c0 as f64) * self.bin_hz;
        let mut sum: f64 = self.capped[a..a + self.width]
            .iter()
            .map(|&v| v as f64)
            .sum();
        let mut best: Option<(f64, i64)> = None;
        for start in first..=last {
            if start > first {
                let (out, inn) = ((start - 1) as usize, (start + w - 1) as usize);
                sum += self.capped[inn] as f64 - self.capped[out] as f64;
            }
            let c = centre_of(start);
            if self.skips.iter().any(|&(f, h)| (c - f).abs() <= h) {
                continue;
            }
            if best.is_none_or(|(v, _)| sum > v) {
                best = Some((sum, start));
            }
        }
        let (bsum, start) = best?;
        let s = start as usize;
        let centroid = crate::freq::centroid(&self.capped, s, s + self.width, floor)
            .map(|k| (k - c0 as f64) * self.bin_hz);
        Some(Band {
            centre_hz: centre_of(start),
            centroid_hz: centroid,
            floor,
            ratio: (bsum / self.width as f64) as f32 / floor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::crb_frequency;
    use sdrkit_algo_sim::rng::Rng;

    fn tone(n: usize, f: f64, phase: f64, snr: f64, rng: &mut Rng) -> Vec<Complex32> {
        (0..n)
            .map(|k| {
                let p = TAU * f * k as f64 + phase;
                Complex32::new(p.cos() as f32, p.sin() as f32) + rng.complex_normal(1.0 / snr)
            })
            .collect()
    }

    /// RMS error of an estimator over trials, divided by the Cramer-Rao bound.
    fn efficiency(
        mut est: impl FnMut(&[Complex32]) -> f64,
        n: usize,
        f_range: f64,
        snr_db: f64,
        seed: u64,
    ) -> f64 {
        let snr = 10f64.powf(snr_db / 10.0);
        let mut rng = Rng::new(seed);
        let trials = 400;
        let mut s2 = 0.0;
        for _ in 0..trials {
            let f = rng.range(-f_range, f_range);
            let ph = rng.range(0.0, TAU);
            let x = tone(n, f, ph, snr, &mut rng);
            let e = est(&x) - f;
            s2 += e * e;
        }
        (s2 / trials as f64).sqrt() / crb_frequency(snr, n)
    }

    #[test]
    fn fine_estimators_approach_the_cramer_rao_bound() {
        let n = 128;
        // Kay at 10 dB per sample: efficient.
        let k = efficiency(|x| kay(x).unwrap(), n, 0.2, 10.0, 1);
        assert!(k < 1.15, "kay {k}");
        // Fitz and L&R with M = N/2 near the bound, for frequencies inside their range.
        let m = n / 2;
        let fz = efficiency(|x| fitz(x, m).unwrap(), n, 0.2 / m as f64, 5.0, 2);
        assert!(fz < 1.3, "fitz {fz}");
        let lr = efficiency(
            |x| luise_reggiannini(x, m).unwrap(),
            n,
            0.5 / m as f64,
            5.0,
            3,
        );
        assert!(lr < 1.3, "l&r {lr}");
        // The FFT line with Candan's three-bin refinement (one frame of n): within 1.8 of the bound at 0 dB per sample (three
        // bins carry less than the whole frame, so some efficiency is lost between bins).
        let mut pl = PowerLine::new(1, n).unwrap();
        let c = efficiency(|x| pl.estimate(x).unwrap().freq, n, 0.4, 0.0, 4);
        assert!(c < 1.8, "candan {c}");
        // Multi-lag: lags 1, 4, 16 reach about the precision of lag 16 with the range of lag 1.
        let ml = efficiency(|x| multi_lag(x, &[1, 4, 16]).unwrap(), n, 0.4, 15.0, 5);
        assert!(ml < 3.0, "multi lag {ml}");
    }

    #[test]
    fn power_lines_remove_psk_and_find_msk_pairs() {
        let mut rng = Rng::new(7);
        let f = -0.0371;
        for (m, set) in [(2u32, 2usize), (4, 4), (8, 8)] {
            let x: Vec<Complex32> = (0..4096)
                .map(|k| {
                    let d = rng.below(set as u64) as f64 * TAU / set as f64;
                    let p = TAU * f * k as f64 + d + 0.3;
                    Complex32::new(p.cos() as f32, p.sin() as f32) + rng.complex_normal(0.05)
                })
                .collect();
            let mut pl = PowerLine::new(m, 4096).unwrap();
            let l = pl.estimate(&x).unwrap();
            assert!((l.freq - f).abs() < 2e-5, "m {m}: {}", l.freq);
            assert!(l.snr_db > 20.0);
            // Averaged frames (8 x 512): log-parabolic refinement.
            let mut pl = PowerLine::new(m, 512).unwrap();
            let l = pl.estimate(&x).unwrap();
            assert!((l.freq - f).abs() < 2e-4, "m {m} averaged: {}", l.freq);
        }
        // MSK: phase moves +-pi/2 per symbol; x^2 has lines at 2 fc +- Rs/2.
        let (sps, fc) = (8usize, 0.0123);
        let mut x = vec![Complex32::default(); 8192];
        let bits: Vec<f64> = (0..8192 / sps + 1)
            .map(|_| if rng.bit() == 1 { 1.0 } else { -1.0 })
            .collect();
        let mut phase = 0.3;
        for (k, v) in x.iter_mut().enumerate() {
            phase += bits[k / sps] * std::f64::consts::FRAC_PI_2 / sps as f64 + TAU * fc;
            *v = Complex32::new(phase.cos() as f32, phase.sin() as f32);
        }
        let mut pl = PowerLine::new(2, 8192).unwrap();
        let p = pl.estimate_pair(&x, 1.0 / sps as f64).unwrap();
        assert!((p.freq - fc).abs() < 1e-4, "{}", p.freq);
        assert!((p.spacing - 1.0 / sps as f64).abs() < 1e-4, "{}", p.spacing);
    }

    #[test]
    fn spans_rejects_and_errors() {
        let mut rng = Rng::new(9);
        // Two tones: the stronger at 0.1 is rejected, so the weaker at -0.05 is found.
        let x: Vec<Complex32> = (0..1024)
            .map(|k| {
                let a = TAU * 0.1 * k as f64;
                let b = TAU * -0.05 * k as f64;
                Complex32::new(a.cos() as f32, a.sin() as f32) * 2.0
                    + Complex32::new(b.cos() as f32, b.sin() as f32)
                    + rng.complex_normal(0.01)
            })
            .collect();
        let mut pl = PowerLine::new(1, 1024).unwrap();
        assert!((pl.estimate(&x).unwrap().freq - 0.1).abs() < 1e-4);
        pl.set_reject(&[0.1], 0.01).unwrap();
        assert!((pl.estimate(&x).unwrap().freq + 0.05).abs() < 1e-4);
        pl.set_reject(&[], 0.0).unwrap();
        pl.set_span(0.07).unwrap();
        assert!((pl.estimate(&x).unwrap().freq + 0.05).abs() < 1e-4);
        assert!(pl.set_span(0.6).is_err());
        assert!(pl.set_reject(&[0.0; 17], 0.1).is_err());
        assert!(PowerLine::new(0, 64).is_err());
        assert!(pl.estimate(&x[..2]).is_err());
        assert!(kay(&x[..2]).is_err());
        assert!(fitz(&x, 0).is_err());
        assert!(lag_phase(&[Complex32::new(f32::NAN, 0.0); 8], 1).is_err());
        assert!(multi_lag(&x, &[4, 2]).is_err());
        // Lag phase of a known tone.
        let t = tone(256, 0.01, 0.0, 1e9, &mut rng);
        assert!((lag_phase(&t, 16).unwrap() - 0.01).abs() < 1e-6);
        // Centroid of a symmetric bump.
        let p = [0.0, 0.0, 1.0, 3.0, 1.0, 0.0];
        assert!((centroid(&p, 0, 6, 0.0).unwrap() - 3.0).abs() < 1e-12);
        assert!(centroid(&p, 0, 6, 5.0).is_none());
    }

    /// The averaged power spectrum (lowest frequency first, DC at n / 2) of `x` in Hann-windowed frames of `n`.
    fn averaged(x: &[Complex32], n: usize) -> Vec<f32> {
        let mut w = sdrkit_algo_dsp::spectrum::Welch::new(
            n,
            sdrkit_algo_dsp::window::Window::Hann,
            0.0,
            sdrkit_algo_dsp::spectrum::Scaling::Power,
        )
        .unwrap();
        let mut p = vec![0.0; n];
        w.compute(x, &mut p).unwrap();
        p
    }

    /// QPSK symbols through an RRC pulse at `sps`, moved by `f` cycles per sample, plus noise of variance `n0`.
    fn rrc_qpsk(n: usize, sps: usize, beta: f64, f: f64, n0: f64, seed: u64) -> Vec<Complex32> {
        let mut r = Rng::new(seed);
        let h = sdrkit_algo_dsp::pulse::taps(
            sdrkit_algo_dsp::pulse::Shape::Rrc { beta },
            sps as f64,
            8.0,
            sdrkit_algo_dsp::pulse::Norm::Energy,
        )
        .unwrap();
        let mut up = vec![Complex32::default(); n + h.len()];
        for k in 0..n / sps {
            let s = std::f32::consts::FRAC_1_SQRT_2;
            up[k * sps] = Complex32::new(
                if r.bit() == 0 { s } else { -s },
                if r.bit() == 0 { s } else { -s },
            );
        }
        (0..n)
            .map(|i| {
                let mut v = Complex32::default();
                for (j, &t) in h.iter().enumerate() {
                    if i + j < up.len() && i + h.len() > j {
                        v += up[i + h.len() - 1 - j] * t;
                    }
                }
                let p = TAU * ((f * i as f64) % 1.0);
                v * Complex32::new(p.cos() as f32, p.sin() as f32) + r.complex_normal(n0)
            })
            .collect()
    }

    #[test]
    fn the_template_fit_finds_a_raised_cosine_signal() {
        // 72 ksym/s, roll-off 0.6, at 288 kHz (4 samples per symbol), 5.3 kHz high, Es/N0 about 10 dB: 1024-point spectra
        // averaged over 48 frames, as a LEO weather satellite search does.
        let rate = 288e3;
        let x = rrc_qpsk(1024 * 48, 4, 0.6, 5300.0 / rate, 0.1, 5);
        let p = averaged(&x, 1024);
        let bin = rate / 1024.0;
        let t = TemplateFit::new(bin, 72e3, 0.6, 1024, 15e3).unwrap();
        let got = t.fit(&p, 15e3).unwrap();
        assert!(
            (got.freq_hz - 5300.0).abs() < 0.5 * bin,
            "{} Hz",
            got.freq_hz
        );
        assert!(got.score > 0.0);
        // Out of the span the fit stays at its edge. A flat spectrum scores 0.
        let edge = t.fit(&p, 2e3).unwrap();
        assert!(edge.freq_hz <= 2e3 + bin);
        let flat = vec![1.0f32; 1024];
        assert!(t.fit(&flat, 15e3).unwrap().score.abs() < 1e-6);
        assert!(t.fit(&p[..100], 15e3).is_err());
        assert!(TemplateFit::new(bin, 72e3, 0.0, 1024, 15e3).is_err());
        assert!(TemplateFit::new(bin, 72e3, 0.6, 1024, 200e3).is_err());
    }

    #[test]
    fn the_band_search_finds_the_strongest_band_and_skips_bad_places() {
        // A 12.5 kHz channel's 4FSK-like signal 5 kHz wide at +7.2 kHz, a strong narrow spur at -3 kHz, at 48 kHz in 1024 bins.
        let rate = 48e3;
        let n = 1024;
        let bin = rate / n as f64;
        let mut r = Rng::new(9);
        let x: Vec<Complex32> = (0..n * 8)
            .map(|i| {
                let fsk =
                    7.2e3 + 1800.0 * [-1.0, -1.0 / 3.0, 1.0 / 3.0, 1.0][(i / 10 * 7 + i / 31) % 4];
                let p = TAU * ((fsk / rate * i as f64) % 1.0);
                let q = TAU * ((-3e3 / rate * i as f64) % 1.0);
                Complex32::new(p.cos() as f32, p.sin() as f32)
                    + Complex32::new(q.cos() as f32, q.sin() as f32) * 30.0
                    + r.complex_normal(0.5)
            })
            .collect();
        let p = averaged(&x, n);
        let mut s = BandSearch::new(n, 5e3, bin).unwrap();
        s.set_floor(0.25, 1.3).unwrap();
        let b = s.find(&p, -18e3, 18e3).unwrap();
        assert!((b.centre_hz - 7.2e3).abs() < 600.0, "{}", b.centre_hz);
        let c = b.centroid_hz.unwrap();
        assert!((c - 7.2e3).abs() < 300.0, "{c}");
        assert!(b.ratio > 2.0, "{}", b.ratio);
        // Without the cap the spur wins.
        s.set_cap(None).unwrap();
        let spur = s.find(&p, -18e3, 18e3).unwrap();
        assert!((spur.centre_hz + 3e3).abs() < 2.6e3, "{}", spur.centre_hz);
        // A place that never locked is skipped. The near range alone sees no signal beyond it.
        s.set_cap(Some(4.0)).unwrap();
        s.skip(7.2e3, 3e3);
        let other = s.find(&p, -18e3, 18e3).unwrap();
        assert!((other.centre_hz - 7.2e3).abs() > 3e3);
        s.clear_skips();
        let near = s.find(&p, -2e3, 2e3).unwrap();
        assert!(near.centre_hz.abs() <= 2e3 + bin);
        assert!(s.find(&p, 30e3, 40e3).is_none());
        assert!(s.find(&p[..10], -1e3, 1e3).is_none());
        assert!(BandSearch::new(n, 0.0, bin).is_err());
        assert!(s.set_floor(1.5, 1.0).is_err());
    }
}

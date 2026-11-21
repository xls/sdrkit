//! Decimators: a FIR decimator that computes only the outputs it keeps, a half-band decimator by 2, and a multi-stage [`Decimator`]
//! that reaches any integer factor through a chain of them.
//!
//! All of them append to a caller-owned `Vec` and produce exactly one output per `factor` inputs, counted across calls.

use crate::fir::{design_beta, kaiser_len, push_room, windowed_sinc, FirKernel, CHUNK};
use crate::window::kaiser;
use crate::{Append, Complex32, DspError, Param, Sample};
use std::f64::consts::PI;

/// Decimation by `factor` with a FIR filter, computing only the kept outputs (the cost per input is `taps / factor`).
///
/// It works on blocks: each call's input is appended once to a buffer that holds the last `taps - 1` samples, and every kept output
/// is one contiguous dot product over it, so there is no per-sample bookkeeping.
#[derive(Clone, Debug)]
pub struct FirDecimator<T: Sample> {
    /// The taps, oldest-sample first.
    kernel: FirKernel<T>,
    factor: usize,
    /// The last `taps - 1` samples, then the new input.
    buf: Vec<T>,
    /// The index in `buf` of the sample the next output is made at.
    due: usize,
}

impl<T: Sample> FirDecimator<T> {
    /// A decimator with these taps (`taps[0]` on the newest sample). The first output comes after `factor` inputs.
    pub fn new(taps: &[f32], factor: usize) -> Self {
        assert!(factor >= 1, "factor must be at least 1");
        assert!(!taps.is_empty(), "a FIR filter needs at least one tap");
        let rtaps: Vec<f32> = taps.iter().rev().copied().collect();
        let mut d = FirDecimator {
            kernel: FirKernel::new(&rtaps),
            factor,
            buf: Vec::new(),
            due: 0,
        };
        d.reset();
        d
    }

    /// A decimator whose output is flat to `pass_hz` and free of aliases below `alias_free_hz` (at least `pass_hz`), from a Kaiser lowpass
    /// with stopband edge `input_rate / factor - alias_free_hz`. Errors unless the factor is at least 1, the stop band edge is
    /// above `pass_hz` and the filter design is valid.
    pub fn design(
        input_rate: f64,
        factor: usize,
        pass_hz: f64,
        alias_free_hz: f64,
        atten_db: f64,
    ) -> Result<Self, DspError> {
        if factor == 0 {
            return Param::Decimation.at_least(1.0, 0.0);
        }
        if input_rate.is_nan() || input_rate <= 0.0 {
            return Param::InputRate.positive(input_rate);
        }
        let out = input_rate / factor as f64;
        let stop = (out - alias_free_hz).min(input_rate / 2.0);
        if !(stop > pass_hz && pass_hz >= 0.0) {
            return Param::Pass.range(0.0, stop, pass_hz);
        }
        let n = kaiser_len(atten_db, (stop - pass_hz) / input_rate)?;
        let h = windowed_sinc(
            n,
            (pass_hz + stop) / 2.0 / input_rate,
            design_beta(atten_db),
        )?;
        Ok(Self::new(
            &h.iter().map(|&v| v as f32).collect::<Vec<_>>(),
            factor,
        ))
    }

    /// The decimation factor.
    pub fn factor(&self) -> usize {
        self.factor
    }

    /// The filter delay in input samples.
    pub fn delay(&self) -> f64 {
        (self.kernel.len() - 1) as f64 / 2.0
    }

    /// The number of taps.
    pub fn num_taps(&self) -> usize {
        self.kernel.len()
    }

    /// Takes the history and the output phase of another decimator with the same number of taps and factor (to continue its stream
    /// without a transient). Allocates nothing once the history has its size.
    ///
    /// # Panics
    /// If the tap count or the factor differ.
    pub fn copy_state_from(&mut self, other: &FirDecimator<T>) {
        assert!(
            self.kernel.len() == other.kernel.len() && self.factor == other.factor,
            "different decimators"
        );
        self.buf.clone_from(&other.buf);
        self.due = other.due;
    }

    /// Clears the history and the output phase.
    pub fn reset(&mut self) {
        let n = self.kernel.len();
        self.buf.clear();
        self.buf.reserve(n - 1 + push_room(n));
        self.buf.resize(n - 1, T::default());
        self.due = n - 1 + self.factor - 1;
    }

    /// Takes one input sample and returns an output every `factor` inputs (counted across calls, and together with [`process`]
    /// calls: the two can be mixed freely and give the same stream). Convenient inside symbol loops; [`process`] is faster for
    /// blocks. Allocates nothing.
    ///
    /// [`process`]: FirDecimator::process
    #[inline]
    pub fn push(&mut self, x: T) -> Option<T> {
        let n = self.kernel.len();
        if self.buf.len() >= n - 1 + push_room(n) {
            // Keep the last n - 1 samples (amortised: one move of n - 1 samples per `push_room` inputs).
            let drop = self.buf.len() - (n - 1);
            self.buf.copy_within(drop.., 0);
            self.buf.truncate(n - 1);
            self.due -= drop;
        }
        self.buf.push(x);
        if self.due + 1 != self.buf.len() {
            return None;
        }
        let y = self.kernel.one(&self.buf[self.due + 1 - n..=self.due]);
        self.due += self.factor;
        Some(y)
    }

    /// Appends the outputs for `input` to `out`.
    pub fn process(&mut self, input: &[T], out: &mut impl Append<T>) {
        let n = self.kernel.len();
        self.buf.extend_from_slice(input);
        let len = self.buf.len();
        if self.due < len {
            out.reserve((len - self.due).div_ceil(self.factor));
        }
        let mut chunk = [T::default(); CHUNK];
        while self.due < len {
            let k = (len - self.due).div_ceil(self.factor).min(CHUNK);
            self.kernel
                .run(&self.buf[self.due + 1 - n..], self.factor, &mut chunk[..k]);
            out.extend_from_slice(&chunk[..k]);
            self.due += k * self.factor;
        }
        let drop = len - (n - 1);
        self.buf.drain(..drop);
        self.due -= drop;
    }
}

/// Decimation by `factor` with a FIR filter of **complex** taps on complex samples (for example a [`crate::fir::bandpass_complex`]
/// design, the SSB band-pass, or a matched filter for a carrier off centre), computing only the kept outputs.
///
/// The taps are kept split into real and imaginary parts, each laid out per lane, so each output is two plain `f32` dot products that
/// vectorise: `y = (sum hr x) + j (sum hi x)` over the window.
///
/// ```
/// use sdrkit_algo_dsp::decim::ComplexDecimator;
/// use sdrkit_algo_dsp::Complex32;
/// let taps = [Complex32::new(0.5, 0.0), Complex32::new(0.0, 0.5)];
/// let mut d = ComplexDecimator::new(&taps, 2).unwrap();
/// let mut out = Vec::new();
/// d.process(&[Complex32::new(1.0, 0.0); 4], &mut out);
/// assert_eq!(out, vec![Complex32::new(0.5, 0.5); 2]);
/// ```
#[derive(Clone, Debug)]
pub struct ComplexDecimator {
    /// Real parts of the taps, oldest-sample first.
    re: FirKernel<Complex32>,
    /// Imaginary parts, the same way.
    im: FirKernel<Complex32>,
    factor: usize,
    buf: Vec<Complex32>,
    due: usize,
}

impl ComplexDecimator {
    /// A decimator with these taps (`taps[0]` on the newest sample). The first output comes after `factor` inputs. Errors for no taps,
    /// a factor of 0, or a tap that is not finite.
    pub fn new(taps: &[Complex32], factor: usize) -> Result<Self, DspError> {
        if taps.is_empty() {
            return Param::Taps.at_least(1.0, 0.0);
        }
        if factor == 0 {
            return Param::Decimation.at_least(1.0, 0.0);
        }
        if taps.iter().any(|t| !(t.re.is_finite() && t.im.is_finite())) {
            return Param::Taps.not_finite();
        }
        let re: Vec<f32> = taps.iter().rev().map(|t| t.re).collect();
        let im: Vec<f32> = taps.iter().rev().map(|t| t.im).collect();
        let mut d = ComplexDecimator {
            re: FirKernel::new(&re),
            im: FirKernel::new(&im),
            factor,
            buf: Vec::new(),
            due: 0,
        };
        d.reset();
        Ok(d)
    }

    /// The decimation factor.
    pub fn factor(&self) -> usize {
        self.factor
    }

    /// The number of taps.
    pub fn num_taps(&self) -> usize {
        self.re.len()
    }

    /// The delay of a linear-phase design in input samples, `(taps - 1) / 2`.
    pub fn delay(&self) -> f64 {
        (self.num_taps() - 1) as f64 / 2.0
    }

    /// Clears the history and the output phase.
    pub fn reset(&mut self) {
        let n = self.num_taps();
        self.buf.clear();
        self.buf.reserve(n - 1 + push_room(n));
        self.buf.resize(n - 1, Complex32::default());
        self.due = n - 1 + self.factor - 1;
    }

    #[inline]
    fn output_at(&self, due: usize) -> Complex32 {
        let n = self.num_taps();
        let w = &self.buf[due + 1 - n..=due];
        join(self.re.one(w), self.im.one(w))
    }

    /// Takes one input sample; returns an output every `factor` inputs (mixes freely with [`process`](Self::process)).
    #[inline]
    pub fn push(&mut self, x: Complex32) -> Option<Complex32> {
        let n = self.num_taps();
        if self.buf.len() >= n - 1 + push_room(n) {
            let drop = self.buf.len() - (n - 1);
            self.buf.copy_within(drop.., 0);
            self.buf.truncate(n - 1);
            self.due -= drop;
        }
        self.buf.push(x);
        if self.due + 1 != self.buf.len() {
            return None;
        }
        let y = self.output_at(self.due);
        self.due += self.factor;
        Some(y)
    }

    /// Appends the outputs for `input` to `out`.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        let n = self.num_taps();
        self.buf.extend_from_slice(input);
        let len = self.buf.len();
        if self.due < len {
            out.reserve((len - self.due).div_ceil(self.factor));
        }
        let mut a = [Complex32::default(); CHUNK];
        let mut b = [Complex32::default(); CHUNK];
        while self.due < len {
            let k = (len - self.due).div_ceil(self.factor).min(CHUNK);
            let x = &self.buf[self.due + 1 - n..];
            self.re.run(x, self.factor, &mut a[..k]);
            self.im.run(x, self.factor, &mut b[..k]);
            out.extend(a[..k].iter().zip(&b[..k]).map(|(&a, &b)| join(a, b)));
            self.due += k * self.factor;
        }
        let drop = len - (n - 1);
        self.buf.drain(..drop);
        self.due -= drop;
    }
}

/// `y = a + j b`, with `a` the input filtered by the real parts of complex taps and `b` by their imaginary parts.
#[inline]
fn join(a: Complex32, b: Complex32) -> Complex32 {
    Complex32::new(a.re - b.im, a.im + b.re)
}

/// The non-zero taps of a half-band lowpass of length `4k - 1`: the `2k` taps at even positions (the centre tap, 0.5, is not included).
/// They sum to 0.5, so the filter has unity DC gain.
pub fn halfband_taps(k: usize, beta: f64) -> Vec<f32> {
    assert!(k >= 1);
    let n = 4 * k - 1;
    let w = kaiser(n, beta);
    let c = (2 * k - 1) as f64;
    let mut g: Vec<f64> = (0..2 * k)
        .map(|j| {
            let m = 2.0 * j as f64 - c;
            (PI * m / 2.0).sin() / (PI * m) * w[2 * j]
        })
        .collect();
    let s: f64 = g.iter().sum();
    g.iter_mut().for_each(|v| *v *= 0.5 / s);
    g.into_iter().map(|v| v as f32).collect()
}

/// A half-band decimator by 2. Half of a half-band filter's taps are zero and the centre one is 0.5, so each output costs `2k`
/// multiplies for a `4k - 1` tap filter. Passband ripple and stopband attenuation are the same `10^(-atten/20)`.
#[derive(Clone, Debug)]
pub struct HalfBand<T: Sample> {
    g: Vec<f32>,
    /// Second sample of each pair, through `g` (double buffer of 2 * 2k).
    hist: Vec<T>,
    hidx: usize,
    /// First sample of each pair, delayed by k - 1 pairs.
    delay: Vec<T>,
    didx: usize,
    odd: bool,
}

impl<T: Sample> HalfBand<T> {
    /// A half-band decimator with `4k - 1` taps.
    pub fn new(k: usize, beta: f64) -> Self {
        let g = halfband_taps(k, beta);
        HalfBand {
            hist: vec![T::default(); 2 * g.len()],
            g,
            hidx: 0,
            delay: vec![T::default(); k],
            didx: 0,
            odd: false,
        }
    }

    /// The `k` (taps `4k - 1`) for a half-band whose passband edge is `pass_frac` of the input rate. Errors unless `pass_frac` is
    /// in `(0, 0.25)` and `atten_db` is finite.
    pub fn k_for(pass_frac: f64, atten_db: f64) -> Result<usize, DspError> {
        if !(pass_frac > 0.0 && pass_frac < 0.25) {
            return Param::Pass.range(0.0, 0.25, pass_frac);
        }
        let n = kaiser_len(atten_db, 0.5 - 2.0 * pass_frac)?;
        Ok((n + 1).div_ceil(4))
    }

    /// A half-band flat to `pass_hz` at `input_rate`; it is then alias-free below `pass_hz` at the output.
    /// Errors as [`HalfBand::k_for`] does.
    pub fn design(input_rate: f64, pass_hz: f64, atten_db: f64) -> Result<Self, DspError> {
        Ok(Self::new(
            Self::k_for(pass_hz / input_rate, atten_db)?,
            design_beta(atten_db),
        ))
    }

    /// The number of taps of the full filter.
    pub fn num_taps(&self) -> usize {
        2 * self.g.len() - 1
    }

    /// The filter delay in input samples.
    pub fn delay(&self) -> f64 {
        (self.num_taps() - 1) as f64 / 2.0
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.hist.iter_mut().for_each(|v| *v = T::default());
        self.delay.iter_mut().for_each(|v| *v = T::default());
        self.hidx = 0;
        self.didx = 0;
        self.odd = false;
    }

    /// Appends one output per two inputs to `out`.
    pub fn process(&mut self, input: &[T], out: &mut impl Append<T>) {
        let n = self.g.len();
        let k = self.delay.len();
        out.reserve(input.len().div_ceil(2));
        for &x in input {
            if !self.odd {
                self.delay[self.didx] = x;
                self.odd = true;
            } else {
                self.hist[self.hidx] = x;
                self.hist[self.hidx + n] = x;
                self.hidx += 1;
                if self.hidx == n {
                    self.hidx = 0;
                }
                self.didx += 1;
                if self.didx == k {
                    self.didx = 0;
                }
                let centre = self.delay[self.didx];
                let a = T::dot(&self.g, &self.hist[self.hidx..self.hidx + n]);
                out.push(a + centre * 0.5);
                self.odd = false;
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Stage<T: Sample> {
    Half(HalfBand<T>),
    Fir(FirDecimator<T>),
}

impl<T: Sample> Stage<T> {
    fn process(&mut self, input: &[T], out: &mut impl Append<T>) {
        match self {
            Stage::Half(h) => h.process(input, out),
            Stage::Fir(f) => f.process(input, out),
        }
    }
    fn reset(&mut self) {
        match self {
            Stage::Half(h) => h.reset(),
            Stage::Fir(f) => f.reset(),
        }
    }
    fn delay(&self) -> f64 {
        match self {
            Stage::Half(h) => h.delay(),
            Stage::Fir(f) => f.delay(),
        }
    }
    fn factor(&self) -> usize {
        match self {
            Stage::Half(_) => 2,
            Stage::Fir(f) => f.factor(),
        }
    }
}

/// The prime factors of `n`: the 2s first, then the odd ones from largest to smallest. This is the stage order [`Decimator`] uses.
pub fn stage_factors(mut n: usize) -> Vec<usize> {
    let mut twos = Vec::new();
    while n.is_multiple_of(2) && n > 1 {
        twos.push(2);
        n /= 2;
    }
    let mut odd = Vec::new();
    let mut p = 3;
    while n > 1 {
        if p * p > n {
            odd.push(n);
            break;
        }
        while n.is_multiple_of(p) {
            odd.push(p);
            n /= p;
        }
        p += 2;
    }
    odd.sort_unstable_by(|a, b| b.cmp(a));
    twos.extend(odd);
    twos
}

/// Decimation by any integer factor through a chain of stages: a half-band for every factor of 2, then a FIR decimator for each odd prime
/// factor. Each stage keeps `0..pass_hz` flat and free of aliases below `alias_free_hz`, so early stages (at high rates, with wide
/// transition bands) are short and the sharp filtering happens at the lowest rate.
#[derive(Clone, Debug)]
pub struct Decimator<T: Sample> {
    stages: Vec<Stage<T>>,
    bufs: Vec<Vec<T>>,
    factor: usize,
    input_rate: f64,
}

impl<T: Sample> Decimator<T> {
    /// A decimator flat to `pass_hz` and alias-free below it (aliases may land in `pass_hz .. output_rate/2`).
    pub fn new(input_rate: f64, factor: usize, pass_hz: f64, atten_db: f64) -> Self {
        Self::with_alias_free(input_rate, factor, pass_hz, pass_hz, atten_db)
    }

    /// A decimator flat to `pass_hz` and alias-free below `alias_free_hz`; `alias_free_hz = output_rate / 2` produces an output with no
    /// aliases at all.
    ///
    /// # Panics
    /// Unless `0 < pass_hz <= alias_free_hz < output_rate - pass_hz`, `factor >= 1` and `atten_db` is finite.
    pub fn with_alias_free(
        input_rate: f64,
        factor: usize,
        pass_hz: f64,
        alias_free_hz: f64,
        atten_db: f64,
    ) -> Self {
        assert!(factor >= 1, "factor must be at least 1");
        let out = input_rate / factor as f64;
        assert!(
            pass_hz > 0.0 && alias_free_hz >= pass_hz && alias_free_hz < out - pass_hz + 1e-9 * out && pass_hz < out / 2.0,
            "pass {pass_hz} Hz / alias-free {alias_free_hz} Hz do not fit an output rate of {out} Hz"
        );
        assert!(atten_db.is_finite(), "the attenuation must be finite");
        let mut stages = Vec::new();
        let mut rate = input_rate;
        for f in stage_factors(factor) {
            // A half-band is symmetric about a quarter of its rate, so it can keep aliases out only below that; a stage that must
            // keep them out up to the output's Nyquist (or close) gets a general FIR instead.
            if f == 2 && alias_free_hz < 0.24 * rate {
                stages.push(Stage::Half(
                    HalfBand::design(rate, alias_free_hz, atten_db).expect("checked above"),
                ));
            } else {
                stages.push(Stage::Fir(
                    FirDecimator::design(rate, f, pass_hz, alias_free_hz, atten_db)
                        .expect("checked above"),
                ));
            }
            rate /= f as f64;
        }
        let n = stages.len().saturating_sub(1);
        Decimator {
            stages,
            bufs: vec![Vec::new(); n],
            factor,
            input_rate,
        }
    }

    /// The total factor.
    pub fn factor(&self) -> usize {
        self.factor
    }

    /// The output rate.
    pub fn output_rate(&self) -> f64 {
        self.input_rate / self.factor as f64
    }

    /// The factor of each stage, in order.
    pub fn stage_factors(&self) -> Vec<usize> {
        self.stages.iter().map(|s| s.factor()).collect()
    }

    /// The delay through all stages, in input samples.
    pub fn delay(&self) -> f64 {
        let mut d = 0.0;
        let mut m = 1.0;
        for s in &self.stages {
            d += s.delay() * m;
            m *= s.factor() as f64;
        }
        d
    }

    /// The delay of the outputs from their place on the input clock, in input samples: output `j` describes the input at sample
    /// `j * factor - output_delay()`. This is [`Decimator::delay`] less the lead each stage takes because it keeps the last input
    /// of each group of `R` (its output `j` comes with input `j R + R - 1`). Use it to give each output its time.
    pub fn output_delay(&self) -> f64 {
        let mut d = 0.0;
        let mut m = 1.0;
        for s in &self.stages {
            let r = s.factor() as f64;
            d += (s.delay() - (r - 1.0)) * m;
            m *= r;
        }
        d
    }

    /// Clears all stages.
    pub fn reset(&mut self) {
        self.stages.iter_mut().for_each(|s| s.reset());
    }

    /// Appends the outputs for `input` to `out`.
    pub fn process(&mut self, input: &[T], out: &mut impl Append<T>) {
        let n = self.stages.len();
        if n == 0 {
            out.extend_from_slice(input);
            return;
        }
        for i in 0..n {
            let (done, rest) = self.bufs.split_at_mut(i);
            let src: &[T] = if i == 0 { input } else { &done[i - 1] };
            if i + 1 == n {
                self.stages[i].process(src, out);
            } else {
                let dst = &mut rest[0];
                dst.clear();
                // Room for the most this stage can give, so a later call with one more output does not grow the buffer.
                dst.reserve(src.len() / self.stages[i].factor() + 1);
                self.stages[i].process(src, dst);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use crate::Complex32;

    #[test]
    fn the_output_delay_returns_each_output_its_time() {
        // Output j describes input sample j * factor - output_delay().
        for (rate, factor) in [
            (48_000.0, 2usize),
            (48_000.0, 3),
            (240_000.0, 6),
            (240_000.0, 20),
            (1_200_000.0, 50),
        ] {
            let mut d = Decimator::<Complex32>::new(rate, factor, 800.0, 60.0);
            let out_d = d.output_delay();
            // A slow tone: its phase at an output names the input time that output describes.
            let f = 50.0;
            let n = (0.2 * rate) as usize;
            let x: Vec<Complex32> = (0..n)
                .map(|i| {
                    Complex32::from_polar(1.0, (std::f64::consts::TAU * f * i as f64 / rate) as f32)
                })
                .collect();
            let mut y = Vec::new();
            d.process(&x, &mut y);
            let k = y.len() * 3 / 4;
            let want = std::f64::consts::TAU * f * (k as f64 * factor as f64 - out_d) / rate;
            let err = (y[k].arg() as f64 - want).sin().asin();
            // The phase at 50 Hz names the time to 1/100 of an input sample.
            assert!(
                (err / (std::f64::consts::TAU * f) * rate).abs() < 0.01,
                "factor {factor}: {err}"
            );
            // The lead of each stage makes the output delay smaller than the filter delay.
            assert!(out_d < d.delay() || factor == 1);
        }
    }

    #[test]
    fn halfband_counts_and_response() {
        let rate = 96_000.0;
        let mut hb = HalfBand::<Complex32>::design(rate, 20_000.0, 70.0).unwrap();
        let mut out = Vec::new();
        let x = ctone(8193, 5000.0 / rate, 1.0);
        for c in x.chunks(3) {
            hb.process(c, &mut out);
        }
        assert_eq!(out.len(), 8193 / 2);
        let g = db(camp(&out[200..], 2.0 * 5000.0 / rate));
        assert!(g.abs() < 0.01, "pass gain {g}");
        // 30 kHz at 96 kHz is in the stopband (aliases to 18 kHz at 48 kHz).
        hb.reset();
        out.clear();
        hb.process(&ctone(8192, 30_000.0 / rate, 1.0), &mut out);
        let a = db(camp(&out[200..], 2.0 * (30_000.0 - 48_000.0) / rate));
        assert!(a < -70.0, "alias {a}");
    }

    #[test]
    fn fir_decimator_counts_across_blocks() {
        let mut d = FirDecimator::<f32>::design(48_000.0, 5, 3000.0, 3000.0, 60.0).unwrap();
        let mut out = Vec::new();
        let mut total = 0;
        for len in [1, 2, 3, 7, 100, 4, 999] {
            total += len;
            d.process(&vec![0.5; len], &mut out);
            assert_eq!(out.len(), total / 5);
        }
        assert!((out.last().unwrap() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn multistage_reaches_the_factor_and_keeps_the_tone() {
        let rate = 2.4e6;
        let mut d = Decimator::<Complex32>::new(rate, 50, 20_000.0, 80.0);
        assert_eq!(d.stage_factors(), vec![2, 5, 5]);
        assert_eq!(d.output_rate(), 48_000.0);
        let f = 7_000.0;
        let x = ctone(240_000, f / rate, 0.7);
        let mut out = Vec::new();
        for c in x.chunks(4097) {
            d.process(c, &mut out);
        }
        assert_eq!(out.len(), 4800);
        let a = camp(&out[800..], f / 48_000.0);
        assert!((db(a / 0.7)).abs() < 0.02, "gain {}", db(a / 0.7));
        // A tone that would alias onto 10 kHz (48 kHz + 10 kHz) is gone.
        d.reset();
        out.clear();
        d.process(&ctone(240_000, 58_000.0 / rate, 1.0), &mut out);
        assert!(db(camp(&out[800..], 10_000.0 / 48_000.0)) < -80.0);
    }

    #[test]
    fn push_returns_the_same_stream_as_process() {
        let taps = crate::fir::lowpass(1.0, 0.05, 0.15, 60.0).unwrap();
        let x = ctone(3000, 0.03, 1.0);
        let mut a = FirDecimator::<Complex32>::new(&taps, 3);
        let mut ya = Vec::new();
        a.process(&x, &mut ya);
        // Pushing one at a time, and mixing pushes with blocks, produces the same samples bit for bit.
        let mut b = FirDecimator::<Complex32>::new(&taps, 3);
        let mut yb = Vec::new();
        let mut i = 0;
        while i < x.len() {
            if (i / 500) % 2 == 0 {
                if let Some(y) = b.push(x[i]) {
                    yb.push(y);
                }
                i += 1;
            } else {
                let end = (i + 37).min(x.len());
                b.process(&x[i..end], &mut yb);
                i = end;
            }
        }
        assert_eq!(ya, yb);
        assert_eq!(ya.len(), 1000);
    }

    #[test]
    fn complex_decimator_matches_a_direct_complex_convolution() {
        let rate = 48_000.0;
        let taps = crate::fir::bandpass_complex(rate, 300.0, 2700.0, 400.0, 60.0).unwrap();
        let x = ctone(4000, 1000.0 / rate, 1.0)
            .iter()
            .zip(ctone(4000, -1500.0 / rate, 1.0))
            .map(|(a, b)| a + b)
            .collect::<Vec<_>>();
        let mut d = ComplexDecimator::new(&taps, 4).unwrap();
        let mut y = Vec::new();
        for c in x.chunks(333) {
            d.process(c, &mut y);
        }
        assert_eq!(y.len(), 1000);
        // Output k is made at input 4k + 3.
        for (k, v) in y.iter().enumerate().skip(100).step_by(37) {
            let n = 4 * k + 3;
            let mut want = Complex32::default();
            for (j, t) in taps.iter().enumerate() {
                if n >= j {
                    want += t * x[n - j];
                }
            }
            assert!((v - want).norm() < 1e-4, "{k}: {v} vs {want}");
        }
        // The upper sideband passes, the lower one is gone (each measured alone, so neither leaks into the other's measurement).
        let alone = |f: f64| {
            let mut d = ComplexDecimator::new(&taps, 4).unwrap();
            let mut y = Vec::new();
            d.process(&ctone(8000, f / rate, 1.0), &mut y);
            db(camp(&y[200..], 4.0 * f / rate))
        };
        let (pass, stop) = (alone(1000.0), alone(-1500.0));
        assert!(pass.abs() < 0.05 && stop < -60.0, "{pass} {stop}");
        // Push produces the same samples.
        let mut p = ComplexDecimator::new(&taps, 4).unwrap();
        let yp: Vec<Complex32> = x.iter().filter_map(|&v| p.push(v)).collect();
        assert_eq!(y, yp);
        assert!(ComplexDecimator::new(&[], 2).is_err());
        assert!(ComplexDecimator::new(&taps, 0).is_err());
    }

    #[test]
    fn factors() {
        assert_eq!(stage_factors(1), Vec::<usize>::new());
        assert_eq!(stage_factors(8), vec![2, 2, 2]);
        assert_eq!(stage_factors(90), vec![2, 5, 3, 3]);
        assert_eq!(stage_factors(37), vec![37]);
    }
}

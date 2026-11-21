//! IIR filters: biquads (RBJ cookbook), a one-pole lowpass, FM de-emphasis and pre-emphasis, and the complex forms (a biquad and a
//! one-pole on [`Complex32`] samples, with real or complex coefficients). State and coefficients are `f64`, so low corners at high
//! rates stay accurate.

use crate::{Complex32, DspError, Param};
use num_complex::Complex64;
use std::f64::consts::TAU;

/// Normalised biquad coefficients: `H(z) = (b0 + b1 z^-1 + b2 z^-2) / (1 + a1 z^-1 + a2 z^-2)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiquadCoeffs {
    /// Feed-forward.
    pub b0: f64,
    /// Feed-forward.
    pub b1: f64,
    /// Feed-forward.
    pub b2: f64,
    /// Feedback.
    pub a1: f64,
    /// Feedback.
    pub a2: f64,
}

impl BiquadCoeffs {
    fn norm(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    fn w0(rate: f64, f0: f64, q: f64) -> (f64, f64, f64) {
        assert!(
            f0 > 0.0 && f0 < rate / 2.0 && q > 0.0,
            "need 0 < f0 < rate/2 and q > 0"
        );
        let w0 = TAU * f0 / rate;
        let (s, c) = w0.sin_cos();
        (c, s, s / (2.0 * q))
    }

    /// Lowpass (`q` = 1/sqrt(2) for Butterworth).
    pub fn lowpass(rate: f64, f0: f64, q: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        Self::norm(
            (1.0 - c) / 2.0,
            1.0 - c,
            (1.0 - c) / 2.0,
            1.0 + a,
            -2.0 * c,
            1.0 - a,
        )
    }

    /// Highpass.
    pub fn highpass(rate: f64, f0: f64, q: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        Self::norm(
            (1.0 + c) / 2.0,
            -(1.0 + c),
            (1.0 + c) / 2.0,
            1.0 + a,
            -2.0 * c,
            1.0 - a,
        )
    }

    /// Band-pass with 0 dB gain at `f0`.
    pub fn bandpass(rate: f64, f0: f64, q: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        Self::norm(a, 0.0, -a, 1.0 + a, -2.0 * c, 1.0 - a)
    }

    /// Notch at `f0`.
    pub fn notch(rate: f64, f0: f64, q: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        Self::norm(1.0, -2.0 * c, 1.0, 1.0 + a, -2.0 * c, 1.0 - a)
    }

    /// All-pass (phase turns through 180 degrees at `f0`).
    pub fn allpass(rate: f64, f0: f64, q: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        Self::norm(1.0 - a, -2.0 * c, 1.0 + a, 1.0 + a, -2.0 * c, 1.0 - a)
    }

    /// Peaking EQ: `gain_db` at `f0`.
    pub fn peaking(rate: f64, f0: f64, q: f64, gain_db: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        let g = 10f64.powf(gain_db / 40.0);
        Self::norm(
            1.0 + a * g,
            -2.0 * c,
            1.0 - a * g,
            1.0 + a / g,
            -2.0 * c,
            1.0 - a / g,
        )
    }

    /// Low shelf: `gain_db` below `f0`.
    pub fn low_shelf(rate: f64, f0: f64, q: f64, gain_db: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        let g = 10f64.powf(gain_db / 40.0);
        let sq = 2.0 * g.sqrt() * a;
        Self::norm(
            g * ((g + 1.0) - (g - 1.0) * c + sq),
            2.0 * g * ((g - 1.0) - (g + 1.0) * c),
            g * ((g + 1.0) - (g - 1.0) * c - sq),
            (g + 1.0) + (g - 1.0) * c + sq,
            -2.0 * ((g - 1.0) + (g + 1.0) * c),
            (g + 1.0) + (g - 1.0) * c - sq,
        )
    }

    /// High shelf: `gain_db` above `f0`.
    pub fn high_shelf(rate: f64, f0: f64, q: f64, gain_db: f64) -> Self {
        let (c, _, a) = Self::w0(rate, f0, q);
        let g = 10f64.powf(gain_db / 40.0);
        let sq = 2.0 * g.sqrt() * a;
        Self::norm(
            g * ((g + 1.0) + (g - 1.0) * c + sq),
            -2.0 * g * ((g - 1.0) + (g + 1.0) * c),
            g * ((g + 1.0) + (g - 1.0) * c - sq),
            (g + 1.0) - (g - 1.0) * c + sq,
            2.0 * ((g - 1.0) - (g + 1.0) * c),
            (g + 1.0) - (g - 1.0) * c - sq,
        )
    }

    /// The complex response at `freq` Hz.
    pub fn response(&self, rate: f64, freq: f64) -> Complex64 {
        let z1 = Complex64::from_polar(1.0, -TAU * freq / rate);
        let z2 = z1 * z1;
        (self.b0 + z1 * self.b1 + z2 * self.b2) / (1.0 + z1 * self.a1 + z2 * self.a2)
    }
}

/// A streaming biquad (transposed direct form II) on `f32` samples.
#[derive(Clone, Debug)]
pub struct Biquad {
    c: BiquadCoeffs,
    s1: f64,
    s2: f64,
}

impl Biquad {
    /// A filter with these coefficients and zero state.
    pub fn new(c: BiquadCoeffs) -> Self {
        Biquad {
            c,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// The coefficients.
    pub fn coeffs(&self) -> &BiquadCoeffs {
        &self.c
    }

    /// Changes the coefficients, keeping the state (fine for gradual changes).
    pub fn set_coeffs(&mut self, c: BiquadCoeffs) {
        self.c = c;
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let c = &self.c;
        let y = c.b0 * x + self.s1;
        self.s1 = c.b1 * x - c.a1 * y + self.s2;
        self.s2 = c.b2 * x - c.a2 * y;
        y as f32
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        for v in buf {
            *v = self.process_sample(*v);
        }
    }
}

/// The Q of the two sections of a fourth-order Butterworth filter, 1 / (2 cos(pi/8)) and 1 / (2 cos(3 pi/8)), to four
/// decimal places.
pub const BUTTERWORTH4_Q: [f64; 2] = [0.5412, 1.3066];

/// A fourth-order Butterworth lowpass on `f32` samples: two [`Biquad`] sections with the Q of [`BUTTERWORTH4_Q`].
#[derive(Clone, Debug)]
pub struct Butterworth4 {
    a: Biquad,
    b: Biquad,
}

impl Butterworth4 {
    /// A lowpass at `rate` with cut-off `fc`.
    pub fn lowpass(rate: f64, fc: f64) -> Self {
        Butterworth4 {
            a: Biquad::new(BiquadCoeffs::lowpass(rate, fc, BUTTERWORTH4_Q[0])),
            b: Biquad::new(BiquadCoeffs::lowpass(rate, fc, BUTTERWORTH4_Q[1])),
        }
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        self.b.process_sample(self.a.process_sample(x))
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
    }
}

/// A one-pole lowpass `y += a (x - y)`, `a = 1 - e^{-2 pi fc / rate}`.
#[derive(Clone, Debug)]
pub struct OnePole {
    a: f64,
    y: f64,
}

impl OnePole {
    /// A lowpass with its corner at `cutoff_hz`.
    pub fn new(rate: f64, cutoff_hz: f64) -> Self {
        OnePole {
            a: Self::coeff(rate, cutoff_hz),
            y: 0.0,
        }
    }

    /// A lowpass with time constant `tau_s` seconds.
    pub fn with_time_constant(rate: f64, tau_s: f64) -> Self {
        OnePole {
            a: 1.0 - (-1.0 / (tau_s * rate)).exp(),
            y: 0.0,
        }
    }

    /// A smoother `y += a (x - y)` with the factor `a` given directly (for example 0.002 per sample). Errors unless `a` is in
    /// `(0, 1]`.
    ///
    /// ```
    /// use sdrkit_algo_dsp::iir::OnePole;
    /// let mut p = OnePole::with_factor(0.5).unwrap();
    /// assert_eq!(p.process_sample(1.0), 0.5);
    /// assert_eq!(p.process_sample(1.0), 0.75);
    /// ```
    pub fn with_factor(a: f64) -> Result<Self, DspError> {
        if !(a > 0.0 && a <= 1.0) {
            return Param::Smoothing.range(0.0, 1.0, a);
        }
        Ok(OnePole { a, y: 0.0 })
    }

    /// The factor `a` of `y += a (x - y)`.
    pub fn factor(&self) -> f64 {
        self.a
    }

    /// Back to 0.
    pub fn reset(&mut self) {
        self.y = 0.0;
    }

    fn coeff(rate: f64, fc: f64) -> f64 {
        1.0 - (-TAU * fc / rate).exp()
    }

    /// Changes the corner, keeping the state.
    pub fn set_cutoff(&mut self, rate: f64, cutoff_hz: f64) {
        self.a = Self::coeff(rate, cutoff_hz);
    }

    /// The current output.
    pub fn value(&self) -> f32 {
        self.y as f32
    }

    /// Sets the state (to start without a transient).
    pub fn set_value(&mut self, y: f32) {
        self.y = y as f64;
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        self.y += self.a * (x as f64 - self.y);
        self.y as f32
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        for v in buf {
            *v = self.process_sample(*v);
        }
    }
}

/// FM de-emphasis: the first-order lowpass `1 / (1 + j 2 pi f tau)` (50 us in Europe, 75 us in the Americas).
///
/// The digital filter is `g (1 - q z^-1) / (1 - p z^-1)` with unity gain at DC; the pole and zero are fitted (minimax, in dB) to the
/// analog curve over the audio band, up to 16 kHz or 0.45 of the rate. The worst error over that band is below 0.17 dB at 44.1, 48 and
/// 96 kHz, and below 0.05 dB at 250 kHz.
#[derive(Clone, Debug)]
pub struct Deemphasis {
    b0: f64,
    b1: f64,
    p: f64,
    x1: f64,
    y1: f64,
}

impl Deemphasis {
    /// De-emphasis with time constant `tau_s` (for example `50e-6`) at `rate` Hz.
    pub fn new(tau_s: f64, rate: f64) -> Self {
        assert!(tau_s > 0.0 && rate > 0.0);
        let (p, q, g) = Self::design(tau_s, rate);
        Deemphasis {
            b0: g,
            b1: -g * q,
            p,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// The analog curve in dB at `freq`.
    pub fn analog_db(tau_s: f64, freq: f64) -> f64 {
        let w = TAU * freq * tau_s;
        -10.0 * (1.0 + w * w).log10()
    }

    fn zero_for(p: f64, hn: f64) -> f64 {
        let r = hn * (1.0 + p) / (1.0 - p);
        (r - 1.0) / (r + 1.0)
    }

    /// The worst error in dB against the analog curve over the audio band (up to 16 kHz or 0.45 of the rate).
    fn worst_err(tau: f64, rate: f64, p: f64, q: f64) -> f64 {
        if !(p > 0.0 && p < 1.0 && q > -1.0 && q < 1.0) {
            return f64::MAX;
        }
        let g = (1.0 - p) / (1.0 - q);
        let fmax = (0.45 * rate).min(16_000.0);
        (1..=32)
            .map(|i| {
                let f = fmax * i as f64 / 32.0;
                let z = Complex64::from_polar(1.0, -TAU * f / rate);
                let h = g * (1.0 - q * z) / (1.0 - p * z);
                (20.0 * h.norm().log10() - Self::analog_db(tau, f)).abs()
            })
            .fold(0.0, f64::max)
    }

    /// Pattern search from `(p, q)`.
    fn refine(tau: f64, rate: f64, mut p: f64, mut q: f64) -> (f64, f64, f64) {
        let mut e = Self::worst_err(tau, rate, p, q);
        let mut step = 0.01;
        while step > 1e-7 {
            let mut improved = false;
            for (dp, dq) in [
                (1.0, 0.0),
                (-1.0, 0.0),
                (0.0, 1.0),
                (0.0, -1.0),
                (1.0, 1.0),
                (-1.0, -1.0),
                (1.0, -1.0),
                (-1.0, 1.0),
            ] {
                let (np, nq) = (p + dp * step, q + dq * step);
                let ne = Self::worst_err(tau, rate, np, nq);
                if ne < e {
                    (p, q, e) = (np, nq, ne);
                    improved = true;
                }
            }
            if !improved {
                step /= 2.0;
            }
        }
        (p, q, e)
    }

    /// Minimax fit of the pole `p` and zero `q` (unity DC gain): the best of a coarse grid and of the family that matches the analog
    /// magnitude at Nyquist, each refined by pattern search. Runs once at construction (well under a millisecond in release builds).
    fn design(tau: f64, rate: f64) -> (f64, f64, f64) {
        let hn = 10f64.powf(Self::analog_db(tau, rate / 2.0) / 20.0);
        let mut grid = (f64::MAX, 0.5, 0.0);
        for i in 1..50 {
            for j in -49..50 {
                let (p, q) = (i as f64 / 50.0, j as f64 / 50.0);
                let e = Self::worst_err(tau, rate, p, q);
                if e < grid.0 {
                    grid = (e, p, q);
                }
            }
        }
        let mut nyq = (f64::MAX, 0.5, 0.0);
        for i in 1..400 {
            let p = i as f64 / 400.0;
            let q = Self::zero_for(p, hn);
            let e = Self::worst_err(tau, rate, p, q);
            if e < nyq.0 {
                nyq = (e, p, q);
            }
        }
        let a = Self::refine(tau, rate, grid.1, grid.2);
        let b = Self::refine(tau, rate, nyq.1, nyq.2);
        let (p, q, _) = if a.2 <= b.2 { a } else { b };
        (p, q, (1.0 - p) / (1.0 - q))
    }

    /// De-emphasis by the bilinear transform of `1 / (1 + s tau)` with `s = 2 rate (1 - z^-1) / (1 + z^-1)`: `H(z) = g (1 + z^-1) /
    /// (1 - p z^-1)`, `p = (2 rate tau - 1) / (2 rate tau + 1)`, `g = (1 - p) / 2`. Unity gain at DC and a zero at Nyquist; the response
    /// at `f` is the analog one at the warped frequency `(rate / pi) tan(pi f / rate)`, which is close below about rate / 6. Errors
    /// unless `tau_s` and `rate` are positive and finite.
    pub fn bilinear(tau_s: f64, rate: f64) -> Result<Self, DspError> {
        Self::bilinear_k(tau_s, rate, 2.0 * rate)
    }

    /// De-emphasis by the bilinear transform pre-warped at the corner `1 / (2 pi tau)`, so the digital filter is exactly -3.01 dB
    /// there (`s = w_c / tan(w_c / (2 rate)) (1 - z^-1) / (1 + z^-1)`). Errors unless the corner is below Nyquist.
    pub fn bilinear_prewarped(tau_s: f64, rate: f64) -> Result<Self, DspError> {
        let wc = 1.0 / tau_s;
        if (wc / (2.0 * rate)).is_nan() || wc / (2.0 * rate) >= std::f64::consts::FRAC_PI_2 {
            return Param::Tau.conflict(Param::Rate);
        }
        Self::bilinear_k(tau_s, rate, wc / (wc / (2.0 * rate)).tan())
    }

    fn bilinear_k(tau_s: f64, rate: f64, k: f64) -> Result<Self, DspError> {
        if !(tau_s > 0.0 && tau_s.is_finite()) {
            return Param::Tau.positive(tau_s);
        }
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        if !k.is_finite() {
            return Param::Tau.conflict(Param::Rate);
        }
        let kt = k * tau_s;
        let p = (kt - 1.0) / (kt + 1.0);
        let g = 1.0 / (1.0 + kt);
        Ok(Deemphasis {
            b0: g,
            b1: g,
            p,
            x1: 0.0,
            y1: 0.0,
        })
    }

    /// The ITU-T J.17 de-emphasis at `rate` Hz: the exact inverse of [`PreEmphasis::j17`], with 0 dB at 2 kHz. Errors unless
    /// the rate is finite and above about 16.5 kHz.
    ///
    /// Standard: ITU-T J.17.
    pub fn j17(rate: f64) -> Result<Self, DspError> {
        let (b0, b1, a1) = j17_pre(rate)?;
        Ok(Deemphasis {
            b0: 1.0 / b0,
            b1: a1 / b0,
            p: -b1 / b0,
            x1: 0.0,
            y1: 0.0,
        })
    }

    /// The coefficients `(b0, b1, p)` of `y[n] = b0 x[n] + b1 x[n-1] + p y[n-1]`.
    pub fn coefficients(&self) -> (f64, f64, f64) {
        (self.b0, self.b1, self.p)
    }

    /// The response in dB at `freq`.
    pub fn response_db(&self, rate: f64, freq: f64) -> f64 {
        let z = Complex64::from_polar(1.0, -TAU * freq / rate);
        20.0 * ((self.b0 + self.b1 * z) / (1.0 - self.p * z))
            .norm()
            .log10()
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        self.process_f64(x as f64) as f32
    }

    /// One `f64` sample, for paths that keep full precision (for example 14-bit audio before it is rounded).
    #[inline]
    pub fn process_f64(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.p * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        for v in buf {
            *v = self.process_sample(*v);
        }
    }
}

/// The coefficients `(b0, b1, a1)` of the J.17 pre-emphasis `y[n] = b0 x[n] + b1 x[n-1] - a1 y[n-1]`, 0 dB at 2 kHz.
fn j17_pre(rate: f64) -> Result<(f64, f64, f64), DspError> {
    let (wz, wp) = (3000.0f64, 3000.0 * 75f64.sqrt());
    let k = 2.0 * rate;
    if !(rate.is_finite() && wp / k < std::f64::consts::FRAC_PI_2) {
        return Param::Rate.at_least(2.0 * wp / std::f64::consts::PI, rate);
    }
    let (z, p) = (k * (wz / k).tan(), k * (wp / k).tan());
    let (b0, b1, a0, a1) = (k + z, z - k, k + p, p - k);
    let e = Complex64::from_polar(1.0, -TAU * 2000.0 / rate);
    let g = ((b0 + b1 * e) / (a0 + a1 * e)).norm();
    Ok((b0 / (g * a0), b1 / (g * a0), a1 / a0))
}

/// FM pre-emphasis for generators: a first-order section `y[n] = b0 x[n] + b1 x[n-1] - a1 y[n-1]` that lifts the treble, so that
/// the chain is flat after the receiver's de-emphasis.
#[derive(Clone, Debug)]
pub struct PreEmphasis {
    b0: f64,
    b1: f64,
    a1: f64,
    x1: f64,
    y1: f64,
}

impl PreEmphasis {
    /// The exact inverse of [`Deemphasis::new`] for the same `tau_s` and `rate`: pre-emphasis followed by that de-emphasis is the
    /// identity (to rounding). Unity gain at DC.
    pub fn new(tau_s: f64, rate: f64) -> Result<Self, DspError> {
        if !(tau_s > 0.0 && tau_s.is_finite()) {
            return Param::Tau.positive(tau_s);
        }
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        Self::inverse_of(&Deemphasis::new(tau_s, rate))
    }

    /// The exact inverse of a de-emphasis filter. Errors when that inverse is not stable (the bilinear de-emphasis has its zero at
    /// Nyquist, so it has no stable inverse: use [`PreEmphasis::bilinear`]).
    pub fn inverse_of(d: &Deemphasis) -> Result<Self, DspError> {
        let (b0, b1, p) = d.coefficients();
        if !(b0.abs() > 0.0 && (b1 / b0).abs() < 1.0 - 1e-9) {
            return Param::Tau.degenerate();
        }
        Ok(PreEmphasis {
            b0: 1.0 / b0,
            b1: -p / b0,
            a1: b1 / b0,
            x1: 0.0,
            y1: 0.0,
        })
    }

    /// The bilinear transform of `(1 + s tau) / (1 + s tau2)`, `tau2 = 1 / (2 pi upper_hz)`: the pre-emphasis curve up to
    /// `upper_hz`, where a real transmitter's boost levels off. Errors unless `0 < 1 / (2 pi tau) < upper_hz`.
    pub fn bilinear(tau_s: f64, upper_hz: f64, rate: f64) -> Result<Self, DspError> {
        if tau_s <= 0.0 || tau_s.is_nan() {
            return Param::Tau.positive(tau_s);
        }
        if rate <= 0.0 || rate.is_nan() {
            return Param::Rate.positive(rate);
        }
        if !(upper_hz > 1.0 / (TAU * tau_s) && upper_hz.is_finite()) {
            return Param::UpperHz.at_least(1.0 / (TAU * tau_s), upper_hz);
        }
        let k = 2.0 * rate;
        let tau2 = 1.0 / (TAU * upper_hz);
        let d0 = 1.0 + k * tau2;
        Ok(PreEmphasis {
            b0: (1.0 + k * tau_s) / d0,
            b1: (1.0 - k * tau_s) / d0,
            a1: (1.0 - k * tau2) / d0,
            x1: 0.0,
            y1: 0.0,
        })
    }

    /// The ITU-T J.17 pre-emphasis at `rate` Hz, with 0 dB at 2 kHz (the gain of the standard's curve there). It is the bilinear
    /// transform of `(s + 3000) / (s + 3000 sqrt(75))`, pre-warped at the zero and at the pole. [`Deemphasis::j17`] is its exact
    /// inverse. Errors unless the rate is finite and above 2 sqrt(75) 3000 / pi (about 16.5 kHz).
    ///
    /// Standard: ITU-T J.17 (insertion loss `10 log10((75 + (w / 3000)^2) / (1 + (w / 3000)^2))` dB).
    ///
    /// ```
    /// use sdrkit_algo_dsp::iir::PreEmphasis;
    /// let pre = PreEmphasis::j17(32_000.0).unwrap();
    /// assert!(pre.response_db(32_000.0, 2_000.0).abs() < 1e-9);
    /// ```
    pub fn j17(rate: f64) -> Result<Self, DspError> {
        let (b0, b1, a1) = j17_pre(rate)?;
        Ok(PreEmphasis {
            b0,
            b1,
            a1,
            x1: 0.0,
            y1: 0.0,
        })
    }

    /// The analog ITU-T J.17 pre-emphasis curve in dB at `freq`, relative to its value at 2 kHz.
    pub fn j17_analog_db(freq: f64) -> f64 {
        let loss = |f: f64| {
            let r = (TAU * f / 3000.0).powi(2);
            10.0 * ((75.0 + r) / (1.0 + r)).log10()
        };
        loss(2000.0) - loss(freq)
    }

    /// The analog curve `(1 + j w tau) / (1 + j w tau2)` in dB at `freq` (`upper_hz` infinite for the plain `1 + j w tau`).
    pub fn analog_db(tau_s: f64, upper_hz: f64, freq: f64) -> f64 {
        let w = TAU * freq;
        let tau2 = if upper_hz.is_finite() {
            1.0 / (TAU * upper_hz)
        } else {
            0.0
        };
        10.0 * ((1.0 + (w * tau_s).powi(2)) / (1.0 + (w * tau2).powi(2))).log10()
    }

    /// The response in dB at `freq`.
    pub fn response_db(&self, rate: f64, freq: f64) -> f64 {
        let z = Complex64::from_polar(1.0, -TAU * freq / rate);
        20.0 * ((self.b0 + self.b1 * z) / (1.0 + self.a1 * z))
            .norm()
            .log10()
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        self.process_f64(x as f64) as f32
    }

    /// One `f64` sample, for paths that keep full precision.
    #[inline]
    pub fn process_f64(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        for v in buf {
            *v = self.process_sample(*v);
        }
    }
}

/// Biquad coefficients with complex values, `H(z) = (b0 + b1 z^-1 + b2 z^-2) / (1 + a1 z^-1 + a2 z^-2)`, for complex signals: a real
/// design used on I and Q together, or a real design moved in frequency (an asymmetric band-pass, such as one sideband).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComplexBiquadCoeffs {
    /// Feed-forward.
    pub b: [Complex64; 3],
    /// Feedback (`a[0]` is `a1`, `a[1]` is `a2`).
    pub a: [Complex64; 2],
}

impl ComplexBiquadCoeffs {
    /// A real design, unchanged.
    pub fn from_real(c: BiquadCoeffs) -> Self {
        let r = |v: f64| Complex64::new(v, 0.0);
        ComplexBiquadCoeffs {
            b: [r(c.b0), r(c.b1), r(c.b2)],
            a: [r(c.a1), r(c.a2)],
        }
    }

    /// A real design moved up by `shift_hz` (negative moves it down): `H'(f) = H(f - shift)`, by `b_k e^(j k w)`, `a_k e^(j k w)`.
    pub fn shifted(c: BiquadCoeffs, rate: f64, shift_hz: f64) -> Self {
        let w = TAU * shift_hz / rate;
        let e1 = Complex64::from_polar(1.0, w);
        let e2 = e1 * e1;
        ComplexBiquadCoeffs {
            b: [Complex64::new(c.b0, 0.0), e1 * c.b1, e2 * c.b2],
            a: [e1 * c.a1, e2 * c.a2],
        }
    }

    /// The complex response at `freq` Hz (negative frequencies differ from positive ones when the coefficients are complex).
    pub fn response(&self, rate: f64, freq: f64) -> Complex64 {
        let z1 = Complex64::from_polar(1.0, -TAU * freq / rate);
        let z2 = z1 * z1;
        (self.b[0] + z1 * self.b[1] + z2 * self.b[2]) / (1.0 + z1 * self.a[0] + z2 * self.a[1])
    }
}

/// A streaming biquad (transposed direct form II) on complex samples with complex coefficients.
#[derive(Clone, Debug)]
pub struct ComplexBiquad {
    c: ComplexBiquadCoeffs,
    s1: Complex64,
    s2: Complex64,
}

impl ComplexBiquad {
    /// A filter with these coefficients and zero state.
    pub fn new(c: ComplexBiquadCoeffs) -> Self {
        ComplexBiquad {
            c,
            s1: Complex64::default(),
            s2: Complex64::default(),
        }
    }

    /// A real design applied to I and Q.
    pub fn from_real(c: BiquadCoeffs) -> Self {
        Self::new(ComplexBiquadCoeffs::from_real(c))
    }

    /// The coefficients.
    pub fn coeffs(&self) -> &ComplexBiquadCoeffs {
        &self.c
    }

    /// Changes the coefficients, keeping the state.
    pub fn set_coeffs(&mut self, c: ComplexBiquadCoeffs) {
        self.c = c;
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.s1 = Complex64::default();
        self.s2 = Complex64::default();
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: Complex32) -> Complex32 {
        let x = Complex64::new(x.re as f64, x.im as f64);
        let c = &self.c;
        let y = c.b[0] * x + self.s1;
        self.s1 = c.b[1] * x - c.a[0] * y + self.s2;
        self.s2 = c.b[2] * x - c.a[1] * y;
        Complex32::new(y.re as f32, y.im as f32)
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for v in buf {
            *v = self.process_sample(*v);
        }
    }
}

/// A one-pole filter on complex samples, `y[n] = a y[n-1] + b x[n]` with a complex pole `a`: a lowpass (real pole) or a resonator
/// centred on any frequency, positive or negative (a single-sided band-pass, the cheapest tone or carrier tracker).
#[derive(Clone, Debug)]
pub struct ComplexOnePole {
    a: Complex64,
    b: f64,
    y: Complex64,
}

impl ComplexOnePole {
    /// A lowpass with its -3 dB corner close to `cutoff_hz` (`a = e^(-2 pi fc / rate)`, `b = 1 - a`, unity gain at DC). Errors unless
    /// `0 < cutoff_hz < rate / 2`.
    pub fn lowpass(rate: f64, cutoff_hz: f64) -> Result<Self, DspError> {
        Self::resonator(rate, 0.0, 2.0 * cutoff_hz)
    }

    /// A resonator at `centre_hz` with a -3 dB bandwidth close to `bandwidth_hz` (full width): pole `r e^(j 2 pi fc / rate)`,
    /// `r = e^(-pi bw / rate)`, and `b = 1 - r`, so the gain at the centre is exactly 1 with no phase shift. Errors unless
    /// `0 < bandwidth_hz < rate` and `|centre_hz| <= rate / 2`.
    pub fn resonator(rate: f64, centre_hz: f64, bandwidth_hz: f64) -> Result<Self, DspError> {
        if rate <= 0.0 || rate.is_nan() {
            return Param::Rate.positive(rate);
        }
        if !(bandwidth_hz > 0.0 && bandwidth_hz < rate) {
            return Param::Bandwidth.range(0.0, rate, bandwidth_hz);
        }
        if centre_hz.abs() > rate / 2.0 || centre_hz.is_nan() {
            return Param::CentreHz.range(-rate / 2.0, rate / 2.0, centre_hz);
        }
        let r = (-std::f64::consts::PI * bandwidth_hz / rate).exp();
        Ok(ComplexOnePole {
            a: Complex64::from_polar(r, TAU * centre_hz / rate),
            b: 1.0 - r,
            y: Complex64::default(),
        })
    }

    /// The response at `freq` Hz.
    pub fn response(&self, rate: f64, freq: f64) -> Complex64 {
        let z1 = Complex64::from_polar(1.0, -TAU * freq / rate);
        self.b / (1.0 - self.a * z1)
    }

    /// The current output.
    pub fn value(&self) -> Complex32 {
        Complex32::new(self.y.re as f32, self.y.im as f32)
    }

    /// Sets the state (to start without a transient).
    pub fn set_value(&mut self, y: Complex32) {
        self.y = Complex64::new(y.re as f64, y.im as f64);
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.y = Complex64::default();
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: Complex32) -> Complex32 {
        self.y = self.a * self.y + Complex64::new(x.re as f64, x.im as f64) * self.b;
        self.value()
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for v in buf {
            *v = self.process_sample(*v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn j17_follows_the_standard_curve_and_its_inverse_undoes_it() {
        let rate = 32_000.0;
        let pre = PreEmphasis::j17(rate).unwrap();
        // ITU-T J.17: 0 dB at 2 kHz, -9.50 dB at 400 Hz and -11.77 dB at DC relative to 2 kHz.
        assert!((PreEmphasis::j17_analog_db(400.0) + 9.50).abs() < 0.01);
        assert!((PreEmphasis::j17_analog_db(0.0) + 11.77).abs() < 0.01);
        // The bilinear transform moves the curve a little near the pole (about 4.1 kHz).
        for (f, tol) in [
            (0.0, 0.25),
            (400.0, 0.25),
            (1000.0, 0.25),
            (2000.0, 1e-9),
            (5000.0, 0.5),
        ] {
            let e = pre.response_db(rate, f) - PreEmphasis::j17_analog_db(f);
            assert!(e.abs() < tol, "{f} Hz: {e} dB");
        }
        let de = Deemphasis::j17(rate).unwrap();
        for f in [100.0, 1000.0, 5000.0, 12_000.0] {
            assert!((pre.response_db(rate, f) + de.response_db(rate, f)).abs() < 1e-9);
        }
        let (mut a, mut b) = (pre.clone(), de.clone());
        for i in 0..200 {
            let x = (i as f32 * 0.37).sin();
            assert!((b.process_sample(a.process_sample(x)) - x).abs() < 1e-5);
        }
        assert!(PreEmphasis::j17(8_000.0).is_err());
        assert!(Deemphasis::j17(f64::NAN).is_err());
    }

    #[test]
    fn bilinear_deemphasis_is_the_analog_curve_at_the_warped_frequency() {
        for tau in [50e-6, 75e-6, 530e-6] {
            for rate in [32_000.0, 48_000.0, 192_000.0] {
                let d = Deemphasis::bilinear(tau, rate).unwrap();
                for f in [100.0, 1000.0, 5000.0, 12_000.0] {
                    let fa = rate / std::f64::consts::PI * (std::f64::consts::PI * f / rate).tan();
                    let want = Deemphasis::analog_db(tau, fa);
                    assert!(
                        (d.response_db(rate, f) - want).abs() < 1e-9,
                        "{tau} {rate} {f}"
                    );
                }
                assert!(d.response_db(rate, 0.01).abs() < 1e-6);
                // Streaming matches the computed response.
                let mut s = d.clone();
                let g = measure(&mut |b| s.process(b), 3000.0, rate);
                assert!((g - d.response_db(rate, 3000.0)).abs() < 0.02);
                // Pre-warped: exactly -3.01 dB at the corner.
                let p = Deemphasis::bilinear_prewarped(tau, rate).unwrap();
                let fc = 1.0 / (TAU * tau);
                assert!(
                    (p.response_db(rate, fc) + 3.0103).abs() < 1e-3,
                    "{tau} {rate}"
                );
            }
        }
        assert!(Deemphasis::bilinear(0.0, 48e3).is_err());
        assert!(Deemphasis::bilinear(50e-6, f64::NAN).is_err());
    }

    #[test]
    fn pre_emphasis_undoes_de_emphasis() {
        let rate = 48_000.0;
        let mut pre = PreEmphasis::new(50e-6, rate).unwrap();
        let mut de = Deemphasis::new(50e-6, rate);
        let x: Vec<f32> = (0..2000)
            .map(|i| ((i * 37 % 101) as f32 / 50.0) - 1.0)
            .collect();
        let mut y = x.clone();
        pre.process(&mut y);
        de.process(&mut y);
        for (a, b) in x.iter().zip(&y) {
            assert!((a - b).abs() < 1e-5);
        }
        // The pre-emphasis follows 1 + j w tau over the band (as closely as the de-emphasis follows its curve).
        for f in [1000.0, 5000.0, 15_000.0] {
            let got = pre.response_db(rate, f);
            assert!(
                (got - PreEmphasis::analog_db(50e-6, f64::INFINITY, f)).abs() < 0.2,
                "{f}: {got}"
            );
        }
        // The bilinear form with an upper corner matches its curve at the warped frequency.
        let b = PreEmphasis::bilinear(75e-6, 20_000.0, 192_000.0).unwrap();
        for f in [500.0, 2122.0, 10_000.0] {
            let fa =
                192_000.0 / std::f64::consts::PI * (std::f64::consts::PI * f / 192_000.0).tan();
            assert!(
                (b.response_db(192_000.0, f) - PreEmphasis::analog_db(75e-6, 20_000.0, fa)).abs()
                    < 1e-9
            );
        }
        assert!(PreEmphasis::inverse_of(&Deemphasis::bilinear(50e-6, rate).unwrap()).is_err());
        assert!(PreEmphasis::bilinear(75e-6, 1000.0, rate).is_err());
    }

    #[test]
    fn complex_biquad_on_iq_is_the_real_one_on_each_part() {
        let rate = 48_000.0;
        let c = BiquadCoeffs::lowpass(rate, 2000.0, 0.707);
        let mut cb = ComplexBiquad::from_real(c);
        let (mut bi, mut bq) = (Biquad::new(c), Biquad::new(c));
        for i in 0..500 {
            let x = Complex32::new((i as f32 * 0.37).sin(), (i as f32 * 0.11).cos());
            let y = cb.process_sample(x);
            assert!((y.re - bi.process_sample(x.re)).abs() < 1e-6);
            assert!((y.im - bq.process_sample(x.im)).abs() < 1e-6);
        }
        // Shifted up by 3 kHz: a band-pass at +3 kHz that rejects -3 kHz.
        let lp = BiquadCoeffs::lowpass(rate, 1000.0, std::f64::consts::FRAC_1_SQRT_2);
        let s = ComplexBiquadCoeffs::shifted(lp, rate, 3000.0);
        for f in [-9000.0, -3000.0, 0.0, 2500.0, 3000.0, 4000.0, 11_000.0] {
            assert!(
                (s.response(rate, f) - lp.response(rate, f - 3000.0)).norm() < 1e-12,
                "{f}"
            );
        }
        assert!((s.response(rate, 4000.0).norm() - c_db(-3.0103)).abs() < 1e-3);
        assert!(s.response(rate, -3000.0).norm() < 0.03);
        let mut f = ComplexBiquad::new(s);
        let mut x = ctone(24_000, 3500.0 / rate, 1.0);
        f.process(&mut x);
        let got = camp(&x[12_000..], 3500.0 / rate);
        assert!((got - s.response(rate, 3500.0).norm()).abs() < 1e-3);
    }

    fn c_db(db: f64) -> f64 {
        10f64.powf(db / 20.0)
    }

    #[test]
    fn complex_one_pole_resonates_on_one_side() {
        let rate = 8000.0;
        let mut r = ComplexOnePole::resonator(rate, -1000.0, 50.0).unwrap();
        let g = r.response(rate, -1000.0);
        assert!((g.re - 1.0).abs() < 1e-12 && g.im.abs() < 1e-12);
        // -3 dB at +-25 Hz from the centre, within a few percent of the width.
        let edge = 20.0 * r.response(rate, -1025.0).norm().log10();
        assert!((edge + 3.01).abs() < 0.05, "{edge}");
        assert!(r.response(rate, 1000.0).norm() < 0.02);
        let mut x = ctone(16_000, -1000.0 / rate, 0.5);
        r.process(&mut x);
        assert!((x[15_999].norm() - 0.5).abs() < 1e-3);
        let lp = ComplexOnePole::lowpass(rate, 100.0).unwrap();
        assert!((lp.response(rate, 0.0).norm() - 1.0).abs() < 1e-12);
        assert!(ComplexOnePole::resonator(rate, 5000.0, 50.0).is_err());
        assert!(ComplexOnePole::lowpass(rate, 0.0).is_err());
    }

    fn measure(f: &mut impl FnMut(&mut [f32]), freq: f64, rate: f64) -> f64 {
        let mut x = rtone(24_000, freq / rate, 0.5);
        f(&mut x);
        db(ramp(&x[12_000..], freq / rate) / 0.5)
    }

    #[test]
    fn deemphasis_matches_the_analog_curve() {
        for tau in [50e-6, 75e-6] {
            for rate in [48_000.0, 44_100.0, 96_000.0, 250_000.0] {
                let d = Deemphasis::new(tau, rate);
                for f in [1000.0, 10_000.0] {
                    let want = Deemphasis::analog_db(tau, f);
                    let mut s = d.clone();
                    let got = measure(&mut |b| s.process(b), f, rate);
                    assert!(
                        (got - want).abs() < 0.2,
                        "tau {tau} rate {rate} f {f}: {got} vs {want}"
                    );
                    assert!((d.response_db(rate, f) - want).abs() < 0.2);
                }
                assert!(d.response_db(rate, 1.0).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn biquads_have_their_textbook_responses() {
        let rate = 48_000.0;
        let lp = BiquadCoeffs::lowpass(rate, 1000.0, std::f64::consts::FRAC_1_SQRT_2);
        assert!((20.0 * lp.response(rate, 1000.0).norm().log10() + 3.0103).abs() < 0.01);
        assert!(lp.response(rate, 1.0).norm() > 0.9999);
        let mut b = Biquad::new(lp);
        let g = measure(&mut |x| b.process(x), 8000.0, rate);
        assert!((g - 20.0 * lp.response(rate, 8000.0).norm().log10()).abs() < 0.05);
        let pk = BiquadCoeffs::peaking(rate, 2000.0, 1.0, 6.0);
        assert!((20.0 * pk.response(rate, 2000.0).norm().log10() - 6.0).abs() < 1e-6);
        let n = BiquadCoeffs::notch(rate, 3000.0, 5.0);
        assert!(n.response(rate, 3000.0).norm() < 1e-9);
        let bp = BiquadCoeffs::bandpass(rate, 3000.0, 5.0);
        assert!((bp.response(rate, 3000.0).norm() - 1.0).abs() < 1e-9);
        let ls = BiquadCoeffs::low_shelf(rate, 300.0, 0.707, -12.0);
        assert!((20.0 * ls.response(rate, 10.0).norm().log10() + 12.0).abs() < 0.1);
        let hs = BiquadCoeffs::high_shelf(rate, 3000.0, 0.707, 6.0);
        assert!((20.0 * hs.response(rate, 20_000.0).norm().log10() - 6.0).abs() < 0.2);
        let ap = BiquadCoeffs::allpass(rate, 3000.0, 1.0);
        assert!((ap.response(rate, 500.0).norm() - 1.0).abs() < 1e-9);
        let hp = BiquadCoeffs::highpass(rate, 100.0, 0.707);
        assert!(hp.response(rate, 10.0).norm() < 0.02);
    }

    #[test]
    fn one_pole_settles() {
        let mut lp = OnePole::new(48_000.0, 100.0);
        let mut x = vec![1.0f32; 4800];
        lp.process(&mut x);
        assert!((x[4799] - 1.0).abs() < 1e-3);
        // -3 dB near the corner (one-pole matched-z is close to analog well below Nyquist).
        let mut lp = OnePole::new(48_000.0, 100.0);
        let g = measure(&mut |b| lp.process(b), 100.0, 48_000.0);
        assert!((g + 3.0).abs() < 0.1, "{g}");
    }
}

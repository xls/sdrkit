//! Pulse shapes for modulators and matched filters: root raised cosine, raised cosine, the Gaussian frequency pulse of GMSK and
//! GFSK, the Gaussian filter itself, half-sine and rectangular, with the error function they need.
//!
//! Time `t` is in symbols, measured from the centre of the pulse. Each `fn` of `t` is the continuous pulse; [`taps`] samples one at
//! any (fractional) number of samples per symbol with a stated normalisation, and [`PulseTable`] tabulates one for a generator that
//! needs values at arbitrary instants without an `erf` or a `sin` per sample.
//!
//! ```
//! use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
//! // A unit-energy RRC matched filter, roll-off 0.35, 4 samples per symbol, 8 symbols long.
//! let h = taps(Shape::Rrc { beta: 0.35 }, 4.0, 8.0, Norm::Energy).unwrap();
//! assert_eq!(h.len(), 33);
//! let e: f32 = h.iter().map(|v| v * v).sum();
//! assert!((e - 1.0).abs() < 1e-5);
//! ```

use crate::{DspError, Param};
use std::f64::consts::{LN_2, PI};

/// `2 / sqrt(pi)`.
const TWO_OVER_SQRT_PI: f64 = std::f64::consts::FRAC_2_SQRT_PI;

/// The error function `erf(x) = 2/sqrt(pi) * integral from 0 to x of e^(-t^2) dt`, to within a few units in the last place of an
/// `f64` (within 2 units in the last place of the published values; relative error below 1e-15).
///
/// For `|x| < 2.5` it sums the series `erf(x) = 2/sqrt(pi) e^(-x^2) sum_n 2^n x^(2n+1) / (1 * 3 * ... * (2n+1))` (Abramowitz and
/// Stegun 7.1.6), whose terms are all positive, so nothing cancels. Beyond it uses `1 - erfc(x)` with the continued fraction of
/// [`erfc`]. NaN returns NaN.
pub fn erf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let a = x.abs();
    if a < 2.5 {
        erf_series(x)
    } else {
        (1.0 - erfc_cf(a)).copysign(x)
    }
}

/// The complementary error function `erfc(x) = 1 - erf(x)`, accurate in relative terms also far into the tail (where `1 - erf`
/// would lose every digit): `erfc(5) = 1.537e-12` to full precision.
///
/// For `x >= 2.5` it evaluates the continued fraction `erfc(x) = e^(-x^2)/sqrt(pi) * 1/(x + (1/2)/(x + 1/(x + (3/2)/(x + ...))))`
/// (Abramowitz and Stegun 7.1.14) by the modified Lentz method; below that it uses `1 - erf(x)`.
pub fn erfc(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x >= 2.5 {
        erfc_cf(x)
    } else if x <= -2.5 {
        2.0 - erfc_cf(-x)
    } else {
        1.0 - erf_series(x)
    }
}

/// The positive-term series of A&S 7.1.6, for `|x| < 2.5`.
fn erf_series(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    let mut k = 0.0;
    loop {
        k += 1.0;
        term *= 2.0 * x2 / (2.0 * k + 1.0);
        sum += term;
        if term.abs() <= sum.abs() * 1e-17 {
            break;
        }
    }
    TWO_OVER_SQRT_PI * (-x2).exp() * sum
}

/// `erfc(x)` for `x >= 2.5` by the continued fraction of A&S 7.1.14 (modified Lentz).
fn erfc_cf(x: f64) -> f64 {
    if x > 27.3 {
        return 0.0; // below the smallest f64
    }
    // erfc(x) = e^{-x^2}/sqrt(pi) / (x + a1/(x + a2/(x + ...))), a_k = k/2.
    let tiny = 1e-300;
    let mut f = x;
    let mut c = x;
    let mut d = 0.0;
    for k in 1..300 {
        let a = k as f64 / 2.0;
        d = x + a * d;
        d = if d.abs() < tiny { tiny } else { d };
        c = x + a / c;
        c = if c.abs() < tiny { tiny } else { c };
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x * x).exp() / (PI.sqrt() * f)
}

/// The root raised cosine pulse at `t` symbols with roll-off `beta` (0 to 1), in the textbook form with unit energy
/// (`integral of rrc^2 dt = 1` with `t` in symbols), so that `rrc * rrc = rc`. The singular points are handled: `rrc(0) = 1 - beta +
/// 4 beta / pi` and `rrc(+-1/(4 beta)) = beta/sqrt(2) ((1 + 2/pi) sin(pi/(4 beta)) + (1 - 2/pi) cos(pi/(4 beta)))`.
pub fn rrc(t: f64, beta: f64) -> f64 {
    if t.abs() < 1e-9 {
        return 1.0 - beta + 4.0 * beta / PI;
    }
    if beta > 0.0 && ((4.0 * beta * t).abs() - 1.0).abs() < 1e-9 {
        let q = PI / (4.0 * beta);
        return beta / 2f64.sqrt() * ((1.0 + 2.0 / PI) * q.sin() + (1.0 - 2.0 / PI) * q.cos());
    }
    ((PI * t * (1.0 - beta)).sin() + 4.0 * beta * t * (PI * t * (1.0 + beta)).cos())
        / (PI * t * (1.0 - (4.0 * beta * t).powi(2)))
}

/// `sin(pi x) / (pi x)`, 1 at 0.
pub fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// The raised cosine pulse at `t` symbols with roll-off `beta`: `sinc(t) cos(pi beta t) / (1 - (2 beta t)^2)`, 1 at 0 and 0 at every
/// other whole symbol (the Nyquist property). At `t = +-1/(2 beta)` it is `(pi/4) sinc(1/(2 beta))`.
pub fn rc(t: f64, beta: f64) -> f64 {
    if beta > 0.0 && ((2.0 * beta * t).abs() - 1.0).abs() < 1e-9 {
        return PI / 4.0 * sinc(1.0 / (2.0 * beta));
    }
    sinc(t) * (PI * beta * t).cos() / (1.0 - (2.0 * beta * t).powi(2))
}

/// The Gaussian frequency pulse of GMSK and GFSK for a bandwidth-time product `bt`: a one-symbol rectangle through a Gaussian
/// filter of 3 dB bandwidth `bt / T`, at `t` symbols from the symbol's centre, with unit area (`integral g dt = 1`):
///
/// `g(t) = (1/2) (erf(c bt (t + 1/2)) - erf(c bt (t - 1/2)))`, `c = pi sqrt(2 / ln 2)`.
///
/// 3GPP TS 45.004 clause 2.5 (GSM, BT 0.3) writes the same shape as `1/(2T) (Q(2 pi B (t - T/2) / sqrt(ln 2)) - Q(2 pi B (t + T/2) /
/// sqrt(ln 2)))`, which is half of this unit-area form; ITU-R M.1371 Annex 2 (AIS, BT 0.4) uses it too. A modulator advances the
/// phase by `pi h a_k g(t - k)` per symbol time for symbol `a_k` and modulation index `h` (so `pi h` per symbol in all).
pub fn gaussian_freq(t: f64, bt: f64) -> f64 {
    let c = PI * (2.0 / LN_2).sqrt() * bt;
    // Written as a difference of erfc on the far side, so the tails keep their relative precision.
    if t > 0.5 {
        0.5 * (erfc(c * (t - 0.5)) - erfc(c * (t + 0.5)))
    } else if t < -0.5 {
        0.5 * (erfc(-c * (t + 0.5)) - erfc(-c * (t - 0.5)))
    } else {
        0.5 * (erf(c * (t + 0.5)) - erf(c * (t - 0.5)))
    }
}

/// The impulse response of the Gaussian filter of bandwidth-time product `bt` (unit area, `t` in symbols):
/// `sqrt(2 pi / ln 2) bt e^(-2 (pi bt t)^2 / ln 2)`. Its standard deviation is `sqrt(ln 2) / (2 pi bt)` symbols.
pub fn gaussian(t: f64, bt: f64) -> f64 {
    (2.0 * PI / LN_2).sqrt() * bt * (-2.0 * (PI * bt * t).powi(2) / LN_2).exp()
}

/// The half-sine pulse one symbol long: `cos(pi t)` for `-1/2 <= t < 1/2`, 0 elsewhere (OQPSK in IEEE 802.15.4 and the RDS
/// biphase symbol are built from it).
pub fn half_sine(t: f64) -> f64 {
    if (-0.5..0.5).contains(&t) {
        (PI * t).cos()
    } else {
        0.0
    }
}

/// The rectangular pulse one symbol long: 1 for `-1/2 <= t < 1/2`, 0 elsewhere (half-open, so sampling it at a whole number of
/// samples per symbol produces exactly that many ones).
pub fn rect(t: f64) -> f64 {
    if (-0.5..0.5).contains(&t) {
        1.0
    } else {
        0.0
    }
}

/// A raised-cosine ramp for burst edges: 0 for `x <= 0`, `(1 - cos(pi x)) / 2` for `0 < x < 1`, 1 for `x >= 1` (NaN returns 0).
/// Scale `x` by the ramp length: `ramp(t / ramp_s)` rising, `ramp((end - t) / ramp_s)` falling.
pub fn ramp(x: f64) -> f64 {
    if x.is_nan() || x <= 0.0 {
        0.0
    } else if x >= 1.0 {
        1.0
    } else {
        0.5 - 0.5 * (PI * x).cos()
    }
}

/// A pulse shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Root raised cosine, roll-off `beta` in `[0, 1]` ([`rrc`]).
    Rrc {
        /// Roll-off.
        beta: f64,
    },
    /// Raised cosine, roll-off `beta` in `[0, 1]` ([`rc`]).
    Rc {
        /// Roll-off.
        beta: f64,
    },
    /// The Gaussian frequency pulse of GMSK/GFSK, `bt > 0` ([`gaussian_freq`]).
    GaussianFreq {
        /// Bandwidth-time product.
        bt: f64,
    },
    /// The Gaussian filter alone, `bt > 0` ([`gaussian`]).
    Gaussian {
        /// Bandwidth-time product.
        bt: f64,
    },
    /// One-symbol half-sine ([`half_sine`]).
    HalfSine,
    /// One-symbol rectangle ([`rect`]).
    Rect,
}

impl Shape {
    /// The value at `t` symbols from the centre.
    pub fn value(self, t: f64) -> f64 {
        match self {
            Shape::Rrc { beta } => rrc(t, beta),
            Shape::Rc { beta } => rc(t, beta),
            Shape::GaussianFreq { bt } => gaussian_freq(t, bt),
            Shape::Gaussian { bt } => gaussian(t, bt),
            Shape::HalfSine => half_sine(t),
            Shape::Rect => rect(t),
        }
    }

    /// Checks the parameter.
    pub fn check(self) -> Result<(), DspError> {
        match self {
            Shape::Rrc { beta } | Shape::Rc { beta } if !(0.0..=1.0).contains(&beta) => {
                Param::RollOff.range(0.0, 1.0, beta)
            }
            Shape::GaussianFreq { bt } | Shape::Gaussian { bt }
                if !(bt > 0.0 && bt.is_finite()) =>
            {
                Param::Bt.positive(bt)
            }
            _ => Ok(()),
        }
    }
}

/// How sampled taps are scaled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Norm {
    /// `sum h^2 = 1`: a matched filter that leaves white noise at its input power.
    Energy,
    /// `sum h = 1`: unit gain at DC.
    Dc,
    /// `max |h| = 1`.
    Peak,
    /// The continuous pulse's values as they are (for an RRC at `sps` samples per symbol, `sum h^2` is close to `sps`).
    Unscaled,
}

/// The number of taps [`taps`] returns: `2 floor(span sps / 2) + 1` (odd, so the centre tap is at `t = 0`).
pub fn taps_len(sps: f64, span: f64) -> usize {
    2 * ((span * sps / 2.0).floor() as usize) + 1
}

/// The pulse `shape` sampled at `sps` samples per symbol (any value above 0, fractional allowed) over `span` symbols in all, centred
/// on its middle tap ([`taps_len`] taps, at least the centre one), scaled by `norm`. Errors for a bad shape parameter, `sps` or
/// `span` not positive, or taps that sum to zero under the normalisation.
pub fn taps(shape: Shape, sps: f64, span: f64, norm: Norm) -> Result<Vec<f32>, DspError> {
    let mut out = Vec::new();
    taps_into(shape, sps, span, norm, &mut out)?;
    Ok(out)
}

/// [`taps`] into a caller's `Vec` (cleared first; no allocation once it has the capacity).
pub fn taps_into(
    shape: Shape,
    sps: f64,
    span: f64,
    norm: Norm,
    out: &mut Vec<f32>,
) -> Result<(), DspError> {
    shape.check()?;
    if !(sps > 0.0 && sps.is_finite()) {
        return Param::Sps.positive(sps);
    }
    if !(span > 0.0 && span.is_finite()) {
        return Param::Span.positive(span);
    }
    let n = taps_len(sps, span);
    if n > 1 << 24 {
        return Param::Taps.range(1.0, (1u32 << 24) as f64, n as f64);
    }
    let c = (n / 2) as f64;
    let mut s = 0.0f64;
    let mut e = 0.0f64;
    let mut pk = 0.0f64;
    for i in 0..n {
        let v = shape.value((i as f64 - c) / sps);
        s += v;
        e += v * v;
        pk = pk.max(v.abs());
    }
    let k = match norm {
        Norm::Energy => 1.0 / e.sqrt(),
        Norm::Dc => 1.0 / s,
        Norm::Peak => 1.0 / pk,
        Norm::Unscaled => 1.0,
    };
    if !k.is_finite() {
        return Param::Shape.degenerate();
    }
    out.clear();
    out.extend((0..n).map(|i| (shape.value((i as f64 - c) / sps) * k) as f32));
    Ok(())
}

/// A pulse tabulated at `resolution` points per symbol over `-half_span..half_span` symbols, read with linear interpolation: the
/// value at any instant for a generator at a fractional sample rate, with a clock error, or with timing trials, at the cost of one
/// table read. Outside the span it is 0.
#[derive(Clone, Debug)]
pub struct PulseTable {
    table: Vec<f32>,
    half_span: f64,
    res: f64,
}

impl PulseTable {
    /// Tabulates `shape` (unscaled values) over `half_span` symbols each side at `resolution` points per symbol (256 keeps the
    /// interpolation error of a Gaussian frequency pulse below 1e-5).
    pub fn new(shape: Shape, half_span: f64, resolution: usize) -> Result<Self, DspError> {
        shape.check()?;
        if !(half_span > 0.0 && half_span <= 1024.0) {
            return Param::HalfSpan.range(0.0, 1024.0, half_span);
        }
        if !(1..=1 << 16).contains(&resolution) {
            return Param::Resolution.range(1.0, 65_536.0, resolution as f64);
        }
        let res = resolution as f64;
        let n = (2.0 * half_span * res).ceil() as usize + 2;
        let table = (0..n)
            .map(|i| shape.value(i as f64 / res - half_span) as f32)
            .collect();
        Ok(PulseTable {
            table,
            half_span,
            res,
        })
    }

    /// The half span in symbols.
    pub fn half_span(&self) -> f64 {
        self.half_span
    }

    /// The value at `t` symbols from the centre (0 outside the span, and for NaN).
    #[inline]
    pub fn value(&self, t: f64) -> f32 {
        let u = (t + self.half_span) * self.res;
        if u.is_nan() || u < 0.0 {
            return 0.0;
        }
        let i = u as usize;
        if i + 1 >= self.table.len() {
            return 0.0;
        }
        let f = (u - i as f64) as f32;
        let a = self.table[i];
        a + (self.table[i + 1] - a) * f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erf_matches_published_values() {
        // Abramowitz and Stegun table 7.1 (erf up to x = 2) and tabulated values beyond, with the erfc tails below.
        let known = [
            (0.1, 0.1124629160182849),
            (0.5, 0.5204998778130465),
            (1.0, 0.8427007929497149),
            (1.5, 0.9661051464753108),
            (2.0, 0.9953222650189527),
            (2.5, 0.999_593_047_982_555),
            (3.0, 0.9999779095030014),
        ];
        for (x, want) in known {
            assert!((erf(x) - want).abs() < 4e-16, "erf({x}) = {}", erf(x));
            assert!((erf(-x) + want).abs() < 4e-16);
        }
        let tails = [
            (3.0, 2.209049699858544e-5),
            (4.0, 1.541725790028002e-8),
            (5.0, 1.537459794428035e-12),
            (10.0, 2.088487583762545e-45),
        ];
        for (x, want) in tails {
            assert!(
                ((erfc(x) - want) / want).abs() < 1e-13,
                "erfc({x}) = {}",
                erfc(x)
            );
        }
        assert_eq!(erf(0.0), 0.0);
        assert!(erf(f64::NAN).is_nan());
        assert_eq!(erf(40.0), 1.0);
        assert!((erfc(-3.0) - (2.0 - 2.209049699858544e-5)).abs() < 1e-15);
        // Continuity at the switch point.
        assert!((erf(2.5 - 1e-12) - erf(2.5 + 1e-12)).abs() < 1e-14);
    }

    #[test]
    fn rrc_has_its_closed_form_values() {
        for beta in [0.2f64, 0.35, 0.5, 1.0] {
            assert!((rrc(0.0, beta) - (1.0 - beta + 4.0 * beta / PI)).abs() < 1e-15);
            // Just beside the singular point equals the limit there.
            let ts = 1.0 / (4.0 * beta);
            assert!(
                (rrc(ts, beta) - rrc(ts + 1e-7, beta)).abs() < 1e-5,
                "{beta}"
            );
            assert!((rrc(-ts, beta) - rrc(ts, beta)).abs() < 1e-15);
        }
        // beta = 0 is the sinc.
        assert!((rrc(0.3, 0.0) - sinc(0.3)).abs() < 1e-15);
        // A direct evaluation at t = 0.5, beta = 0.35: (sin(0.325 pi) + 0.7 cos(0.675 pi)) / (0.5 pi (1 - 0.49)).
        let want = ((0.325 * PI).sin() + 0.7 * (0.675 * PI).cos()) / (0.5 * PI * 0.51);
        assert!((rrc(0.5, 0.35) - want).abs() < 1e-15);
    }

    #[test]
    fn rrc_has_unit_energy_and_two_make_a_nyquist_pulse() {
        // integral of rrc^2 dt = 1 (rectangle rule on a fine grid over +-40 symbols).
        let beta = 0.35;
        let dt = 1.0 / 64.0;
        let e: f64 = (-40 * 64..=40 * 64)
            .map(|i| rrc(i as f64 * dt, beta).powi(2) * dt)
            .sum();
        assert!((e - 1.0).abs() < 2e-3, "{e}");
        // Sampled at 8 sps, rrc * rrc has (near) zero ISI and matches rc.
        let h = taps(Shape::Rrc { beta }, 8.0, 40.0, Norm::Unscaled).unwrap();
        let n = h.len();
        let mut conv = vec![0f64; 2 * n - 1];
        for i in 0..n {
            for j in 0..n {
                conv[i + j] += h[i] as f64 * h[j] as f64 / 8.0;
            }
        }
        let c = n - 1;
        assert!((conv[c] - 1.0).abs() < 5e-3, "{}", conv[c]);
        for k in 1..6 {
            assert!(
                conv[c + 8 * k].abs() < 5e-3,
                "ISI at {k}: {}",
                conv[c + 8 * k]
            );
            assert!((conv[c + 4 * k] - rc(k as f64 / 2.0, beta)).abs() < 5e-3);
        }
    }

    #[test]
    fn rc_is_nyquist_and_continuous() {
        for beta in [0.1, 0.25, 0.5, 1.0] {
            assert_eq!(rc(0.0, beta), 1.0);
            for k in 1..10 {
                assert!(rc(k as f64, beta).abs() < 1e-15, "{beta} {k}");
            }
            let ts = 1.0 / (2.0 * beta);
            assert!((rc(ts, beta) - rc(ts + 1e-7, beta)).abs() < 1e-6);
        }
    }

    #[test]
    fn gaussian_pulse_matches_the_gsm_definition() {
        // TS 45.004 2.5: g(t) = 1/(2T) (Q(2 pi B (t - T/2)/sqrt(ln 2)) - Q(2 pi B (t + T/2)/sqrt(ln 2))), Q(x) = erfc(x/sqrt 2)/2.
        // With T = 1 our unit-area pulse is twice that.
        let q = |x: f64| 0.5 * erfc(x / 2f64.sqrt());
        for bt in [0.25, 0.3, 0.4, 0.5, 1.0] {
            let mut area = 0.0;
            for i in -4000..=4000 {
                let t = i as f64 / 1000.0;
                let gsm = 0.5
                    * (q(2.0 * PI * bt * (t - 0.5) / LN_2.sqrt())
                        - q(2.0 * PI * bt * (t + 0.5) / LN_2.sqrt()));
                let g = gaussian_freq(t, bt);
                assert!((g - 2.0 * gsm).abs() < 1e-14, "bt {bt} t {t}");
                area += g / 1000.0;
            }
            assert!((area - 1.0).abs() < 1e-9, "bt {bt}: area {area}");
        }
        // As BT grows the pulse becomes the rectangle.
        assert!((gaussian_freq(0.0, 50.0) - 1.0).abs() < 1e-12);
        assert!(gaussian_freq(0.7, 50.0) < 1e-12);
        // The Gaussian filter has unit area and the stated deviation.
        let bt = 0.3;
        let (mut a, mut v) = (0.0, 0.0);
        for i in -20000..=20000 {
            let t = i as f64 / 2000.0;
            a += gaussian(t, bt) / 2000.0;
            v += t * t * gaussian(t, bt) / 2000.0;
        }
        let sigma = LN_2.sqrt() / (2.0 * PI * bt);
        assert!((a - 1.0).abs() < 1e-9 && (v.sqrt() - sigma).abs() < 1e-9);
    }

    #[test]
    fn taps_follow_their_normalisation() {
        let h = taps(Shape::Rrc { beta: 0.5 }, 3.3, 10.0, Norm::Energy).unwrap();
        assert_eq!(h.len(), taps_len(3.3, 10.0));
        assert_eq!(h.len() % 2, 1);
        let e: f32 = h.iter().map(|v| v * v).sum();
        assert!((e - 1.0).abs() < 1e-5);
        let h = taps(Shape::GaussianFreq { bt: 0.3 }, 4.0, 4.0, Norm::Dc).unwrap();
        assert!((h.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        let h = taps(Shape::Rc { beta: 0.2 }, 5.0, 6.0, Norm::Peak).unwrap();
        assert_eq!(h[h.len() / 2], 1.0);
        let h = taps(Shape::Rect, 8.0, 1.0, Norm::Unscaled).unwrap();
        assert_eq!(h.iter().filter(|&&v| v == 1.0).count(), 8);
        let h = taps(Shape::HalfSine, 4.0, 2.0, Norm::Peak).unwrap();
        let want = [0.0, 0.0, 0.0, 0.70710677, 1.0, 0.70710677, 0.0, 0.0, 0.0];
        assert_eq!(h.len(), want.len());
        for (a, b) in h.iter().zip(want) {
            assert!((a - b).abs() < 1e-7, "{h:?}");
        }
        // Bad parameters are refused.
        assert!(taps(Shape::Rrc { beta: 1.5 }, 4.0, 8.0, Norm::Energy).is_err());
        assert!(taps(Shape::GaussianFreq { bt: 0.0 }, 4.0, 8.0, Norm::Energy).is_err());
        assert!(taps(Shape::Rect, f64::NAN, 8.0, Norm::Energy).is_err());
        assert!(taps(Shape::Rect, 4.0, -1.0, Norm::Energy).is_err());
        // A span shorter than a sample produces the centre tap alone.
        assert_eq!(
            taps(Shape::HalfSine, 4.0, 0.2, Norm::Dc).unwrap(),
            vec![1.0]
        );
    }

    #[test]
    fn ramp_rises_smoothly() {
        assert_eq!(ramp(-1.0), 0.0);
        assert_eq!(ramp(0.0), 0.0);
        assert!((ramp(0.5) - 0.5).abs() < 1e-15);
        assert!((ramp(0.25) - (0.5 - 0.5 * (PI / 4.0).cos())).abs() < 1e-15);
        assert_eq!(ramp(1.0), 1.0);
        assert_eq!(ramp(f64::NAN), 0.0);
    }

    #[test]
    fn the_table_matches_the_pulse() {
        let shape = Shape::GaussianFreq { bt: 0.3 };
        let t = PulseTable::new(shape, 3.0, 256).unwrap();
        let mut worst = 0.0f64;
        for i in -3000..3000 {
            let x = i as f64 / 1000.0 + 0.0003;
            worst = worst.max((t.value(x) as f64 - shape.value(x)).abs());
        }
        assert!(worst < 1e-5, "{worst}");
        assert_eq!(t.value(3.5), 0.0);
        assert_eq!(t.value(-3.5), 0.0);
        assert_eq!(t.value(f64::NAN), 0.0);
        assert!(PulseTable::new(shape, 0.0, 256).is_err());
    }
}

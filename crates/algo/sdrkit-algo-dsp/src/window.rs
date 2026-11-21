//! Window functions, for spectrum analysis and for filter design.
//!
//! Every [`Window`] comes in two forms:
//!
//! - **periodic** (DFT-even): `w[i]` for `i` in `0..n` is the first `n` points of the `n + 1` point symmetric window, so the window
//!   repeats with period `n`. This is the form for spectra ([`Window::value`], [`Window::coefficients`], [`Window::periodic`]).
//! - **symmetric**: `w[i] = w[n - 1 - i]`, the form for FIR design, where the filter must have linear phase ([`Window::value_symmetric`],
//!   [`Window::symmetric`]).
//!
//! The cosine-sum windows use the coefficients of F. J. Harris, "On the use of windows for harmonic analysis with the discrete Fourier
//! transform", Proc. IEEE 66(1), 1978 (Hamming, exact Blackman's rounded form 0.42/0.5/0.08, 4-term minimum Blackman-Harris), and the
//! 5-term flat-top of the HP/SRS analysers.
//!
//! ```
//! use sdrkit_algo_dsp::window::Window;
//! let w = Window::Hamming.symmetric(5);
//! assert!((w[0] - 0.08).abs() < 1e-12 && (w[2] - 1.0).abs() < 1e-12);
//! ```

use std::f64::consts::TAU;

/// The modified Bessel function of the first kind, order 0 (power series; accurate to double precision for the betas used here).
pub fn bessel_i0(x: f64) -> f64 {
    let q = x * x / 4.0;
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut k = 1.0;
    loop {
        term *= q / (k * k);
        sum += term;
        if term < sum * 1e-17 {
            return sum;
        }
        k += 1.0;
    }
}

/// Kaiser's beta for a stopband attenuation in dB (Kaiser's empirical formula).
pub fn kaiser_beta(atten_db: f64) -> f64 {
    if atten_db > 50.0 {
        0.1102 * (atten_db - 8.7)
    } else if atten_db >= 21.0 {
        0.5842 * (atten_db - 21.0).powf(0.4) + 0.07886 * (atten_db - 21.0)
    } else {
        0.0
    }
}

/// A symmetric Kaiser window of `n` points.
pub fn kaiser(n: usize, beta: f64) -> Vec<f64> {
    if n == 1 {
        return vec![1.0];
    }
    let d = bessel_i0(beta);
    let m = (n - 1) as f64;
    (0..n)
        .map(|i| {
            let r = 2.0 * i as f64 / m - 1.0;
            bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / d
        })
        .collect()
}

/// The shape parameter of a [`Window::Kaiser`]. It compares by bit pattern, so [`Window`] keeps `Eq`.
#[derive(Clone, Copy, Debug)]
pub struct Beta(pub f64);

impl PartialEq for Beta {
    fn eq(&self, o: &Beta) -> bool {
        self.0.to_bits() == o.0.to_bits()
    }
}
impl Eq for Beta {}

/// A window function.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Window {
    /// No window: the narrowest main lobe, the worst leakage (-13 dB sidelobes).
    Rectangular,
    /// Hann: -31 dB sidelobes, 1.5 bins noise bandwidth.
    Hann,
    /// Blackman-Harris, 4 terms (minimum sidelobe form): -92 dB sidelobes, 2.0 bins noise bandwidth. The default.
    #[default]
    BlackmanHarris4,
    /// Hamming (0.54 - 0.46 cos): -43 dB sidelobes, 1.36 bins noise bandwidth.
    Hamming,
    /// Blackman, 3 terms (0.42 - 0.5 cos + 0.08 cos 2x): -58 dB sidelobes, 1.73 bins noise bandwidth.
    Blackman,
    /// Flat-top, 5 terms: amplitude within 0.01 dB wherever a tone falls between bins, at 3.77 bins noise bandwidth. For reading the
    /// level of a tone.
    FlatTop,
    /// Kaiser with this beta (`beta = pi alpha`; 0 is rectangular, 8.6 is close to Blackman). See [`kaiser_beta`].
    Kaiser(Beta),
}

const HAMMING: [f64; 2] = [0.54, 0.46];
const BLACKMAN: [f64; 3] = [0.42, 0.5, 0.08];
const BH4: [f64; 4] = [0.35875, 0.48829, 0.14128, 0.01168];
const FLAT_TOP: [f64; 5] = [
    0.215_578_95,
    0.416_631_58,
    0.277_263_158,
    0.083_578_947,
    0.006_947_368,
];

/// `sum_k (-1)^k a_k cos(k x)`.
#[inline]
fn cosine_sum(a: &[f64], x: f64) -> f64 {
    a.iter()
        .enumerate()
        .map(|(k, &c)| if k % 2 == 0 { c } else { -c } * (k as f64 * x).cos())
        .sum()
}

impl Window {
    /// A Kaiser window with this beta.
    pub const fn kaiser(beta: f64) -> Window {
        Window::Kaiser(Beta(beta))
    }

    /// The value at `i` of a window whose period is `m` points (`m = n` periodic, `m = n - 1` symmetric).
    fn at(self, i: usize, m: usize) -> f64 {
        if m == 0 {
            return 1.0;
        }
        let x = TAU * i as f64 / m as f64;
        match self {
            Window::Rectangular => 1.0,
            Window::Hann => 0.5 - 0.5 * x.cos(),
            Window::BlackmanHarris4 => cosine_sum(&BH4, x),
            Window::Hamming => cosine_sum(&HAMMING, x),
            Window::Blackman => cosine_sum(&BLACKMAN, x),
            Window::FlatTop => cosine_sum(&FLAT_TOP, x),
            Window::Kaiser(Beta(beta)) => {
                let r = 2.0 * i as f64 / m as f64 - 1.0;
                bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(beta)
            }
        }
    }

    /// Coefficient `i` of the periodic `n`-point window.
    pub fn value(self, i: usize, n: usize) -> f64 {
        self.at(i, n)
    }

    /// Coefficient `i` of the symmetric `n`-point window (`w[0] = w[n-1]`; a 1-point window is `[1]`).
    pub fn value_symmetric(self, i: usize, n: usize) -> f64 {
        self.at(i, n.saturating_sub(1))
    }

    /// All `n` periodic coefficients, as `f32`.
    pub fn coefficients(self, n: usize) -> Vec<f32> {
        (0..n).map(|i| self.value(i, n) as f32).collect()
    }

    /// All `n` periodic coefficients, as `f64`.
    pub fn periodic(self, n: usize) -> Vec<f64> {
        (0..n).map(|i| self.value(i, n)).collect()
    }

    /// All `n` symmetric coefficients, as `f64`.
    pub fn symmetric(self, n: usize) -> Vec<f64> {
        (0..n).map(|i| self.value_symmetric(i, n)).collect()
    }

    /// Fills `out` with the periodic window of `out.len()` points (no allocation).
    pub fn fill_periodic(self, out: &mut [f32]) {
        let n = out.len();
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.value(i, n) as f32;
        }
    }

    /// Fills `out` with the symmetric window of `out.len()` points (no allocation).
    pub fn fill_symmetric(self, out: &mut [f32]) {
        let n = out.len();
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.value_symmetric(i, n) as f32;
        }
    }

    /// The coherent gain of the periodic `n`-point window: `sum w / n` (the amplitude a tone on a bin reads at, before correction).
    pub fn coherent_gain(self, n: usize) -> f64 {
        (0..n).map(|i| self.value(i, n)).sum::<f64>() / n as f64
    }

    /// The equivalent noise bandwidth of the periodic `n`-point window in bins: `n sum w^2 / (sum w)^2`.
    pub fn enbw_bins(self, n: usize) -> f64 {
        let (s1, s2) = (0..n).fold((0.0, 0.0), |(a, b), i| {
            let w = self.value(i, n);
            (a + w, b + w * w)
        });
        n as f64 * s2 / (s1 * s1)
    }
}

/// The power response of a long Hann window to a tone `offset` bins from a bin centre, 1 at the centre. Inside the main lobe
/// (`|offset| < 2`) it falls from 1 to 0. A receiver divides by it to undo the loss of a tone between two bins. The value is
/// `(sinc(d) / (1 - d^2))^2` with `sinc(d) = sin(pi d) / (pi d)`, the limit for a long window. Harris (1978) table 1 lists its
/// worst loss, half way between bins: 1.42 dB.
///
/// ```
/// use sdrkit_algo_dsp::window::hann_bin_power;
/// assert_eq!(hann_bin_power(0.0), 1.0);
/// assert!((hann_bin_power(1.0) - 0.25).abs() < 1e-12);
/// assert!((-10.0 * hann_bin_power(0.5).log10() - 1.42).abs() < 0.01);
/// ```
pub fn hann_bin_power(offset: f64) -> f64 {
    let d = offset.abs();
    if d < 1e-9 {
        return 1.0;
    }
    // At d = 1 both parts of the ratio go to zero. The limit is 1/2 in amplitude.
    if (d - 1.0).abs() < 1e-9 {
        return 0.25;
    }
    let a = (std::f64::consts::PI * d).sin() / (std::f64::consts::PI * d) / (1.0 - d * d);
    a * a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hann_bin_power_matches_a_long_window() {
        // A tone at a fractional bin through a 4096-point periodic Hann window, against the formula.
        let n = 4096;
        let w = Window::Hann.periodic(n);
        let gain = |d: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, wi) in w.iter().enumerate() {
                let p = TAU * d * i as f64 / n as f64;
                re += wi * p.cos();
                im += wi * p.sin();
            }
            (re * re + im * im) / (n as f64 / 2.0).powi(2)
        };
        for d in [0.0, 0.25, 0.5, 0.9, 1.0, 1.5, 1.99, -0.5] {
            assert!((hann_bin_power(d) - gain(d.abs())).abs() < 1e-5, "{d}");
        }
        assert!(hann_bin_power(2.0) < 1e-12);
        assert!(hann_bin_power(f64::NAN).is_nan());
    }

    #[test]
    fn i0_known_values() {
        assert!((bessel_i0(0.0) - 1.0).abs() < 1e-15);
        assert!((bessel_i0(1.0) - 1.2660658777520082).abs() < 1e-13);
        assert!((bessel_i0(10.0) - 2815.716628466254).abs() < 1e-8);
    }

    #[test]
    fn kaiser_is_symmetric_and_peaks_at_one() {
        let w = kaiser(31, 8.0);
        assert!((w[15] - 1.0).abs() < 1e-12);
        for i in 0..31 {
            assert!((w[i] - w[30 - i]).abs() < 1e-12);
        }
        // The enum form returns the same values.
        let e = Window::kaiser(8.0).symmetric(31);
        for i in 0..31 {
            assert!((w[i] - e[i]).abs() < 1e-15);
        }
    }

    const ALL: [Window; 7] = [
        Window::Rectangular,
        Window::Hann,
        Window::BlackmanHarris4,
        Window::Hamming,
        Window::Blackman,
        Window::FlatTop,
        Window::Kaiser(Beta(6.0)),
    ];

    #[test]
    fn forms_are_symmetric_and_periodic() {
        for w in ALL {
            let n = 33;
            let s = w.symmetric(n);
            for i in 0..n {
                assert!((s[i] - s[n - 1 - i]).abs() < 1e-12, "{w:?}");
            }
            // The periodic n-point window is the first n points of the symmetric n+1 point one.
            let p = w.periodic(n);
            let s1 = w.symmetric(n + 1);
            for i in 0..n {
                assert!((p[i] - s1[i]).abs() < 1e-12, "{w:?}");
            }
            assert_eq!(w.symmetric(1), vec![1.0]);
        }
        // Published end and centre values.
        let h = Window::Hamming.symmetric(9);
        assert!((h[0] - 0.08).abs() < 1e-15 && (h[4] - 1.0).abs() < 1e-15);
        let b = Window::Blackman.symmetric(9);
        assert!(b[0].abs() < 1e-15 && (b[4] - 1.0).abs() < 1e-15);
        let bh = Window::BlackmanHarris4.symmetric(9);
        assert!((bh[0] - 6e-5).abs() < 1e-15);
    }

    /// Highest sidelobe in dB, from a 64x zero-padded DFT of the symmetric window, searched beyond `skip` bins (inside the main lobe).
    fn sidelobe_db(w: Window, n: usize, skip: f64) -> f64 {
        let c = w.symmetric(n);
        let pad = 64;
        let resp = |f: f64| -> f64 {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in c.iter().enumerate() {
                re += v * (TAU * f * i as f64).cos();
                im -= v * (TAU * f * i as f64).sin();
            }
            (re * re + im * im).sqrt()
        };
        let peak = resp(0.0);
        let mut k = (skip * pad as f64) as usize;
        let mut prev = resp(k as f64 / (n * pad) as f64);
        // Walk down the main lobe to its first minimum, then take the maximum beyond.
        loop {
            let v = resp(k as f64 / (n * pad) as f64);
            if v > prev {
                break;
            }
            prev = v;
            k += 1;
        }
        let worst = (k..n * pad / 2)
            .map(|j| resp(j as f64 / (n * pad) as f64))
            .fold(0.0, f64::max);
        20.0 * (worst / peak).log10()
    }

    #[test]
    fn windows_have_their_published_figures() {
        // Harris 1978 table 1: ENBW (bins), coherent gain, highest sidelobe (dB).
        let table = [
            (Window::Rectangular, 1.00, 1.00, -13.3, 0.5),
            (Window::Hann, 1.50, 0.50, -31.5, 1.0),
            (Window::Hamming, 1.36, 0.54, -42.7, 1.0),
            (Window::Blackman, 1.73, 0.42, -58.1, 1.5),
            (Window::BlackmanHarris4, 2.00, 0.36, -92.0, 2.0),
            (Window::FlatTop, 3.77, 0.22, -93.0, 3.0),
        ];
        for (w, enbw, cg, sl, skip) in table {
            assert!(
                (w.enbw_bins(1024) - enbw).abs() < 0.01,
                "{w:?} enbw {}",
                w.enbw_bins(1024)
            );
            assert!((w.coherent_gain(1024) - cg).abs() < 0.005, "{w:?} cg");
            let got = sidelobe_db(w, 256, skip);
            assert!(got < sl + 1.0 && got > sl - 6.0, "{w:?} sidelobe {got}");
        }
    }

    #[test]
    fn flat_top_reads_tones_between_bins() {
        // A tone half way between bins reads within 0.01 dB of one on a bin.
        let n = 256;
        let w = Window::FlatTop.periodic(n);
        let amp = |f: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in w.iter().enumerate() {
                re += v * (TAU * f * i as f64 / n as f64).cos();
                im += v * (TAU * f * i as f64 / n as f64).sin();
            }
            (re * re + im * im).sqrt()
        };
        let loss = 20.0 * (amp(0.5) / amp(0.0)).log10();
        assert!(loss.abs() < 0.01, "{loss}");
    }
}

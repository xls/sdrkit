//! Fractional peak positions from three samples around a maximum, and least-squares lines.
//!
//! | Function | Input | Exact for |
//! |---|---|---|
//! | [`parabolic`] | three real values (magnitudes, correlations) | a parabola |
//! | [`log_parabolic`] | three powers | a Gaussian peak (a Gaussian window's spectrum, closely a Hann or Blackman one) |
//! | [`candan`] | three complex DFT bins | a complex tone under a rectangular window (unbiased fractional bin) |
//!
//! Offsets are in samples (or bins) from the middle value, clamped to `[-0.5, 0.5]`; a positive offset means the peak lies towards
//! the third value.
//!
//! ```
//! use sdrkit_algo_dsp::peak::parabolic;
//! // y = 4 - (x - 0.3)^2 at x = -1, 0, 1.
//! let p = parabolic(4.0 - 1.69, 4.0 - 0.09, 4.0 - 0.49);
//! assert!((p.offset - 0.3).abs() < 1e-12 && (p.value - 4.0).abs() < 1e-12);
//! ```

use crate::Complex32;
use crate::{DspError, Param};
use std::f64::consts::PI;

/// A fitted peak: its offset from the middle sample and its interpolated height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peak {
    /// Offset from the middle sample, in `[-0.5, 0.5]`.
    pub offset: f64,
    /// The interpolated height (in the units of the input: for [`log_parabolic`], a power).
    pub value: f64,
}

/// The vertex of the parabola through `(-1, ym)`, `(0, y0)`, `(1, yp)`. When the three values do not curve down (no maximum), or any
/// is not finite, the offset is 0 and the value is `y0`.
pub fn parabolic(ym: f64, y0: f64, yp: f64) -> Peak {
    let den = ym - 2.0 * y0 + yp;
    if den.is_nan() || den >= 0.0 || !(ym.is_finite() && y0.is_finite() && yp.is_finite()) {
        return Peak {
            offset: 0.0,
            value: y0,
        };
    }
    let d = (0.5 * (ym - yp) / den).clamp(-0.5, 0.5);
    Peak {
        offset: d,
        value: y0 - 0.25 * (ym - yp) * d,
    }
}

/// The parabolic fit on the logarithms of three powers (exact for a Gaussian peak). The value is a power. Powers that are not
/// positive give the offset 0 and the value `p0`.
pub fn log_parabolic(pm: f64, p0: f64, pp: f64) -> Peak {
    if !(pm > 0.0 && p0 > 0.0 && pp > 0.0) {
        return Peak {
            offset: 0.0,
            value: p0,
        };
    }
    let p = parabolic(pm.ln(), p0.ln(), pp.ln());
    Peak {
        offset: p.offset,
        value: p.value.exp(),
    }
}

/// Candan's fractional-bin estimator from three complex DFT bins of an `n`-point transform (rectangular window), with its bias
/// correction: `delta = tan(pi/n)/(pi/n) Re((X[k-1] - X[k+1]) / (2 X[k] - X[k-1] - X[k+1]))` (Ç. Candan, "A method for fine
/// resolution frequency estimation from three DFT samples", IEEE Signal Processing Letters 18(6), 2011). Nearly unbiased for any
/// fraction; the same formula is used by dechirped LoRa symbols. Returns the offset in bins (0 for a degenerate input).
pub fn candan(xm: Complex32, x0: Complex32, xp: Complex32, n: usize) -> f64 {
    let den = x0 * 2.0 - xm - xp;
    if n < 2 || den.norm_sqr().is_nan() || den.norm_sqr() <= 0.0 {
        return 0.0;
    }
    let r = ((xm - xp) / den).re as f64;
    let c = (PI / n as f64).tan() / (PI / n as f64);
    let d = r * c;
    if d.is_finite() {
        d.clamp(-0.5, 0.5)
    } else {
        0.0
    }
}

/// The index of the largest value (the first one on ties); `None` for an empty slice or one holding only NaN.
pub fn argmax(x: &[f32]) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, &v) in x.iter().enumerate() {
        if v.is_nan() {
            continue;
        }
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((i, v));
        }
    }
    best.map(|(i, _)| i)
}

/// The values either side of `x[k]`: `(x[k-1], x[k], x[k+1])`. With `circular` the ends wrap (FFT bins); otherwise `None` at an end
/// or out of range.
pub fn neighbours<T: Copy>(x: &[T], k: usize, circular: bool) -> Option<(T, T, T)> {
    let n = x.len();
    if k >= n || n < 3 {
        return None;
    }
    if circular {
        Some((x[(k + n - 1) % n], x[k], x[(k + 1) % n]))
    } else if k == 0 || k + 1 == n {
        None
    } else {
        Some((x[k - 1], x[k], x[k + 1]))
    }
}

/// A straight line `y = slope x + intercept`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    /// The slope.
    pub slope: f64,
    /// The value at `x = 0`.
    pub intercept: f64,
}

impl Line {
    /// The value at `x`.
    pub fn at(&self, x: f64) -> f64 {
        self.slope * x + self.intercept
    }
}

/// The least-squares line through the points `(x[i], y[i])`, optionally weighted (`w[i] >= 0`; pass `None` for equal weights).
/// Sums are centred on the weighted means, so long time axes keep their precision. Errors when the lengths differ, fewer than two
/// distinct `x` carry weight, or a value is not finite.
pub fn line_fit(x: &[f64], y: &[f64], w: Option<&[f64]>) -> Result<Line, DspError> {
    if x.len() != y.len() {
        return Param::Y.length(x.len(), y.len());
    }
    if let Some(w) = w.filter(|w| w.len() != x.len()) {
        return Param::Weight.length(x.len(), w.len());
    }
    let wt = |i: usize| w.map_or(1.0, |w| w[i]);
    let (mut sw, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for i in 0..x.len() {
        let wi = wt(i);
        if !(wi >= 0.0 && wi.is_finite()) {
            return Param::Weight.at_least(0.0, wi);
        }
        if !x[i].is_finite() {
            return Param::X.not_finite();
        }
        if !y[i].is_finite() {
            return Param::Y.not_finite();
        }
        sw += wi;
        sx += wi * x[i];
        sy += wi * y[i];
    }
    if sw <= 0.0 {
        return Param::Weight.degenerate();
    }
    let (mx, my) = (sx / sw, sy / sw);
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for i in 0..x.len() {
        let dx = x[i] - mx;
        sxx += wt(i) * dx * dx;
        sxy += wt(i) * dx * (y[i] - my);
    }
    if sxx <= 0.0 {
        return Param::X.degenerate();
    }
    let slope = sxy / sxx;
    Ok(Line {
        slope,
        intercept: my - slope * mx,
    })
}

/// The least-squares line through `y[i]` at `x = i` (equal spacing), with no allocation.
pub fn line_fit_uniform(y: &[f64]) -> Result<Line, DspError> {
    let n = y.len();
    if n < 2 {
        return Param::Y.at_least(2.0, n as f64);
    }
    if y.iter().any(|v| !v.is_finite()) {
        return Param::Y.not_finite();
    }
    let mx = (n - 1) as f64 / 2.0;
    let my = y.iter().sum::<f64>() / n as f64;
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for (i, &v) in y.iter().enumerate() {
        let dx = i as f64 - mx;
        sxx += dx * dx;
        sxy += dx * (v - my);
    }
    let slope = sxy / sxx;
    Ok(Line {
        slope,
        intercept: my - slope * mx,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn parabolic_and_log_parabolic_are_exact_for_their_shapes() {
        for d in [-0.5, -0.31, 0.0, 0.12, 0.49] {
            let q = |x: f64| 2.0 - 0.7 * (x - d) * (x - d);
            let p = parabolic(q(-1.0), q(0.0), q(1.0));
            assert!((p.offset - d).abs() < 1e-12 && (p.value - 2.0).abs() < 1e-12);
            let g = |x: f64| 5.0 * (-(x - d) * (x - d) / 0.8).exp();
            let p = log_parabolic(g(-1.0), g(0.0), g(1.0));
            assert!((p.offset - d).abs() < 1e-12 && (p.value - 5.0).abs() < 1e-10);
        }
        // Degenerate inputs never panic.
        assert_eq!(parabolic(1.0, 1.0, 1.0).offset, 0.0);
        assert_eq!(parabolic(1.0, 0.0, 1.0).offset, 0.0);
        assert_eq!(parabolic(f64::NAN, 1.0, 0.0).offset, 0.0);
        assert_eq!(log_parabolic(0.0, 1.0, 0.5).offset, 0.0);
    }

    fn bins(n: usize, f: f64, k: usize) -> (Complex32, Complex32, Complex32) {
        let x: Vec<num_complex::Complex64> = (0..n)
            .map(|i| num_complex::Complex64::from_polar(1.0, TAU * f * i as f64 / n as f64 + 0.4))
            .collect();
        let bin = |b: usize| {
            let s: num_complex::Complex64 = x
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    v * num_complex::Complex64::from_polar(1.0, -TAU * (b * i) as f64 / n as f64)
                })
                .sum();
            Complex32::new(s.re as f32, s.im as f32)
        };
        (bin(k - 1), bin(k), bin(k + 1))
    }

    #[test]
    fn candan_finds_the_fractional_bin() {
        for n in [16usize, 64, 1024] {
            let mut worst = 0.0f64;
            for i in 0..=20 {
                let d = -0.5 + i as f64 / 20.0;
                let (a, b, c) = bins(n, 8.0 + d, 8);
                worst = worst.max((candan(a, b, c, n) - d).abs());
            }
            assert!(worst < 2e-3, "n {n}: {worst}");
        }
        let z = Complex32::default();
        assert_eq!(candan(z, z, z, 64), 0.0);
    }

    #[test]
    fn argmax_and_neighbours() {
        assert_eq!(argmax(&[1.0, 3.0, f32::NAN, 3.0, 2.0]), Some(1));
        assert_eq!(argmax(&[]), None);
        assert_eq!(argmax(&[f32::NAN]), None);
        let x = [1, 2, 3, 4];
        assert_eq!(neighbours(&x, 0, true), Some((4, 1, 2)));
        assert_eq!(neighbours(&x, 0, false), None);
        assert_eq!(neighbours(&x, 2, false), Some((2, 3, 4)));
        assert_eq!(neighbours(&x, 9, true), None);
    }

    #[test]
    fn line_fits_recover_lines() {
        let x: Vec<f64> = (0..50).map(|i| 1e9 + i as f64 * 0.5).collect();
        let y: Vec<f64> = x.iter().map(|&v| 3.0 - 0.25 * (v - 1e9)).collect();
        let l = line_fit(&x, &y, None).unwrap();
        assert!((l.slope + 0.25).abs() < 1e-9 && (l.at(1e9) - 3.0).abs() < 1e-6);
        // Weights: a zero weight ignores an outlier.
        let mut y2 = y.clone();
        y2[10] = 1e6;
        let mut w = vec![1.0; 50];
        w[10] = 0.0;
        let l = line_fit(&x, &y2, Some(&w)).unwrap();
        assert!((l.slope + 0.25).abs() < 1e-9);
        let u = line_fit_uniform(&[1.0, 3.0, 5.0, 7.0]).unwrap();
        assert!((u.slope - 2.0).abs() < 1e-12 && (u.intercept - 1.0).abs() < 1e-12);
        assert!(line_fit(&[1.0, 1.0], &[0.0, 1.0], None).is_err());
        assert!(line_fit(&[1.0], &[0.0, 1.0], None).is_err());
        assert!(line_fit_uniform(&[1.0]).is_err());
        assert!(line_fit(&[0.0, 1.0], &[f64::NAN, 1.0], None).is_err());
    }
}

//! Fits on known symbols that several stacks use: a complex gain that changes linearly in time, and the phase slope that best
//! lines up a set of known symbols.

use sdrkit_core::Complex32;

/// The weighted least-squares fit `r(t) = a + b t` of the ratios `y / p` of received `y` to known `p` at times `t`, each
/// weighted by `|p|^2`. Returns `(a, b)`; `(0, 0)` when there are fewer than two points.
pub fn line_gain(points: &[(f64, Complex32, Complex32)]) -> (Complex32, Complex32) {
    let (mut sw, mut st, mut stt) = (0f64, 0f64, 0f64);
    let (mut sr, mut str_) = (Complex32::new(0.0, 0.0), Complex32::new(0.0, 0.0));
    for &(t, y, p) in points {
        let w = f64::from(p.norm_sqr());
        if w <= 0.0 {
            continue;
        }
        // w * (y / p) = y conj(p).
        let wr = y * p.conj();
        sw += w;
        st += w * t;
        stt += w * t * t;
        sr += wr;
        str_ += wr * t as f32;
    }
    let det = sw * stt - st * st;
    if points.len() < 2 || det.abs() < 1e-12 {
        let a = if sw > 0.0 { sr / sw as f32 } else { Complex32::new(0.0, 0.0) };
        return (a, Complex32::new(0.0, 0.0));
    }
    let a = (sr * stt as f32 - str_ * st as f32) / det as f32;
    let b = (str_ * sw as f32 - sr * st as f32) / det as f32;
    (a, b)
}

/// The phase slope `w` (radians per unit of position) in `centre ± half` that maximises `|sum v e^(-j w pos)|`, searched in
/// steps of `step` and refined with a parabola, with the sum at that slope.
pub fn phase_slope(v: &[(f64, Complex32)], centre: f64, half: f64, step: f64) -> (f64, Complex32) {
    let eval = |w: f64| {
        v.iter()
            .map(|&(p, z)| {
                let a = -w * p;
                z * Complex32::new(a.cos() as f32, a.sin() as f32)
            })
            .sum::<Complex32>()
    };
    let n = (half / step).ceil() as i64;
    let count = (2 * n + 1) as usize;
    // Each term turns by a fixed step from one trial slope to the next.
    let mut acc = vec![Complex32::new(0.0, 0.0); count];
    for &(p, z) in v {
        let a0 = -(centre - n as f64 * step) * p;
        let mut r = z * Complex32::new(a0.cos() as f32, a0.sin() as f32);
        let st = Complex32::new((-step * p).cos() as f32, (-step * p).sin() as f32);
        for a in acc.iter_mut() {
            *a += r;
            r *= st;
        }
    }
    let vals: Vec<f32> = acc.iter().map(|c| c.norm()).collect();
    let k = vals
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(n as usize, |(i, _)| i);
    let mut w = centre + (k as f64 - n as f64) * step;
    if k > 0 && k + 1 < vals.len() {
        let (a, b, c) = (f64::from(vals[k - 1]), f64::from(vals[k]), f64::from(vals[k + 1]));
        let d = a - 2.0 * b + c;
        if d < 0.0 {
            w += step * (0.5 * (a - c) / d).clamp(-0.5, 0.5);
        }
    }
    (w, eval(w))
}

/// The parabola offset of a peak `b` between neighbours `a` and `c`, limited to ±0.5.
pub fn parabola(a: f64, b: f64, c: f64) -> f64 {
    let d = a - 2.0 * b + c;
    if d < 0.0 {
        (0.5 * (a - c) / d).clamp(-0.5, 0.5)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_recover_a_slope_and_a_gain() {
        let g0 = Complex32::new(0.6, -0.3);
        let w = 0.0123;
        let pts: Vec<(f64, Complex32)> = (0..40)
            .map(|i| {
                let p = (i * 4) as f64;
                (p, g0 * Complex32::new((w * p).cos() as f32, (w * p).sin() as f32))
            })
            .collect();
        let (est, s) = phase_slope(&pts, 0.0, 0.03, 0.00025);
        assert!((est - w).abs() < 2e-5, "{est}");
        assert!((s / 40.0 - g0).norm() < 0.01);
        let lin: Vec<(f64, Complex32, Complex32)> = (0..10)
            .map(|i| {
                let t = i as f64 - 4.5;
                let p = Complex32::new(1.0, (i % 2) as f32);
                (t, (g0 + Complex32::new(0.01, 0.02) * t as f32) * p, p)
            })
            .collect();
        let (a, b) = line_gain(&lin);
        assert!((a - g0).norm() < 1e-4 && (b - Complex32::new(0.01, 0.02)).norm() < 1e-4);
        assert!((parabola(1.0, 2.0, 1.0)).abs() < 1e-12);
    }
}

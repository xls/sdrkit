//! An MMSE decision-feedback equaliser designed from a known symbol-spaced channel, as a probe-based modem uses it between its
//! channel estimates.
//!
//! The channel is `y(n) = sum_l h(l) a(n - l) + w(n)`, `l` from `lag0` to `lag0 + len - 1`, noise of variance `noise` per sample,
//! symbols of unit energy. The feed-forward filter reads `y(k + first)` to `y(k + first + nf - 1)` for symbol `k`; every
//! earlier symbol that reaches that window is cancelled by the feedback filter (with decisions or with known symbols). The
//! feed-forward taps solve `R f = g_0` with `R = sum_{d >= 0} g_d g_d^H + noise I` by a complex Cholesky factorisation, `g_d` the
//! window's response to symbol `k + d`. The output is unbiased: divided by the gain `g = f^H g_0`; its expected noise is
//! `(1 - g) / g`.
//!
//! [`MmseDfe::linear`] designs the equaliser without the feedback filter (`R` over every `d`).
//!
//! [`estimate_channel`] fits the channel taps by least squares where the symbols are known (training, pilots, or decisions), and
//! [`MmseDfe::run`] equalises a block, feeding back the known symbols where there are any and its own decisions elsewhere.
//!
//! ```
//! use sdrkit_algo_modem::mmse_dfe::MmseDfe;
//! use sdrkit_algo_modem::Complex32;
//! // One path and an echo one symbol later at half the amplitude.
//! let h = [Complex32::new(1.0, 0.0), Complex32::new(0.5, 0.0)];
//! let d = MmseDfe::design(&h, 0, -2, 7, 1e-3).unwrap();
//! assert!(d.gain() > 0.99);
//! // Symbols up to three before the current one reach the window from lag -2.
//! assert_eq!(d.feedback().len(), 3);
//! ```

use crate::error::{ModemError, Param};
use sdrkit_core::Complex32;

/// The largest feed-forward filter.
pub const MAX_FF: usize = 64;

/// A designed equaliser.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MmseDfe {
    /// The feed-forward taps, already conjugated: the output is `sum w_i y(k + first + i)`.
    w: Vec<Complex32>,
    first: i32,
    /// The feedback taps: `b[j]` multiplies the symbol `j + 1` before the current one.
    b: Vec<Complex32>,
    gain: f32,
}

impl MmseDfe {
    /// Designs the equaliser for channel `h` (taps at lags `lag0`, `lag0 + 1`, ...), a feed-forward window of `nf` samples from
    /// lag `first` relative to the symbol, and noise variance `noise` (the diagonal loading, at least 1e-6).
    ///
    /// # Errors
    ///
    /// [`ModemError`] for an empty channel, a window of 0 or more than [`MAX_FF`] taps, a noise that is not finite, or a system
    /// that cannot be solved (a channel that is not finite).
    pub fn design(
        h: &[Complex32],
        lag0: i32,
        first: i32,
        nf: usize,
        noise: f32,
    ) -> Result<MmseDfe, ModemError> {
        Self::solve(h, lag0, first, nf, noise, true)
    }

    /// Designs the linear MMSE equaliser for the same channel and window: no feedback filter, so the feed-forward taps also
    /// suppress the earlier symbols (`R` sums over every `d`). It takes no decisions, which suits symbols that a decoder reads
    /// at an error rate where decisions would be wrong too often.
    ///
    /// # Errors
    ///
    /// As [`design`](Self::design).
    pub fn linear(h: &[Complex32], lag0: i32, first: i32, nf: usize, noise: f32) -> Result<MmseDfe, ModemError> {
        Self::solve(h, lag0, first, nf, noise, false)
    }

    fn solve(h: &[Complex32], lag0: i32, first: i32, nf: usize, noise: f32, feedback: bool) -> Result<MmseDfe, ModemError> {
        if h.is_empty() {
            return Param::Taps.at_least(1.0, 0.0);
        }
        Param::Taps.check_count(nf, 1, MAX_FF)?;
        Param::NoiseVariance.check_finite(noise)?;
        let len = h.len() as i32;
        let tap = |l: i32| -> Complex32 {
            let i = l - lag0;
            if (0..len).contains(&i) {
                h[i as usize]
            } else {
                Complex32::default()
            }
        };
        // Symbol k + d reaches window sample i through h(first + i - d).
        let d_min = first - (lag0 + len - 1);
        let d_max = first + nf as i32 - 1 - lag0;
        let n = nf;
        let mut r = vec![Complex32::default(); n * n];
        let mut g0 = vec![Complex32::default(); n];
        let mut g = vec![Complex32::default(); n];
        for d in (if feedback { 0.max(d_min) } else { d_min })..=d_max {
            for (i, gi) in g.iter_mut().enumerate() {
                *gi = tap(first + i as i32 - d);
            }
            if d == 0 {
                g0.copy_from_slice(&g);
            }
            for i in 0..n {
                if g[i] == Complex32::default() {
                    continue;
                }
                for j in 0..n {
                    r[i * n + j] += g[i] * g[j].conj();
                }
            }
        }
        let load = noise.max(1e-6);
        for i in 0..n {
            r[i * n + i] += load;
        }
        let mut f = g0.clone();
        cholesky_solve(&mut r, &mut f, n)?;
        let gain: f32 = f.iter().zip(&g0).map(|(a, b)| (a.conj() * b).re).sum();
        if !(gain.is_finite() && gain > 0.0) {
            return Param::Estimator.degenerate();
        }
        let w: Vec<Complex32> = f.iter().map(|v| v.conj()).collect();
        let mut b = Vec::new();
        for d in (if feedback { d_min } else { 0 }..0).rev() {
            let mut c = Complex32::default();
            for (i, wi) in w.iter().enumerate() {
                c += wi * tap(first + i as i32 - d);
            }
            b.push(c);
        }
        Ok(MmseDfe { w, first, b, gain })
    }

    /// The feed-forward taps (conjugated, oldest sample first).
    pub fn feed_forward(&self) -> &[Complex32] {
        &self.w
    }

    /// The lag of the first feed-forward sample, relative to the symbol.
    pub fn first(&self) -> i32 {
        self.first
    }

    /// The feedback taps, the newest past symbol first.
    pub fn feedback(&self) -> &[Complex32] {
        &self.b
    }

    /// The gain `f^H g_0` (0 to 1).
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// The noise variance of the unbiased output that the design expects: `(1 - g) / g`.
    pub fn expected_noise(&self) -> f32 {
        (1.0 - self.gain).max(0.0) / self.gain.max(1e-6)
    }

    /// Equalises the symbol-spaced block `y`: `out[k]` is the unbiased output for symbol `k`. The feedback takes `known[k]` where
    /// it is `Some` and `decide(out[k])` elsewhere; samples outside `y` count as 0, as do symbols before the block.
    pub fn run(&self, y: &[Complex32], known: &[Option<Complex32>], decide: impl Fn(Complex32) -> Complex32, out: &mut Vec<Complex32>) {
        out.clear();
        let nb = self.b.len();
        let mut past: Vec<Complex32> = Vec::with_capacity(y.len());
        let mut window = vec![Complex32::default(); self.w.len()];
        let mut recent = vec![Complex32::default(); nb];
        for k in 0..y.len() {
            for (i, v) in window.iter_mut().enumerate() {
                let n = k as i64 + i64::from(self.first) + i as i64;
                *v = if (0..y.len() as i64).contains(&n) { y[n as usize] } else { Complex32::default() };
            }
            for (j, r) in recent.iter_mut().enumerate() {
                *r = if k > j { past[k - 1 - j] } else { Complex32::default() };
            }
            let z = self.output(&window, &recent);
            out.push(z);
            past.push(known.get(k).copied().flatten().unwrap_or_else(|| decide(z)));
        }
    }

    /// The unbiased output for a symbol: `window` holds the samples from lag [`first`](Self::first) on (at least as many as the
    /// feed-forward taps), `past` the earlier symbols, the newest first (at least as many as the feedback taps; missing ones
    /// count as 0).
    pub fn output(&self, window: &[Complex32], past: &[Complex32]) -> Complex32 {
        let mut z = Complex32::default();
        for (w, y) in self.w.iter().zip(window) {
            z += w * y;
        }
        for (b, a) in self.b.iter().zip(past) {
            z -= b * a;
        }
        z / self.gain.max(1e-6)
    }
}

/// The least-squares estimate of a symbol-spaced channel `y(n) = sum_i h(i) a(n - lag0 - i) + w(n)`, `i` from 0 to `len - 1`,
/// from the samples `n` whose symbols `a(n - lag0 - len + 1)` to `a(n - lag0)` are all known (`Some`). Returns the taps and the
/// noise variance of the residual.
///
/// # Errors
///
/// [`ModemError`] for a length of 0 or more than [`MAX_FF`], or fewer than `2 len` usable samples, or a system that cannot be
/// solved.
pub fn estimate_channel(y: &[Complex32], a: &[Option<Complex32>], lag0: i32, len: usize) -> Result<(Vec<Complex32>, f32), ModemError> {
    Param::Taps.check_count(len, 1, MAX_FF)?;
    let row = |n: usize| -> Option<Vec<Complex32>> {
        (0..len)
            .map(|i| {
                let m = n as i64 - i64::from(lag0) - i as i64;
                if m < 0 {
                    None
                } else {
                    a.get(m as usize).copied().flatten()
                }
            })
            .collect()
    };
    let mut r = vec![Complex32::default(); len * len];
    let mut h = vec![Complex32::default(); len];
    let mut rows = Vec::new();
    for (n, &yn) in y.iter().enumerate() {
        let Some(x) = row(n) else { continue };
        for i in 0..len {
            h[i] += x[i].conj() * yn;
            for j in 0..len {
                r[i * len + j] += x[i].conj() * x[j];
            }
        }
        rows.push(n);
    }
    if rows.len() < 2 * len {
        return Param::Estimator.degenerate();
    }
    let trace: f32 = (0..len).map(|i| r[i * len + i].re).sum();
    for i in 0..len {
        r[i * len + i] += 1e-6 * trace / len as f32;
    }
    cholesky_solve(&mut r, &mut h, len)?;
    let mut e = 0f64;
    for &n in &rows {
        let x = row(n).unwrap_or_default();
        let fit: Complex32 = x.iter().zip(&h).map(|(a, b)| a * b).sum();
        e += f64::from((y[n] - fit).norm_sqr());
    }
    Ok((h, (e / (rows.len() - len) as f64) as f32))
}

/// Solves `a x = b` for a Hermitian positive definite `a` (`n x n`, row major) by Cholesky factorisation, in place: `b` becomes
/// `x`, `a` is overwritten.
fn cholesky_solve(a: &mut [Complex32], b: &mut [Complex32], n: usize) -> Result<(), ModemError> {
    // a = L L^H, L stored in the lower triangle.
    for j in 0..n {
        let mut d = a[j * n + j].re;
        for k in 0..j {
            d -= a[j * n + k].norm_sqr();
        }
        if !(d.is_finite() && d > 0.0) {
            return Param::Estimator.degenerate();
        }
        let d = d.sqrt();
        a[j * n + j] = Complex32::new(d, 0.0);
        for i in j + 1..n {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= a[i * n + k] * a[j * n + k].conj();
            }
            a[i * n + j] = s / d;
        }
    }
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= a[i * n + k] * b[k];
        }
        b[i] = s / a[i * n + i].re;
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for k in i + 1..n {
            s -= a[k * n + i].conj() * b[k];
        }
        b[i] = s / a[i * n + i].re;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(re: f32, im: f32) -> Complex32 {
        Complex32::new(re, im)
    }

    /// A channel estimated from known symbols, then a block equalised with decisions only.
    #[test]
    fn an_estimated_channel_and_a_block_run() {
        let h = [c(1.0, 0.0), c(0.35, 0.1)];
        let mut seed = 11u32;
        let pts = [c(0.7, 0.7), c(-0.7, 0.7), c(-0.7, -0.7), c(0.7, -0.7)];
        let a: Vec<Complex32> = (0..400)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                pts[((seed >> 16) & 3) as usize]
            })
            .collect();
        let y: Vec<Complex32> = (0..a.len()).map(|n| h[0] * a[n] + if n > 0 { h[1] * a[n - 1] } else { c(0.0, 0.0) }).collect();
        let known: Vec<Option<Complex32>> = a.iter().enumerate().map(|(n, &v)| (n < 60).then_some(v)).collect();
        let (est, nv) = estimate_channel(&y, &known, -1, 4).unwrap();
        assert!((est[1] - h[0]).norm() < 1e-3 && (est[2] - h[1]).norm() < 1e-3 && est[0].norm() < 1e-3 && nv < 1e-6, "{est:?}");
        let d = MmseDfe::design(&est, -1, -2, 5, 1e-4).unwrap();
        let mut out = Vec::new();
        let nearest = |z: Complex32| *pts.iter().min_by(|p, q| (z - **p).norm().total_cmp(&(z - **q).norm())).unwrap();
        d.run(&y, &vec![None; y.len()], nearest, &mut out);
        assert!(out.iter().zip(&a).all(|(o, s)| (o - s).norm() < 0.05));
    }

    /// Symbols through `h` then the equaliser with correct decisions: the output equals the symbols.
    #[test]
    fn a_multipath_channel_is_undone() {
        let h = [c(0.3, 0.1), c(1.0, 0.0), c(0.0, -0.6), c(0.2, 0.2)];
        let lag0 = -1;
        let d = MmseDfe::design(&h, lag0, -3, 9, 1e-5).unwrap();
        let mut seed = 7u32;
        let a: Vec<Complex32> = (0..300)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let q = (seed >> 16) & 3;
                [c(1.0, 0.0), c(0.0, 1.0), c(-1.0, 0.0), c(0.0, -1.0)][q as usize]
            })
            .collect();
        let y: Vec<Complex32> = (0..a.len() as i32)
            .map(|n| {
                (0..h.len() as i32)
                    .map(|i| {
                        let m = n - (lag0 + i);
                        if (0..a.len() as i32).contains(&m) {
                            h[i as usize] * a[m as usize]
                        } else {
                            Complex32::default()
                        }
                    })
                    .sum()
            })
            .collect();
        for k in 20..280i32 {
            let w0 = (k + d.first()) as usize;
            let window = &y[w0..w0 + d.feed_forward().len()];
            let past: Vec<Complex32> = (1..=d.feedback().len())
                .map(|j| a[(k - j as i32) as usize])
                .collect();
            let z = d.output(window, &past);
            assert!(
                (z - a[k as usize]).norm() < 0.02,
                "{k}: {z} {}",
                a[k as usize]
            );
        }
        assert!(d.expected_noise() < 1e-3);
    }

    /// The linear design has no feedback taps and undoes an echo on both sides without decisions.
    #[test]
    fn the_linear_design_takes_no_decisions() {
        let h = [c(0.2, -0.1), c(1.0, 0.0), c(0.4, 0.3)];
        let d = MmseDfe::linear(&h, -1, -6, 13, 1e-4).unwrap();
        assert!(d.feedback().is_empty());
        let mut seed = 3u32;
        let a: Vec<Complex32> = (0..300)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                [c(1.0, 0.0), c(0.0, 1.0), c(-1.0, 0.0), c(0.0, -1.0)][((seed >> 16) & 3) as usize]
            })
            .collect();
        let y: Vec<Complex32> = (0..a.len() as i32)
            .map(|n| (0..3).map(|i| a.get((n + 1 - i) as usize).map_or(Complex32::default(), |s| h[i as usize] * s)).sum())
            .collect();
        let mut out = Vec::new();
        d.run(&y, &[], |z| z, &mut out);
        let worst = (20..280).map(|k| (out[k] - a[k]).norm()).fold(0f32, f32::max);
        assert!(worst < 0.1, "{worst}");
        assert!(d.expected_noise() < 0.01, "{}", d.expected_noise());
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(MmseDfe::design(&[], 0, 0, 3, 0.1).is_err());
        assert!(MmseDfe::design(&[c(1.0, 0.0)], 0, 0, 0, 0.1).is_err());
        assert!(MmseDfe::design(&[c(f32::NAN, 0.0)], 0, 0, 3, 0.1).is_err());
        let one = MmseDfe::design(&[c(2.0, 0.0)], 0, 0, 1, 1.0).unwrap();
        assert!((one.expected_noise() - 0.25).abs() < 1e-4);
    }
}

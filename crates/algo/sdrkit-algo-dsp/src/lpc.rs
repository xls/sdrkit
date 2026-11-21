//! Linear prediction in `f32`: the autocorrelation, the Levinson-Durbin recursion, bandwidth expansion, and the conversions
//! between predictor coefficients and line spectral pairs (LSPs, the angles of the roots of the sum and difference polynomials).
//!
//! Every transcendental function and every division by the error power runs in `f64` and rounds once to `f32`. Thus the
//! results do not depend on the `f32` math library of the platform.
//!
//! Standard: J. Makhoul, "Linear Prediction: A Tutorial Review", Proc. IEEE 63(4), 1975 (autocorrelation method, Levinson-Durbin).
//! P. Kabal and R. P. Ramachandran, "The Computation of Line Spectral Frequencies Using Chebyshev Polynomials", IEEE Trans. ASSP
//! 34(6), 1986 (the LSP root search).

use crate::{DspError, Param};

/// The highest predictor order that the functions in this module accept.
pub const MAX_ORDER: usize = 32;

/// Refuses an LSP order that is not even or not from 2 to [`MAX_ORDER`].
fn check_even_order(p: usize) -> Result<(), DspError> {
    if p == 0 || p > MAX_ORDER {
        return Param::Order.range(2.0, MAX_ORDER as f64, p as f64);
    }
    if !p.is_multiple_of(2) {
        return Param::Order.not_allowed(p as f64);
    }
    Ok(())
}

/// Writes the autocorrelation `r[j] = sum over i of x[i] x[i + j]` for `j = 0 .. r.len()`. The sum runs over the samples that
/// overlap. It runs in `f32`, in rising `i`. Errors if `r` is empty or longer than `x`.
///
/// ```
/// use sdrkit_algo_dsp::lpc::autocorrelate;
/// let mut r = [0.0; 3];
/// autocorrelate(&[1.0, 2.0, 3.0], &mut r).unwrap();
/// assert_eq!(r, [14.0, 8.0, 3.0]);
/// ```
pub fn autocorrelate(x: &[f32], r: &mut [f32]) -> Result<(), DspError> {
    if r.is_empty() || r.len() > x.len() {
        return Param::Lags.range(1.0, x.len() as f64, r.len() as f64);
    }
    for (j, rj) in r.iter_mut().enumerate() {
        let mut s = 0.0f32;
        for i in 0..x.len() - j {
            s += x[i] * x[i + j];
        }
        *rj = s;
    }
    Ok(())
}

/// Solves the normal equations of the autocorrelation method by the Levinson-Durbin recursion. The input is `r[0..=p]`. The
/// output is `a[0..=p]` with `a[0] = 1`, for the predictor error filter `A(z) = sum a[j] z^-j`. If a reflection coefficient has a
/// magnitude above 1, the recursion uses 0 in its place, so the filter stays stable. The return value is the prediction error
/// power. Errors if `r` and `a` differ in length, if `p` is 0 or above [`MAX_ORDER`], or if `r[0]` is not positive.
///
/// Standard: Makhoul 1975, equations (38a) to (38e).
///
/// ```
/// use sdrkit_algo_dsp::lpc::levinson_durbin;
/// // The autocorrelation of the first-order process x[n] = 0.5 x[n-1] + e[n].
/// let r = [1.0, 0.5, 0.25];
/// let mut a = [0.0; 3];
/// levinson_durbin(&r, &mut a).unwrap();
/// assert!((a[1] + 0.5).abs() < 1e-6 && a[2].abs() < 1e-6);
/// ```
pub fn levinson_durbin(r: &[f32], a: &mut [f32]) -> Result<f32, DspError> {
    let p = r.len().saturating_sub(1);
    if p == 0 || p > MAX_ORDER {
        return Param::Order.range(1.0, MAX_ORDER as f64, p as f64);
    }
    if r.len() != a.len() {
        return Param::Coefficients.length(r.len(), a.len());
    }
    if r[0].is_nan() || r[0] <= 0.0 {
        return Param::Power.positive(r[0] as f64);
    }
    let mut prev = [0.0f32; MAX_ORDER + 1];
    a.fill(0.0);
    a[0] = 1.0;
    let mut err = r[0];
    for i in 1..=p {
        prev[..i].copy_from_slice(&a[..i]);
        let mut s = 0.0f32;
        for j in 1..i {
            s += prev[j] * r[i - j];
        }
        let mut k = (-((r[i] + s) as f64) / err as f64) as f32;
        if k.abs() > 1.0 {
            k = 0.0;
        }
        a[i] = k;
        for j in 1..i {
            a[j] = prev[j] + k * prev[i - j];
        }
        err *= 1.0 - k * k;
    }
    Ok(err)
}

/// Multiplies `a[i]` by `gamma^i`. The power runs in `f64` and rounds once to `f32`. This moves the poles of `1 / A(z)` toward the origin and widens the formant bandwidths.
///
/// ```
/// use sdrkit_algo_dsp::lpc::bandwidth_expand;
/// let mut a = [1.0, 1.0, 1.0];
/// bandwidth_expand(&mut a, 0.5);
/// assert_eq!(a, [1.0, 0.5, 0.25]);
/// ```
pub fn bandwidth_expand(a: &mut [f32], gamma: f32) {
    for (i, v) in a.iter_mut().enumerate() {
        *v *= (gamma as f64).powf(i as f64) as f32;
    }
}

/// The settings of the LSP root search in [`lpc_to_lsp`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LspSearch {
    /// The grid step in `x = cos(omega)`. The search walks from `x = 1` down to `x = -1` in this step.
    pub step: f32,
    /// The number of bisections after a sign change brackets a root.
    pub bisections: u32,
}

impl Default for LspSearch {
    /// A step of 0.01 and 6 bisections: each root is within 0.01 / 64 in `x`.
    fn default() -> Self {
        LspSearch {
            step: 0.01,
            bisections: 6,
        }
    }
}

/// The value of `sum over i of c[m - i] T_i(x)`, with `T_i` the Chebyshev polynomials of the first kind and `m = c.len() - 1`.
fn chebyshev(c: &[f32], x: f32) -> f32 {
    let m = c.len() - 1;
    let (mut t0, mut t1) = (1.0f32, x);
    let mut sum = c[m];
    if m >= 1 {
        sum += c[m - 1] * x;
    }
    for i in 2..=m {
        let t = 2.0 * x * t1 - t0;
        sum += c[m - i] * t;
        t0 = t1;
        t1 = t;
    }
    sum
}

/// Finds the line spectral pairs of `A(z) = sum a[j] z^-j` (with `a[0] = 1` and an even order `p = a.len() - 1`). It forms
/// `P(z) = A(z) + z^-(p+1) A(1/z)` and `Q(z) = A(z) - z^-(p+1) A(1/z)`, removes their trivial roots, and searches for the roots of
/// each as a Chebyshev series in `x = cos(omega)`. Each root found is bracketed on a grid and then bisected (see [`LspSearch`]).
/// The roots of `P` and `Q` alternate: `lsp[0]` comes from `P`, `lsp[1]` from `Q`, and so on. `lsp[j]` is the angle in radians,
/// from 0 to pi, in rising order. The return value is the number of roots found. If it is less than `p`, the tail of `lsp` is
/// not written. Errors if `lsp` does not hold `p` values, if `p` is odd, 0 or above [`MAX_ORDER`], or if the step is not
/// positive.
///
/// Standard: Kabal and Ramachandran 1986, section III.
///
/// ```
/// use sdrkit_algo_dsp::lpc::{lpc_to_lsp, LspSearch};
/// // For A(z) = 1 the roots of P and Q lie at (j + 1) pi / (p + 1).
/// let mut a = [0.0f32; 11];
/// a[0] = 1.0;
/// let mut lsp = [0.0f32; 10];
/// assert_eq!(lpc_to_lsp(&a, &mut lsp, LspSearch::default()).unwrap(), 10);
/// assert!((lsp[0] - std::f32::consts::PI / 11.0).abs() < 1e-3);
/// ```
pub fn lpc_to_lsp(a: &[f32], lsp: &mut [f32], search: LspSearch) -> Result<usize, DspError> {
    let p = a.len().saturating_sub(1);
    check_even_order(p)?;
    if lsp.len() != p {
        return Param::Lsp.length(p, lsp.len());
    }
    if search.step.is_nan() || search.step <= 0.0 {
        return Param::GridStep.positive(search.step as f64);
    }
    let m = p / 2;
    let mut pc = [0.0f32; MAX_ORDER / 2 + 1];
    let mut qc = [0.0f32; MAX_ORDER / 2 + 1];
    pc[0] = 1.0;
    qc[0] = 1.0;
    for i in 1..=m {
        pc[i] = a[i] + a[p + 1 - i] - pc[i - 1];
        qc[i] = a[i] - a[p + 1 - i] + qc[i - 1];
    }
    for i in 0..m {
        pc[i] *= 2.0;
        qc[i] *= 2.0;
    }
    let (pc, qc) = (&pc[..=m], &qc[..=m]);

    let mut found = 0;
    let mut xl = 1.0f32;
    let mut xr = 0.0f32;
    for (j, out) in lsp.iter_mut().enumerate() {
        let c = if j % 2 == 0 { pc } else { qc };
        let mut fl = chebyshev(c, xl);
        let mut root = false;
        while !root && xr >= -1.0 {
            xr = xl - search.step;
            let fr = chebyshev(c, xr);
            if fr * fl < 0.0 || fr == 0.0 {
                let mut xm = xl;
                for _ in 0..search.bisections {
                    xm = (xl + xr) / 2.0;
                    let fm = chebyshev(c, xm);
                    if fm * fl > 0.0 {
                        xl = xm;
                        fl = fm;
                    } else {
                        xr = xm;
                    }
                }
                *out = (xm as f64).acos() as f32;
                xl = xm;
                found += 1;
                root = true;
            } else {
                fl = fr;
                xl = xr;
            }
        }
        if !root {
            break;
        }
    }
    Ok(found)
}

/// Rebuilds `A(z)` from its line spectral pairs. It forms `P(z) = (1 + z^-1) times the product of (1 - 2 cos(lsp[2i]) z^-1 +
/// z^-2)` and `Q(z) = (1 - z^-1) times the product of (1 - 2 cos(lsp[2i + 1]) z^-1 + z^-2)`, and writes `a[j] = (p_j + q_j) / 2`.
/// The coefficients come from the impulse response of each cascade of sections, in the order of `lsp`. Errors if `a` does not
/// hold `lsp.len() + 1` values, or if the order is odd, 0 or above [`MAX_ORDER`].
///
/// Standard: Kabal and Ramachandran 1986, equations (3) and (4).
///
/// ```
/// use sdrkit_algo_dsp::lpc::lsp_to_lpc;
/// let lsp: Vec<f32> = (1..=10).map(|j| j as f32 * std::f32::consts::PI / 11.0).collect();
/// let mut a = [0.0f32; 11];
/// lsp_to_lpc(&lsp, &mut a).unwrap();
/// assert!((a[0] - 1.0).abs() < 1e-6 && a[1..].iter().all(|v| v.abs() < 1e-5));
/// ```
pub fn lsp_to_lpc(lsp: &[f32], a: &mut [f32]) -> Result<(), DspError> {
    let p = lsp.len();
    check_even_order(p)?;
    if a.len() != p + 1 {
        return Param::Coefficients.length(p + 1, a.len());
    }
    let m = p / 2;
    let mut cp = [0.0f32; MAX_ORDER / 2];
    let mut cq = [0.0f32; MAX_ORDER / 2];
    for i in 0..m {
        cp[i] = -2.0 * ((lsp[2 * i] as f64).cos() as f32);
        cq[i] = -2.0 * ((lsp[2 * i + 1] as f64).cos() as f32);
    }
    // Delay memories of each second-order section: [z^-1, z^-2].
    let mut mp = [[0.0f32; 2]; MAX_ORDER / 2];
    let mut mq = [[0.0f32; 2]; MAX_ORDER / 2];
    let (mut dp, mut dq) = (0.0f32, 0.0f32);
    for (n, out) in a.iter_mut().enumerate() {
        let x = if n == 0 { 1.0f32 } else { 0.0 };
        let (mut yp, mut yq) = (x, x);
        for i in 0..m {
            let np = yp + cp[i] * mp[i][0] + mp[i][1];
            mp[i][1] = mp[i][0];
            mp[i][0] = yp;
            yp = np;
            let nq = yq + cq[i] * mq[i][0] + mq[i][1];
            mq[i][1] = mq[i][0];
            mq[i][0] = yq;
            yq = nq;
        }
        let fp = yp + dp;
        let fq = yq - dq;
        dp = yp;
        dq = yq;
        *out = (fp + fq) / 2.0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn autocorrelation_known_answer_and_errors() {
        let mut r = [0.0; 4];
        autocorrelate(&[1.0, -1.0, 2.0, 0.5], &mut r).unwrap();
        // r0 = 1 + 1 + 4 + 0.25, r1 = -1 - 2 + 1, r2 = 2 - 0.5, r3 = 0.5.
        assert_eq!(r, [6.25, -2.0, 1.5, 0.5]);
        assert!(autocorrelate(&[1.0], &mut r).is_err());
        assert!(autocorrelate(&[1.0], &mut []).is_err());
    }

    /// The Yule-Walker equations of the second-order process x[n] = -a1 x[n-1] - a2 x[n-2] + e[n] give
    /// rho1 = -a1 / (1 + a2) and rho2 = -a1 rho1 - a2 (Makhoul 1975, equation (12)).
    #[test]
    fn levinson_recovers_a_second_order_process() {
        let (a1, a2) = (-0.9f64, 0.2f64);
        let rho1 = -a1 / (1.0 + a2);
        let rho2 = -a1 * rho1 - a2;
        let r = [1.0, rho1 as f32, rho2 as f32, 0.0];
        // The third lag of the process: rho3 = -a1 rho2 - a2 rho1.
        let r = [r[0], r[1], r[2], (-a1 * rho2 - a2 * rho1) as f32];
        let mut a = [0.0; 4];
        let err = levinson_durbin(&r, &mut a).unwrap();
        assert!((a[1] as f64 - a1).abs() < 1e-5);
        assert!((a[2] as f64 - a2).abs() < 1e-5);
        assert!(a[3].abs() < 1e-5);
        // The error power is r0 (1 - k1^2)(1 - k2^2) with k2 = a2 and k1 = -rho1.
        let expect = (1.0 - rho1 * rho1) * (1.0 - a2 * a2);
        assert!((err as f64 - expect).abs() < 1e-5);
    }

    #[test]
    fn levinson_zeroes_an_unstable_reflection_and_refuses_bad_input() {
        // With r1 > r0, k1 = -2, which the recursion replaces by 0.
        let mut a = [0.0; 2];
        let err = levinson_durbin(&[1.0, 2.0], &mut a).unwrap();
        assert_eq!(a, [1.0, 0.0]);
        assert_eq!(err, 1.0);
        assert!(levinson_durbin(&[0.0, 0.0], &mut a).is_err());
        assert!(levinson_durbin(&[1.0], &mut [0.0]).is_err());
        assert!(levinson_durbin(&[1.0, 0.5, 0.1], &mut a).is_err());
    }

    #[test]
    fn lsp_of_a_flat_filter_are_evenly_spaced() {
        let mut a = [0.0f32; 11];
        a[0] = 1.0;
        let mut lsp = [0.0f32; 10];
        assert_eq!(lpc_to_lsp(&a, &mut lsp, LspSearch::default()).unwrap(), 10);
        for (j, &w) in lsp.iter().enumerate() {
            let want = (j + 1) as f32 * PI / 11.0;
            // A root is within step / 2^bisections in x, which is under 0.002 rad here.
            assert!((w - want).abs() < 2e-3, "{j}: {w} {want}");
        }
    }

    #[test]
    fn lsp_round_trip_of_a_stable_filter() {
        // A tenth-order filter from a speech-like autocorrelation.
        let x: Vec<f32> = (0..320)
            .map(|n| {
                let t = n as f32;
                (0.3 * t).sin() + 0.5 * (0.9 * t).sin() + 0.2 * (2.1 * t).cos()
            })
            .collect();
        let mut r = [0.0f32; 11];
        autocorrelate(&x, &mut r).unwrap();
        r[0] *= 1.0001;
        let mut a = [0.0f32; 11];
        levinson_durbin(&r, &mut a).unwrap();
        bandwidth_expand(&mut a, 0.994);
        let mut lsp = [0.0f32; 10];
        let search = LspSearch {
            step: 0.01,
            bisections: 20,
        };
        assert_eq!(lpc_to_lsp(&a, &mut lsp, search).unwrap(), 10);
        assert!(lsp.windows(2).all(|w| w[0] < w[1]));
        let mut back = [0.0f32; 11];
        lsp_to_lpc(&lsp, &mut back).unwrap();
        for (u, v) in a.iter().zip(back) {
            assert!((u - v).abs() < 2e-3, "{u} {v}");
        }
    }

    #[test]
    fn conversions_refuse_bad_sizes() {
        let mut lsp = [0.0f32; 3];
        assert!(lpc_to_lsp(&[1.0, 0.0, 0.0, 0.0], &mut lsp, LspSearch::default()).is_err());
        let bad = LspSearch {
            step: 0.0,
            bisections: 1,
        };
        assert!(lpc_to_lsp(&[1.0, 0.0, 0.0], &mut [0.0; 2], bad).is_err());
        assert!(lsp_to_lpc(&[0.1, 0.2], &mut [0.0; 2]).is_err());
        assert!(lsp_to_lpc(&[], &mut [0.0]).is_err());
    }
}

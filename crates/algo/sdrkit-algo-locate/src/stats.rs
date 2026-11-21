//! Quantiles of the normal and chi-square distributions in closed form.

/// The quantile of the standard normal distribution: the `x` with `Phi(x) = p`, for `p` in (0, 1). The rational approximation of
/// M. Abramowitz and I. A. Stegun, "Handbook of Mathematical Functions", 26.2.23 (absolute error below 4.5e-4). NaN outside (0, 1).
///
/// ```
/// use sdrkit_algo_locate::stats::normal_quantile;
/// assert!((normal_quantile(0.975) - 1.95996).abs() < 4.5e-4);
/// ```
pub fn normal_quantile(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        return f64::NAN;
    }
    const C: [f64; 3] = [2.515_517, 0.802_853, 0.010_328];
    const D: [f64; 3] = [1.432_788, 0.189_269, 0.001_308];
    // 26.2.23 gives the upper-tail point for a tail probability of at most 0.5; the lower half follows by symmetry.
    let tail = if p < 0.5 { p } else { 1.0 - p };
    let t = (-2.0 * tail.ln()).sqrt();
    let x = t - (C[0] + t * (C[1] + t * C[2])) / (1.0 + t * (D[0] + t * (D[1] + t * D[2])));
    if p < 0.5 {
        -x
    } else {
        x
    }
}

/// The quantile of the chi-square distribution with `dof` degrees of freedom (above 0) at probability `p` in (0, 1): the
/// approximation of E. B. Wilson and M. M. Hilferty (1931), `k (1 - 2/(9k) + z sqrt(2/(9k)))^3` with `z` from
/// [`normal_quantile`], never below 0. NaN for bad input.
///
/// ```
/// use sdrkit_algo_locate::stats::chi_square_quantile;
/// // The tabulated 99.9 % point for 10 degrees of freedom is 29.588.
/// assert!((chi_square_quantile(0.999, 10.0) - 29.588).abs() < 0.3);
/// ```
pub fn chi_square_quantile(p: f64, dof: f64) -> f64 {
    if !(dof > 0.0 && dof.is_finite()) {
        return f64::NAN;
    }
    let z = normal_quantile(p);
    let a = 2.0 / (9.0 * dof);
    let c = 1.0 - a + z * a.sqrt();
    (dof * c * c * c).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_match_tables() {
        // Normal: tabulated points.
        for (p, x) in [
            (0.5, 0.0),
            (0.8413, 0.99982),
            (0.95, 1.64485),
            (0.999, 3.09023),
            (0.001, -3.09023),
        ] {
            assert!((normal_quantile(p) - x).abs() < 4.5e-4, "{p}");
        }
        assert!(normal_quantile(0.0).is_nan() && normal_quantile(1.0).is_nan());
        // Chi-square: tabulated points (Wilson-Hilferty is within 3 % from 2 degrees of freedom up, closer for more).
        for (p, k, x, tol) in [
            (0.95, 2.0, 5.991, 0.03),
            (0.999, 2.0, 13.816, 0.03),
            (0.999, 3.0, 16.266, 0.03),
            (0.999, 10.0, 29.588, 0.01),
            (0.95, 30.0, 43.773, 0.01),
        ] {
            let q = chi_square_quantile(p, k);
            assert!((q / x - 1.0).abs() < tol, "{p} {k} {q}");
        }
        assert!(chi_square_quantile(0.999, 0.0).is_nan());
    }
}

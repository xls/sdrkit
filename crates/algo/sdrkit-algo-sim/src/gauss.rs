//! Gaussian values by the ziggurat method with 128 layers, and the inverse of the normal distribution function. The layer index
//! and the position come from separate bits of one 64-bit draw (Doornik, "An Improved Ziggurat Method", 2005). The tables are
//! built on first use from `R = 3.442619855899` and `V = 9.91256303526217e-3`.
//!
//! Standard: Marsaglia and Tsang, "The Ziggurat Method for Generating Random Variables", Journal of Statistical Software 5(8), 2000.

use crate::rng::Rng;
use std::sync::OnceLock;

/// The number of layers.
const LAYERS: usize = 128;
/// The start of the tail, `R`.
pub const ZIGGURAT_R: f64 = 3.442_619_855_899;
/// The area of each layer, `V`.
pub const ZIGGURAT_V: f64 = 9.912_563_035_262_17e-3;
const M1: f64 = 2_147_483_648.0; // 2^31

/// The ziggurat tables: `k[i]` the acceptance limit for layer `i` (in units of 2^-31 of the position), `w[i]` the position scale,
/// `f[i]` the density at the layer's edge.
#[derive(Debug)]
pub struct Ziggurat {
    k: [u32; LAYERS],
    w: [f64; LAYERS],
    f: [f64; LAYERS],
}

fn build() -> Ziggurat {
    let mut k = [0u32; LAYERS];
    let mut w = [0f64; LAYERS];
    let mut f = [0f64; LAYERS];
    let mut dn = ZIGGURAT_R;
    let mut tn = dn;
    let q = ZIGGURAT_V / (-0.5 * dn * dn).exp();
    k[0] = ((dn / q) * M1) as u32;
    k[1] = 0;
    w[0] = q / M1;
    w[LAYERS - 1] = dn / M1;
    f[0] = 1.0;
    f[LAYERS - 1] = (-0.5 * dn * dn).exp();
    for i in (1..=LAYERS - 2).rev() {
        dn = (-2.0 * (ZIGGURAT_V / dn + (-0.5 * dn * dn).exp()).ln()).sqrt();
        k[i + 1] = ((dn / tn) * M1) as u32;
        tn = dn;
        f[i] = (-0.5 * dn * dn).exp();
        w[i] = dn / M1;
    }
    Ziggurat { k, w, f }
}

/// The tables, built on first use and shared (read only) by every generator.
pub fn tables() -> &'static Ziggurat {
    static T: OnceLock<Ziggurat> = OnceLock::new();
    T.get_or_init(build)
}

/// One standard normal value from `rng` using the tables `z`.
#[inline]
pub(crate) fn sample(z: &Ziggurat, rng: &mut Rng) -> f64 {
    let u = rng.next_u64();
    let iz = (u & (LAYERS as u64 - 1)) as usize;
    let hz = (u >> 32) as u32 as i32;
    if hz.unsigned_abs() < z.k[iz] {
        return hz as f64 * z.w[iz];
    }
    slow(z, rng, hz, iz)
}

#[cold]
fn slow(z: &Ziggurat, rng: &mut Rng, mut hz: i32, mut iz: usize) -> f64 {
    loop {
        let x = hz as f64 * z.w[iz];
        if iz == 0 {
            // The tail beyond R (Marsaglia 1964): x = -ln(U1)/R, y = -ln(U2), accept when 2y > x^2.
            loop {
                let x = -rng.uniform_open().ln() / ZIGGURAT_R;
                let y = -rng.uniform_open().ln();
                if y + y >= x * x {
                    return if hz > 0 {
                        ZIGGURAT_R + x
                    } else {
                        -ZIGGURAT_R - x
                    };
                }
            }
        }
        if z.f[iz] + rng.uniform() * (z.f[iz - 1] - z.f[iz]) < (-0.5 * x * x).exp() {
            return x;
        }
        let u = rng.next_u64();
        iz = (u & (LAYERS as u64 - 1)) as usize;
        hz = (u >> 32) as u32 as i32;
        if hz.unsigned_abs() < z.k[iz] {
            return hz as f64 * z.w[iz];
        }
    }
}

/// The inverse of the standard normal distribution function, `Phi^-1(p)` for `p` in `(0, 1)`: Acklam's rational approximation
/// (relative error below 1.15e-9) refined by one Halley step on `erfc` from `sdrkit_algo_dsp::pulse`, which brings it to about 1e-15.
/// Returns NaN for `p` outside `(0, 1)` or NaN. Used to place the frequencies of a Gaussian Doppler spectrum; not for per-sample use.
pub fn inverse_normal_cdf(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        return f64::NAN;
    }
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let lo = 0.02425;
    let x = if p < lo {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - lo {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    // One Halley step: e = Phi(x) - p, u = e sqrt(2 pi) e^(x^2/2).
    let e = 0.5 * sdrkit_algo_dsp::pulse::erfc(-x / std::f64::consts::SQRT_2) - p;
    let u = e * (2.0 * std::f64::consts::PI).sqrt() * (0.5 * x * x).exp();
    x - u / (1.0 + 0.5 * x * u)
}

/// The standard normal distribution function `Phi(x) = erfc(-x / sqrt 2) / 2`.
pub fn normal_cdf(x: f64) -> f64 {
    0.5 * sdrkit_algo_dsp::pulse::erfc(-x / std::f64::consts::SQRT_2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_has_the_same_area_and_the_top_closes_at_the_peak() {
        // Rebuild the layer edges x_i from the recursion and check the construction the tables come from: each rectangle
        // x_i (f(x_{i+1}) - f(x_i)) has area V, the base strip R f(R) + tail has area V, and the last edge reaches x = 0 (f = 1).
        let f = |x: f64| (-0.5 * x * x).exp();
        let tail = (std::f64::consts::PI / 2.0).sqrt()
            * sdrkit_algo_dsp::pulse::erfc(ZIGGURAT_R / std::f64::consts::SQRT_2);
        assert!((ZIGGURAT_R * f(ZIGGURAT_R) + tail - ZIGGURAT_V).abs() < 1e-12);
        let mut x = ZIGGURAT_R;
        for _ in 0..LAYERS - 2 {
            let next = (-2.0 * (ZIGGURAT_V / x + f(x)).ln()).sqrt();
            assert!((x * (f(next) - f(x)) - ZIGGURAT_V).abs() < 1e-12);
            x = next;
        }
        // The top layer: from x_1 to the peak, area V again (to the precision of the published R).
        assert!(
            (x * (1.0 - f(x)) - ZIGGURAT_V).abs() < 1e-9,
            "top layer {x}"
        );
        let t = tables();
        assert_eq!(t.k[1], 0);
        assert_eq!(t.f[0], 1.0);
    }

    #[test]
    fn the_inverse_cdf_matches_published_quantiles() {
        // Standard normal quantiles (Abramowitz and Stegun table 26.5): Phi^-1(0.975) = 1.959963984540054,
        // Phi^-1(0.995) = 2.575829303548901, Phi^-1(0.8413447460685429) = 1.
        assert!((inverse_normal_cdf(0.975) - 1.959_963_984_540_054).abs() < 1e-12);
        assert!((inverse_normal_cdf(0.995) - 2.575_829_303_548_901).abs() < 1e-12);
        assert!((inverse_normal_cdf(0.841_344_746_068_542_9) - 1.0).abs() < 1e-12);
        assert!((inverse_normal_cdf(1e-10) + 6.361_340_902_404_056).abs() < 1e-9);
        assert_eq!(inverse_normal_cdf(0.5), 0.0);
        assert!(inverse_normal_cdf(0.0).is_nan() && inverse_normal_cdf(1.0).is_nan());
        assert!(inverse_normal_cdf(f64::NAN).is_nan());
        assert!((normal_cdf(1.959_963_984_540_054) - 0.975).abs() < 1e-15);
    }

    #[test]
    fn draws_follow_the_normal_distribution() {
        // Kolmogorov-Smirnov against Phi over a million draws (the 1 % critical value is 1.63 / sqrt(n) = 0.0016), the moments,
        // and the tail beyond R (2 (1 - Phi(R)) = 5.76e-4).
        let mut r = Rng::new(42);
        let n = 1_000_000;
        let mut v: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let mean = v.iter().sum::<f64>() / n as f64;
        let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64;
        let kurt = v.iter().map(|x| (x - mean).powi(4)).sum::<f64>() / n as f64 / (var * var);
        assert!(mean.abs() < 0.004, "mean {mean}");
        assert!((var - 1.0).abs() < 0.005, "variance {var}");
        assert!((kurt - 3.0).abs() < 0.03, "kurtosis {kurt}");
        let tail = v.iter().filter(|x| x.abs() > ZIGGURAT_R).count() as f64 / n as f64;
        let want = 2.0 * (1.0 - normal_cdf(ZIGGURAT_R));
        assert!((tail - want).abs() < 1e-4, "tail {tail} want {want}");
        v.sort_by(|a, b| a.total_cmp(b));
        let mut d = 0.0f64;
        for (i, &x) in v.iter().enumerate() {
            let c = normal_cdf(x);
            d = d
                .max((c - i as f64 / n as f64).abs())
                .max(((i + 1) as f64 / n as f64 - c).abs());
        }
        assert!(d < 1.63 / (n as f64).sqrt(), "KS distance {d}");
    }
}

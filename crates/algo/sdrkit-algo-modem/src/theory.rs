//! Closed forms from communication theory, for tests, link budgets and displays: error rates over the AWGN channel and the
//! Cramer-Rao bound of a tone's frequency.
//!
//! Signal-to-noise ratios are linear (not dB) unless the name says `_db`. `ebn0` is the energy per bit over the one-sided noise
//! density; `esn0 = ebn0 * bits per symbol`.
//!
//! ```
//! use sdrkit_algo_dsp::meter::db_to_power;
//! use sdrkit_algo_modem::theory::ber_bpsk;
//! // BPSK at 9.6 dB Eb/N0: about 1e-5 (the textbook value).
//! let p = ber_bpsk(db_to_power(9.6));
//! assert!(p > 0.9e-5 && p < 1.1e-5);
//! ```

use sdrkit_algo_dsp::pulse::erfc;

/// The Gaussian tail `Q(x) = P(N(0,1) > x) = erfc(x / sqrt 2) / 2`.
pub fn q(x: f64) -> f64 {
    0.5 * erfc(x / std::f64::consts::SQRT_2)
}

/// Bit error rate of coherent BPSK (and of Gray QPSK per bit): `Q(sqrt(2 Eb/N0))`.
pub fn ber_bpsk(ebn0: f64) -> f64 {
    q((2.0 * ebn0).sqrt())
}

/// Bit error rate of differentially detected DBPSK: `exp(-Eb/N0) / 2`.
pub fn ber_dbpsk(ebn0: f64) -> f64 {
    0.5 * (-ebn0).exp()
}

/// Bit error rate of `m`-ary orthogonal FSK detected non-coherently (energy per tone, the largest wins), at `ebn0`: the symbol error
/// rate times `m / (2 (m - 1))`. The symbol error rate is the integral over the right tone's Rician envelope `r` of the chance that
/// some other tone's Rayleigh envelope exceeds it, `1 - (1 - e^(-r^2/2))^(m-1)` (Proakis, *Digital Communications*, 4.5-43, whose
/// alternating binomial sum is the same value but cancels catastrophically for large `m`); evaluated in the log domain, so any `m`
/// from 2 to 2^16 (LoRa SF 12 included) is exact. For `m = 2` it is `exp(-Eb / 2N0) / 2`.
pub fn ber_fsk_noncoherent(m: usize, ebn0: f64) -> f64 {
    if m < 2 || ebn0.is_nan() || ebn0 < 0.0 {
        return f64::NAN;
    }
    let k = (m as f64).log2();
    // Noise variance 1 per dimension; the right tone's amplitude a with a^2 / 2 = Es / N0.
    let a = (2.0 * k * ebn0).sqrt();
    let lo = (a - 12.0).max(0.0);
    let hi = a + 12.0;
    let steps = 24_000;
    let h = (hi - lo) / steps as f64;
    let mut ps = 0.0;
    for i in 0..=steps {
        let r = lo + i as f64 * h;
        if r <= 0.0 {
            continue;
        }
        // ln of the Rician density r exp(-(r^2 + a^2)/2) I0(a r).
        let ln_f = r.ln() - 0.5 * (r - a) * (r - a) + ln_i0_scaled(a * r);
        let u = (-0.5 * r * r).exp();
        let lose = -((m as f64 - 1.0) * (-u).ln_1p()).exp_m1();
        let w = if i == 0 || i == steps { 0.5 } else { 1.0 };
        ps += w * ln_f.exp() * lose;
    }
    ps * h * m as f64 / (2.0 * (m as f64 - 1.0))
}

/// `ln I0(x) - x` for `x >= 0`.
fn ln_i0_scaled(x: f64) -> f64 {
    if x < 15.0 {
        sdrkit_algo_dsp::window::bessel_i0(x).ln() - x
    } else {
        // Abramowitz and Stegun 9.7.1.
        let t = 1.0 / (8.0 * x);
        -0.5 * (std::f64::consts::TAU * x).ln() + (1.0 + t + 4.5 * t * t + 37.5 * t * t * t).ln()
    }
}

/// The exact bit error rate of `levels`-PAM with the axis Gray code `code[level]` (levels from the most negative) at `snr` =
/// (mean symbol energy) / (noise variance on the axis): every transmitted level, every decision region, weighted by the bits that
/// differ.
pub fn ber_pam_code(code: &[u32], snr: f64) -> f64 {
    let l = code.len();
    let bits = (l as f64).log2();
    // Levels -(l-1) .. l-1 step 2; energy (l^2 - 1) / 3.
    let es = (l * l - 1) as f64 / 3.0;
    let sigma = (es / snr).sqrt();
    let level = |i: usize| (2 * i) as f64 - (l - 1) as f64;
    let mut sum = 0.0;
    for i in 0..l {
        for j in 0..l {
            if i == j {
                continue;
            }
            let lo = if j == 0 {
                f64::NEG_INFINITY
            } else {
                level(j) - 1.0
            };
            let hi = if j == l - 1 {
                f64::INFINITY
            } else {
                level(j) + 1.0
            };
            let p = q((lo - level(i)) / sigma) - q((hi - level(i)) / sigma);
            sum += p * (code[i] ^ code[j]).count_ones() as f64;
        }
    }
    sum / (l as f64 * bits)
}

/// The exact bit error rate of square `m`-QAM with a Gray code on each axis (any of the standard maps: they all have the same
/// rate), at `ebn0`. 4-QAM equals [`ber_bpsk`]; for 16-QAM it is `3/4 Q(x) + 1/2 Q(3x) - 1/4 Q(5x)` with `x = sqrt(4 Eb / 5 N0)`
/// (K. Cho and D. Yoon, IEEE Trans. Commun. 50(7), 2002).
pub fn ber_square_qam(m: usize, ebn0: f64) -> f64 {
    let k = (m as f64).log2();
    let side = (m as f64).sqrt().round() as usize;
    let code: Vec<u32> = (0..side as u32).map(sdrkit_util_bits::gray::gray).collect();
    // Per axis: half the symbol energy over half the noise.
    ber_pam_code(&code, k * ebn0)
}

/// The Cramer-Rao bound on the standard deviation of a frequency estimate, in cycles per sample, for a complex tone of `n` samples
/// at `snr` (tone power over complex noise variance): `sqrt(6 / (snr n (n^2 - 1))) / 2 pi` (D. C. Rife and R. R. Boorstyn, IEEE
/// Trans. Inf. Theory 20(5), 1974).
pub fn crb_frequency(snr: f64, n: usize) -> f64 {
    let n = n as f64;
    (6.0 / (snr * n * (n * n - 1.0))).sqrt() / std::f64::consts::TAU
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_forms_match_published_values() {
        assert!((q(0.0) - 0.5).abs() < 1e-15);
        assert!((q(3.0) - 1.349_898e-3).abs() < 1e-9);
        // 16-QAM: the Cho-Yoon form.
        for e in [1.0f64, 5.0, 20.0] {
            let x = (0.8 * e).sqrt();
            let want = 0.75 * q(x) + 0.5 * q(3.0 * x) - 0.25 * q(5.0 * x);
            assert!((ber_square_qam(16, e) - want).abs() < 1e-12, "{e}");
            assert!((ber_square_qam(4, e) - ber_bpsk(e)).abs() < 1e-12);
        }
        assert!((ber_fsk_noncoherent(2, 4.0) / (0.5 * (-2.0f64).exp()) - 1.0).abs() < 1e-6);
        // 4-FSK against the closed alternating sum: Ps = sum_{n=1}^{3} (-1)^(n+1) C(3, n) / (n+1) exp(-n Es / ((n+1) N0)).
        let es = 2.0 * 3.0;
        let ps: f64 = [(1.0, 3.0), (2.0, 3.0), (3.0, 1.0)]
            .iter()
            .map(|&(n, c): &(f64, f64)| if n as i32 % 2 == 1 { 1.0 } else { -1.0 } * c / (n + 1.0) * (-n * es / (n + 1.0)).exp())
            .sum();
        assert!((ber_fsk_noncoherent(4, 3.0) / (ps * 4.0 / 6.0) - 1.0).abs() < 1e-6);
        // Large m stays a probability that falls with SNR.
        let a = ber_fsk_noncoherent(4096, 1.0);
        let b = ber_fsk_noncoherent(4096, 2.0);
        assert!(a > b && b > 0.0 && a < 0.5);
        assert!((crb_frequency(1.0, 2) - 1.0 / std::f64::consts::TAU).abs() < 1e-12);
    }
}

//! Fading statistics of the complex envelope of one carrier: the scintillation index S4 (ITU-R P.531), the Rician K-factor
//! by moments (ITU-R P.1057), the level percentiles against the mean power, the downward crossings of a threshold with the
//! crossing rate and the average fade length (Jakes 1974), and the RMS Doppler spread from a Welch spectrum.
//!
//! For envelope powers `P` with mean `m` and population variance `v`, `g = v / m^2`: `S4 = sqrt(g)`; `K` solves
//! `g = (1 + 2K) / (1 + K)^2`, `K = ((1 - g) + sqrt(1 - g)) / g` for `1e-4 <= g < 1`, 0 (Rayleigh) for `g >= 1` and
//! [`K_NO_FADING`] for `g < 1e-4`. For Rayleigh fading at the largest Doppler `fd` and the threshold `u = 0.1` of the mean
//! power, the crossing rate is `0.717 fd`, the average fade `0.133 / fd` and the RMS Doppler `fd / sqrt(2)`.

use crate::spectrum::{Scaling, Welch};
use crate::window::Window;
use num_complex::Complex32;

/// The K-factor of an envelope whose power hardly varies (40 dB).
pub const K_NO_FADING: f64 = 1e4;
/// The segment of the Doppler spectrum.
pub const DOPPLER_SEGMENT: usize = 2048;

/// The statistics of one window of an envelope.
#[derive(Clone, Debug, PartialEq)]
pub struct FadeStats {
    /// The mean power.
    pub mean: f64,
    /// The scintillation index.
    pub s4: f64,
    /// The Rician K-factor (linear; 0 for Rayleigh).
    pub k: f64,
    /// The power at each share asked for against the mean, dB.
    pub levels_db: Vec<f64>,
    /// Downward crossings of the threshold.
    pub crossings: usize,
    /// Downward crossings per second.
    pub crossing_rate_hz: f64,
    /// The time below the threshold over the crossings, s; `None` without a crossing.
    pub fade_length_s: Option<f64>,
    /// The power-weighted mean frequency of the envelope spectrum, Hz; `None` when the window is shorter than a segment.
    pub doppler_mean_hz: Option<f64>,
    /// The RMS Doppler spread about that mean, Hz.
    pub doppler_rms_hz: Option<f64>,
}

/// The Rician K-factor of the normalised power variance `g` (module docs).
pub fn rician_k(g: f64) -> f64 {
    if g >= 1.0 {
        0.0
    } else if g < 1e-4 {
        K_NO_FADING
    } else {
        ((1.0 - g) + (1.0 - g).sqrt()) / g
    }
}

/// The statistics of `env` at the envelope rate `rate_hz`, whose filter passes `pass_hz`, the threshold at `u` times the
/// mean power, the levels at `shares` (index `floor(f N)` of the powers sorted from the lowest). `None` for an empty window
/// or no power.
pub fn fade_stats(
    env: &[Complex32],
    rate_hz: f64,
    pass_hz: f64,
    u: f64,
    shares: &[f64],
) -> Option<FadeStats> {
    let n = env.len();
    if n == 0 {
        return None;
    }
    let p: Vec<f64> = env.iter().map(|z| f64::from(z.norm_sqr())).collect();
    let m = p.iter().sum::<f64>() / n as f64;
    if m <= 1e-30 {
        return None;
    }
    let v = p.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n as f64;
    let g = v / (m * m);
    let mut sorted = p.clone();
    sorted.sort_by(f64::total_cmp);
    let levels_db = shares
        .iter()
        .map(|f| {
            let k = ((f * n as f64).floor() as usize).min(n - 1);
            10.0 * (sorted[k] / m).max(1e-30).log10()
        })
        .collect();
    let thr = u * m;
    let mut crossings = 0;
    let mut below = 0usize;
    for (i, &x) in p.iter().enumerate() {
        if x < thr {
            below += 1;
            if i > 0 && p[i - 1] >= thr {
                crossings += 1;
            }
        }
    }
    let span = n as f64 / rate_hz;
    let (dm, drms) = doppler(env, rate_hz, pass_hz).unzip();
    Some(FadeStats {
        mean: m,
        s4: g.sqrt(),
        k: rician_k(g),
        levels_db,
        crossings,
        crossing_rate_hz: crossings as f64 / span,
        fade_length_s: (crossings > 0).then(|| below as f64 / rate_hz / crossings as f64),
        doppler_mean_hz: dm,
        doppler_rms_hz: drms,
    })
}

/// The power-weighted mean and RMS spread of the Welch spectrum of `env` (Hann segments of [`DOPPLER_SEGMENT`], half
/// overlap), bins over `0.9 pass_hz` left out.
fn doppler(env: &[Complex32], rate_hz: f64, pass_hz: f64) -> Option<(f64, f64)> {
    let n = DOPPLER_SEGMENT;
    if env.len() < n {
        return None;
    }
    let mut w = Welch::new(n, Window::Hann, 0.5, Scaling::Power).ok()?;
    let mut out = vec![0f32; n];
    w.compute(env, &mut out).ok()?;
    let bin = rate_hz / n as f64;
    let edge = 0.9 * pass_hz;
    let (mut s0, mut s1, mut s2) = (0.0, 0.0, 0.0);
    for (i, &q) in out.iter().enumerate() {
        let f = (i as f64 - (n / 2) as f64) * bin;
        if f.abs() > edge {
            continue;
        }
        let q = f64::from(q);
        s0 += q;
        s1 += q * f;
        s2 += q * f * f;
    }
    if s0 <= 0.0 {
        return None;
    }
    let mean = s1 / s0;
    Some((mean, (s2 / s0 - mean * mean).max(0.0).sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_envelope_reads_no_fading_and_gaussian_reads_rayleigh() {
        let c = vec![Complex32::new(0.5, 0.5); 10_000];
        let s = fade_stats(&c, 1000.0, 220.0, 0.1, &[0.01, 0.5]).unwrap();
        assert!(s.s4 < 1e-6 && s.k == K_NO_FADING && s.crossings == 0 && s.fade_length_s.is_none());
        assert!(s.levels_db.iter().all(|l| l.abs() < 1e-5));
        let mut st = 0x9e37_79b9_7f4a_7c15u64;
        let mut u = || {
            st ^= st << 13;
            st ^= st >> 7;
            st ^= st << 17;
            ((st >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        for _ in 0..3 {
            let g: Vec<Complex32> = (0..200_000)
                .map(|_| {
                    let r = (-u().ln()).sqrt();
                    Complex32::from_polar(r as f32, (std::f64::consts::TAU * u()) as f32)
                })
                .collect();
            let s = fade_stats(&g, 1000.0, 220.0, 0.1, &[0.5]).unwrap();
            // K is 0 for g >= 1; the estimate of g scatters by about 0.006 at this length, so K stays under 0.2 (-7 dB).
            assert!((s.s4 - 1.0).abs() < 0.05 && s.k < 0.2, "{} {}", s.s4, s.k);
            assert!(
                (s.levels_db[0] - 10.0 * 2f64.ln().log10()).abs() < 0.1,
                "{}",
                s.levels_db[0]
            );
        }
        assert!((rician_k(0.5) - (0.5 + 0.5f64.sqrt()) / 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_line_reads_its_offset_and_no_spread() {
        let rate = 1000.0;
        let x: Vec<Complex32> = (0..20_000)
            .map(|i| {
                Complex32::from_polar(1.0, (std::f64::consts::TAU * 30.0 * i as f64 / rate) as f32)
            })
            .collect();
        let s = fade_stats(&x, rate, 220.0, 0.1, &[]).unwrap();
        assert!(
            (s.doppler_mean_hz.unwrap() - 30.0).abs() < 0.1,
            "{:?}",
            s.doppler_mean_hz
        );
        assert!(s.doppler_rms_hz.unwrap() < 1.0, "{:?}", s.doppler_rms_hz);
    }
}

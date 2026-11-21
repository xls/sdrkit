//! Statistical known-answer tests of the fading model and the standards' profiles.

use super::*;
use sdrkit_core::Choice;
use std::f64::consts::PI;

/// Gains of one fader sampled at `rate` for `n` samples.
fn gains(d: Doppler, rate: f64, n: usize, seed: u64) -> Vec<Complex32> {
    let mut f = Fader::new(d, rate, &mut Rng::new(seed)).unwrap();
    (0..n).map(|_| f.next()).collect()
}

/// The Bessel function J0 by its power series (exact enough for |x| < 10).
fn j0(x: f64) -> f64 {
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..60 {
        term *= -(x * x / 4.0) / (k as f64 * k as f64);
        sum += term;
    }
    sum
}

/// Mean frequency and standard deviation (Hz) of a gain sequence sampled at `rate`, from lag-1 moments:
/// `f_mean = arg(sum g[n+1] g*[n]) / (2 pi dt)`, `E f^2 = E|g[n+1] - g[n]|^2 / (E|g|^2 (2 pi dt)^2)`.
fn moments(g: &[Complex32], rate: f64) -> (f64, f64) {
    let (mut cr, mut ci, mut d, mut p) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for w in g.windows(2) {
        let c = w[1] * w[0].conj();
        cr += c.re as f64;
        ci += c.im as f64;
        d += (w[1] - w[0]).norm_sqr() as f64;
        p += w[0].norm_sqr() as f64;
    }
    let dt = 1.0 / rate;
    let mean = ci.atan2(cr) / (TAU * dt);
    let f2 = d / p / (TAU * dt).powi(2);
    (mean, (f2 - mean * mean).max(0.0).sqrt())
}

#[test]
fn rayleigh_envelope_power_is_exponential() {
    // A classical-spectrum gain has |g|^2 exponential with mean 1: P(|g|^2 < x) = 1 - e^-x. 40 seeds, 400 coherence times each.
    let mut v = Vec::new();
    for seed in 0..40 {
        for g in gains(Doppler::Jakes { max_hz: 10.0 }, 200.0, 8000, seed)
            .iter()
            .step_by(4)
        {
            v.push(g.norm_sqr() as f64);
        }
    }
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    assert!((mean - 1.0).abs() < 0.03, "mean power {mean}");
    for x in [0.05, 0.1, 0.5, 1.0, 2.0, 3.0] {
        let p = v.iter().filter(|&&e| e < x).count() as f64 / n;
        let want = 1.0 - (-x).exp();
        assert!(
            (p - want).abs() < 0.012,
            "P(|g|^2 < {x}) = {p}, want {want}"
        );
    }
}

#[test]
fn the_classical_spectrum_has_its_moments_shape_and_correlation() {
    // Clarke/Jakes: E f^2 = f_d^2 / 2; a third of the power below f_d / 2 (P(|cos a| < 1/2) = 1/3); none beyond f_d; and the
    // autocorrelation is J0(2 pi f_d tau).
    let fd = 10.0;
    let rate = 400.0;
    let mut sd = 0.0;
    let mut acf = [0.0f64; 3];
    let lags = [10usize, 20, 38]; // f_d tau = 0.25, 0.5, 0.95
    let seeds = 60;
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(4096);
    let mut inner = 0.0;
    let mut outer = 0.0;
    let mut total = 0.0;
    for seed in 0..seeds {
        let g = gains(Doppler::Jakes { max_hz: fd }, rate, 4096, 100 + seed);
        let (m, s) = moments(&g, rate);
        assert!(m.abs() < 1.5, "mean frequency {m}");
        sd += s / seeds as f64;
        let p: f64 = g.iter().map(|v| v.norm_sqr() as f64).sum::<f64>();
        for (a, &k) in acf.iter_mut().zip(&lags) {
            let c: f64 = (0..g.len() - k)
                .map(|i| (g[i + k] * g[i].conj()).re as f64)
                .sum();
            *a += c / p / seeds as f64;
        }
        let mut x = g.clone();
        fft.process(&mut x);
        for (i, v) in x.iter().enumerate() {
            let f = if i < 2048 {
                i as f64
            } else {
                i as f64 - 4096.0
            } * rate
                / 4096.0;
            let e = v.norm_sqr() as f64;
            total += e;
            if f.abs() < fd / 2.0 {
                inner += e;
            }
            if f.abs() > fd * 1.05 {
                outer += e;
            }
        }
    }
    assert!(
        (sd - fd / 2f64.sqrt()).abs() < 0.2,
        "rms Doppler {sd}, want {}",
        fd / 2f64.sqrt()
    );
    assert!(
        (inner / total - 1.0 / 3.0).abs() < 0.02,
        "share below f_d/2: {}",
        inner / total
    );
    assert!(outer / total < 0.01, "share beyond f_d: {}", outer / total);
    for (a, &k) in acf.iter().zip(&lags) {
        let want = j0(TAU * fd * k as f64 / rate);
        assert!((a - want).abs() < 0.05, "lag {k}: {a} want {want}");
    }
}

#[test]
fn a_gaussian_spectrum_holds_68_percent_within_its_half_spread() {
    // ES 201 980 equation B.4: the spread D_sp = 2 sigma holds 68 % of the power (Phi(1) - Phi(-1) = 0.6827); the mean is the
    // shift.
    let (shift, spread) = (1.2, 2.4);
    let rate = 50.0;
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(8192);
    let (mut inner, mut total, mut mean, mut sd) = (0.0, 0.0, 0.0, 0.0);
    let seeds = 40;
    for seed in 0..seeds {
        let mut g = gains(
            Doppler::Gaussian {
                shift_hz: shift,
                spread_hz: spread,
            },
            rate,
            8192,
            7 + seed,
        );
        let (m, s) = moments(&g, rate);
        mean += m / seeds as f64;
        sd += s / seeds as f64;
        fft.process(&mut g);
        for (i, v) in g.iter().enumerate() {
            let f = if i < 4096 {
                i as f64
            } else {
                i as f64 - 8192.0
            } * rate
                / 8192.0;
            let e = v.norm_sqr() as f64;
            total += e;
            if (f - shift).abs() < spread / 2.0 {
                inner += e;
            }
        }
    }
    assert!(
        (inner / total - 0.6827).abs() < 0.02,
        "share within the spread {}",
        inner / total
    );
    assert!((mean - shift).abs() < 0.05, "mean {mean}");
    assert!((sd - spread / 2.0).abs() < 0.08, "sigma {sd}");
}

#[test]
fn a_rice_gain_has_its_k_factor() {
    // Rice: E|g|^4 / (E|g|^2)^2 = (2 + 4K + K^2) / (1 + K)^2; with the direct path at 0 Hz, |E g|^2 = K / (1 + K).
    for k_db in [0.0f64, 6.0, 10.0] {
        let k = 10f64.powf(k_db / 10.0);
        let (mut m2, mut m4, mut n) = (0.0, 0.0, 0.0);
        for seed in 0..30 {
            for g in gains(
                Doppler::Rice {
                    max_hz: 5.0,
                    los_hz: 0.0,
                    k,
                },
                100.0,
                8000,
                seed,
            ) {
                let p = g.norm_sqr() as f64;
                m2 += p;
                m4 += p * p;
                n += 1.0;
            }
        }
        let ratio = (m4 / n) / (m2 / n).powi(2);
        let want = (2.0 + 4.0 * k + k * k) / (1.0 + k).powi(2);
        assert!(
            (ratio - want).abs() < 0.04,
            "K {k_db} dB: {ratio} want {want}"
        );
        assert!((m2 / n - 1.0).abs() < 0.04);
    }
}

/// Runs impulses `spacing` samples apart through `tdl` for `n` impulses; returns, for each delay in `delays` (samples), the gain
/// sequence it saw.
fn impulse_gains(tdl: &mut Tdl, spacing: usize, n: usize, delays: &[usize]) -> Vec<Vec<Complex32>> {
    let mut out = vec![Vec::with_capacity(n); delays.len()];
    let mut buf = vec![Complex32::default(); spacing];
    for _ in 0..n {
        buf.iter_mut().for_each(|v| *v = Complex32::default());
        buf[0] = Complex32::new(1.0, 0.0);
        tdl.process(&mut buf);
        for (o, &d) in out.iter_mut().zip(delays) {
            o.push(buf[d + tdl.latency()]);
        }
    }
    out
}

#[test]
fn drm_channels_have_the_paths_of_table_b1() {
    // ES 201 980 table B.1, typed here from the standard: per path the delay, rms gain, Doppler shift and spread. Measured: the
    // impulse response at each delay has power gain^2 / sum (within 0.6 dB), mean frequency = shift and standard deviation =
    // spread / 2.
    let table: [&[(f64, f64, f64, f64)]; 6] = [
        &[(0.0, 1.0, 0.0, 0.0)],
        &[(0.0, 1.0, 0.0, 0.0), (1.0, 0.5, 0.0, 0.1)],
        &[
            (0.0, 1.0, 0.1, 0.1),
            (0.7, 0.7, 0.2, 0.5),
            (1.5, 0.5, 0.5, 1.0),
            (2.2, 0.25, 1.0, 2.0),
        ],
        &[(0.0, 1.0, 0.0, 1.0), (2.0, 1.0, 0.0, 1.0)],
        &[(0.0, 1.0, 0.0, 2.0), (4.0, 1.0, 0.0, 2.0)],
        &[
            (0.0, 0.5, 0.0, 0.1),
            (2.0, 1.0, 1.2, 2.4),
            (4.0, 0.25, 2.4, 4.8),
            (6.0, 0.0625, 3.6, 7.2),
        ],
    ];
    let rate = 10_000.0;
    let spacing = 100; // 100 gain samples a second
    for (ch, paths) in table.iter().enumerate() {
        let total: f64 = paths.iter().map(|p| p.1 * p.1).sum();
        let delays: Vec<usize> = paths
            .iter()
            .map(|p| (p.0 * 10.0).round() as usize)
            .collect();
        let mut stats = vec![(0.0f64, 0.0f64, 0.0f64); paths.len()];
        let seeds = 12;
        for seed in 0..seeds {
            let mut tdl = Tdl::from_profile(&Profile::Drm(ch as u8 + 1), rate, seed).unwrap();
            assert_eq!(tdl.latency(), 0);
            let g = impulse_gains(&mut tdl, spacing, 30_000, &delays);
            for (s, gk) in stats.iter_mut().zip(&g) {
                let p = gk.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / gk.len() as f64;
                let (m, sd) = moments(gk, rate / spacing as f64);
                s.0 += p / seeds as f64;
                s.1 += m / seeds as f64;
                s.2 += sd / seeds as f64;
            }
        }
        for (k, (&(_, rho, shift, spread), s)) in paths.iter().zip(&stats).enumerate() {
            let want_db = 10.0 * (rho * rho / total).log10();
            let got_db = 10.0 * s.0.log10();
            assert!(
                (got_db - want_db).abs() < 0.6,
                "channel {} path {k}: {got_db} dB, want {want_db}",
                ch + 1
            );
            assert!(
                (s.1 - shift).abs() < 0.05 + 0.03 * spread,
                "channel {} path {k}: shift {}",
                ch + 1,
                s.1
            );
            let tol = 0.03 + 0.1 * spread;
            assert!(
                (s.2 - spread / 2.0).abs() < tol,
                "channel {} path {k}: sigma {} want {}",
                ch + 1,
                s.2,
                spread / 2.0
            );
        }
    }
}

#[test]
fn gsm_profiles_have_the_taps_of_annex_c3() {
    // TS 45.005 V17.0.0 Annex C.3, setting (1), typed here from the standard: (delay us, power dB). At 10 MHz every delay is a
    // whole number of samples; the measured power-delay profile matches within 0.6 dB, and the RICE tap of RA6 has the fourth
    // moment of K = A1/A0 = 0.83/0.17.
    let cases: [(Gsm, &[(f64, f64)]); 4] = [
        (
            Gsm::Ra6,
            &[
                (0.0, 0.0),
                (0.1, -4.0),
                (0.2, -8.0),
                (0.3, -12.0),
                (0.4, -16.0),
                (0.5, -20.0),
            ],
        ),
        (
            Gsm::Tu6,
            &[
                (0.0, -3.0),
                (0.2, 0.0),
                (0.5, -2.0),
                (1.6, -6.0),
                (2.3, -8.0),
                (5.0, -10.0),
            ],
        ),
        (
            Gsm::Ht6,
            &[
                (0.0, 0.0),
                (0.1, -1.5),
                (0.3, -4.5),
                (0.5, -7.5),
                (15.0, -8.0),
                (17.2, -17.7),
            ],
        ),
        (
            Gsm::Tu12,
            &[
                (0.0, -4.0),
                (0.1, -3.0),
                (0.3, 0.0),
                (0.5, -2.6),
                (0.8, -3.0),
                (1.1, -5.0),
                (1.3, -7.0),
                (1.7, -5.0),
                (2.3, -6.5),
                (3.1, -8.6),
                (3.2, -11.0),
                (5.0, -10.0),
            ],
        ),
    ];
    let rate = 10e6;
    for (model, taps) in cases {
        let total: f64 = taps.iter().map(|t| 10f64.powf(t.1 / 10.0)).sum();
        let delays: Vec<usize> = taps.iter().map(|t| (t.0 * 10.0).round() as usize).collect();
        let mut power = vec![0.0f64; taps.len()];
        let (mut m2, mut m4, mut n) = (0.0, 0.0, 0.0);
        let seeds = 8;
        for seed in 0..seeds {
            let profile = Profile::Gsm {
                model,
                max_doppler_hz: 2000.0,
            };
            let mut tdl = Tdl::from_profile(&profile, rate, seed).unwrap();
            let g = impulse_gains(&mut tdl, 256, 8000, &delays);
            for (p, gk) in power.iter_mut().zip(&g) {
                *p += gk.iter().map(|v| v.norm_sqr() as f64).sum::<f64>()
                    / gk.len() as f64
                    / seeds as f64;
            }
            for v in &g[0] {
                let p = v.norm_sqr() as f64;
                m2 += p;
                m4 += p * p;
                n += 1.0;
            }
        }
        for (k, (t, p)) in taps.iter().zip(&power).enumerate() {
            let want = t.1 - 10.0 * total.log10();
            let got = 10.0 * p.log10();
            assert!(
                (got - want).abs() < 0.6,
                "{model:?} tap {k}: {got} dB, want {want}"
            );
        }
        let ratio = (m4 / n) / (m2 / n).powi(2);
        let want = if model == Gsm::Ra6 {
            let k = 0.83f64 / 0.17;
            (2.0 + 4.0 * k + k * k) / (1.0 + k).powi(2)
        } else {
            2.0
        };
        assert!(
            (ratio - want).abs() < 0.12,
            "{model:?} first tap fourth moment {ratio}, want {want}"
        );
    }
}

#[test]
fn hf_profiles_have_their_delays_and_spreads() {
    // F.520-2 Annex 1 section 2.2 and F.1487 Annex 3: two equal paths, delays and 2-sigma spreads as printed.
    let cases = [
        (Profile::Watterson(profiles::Watterson::Good), 0.5, 0.1),
        (Profile::Watterson(profiles::Watterson::Moderate), 1.0, 0.5),
        (Profile::Watterson(profiles::Watterson::Poor), 2.0, 1.0),
        (Profile::Watterson(profiles::Watterson::Flutter), 0.5, 10.0),
        (
            Profile::F1487 {
                latitude: Latitude::Low,
                condition: F1487::Disturbed,
            },
            6.0,
            10.0,
        ),
        (
            Profile::F1487 {
                latitude: Latitude::High,
                condition: F1487::Disturbed,
            },
            7.0,
            30.0,
        ),
        (
            Profile::F1487 {
                latitude: Latitude::Mid,
                condition: F1487::DisturbedNvis,
            },
            7.0,
            1.0,
        ),
    ];
    for (p, ms, spread) in cases {
        let taps = p.taps().unwrap();
        assert_eq!(taps.len(), 2);
        assert_eq!(taps[0].delay_s, 0.0);
        assert!((taps[1].delay_s - ms * 1e-3).abs() < 1e-12);
        assert_eq!(taps[0].power_db, taps[1].power_db);
        for t in &taps {
            assert_eq!(
                t.doppler,
                Doppler::Gaussian {
                    shift_hz: 0.0,
                    spread_hz: spread
                }
            );
        }
        // Measured: the spread of path 2 at 1 kHz.
        let rate = 1000.0;
        let d = (ms).round() as usize;
        let mut sd = 0.0;
        for seed in 0..6 {
            let mut tdl = Tdl::from_profile(&p, rate, seed).unwrap();
            let spacing = 10.max(d + 1);
            let g = impulse_gains(&mut tdl, spacing, 20_000, &[d]);
            sd += moments(&g[0], rate / spacing as f64).1 / 6.0;
        }
        if (ms - ms.round()).abs() < 1e-9 {
            assert!(
                (sd - spread / 2.0).abs() < 0.03 + 0.12 * spread,
                "{p:?}: sigma {sd}"
            );
        }
    }
    assert!(Profile::F1487 {
        latitude: Latitude::Low,
        condition: F1487::DisturbedNvis
    }
    .taps()
    .is_err());
}

#[test]
fn drm_vhf_channels_follow_table_b2() {
    // Spot checks of table B.2: channel 11 has 12 paths, path 5 at 30 us, -3.1 dB, Gauss1 = Gaussian at +0.7 f_d with sigma
    // 0.1 f_d; channel 12 path 3 at 220 us, Gauss2.
    let fd = 50.0;
    let t11 = Profile::DrmVhf {
        channel: 11,
        max_doppler_hz: fd,
    }
    .taps()
    .unwrap();
    assert_eq!(t11.len(), 12);
    assert!((t11[4].delay_s - 30e-6).abs() < 1e-15 && t11[4].power_db == -3.1);
    assert_eq!(
        t11[4].doppler,
        Doppler::Gaussian {
            shift_hz: 35.0,
            spread_hz: 10.0
        }
    );
    let t12 = Profile::DrmVhf {
        channel: 12,
        max_doppler_hz: fd,
    }
    .taps()
    .unwrap();
    assert!((t12[2].delay_s - 220e-6).abs() < 1e-15 && t12[2].power_db == -18.0);
    assert_eq!(
        t12[2].doppler,
        Doppler::Gaussian {
            shift_hz: -35.0,
            spread_hz: 10.0
        }
    );
    let t7 = Profile::DrmVhf {
        channel: 7,
        max_doppler_hz: fd,
    }
    .taps()
    .unwrap();
    assert_eq!(t7, vec![Tap::new(0.0, 0.0, Doppler::Static)]);
    assert!(Profile::Drm(7).taps().is_err());
    assert!(Profile::DrmVhf {
        channel: 6,
        max_doppler_hz: fd
    }
    .taps()
    .is_err());
}

#[test]
fn a_static_channel_and_fractional_delays_are_exact() {
    // An echo 2.5 samples late at -6 dB with a 90 degree carrier turn, on a slow tone: y = a x(n - L) + b j x(n - L - 2.5), with
    // a^2 + b^2 = 1 and b/a = 10^(-6/20). The fractional read is compared with the exact delayed tone.
    let rate = 1000.0;
    let p = Profile::Echo {
        delay_s: 2.5e-3,
        level_db: -6.0,
        phase_deg: 90.0,
    };
    let mut tdl = Tdl::from_profile(&p, rate, 1).unwrap();
    assert_eq!(tdl.latency(), 8);
    let r = 10f64.powf(-6.0 / 20.0);
    let a = 1.0 / (1.0 + r * r).sqrt();
    let b = r * a;
    let f = 0.031;
    let tone = |t: f64| {
        let p = TAU * f * t;
        Complex32::new(p.cos() as f32, p.sin() as f32)
    };
    let mut x: Vec<Complex32> = (0..2000).map(|n| tone(n as f64)).collect();
    tdl.process(&mut x);
    for (n, v) in x.iter().enumerate().skip(100) {
        let t = n as f64 - 8.0;
        let want = tone(t) * a as f32 + tone(t - 2.5) * Complex32::new(0.0, b as f32);
        assert!((v - want).norm() < 2e-3, "sample {n}: {v} vs {want}");
    }
    // No channel: the identity.
    let mut none = Tdl::from_profile(&Profile::None, rate, 1).unwrap();
    let mut y = vec![Complex32::new(0.3, -0.2); 50];
    none.process(&mut y);
    assert!(y.iter().all(|v| *v == Complex32::new(0.3, -0.2)));
}

#[test]
fn fading_is_repeatable_and_independent_of_the_cuts() {
    let p = Profile::Gsm {
        model: Gsm::Tu6,
        max_doppler_hz: 300.0,
    };
    let x: Vec<Complex32> = (0..20_000)
        .map(|i| Complex32::new((i as f32 * 0.01).cos(), 0.1))
        .collect();
    let mut a = Tdl::from_profile(&p, 1e6, 5).unwrap();
    let mut b = Tdl::from_profile(&p, 1e6, 5).unwrap();
    let mut ya = x.clone();
    a.process(&mut ya);
    let mut yb = x.clone();
    for c in yb.chunks_mut(97) {
        b.process(c);
    }
    assert_eq!(ya, yb);
    let mut c = Tdl::from_profile(&p, 1e6, 6).unwrap();
    let mut yc = x.clone();
    c.process(&mut yc);
    assert_ne!(ya, yc);
    let amps: f32 = a.tap_amplitudes().map(|v| v * v).sum();
    assert!((amps - 1.0).abs() < 1e-5);
    assert!(Tdl::new(&[], 1.0, 0).is_err());
    assert!(Tdl::new(&[Tap::new(-1.0, 0.0, Doppler::Static)], 1.0, 0).is_err());
    for index in 0..profiles::ProfileName::CHOICES.len() as u16 {
        let name = profiles::ProfileName::from_index(index).unwrap();
        let p = Profile::from_name(name, &ProfileParams::default()).unwrap();
        assert!(p.taps().is_ok(), "{name:?}");
    }
    let _ = PI;
}

use profiles::{Gsm, ProfileParams};

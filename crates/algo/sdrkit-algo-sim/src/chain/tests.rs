//! Known-answer tests of the chain: measured noise levels, offsets and clock error against the settings.

use super::*;
use crate::modulate::Linear;
use crate::points::qam;
use crate::rng::Rng;
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_core::{Choice, Value};
use std::f64::consts::TAU;

/// The default generator settings.
fn defaults() -> Values {
    Values::defaults(&GenSetting::schema())
}

fn tone(n: usize, f: f64) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let p = TAU * (f * i as f64).rem_euclid(1.0);
            Complex32::new(p.cos() as f32, p.sin() as f32)
        })
        .collect()
}

fn mean_freq(x: &[Complex32]) -> f64 {
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for w in x.windows(2) {
        let p = w[1] * w[0].conj();
        re += p.re as f64;
        im += p.im as f64;
    }
    im.atan2(re) / TAU
}

/// Es/N0 measured after an RRC matched filter at the symbol centres: mean |s|^2 over mean |y - s|^2.
fn measured_esn0(cfg: &ChainConfig, level_dbfs: f64) -> f64 {
    let sps = 8.0;
    let rs = 10_000.0;
    let rate = rs * sps;
    let mut rng = Rng::new(11);
    let pts = qam(4).unwrap();
    let syms: Vec<Complex32> = (0..20_000).map(|_| pts[rng.below(4) as usize]).collect();
    let mut m = Linear::rrc(0.35, 16.0, sps).unwrap();
    let mut clean = Vec::new();
    m.push(&syms, &mut clean);
    let sig = SignalInfo::new(rate)
        .with_symbol_rate(rs)
        .with_bit_rate(2.0 * rs)
        .with_bandwidth(1.35 * rs);
    let mut chain = Chain::new(cfg, &sig).unwrap();
    let mut y = Vec::new();
    chain.process(&clean, &mut y);
    let h = taps(Shape::Rrc { beta: 0.35 }, sps, 16.0, Norm::Energy).unwrap();
    let c = h.len() / 2;
    let g = 10f64.powf(-level_dbfs / 20.0) as f32 / (sps as f32).sqrt();
    let (mut es, mut err) = (0.0f64, 0.0f64);
    for (k, s) in syms.iter().enumerate().skip(20).take(19_000) {
        let centre = m.symbol_centre(k as u64).round() as usize;
        let mut acc = Complex32::default();
        for (j, t) in h.iter().enumerate() {
            acc += y[centre + j - c] * *t;
        }
        es += s.norm_sqr() as f64;
        err += (acc * g - s).norm_sqr() as f64;
    }
    10.0 * (es / err).log10()
}

#[test]
fn measured_es_n0_matches_the_setting() {
    // Es/N0 set to 10 dB and measured after the matched filter (the matched-filter output SNR of an RRC signal is Es/N0), at two
    // levels: the noise follows the signal's level.
    for level in [0.0, -30.0] {
        let cfg = ChainConfig {
            level_dbfs: level,
            noise: NoiseLevel::EsN0Db {
                esn0_db: 10.0,
                symbol_rate_hz: 10_000.0,
            },
            ..ChainConfig::default()
        };
        let got = measured_esn0(&cfg, level);
        assert!((got - 10.0).abs() < 0.15, "level {level}: Es/N0 {got} dB");
    }
    // Eb/N0 of 7 dB for QPSK (2 bits per symbol) is Es/N0 of 10.01 dB; C/N0 of 50 dB at 10 kBd is Es/N0 10 dB.
    let eb = ChainConfig {
        noise: NoiseLevel::EbN0Db {
            ebn0_db: 7.0,
            bit_rate_hz: 20_000.0,
        },
        ..ChainConfig::default()
    };
    let got = measured_esn0(&eb, 0.0);
    assert!((got - (7.0 + 10.0 * 2f64.log10())).abs() < 0.15, "{got}");
    let cn0 = ChainConfig {
        noise: NoiseLevel::Cn0DbHz(50.0),
        ..ChainConfig::default()
    };
    assert!((measured_esn0(&cn0, 0.0) - 10.0).abs() < 0.15);
}

#[test]
fn measured_snr_in_a_bandwidth_matches_the_setting() {
    // A unit tone at 15 dB SNR in 12.5 kHz at 250 kHz: the noise in any 12.5 kHz is 1/31.6; measured with a brick-wall band of
    // FFT bins away from the tone.
    let rate = 250e3;
    let cfg = ChainConfig {
        noise: NoiseLevel::SnrDb {
            snr_db: 15.0,
            bandwidth_hz: 12.5e3,
        },
        seed: 3,
        ..ChainConfig::default()
    };
    let mut chain = Chain::new(&cfg, &SignalInfo::new(rate).with_bandwidth(12.5e3)).unwrap();
    let n = 1 << 16;
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(n);
    let mut band = 0.0;
    let reps = 20;
    for _ in 0..reps {
        let mut y = Vec::with_capacity(n);
        chain.process(&vec![Complex32::default(); n], &mut y);
        fft.process(&mut y);
        // Bins from 50 kHz to 62.5 kHz.
        let lo = (50e3 / rate * n as f64) as usize;
        let hi = (62.5e3 / rate * n as f64) as usize;
        band += y[lo..hi].iter().map(|v| v.norm_sqr() as f64).sum::<f64>()
            / (n as f64 * n as f64)
            / reps as f64;
    }
    let snr = 10.0 * (1.0 / band).log10();
    assert!((snr - 15.0).abs() < 0.1, "SNR in 12.5 kHz {snr} dB");
    assert!((chain.noise_power() - 10f64.powf(-1.5) * 20.0).abs() < 1e-12);
}

#[test]
fn offset_drift_inversion_and_clock_are_where_the_settings_put_them() {
    let rate = 100e3;
    // A tone at +5 kHz, inverted, then moved by +2 kHz: it appears at -3 kHz.
    let cfg = ChainConfig {
        inverted: true,
        offset_hz: 2000.0,
        ..ChainConfig::default()
    };
    let mut c = Chain::new(&cfg, &SignalInfo::new(rate)).unwrap();
    let mut y = Vec::new();
    c.process(&tone(10_000, 5000.0 / rate), &mut y);
    assert!(
        (mean_freq(&y) * rate + 3000.0).abs() < 0.01,
        "{}",
        mean_freq(&y) * rate
    );
    // A drift of 1 kHz/s over 1 s: the frequency of the last 1000 samples is 1 kHz above the first 1000.
    let cfg = ChainConfig {
        drift_hz_s: 1000.0,
        ..ChainConfig::default()
    };
    let mut c = Chain::new(&cfg, &SignalInfo::new(rate)).unwrap();
    let mut y = Vec::new();
    c.process(&vec![Complex32::new(1.0, 0.0); 100_000], &mut y);
    let f0 = mean_freq(&y[..1000]) * rate;
    let f1 = mean_freq(&y[99_000..]) * rate;
    assert!((f1 - f0 - 990.0).abs() < 0.1, "{f0} {f1}");
    // A clock 200 ppm fast: 200 extra samples per million and a 20 kHz tone reads 20 kHz / 1.0002.
    let cfg = ChainConfig {
        clock_ppm: 200.0,
        ..ChainConfig::default()
    };
    let mut c = Chain::new(&cfg, &SignalInfo::new(rate)).unwrap();
    let x = tone(1_000_000, 0.2);
    let n = c.outputs_for(x.len());
    assert!((n as i64 - 1_000_200).abs() <= 1, "{n}");
    let mut y = Vec::with_capacity(n);
    c.process(&x, &mut y);
    assert_eq!(y.len(), n);
    let f = mean_freq(&y[100..]) * rate;
    assert!((f - 20e3 / 1.0002).abs() < 0.01, "{f}");
    assert!((c.latency() - 16.0).abs() < 1e-12);
}

#[test]
fn the_full_chain_is_repeatable_and_independent_of_the_cuts() {
    let cfg = ChainConfig {
        seed: 77,
        level_dbfs: -10.0,
        inverted: true,
        fading: Profile::Watterson(crate::fading::profiles::Watterson::Poor),
        offset_hz: 123.0,
        drift_hz_s: 4.0,
        phase_noise: Some(PhaseNoiseSpec {
            dbc_hz: -70.0,
            offset_hz: 1000.0,
            floor_dbc_hz: Some(-120.0),
        }),
        clock_ppm: -35.0,
        bit_jitter: 0.0,
        hum: Some(HumSpec {
            freq_hz: 50.0,
            depth: 0.05,
        }),
        noise: NoiseLevel::SnrDb {
            snr_db: 10.0,
            bandwidth_hz: 3000.0,
        },
        impulse: Some(ImpulseSpec {
            rate_hz: 5.0,
            width_s: 1e-3,
            level_dbfs: 0.0,
        }),
        iq_gain_db: 0.5,
        iq_phase_deg: 2.0,
        dc: Complex32::new(0.01, -0.02),
        adc_bits: Some(8),
    };
    let sig = SignalInfo::new(12_000.0).with_bandwidth(3000.0);
    let x = tone(50_000, 0.07);
    let mut a = Chain::new(&cfg, &sig).unwrap();
    let mut b = Chain::new(&cfg, &sig).unwrap();
    let (mut ya, mut yb) = (Vec::new(), Vec::new());
    a.process(&x, &mut ya);
    for c in x.chunks(313) {
        b.process(c, &mut yb);
    }
    assert_eq!(ya, yb);
    let mut c = Chain::new(
        &ChainConfig {
            seed: 78,
            ..cfg.clone()
        },
        &sig,
    )
    .unwrap();
    let mut yc = Vec::new();
    c.process(&x, &mut yc);
    assert_ne!(ya, yc);
    // The default chain is the identity.
    let mut id = Chain::new(&ChainConfig::default(), &sig).unwrap();
    let mut yi = Vec::new();
    id.process(&x, &mut yi);
    assert_eq!(yi, x);
}

#[test]
fn an_impaired_generator_returns_what_the_chain_gives() {
    struct Src(u64);
    impl Generator for Src {
        fn rate_hz(&self) -> f64 {
            8000.0
        }
        fn generate(&mut self, out: &mut [Complex32]) -> usize {
            for o in out.iter_mut() {
                let p = TAU * (0.01 * self.0 as f64).rem_euclid(1.0);
                *o = Complex32::new(p.cos() as f32, p.sin() as f32);
                self.0 += 1;
            }
            out.len()
        }
    }
    let cfg = ChainConfig {
        clock_ppm: 300.0,
        noise: NoiseLevel::Cn0DbHz(50.0),
        ..ChainConfig::default()
    };
    let sig = SignalInfo::new(8000.0);
    let mut g = Impaired::new(Src(0), &cfg, sig).unwrap();
    let mut out = vec![Complex32::default(); 20_000];
    for c in out.chunks_mut(999) {
        assert_eq!(g.generate(c), c.len());
    }
    let mut chain = Chain::new(&cfg, &sig).unwrap();
    let mut src = Src(0);
    let mut x = vec![Complex32::default(); 20_480];
    src.generate(&mut x);
    let mut y = Vec::new();
    chain.process(&x, &mut y);
    assert_eq!(&y[..20_000], &out[..]);
    assert!(Impaired::new(Src(0), &cfg, SignalInfo::new(9000.0)).is_err());
}

#[test]
fn settings_are_read_strictly() {
    let sig = SignalInfo::new(48_000.0).with_bandwidth(3000.0);
    let mut v = defaults();
    // Defaults: nothing happens.
    assert_eq!(
        ChainConfig::from_values(&v, &sig).unwrap(),
        ChainConfig::default()
    );
    v.set(
        GenSetting::Noise.id(),
        &Value::Choice(NoiseKind::Snr.index()),
    )
    .unwrap();
    v.set(GenSetting::SnrDb.id(), &Value::F64(12.0)).unwrap();
    v.set(GenSetting::OffsetHz.id(), &Value::F64(-250.0))
        .unwrap();
    v.set(
        GenSetting::Fading.id(),
        &Value::Choice(ProfileName::Drm3.index()),
    )
    .unwrap();
    v.set(GenSetting::AdcBits.id(), &Value::I64(10)).unwrap();
    v.set(GenSetting::Seed.id(), &Value::I64(99)).unwrap();
    let c = ChainConfig::from_values(&v, &sig).unwrap();
    assert_eq!(
        c.noise,
        NoiseLevel::SnrDb {
            snr_db: 12.0,
            bandwidth_hz: 3000.0
        }
    );
    assert_eq!(c.offset_hz, -250.0);
    assert_eq!(c.fading, Profile::Drm(3));
    assert_eq!(c.adc_bits, Some(10));
    assert_eq!(c.seed, 99);
    assert!(Chain::new(&c, &sig).is_ok());
    // Out of range, wrong kind, unknown choice: refused, never clamped.
    for (s, val) in [
        (GenSetting::ClockPpm, Value::F64(1e6)),
        (GenSetting::SnrDb, Value::Text("loud".into())),
        (GenSetting::Fading, Value::Choice(200)),
        (GenSetting::Inverted, Value::F64(1.0)),
    ] {
        let mut bad = v.clone();
        assert!(bad.set(s.id(), &val).is_err(), "{s:?}");
        assert_eq!(bad, v, "{s:?}");
    }
    // Es/N0 for a signal with no symbol rate.
    let mut es = defaults();
    es.set(
        GenSetting::Noise.id(),
        &Value::Choice(NoiseKind::EsN0.index()),
    )
    .unwrap();
    assert_eq!(
        ChainConfig::from_values(&es, &sig),
        Err(SimError::SignalLacks {
            param: Param::SymbolRate
        })
    );
    assert!(ChainConfig::from_values(&es, &sig.with_symbol_rate(1000.0)).is_ok());
}

#[test]
fn the_schema_is_consistent() {
    let s = GenSetting::schema();
    for set in s.settings() {
        assert!(set.field.key.starts_with("gen_"), "{}", set.field.key);
        assert_eq!(set.section, "Test signal");
    }
    for k in [
        "gen_snr_db",
        "gen_offset_hz",
        "gen_drift_hz_s",
        "gen_clock_ppm",
        "gen_fading",
        "gen_level_dbfs",
        "gen_seed",
    ] {
        assert!(is_generator_key(k), "{k}");
    }
    assert!(!is_generator_key("gen_nothing") && !is_generator_key("snr_db"));
}

/// The cross-correlation of `y` with `x` at lag `d`, over the power of `x`: the gain of the path `d` samples late (white `x`).
fn path_gain(x: &[Complex32], y: &[Complex32], d: usize) -> Complex32 {
    let (mut acc, mut p) = (Complex32::default(), 0.0f32);
    for n in d..x.len().min(y.len()) {
        acc += y[n] * x[n - d].conj();
        p += x[n - d].norm_sqr();
    }
    acc / p
}

#[test]
fn several_echoes_from_the_settings_show_at_their_delays_with_their_gains_and_phases() {
    // White QPSK at 1 MHz through gen_fading = echoes, nothing else. Theory: the output is sum_k g_k x(n - d_k), so its
    // cross-correlation with the input at lag d_k is g_k, with |g_k| = 10^(L_k / 20) / sqrt(sum 10^(L_j / 10)) (the line keeps
    // unit power) and arg g_k the echo's phase. A pre-echo (-2 us) delays every path by 2 samples. Other lags hold only the
    // correlation noise of a finite white sequence, about 1 / sqrt(N).
    let rate = 1e6;
    let sig = SignalInfo::new(rate);
    let mut v = defaults();
    v.set(
        GenSetting::Fading.id(),
        &Value::Choice(ProfileName::Echoes.index()),
    )
    .unwrap();
    v.set(
        GenSetting::Echoes.id(),
        &Value::Text("3e-6 -6 90; 7e-6, -10, -45; -2e-6 -12".into()),
    )
    .unwrap();
    let cfg = ChainConfig::from_values(&v, &sig).unwrap();
    let mut chain = Chain::new(&cfg, &sig).unwrap();
    let mut rng = Rng::new(21);
    let r = std::f32::consts::FRAC_1_SQRT_2;
    let x: Vec<Complex32> = (0..1 << 16)
        .map(|_| {
            let (a, b) = (rng.bit(), rng.bit());
            Complex32::new(if a == 1 { r } else { -r }, if b == 1 { r } else { -r })
        })
        .collect();
    let mut y = Vec::with_capacity(x.len());
    chain.process(&x, &mut y);
    // (lag in samples, level dB, phase degrees): the direct path waits 2 samples for the pre-echo.
    let paths = [
        (2usize, 0.0f64, 0.0f64),
        (5, -6.0, 90.0),
        (9, -10.0, -45.0),
        (0, -12.0, 0.0),
    ];
    let total: f64 = paths.iter().map(|p| 10f64.powf(p.1 / 10.0)).sum();
    for &(d, db, deg) in &paths {
        let g = path_gain(&x, &y, d);
        let want = 10f64.powf(db / 20.0) / total.sqrt();
        assert!(
            (g.norm() as f64 - want).abs() < 0.02,
            "lag {d}: |g| {} want {want}",
            g.norm()
        );
        let e = (g.arg() as f64).to_degrees() - deg;
        let e = (e + 180.0).rem_euclid(360.0) - 180.0;
        assert!(
            e.abs() < 3.0,
            "lag {d}: phase {} want {deg}",
            g.arg().to_degrees()
        );
    }
    for d in [1usize, 3, 4, 6, 8, 10, 20] {
        assert!(path_gain(&x, &y, d).norm() < 0.02, "lag {d} holds no path");
    }
}

#[test]
fn the_single_echo_keys_still_give_one_echo_and_bad_lists_are_refused() {
    let sig = SignalInfo::new(1e6);
    let mut v = defaults();
    v.set(
        GenSetting::Fading.id(),
        &Value::Choice(ProfileName::Echo.index()),
    )
    .unwrap();
    v.set(GenSetting::EchoDelayS.id(), &Value::F64(1.5e-6))
        .unwrap();
    v.set(GenSetting::EchoDb.id(), &Value::F64(-12.0)).unwrap();
    v.set(GenSetting::EchoPhaseDeg.id(), &Value::F64(57.3))
        .unwrap();
    // A list does not change the single echo.
    v.set(GenSetting::Echoes.id(), &Value::Text("1e-6 -3".into()))
        .unwrap();
    let c = ChainConfig::from_values(&v, &sig).unwrap();
    assert_eq!(
        c.fading,
        Profile::Echo {
            delay_s: 1.5e-6,
            level_db: -12.0,
            phase_deg: 57.3
        }
    );
    // An empty list: the direct path alone.
    let mut e = defaults();
    e.set(
        GenSetting::Fading.id(),
        &Value::Choice(ProfileName::Echoes.index()),
    )
    .unwrap();
    let c = ChainConfig::from_values(&e, &sig).unwrap();
    assert_eq!(c.fading.taps().unwrap().len(), 1);
    for bad in [
        "1e-6",
        "1e-6 -3 0 4",
        "x -3",
        "2 -3",
        "1e-6 -90",
        "1e-6 -3 200",
    ] {
        let mut b = e.clone();
        b.set(GenSetting::Echoes.id(), &Value::Text(bad.into()))
            .unwrap();
        match ChainConfig::from_values(&b, &sig) {
            Err(SimError::EchoText { .. } | SimError::OutOfRange { .. }) => {}
            other => panic!("{bad}: {other:?}"),
        }
    }
    let many = vec!["1e-6 -20"; EchoPath::MAX + 1].join(";");
    assert!(EchoPath::parse_list(&many).is_err());
    assert!(EchoPath::parse_list(&vec!["1e-6 -20"; EchoPath::MAX].join(";")).is_ok());
}

#[test]
fn bit_edge_jitter_needs_a_bit_rate() {
    let mut v = defaults();
    v.set(GenSetting::BitJitter.id(), &Value::F64(0.1)).unwrap();
    let sig = SignalInfo::new(48_000.0);
    let c = ChainConfig::from_values(&v, &sig).unwrap();
    assert_eq!(c.bit_jitter, 0.1);
    assert!(Chain::new(&c, &sig).is_err());
    let sig = sig.with_bit_rate(4800.0);
    let mut chain = Chain::new(&c, &sig).unwrap();
    let x = tone(48_000, 0.01);
    let mut y = Vec::with_capacity(chain.outputs_for(x.len()));
    let cap = y.capacity();
    chain.process(&x, &mut y);
    assert!(y.len() <= cap && (y.len() as i64 - x.len() as i64).abs() <= 2);
    assert!(v.set(GenSetting::BitJitter.id(), &Value::F64(0.3)).is_err());
    let c = ChainConfig {
        bit_jitter: 0.3,
        ..c
    };
    assert!(Chain::new(&c, &sig).is_err());
}

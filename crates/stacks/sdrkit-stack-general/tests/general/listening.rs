//! The listening chain: filters, notch, EQ, AGC, squelch, stereo modes, cleanup, and settings changes while it runs.

use sdrkit_algo_dsp::iir::BiquadCoeffs;
use sdrkit_core::Choice;
use sdrkit_stack_general::listen::control::squelch_gate;
use sdrkit_stack_general::listen::{
    AgcSpeed, ChainSettings, ChainSquelch, EqPreset, ListeningChain, ModeKind, StereoMode,
};
use std::f64::consts::TAU;

const R: f64 = 48_000.0;

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

fn resp_db(c: &BiquadCoeffs, f: f64) -> f64 {
    db(c.response(R, f).norm())
}

fn sine(f: f64, a: f64, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (a * (TAU * f * i as f64 / R).sin()) as f32)
        .collect()
}

fn tone(x: &[f32], f: f64) -> f64 {
    sdrkit_dev_test::measure::tone_amplitude(x, R, f)
}

fn power(x: &[f32]) -> f64 {
    x.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64
}

/// Gaussian noise of unit variance from `sdrkit-algo-sim`.
struct Noise {
    rng: sdrkit_algo_sim::rng::Rng,
}
/// The noise of seed `seed`.
#[allow(non_snake_case)]
fn Noise(seed: u64) -> Noise {
    Noise {
        rng: sdrkit_algo_sim::rng::Rng::new(seed),
    }
}
impl Noise {
    fn g(&mut self) -> f64 {
        self.rng.normal()
    }
}

/// Runs mono audio through a chain in blocks of 480.
fn through(chain: &mut ListeningChain, x: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(x.len());
    for b in x.chunks(480) {
        let mut st: Vec<[f32; 2]> = b.iter().map(|&v| [v, v]).collect();
        chain.process(&mut st);
        out.extend(st.iter().map(|f| f[0]));
    }
    out
}

fn chain_with(f: impl FnOnce(&mut ChainSettings)) -> ListeningChain {
    let mut s = ChainSettings::default();
    s.control.on = true;
    f(&mut s);
    ListeningChain::new(R, s)
}

/// The amplitude of a sine at `f` through `chain` (over the last 0.2 s of 0.6 s).
fn gain_at(chain: &mut ListeningChain, f: f64, a: f64) -> f64 {
    let y = through(chain, &sine(f, a, 28_800));
    tone(&y[y.len() - 9600..], f)
}

#[test]
fn biquads() {
    let hp = BiquadCoeffs::highpass(R, 300.0, std::f64::consts::FRAC_1_SQRT_2);
    let lp = BiquadCoeffs::lowpass(R, 3000.0, std::f64::consts::FRAC_1_SQRT_2);
    assert!((resp_db(&hp, 300.0) + 3.0).abs() <= 0.3);
    assert!((resp_db(&lp, 3000.0) + 3.0).abs() <= 0.3);
    assert!(resp_db(&hp, 30.0) < -38.0 && resp_db(&hp, 3000.0) > -0.2);
    assert!(resp_db(&lp, 20_000.0) < -30.0 && resp_db(&lp, 300.0) > -0.2);
}

#[test]
fn notch() {
    let n = BiquadCoeffs::notch(R, 1000.0, 10.0);
    assert!(resp_db(&n, 1000.0) < -60.0);
    assert!(resp_db(&n, 500.0).abs() <= 0.5 && resp_db(&n, 2000.0).abs() <= 0.5);
    assert!(
        resp_db(&BiquadCoeffs::notch(R, 1000.0, 40.0), 1100.0)
            > resp_db(&BiquadCoeffs::notch(R, 1000.0, 4.0), 1100.0)
    );
}

#[test]
fn eq() {
    let mut flat = chain_with(|s| s.control.eq = EqPreset::Flat);
    for f in [50.0, 300.0, 1000.0, 5000.0, 20_000.0] {
        assert!(
            db(gain_at(&mut flat, f, 0.1) / 0.1).abs() <= 0.01,
            "flat at {f}"
        );
    }
    for (k, f) in [(1, 400.0), (2, 1000.0), (3, 2500.0)] {
        let mut c = chain_with(|s| {
            s.control.eq = EqPreset::Custom;
            s.control.eq_gains[k] = 9.0;
        });
        let g = db(gain_at(&mut c, f, 0.05) / 0.05);
        assert!((g - 9.0).abs() <= 0.6, "band {k}: {g}");
    }
    let mut low = chain_with(|s| {
        s.control.eq = EqPreset::Custom;
        s.control.eq_gains[0] = -9.0;
    });
    assert!(db(gain_at(&mut low, 40.0, 0.1) / 0.1) < -8.0);
    assert!(db(gain_at(&mut low, 4000.0, 0.1) / 0.1).abs() <= 0.5);
    let mut high = chain_with(|s| {
        s.control.eq = EqPreset::Custom;
        s.control.eq_gains[4] = 6.0;
    });
    assert!(db(gain_at(&mut high, 15_000.0, 0.1) / 0.1) > 5.0);
    assert!(db(gain_at(&mut high, 300.0, 0.1) / 0.1).abs() <= 0.5);
    let mut voice = chain_with(|s| s.control.eq = EqPreset::Voice);
    assert!(db(gain_at(&mut voice, 2500.0, 0.1) / 0.1) > 2.0);
    assert!(db(gain_at(&mut voice, 100.0, 0.1) / 0.1) < -4.0);
    let mut cw = chain_with(|s| s.control.eq = EqPreset::Cw);
    assert!(db(gain_at(&mut cw, 1000.0, 0.1) / 0.1) > 4.0);
}

#[test]
fn squelch_core() {
    // 10 ms blocks, a hang of 100 ms (10 blocks).
    let mut c = squelch_gate(-50.0, 4.0, 100.0);
    let mut s = |m: f32| c.update(Some(m));
    assert!(!s(-60.0));
    assert!(!s(-51.0));
    assert!(s(-49.0));
    assert!(s(-52.0) && s(-53.0));
    for _ in 0..9 {
        assert!(s(-60.0));
    }
    assert!(s(-53.0), "restarts the hang");
    for _ in 0..9 {
        assert!(s(-60.0));
    }
    s(-60.0);
    assert!(!s(-60.0));
    // A threshold that is not a number produces the default gate, and the chain runs on.
    let mut c = chain_with(|s| {
        s.control.squelch = ChainSquelch::Level;
        s.control.squelch_open = f32::NAN;
    });
    let y = through(&mut c, &sine(1000.0, 0.3, 4800));
    assert!(y.iter().all(|v| v.is_finite()));
}

#[test]
fn agc() {
    for a in [0.02, 0.5] {
        let mut c = chain_with(|s| {
            s.control.agc = AgcSpeed::Fast;
            s.control.agc_target_db = -18.0;
        });
        let y = through(&mut c, &sine(1000.0, a, 72_000));
        let peak = y[y.len() - 4800..]
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            (db(peak as f64) + 18.0).abs() <= 1.5,
            "{a}: {}",
            db(peak as f64)
        );
    }
    let mut c = chain_with(|s| s.control.agc = AgcSpeed::Slow);
    through(&mut c, &sine(1000.0, 0.001, 48_000));
    assert!(
        c.control().agc_gain_db() < 20.0 * 1.2f32.log10(),
        "the floor is not boosted"
    );
}

#[test]
fn off_and_stereo_modes() {
    let mut off = ListeningChain::new(R, ChainSettings::default());
    let x: Vec<[f32; 2]> = (0..4800)
        .map(|i| [0.3 * (i as f32 * 0.1).sin(), 0.1 * (i as f32 * 0.1).sin()])
        .collect();
    let mut y = x.clone();
    assert!(!off.process(&mut y), "says inactive");
    assert_eq!(x, y, "returns the input itself");
    let run = |mode: StereoMode| {
        let mut c = chain_with(|s| s.control.stereo = mode);
        let mut buf: Vec<[f32; 2]> = (0..28_800)
            .map(|i| {
                let s = (TAU * 1000.0 * i as f64 / R).sin() as f32;
                [0.3 * s, 0.1 * s]
            })
            .collect();
        for b in buf.chunks_mut(480) {
            c.process(b);
        }
        let l: Vec<f32> = buf[19_200..].iter().map(|f| f[0]).collect();
        let r: Vec<f32> = buf[19_200..].iter().map(|f| f[1]).collect();
        (tone(&l, 1000.0), tone(&r, 1000.0))
    };
    let (l, r) = run(StereoMode::Swap);
    assert!((db(l / 0.1)).abs() < 0.5 && (db(r / 0.3)).abs() < 0.5);
    let (l, r) = run(StereoMode::Mono);
    assert!((db(l / 0.2)).abs() < 0.5 && (db(r / 0.2)).abs() < 0.5);
    let (_, r) = run(StereoMode::Left);
    assert!((db(r / 0.3)).abs() < 0.5);
    let (l, _) = run(StereoMode::Right);
    assert!((db(l / 0.1)).abs() < 0.5);
}

#[test]
fn ssb_defaults() {
    let mut s = ChainSettings::defaults_for(ModeKind::Ssb);
    s.control.on = true;
    let mut c = ListeningChain::new(R, s.clone());
    assert!(db(gain_at(&mut c, 100.0, 0.5)) < -40.0);
    let mut c = ListeningChain::new(R, s.clone());
    assert!((db(gain_at(&mut c, 1000.0, 0.5)) + 6.0).abs() <= 0.7);
    let mut c = ListeningChain::new(R, s);
    assert!(db(gain_at(&mut c, 6000.0, 0.5)) < -30.0);
}

#[test]
fn notch_and_eq_in_the_chain() {
    let mut c = chain_with(|s| {
        s.control.notch = true;
        s.control.notch_hz = 1000.0;
        s.control.notch_q = 12.0;
    });
    assert!(db(gain_at(&mut c, 1000.0, 0.5)) < -55.0);
    let mut c = chain_with(|s| {
        s.control.eq = EqPreset::Custom;
        s.control.eq_gains[2] = 12.0;
    });
    assert!((db(gain_at(&mut c, 1000.0, 0.1)) + 8.0).abs() <= 1.0);
}

#[test]
fn automatic_notch() {
    let mut c = chain_with(|s| s.control.auto_notch = true);
    let mut n = Noise(3);
    let noise: Vec<f32> = (0..144_000).map(|_| (0.05 * n.g()) as f32).collect();
    let x: Vec<f32> = sine(1234.0, 0.2, 144_000)
        .iter()
        .zip(&noise)
        .map(|(a, b)| a + b)
        .collect();
    let y = through(&mut c, &x);
    let tail = &y[y.len() - 48_000..];
    assert!(
        db(tone(tail, 1234.0) / 0.2) < -20.0,
        "{}",
        db(tone(tail, 1234.0) / 0.2)
    );
    let p = power(tail);
    assert!(
        p > 0.5 * 0.05 * 0.05 && p < power(&x[x.len() - 48_000..]),
        "{p}"
    );
    let f = c.control().auto_notch_hz();
    assert!(f.is_none() || (f.unwrap() - 1234.0).abs() < 100.0);
}

#[test]
fn clarifier_and_cw_pitch() {
    let mut c = chain_with(|s| s.control.clarifier_hz = 300.0);
    let y = through(&mut c, &sine(1000.0, 0.4, 28_800));
    let t = &y[y.len() - 9600..];
    assert!(db(tone(t, 1300.0)) > -9.0);
    assert!(db(tone(t, 1300.0)) - db(tone(t, 1000.0)) > 25.0);
    let mut c = chain_with(|s| s.control.cw_pitch_hz = 900.0);
    let y = through(&mut c, &sine(700.0, 0.4, 28_800));
    assert!(db(tone(&y[y.len() - 9600..], 900.0)) > -9.0);
}

fn voice_like(n: usize, amp: f64, seed: u64, noise: f64) -> Vec<f32> {
    let mut g = Noise(seed);
    (0..n)
        .map(|i| {
            let t = i as f64 / R;
            let v = amp * ((TAU * 500.0 * t).sin() + 0.6 * (TAU * 1500.0 * t).sin());
            (v + noise * g.g()) as f32
        })
        .collect()
}

#[test]
fn snr_and_level_squelch() {
    let mut c = chain_with(|s| {
        s.control.squelch = ChainSquelch::Snr;
        s.control.squelch_open = 8.0;
        s.control.squelch_hang_ms = 50.0;
    });
    let mut n = Noise(9);
    let x: Vec<f32> = (0..48_000).map(|_| (0.2 * n.g()) as f32).collect();
    let y = through(&mut c, &x);
    assert!(!c.control().squelch_open());
    assert!(power(&y[24_000..]) < 1e-6);
    let y = through(&mut c, &voice_like(48_000, 0.3, 1, 0.003));
    assert!(c.control().squelch_open());
    assert!(power(&y[24_000..]) > 1e-3);
    let level = |a: f64| {
        let mut c = chain_with(|s| {
            s.control.squelch = ChainSquelch::Level;
            s.control.squelch_open = -30.0;
            s.control.squelch_hysteresis = 3.0;
        });
        through(&mut c, &sine(1000.0, a, 24_000));
        c.control().squelch_open()
    };
    assert!(!level(0.001));
    assert!(level(0.3));
}

#[test]
fn tail_elimination() {
    let burst = |tail: f32| {
        let mut c = chain_with(|s| {
            s.control.squelch = ChainSquelch::Snr;
            s.control.squelch_open = 8.0;
            s.control.squelch_hang_ms = 0.0;
            s.control.tail_ms = tail;
        });
        let mut x = voice_like(24_000, 0.3, 4, 0.003);
        let mut n = Noise(5);
        x.extend((0..24_000).map(|_| (0.2 * n.g()) as f32));
        let y = through(&mut c, &x);
        // The noise heard in the 58 ms after the drop (power of the second difference).
        let d = (tail as f64 * 1e-3 * R) as usize;
        let seg = &y[24_000 + d..24_000 + d + (0.058 * R) as usize];
        let dd: Vec<f32> = seg.windows(3).map(|w| w[2] - 2.0 * w[1] + w[0]).collect();
        power(&dd)
    };
    let without = burst(0.0);
    let with = burst(60.0);
    assert!(without > 0.005, "{without}");
    assert!(with < 0.25 * without, "{with} vs {without}");
}

#[test]
fn af_spectrum_and_meters() {
    let mut c = chain_with(|s| s.control.spectrum = true);
    through(&mut c, &sine(3000.0, 0.5, 4800));
    let mut bins = [0.0f32; 128];
    c.control_mut().af_spectrum(&mut bins);
    let (k, &v) = bins
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    let hz = c.control().af_bin_hz(k);
    assert!(
        (hz - 3000.0).abs() <= 2.0 * R / 1024.0 * 4.0 / 2.0 + 1.0,
        "{hz}"
    );
    assert!((-9.0..=-4.0).contains(&v), "{v}");
    c.control_mut().meters.take();
    through(&mut c, &sine(1000.0, 1.0, 4800));
    let (peak, _, clip) = c.control_mut().meters.take();
    assert!(c.control().meters.clips > 0 && clip && peak > -0.1);
    let mut c = chain_with(|_| {});
    through(&mut c, &sine(1000.0, 0.1, 4800));
    let (_, rms, _) = c.control_mut().meters.take();
    assert_eq!(c.control().meters.clips, 0);
    assert!((rms + 23.0).abs() <= 0.6, "{rms}");
}

// The cleanup.

/// A harmonic speech-like signal: a 140 Hz voice with formants, in syllables of about 0.3 s with pauses.
fn speech(n: usize, amp: f64) -> Vec<f32> {
    speech_every(n, amp, 0.5)
}

/// The same with a syllable every `period` seconds (on for 60 % of it).
fn speech_every(n: usize, amp: f64, period: f64) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f64 / R;
            let syl = (t / period).fract();
            let env = if syl < 0.6 {
                (std::f64::consts::PI * syl / 0.6).sin()
            } else {
                0.0
            };
            let f0 = 140.0 + 15.0 * (TAU * 0.7 * t).sin();
            let mut v = 0.0;
            for h in 1..=20 {
                let f = f0 * h as f64;
                let form = (-((f - 500.0) / 300.0).powi(2)).exp()
                    + 0.6 * (-((f - 1500.0) / 400.0).powi(2)).exp()
                    + 0.3 * (-((f - 2500.0) / 500.0).powi(2)).exp();
                v += form * (TAU * f * t + h as f64).sin();
            }
            (amp * env * v) as f32
        })
        .collect()
}

fn cleanup_chain(f: impl FnOnce(&mut ChainSettings)) -> ListeningChain {
    let mut s = ChainSettings::default();
    s.cleanup.on = true;
    f(&mut s);
    ListeningChain::new(R, s)
}

fn snr_db(clean: &[f32], out: &[f32], delay: usize, from: usize) -> f64 {
    let (mut sp, mut ep) = (0.0, 0.0);
    for i in from..out.len() {
        let c = clean[i - delay] as f64;
        sp += c * c;
        ep += (out[i] as f64 - c).powi(2);
    }
    10.0 * (sp / ep.max(1e-30)).log10()
}

#[test]
fn cleanup_noise_reduction_improves_the_snr() {
    let n = 8 * 48_000;
    let clean = speech(n, 0.05);
    let ps = power(&clean);
    let sigma = (ps / 10f64.powf(0.63)).sqrt();
    let mut g = Noise(11);
    let noisy: Vec<f32> = clean.iter().map(|&c| c + (sigma * g.g()) as f32).collect();
    let input_snr = snr_db(&clean, &noisy, 0, 48_000);
    for s in [0.3, 0.6, 1.0] {
        let mut c = cleanup_chain(|x| {
            x.cleanup.nr = true;
            x.cleanup.nr_strength = s;
        });
        let d = c.cleanup().latency();
        let y = through(&mut c, &noisy);
        let out_snr = snr_db(&clean, &y, d, 4 * 48_000);
        eprintln!(
            "noise reduction {:.0} %: SNR {input_snr:.1} -> {out_snr:.1} dB (+{:.1} dB)",
            s * 100.0,
            out_snr - input_snr
        );
        assert!(
            out_snr - input_snr > 4.0,
            "strength {s}: {input_snr:.1} -> {out_snr:.1} dB"
        );
        // Between bursts (the pauses), the noise goes down.
        let (mut pin, mut pout) = (0.0, 0.0);
        for i in 4 * 48_000..n {
            let t = ((i - d) as f64 / R * 2.0).fract();
            if t > 0.7 && t < 0.95 {
                pin += (noisy[i - d] as f64).powi(2);
                pout += (y[i] as f64).powi(2);
            }
        }
        eprintln!(
            "noise reduction {:.0} %: noise between bursts {:.1} dB down",
            s * 100.0,
            10.0 * (pin / pout).log10()
        );
        assert!(
            10.0 * (pin / pout).log10() > 8.0,
            "strength {s}: noise down {:.1} dB",
            10.0 * (pin / pout).log10()
        );
    }
    // Steady white noise alone at 100 %.
    let mut g = Noise(12);
    let white: Vec<f32> = (0..6 * 48_000).map(|_| (0.05 * g.g()) as f32).collect();
    let mut c = cleanup_chain(|x| {
        x.cleanup.nr = true;
        x.cleanup.nr_strength = 1.0;
    });
    let y = through(&mut c, &white);
    let down = 10.0 * (power(&white[3 * 48_000..]) / power(&y[3 * 48_000..])).log10();
    eprintln!("noise reduction 100 %: steady white noise {down:.1} dB down");
    assert!(down > 15.0, "{down}");
}

#[test]
fn cleanup_clean_speech_is_not_damaged() {
    let clean = speech(6 * 48_000, 0.1);
    let mut c = cleanup_chain(|x| {
        x.cleanup.nr = true;
        x.cleanup.auto_notch = true;
    });
    let d = c.cleanup().latency();
    let y = through(&mut c, &clean);
    let s = snr_db(&clean, &y, d, 3 * 48_000);
    assert!(s > 20.0, "{s}");
    let mut c = cleanup_chain(|_| {});
    let y = through(&mut c, &clean);
    assert!(
        snr_db(&clean, &y, d, 48_000) > 60.0,
        "all stages off: a pure delay"
    );
}

#[test]
fn cleanup_automatic_notch_follows_a_whistle() {
    let n = 7 * 48_000;
    let sp = speech(n, 0.05);
    let mut phase = 0.0;
    let whistle: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / R;
            let on = t < 5.5;
            let f = 1500.0 + 60.0 * t.min(5.5) * 0.1 + 40.0 * (TAU * 0.5 * t).sin() * 0.1;
            phase += TAU * f / R;
            if on {
                (0.15 * phase.sin()) as f32
            } else {
                0.0
            }
        })
        .collect();
    let x: Vec<f32> = sp.iter().zip(&whistle).map(|(a, b)| a + b).collect();
    let mut c = cleanup_chain(|s| s.cleanup.auto_notch = true);
    let d = c.cleanup().latency();
    let y = through(&mut c, &x);
    // After 5 s the whistle is mostly gone: compare the residual against the speech.
    let from = (4.5 * R) as usize;
    let to = (5.4 * R) as usize;
    let (mut wpow, mut epow) = (0.0, 0.0);
    for i in from..to {
        let s = sp[i - d] as f64;
        let w = whistle[i - d] as f64;
        wpow += w * w;
        epow += (y[i] as f64 - s).powi(2);
    }
    let removed = 10.0 * (wpow / epow).log10();
    assert!(removed > 10.0, "{removed}");
    // It lets go after the tone stops.
    let late = &y[(6.5 * R) as usize..];
    let late_sp = &sp[(6.5 * R) as usize - d..n - d];
    let s = snr_db(late_sp, late, 0, 0);
    assert!(s > 15.0, "{s}");
    assert_eq!(c.cleanup().notched().len(), 0);
}

#[test]
fn cleanup_manual_notch() {
    let mut c = cleanup_chain(|s| {
        s.cleanup.notch = true;
        s.cleanup.notch_hz = 1000.0;
        s.cleanup.notch_width = 0.5;
    });
    let y = through(&mut c, &sine(1000.0, 0.3, 48_000));
    assert!(tone(&y[24_000..], 1000.0) < 0.01);
    let mut c = cleanup_chain(|s| {
        s.cleanup.notch = true;
        s.cleanup.notch_hz = 1000.0;
    });
    let y = through(&mut c, &sine(2500.0, 0.3, 48_000));
    assert!((db(tone(&y[24_000..], 2500.0) / 0.3)).abs() < 0.5);
}

#[test]
fn cleanup_leveller() {
    let target = -20.0f32;
    for gain_db in [-16.0, 0.0, 9.5] {
        let amp = 0.1 * 10f64.powf(gain_db / 20.0);
        let x = speech(8 * 48_000, amp);
        let mut c = cleanup_chain(|s| {
            s.cleanup.leveller = true;
            s.cleanup.leveller_amount = 1.0;
            s.cleanup.leveller_target_db = target;
        });
        let y = through(&mut c, &x);
        assert!(y.iter().all(|v| v.abs() <= 1.0));
        // The level of the speech itself (the middle of each syllable), not of the pauses.
        let d = c.cleanup().latency();
        let active = |v: &[f32], off: usize| {
            let (mut p, mut n) = (0.0, 0);
            for i in 4 * 48_000..v.len() - d {
                let s = ((i as f64) / R * 2.0).fract();
                if (0.15..0.45).contains(&s) {
                    p += (v[i + off] as f64).powi(2);
                    n += 1;
                }
            }
            (10.0 * (p / n as f64).log10()) as f32
        };
        let (lin, lvl) = (active(&x, 0), active(&y, d));
        assert!(
            (lvl - target).abs() <= 3.0,
            "{gain_db}: {lin:.1} -> {lvl:.1}"
        );
    }
    // Hiss at -54 dBFS is not boosted.
    let mut g = Noise(2);
    let hiss: Vec<f32> = (0..4 * 48_000).map(|_| (0.002 * g.g()) as f32).collect();
    let mut c = cleanup_chain(|s| s.cleanup.leveller = true);
    let y = through(&mut c, &hiss);
    assert!(power(&y[2 * 48_000..]) <= 1.05 * power(&hiss[2 * 48_000..]));
    // A 20 dB quiet/loud difference shrinks at 80 %.
    let mut x = speech(6 * 48_000, 0.01);
    x.extend(speech(6 * 48_000, 0.1));
    let mut c = cleanup_chain(|s| {
        s.cleanup.leveller = true;
        s.cleanup.leveller_amount = 0.8;
    });
    let y = through(&mut c, &x);
    let q = 10.0 * power(&y[3 * 48_000..6 * 48_000]).log10();
    let l = 10.0 * power(&y[9 * 48_000..]).log10();
    assert!(l - q < 8.0, "{}", l - q);
}

#[test]
fn cleanup_silence_gate() {
    let n = 10 * 48_000;
    let sp = speech_every(n, 0.1, 1.0);
    let mut g = Noise(21);
    let x: Vec<f32> = sp.iter().map(|&s| s + (0.002 * g.g()) as f32).collect();
    let mut c = cleanup_chain(|s| s.cleanup.gate = true);
    let d = c.cleanup().latency();
    let y = through(&mut c, &x);
    let (mut pin, mut pout, mut sin, mut sout) = (0.0, 0.0, 0.0, 0.0);
    for i in 5 * 48_000..n {
        let t = ((i - d) as f64 / R).fract();
        if t > 0.85 && t < 0.98 {
            pin += (x[i - d] as f64).powi(2);
            pout += (y[i] as f64).powi(2);
        } else if t > 0.2 && t < 0.4 {
            sin += (x[i - d] as f64).powi(2);
            sout += (y[i] as f64).powi(2);
        }
    }
    assert!(10.0 * (pin / pout.max(1e-30)).log10() > 15.0);
    assert!((10.0 * (sout / sin).log10()).abs() < 1.0);
}

#[test]
fn cleanup_de_esser() {
    let n = 2 * 48_000;
    let mut g = Noise(31);
    let mut bp = sdrkit_algo_dsp::iir::Biquad::new(BiquadCoeffs::bandpass(R, 6500.0, 2.0));
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / R;
            let voice = 0.1 * (TAU * 300.0 * t).sin();
            let s = if (0.8..1.2).contains(&t) {
                bp.process_sample((0.5 * g.g()) as f32) as f64
            } else {
                0.0
            };
            (voice + s) as f32
        })
        .collect();
    let mut c = cleanup_chain(|s| s.cleanup.deesser = true);
    let d = c.cleanup().latency();
    let y = through(&mut c, &x);
    let burst =
        |v: &[f32], off: usize| power(&v[(0.9 * R) as usize + off..(1.1 * R) as usize + off]);
    let down = 10.0 * (burst(&x, 0) / burst(&y, d)).log10();
    assert!(down > 3.0, "{down}");
    let around = |v: &[f32], off: usize| {
        tone(
            &v[(1.5 * R) as usize + off..(1.9 * R) as usize + off],
            300.0,
        )
    };
    assert!(db(around(&y, d) / around(&x, 0)).abs() < 0.2);
}

#[test]
fn switching_stages_never_clicks() {
    let x = sine(440.0, 0.5, 4 * 48_000);
    let own = x
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    let mut s = ChainSettings::default();
    s.cleanup.on = true;
    s.control.on = true;
    let mut c = ListeningChain::new(R, s.clone());
    let mut out = Vec::new();
    let mut k = 0u64;
    for (i, b) in x.chunks(3360).enumerate() {
        k = k
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407 + i as u64);
        s.cleanup.nr = k >> 60 & 1 == 1;
        s.cleanup.auto_notch = k >> 59 & 1 == 1;
        s.cleanup.voice = k >> 58 & 1 == 1;
        s.cleanup.leveller = k >> 57 & 1 == 1;
        s.cleanup.notch = k >> 56 & 1 == 1;
        s.cleanup.notch_hz = 3000.0;
        s.control.eq = if k >> 55 & 1 == 1 {
            EqPreset::Voice
        } else {
            EqPreset::Flat
        };
        c.set(s.clone());
        let mut st: Vec<[f32; 2]> = b.iter().map(|&v| [v, v]).collect();
        c.process(&mut st);
        out.extend(st.iter().map(|f| f[0]));
    }
    let step = out[48_000..]
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(step <= 2.0 * own, "{step} against {own}");
}

#[test]
fn settings_from_another_thread_keep_the_output_bounded() {
    use std::sync::{Arc, Mutex};
    let shared = Arc::new(Mutex::new(ChainSettings::default()));
    let w = shared.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..200 {
            let mut s = w.lock().unwrap();
            s.cleanup.on = true;
            s.cleanup.nr = i % 2 == 1;
            s.control.on = true;
            s.control.agc = AgcSpeed::from_index((i % 3) as u16).unwrap();
            s.control.eq = EqPreset::from_index((i % 5) as u16).unwrap();
            s.control.squelch = ChainSquelch::from_index((i % 3) as u16).unwrap();
            s.control.clarifier_hz = ((i % 7) * 100) as f32;
            drop(s);
            std::thread::yield_now();
        }
    });
    let mut c = ListeningChain::new(R, ChainSettings::default());
    let x = speech(2 * 48_000, 0.2);
    for b in x.chunks(480) {
        let s = shared.lock().unwrap().clone();
        c.set(s);
        let mut st: Vec<[f32; 2]> = b.iter().map(|&v| [v, v]).collect();
        c.process(&mut st);
        assert!(st.iter().all(|f| f[0].is_finite() && f[0].abs() <= 4.0));
    }
    writer.join().unwrap();
}

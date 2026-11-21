//! HF waveform ID: the identification classes, noise, the recordings.

use crate::common::*;
use sdrkit_stack_hfdata::hfid::{self, identify, synth};

const FS: f64 = 12_000.0;

fn best(x: &[f32]) -> (String, f64, hfid::HfidResult) {
    let r = identify(x, FS).unwrap();
    let c = r
        .candidates
        .first()
        .map(|c| (c.waveform.name().to_string(), c.confidence))
        .unwrap_or_default();
    (c.0, c.1, r)
}

fn trials(make: impl Fn(u64) -> Vec<f32>, want: &str, n: u64, need: u64) {
    let mut ok = 0;
    for seed in 0..n {
        let mut x = make(seed);
        synth::add_noise(&mut x, FS, 10.0, 100 + seed);
        let (b, c, r) = best(&x);
        if b.starts_with(want) {
            ok += 1;
        } else {
            eprintln!("seed {seed}: {b} {c:.2} {:?}", r.lines);
        }
    }
    assert!(ok >= need, "{want}: {ok} of {n}");
}

#[test]
fn i1_stanag_4285() {
    trials(
        |s| synth::serial_tone(FS, 6.0, 2400.0, 1800.0, 256, 80, s),
        "STANAG 4285",
        5,
        4,
    );
}

#[test]
fn i2_110a() {
    trials(
        |s| synth::serial_tone(FS, 6.0, 2400.0, 1800.0, 48, 16, s),
        "MIL-STD-188-110A",
        5,
        4,
    );
}

#[test]
fn i3_4285_off_centre() {
    trials(
        |s| synth::serial_tone(FS, 6.0, 2400.0, 1830.0, 256, 80, s),
        "STANAG 4285",
        5,
        4,
    );
}

#[test]
fn i4_link11() {
    trials(|s| synth::link11(FS, 6.0, s), "Link 11", 5, 4);
}

#[test]
fn i5_mfsk12() {
    trials(
        |s| synth::mfsk(FS, 6.0, 12, 800.0, 100.0, 50.0, s),
        "Multi-tone MFSK",
        5,
        4,
    );
}

#[test]
fn i6_noise() {
    for seed in 0..10 {
        let mut x = vec![0f32; 72_000];
        let mut r = sdrkit_algo_sim::rng::Rng::new(seed);
        for v in x.iter_mut() {
            *v = (r.normal() * 0.05) as f32;
        }
        let res = identify(&x, FS).unwrap();
        assert!(!res.features.signal, "seed {seed}: {:?}", res.lines);
        assert!(res.candidates.is_empty());
    }
}

/// 120 s of noise through the receiver: analyses run, but the lock is never set. A signal sets it.
#[test]
fn noise_never_locks() {
    let rate = 24e3;
    let f = hfid::HFID.tuning.default_hz;
    let mut rx = receiver(&hfid::HFID, &[]);
    rx.configure(&sdrkit_core::InputInfo::new(rate, f, f).recording())
        .unwrap();
    let mut noise = sdrkit_dev_test::Awgn::with_sigma(0.05, 12);
    let mut locked = 0;
    for _ in 0..480 {
        feed(rx.as_mut(), &noise.noise((rate / 4.0) as usize), 6000);
        locked += usize::from(rx.results().status().lock_state == sdrkit_core::LockState::Locked);
    }
    let analyses = field(
        rx.as_ref(),
        sdrkit_stack_hfdata::typed::HfidFields::ANALYSES.key,
    )
    .unwrap_or(0.0);
    assert!(analyses >= 55.0, "{analyses} analyses");
    assert_eq!(locked, 0, "locked after {locked} of 480 pieces");
    assert_eq!(
        rx.results().status().lock_state,
        sdrkit_core::LockState::Searching
    );
    let x = synth::serial_tone(FS, 6.0, 2400.0, 1800.0, 256, 80, 3);
    let iq: Vec<sdrkit_core::Complex32> =
        sdrkit_algo_dsp::resample::Resampler::with_quality(FS, rate, 0.2, 60.0)
            .map(|mut r| {
                let mut y = Vec::new();
                r.process(
                    &x.iter()
                        .map(|&v| sdrkit_core::Complex32::new(v, 0.0))
                        .collect::<Vec<_>>(),
                    &mut y,
                );
                y
            })
            .unwrap();
    feed(rx.as_mut(), &iq, 6000);
    assert_eq!(
        rx.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
}

#[test]
fn i7_recordings() {
    let exp = sdrkit_dev_test::sigmf::read_expected("hfdata").unwrap();
    for name in [
        "wfid_4285_24k",
        "wfid_110a_24k",
        "wfid_link11_24k",
        "wfid_mfsk12_24k",
    ] {
        let rec = vector(name);
        let e = &exp[format!("{name}.sigmf-meta")];
        let mut rx = receiver(&hfid::HFID, &[]);
        run_vector(rx.as_mut(), &rec);
        let rx = rx.as_ref();
        let f = |k: &str| field(rx, k).unwrap_or(f64::NAN);
        let best = field_text(rx, sdrkit_stack_hfdata::typed::HfidFields::BEST.key);
        eprintln!(
            "{name}: {:?} {:?}",
            best,
            field_text(rx, sdrkit_stack_hfdata::typed::HfidFields::MEASURED.key)
        );
        assert_eq!(Some(best.as_str()), e["best"].as_str(), "{name}");
        let m = &e["measured"];
        if let Some(b) = m.get("baud_hz") {
            assert!((f("symbol_rate_hz") - b.as_f64().unwrap()).abs() <= 5.0);
        }
        if let Some(rs) = m.get("repeat_symbols") {
            assert!(
                (f(sdrkit_stack_hfdata::typed::HfidFields::REPEAT_SYMBOLS.key)
                    - rs.as_f64().unwrap())
                .abs()
                    <= 1.0
            );
        }
        if let Some(t) = m.get("tones") {
            let n = f(sdrkit_stack_hfdata::typed::HfidFields::COMB_TONES.key);
            assert!((n - t.as_f64().unwrap()).abs() <= 2.0, "{name} tones {n}");
            assert!(
                (f(sdrkit_stack_hfdata::typed::HfidFields::COMB_SPACING.key)
                    - m["spacing_hz"].as_f64().unwrap())
                .abs()
                    <= 3.0
            );
        }
        if e.get("best_confidence").is_some() {
            assert!(f(sdrkit_stack_hfdata::typed::HfidFields::BEST_CONFIDENCE_PCT.key) >= 60.0);
        }
        let cands = rows(rx.results(), &sdrkit_stack_hfdata::typed::HFID_CANDIDATES);
        assert_eq!(text(&cands[0], "waveform"), e["best"].as_str().unwrap());
    }
}

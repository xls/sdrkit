//! The identifier on synthetic signals from published parameters, impaired by `sdrkit-dev-test`.

use sdrkit_core::Complex32;
use sdrkit_dev_test::{Awgn, ClockError, FrequencyOffset};
use sdrkit_svc_identify::scan::{scan_and_identify, to_csv};
use sdrkit_svc_identify::synth::{self, chirps, fsk, psk};
use sdrkit_svc_identify::table::dmr_syncs;
use sdrkit_svc_identify::voiceauto::{voice_identifier, voice_systems, VoiceAction, VoiceAuto};
use sdrkit_svc_identify::{FrameSync, Identifier, ModClass, Sureness};

const RATE: f64 = 250e3;

fn impair(
    mut x: Vec<Complex32>,
    offset_hz: f64,
    snr_db: f64,
    bw_hz: f64,
    ppm: f64,
) -> Vec<Complex32> {
    if ppm != 0.0 {
        let mut out = Vec::new();
        ClockError::new(RATE, ppm).process(&x, &mut out);
        x = out;
    }
    FrequencyOffset::new(RATE, offset_hz).process(&mut x);
    let p = x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    Awgn::new(p, snr_db, bw_hz, RATE, 91).process(&mut x);
    x
}

fn dmr(n: usize, seed: u64) -> Vec<Complex32> {
    fsk(
        RATE,
        4800.0,
        &[-3, -1, 1, 3],
        648.0,
        n,
        seed,
        Some((&dmr_syncs()[0], 144)),
        None,
    )
}
fn p25(n: usize, seed: u64) -> Vec<Complex32> {
    let s = FrameSync::fsk4("fs", 0x5575_F5FF_77FF, 24);
    fsk(
        RATE,
        4800.0,
        &[-3, -1, 1, 3],
        600.0,
        n,
        seed,
        Some((&s, 864)),
        None,
    )
}
fn nxdn96(n: usize, seed: u64) -> Vec<Complex32> {
    let s = FrameSync::fsk4("fsw", 0xCDF59, 10);
    fsk(
        RATE,
        4800.0,
        &[-3, -1, 1, 3],
        800.0,
        n,
        seed,
        Some((&s, 192)),
        None,
    )
}

#[test]
fn psk_class_rate_width_centre() {
    let id = Identifier::new();
    for (order, class) in [(2, ModClass::Bpsk), (4, ModClass::Qpsk)] {
        for off in [-40e3, 15e3] {
            let x = impair(
                psk(RATE, 9600.0, order, 0.35, 250_000, 3, false),
                off,
                20.0,
                13e3,
                0.0,
            );
            let r = id.identify(&x, RATE, 100e6, None);
            let f = &r.features;
            assert_eq!(f.class, class, "{}", r.text());
            let rs = f.symbol_rate.unwrap();
            assert!((rs / 9600.0 - 1.0).abs() < 0.02, "{rs}");
            assert!(
                (f.width_99_hz / (9600.0 * 1.35) - 1.0).abs() < 0.15,
                "{}",
                f.width_99_hz
            );
            assert!((f.offset_hz - off).abs() < 500.0, "{}", f.offset_hz);
            assert!(f.duty > 0.97);
        }
    }
}

#[test]
fn fsk_levels_fm_am_bursts_ofdm() {
    let id = Identifier::new();
    // 2FSK ±2400 Hz.
    let x = impair(
        fsk(RATE, 1200.0, &[-1, 1], 2400.0, 250_000, 4, None, None),
        8e3,
        20.0,
        8e3,
        0.0,
    );
    let f = id.identify(&x, RATE, 100e6, None).features;
    assert!(
        matches!(f.class, ModClass::Fsk2 | ModClass::Gmsk),
        "{:?}",
        f.class
    );
    assert!((f.fsk.as_ref().unwrap().outer_dev_hz / 2400.0 - 1.0).abs() < 0.15);
    // 4FSK levels ±648 / ±1944.
    let x = impair(dmr(250_000, 5), -20e3, 20.0, 9e3, 0.0);
    let f = id.identify(&x, RATE, 446e6, None).features;
    assert_eq!(f.class, ModClass::Fsk4);
    let k = f.fsk.unwrap();
    assert!((k.outer_dev_hz / 1944.0 - 1.0).abs() < 0.15, "{k:?}");
    assert!(
        (k.inner_dev_hz.unwrap() / 648.0 - 1.0).abs() < 0.25,
        "{k:?}"
    );
    assert!((f.offset_hz + 20e3).abs() < 300.0);
    // FM by a tone: constant envelope.
    let x = impair(
        synth::fm(RATE, 1000.0, 3000.0, 250_000),
        0.0,
        25.0,
        10e3,
        0.0,
    );
    let f = id.identify(&x, RATE, 100e6, None).features;
    assert_eq!(f.class, ModClass::Fm);
    assert!(f.kurtosis < 1.1);
    // AM: a carrier line over 12 dB.
    let x = impair(synth::am(RATE, 800.0, 0.6, 250_000), 30e3, 25.0, 4e3, 0.0);
    let f = id.identify(&x, RATE, 1e6, None).features;
    assert_eq!(f.class, ModClass::Am);
    assert!(f.carrier_line_db > 12.0);
    // Bursts of 1 ms every 5 ms.
    let x = impair(
        fsk(
            RATE,
            4800.0,
            &[-1, 1],
            2400.0,
            500_000,
            6,
            None,
            Some((1e-3, 5e-3)),
        ),
        0.0,
        20.0,
        8e3,
        0.0,
    );
    let f = id.identify(&x, RATE, 100e6, None).features;
    assert!(f.duty > 0.1 && f.duty < 0.3, "{}", f.duty);
    assert!(
        (f.burst_s.unwrap() / 1e-3 - 1.0).abs() < 0.2,
        "{:?}",
        f.burst_s
    );
    assert!((f.burst_period_s.unwrap() / 5e-3 - 1.0).abs() < 0.1);
    // OFDM: the useful symbol within 3 %.
    let mut o = synth::ofdm(1024, 800, 128, 200, 7);
    o.resize(250_000, Complex32::default());
    let x = impair(o, 0.0, 20.0, 200e3, 0.0);
    let f = id.identify(&x, RATE, 100e6, None).features;
    assert_eq!(f.class, ModClass::Ofdm);
    assert!((f.ofdm.unwrap().tu_s / (1024.0 / RATE) - 1.0).abs() < 0.03);
}

#[test]
fn signal_types_are_named_with_their_frame_syncs() {
    let id = Identifier::new();
    // DMR with a carrier error and a clock error.
    let x = impair(dmr(500_000, 8), 10e3, 18.0, 9e3, 20.0);
    let r = id.identify(&x, RATE, 446.1e6, None);
    assert_eq!(r.candidates[0].signal, "dmr", "{}", r.text());
    assert_eq!(r.sureness, Sureness::Sure, "{}", r.text());
    assert_eq!(r.line, "Sure: DMR");
    let x = impair(p25(500_000, 9), -5e3, 18.0, 9e3, 0.0);
    let r = id.identify(&x, RATE, 851e6, None);
    assert_eq!(r.candidates[0].signal, "p25_phase_1", "{}", r.text());
    let x = impair(nxdn96(500_000, 10), 3e3, 18.0, 9e3, 0.0);
    let r = id.identify(&x, RATE, 446e6, None);
    assert_eq!(r.candidates[0].signal, "nxdn96", "{}", r.text());
    let k = r.features.fsk.as_ref().unwrap();
    assert!((k.outer_dev_hz / 2400.0 - 1.0).abs() < 0.2);
    // TETRA, POCSAG, LoRa.
    let x = impair(
        psk(RATE, 18_000.0, 4, 0.35, 250_000, 11, true),
        0.0,
        20.0,
        25e3,
        0.0,
    );
    assert_eq!(
        id.identify(&x, RATE, 390e6, None).candidates[0].signal,
        "tetra"
    );
    let s = FrameSync::fsk2("sync", 0x7CD2_15D8, 32);
    let x = impair(
        fsk(
            RATE,
            1200.0,
            &[-1, 1],
            4500.0,
            500_000,
            12,
            Some((&s, 544)),
            None,
        ),
        -30e3,
        20.0,
        12e3,
        0.0,
    );
    let r = id.identify(&x, RATE, 153e6, None);
    assert_eq!(r.candidates[0].signal, "pocsag");
    assert_eq!(r.sureness, Sureness::Sure);
    let x = impair(chirps(RATE, 125e3, 9, 250_000, 13), 0.0, 15.0, 125e3, 0.0);
    let r = id.identify(&x, RATE, 868.1e6, None);
    assert_eq!(r.features.chirp.unwrap().sf, Some(9));
    assert_eq!(r.candidates[0].signal, "lora");
}

#[test]
fn scan_finds_three_voice_systems_and_never_says_sure_wrongly() {
    let n = 500_000;
    let mut x = impair(dmr(n, 21), -60e3, 22.0, 9e3, 0.0);
    let y = impair(p25(n, 22), 10e3, 22.0, 9e3, 0.0);
    let z = impair(nxdn96(n, 23), 70e3, 22.0, 9e3, 0.0);
    for i in 0..n {
        x[i] += y[i] + z[i];
    }
    let id = Identifier::new();
    let rows = scan_and_identify(&id, &x, RATE, 446e6);
    for (want, at) in [("DMR", -60e3), ("P25 Phase 1", 10e3), ("NXDN96", 70e3)] {
        let r = rows
            .iter()
            .find(|r| (r.freq_hz - (446e6 + at)).abs() < 8e3)
            .unwrap_or_else(|| panic!("{want} not found: {rows:?}"));
        assert!(
            r.best.as_deref() == Some(want) || r.ranked.iter().any(|(n, _)| n == want),
            "{want}: {r:?}"
        );
        if r.sureness == Sureness::Sure {
            assert_eq!(r.best.as_deref(), Some(want));
        }
        assert!(r.signal.is_some());
    }
    assert!(to_csv(&rows).lines().count() >= 4);
    // Digital voice (auto) on the same span plays one of the three.
    let found = voice_systems(&voice_identifier().identify_all(&x, RATE, 446e6));
    assert!(found.len() >= 3, "{found:?}");
    let mut auto = VoiceAuto::default();
    assert!(matches!(auto.on_scan(0.0, found), VoiceAction::Play { .. }));
    // Noise alone: nothing likely or sure.
    let mut noise = vec![Complex32::default(); n];
    Awgn::with_sigma(0.01, 5).process(&mut noise);
    for r in scan_and_identify(&id, &noise, RATE, 446e6) {
        assert!(r.sureness < Sureness::Likely, "{r:?}");
    }
}

#[test]
fn deterministic() {
    let id = Identifier::new();
    let x = impair(dmr(200_000, 41), 0.0, 15.0, 9e3, 0.0);
    let a = id.identify(&x, RATE, 446e6, None);
    let b = id.identify(&x, RATE, 446e6, None);
    assert_eq!(a.candidates, b.candidates);
    assert_eq!(a.headline, b.headline);
}

/// A small evaluation set: each case at 15 and 25 dB, off the centre, with a clock error. Top-3 must hold the truth in most cases,
/// and a "sure" answer must never be wrong.
#[test]
fn evaluation_set() {
    type Make = fn(u64) -> Vec<Complex32>;
    let cases: Vec<(&[&str], Make, f64, f64)> = vec![
        (&["dmr"], |s| dmr(400_000, s), 9e3, 446e6),
        (&["p25_phase_1"], |s| p25(400_000, s), 9e3, 851e6),
        (&["nxdn96", "nxdn48"], |s| nxdn96(400_000, s), 9e3, 446e6),
        (
            &["nxdn96", "nxdn48"],
            |s| {
                let f = FrameSync::fsk4("fsw", 0xCDF59, 10);
                fsk(
                    RATE,
                    2400.0,
                    &[-3, -1, 1, 3],
                    350.0,
                    400_000,
                    s,
                    Some((&f, 192)),
                    None,
                )
            },
            4.5e3,
            446e6,
        ),
        (
            &["tetra"],
            |s| psk(RATE, 18_000.0, 4, 0.35, 400_000, s, true),
            25e3,
            390e6,
        ),
        (
            &["pocsag"],
            |s| {
                let f = FrameSync::fsk2("sync", 0x7CD2_15D8, 32);
                fsk(
                    RATE,
                    1200.0,
                    &[-1, 1],
                    4500.0,
                    400_000,
                    s,
                    Some((&f, 544)),
                    None,
                )
            },
            12e3,
            153e6,
        ),
        (
            &["lora"],
            |s| chirps(RATE, 125e3, 8, 400_000, s),
            125e3,
            868e6,
        ),
        (
            &[
                "wide_fm",
                "am_broadcast",
                "am_voice",
                "narrow_fm_voice",
                "ssb_voice_upper",
                "ssb_voice_lower",
                "cw_morse",
            ],
            |_| synth::am(RATE, 700.0, 0.7, 400_000),
            3e3,
            1e6,
        ),
        (
            &[
                "wide_fm",
                "am_broadcast",
                "am_voice",
                "narrow_fm_voice",
                "ssb_voice_upper",
                "ssb_voice_lower",
                "cw_morse",
            ],
            |_| synth::fm(RATE, 900.0, 2500.0, 400_000),
            10e3,
            160e6,
        ),
        (
            &["ais"],
            |s| synth::gmsk(RATE, 9600.0, 400_000, s),
            14e3,
            162e6,
        ),
    ];
    let id = Identifier::new();
    let (mut top3, mut total, mut sure_wrong) = (0, 0, 0);
    for (i, (signals, make, bw, rf)) in cases.iter().enumerate() {
        for snr in [15.0, 25.0] {
            let off = ((i as f64 * 7919.0) % 60e3) - 30e3;
            let x = impair(make(i as u64 + 100), off, snr, *bw, 10.0);
            let r = id.identify(&x, RATE, *rf, None);
            total += 1;
            if r.candidates
                .iter()
                .take(3)
                .any(|c| signals.contains(&c.signal.as_str()))
            {
                top3 += 1;
            } else {
                eprintln!("{signals:?} at {snr} dB: {}", r.text());
            }
            if r.sureness == Sureness::Sure && !signals.contains(&r.candidates[0].signal.as_str()) {
                sure_wrong += 1;
                eprintln!("sure and wrong: {signals:?}: {}", r.text());
            }
        }
    }
    assert_eq!(sure_wrong, 0);
    assert!(top3 as f64 >= 0.8 * total as f64, "top-3 {top3} of {total}");
}

/// The time budget: run with `cargo test --release -p sdrkit-svc-identify -- --ignored`.
#[test]
#[ignore]
fn identify_is_fast_enough_in_release() {
    let id = Identifier::new();
    let x = impair(dmr(500_000, 51), 10e3, 20.0, 9e3, 0.0);
    let t = std::time::Instant::now();
    for _ in 0..5 {
        id.identify(&x, RATE, 446e6, None);
    }
    let ms = t.elapsed().as_secs_f64() * 1e3 / 5.0;
    eprintln!("identify: {ms:.0} ms on average");
    assert!(ms < 150.0, "{ms} ms");
}

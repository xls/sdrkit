//! Globalstar activity monitor: the generator through the standard impairments into the receiver.

use crate::common::{cell_num, row_count, set, values};
use sdrkit_core::{InputInfo, Record, Results, StackConfig, StateBlob, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_satcom::modes::globalstar::{self, gen};
use sdrkit_stack_satcom::typed::{GlobalstarActivityEvent, GLOBALSTAR_DOPPLER};
use serde_json::json;

const RATE: f64 = 6e6;

fn harness(
    scenario: sdrkit_stack_satcom::modes::globalstar::gen::Scenario,
    seconds: f64,
) -> Harness {
    Harness::new(&globalstar::DESCRIPTOR)
        .rate(RATE)
        .seconds(seconds)
        .piece(50_000)
        .set(
            &sdrkit_stack_satcom::modes::globalstar::gen::GEN_SCENARIO,
            scenario,
        )
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 10.0)
}

fn run(h: Harness) -> Run {
    h.run().expect("the run")
}

/// The row of channel `ch` in the channel table.
fn row(r: &Results, ch: usize) -> &Record {
    let id = r.decl().table_id("channels").expect("the channel table");
    r.table(id)
        .and_then(|t| t.row(&format!("{ch:02}")))
        .expect("a channel row")
}

/// The number in column `key` of channel `ch`.
fn cell(r: &Results, ch: usize, key: &str) -> f64 {
    cell_num(row(r, ch), key).expect("a number")
}

fn active(r: &Results, ch: usize) -> bool {
    crate::common::cell(row(r, ch), "active") == "true"
}

fn series_points(r: &Results, key: &str) -> Vec<[f32; 2]> {
    let id = r.decl().series_id(key).expect("a series");
    r.series(id).map_or(Vec::new(), |s| s.points.clone())
}

#[test]
fn g1_steady_channel_over_snr_and_doppler() {
    for (snr, dop) in [(6.0, 0.0), (12.0, 20e3), (20.0, -35e3)] {
        let r = run(harness(
            sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Steady,
            1.5,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, snr)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, dop));
        let res = &r.results;
        assert_eq!(
            res.status().lock_state,
            sdrkit_core::LockState::Locked,
            "snr {snr}"
        );
        assert!(active(res, 7), "snr {snr}");
        let d = cell(res, 7, "doppler");
        assert!((d - dop).abs() < 3.5e3, "snr {snr} dop {dop}: {d}");
        let s = cell(res, 7, "snr_db");
        assert!((s - snr).abs() < 3.0, "snr {snr}: {s}");
        assert_eq!((1..=13).filter(|&c| active(res, c)).count(), 1);
        assert_eq!(r.field("active_channels"), Some(&Value::I64(1)));
    }
}

#[test]
fn g2_radio_clock_error() {
    for ppm in [100.0, -100.0] {
        let r = run(harness(
            sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Steady,
            1.5,
        )
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 10e3)
        .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, ppm));
        assert!(active(&r.results, 7));
        let d = cell(&r.results, 7, "doppler");
        assert!((d - 10e3).abs() < 4e3, "{d}");
    }
}

#[test]
fn g3_noise_and_silence_give_nothing() {
    let r = run(harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Silence,
        2.0,
    ));
    let res = &r.results;
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Searching);
    assert!((1..=13).all(|c| !active(res, c)));
    assert_eq!(row_count(res, "activity"), 0);
    assert!((1..=13).all(|c| series_points(res, GLOBALSTAR_DOPPLER[c - 1].key).is_empty()));
    assert_eq!(r.events_of("globalstar.activity").count(), 0);
    assert_eq!(
        res.status().summary,
        "listening: no Globalstar terminal uplink"
    );
}

#[test]
fn g4_pass_at_ten_times_and_g5_bursts() {
    let h = harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Pass,
        4.0,
    )
    .set(
        &sdrkit_stack_satcom::modes::globalstar::gen::GEN_TIME_SCALE,
        10.0,
    )
    .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 12.0)
    .piece(100_000);
    let mut p = gen::GenParams::new(RATE);
    p.time_scale = 10.0;
    let g = gen::GsGenerator::new(p);
    let r = run(h);
    let ch = g.channel();
    let pts = &series_points(&r.results, GLOBALSTAR_DOPPLER[ch - 1].key);
    assert!(pts.len() >= 12, "{}", pts.len());
    let mut se = 0.0;
    let mut worst: f64 = 0.0;
    for t in pts {
        // A point averages the interval before it.
        let t0 = t[0] as f64;
        let want = (g.doppler_at(t0 - 0.125) + g.doppler_at(t0 - 0.01)) / 2.0;
        let e = t[1] as f64 - want;
        se += e * e;
        worst = worst.max(e.abs());
    }
    let rms = (se / pts.len() as f64).sqrt();
    assert!(rms < 2.5e3 && worst < 5e3, "rms {rms} worst {worst}");
    // The bursts on the next channel: 300 ms every 800 ms. The first burst falls in the receiver's 0.2 s warm-up.
    let bursts: Vec<_> = r
        .events_of("globalstar.activity")
        .filter(|e| {
            e.at(&GlobalstarActivityEvent::CHANNEL) == Some((ch + 1) as i64) && e.time > 0.3
        })
        .collect();
    assert!((3..=5).contains(&bursts.len()), "{bursts:?}");
    for b in &bursts {
        let d = b.at(&GlobalstarActivityEvent::DURATION).unwrap();
        assert!((d - 0.3).abs() < 0.08, "{b:?}");
    }
    assert!(cell(&r.results, ch + 1, "occupancy_pct") > 1.0);
}

#[test]
fn g5_bursts_over_seven_seconds() {
    let r = run(harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Pass,
        7.5,
    )
    .piece(1 << 17));
    let n = r
        .events_of("globalstar.activity")
        .filter(|e| e.at(&GlobalstarActivityEvent::CHANNEL) == Some(8))
        .count();
    assert!((6..=9).contains(&n), "{n}");
    assert!(cell(&r.results, 8, "occupancy_pct") > 100.0 * 0.05 * 7.5 / 60.0);
    assert_eq!(row_count(&r.results, "activity"), r.events.len());
}

#[test]
fn cut_independence() {
    let h = harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Pass,
        1.6,
    )
    .set(
        &sdrkit_stack_satcom::modes::globalstar::gen::GEN_TIME_SCALE,
        10.0,
    );
    let a = run(h.clone().piece(997));
    let b = run(h.piece(1 << 16));
    assert_eq!(
        serde_json::to_value(&a.results).unwrap(),
        serde_json::to_value(&b.results).unwrap()
    );
}

#[test]
fn every_event_is_declared_and_has_a_frequency() {
    let r = run(harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Pass,
        3.0,
    ));
    assert!(!r.events.is_empty());
    for e in &r.events {
        assert!(
            e.is(&sdrkit_stack_satcom::typed::GLOBALSTAR_ACTIVITY_EVENT),
            "{e:?}"
        );
        assert!(e.freq_hz > 1.6e9, "{e:?}");
    }
}

#[test]
fn saved_state_comes_back() {
    let h = harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Pass,
        3.0,
    );
    let x = h.generate().unwrap();
    let mut a = h.receiver().unwrap();
    a.configure(&InputInfo::new(
        RATE,
        globalstar::BAND_CENTRE_HZ,
        globalstar::BAND_CENTRE_HZ,
    ))
    .unwrap();
    let mut out = sdrkit_core::Output::default();
    for c in x.chunks(60_000) {
        a.feed(c, &mut out);
    }
    let state = a.save_state().expect("a saved state");
    let mut b = h.receiver().unwrap();
    b.configure(&InputInfo::new(
        RATE,
        globalstar::BAND_CENTRE_HZ,
        globalstar::BAND_CENTRE_HZ,
    ))
    .unwrap();
    b.restore_state(&state).unwrap();
    b.feed(&x[..60_000], &mut out);
    assert_eq!(
        row_count(a.results(), "activity"),
        row_count(b.results(), "activity")
    );
    assert!(b
        .restore_state(&StateBlob::new("argos", 1, Vec::new()))
        .is_err());
}

#[test]
fn bad_settings_are_refused() {
    let mut r = Harness::new(&globalstar::DESCRIPTOR).receiver().unwrap();
    let r = r.as_mut();
    assert!(set(r, "window_s", json!(5000.0)).is_err());
    assert!(set(r, "window_s", json!("wide")).is_err());
    assert!(set(r, "pass_overlay", json!("maybe")).is_err());
    assert!(set(r, "window_s", json!(120.0)).is_ok());
}

#[test]
fn rate_too_low_is_refused() {
    let cfg = StackConfig::new(values(&globalstar::DESCRIPTOR, &[]));
    let mut r = (globalstar::DESCRIPTOR.make_receiver)(&cfg).unwrap();
    let e = r.configure(&InputInfo::new(2e6, 1618.25e6, 1618.25e6));
    assert!(e.is_err());
    assert_eq!(
        r.results().status().summary,
        "Globalstar needs a sample rate of at least 3000 ksps"
    );
}

#[test]
fn the_verification_case_holds_behind_an_8_bit_converter_with_headroom() {
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 3.0,
        clean: false,
        snr_db: 30.0,
    };
    let case = &(globalstar::HOOKS.cases)(&o)[0];
    let mut h = Harness::new(&globalstar::DESCRIPTOR)
        .rate(10e6)
        .seconds(1.5)
        .piece(50_000)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(
            &sdrkit_algo_sim::chain::GEN_SNR_DB,
            case.snr_db.expect("an SNR"),
        )
        .set(
            &sdrkit_algo_sim::chain::GEN_LEVEL_DBFS,
            globalstar::CONVERTER_LEVEL_DBFS,
        );
    h = h.with_settings(&case.settings);
    // An 8-bit converter, as the engine's synthetic radio has: round(clamp(v x 128, -128, 127)) / 128.
    let q = |v: f32| (v * 128.0).clamp(-128.0, 127.0).round() / 128.0;
    let x: Vec<_> = h
        .generate()
        .expect("the signal")
        .iter()
        .map(|c| sdrkit_core::Complex32::new(q(c.re), q(c.im)))
        .collect();
    let r = h.feed(&x).expect("the run");
    let on: Vec<usize> = (1..=13).filter(|&c| active(&r.results, c)).collect();
    assert_eq!(on, [7]);
}

/// The bursts of channel 8 (300 ms every 800 ms) that end inside a 7.5 s pass at `snr` dB in one 1.23 MHz channel.
fn bursts_heard(snr: f64, offset_hz: f64, ppm: f64) -> usize {
    let r = run(harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Pass,
        7.5,
    )
    .piece(1 << 17)
    .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, snr)
    .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, offset_hz)
    .set(&sdrkit_algo_sim::chain::GEN_CLOCK_PPM, ppm));
    // The burst at 0 s falls in the warm-up.
    r.events_of("globalstar.activity")
        .filter(|e| e.at(&GlobalstarActivityEvent::CHANNEL) == Some(8) && e.time > 0.3)
        .count()
}

/// The activity gate opens 0.8 dB over the floor: every burst at -4.5 dB in one channel, also with 18 kHz of carrier error and
/// a clock 50 ppm fast. The bursts from 0.8 to 6.4 s end inside the run (8 of them).
#[test]
fn the_gate_hears_every_burst_at_minus_4_5_db() {
    for (offset, ppm) in [(0.0, 0.0), (18e3, 50.0)] {
        let n = bursts_heard(-4.5, offset, ppm);
        assert!(n >= 8, "{offset} Hz {ppm} ppm: {n} of 8");
    }
}

/// Noise alone whose level drifts by +-0.26 dB over 7 s opens no channel.
#[test]
fn drifting_noise_opens_no_channel() {
    let h = harness(
        sdrkit_stack_satcom::modes::globalstar::gen::Scenario::Silence,
        7.0,
    )
    .set(
        &sdrkit_algo_sim::chain::GEN_NOISE,
        sdrkit_algo_sim::chain::NoiseKind::Off,
    );
    let mut rng = sdrkit_algo_sim::rng::Rng::new(11);
    let n = (7.0 * RATE) as usize;
    let x: Vec<sdrkit_core::Complex32> = (0..n)
        .map(|i| {
            // From -0.26 to +0.26 dB and back.
            let db = 0.26 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            rng.complex_normal(0.01 * 10f64.powf(db / 10.0))
        })
        .collect();
    let r = h.feed(&x).expect("the run");
    assert_eq!(r.events_of("globalstar.activity").count(), 0);
    assert!((1..=13).all(|c| !active(&r.results, c)));
}

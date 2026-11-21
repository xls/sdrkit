//! Wave buoy: the recording, round trips through the standard test-signal chain, and the receiver contract.

use crate::common::*;
use sdrkit_core::{InputInfo, LockState, Receiver, Value};
use sdrkit_stack_hfdata::buoy::gen::{test_system_words, test_vector, test_waves};
use sdrkit_stack_hfdata::buoy::hxv::{system_values, BlockReader, VectorSync};
use sdrkit_stack_hfdata::buoy::{
    self,
    gen::{BuoyGenOptions, BuoyGenerator, BuoySignal},
};
use sdrkit_stack_hfdata::typed::{WAVEBUOY_FRAMES, WAVEBUOY_SPECTRUM};

fn number(rx: &dyn Receiver, key: &str) -> f64 {
    field(rx, key).unwrap_or(f64::NAN)
}

fn text(rx: &dyn Receiver, key: &str) -> String {
    field_text(rx, key)
}

fn yes(rx: &dyn Receiver, key: &str) -> bool {
    value(rx, key) == Value::Bool(true)
}

fn show(rx: &dyn Receiver) {
    let keys = [
        "format",
        "signal_found",
        "offset_hz",
        "channel",
        "width",
        "snr_db",
        "symbol_rate_hz",
        "clock_ppm",
        "tone_step",
        "symbol_error_steps",
        "hxv_locked",
        "vector_count",
        "lost_count",
        "block_count",
        "system_word_count",
        "period_bits",
        "sync",
        "crc_attempt",
        "frames_ok",
    ];
    let line: Vec<String> = keys
        .iter()
        .map(|k| format!("{k} {}", field_text(rx, k)))
        .collect();
    eprintln!("{} | {}", line.join(", "), rx.results().status().summary);
}

#[test]
fn w12_recording() {
    let rec = vector("datawell_2fsk_24k");
    let exp = expected("datawell_2fsk_24k");
    let mut rx = receiver(&buoy::WAVEBUOY, &[]);
    run_vector(rx.as_mut(), &rec);
    let rx = rx.as_ref();
    show(rx);
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert!(yes(rx, "data_valid"));
    assert_eq!(text(rx, "format"), "2FSK");
    assert_eq!(text(rx, "channel"), "Default");
    assert!((number(rx, "offset_hz") - 1500.0).abs() <= 30.0);
    assert!((number(rx, "symbol_rate_hz") - 81.92).abs() <= 0.3);
    assert!((number(rx, "tone_step") - 129.0).abs() <= 10.0);
    let bw = number(rx, "width");
    assert!((115.0..=195.0).contains(&bw), "bw {bw}");
    let snr = number(rx, "snr_db");
    assert!((20.0..=28.0).contains(&snr), "snr {snr}");
    assert!(yes(rx, "hxv_locked"));
    assert!(!yes(rx, "hxv_inverted"));
    assert!(number(rx, "vector_count") >= 33.0);
    assert!(number(rx, "lost_count") <= 1.0);
    assert!(number(rx, "block_count") >= 2.0);
    assert!((number(rx, "hs_m") - 3.4).abs() < 1e-9);
    assert!((number(rx, "hrms_m") - 0.85).abs() < 1e-9);
    assert!((number(rx, "tz_s") - 400.0 / 56.0).abs() < 1e-9);
    let pts = rows(rx.results(), &WAVEBUOY_SPECTRUM);
    for w in exp["spectrum_points"].as_array().unwrap() {
        let g = pts
            .iter()
            .find(|p| num(p, "entry") == w["n"].as_f64().unwrap())
            .expect("point");
        for (k, e) in [
            ("frequency", "hz"),
            ("direction_deg", "dir_deg"),
            ("spread_deg", "spread_deg"),
            ("m2", "m2"),
            ("n2", "n2"),
            ("relative_psd", "rpsd"),
            ("k", "k"),
        ] {
            assert!((num(g, k) - w[e].as_f64().unwrap()).abs() < 1e-3, "{k} {w}");
        }
    }
    assert!(rx
        .results()
        .status()
        .summary
        .starts_with("HXV buoy data, Hs 3.40 m, Tz 7.1 s"));
}

/// The clean signal of `o` through the standard test-signal chain (`gen_*` keys of `set_gen`), fed to a receiver with `set`
/// applied, the radio centre at `centre` and the dial at `tuned`.
fn run(
    o: BuoyGenOptions,
    secs: f64,
    centre: f64,
    tuned: f64,
    set_gen: &[(&str, Json)],
    set: &[(&str, Json)],
) -> Box<dyn Receiver> {
    let rate = o.rate;
    let mut v = gen_values();
    put(&mut v, "gen_seed", json!(11.0));
    for (k, x) in set_gen {
        put(&mut v, k, x.clone());
    }
    let mut g =
        sdrkit_components::sim::impaired(Box::new(BuoyGenerator::new(o)), &v).expect("impairments");
    let x = generate(g.as_mut(), secs);
    let mut rx = receiver(&buoy::WAVEBUOY, set);
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .unwrap();
    feed(rx.as_mut(), &x, 16_384);
    rx
}

/// Noise at `snr_db` in 100 Hz, as the receiver measures it.
fn snr(snr_db: f64) -> Vec<(&'static str, Json)> {
    vec![("gen_noise", json!("snr")), ("gen_snr_db", json!(snr_db))]
}

const DIAL: f64 = 30.4735e6;

#[test]
fn w3_receiver_2fsk() {
    let rx = run(
        BuoyGenOptions::default(),
        100.0,
        DIAL,
        DIAL,
        &snr(24.0),
        &[],
    );
    let rx = rx.as_ref();
    show(rx);
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert!((number(rx, "offset_hz") - 1500.0).abs() <= 30.0);
    assert!(number(rx, "lost_count") <= 3.0);
    assert!(number(rx, "vector_count") >= 120.0);
    assert!((number(rx, "tw_c") - 18.5).abs() < 1e-9);
    assert!((number(rx, "hs_m") - 3.4).abs() < 1e-9);
    assert!((number(rx, "tz_s") - 400.0 / 56.0).abs() < 1e-9);
    assert!((number(rx, "symbol_rate_hz") - 81.92).abs() <= 0.3);
    assert!((number(rx, "tone_step") - 129.0).abs() <= 10.0);
    assert!((number(rx, "width") - 155.0).abs() <= 40.0);
    assert!(yes(rx, "data_valid") && !yes(rx, "hxv_inverted"));
    let pts = rows(rx.results(), &WAVEBUOY_SPECTRUM);
    assert!(!pts.is_empty());
    for p in &pts {
        let n = num(p, "entry");
        let dir = ((40 + 3 * n as u32) & 0xFF) as f64 * 360.0 / 256.0;
        assert!((num(p, "direction_deg") - dir).abs() < 1e-9, "entry {n}");
        assert!(
            (num(p, "k") - (100.0 + n) / 100.0).abs() < 1e-9,
            "entry {n}"
        );
    }
}

#[test]
fn w7_receiver_4fsk() {
    let rx = run(
        BuoyGenOptions {
            signal: BuoySignal::Fsk4,
            offset_hz: -2300.0,
            ..Default::default()
        },
        70.0,
        DIAL,
        DIAL,
        &snr(24.0),
        &[],
    );
    let rx = rx.as_ref();
    show(rx);
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert_eq!(text(rx, "format"), "4MFSK");
    assert!((number(rx, "symbol_rate_hz") - 81.92).abs() <= 0.3);
    assert!((number(rx, "tone_step") - 78.0).abs() <= 10.0);
    assert!(!yes(rx, "hxv_locked"));
    assert_eq!(number(rx, "period_bits"), 128.0);
    assert!(text(rx, "sync").starts_with("D391"));
    assert_eq!(text(rx, "crc_attempt"), "CRC-16/IBM-3740");
    assert!(number(rx, "frames_ok") >= 3.0);
    assert!(!rows(rx.results(), &WAVEBUOY_FRAMES).is_empty());
}

#[test]
fn w2_structure_reader() {
    for inverted in [false, true] {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
        let mut bits: Vec<u8> = (0..20).map(|_| rng.below(2) as u8).collect();
        for i in 0..306u64 {
            let v = test_vector(i, false);
            for k in 0..64 {
                bits.push(((v >> (63 - k)) & 1) as u8);
            }
        }
        let mut s = VectorSync::default();
        let mut r = BlockReader::default();
        let mut recent = Vec::new();
        for (i, &b) in bits.iter().enumerate() {
            let b = if inverted { b ^ 1 } else { b };
            match s.push(b, i as f64) {
                Some(Ok(v)) => {
                    if r.take(&v) {
                        s.flip();
                        r.restart();
                    }
                    recent.push(v);
                }
                Some(Err(idx)) => r.lost(idx),
                None => {}
            }
        }
        let (locked, inv) = s.state();
        assert!(locked && inv == inverted);
        assert!(s.vectors >= 300 && s.lost <= 3, "{} {}", s.vectors, s.lost);
        assert!(r.blocks >= 15);
        let words = test_system_words();
        for (k, w) in words.iter().enumerate() {
            assert_eq!(r.system[k], Some(*w), "word {k}");
        }
        let sv = system_values(&r.system);
        assert!(
            (sv.hrms_m.unwrap() - 0.85).abs() < 1e-12 && (sv.hs_m.unwrap() - 3.4).abs() < 1e-12
        );
        assert!((sv.tz_s.unwrap() - 400.0 / 56.0).abs() < 1e-9);
        assert!((sv.psd_max.unwrap() - 5000.0 * (-5f64).exp()).abs() < 1e-9);
        assert!((sv.tr_c.unwrap() - 25.0).abs() < 1e-12 && (sv.tw_c.unwrap() - 18.5).abs() < 1e-12);
        assert_eq!(
            (sv.weeks_left, sv.battery, sv.tn, sv.gps_fix, sv.gps_ok),
            (Some(26), Some(1), Some(2), Some(true), Some(true))
        );
        assert!((sv.av0.unwrap() - 0.2).abs() < 1e-12 && (sv.ay0.unwrap() + 0.1).abs() < 1e-12);
        assert!((sv.lat.unwrap() - 54.5).abs() < 1e-4 && (sv.lon.unwrap() + 5.2).abs() < 1e-4);
        assert!((sv.orientation_deg.unwrap() - 90.0).abs() < 1e-12);
        assert!(
            (sv.inclination_deg.unwrap() - 90.0 / 128.0 * (225.0 - 128.0 + 6.0 / 16.0)).abs()
                < 1e-9
        );
        let pts = r.points();
        assert!(pts.len() >= 60);
        for p in &pts {
            let n = p.n as f64;
            assert!(
                (p.dir_deg - ((40 + 3 * p.n as u32) & 0xFF) as f64 * 360.0 / 256.0).abs() < 1e-9
            );
            assert!((p.rpsd - (-(100.0 + 40.0 * n) / 200.0).exp()).abs() < 1e-9);
            assert!((p.k - (100.0 + n) / 100.0).abs() < 1e-9);
            assert!((p.psd.unwrap() - p.rpsd * sv.psd_max.unwrap()).abs() < 1e-9);
        }
        for v in recent.iter().rev().take(128) {
            assert!(v.heave.abs() <= 100 && v.north.abs() <= 40 && v.west.abs() <= 30);
        }
        let _ = test_waves(0.0);
    }
}

#[test]
fn w2_bit_slip_is_followed() {
    let mut bits: Vec<u8> = Vec::new();
    for i in 0..60u64 {
        let v = test_vector(i, false);
        for k in 0..64 {
            bits.push(((v >> (63 - k)) & 1) as u8);
        }
        if i == 30 {
            bits.pop(); // one bit lost
        }
    }
    let mut s = VectorSync::default();
    let mut got = 0;
    for (i, &b) in bits.iter().enumerate() {
        if let Some(Ok(_)) = s.push(b, i as f64) {
            got += 1;
        }
    }
    assert!(got >= 55 && s.lost <= 2, "{got} {}", s.lost);
}

#[test]
fn w4_radio_offset() {
    let o = BuoyGenOptions {
        offset_hz: 21_500.0,
        ..Default::default()
    };
    let rx = run(o, 30.0, DIAL - 20e3, DIAL, &snr(24.0), &[]);
    let rx = rx.as_ref();
    show(rx);
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
    assert_eq!(text(rx, "channel"), "Default");
    assert!((number(rx, "offset_hz") - 1500.0).abs() <= 30.0);
}

#[test]
fn w5_inverted_wrsg_and_clock() {
    let o = BuoyGenOptions {
        offset_hz: -2300.0,
        inverted: true,
        wrsg: true,
        ..Default::default()
    };
    let rx = run(o, 40.0, DIAL, DIAL, &snr(24.0), &[]);
    let rx = rx.as_ref();
    show(rx);
    assert!(yes(rx, "hxv_locked"));
    assert!(yes(rx, "hxv_inverted"));
    assert!(yes(rx, "wrsg"));
    for ppm in [3000.0, -3000.0] {
        let mut g = snr(24.0);
        g.push(("gen_clock_ppm", json!(ppm)));
        let rx = run(BuoyGenOptions::default(), 40.0, DIAL, DIAL, &g, &[]);
        let rx = rx.as_ref();
        assert_eq!(rx.results().status().lock_state, LockState::Locked, "{ppm}");
        // A receiver clock running fast counts more samples per symbol: fewer symbols a second, a negative error against 81.92.
        let got = -number(rx, "clock_ppm");
        assert!((got - ppm).abs() <= 1500.0, "{ppm} {got}");
    }
}

#[test]
fn w6_fifteen_db() {
    let mut g = snr(15.0);
    g.push(("gen_offset_hz", json!(7.0)));
    let rx = run(BuoyGenOptions::default(), 60.0, DIAL, DIAL, &g, &[]);
    let rx = rx.as_ref();
    assert!(yes(rx, "hxv_locked"));
    assert!(number(rx, "vector_count") >= 60.0);
    assert!(number(rx, "lost_count") <= 8.0);
}

#[test]
fn w7_forced_4mfsk_on_2fsk() {
    let rx = run(
        BuoyGenOptions::default(),
        30.0,
        DIAL,
        DIAL,
        &snr(24.0),
        &[("format", json!("4mfsk"))],
    );
    let rx = rx.as_ref();
    assert_eq!(text(rx, "format"), "4MFSK");
    assert!(yes(rx, "format_forced"));
    assert_eq!(number(rx, "vector_count"), 0.0);
    assert!((value(rx, "crc_attempt") == Value::None));
}

#[test]
fn a_wrong_format_choice_is_refused() {
    let mut rx = receiver(&buoy::WAVEBUOY, &[]);
    assert!(set(rx.as_mut(), "format", json!("8fsk")).is_err());
    assert!(set(rx.as_mut(), "format", json!("2fsk")).is_ok());
}

#[test]
fn w8_noise_and_carrier() {
    let rx = run(
        BuoyGenOptions {
            signal: BuoySignal::Nothing,
            ..Default::default()
        },
        20.0,
        DIAL,
        DIAL,
        &snr(24.0),
        &[],
    );
    let rx = rx.as_ref();
    assert_eq!(rx.results().status().lock_state, LockState::Searching);
    assert_eq!(number(rx, "vector_count"), 0.0);
    let rx = run(
        BuoyGenOptions {
            signal: BuoySignal::Carrier,
            ..Default::default()
        },
        20.0,
        DIAL,
        DIAL,
        &snr(24.0),
        &[],
    );
    let rx = rx.as_ref();
    assert_ne!(rx.results().status().lock_state, LockState::Locked);
    assert_eq!(text(rx, "bits"), "");
}

#[test]
fn w9_other_rates() {
    for rate in [24e3, 2e6] {
        let rx = run(
            BuoyGenOptions {
                rate,
                ..Default::default()
            },
            36.0,
            DIAL,
            DIAL,
            &snr(24.0),
            &[],
        );
        let rx = rx.as_ref();
        assert!(yes(rx, "hxv_locked"), "{rate}");
        assert!(number(rx, "vector_count") >= 40.0, "{rate}");
        assert!(number(rx, "lost_count") <= 3.0, "{rate}");
    }
}

#[test]
fn clear_forgets_the_frames_and_the_sea_state() {
    let mut rx = run(BuoyGenOptions::default(), 30.0, DIAL, DIAL, &snr(24.0), &[]);
    assert!(number(rx.as_ref(), "system_word_count") > 0.0);
    invoke(rx.as_mut(), "clear", None);
    assert_eq!(number(rx.as_ref(), "system_word_count"), 0.0);
    assert!(rows(rx.results(), &WAVEBUOY_SPECTRUM).is_empty());
    assert_eq!(text(rx.as_ref(), "bits"), "");
}

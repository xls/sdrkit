//! Fleet Broadcast: the column classes, the receiver on its test signal at several carrier offsets, rates, clock errors,
//! drift, noise and fading, the scenarios without a sync channel, and the settings.

use crate::common::{cell, field_text, rows, set, values};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Generator, InputInfo, LockState, Output, Receiver, StackConfig};
use sdrkit_stack_satcom::modes::fleet::{self, gen};
use serde_json::json;

const DIAL: f64 = 250.45e6;

fn run(rate: f64, seconds: f64, set_gen: &[CaseSetting], tune_offset: f64) -> Box<dyn Receiver> {
    let mut set = vec![CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SEED, 4)];
    set.extend_from_slice(set_gen);
    let v = values(&fleet::DESCRIPTOR, &set);
    let cfg = sdrkit_core::GenConfig::new(rate, v.clone()).with_seed(4);
    let g = (fleet::DESCRIPTOR.make_generator.unwrap())(&cfg).unwrap();
    let mut g = sdrkit_components::sim::impaired(g, &v).unwrap();
    let mut x = vec![Complex32::default(); (seconds * rate) as usize];
    g.generate(&mut x);
    let mut r = (fleet::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    r.configure(&InputInfo::new(rate, DIAL + tune_offset, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks((rate * 0.05) as usize) {
        out.clear();
        r.feed(c, &mut out);
    }
    r
}

fn num(r: &dyn Receiver, key: &str) -> f64 {
    field_text(r.results(), key).parse().unwrap_or(f64::NAN)
}

fn snr(db: f64) -> Vec<CaseSetting> {
    vec![
        CaseSetting::new(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        ),
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, db),
    ]
}

/// The classes of scenario 0, channel 0 first: sync, reversals, ones, zeros, reversals, then random bits.
const MIXED: &str = "[Sriireeeeeeeeeee]";

fn assert_mixed(r: &dyn Receiver, what: &str, carrier: f64, tol: f64) {
    let res = r.results();
    assert_eq!(num(r, "state"), 3.0, "{what}: {}", res.status().summary);
    assert_eq!(field_text(res, "classes"), MIXED, "{what}");
    assert_eq!(num(r, "sync_period"), 31.0, "{what}");
    let off = num(r, "offset_hz");
    assert!((off - carrier).abs() < tol, "{what}: carrier {off}");
    let ch2 = rows(res, "channels")
        .into_iter()
        .find(|(_, c)| cell(c, "channel") == "2")
        .unwrap()
        .1;
    assert_eq!(cell(ch2, "last_bits"), "ffffffffffffffff", "{what}");
    assert_eq!(res.status().lock_state, LockState::Locked);
}

#[test]
fn scenario_0_frames_at_every_carrier_offset() {
    for carrier in [-4000.0, -2500.0, 0.0, 1500.0, 3000.0, 4000.0] {
        let g = [
            snr(20.0),
            vec![CaseSetting::new(&gen::GEN_CARRIER_HZ, carrier)],
        ]
        .concat();
        let r = run(250e3, 14.0, &g, fleet::TUNE_OFFSET_HZ);
        assert_mixed(r.as_ref(), &format!("{carrier} Hz"), carrier, 20.0);
        assert!(num(r.as_ref(), "frames_ok") >= 250.0);
        assert!(field_text(r.results(), "note").contains("not confirmed by a public document"));
    }
}

#[test]
fn clock_error_drift_noise_fading_and_a_low_rate() {
    let base = |db| {
        [
            snr(db),
            vec![CaseSetting::new(&gen::GEN_CARRIER_HZ, 1500.0)],
        ]
        .concat()
    };
    for ppm in [-100.0, 100.0] {
        let g = [
            base(20.0),
            vec![CaseSetting::new(
                &sdrkit_algo_sim::chain::GEN_CLOCK_PPM,
                ppm,
            )],
        ]
        .concat();
        let r = run(250e3, 14.0, &g, fleet::TUNE_OFFSET_HZ);
        assert_mixed(r.as_ref(), &format!("{ppm} ppm"), 1500.0, 20.0);
    }
    let g = [
        base(20.0),
        vec![CaseSetting::new(
            &sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S,
            20.0,
        )],
    ]
    .concat();
    let r = run(250e3, 14.0, &g, fleet::TUNE_OFFSET_HZ);
    assert_mixed(r.as_ref(), "drift", 1780.0, 60.0);
    for db in [10.0, 8.0] {
        let r = run(250e3, 16.0, &base(db), fleet::TUNE_OFFSET_HZ);
        assert_mixed(r.as_ref(), &format!("{db} dB"), 1500.0, 20.0);
    }
    let g = [base(15.0), vec![CaseSetting::new(&gen::GEN_FADE_DB, 12.0)]].concat();
    let r = run(250e3, 16.0, &g, fleet::TUNE_OFFSET_HZ);
    assert_mixed(r.as_ref(), "fading", 1500.0, 20.0);
    let g = [
        snr(20.0),
        vec![CaseSetting::new(&gen::GEN_CARRIER_HZ, -2200.0)],
    ]
    .concat();
    let r = run(48e3 * 2.0, 14.0, &g, fleet::TUNE_OFFSET_HZ);
    assert_mixed(r.as_ref(), "96 ksps", -2200.0, 20.0);
}

/// The generator alone (its own carrier placement, the stack's tune offset) at 8 dB in 1.2 kHz: the receiver locks on the
/// carrier where the generator put it, not on an image of the decimation 30 kHz away.
#[test]
fn locks_at_8_db_from_the_generator_alone() {
    let r = run(250e3, 16.0, &snr(8.0), fleet::TUNE_OFFSET_HZ);
    // The generator's own carrier, 1500 Hz above the dial.
    assert_mixed(r.as_ref(), "8 dB", 1500.0, 20.0);
}

#[test]
fn scenarios_without_a_sync_channel() {
    let sc = |s: gen::GenScenario| vec![CaseSetting::new(&gen::GEN_SCENARIO, s)];
    let r = run(
        250e3,
        14.0,
        &[snr(20.0), sc(gen::GenScenario::Idle)].concat(),
        fleet::TUNE_OFFSET_HZ,
    );
    assert_eq!(num(r.as_ref(), "state"), 3.0);
    assert!(field_text(r.results(), "classes").starts_with("[Si"));
    let r = run(
        250e3,
        20.0,
        &[snr(20.0), sc(gen::GenScenario::Noise)].concat(),
        fleet::TUNE_OFFSET_HZ,
    );
    assert!(num(r.as_ref(), "state") < 2.0);
    assert_eq!(num(r.as_ref(), "symbols"), 0.0);
    let r = run(
        250e3,
        20.0,
        &[snr(20.0), sc(gen::GenScenario::Carrier)].concat(),
        fleet::TUNE_OFFSET_HZ,
    );
    assert_ne!(num(r.as_ref(), "state"), 3.0);
    assert_eq!(num(r.as_ref(), "symbols"), 0.0);
    let r = run(
        250e3,
        14.0,
        &[snr(20.0), sc(gen::GenScenario::Random)].concat(),
        fleet::TUNE_OFFSET_HZ,
    );
    assert_eq!(num(r.as_ref(), "state"), 2.0);
    let classes = field_text(r.results(), "classes");
    assert!(classes.matches('e').count() >= 14, "{classes}");
    assert!(!classes.contains('S'));
}

#[test]
fn settings_are_checked() {
    let v = values(&fleet::DESCRIPTOR, &[]);
    let mut r = (fleet::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    assert!(set(r.as_mut(), "invert", json!("perhaps")).is_err());
    assert!(set(r.as_mut(), "invert", json!(true)).is_ok());
    assert!(r.configure(&InputInfo::new(60e3, DIAL, DIAL)).is_err());
}

/// The data file holds the three carriers of the specification.
#[test]
fn the_carrier_file_has_its_rows() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/satcom/fleet_broadcast_carriers.csv");
    let text = std::fs::read_to_string(path).expect("the data file");
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .collect();
    assert_eq!(rows.len(), 3);
    assert!(rows
        .iter()
        .all(|r| r.split(',').nth(1).unwrap().parse::<f64>().unwrap() > 250e6));
}

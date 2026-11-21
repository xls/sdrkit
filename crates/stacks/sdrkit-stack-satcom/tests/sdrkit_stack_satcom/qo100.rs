//! QO-100: the beacon lock and LNB correction from an LNB error set by the standard impairments, the
//! signals, the inverted LO, and the declared results and events.

use crate::common::{cell, cell_num, field_text, number, rows, set};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Results, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_satcom::modes::qo100::{self, gen, LoChoice};
use serde_json::json;

/// The noise of the test signal: about 29 dB under the signal over 2 MHz.
const FLOOR_DBFS: f64 = -29.0;

/// The LNB error at time `t` of a run: 27 kHz high, drifting 120 Hz/min.
fn error_at(t: f64) -> f64 {
    27e3 + 2.0 * t
}

/// The test signal through the receiver, with the LNB error of `error_at` (`sign` -1 for an LO under the downlink, +1 over it).
fn run(lo: LoChoice, sign: f64, secs: f64, extra: &[CaseSetting]) -> Run {
    let mut p = gen::GenParams::new(2e6);
    p.lo = qo100::lo_of(lo, 0.0);
    let lo = match lo {
        LoChoice::L10600 => LoChoice::L10600,
        _ => LoChoice::L9750,
    };
    let mut h = Harness::new(&qo100::DESCRIPTOR)
        .rate(2e6)
        .freq(p.centre_if())
        .seconds(secs)
        .piece(65_536)
        .set(&sdrkit_stack_satcom::modes::qo100::gen::GEN_LO, lo)
        .set(&sdrkit_stack_satcom::modes::qo100::rx::LO, lo)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Floor,
        )
        .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, FLOOR_DBFS)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, sign * error_at(0.0))
        .set(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, sign * 2.0);
    h = h.with_settings(extra);
    h.run().expect("the run")
}

fn num(r: &Results, key: &str) -> f64 {
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .and_then(number)
        .unwrap_or(f64::NAN)
}

fn flag(r: &Results, key: &str) -> Option<bool> {
    let id = r.decl().field_id(key)?;
    match r.fields().value(id)? {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// The RF of every signal of the last narrowband spectrum.
fn signals(r: &Results) -> Vec<f64> {
    rows(r, "signals")
        .iter()
        .filter_map(|(_, row)| cell_num(row, "rf_hz"))
        .collect()
}

fn nearest(v: &[f64], f: f64) -> f64 {
    v.iter()
        .copied()
        .min_by(|a, b| (a - f).abs().total_cmp(&(b - f).abs()))
        .unwrap_or(f64::NAN)
}

#[test]
fn q2_q4_lock_error_beacons_and_signals_at_2_msps() {
    let secs = 6.0;
    let run = run(LoChoice::L9750, -1.0, secs, &[]);
    let r = &run.results;
    assert_eq!(field_text(r, "beacon_lock"), "locked");
    let want = error_at(secs);
    let err = num(r, "lnb_error_hz");
    assert!((err - want).abs() < 50.0, "{err} vs {want}");
    assert!((num(r, "lnb_error_ppm") - want / 9750e6 * 1e6).abs() < 0.01);
    assert!(num(r, "measurements") > 10.0);
    for (id, row) in rows(r, "beacons") {
        assert_eq!(cell(row, "found"), "true", "{id}");
    }
    let s = signals(r);
    let tc = nearest(&s, gen::TEST_CARRIER);
    assert!((tc - gen::TEST_CARRIER).abs() < 20.0, "{tc}");
    // The wideband transponder is outside a 2 Msps capture at 10490.1 MHz.
    assert_eq!(flag(r, "wideband_outside"), Some(true));
    for f in gen::SSB_STATIONS {
        assert!(
            s.iter().any(|&x| x > f && x < f + 2500.0),
            "station {f}: {s:?}"
        );
    }
    // The narrowband spectrum has bins under 80 Hz.
    let id = r.decl().series_id("narrowband").unwrap();
    let nb = &r.series(id).unwrap().points;
    assert!(nb.len() > 2 && nb[1][0] - nb[0][0] < 80.0);
}

#[test]
fn q3_correction_off_still_measures() {
    let run = run(
        LoChoice::L9750,
        -1.0,
        4.0,
        &[CaseSetting::new(
            &sdrkit_stack_satcom::modes::qo100::rx::AUTO_CORRECTION,
            false,
        )],
    );
    let r = &run.results;
    assert_eq!(field_text(r, "beacon_lock"), "locked");
    assert!((num(r, "lnb_error_hz") - error_at(4.0)).abs() < 50.0);
    // Frequencies are not corrected: the test carrier shows the LNB's error.
    let tc = nearest(&signals(r), gen::TEST_CARRIER - error_at(4.0));
    assert!(
        (tc - (gen::TEST_CARRIER - error_at(4.0))).abs() < 3e3,
        "{tc}"
    );
}

#[test]
fn q5_inverted_lo() {
    let run = run(LoChoice::L10600, 1.0, 5.0, &[]);
    let r = &run.results;
    assert_eq!(field_text(r, "beacon_lock"), "locked");
    let err = num(r, "lnb_error_hz");
    assert!((err - error_at(5.0)).abs() < 50.0, "{err}");
    let tc = nearest(&signals(r), gen::TEST_CARRIER);
    assert!((tc - gen::TEST_CARRIER).abs() < 20.0, "{tc}");
}

#[test]
fn noise_only_never_locks() {
    let run = run(
        LoChoice::L9750,
        -1.0,
        3.0,
        &[CaseSetting::new(
            &sdrkit_stack_satcom::modes::qo100::gen::GEN_EMPTY,
            true,
        )],
    );
    assert_ne!(field_text(&run.results, "beacon_lock"), "locked");
}

#[test]
fn results_and_events_are_declared() {
    use sdrkit_stack_satcom::typed::{
        QO100_CONVERSATION as CONVERSATION_EVENT, QO100_LOCK as LOCK_EVENT,
    };
    let run = run(LoChoice::L9750, -1.0, 4.0, &[]);
    assert!(run.events.iter().any(|e| e.is(&LOCK_EVENT)));
    for e in &run.events {
        assert!(e.is(&LOCK_EVENT) || e.is(&CONVERSATION_EVENT), "{e:?}");
        assert!(e.freq_hz > 0.0, "{e:?}");
    }
}

#[test]
fn bad_settings_are_refused() {
    let mut rx = Harness::new(&qo100::DESCRIPTOR).receiver().unwrap();
    let rx = rx.as_mut();
    assert!(set(rx, "lo", json!("9999")).is_err());
    assert!(set(rx, "lnb_ppm", json!(500.0)).is_err());
    assert!(set(rx, "auto_listen", json!("sometimes")).is_err());
    assert!(set(rx, "conversation_gap_s", json!(1.0)).is_err());
    assert!(set(rx, "lo", json!("10600")).is_ok());
}

#[test]
fn the_signal_above_the_dial_is_measured_against_the_psk_beacon() {
    let level = |r: &Results, pick: &dyn Fn(&sdrkit_core::Record) -> bool| {
        rows(r, "signals")
            .iter()
            .filter(|(_, row)| pick(row))
            .filter_map(|(_, row)| cell_num(row, "level_dbfs"))
            .fold(f64::NEG_INFINITY, f64::max)
    };
    // The dial on a station of the test signal: its level against the beacon.
    let dial = gen::SSB_STATIONS[1];
    let run_on = run(
        LoChoice::L9750,
        -1.0,
        6.0,
        &[CaseSetting::new(&qo100::display::DIAL, dial)],
    );
    let r = &run_on.results;
    let beacon = level(r, &|row| cell(row, "beacon") == "PSK");
    let own = level(r, &|row| {
        cell(row, "beacon").is_empty()
            && cell_num(row, "rf_hz").is_some_and(|f| f >= dial && f <= dial + 2700.0)
    });
    let got = num(r, "own_to_beacon_db");
    assert!(beacon.is_finite() && own.is_finite(), "{beacon} {own}");
    assert!((got - (own - beacon)).abs() < 1e-6, "{got} vs {own} - {beacon}");
    // The dial on an empty place: the peaks of the noise there, far under the station.
    let run_off = run(
        LoChoice::L9750,
        -1.0,
        6.0,
        &[CaseSetting::new(&qo100::display::DIAL, 10_489.70e6)],
    );
    let off = num(&run_off.results, "own_to_beacon_db");
    eprintln!("qo100 own level against the beacon: {got:.1} dB on a station, {off:.1} dB on an empty place");
    assert!(off < got - 20.0, "{off} vs {got}");
}

//! Results persistence: each stack returns its tables as saved state, a fresh receiver loads it back with the times heard kept,
//! and a state of another format is refused with nothing changed.

use crate::common::*;
use sdrkit_core::{Receiver, StackDescriptor, Value};
use sdrkit_stack_marine::atis::AtisStationRow;
use sdrkit_stack_marine::vhf::VhfCallRow;
use sdrkit_stack_marine::{AIS, ATIS, MARINE, MARINEVHF, VDES};
use sdrkit_util_objects::publish::columns;
use serde_json::json;

/// The wall-clock time of the first sample of each recording.
const TIME_BASE: f64 = 1.7e9;

/// Runs a vector of `d` (or generated samples) and returns the receiver.
fn run_on(d: &StackDescriptor, set: &[(&str, Json)], name: &str) -> Option<Box<dyn Receiver>> {
    let rec = vector(name)?;
    let mut rx = receiver(d, None, set);
    configure(rx.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let _ = run(rx.as_mut(), &rec.samples, 8192);
    Some(rx)
}

#[test]
fn ais_tables_survive_a_save_and_load() {
    let Some(rec) = vector("ais_gpsd_250k") else {
        return;
    };
    let Some(rx) = run_on(&AIS, &[("time_base", json!(TIME_BASE))], "ais_gpsd_250k") else {
        return;
    };
    let r0 = rx.results().clone();
    assert_eq!(count(&r0, "stations"), 12);
    let state = rx.save_state().expect("a saved state");
    assert!(holds(&state, "marine.ais_stations"));
    // A fresh receiver with some signal behind it, then the state loaded.
    let mut rx2 = receiver(&AIS, None, &[("time_base", json!(TIME_BASE))]);
    configure(rx2.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let _ = run(rx2.as_mut(), &silence((rec.rate_hz * 0.5) as usize), 8192);
    rx2.restore_state(&state).expect("its own state loads");
    let r = rx2.results().clone();
    assert_eq!(count(&r, "stations"), 12);
    // The sentences are kept. Their signal times move into the signal time of the new receiver.
    let sentences = |r: &sdrkit_core::Results| -> Vec<String> {
        text_rows(r, "sentences")
            .into_iter()
            .map(|row| row[0].clone())
            .collect()
    };
    assert_eq!(sentences(&r), sentences(&r0));
    // The times heard are kept: a station was last heard when it was before the save.
    let last = |r: &sdrkit_core::Results| {
        let t = r.table(r.decl().table_id("stations").unwrap()).unwrap();
        t.row("371798000")
            .unwrap()
            .get(&columns::LAST_HEARD)
            .unwrap()
    };
    assert_eq!(last(&r), last(&r0));
    // A state of another format, or no state at all, is refused and changes nothing.
    assert!(rx2
        .restore_state(&renamed(&state, "marine.ais_stations", "marine.vhf_calls"))
        .is_err());
    assert!(rx2.restore_state(&not_a_state()).is_err());
    assert_eq!(count(rx2.results(), "stations"), 12);
}

#[test]
fn marine_hf_lists_survive_a_save_and_load() {
    let Some(rec) = vector("dsc_mfhf_12k") else {
        return;
    };
    let Some(rx) = run_on(&MARINE, &[], "dsc_mfhf_12k") else {
        return;
    };
    let r0 = rx.results().clone();
    assert_eq!(rows(&r0, "dsc").len(), 4);
    let state = rx.save_state().expect("a saved state");
    assert!(holds(&state, "marine.hf_lists"));
    let mut rx2 = receiver(&MARINE, None, &[]);
    configure(rx2.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let _ = run(rx2.as_mut(), &silence((rec.rate_hz * 0.5) as usize), 8192);
    rx2.restore_state(&state).expect("its own state loads");
    let r = rx2.results().clone();
    assert_eq!(text_rows(&r, "dsc"), text_rows(&r0, "dsc"));
    assert_eq!(count(&r, "dsc_good"), 4);
    // A state of another format, or no state at all, is refused and changes nothing.
    assert!(rx2
        .restore_state(&renamed(&state, "marine.hf_lists", "marine.vhf_calls"))
        .is_err());
    assert!(rx2.restore_state(&not_a_state()).is_err());
    assert_eq!(text_rows(rx2.results(), "dsc"), text_rows(&r0, "dsc"));
    // Clear empties the lists and the totals.
    invoke(rx2.as_mut(), "clear", None);
    let _ = run(rx2.as_mut(), &silence((rec.rate_hz * 0.3) as usize), 8192);
    assert_eq!(rows(rx2.results(), "dsc").len(), 0);
    assert_eq!(count(rx2.results(), "dsc_good"), 0);
}

#[test]
fn vdes_lists_survive_a_save_and_load() {
    let Some(rec) = vector("vdes_asm_vdeter_400k") else {
        return;
    };
    let Some(rx) = run_on(&VDES, &[], "vdes_asm_vdeter_400k") else {
        return;
    };
    let r0 = rx.results().clone();
    assert_eq!(rows(&r0, "asm").len(), 2);
    let state = rx.save_state().expect("a saved state");
    assert!(holds(&state, "marine.vdes_lists"));
    let mut rx2 = receiver(&VDES, None, &[]);
    configure(rx2.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let _ = run(rx2.as_mut(), &silence((rec.rate_hz * 0.5) as usize), 8192);
    rx2.restore_state(&state).expect("its own state loads");
    let r = rx2.results().clone();
    assert_eq!(text_rows(&r, "asm"), text_rows(&r0, "asm"));
    assert_eq!(text_rows(&r, "bursts"), text_rows(&r0, "bursts"));
    // A state of another format, or no state at all, is refused and changes nothing.
    assert!(rx2
        .restore_state(&renamed(&state, "marine.vdes_lists", "marine.atis_lists"))
        .is_err());
    assert!(rx2.restore_state(&not_a_state()).is_err());
    assert_eq!(text_rows(rx2.results(), "asm"), text_rows(&r0, "asm"));
    // Clear empties the lists.
    invoke(rx2.as_mut(), "clear", None);
    let _ = run(rx2.as_mut(), &silence((rec.rate_hz * 0.3) as usize), 8192);
    assert_eq!(rows(rx2.results(), "asm").len(), 0);
    assert_eq!(rows(rx2.results(), "bursts").len(), 0);
}

#[test]
fn atis_lists_survive_a_save_and_load() {
    let Some(rec) = vector("atis_inland_48k") else {
        return;
    };
    let Some(rx) = run_on(&ATIS, &[("time_base", json!(TIME_BASE))], "atis_inland_48k") else {
        return;
    };
    let r0 = rx.results().clone();
    assert_eq!(rows(&r0, "stations").len(), 5);
    let state = rx.save_state().expect("a saved state");
    assert!(holds(&state, "marine.atis_lists"));
    let mut rx2 = receiver(&ATIS, None, &[("time_base", json!(TIME_BASE))]);
    configure(rx2.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let _ = run(rx2.as_mut(), &silence((rec.rate_hz * 0.5) as usize), 8192);
    rx2.restore_state(&state).expect("its own state loads");
    let r = rx2.results().clone();
    // The same stations and counts, last heard when they were before the save.
    let (sa, sb) = (rows(&r0, "stations"), rows(&r, "stations"));
    assert_eq!(sb.len(), 5);
    let cells =
        |r: &sdrkit_core::Record| -> Vec<Value> { r.iter().map(|(_, v)| v.clone()).collect() };
    for (x, y) in sa.iter().zip(&sb) {
        assert_eq!(cells(x)[..5], cells(y)[..5]);
        assert_eq!(x.at(&AtisStationRow::DIGITS), y.at(&AtisStationRow::DIGITS));
        // A number is read back to within one unit in the last place, so the level is compared with a tolerance.
        let (a, b) = (
            x.at(&AtisStationRow::BEST_LEVEL_DB).unwrap(),
            y.at(&AtisStationRow::BEST_LEVEL_DB).unwrap(),
        );
        assert!((a - b).abs() < 1e-9, "{a} {b}");
        assert_eq!(x.get(&columns::HEARD), y.get(&columns::HEARD));
        assert_eq!(x.get(&columns::LAST_HEARD), y.get(&columns::LAST_HEARD));
    }
    assert_eq!(text_rows(&r, "messages"), text_rows(&r0, "messages"));
    assert_eq!(count(&r, "frames_ok"), count(&r0, "frames_ok"));
    // A state of another format, or no state at all, is refused and changes nothing.
    assert!(rx2
        .restore_state(&renamed(&state, "marine.atis_lists", "marine.vdes_lists"))
        .is_err());
    assert!(rx2.restore_state(&not_a_state()).is_err());
    assert_eq!(rows(rx2.results(), "stations").len(), 5);
}

#[test]
fn marine_vhf_call_log_survives_a_save_and_load() {
    use sdrkit_stack_marine::vhf::{VhfGen, VhfGenerator};
    let o = VhfGen::default();
    let (rate, centre) = (o.rate, o.centre_hz);
    let mut g = impaired(Box::new(VhfGenerator::new(o)), &snr(30.0));
    let x = generate(g.as_mut(), 9.0);
    let mut rx = receiver(&MARINEVHF, None, &[]);
    configure(rx.as_mut(), rate, centre, centre - 12.5e3);
    let _ = run(rx.as_mut(), &x, 65_536);
    let state = rx.save_state().expect("a saved state");
    assert!(holds(&state, "marine.vhf_lists"));
    let a = rows(rx.results(), "calls");
    assert_eq!(a.len(), 3);
    let mut rx2 = receiver(&MARINEVHF, None, &[]);
    configure(rx2.as_mut(), rate, centre, centre - 12.5e3);
    let _ = run(rx2.as_mut(), &silence((rate * 0.8) as usize), 65_536);
    rx2.restore_state(&state).expect("its own state loads");
    let b = rows(rx2.results(), "calls");
    assert_eq!(b.len(), 3);
    let cells =
        |r: &sdrkit_core::Record| -> Vec<Value> { r.iter().map(|(_, v)| v.clone()).collect() };
    for (x, y) in a.iter().zip(&b) {
        // The serial, channel and frequency stay.
        assert_eq!(cells(x)[..3], cells(y)[..3]);
        // The start moves with the clock: the saved receiver had about 9 s behind it, the new one 0.8 s.
        let dt = x.at(&VhfCallRow::START).unwrap() - y.at(&VhfCallRow::START).unwrap();
        assert!((dt - 8.2).abs() < 0.3, "{dt}");
    }
    // A state of another format, or no state at all, is refused and changes nothing.
    assert!(rx2
        .restore_state(&renamed(&state, "marine.vhf_lists", "marine.atis_lists"))
        .is_err());
    assert!(rx2.restore_state(&not_a_state()).is_err());
    assert_eq!(rows(rx2.results(), "calls").len(), 3);
    // Clear empties the log.
    invoke(rx2.as_mut(), "clear", None);
    let _ = run(rx2.as_mut(), &silence((rate * 0.3) as usize), 65_536);
    assert_eq!(rows(rx2.results(), "calls").len(), 0);
}

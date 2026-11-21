//! Inmarsat, all carriers: the detector's classes, the decoding of Aero P and Inmarsat-C carriers in one capture, the windows and
//! the satellite identification.

use crate::common::{cell, command_with, field_text, number, set, values};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    Complex32, Event, InputInfo, Output, Receiver, StackConfig, Station, TableDef, Value,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satcom::modes::inmarsat;
use sdrkit_stack_satcom::typed;
use sdrkit_util_geo::Position;
use serde_json::json;

const RATE: f64 = 1.2e6;
const CENTRE: f64 = 1545.57e6;

fn rx_for(settings: &[CaseSetting], station: Option<Station>) -> Box<dyn Receiver> {
    let values = values(&inmarsat::DESCRIPTOR, settings);
    let mut rx = (inmarsat::DESCRIPTOR.make_receiver)(&StackConfig::new(values)).unwrap();
    if let Some(s) = station {
        rx.set_station(&s);
    }
    rx.configure(&InputInfo::new(RATE, CENTRE, CENTRE).recording())
        .unwrap();
    rx
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32], piece: usize) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(piece.max(1)) {
        out.clear();
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
    }
    ev
}

fn num(rx: &dyn Receiver, key: &str) -> f64 {
    let r = rx.results();
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .and_then(number)
        .unwrap_or_else(|| panic!("no number {key}"))
}

fn def(table: &str) -> &'static TableDef {
    match table {
        "carriers" => &typed::INMARSAT_CARRIERS,
        _ => &typed::INMARSAT_CANDIDATES,
    }
}

/// The cells of a table as text, newest row first, in the order of the declared columns.
fn rows(rx: &dyn Receiver, table: &str) -> Vec<Vec<String>> {
    crate::common::rows(rx.results(), table)
        .into_iter()
        .map(|(_, r)| {
            def(table)
                .columns
                .iter()
                .map(|c| cell(r, c.key()))
                .collect()
        })
        .collect()
}

fn col(table: &str, key: &str) -> usize {
    def(table)
        .columns
        .iter()
        .position(|c| c.key() == key)
        .unwrap_or_else(|| panic!("no column {key} in {table}"))
}

#[test]
fn one_capture_decodes_every_kind() {
    let x = Harness::new(&inmarsat::DESCRIPTOR)
        .rate(RATE)
        .seconds(28.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::EbN0,
        )
        .set(&sdrkit_algo_sim::chain::GEN_EBN0_DB, 14.0)
        .generate()
        .unwrap();
    let station = Station::at(Position::new(48.0, 11.0));
    let mut r = rx_for(&[], Some(station));
    let ev = feed(r.as_mut(), &x, 65_536);
    let carriers = rows(r.as_ref(), "carriers");
    let (cl, dec, id) = (
        col("carriers", "class"),
        col("carriers", "decodes"),
        col("carriers", "id"),
    );
    let classes: Vec<&str> = carriers.iter().map(|c| c[cl].as_str()).collect();
    assert!(
        classes.contains(&"narrow") && classes.contains(&"wide"),
        "{classes:?}"
    );
    assert!(
        carriers.iter().filter(|c| c[dec] == "aero_p").count() >= 3,
        "{carriers:?}"
    );
    assert!(num(r.as_ref(), "messages") >= 5.0);
    assert!(num(r.as_ref(), "egc_frames") >= 1.0);
    assert_eq!(
        r.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert!(!field_text(r.results(), "satellite").is_empty());
    assert!(r.results().status().summary.starts_with("Inmarsat: "));
    let res = r.results();
    let spectrum = res.decl().series_id(inmarsat::SPECTRUM.key).unwrap();
    assert!(!res.series(spectrum).unwrap().points.is_empty());
    for e in &ev {
        assert!(
            e.is(&typed::AERO_MESSAGE) || e.is(&typed::AERO_LOGON) || e.is(&typed::INMARSATC_EGC),
            "{e:?}"
        );
        assert!(e.freq_hz > 1.5e9, "{e:?}");
    }
    // A tiny budget parks windows; the focused carrier keeps its window.
    let focus: f64 = carriers[0][id].parse().unwrap();
    set(r.as_mut(), "budget", json!(2.0)).unwrap();
    let row = Value::Text(format!("{:05}", focus as u64));
    command_with(r.as_mut(), "carrier.focus", &[("row", row)]).unwrap();
    feed(r.as_mut(), &x[..(0.5 * RATE) as usize], 65_536);
    let carriers = rows(r.as_ref(), "carriers");
    let parked = col("carriers", "parked");
    assert!(carriers.iter().any(|c| c[parked] == "true"));
    assert!(carriers
        .iter()
        .filter(|c| c[id].parse::<f64>().ok() == Some(focus))
        .all(|c| c[parked] == "false"));
    // A change of frequency by the host keeps the logs of the two displays and the totals of the stack. The count of
    // the frames of the Aero channels goes.
    let count = |r: &dyn Receiver, table: &str| crate::common::row_count(r.results(), table);
    let kept = |r: &dyn Receiver| {
        (
            count(r, "aero_display.messages"),
            count(r, "aero_display.aircraft"),
            count(r, "egc.messages"),
            num(r, "messages"),
            num(r, "egc_frames"),
            num(r, "frames_ok"),
        )
    };
    let held = kept(r.as_ref());
    assert!(held.0 >= 5 && held.2 >= 1, "{held:?}");
    assert!(num(r.as_ref(), "aero_display.frames") > 0.0);
    r.forget();
    assert_eq!(kept(r.as_ref()), held);
    assert_eq!(num(r.as_ref(), "aero_display.frames"), 0.0);
}

#[test]
fn noise_has_no_carrier() {
    let x = Harness::new(&inmarsat::DESCRIPTOR)
        .rate(RATE)
        .seconds(3.0)
        .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Floor,
        )
        .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -34.0)
        .generate()
        .unwrap();
    let mut r = rx_for(&[], None);
    feed(r.as_mut(), &x, 65_536);
    assert!(rows(r.as_ref(), "carriers").is_empty());
    assert_eq!(
        r.results().status().lock_state,
        sdrkit_core::LockState::Searching
    );
}

#[test]
fn bad_setting_values_are_refused() {
    let mut r = rx_for(&[], None);
    assert!(set(r.as_mut(), "budget", json!(100.0)).is_err());
    assert!(set(r.as_mut(), "satellite", json!("Sputnik")).is_err());
    assert!(set(r.as_mut(), "inmarsat_c", json!("perhaps")).is_err());
}

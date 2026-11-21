//! The rules every stack of the crate follows: identities, refused rates, bad samples, results at the end of a short stream,
//! declared events, strict settings, commands and saved state.

use super::rows;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::{keys, TableDef};
use sdrkit_core::{Complex32, InputInfo, Output, StackConfig, StackDescriptor, StateBlob, Value};
use sdrkit_dev_test::Harness;
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::dji::{GEN_PERIOD_S, WALL_CLOCK};
use sdrkit_stack_drones::flarm::START_TIME;
use sdrkit_stack_drones::{
    descriptors, ADSL, DJI_DRONEID, FLARM, MAVLINK_SIK,
    OGN, RC_LINKS, REMOTE_ID,
};

/// The harness of stack `d` with the settings that make its test signal give events.
fn harness(d: &'static StackDescriptor) -> Harness {
    let mut h = Harness::new(d).seconds(if d.id == "flarm" { 3.0 } else { 0.7 });
    if d.id == "flarm" {
        h = h
            .set(&WALL_CLOCK, false)
            .set(&START_TIME, 1_800_000_000.0)
            .station(sdrkit_core::Station::at(sdrkit_util_geo::Position::new(
                54.70, 25.30,
            )));
    }
    if d.id == "dji_droneid" {
        h = h.set(&GEN_PERIOD_S, 0.06);
    }
    h
}

/// The table of tracked objects of stack `d`.
fn objects(d: &StackDescriptor) -> &'static TableDef {
    match d.id {
        "remote_id" => typed::REMOTE_ID_DRONES.def(),
        "dji_droneid" => typed::DJI_DRONES.def(),
        "adsl" => typed::ADSL_AIRCRAFT.def(),
        "ogn" => typed::OGN_AIRCRAFT.def(),
        "rc_links" | "mavlink_sik" => typed::LINKS.def(),
        _ => typed::FLARM_AIRCRAFT.def(),
    }
}

#[test]
fn descriptors_have_their_identities() {
    let ids: Vec<&str> = descriptors().iter().map(|d| d.id).collect();
    assert_eq!(
        ids,
        ["remote_id", "dji_droneid", "flarm", "adsl", "ogn", "rc_links", "mavlink_sik"]
    );
    assert_eq!(RC_LINKS.tuning.min_rate_hz, 1e6);
    assert_eq!(MAVLINK_SIK.tuning.min_rate_hz, 1e6);
    assert_eq!(ADSL.tuning.min_rate_hz, 1e6);
    assert_eq!(OGN.tuning.min_rate_hz, 1e6);
    assert_eq!(REMOTE_ID.tuning.min_rate_hz, 20e6);
    assert_eq!(DJI_DRONEID.tuning.min_rate_hz, 16e6);
    assert_eq!(FLARM.tuning.min_rate_hz, 1e6);
    for d in descriptors() {
        assert!(d.make_generator.is_some());
        let s = (d.settings)();
        for k in [
            "clear",
            "gen_snr_db",
            "gen_offset_hz",
            "export",
            "restore",
            "receiver_position",
        ] {
            assert!(s.id(k).is_err(), "{} declares {k}", d.id);
        }
        let rx = (d.make_receiver)(&StackConfig::new(Harness::new(d).values().unwrap())).unwrap();
        for k in ["clear", "remove"] {
            assert!(rx.commands().id(k).is_ok(), "{} has no command {k}", d.id);
        }
    }
}

#[test]
fn too_low_a_rate_is_refused_with_the_stack_message() {
    for d in descriptors() {
        let mut rx = Harness::new(d).receiver().unwrap();
        let r = d.tuning.min_rate_hz / 2.0;
        assert!(rx
            .configure(&InputInfo::new(r, d.tuning.default_hz, d.tuning.default_hz).recording())
            .is_err());
        assert!(
            Owner::results(&*rx)
                .status()
                .summary
                .contains("needs a sample rate of at least"),
            "{}",
            d.id
        );
    }
}

#[test]
fn bad_samples_are_counted_and_the_receiver_runs_on() {
    for d in descriptors() {
        let mut rx = Harness::new(d).receiver().unwrap();
        let rate = d.tuning.rate_hz;
        rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording())
            .unwrap();
        let mut x = vec![Complex32::new(0.001, 0.0); (rate * 0.05) as usize];
        x[10] = Complex32::new(f32::NAN, 0.0);
        x[20] = Complex32::new(f32::INFINITY, 1.0);
        let mut out = Output::default();
        rx.feed(&x, &mut out);
        rx.feed(&[], &mut out);
        let bad = Owner::results(&*rx).fields().get(&keys::BAD_SAMPLES);
        assert!(bad.unwrap_or(0) >= 2, "{}", d.id);
    }
}

#[test]
fn every_stack_publishes_and_declares_its_events() {
    for d in descriptors() {
        let run = harness(d).run().unwrap();
        assert!(!run.events.is_empty(), "{} sent no event", d.id);
        let declared = run.results.decl().events();
        for e in &run.events {
            assert!(
                declared.iter().any(|k| k.kind == e.kind()),
                "{}: {} is not declared",
                d.id,
                e.kind()
            );
            assert!(e.get(&keys::SOURCE).is_some(), "{}: no source", d.id);
            // Radio-control links carry no position.
            if d.id != "rc_links" {
                assert!(e.get(&keys::POSITION).is_some(), "{}: no position", d.id);
            }
            assert!(
                e.freq_hz != 0.0,
                "{}: {} sent with no frequency",
                d.id,
                e.kind()
            );
        }
    }
}

#[test]
fn settings_are_strict() {
    for d in descriptors() {
        let rx = Harness::new(d).receiver().unwrap();
        assert!(rx.schema().id("no_such_key").is_err(), "{}", d.id);
        assert!(rx.commands().id("no_such_command").is_err(), "{}", d.id);
    }
    let mut rx = Harness::new(&FLARM).receiver().unwrap();
    let expiry = rx.schema().id("expiry_s").unwrap();
    assert!(owner::apply(&mut *rx, expiry, &Value::F64(1.0)).is_err());
    assert!(owner::apply(&mut *rx, expiry, &Value::Bool(true)).is_err());
}

#[test]
fn saved_state_restores_the_tables() {
    for d in descriptors() {
        let h = harness(d);
        let x = h.generate().unwrap();
        let mut rx = h.receiver().unwrap();
        let input =
            InputInfo::new(d.tuning.rate_hz, d.tuning.default_hz, d.tuning.default_hz).recording();
        rx.configure(&input).unwrap();
        let mut out = Output::default();
        rx.feed(&x, &mut out);
        rx.feed(&[], &mut out);
        let state = rx.save_state().expect("a saved state");
        let mut fresh = h.receiver().unwrap();
        fresh.configure(&input).unwrap();
        fresh.restore_state(&state).unwrap();
        fresh.feed(&[], &mut out);
        let table = objects(d);
        let a = rows(Owner::results(&*rx), table).len();
        let b = rows(Owner::results(&*fresh), table).len();
        assert!(a > 0 && a == b, "{}: {a} rows, {b} restored", d.id);
        assert!(fresh
            .restore_state(&StateBlob::new("other", 1, b"{}".to_vec()))
            .is_err());
        let clear = rx.commands().id("clear").unwrap();
        let args = rx.commands().defs()[clear.index()].new_args();
        owner::invoke(&mut *rx, clear, &args).unwrap();
        rx.feed(&[], &mut out);
        assert!(rows(Owner::results(&*rx), table).is_empty(), "{}", d.id);
    }
}

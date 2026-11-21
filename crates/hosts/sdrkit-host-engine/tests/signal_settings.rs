//! The test-signal settings of a stack (`gen_*`) that a session sets reach the test signal of its radio: given at the start
//! of the session and changed while it runs. A session that leaves one at its default keeps the radio's value.

mod common;

use common::{field, wait_for};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Facilities, MemStorage, Random, Value, Values};
use sdrkit_host_engine::{Engine, Input, OwnerRef, SessionId, Setup};
use std::sync::Arc;

/// The time a test waits for the signal.
const T: f64 = 30.0;
/// Over this SNR the session hears the tone of the test signal; under [`NOISE_DB`] it hears noise only.
const TONE_DB: f64 = 40.0;
const NOISE_DB: f64 = 10.0;

/// An engine on the system clock with the general stack.
fn engine() -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&sdrkit_stack_general::DESCRIPTOR],
        families: Vec::new(),
        components: sdrkit_graph::Registry::standard(),
    })
    .unwrap()
}

/// A session of the general stack on its test-signal radio, with `radio` set on the radio and `stack` on the session.
fn session(e: &Engine, radio: &[(&str, Value)], stack: &[(&str, Value)]) -> SessionId {
    let schema = e.radio_schema("synth", "synth:general").unwrap();
    let mut r = Values::defaults(&schema);
    for (k, v) in radio {
        let id = r.schema().id(k).unwrap();
        r.set(id, v).unwrap();
    }
    let radio = e.open_radio("synth", "synth:general", &r).unwrap();
    let mut s = common::defaults(e, e.find("stack.general").unwrap());
    for (k, v) in stack {
        let id = s.schema().id(k).unwrap();
        s.set(id, v).unwrap();
    }
    e.start_stack("general", None, &s, Input::Radio(radio), None)
        .unwrap()
}

fn snr(e: &Engine, s: SessionId) -> Option<f64> {
    match field(e, OwnerRef::Session(s), "snr_db") {
        Some(Value::F64(x)) => Some(x),
        _ => None,
    }
}

/// Waits until the SNR of `s` passes `test`, and returns the last SNR read.
fn wait_snr(e: &Engine, s: SessionId, test: impl Fn(f64) -> bool) -> Result<f64, Option<f64>> {
    wait_for(T, || snr(e, s).filter(|x| test(*x))).ok_or_else(|| snr(e, s))
}

/// Sets `key` of the session `s`.
fn set(e: &Engine, s: SessionId, key: &str, value: Value) {
    e.owner(OwnerRef::Session(s), |o| {
        let id = o.schema().id(key).unwrap();
        sdrkit_core::owner::apply(o, id, &value).unwrap();
    })
    .unwrap();
}

#[test]
fn a_test_signal_setting_given_at_the_start_of_the_session_reaches_the_signal() {
    let e = engine();
    let s = session(&e, &[], &[("gen_noise_only", Value::Bool(true))]);
    let held = e
        .owner(OwnerRef::Session(s), |o| {
            let id = o.schema().id("gen_noise_only").unwrap();
            o.values().and_then(|v| v.value(id).cloned())
        })
        .unwrap();
    assert_eq!(held, Some(Value::Bool(true)), "the session keeps the value");
    wait_snr(&e, s, |x| x < NOISE_DB).expect("noise only, SNR");
    // The signal stays noise: no tone comes back.
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert!(snr(&e, s).is_some_and(|x| x < NOISE_DB), "{:?}", snr(&e, s));
}

#[test]
fn a_test_signal_setting_changed_while_the_session_runs_makes_the_signal_again() {
    let e = engine();
    let s = session(&e, &[], &[]);
    wait_snr(&e, s, |x| x > TONE_DB).expect("the tone, SNR");
    set(&e, s, "gen_noise_only", Value::Bool(true));
    wait_snr(&e, s, |x| x < NOISE_DB).expect("noise only, SNR");
    set(&e, s, "gen_noise_only", Value::Bool(false));
    wait_snr(&e, s, |x| x > TONE_DB).expect("the tone again, SNR");
}

#[test]
fn a_session_at_the_default_keeps_the_value_of_the_radio() {
    let e = engine();
    let s = session(&e, &[("gen_noise_only", Value::Bool(true))], &[]);
    wait_snr(&e, s, |x| x < NOISE_DB).expect("noise only, SNR");
}

//! A radio whose rate is on Automatic runs at the rate that the stack of its session needs. A rate that the host sets
//! stays, and a stack that needs more refuses to start.

mod common;

use common::{defaults, fake_rtl, number, wait_for};
use sdrkit_core::{ConfigError, StackDescriptor, Value, Values};
use sdrkit_host_engine::{Engine, EngineError, Input, OwnerRef, RadioId, SessionId};
use std::sync::Arc;

/// The stacks that need more than the 2 Msps of the test radio without a stack.
const WIDE: [&str; 11] = [
    "aeromacs",
    "atsc",
    "atsc3",
    "cv2x",
    "eodb",
    "gk2a_uhrit",
    "goes_grb",
    "isdbt",
    "rdas",
    "sat_filecast",
    "v2x",
];

/// The wide stack of the tests.
const STACK: &str = "aeromacs";

fn stack(id: &str) -> &'static StackDescriptor {
    sdrkit_host_catalog::all()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap()
}

/// The rate that stack `id` would like with its default settings.
fn wanted(id: &str) -> f64 {
    let d = stack(id);
    d.rate_hz(&Values::defaults(&(d.settings)()))
}

/// Opens the test radio of stack `id`. With `rate_hz`, the host sets that rate; without, the rate is on Automatic.
fn open(e: &Engine, id: &str, rate_hz: Option<f64>) -> Result<RadioId, EngineError> {
    let mut v = defaults(e, e.find("driver.synth").unwrap());
    if let Some(hz) = rate_hz {
        let rate = v.schema().id("rate").unwrap();
        v.set(rate, &Value::Hz(hz)).unwrap();
    }
    e.open_radio("synth", &format!("synth:{id}"), &v)
}

/// Starts stack `id` with its default settings on `radio`.
fn start(e: &Engine, id: &str, radio: RadioId) -> Result<SessionId, EngineError> {
    let values = defaults(e, e.find(&format!("stack.{id}")).unwrap());
    e.start_stack(id, None, &values, Input::Radio(radio), None)
}

/// The rate setting of `radio`.
fn rate(e: &Engine, radio: RadioId) -> f64 {
    e.owner(OwnerRef::Radio(radio), |o| {
        let id = o.schema().id("rate").unwrap();
        match o.values().and_then(|v| v.value(id)) {
            Some(Value::Hz(x) | Value::F64(x)) => *x,
            _ => f64::NAN,
        }
    })
    .unwrap()
}

fn set_rate(e: &Engine, radio: RadioId, hz: f64) {
    e.owner(OwnerRef::Radio(radio), |o| {
        let id = o.schema().id("rate").unwrap();
        sdrkit_core::owner::apply(o, id, &Value::Hz(hz))
    })
    .unwrap()
    .unwrap();
}

/// The refusal of a rate: the least rate of the stack and the rate that the radio has.
fn too_low(e: EngineError) -> (f64, f64) {
    match e {
        EngineError::Config(ConfigError::RateTooLow { need, got }) => (need, got),
        e => panic!("not a refusal of the rate: {e}"),
    }
}

#[test]
fn a_wide_stack_starts_on_its_test_signal_with_the_rate_on_automatic() {
    let e = sdrkit_host_catalog::engine().unwrap();
    assert!(wanted(STACK) > 2e6);
    let radio = open(&e, STACK, None).unwrap();
    assert_eq!(rate(&e, radio), wanted(STACK));
    let id = start(&e, STACK, radio).unwrap();
    assert_eq!(rate(&e, radio), wanted(STACK));
    // The radio delivers that rate, and the session reads samples.
    let given = wait_for(10.0, || {
        Some(number(&e, OwnerRef::Radio(radio), "rate_hz")).filter(|hz| *hz == wanted(STACK))
    });
    assert!(given.is_some(), "the radio runs at the rate of the stack");
    e.stop_session(id).unwrap();
}

#[test]
fn a_rate_of_the_host_that_is_too_low_is_refused_with_both_numbers() {
    let e = sdrkit_host_catalog::engine().unwrap();
    let least = stack(STACK).tuning.min_rate_hz;
    assert!(least > 2e6);
    // The test signal of the stack refuses the rate at the open, or the stack refuses it at the start.
    let refused = match open(&e, STACK, Some(2e6)) {
        Ok(radio) => start(&e, STACK, radio).unwrap_err(),
        Err(e) => e,
    };
    assert_eq!(too_low(refused), (least, 2e6));
}

#[test]
fn a_rate_of_the_host_stays_when_a_stack_starts_and_automatic_takes_the_rate_of_the_stack() {
    let e = sdrkit_host_catalog::engine().unwrap();
    let radio = open(&e, "fm", Some(5e6)).unwrap();
    let id = start(&e, "fm", radio).unwrap();
    assert_ne!(wanted("fm"), 5e6);
    assert_eq!(rate(&e, radio), 5e6);
    // The host gives the rate back: the radio takes the rate of the stack that reads it.
    set_rate(&e, radio, 0.0);
    assert_eq!(rate(&e, radio), wanted("fm"));
    // A rate that the host sets on the running radio stays at the next start.
    set_rate(&e, radio, 3e6);
    e.stop_session(id).unwrap();
    start(&e, "fm", radio).unwrap();
    assert_eq!(rate(&e, radio), 3e6);
}

#[test]
fn a_wide_test_radio_takes_automatic_back_from_the_host_and_from_saved_settings() {
    let e = sdrkit_host_catalog::engine().unwrap();
    let radio = open(&e, STACK, Some(8e6)).unwrap();
    start(&e, STACK, radio).unwrap();
    assert_eq!(rate(&e, radio), 8e6);
    set_rate(&e, radio, 0.0);
    assert_eq!(rate(&e, radio), wanted(STACK));
    // The saved settings hold Automatic, and bring it back over a rate of the host.
    let saved = e.export_settings().unwrap();
    set_rate(&e, radio, 8e6);
    e.import_settings(&saved).unwrap();
    assert_eq!(rate(&e, radio), wanted(STACK));
}

#[test]
fn a_rate_of_the_host_below_the_least_rate_of_the_stack_refuses_the_start() {
    let e = sdrkit_host_catalog::engine().unwrap();
    let least = stack("fm").tuning.min_rate_hz;
    let low = (least / 2.0).floor();
    // The radio plays silence, so only the engine checks the rate.
    let radio = open(&e, "", Some(low)).unwrap();
    assert_eq!(
        too_low(start(&e, "fm", radio).unwrap_err()),
        (least, low)
    );
    assert_eq!(rate(&e, radio), low);
}

#[test]
fn the_next_stack_on_the_radio_gets_its_own_rate() {
    let e = sdrkit_host_catalog::engine().unwrap();
    let radio = open(&e, STACK, None).unwrap();
    let wide = start(&e, STACK, radio).unwrap();
    assert_eq!(rate(&e, radio), wanted(STACK));
    e.stop_session(wide).unwrap();
    // A stack with a smaller need does not keep the high rate, and the wide stack gets its rate back.
    assert!(wanted("fm") < wanted(STACK));
    let narrow = start(&e, "fm", radio).unwrap();
    assert_eq!(rate(&e, radio), wanted("fm"));
    e.stop_session(narrow).unwrap();
    start(&e, STACK, radio).unwrap();
    assert_eq!(rate(&e, radio), wanted(STACK));
}

#[test]
fn a_radio_without_a_rate_for_the_stack_refuses_with_the_need_and_its_highest_rate() {
    let server = fake_rtl();
    let e = sdrkit_host_catalog::engine_on(Arc::new(server.net().clone())).unwrap();
    let v = defaults(&e, e.find("driver.rtl_tcp").unwrap());
    let radio = e.open_radio("rtl_tcp", &server.addr(), &v).unwrap();
    let highest = e
        .owner(OwnerRef::Radio(radio), |o| {
            let id = o.schema().id("rate").unwrap();
            o.schema().setting(id).and_then(|s| s.range).unwrap().max
        })
        .unwrap();
    let least = stack(STACK).tuning.min_rate_hz;
    assert!(highest < least);
    assert_eq!(
        too_low(start(&e, STACK, radio).unwrap_err()),
        (least, highest)
    );
    // A stack that the radio has a rate for starts, at the least rate of the radio that is enough.
    start(&e, "fm", radio).unwrap();
    assert!(rate(&e, radio) >= wanted("fm").min(highest));
}

/// Every wide stack opens its test radio with the default settings of the driver and starts on it.
#[test]
#[ignore = "runs every wide stack; about a minute"]
fn every_wide_stack_starts_on_its_test_signal_with_default_settings() {
    let e = sdrkit_host_catalog::engine().unwrap();
    let mut failed = Vec::new();
    for id in WIDE {
        let r = open(&e, id, None).and_then(|radio| {
            let s = start(&e, id, radio);
            let at = rate(&e, radio);
            let _ = s.as_ref().map(|s| e.stop_session(*s));
            let _ = e.close_radio(radio);
            s.map(|_| at)
        });
        match r {
            Ok(at) => println!("{id}: starts at {at} (wanted {})", wanted(id)),
            Err(err) => failed.push(format!("{id}: {err}")),
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

//! The sample rate that a stack states from its settings reaches the radio: DVB-T in a 7 MHz channel runs the radio at
//! 8 Msps, and a change of the width changes the rate of the radio.

mod common;

use common::*;
use sdrkit_core::{Choice, Facilities, LockState, MemStorage, Random, Value};
use sdrkit_host_engine::{Engine, Input, OwnerRef, Setup};
use sdrkit_stack_dvbt::params::{Bandwidth, CodeRate, Constellation, Guard, Mode};
use sdrkit_stack_dvbt::settings::BandwidthChoice;
use std::sync::Arc;

fn set(e: &Engine, r: OwnerRef, key: &str, v: Value) {
    e.owner(r, |o| {
        let id = o.schema().id(key).unwrap();
        sdrkit_core::owner::apply(o, id, &v)
    })
    .unwrap()
    .unwrap();
}

/// The value of the setting `key` of owner `r` as a number.
fn setting(e: &Engine, r: OwnerRef, key: &str) -> f64 {
    e.owner(r, |o| {
        let id = o.schema().id(key).unwrap();
        match o.values().and_then(|v| v.value(id)) {
            Some(Value::F64(x) | Value::Hz(x)) => *x,
            _ => f64::NAN,
        }
    })
    .unwrap()
}

/// An engine with the DVB-T stack on the system clock.
fn dvbt_engine() -> Engine {
    dvbt_engine_on(sdrkit_core::net::FakeTransport::new())
}

/// An engine with the DVB-T stack on the system clock and the network `net`.
fn dvbt_engine_on(net: sdrkit_core::net::FakeTransport) -> Engine {
    let mut components = sdrkit_graph::Registry::standard();
    sdrkit_stack_dvbt::register(&mut components);
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(net),
        stacks: vec![&sdrkit_stack_dvbt::DVBT, &sdrkit_stack_fm::DESCRIPTOR],
        families: Vec::new(),
        components,
    })
    .unwrap()
}

#[test]
fn dvbt_in_a_7_mhz_channel_runs_the_radio_at_8_msps() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    for (key, x) in [
        ("gen_mode", Mode::K2.index()),
        ("gen_guard", Guard::G8.index()),
        ("gen_constellation", Constellation::Qam16.index()),
        ("gen_rate_hp", CodeRate::R23.index()),
        ("gen_bandwidth", Bandwidth::B7.index()),
    ] {
        set(&e, OwnerRef::Radio(radio), key, Value::Choice(x));
    }
    let stack = e.find("stack.dvbt").unwrap();
    let mut values = defaults(&e, stack);
    let width = values.schema().id("bandwidth").unwrap();
    values
        .set(width, &Value::Choice(BandwidthChoice::B7.index()))
        .unwrap();
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    let r = OwnerRef::Radio(radio);
    assert_eq!(setting(&e, r, "rate"), 8e6, "the rate of a 7 MHz channel");
    let s = OwnerRef::Session(id);
    let locked = wait_for(60.0, || {
        e.owner(s, |o| o.results().status().lock_state)
            .ok()
            .filter(|l| *l == LockState::Locked)
    });
    assert!(locked.is_some(), "DVB-T locks at 8 Msps");
    let stack = OwnerRef::Stack(id);
    // The input stage of the stack has the rate of the width: it resamples only while it corrects the sample clock.
    assert_eq!(number(&e, stack, "input_rate"), 8e6);
    assert_eq!(number(&e, stack, "natural_rate"), 8e6);
    let (resampling, ppm) = e
        .owner(stack, |o| {
            let r = o.results();
            let get = |key| {
                let id = r.decl().field_id(key)?;
                r.fields().value(id).cloned()
            };
            (get("resampling"), get("clock_ppm"))
        })
        .unwrap();
    let corrects = ppm != Some(Value::F64(0.0));
    assert_eq!(resampling, Some(Value::Bool(corrects)));
    // A change of the width changes the rate of the radio through the engine.
    for (choice, rate) in [
        (BandwidthChoice::B6, 48e6 / 7.0),
        (BandwidthChoice::B8, 64e6 / 7.0),
        (BandwidthChoice::Auto, 64e6 / 7.0),
    ] {
        set(&e, s, "bandwidth", Value::Choice(choice.index()));
        assert_eq!(setting(&e, r, "rate"), rate, "{choice:?}");
    }
    // The same through the owner of the stack.
    set(
        &e,
        stack,
        "bandwidth",
        Value::Choice(BandwidthChoice::B6.index()),
    );
    assert_eq!(setting(&e, r, "rate"), 48e6 / 7.0);
    // Back at the width of the signal the stack is fed 8 Msps again and locks.
    set(
        &e,
        s,
        "bandwidth",
        Value::Choice(BandwidthChoice::B7.index()),
    );
    assert_eq!(setting(&e, r, "rate"), 8e6);
    let again = wait_for(60.0, || {
        let locked = e.owner(s, |o| o.results().status().lock_state).ok()? == LockState::Locked;
        (locked && number(&e, stack, "input_rate") == 8e6).then_some(())
    });
    assert!(
        again.is_some(),
        "locked at 8 Msps after the changes of the width"
    );
}

/// The test signal takes the width from the stack settings of the session that its radio serves: nothing is set on the
/// radio.
#[test]
fn the_test_signal_has_the_width_that_the_session_sets() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    let mut values = defaults(&e, e.find("stack.dvbt").unwrap());
    let width = values.schema().id("bandwidth").unwrap();
    values
        .set(width, &Value::Choice(BandwidthChoice::B7.index()))
        .unwrap();
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    assert_eq!(setting(&e, OwnerRef::Radio(radio), "rate"), 8e6);
    let s = OwnerRef::Session(id);
    let locked = wait_for(60.0, || {
        e.owner(s, |o| o.results().status().lock_state)
            .ok()
            .filter(|l| *l == LockState::Locked)
    });
    assert!(locked.is_some(), "DVB-T locks on a 7 MHz test signal");
    assert_eq!(number(&e, OwnerRef::Stack(id), "natural_rate"), 8e6);
}

#[test]
fn a_rate_that_the_host_sets_by_hand_stays_until_the_host_gives_it_back() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    let values = defaults(&e, e.find("stack.dvbt").unwrap());
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    let (r, s) = (OwnerRef::Radio(radio), OwnerRef::Session(id));
    set(&e, r, "rate", Value::Hz(10e6));
    // Reads of the session and a setting that does not give the rate leave the radio as it is.
    let _ = field(&e, s, "problem");
    let _ = field(&e, OwnerRef::Stack(id), "input_rate");
    set(&e, s, "viterbi", Value::Choice(0));
    assert_eq!(setting(&e, r, "rate"), 10e6);
    set(
        &e,
        s,
        "bandwidth",
        Value::Choice(BandwidthChoice::B7.index()),
    );
    assert_eq!(setting(&e, r, "rate"), 10e6);
    // Automatic gives the rate back to the stack.
    set(&e, r, "rate", Value::Hz(0.0));
    assert_eq!(setting(&e, r, "rate"), 8e6);
}

#[test]
fn a_radio_that_two_sessions_read_keeps_its_rate() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    let values = defaults(&e, e.find("stack.dvbt").unwrap());
    let first = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    let r = OwnerRef::Radio(radio);
    assert_eq!(setting(&e, r, "rate"), 64e6 / 7.0);
    e.start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    let narrow = Value::Choice(BandwidthChoice::B7.index());
    set(&e, OwnerRef::Session(first), "bandwidth", narrow);
    assert_eq!(setting(&e, r, "rate"), 64e6 / 7.0);
}

/// The sample rates that the board of `srv` was asked for, in order.
fn asked(srv: &sdrkit_driver_core::iiod::fake::FakeIiod) -> Vec<f64> {
    srv.board
        .log()
        .iter()
        .filter_map(|l| l.strip_prefix("iio:device1 INPUT voltage0 sampling_frequency="))
        .map(|hz| hz.parse().unwrap())
        .collect()
}

/// The DVB-T settings with the width `width` and a list of channels, as a host makes them from the data of a region.
fn regional(e: &Engine, width: BandwidthChoice) -> sdrkit_core::Values {
    let mut values = defaults(e, e.find("stack.dvbt").unwrap());
    for (key, v) in [
        ("bandwidth", Value::Choice(width.index())),
        ("channels", Value::Text("177500000;184500000".into())),
    ] {
        let id = values.schema().id(key).unwrap();
        values.set(id, &v).unwrap();
    }
    values
}

#[test]
fn a_board_is_asked_for_the_rate_of_the_width_before_and_after_the_start() {
    use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
    let net = sdrkit_core::net::FakeTransport::new();
    let srv = FakeIiod::start_on(net.clone(), FakeOpts::default());
    let e = dvbt_engine_on(net);
    let kind = e.find("driver.iiod").unwrap();
    let radio = e
        .open_radio("iiod", &srv.addr(), &defaults(&e, kind))
        .unwrap();
    let opened = asked(&srv).len();
    // The width is set before the start: the board gets one request, for 8 Msps, before the session reads it.
    let values = regional(&e, BandwidthChoice::B7);
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), Some(177.5e6))
        .unwrap();
    assert_eq!(asked(&srv)[opened..], [8e6]);
    let stack = OwnerRef::Stack(id);
    let fed = wait_for(20.0, || {
        (number(&e, stack, "input_rate") == 8e6).then_some(())
    });
    assert!(fed.is_some(), "the stack is fed 8 Msps");
    // The width changes after the start.
    let s = OwnerRef::Session(id);
    set(
        &e,
        s,
        "bandwidth",
        Value::Choice(BandwidthChoice::B6.index()),
    );
    set(
        &e,
        s,
        "bandwidth",
        Value::Choice(BandwidthChoice::B7.index()),
    );
    assert_eq!(asked(&srv)[opened..], [8e6, 6_857_143.0, 8e6]);
}

/// Runs command `cmd` of the memory service and returns the row of its reply.
fn memory(e: &Engine, cmd: &str, args: &[(&str, Value)]) -> Option<String> {
    let reply = e
        .owner(e.find("service.memory").unwrap(), |o| {
            let id = o.commands().id(cmd)?;
            let mut rec = o.commands().def(id).unwrap().new_args();
            for (k, v) in args {
                let f = rec.decl().id(k).unwrap();
                rec.set_value(f, v).unwrap();
            }
            sdrkit_core::owner::invoke(o, id, &rec)
        })
        .unwrap()
        .unwrap()?;
    match reply.value(reply.decl().id("row")?) {
        Some(Value::Text(k)) => Some(k.to_string()),
        _ => None,
    }
}

/// A memory of DVB-T at 177.5 MHz. With `width`, it holds that width as a setting of the stack.
fn tv_memory(e: &Engine, width: Option<BandwidthChoice>) -> String {
    let row = memory(
        e,
        "add",
        &[
            ("name", Value::Text("tv".into())),
            ("freq_hz", Value::Hz(177.5e6)),
            ("stack", Value::Text("dvbt".into())),
        ],
    )
    .unwrap();
    if let Some(w) = width {
        memory(
            e,
            "item.set",
            &[
                ("row", Value::Text(row.clone())),
                ("part", Value::Choice(0)),
                ("owner", Value::Text("stack.dvbt".into())),
                ("key", Value::Text("bandwidth".into())),
                ("value", Value::Choice(w.index())),
            ],
        );
    }
    row
}

fn recall(e: &Engine, row: &str, id: sdrkit_host_engine::SessionId) {
    let session = Value::Text(format!("session.s{}", id.to_u64()));
    memory(
        e,
        "recall",
        &[("row", Value::Text(row.into())), ("session", session)],
    );
}

#[test]
fn a_tune_request_with_the_width_sets_the_rate_of_the_radio() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    let values = defaults(&e, e.find("stack.dvbt").unwrap());
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    let r = OwnerRef::Radio(radio);
    assert_eq!(setting(&e, r, "rate"), 64e6 / 7.0);
    recall(&e, &tv_memory(&e, Some(BandwidthChoice::B7)), id);
    let at = wait_for(10.0, || (setting(&e, r, "rate") == 8e6).then_some(()));
    assert!(at.is_some(), "the radio runs at {}", setting(&e, r, "rate"));
}

#[test]
fn a_tune_request_that_puts_the_stack_in_a_session_sets_its_rate() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    for (width, rate) in [(None, 64e6 / 7.0), (Some(BandwidthChoice::B7), 8e6)] {
        let radio = e
            .open_radio("synth", "synth:fm", &defaults(&e, synth))
            .unwrap();
        let values = defaults(&e, e.find("stack.fm").unwrap());
        let id = e
            .start_stack("fm", None, &values, Input::Radio(radio), None)
            .unwrap();
        let r = OwnerRef::Radio(radio);
        assert!(setting(&e, r, "rate") < 5e6);
        recall(&e, &tv_memory(&e, width), id);
        let at = wait_for(10.0, || (setting(&e, r, "rate") == rate).then_some(()));
        assert!(
            at.is_some(),
            "{width:?}: the radio runs at {}",
            setting(&e, r, "rate")
        );
        e.stop_session(id).unwrap();
        e.close_radio(radio).unwrap();
    }
}

/// A memory of DVB-T at 177.5 MHz that names only the width of the channel.
fn width_memory(e: &Engine, width_hz: f64) -> String {
    memory(
        e,
        "add",
        &[
            ("name", Value::Text("wide".into())),
            ("freq_hz", Value::Hz(177.5e6)),
            ("stack", Value::Text("dvbt".into())),
            ("bandwidth_hz", Value::Hz(width_hz)),
        ],
    )
    .unwrap()
}

#[test]
fn a_tune_request_that_names_the_width_sets_the_bandwidth_of_the_stack() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    let values = defaults(&e, e.find("stack.dvbt").unwrap());
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), None)
        .unwrap();
    let (r, s) = (OwnerRef::Radio(radio), OwnerRef::Session(id));
    for (width, choice, rate) in [
        (7e6, BandwidthChoice::B7, 8e6),
        (6e6, BandwidthChoice::B6, 48e6 / 7.0),
    ] {
        recall(&e, &width_memory(&e, width), id);
        let at = wait_for(10.0, || (setting(&e, r, "rate") == rate).then_some(()));
        assert!(
            at.is_some(),
            "{width}: the radio runs at {}",
            setting(&e, r, "rate")
        );
        assert_eq!(
            field_of_setting(&e, s, "bandwidth"),
            Some(Value::Choice(choice.index()))
        );
    }
}

/// The value of the setting `key` of owner `r`.
fn field_of_setting(e: &Engine, r: OwnerRef, key: &str) -> Option<Value> {
    e.owner(r, |o| {
        let id = o.schema().id(key).unwrap();
        o.values().and_then(|v| v.value(id).cloned())
    })
    .unwrap()
}

#[test]
fn dvbt_on_the_automatic_width_asks_for_the_rate_of_the_width_it_locked_on() {
    let e = dvbt_engine();
    let synth = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:dvbt", &defaults(&e, synth))
        .unwrap();
    let r = OwnerRef::Radio(radio);
    set(&e, r, "gen_bandwidth", Value::Choice(Bandwidth::B7.index()));
    let values = defaults(&e, e.find("stack.dvbt").unwrap());
    let id = e
        .start_stack("dvbt", None, &values, Input::Radio(radio), Some(177.5e6))
        .unwrap();
    // The width is not known at the start: the radio runs at the rate of the widest channel.
    assert_eq!(setting(&e, r, "rate"), 64e6 / 7.0);
    let (s, stack) = (OwnerRef::Session(id), OwnerRef::Stack(id));
    let locked = |rate: f64| {
        wait_for(60.0, || {
            let l = e.owner(s, |o| o.results().status().lock_state).ok()?;
            (l == LockState::Locked && number(&e, stack, "input_rate") == rate).then_some(())
        })
    };
    assert!(
        locked(8e6).is_some(),
        "locked at {} with the radio at {}",
        number(&e, stack, "input_rate"),
        setting(&e, r, "rate")
    );
    assert_eq!(setting(&e, r, "rate"), 8e6);
    // The width stays on Auto, and a read of the session leaves the rate.
    assert_eq!(
        field_of_setting(&e, s, "bandwidth"),
        Some(Value::Choice(BandwidthChoice::Auto.index()))
    );
    assert_eq!(setting(&e, r, "rate"), 8e6);
    // Another frequency may have another width: the radio goes back to the rate of the widest channel.
    set(&e, s, "tuned_hz", Value::Hz(177.501e6));
    assert_eq!(setting(&e, r, "rate"), 64e6 / 7.0);
    // The stack locks there again and asks again.
    let back = wait_for(60.0, || {
        (number(&e, stack, "input_rate") == 64e6 / 7.0).then_some(())
    });
    assert!(
        back.is_some(),
        "the stack did not see the rate of the widest channel"
    );
    assert!(
        locked(8e6).is_some(),
        "after the change of frequency: lock {:?} at {} with the radio at {}",
        e.owner(s, |o| o.results().status().lock_state).ok(),
        number(&e, stack, "input_rate"),
        setting(&e, r, "rate")
    );
}

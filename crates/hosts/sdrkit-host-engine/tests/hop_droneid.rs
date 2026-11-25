//! The `dji_droneid` stack set to step through the DroneID frequencies, on its test signal with the bursts on the DroneID
//! frequency of the radio centre: the engine hops the radio, and the stack decodes bursts on one DroneID frequency, then
//! on the next.

use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Event, Value, Values};
use sdrkit_host_engine::{Input, OwnerRef};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Sets `key` of `v` to `value`.
fn put(v: &mut Values, key: &str, value: Value) {
    v.set(v.schema().id(key).unwrap(), &value).unwrap();
}

/// The frequencies of `freqs` in turn, from the first DroneID frequency of the band on.
fn runs(freqs: &[f64]) -> Vec<f64> {
    let mut r: Vec<f64> = freqs
        .iter()
        .copied()
        .skip_while(|f| *f != 2399.5e6)
        .collect();
    r.dedup();
    r
}

#[test]
fn the_droneid_stack_set_to_step_decodes_bursts_on_two_droneid_frequencies_in_turn() {
    let e = sdrkit_host_catalog::engine_on(Arc::new(FakeTransport::new())).unwrap();
    let mut radio = Values::defaults(&e.radio_schema("synth", "synth:dji_droneid").unwrap());
    put(&mut radio, "benchmark", Value::Bool(true));
    let r = e.open_radio("synth", "synth:dji_droneid", &radio).unwrap();
    let stack = e.find("stack.dji_droneid").unwrap();
    let mut v = e.owner(stack, |o| Values::defaults(o.schema())).unwrap();
    put(&mut v, "droneid_tuning", Value::Choice(1));
    put(&mut v, "droneid_dwell_s", Value::Duration(10.0));
    put(&mut v, "gen_droneid_frequency", Value::Bool(true));
    put(&mut v, "gen_scenario", Value::Choice(1));
    // A burst each 5 periods: one a second.
    put(&mut v, "gen_period_s", Value::F64(0.2));
    let s = e
        .start_stack("dji_droneid", None, &v, Input::Radio(r), Some(2440e6))
        .unwrap();
    // The frequency of each decoded packet, in order, until two DroneID frequencies have come in turn.
    let mut freqs: Vec<f64> = Vec::new();
    // Each packet's frequency with its time, seconds of signal.
    let mut times: Vec<(f64, f64)> = Vec::new();
    let mut events: Vec<Event> = Vec::new();
    let began = Instant::now();
    while began.elapsed() < Duration::from_secs(600) {
        std::thread::sleep(Duration::from_millis(200));
        e.take_events(OwnerRef::Session(s), &mut events).unwrap();
        for ev in events.drain(..).filter(|ev| ev.kind() == "dji.droneid") {
            let f = (ev.freq_hz / 1e5).round() * 1e5;
            freqs.push(f);
            times.push((f, ev.time));
        }
        if runs(&freqs).len() >= 2 {
            break;
        }
    }
    // The hop starts at the first DroneID frequency of the band and steps to the next one after the dwell. Before the
    // hop the test signal had its bursts below the centre where the session started.
    let hop = runs(&freqs);
    assert!(hop.len() >= 2, "{freqs:?}");
    assert_eq!(hop[..2], [2399.5e6, 2414.5e6], "{freqs:?}");
    // A burst a second for the dwell of 10 s of signal on the first frequency.
    let first: Vec<f64> = times
        .iter()
        .filter(|(f, _)| *f == 2399.5e6)
        .map(|(_, t)| *t)
        .collect();
    let span = first.iter().fold(f64::MIN, |a, t| a.max(*t))
        - first.iter().fold(f64::MAX, |a, t| a.min(*t));
    assert!(
        first.len() >= 8 && (8.0..10.5).contains(&span),
        "{span} s: {times:?}"
    );
    let o = OwnerRef::Session(s);
    let dropped = e
        .owner(o, |o| {
            let res = o.results();
            res.decl()
                .field_id("session.dropped")
                .and_then(|id| res.fields().value(id).cloned())
        })
        .unwrap();
    assert_eq!(dropped, Some(Value::I64(0)));
    e.stop_session(s).unwrap();
    e.close_session(s).unwrap();
    e.close_radio(r).unwrap();
}

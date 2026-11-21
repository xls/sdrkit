//! Transmit through a radio: a session's `tx.sink` reaches the transmitter of its radio only while the engine allows it.

mod common;

use common::*;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Event, Value};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts as IiodOpts};
use sdrkit_graph::{GraphPlan, PortRef, Registry};
use sdrkit_host_engine::{
    Engine, EngineError, Input, OwnerRef, SessionId, BURST_ATTEMPTS, BURST_REFUSED, BURST_SENT,
    TRANSMIT_ALLOWED,
};

const T: f64 = 10.0;

/// An engine and an open radio on a fake iiod server, tuned to 915 MHz.
fn iiod_radio() -> (FakeIiod, Engine, sdrkit_host_engine::RadioId) {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(net.clone(), IiodOpts::default());
    let e = engine_on(&net);
    let kind = e.find("driver.iiod").unwrap();
    let mut v = defaults(&e, kind);
    for (key, value) in [("centre_hz", Value::Hz(915e6)), ("rate", Value::Hz(1e6))] {
        let id = v.schema().id(key).unwrap();
        v.set(id, &value).unwrap();
    }
    let r = e.open_radio("iiod", &srv.addr(), &v).unwrap();
    (srv, e, r)
}

/// A session on radio `r` whose graph sends what it receives to the transmit sink.
fn echo(e: &Engine, r: sdrkit_host_engine::RadioId) -> SessionId {
    let reg = Registry::standard();
    let mut plan = GraphPlan::new();
    let tx = plan.add(&reg, "tx", "tx.sink").unwrap();
    plan.input(PortRef::new(tx, 0)).unwrap();
    e.start_graph(plan, Input::Radio(r)).unwrap()
}

fn allow(e: &Engine, on: bool) {
    e.owner(OwnerRef::Engine, |o| {
        let id = o.schema().id(TRANSMIT_ALLOWED.key()).unwrap();
        o.apply(id, &Value::Bool(on))
    })
    .unwrap()
    .unwrap();
}

/// The `session.transmit` events of session `s` that wait.
fn bursts(e: &Engine, s: SessionId) -> Vec<Event> {
    let mut out = Vec::new();
    e.take_events(OwnerRef::Session(s), &mut out).unwrap();
    out.retain(|ev| ev.is(&sdrkit_host_engine::TRANSMIT));
    out
}

#[test]
fn an_allowed_burst_reaches_the_transmitter_of_the_radio() {
    let (srv, e, r) = iiod_radio();
    // The radio's transmit settings are the radio owner's settings.
    let att = e
        .owner(OwnerRef::Radio(r), |o| {
            let id = o
                .schema()
                .id(sdrkit_driver_core::TX_POWER.key())
                .unwrap();
            o.apply(id, &Value::F64(-40.0))
        })
        .unwrap();
    assert_eq!(att, Ok(()));
    allow(&e, true);
    let s = echo(&e, r);
    let got = wait_for(T, || {
        (!srv.board.tx_bytes.lock().unwrap().is_empty()).then_some(())
    });
    assert!(got.is_some(), "the fake server got no transmit samples");
    let mut buf = vec![sdrkit_core::Complex32::default(); 16];
    assert!(matches!(e.transmit(s, &mut buf), Err(EngineError::ToRadio)));
    e.stop_session(s).unwrap();
    let ended = wait_for(T, || {
        (number(&e, OwnerRef::Session(s), "session.tx_sent") >= 1.0).then_some(())
    });
    assert!(ended.is_some(), "no burst counted as sent");
    assert_eq!(number(&e, OwnerRef::Session(s), "session.tx_failed"), 0.0);
    let ev = wait_for(T, || bursts(&e, s).pop()).expect("a burst event");
    assert_eq!(ev.fields.get(&BURST_SENT), Some(true));
    assert_eq!(ev.fields.get(&BURST_ATTEMPTS), Some(1));
    let log = srv.board.tx_log().join("\n");
    assert!(log.contains("hardwaregain"), "{log}");
    // The transmit frequency and rate of the radio's settings are those of the burst. Before a burst the rate is 0.
    let shown = e
        .owner(OwnerRef::Radio(r), |o| {
            let v = o.values().unwrap();
            (
                v.get(&sdrkit_driver_core::TX_FREQUENCY),
                v.get(&sdrkit_driver_core::TX_RATE),
            )
        })
        .unwrap();
    assert_eq!(shown.0, ev.freq_hz);
    assert!(shown.1 > 0.0, "{shown:?}");
}

#[test]
fn a_burst_is_refused_while_the_engine_does_not_allow_transmit() {
    let (srv, e, r) = iiod_radio();
    let s = echo(&e, r);
    let failed = wait_for(T, || {
        (number(&e, OwnerRef::Session(s), "session.tx_failed") >= 1.0).then_some(())
    });
    assert!(failed.is_some(), "no refusal counted");
    let summary = e
        .owner(OwnerRef::Session(s), |o| {
            o.results().status().summary.clone()
        })
        .unwrap();
    assert!(summary.contains("transmit_allowed"), "{summary}");
    e.stop_session(s).unwrap();
    let ev = wait_for(T, || bursts(&e, s).pop()).expect("a burst event");
    assert_eq!(ev.fields.get(&BURST_SENT), Some(false));
    assert_eq!(ev.fields.get(&BURST_ATTEMPTS), Some(1));
    assert!(ev
        .fields
        .get(&BURST_REFUSED)
        .is_some_and(|t| t.contains("transmit_allowed")));
    assert!(srv.board.tx_bytes.lock().unwrap().is_empty());
}

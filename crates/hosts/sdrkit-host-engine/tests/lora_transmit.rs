//! A stack transmits through a radio: a Meshtastic text sent by a `meshtastic` session reaches the transmitter of its radio
//! in whole bursts, also when a burst is longer than the session's transmit stream. While the transmit switch of the
//! engine is off, a command that transmits is refused and nothing is sent.

mod common;

use common::*;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Choice, Clock, CommandError, Facilities, MemStorage, Random, Value, Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts as IiodOpts};
use sdrkit_host_engine::{
    Engine, Input, OwnerRef, Setup, BURST_SAMPLES, BURST_SENT, TRANSMIT_ALLOWED,
};
use sdrkit_stack_lora::mesh::types::DeliveryState;
use sdrkit_stack_lora::settings::{DUTY_LIMIT, MT_PRESET};
use std::sync::Arc;

const T: f64 = 20.0;

/// An engine with the LoRa stacks on `net`.
fn lora_engine(net: &FakeTransport) -> Engine {
    let mut components = sdrkit_graph::Registry::standard();
    sdrkit_stack_lora::register(&mut components);
    // The clock runs: a burst with a stated start goes to the radio when its time comes.
    let t0 = std::time::Instant::now();
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::new(move || 1.7e9 + t0.elapsed().as_secs_f64()),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(net.clone()),
        stacks: sdrkit_stack_lora::descriptors(),
        families: vec![sdrkit_stack_lora::typed::family()],
        components,
    })
    .unwrap()
}

/// Sets the transmit switch of the engine.
fn switch(e: &Engine, on: bool) {
    e.owner(OwnerRef::Engine, |o| {
        let id = o.schema().id(TRANSMIT_ALLOWED.key()).unwrap();
        o.apply(id, &Value::Bool(on))
    })
    .unwrap()
    .unwrap();
}

/// Sets `key` of `v` to `value`.
fn put(v: &mut Values, key: &str, value: &Value) {
    let id = v.schema().id(key).unwrap();
    v.set(id, value).unwrap();
}

/// The cell in column `col` of each row of table `table` of owner `r`.
fn column(e: &Engine, r: OwnerRef, table: &str, col: &str) -> Vec<Option<Value>> {
    e.owner(r, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table).unwrap()).unwrap();
        let c = t.columns().id(col).unwrap();
        t.rows_newest_first()
            .map(|(_, row)| row.value(c).cloned())
            .collect()
    })
    .unwrap()
}

/// The cell in column `col` of row `row` of table `table` of owner `r`.
fn cell(e: &Engine, r: OwnerRef, table: &str, row: &str, col: &str) -> Option<Value> {
    e.owner(r, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table)?)?;
        t.row(row)?.value(t.columns().id(col)?).cloned()
    })
    .unwrap()
}

/// Runs `mt.send` with the text `text` on session `s`, and returns the packet id of the reply.
fn mt_send(e: &Engine, s: OwnerRef, text: &str) -> Result<i64, CommandError> {
    e.owner(s, |o| {
        let id = o.commands().id("mt.send")?;
        let def = o.commands().def(id).unwrap().clone();
        let mut args = def.new_args();
        let t = args.decl().id("text").unwrap();
        args.set_value(t, &Value::Text(text.into())).unwrap();
        let reply = sdrkit_core::owner::invoke(o, id, &args)?.expect("a reply");
        let p = reply.decl().id("packet_id").unwrap();
        match reply.value(p) {
            Some(Value::I64(n)) => Ok(*n),
            v => panic!("packet id {v:?}"),
        }
    })
    .unwrap()
}

/// Sends a Meshtastic text with preset `preset` through a `meshtastic` session on the fake iiod server, and checks that
/// every burst goes on air whole: the samples of the burst events add up to the samples that the modulator wrote, and the
/// radio received them. Returns the samples of the first burst.
fn send_text(preset: &str, wait_s: f64) -> usize {
    let net = FakeTransport::new();
    // The fake radio plays the transmit samples at the sample rate, as a converter does.
    let srv = FakeIiod::start_on(
        net.clone(),
        IiodOpts {
            tx_plays: true,
            ..IiodOpts::default()
        },
    );
    let e = lora_engine(&net);
    let kind = e.find("driver.iiod").unwrap();
    let r = e
        .open_radio("iiod", &srv.addr(), &defaults(&e, kind))
        .unwrap();
    switch(&e, true);
    // A power above the mute value, so that the fake radio sees when the output is on.
    e.owner(OwnerRef::Radio(r), |o| {
        let id = o.schema().id("tx_power").unwrap();
        sdrkit_core::owner::apply(o, id, &Value::F64(-10.0))
    })
    .unwrap()
    .unwrap();

    let d = &sdrkit_stack_lora::MESHTASTIC;
    let mut values = Values::defaults(&(d.settings)());
    put(&mut values, DUTY_LIMIT.key(), &Value::Bool(false));
    let p = (MT_PRESET.choices)()
        .iter()
        .position(|c| c.key == preset)
        .unwrap();
    put(
        &mut values,
        MT_PRESET.key,
        &Value::Choice(u16::try_from(p).unwrap()),
    );
    let s = e
        .start_stack(d.id, Some("meshtastic"), &values, Input::Radio(r), None)
        .unwrap();
    // The client sends on the channel of the plan once the plan is made.
    let planned = wait_for(T, || {
        matches!(field(&e, OwnerRef::Session(s), "searching"), Some(Value::Text(t)) if t.contains(preset))
            .then_some(())
    });
    assert!(planned.is_some(), "no plan");
    let session = OwnerRef::Session(s);
    let packet = mt_send(&e, session, "hello radio").unwrap();
    assert!(packet > 0);

    // The client sends its NodeInfo, then the text and its retransmissions.
    let mut bursts = Vec::new();
    let samples_of = |b: &sdrkit_core::Event| {
        usize::try_from(b.fields.get(&BURST_SAMPLES).unwrap_or(0)).unwrap()
    };
    // Done when every burst that the modulator ended is on air, and their samples are all the modulator wrote.
    let done = wait_for(wait_s, || {
        let written = number(&e, session, "modulate.tx_bursts");
        let samples = number(&e, session, "modulate.tx_samples");
        e.take_events(session, &mut bursts).unwrap();
        bursts.retain(|ev| ev.is(&sdrkit_host_engine::TRANSMIT));
        let total: usize = bursts.iter().map(samples_of).sum();
        (bursts.len() >= 2 && written == bursts.len() as f64 && samples == total as f64)
            .then_some(total)
    });
    assert!(
        done.is_some(),
        "{} bursts sent, {} written ({} samples, {} dropped): failed {} lost {} {:?}",
        bursts.len(),
        number(&e, session, "modulate.tx_bursts"),
        number(&e, session, "modulate.tx_samples"),
        number(&e, session, "modulate.tx_dropped"),
        number(&e, session, "session.tx_failed"),
        number(&e, session, "session.transmit_lost"),
        field(&e, session, "session.problem")
    );
    assert_eq!(number(&e, session, "session.tx_failed"), 0.0);
    assert_eq!(number(&e, session, "session.transmit_lost"), 0.0);
    // The receive side ran while the bursts went out: the session lost no sample.
    assert_eq!(number(&e, session, "session.dropped"), 0.0);
    for b in &bursts {
        assert_eq!(b.fields.get(&BURST_SENT), Some(true));
    }
    // The row of the text has the packet id of the reply, and its state says that it went on air. No node answers here.
    let ids = column(&e, session, "messages", "packet_id");
    let states = column(&e, session, "messages", "state");
    let row = ids.iter().position(|v| *v == Some(Value::I64(packet)));
    assert!(row.is_some(), "{ids:?}");
    let sent = Value::Choice(DeliveryState::Sent.index());
    assert_eq!(states[row.unwrap()].as_ref(), Some(&sent));
    // The engine has a clock: the row has the time of the text.
    match &column(&e, session, "messages", "utc")[row.unwrap()] {
        Some(Value::Time(t)) => assert!((t - 1.7e9).abs() < 120.0, "{t}"),
        v => panic!("the time of the text: {v:?}"),
    }
    let total = done.unwrap_or(0);
    let received = || srv.board.tx_bytes.lock().unwrap().len() / 4;
    assert!(wait_for(T, || (received() >= total).then_some(())).is_some());
    e.stop_session(s).unwrap();
    // The radio played every sample before the output was muted, and it never ran dry inside a burst.
    let count = |c: &std::sync::atomic::AtomicU64| c.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(count(&srv.board.tx_cut), 0, "samples cut at the mute");
    assert_eq!(count(&srv.board.tx_gap), 0, "samples of gaps");
    samples_of(&bursts[0])
}

#[test]
fn a_meshtastic_text_reaches_the_transmitter_of_the_radio() {
    // LongFast: a burst is about 1 s, many times the transmit buffers of the radio.
    let first = send_text("LongFast", T);
    assert!(first > 1_000_000, "{first} samples");
}

#[test]
fn a_long_burst_is_sent_whole() {
    // LongSlow: a burst is longer than the 2.5 s transmit stream of the session.
    let first = send_text("LongSlow", 60.0);
    assert!(first > 5_000_000, "{first} samples");
}

/// Starts stack `d` with its defaults on the fake iiod server, sends a MeshCore advert with the transmit switch of the
/// engine off, sets the switch while the stack runs and sends an advert again. Returns the count of bursts that the
/// modulator wrote. No setting of the stack is changed.
fn advert_after_the_switch(
    d: &'static sdrkit_core::stack::StackDescriptor,
    submode: Option<&str>,
) -> f64 {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(net.clone(), IiodOpts::default());
    let e = lora_engine(&net);
    let kind = e.find("driver.iiod").unwrap();
    let r = e
        .open_radio("iiod", &srv.addr(), &defaults(&e, kind))
        .unwrap();
    let values = Values::defaults(&(d.settings)());
    let s = e
        .start_stack(d.id, submode, &values, Input::Radio(r), None)
        .unwrap();
    let session = OwnerRef::Session(s);
    let advert = || {
        e.owner(session, |o| {
            let id = o.commands().id("mc.advert")?;
            let args = o.commands().def(id).unwrap().clone().new_args();
            sdrkit_core::owner::invoke(o, id, &args).map(|_| ())
        })
        .unwrap()
    };
    // Off: the advert is refused and not sent.
    assert_eq!(advert(), Err(CommandError::TransmitOff));
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(number(&e, session, "modulate.tx_bursts"), 0.0);
    switch(&e, true);
    assert_eq!(advert(), Ok(()));
    let written = wait_for(T, || {
        let n = number(&e, session, "modulate.tx_bursts");
        (n >= 1.0).then_some(n)
    });
    e.stop_session(s).unwrap();
    written.unwrap_or(0.0)
}

#[test]
fn the_one_transmit_switch_set_while_the_stack_runs_turns_every_station_on() {
    assert!(advert_after_the_switch(&sdrkit_stack_lora::MESHCORE, None) >= 1.0);
    assert!(advert_after_the_switch(&sdrkit_stack_lora::MESH, Some("meshcore")) >= 1.0);
    assert!(advert_after_the_switch(&sdrkit_stack_lora::MESH, None) >= 1.0);
}

#[test]
fn a_text_with_the_switch_off_is_refused_and_nothing_is_sent() {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(net.clone(), IiodOpts::default());
    let e = lora_engine(&net);
    let kind = e.find("driver.iiod").unwrap();
    let r = e
        .open_radio("iiod", &srv.addr(), &defaults(&e, kind))
        .unwrap();
    let d = &sdrkit_stack_lora::MESHTASTIC;
    let s = e
        .start_stack(
            d.id,
            None,
            &Values::defaults(&(d.settings)()),
            Input::Radio(r),
            None,
        )
        .unwrap();
    let session = OwnerRef::Session(s);
    let planned = wait_for(T, || {
        matches!(field(&e, session, "searching"), Some(Value::Text(t)) if !t.is_empty())
            .then_some(())
    });
    assert!(planned.is_some(), "no plan");
    // What can transmit, from the catalogs of the engine.
    let engine = OwnerRef::Engine;
    let yes = Some(Value::Bool(true));
    assert_eq!(cell(&e, engine, "stacks", "meshtastic", "transmit"), yes);
    assert_eq!(
        cell(&e, engine, "stacks", "fanet", "transmit"),
        Some(Value::Bool(false))
    );
    let text = Some(Value::Choice(sdrkit_core::TxContent::Text.index()));
    assert_eq!(
        cell(
            &e,
            engine,
            "stack_transmits",
            "meshtastic/mt.send",
            "content"
        ),
        text
    );
    assert_eq!(
        cell(&e, engine, "stack_transmits", "fanet/send", "content"),
        None
    );
    assert_eq!(cell(&e, engine, "drivers", "iiod", "transmit"), yes);
    assert!(matches!(
        cell(&e, engine, "drivers", "iiod", "tx_max_rate_hz"),
        Some(Value::Hz(hz)) if hz > 0.0
    ));
    let radio = format!("radio.r{}", r.to_u64());
    assert_eq!(cell(&e, engine, "radios", &radio, "transmit"), yes);
    let hz = |col| match cell(&e, engine, "radios", &radio, col) {
        Some(Value::Hz(hz)) => hz,
        v => panic!("{col}: {v:?}"),
    };
    assert!(0.0 < hz("tx_min_hz") && hz("tx_min_hz") < hz("tx_max_hz"));
    assert!(hz("tx_max_rate_hz") > 0.0);
    // The radio can transmit; the switch of the engine is off. The stack as an owner refuses the command as well.
    assert_eq!(
        mt_send(&e, session, "not sent"),
        Err(CommandError::TransmitOff)
    );
    assert_eq!(
        mt_send(&e, OwnerRef::Stack(s), "not sent"),
        Err(CommandError::TransmitOff)
    );
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut events = Vec::new();
    e.take_events(session, &mut events).unwrap();
    let kinds: Vec<&str> = events.iter().map(|ev| ev.kind()).collect();
    assert!(
        !kinds
            .iter()
            .any(|k| ["mesh.text", "tx.request", "session.transmit"].contains(k)),
        "{kinds:?}"
    );
    assert!(column(&e, session, "messages", "text").is_empty());
    assert_eq!(number(&e, session, "tx_pending"), 0.0);
    assert_eq!(number(&e, session, "modulate.tx_bursts"), 0.0);
    assert!(srv.board.tx_bytes.lock().unwrap().is_empty());
    e.stop_session(s).unwrap();
}

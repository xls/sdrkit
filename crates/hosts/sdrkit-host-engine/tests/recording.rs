//! The record and outputs services on the streams of sessions: the rewind buffer, an audio recording, and an output fed by
//! the events of a session, and a transport stream recording.

mod common;

use common::*;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{
    owner, Complex32, Facilities, MemStorage, Random, Record, Storage, StorePath, Value,
};
use sdrkit_host_engine::{Engine, Input, OwnerRef, Samples, Setup};
use std::sync::Arc;

const T: f64 = 20.0;

/// Runs command `cmd` of owner `r` with `args`.
fn call(e: &Engine, r: OwnerRef, cmd: &str, args: &[(&str, Value)]) -> Option<Record> {
    e.owner(r, |o| {
        let id = o.commands().id(cmd).unwrap();
        let mut rec = o.commands().def(id).unwrap().new_args();
        for (k, v) in args {
            let f = rec.decl().id(k).unwrap();
            rec.set_value(f, v).unwrap();
        }
        owner::invoke(o, id, &rec).unwrap()
    })
    .unwrap()
}

/// Sets setting `key` of owner `r` to `v`.
fn set(e: &Engine, r: OwnerRef, key: &str, v: Value) {
    e.owner(r, |o| {
        let id = o.schema().id(key).unwrap();
        owner::apply(o, id, &v)
    })
    .unwrap()
    .unwrap();
}

/// The files in folder `folder` of `storage` whose names end in `end`.
fn files(storage: &MemStorage, folder: &str, end: &str) -> Vec<String> {
    let Ok(list) = storage.list(&StorePath::new(folder).unwrap()) else {
        return Vec::new();
    };
    list.iter()
        .filter(|e| e.name.ends_with(end))
        .map(|e| format!("{folder}/{}", e.name))
        .collect()
}

#[test]
fn the_rewind_buffer_saves_the_last_seconds_and_audio_is_recorded() {
    let storage = Arc::new(MemStorage::new());
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: storage.clone(),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&sdrkit_stack_fm::DESCRIPTOR],
        families: Vec::new(),
        components: sdrkit_stack_fm::graph::registry(),
    })
    .unwrap();
    let rec = e.find("service.record").unwrap();
    set(&e, rec, "reserve_bytes", Value::I64(0));
    set(&e, rec, "rewind_bytes", Value::I64(4 << 20));
    set(&e, rec, "rewind", Value::Bool(true));
    let synth = e.find("driver.synth").unwrap();
    let r = e
        .open_radio("synth", "synth:fm", &defaults(&e, synth))
        .unwrap();
    let fm = e.find("stack.fm").unwrap();
    let s = e
        .start_stack("fm", None, &defaults(&e, fm), Input::Radio(r), None)
        .unwrap();
    // The rewind buffer fills from the session's samples.
    wait_for(T, || (number(&e, rec, "rewind_s") > 0.3).then_some(()))
        .expect("the rewind buffer holds samples");
    call(&e, rec, "rewind.save", &[("pre_s", Value::Duration(0.2))]);
    let saved = wait_for(T, || {
        let f = files(&storage, "recordings", ".sigmf-data");
        (!f.is_empty()).then_some(f)
    })
    .expect("a saved rewind");
    assert!(!storage
        .read(&StorePath::new(&saved[0]).unwrap())
        .unwrap()
        .is_empty());
    // The session's audio goes into a file.
    call(
        &e,
        rec,
        "audio.start",
        &[
            ("session", Value::Text(format!("session.s{}", s.to_u64()))),
            ("seconds", Value::Duration(0.4)),
        ],
    );
    let audio = wait_for(T, || {
        let f = files(&storage, "recordings", ".wav");
        let full = f.iter().any(|p| {
            storage
                .read(&StorePath::new(p).unwrap())
                .is_ok_and(|b| b.len() > 44)
        });
        full.then_some(f)
    });
    assert!(audio.is_some(), "an audio file with samples");
}

#[test]
fn the_events_of_a_session_feed_an_output() {
    let storage = Arc::new(MemStorage::new());
    let e = live_engine(storage.clone());
    let out = e.find("service.outputs").unwrap();
    call(
        &e,
        out,
        "output.add",
        &[
            ("row", Value::Text("file".into())),
            ("kind", Value::Choice(2)),
            ("enabled", Value::Bool(true)),
            ("address", Value::Text("out.jsonl".into())),
        ],
    );
    call(&e, out, "start", &[]);
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let feed = e.open_feed(48e3, 100e6);
    e.start_stack("meter", None, &v, Input::Feed(feed), None)
        .unwrap();
    let tone: Vec<Complex32> = (0..4800)
        .map(|i| Complex32::from_polar(1.0, i as f32 * 0.3))
        .collect();
    let written = wait_for(T, || {
        let _ = e.push(feed, Samples::Iq(&tone));
        let b = storage.read(&StorePath::new("out.jsonl").ok()?).ok()?;
        String::from_utf8(b)
            .ok()?
            .contains("meter.block")
            .then_some(())
    });
    assert!(written.is_some(), "the output wrote the session's events");
}

#[test]
fn the_transport_stream_of_a_dvbt_session_is_recorded() {
    use sdrkit_core::{Choice, GenConfig};
    use sdrkit_stack_dvbt::params::{Bandwidth, CodeRate, Constellation, Guard, Mode};
    use sdrkit_stack_dvbt::settings::GenSource;

    const RATE: f64 = 8e6;
    let storage = Arc::new(MemStorage::new());
    let mut components = sdrkit_graph::Registry::standard();
    sdrkit_stack_dvbt::register(&mut components);
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: storage.clone(),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&sdrkit_stack_dvbt::DVBT],
        families: Vec::new(),
        components,
    })
    .unwrap();
    let mut v = defaults(&e, e.find("stack.dvbt").unwrap());
    for (key, x) in [
        ("gen_mode", Mode::K2.index()),
        ("gen_guard", Guard::G4.index()),
        ("gen_constellation", Constellation::Qpsk.index()),
        ("gen_rate_hp", CodeRate::R12.index()),
        ("gen_bandwidth", Bandwidth::B7.index()),
        ("gen_source", GenSource::Pattern.index()),
    ] {
        let id = v.schema().id(key).unwrap();
        v.set(id, &Value::Choice(x)).unwrap();
    }
    let make = sdrkit_stack_dvbt::DVBT.make_generator.unwrap();
    let mut gen = make(&GenConfig::new(RATE, v.clone())).unwrap();
    let feed = e.open_feed(RATE, 0.0);
    let s = e
        .start_stack("dvbt", None, &v, Input::Feed(feed), None)
        .unwrap();
    let rec = e.find("service.record").unwrap();
    set(&e, rec, "reserve_bytes", Value::I64(0));
    let session = Value::Text(format!("session.s{}", s.to_u64()));
    call(&e, rec, "ts.start", &[("session", session)]);
    let mut x = vec![Complex32::default(); 65536];
    // Two seconds of signal, each block written again until the session has read the one before.
    for _ in 0..(2.0 * RATE) as usize / x.len() {
        gen.generate(&mut x);
        wait_for(T, || e.push(feed, Samples::Iq(&x)).ok()).expect("the session reads its feed");
    }
    e.stop_session(s).unwrap();
    let ts = wait_for(T, || {
        let f = files(&storage, "recordings", ".ts");
        let b = storage.read(&StorePath::new(f.first()?).ok()?).ok()?;
        (!b.is_empty()).then_some(b)
    })
    .expect("a transport stream file with packets");
    assert_eq!(ts.len() % 188, 0);
    assert!(ts.chunks(188).all(|p| p[0] == 0x47));
}

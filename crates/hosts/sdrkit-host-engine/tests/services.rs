//! The services as owners, the requests of receivers and services, the saved settings and the saved state.

mod common;

use common::*;
use sdrkit_core::{owner, CommandError, Complex32, Event, Record, Value, Values};
use sdrkit_host_engine::{Engine, EngineError, ImportError, Input, OwnerRef, Samples, SessionId};

const T: f64 = 5.0;

/// Every service, by owner key.
const SERVICES: [&str; 12] = [
    "service.store",
    "service.sessions",
    "service.automation",
    "service.outputs",
    "service.watch",
    "service.memory",
    "service.scan",
    "service.freqdb",
    "service.sweep",
    "service.identify",
    "service.bank",
    "service.record",
];

/// The row keys of table `table` of owner `r`, sorted.
fn rows(e: &Engine, r: OwnerRef, table: &str) -> Vec<String> {
    e.owner(r, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table).unwrap()).unwrap();
        let mut v: Vec<String> = t.rows_newest_first().map(|(k, _)| k.to_owned()).collect();
        v.sort();
        v
    })
    .unwrap()
}

/// The value in column `col` of row `row` of table `table` of the engine owner.
fn cell(e: &Engine, table: &str, row: &str, col: &str) -> Option<Value> {
    e.owner(OwnerRef::Engine, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table)?)?;
        let c = t.columns().id(col)?;
        t.row(row)?.value(c).cloned()
    })
    .unwrap()
}

/// Runs command `cmd` of owner `r` with the arguments `args`, by key.
fn call(
    e: &Engine,
    r: OwnerRef,
    cmd: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    e.owner(r, |o| {
        let id = o.commands().id(cmd)?;
        let mut rec = o.commands().def(id).unwrap().new_args();
        for (k, v) in args {
            let f = rec.decl().id(k).unwrap();
            rec.set_value(f, v).unwrap();
        }
        owner::invoke(o, id, &rec)
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

fn values(e: &Engine, r: OwnerRef) -> Values {
    e.owner(r, |o| o.values().unwrap().clone()).unwrap()
}

fn tuned(e: &Engine, s: SessionId) -> f64 {
    match values(e, OwnerRef::Session(s)).value(sdrkit_core::SettingId(0)) {
        Some(Value::Hz(hz)) => *hz,
        _ => 0.0,
    }
}

fn tone(n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| Complex32::from_polar(1.0, i as f32 * 0.3))
        .collect()
}

#[test]
fn every_service_is_an_owner_by_key_and_the_services_table_lists_them() {
    let e = engine();
    let mut keys: Vec<String> = SERVICES.iter().map(|k| (*k).to_owned()).collect();
    keys.sort();
    assert_eq!(rows(&e, OwnerRef::Engine, "services"), keys);
    for k in SERVICES {
        let r = e.find(k).unwrap();
        assert!(matches!(r, OwnerRef::Service(_)), "{k}");
        assert_eq!(e.key_of(r).as_deref(), Some(k));
        let named = e.owner(r, |o| o.results_decl().tables().len() + o.commands().len());
        assert!(named.unwrap() > 0, "{k} declares tables or commands");
    }
    assert!(matches!(
        e.find("service.nope"),
        Err(EngineError::NoSuchKey { .. })
    ));
}

#[test]
fn a_request_from_the_scanner_tunes_a_session() {
    let e = engine();
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let scan = e.find("service.scan").unwrap();
    call(
        &e,
        scan,
        "channels",
        &[
            ("session", Value::Text(format!("session.s{}", s.to_u64()))),
            ("stack", Value::Text("meter".into())),
            ("start_hz", Value::Hz(144e6)),
            ("stop_hz", Value::Hz(144.1e6)),
            ("step_hz", Value::Hz(25e3)),
        ],
    )
    .unwrap();
    let hz = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(4800)));
        let hz = tuned(&e, s);
        (144e6..=144.1e6).contains(&hz).then_some(hz)
    });
    assert!(hz.is_some(), "the scanner tuned the session");
    let from = wait_for(T, || cell(&e, "requests", "1", "from"));
    assert_eq!(from, Some(Value::Text("service.scan".into())));
    assert_eq!(cell(&e, "requests", "1", "done"), Some(Value::Bool(true)));
}

/// Sets the frequency of session `s` to `hz`.
fn tune(e: &Engine, s: SessionId, hz: f64) {
    e.owner(OwnerRef::Session(s), |o| {
        o.apply(sdrkit_core::SettingId(0), &Value::Hz(hz))
            .map(|_| ())
    })
    .unwrap()
    .unwrap();
}

#[test]
fn a_frequency_set_by_the_host_stays_when_the_scan_stops() {
    let e = engine();
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    tune(&e, s, 100.5e6);
    let scan = e.find("service.scan").unwrap();
    call(
        &e,
        scan,
        "channels",
        &[
            ("session", Value::Text(format!("session.s{}", s.to_u64()))),
            ("stack", Value::Text("meter".into())),
            ("start_hz", Value::Hz(144e6)),
            ("stop_hz", Value::Hz(144.1e6)),
            ("step_hz", Value::Hz(25e3)),
        ],
    )
    .unwrap();
    let scanned = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(4800)));
        (144e6..=144.1e6).contains(&tuned(&e, s)).then_some(())
    });
    assert!(scanned.is_some(), "the scanner tuned the session");
    // The host stops the scan and tunes at once, before the engine sees that the scan ended.
    call(&e, scan, "stop", &[]).unwrap();
    tune(&e, s, 101e6);
    let moved = wait_for(1.0, || (tuned(&e, s) != 101e6).then(|| tuned(&e, s)));
    assert_eq!(moved, None, "the session left the frequency of the host");
}

#[test]
fn a_follow_request_starts_the_stack_that_declares_its_signal_type() {
    let rtl = fake_rtl();
    let e = engine_on(rtl.net());
    let r = e
        .open_radio(
            "rtl_tcp",
            &rtl.addr(),
            &defaults(&e, e.find("driver.rtl_tcp").unwrap()),
        )
        .unwrap();
    let mut v = defaults(&e, e.find("stack.meter").unwrap());
    v.set(v.schema().id("follow_hz").unwrap(), &Value::Hz(118.1e6))
        .unwrap();
    let s = e
        .start_stack("meter", None, &v, Input::Radio(r), None)
        .unwrap();
    let follower = wait_for(T, || {
        rows(&e, OwnerRef::Engine, "sessions")
            .into_iter()
            .find(|k| cell(&e, "sessions", k, "stack") == Some(Value::Text("voice".into())))
    })
    .expect("the voice stack follows");
    let f = SessionId::from_u64(follower.strip_prefix("session.s").unwrap().parse().unwrap());
    assert_eq!(tuned(&e, f), 118.1e6);
    // The follower starts with the stack settings of the request.
    let gain = e
        .owner(OwnerRef::Stack(f), |o| {
            let id = o.schema().id("gain").unwrap();
            o.values().and_then(|v| v.value(id).cloned())
        })
        .unwrap();
    assert_eq!(gain, Some(Value::F64(2.5)));
    assert!(matches!(
        e.close_radio(r),
        Err(EngineError::RadioInUse { sessions: 2 })
    ));
    for id in [s, f] {
        e.stop_session(id).unwrap();
        e.close_session(id).unwrap();
    }
    e.close_radio(r).unwrap();
}

#[test]
fn a_request_that_cannot_be_served_shows_in_the_requesting_owner_s_status() {
    let e = engine();
    let mut v = defaults(&e, e.find("stack.meter").unwrap());
    v.set(v.schema().id("hop").unwrap(), &Value::Bool(true))
        .unwrap();
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let why = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(4800)));
        match field(&e, OwnerRef::Session(s), "session.request_problem") {
            Some(Value::Text(t)) => Some(t),
            _ => None,
        }
    });
    assert!(why.is_some_and(|t| t.contains("hop")));
    let mut events: Vec<Event> = Vec::new();
    e.take_events(OwnerRef::Engine, &mut events).unwrap();
    let id = e
        .event_catalog()
        .id_of(&sdrkit_host_engine::REQUEST)
        .unwrap();
    let ev = events.iter().find(|ev| ev.kind_id() == Some(id)).unwrap();
    let done = ev.fields.iter().find(|(d, _)| d.key == "done");
    assert_eq!(done.map(|(_, v)| v), Some(&Value::Bool(false)));
    // With requests off, the engine refuses each one.
    set(&e, OwnerRef::Engine, "requests_allowed", Value::Bool(false));
    let mut v = defaults(&e, e.find("stack.meter").unwrap());
    v.set(v.schema().id("follow_hz").unwrap(), &Value::Hz(118.1e6))
        .unwrap();
    let t_feed = e.open_feed(48e3, 100e6);
    let t = e
        .start_stack("meter", None, &v, Input::Feed(t_feed), None)
        .unwrap();
    let why = wait_for(T, || {
        let _ = e.push(t_feed, Samples::Iq(&tone(4800)));
        match field(&e, OwnerRef::Session(t), "session.request_problem") {
            Some(Value::Text(w)) => Some(w),
            _ => None,
        }
    });
    assert!(why.is_some_and(|w| w.contains("not allowed")));
}

/// An engine with a radio, a stack session on it and every service, and the owner keys of the radio and the session.
fn busy_engine(rtl: &FakeRtl) -> (Engine, OwnerRef, OwnerRef) {
    let e = engine_on(rtl.net());
    let r = e
        .open_radio(
            "rtl_tcp",
            &rtl.addr(),
            &defaults(&e, e.find("driver.rtl_tcp").unwrap()),
        )
        .unwrap();
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s = e
        .start_stack("meter", None, &v, Input::Radio(r), None)
        .unwrap();
    (e, OwnerRef::Radio(r), OwnerRef::Session(s))
}

/// The values of the engine, the services and `more`, as JSON text by owner key.
fn every_value(e: &Engine, more: &[&str]) -> Vec<(String, String)> {
    let mut keys = vec!["engine".to_owned()];
    keys.extend(SERVICES.iter().map(|k| (*k).to_owned()));
    keys.extend(more.iter().map(|k| (*k).to_owned()));
    keys.into_iter()
        .map(|k| {
            let r = e.find(&k).unwrap();
            let v = e.owner(r, |o| o.values().map(Values::to_json)).unwrap();
            (k, serde_json::to_string(&v).unwrap())
        })
        .collect()
}

/// The entries of one owner in a hand-written blob: (setting key, tag, value bytes).
type Entries<'a> = &'a [(&'a str, u8, Vec<u8>)];

/// A settings blob of format version 1, written by hand: owners by key, each with its entries. It stands for a blob that
/// an earlier version of the library wrote.
fn blob(owners: &[(&str, Entries)]) -> Vec<u8> {
    let len = |out: &mut Vec<u8>, n: usize| out.extend_from_slice(&(n as u32).to_le_bytes());
    let mut out = b"SDKS".to_vec();
    out.extend_from_slice(&1u16.to_le_bytes());
    len(&mut out, owners.len());
    for (key, entries) in owners {
        len(&mut out, key.len());
        out.extend_from_slice(key.as_bytes());
        len(&mut out, entries.len());
        for (k, tag, v) in entries.iter() {
            len(&mut out, k.len());
            out.extend_from_slice(k.as_bytes());
            out.push(*tag);
            len(&mut out, v.len());
            out.extend_from_slice(v);
        }
    }
    let crc = sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC.checksum(&out);
    out.extend_from_slice(&crc.to_le_bytes());
    out
}

const TEXT: u8 = 4;
const F64: u8 = 3;
const I64: u8 = 2;
const BOOL: u8 = 1;

fn engine_value(e: &Engine, key: &str) -> Option<Value> {
    e.owner(OwnerRef::Engine, |o| {
        o.values()
            .and_then(|v| v.value(v.schema().id(key).ok()?).cloned())
    })
    .unwrap()
}

#[test]
fn the_settings_blob_round_trips_every_owner_and_a_bad_value_changes_nothing() {
    let rtl = fake_rtl();
    let (e, radio, session) = busy_engine(&rtl);
    let (rk, sk) = (e.key_of(radio).unwrap(), e.key_of(session).unwrap());
    let saved = e.export_settings().unwrap();
    assert_eq!(&saved[..4], b"SDKS");
    let before = every_value(&e, &[&rk, &sk]);

    set(
        &e,
        OwnerRef::Engine,
        "callsign",
        Value::Text("VK2XYZ".into()),
    );
    set(&e, session, "gain", Value::F64(3.0));
    set(&e, radio, "centre_hz", Value::Hz(433.92e6));
    assert_ne!(every_value(&e, &[&rk, &sk]), before);
    e.import_settings(&saved).unwrap();
    assert_eq!(
        every_value(&e, &[&rk, &sk]),
        before,
        "the blob brings every owner back"
    );

    let changed = every_value(&e, &[&rk, &sk]);
    let bad = blob(&[
        ("engine", &[("callsign", TEXT, b"VK2ABC".to_vec())]),
        (&sk, &[("gain", TEXT, b"loud".to_vec())]),
    ]);
    let refused = e.import_settings(&bad);
    assert!(
        matches!(&refused, Err(EngineError::Import(ImportError::Value { owner, setting: Some(k), .. }))
            if *owner == sk && k == "gain"),
        "{refused:?}"
    );
    assert_eq!(every_value(&e, &[&rk, &sk]), changed, "nothing changed");
}

#[test]
fn a_blob_with_unknown_keys_loads_and_a_missing_setting_takes_its_default() {
    let e = engine();
    let default_callsign = defaults(&e, OwnerRef::Engine)
        .value(
            e.owner(OwnerRef::Engine, |o| o.schema().id("callsign").unwrap())
                .unwrap(),
        )
        .cloned();
    set(
        &e,
        OwnerRef::Engine,
        "callsign",
        Value::Text("VK2XYZ".into()),
    );
    let old = blob(&[
        (
            "engine",
            &[
                ("no_such_setting", I64, 1i64.to_le_bytes().to_vec()),
                ("a_later_type", 200, vec![1, 2, 3]),
            ],
        ),
        (
            "session.s999",
            &[("gain", F64, 2.0f64.to_le_bytes().to_vec())],
        ),
        ("service.no_such_service", &[]),
    ]);
    e.import_settings(&old).unwrap();
    assert_eq!(engine_value(&e, "callsign"), default_callsign);
    let named = blob(&[("engine", &[("callsign", TEXT, b"VK2ABC".to_vec())])]);
    e.import_settings(&named).unwrap();
    assert_eq!(
        engine_value(&e, "callsign"),
        Some(Value::Text("VK2ABC".into()))
    );
}

#[test]
fn a_later_version_or_damaged_bytes_are_refused_and_change_nothing() {
    let e = engine();
    let mut saved = e.export_settings().unwrap();
    set(
        &e,
        OwnerRef::Engine,
        "callsign",
        Value::Text("VK2XYZ".into()),
    );
    let before = every_value(&e, &[]);
    saved[4..6].copy_from_slice(&2u16.to_le_bytes());
    assert!(matches!(
        e.import_settings(&saved),
        Err(EngineError::Import(ImportError::Version { found: 2 }))
    ));
    saved[4..6].copy_from_slice(&1u16.to_le_bytes());
    let n = saved.len();
    assert!(matches!(
        e.import_settings(&saved[..n - 1]),
        Err(EngineError::Import(ImportError::Damaged { .. }))
    ));
    assert!(matches!(
        e.import_settings(br#"{ "version": 1, "owners": {} }"#),
        Err(EngineError::Import(ImportError::NotSettings))
    ));
    assert_eq!(every_value(&e, &[]), before);
}

#[test]
fn transmit_allowed_is_neither_written_to_nor_read_from_a_settings_blob() {
    let e = engine();
    set(&e, OwnerRef::Engine, "transmit_allowed", Value::Bool(true));
    let saved = e.export_settings().unwrap();
    let has = |b: &[u8], k: &[u8]| b.windows(k.len()).any(|w| w == k);
    assert!(!has(&saved, b"transmit_allowed"));
    assert!(has(&saved, b"callsign"));
    e.import_settings(&saved).unwrap();
    assert_eq!(
        engine_value(&e, "transmit_allowed"),
        Some(Value::Bool(true)),
        "an import leaves transmit as it is"
    );
    let on = blob(&[("engine", &[("transmit_allowed", BOOL, vec![1])])]);
    assert!(matches!(
        e.import_settings(&on),
        Err(EngineError::Import(ImportError::NotSaved { .. }))
    ));
    assert_eq!(
        engine_value(&engine(), "transmit_allowed"),
        Some(Value::Bool(false)),
        "every run starts with transmit off"
    );
}

#[test]
fn the_json_schema_returns_kind_range_choices_defaults_and_labels() {
    let e = engine();
    let text = e.settings_schema(OwnerRef::Engine).unwrap();
    let s: serde_json::Value = serde_json::from_str(&text).unwrap();
    let p = &s["properties"];
    assert_eq!(s["additionalProperties"], false);
    assert_eq!(p["requests_allowed"]["type"], "boolean");
    assert_eq!(p["requests_allowed"]["default"], true);
    assert_eq!(p["callsign"]["type"], "string");
    assert!(p["callsign"].get("enum").is_none());
    assert!(!p["backend_fft"]["enum"].as_array().unwrap().is_empty());
    assert_eq!(p["time_zone"]["minimum"], -720.0);
    assert_eq!(p["time_zone"]["multipleOf"], 1.0);
    assert_eq!(p["callsign"]["title"], "Callsign");
    assert_eq!(p["position"]["type"], "object");
}

/// The saved state of `e`, from the engine's command `state.save`.
fn save(e: &Engine) -> std::sync::Arc<[u8]> {
    let reply = call(e, OwnerRef::Engine, "state.save", &[])
        .unwrap()
        .unwrap();
    match reply.value(reply.decl().id("state").unwrap()).cloned() {
        Some(Value::Bytes(b)) => b,
        other => panic!("the reply holds the state: {other:?}"),
    }
}

/// The text of `e` and of every error below it.
fn chain(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut at = e.source();
    while let Some(s) = at {
        text.push_str(": ");
        text.push_str(&s.to_string());
        at = s.source();
    }
    text
}

#[test]
fn the_saved_state_round_trips() {
    let rtl = fake_rtl();
    let (e, _, session) = busy_engine(&rtl);
    let OwnerRef::Session(s) = session else {
        unreachable!()
    };
    set(&e, session, "tuned_hz", Value::Hz(100.1e6));
    let bytes = save(&e);
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["radios"][0]["address"], rtl.addr());
    assert_eq!(doc["sessions"][0]["stack"], "meter");
    assert_eq!(doc["sessions"][0]["radio"], rtl.addr());
    assert!(doc["services"]["service.bank"].is_object());
    let handles = serde_json::to_string(&doc).unwrap();
    assert!(!handles.contains("radio.r") && !handles.contains("session.s"));
    // In the same run, what runs is kept.
    let before = rows(&e, OwnerRef::Engine, "sessions");
    call(
        &e,
        OwnerRef::Engine,
        "state.restore",
        &[("state", Value::Bytes(bytes.clone()))],
    )
    .unwrap();
    assert_eq!(rows(&e, OwnerRef::Engine, "sessions"), before);
    assert_eq!(save(&e), bytes);
    // In a new run, the radio opens again and the session starts again, tuned.
    let again = engine_on(rtl.net());
    call(
        &again,
        OwnerRef::Engine,
        "state.restore",
        &[("state", Value::Bytes(bytes.clone()))],
    )
    .unwrap();
    let radios = rows(&again, OwnerRef::Engine, "radios");
    assert_eq!(radios.len(), 1);
    assert_eq!(
        cell(&again, "radios", &radios[0], "address"),
        Some(Value::Text(rtl.addr()))
    );
    let sessions = rows(&again, OwnerRef::Engine, "sessions");
    assert_eq!(sessions.len(), 1);
    let id = SessionId::from_u64(
        sessions[0]
            .strip_prefix("session.s")
            .unwrap()
            .parse()
            .unwrap(),
    );
    assert_eq!(tuned(&again, id), 100.1e6);
    assert_eq!(tuned(&e, s), 100.1e6);
    // A state with a stack that is not there is refused at that session.
    let mut bad = doc;
    bad["sessions"][0]["stack"] = "nope".into();
    let third = engine_on(rtl.net());
    let refused = call(
        &third,
        OwnerRef::Engine,
        "state.restore",
        &[(
            "state",
            Value::Bytes(serde_json::to_vec(&bad).unwrap().into()),
        )],
    );
    let Err(err) = refused else {
        panic!("a state with an unknown stack is refused");
    };
    assert!(matches!(err, CommandError::Failed { .. }));
    assert!(chain(&err).contains("session 0"), "{}", chain(&err));
    assert_eq!(rows(&third, OwnerRef::Engine, "radios").len(), 1);
}

#[test]
fn the_sweep_reads_the_frames_of_the_shared_derivation() {
    let rtl = fake_rtl();
    let (e, radio, session) = busy_engine(&rtl);
    let OwnerRef::Session(s) = session else {
        unreachable!()
    };
    let OwnerRef::Radio(r) = radio else {
        unreachable!()
    };
    let sweep = e.find("service.sweep").unwrap();
    set(&e, sweep, "settle_auto", Value::Bool(false));
    call(
        &e,
        sweep,
        "start",
        &[
            ("session", Value::Text(format!("session.s{}", s.to_u64()))),
            ("low_hz", Value::Hz(100e6)),
            ("high_hz", Value::Hz(110e6)),
        ],
    )
    .unwrap();
    let hops = wait_for(3.0 * T, || {
        let n = number(&e, sweep, "hops_done");
        (n > 0.0).then_some(n)
    });
    let state = field(&e, sweep, "state");
    assert!(hops.is_some(), "the sweep hops: {state:?}");
    let report = e.derivations(r).unwrap();
    let spectra = report
        .adapters
        .iter()
        .filter(|x| x.kind == "dsp.spectrum")
        .count();
    assert_eq!(spectra, 1, "{report:?}");
}

#[test]
fn the_bank_adds_a_span_and_its_channel_rows_refer_to_live_sessions() {
    let rtl = fake_rtl();
    let (e, radio, _) = busy_engine(&rtl);
    let OwnerRef::Radio(radio) = radio else {
        unreachable!()
    };
    let bank = e.find("service.bank").unwrap();
    call(
        &e,
        bank,
        "span.add",
        &[
            ("radio", Value::Text(format!("radio.r{}", radio.to_u64()))),
            ("stack", Value::Text("meter".into())),
            ("raster", Value::Text("100000000:25000:3".into())),
            ("open", Value::Choice(0)),
        ],
    )
    .unwrap();
    let session_of = |c: &str| {
        e.owner(bank, |o| {
            let res = o.results();
            let t = res.table(res.decl().table_id("channels")?)?;
            let col = t.columns().id("session")?;
            match t.row(c)?.value(col)? {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            }
        })
        .unwrap()
    };
    let refs = wait_for(T, || {
        let r = rows(&e, bank, "channels");
        let s: Vec<String> = r.iter().filter_map(|c| session_of(c)).collect();
        (r.len() == 3 && s.len() == 3).then_some(s)
    })
    .expect("three channels, each with its session");
    let sessions = rows(&e, OwnerRef::Engine, "sessions");
    for s in refs {
        assert!(sessions.contains(&s), "{s} in {sessions:?}");
        let id = SessionId::from_u64(s.strip_prefix("session.s").unwrap().parse().unwrap());
        assert!(e.owner(OwnerRef::Session(id), |_| ()).is_ok());
        // The bank's mix reads the channel's audio: the host does not.
        let refused = wait_for(T, || {
            matches!(e.audio(id, &mut [0.0; 64]), Err(EngineError::InMix)).then_some(())
        });
        assert!(refused.is_some(), "a host read of a channel session");
    }
}

#[test]
fn the_engine_returns_its_station_to_every_receiver() {
    let e = engine();
    set(
        &e,
        OwnerRef::Engine,
        "callsign",
        Value::Text("VK2XYZ".into()),
    );
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let call = |want: &str| {
        wait_for(T, || {
            let _ = e.push(s_feed, Samples::Iq(&tone(4800)));
            (field(&e, OwnerRef::Session(s), "call") == Some(Value::Text(want.into())))
                .then_some(())
        })
    };
    assert!(call("VK2XYZ").is_some(), "a new receiver gets the station");
    set(
        &e,
        OwnerRef::Engine,
        "callsign",
        Value::Text("VK2ABC".into()),
    );
    assert!(call("VK2ABC").is_some(), "a running receiver gets a change");
}

/// A journal of more messages than an open reads: a search through the store owner shows the newest messages at once,
/// and the engine lands the full read on its own, which runs the search again on every message.
#[test]
fn a_search_of_a_large_store_ends_on_every_message() {
    let storage = std::sync::Arc::new(sdrkit_core::MemStorage::new());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let n = 5_000;
    let mut journal = String::new();
    for i in 0..n {
        let t = now - f64::from(n - i);
        journal.push_str(&format!(
            "{{\"id\":{},\"time\":{t},\"last_time\":{t},\"kind\":\"engine.request\",\"family\":\"other\",\
             \"urgency\":\"none\",\"freq_hz\":0.0,\"text\":\"request number {i} of a long journal\",\
             \"repeats\":0,\"fields\":{{}}}}\n",
            i + 1
        ));
    }
    let path = sdrkit_core::StorePath::new("store/messages.jsonl").unwrap();
    sdrkit_core::Storage::write(&*storage, &path, journal.as_bytes()).unwrap();
    let e = live_engine(storage);
    let store = e.find("service.store").unwrap();
    let field = |key: &str| {
        e.owner(store, |o| {
            let r = o.results();
            let id = r.decl().fields().id(key)?;
            r.fields().value(id).cloned()
        })
        .unwrap()
    };
    assert_eq!(field("loaded"), Some(Value::Bool(false)));
    call(&e, store, "search", &[("limit", Value::I64(10))]).unwrap();
    assert_eq!(rows(&e, store, "messages").len(), 10);
    wait_for(30.0, || {
        (field("matches") == Some(Value::I64(i64::from(n)))).then_some(())
    })
    .expect("the search runs again on every message");
    assert_eq!(field("loaded"), Some(Value::Bool(true)));
    assert_eq!(field("rows"), Some(Value::I64(i64::from(n))));
}

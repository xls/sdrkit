//! What the services ask the engine to do, carried out from end to end: the identify capture and the automatic choices,
//! the memory scan, recall and presets, the rules of the automation, and stored memories.

mod common;

use common::*;
use sdrkit_core::{owner, CommandError, Complex32, MemStorage, Record, Storage, Value};
use sdrkit_host_engine::{Engine, Input, OwnerRef, Samples, SessionId};
use std::sync::Arc;

const T: f64 = 10.0;

fn tone(n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| Complex32::from_polar(1.0, i as f32 * 0.3))
        .collect()
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

/// The value of field `key` of a reply.
fn reply(r: &Record, key: &str) -> Option<Value> {
    r.value(r.decl().id(key)?).cloned()
}

/// The rows of table `table` of owner `r`: each row key with the value in column `col`.
fn column(e: &Engine, r: OwnerRef, table: &str, col: &str) -> Vec<(String, Option<Value>)> {
    e.owner(r, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table).unwrap()).unwrap();
        let c = t.columns().id(col).unwrap();
        let mut v: Vec<(String, Option<Value>)> = t
            .rows_newest_first()
            .map(|(k, row)| (k.to_owned(), row.value(c).cloned()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    })
    .unwrap()
}

fn text(v: Option<Value>) -> String {
    match v {
        Some(Value::Text(t)) => t,
        _ => String::new(),
    }
}

fn hz(v: Option<Value>) -> f64 {
    match v {
        Some(Value::Hz(x) | Value::F64(x)) => x,
        Some(Value::I64(n)) => n as f64,
        _ => f64::NAN,
    }
}

/// The key of the choice in field `key` of owner `r`.
fn state(e: &Engine, r: OwnerRef, key: &str) -> String {
    e.owner(r, |o| {
        let res = o.results();
        let id = res.decl().field_id(key)?;
        let d = res.decl().fields().field(id)?;
        match res.fields().value(id)? {
            Value::Choice(i) => d.choices.get(usize::from(*i)).map(|c| c.key.to_string()),
            _ => None,
        }
    })
    .unwrap()
    .unwrap_or_default()
}

/// The tuned frequency of session `s`.
fn tuned(e: &Engine, s: SessionId) -> f64 {
    e.owner(OwnerRef::Session(s), |o| {
        let v = o.values()?;
        match v.value(o.schema().id("tuned_hz").ok()?)? {
            Value::Hz(x) => Some(*x),
            _ => None,
        }
    })
    .unwrap()
    .unwrap_or(0.0)
}

/// The stack that session `s` runs, from the engine's `sessions` table.
fn stack_of(e: &Engine, s: SessionId) -> String {
    column(e, OwnerRef::Engine, "sessions", "stack")
        .into_iter()
        .find(|(k, _)| *k == format!("session.s{}", s.to_u64()))
        .map(|(_, v)| text(v))
        .unwrap_or_default()
}

#[test]
fn the_identifier_measures_the_strongest_signal() {
    let e = live_engine(Arc::new(MemStorage::new()));
    tone_radio(&e, 250e3);
    let id = e.find("service.identify").unwrap();
    call(&e, id, "arm", &[]).unwrap();
    let done = wait_for(6.0 * T, || (state(&e, id, "state") == "done").then_some(()));
    assert!(
        done.is_some(),
        "no identification: {}",
        state(&e, id, "state")
    );
    let freqs = column(&e, id, "signals", "freq_hz");
    let rf = hz(freqs.first().and_then(|(_, v)| v.clone()));
    assert!(
        (rf - (100e6 + TONE_OFFSET_HZ)).abs() < 5e3,
        "{rf} in {freqs:?}"
    );
    assert!(!text(field(&e, id, "text")).is_empty());
    // The capture session ended once the capture was full.
    let left = wait_for(T, || {
        let rows = column(&e, OwnerRef::Engine, "sessions", "state");
        rows.is_empty().then_some(())
    });
    assert!(left.is_some(), "the capture session ends");
}

#[test]
fn a_job_reports_progress_and_can_be_cancelled() {
    let e = live_engine(Arc::new(MemStorage::new()));
    tone_radio(&e, 250e3);
    let id = e.find("service.identify").unwrap();
    call(&e, id, "arm", &[("target", Value::Choice(3))]).unwrap();
    let mut progress = 0.0f64;
    let done = wait_for(6.0 * T, || {
        progress = progress.max(number(&e, id, "progress"));
        (state(&e, id, "state") == "done").then_some(())
    });
    assert!(
        done.is_some(),
        "the analysis ends: {}",
        state(&e, id, "state")
    );
    assert!(progress > 0.0 || number(&e, id, "progress") > 0.0);
    // Another analysis of the capture that the service holds, cancelled while it waits or runs.
    call(&e, id, "capture", &[("target", Value::Choice(3))]).unwrap();
    call(&e, id, "cancel", &[]).unwrap();
    let s = state(&e, id, "state");
    assert!(s == "cancelled" || s == "idle", "{s}");
    std::thread::sleep(std::time::Duration::from_millis(500));
    let s = state(&e, id, "state");
    assert!(
        s == "cancelled" || s == "idle",
        "nothing runs after a cancel: {s}"
    );
}

#[test]
fn tv_auto_returns_up_on_a_plain_tone_and_voice_auto_lists_nothing_sure() {
    let e = live_engine(Arc::new(MemStorage::new()));
    let r = tone_radio(&e, 250e3);
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s = e
        .start_stack("meter", None, &v, Input::Radio(r), None)
        .unwrap();
    let id = e.find("service.identify").unwrap();
    let session = Value::Text(format!("session.s{}", s.to_u64()));
    call(
        &e,
        id,
        "arm",
        &[("target", Value::Choice(4)), ("session", session.clone())],
    )
    .unwrap();
    let first = text(field(&e, id, "auto_text"));
    let ended = wait_for(9.0 * T, || {
        let t = text(field(&e, id, "auto_text"));
        (state(&e, id, "state") == "done" && t != first).then_some(t)
    });
    assert!(ended.is_some(), "TV auto ends: {first}");
    assert_eq!(stack_of(&e, s), "meter", "{ended:?}");
    call(
        &e,
        id,
        "arm",
        &[("target", Value::Choice(5)), ("session", session)],
    )
    .unwrap();
    let looked = wait_for(9.0 * T, || {
        let t = text(field(&e, id, "auto_text"));
        (!t.starts_with("Looking")).then_some(t)
    });
    assert!(looked.is_some(), "voice auto looks");
    assert_eq!(stack_of(&e, s), "meter", "{looked:?}");
    call(&e, id, "cancel", &[]).unwrap();
    let st = state(&e, id, "state");
    assert!(st == "cancelled" || st == "idle", "{st}");
}

/// Adds a memory of `stack` at `freq_hz` and returns its row key.
fn add_memory(e: &Engine, name: &str, freq_hz: f64, stack: &str) -> String {
    let mem = e.find("service.memory").unwrap();
    let r = call(
        e,
        mem,
        "add",
        &[
            ("name", Value::Text(name.into())),
            ("freq_hz", Value::Hz(freq_hz)),
            ("stack", Value::Text(stack.into())),
        ],
    )
    .unwrap()
    .unwrap();
    match reply(&r, "row").or_else(|| reply(&r, "id")) {
        Some(Value::Text(k)) => k,
        Some(Value::I64(n)) => n.to_string(),
        other => panic!("no row in the reply: {other:?}"),
    }
}

#[test]
fn a_memory_scan_stops_on_the_busy_channel_switches_stacks_and_writes_hits_back() {
    let e = live_engine(Arc::new(MemStorage::new()));
    add_memory(&e, "quiet", 100.05e6, "meter");
    let busy = add_memory(&e, "busy", BUSY_HZ, "meter");
    add_memory(&e, "other", 100.08e6, "voice");
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let scan = e.find("service.scan").unwrap();
    for (k, x) in [
        ("settle", 0.05),
        ("dwell", 0.3),
        ("hang", 0.3),
        ("max_hold", 1.0),
    ] {
        e.owner(scan, |o| {
            let id = o.schema().id(k).unwrap();
            owner::apply(o, id, &Value::Duration(x))
        })
        .unwrap()
        .unwrap();
    }
    let n = call(
        &e,
        scan,
        "memories",
        &[("session", Value::Text(format!("session.s{}", s.to_u64())))],
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply(&n, "channels"), Some(Value::I64(3)));
    let feed = |want: &dyn Fn() -> bool| {
        wait_for(3.0 * T, || {
            let _ = e.push(s_feed, Samples::Iq(&tone(4800)));
            want().then_some(())
        })
    };
    assert!(
        feed(&|| tuned(&e, s) == BUSY_HZ).is_some(),
        "the scan holds the busy channel"
    );
    assert!(
        feed(&|| stack_of(&e, s) == "voice").is_some(),
        "the scan visits the memory of the other stack"
    );
    call(&e, scan, "stop", &[]).unwrap();
    assert!(
        feed(&|| stack_of(&e, s) == "meter").is_some(),
        "the stack of the session comes back"
    );
    let mem = e.find("service.memory").unwrap();
    let hits = feed(&|| {
        column(&e, mem, "memories", "hits")
            .into_iter()
            .any(|(k, v)| k == busy && hz(v) >= 1.0)
    });
    assert!(
        hits.is_some(),
        "the busy memory has its hit: {:?}",
        column(&e, mem, "memories", "hits")
    );
}

#[test]
fn recall_and_presets_tune() {
    let e = engine();
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let session = Value::Text(format!("session.s{}", s.to_u64()));
    let here = add_memory(&e, "Here", 446.00625e6, "meter");
    let mem = e.find("service.memory").unwrap();
    call(
        &e,
        mem,
        "recall",
        &[("row", Value::Text(here)), ("session", session.clone())],
    )
    .unwrap();
    assert!(wait_for(T, || (tuned(&e, s) == 446.00625e6).then_some(())).is_some());
    // A preset of another stack switches to it with its setting.
    let p = call(
        &e,
        mem,
        "preset.save",
        &[
            ("stack", Value::Text("voice".into())),
            ("name", Value::Text("Mine".into())),
            ("freq_hz", Value::Hz(100.5e6)),
        ],
    )
    .unwrap()
    .unwrap();
    let row = match reply(&p, "row").or_else(|| reply(&p, "id")) {
        Some(Value::Text(k)) => k,
        Some(Value::I64(n)) => n.to_string(),
        other => panic!("{other:?}"),
    };
    call(
        &e,
        mem,
        "item.set",
        &[
            ("row", Value::Text(row.clone())),
            ("part", Value::Choice(2)),
            ("owner", Value::Text("stack.voice".into())),
            ("key", Value::Text("gain".into())),
            ("value", Value::F64(3.0)),
        ],
    )
    .unwrap();
    call(
        &e,
        mem,
        "preset.apply",
        &[("row", Value::Text(row)), ("session", session.clone())],
    )
    .unwrap();
    let switched = wait_for(T, || {
        (stack_of(&e, s) == "voice" && tuned(&e, s) == 100.5e6).then_some(())
    });
    assert!(
        switched.is_some(),
        "the preset switches the stack and tunes"
    );
    let gain = e
        .owner(OwnerRef::Session(s), |o| {
            o.values()?.value(o.schema().id("gain").ok()?).cloned()
        })
        .unwrap();
    assert_eq!(gain, Some(Value::F64(3.0)));
    // What does not exist is refused.
    let none = call(
        &e,
        mem,
        "recall",
        &[("row", Value::Text("9999".into())), ("session", session)],
    );
    assert!(matches!(none, Err(CommandError::NoSuchRow { .. })));
}

#[test]
fn a_rule_tunes_and_records_the_session_of_its_event() {
    let e = engine();
    let auto = e.find("service.automation").unwrap();
    call(
        &e,
        auto,
        "rule.set",
        &[
            ("row", Value::Text("r1".into())),
            ("name", Value::Text("Move".into())),
            ("kinds", Value::Text("meter.block".into())),
            ("cooldown", Value::Duration(3600.0)),
        ],
    )
    .unwrap();
    for (action, freq) in [(2, "101000000"), (3, "")] {
        call(
            &e,
            auto,
            "action.add",
            &[
                ("rule", Value::Text("r1".into())),
                ("action", Value::Choice(action)),
                ("freq", Value::Text(freq.into())),
            ],
        )
        .unwrap();
    }
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let moved = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(4800)));
        (tuned(&e, s) == 101e6).then_some(())
    });
    assert!(moved.is_some(), "the rule tunes the session");
    // The rule's second action is carried out in the same tick as the first: the tick must end before the read.
    assert!(
        e.settle(std::time::Duration::from_secs_f64(T)),
        "the control thread completes a tick"
    );
    let kinds = column(&e, OwnerRef::Engine, "requests", "kind");
    let refused = column(&e, OwnerRef::Engine, "requests", "refused");
    let froms = column(&e, OwnerRef::Engine, "requests", "from");
    assert!(froms
        .iter()
        .all(|(_, f)| text(f.clone()) == "service.automation"));
    let record = kinds
        .iter()
        .find(|(_, k)| *k == Some(Value::Choice(7)))
        .map(|(row, _)| row.clone())
        .unwrap_or_else(|| panic!("a record request: {kinds:?} {refused:?}"));
    let why = refused.iter().find(|(k, _)| *k == record).unwrap();
    assert!(text(why.1.clone()).is_empty(), "{why:?}");
    let rec = e.find("service.record").unwrap();
    let sessions = column(&e, rec, "running", "session");
    assert!(
        sessions
            .iter()
            .any(|(_, v)| text(v.clone()) == format!("session.s{}", s.to_u64())),
        "{sessions:?}"
    );
}

#[test]
fn a_second_store_of_the_same_channel_updates_the_memory() {
    let e = engine();
    let mut v = defaults(&e, e.find("stack.meter").unwrap());
    v.set(v.schema().id("store").unwrap(), &Value::Bool(true))
        .unwrap();
    let s_feed = e.open_feed(48e3, 100e6);
    e.start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let mem = e.find("service.memory").unwrap();
    let both = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(4800)));
        let n = column(&e, OwnerRef::Engine, "requests", "done")
            .into_iter()
            .filter(|(_, d)| *d == Some(Value::Bool(true)))
            .count();
        (n == 2).then_some(())
    });
    assert!(both.is_some(), "both stores are carried out");
    let rows = column(&e, mem, "memories", "freq_hz");
    assert_eq!(rows.len(), 1, "{rows:?}");
}

#[test]
fn a_sweep_of_a_recording_fails_at_once_with_a_reason() {
    let storage = MemStorage::new();
    let meta = br#"{"global":{"core:datatype":"cf32_le","core:sample_rate":250000},
        "captures":[{"core:sample_start":0,"core:frequency":100000000.0}]}"#;
    let write = |name: &str, data: &[u8]| {
        storage
            .write(&sdrkit_core::StorePath::new(name).unwrap(), data)
            .unwrap();
    };
    write("rec.sigmf-meta", meta);
    write("rec.sigmf-data", &vec![0u8; 8 * 250_000]);
    let e = live_engine(Arc::new(storage));
    let kind = e.find("driver.file").unwrap();
    let mut v = defaults(&e, kind);
    v.set(v.schema().id("loop").unwrap(), &Value::Bool(true))
        .unwrap();
    let radio = e.open_radio("file", "rec.sigmf-meta", &v).unwrap();
    let range = e
        .owner(OwnerRef::Radio(radio), |o| {
            let id = o.schema().id("centre_hz").unwrap();
            owner::apply(o, id, &Value::Hz(90e6))
        })
        .unwrap();
    assert!(range.is_err(), "the recording refuses a new centre");
    let params = defaults(&e, e.find("stack.meter").unwrap());
    let s = e
        .start_stack("meter", None, &params, Input::Radio(radio), None)
        .unwrap();
    let sweep = e.find("service.sweep").unwrap();
    call(
        &e,
        sweep,
        "start",
        &[
            ("session", Value::Text(format!("session.s{}", s.to_u64()))),
            ("low_hz", Value::Hz(99.9e6)),
            ("high_hz", Value::Hz(100.1e6)),
        ],
    )
    .unwrap();
    let ended = wait_for(3.0 * T, || {
        matches!(state(&e, sweep, "state").as_str(), "done" | "failed").then_some(())
    });
    assert!(
        ended.is_some(),
        "the sweep ends: {}",
        state(&e, sweep, "state")
    );
    assert_eq!(state(&e, sweep, "state"), "failed");
    let log = e
        .owner(sweep, |o| o.results().log_lines().back().cloned())
        .unwrap();
    assert!(log.is_some_and(|l| !l.is_empty()), "the sweep says why");
    assert_eq!(number(&e, OwnerRef::Radio(radio), "centre_hz"), 100e6);
}

/// A receiver that stops on a fault of its own ends its session only: the session has failed, its problem carries the
/// message of the fault, and the engine starts another session after it.
#[test]
fn a_fault_in_a_receiver_fails_its_session_and_shows_as_its_problem() {
    let e = live_engine(Arc::new(MemStorage::new()));
    let r = tone_radio(&e, 250e3);
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s = e
        .start_stack("meter", None, &v, Input::Radio(r), None)
        .unwrap();
    e.set(OwnerRef::Stack(s), "gain", common::FAULT_GAIN)
        .unwrap();
    let problem = wait_for(6.0 * T, || {
        let p = text(field(&e, OwnerRef::Session(s), "session.problem"));
        (!p.is_empty()).then_some(p)
    })
    .expect("a problem");
    assert!(
        problem.contains("fault in the library") && problem.contains("the meter has no gain of 10"),
        "{problem}"
    );
    assert_eq!(state(&e, OwnerRef::Session(s), "session.state"), "failed");
    let again = e.start_stack("meter", None, &v, Input::Radio(r), None);
    assert!(again.is_ok(), "{again:?}");
}

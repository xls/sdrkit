//! The safe wrapper of the C API on an engine with its files in memory and the FM test signal.

use sdrkit_host_tui::api::{Handle, Value};
use sdrkit_host_tui::results::Results;
use std::time::{Duration, Instant};

/// Calls `f` every 10 ms until it returns `Some`, for at most 20 seconds.
fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let t0 = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn engine_settings_commands_and_snapshots() {
    let engine = Handle::engine_in_memory().expect("engine");
    let objects = engine.objects().expect("objects");
    assert!(objects.iter().any(|(k, _)| k == "service.scan"));

    // A setting set and read back; a refused value names its code.
    let height = engine.setting_find("height_m").expect("height_m");
    engine.set(height, &Value::F64(12.5)).expect("set");
    let mut v = Value::None;
    engine.get(height, &mut v).expect("get");
    assert_eq!(v, Value::F64(12.5));
    let e = engine
        .set(height, &Value::F64(1e9))
        .expect_err("out of range");
    assert_eq!(e.name(), "SDRKIT_E_OUT_OF_RANGE");
    assert!(!e.message.is_empty());

    // A command with a reply.
    let save = engine.command_find("state.save").expect("state.save");
    let reply = engine.invoke(save, &[]).expect("invoke");
    assert!(matches!(&reply[..], [Value::Bytes(b)] if !b.is_empty()));

    // A snapshot by counter: the second read after the counter holds no changed row.
    let mut r = Results::default();
    r.poll(&engine).expect("poll");
    let stacks = r.table_id("stacks").expect("stacks table");
    assert!(r.tables[stacks].rows.len() > 10);
    let mut summary = String::new();
    let info = engine
        .snapshot_read(r.counter, &mut summary)
        .expect("snapshot");
    assert_eq!(engine.snapshot_table(stacks as u32).expect("rows").rows, 0);
    assert!(info.counter >= r.counter);
}

#[test]
fn fm_session_audio_events_and_close() {
    let engine = Handle::engine_in_memory().expect("engine");
    let types = engine.event_types().expect("event types");
    let connected = types
        .iter()
        .position(|t| t.key == "radio.connected")
        .expect("radio.connected") as u32;
    let radio = engine.radio_open("synth:fm", &[]).expect("radio");
    let session = engine
        .stack_start("fm", None, Some(99.5e6), &[], &radio)
        .expect("session");
    let tuned = session.setting_find("tuned_hz").expect("tuned_hz");
    let mut got = Value::None;
    session.get(tuned, &mut got).expect("tuned");
    assert_eq!(got, Value::Hz(99.5e6));

    // The rows of the radios and sessions tables are keyed by the object key.
    let mut r = Results::default();
    r.poll(&engine).expect("poll");
    for (table, object) in [("radios", &radio), ("sessions", &session)] {
        let t = r.table_id(table).expect("table");
        let key = object.key().expect("key");
        assert!(r.tables[t].rows.iter().any(|row| row.key == key), "{table}");
    }
    let stack = session.stack().expect("stack");
    assert!(!stack.results_decl().expect("decl").fields.is_empty());

    let mut audio = vec![0.0f32; 9600];
    let got = wait_for(|| {
        let a = session.audio_read(&mut audio).expect("audio");
        (a.frames > 0).then_some(a)
    });
    assert!(got.channels >= 1 && got.rate_hz > 0.0);

    wait_for(|| {
        let n = radio.events_take().expect("take");
        (0..n)
            .any(|i| radio.event(i).expect("event").ty == connected)
            .then_some(())
    });

    session.session_stop().expect("stop");
    wait_for(|| match session.session_close() {
        Ok(()) => Some(()),
        Err(e) if e.name() == "SDRKIT_E_STILL_RUNNING" => None,
        Err(e) => panic!("close: {e}"),
    });
    radio.radio_close().expect("radio close");
    let e = radio.events_take().expect_err("dead handle");
    assert!(e.is_dead_handle());
    assert_eq!(e.name(), "SDRKIT_E_NO_SUCH_OBJECT");
}

//! The stacks table says how many channels of host audio the graph of each stack takes, before a session runs: 2 for
//! the stereo FM sender, 1 for a mono voice or TV sender, 0 for a stack that sends no host audio. Every stack whose
//! session asks for host audio after a transmit command has a count that is not 0.

mod common;

use common::{defaults, field, wait_for};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Value, ValueKind, Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts as IiodOpts};
use sdrkit_host_engine::{Engine, Input, OwnerRef, SessionId, TRANSMIT_ALLOWED};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The cell `col` of row `row` of the engine table `table`.
fn cell(e: &Engine, table: &str, row: &str, col: &str) -> Option<Value> {
    e.owner(OwnerRef::Engine, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table)?)?;
        let c = t.columns().id(col)?;
        t.row(row)?.value(c).cloned()
    })
    .unwrap()
}

/// The host audio channels of stack `id`.
fn channels(e: &Engine, id: &str) -> i64 {
    match cell(e, "stacks", id, "host_audio_channels") {
        Some(Value::I64(n)) => n,
        other => panic!("{id}: host_audio_channels is {other:?}"),
    }
}

#[test]
fn the_stacks_table_counts_the_host_audio_channels_of_each_graph() {
    let e = sdrkit_host_catalog::engine().unwrap();
    // The counts come from a thread of the engine after its start.
    wait_for(30.0, || (channels(&e, "fm") == 2).then_some(()))
        .expect("the host audio channels are counted");
    let mut counts: Vec<(String, i64)> = sdrkit_host_catalog::all()
        .iter()
        .map(|d| (d.id.to_owned(), channels(&e, d.id)))
        .filter(|(_, n)| *n != 0)
        .collect();
    counts.sort();
    println!("stacks with host audio: {counts:?}");
    let expect = [
        ("fm", 2),
        ("general", 1),
        ("m17", 1),
        ("freedv", 1),
        ("qo100", 1),
        ("atv", 1),
        ("fpv", 1),
        ("dvbt", 0),
        ("dvbs", 0),
        ("pager", 0),
        ("cubesat", 0),
        ("rtty", 0),
        ("file_transfer", 0),
        ("qam_video", 0),
        ("picture_burst", 0),
    ];
    for (id, n) in expect {
        assert_eq!(channels(&e, id), n, "{id}");
    }
}

/// A transmit command and the command that stops its transmission, if one does.
type TxCommand = (String, Option<String>);

/// The stacks of the table `stack_transmits`, each with its transmit commands.
fn transmit_commands(e: &Engine) -> Vec<(String, Vec<TxCommand>)> {
    let rows: Vec<(String, Option<String>)> = e
        .owner(OwnerRef::Engine, |o| {
            let res = o.results();
            let t = res
                .table(res.decl().table_id("stack_transmits").unwrap())
                .unwrap();
            let stop = t.columns().id("stopped_by").unwrap();
            t.rows_newest_first()
                .map(|(k, r)| {
                    let by = match r.value(stop) {
                        Some(Value::Text(c)) => Some(c.to_string()),
                        _ => None,
                    };
                    (k.to_owned(), by)
                })
                .collect()
        })
        .unwrap();
    let mut by_stack: Vec<(String, Vec<TxCommand>)> = Vec::new();
    for (k, stop) in rows {
        let (stack, command) = k.split_once('/').unwrap();
        let c = (command.to_owned(), stop);
        match by_stack.iter_mut().find(|(s, _)| s == stack) {
            Some((_, cs)) => cs.push(c),
            None => by_stack.push((stack.to_owned(), vec![c])),
        }
    }
    by_stack.sort();
    by_stack
}

/// Settings that a sender refuses to start without, with a test value: the own call sign and the PI code of FM.
const STATION: [(&str, &str); 2] = [("callsign", "VK2TST"), ("tx_pi", "1234")];

/// Runs command `key` of session `s` with its default arguments, every yes-or-no argument set to yes and every empty
/// text set to a short text. A refusal is no error here.
fn invoke(e: &Engine, s: OwnerRef, key: &str) {
    let _ = e.owner(s, |o| {
        let id = o.commands().id(key).ok()?;
        let mut args = o.commands().def(id)?.new_args();
        let fields: Vec<(String, ValueKind)> = args
            .decl()
            .fields()
            .iter()
            .map(|f| (f.key.to_string(), f.kind))
            .collect();
        for (k, kind) in fields {
            let a = args.decl().id(&k)?;
            let v = match kind {
                ValueKind::Bool => Value::Bool(true),
                ValueKind::Text => Value::Text("TEST".into()),
                _ => continue,
            };
            let _ = args.set_value(a, &v);
        }
        sdrkit_core::owner::invoke(o, id, &args).ok()
    });
}

/// True if session `s` asks for host audio within `seconds`.
fn asks(e: &Engine, s: SessionId, seconds: f64) -> bool {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < seconds {
        if field(e, OwnerRef::Session(s), "session.audio_in_wanted") == Some(Value::Bool(true)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Each stack that has a transmit command starts on a fake radio that transmits, with the transmit switch on, and
/// runs its transmit commands one by one, each stopped by its stop command. A stack whose session then asks for host
/// audio has host audio channels.
#[test]
fn every_stack_that_asks_for_host_audio_after_a_transmit_command_counts_its_channels() {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(
        net.clone(),
        IiodOpts {
            tx_plays: true,
            lo_range: "[100000 1 6000000000]",
            ..IiodOpts::default()
        },
    );
    let e = sdrkit_host_catalog::engine_on(Arc::new(net.clone())).unwrap();
    let kind = e.find("driver.iiod").unwrap();
    let r = e
        .open_radio("iiod", &srv.addr(), &defaults(&e, kind))
        .unwrap();
    e.owner(OwnerRef::Engine, |o| {
        let id = o.schema().id(TRANSMIT_ALLOWED.key()).unwrap();
        sdrkit_core::owner::apply(o, id, &Value::Bool(true))
    })
    .unwrap()
    .unwrap();
    let mut asked = Vec::new();
    let mut not_started = Vec::new();
    for (stack, commands) in transmit_commands(&e) {
        let d = sdrkit_host_catalog::all()
            .into_iter()
            .find(|d| d.id == stack)
            .unwrap();
        let values = Values::defaults(&(d.settings)());
        let Ok(s) = e.start_stack(d.id, None, &values, Input::Radio(r), None) else {
            not_started.push(stack);
            continue;
        };
        // A session whose graph takes host audio has a host audio rate once the graph runs.
        let _ = wait_for(2.0, || {
            field(&e, OwnerRef::Session(s), "session.audio_in_rate_hz")
                .filter(|v| matches!(v, Value::Hz(_)))
        });
        for (key, v) in STATION {
            let _ = e.owner(OwnerRef::Session(s), |o| {
                let id = o.schema().id(key).ok()?;
                sdrkit_core::owner::apply(o, id, &Value::Text(v.into())).ok()
            });
        }
        let wanted = commands.iter().any(|(c, stop)| {
            invoke(&e, OwnerRef::Session(s), c);
            let asked = asks(&e, s, 0.6);
            if let Some(stop) = stop {
                invoke(&e, OwnerRef::Session(s), stop);
            }
            asked
        });
        if wanted {
            assert_ne!(channels(&e, &stack), 0, "{stack} asks for host audio");
            asked.push(stack);
        }
        e.stop_session(s).unwrap();
        let _ = wait_for(5.0, || e.close_session(s).ok());
    }
    println!("ask for host audio: {asked:?}; not started: {not_started:?}");
    // The voice senders without Codec 2 (the engine of the test has no Codec 2 tables, so M17 and FreeDV refuse
    // push-to-talk), and the FM and TV senders, whose `start` sends the host audio.
    for id in ["airband", "atv", "fm", "fpv", "general", "qo100"] {
        assert!(
            asked.iter().any(|s| s == id),
            "{id} did not ask for host audio"
        );
    }
}

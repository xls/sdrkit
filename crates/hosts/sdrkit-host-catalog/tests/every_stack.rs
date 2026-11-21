//! Every stack of the catalog, on its own test signal with its default settings, on its default frequency or on one
//! that the host gives: it starts, it runs without a problem, each of its commands answers a call without arguments
//! with a result or with a refusal that names what is missing (an argument, or the transmitter that the test-signal
//! radio does not have), and the band scan of the scanner takes it.
//!
//! The test-signal radio is opened at the rate that the stack declares for its default settings. The stacks run one
//! after the other on one engine.

use sdrkit_core::net::FakeTransport;
use sdrkit_core::{LockState, StackDescriptor, Value, ValueKind, Values};
use sdrkit_driver_core::{CENTRE_GIVEN, RATE, RATE_GIVEN};
use sdrkit_host_engine::{Engine, EngineError, OwnerRef, RadioId, SessionId};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The stacks of the catalog that this test cannot run offline, each with the reason.
const NOT_OFFLINE: &[(&str, &str)] = &[
    (
        "tv-auto",
        "it has no test signal: it detects the standard of a TV channel and asks the host to start the stack of that \
         standard",
    ),
    (
        "hamdrm",
        "its test signal needs the HamDRM format file (setting hamdrm_format), which is not part of the library",
    ),
];

/// What one stack showed.
#[derive(Debug)]
struct Seen {
    stack: &'static str,
    /// Why the stack did not start, or the problem of its session.
    problem: Option<String>,
    /// The lock state at the end of the listening time.
    lock: Option<LockState>,
    /// The time from the start of the session to the first `Locked`, if it locked.
    locked_after: Option<Duration>,
    /// The samples that the session did not take in the listening time.
    dropped: i64,
    /// The centre and the rate of the radio and the frequency of the session after the listening time, Hz.
    radio: (f64, f64),
    tuned: f64,
    /// Each command that a call without arguments did not run, with the refusal.
    refused: Vec<(String, String)>,
    /// The channels that the scan took, or why it did not start.
    scan: Result<i64, String>,
    /// Columns of frequencies that are not named as a frequency to tune: `table.column`.
    hz_columns: Vec<String>,
    /// Why the radio did not close after the session.
    close: Option<String>,
    /// The time that the stack took in this test.
    took: Duration,
}

fn engine() -> Engine {
    sdrkit_host_catalog::engine_on(Arc::new(FakeTransport::new())).unwrap()
}

/// Opens the test-signal radio of stack `d` at the rate that the stack declares for its default settings.
fn open_at_stack_rate(sdr: &Engine, d: &StackDescriptor) -> Result<RadioId, EngineError> {
    let rate = d.rate_hz(&Values::defaults(&(d.settings)()));
    let schema = sdr.radio_schema("synth", d.id)?;
    let mut values = Values::defaults(&schema);
    let id = EngineError::id_of(&schema, RATE.field.key)?;
    values
        .set(id, &Value::Hz(rate))
        .map_err(|e| EngineError::setting(&schema, e))?;
    sdr.open_radio("synth", d.id, &values)
}

/// Runs stack `d` on its test signal on `sdr` for at most `listen`, or until it locks if `until_lock`.
fn look(sdr: &Engine, d: &'static StackDescriptor, listen: Duration, until_lock: bool) -> Seen {
    let began = Instant::now();
    let stack = d.id;
    let mut seen = Seen {
        stack,
        problem: None,
        lock: None,
        locked_after: None,
        dropped: 0,
        radio: (0.0, 0.0),
        tuned: 0.0,
        refused: Vec::new(),
        scan: Err(String::new()),
        hz_columns: Vec::new(),
        close: None,
        took: Duration::ZERO,
    };
    // A stack without a default frequency gets one from the host: here the middle of its range.
    let t = &d.tuning;
    let hz = (t.default_hz <= 0.0 && t.max_hz > t.min_hz).then(|| (t.min_hz + t.max_hz) / 2.0);
    let radio = match open_at_stack_rate(sdr, d) {
        Ok(r) => r,
        Err(e) => {
            seen.problem = Some(format!("did not open: {e}"));
            seen.took = began.elapsed();
            return seen;
        }
    };
    let session: SessionId = match sdr.start(stack, radio, hz) {
        Ok(s) => s,
        Err(e) => {
            seen.problem = Some(format!("did not start: {e}"));
            seen.close = sdr.close_radio(radio).err().map(|e| e.to_string());
            seen.took = began.elapsed();
            return seen;
        }
    };
    let started = Instant::now();
    while started.elapsed() < listen {
        std::thread::sleep(Duration::from_millis(100));
        seen.lock = sdr
            .owner(OwnerRef::Session(session), |o| {
                o.results().status().lock_state
            })
            .ok();
        if seen.lock == Some(LockState::Locked) {
            seen.locked_after.get_or_insert(started.elapsed());
            if until_lock {
                break;
            }
        }
    }
    seen.problem = sdr.problem(session).unwrap_or_else(|e| Some(e.to_string()));
    seen.dropped = sdr
        .owner(OwnerRef::Session(session), |o| {
            o.results().fields().get(&sdrkit_host_engine::DROPPED)
        })
        .ok()
        .flatten()
        .unwrap_or(0);
    seen.radio = sdr
        .owner(OwnerRef::Radio(radio), |o| {
            let f = o.results().fields();
            (
                f.get(&CENTRE_GIVEN).unwrap_or(0.0),
                f.get(&RATE_GIVEN).unwrap_or(0.0),
            )
        })
        .unwrap_or((0.0, 0.0));
    seen.tuned = sdr
        .owner(OwnerRef::Session(session), |o| {
            o.values()
                .map_or(0.0, |v| v.get(&sdrkit_host_engine::TUNED))
        })
        .unwrap_or(0.0);
    let owner = OwnerRef::Stack(session);
    let commands: Vec<(String, bool)> = sdr
        .owner(owner, |o| {
            o.commands()
                .defs()
                .iter()
                .map(|d| (d.key.to_string(), d.transmits.is_some()))
                .collect()
        })
        .unwrap_or_default();
    for (key, transmits) in commands {
        if let Err(e) = sdr.call(owner, &key, &[]) {
            // A command that transmits is refused by the engine: the radio of the test signal has no transmitter.
            let text = e.to_string();
            if !(transmits && text.contains("transmit is off")) {
                seen.refused.push((key, text));
            }
        }
    }
    seen.hz_columns = sdr
        .owner(owner, |o| {
            let decl = o.results_decl();
            decl.tables()
                .iter()
                .flat_map(|t| {
                    t.columns
                        .fields()
                        .iter()
                        .filter(|c| {
                            c.kind == ValueKind::Hz
                                && !(c.key.starts_with("freq") || c.key == "centre")
                        })
                        .map(move |c| format!("{}.{}", t.key, c.key))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();
    let scan = sdr.find("service.scan").unwrap();
    let key = sdr.key_of(OwnerRef::Session(session)).unwrap();
    // The scan of a wide range goes around what the session receives, which the scanner knows a moment after the
    // first samples of the session. The call is made again until then, for 10 s at most.
    let asked = Instant::now();
    seen.scan = loop {
        let reply = sdr
            .call(
                scan,
                "start",
                &[
                    ("session", Value::Text(key.clone())),
                    ("stack", Value::Text(stack.into())),
                ],
            )
            .map(
                |r| match r.as_ref().and_then(|r| r.value(r.decl().id("channels")?)) {
                    Some(Value::I64(n)) => *n,
                    _ => -1,
                },
            )
            .map_err(|e| e.to_string());
        if reply.is_ok() || asked.elapsed() > Duration::from_secs(10) {
            break reply;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = sdr.call(scan, "stop", &[]);
    // A problem that came after the listening time counts too.
    if seen.problem.is_none() {
        seen.problem = sdr.problem(session).unwrap_or_else(|e| Some(e.to_string()));
    }
    // The radio closes once its session is closed: a radio left open keeps its signal running for the next stacks.
    seen.close = sdr
        .stop_session(session)
        .and_then(|()| sdr.close_session(session))
        .and_then(|()| sdr.close_radio(radio))
        .err()
        .map(|e| e.to_string());
    seen.took = began.elapsed();
    seen
}

/// The stacks of the catalog that the test runs: all but those of [`NOT_OFFLINE`].
fn runnable() -> Vec<&'static StackDescriptor> {
    sdrkit_host_catalog::all()
        .into_iter()
        .filter(|d| !NOT_OFFLINE.iter().any(|(id, _)| *id == d.id))
        .collect()
}

/// Looks at `stacks` one after the other on one engine.
fn look_at(stacks: &[&'static StackDescriptor], listen: Duration, until_lock: bool) -> Vec<Seen> {
    let sdr = engine();
    stacks
        .iter()
        .map(|d| look(&sdr, d, listen, until_lock))
        .collect()
}

#[test]
fn every_stack_starts_runs_its_commands_and_takes_a_scan() {
    let all = sdrkit_host_catalog::all();
    let mut faults = Vec::new();
    // Each entry of the list names a stack of the catalog, and every other stack has a test signal.
    for (id, _) in NOT_OFFLINE {
        if !all.iter().any(|d| d.id == *id) {
            faults.push(format!(
                "{id}: listed as not offline, but the catalog has no such stack"
            ));
        }
    }
    for d in &all {
        if d.make_generator.is_none() && !NOT_OFFLINE.iter().any(|(id, _)| *id == d.id) {
            faults.push(format!(
                "{}: has no test signal and is not listed as not offline",
                d.id
            ));
        }
    }
    // A listed stack that has a test signal still cannot open it: else the entry is out of date.
    let sdr = engine();
    for d in all.iter().filter(|d| d.make_generator.is_some()) {
        if NOT_OFFLINE.iter().any(|(id, _)| *id == d.id) {
            if let Ok(r) = open_at_stack_rate(&sdr, d) {
                let _ = sdr.close_radio(r);
                faults.push(format!(
                    "{}: listed as not offline, but its test signal opens",
                    d.id
                ));
            }
        }
    }
    drop(sdr);
    let began = Instant::now();
    let stacks = runnable();
    let seen = look_at(&stacks, Duration::from_millis(1500), true);
    for s in &seen {
        println!(
            "STACK {} took={:.1}s lock={:?} locked_after={:?} dropped={}",
            s.stack,
            s.took.as_secs_f64(),
            s.lock,
            s.locked_after,
            s.dropped
        );
        if let Some(c) = &s.close {
            faults.push(format!("{} did not close: {c}", s.stack));
        }
        if let Some(p) = &s.problem {
            faults.push(format!("{}: {p}", s.stack));
            continue;
        }
        // A call without arguments runs, or it is refused for an argument that it names. A command that sends without a
        // declared content is refused by its stack: the engine keeps the stack's transmitter off, as the radio of the
        // test signal has none.
        for (command, refusal) in &s.refused {
            let no_transmitter = refusal.ends_with("the transmitter is off");
            if !refusal.contains("needs its argument") && !no_transmitter {
                faults.push(format!("{} command {command}: {refusal}", s.stack));
            }
        }
        match &s.scan {
            Ok(n) if *n > 0 => {}
            other => faults.push(format!("{} scan: {other:?}", s.stack)),
        }
    }
    println!(
        "every_stack: {} stacks in {:.0} s, {} not offline",
        seen.len(),
        began.elapsed().as_secs_f64(),
        NOT_OFFLINE.len()
    );
    assert!(
        faults.is_empty(),
        "{} faults:\n{}",
        faults.len(),
        faults.join("\n")
    );
}

/// Prints what every stack shows, for a person to read: `cargo test --test every_stack -- --ignored --nocapture`.
/// `SDRKIT_LISTEN_MS` sets how long each stack listens to its test signal before its lock state is read.
/// `SDRKIT_STACKS` names the stacks to look at, with commas between them; `SDRKIT_SKIP` names stacks to leave out.
#[test]
#[ignore = "a report for a person"]
fn report() {
    let listen = std::env::var("SDRKIT_LISTEN_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2500);
    let only = std::env::var("SDRKIT_STACKS").unwrap_or_default();
    let skip = std::env::var("SDRKIT_SKIP").unwrap_or_default();
    let named = |list: &str, id: &str| list.split(',').any(|s| s == id);
    let stacks: Vec<_> = runnable()
        .into_iter()
        .filter(|d| (only.is_empty() || named(&only, d.id)) && !named(&skip, d.id))
        .collect();
    for (id, why) in NOT_OFFLINE {
        println!("NOT OFFLINE {id}: {why}");
    }
    for s in look_at(&stacks, Duration::from_millis(listen), false) {
        println!(
            "STACK {} lock={:?} locked_after={:?} dropped={} problem={:?} scan={:?} hz={:?} took={:.1}s",
            s.stack,
            s.lock,
            s.locked_after,
            s.dropped,
            s.problem,
            s.scan,
            s.hz_columns,
            s.took.as_secs_f64()
        );
        for (c, e) in &s.refused {
            println!("  CMD {} {c}: {e}", s.stack);
        }
        // Where the stack put the radio and itself: one line that a later run is compared with.
        println!(
            "PICTURE {} centre={} rate={} tuned={}",
            s.stack, s.radio.0, s.radio.1, s.tuned
        );
    }
}

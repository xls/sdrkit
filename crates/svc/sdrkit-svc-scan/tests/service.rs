//! The scan service as an owner: tables and commands by key, checked arguments, files through the host's storage, and scans
//! that the engine steps.

use sdrkit_core::command::CommandError;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{
    Choice, Clock, HostRequest, LockProfile, LockState, MemStorage, Record, Schema, StackConfig,
    StackDescriptor, Status, Storage, StorageError, StorePath, Tuning, Value,
};
use sdrkit_svc_memory::{Memory, MemoryFilter, MemoryService};
use sdrkit_svc_scan::*;
use std::sync::{Arc, Mutex};

static DAB: StackDescriptor = StackDescriptor {
    id: "dab",
    name: "DAB",
    group: "Broadcast",
    blurb: "Digital radio.",
    tuning: Tuning::anywhere(225e6, 2.048e6, 2.048e6, 1.536e6),
    submodes: &[],
    settings: Arc::<Schema>::default,
    make_receiver:
        |_: &StackConfig| -> Result<Box<dyn sdrkit_core::Receiver>, sdrkit_core::ConfigError> {
            unreachable!("the scan service never makes a receiver")
        },
    make_generator: None,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: sdrkit_core::AutoGain::Radio,
    verify: None,
    benchmarks: None,
};

/// A stack with a band of three channels of 1 MHz.
static BAND: StackDescriptor = StackDescriptor {
    id: "band",
    tuning: Tuning {
        min_hz: 174e6,
        max_hz: 176e6,
        ..Tuning::anywhere(175e6, 2.048e6, 2.048e6, 1e6)
    },
    ..DAB
};

fn stacks(id: &str) -> Option<&'static StackDescriptor> {
    [&DAB, &BAND].into_iter().find(|d| d.id == id)
}

/// The lookup of the memories that the engine supplies: the memories of `m` that a filter takes.
fn lookup(m: &Arc<Mutex<MemoryService>>) -> MemoryLookup {
    let m = Arc::clone(m);
    Arc::new(move |f: &MemoryFilter| {
        m.lock()
            .unwrap()
            .store()
            .filter(f)
            .into_iter()
            .cloned()
            .collect()
    })
}

struct Fixture {
    scan: ScanService,
    memories: Arc<Mutex<MemoryService>>,
    storage: Arc<MemStorage>,
}

fn fixture() -> Fixture {
    let storage = Arc::new(MemStorage::new());
    let clock = Clock::fixed(1_000_000.0);
    let memories = Arc::new(Mutex::new(
        MemoryService::open(storage.clone(), clock.clone()).unwrap(),
    ));
    let scan =
        ScanService::open(storage.clone(), clock, Arc::new(stacks), lookup(&memories)).unwrap();
    Fixture {
        scan,
        memories,
        storage,
    }
}

fn call(
    s: &mut ScanService,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = s.commands().id(key)?;
    let mut given = s.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let fid = given.decl().id(k).unwrap();
        given.set_value(fid, v).unwrap();
    }
    owner::invoke(s, id, &given)
}

fn text(v: &str) -> Value {
    Value::Text(v.into())
}

fn rows(s: &ScanService, key: &str) -> Vec<Record> {
    let id = s.results_decl().table_id(key).unwrap();
    s.results()
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(_, r)| r.clone())
        .collect()
}

fn cell<'a>(r: &'a Record, key: &str) -> &'a Value {
    r.value(r.decl().id(key).unwrap()).unwrap()
}

fn field<'a>(s: &'a ScanService, key: &str) -> &'a Value {
    s.results()
        .fields()
        .value(s.results_decl().field_id(key).unwrap())
        .unwrap()
}

fn reply(r: &Record, key: &str) -> Value {
    cell(r, key).clone()
}

#[test]
fn the_service_is_an_owner_and_checks_arguments_first() {
    let Fixture { mut scan, .. } = fixture();
    assert_eq!(KEY.as_str(), "service.scan");
    for t in [
        "entries",
        "entry_settings",
        "hits",
        "hit_values",
        "table",
        "table_settings",
        "table_values",
        "lockouts",
        "problems",
    ] {
        assert!(scan.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in [
        "memories", "entries", "band", "channels", "start", "stop", "control", "import", "export",
        "table", "store",
    ] {
        assert!(scan.commands().id(c).is_ok(), "{c}");
    }
    assert!(matches!(
        scan.commands().id("nope"),
        Err(CommandError::Unknown { .. })
    ));
    let e = call(&mut scan, "band", &[("session", text("1"))]).unwrap_err();
    assert!(matches!(e, CommandError::MissingArgument { .. }), "{e:?}");
    let e = call(
        &mut scan,
        "table",
        &[("stack", text("dab")), ("limit", Value::I64(0))],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    let e = call(
        &mut scan,
        "channels",
        &[
            ("session", text("1")),
            ("stack", text("nope")),
            ("start_hz", Value::Hz(174e6)),
            ("stop_hz", Value::Hz(175e6)),
            ("step_hz", Value::Hz(1e6)),
        ],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    assert!(scan.session().is_none(), "nothing ran");
    let e = call(&mut scan, "entry.remove", &[("row", text("7"))]).unwrap_err();
    assert_eq!(
        e,
        CommandError::NoSuchRow {
            table: scan.results_decl().table_id("entries").unwrap(),
            key: "7".into()
        }
    );
    let e = call(
        &mut scan,
        "control",
        &[("action", Value::Choice(Action::Skip.index()))],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::NotNow { .. }), "{e:?}");
}

#[test]
fn an_import_with_one_bad_line_returns_the_count_and_one_problem() {
    let Fixture {
        mut scan, storage, ..
    } = fixture();
    let file = StorePath::new("in/list.csv").unwrap();
    storage
        .write(
            &file,
            b"145.5 MHz,general,A\nbad,general,B\n145.6 MHz,general,C\n",
        )
        .unwrap();
    let r = call(&mut scan, "import", &[("path", text("in/list.csv"))])
        .unwrap()
        .unwrap();
    assert_eq!(reply(&r, "added"), Value::I64(2));
    assert_eq!(reply(&r, "problems"), Value::I64(1));
    let problems = rows(&scan, "problems");
    assert_eq!(problems.len(), 1);
    assert_eq!(cell(&problems[0], "line"), &Value::I64(2));
    assert_eq!(
        cell(&problems[0], "kind"),
        &Value::Choice(ScanProblem::NotAFrequency.index())
    );
    assert_eq!(rows(&scan, "entries").len(), 2);
    let e = call(&mut scan, "import", &[("path", text("../x"))]).unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
}

#[test]
fn files_go_through_storage_and_a_failure_is_typed() {
    let Fixture {
        mut scan,
        storage,
        memories,
    } = fixture();
    storage
        .write(&StorePath::new("l.csv").unwrap(), b"145.5 MHz,general,A\n")
        .unwrap();
    call(&mut scan, "import", &[("path", text("l.csv"))]).unwrap();
    let again = ScanService::open(
        storage.clone(),
        Clock::fixed(0.0),
        Arc::new(stacks),
        lookup(&memories),
    )
    .unwrap();
    assert_eq!(rows(&again, "entries").len(), 1, "the list is kept");
    storage.fail_next(StorageError::NoSpace);
    let e = call(&mut scan, "entry.remove", &[("row", text("0"))]).unwrap_err();
    let CommandError::Failed { source } = e else {
        panic!("{e:?}")
    };
    assert!(matches!(
        source.downcast_ref::<ScanError>(),
        Some(ScanError::Storage {
            source: StorageError::NoSpace,
            ..
        })
    ));
    storage.fail_next(StorageError::Denied);
    assert!(matches!(
        ScanService::open(
            storage,
            Clock::fixed(0.0),
            Arc::new(stacks),
            lookup(&memories)
        ),
        Err(ScanError::Storage {
            source: StorageError::Denied,
            ..
        })
    ));
}

const SQUELCH: Field<field::Bool> = Field::new("squelch_open", "Squelch open", "");
const LABEL: Field<field::Text> = Field::new("ensemble_label", "Ensemble", "");

/// Runs the scan that the service holds like the engine: a step every 50 ms, with `busy` telling whether the tuned channel
/// is active. Returns the frequencies tuned.
fn run(scan: &mut ScanService, until_s: f64, busy: impl Fn(f64, f64) -> bool) -> Vec<f64> {
    let mut p = Published::new(&[&SQUELCH, &LABEL]);
    let mut tuned = Vec::new();
    let mut freq = 0.0;
    let mut t = 0.0;
    while t < until_s {
        let open = busy(freq, t);
        p.set(&SQUELCH, open);
        p.results_mut()
            .set_status(&Status::new(LockState::Searching));
        match scan.step(t, Some(p.results())) {
            Some(Step::Tune) => {
                freq = scan.target().unwrap().freq_hz;
                tuned.push(freq);
            }
            Some(Step::Done) => break,
            None => {}
        }
        t += 0.05;
    }
    tuned
}

#[test]
fn a_memory_scan_fills_the_hits_and_asks_to_count_them() {
    let Fixture {
        mut scan, memories, ..
    } = fixture();
    let id = memories
        .lock()
        .unwrap()
        .update_store(|s| {
            s.insert(Memory::new("Repeater", 145.6e6, "general"), 0.0)
                .unwrap()
        })
        .unwrap();
    memories
        .lock()
        .unwrap()
        .update_store(|s| s.insert(Memory::new("Quiet", 145.8e6, "general"), 0.0))
        .unwrap()
        .unwrap();
    let r = call(&mut scan, "memories", &[("session", text("1"))])
        .unwrap()
        .unwrap();
    assert_eq!(reply(&r, "channels"), Value::I64(2));
    assert_eq!(scan.session(), Some("1"));
    let tuned = run(&mut scan, 6.0, |f, t| {
        f == 145.6e6 && (1.0..2.0).contains(&t)
    });
    assert!(tuned.contains(&145.8e6));
    assert_eq!(
        field(&scan, "kind"),
        &Value::Choice(ScanKind::Activity.index())
    );
    call(&mut scan, "stop", &[]).unwrap();
    assert!(scan.session().is_none());
    assert_eq!(
        field(&scan, "end"),
        &Value::Choice(EndReason::Stopped.index())
    );
    let hits = rows(&scan, "hits");
    assert_eq!(hits.len(), 1);
    assert_eq!(cell(&hits[0], "freq_hz"), &Value::Hz(145.6e6));
    assert_eq!(cell(&hits[0], "memory"), &text(&id.to_string()));
    let asked = scan.take_requests();
    assert!(
        matches!(&asked[..], [HostRequest::MemoryHit { memory, .. }] if *memory == id.to_string()),
        "{asked:?}"
    );
    assert_eq!(memories.lock().unwrap().store().get(id).unwrap().hits, 0);
}

#[test]
fn a_channel_scan_keeps_its_table_and_stores_it_as_memories() {
    let Fixture {
        mut scan, memories, ..
    } = fixture();
    let r = call(
        &mut scan,
        "channels",
        &[
            ("session", text("1")),
            ("stack", text("dab")),
            ("start_hz", Value::Hz(174e6)),
            ("stop_hz", Value::Hz(176e6)),
            ("step_hz", Value::Hz(1e6)),
        ],
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply(&r, "channels"), Value::I64(3));
    // No channel locks: the default profile leaves each one after its quiet time.
    let tuned = run(&mut scan, 30.0, |_, _| false);
    assert_eq!(tuned, vec![174e6, 175e6, 176e6]);
    assert!(scan.session().is_none(), "the scan ended by itself");
    assert_eq!(
        field(&scan, "end"),
        &Value::Choice(EndReason::Complete.index())
    );
    call(&mut scan, "table", &[("stack", text("dab"))]).unwrap();
    assert_eq!(field(&scan, "total"), &Value::I64(0), "nothing locked");
    let r = call(&mut scan, "store", &[("stack", text("dab"))])
        .unwrap()
        .unwrap();
    assert_eq!(reply(&r, "stored"), Value::I64(0));
    assert!(scan.take_requests().is_empty());
    assert!(memories.lock().unwrap().store().is_empty());
}

#[test]
fn start_scans_the_band_of_the_stack() {
    let Fixture { mut scan, .. } = fixture();
    let r = call(
        &mut scan,
        "start",
        &[("session", text("1")), ("stack", text("band"))],
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply(&r, "channels"), Value::I64(3));
    let tuned = run(&mut scan, 30.0, |_, _| false);
    assert_eq!(tuned, vec![174e6, 175e6, 176e6]);
    let e = call(
        &mut scan,
        "start",
        &[("session", text("1")), ("stack", text("tv"))],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
}

#[test]
fn start_on_a_stack_that_tunes_anywhere_scans_the_channels_of_the_band_plans() {
    let Fixture { mut scan, .. } = fixture();
    let start = |scan: &mut ScanService| {
        call(
            scan,
            "start",
            &[("session", text("session.s1")), ("stack", text("dab"))],
        )
    };
    let mut plans = PlanChannels::new();
    plans.insert(
        "dab".into(),
        vec![("5A".into(), 174.928e6), ("5B".into(), 176.64e6)],
    );
    scan.set_plan_channels(plans);
    let r = start(&mut scan).unwrap().unwrap();
    assert_eq!(reply(&r, "channels"), Value::I64(2));
    let tuned = run(&mut scan, 30.0, |_, _| false);
    assert_eq!(tuned, vec![174.928e6, 176.64e6]);
}

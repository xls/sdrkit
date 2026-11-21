//! The sweep service read as an owner: its declarations by key, the checks of the framework, row keys, pages and files.

mod common;

use common::fake::{FakeRadio, FakeSignal};
use common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::settings::SettingError;
use sdrkit_core::{CommandError, Storage, StorageError, StorePath, Value};
use sdrkit_svc_sweep::{SweepError, SweepService, KEY};

/// A service after twelve sweeps of a range with five carriers.
fn swept() -> (SweepService, std::sync::Arc<sdrkit_core::MemStorage>) {
    let (mut s, st) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.001));
    set(&mut s, "sweeps", Value::I64(12));
    let mut radio = FakeRadio::new(RATE, 145e6, 5);
    for hz in [144.25e6, 144.6e6, 145.35e6, 145.7e6, 145.95e6] {
        radio.add(FakeSignal::carrier(hz, -50.0));
    }
    let mut d = Driver::new(radio);
    d.start(
        &mut s,
        &[("low_hz", Value::Hz(144e6)), ("high_hz", Value::Hz(146e6))],
    );
    d.run(&mut s, 30.0).unwrap();
    (s, st)
}

#[test]
fn tables_commands_and_settings_are_found_by_key() {
    let (s, _) = service();
    assert_eq!(KEY.as_str(), "service.sweep");
    for t in [
        "findings",
        "bursts",
        "diffs",
        "drift",
        "interference",
        "persistent",
        "quiet",
        "presets",
        "preset_bands",
        "recordings",
        "advice",
        "health_bands",
    ] {
        assert!(s.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in [
        "start",
        "stop",
        "finish",
        "hit_action",
        "exclude",
        "reset_max",
        "hunt_target",
        "relearn",
        "burst_clear",
        "findings",
        "quiet",
        "diff",
        "export",
        "floor.save",
        "floor.restore",
    ] {
        assert!(s.commands().id(c).is_ok(), "{c}");
    }
    for k in ["bin_hz", "threshold_db", "cpu_load", "record", "sweeps"] {
        assert!(s.schema().id(k).is_ok(), "{k}");
    }
    let kinds: Vec<_> = s.events().iter().map(|e| e.kind.as_ref()).collect();
    assert_eq!(
        kinds,
        [
            "sweep.hit",
            "sweep.burst",
            "sweep.interference",
            "sweep.save"
        ]
    );
    // The presets are rows, each with its band.
    let presets = rows(&s, "presets");
    assert!(presets.iter().any(|(k, _)| *k == "2 m"));
    assert_eq!(rows(&s, "preset_bands").len(), presets.len());
}

#[test]
fn a_bad_argument_is_refused_before_the_service_runs() {
    let (mut s, _) = service();
    // No session: the framework refuses it, and nothing starts.
    let e = call(&mut s, "start", &[("low_hz", Value::Hz(1e8))]).unwrap_err();
    assert!(matches!(e, CommandError::MissingArgument { .. }), "{e:?}");
    // A limit out of range.
    let e = call(&mut s, "findings", &[("limit", Value::I64(0))]).unwrap_err();
    assert!(
        matches!(
            e,
            CommandError::BadArgument {
                error: SettingError::OutOfRange { .. },
                ..
            }
        ),
        "{e:?}"
    );
    // A frequency below 0: refused before the service sees that no hunt runs.
    let e = call(&mut s, "hunt_target", &[("hz", Value::Hz(-5.0))]).unwrap_err();
    assert!(
        matches!(
            e,
            CommandError::BadArgument {
                error: SettingError::OutOfRange { .. },
                ..
            }
        ),
        "{e:?}"
    );
    assert_eq!(field_choice(&s, "state"), "idle");
    // The service refuses an empty range itself.
    let e = call(
        &mut s,
        "start",
        &[
            ("session", text("1")),
            ("low_hz", Value::Hz(2e8)),
            ("high_hz", Value::Hz(1e8)),
        ],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    // No sweep: not now.
    assert!(matches!(
        call(&mut s, "reset_max", &[]),
        Err(CommandError::NotNow { .. })
    ));
}

#[test]
fn an_unknown_row_key_is_no_such_row() {
    let (mut s, _) = service();
    let e = call(
        &mut s,
        "start",
        &[("session", text("1")), ("preset", text("No such band"))],
    )
    .unwrap_err();
    assert!(
        matches!(&e, CommandError::NoSuchRow { key, .. } if key == "No such band"),
        "{e:?}"
    );
    let (mut s, _) = swept();
    for row in ["424242", "not a number"] {
        let e = call(
            &mut s,
            "hit_action",
            &[("row", text(row)), ("action", Value::Choice(1))],
        )
        .unwrap_err();
        assert!(
            matches!(&e, CommandError::NoSuchRow { key, .. } if key == row),
            "{e:?}"
        );
    }
}

#[test]
fn the_findings_table_is_paged() {
    let (mut s, _) = swept();
    assert_eq!(field_num(&s, "total"), 5.0);
    assert_eq!(rows(&s, "findings").len(), 5);
    call(
        &mut s,
        "findings",
        &[("limit", Value::I64(2)), ("offset", Value::I64(0))],
    )
    .unwrap();
    let page = |s: &SweepService| {
        let mut p: Vec<(i64, String, f64)> = rows(s, "findings")
            .into_iter()
            .map(|(k, r)| (num(r, "position") as i64, k.to_owned(), num(r, "freq_hz")))
            .collect();
        p.sort_by_key(|x| x.0);
        p
    };
    let first = page(&s);
    assert_eq!(first.iter().map(|x| x.0).collect::<Vec<_>>(), [0, 1]);
    assert!(first[0].2 < first[1].2, "in frequency order");
    assert_eq!(field_num(&s, "total"), 5.0);
    call(
        &mut s,
        "findings",
        &[("limit", Value::I64(2)), ("offset", Value::I64(2))],
    )
    .unwrap();
    let next = page(&s);
    assert_eq!(next.len(), 2);
    assert!(next.iter().all(|x| first.iter().all(|f| f.1 != x.1)));
    assert!(next[0].2 > first[1].2);
    call(
        &mut s,
        "findings",
        &[("limit", Value::I64(2)), ("offset", Value::I64(4))],
    )
    .unwrap();
    assert_eq!(page(&s).len(), 1);
}

#[test]
fn an_export_goes_through_the_storage() {
    let (mut s, st) = swept();
    let r = call(
        &mut s,
        "export",
        &[
            ("path", text("sweeps/signals.csv")),
            ("format", Value::Choice(2)),
        ],
    )
    .unwrap()
    .unwrap();
    let file = st
        .read(&StorePath::new("sweeps/signals.csv").unwrap())
        .unwrap();
    assert_eq!(num(&r, "written"), file.len() as f64);
    assert_eq!(String::from_utf8(file).unwrap().lines().count(), 6);
    // The storage fails: the typed error, with the path.
    st.fail_next(StorageError::NoSpace);
    let e = call(&mut s, "export", &[("path", text("sweeps/again.csv"))]).unwrap_err();
    let CommandError::Failed { source } = e else {
        panic!("{e:?}");
    };
    assert_eq!(
        source.downcast_ref::<SweepError>(),
        Some(&SweepError::Storage {
            path: StorePath::new("sweeps/again.csv").unwrap(),
            source: StorageError::NoSpace,
        })
    );
    // A path that is not a store path.
    let e = call(&mut s, "export", &[("path", text("../out.csv"))]).unwrap_err();
    assert!(
        matches!(
            e,
            CommandError::BadArgument {
                error: SettingError::Malformed { .. },
                ..
            }
        ),
        "{e:?}"
    );
    // A file that is not a saved floor.
    let e = call(
        &mut s,
        "floor.restore",
        &[("path", text("sweeps/signals.csv"))],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::NotNow { .. }), "{e:?}");
}

#[test]
fn stop_ends_the_sweep_at_the_next_step() {
    let (mut s, _) = service();
    let mut d = Driver::new(FakeRadio::new(RATE, 99e6, 3));
    d.start(
        &mut s,
        &[("low_hz", Value::Hz(100e6)), ("high_hz", Value::Hz(104e6))],
    );
    assert_eq!(s.session(), Some("1"));
    call(&mut s, "stop", &[]).unwrap();
    let step = s.step(d.radio.time_s(), &d.info()).unwrap();
    assert_eq!(step, Some(sdrkit_svc_sweep::Step::Done { return_hz: 99e6 }));
    assert_eq!(field_choice(&s, "end"), "stopped");
    assert_eq!(s.session(), None);
    assert!(matches!(
        call(&mut s, "stop", &[]),
        Err(CommandError::NotNow { .. })
    ));
}

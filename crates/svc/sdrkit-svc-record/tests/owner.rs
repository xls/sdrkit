//! The record service read as an owner: its tables, commands and settings by key, checked arguments and row keys.

mod support;

use sdrkit_core::command::CommandError;
use sdrkit_core::owner::Owner;
use sdrkit_core::settings::SettingError;
use sdrkit_core::Value;
use sdrkit_svc_record::RECORD_EVENTS;
use support::*;

#[test]
fn tables_commands_and_settings_are_found_by_key() {
    let rig = Rig::new(&[]);
    let d = rig.svc.results_decl();
    for t in [
        "recordings",
        "running",
        "ts_files",
        "schedules",
        "schedule_runs",
    ] {
        assert!(d.table_id(t).is_some(), "{t}");
    }
    for c in [
        "audio.start",
        "audio.stop",
        "iq.start",
        "iq.stop",
        "ts.start",
        "ts.stop",
        "timeshift.start",
        "timeshift.pause",
        "timeshift.seek",
        "timeshift.live",
        "rewind.save",
        "schedule.add",
        "schedule.remove",
        "delete",
    ] {
        assert!(rig.svc.commands().id(c).is_ok(), "{c}");
    }
    assert!(matches!(
        rig.svc.commands().id("audio.pause"),
        Err(CommandError::Unknown { .. })
    ));
    for s in [
        "folder",
        "audio_container",
        "iq_sample",
        "rewind",
        "budget_bytes",
    ] {
        assert!(rig.svc.schema().id(s).is_ok(), "{s}");
    }
    assert_eq!(rig.svc.events().len(), RECORD_EVENTS.len());
}

#[test]
fn a_bad_argument_is_refused_before_the_service_runs() {
    let mut rig = Rig::new(&[]);
    // The session is required.
    assert!(matches!(
        rig.call("audio.start", &[]),
        Err(CommandError::MissingArgument { .. })
    ));
    // A negative pre-roll is out of the declared range.
    match rig.call("rewind.save", &[("pre_s", Value::Duration(-1.0))]) {
        Err(CommandError::BadArgument {
            error: SettingError::OutOfRange { .. },
            ..
        }) => {}
        r => panic!("{r:?}"),
    }
    // Nothing started.
    assert!(rows(&rig.svc, "running").is_empty());
    assert!(rig.svc.take_feeds().is_empty());
    // A list of services that does not read produces the position of the bad item.
    match rig.call(
        "ts.start",
        &[("session", text("s1")), ("services", text("1, x"))],
    ) {
        Err(CommandError::BadArgument {
            error: SettingError::Malformed { at: Some(2), .. },
            ..
        }) => {}
        r => panic!("{r:?}"),
    }
}

#[test]
fn an_unknown_row_is_no_such_row() {
    let mut rig = Rig::new(&[]);
    for c in [
        "audio.stop",
        "iq.stop",
        "ts.stop",
        "schedule.remove",
        "delete",
    ] {
        assert!(
            matches!(
                rig.call(c, &[("row", text("99"))]),
                Err(CommandError::NoSuchRow { .. })
            ),
            "{c}"
        );
    }
    // A row of another kind is not a row of this command.
    let (row, _feed) = rig.start("iq.start", &[("session", text("s1"))]);
    assert!(matches!(
        rig.call("audio.stop", &[("row", text(&row))]),
        Err(CommandError::NoSuchRow { .. })
    ));
    rig.call("iq.stop", &[("row", text(&row))]).unwrap();
}

#[test]
fn schedules_are_rows_and_a_bad_cron_rule_names_its_field() {
    let mut rig = Rig::new(&[]);
    let base = [
        ("name", text("News")),
        ("session", text("s1")),
        ("start", Value::Time(1_735_736_400.0)),
        ("duration", Value::Duration(600.0)),
        ("freq_hz", Value::Hz(98.5e6)),
        ("stack", text("fm")),
    ];
    let reply = rig.call("schedule.add", &base).unwrap().unwrap();
    let Value::Text(key) = cell(&reply, "row") else {
        panic!("no row")
    };
    let s = rows(&rig.svc, "schedules");
    assert_eq!(s.len(), 1);
    assert_eq!(cell(&s[0].1, "next"), Value::Time(1_735_736_400.0));
    let mut bad = base.to_vec();
    bad.push(("repeat", Value::Choice(4)));
    bad.push(("cron", text("0 61 * * *")));
    match rig.call("schedule.add", &bad) {
        Err(CommandError::BadArgument {
            error: SettingError::Malformed { at: Some(2), .. },
            ..
        }) => {}
        r => panic!("{r:?}"),
    }
    // The schedules are saved and read back by a new service on the same storage.
    let again =
        sdrkit_svc_record::RecordService::open(rig.storage.clone(), clock(&rig.time)).unwrap();
    assert_eq!(rows(&again, "schedules").len(), 1);
    rig.call("schedule.remove", &[("row", text(&key))]).unwrap();
    assert!(rows(&rig.svc, "schedules").is_empty());
}

#[test]
fn a_due_schedule_asks_the_engine_to_tune_and_records() {
    let mut rig = Rig::new(&[("reserve_bytes", Value::I64(0))]);
    rig.call(
        "schedule.add",
        &[
            ("name", text("News")),
            ("kind", Value::Choice(1)),
            ("session", text("s1")),
            ("start", Value::Time(1_735_732_810.0)),
            ("duration", Value::Duration(60.0)),
            ("freq_hz", Value::Hz(98.5e6)),
        ],
    )
    .unwrap();
    rig.advance(10.0);
    rig.svc.tick(1_735_732_810.0, &mut rig.events);
    let req = rig.svc.take_requests();
    assert!(
        matches!(&req[..], [sdrkit_core::HostRequest::Tune { session, freq_hz: Some(f), .. }] if session == "s1" && *f == 98.5e6)
    );
    let feeds = rig.svc.take_feeds();
    assert!(matches!(&feeds[..], [sdrkit_svc_record::Feed::Iq(f)] if f.session() == "s1"));
    assert_eq!(rows(&rig.svc, "running").len(), 1);
}

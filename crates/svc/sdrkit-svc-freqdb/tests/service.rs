//! The frequency database as an owner: tables and commands by key, checked arguments, pages of a search, imports with
//! problems, and files through the host's storage.

use sdrkit_core::command::CommandError;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{
    Choice, HostRequest, MemStorage, OwnerKey, OwnerSetting, Record, Storage, StorageError,
    StorePath, TuneTarget, Value,
};
use sdrkit_svc_freqdb::*;
use std::sync::Arc;

fn service() -> (FreqDbService, Arc<MemStorage>) {
    let st = Arc::new(MemStorage::new());
    let mut s = FreqDbService::open(st.clone()).unwrap();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/freqdb/builtin.csv");
    s.set_supplied(supplied_entries(&std::fs::read_to_string(path).unwrap()).unwrap());
    (s, st)
}

fn call(
    s: &mut FreqDbService,
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

/// The rows of table `key`, in the order of the column `position` if it has one.
fn rows(s: &FreqDbService, key: &str) -> Vec<(String, Record)> {
    let id = s.results_decl().table_id(key).unwrap();
    let mut v: Vec<(String, Record)> = s
        .results()
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(k, r)| (k.to_owned(), r.clone()))
        .collect();
    v.sort_by_key(
        |(_, r)| match r.decl().id("position").and_then(|p| r.value(p)) {
            Some(Value::I64(p)) => *p,
            _ => 0,
        },
    );
    v
}

fn cell<'a>(r: &'a Record, key: &str) -> &'a Value {
    r.value(r.decl().id(key).unwrap()).unwrap()
}

fn total(s: &FreqDbService) -> Value {
    s.results()
        .fields()
        .value(s.results_decl().field_id("total").unwrap())
        .unwrap()
        .clone()
}

fn add(s: &mut FreqDbService, name: &str, hz: f64) -> String {
    let r = call(
        s,
        "add",
        &[("name", text(name)), ("freq_hz", Value::Hz(hz))],
    )
    .unwrap()
    .unwrap();
    match cell(&r, "row") {
        Value::Text(t) => t.clone(),
        v => panic!("{v:?}"),
    }
}

#[test]
fn the_service_is_an_owner_and_checks_arguments_first() {
    let (mut s, _) = service();
    assert_eq!(KEY.as_str(), "service.freqdb");
    for t in [
        "entries",
        "entry_scopes",
        "entry_tags",
        "entry_settings",
        "problems",
    ] {
        assert!(s.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in [
        "search", "near", "add", "update", "remove", "star", "tag.add", "item.set", "tune",
        "import", "export",
    ] {
        assert!(s.commands().id(c).is_ok(), "{c}");
    }
    let e = call(&mut s, "add", &[("name", text("A"))]).unwrap_err();
    assert!(matches!(e, CommandError::MissingArgument { .. }), "{e:?}");
    let e = call(&mut s, "search", &[("limit", Value::I64(5000))]).unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    let e = call(&mut s, "star", &[("row", text("b:nope"))]).unwrap_err();
    assert_eq!(
        e,
        CommandError::NoSuchRow {
            table: s.results_decl().table_id("entries").unwrap(),
            key: "b:nope".into()
        }
    );
    assert!(s.db().user_entries().is_empty(), "nothing ran");
}

#[test]
fn a_search_returns_a_page_a_total_and_the_next_page() {
    let (mut s, _) = service();
    call(
        &mut s,
        "search",
        &[("text", text("ft8")), ("limit", Value::I64(5))],
    )
    .unwrap();
    let all = match total(&s) {
        Value::I64(n) => n,
        v => panic!("{v:?}"),
    };
    assert!(all >= 12, "{all}");
    let first = rows(&s, "entries");
    assert_eq!(first.len(), 5);
    assert_eq!(cell(&first[0].1, "freq_hz"), &Value::Hz(1.84e6));
    call(
        &mut s,
        "search",
        &[
            ("text", text("ft8")),
            ("limit", Value::I64(5)),
            ("offset", Value::I64(5)),
        ],
    )
    .unwrap();
    let next = rows(&s, "entries");
    assert_eq!(next.len(), 5);
    assert_eq!(total(&s), Value::I64(all));
    assert!(next.iter().all(|(k, _)| first.iter().all(|(f, _)| f != k)));
    // The tags of the page refer to its rows.
    let tags = rows(&s, "entry_tags");
    assert!(!tags.is_empty());
    assert!(tags
        .iter()
        .all(|(_, r)| next.iter().any(|(k, _)| cell(r, "entry") == &text(k))));
    call(
        &mut s,
        "near",
        &[("freq_hz", Value::Hz(156.8e6)), ("scope", text("DE itu1"))],
    )
    .unwrap();
    assert!(rows(&s, "entries").iter().any(|(k, _)| k == "b:marine-16"));
}

#[test]
fn an_import_with_one_bad_line_returns_the_count_and_one_problem() {
    let (mut s, st) = service();
    st.write(
        &StorePath::new("in/list.txt").unwrap(),
        b"145.500,Calling\nbad,line\n7074 kHz,FT8\n",
    )
    .unwrap();
    let r = call(&mut s, "import", &[("path", text("in/list.txt"))])
        .unwrap()
        .unwrap();
    assert_eq!(cell(&r, "added"), &Value::I64(2));
    assert_eq!(cell(&r, "problems"), &Value::I64(1));
    let problems = rows(&s, "problems");
    assert_eq!(problems.len(), 1);
    assert_eq!(cell(&problems[0].1, "line"), &Value::I64(2));
    assert_eq!(
        cell(&problems[0].1, "kind"),
        &Value::Choice(Problem::NoFrequency.index())
    );
    let e = call(&mut s, "import", &[("path", text("/abs"))]).unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
}

#[test]
fn files_go_through_storage_and_a_failure_is_typed() {
    let (mut s, st) = service();
    let id = add(&mut s, "Club repeater", 145.6e6);
    call(&mut s, "star", &[("row", text(&id))]).unwrap();
    let again = FreqDbService::open(st.clone()).unwrap();
    assert_eq!(again.db().user_entries().len(), 1);
    assert!(again.db().is_starred(&id));
    st.fail_next(StorageError::NoSpace);
    let e = call(
        &mut s,
        "add",
        &[("name", text("B")), ("freq_hz", Value::Hz(1e6))],
    )
    .unwrap_err();
    let CommandError::Failed { source } = e else {
        panic!("{e:?}")
    };
    assert!(matches!(
        source.downcast_ref::<DbError>(),
        Some(DbError::Storage {
            source: StorageError::NoSpace,
            ..
        })
    ));
    st.fail_next(StorageError::Denied);
    assert!(matches!(
        FreqDbService::open(st),
        Err(DbError::Storage {
            source: StorageError::Denied,
            ..
        })
    ));
}

#[test]
fn an_entry_keeps_typed_settings_and_a_tune_returns_a_host_request() {
    let (mut s, _) = service();
    let id = add(&mut s, "Tower", 118.1e6);
    call(
        &mut s,
        "item.set",
        &[
            ("row", text(&id)),
            ("owner", text("stack.general")),
            ("key", text("width")),
            ("value", Value::Hz(8330.0)),
        ],
    )
    .unwrap();
    call(
        &mut s,
        "scope.add",
        &[("row", text(&id)), ("key", text("DE"))],
    )
    .unwrap();
    call(&mut s, "search", &[("user_only", Value::Bool(true))]).unwrap();
    let settings = rows(&s, "entry_settings");
    assert_eq!(settings.len(), 1);
    assert_eq!(cell(&settings[0].1, "value"), &Value::Hz(8330.0));
    assert_eq!(cell(&rows(&s, "entry_scopes")[0].1, "key"), &text("DE"));
    call(
        &mut s,
        "tune",
        &[("row", text(&id)), ("session", text("3"))],
    )
    .unwrap();
    assert_eq!(
        s.take_requests(),
        vec![HostRequest::Tune {
            session: "3".into(),
            freq_hz: Some(118.1e6),
            target: TuneTarget::Running,
            submode: String::new(),
            rate_hz: None,
            bandwidth_hz: None,
            settings: vec![OwnerSetting {
                owner: OwnerKey::of("stack.general"),
                key: "width".into(),
                value: Value::Hz(8330.0),
            }],
        }]
    );
}

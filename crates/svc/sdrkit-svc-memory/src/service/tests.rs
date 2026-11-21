use super::*;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{MemStorage, SettingId, StorageError, TuneTarget, Value};
use std::sync::atomic::{AtomicU64, Ordering};

/// A clock that a test moves.
fn clock(start_s: u64) -> (Clock, Arc<AtomicU64>) {
    let t = Arc::new(AtomicU64::new(start_s));
    let source = t.clone();
    (Clock::new(move || source.load(Ordering::Relaxed) as f64), t)
}

fn service() -> (MemoryService, Arc<MemStorage>) {
    let st = Arc::new(MemStorage::new());
    let s = MemoryService::open(st.clone(), Clock::fixed(1000.0)).unwrap();
    (s, st)
}

/// Runs command `key` with the arguments `args` given by key, through the checked path a host uses.
fn call(
    s: &mut MemoryService,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = s.commands().id(key)?;
    let def = s.commands().def(id).unwrap();
    let mut given = def.new_args();
    for (k, v) in args {
        let fid = given.decl().id(k).unwrap();
        given.set_value(fid, v).unwrap();
    }
    owner::invoke(s, id, &given)
}

fn text(v: &str) -> Value {
    Value::Text(v.into())
}

fn reply_text(r: &Record, key: &str) -> String {
    match r.value(r.decl().id(key).unwrap()) {
        Some(Value::Text(t)) => t.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

fn reply_i64(r: &Record, key: &str) -> i64 {
    match r.value(r.decl().id(key).unwrap()) {
        Some(Value::I64(v)) => *v,
        other => panic!("{key}: {other:?}"),
    }
}

/// The rows of table `key` as (row key, record), found by reflection only.
fn rows<'a>(s: &'a MemoryService, key: &str) -> Vec<(&'a str, &'a Record)> {
    let id = s.results_decl().table_id(key).unwrap();
    s.results().table(id).unwrap().rows_newest_first().collect()
}

fn cell<'a>(r: &'a Record, key: &str) -> &'a Value {
    r.value(r.decl().id(key).unwrap()).unwrap()
}

#[test]
fn the_service_is_an_owner_and_refuses_a_bad_argument_first() {
    let (mut s, _) = service();
    for t in [
        "memories",
        "memory_tags",
        "memory_settings",
        "memory_gains",
        "bookmarks",
        "presets",
        "problems",
    ] {
        assert!(s.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in [
        "query",
        "add",
        "remove",
        "undo",
        "import",
        "export",
        "preset.save",
        "bookmark.add",
    ] {
        assert!(s.commands().id(c).is_ok(), "{c}");
    }
    assert_eq!(KEY.as_str(), "service.memory");
    let e = call(
        &mut s,
        "add",
        &[("freq_hz", Value::Hz(1e6)), ("rate_hz", Value::Hz(-5.0))],
    )
    .unwrap_err();
    assert!(
        matches!(
            e,
            CommandError::BadArgument {
                arg: SettingId(4),
                ..
            }
        ),
        "{e:?}"
    );
    let e = call(&mut s, "add", &[]).unwrap_err();
    assert!(matches!(e, CommandError::MissingArgument { .. }), "{e:?}");
    let e = call(&mut s, "query", &[("limit", Value::I64(0))]).unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    assert!(rows(&s, "memories").is_empty(), "nothing ran");
}

#[test]
fn a_command_on_a_missing_row_returns_no_such_row() {
    let (mut s, _) = service();
    let e = call(&mut s, "remove", &[("row", text("99"))]).unwrap_err();
    assert_eq!(
        e,
        CommandError::NoSuchRow {
            table: s.results_decl().table_id("memories").unwrap(),
            key: "99".into()
        }
    );
    let e = call(&mut s, "preset.delete", &[("row", text("nope"))]).unwrap_err();
    assert!(matches!(e, CommandError::NoSuchRow { .. }));
}

#[test]
fn a_memory_with_gains_and_tags_reads_through_its_tables() {
    let (mut s, _) = service();
    let r = call(
        &mut s,
        "add",
        &[
            ("name", text("Tower")),
            ("freq_hz", Value::Hz(118.1e6)),
            ("stack", text("general")),
        ],
    )
    .unwrap()
    .unwrap();
    let row = reply_text(&r, "row");
    for tag in ["air", "local"] {
        call(
            &mut s,
            "tag.add",
            &[("row", text(&row)), ("key", text(tag))],
        )
        .unwrap();
    }
    let part = |p: Part| Value::Choice(sdrkit_core::Choice::index(p));
    for (p, owner, k, v) in [
        (Part::MemoryGain, "radio", "lna", Value::F64(32.0)),
        (
            Part::MemorySetting,
            "stack.general",
            "demod",
            Value::Choice(2),
        ),
    ] {
        call(
            &mut s,
            "item.set",
            &[
                ("row", text(&row)),
                ("part", part(p)),
                ("owner", text(owner)),
                ("key", text(k)),
                ("value", v),
            ],
        )
        .unwrap();
    }
    let memories = rows(&s, "memories");
    assert_eq!(memories.len(), 1);
    let (key, m) = memories[0];
    assert_eq!(cell(m, "name"), &text("Tower"));
    let decl = s.results_decl();
    let tags_decl = decl.table(decl.table_id("memory_tags").unwrap()).unwrap();
    let of = tags_decl
        .columns
        .field(tags_decl.columns.id("memory").unwrap())
        .unwrap();
    assert_eq!(
        of.refers_to.as_ref().map(|r| r.table.as_ref()),
        Some("memories")
    );
    let mut tags: Vec<_> = rows(&s, "memory_tags")
        .into_iter()
        .filter(|(_, r)| cell(r, "memory") == &text(key))
        .map(|(_, r)| cell(r, "key").clone())
        .collect();
    tags.sort_by_key(|v| format!("{v:?}"));
    assert_eq!(tags, vec![text("air"), text("local")]);
    let gains: Vec<_> = rows(&s, "memory_gains")
        .into_iter()
        .filter(|(_, r)| cell(r, "memory") == &text(key))
        .map(|(_, r)| (cell(r, "key").clone(), cell(r, "value").clone()))
        .collect();
    assert_eq!(gains, vec![(text("lna"), Value::F64(32.0))]);
    call(
        &mut s,
        "tag.remove",
        &[("row", text(&row)), ("key", text("air"))],
    )
    .unwrap();
    assert_eq!(rows(&s, "memory_tags").len(), 1);
    call(
        &mut s,
        "item.remove",
        &[
            ("row", text(&row)),
            ("part", part(Part::MemoryGain)),
            ("owner", text("radio")),
            ("key", text("lna")),
        ],
    )
    .unwrap();
    assert!(rows(&s, "memory_gains").is_empty());
    let e = call(
        &mut s,
        "item.set",
        &[
            ("row", text(&row)),
            ("part", part(Part::MemoryGain)),
            ("owner", text("Not A Key")),
            ("key", text("lna")),
            ("value", Value::F64(1.0)),
        ],
    )
    .unwrap_err();
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
}

#[test]
fn a_stored_setting_reads_back_typed_with_its_owner_and_key() {
    let (mut s, st) = service();
    let r = call(&mut s, "add", &[("freq_hz", Value::Hz(118.1e6))])
        .unwrap()
        .unwrap();
    let row = reply_text(&r, "row");
    let set = |s: &mut MemoryService, key: &str, v: Value| {
        call(
            s,
            "item.set",
            &[
                ("row", text(&row)),
                (
                    "part",
                    Value::Choice(sdrkit_core::Choice::index(Part::MemorySetting)),
                ),
                ("owner", text("stack.general")),
                ("key", text(key)),
                ("value", v),
            ],
        )
        .unwrap();
    };
    set(&mut s, "demod", Value::Choice(2));
    set(&mut s, "width", Value::Hz(8330.0));
    let mut found: Vec<_> = rows(&s, "memory_settings")
        .into_iter()
        .map(|(_, r)| {
            (
                cell(r, "owner").clone(),
                cell(r, "key").clone(),
                cell(r, "value").clone(),
            )
        })
        .collect();
    found.sort_by_key(|v| format!("{v:?}"));
    assert_eq!(
        found,
        vec![
            (text("stack.general"), text("demod"), Value::Choice(2)),
            (text("stack.general"), text("width"), Value::Hz(8330.0)),
        ]
    );
    let again = MemoryService::open(st, Clock::fixed(0.0)).unwrap();
    let m = &again.store().memories()[0];
    assert_eq!(m.settings[0].owner, OwnerKey::of("stack.general"));
    assert_eq!(m.settings[1].value, Value::Hz(8330.0));
}

#[test]
fn a_recall_returns_a_host_request_for_the_session() {
    let (mut s, _) = service();
    let r = call(
        &mut s,
        "add",
        &[
            ("freq_hz", Value::Hz(145.5e6)),
            ("stack", text("general")),
            ("submode", text("nfm")),
        ],
    )
    .unwrap()
    .unwrap();
    let row = reply_text(&r, "row");
    call(
        &mut s,
        "item.set",
        &[
            ("row", text(&row)),
            (
                "part",
                Value::Choice(sdrkit_core::Choice::index(Part::MemoryGain)),
            ),
            ("owner", text("radio")),
            ("key", text("lna")),
            ("value", Value::F64(20.0)),
        ],
    )
    .unwrap();
    let e = call(&mut s, "recall", &[("row", text(&row))]).unwrap_err();
    assert!(matches!(e, CommandError::MissingArgument { .. }), "{e:?}");
    let reply = call(
        &mut s,
        "recall",
        &[("row", text(&row)), ("session", text("2"))],
    )
    .unwrap();
    assert!(reply.is_none(), "there is nothing for the host to apply");
    let req = s.take_requests();
    assert_eq!(
        req,
        vec![HostRequest::Tune {
            session: "2".into(),
            freq_hz: Some(145.5e6),
            target: TuneTarget::Stack("general".into()),
            submode: "nfm".into(),
            rate_hz: None,
            bandwidth_hz: None,
            settings: vec![OwnerSetting {
                owner: OwnerKey::of("radio"),
                key: "lna".into(),
                value: Value::F64(20.0),
            }],
        }]
    );
    assert!(s.take_requests().is_empty());
    assert_eq!(
        s.store().get(row.parse().unwrap()).unwrap().last_used_s,
        Some(1000.0)
    );
    let preset = rows(&s, "presets")[0].0.to_owned();
    call(
        &mut s,
        "preset.apply",
        &[("row", text(&preset)), ("session", text("2"))],
    )
    .unwrap();
    assert!(
        matches!(&s.take_requests()[..], [HostRequest::Tune { target: TuneTarget::Stack(stack), .. }] if stack == "general")
    );
}

#[test]
fn the_undo_window_is_a_setting() {
    let st = Arc::new(MemStorage::new());
    let (clock, t) = clock(1000);
    let mut s = MemoryService::open(st, clock).unwrap();
    let window = s.schema().id("undo_window").unwrap();
    assert_eq!(
        s.values().unwrap().value(window),
        Some(&Value::Duration(10.0))
    );
    owner::apply(&mut s, window, &Value::Duration(60.0)).unwrap();
    let r = call(&mut s, "add", &[("freq_hz", Value::Hz(1e6))])
        .unwrap()
        .unwrap();
    call(&mut s, "remove", &[("row", text(&reply_text(&r, "row")))]).unwrap();
    t.store(1030, Ordering::Relaxed);
    call(&mut s, "undo", &[]).unwrap();
    assert_eq!(
        rows(&s, "memories").len(),
        1,
        "30 s is inside a 60 s window"
    );
    call(&mut s, "remove", &[("row", text(&reply_text(&r, "row")))]).unwrap();
    owner::apply(&mut s, window, &Value::Duration(5.0)).unwrap();
    t.store(1040, Ordering::Relaxed);
    call(&mut s, "undo", &[]).unwrap();
    assert!(
        rows(&s, "memories").is_empty(),
        "10 s is outside a 5 s window"
    );
    assert!(owner::apply(&mut s, window, &Value::Duration(-1.0)).is_err());
}

#[test]
fn a_query_returns_a_page_and_a_total() {
    let (mut s, _) = service();
    for i in 0..5 {
        call(
            &mut s,
            "add",
            &[("freq_hz", Value::Hz(1e6 * f64::from(i + 1)))],
        )
        .unwrap();
    }
    let sort = Value::Choice(sdrkit_core::Choice::index(SortKey::Frequency));
    call(
        &mut s,
        "query",
        &[
            ("sort", sort.clone()),
            ("limit", Value::I64(2)),
            ("offset", Value::I64(2)),
        ],
    )
    .unwrap();
    let total = s
        .results()
        .fields()
        .value(s.results_decl().field_id("total").unwrap());
    assert_eq!(total, Some(&Value::I64(5)));
    let page: Vec<_> = rows(&s, "memories")
        .into_iter()
        .map(|(_, r)| cell(r, "freq_hz").clone())
        .collect();
    assert_eq!(page, vec![Value::Hz(3e6), Value::Hz(4e6)]);
}

#[test]
fn an_import_with_one_bad_line_returns_the_count_and_one_problem() {
    let (mut s, st) = service();
    let file = StorePath::new("in/list.txt").unwrap();
    st.write(&file, b"145.5,Calling\noops,Bad\n7100 kHz,Forty\n")
        .unwrap();
    let e = call(&mut s, "import", &[("path", text("../x"))]).unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    let r = call(&mut s, "import", &[("path", text("in/list.txt"))])
        .unwrap()
        .unwrap();
    assert_eq!(reply_i64(&r, "added"), 2);
    assert_eq!(reply_i64(&r, "problems"), 1);
    let problems = rows(&s, "problems");
    assert_eq!(problems.len(), 1);
    assert_eq!(cell(problems[0].1, "line"), &Value::I64(2));
    assert_eq!(
        cell(problems[0].1, "kind"),
        &Value::Choice(sdrkit_core::Choice::index(Problem::NotAFrequency))
    );
    assert_eq!(rows(&s, "memories").len(), 2);
    let r = call(
        &mut s,
        "export",
        &[
            ("path", text("out/m.csv")),
            (
                "format",
                Value::Choice(sdrkit_core::Choice::index(Format::Csv)),
            ),
        ],
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply_i64(&r, "written"), 2);
    assert!(st
        .read(&StorePath::new("out/m.csv").unwrap())
        .unwrap()
        .starts_with(b"name,"));
}

#[test]
fn files_go_through_storage_and_a_failure_is_typed() {
    let (mut s, st) = service();
    call(
        &mut s,
        "add",
        &[("name", text("A")), ("freq_hz", Value::Hz(1e6))],
    )
    .unwrap();
    call(&mut s, "bookmark.add", &[("freq_hz", Value::Hz(2e6))]).unwrap();
    let again = MemoryService::open(st.clone(), Clock::fixed(0.0)).unwrap();
    assert_eq!(again.store().len(), 1);
    assert_eq!(rows(&again, "bookmarks").len(), 1);
    st.fail_next(StorageError::NoSpace);
    let e = call(&mut s, "add", &[("freq_hz", Value::Hz(3e6))]).unwrap_err();
    let CommandError::Failed { source } = e else {
        panic!("{e:?}")
    };
    assert!(matches!(
        source.downcast_ref::<MemoryError>(),
        Some(MemoryError::Storage {
            source: StorageError::NoSpace,
            ..
        })
    ));
    st.fail_next(StorageError::Denied);
    assert!(matches!(
        MemoryService::open(st, Clock::fixed(0.0)),
        Err(MemoryError::Storage {
            source: StorageError::Denied,
            ..
        })
    ));
}

#[test]
fn remove_and_undo_move_the_rows() {
    let (mut s, _) = service();
    let r = call(&mut s, "add", &[("freq_hz", Value::Hz(1e6))])
        .unwrap()
        .unwrap();
    let row = reply_text(&r, "row");
    call(&mut s, "remove", &[("row", text(&row))]).unwrap();
    assert!(rows(&s, "memories").is_empty());
    call(&mut s, "undo", &[]).unwrap();
    assert_eq!(rows(&s, "memories").len(), 1);
}

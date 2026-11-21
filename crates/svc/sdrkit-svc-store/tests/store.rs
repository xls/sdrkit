//! The store as an owner: inserts, the throttle, retention, search, the journal in storage, exports, daily files and
//! sessions.

use std::sync::{Arc, LazyLock};

use sdrkit_core::command::CommandError;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::{field, Field};
use sdrkit_core::{
    Event, EventCatalog, EventDef, MemStorage, Record, SettingError, Storage, StorageError,
    StorePath, Value,
};
use sdrkit_svc_events::{Context, Kinds, NameError, Position};
use sdrkit_svc_store::session::{COUNT, OPEN, SESSIONS};
use sdrkit_svc_store::store::{
    BAD_LINES, FOLDED, MATCHES, MAX_AGE, MAX_BYTES, MAX_ROWS, MESSAGES, PRUNED, ROWS,
};
use sdrkit_svc_store::{
    export, Added, DailyFiles, DailyFormat, Export, SessionBook, SessionType, Store, StoreError,
};

const ALTITUDE_FT: Field<field::I64> = Field::new("altitude_ft", "Altitude", "ft");
static PLANE: EventDef = EventDef::new(
    "air.plane",
    "Plane",
    &[
        &keys::SOURCE,
        &keys::FAMILY,
        &keys::POSITION,
        &keys::CALLSIGN,
        &keys::URGENCY,
        &ALTITUDE_FT,
    ],
);
static PAGE: EventDef = EventDef::new(
    "page.message",
    "Page",
    &[&keys::SOURCE, &keys::FAMILY, &keys::TEXT],
);
static RADIO: EventDef = EventDef::new("radio.text", "Radio text", &[&keys::TEXT]);
static CALL: EventDef = EventDef::new(
    "voice.call",
    "Call",
    &[
        &keys::SOURCE,
        &keys::TARGET,
        &keys::CHANNEL,
        &keys::FAMILY,
        &keys::TEXT,
        &keys::PRIVATE,
    ],
);
static PACKET: EventDef = EventDef::new(
    "aprs.packet",
    "Packet",
    &[&keys::SOURCE, &keys::FAMILY, &keys::TEXT, &keys::POSITION],
);

fn kinds() -> Arc<Kinds> {
    static KINDS: LazyLock<Arc<Kinds>> = LazyLock::new(|| {
        let mut defs: Vec<&'static EventDef> = vec![&PLANE, &PAGE, &RADIO, &CALL, &PACKET];
        defs.extend(sdrkit_svc_store::session::SESSION_EVENTS);
        Arc::new(Kinds::new(Arc::new(EventCatalog::new(&defs).unwrap())))
    });
    Arc::clone(&KINDS)
}

/// `e` with its kind id, as the engine stamps it when the event enters.
fn st(mut e: Event) -> Event {
    kinds().catalog().stamp(&mut e);
    e
}

fn memory(k: &Arc<Kinds>) -> Store {
    Store::in_memory(Arc::clone(k), Arc::new(MemStorage::new()))
}

fn put(s: &mut Store, k: &Kinds, t: f64, e: &Event) -> Added {
    let ctx = Context {
        unix_time: t,
        receiver: Some(Position::new(-33.0, 151.0)),
    };
    let e = st(e.clone());
    s.insert(&k.observe(&e, &ctx).unwrap()).unwrap()
}

fn plane(id: &str, alt: i64) -> Event {
    st(PLANE
        .event(0.0, 1090e6)
        .set(&keys::SOURCE, id)
        .set(&keys::FAMILY, Family::Aircraft)
        .set(&ALTITUDE_FT, alt))
}

fn text(t: &str) -> Event {
    st(RADIO.event(0.0, 98.1e6).set(&keys::TEXT, t))
}

fn set(s: &mut Store, key: &str, v: Value) {
    let id = s.schema().id(key).unwrap();
    owner::apply(s, id, &v).unwrap();
}

fn search(s: &mut Store, args: &[(&str, Value)]) -> Result<i64, CommandError> {
    let id = s.commands().id("search")?;
    let mut given = s.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let a = given.decl().id(k).unwrap();
        given.set_value(a, v).unwrap();
    }
    owner::invoke(s, id, &given)?;
    Ok(s.results().fields().get(&MATCHES).unwrap_or(0))
}

fn field(s: &Store, f: &Field<field::I64>) -> i64 {
    s.results().fields().get(f).unwrap_or(0)
}

#[test]
fn repeats_fold_and_new_text_does_not() {
    let k = kinds();
    let mut s = memory(&k);
    let a = put(&mut s, &k, 100.0, &plane("7C6DB8", 1000));
    let b = put(&mut s, &k, 110.0, &plane("7C6DB8", 2000));
    assert_eq!(b, Added::Folded(a.id()));
    let m = s.get(a.id()).unwrap();
    assert_eq!((m.repeats, m.last_time, m.time), (1, 110.0, 100.0));
    assert_eq!(m.fields.get(&ALTITUDE_FT), Some(2000));
    assert!(matches!(
        put(&mut s, &k, 200.0, &plane("7C6DB8", 0)),
        Added::New(_)
    ));
    let page = |t: &str| {
        PAGE.event(0.0, 0.0)
            .set(&keys::SOURCE, "1234")
            .set(&keys::TEXT, t)
    };
    let p1 = put(&mut s, &k, 201.0, &page("one"));
    let p2 = put(&mut s, &k, 202.0, &page("two"));
    assert_ne!(p1.id(), p2.id());
    assert_eq!(field(&s, &FOLDED), 1);
}

#[test]
fn retention_by_rows_bytes_and_age() {
    let k = kinds();
    let mut s = memory(&k);
    set(&mut s, "max_rows", Value::I64(3));
    set(&mut s, "max_age", Value::Duration(100.0));
    set(&mut s, "fold", Value::Duration(0.0));
    for i in 0..5 {
        put(&mut s, &k, f64::from(i), &text(&format!("line {i}")));
    }
    assert_eq!(s.len(), 3);
    assert_eq!(s.iter().next().unwrap().text.as_deref(), Some("line 2"));
    put(&mut s, &k, 150.0, &text("late"));
    assert_eq!(s.len(), 1);
    let big = field(&s, &sdrkit_svc_store::store::BYTES);
    set(&mut s, "max_rows", Value::I64(100));
    set(&mut s, "max_bytes", Value::I64(big + 10));
    set(&mut s, "max_age", Value::Duration(0.0));
    put(&mut s, &k, 151.0, &text("another"));
    assert_eq!(s.len(), 1);
    assert!(field(&s, &PRUNED) >= 4);
    let _ = (MAX_ROWS, MAX_BYTES, MAX_AGE);
}

fn fill(s: &mut Store, k: &Kinds) {
    put(
        s,
        k,
        10.0,
        &plane("7C6DB8", 1000)
            .set(&keys::CALLSIGN, "QFA1")
            .set(&keys::POSITION, (-33.9, 151.2)),
    );
    put(
        s,
        k,
        20.0,
        &PAGE
            .event(0.0, 0.0)
            .set(&keys::SOURCE, "99")
            .set(&keys::FAMILY, Family::Pager)
            .set(&keys::TEXT, "Fire alarm at depot"),
    );
    put(s, k, 30.0, &text("Weather now"));
    put(
        s,
        k,
        40.0,
        &plane("7C0001", 500).set(&keys::URGENCY, Urgency::Emergency),
    );
    put(
        s,
        k,
        50.0,
        &CALL
            .event(0.0, 446e6)
            .set(&keys::SOURCE, "100")
            .set(&keys::TEXT, "secret")
            .set(&keys::PRIVATE, true),
    );
}

#[test]
fn the_store_is_an_owner_whose_search_fills_its_table() {
    let k = kinds();
    let mut s = memory(&k);
    fill(&mut s, &k);
    let table = s.results_decl().table_id(MESSAGES.key).unwrap();
    assert!(s.commands().id("delete").is_ok());
    assert_eq!(search(&mut s, &[]), Ok(5));
    let t = s.results().table(table).unwrap();
    assert_eq!(t.len(), 5);
    let (newest, _) = t.rows_newest_first().next().unwrap();
    assert_eq!(
        s.get(newest.parse().unwrap()).unwrap().kind_key,
        "voice.call"
    );
    assert_eq!(
        search(&mut s, &[("text", Value::Text("fire DEPOT".into()))]),
        Ok(1)
    );
    assert_eq!(
        search(&mut s, &[("text", Value::Text("\"alarm at\"".into()))]),
        Ok(1)
    );
    assert_eq!(
        search(&mut s, &[("text", Value::Text("alarm -fire".into()))]),
        Ok(0)
    );
    assert_eq!(
        search(&mut s, &[("text", Value::Text("callsign:qfa".into()))]),
        Ok(1)
    );
    assert_eq!(
        search(&mut s, &[("text", Value::Text("secret".into()))]),
        Ok(0)
    );
    assert_eq!(
        search(&mut s, &[("kind", Value::Text("air.*".into()))]),
        Ok(2)
    );
    assert_eq!(
        search(&mut s, &[("family", Value::Text("pager".into()))]),
        Ok(1)
    );
    assert_eq!(
        search(&mut s, &[("identity", Value::Text("7c6db8".into()))]),
        Ok(1)
    );
    assert_eq!(search(&mut s, &[("urgent_only", Value::Bool(true))]), Ok(1));
    assert_eq!(
        search(&mut s, &[("with_position", Value::Bool(true))]),
        Ok(1)
    );
    assert_eq!(
        search(
            &mut s,
            &[("since", Value::Time(25.0)), ("until", Value::Time(45.0))]
        ),
        Ok(2)
    );
    assert_eq!(s.results().table(table).unwrap().len(), 2);
    let m = s.iter().next().unwrap();
    assert!(m.distance_km.is_some());
}

#[test]
fn a_bad_argument_is_refused_before_the_store_runs() {
    let k = kinds();
    let mut s = memory(&k);
    fill(&mut s, &k);
    let err = search(&mut s, &[("limit", Value::I64(0))]).unwrap_err();
    assert!(matches!(
        err,
        CommandError::BadArgument {
            error: SettingError::OutOfRange { .. },
            ..
        }
    ));
    assert_eq!(field(&s, &MATCHES), 0);
    let del = s.commands().id("delete").unwrap();
    let mut given = s.commands().def(del).unwrap().new_args();
    assert!(matches!(
        owner::invoke(&mut s, del, &given),
        Err(CommandError::MissingArgument { .. })
    ));
    given
        .set_value(given.decl().id("row").unwrap(), &Value::Text("999".into()))
        .unwrap();
    assert!(matches!(
        owner::invoke(&mut s, del, &given),
        Err(CommandError::NoSuchRow { .. })
    ));
    given
        .set_value(given.decl().id("row").unwrap(), &Value::Text("1".into()))
        .unwrap();
    owner::invoke(&mut s, del, &given).unwrap();
    assert_eq!(s.len(), 4);
}

#[test]
fn an_unknown_field_in_the_search_fails_with_its_position() {
    let k = kinds();
    let mut s = memory(&k);
    let err = search(&mut s, &[("text", Value::Text("fire nosuch:x".into()))]).unwrap_err();
    let CommandError::Failed { source } = err else {
        panic!("{err:?}")
    };
    assert_eq!(
        source.downcast_ref::<NameError>(),
        Some(&NameError::UnknownField {
            name: "nosuch".into(),
            at: 5
        })
    );
    let err = search(&mut s, &[("kind", Value::Text("nothing.*".into()))]).unwrap_err();
    assert!(matches!(err, CommandError::Failed { .. }));
}

#[test]
fn the_journal_survives_a_restart_and_compacts() {
    let k = kinds();
    let storage = Arc::new(MemStorage::new());
    let path = StorePath::new("store/messages.jsonl").unwrap();
    {
        let mut s = Store::open(Arc::clone(&k), storage.clone(), path.clone()).unwrap();
        fill(&mut s, &k);
    }
    let mut data = storage.read(&path).unwrap();
    data.extend_from_slice(b"not json\n");
    storage.write(&path, &data).unwrap();
    let mut s = Store::open(Arc::clone(&k), storage.clone(), path.clone()).unwrap();
    assert_eq!(s.len(), 5);
    assert_eq!(field(&s, &BAD_LINES), 1);
    assert_eq!(field(&s, &ROWS), 5);
    let m = s.get(1).unwrap();
    assert_eq!((&*m.kind_key, m.family), ("air.plane", Family::Aircraft));
    assert_eq!(m.fields.get(&keys::CALLSIGN), Some("QFA1"));
    let added = put(&mut s, &k, 60.0, &text("more"));
    assert_eq!(added.id(), 6);
    let del = s.commands().id("delete").unwrap();
    let mut given = s.commands().def(del).unwrap().new_args();
    given
        .set_value(given.decl().id("row").unwrap(), &Value::Text("1".into()))
        .unwrap();
    owner::invoke(&mut s, del, &given).unwrap();
    let text = String::from_utf8(storage.read(&path).unwrap()).unwrap();
    assert_eq!(text.lines().count(), 5, "a delete compacts the journal");
    let clear = s.commands().id("clear").unwrap();
    let args = s.commands().def(clear).unwrap().new_args();
    owner::invoke(&mut s, clear, &args).unwrap();
    assert!(s.is_empty());
    assert!(storage.read(&path).unwrap().is_empty());
}

#[test]
fn a_storage_failure_returns_the_typed_error() {
    let k = kinds();
    let storage = Arc::new(MemStorage::new());
    let path = StorePath::new("messages.jsonl").unwrap();
    storage.fail_next(StorageError::Denied);
    assert!(matches!(
        Store::open(Arc::clone(&k), storage.clone(), path.clone()),
        Err(StoreError::Storage {
            source: StorageError::Denied,
            ..
        })
    ));
    let mut s = Store::open(Arc::clone(&k), storage.clone(), path).unwrap();
    storage.fail_next(StorageError::NoSpace);
    let x = text("x");
    let e = k.observe(&x, &Context::default()).unwrap();
    assert!(matches!(
        s.insert(&e),
        Err(StoreError::Storage {
            source: StorageError::NoSpace,
            ..
        })
    ));
    let mut d = DailyFiles::new(
        storage.clone(),
        StorePath::new("daily").unwrap(),
        DailyFormat::JsonLines,
        0,
    );
    storage.fail_next(StorageError::Denied);
    assert!(matches!(
        d.append(&e),
        Err(StoreError::Storage {
            source: StorageError::Denied,
            ..
        })
    ));
}

#[test]
fn exports() {
    let k = kinds();
    let mut s = memory(&k);
    let ctx = Context {
        unix_time: 1_762_598_527.0,
        receiver: None,
    };
    for e in [
        PACKET
            .event(0.0, 144.39e6)
            .set(&keys::SOURCE, "N0CALL-9")
            .set(&keys::FAMILY, Family::Aprs)
            .set(&keys::TEXT, "hi, \"there\"")
            .set(&keys::POSITION, (40.0, -105.0)),
        text("News <now> & weather"),
    ] {
        let e = st(e);
        s.insert(&k.observe(&e, &ctx).unwrap()).unwrap();
    }
    let out = export(s.iter(), Export::Csv);
    let header = out.split("\r\n").next().unwrap();
    assert!(
        header.starts_with("id,time,last_time,kind")
            && header.contains(",family,")
            && header.ends_with(",text")
    );
    assert!(out.contains("\"hi, \"\"there\"\"\""));
    assert_eq!(out.matches("\r\n").count(), 3);
    let a = export(s.iter(), Export::Adif);
    assert!(
        a.contains("<CALL:8>N0CALL-9")
            && a.contains("<MODE:3>PKT")
            && a.contains("<FREQ:10>144.390000")
    );
    assert!(a.contains("<QSO_DATE:8>20251108"));
    assert_eq!(a.matches("<EOR>").count(), 1);
    let kml = export(s.iter(), Export::Kml);
    assert_eq!(kml.matches("<Placemark>").count(), 1);
    assert!(kml.contains("-105,40,0") && !kml.contains("<now>"));
    assert!(export(s.iter(), Export::Gpx).contains("<wpt lat=\"40\" lon=\"-105\">"));
    let g = export(s.iter(), Export::GeoJson);
    assert_eq!(g.matches("\"Feature\"").count(), 1);
    assert!(g.contains("\"coordinates\":[-105.0,40.0]"));
    assert_eq!(export(s.iter(), Export::JsonLines).lines().count(), 2);
    assert!(export(s.iter(), Export::Json).starts_with('['));
    // The export command writes the messages of the last search to a file in storage.
    let storage = Arc::new(MemStorage::new());
    let mut s2 = Store::in_memory(Arc::clone(&k), storage.clone());
    put(&mut s2, &k, 1.0, &text("one"));
    put(&mut s2, &k, 2.0, &text("two"));
    search(&mut s2, &[]).unwrap();
    let id = s2.commands().id("export").unwrap();
    let mut given = s2.commands().def(id).unwrap().new_args();
    given
        .set_value(given.decl().id("format").unwrap(), &Value::Choice(2))
        .unwrap();
    given
        .set_value(
            given.decl().id("path").unwrap(),
            &Value::Text("exports/all.jsonl".into()),
        )
        .unwrap();
    let reply: Record = owner::invoke(&mut s2, id, &given).unwrap().unwrap();
    assert_eq!(
        reply.value(reply.decl().id("written").unwrap()),
        Some(&Value::I64(2))
    );
    let file = storage
        .read(&StorePath::new("exports/all.jsonl").unwrap())
        .unwrap();
    assert_eq!(String::from_utf8(file).unwrap().lines().count(), 2);
    given
        .set_value(
            given.decl().id("path").unwrap(),
            &Value::Text("../up".into()),
        )
        .unwrap();
    assert!(matches!(
        owner::invoke(&mut s2, id, &given),
        Err(CommandError::BadArgument {
            error: SettingError::Malformed { .. },
            ..
        })
    ));
    storage.fail_next(StorageError::NoSpace);
    given
        .set_value(
            given.decl().id("path").unwrap(),
            &Value::Text("x.jsonl".into()),
        )
        .unwrap();
    let Err(CommandError::Failed { source }) = owner::invoke(&mut s2, id, &given) else {
        panic!("a failed write is an error")
    };
    assert!(matches!(
        source.downcast_ref::<StoreError>(),
        Some(StoreError::Storage {
            source: StorageError::NoSpace,
            ..
        })
    ));
}

#[test]
fn daily_files_go_through_storage() {
    let k = kinds();
    let storage = Arc::new(MemStorage::new());
    let folder = StorePath::new("daily").unwrap();
    let ctx = |t: f64| Context {
        unix_time: t,
        receiver: None,
    };
    let e = text("a, b");
    let mut w = DailyFiles::new(storage.clone(), folder.clone(), DailyFormat::JsonLines, 0);
    let p1 = w
        .append(&k.observe(&e, &ctx(1_762_598_527.0)).unwrap())
        .unwrap();
    let p2 = w
        .append(&k.observe(&e, &ctx(1_762_598_528.0)).unwrap())
        .unwrap();
    assert_eq!(p1, p2);
    assert_eq!(p1.as_str(), "daily/2025-11-08/radio.jsonl");
    assert_eq!(
        String::from_utf8(storage.read(&p1).unwrap())
            .unwrap()
            .lines()
            .count(),
        2
    );
    let mut j = DailyFiles::new(
        storage.clone(),
        StorePath::new("j").unwrap(),
        DailyFormat::Json,
        0,
    );
    let p = j.append(&k.observe(&e, &ctx(1.0)).unwrap()).unwrap();
    j.append(&k.observe(&e, &ctx(2.0)).unwrap()).unwrap();
    let s = String::from_utf8(storage.read(&p).unwrap()).unwrap();
    assert!(s.starts_with("[\n") && s.ends_with("\n]\n") && s.matches("\"kind\"").count() == 2);
    let mut c = DailyFiles::new(
        storage.clone(),
        StorePath::new("c").unwrap(),
        DailyFormat::Csv,
        0,
    );
    let p = c.append(&k.observe(&e, &ctx(1.0)).unwrap()).unwrap();
    c.append(&k.observe(&e, &ctx(2.0)).unwrap()).unwrap();
    let s = String::from_utf8(storage.read(&p).unwrap()).unwrap();
    assert!(s.starts_with("time,kind") && s.lines().count() == 3 && s.contains("\"a, b\""));
}

fn observe(b: &mut SessionBook, k: &Kinds, t: f64, e: &Event) -> Vec<String> {
    let e = st(e.clone());
    let o = k
        .observe(
            &e,
            &Context {
                unix_time: t,
                receiver: None,
            },
        )
        .unwrap();
    b.observe(&o)
        .into_iter()
        .map(|(_, e)| e.kind().to_owned())
        .collect()
}

/// The rows of the sessions table: (row key, open, count, source).
fn sessions(b: &SessionBook) -> Vec<(String, bool, i64, String)> {
    let t = b.results_decl().table_id(SESSIONS.key).unwrap();
    let mut v: Vec<_> = b
        .results()
        .table(t)
        .unwrap()
        .rows_newest_first()
        .map(|(key, r)| {
            (
                key.to_owned(),
                r.get(&OPEN).unwrap(),
                r.get(&COUNT).unwrap(),
                r.get(&keys::SOURCE).unwrap().to_owned(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn an_aircraft_track_begins_updates_and_ends_on_a_gap() {
    let k = kinds();
    let mut b = SessionBook::default();
    let ac = |lat: f64| plane("7C6DB8", 0).set(&keys::POSITION, (lat, 151.0));
    assert_eq!(observe(&mut b, &k, 0.0, &ac(-33.0)), ["session.begin"]);
    assert!(observe(&mut b, &k, 5.0, &ac(-33.1)).is_empty());
    assert_eq!(observe(&mut b, &k, 15.0, &ac(-33.2)), ["session.update"]);
    // 200 s later: past the 120 s aircraft gap, so the old track ends and a new one begins.
    let late = ac(-34.0);
    let o = k
        .observe(
            &late,
            &Context {
                unix_time: 215.0,
                receiver: None,
            },
        )
        .unwrap();
    let v = b.observe(&o);
    assert_eq!(
        v.iter()
            .map(|(_, e)| e.kind().to_owned())
            .collect::<Vec<_>>(),
        ["session.end", "session.begin"]
    );
    assert_eq!(v[0].1.get(&COUNT), Some(3));
    assert_eq!(b.get(1).unwrap().trail.len(), 3);
    assert_eq!(
        sessions(&b),
        [
            ("1".into(), false, 3, "7C6DB8".into()),
            ("2".into(), true, 1, "7C6DB8".into())
        ]
    );
    let ended: Vec<_> = b
        .tick(1000.0)
        .into_iter()
        .map(|(_, e)| e.kind().to_owned())
        .collect();
    assert_eq!(ended, ["session.end"]);
    assert!(sessions(&b).iter().all(|s| !s.1));
}

#[test]
fn pager_threads_and_voice_conversations() {
    let k = kinds();
    let mut b = SessionBook::default();
    let page = |m: &str| {
        PAGE.event(0.0, 0.0)
            .set(&keys::SOURCE, "1234567")
            .set(&keys::FAMILY, Family::Pager)
            .set(&keys::TEXT, m)
    };
    observe(&mut b, &k, 0.0, &page("one"));
    observe(&mut b, &k, 60.0, &page("two"));
    let s = b.get(1).unwrap();
    assert_eq!((s.ty, s.messages.len()), (SessionType::Thread, 2));
    let call = CALL
        .event(0.0, 446.00625e6)
        .set(&keys::SOURCE, "100")
        .set(&keys::TARGET, "200")
        .set(&keys::CHANNEL, "9")
        .set(&keys::FAMILY, Family::Voice);
    assert_eq!(observe(&mut b, &k, 100.0, &call), ["session.begin"]);
    observe(&mut b, &k, 110.0, &call);
    let conv = b.get(2).unwrap();
    assert_eq!(conv.ty, SessionType::Conversation);
    assert_eq!(conv.parties, ["100", "200"]);
    assert_eq!(conv.count, 2);
    let ended = b.end_all();
    assert_eq!(ended.len(), 2);
    // An event with no identity makes no session.
    assert!(observe(&mut b, &k, 200.0, &text("sync lost")).is_empty());
}

fn set_book(b: &mut SessionBook, key: &str, v: Value) {
    let id = b.schema().id(key).unwrap();
    owner::apply(b, id, &v).unwrap();
}

#[test]
fn caps_hold() {
    let k = kinds();
    let mut b = SessionBook::new();
    set_book(&mut b, "open_cap", Value::I64(2));
    set_book(&mut b, "history_cap", Value::I64(1));
    for i in 0..4 {
        observe(
            &mut b,
            &k,
            f64::from(i),
            &plane(&format!("{}", 503_000_000 + i), 0),
        );
    }
    let rows = sessions(&b);
    assert_eq!(rows.iter().filter(|s| s.1).count(), 2);
    assert_eq!(rows.iter().filter(|s| !s.1).count(), 1);
}

#[test]
fn the_session_book_is_an_owner() {
    let k = kinds();
    let mut b = SessionBook::new();
    assert_eq!(sdrkit_svc_store::session::KEY.as_str(), "service.sessions");
    assert!(b.results_decl().table_id("sessions").is_some());
    assert!(b.events().iter().any(|e| e.kind == "session.end"));
    let id = b.schema().id("track_gap").unwrap();
    assert!(matches!(
        owner::apply(&mut b, id, &Value::Duration(-1.0)),
        Err(SettingError::OutOfRange { .. })
    ));
    observe(&mut b, &k, 0.0, &plane("7C6DB8", 0));
    let end = b.commands().id("end").unwrap();
    assert!(b.commands().id("nothing").is_err());
    let mut given = b.commands().def(end).unwrap().new_args();
    assert!(matches!(
        owner::invoke(&mut b, end, &given),
        Err(CommandError::MissingArgument { .. })
    ));
    given
        .set_value(given.decl().id("row").unwrap(), &Value::Text("9".into()))
        .unwrap();
    assert!(matches!(
        owner::invoke(&mut b, end, &given),
        Err(CommandError::NoSuchRow { .. })
    ));
    given
        .set_value(given.decl().id("row").unwrap(), &Value::Text("1".into()))
        .unwrap();
    owner::invoke(&mut b, end, &given).unwrap();
    assert!(!sessions(&b)[0].1);
    // The end event reaches the engine on its next tick.
    let ended: Vec<_> = b
        .tick(1.0)
        .into_iter()
        .map(|(_, e)| e.kind().to_owned())
        .collect();
    assert_eq!(ended, ["session.end"]);
    let forget = b.commands().id("forget").unwrap();
    let args = b.commands().def(forget).unwrap().new_args();
    owner::invoke(&mut b, forget, &args).unwrap();
    assert!(sessions(&b).is_empty());
}

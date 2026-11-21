//! The automation as an owner: rules, actions, areas and schedules set by commands and read back as tables, names resolved
//! when a rule is set, and the requests that rules make as events.

use std::sync::{Arc, LazyLock};

use sdrkit_core::command::CommandError;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::{field, Field};
use sdrkit_core::{
    Event, EventCatalog, EventDef, MemStorage, Record, SettingError, StorageError, Value,
};
use sdrkit_svc_automation::events::{CPA_KM, FENCE, SCHEDULE, SECONDS, STACK, TITLE, WHAT};
use sdrkit_svc_automation::{
    Automation, AutomationError, ExprError, RecordWhat, Syntax, TextError, AUTOMATION_EVENTS,
};
use sdrkit_svc_events::{Context, Kinds, NameError, Position};

const ALTITUDE_FT: Field<field::I64> = Field::new("altitude_ft", "Altitude", "ft");
const SQUAWK: Field<field::Text> = Field::new("squawk", "Squawk", "");
static PLANE: EventDef = EventDef::new(
    "air.plane",
    "Plane",
    &[
        &keys::SOURCE,
        &keys::FAMILY,
        &keys::POSITION,
        &keys::CALLSIGN,
        &keys::URGENCY,
        &SQUAWK,
        &ALTITUDE_FT,
    ],
);
static SHIP: EventDef = EventDef::new(
    "sea.ship",
    "Ship",
    &[
        &keys::SOURCE,
        &keys::FAMILY,
        &keys::POSITION,
        &keys::SPEED_KT,
        &keys::COURSE_DEG,
    ],
);
static TEXT: EventDef = EventDef::new("radio.text", "Text", &[&keys::TEXT]);

fn kinds() -> Arc<Kinds> {
    static K: LazyLock<Arc<Kinds>> = LazyLock::new(|| {
        let mut defs: Vec<&'static EventDef> = vec![&PLANE, &SHIP, &TEXT];
        defs.extend(AUTOMATION_EVENTS);
        Arc::new(Kinds::new(Arc::new(EventCatalog::new(&defs).unwrap())))
    });
    Arc::clone(&K)
}

fn st(mut e: Event) -> Event {
    kinds().catalog().stamp(&mut e);
    e
}

fn plane(id: &str, alt: i64) -> Event {
    st(PLANE
        .event(0.0, 1090e6)
        .set(&keys::SOURCE, id)
        .set(&keys::FAMILY, Family::Aircraft)
        .set(&keys::CALLSIGN, "QFA12")
        .set(&SQUAWK, "7700")
        .set(&ALTITUDE_FT, alt)
        .set(&keys::POSITION, (-33.0, 151.0)))
}

fn ship(lat: f64, lon: f64, sog: f64, cog: f64) -> Event {
    st(SHIP
        .event(0.0, 161.975e6)
        .set(&keys::SOURCE, "503000001")
        .set(&keys::FAMILY, Family::Vessel)
        .set(&keys::POSITION, (lat, lon))
        .set(&keys::SPEED_KT, sog)
        .set(&keys::COURSE_DEG, cog))
}

fn ctx(t: f64) -> Context {
    Context {
        unix_time: t,
        receiver: Some(Position::new(0.0, 0.0)),
    }
}

fn open(storage: &Arc<MemStorage>) -> Automation {
    Automation::open(kinds(), storage.clone()).unwrap()
}

fn call(
    a: &mut Automation,
    cmd: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = a.commands().id(cmd)?;
    let mut given = a.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let i = given.decl().id(k).unwrap();
        given.set_value(i, v).unwrap();
    }
    owner::invoke(a, id, &given)
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

fn handle(a: &mut Automation, t: f64, e: &Event) -> Vec<Event> {
    let k = kinds();
    let c = ctx(t);
    let o = k.observe(e, &c).unwrap();
    let mut out = Vec::new();
    a.handle(&o, &c, 0, &mut out);
    out
}

fn rows(a: &Automation, table: &str) -> Vec<(String, Record)> {
    let t = a.results_decl().table_id(table).unwrap();
    let mut v: Vec<_> = a
        .results()
        .table(t)
        .unwrap()
        .rows_newest_first()
        .map(|(k, r)| (k.to_owned(), r.clone()))
        .collect();
    v.sort_by(|x, y| x.0.cmp(&y.0));
    v
}

fn text_error(r: Result<Option<Record>, CommandError>) -> (&'static str, TextError) {
    let Err(CommandError::Failed { source }) = r else {
        panic!("{r:?}")
    };
    match source.downcast_ref::<AutomationError>() {
        Some(AutomationError::Text { arg, error }) => (arg, error.clone()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_automation_is_an_owner() {
    let mut a = open(&Arc::new(MemStorage::new()));
    for t in [
        "rules",
        "rule_actions",
        "rule_fences",
        "rule_schedules",
        "rule_problems",
        "log",
    ] {
        assert!(a.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in [
        "rule.set",
        "rule.remove",
        "action.add",
        "fence.set",
        "schedule.set",
    ] {
        assert!(a.commands().id(c).is_ok(), "{c}");
    }
    assert!(a.events().iter().any(|e| e.kind == "automation.tune"));
    // A bad argument is refused before the automation runs.
    let r = call(
        &mut a,
        "rule.set",
        &[("row", text("r")), ("max_km", Value::F64(-1.0))],
    );
    assert!(matches!(
        r,
        Err(CommandError::BadArgument {
            error: SettingError::OutOfRange { .. },
            ..
        })
    ));
    assert!(matches!(
        call(&mut a, "rule.set", &[]),
        Err(CommandError::MissingArgument { .. })
    ));
    assert!(rows(&a, "rules").is_empty());
    assert!(matches!(
        call(&mut a, "rule.remove", &[("row", text("nothing"))]),
        Err(CommandError::NoSuchRow { .. })
    ));
    assert!(matches!(
        call(&mut a, "action.add", &[("rule", text("nothing"))]),
        Err(CommandError::NoSuchRow { .. })
    ));
}

#[test]
fn unknown_names_fail_when_the_rule_is_set() {
    let mut a = open(&Arc::new(MemStorage::new()));
    let r = call(
        &mut a,
        "rule.set",
        &[
            ("row", text("r")),
            ("kinds", text("air.*")),
            ("condition", text("altitude_ft < 10 and speed > 3")),
        ],
    );
    assert_eq!(
        text_error(r),
        (
            "condition",
            TextError::Expr(ExprError::Name(NameError::UnknownField {
                name: "speed".into(),
                at: 21
            }))
        )
    );
    // A field of another kind is unknown within the rule's kinds.
    let r = call(
        &mut a,
        "rule.set",
        &[
            ("row", text("r")),
            ("kinds", text("radio.text")),
            ("condition", text("squawk == 1")),
        ],
    );
    assert_eq!(text_error(r).1.at(), 0);
    let r = call(
        &mut a,
        "rule.set",
        &[("row", text("r")), ("kinds", text("air.plane, nope.*"))],
    );
    assert_eq!(
        text_error(r),
        (
            "kinds",
            TextError::Name(NameError::UnknownKind {
                name: "nope.*".into(),
                at: 11
            })
        )
    );
    let r = call(
        &mut a,
        "rule.set",
        &[("row", text("r")), ("condition", text("a == 'open"))],
    );
    assert!(matches!(
        text_error(r).1,
        TextError::Expr(ExprError::Syntax {
            problem: Syntax::TextNotClosed,
            ..
        })
    ));
    let r = call(
        &mut a,
        "rule.set",
        &[("row", text("r")), ("from", text("25:00"))],
    );
    assert_eq!(text_error(r), ("from", TextError::Time { at: 0 }));
    call(
        &mut a,
        "rule.set",
        &[("row", text("r")), ("kinds", text("air.plane"))],
    )
    .unwrap();
    let r = call(
        &mut a,
        "action.add",
        &[
            ("rule", text("r")),
            ("title", text("Low {callsign} {nosuch}")),
        ],
    );
    assert_eq!(
        text_error(r),
        (
            "title",
            TextError::Name(NameError::UnknownField {
                name: "nosuch".into(),
                at: 16
            })
        )
    );
    assert!(rows(&a, "rule_actions").is_empty());
}

#[test]
fn a_rule_with_an_area_and_a_schedule_reads_back_joined_by_key() {
    let storage = Arc::new(MemStorage::new());
    let mut a = open(&storage);
    call(
        &mut a,
        "fence.set",
        &[
            ("row", text("harbour")),
            ("lat", Value::F64(0.0)),
            ("lon", Value::F64(0.0)),
            ("radius_km", Value::F64(5.0)),
        ],
    )
    .unwrap();
    call(
        &mut a,
        "rule.set",
        &[
            ("row", text("in_port")),
            ("kinds", text("sea.ship")),
            ("fence", text("harbour")),
        ],
    )
    .unwrap();
    let reply = call(
        &mut a,
        "action.add",
        &[
            ("rule", text("in_port")),
            ("action", Value::Choice(1)),
            ("text", text("{source} in port")),
        ],
    )
    .unwrap()
    .unwrap();
    let action_row = reply
        .get(&Field::<field::Text>::new("row", "", ""))
        .unwrap()
        .to_owned();
    call(
        &mut a,
        "schedule.set",
        &[
            ("row", text("net")),
            ("name", text("Monday net")),
            ("times", text("19:30")),
            ("days", text("0")),
        ],
    )
    .unwrap();
    let rules = rows(&a, "rules");
    let actions = rows(&a, "rule_actions");
    let fences = rows(&a, "rule_fences");
    let decl = a.results_decl();
    let rules_decl = decl.table(decl.table_id("rules").unwrap()).unwrap();
    let fence_col = rules_decl
        .columns
        .field(rules_decl.columns.id("fence").unwrap())
        .unwrap()
        .clone();
    assert_eq!(
        fence_col.refers_to.as_ref().map(|r| r.table.as_ref()),
        Some("rule_fences")
    );
    let (rule_key, rule) = &rules[0];
    let fence_key = rule
        .get(&Field::<field::Text>::new("fence", "", ""))
        .unwrap();
    assert!(fences.iter().any(|(k, _)| k == fence_key));
    let (akey, action) = &actions[0];
    assert_eq!(akey, &action_row);
    assert_eq!(
        action.get(&Field::<field::Text>::new("rule", "", "")),
        Some(rule_key.as_str())
    );
    let schedules = rows(&a, "rule_schedules");
    assert_eq!(schedules[0].0, "net");
    assert!(rows(&a, "rule_problems").is_empty());
    // The rows survive a restart through the storage.
    let b = open(&storage);
    assert_eq!(rows(&b, "rules").len(), 1);
    assert_eq!(rows(&b, "rule_actions").len(), 1);
    // A ship that enters the harbour starts the rule and returns geo.enter.
    let mut a = b;
    assert!(handle(&mut a, 0.0, &ship(0.2, 0.0, 0.0, 0.0)).is_empty());
    let out = handle(&mut a, 1.0, &ship(0.01, 0.0, 0.0, 0.0));
    assert_eq!(
        out.iter().map(Event::kind).collect::<Vec<_>>(),
        ["geo.enter"]
    );
    assert_eq!(out[0].get(&FENCE), Some("harbour"));
    let log = rows(&a, "log");
    assert_eq!(log[0].1.get(&keys::TEXT), Some("503000001 in port"));
    assert_eq!(
        handle(&mut a, 2.0, &ship(0.3, 0.0, 0.0, 0.0))[0].kind(),
        "geo.exit"
    );
    // Removing the area leaves the rule with a problem row, and the rule does not run.
    call(&mut a, "fence.remove", &[("row", text("harbour"))]).unwrap();
    let problems = rows(&a, "rule_problems");
    assert_eq!(problems[0].0, "in_port/no_fence");
}

#[test]
fn a_matching_rule_returns_the_request_event() {
    let mut a = open(&Arc::new(MemStorage::new()));
    call(
        &mut a,
        "rule.set",
        &[
            ("row", text("low")),
            ("kinds", text("air.*")),
            (
                "condition",
                text("altitude_ft < 3000 and callsign ^= 'qfa'"),
            ),
            ("cooldown", Value::Duration(60.0)),
        ],
    )
    .unwrap();
    call(
        &mut a,
        "action.add",
        &[
            ("rule", text("low")),
            ("title", text("Low {callsign}")),
            ("urgent", Value::Bool(true)),
        ],
    )
    .unwrap();
    call(
        &mut a,
        "action.add",
        &[
            ("rule", text("low")),
            ("action", Value::Choice(2)),
            ("freq", text("{freq_hz}")),
            ("stack", text("am")),
        ],
    )
    .unwrap();
    call(
        &mut a,
        "action.add",
        &[
            ("rule", text("low")),
            ("action", Value::Choice(3)),
            ("what", Value::Choice(1)),
            ("seconds", Value::Duration(20.0)),
        ],
    )
    .unwrap();
    assert!(rows(&a, "rule_problems").is_empty());
    let out = handle(&mut a, 0.0, &plane("A", 1000));
    assert_eq!(
        out.iter().map(Event::kind).collect::<Vec<_>>(),
        ["automation.notify", "automation.tune", "automation.record"]
    );
    assert_eq!(out[0].get(&TITLE), Some("Low QFA12"));
    assert_eq!(out[0].get(&keys::URGENCY), Some(Urgency::Urgent));
    assert_eq!((out[1].freq_hz, out[1].get(&STACK)), (1090e6, Some("am")));
    assert_eq!(out[2].get(&WHAT), Some(RecordWhat::Iq));
    assert_eq!(out[2].get(&SECONDS), Some(20.0));
    // The events carry their kind ids, so the engine observes them like any other event.
    assert!(kinds().observe(&out[0], &ctx(0.0)).is_some());
    // A request event does not start rules again.
    assert!(handle(&mut a, 0.5, &out[1]).is_empty());
    assert!(
        handle(&mut a, 30.0, &plane("A", 1000)).is_empty(),
        "cooldown"
    );
    assert_eq!(
        handle(&mut a, 31.0, &plane("B", 1000)).len(),
        3,
        "per object"
    );
    assert!(
        handle(&mut a, 61.0, &plane("A", 5000)).is_empty(),
        "condition"
    );
}

#[test]
fn schedules_and_windows() {
    // 2026-11-09 is a Monday.
    let monday = |h: f64| 1_794_182_400.0 + h * 3600.0;
    let mut a = open(&Arc::new(MemStorage::new()));
    call(
        &mut a,
        "schedule.set",
        &[("row", text("hourly")), ("every", Value::Duration(3600.0))],
    )
    .unwrap();
    call(
        &mut a,
        "schedule.set",
        &[
            ("row", text("net")),
            ("times", text("19:30")),
            ("days", text("0")),
        ],
    )
    .unwrap();
    call(
        &mut a,
        "schedule.set",
        &[
            ("row", text("office")),
            ("every", Value::Duration(3600.0)),
            ("from", text("08:00")),
            ("to", text("10:00")),
        ],
    )
    .unwrap();
    let mut out = Vec::new();
    a.tick(monday(-24.0), 0, &mut out);
    assert!(out.is_empty(), "the first tick starts the clock");
    a.tick(monday(24.0 * 6.0), 0, &mut out);
    let of = |key: &str| {
        out.iter()
            .filter(|e| e.get(&SCHEDULE) == Some(key))
            .map(|e| e.time)
            .collect::<Vec<_>>()
    };
    assert_eq!(of("net"), [monday(19.5)]);
    assert_eq!(of("office").len(), 14);
    assert_eq!(of("hourly").len(), 24 * 7);
    let mut out = Vec::new();
    let mut b = open(&Arc::new(MemStorage::new()));
    call(
        &mut b,
        "schedule.set",
        &[("row", text("net")), ("times", text("19:30"))],
    )
    .unwrap();
    b.tick(monday(0.0), 600, &mut out);
    b.tick(monday(24.0), 600, &mut out);
    assert_eq!(
        out.iter().map(|e| e.time).collect::<Vec<_>>(),
        [monday(9.5)]
    );
    // A window over midnight: Monday 22:00 to 06:00.
    let mut c = open(&Arc::new(MemStorage::new()));
    call(
        &mut c,
        "rule.set",
        &[
            ("row", text("night")),
            ("days", text("0")),
            ("from", text("22:00")),
            ("to", text("06:00")),
        ],
    )
    .unwrap();
    call(&mut c, "action.add", &[("rule", text("night"))]).unwrap();
    let fired = |c: &mut Automation, t: f64| !handle(c, t, &plane("N", 0)).is_empty();
    assert!(fired(&mut c, monday(23.0)));
    assert!(!fired(&mut c, monday(3.0)));
    assert!(fired(&mut c, monday(27.0)));
    assert!(!fired(&mut c, monday(12.0)));
}

#[test]
fn the_closest_approach_warns_once() {
    let mut a = open(&Arc::new(MemStorage::new()));
    let set = |a: &mut Automation, key: &str, v: Value| {
        let id = a.schema().id(key).unwrap();
        owner::apply(a, id, &v)
    };
    set(&mut a, "cpa_on", Value::Bool(true)).unwrap();
    // 5 km west, heading east at 12 kt: passes over the receiver in about 13 minutes.
    let at = Position::new(0.0, 0.0).destination(270.0, 5000.0);
    let out = handle(&mut a, 0.0, &ship(at.lat_deg, at.lon_deg, 12.0, 90.0));
    assert_eq!(out.len(), 1);
    assert!(out[0].get(&CPA_KM).unwrap() < 0.1);
    assert!(handle(&mut a, 10.0, &ship(at.lat_deg, at.lon_deg, 12.0, 90.0)).is_empty());
    let mut b = open(&Arc::new(MemStorage::new()));
    set(&mut b, "cpa_on", Value::Bool(true)).unwrap();
    assert!(handle(&mut b, 0.0, &ship(at.lat_deg, at.lon_deg, 12.0, 270.0)).is_empty());
    assert!(handle(&mut b, 0.0, &ship(at.lat_deg, at.lon_deg, 102.3, 90.0)).is_empty());
}

#[test]
fn conditions() {
    let mut a = open(&Arc::new(MemStorage::new()));
    let holds = |a: &mut Automation, cond: &str| {
        call(
            a,
            "rule.set",
            &[
                ("row", text("c")),
                ("kinds", text("air.plane")),
                ("condition", text(cond)),
            ],
        )
        .unwrap_or_else(|e| panic!("{cond}: {e}"));
        if rows(a, "rule_actions").is_empty() {
            call(a, "action.add", &[("rule", text("c"))]).unwrap();
        }
        !handle(a, 0.0, &plane("7C6DB8", 9000)).is_empty()
    };
    for c in [
        "",
        "altitude_ft < 10000 and callsign ^= 'qfa'",
        "squawk in ['7500', '7600', '7700']",
        "squawk == 7700",
        "family == 'aircraft'",
        "not exists(text) and exists(callsign)",
        "distance_km > 5",
        "callsign ~ \"a1\" and callsign $= '12'",
        "callsign != 'x'",
        "(kind == 'radio.text') or kind == 'air.plane'",
        "!(altitude_ft > 1e5)",
        "len(callsign) == 5 and lower(callsign) == 'qfa12' and upper('a') == 'A'",
        "abs(-3) == 3 and within(-33.0, 151.0, 1)",
        "freq_mhz > 1089.9 and -5 < 0",
        "source = '7c6db8'",
    ] {
        assert!(holds(&mut a, c), "{c}");
    }
    for c in [
        "altitude_ft >= 10000",
        "callsign == 'x'",
        "within(0, 0, 100)",
    ] {
        assert!(!holds(&mut a, c), "{c}");
    }
    let err = |a: &mut Automation, cond: &str| {
        let r = call(
            a,
            "rule.set",
            &[("row", text("e")), ("condition", text(cond))],
        );
        text_error(r).1
    };
    assert_eq!(err(&mut a, "altitude_ft <").at(), 13);
    assert!(matches!(
        err(&mut a, "nope(1)"),
        TextError::Expr(ExprError::Syntax {
            problem: Syntax::UnknownFunction,
            ..
        })
    ));
    assert!(matches!(
        err(&mut a, "exists(1)"),
        TextError::Expr(ExprError::Syntax {
            problem: Syntax::ExistsTakesAName,
            ..
        })
    ));
    for c in [
        "(squawk == 1",
        "squawk == 1 squawk",
        "squawk # 1",
        "within(1, 2)",
    ] {
        assert!(
            matches!(err(&mut a, c), TextError::Expr(ExprError::Syntax { .. })),
            "{c}"
        );
    }
}

#[test]
fn a_rule_without_an_action_is_a_problem_row() {
    let mut a = open(&Arc::new(MemStorage::new()));
    call(&mut a, "rule.set", &[("row", text("r"))]).unwrap();
    let p = rows(&a, "rule_problems");
    assert_eq!(p[0].0, "r/no_action");
    call(&mut a, "action.add", &[("rule", text("r"))]).unwrap();
    assert!(rows(&a, "rule_problems").is_empty());
    call(&mut a, "rule.remove", &[("row", text("r"))]).unwrap();
    assert!(rows(&a, "rule_actions").is_empty());
}

#[test]
fn files_go_through_storage() {
    let storage = Arc::new(MemStorage::new());
    let mut a = open(&storage);
    storage.fail_next(StorageError::NoSpace);
    let Err(CommandError::Failed { source }) = call(&mut a, "rule.set", &[("row", text("r"))])
    else {
        panic!("a failed write is an error")
    };
    assert!(matches!(
        source.downcast_ref::<AutomationError>(),
        Some(AutomationError::Storage {
            source: StorageError::NoSpace,
            ..
        })
    ));
    storage.fail_next(StorageError::Denied);
    assert!(matches!(
        Automation::open(kinds(), storage.clone()),
        Err(AutomationError::Storage {
            source: StorageError::Denied,
            ..
        })
    ));
}

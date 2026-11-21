//! One event through every service, as the engine runs them: the store, the session book, the automation, the watch and
//! the outputs. Events that the services give go round again.

use std::sync::Arc;

use sdrkit_core::net::FakeTransport;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::{Clock, Event, EventCatalog, EventDef, MemStorage, Value};
use sdrkit_svc_automation::events::{SECONDS, TITLE};
use sdrkit_svc_automation::{Automation, AUTOMATION_EVENTS};
use sdrkit_svc_events::{Context, Kinds, Position};
use sdrkit_svc_outputs::Outputs;
use sdrkit_svc_store::session::{SessionBook, SESSION_EVENTS};
use sdrkit_svc_store::Store;
use sdrkit_svc_watch::{Watch, WATCH_EVENTS};

static PLANE: EventDef = EventDef::new(
    "adsb.aircraft",
    "Aircraft",
    &[
        &keys::SOURCE,
        &keys::FAMILY,
        &keys::URGENCY,
        &keys::TEXT,
        &keys::POSITION,
    ],
);

fn call(o: &mut dyn Owner, cmd: &str, args: &[(&str, &str)]) {
    let id = o.commands().id(cmd).unwrap();
    let mut given = o.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let i = given.decl().id(k).unwrap();
        let v = match given.decl().field(i).map(|d| d.kind) {
            Some(sdrkit_core::ValueKind::F64) => Value::F64(v.parse().unwrap()),
            Some(sdrkit_core::ValueKind::Duration) => Value::Duration(v.parse().unwrap()),
            Some(sdrkit_core::ValueKind::Choice) => Value::Choice(v.parse().unwrap()),
            _ => Value::Text((*v).into()),
        };
        given.set_value(i, &v).unwrap();
    }
    owner::invoke(o, id, &given).unwrap();
}

#[test]
fn an_emergency_aircraft_entering_an_area_flows_through_every_service() {
    let mut defs: Vec<&'static EventDef> = vec![&PLANE];
    defs.extend(SESSION_EVENTS);
    defs.extend(AUTOMATION_EVENTS);
    defs.extend(WATCH_EVENTS);
    let kinds = Arc::new(Kinds::new(Arc::new(EventCatalog::new(&defs).unwrap())));
    let storage = Arc::new(MemStorage::new());
    let mut store = Store::in_memory(Arc::clone(&kinds), storage.clone());
    let mut sessions = SessionBook::new();
    let mut auto = Automation::open(Arc::clone(&kinds), storage.clone()).unwrap();
    let mut watch = Watch::open(
        Arc::clone(&kinds),
        storage.clone(),
        Arc::new(FakeTransport::new()),
        Clock::fixed(0.0),
    )
    .unwrap();
    let mut outputs = Outputs::open(
        Arc::clone(&kinds),
        storage.clone(),
        Arc::new(FakeTransport::new()),
        Clock::fixed(0.0),
    )
    .unwrap();
    call(
        &mut auto,
        "fence.set",
        &[
            ("row", "airport"),
            ("lat", "-33.95"),
            ("lon", "151.18"),
            ("radius_km", "10"),
        ],
    );
    call(
        &mut auto,
        "rule.set",
        &[("row", "rec"), ("kinds", "adsb.*")],
    );
    call(
        &mut auto,
        "action.add",
        &[("rule", "rec"), ("action", "4"), ("seconds", "30")],
    );
    call(
        &mut auto,
        "rule.set",
        &[
            ("row", "zone"),
            ("kinds", "geo.enter"),
            ("condition", "fence == 'airport'"),
        ],
    );
    call(
        &mut auto,
        "action.add",
        &[("rule", "zone"), ("title", "{source} entered {fence}")],
    );
    let receiver = Position::new(-33.9, 151.2);
    let ctx = Context {
        unix_time: 1000.0,
        receiver: Some(receiver),
    };
    let mut e = PLANE
        .event(1.0, 1090e6)
        .set(&keys::SOURCE, "7C6DB8")
        .set(&keys::FAMILY, Family::Aircraft)
        .set(
            &keys::URGENCY,
            sdrkit_core::publish::keys::Urgency::Emergency,
        )
        .set(&keys::TEXT, "squawk 7700")
        .set(&keys::POSITION, (-33.94, 151.17));
    kinds.catalog().stamp(&mut e);
    let mut queue = vec![e];
    let mut seen: Vec<Event> = Vec::new();
    while let Some(mut ev) = queue.pop() {
        // The engine stamps the kind id of each event when it enters.
        kinds.catalog().stamp(&mut ev);
        let Some(o) = kinds.observe(&ev, &ctx) else {
            continue;
        };
        store.insert(&o).unwrap();
        let mut out = Vec::new();
        out.extend(sessions.observe(&o).into_iter().map(|(_, e)| e));
        auto.handle(&o, &ctx, 0, &mut out);
        watch.check(&o, 0, &mut out);
        outputs.submit(&o);
        queue.extend(out);
        seen.push(ev);
    }
    let kinds_seen: Vec<&str> = seen.iter().map(Event::kind).collect();
    for k in [
        "adsb.aircraft",
        "session.begin",
        "geo.enter",
        "automation.rewind",
        "automation.notify",
        "watch.alert",
    ] {
        assert!(kinds_seen.contains(&k), "{k} in {kinds_seen:?}");
    }
    let rewind = seen
        .iter()
        .find(|e| e.kind() == "automation.rewind")
        .unwrap();
    assert_eq!(rewind.get(&SECONDS), Some(30.0));
    let note = seen
        .iter()
        .find(|e| e.kind() == "automation.notify")
        .unwrap();
    assert_eq!(note.get(&TITLE), Some("7C6DB8 entered airport"));
    let alert = seen.iter().find(|e| e.kind() == "watch.alert").unwrap();
    assert!(alert.get(&keys::TEXT).unwrap().contains("7700"));
    let m = store
        .iter()
        .find(|m| &*m.kind_key == "adsb.aircraft")
        .unwrap();
    assert!(m.distance_km.unwrap() < 10.0 && m.urgency != keys::Urgency::None);
    assert_eq!(sessions.end_all().len(), 1);
}

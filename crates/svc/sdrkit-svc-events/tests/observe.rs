//! Observations read standard fields by definition, and names in user text resolve against the catalog.

use std::sync::Arc;

use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::{field, Field};
use sdrkit_core::{Event, EventCatalog, EventDef};
use sdrkit_svc_events::{Context, FieldRef, Kinds, NameError, Position, Reading, Standard};

const ALTITUDE_FT: Field<field::I64> = Field::new("altitude_ft", "Altitude", "ft");
static PLANE: EventDef = EventDef::new(
    "test.plane",
    "Plane seen",
    &[
        &keys::SOURCE,
        &keys::URGENCY,
        &keys::FAMILY,
        &keys::POSITION,
        &keys::TEXT,
        &ALTITUDE_FT,
    ],
);
static CALL: EventDef = EventDef::new(
    "other.call",
    "Call",
    &[&keys::SOURCE, &keys::TEXT, &keys::PRIVATE],
);

fn kinds() -> Kinds {
    Kinds::new(Arc::new(EventCatalog::new(&[&PLANE, &CALL]).unwrap()))
}

/// `e` with its kind id, as the engine stamps it when the event enters.
fn stamped(k: &Kinds, mut e: Event) -> Event {
    k.catalog().stamp(&mut e);
    e
}

#[test]
fn the_urgency_field_makes_an_event_urgent() {
    let k = kinds();
    let e = stamped(
        &k,
        PLANE
            .event(1.0, 1090e6)
            .set(&keys::SOURCE, "7c1234")
            .set(&keys::URGENCY, Urgency::Emergency)
            .set(&keys::FAMILY, Family::Aircraft)
            .set(&keys::POSITION, (-33.9, 151.2)),
    );
    let ctx = Context {
        unix_time: 100.0,
        receiver: Some(Position::new(-33.8, 151.2)),
    };
    let o = k.observe(&e, &ctx).unwrap();
    assert!(o.is_urgent());
    assert_eq!(o.urgency, Urgency::Emergency);
    assert_eq!(o.family, Family::Aircraft);
    assert_eq!(o.identity, Some("7c1234"));
    assert!((o.distance_km.unwrap() - 11.1).abs() < 0.2);
    assert_eq!(
        o.read(&FieldRef::Standard(Standard::Urgency)),
        Some(Reading::Text("emergency"))
    );

    let quiet = stamped(&k, PLANE.event(1.0, 1090e6).set(&keys::SOURCE, "7c1234"));
    let o = k.observe(&quiet, &ctx).unwrap();
    assert!(!o.is_urgent());
    assert_eq!(o.family, Family::Other);
}

#[test]
fn private_content_returns_no_words() {
    let k = kinds();
    let e = stamped(
        &k,
        CALL.event(1.0, 1.0)
            .set(&keys::TEXT, "secret")
            .set(&keys::PRIVATE, true),
    );
    let o = k.observe(&e, &Context::default()).unwrap();
    assert_eq!(o.text, None);
    assert_eq!(o.words().count(), 0);
    let open = stamped(&k, CALL.event(1.0, 1.0).set(&keys::TEXT, "hello"));
    let o = k.observe(&open, &Context::default()).unwrap();
    assert_eq!(o.words().collect::<Vec<_>>(), ["hello"]);
}

#[test]
fn a_declared_field_reads_by_id() {
    let k = kinds();
    let f = k.field("altitude_ft", 0, None).unwrap();
    let e = stamped(&k, PLANE.event(1.0, 1.0).set(&ALTITUDE_FT, 2500));
    let o = k.observe(&e, &Context::default()).unwrap();
    assert_eq!(o.read(&f), Some(Reading::Number(2500.0)));
    let c = stamped(&k, CALL.event(1.0, 1.0));
    let o = k.observe(&c, &Context::default()).unwrap();
    assert_eq!(o.read(&f), None);
}

#[test]
fn unknown_names_fail_with_their_position() {
    let k = kinds();
    assert_eq!(
        k.field("altitude_m", 7, None),
        Err(NameError::UnknownField {
            name: "altitude_m".into(),
            at: 7
        })
    );
    let calls = k.kinds("other.*", 0).unwrap();
    assert_eq!(
        k.field("altitude_ft", 3, Some(&calls)),
        Err(NameError::UnknownField {
            name: "altitude_ft".into(),
            at: 3
        })
    );
    assert_eq!(
        k.kinds("adsb.*", 12),
        Err(NameError::UnknownKind {
            name: "adsb.*".into(),
            at: 12
        })
    );
    assert!(k
        .kinds("*", 0)
        .unwrap()
        .contains(k.catalog().id("test.plane").unwrap()));
    assert_eq!(Kinds::family("aircraft", 0), Ok(Family::Aircraft));
    assert!(matches!(
        Kinds::family("planes", 4),
        Err(NameError::UnknownFamily { at: 4, .. })
    ));
}

#[test]
fn an_event_outside_the_catalog_returns_no_observation() {
    static STRAY: EventDef = EventDef::new("stray.kind", "Stray", &[]);
    let k = kinds();
    let e = stamped(&k, STRAY.event(0.0, 0.0));
    assert!(k.observe(&e, &Context::default()).is_none());
}

#[test]
fn an_event_without_a_kind_id_returns_no_observation() {
    let k = kinds();
    let e = PLANE.event(0.0, 0.0);
    assert!(k.observe(&e, &Context::default()).is_none());
    let e = stamped(&k, e);
    assert!(k.observe(&e, &Context::default()).is_some());
}

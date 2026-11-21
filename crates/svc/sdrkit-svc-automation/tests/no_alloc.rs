//! The event path allocates nothing when no rule, area or warning takes the event.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::{EventCatalog, EventDef, MemStorage, Value};
use sdrkit_svc_automation::{Automation, AUTOMATION_EVENTS};
use sdrkit_svc_events::{Context, Kinds, Position};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

static PLANE: EventDef = EventDef::new(
    "air.plane",
    "Plane",
    &[&keys::SOURCE, &keys::FAMILY, &keys::POSITION],
);
static SHIP: EventDef = EventDef::new("sea.ship", "Ship", &[&keys::SOURCE, &keys::FAMILY]);

#[test]
fn an_event_that_nothing_takes_allocates_nothing() {
    let mut defs: Vec<&'static EventDef> = vec![&PLANE, &SHIP];
    defs.extend(AUTOMATION_EVENTS);
    let kinds = Arc::new(Kinds::new(Arc::new(EventCatalog::new(&defs).unwrap())));
    let mut a = Automation::open(Arc::clone(&kinds), Arc::new(MemStorage::new())).unwrap();
    let call = |a: &mut Automation, cmd: &str, args: &[(&str, &str)]| {
        let id = a.commands().id(cmd).unwrap();
        let mut given = a.commands().def(id).unwrap().new_args();
        for (k, v) in args {
            let i = given.decl().id(k).unwrap();
            given.set_value(i, &Value::Text((*v).into())).unwrap();
        }
        owner::invoke(a, id, &given).unwrap();
    };
    call(&mut a, "rule.set", &[("row", "r"), ("kinds", "sea.ship")]);
    call(
        &mut a,
        "action.add",
        &[("rule", "r"), ("title", "{source}")],
    );
    call(
        &mut a,
        "fence.set",
        &[("row", "far"), ("families", "vessel")],
    );
    let mut e = PLANE
        .event(1.0, 1090e6)
        .set(&keys::SOURCE, "7c6db8")
        .set(&keys::FAMILY, Family::Aircraft)
        .set(&keys::POSITION, (-33.0, 151.0));
    kinds.catalog().stamp(&mut e);
    let ctx = Context {
        unix_time: 1.0,
        receiver: Some(Position::new(-33.0, 151.0)),
    };
    let mut out = Vec::with_capacity(16);
    let o = kinds.observe(&e, &ctx).unwrap();
    a.handle(&o, &ctx, 0, &mut out);
    let before = ALLOCS.load(Ordering::Relaxed);
    for i in 0..1000 {
        let c = Context {
            unix_time: 2.0 + f64::from(i),
            ..ctx
        };
        let o = kinds.observe(&e, &c).unwrap();
        a.handle(&o, &c, 0, &mut out);
    }
    assert_eq!(ALLOCS.load(Ordering::Relaxed) - before, 0);
    assert!(out.is_empty());
}

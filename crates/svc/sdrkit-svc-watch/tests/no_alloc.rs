//! The event path allocates nothing when no rule, area, watch or output takes the event, with the automation, the outputs
//! and the watch attached.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sdrkit_core::net::FakeTransport;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::{Clock, EventCatalog, EventDef, MemStorage, Value};
use sdrkit_svc_automation::{Automation, AUTOMATION_EVENTS};
use sdrkit_svc_events::{Context, Kinds, Position};
use sdrkit_svc_outputs::Outputs;
use sdrkit_svc_watch::{Watch, WATCH_EVENTS};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Allocations count on the thread of the test only. The threads of the outputs run on their own.
    static COUNT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNT.with(std::cell::Cell::get) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
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

fn call(o: &mut dyn Owner, cmd: &str, args: &[(&str, Value)]) {
    let id = o.commands().id(cmd).unwrap();
    let mut given = o.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let i = given.decl().id(k).unwrap();
        given.set_value(i, v).unwrap();
    }
    owner::invoke(o, id, &given).unwrap();
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

#[test]
fn an_event_that_nothing_takes_allocates_nothing() {
    let mut defs: Vec<&'static EventDef> = vec![&PLANE, &SHIP];
    defs.extend(AUTOMATION_EVENTS);
    defs.extend(WATCH_EVENTS);
    let kinds = Arc::new(Kinds::new(Arc::new(EventCatalog::new(&defs).unwrap())));
    let storage = Arc::new(MemStorage::new());
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
        "rule.set",
        &[("row", text("r")), ("kinds", text("sea.ship"))],
    );
    call(&mut auto, "action.add", &[("rule", text("r"))]);
    call(
        &mut auto,
        "fence.set",
        &[("row", text("f")), ("families", text("vessel"))],
    );
    call(
        &mut watch,
        "add",
        &[("row", text("w")), ("value", text("503123456"))],
    );
    call(
        &mut watch,
        "add",
        &[
            ("row", text("v")),
            ("families", text("vessel")),
            ("value", text("*")),
        ],
    );
    call(
        &mut outputs,
        "output.add",
        &[
            ("row", text("u")),
            ("kind", Value::Choice(3)),
            ("address", text("127.0.0.1:9")),
            ("enabled", Value::Bool(true)),
            ("kinds", text("sea.ship")),
        ],
    );
    call(&mut outputs, "start", &[]);
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
    let mut path = |c: &Context, out: &mut Vec<sdrkit_core::Event>| {
        let o = kinds.observe(&e, c).unwrap();
        auto.handle(&o, c, 0, out);
        watch.check(&o, 0, out);
        outputs.submit(&o);
    };
    path(&ctx, &mut out);
    COUNT.with(|c| c.set(true));
    let before = ALLOCS.load(Ordering::Relaxed);
    for i in 0..1000 {
        let c = Context {
            unix_time: 2.0 + f64::from(i),
            ..ctx
        };
        path(&c, &mut out);
    }
    let after = ALLOCS.load(Ordering::Relaxed);
    COUNT.with(|c| c.set(false));
    assert_eq!(after - before, 0);
    assert!(out.is_empty());
}

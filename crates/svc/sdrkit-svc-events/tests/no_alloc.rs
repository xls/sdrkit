//! Reading an observation allocates nothing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sdrkit_core::publish::keys::{self, Urgency};
use sdrkit_core::{EventCatalog, EventDef};
use sdrkit_svc_events::{Context, Kinds};

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
    "test.plane",
    "Plane",
    &[&keys::SOURCE, &keys::URGENCY, &keys::POSITION, &keys::TEXT],
);

#[test]
fn observing_an_event_allocates_nothing() {
    let k = Kinds::new(Arc::new(EventCatalog::new(&[&PLANE]).unwrap()));
    let mut e = PLANE
        .event(1.0, 1.0)
        .set(&keys::SOURCE, "abc")
        .set(&keys::URGENCY, Urgency::Urgent)
        .set(&keys::POSITION, (1.0, 2.0));
    k.catalog().stamp(&mut e);
    let ctx = Context::default();
    let before = ALLOCS.load(Ordering::Relaxed);
    let mut urgent = 0;
    for _ in 0..1000 {
        let o = k.observe(&e, &ctx).unwrap();
        urgent += usize::from(o.is_urgent()) + o.words().count();
    }
    assert_eq!(ALLOCS.load(Ordering::Relaxed) - before, 0);
    assert_eq!(urgent, 2000);
}

//! The step path of a running scan allocates nothing.

use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{Clock, LockState, MemStorage, Status, Value};
use sdrkit_svc_memory::MemoryFilter;
use sdrkit_svc_scan::{ScanService, Step};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

struct Counting;

static COUNT: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call goes to the system allocator unchanged. The counter only reads a flag and adds one.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNT.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the layout comes from the caller, as for the system allocator.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` was given by `alloc` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNT.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: `p` was given by `alloc` with layout `l`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const SQUELCH: Field<field::Bool> = Field::new("squelch_open", "Squelch open", "");
const TONE: Field<field::Text> = Field::new("tone", "Tone", "");

#[test]
fn stepping_a_band_scan_allocates_nothing() {
    let storage = Arc::new(MemStorage::new());
    let clock = Clock::fixed(0.0);
    let mut scan = ScanService::open(
        storage,
        clock,
        Arc::new(|_: &str| None),
        Arc::new(|_: &MemoryFilter| Vec::new()),
    )
    .unwrap();
    let id = scan.commands().id("band").unwrap();
    let mut args = scan.commands().def(id).unwrap().new_args();
    for (k, v) in [
        ("session", Value::Text("1".into())),
        ("start_hz", Value::Hz(144e6)),
        ("stop_hz", Value::Hz(146e6)),
        ("step_hz", Value::Hz(12.5e3)),
        ("stack", Value::Text("general".into())),
    ] {
        args.set_value(args.decl().id(k).unwrap(), &v).unwrap();
    }
    owner::invoke(&mut scan, id, &args).unwrap();
    let mut p = Published::new(&[&SQUELCH, &TONE]);
    p.set(&SQUELCH, false);
    p.set(&TONE, "");
    let mut t = 0.0;
    let mut tunes = 0;
    let step = |scan: &mut ScanService, p: &mut Published, t: &mut f64, tunes: &mut usize| {
        p.results_mut()
            .set_status(&Status::new(LockState::Searching));
        if let Some(Step::Tune) = scan.step(*t, Some(p.results())) {
            *tunes += 1;
            assert!(scan.target().is_some());
        }
        *t += 0.05;
    };
    // The first steps make the scanner and fill the results.
    for _ in 0..100 {
        step(&mut scan, &mut p, &mut t, &mut tunes);
    }
    COUNT.store(true, Ordering::Relaxed);
    for _ in 0..2000 {
        step(&mut scan, &mut p, &mut t, &mut tunes);
    }
    COUNT.store(false, Ordering::Relaxed);
    assert!(tunes > 100, "the scan moved on: {tunes} tunes");
    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0);
}

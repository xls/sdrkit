//! The receive path of the FT8 stack allocates nothing per block once warm, between two slot ends.

use sdrkit_core::{Choice, Complex32, GenConfig, InputInfo, Output, StackConfig, Value, Values};
use sdrkit_stack_wsjt::slots::TimeChoice;
use sdrkit_stack_wsjt::Mode;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

/// The allocations of one call of `f`.
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCS.store(0, Ordering::Relaxed);
    COUNT.store(true, Ordering::Relaxed);
    f();
    COUNT.store(false, Ordering::Relaxed);
    ALLOCS.load(Ordering::Relaxed)
}

#[test]
fn the_ft8_receiver_allocates_nothing_between_slot_ends() {
    const RATE: f64 = 48_000.0;
    let schema = sdrkit_stack_wsjt::settings::schema_for(Some(Mode::Ft8));
    let mut v = Values::defaults(&schema);
    let set = |v: &mut Values, k: &str, x: Value| v.set(schema.id(k).unwrap(), &x).unwrap();
    set(
        &mut v,
        "time_source",
        Value::Choice(TimeChoice::Start.index()),
    );
    set(
        &mut v,
        "time_start",
        Value::Text("2026-01-01T00:00:00Z".into()),
    );
    let desc = sdrkit_stack_wsjt::descriptors()
        .into_iter()
        .find(|d| d.id == "ft8")
        .unwrap();
    let mut g = (desc.make_generator.unwrap())(&GenConfig::new(RATE, v.clone())).unwrap();
    let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(RATE, 14.074e6, 14.074e6).recording())
        .unwrap();
    // Blocks of 0.1 s. A slot of 15 s ends and is decoded near 13 s into it, so the blocks from 16 s to 26 s of the second slot
    // carry no slot end.
    let block = (RATE * 0.1) as usize;
    let mut iq = vec![Complex32::default(); block];
    let mut out = Output::default();
    let mut busy = Vec::new();
    for i in 0..280 {
        g.generate(&mut iq);
        let n = allocations(|| rx.feed(&iq, &mut out));
        out.clear();
        if (160..260).contains(&i) && n > 0 {
            busy.push((i, n));
        }
    }
    assert!(busy.is_empty(), "blocks that allocated: {busy:?}");
}

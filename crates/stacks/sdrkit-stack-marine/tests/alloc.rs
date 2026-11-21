//! Steady-state decoding allocates little: once warmed up, a second of each stack's test signal costs no more allocations than
//! the limit of the stack. The count is kept for each thread, so tests running beside this one do not add to it.

use sdrkit_core::{InputInfo, Output, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_marine::{AIS, ATIS, MARINE, MARINEVHF, VDES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

fn count() {
    // A thread that is ending has no counter left: its allocations are not counted.
    let _ = COUNT.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call is forwarded unchanged to the system allocator, and the counter is a thread-local cell that never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The allocations of one second of the stack's test signal through its stack graph, after two seconds to warm up, and the
/// blocks fed in that second.
fn allocations_per_second(
    d: &'static StackDescriptor,
    sub: Option<&str>,
    rate: f64,
    centre: f64,
    tuned: f64,
) -> (u64, u64) {
    let mut h = Harness::new(d).rate(rate).freq(tuned).seconds(3.0);
    if let Some(s) = sub {
        h = h.submode(s);
    }
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .unwrap();
    let mut out = Output::default();
    let warm = (2.0 * rate) as usize;
    let block = 8192;
    for c in x[..warm].chunks(block) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let before = COUNT.with(Cell::get);
    let mut blocks = 0;
    for c in x[warm..].chunks(block) {
        out.clear();
        rx.feed(c, &mut out);
        blocks += 1;
    }
    (COUNT.with(Cell::get) - before, blocks)
}

#[test]
fn steady_state_allocations_are_few() {
    for (d, sub, rate, centre, tuned, limit) in [
        (&AIS, None, 250e3, 162e6, 162e6, 1000),
        (&MARINE, Some("navtex"), 48e3, 518e3, 518e3, 40),
        (&MARINE, Some("dsc"), 48e3, 2187.5e3, 2187.5e3, 40),
        (&VDES, None, 400e3, 161.9e6, 161.9e6, 450),
        (&MARINEVHF, None, 1e6, 156.8e6, 156.8e6, 5000),
        (&ATIS, None, 48e3, 156.5e6, 156.5e6, 20),
    ] {
        let (n, blocks) = allocations_per_second(d, sub, rate, centre, tuned);
        println!("{} {sub:?}: {n} allocations in {blocks} blocks", d.id);
        assert!(n <= limit, "{} {sub:?}: {n} allocations a second", d.id);
    }
}

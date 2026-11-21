//! Steady-state decoding does not allocate for each call: once warmed up, a second of signal costs no allocation, or a few
//! for each thing decoded (an identification word, a scan of the band, a frame), never one for each sample or pulse. The count
//! is kept for each thread, so tests running beside this one do not add to it.

use sdrkit_core::{InputInfo, Output, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_avnav::{DGPS, DME, ILS, LORAN, NDB, VOR};
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

fn allocations_per_second(
    d: &'static StackDescriptor,
    submode: Option<&str>,
    rate: f64,
    freq: f64,
) -> u64 {
    let mut h = Harness::new(d).rate(rate).freq(freq).seconds(4.0);
    if let Some(s) = submode {
        h = h.submode(s);
    }
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, freq, freq).recording())
        .unwrap();
    let mut out = Output::default();
    let half = x.len() / 2;
    for c in x[..half].chunks(8192) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let before = COUNT.with(Cell::get);
    for c in x[half..].chunks(8192) {
        out.clear();
        rx.feed(c, &mut out);
    }
    (COUNT.with(Cell::get) - before) / 2
}

#[test]
fn steady_state_allocations_are_few() {
    for (d, submode, rate, freq, limit) in [
        (&NDB, None, 250e3, 344e3, 30),
        (&VOR, None, 250e3, 113.1e6, 1),
        (&ILS, None, 250e3, 110.3e6, 1),
        (&ILS, Some("marker"), 250e3, 75e6, 1),
        (&DME, None, 2e6, 978e6, 30),
        (&LORAN, None, 250e3, 100e3, 20),
        (&DGPS, None, 250e3, 304e3, 40),
    ] {
        let n = allocations_per_second(d, submode, rate, freq);
        assert!(
            n < limit,
            "{}: {n} allocations a second at {rate} sps",
            d.id
        );
    }
}

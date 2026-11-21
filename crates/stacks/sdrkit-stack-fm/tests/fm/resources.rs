//! The FM graph allocates nothing once warmed up (a counting allocator counts it), and its memory stays put.

use super::common::{sig, values};
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, InputInfo, Output, StackConfig};
use sdrkit_stack_fm::typed::FmFields;
use sdrkit_stack_fm::GenParams;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts the allocations, and the bytes held, of the current thread.
struct Counting;
thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static LIVE: Cell<i64> = const { Cell::new(0) };
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        let _ = LIVE.try_with(|c| c.set(c.get() + l.size() as i64));
        // SAFETY: forwards to the system allocator with the same layout.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        let _ = LIVE.try_with(|c| c.set(c.get() - l.size() as i64));
        // SAFETY: `p` came from `alloc` above with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        let _ = LIVE.try_with(|c| c.set(c.get() + n as i64 - l.size() as i64));
        // SAFETY: as for `alloc` and `dealloc`.
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.with(|c| c.get())
}
fn live() -> i64 {
    LIVE.with(|c| c.get())
}

#[test]
fn the_fm_graph_allocates_nothing_once_warmed_up_and_keeps_its_memory() {
    let rate = 2.4e6;
    let mut g = sig(
        GenParams {
            rate_hz: rate,
            gain: 0.3,
            ..Default::default()
        },
        30.0,
    )
    .source();
    let mut x = vec![Complex32::default(); (rate * 6.0) as usize];
    g.generate(&mut x);
    let before_rx = live();
    let mut rx =
        (sdrkit_stack_fm::DESCRIPTOR.make_receiver)(&StackConfig::new(values(&[]))).unwrap();
    rx.configure(&InputInfo::new(rate, 100e6, 100e6).recording())
        .unwrap();
    let mut out = Output::default();
    let (warm, steady) = x.split_at((rate * 4.0) as usize);
    // Warm-up: buffers grow to their size, the station's name and text come in.
    for b in warm.chunks(65_536) {
        out.clear();
        rx.feed(b, &mut out);
    }
    let memory = live() - before_rx;
    let a = allocs();
    for b in steady.chunks(65_536) {
        out.clear();
        rx.feed(b, &mut out);
    }
    let made = allocs() - a;
    let after = live() - before_rx;
    assert_eq!(
        Owner::results(rx.as_ref()).fields().at(&FmFields::STATION),
        Some("SDRKIT  ")
    );
    eprintln!("FM at 2.4 Msps: {memory} bytes after warm-up, {after} after 2 s more, {made} allocations in those 2 s");
    assert_eq!(made, 0, "allocations in 2 s of signal after warm-up");
    assert!(
        after <= memory,
        "memory grew from {memory} to {after} bytes"
    );
    assert!(memory < 4_000_000, "the FM graph holds {memory} bytes");
}

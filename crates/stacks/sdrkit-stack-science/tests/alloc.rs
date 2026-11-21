//! Allocations on the receive path of the `same` stack, counted by an allocator that counts only on the thread that enables
//! it, so other tests that run at the same time do not change the count.

use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, StackConfig};
use sdrkit_stack_science::same::{GenMessage, SameGenerator};
use std::alloc::{GlobalAlloc, Layout, System as Heap};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn note() {
    // `try_with` fails only while the thread ends. Nothing is counted then.
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call goes to the system allocator with the same arguments. `note` only touches thread-local cells.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { Heap.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations that `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    f();
    COUNTING.with(|on| on.set(false));
    ALLOCS.with(|n| n.get())
}

const RATE: f64 = 250e3;
const BLOCK: usize = 4096;

/// Once warm, the SAME receive path (the channel, the FM audio, the burst reader, the tone detector, the alerts with their
/// measure of the shared spectrum, and the listen path) allocates nothing while it hears the weather channel's carrier.
#[test]
fn the_same_receive_path_allocates_nothing_once_warm() {
    let values = sdrkit_dev_test::Harness::new(&sdrkit_stack_science::SAME)
        .set(&sdrkit_stack_science::same::LISTEN, true)
        .values()
        .unwrap();
    let mut rx = sdrkit_stack_science::same::receiver(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(RATE, 162.55e6, 162.55e6).recording())
        .unwrap();
    // A message, then the carrier alone.
    let mut g = SameGenerator::new(
        RATE,
        &[GenMessage::new(sdrkit_stack_science::same::WEEKLY)],
        1,
        false,
    );
    let mut x = vec![Complex32::new(0.0, 0.0); (RATE * 30.0) as usize];
    let n = g.generate(&mut x);
    x[n..].fill(Complex32::new(1.0, 0.0));
    let mut out = Output::default();
    let warm = (RATE * 20.0) as usize;
    for c in x[..warm].chunks(BLOCK) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let n = allocations(|| {
        for c in x[warm..].chunks(BLOCK) {
            out.clear();
            rx.feed(c, &mut out);
        }
    });
    assert_eq!(n, 0, "allocations over 10 s of the carrier");
}

//! The allocations of the receive path: the DMR stack graph, warmed with its own test signal.

use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig, Values};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts the allocations of the calling thread.
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: every call is passed to the system allocator unchanged. The count is a thread-local cell.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        // SAFETY: the contract of the caller for `alloc` is passed on as it is.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer came from `System.alloc` above, with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations(f: impl FnOnce()) -> u64 {
    let before = ALLOCATIONS.with(Cell::get);
    f();
    ALLOCATIONS.with(Cell::get) - before
}

const RATE: f64 = 96_000.0;
const BLOCK: usize = 4800;

fn dmr_receiver() -> Box<dyn Receiver> {
    let d = &sdrkit_stack_dvoice::DMR;
    let mut rx = (d.make_receiver)(&StackConfig::new(Values::defaults(&(d.settings)())))
        .expect("the receiver builds");
    rx.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .expect("configure");
    rx
}

fn test_signal() -> Vec<Complex32> {
    let d = &sdrkit_stack_dvoice::DMR;
    let cfg = GenConfig::new(RATE, Values::defaults(&(d.settings)()));
    let mut g = sdrkit_stack_dvoice::test_signal("dmr", &cfg).expect("the DMR test signal");
    let mut out = Vec::new();
    let mut buf = vec![Complex32::new(0.0, 0.0); 65536];
    loop {
        let n = sdrkit_core::Generator::generate(&mut g, &mut buf);
        out.extend_from_slice(&buf[..n]);
        if n < buf.len() {
            break;
        }
    }
    out
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32], out: &mut Output) {
    for c in x.chunks(BLOCK) {
        rx.feed(c, out);
        out.clear();
    }
}

#[test]
fn the_receive_path_allocates_nothing_once_warm() {
    let x = test_signal();
    let quiet = vec![Complex32::new(0.0, 0.0); 3 * RATE as usize];
    let mut rx = dmr_receiver();
    let mut out = Output::default();
    // Two passes of the test signal and the quiet after them: every buffer, table row and pool reaches its size.
    for _ in 0..2 {
        feed(rx.as_mut(), &x, &mut out);
        feed(rx.as_mut(), &quiet, &mut out);
    }
    // Quiet after the calls, with a report every 0.25 s.
    let n = allocations(|| feed(rx.as_mut(), &quiet, &mut out));
    assert_eq!(n, 0, "{n} allocations in 3 s of quiet");
}

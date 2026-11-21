//! Allocations on the receive path of every stack of the crate, counted by an allocator that counts only on the thread that
//! enables it, so other tests that run at the same time do not change the count.

use sdrkit_core::{Complex32, InputInfo, Output, StackConfig, StackDescriptor, Values};
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

const BLOCK: usize = 16_384;

/// Feeds `desc`'s receiver at `rate` with noise: `warm_s` seconds, then `seconds` more, block by block. It returns the
/// allocations of the blocks after the warm-up that produced no event and no host request, and the count of those blocks.
fn quiet_allocations(
    desc: &StackDescriptor,
    rate: f64,
    warm_s: f64,
    seconds: f64,
) -> (usize, usize) {
    let mut rx = (desc.make_receiver)(&StackConfig::new(Values::defaults(&(desc.settings)())))
        .expect("the receiver builds");
    let f = desc.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, f, f).recording())
        .expect("the stack takes its own rate");
    let mut noise = sdrkit_algo_sim::noise::Awgn::with_power(1e-4, 3).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); BLOCK];
    let mut out = Output::default();
    let blocks = |s: f64| (s * rate / BLOCK as f64) as usize;
    for _ in 0..blocks(warm_s) {
        x.iter_mut().for_each(|z| *z = Complex32::new(0.0, 0.0));
        noise.process(&mut x);
        rx.feed(&x, &mut out);
        out.clear();
    }
    let (mut total, mut quiet) = (0, 0);
    for _ in 0..blocks(seconds) {
        x.iter_mut().for_each(|z| *z = Complex32::new(0.0, 0.0));
        noise.process(&mut x);
        let n = allocations(|| rx.feed(&x, &mut out));
        if out.events.is_empty() && out.requests.is_empty() {
            total += n;
            quiet += 1;
        }
        out.clear();
    }
    let status = sdrkit_core::Owner::results(rx.as_ref()).status();
    assert_ne!(
        status.lock_state,
        sdrkit_core::LockState::Off,
        "{}: {}",
        desc.id,
        status.summary
    );
    (total, quiet)
}

#[test]
fn quiet_blocks_allocate_nothing() {
    let mut bad = Vec::new();
    for d in sdrkit_stack_trunk::descriptors() {
        let rate = d.tuning.min_rate_hz.max(match d.id {
            "edacs" => 480e3,
            "tones" | "pager" => 2e6,
            _ => 240e3,
        });
        let (allocs, quiet) = quiet_allocations(d, rate, 3.0, 3.0);
        assert!(quiet > 0, "{}: no quiet block", d.id);
        if allocs > 0 {
            bad.push(format!(
                "{}: {allocs} allocations in {quiet} quiet blocks",
                d.id
            ));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

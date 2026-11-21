//! The receive path of COSPAS-SARSAT allocates nothing per block once warm, through the stack graph, on their own test signals.

use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor, Values};
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

/// Runs `desc` on its test signal at `rate` in blocks of 0.1 s for `signal_s` seconds, then on noise alone for `quiet_s` seconds.
/// Returns the noise blocks after the first 2 s of noise that allocated and produced no event, with their counts. The test
/// signal fills the tables and the logs first, so the noise blocks show the receive path in its steady state.
fn noise_blocks_that_allocate(
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
    quiet_s: f64,
) -> Vec<(usize, usize)> {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)()).unwrap();
    let v = Values::defaults(&schema);
    let mut g = (desc.make_generator.unwrap())(&GenConfig::new(rate, v.clone())).unwrap();
    let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
    let dial = desc.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, dial, dial).recording())
        .unwrap();
    let block = (rate * 0.1) as usize;
    let mut iq = vec![Complex32::default(); block];
    let mut out = Output::default();
    for _ in 0..(signal_s * 10.0) as usize {
        g.generate(&mut iq);
        rx.feed(&iq, &mut out);
        out.clear();
    }
    let mut noise = sdrkit_dev_test::Awgn::with_sigma(0.01, 3);
    let mut busy = Vec::new();
    for i in 0..(quiet_s * 10.0) as usize {
        iq.fill(Complex32::default());
        noise.process(&mut iq);
        let n = allocations(|| rx.feed(&iq, &mut out));
        if i >= 20 && n > 0 && out.events.is_empty() {
            busy.push((i, n));
        }
        out.clear();
    }
    busy
}

// One test program holds one test: the counter is shared by every thread of the test program.
#[test]
fn the_sarsat_receiver_allocates_nothing_per_block() {
    use sdrkit_stack_satcom::modes::sarsat;
    let sarsat = noise_blocks_that_allocate(&sarsat::DESCRIPTOR, 256e3, 10.0, 10.0);
    assert!(
        sarsat.is_empty(),
        "SARSAT blocks that allocated: {sarsat:?}"
    );
}

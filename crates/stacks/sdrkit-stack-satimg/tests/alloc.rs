//! Allocations on the receive path of the Meteor-M LRPT stack, counted by an allocator that counts only on the thread that
//! enables it, so other tests that run at the same time do not change the count.

use sdrkit_core::{Complex32, InputInfo, Output, StackConfig};
use sdrkit_dev_test::Harness;
use sdrkit_graph::Graph;
use sdrkit_stack_satimg::modes::meteor::DESCRIPTOR;
use sdrkit_stack_satimg::REPORT_S;
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

/// The results of a run after its warm-up.
#[derive(Debug, Default)]
struct Tally {
    /// Blocks with no report tick and no copy of results, and their allocations.
    quiet: usize,
    quiet_allocs: usize,
    /// Blocks with a report tick or a copy of results, and their allocations.
    busy: usize,
    busy_allocs: usize,
}

/// Feeds `seconds` of `x` (repeated) to the Meteor receiver in blocks of [`BLOCK`] samples, and sorts the blocks after the first
/// `warm_s` seconds.
fn run(h: &Harness, x: &[Complex32], warm_s: f64, seconds: f64) -> Tally {
    let mut rx = (DESCRIPTOR.make_receiver)(&StackConfig::new(h.values().unwrap())).unwrap();
    let dial = DESCRIPTOR.tuning.default_hz;
    rx.configure(&InputInfo::new(RATE, dial, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let block_s = BLOCK as f64 / RATE;
    // A report comes at each tick of the graph clock, and the graph copies the changed results every merge interval.
    let busy_s = Graph::MERGE_INTERVAL_S + block_s;
    let blocks = ((warm_s + seconds) / block_s) as usize;
    let mut t = Tally::default();
    let mut piece = vec![Complex32::default(); BLOCK];
    for k in 0..blocks {
        for (i, s) in piece.iter_mut().enumerate() {
            *s = x[(k * BLOCK + i) % x.len()];
        }
        let n = allocations(|| rx.feed(&piece, &mut out));
        out.clear();
        let end = (k + 1) as f64 * block_s;
        if end < warm_s {
            continue;
        }
        let since_tick = end % REPORT_S;
        if since_tick <= busy_s {
            t.busy += 1;
            t.busy_allocs += n;
        } else {
            t.quiet += 1;
            t.quiet_allocs += n;
        }
    }
    t
}

fn report(what: &str, t: &Tally) {
    eprintln!(
        "{what}: {} quiet blocks: {} allocations; {} blocks with a report: {} allocations ({:.1} a block)",
        t.quiet,
        t.quiet_allocs,
        t.busy,
        t.busy_allocs,
        t.busy_allocs as f64 / t.busy.max(1) as f64
    );
}

/// Noise into the stack: once warm, a block without a report allocates nothing.
#[test]
fn meteor_on_noise_allocates_nothing_between_reports_once_warm() {
    let h = Harness::new(&DESCRIPTOR)
        .rate(RATE)
        .seconds(1.0)
        .set(
            &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
            sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
        )
        .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Floor,
        )
        .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -20.0);
    let x = h.generate().unwrap();
    let t = run(&h, &x, 2.0, 3.0);
    report("meteor on noise", &t);
    assert!(t.quiet > 100, "only {} quiet blocks", t.quiet);
    assert_eq!(t.quiet_allocs, 0, "{t:?}");
}

/// The stack's own test signal: once frames decode, a block without a report allocates only for what it decodes.
#[test]
fn meteor_on_its_test_signal() {
    let h = Harness::new(&DESCRIPTOR).rate(RATE).seconds(8.0).set(
        &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
        sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
    );
    let x = h.generate().unwrap();
    let t = run(&h, &x, 4.0, 4.0);
    report("meteor on its test signal", &t);
    assert!(t.quiet > 100, "only {} quiet blocks", t.quiet);
}

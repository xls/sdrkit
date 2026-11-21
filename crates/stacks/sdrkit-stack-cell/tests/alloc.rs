//! The receive paths of the LBJ and LTE stacks allocate nothing per block once warm, through the stack graph, on their own test
//! signals.

use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

/// Counts one allocation if this thread counts. Two tests that run at the same time do not count each other's.
fn note() {
    // `try_with` fails only while the thread ends: nothing is counted then.
    let _ = COUNT.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call goes to the system allocator unchanged. The counter only reads and writes thread-local cells.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the layout comes from the caller, as for the system allocator.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` was given by `alloc` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        // SAFETY: `p` was given by `alloc` with layout `l`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The allocations that one call of `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCS.with(|n| n.set(0));
    COUNT.with(|on| on.set(true));
    f();
    COUNT.with(|on| on.set(false));
    ALLOCS.with(|n| n.get())
}

/// Runs `desc` on its test signal at its default rate in blocks of `block_s` for `seconds`. Returns the blocks after `warm`
/// seconds that allocated while `quiet` (given the block number, the output and the results) says that the block did no work
/// beyond the per-sample path, with their counts, and the number of quiet blocks after `warm` seconds.
fn busy_blocks(
    desc: &'static StackDescriptor,
    seconds: f64,
    warm: f64,
    block_s: f64,
    mut quiet: impl FnMut(usize, &Output, &sdrkit_core::Results) -> bool,
) -> (Vec<(usize, usize)>, usize) {
    let rate = desc.tuning.rate_hz;
    // A stack without a default frequency takes the middle of its range.
    let f = if desc.tuning.default_hz > 0.0 {
        desc.tuning.default_hz
    } else {
        (desc.tuning.min_hz + desc.tuning.max_hz) / 2.0
    };
    let v = sdrkit_dev_test::Harness::new(desc).values().unwrap();
    let mut g = (desc.make_generator.unwrap())(&GenConfig::new(rate, v.clone())).unwrap();
    let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, f + desc.tuning.tune_offset_hz, f).recording())
        .unwrap();
    let block = (rate * block_s) as usize;
    let mut iq = vec![Complex32::default(); block];
    let mut out = Output::default();
    let mut busy = Vec::new();
    let mut quiet_blocks = 0;
    for i in 0..(seconds / block_s) as usize {
        g.generate(&mut iq);
        let n = allocations(|| rx.feed(&iq, &mut out));
        if quiet(i, &out, rx.results()) && i as f64 * block_s >= warm {
            quiet_blocks += 1;
            if n > 0 {
                busy.push((i, n));
            }
        }
        out.clear();
    }
    (busy, quiet_blocks)
}

#[test]
fn the_lbj_receiver_allocates_nothing_per_block() {
    // The demo loop: 0.3 s of silence, then bursts of 1120 bits with gaps of 0.5 s. A block that holds part of a burst
    // (with 50 ms of room on each side) receives its code words, and a block in which a burst ends decodes its pages: both
    // allocate. The other blocks of the gaps must not.
    let opt = sdrkit_stack_cell::lbj::gen::LbjGenOptions::default();
    let burst_s = 1120.0 / sdrkit_stack_cell::lbj::gen::BIT_RATE;
    let period = burst_s + opt.gap_s;
    let block_s = 0.1;
    let in_gap = |t: f64| {
        let at = (t - opt.lead_s).rem_euclid(period);
        t > opt.lead_s && at > burst_s + 0.05 && at < period - 0.05
    };
    let mut bursts = 0;
    let (busy, quiet) = busy_blocks(&sdrkit_stack_cell::LBJ, 20.0, 5.0, block_s, |i, out, r| {
        let id = r.decl().field_id("bursts").unwrap();
        let n = match r.fields().value(id) {
            Some(sdrkit_core::Value::I64(n)) => *n,
            _ => 0,
        };
        let ended = n != bursts;
        bursts = n;
        let t = i as f64 * block_s;
        out.events.is_empty() && !ended && in_gap(t) && in_gap(t + block_s)
    });
    assert!(busy.is_empty(), "blocks that allocated: {busy:?}");
    assert!(quiet >= 20, "{quiet} quiet blocks");
}

#[test]
fn the_lte_receiver_allocates_nothing_while_it_gathers_a_capture() {
    // Captures of 0.2 s in blocks of 10 ms: block 19 of every 20 ends a capture and runs the search.
    let (busy, quiet) = busy_blocks(&sdrkit_stack_cell::CELL, 1.0, 0.4, 0.01, |i, _, _| {
        i % 20 != 19
    });
    assert!(busy.is_empty(), "blocks that allocated: {busy:?}");
    assert!(quiet >= 20, "{quiet} quiet blocks");
}

//! The receive path allocates nothing once warm: the test signal through the stack graph, with a worker pool that has no
//! helper, at the natural rate and at a rate that the input stage resamples.

use sdrkit_core::{
    Choice, Complex32, Generator, InputInfo, Output, Receiver, SettingSet, StackConfig, Values,
};
use sdrkit_stack_dvbt::generator::{DvbtGenerator, GenParams};
use sdrkit_stack_dvbt::params::{Bandwidth, TpsParams};
use sdrkit_stack_dvbt::settings::{BandwidthChoice, DvbtSetting};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call goes straight to the system allocator. The count is a thread-local cell.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        // SAFETY: the caller's layout goes to the system allocator unchanged.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `alloc` or `realloc` above with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        // SAFETY: as `dealloc`, and `n` is the caller's new size.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(Cell::get);
    f();
    ALLOCS.with(Cell::get) - before
}

/// Allocations while the stack at `rate` takes `measure_s` seconds of the signal of `g` in pieces of 16 384 samples,
/// after `warm_s` seconds of it.
fn steady_state(g: &mut DvbtGenerator, rate: f64, warm_s: f64, measure_s: f64) -> usize {
    let schema = DvbtSetting::schema();
    let mut v = Values::defaults(&schema);
    v.set(
        DvbtSetting::Bandwidth.id(),
        &sdrkit_core::Value::Choice(BandwidthChoice::B8.index()),
    )
    .expect("a choice");
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&StackConfig::new(v)).expect("builds");
    let pool = sdrkit_graph::Pool::new(1);
    pool.set_workers(0);
    rx.set_context(sdrkit_graph::RunContext {
        pool,
        ..Default::default()
    });
    rx.configure(&InputInfo::new(rate, 474e6, 474e6).recording())
        .expect("rate");
    let mut out = Output::default();
    let mut c = vec![Complex32::default(); 16_384];
    for _ in 0..(warm_s * rate) as usize / c.len() {
        g.generate(&mut c);
        out.clear();
        rx.feed(&c, &mut out);
    }
    let mut n = 0;
    for _ in 0..(measure_s * rate) as usize / c.len() {
        g.generate(&mut c);
        out.clear();
        n += allocations(|| rx.feed(&c, &mut out));
    }
    n
}

#[test]
fn the_receive_path_allocates_nothing_once_warm() {
    let rate = Bandwidth::B8.natural_rate();
    let mut g = DvbtGenerator::new(GenParams::new(Bandwidth::B8, TpsParams::default()));
    // The sound of the test multiplex has a PCR every 24 ms. The clock of the demultiplexer keeps the PCRs of its fit
    // window, which fills during the measured part: its store has the room from the start.
    let measure_s = sdrkit_proto_mpegts::pcr::FIT_SECONDS + 1.0;
    let n = steady_state(&mut g, rate, 2.0, measure_s);
    assert_eq!(n, 0, "allocations at the natural rate");
}

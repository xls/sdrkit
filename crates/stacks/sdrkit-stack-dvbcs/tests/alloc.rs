//! The DVB-C receive path allocates nothing once warm: the test signal through the DVB-C components to the transport stream,
//! and through the stack graph with the transport stream layer.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_ESN0_DB, GEN_NOISE};
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, SettingSet, StackConfig, Values};
use sdrkit_graph::GraphBuilder;
use sdrkit_proto_mpegts::monitor::TsMonitorComponent;
use sdrkit_stack_dvbcs::dvbc::{
    self, DvbcDemod, DvbcOuter, DvbcReport, DvbcSetting, DVBC, REPORT_S,
};
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

/// The DVB-C test signal at 10 Msps with an Es/N0 of 35 dB, 2 s long.
fn signal(rate: f64) -> Vec<Complex32> {
    sdrkit_dev_test::Harness::new(&DVBC)
        .rate(rate)
        .seconds(2.0)
        .set(&GEN_NOISE, NoiseKind::EsN0)
        .set(&GEN_ESN0_DB, 35.0)
        .generate()
        .unwrap()
}

/// Allocations while `rx` takes the last 0.5 s of `x` in pieces of 16 384 samples, after the first 1.5 s.
fn steady_state(rx: &mut dyn Receiver, x: &[Complex32], rate: f64) -> usize {
    rx.configure(&InputInfo::new(rate, 346e6, 346e6).recording())
        .unwrap();
    let warm = (1.5 * rate) as usize;
    let mut out = Output::default();
    for c in x[..warm].chunks(16_384) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let mut n = 0;
    for c in x[warm..].chunks(16_384) {
        out.clear();
        n += allocations(|| rx.feed(c, &mut out));
    }
    n
}

#[test]
fn the_dvbc_components_allocate_nothing_once_warm() {
    let rate = 10e6;
    let x = signal(rate);
    let mut g = GraphBuilder::new()
        .add("demod", DvbcDemod::default())
        .add("outer", DvbcOuter::default())
        .add("monitor", TsMonitorComponent::new(1))
        .add("report", DvbcReport::default())
        .input("demod.in")
        .input("report.spectrum")
        .connect("demod.bytes", "outer.in")
        .feedback("outer.lost", "demod.lost")
        .connect("outer.ts", "monitor.in")
        .connect("demod.state", "report.state")
        .connect("outer.stats", "report.stats")
        .clock(REPORT_S)
        .build()
        .unwrap();
    assert_eq!(steady_state(&mut g, &x, rate), 0);
}

#[test]
fn the_dvbc_stack_allocates_nothing_once_warm() {
    let rate = 10e6;
    let x = signal(rate);
    let cfg = StackConfig::new(Values::defaults(&DvbcSetting::schema()));
    let mut rx = dvbc::receiver(&cfg).unwrap();
    assert_eq!(steady_state(&mut rx, &x, rate), 0);
}

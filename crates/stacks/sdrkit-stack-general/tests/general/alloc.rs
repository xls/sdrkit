//! Steady-state decoding allocates nothing per sample: allocations are counted with a counting allocator, for the thread that
//! runs the test only (the other tests of the program run beside it).

use crate::common::*;
use sdrkit_core::{Complex32, Output, Receiver};
use sdrkit_stack_general::listen::{AgcSpeed, ChainSettings, ListeningChain, ModeKind};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

fn count() {
    // A thread that is ending has no counter left. Its allocations are not counted.
    let _ = COUNT.try_with(|c| c.set(c.get() + 1));
}

fn allocations() -> u64 {
    COUNT.with(Cell::get)
}

unsafe impl GlobalAlloc for Counting {
    // SAFETY: forwards to the system allocator unchanged. Only counts the calls.
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocations_per_second(settings: &[(&str, &str)], demod: &str) -> f64 {
    let rate = 2e6;
    let mut rx = receiver(rate, -250e3, settings);
    let mut g = generator(rate, demod, &[("gen_snr_db", "25")]);
    let mut buf = vec![Complex32::default(); 20_000];
    let mut out = Output::default();
    let mut step = |rx: &mut sdrkit_stack_general::GeneralReceiver, out: &mut Output| {
        g.generate(&mut buf);
        out.clear();
        rx.feed(&buf, out);
    };
    for _ in 0..200 {
        step(&mut rx, &mut out);
    }
    let before = allocations();
    for _ in 0..200 {
        step(&mut rx, &mut out);
    }
    let after = allocations();
    (after - before) as f64 / 2.0
}

#[test]
fn steady_state_decoding_never_allocates() {
    // 2 seconds of 2 Msps through the receiver, with its reports ten times a second.
    for (d, s) in [
        ("nfm", vec![("demod", "nfm"), ("nb", "1")]),
        (
            "am",
            vec![("demod", "am"), ("am_auto", "1"), ("am_detector", "auto")],
        ),
        ("usb", vec![("demod", "usb")]),
        ("wfm", vec![("demod", "wfm")]),
    ] {
        let n = allocations_per_second(&s, d);
        assert_eq!(n, 0.0, "{d}: {n} allocations a second");
    }
}

#[test]
fn an_extra_channel_allocates_only_at_its_reports() {
    // The merged results take the table of the main channel at each of its ten reports a second, and the row of the extra
    // channel is written again.
    let n = allocations_per_second(&[("demod", "nfm"), ("extra1", "1")], "nfm");
    assert!(n <= 60.0, "{n} allocations a second");
}

#[test]
fn the_listening_chain_never_allocates() {
    let mut s = ChainSettings::defaults_for(ModeKind::Nfm);
    s.cleanup = ChainSettings::cleanup_preset(ModeKind::Nfm);
    s.control.on = true;
    s.control.auto_notch = true;
    s.control.agc = AgcSpeed::Fast;
    let mut c = ListeningChain::new(48e3, s);
    let mut buf: Vec<[f32; 2]> = (0..480)
        .map(|i| [(i as f32 * 0.1).sin() * 0.2; 2])
        .collect();
    for _ in 0..200 {
        c.process(&mut buf);
    }
    let before = allocations();
    for _ in 0..1000 {
        c.process(&mut buf);
    }
    assert_eq!(allocations() - before, 0);
}

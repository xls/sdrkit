//! Once warm, a spectrum frame and a step allocate nothing, the frames that end a sweep included, while no row of a table
//! changes.

mod common;

use common::fake::{FakeRadio, FakeSignal};
use common::*;
use sdrkit_algo_dsp::Complex32;
use sdrkit_core::Value;
use sdrkit_svc_sweep::{Input, SpectrumFrame, Step};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

thread_local! {
    /// Counting is on for this thread only: the other tests of this file run on other threads.
    static ON: Cell<bool> = const { Cell::new(false) };
}
static COUNT: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call goes to the system allocator unchanged. The counter only reads a thread flag and adds one.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ON.with(Cell::get) {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the layout comes from the caller, as for the system allocator.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` was given by `alloc` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ON.with(Cell::get) {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: `p` was given by `alloc` with layout `l`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counted<T>(f: impl FnOnce() -> T) -> (T, usize) {
    COUNT.store(0, Ordering::Relaxed);
    ON.with(|on| on.set(true));
    let out = f();
    ON.with(|on| on.set(false));
    (out, COUNT.load(Ordering::Relaxed))
}

#[test]
fn frames_and_steps_allocate_nothing_once_warm() {
    assert_eq!(
        counted(|| Vec::<u8>::with_capacity(8)).1,
        1,
        "the counter counts"
    );
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.001));
    set(&mut s, "averages", Value::I64(8));
    set(&mut s, "bin_hz", Value::Hz(5e3));
    set(&mut s, "sweeps", Value::I64(10));
    let mut d = Driver::new(FakeRadio::new(RATE, 100e6, 1));
    d.radio.add(FakeSignal::carrier(103.3e6, -50.0));
    d.start(
        &mut s,
        &[("low_hz", Value::Hz(100e6)), ("high_hz", Value::Hz(112e6))],
    );
    let mut step = d.step_now();
    let mut frames = Frames::new();
    let mut buf = vec![Complex32::default(); 2048];
    let (mut fed, mut ends, mut allocs) = (0, 0, 0);
    loop {
        match step.take() {
            Some(Step::Tune {
                centre_hz,
                rate_hz,
                input,
            }) => {
                assert_eq!(input, Input::Spectrum);
                d.radio.tune(centre_hz, rate_hz);
                frames.retune();
            }
            Some(Step::Done { .. }) => break,
            Some(other) => panic!("{other:?}"),
            None => {}
        }
        d.radio.read(&mut buf);
        let (now, centre) = (d.radio.time_s(), d.radio.centre_hz());
        let need = s.spectrum().unwrap();
        frames.push(need, &buf, |bins| {
            let warm = field_num(&s, "sweeps") >= 3.0;
            let frame = SpectrumFrame {
                time_s: now,
                centre_hz: centre,
                span_hz: need.rate_hz,
                bins_db: bins,
            };
            let sweeps = field_num(&s, "sweeps");
            let (out, n) = counted(|| s.feed(&frame).unwrap());
            if warm {
                fed += 1;
                allocs += n;
                if field_num(&s, "sweeps") != sweeps {
                    ends += 1;
                }
            }
            step = out;
            step.is_none()
        });
        if step.is_none() {
            let info = d.info();
            let (out, n) = counted(|| s.step(now, &info).unwrap());
            if field_num(&s, "sweeps") >= 3.0 {
                allocs += n;
            }
            step = out;
        }
    }
    assert!(fed > 500, "{fed}");
    assert!(ends >= 5, "{ends}");
    assert_eq!(allocs, 0);
}

//! The path of a session's thread (radio ring to graph to results, the audio ring and a tap) allocates nothing once warm, on a
//! fake rtl_tcp radio.

mod common;

use common::*;
use sdrkit_graph::{GraphPlan, PortRef, Registry};
use sdrkit_host_engine::{Engine, Input, OwnerRef, SessionId, SAMPLES};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct Counting;

/// Allocations on the session threads.
static SESSION_ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// 0: not known yet, 1: a session thread, 2: another thread, 3: finding out now.
    static ROLE: Cell<u8> = const { Cell::new(0) };
}

fn count() {
    let role = ROLE.with(Cell::get);
    let role = if role == 0 {
        ROLE.with(|r| r.set(3));
        let named = std::thread::current().name() == Some("sdrkit-session");
        let role = if named { 1 } else { 2 };
        ROLE.with(|r| r.set(role));
        role
    } else {
        role
    };
    if role == 1 {
        SESSION_ALLOCS.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count();
        // SAFETY: the layout is passed on as the caller supplied it.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        count();
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The tests run one at a time: the counter counts every session thread.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn a_session_on_a_radio_allocates_nothing_once_warm() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let rtl_server = fake_rtl();
    let e = engine_on(rtl_server.net());
    let v = defaults(&e, e.find("driver.rtl_tcp").unwrap());
    let r = e.open_radio("rtl_tcp", &rtl_server.addr(), &v).unwrap();
    let mut plan = GraphPlan::new();
    let meter = plan
        .add(&Registry::standard(), "meter", "dsp.power_meter")
        .unwrap();
    plan.input(PortRef::new(meter, 0)).unwrap();
    let s = e.start_graph(plan, Input::Radio(r)).unwrap();
    let samples = || number(&e, OwnerRef::Session(s), SAMPLES.key);
    wait_for(5.0, || (samples() > 2.0 * RTL_RATE).then_some(())).expect("the session warms up");
    let (before, from) = (SESSION_ALLOCS.load(Ordering::Relaxed), samples());
    std::thread::sleep(Duration::from_secs(2));
    let (after, to) = (SESSION_ALLOCS.load(Ordering::Relaxed), samples());
    assert!(
        before > 0,
        "the counter sees the session thread while it warms up"
    );
    assert!(to - from > RTL_RATE, "samples flowed: {from} to {to}");
    assert_eq!(after - before, 0, "allocations on the session thread");
}

/// Counts the session-thread allocations over two seconds of session `s`, while `drain` runs on the test thread.
fn allocations(e: &Engine, s: SessionId, mut drain: impl FnMut()) -> usize {
    let samples = || number(e, OwnerRef::Session(s), SAMPLES.key);
    wait_for(5.0, || {
        drain();
        (samples() > 2.0 * RTL_RATE).then_some(())
    })
    .expect("the session warms up");
    let (before, from) = (SESSION_ALLOCS.load(Ordering::Relaxed), samples());
    let t = std::time::Instant::now();
    while t.elapsed() < Duration::from_secs(2) {
        drain();
        std::thread::sleep(Duration::from_millis(5));
    }
    let (after, to) = (SESSION_ALLOCS.load(Ordering::Relaxed), samples());
    assert!(to - from > RTL_RATE, "samples flowed: {from} to {to}");
    after - before
}

#[test]
fn a_session_with_audio_for_the_host_and_a_tap_allocates_nothing_once_warm() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let rtl_server = fake_rtl();
    let e = engine_on(rtl_server.net());
    let v = defaults(&e, e.find("driver.rtl_tcp").unwrap());
    let r = e.open_radio("rtl_tcp", &rtl_server.addr(), &v).unwrap();
    let reg = Registry::standard();
    let mut plan = GraphPlan::new();
    let am = plan.add(&reg, "am", "dsp.am_demod").unwrap();
    let audio = plan.add(&reg, "audio", "dsp.real_to_audio").unwrap();
    let out = plan.add(&reg, "out", "host.out").unwrap();
    plan.input(PortRef::new(am, 0)).unwrap();
    plan.connect(PortRef::new(am, 0), PortRef::new(audio, 0))
        .unwrap();
    plan.connect(PortRef::new(audio, 0), PortRef::new(out, 0))
        .unwrap();
    let s = e.start_graph(plan, Input::Radio(r)).unwrap();
    let tap = e.open_tap(s, "input", 4).unwrap();
    let mut buf = vec![0.0f32; 1 << 16];
    let mut heard = 0;
    let n = allocations(&e, s, || {
        while let Some(a) = e.audio(s, &mut buf).unwrap() {
            heard += a.frames;
        }
        while e.read_tap(s, tap, &mut buf).unwrap().is_some() {}
    });
    assert!(heard > 0, "audio reached the host");
    assert_eq!(n, 0, "allocations on the session thread");
}

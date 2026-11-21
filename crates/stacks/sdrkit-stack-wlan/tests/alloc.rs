//! The receivers of every stack allocate nothing per block once they are warm: each stack hears its own test signal, then
//! quiet noise, and every block of noise that produces no event must allocate nothing.

use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor, Values};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

/// Counts one allocation of this thread when counting is on.
fn count() {
    if ON.with(Cell::get) {
        ALLOCS.with(|c| c.set(c.get() + 1));
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the layout is passed on unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer came from the system allocator with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: the pointer came from the system allocator with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The allocations that `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCS.with(|c| c.set(0));
    ON.with(|c| c.set(true));
    f();
    ON.with(|c| c.set(false));
    ALLOCS.with(Cell::get)
}

/// Feeds the stack's test signal for `signal_s`, then noise for `quiet_s`, in blocks of 0.05 s. Returns the noise blocks after
/// the first 0.5 s of noise that allocated and produced no event, with their allocation counts.
fn noise_blocks_that_allocate(
    desc: &'static StackDescriptor,
    signal_s: f64,
    quiet_s: f64,
) -> Vec<(usize, usize)> {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)()).unwrap();
    let v = Values::defaults(&schema);
    let rate = desc.tuning.rate_hz;
    let mut g = (desc.make_generator.unwrap())(&GenConfig::new(rate, v.clone())).unwrap();
    let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
    let dial = desc.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, dial, dial).recording())
        .unwrap();
    let block = (rate * 0.05) as usize;
    let mut iq = vec![Complex32::default(); block];
    let mut out = Output::default();
    for _ in 0..(signal_s / 0.05) as usize {
        g.generate(&mut iq);
        rx.feed(&iq, &mut out);
        out.clear();
    }
    let mut noise = sdrkit_dev_test::Awgn::with_sigma(0.001, 3);
    let mut busy = Vec::new();
    for i in 0..(quiet_s / 0.05) as usize {
        iq.fill(Complex32::default());
        noise.process(&mut iq);
        let n = allocations(|| rx.feed(&iq, &mut out));
        if i >= 10 && n > 0 && out.events.is_empty() && out.requests.is_empty() {
            busy.push((i, n));
        }
        out.clear();
    }
    busy
}

#[test]
fn the_ble_and_tracker_receivers_allocate_nothing_per_block() {
    let ble = noise_blocks_that_allocate(&sdrkit_stack_wlan::ble::BLE, 1.0, 2.0);
    assert!(ble.is_empty(), "BLE blocks that allocated: {ble:?}");
    let tracker = noise_blocks_that_allocate(&sdrkit_stack_wlan::tracker::BLETRACKER, 1.0, 2.0);
    assert!(
        tracker.is_empty(),
        "tracker blocks that allocated: {tracker:?}"
    );
}

#[test]
fn the_wifi_and_halow_receivers_allocate_nothing_per_block() {
    let wifi = noise_blocks_that_allocate(&sdrkit_stack_wlan::wifi::WIFI, 0.3, 1.0);
    assert!(wifi.is_empty(), "Wi-Fi blocks that allocated: {wifi:?}");
    let halow = noise_blocks_that_allocate(&sdrkit_stack_wlan::halow::HALOW, 0.8, 1.5);
    assert!(halow.is_empty(), "HaLow blocks that allocated: {halow:?}");
}

#[test]
fn the_bluetooth_classic_receiver_allocates_nothing_per_block() {
    let bt = noise_blocks_that_allocate(&sdrkit_stack_wlan::btclassic::BTCLASSIC, 1.0, 2.0);
    assert!(
        bt.is_empty(),
        "Bluetooth Classic blocks that allocated: {bt:?}"
    );
}

#[test]
fn the_wfb_receiver_allocates_nothing_per_block() {
    let wfb = noise_blocks_that_allocate(&sdrkit_stack_wlan::wfb::WFB, 0.3, 1.0);
    assert!(wfb.is_empty(), "wfb blocks that allocated: {wfb:?}");
}

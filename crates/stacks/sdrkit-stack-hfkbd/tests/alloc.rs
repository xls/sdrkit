//! The receive path of the RTTY stack allocates nothing per block once warm, through the stack graph, on its own test signal;
//! the audio of the CW stack on the dial adds no allocation.

use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor};
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

/// Runs `desc` on its test signal at 12 ksps in blocks of 0.1 s for `seconds`, and returns the blocks after `warm` seconds that
/// allocated and produced no event, with their counts. The decoded text grows until its store is full, so a run is warm only
/// after that.
fn quiet_blocks_that_allocate(
    desc: &'static StackDescriptor,
    seconds: f64,
    warm: f64,
) -> Vec<(usize, usize)> {
    const RATE: f64 = 12_000.0;
    let v = sdrkit_dev_test::Harness::new(desc).values().unwrap();
    let mut g = (desc.make_generator.unwrap())(&GenConfig::new(RATE, v.clone())).unwrap();
    let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(RATE, 14.07e6, 14.07e6).recording())
        .unwrap();
    let block = (RATE * 0.1) as usize;
    let mut iq = vec![Complex32::default(); block];
    let mut out = Output::default();
    let mut busy = Vec::new();
    for i in 0..(seconds * 10.0) as usize {
        g.generate(&mut iq);
        let n = allocations(|| rx.feed(&iq, &mut out));
        if i as f64 >= warm * 10.0 && n > 0 && out.events.is_empty() {
            busy.push((i, n));
        }
        out.clear();
    }
    busy
}

#[test]
fn the_rtty_receiver_allocates_nothing_per_block() {
    // 8000 characters at about 6.6 a second.
    let busy = quiet_blocks_that_allocate(&sdrkit_stack_hfkbd::RTTY, 1340.0, 1310.0);
    assert!(busy.is_empty(), "blocks that allocated: {busy:?}");
}

/// The allocations of the CW stack after `warm` of `seconds` of its noise-only test signal with `listen`, in blocks of 0.1 s.
fn cw_allocations(listen: sdrkit_stack_hfkbd::cw::Listen, seconds: f64, warm: f64) -> usize {
    use sdrkit_stack_hfkbd::cw::{GEN_NOISE_ONLY, LISTEN};
    let desc = &sdrkit_stack_hfkbd::CW;
    let v = sdrkit_dev_test::Harness::new(desc)
        .set(&LISTEN, listen)
        .set(&GEN_NOISE_ONLY, true)
        .values()
        .unwrap();
    let rate = desc.rate_hz(&v);
    let mut g = (desc.make_generator.unwrap())(&GenConfig::new(rate, v.clone())).unwrap();
    let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, 14.07e6 + 20e3, 14.07e6).recording())
        .unwrap();
    let mut iq = vec![Complex32::default(); (rate * 0.1) as usize];
    let mut out = Output::default();
    let mut total = 0;
    for i in 0..(seconds * 10.0) as usize {
        g.generate(&mut iq);
        let n = allocations(|| rx.feed(&iq, &mut out));
        if i as f64 >= warm * 10.0 {
            total += n;
        }
        out.clear();
    }
    total
}

#[test]
fn the_cw_audio_of_the_dial_allocates_nothing_more() {
    // The same noise with the audio off and playing the dial: the audio adds no allocation once warm.
    let off = cw_allocations(sdrkit_stack_hfkbd::cw::Listen::Off, 30.0, 10.0);
    let dial = cw_allocations(sdrkit_stack_hfkbd::cw::Listen::Signal, 30.0, 10.0);
    assert!(dial <= off, "playing the dial: {dial}, off: {off}");
}

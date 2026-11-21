//! Allocations of the receive path once warm: the stack graph of C-QUAM fed its own test signal.

use sdrkit_core::{Complex32, Generator, InputInfo, Output, SettingSet, StackConfig, Values};
use sdrkit_stack_broadcast::cquam::{self, CquamGenerator, GenOptions};

struct Counting;

thread_local! {
    static ALLOCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call goes straight to the system allocator. The count is a thread-local cell.
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        bump();
        // SAFETY: the caller's layout goes to the system allocator unchanged.
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        // SAFETY: `p` came from `alloc` or `realloc` above with this layout.
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        bump();
        // SAFETY: as `dealloc`, and `n` is the caller's new size.
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The allocations that this thread makes while `f` runs.
fn allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(|c| c.get());
    f();
    ALLOCS.with(|c| c.get()) - before
}

#[test]
fn the_cquam_receive_path_allocates_nothing_once_warm() {
    let rate = 250e3;
    let mut g = CquamGenerator::new(
        rate,
        GenOptions {
            centre_offset_hz: cquam::default_centre_offset(rate),
            ..GenOptions::default()
        },
    );
    let mut x = vec![Complex32::default(); (8.0 * rate) as usize];
    g.generate(&mut x);
    let cfg = StackConfig::new(Values::defaults(&cquam::CquamSetting::schema()));
    let mut rx = cquam::make_receiver(&cfg).unwrap();
    rx.configure(&InputInfo::new(rate, 999e3, 999e3).recording())
        .unwrap();
    let mut out = Output::default();
    let (warm, steady) = x.split_at(x.len() / 2);
    for c in warm.chunks(8192) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let n = allocations(|| {
        for c in steady.chunks(8192) {
            rx.feed(c, &mut out);
            out.clear();
        }
    });
    assert_eq!(n, 0, "{n} allocations in 4 s of signal");
}

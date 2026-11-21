//! The receive path allocates nothing once warm: the test signal through the stack graph, playing the DAB+ service and then
//! the Layer II service.

use sdrkit_core::{
    owner, Complex32, Generator, InputInfo, Output, Owner, Receiver, SettingSet, StackConfig,
    Values,
};
use sdrkit_stack_dab::generator::{DabGenerator, GenParams};
use sdrkit_stack_dab::DabSetting;
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

/// Allocations while the receiver takes `x[warm..]` in pieces of 65 536 samples, after `x[..warm]`, playing `service`.
fn steady_state(x: &[Complex32], warm: usize, service: &str) -> usize {
    let mut rx = sdrkit_stack_dab::receiver::receiver(&StackConfig::new(Values::defaults(
        &DabSetting::schema(),
    )))
    .expect("builds");
    let id = rx.commands().id("service.select").expect("the command");
    let mut args = rx.commands().def(id).expect("declared").new_args();
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, service);
    owner::invoke(&mut rx, id, &args).expect("selected");
    rx.configure(&InputInfo::new(2.048e6, 225.648e6, 225.648e6).recording())
        .expect("rate");
    let mut out = Output::default();
    for c in x[..warm].chunks(65_536) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let mut n = 0;
    for c in x[warm..].chunks(65_536) {
        out.clear();
        n += allocations(|| rx.feed(c, &mut out));
    }
    n
}

#[test]
fn the_receive_path_allocates_nothing_once_warm() {
    let mut g = DabGenerator::new(GenParams::default());
    let mut x = vec![Complex32::default(); (8.0 * 2.048e6) as usize];
    g.generate(&mut x);
    let warm = (6.0 * 2.048e6) as usize;
    let plus = steady_state(&x, warm, "C101");
    let layer2 = steady_state(&x, warm, "C104");
    assert_eq!((plus, layer2), (0, 0), "allocations (DAB+, Layer II)");
}

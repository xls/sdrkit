//! The tests of sdrkit-proto-link, in one program. Each module tests one module of the crate.
//!
//! The counting allocator needs `unsafe impl GlobalAlloc`. The library itself has no `unsafe` code.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The allocations `f` makes on this thread.
pub fn allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(|c| c.get());
    f();
    ALLOCS.with(|c| c.get()) - before
}

#[path = "link/aprs.rs"]
mod aprs;
#[path = "link/ax25.rs"]
mod ax25;
#[path = "link/components.rs"]
mod components;
#[path = "link/csp.rs"]
mod csp;
#[path = "link/hdlc.rs"]
mod hdlc;
#[path = "link/kiss.rs"]
mod kiss;
#[path = "link/modem.rs"]
mod modem;

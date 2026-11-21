//! Steady state allocates nothing: after warm-up, 1,000 more calls of every streaming type are counted on this thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Runs `f` 50 times to warm up, then 1,000 times, and returns the allocations of the 1,000.
pub fn count(mut f: impl FnMut()) -> usize {
    for _ in 0..50 {
        f();
    }
    let before = ALLOCS.with(|c| c.get());
    for _ in 0..1000 {
        f();
    }
    ALLOCS.with(|c| c.get()) - before
}

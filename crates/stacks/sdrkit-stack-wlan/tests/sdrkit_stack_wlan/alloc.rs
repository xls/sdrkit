//! A counting allocator for the tests that make sure that a steady state allocates nothing. It counts the allocations of the
//! thread that turned counting on.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

struct Counting;

static COUNT: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ON.with(|c| c.get()) {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the layout is passed on unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer came from the system allocator with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ON.with(|c| c.get()) {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the pointer came from the system allocator with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Allocations counted so far.
pub fn count() -> u64 {
    COUNT.load(Ordering::Relaxed)
}

/// Turns counting on or off for this thread.
pub fn counting(on: bool) {
    ON.with(|c| c.set(on));
}

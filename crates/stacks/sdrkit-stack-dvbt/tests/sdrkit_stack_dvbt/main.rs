//! The tests of the DVB-T stacks, one module for each area.

mod common;

mod acceptance;
mod dvbsh;
mod dvbsh_tdm;
mod edge_cases;
mod faults;
mod generator_vs_recording;
mod input_faults;
mod measurements;
mod rate;
mod real;
mod roundtrip;
mod scan;
mod stacks;
mod terrestrial;
mod transmit;
mod units;
mod vector;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts the allocations of the calling thread, so a test can make sure that a call allocates nothing or once.
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: every call is passed to the system allocator unchanged. The count is a thread-local cell.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        // SAFETY: the caller's contract for `alloc` is passed on as it is.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the pointer came from `System.alloc` above, with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// The allocations of this thread so far.
pub fn allocations() -> u64 {
    ALLOCATIONS.with(|c| c.get())
}

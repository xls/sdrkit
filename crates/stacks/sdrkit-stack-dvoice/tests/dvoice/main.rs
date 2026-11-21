//! The test program of the digital voice stacks: shared behaviour, round trips through the standard impairments, the recorded
//! vectors, and checks of single parts.

mod behaviour;
mod checks;
mod common;
mod dcr;
mod dmr_tx;
mod dstar_tx;
mod freedv_ofdm;
mod freedv_tx;
mod m17_tx;
mod roundtrip;
mod sensitivity;
mod tx_host;
mod tetra_dmo;
mod vectors;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts the allocations of the calling thread, so a test can make sure that a steady-state call allocates nothing.
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
static ALLOCATOR: Counting = Counting;

/// The allocations `f` makes on this thread.
pub fn allocations(f: impl FnOnce()) -> u64 {
    let before = ALLOCATIONS.with(Cell::get);
    f();
    ALLOCATIONS.with(Cell::get) - before
}

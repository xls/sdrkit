//! The tests of the cable and satellite TV stacks: one program, one module per stack, and the edge cases.

mod beacon;
mod common;
mod dvbc;
mod dvbc2;
mod dvbs;
mod dvbs2_transmit;
mod dvbs_transmit;
mod edge_cases;
mod isdbs;

/// Counts the allocations of each test thread, for the tests that make sure that a hot path allocates nothing.
pub mod alloc {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<u64> = const { Cell::new(0) };
    }

    /// The system allocator with a count per thread.
    pub struct Counting;

    // SAFETY: every call goes to the system allocator unchanged. The count is a thread-local cell with no allocation of its own.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            // SAFETY: the caller keeps the contract of `GlobalAlloc::dealloc`.
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            // SAFETY: the caller keeps the contract of `GlobalAlloc::realloc`.
            unsafe { System.realloc(p, l, n) }
        }
    }

    #[global_allocator]
    static A: Counting = Counting;

    /// The allocations this thread made so far.
    pub fn count() -> u64 {
        COUNT.with(|c| c.get())
    }
}

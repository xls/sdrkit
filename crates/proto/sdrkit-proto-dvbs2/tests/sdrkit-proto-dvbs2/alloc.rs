//! Paths that allocate nothing once warm. This module holds the counting allocator of the whole test program; the count
//! looks only at its own thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

/// The system allocator with a count per thread.
struct Counting;

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
fn count() -> u64 {
    COUNT.with(|c| c.get())
}

/// The PL scrambling sequence comes from `sdrkit_util_bits::seq::gold_dvbs2` with no table per call: after the first call into a kept
/// buffer, fifteen more calls allocate nothing.
#[test]
fn pl_scrambling_builds_no_table_per_call() {
    use sdrkit_proto_dvbs2::pl::pl_scrambling;
    let mut out = Vec::new();
    pl_scrambling(77, 33282, &mut out);
    let first = out.clone();
    let before = count();
    for _ in 0..15 {
        pl_scrambling(77, 33282, &mut out);
    }
    assert_eq!(count(), before);
    assert_eq!(out, first);
    let mut want = vec![0u8; 33282];
    sdrkit_util_bits::seq::gold_dvbs2(77, 0, &mut want).unwrap();
    assert_eq!(out, want);
}

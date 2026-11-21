//! The tests of the HF keyboard family: each stack's test signal through the standard impairments and back, the recordings,
//! the framework features every stack shares, and checks of details of the standards.

mod ardop;
mod ardop_tx;
mod auto;
mod common;
mod cp16;
mod cw;
mod domino_tx;
mod features;
mod fixes;
mod fsq;
mod fsq_tx;
mod hell;
mod hell_tx;
mod keyboard;
mod locks;
mod mfsk_tx;
mod mt63;
mod mt63_tx;
mod olivia_tx;
mod psk;
mod psk_tx;
mod qrss;
mod ros;
mod rtty;
mod rtty_tx;
mod sitor;
mod sstv;
mod sstv_tx;
mod throb;
mod tx_common;
mod winmor;

/// Counts the allocations of the thread that turned counting on, for the tests of no allocation in steady state.
pub mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ON: Cell<bool> = const { Cell::new(false) };
        static COUNT: Cell<u64> = const { Cell::new(0) };
    }

    /// The allocator of the test program.
    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            if ON.with(Cell::get) {
                COUNT.with(|c| c.set(c.get() + 1));
            }
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            if ON.with(Cell::get) {
                COUNT.with(|c| c.set(c.get() + 1));
            }
            unsafe { System.realloc(p, l, n) }
        }
    }

    #[global_allocator]
    static A: Counting = Counting;

    /// The allocations `f` makes on this thread.
    pub fn count(f: impl FnOnce()) -> u64 {
        COUNT.with(|c| c.set(0));
        ON.with(|o| o.set(true));
        f();
        ON.with(|o| o.set(false));
        COUNT.with(Cell::get)
    }
}

//! The PCC decoder of the NR+ stack: the time per decoded block, and the allocations of a run of blocks of both sizes.

use std::alloc::{GlobalAlloc, Layout, System as Heap};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn note() {
    // `try_with` fails only while the thread ends. Nothing is counted then.
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call goes to the system allocator with the same arguments. `note` only touches thread-local cells.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { Heap.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations that `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    f();
    COUNTING.with(|on| on.set(false));
    ALLOCS.with(|n| n.get())
}

use sdrkit_stack_ism::nrplus::phy::{encode, PccDecoder, PCC_BITS};
use std::time::Instant;

/// One block of each control field size (A = 40, K = 56; A = 80, K = 96) as LLRs, with their field bits.
fn blocks() -> Vec<([i8; PCC_BITS], Vec<u8>)> {
    [40usize, 80]
        .map(|a| {
            let f: Vec<u8> = (0..a).map(|i| ((i * 7) % 3 % 2) as u8).collect();
            let e = encode(&f, 0);
            (e.map(|b| if b == 1 { -40i8 } else { 40 }), f)
        })
        .into()
}

/// Once the decoder is made, a run of blocks of both sizes allocates nothing, and the bits are right. Prints the time per block.
#[test]
fn the_pcc_decoder_allocates_nothing_after_the_start() {
    let blocks = blocks();
    let mut d = PccDecoder::default();
    let n = 2000;
    let mut wrong = 0;
    let t = Instant::now();
    let allocs = allocations(|| {
        for i in 0..n {
            let (llr, f) = &blocks[i % 2];
            match d.read(llr) {
                Some((field, 0)) if field == &f[..] => {}
                _ => wrong += 1,
            }
        }
    });
    eprintln!("per block: {:?}", t.elapsed() / n as u32);
    assert_eq!(wrong, 0);
    assert_eq!(allocs, 0);
}

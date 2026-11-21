//! Checks that the encoder and the decoder allocate nothing per frame: 200 frames run under a counting allocator.

mod common;

use sdrkit_codec_codec2::{Decoder, Encoder, Mode};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn steady_state_allocates_nothing() {
    let books = common::dummy_books();
    for mode in Mode::ALL {
        let mut enc = Encoder::new(mode, &books).unwrap();
        let mut dec = Decoder::new(mode, &books).unwrap();
        let (n, b) = (mode.samples_per_frame(), mode.bytes_per_frame());
        let speech: Vec<i16> = (0..n).map(|i| ((i * 37 % 200) as i16 - 100) * 50).collect();
        let mut frame = vec![0u8; b];
        let mut out = vec![0i16; n];
        enc.encode(&speech, &mut frame).unwrap();
        dec.decode(&frame, &mut out).unwrap();
        let before = ALLOCS.load(Ordering::Relaxed);
        for _ in 0..200 {
            enc.encode(&speech, &mut frame).unwrap();
            dec.decode(&frame, &mut out).unwrap();
        }
        let n_alloc = ALLOCS.load(Ordering::Relaxed) - before;
        assert_eq!(n_alloc, 0, "{mode:?}: {n_alloc} allocations in 200 frames");
    }
}

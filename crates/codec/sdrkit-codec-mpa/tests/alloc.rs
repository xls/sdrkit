//! Steady state allocates nothing: after a warm-up, the decoder and the encoder run 200 more frames with no allocation.

use sdrkit_codec_mpa::{Decoder, Encoder};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|a| a.set(a.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocations() -> usize {
    ALLOCS.with(|a| a.get())
}

#[test]
fn layer2_decode_and_encode_allocate_nothing() {
    let tone: Vec<f32> = (0..1152).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
    let mut enc = Encoder::new(128, 48_000, true).unwrap();
    let mut dec = Decoder::new();
    let (mut frame, mut pcm) = (Vec::with_capacity(400), Vec::with_capacity(2304));
    for _ in 0..3 {
        enc.encode(&tone, &tone, 4, &mut frame).unwrap();
        pcm.clear();
        dec.decode(&frame, &mut pcm).unwrap();
    }
    let before = allocations();
    for _ in 0..200 {
        enc.encode(&tone, &tone, 4, &mut frame).unwrap();
        pcm.clear();
        dec.decode(&frame, &mut pcm).unwrap();
    }
    assert_eq!(allocations() - before, 0);
}

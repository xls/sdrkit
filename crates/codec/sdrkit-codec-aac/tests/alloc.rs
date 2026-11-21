//! Steady state allocates nothing: after a warm-up, the encoder and the decoder run more frames with no allocation.

use sdrkit_codec_aac::{register, Encoder, FRAME};
use sdrkit_core::codec::{CodecSet, Pcm};
use sdrkit_core::media::{Codec, MediaFormat, Packaging};
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
fn encoding_allocates_nothing() {
    let tone: Vec<f32> = (0..FRAME).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
    let mut enc = Encoder::new(48_000).unwrap();
    let mut block = Vec::with_capacity(256);
    for _ in 0..3 {
        enc.encode(&tone, &tone, &[], 120, &mut block).unwrap();
    }
    let before = allocations();
    for _ in 0..200 {
        enc.encode(&tone, &tone, &[], 120, &mut block).unwrap();
    }
    assert_eq!(allocations() - before, 0);
}

#[test]
fn decoding_he_aac_v2_allocates_nothing() {
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/hev2_48k.aac"
    ))
    .unwrap();
    let mut frames = Vec::new();
    let mut at = 0;
    while at + 7 <= data.len() {
        let len = (usize::from(data[at + 3] & 3) << 11)
            | (usize::from(data[at + 4]) << 3)
            | usize::from(data[at + 5] >> 5);
        frames.push(&data[at..at + len]);
        at += len;
    }
    let mut set = CodecSet::default();
    register(&mut set);
    let entry = set.find(Codec::HeAacV2.fourcc(), "").unwrap();
    let mut dec = (entry.factory)(&MediaFormat::new(Codec::HeAacV2, Packaging::Adts)).unwrap();
    let mut pcm = Pcm::default();
    for f in &frames[..5] {
        pcm.clear();
        dec.decode(f, false, &mut pcm).unwrap();
    }
    let before = allocations();
    for f in &frames[5..] {
        pcm.clear();
        dec.decode(f, false, &mut pcm).unwrap();
    }
    pcm.clear();
    dec.decode(&[], true, &mut pcm).unwrap();
    assert_eq!(allocations() - before, 0);
}

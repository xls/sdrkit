//! Steady state allocates nothing: after the first group of pictures the decoder runs more groups with no allocation.

mod common;

use common::*;
use sdrkit_codec_mpeg2video::Decoder;
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
fn decoding_allocates_nothing_after_the_first_group_of_pictures() {
    let (w, h) = (64, 64);
    let mut writer = Writer::new(w, h);
    let mut units = Vec::new();
    let mut want = Planes::new(w, h);
    writer.sequence();
    writer.picture(Params::new(1), 0);
    writer.intra_picture(&Planes::pattern(w, h, 1), &mut want, 4, &|_, _| false);
    units.push(writer.take());
    for (kind, vector) in [(2, [1, 1]), (3, [-1, 0]), (3, [0, 1])] {
        // Vectors that stay inside the picture.
        let mut mbs: Vec<Mb> = (0..16)
            .map(|i| {
                let edge = |at: usize, v: i32| [1, v, v, -1][at];
                Mb::Inter(Inter::frame(
                    Some([edge(i % 4, vector[0]), edge(i / 4, vector[1])]),
                    None,
                ))
            })
            .collect();
        mbs[5] = Mb::Skip;
        writer.picture(Params::new(kind), 1);
        writer.inter_picture(&mbs, 4);
        units.push(writer.take());
    }
    let mut dec = Decoder::new();
    let mut frames = 0;
    let group = |dec: &mut Decoder, frames: &mut usize| {
        for (n, u) in units.iter().enumerate() {
            dec.decode(u, Some(n as i64 * 3600), &mut |_| *frames += 1);
        }
    };
    group(&mut dec, &mut frames);
    group(&mut dec, &mut frames);
    let before = allocations();
    for _ in 0..50 {
        group(&mut dec, &mut frames);
    }
    assert_eq!(allocations() - before, 0);
    assert_eq!(dec.stats().slice_errors, 0);
    assert_eq!(frames, 4 * 52 - 1);
}

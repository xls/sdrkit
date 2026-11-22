//! Checks that the encoder and the decoder allocate nothing per frame: 200 frames run under a counting allocator.

use sdrkit_codec_mbe::{Coder, Decoder, Encoder, FRAME};
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
    for coder in Coder::ALL {
        let mut enc = Encoder::new(coder);
        let mut dec = Decoder::new(coder);
        let mut frame = [0u8; 144];
        let mut out = [0i16; FRAME];
        let speech: [i16; FRAME] = std::array::from_fn(|i| ((i * 37 % 200) as i16 - 100) * 50);
        enc.encode(&speech, &mut frame);
        dec.decode(&frame[..coder.frame_bits()], &mut out);
        let before = ALLOCS.load(Ordering::Relaxed);
        for k in 0..200 {
            enc.encode(&speech, &mut frame);
            if k % 7 == 3 {
                frame[5] ^= 1;
            }
            dec.decode(&frame[..coder.frame_bits()], &mut out);
            enc.encode_tone(40, 90, &mut frame);
        }
        assert_eq!(ALLOCS.load(Ordering::Relaxed), before, "{coder:?}");
    }
}

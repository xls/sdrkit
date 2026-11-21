//! Counts allocations on the steady-state paths. The allocator counts per thread, so the other tests of this program, which
//! run on other threads, do not disturb the counts.

use crate::common::{confident, AMP};
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::{
    ChannelRx, Codec, DetectorConfig, Header, Layout, LoraParams, Payload, Recovery, TxFrame,
};
use std::alloc::{GlobalAlloc, Layout as AllocLayout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = COUNT.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: AllocLayout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: AllocLayout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: AllocLayout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    let before = COUNT.with(|c| c.get());
    f();
    COUNT.with(|c| c.get()) - before
}

#[test]
fn coding_and_recovery_do_not_allocate() {
    let sf = 9u8;
    let codec = Codec::new(sf).unwrap();
    let layout = Layout::new(sf, false, false).unwrap();
    let payload: Vec<u8> = (0..100).map(|i| (i * 3) as u8).collect();
    let mut syms = Vec::with_capacity(1000);
    codec.encode(&payload, &layout, 6, true, &mut syms).unwrap();
    let soft = confident(&syms, 1 << sf);
    let h = Header {
        len: 100,
        cr: 6,
        crc: true,
    };
    let mut out = Payload::default();
    codec.decode_payload(&soft, false, false, h, true, &mut out);
    let n = count(|| {
        for _ in 0..1000 {
            syms.clear();
            codec.encode(&payload, &layout, 6, true, &mut syms).unwrap();
            codec.decode_payload(&soft, false, false, h, true, &mut out);
            codec.decode_payload(&soft, false, false, h, false, &mut out);
            let _ = codec.decode_header(&soft, true);
        }
    });
    assert_eq!(n, 0);
    // The recovery searches, after their buffers have grown on a first frame.
    let mut bad = soft.clone();
    for v in bad.iter_mut().skip(30) {
        v.bins[0] = (v.bins[0] + 12) % 512;
    }
    let mut r = Recovery::new();
    let _ = r.list_decode(&codec, &bad, false, false, h, true, &mut out);
    let _ = r.jump_search(&codec, &bad, false, h, 11.7, &mut out);
    let n = count(|| {
        for _ in 0..10 {
            let _ = r.list_decode(&codec, &bad, false, false, h, true, &mut out);
            let _ = r.jump_search(&codec, &bad, false, h, 11.7, &mut out);
        }
    });
    assert_eq!(n, 0);
}

#[test]
fn the_receiver_does_not_allocate_between_frames() {
    let rate = 500e3;
    let p = LoraParams::new(8, 125e3, 5, 8, 0x12);
    let mut rx =
        ChannelRx::new(rate, 30e3, 125e3, vec![DetectorConfig::new(8, 125e3, 0x12)]).unwrap();
    let mut out = Vec::with_capacity(16);
    // Noise, in pieces of 1000 samples: first to grow the buffers, then counted.
    let mut noise = vec![Complex32::default(); 1000];
    sdrkit_algo_sim::noise::Awgn::with_power(0.01, 4)
        .unwrap()
        .process(&mut noise);
    for _ in 0..1000 {
        rx.process(&noise, &mut out);
    }
    let n = count(|| {
        for _ in 0..1000 {
            rx.process(&noise, &mut out);
        }
    });
    assert_eq!(n, 0);
    assert!(out.is_empty());
    // Frames: after one frame has grown the buffers, each frame allocates only its payload and its symbols.
    let frame = TxFrame::new(&p, &[7u8; 40])
        .unwrap()
        .carrier(30e3)
        .amplitude(AMP)
        .render(rate, 0.02);
    for c in frame.chunks(1000) {
        rx.process(c, &mut out);
    }
    assert_eq!(out.len(), 1);
    out.clear();
    let n = count(|| {
        for c in frame.chunks(1000) {
            rx.process(c, &mut out);
        }
    });
    assert_eq!(out.len(), 1);
    assert!(n <= 2, "{n} allocations for one frame");
}

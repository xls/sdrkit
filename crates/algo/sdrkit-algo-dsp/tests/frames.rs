//! Stereo frames through the decimator and the resampler: the same result as one engine per channel, and no allocation in
//! steady state. (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined in this test only.)

use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::Frame;
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
static A: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    let before = ALLOCS.load(Ordering::Relaxed);
    f();
    ALLOCS.load(Ordering::Relaxed) - before
}

/// Interleaved stereo: a tone on the left, another on the right.
fn stereo(n: usize) -> Vec<f32> {
    (0..n)
        .flat_map(|i| {
            let t = i as f32;
            [(t * 0.031).sin(), 0.5 * (t * 0.007).cos()]
        })
        .collect()
}

fn channel(x: &[f32], c: usize) -> Vec<f32> {
    x.iter().skip(c).step_by(2).copied().collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn stereo_frames_match_channels_and_do_not_allocate() {
    let x = stereo(4800);
    let mut dec = Decimator::<Frame<2>>::new(48e3, 4, 4e3, 80.0);
    let mut res = Resampler::<Frame<2>>::new(48e3, 44.1e3).unwrap();
    let (mut d_out, mut r_out) = (Vec::with_capacity(4096), Vec::with_capacity(8192));
    dec.process(Frame::view(&x), &mut d_out);
    res.process(Frame::view(&x), &mut r_out);

    // The same as one engine per channel.
    for c in 0..2 {
        let mut d1 = Decimator::<f32>::new(48e3, 4, 4e3, 80.0);
        let mut r1 = Resampler::<f32>::new(48e3, 44.1e3).unwrap();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        d1.process(&channel(&x, c), &mut a);
        r1.process(&channel(&x, c), &mut b);
        let d_c = channel(Frame::flat(&d_out), c);
        let r_c = channel(Frame::flat(&r_out), c);
        assert_eq!(a.len(), d_c.len());
        assert_eq!(b.len(), r_c.len());
        assert!(a.iter().zip(&d_c).all(|(p, q)| (p - q).abs() < 1e-5));
        assert!(b.iter().zip(&r_c).all(|(p, q)| (p - q).abs() < 1e-5));
    }

    let n = count(|| {
        for _ in 0..1000 {
            d_out.clear();
            r_out.clear();
            dec.process(Frame::view(&x[..960]), &mut d_out);
            res.process(Frame::view(&x[..960]), &mut r_out);
        }
    });
    assert_eq!(n, 0, "allocations in steady state");
}

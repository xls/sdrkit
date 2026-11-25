//! Checks that the steady-state paths of the equalisers, sync correlators and burst detector allocate nothing: each is run 1,000
//! times after its buffers are made, under an allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; it
//! is defined in this test only.)

use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_dsp::floor::{FloorTracker, MinFollower};
use sdrkit_algo_modem::burst::{BurstConfig, BurstDetector};
use sdrkit_algo_modem::correlate::{SampleSync, SampleSyncSet, SoftCorrelator};
use sdrkit_algo_modem::equalizer::{AdaptiveEq, EqConfig, LsChannel, Mode};
use sdrkit_algo_modem::{Complex32, Constellation};
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

fn signal(n: usize) -> Vec<Complex32> {
    let mut s = 0x1357_9BDFu32;
    (0..n)
        .map(|k| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = if k % 700 < 200 { 5.0 } else { 0.5 };
            Complex32::new(
                a * ((s >> 16) as f32 / 32768.0 - 1.0),
                a * ((s & 0xFFFF) as f32 / 32768.0 - 1.0),
            )
        })
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    let x = signal(4096);
    let c = Constellation::new(PointSet::qam(16, QamMap::DVB).unwrap()).unwrap();

    let mut cfg = EqConfig::new(28, 2);
    cfg.feedback = 6;
    cfg.normalised = true;
    cfg.mu = 0.05;
    let mut eq = AdaptiveEq::new(cfg, c.clone()).unwrap();
    let mut out = Vec::with_capacity(4096);
    eq.process(&x, &mut out);
    assert_eq!(
        count(|| {
            for k in 0..1000 {
                out.clear();
                eq.process(&x[..64], &mut out);
                let _ = eq.step(&x[k % 100..k % 100 + 2], Some(c.points()[k % 16]));
                if k == 500 {
                    eq.set_mode(Mode::Dd);
                }
            }
        }),
        0,
        "equaliser"
    );

    let t: Vec<Complex32> = (0..26).map(|k| c.points()[(k * 5 + 3) % 16]).collect();
    let ls = LsChannel::new(&t, 5).unwrap();
    let mut h = [Complex32::default(); 5];
    assert_eq!(
        count(|| {
            for k in 0..1000 {
                let _ = ls.estimate(&x[k..k + 22], &mut h);
            }
        }),
        0,
        "least squares"
    );

    let mut s = SampleSync::new(&x[..64], 256, &[-2, -1, 0, 1, 2], 0.2).unwrap();
    let mut peaks = Vec::with_capacity(4096);
    let mut m = vec![0f32; 4096];
    s.process(&x, &mut peaks);
    assert_eq!(
        count(|| {
            for k in 0..1000 {
                peaks.clear();
                s.process(&x[(k * 37) % 3000..(k * 37) % 3000 + 300], &mut peaks);
                if k % 100 == 0 {
                    let _ = s.metrics(&x[..1000], 1, &mut m);
                }
            }
            s.finish(&mut peaks);
        }),
        0,
        "sample sync"
    );

    let mut set = SampleSyncSet::new(&[&x[..64], &x[64..100]], 256, &[-1, 0, 1], 0.2).unwrap();
    let mut lists = [Vec::with_capacity(4096), Vec::with_capacity(4096)];
    set.process(&x, &mut lists);
    assert_eq!(
        count(|| {
            for k in 0..1000 {
                lists.iter_mut().for_each(Vec::clear);
                set.process(&x[(k * 37) % 3000..(k * 37) % 3000 + 300], &mut lists);
            }
            set.finish(&mut lists);
        }),
        0,
        "sample sync set"
    );

    let xr: Vec<f32> = x.iter().map(|z| z.re).collect();
    let mut sc = SoftCorrelator::new(&[3.0, -1.0, 1.0, -3.0, 3.0, 1.0], 10.0, 0.5)
        .unwrap()
        .with_inverted(true);
    let mut sp = Vec::with_capacity(4096);
    sc.process(&xr, &mut sp);
    assert_eq!(
        count(|| {
            for k in 0..1000 {
                sp.clear();
                sc.process(&xr[(k * 13) % 3000..(k * 13) % 3000 + 200], &mut sp);
            }
        }),
        0,
        "soft correlator"
    );

    let floor = FloorTracker::MinFollower(MinFollower::new(0.3, 0.05).unwrap());
    let mut cfg = BurstConfig::new(32, 1024);
    cfg.close_db = 3.0;
    cfg.open_db = 6.0;
    let mut d = BurstDetector::new(cfg, floor).unwrap();
    d.process(&x);
    let mut n = 0usize;
    assert_eq!(
        count(|| {
            for k in 0..1000 {
                d.process(&x[(k * 61) % 3000..(k * 61) % 3000 + 500]);
                while let Some(b) = d.take() {
                    n += b.samples.len();
                }
            }
        }),
        0,
        "burst detector"
    );
    assert!(n > 0);
}

//! Checks that the streaming receivers allocate nothing in steady state: the FSK discriminator front end and blind-tracked LLRs,
//! the streaming MLSE and the GFSK MLSE receiver, the OFDM CP synchroniser and receiver, and the tone detector. Each runs 1,000
//! times after a warm-up call has grown its output buffers, under an allocator that counts. (The counting allocator needs
//! `unsafe impl GlobalAlloc`; it is defined in this test only.)

use sdrkit_algo_modem::fsk::{DiscriminatorConfig, DiscriminatorLevels, FskLlr, TrackedFskLlr};
use sdrkit_algo_modem::gmsk::{GfskConfig, GfskMlseDemod, MlseStream};
use sdrkit_algo_modem::ofdm::{CpSync, CpSyncConfig, Estimator, OfdmLayout, OfdmReceiver};
use sdrkit_algo_modem::tone::{ToneDetector, ToneDetectorConfig};
use sdrkit_algo_modem::Complex32;
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

/// Binary FSK at 10 samples per symbol, deviation 0.05 cycles per sample, a fixed bit pattern.
fn fsk(symbols: usize) -> Vec<Complex32> {
    let mut ph = 0f64;
    let mut x = Vec::with_capacity(symbols * 10);
    for k in 0..symbols {
        let f = if (k * 7 + k / 3) % 2 == 0 {
            0.05
        } else {
            -0.05
        };
        for _ in 0..10 {
            ph += f;
            x.push(Complex32::from_polar(
                1.0,
                (std::f64::consts::TAU * (ph % 1.0)) as f32,
            ));
        }
    }
    x
}

/// Repeating OFDM symbols of the 802.11a size (64 + 16 samples), each with its cyclic prefix.
fn ofdm(symbols: usize) -> Vec<Complex32> {
    let mut x = Vec::with_capacity(symbols * 80);
    for s in 0..symbols as u32 {
        let body: Vec<Complex32> = (0..64u32)
            .map(|i| {
                let v = (i * 97 + s * 31) % 13;
                Complex32::new(v as f32 - 6.0, ((i * 7 + s) % 5) as f32 - 2.0)
            })
            .collect();
        x.extend_from_slice(&body[48..]);
        x.extend_from_slice(&body);
    }
    x
}

// One test function, so no other test thread allocates while counting.
#[test]
fn streaming_receivers_do_not_allocate() {
    // FSK: discriminator levels on IQ and on a frequency, and blind-tracked LLRs.
    let x = fsk(200);
    let f: Vec<f32> = (0..x.len())
        .map(|i| if (i / 10) % 2 == 0 { 0.05 } else { -0.05 })
        .collect();
    let mut d = DiscriminatorLevels::new(DiscriminatorConfig::new(10.0, Some(0.05))).unwrap();
    let mut r = DiscriminatorLevels::new(DiscriminatorConfig::new(10.0, None)).unwrap();
    let mut t = TrackedFskLlr::new(FskLlr::gray(4).unwrap(), 16.0).unwrap();
    let mut levels = Vec::with_capacity(4096);
    let mut l2 = [0f32; 2];
    let n = count(|| {
        for _ in 0..1000 {
            levels.clear();
            d.process_iq(&x, &mut levels);
            r.process_real(&f, &mut levels);
            t.symbol(&[9.0, 1.0, 0.5, 1.2], &mut l2).unwrap();
            let _ = t.symbol(&[f32::NAN, 1.0, 0.5, 1.2], &mut l2);
        }
    });
    assert_eq!(n, 0, "FSK streaming paths allocated {n} times");

    // GMSK: the streaming MLSE (with an erasure) and the GFSK MLSE receiver.
    let g = [
        Complex32::new(0.27, 0.0),
        Complex32::new(1.0, 0.0),
        Complex32::new(0.27, 0.0),
    ];
    let mut m = MlseStream::new(&g, 64, 16).unwrap();
    let y: Vec<Complex32> = (0..100)
        .map(|k| Complex32::new(if (k * 5 + k / 4) % 3 == 0 { 1.2 } else { -1.1 }, 0.0))
        .collect();
    let mut gm = GfskMlseDemod::new(GfskConfig::new(10.0, 0.05, Some(0.4)), 64, 16).unwrap();
    let mut llr = Vec::with_capacity(4096);
    let n = count(|| {
        for _ in 0..1000 {
            llr.clear();
            m.process(&y, 0.3, &mut llr).unwrap();
            m.push(Complex32::new(f32::NAN, 0.0), 0.3, &mut llr)
                .unwrap();
            gm.process(&x, &mut llr).unwrap();
        }
        m.finish(&mut llr);
    });
    assert_eq!(n, 0, "GMSK streaming paths allocated {n} times");

    // OFDM: the CP synchroniser locked on a stream, and the receiver after it (both estimators by name).
    let layout = OfdmLayout::ieee80211a();
    let dvbt = OfdmLayout::dvbt_2k(32).unwrap();
    let w = ofdm(40);
    let mut sync = CpSync::new(&layout, CpSyncConfig::default()).unwrap();
    let mut rx = OfdmReceiver::new(
        &layout,
        Estimator::by_name("pilot_ls_smooth", &layout).unwrap(),
        2,
        0,
    )
    .unwrap();
    let mut rx2 =
        OfdmReceiver::new(&dvbt, Estimator::by_name("time_freq", &dvbt).unwrap(), 8, 0).unwrap();
    let noise: Vec<Complex32> = (0..dvbt.longest_symbol())
        .map(|i| {
            Complex32::new(
                ((i * 7919) % 101) as f32 / 50.0 - 1.0,
                ((i * 104_729) % 97) as f32 / 48.0 - 1.0,
            )
        })
        .collect();
    let (mut out, mut events) = (Vec::with_capacity(w.len() * 2), Vec::with_capacity(256));
    let mut cells = 0usize;
    // Warm-up: lock, and let the history reach its working size.
    sync.process(&w, &mut out, &mut events);
    assert!(sync.locked());
    let n = count(|| {
        for _ in 0..1000 {
            out.clear();
            events.clear();
            sync.process(&w[..800], &mut out, &mut events);
            rx.process(&out, |s| cells += s.cells.len());
            rx2.process(&noise, |s| cells += s.weights.len());
        }
    });
    assert_eq!(n, 0, "OFDM streaming paths allocated {n} times");
    assert!(cells > 0);

    // Tone detection: DTMF tones keyed on and off, with a sample that is not a number.
    let mut a: Vec<f32> = (0..1600)
        .map(|i| {
            let s = (std::f64::consts::TAU * 770.0 * i as f64 / 8000.0).sin()
                + (std::f64::consts::TAU * 1336.0 * i as f64 / 8000.0).sin();
            if i < 800 {
                0.3 * s as f32
            } else {
                0.0
            }
        })
        .collect();
    a[5] = f32::NAN;
    let freqs = [697.0, 770.0, 852.0, 941.0, 1209.0, 1336.0, 1477.0, 1633.0];
    let mut td = ToneDetector::with_rate(8000.0, &freqs, ToneDetectorConfig::new(80)).unwrap();
    let mut changes = 0usize;
    let n = count(|| {
        for _ in 0..1000 {
            td.process(&a, |_| changes += 1);
        }
    });
    assert_eq!(n, 0, "tone detection allocated {n} times");
    assert!(changes >= 4000);

    // The Morse key detector on a keyed envelope.
    let mut kd = sdrkit_algo_modem::key::KeyDetector::new(1000.0).unwrap();
    let env: Vec<f32> = (0..2000)
        .map(|i| if (i / 120) % 2 == 0 { 0.05 } else { 1.0 })
        .collect();
    let mut keys = 0usize;
    let n = count(|| {
        for _ in 0..100 {
            for &e in &env {
                keys += kd.step(e, 0.06).is_some() as usize;
            }
        }
    });
    assert_eq!(n, 0, "the key detector allocated {n} times");
    assert!(keys > 1000);
}

//! Checks that the steady-state paths of the FSK, GMSK, MSK, OOK and chirp types allocate nothing: each runs 1,000 times after its
//! buffers are made, under an allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined in
//! this test only.)

use sdrkit_algo_modem::chirp::{ChirpTracker, Dechirp, Direction};
use sdrkit_algo_modem::fsk::{
    Analytic, FftTones, FskLlr, LevelSlicer, MfskConfig, MfskDemod, ToneCorrelator,
};
use sdrkit_algo_modem::gmsk::{derotate, ls_channel, GfskConfig, GfskDemod, Mlse};
use sdrkit_algo_modem::msk::MskDemod;
use sdrkit_algo_modem::ook::{AskSlicer, OokConfig, OokSlicer, PowerIntegral};
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

fn tone(n: usize, f: f64) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            Complex32::from_polar(1.0, (std::f64::consts::TAU * ((f * i as f64) % 1.0)) as f32)
        })
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    let x = tone(256, 0.03);
    let xr: Vec<f32> = x.iter().map(|v| v.re).collect();

    // M-FSK: correlator, analytic step, demodulator, FFT bins, LLRs, level slicer.
    let mut corr = ToneCorrelator::new(&[-0.05, 0.05, 0.1, 0.15], 16).unwrap();
    let mut e4 = [0f32; 4];
    let mut an = Analytic::new(31, 6.0).unwrap();
    let mut ca = vec![Complex32::default(); 256];
    let mut d = MfskDemod::new(MfskConfig::new(&[-0.05, 0.05], 10.0))
        .unwrap()
        .with_analytic(Analytic::new(31, 6.0).unwrap());
    let mut en = Vec::with_capacity(1 << 16);
    let mut ft = FftTones::new(64, &[3, 7, 11, 15]).unwrap();
    let llr = FskLlr::gray(4).unwrap();
    let mut l8 = [0f32; 8];
    let mut lab = [0u16; 4];
    let mut ls = LevelSlicer::c4fm(0.01, 1.0).unwrap();
    let mut lvl = Vec::with_capacity(1 << 16);
    let n = count(|| {
        for _ in 0..1000 {
            for &v in &x[..16] {
                corr.push(v);
            }
            corr.energies(&mut e4).unwrap();
            an.process(&xr, &mut ca).unwrap();
            d.process(&x[..20], &mut en);
            d.process_real(&xr[..20], &mut en).unwrap();
            ft.energies(&x[..64], &mut e4).unwrap();
            let e16 = [
                1.0f32, 9.0, 0.5, 0.2, 4.0, 0.1, 0.3, 0.2, 0.1, 0.1, 6.0, 0.2, 0.3, 0.1, 0.2, 7.0,
            ];
            llr.llrs(&e16, 3.0, 1.0, &mut l8).unwrap();
            llr.decide(&e16, &mut lab).unwrap();
            ls.slice(2.9, &mut l8[..2]).unwrap();
            lvl.clear();
            ls.process(&[1.0, -3.0, 3.1, -0.9], &mut lvl).unwrap();
        }
    });
    assert_eq!(n, 0, "FSK paths allocated {n} times");

    // GFSK, MLSE, LS channel, MSK.
    let mut g = GfskDemod::new(GfskConfig::new(4.0, 0.0625, Some(0.5))).unwrap();
    let mut gl = Vec::with_capacity(1 << 18);
    let taps = [
        Complex32::new(0.3, 0.0),
        Complex32::new(1.0, 0.1),
        Complex32::new(0.3, 0.0),
    ];
    let mut mlse = Mlse::new(&taps, 64).unwrap();
    let y = &x[..64];
    let mut bits = [0u8; 64];
    let mut soft = [0f32; 64];
    let mut yr = [Complex32::default(); 64];
    let mut est = [Complex32::default(); 3];
    let known: Vec<Complex32> = (0..64)
        .map(|k| {
            Complex32::new(
                if (k * k * 11 + k * 37) % 7 < 3 {
                    1.0
                } else {
                    -1.0
                },
                0.0,
            )
        })
        .collect();
    let mut msk = MskDemod::new(8.0, 0.01).unwrap();
    let mut tracked = MskDemod::new(8.0, 0.01)
        .and_then(|d| d.with_offset_tracking(100.0))
        .unwrap();
    let mut ml = Vec::with_capacity(1 << 18);
    let n = count(|| {
        for _ in 0..1000 {
            g.process(&x, &mut gl).unwrap();
            mlse.decode(y, 0.5, None, &mut bits).unwrap();
            mlse.soft(y, 0.5, Some(0), &mut soft).unwrap();
            derotate(y, 3, &mut yr).unwrap();
            ls_channel(y, &known, &mut est, true).unwrap();
            msk.process(&x, &mut ml).unwrap();
            tracked.process(&x, &mut ml).unwrap();
        }
    });
    assert_eq!(n, 0, "GMSK and MSK paths allocated {n} times");

    // OOK, ASK, PPM.
    let mut ook = OokSlicer::new(OokConfig::default()).unwrap();
    let mut pulses = Vec::with_capacity(1 << 16);
    let power: Vec<f32> = (0..256)
        .map(|i| if (i / 20) % 2 == 0 { 1.0 } else { 1e-3 })
        .collect();
    let mut ask = AskSlicer::new(0.5, 1.0, 0.01, 0.1).unwrap();
    let mut ab = vec![0u8; 256];
    let mut asoft = vec![0f32; 256];
    let mut pi = PowerIntegral::new(256).unwrap();
    let mut pb = [0u8; 100];
    let mut pc = [0f32; 100];
    let n = count(|| {
        for _ in 0..1000 {
            ook.process(&power, &mut pulses);
            ook.process_iq(&x, &mut pulses);
            ask.process(&power, &mut ab, &mut asoft).unwrap();
            pi.load(&x).unwrap();
            pi.load_power(&power).unwrap();
            pi.ppm_bits(1.3, 1.2, &mut pb, &mut pc).unwrap();
            let _ = pi.ppm_symbol(0.0, 2.0, 4);
        }
    });
    assert_eq!(n, 0, "OOK paths allocated {n} times");

    // Chirps.
    let mut dc = Dechirp::new(7, 2).unwrap();
    let sym: Vec<Complex32> = (0..256).map(|k| dc.base_chirp(k / 2 + 5)).collect();
    let mut tr = ChirpTracker::new(256, 2, 0.02, 100.0).unwrap();
    let n = count(|| {
        for _ in 0..1000 {
            let p = dc.dechirp(&sym, Direction::Up, 0.1).unwrap();
            let q = dc.dechirp_at(&sym, 0.4, Direction::Down, 0.0).unwrap();
            let _ = dc.amplitude_noise(&p);
            tr.update(p.timing.unwrap_or(q.frac));
        }
    });
    assert_eq!(n, 0, "chirp paths allocated {n} times");
}

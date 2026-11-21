//! Checks that the steady-state paths of the slicers, synchronisers, frequency estimators, level estimators and squelches allocate
//! nothing: each is run 1,000 times after its buffers are made, under an allocator that counts. (The counting allocator needs
//! `unsafe impl GlobalAlloc`; it is defined in this test only.)

use sdrkit_algo_dsp::constellation::{Dvbs2Rate, PointSet, QamMap};
use sdrkit_algo_dsp::floor::{FloorTracker, MinFollower};
use sdrkit_algo_dsp::interp::{Cubic, Polyphase};
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas, DecisionDirected, ToneLoop};
use sdrkit_algo_modem::differential::Differential;
use sdrkit_algo_modem::freq::{fitz, kay, lag_phase, luise_reggiannini, multi_lag, PowerLine};
use sdrkit_algo_modem::level::{DataAided, Evm, M2m4};
use sdrkit_algo_modem::squelch::{Gate, PowerSquelch, QuietingSquelch, SnrSquelch};
use sdrkit_algo_modem::timing::{
    max_energy, spectral_line, Gardner, MatchedInterp, SymbolSync, SymbolSyncConfig, ZeroCrossing,
};
use sdrkit_algo_modem::{Complex32, Constellation, Demap};
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
    let mut s = 0x2468_ACE1u32;
    (0..n)
        .map(|k| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let ph = 0.013 * k as f32 + if s & 0x8000 != 0 { 0.785 } else { 2.356 };
            Complex32::from_polar(1.0, ph) + Complex32::new((s >> 20) as f32 * 1e-4, 0.0)
        })
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    let x = signal(4096);
    let xr: Vec<f32> = x.iter().map(|z| z.re).collect();
    const N: usize = 1000;

    // Slicers and demappers: separable, PSK and generic paths, both methods.
    let sets = [
        PointSet::qam(256, QamMap::DVB).unwrap(),
        PointSet::psk(8, 0.0).unwrap(),
        PointSet::dvbs2_32apsk(2.84, 5.27).unwrap(),
        PointSet::dvbs2_16apsk(Dvbs2Rate::R3_4.gamma_16apsk()).unwrap(),
    ];
    let cs: Vec<Constellation> = sets
        .into_iter()
        .map(|s| Constellation::new(s).unwrap())
        .collect();
    let mut llr = [0f32; 8 * 64];
    let mut bits = [0u8; 8];
    let w = [1.0f32; 64];
    for c in &cs {
        let a = count(|| {
            for k in 0..N {
                let z = x[k % 4096];
                c.decide(z);
                c.decide_bits(z, &mut bits).unwrap();
                c.demap(z, 0.1, Demap::Exact, &mut llr).unwrap();
                c.demap(z, 0.1, Demap::MaxLog, &mut llr).unwrap();
            }
            c.demap_block(&x[..64], 0.1, Demap::MaxLog, &mut llr)
                .unwrap();
            c.demap_block_weighted(&x[..64], &w, Demap::Exact, &mut llr)
                .unwrap();
        });
        assert_eq!(a, 0, "constellation of {} points", c.len());
    }
    let mut d = Differential::pi4_dqpsk_tetra();
    assert_eq!(
        count(|| {
            for &z in x.iter().take(N) {
                d.decide(z);
                d.demap(z, 0.1, Demap::Exact, &mut llr).unwrap();
            }
        }),
        0,
        "differential"
    );

    // Symbol timing: complex with a polyphase interpolator, real with a cubic, the matched interpolator; burst estimators.
    let mut out = Vec::with_capacity(8192);
    let mut outr: Vec<f32> = Vec::with_capacity(8192);
    let mut s1 = SymbolSync::new(
        Gardner,
        Polyphase::<16, 64>::design(0.8, 60.0).unwrap(),
        SymbolSyncConfig::new(4.0),
    )
    .unwrap();
    let mut cfg = SymbolSyncConfig::new(4.0);
    cfg.two_per_symbol = true;
    cfg.oqpsk = true;
    let mut s2 = SymbolSync::new(
        Gardner,
        MatchedInterp::<32, 64>::new(Shape::Rrc { beta: 0.35 }, 4.0).unwrap(),
        cfg,
    )
    .unwrap();
    let mut s3 = SymbolSync::new(ZeroCrossing, Cubic, SymbolSyncConfig::new(5.5)).unwrap();
    s1.process(&x, &mut out);
    let a = count(|| {
        for k in 0..N {
            out.clear();
            outr.clear();
            let piece = &x[(k * 4) % 4000..(k * 4) % 4000 + 4];
            s1.process(piece, &mut out);
            s2.process(piece, &mut out);
            s3.process(&xr[(k * 4) % 4000..(k * 4) % 4000 + 4], &mut outr);
        }
        spectral_line(&x, 4.0).unwrap();
        max_energy(&x[..512], 4.0, 16).unwrap();
        s1.run(&x[..400], 1.5, &mut out).unwrap();
    });
    assert_eq!(a, 0, "symbol sync");

    // Carrier loops.
    let mut c4 = CarrierSync::new(Costas::<4>, CarrierConfig::new(0.01)).unwrap();
    let mut dd = CarrierSync::new(
        DecisionDirected::new(cs[0].clone()),
        CarrierConfig::new(0.01),
    )
    .unwrap();
    let mut tl = ToneLoop::new(48_000.0, 1000.0, 20.0, 50.0, 200.0).unwrap();
    let mut buf = vec![Complex32::default(); 4096];
    let a = count(|| {
        for &z in x.iter().take(N) {
            c4.push(z);
            dd.push(z);
            c4.push_known(z, z);
            c4.push_free(z);
            tl.push_real(z.re);
            tl.push(z);
            tl.harmonic(2);
        }
        c4.process(&x, &mut buf).unwrap();
    });
    assert_eq!(a, 0, "carrier");

    // Frequency estimators.
    let mut pl = PowerLine::new(4, 1024).unwrap();
    pl.set_reject(&[0.1, 0.2], 0.01).unwrap();
    let mut pp = PowerLine::new(2, 1024).unwrap();
    pl.estimate(&x).unwrap();
    let a = count(|| {
        for k in 0..20 {
            pl.estimate(&x[k..k + 1024]).unwrap();
            pl.estimate(&x).unwrap();
            pp.estimate_pair(&x, 0.125).unwrap();
        }
        for k in 0..N {
            let s = &x[k % 64..k % 64 + 256];
            kay(s).unwrap();
            lag_phase(s, 16).unwrap();
        }
        fitz(&x[..512], 64).unwrap();
        luise_reggiannini(&x[..512], 64).unwrap();
        multi_lag(&x[..512], &[1, 4, 16]).unwrap();
    });
    assert_eq!(a, 0, "frequency estimators");

    // Levels and squelch.
    let mut m = M2m4::for_constellation(&cs[0]).unwrap();
    let mut da = DataAided::new();
    let mut ev = Evm::new();
    let mut ps = PowerSquelch::new(480, Gate::new(-20.0, -25.0, 2, 10).unwrap()).unwrap();
    let floor = FloorTracker::MinFollower(MinFollower::new(0.5, 0.1).unwrap());
    let mut ss = SnrSquelch::new(480, floor, Gate::new(6.0, 3.0, 2, 10).unwrap()).unwrap();
    let mut qs =
        QuietingSquelch::new(24_000.0, 240, 6_000.0, Gate::new(6.0, 3.0, 2, 10).unwrap()).unwrap();
    let a = count(|| {
        for k in 0..N {
            let z = x[k % 4096];
            m.push(z);
            da.push(z, z);
            da.push_decided(z, &cs[1]);
            ev.push_decided(z, &cs[1]);
            m.estimate();
            da.estimate();
            ev.mer_db();
        }
        for _ in 0..10 {
            ps.process(&x);
            ss.process(&x);
            qs.process(&xr);
        }
    });
    assert_eq!(a, 0, "levels and squelch");
}

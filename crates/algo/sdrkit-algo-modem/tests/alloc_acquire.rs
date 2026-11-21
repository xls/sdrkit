//! Makes sure that the steady-state paths of the acquisition and tracking types allocate nothing: the SymbolSync carrier feedback and
//! half-symbol move, the bit clock, the spectrum searches, the DAB frame pieces, the shifted lag correlator, the autocorrelation,
//! the grid interpolator, the continuous FFT retune, the OFDM clock loop, the tone loop gears, the normalised equaliser, the budgeted
//! code search, the carrier loop hand-off, the false-lock test and the weighted M2M4. Each runs many times after its buffers are made, under an allocator that counts. (The counting allocator needs
//! `unsafe impl GlobalAlloc`, so it is defined in this test only.)

use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas, ToneLoop};
use sdrkit_algo_modem::correlate::Autocorrelation;
use sdrkit_algo_modem::dsss::{CodeSearch, Integration, SearchConfig, SearchGrid, SearchJob};
use sdrkit_algo_modem::equalizer::{AdaptiveEq, EqConfig};
use sdrkit_algo_modem::freq::{BandSearch, TemplateFit};
use sdrkit_algo_modem::level::M2m4;
use sdrkit_algo_modem::lock::HalfRateFalseLock;
use sdrkit_algo_modem::ofdm::{
    place_window, quietest_window, CpSync, CpSyncConfig, DelayCleaner, FftFramer, GridInterp,
    ImpulseResponse, LagCorrelator, OfdmLayout,
};
use sdrkit_algo_modem::timing::{EdgeClock, Gardner, SymbolSync, SymbolSyncConfig};
use sdrkit_algo_modem::{Complex32, Constellation, PointSet};
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

fn noise(n: usize) -> Vec<Complex32> {
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let a = (s & 0xFFFF) as f32 / 32768.0 - 1.0;
            let b = ((s >> 16) & 0xFFFF) as f32 / 32768.0 - 1.0;
            Complex32::new(a, b)
        })
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn acquisition_and_tracking_paths_do_not_allocate() {
    let x = noise(16_384);
    let xr: Vec<f32> = x.iter().map(|v| v.re).collect();

    // Symbol timing with the carrier fed back, and the half-symbol move.
    let mut cfg = SymbolSyncConfig::new(4.0);
    cfg.oqpsk = true;
    cfg.pulse = Shape::Rc { beta: 0.5 };
    let mut sync = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
    let mut clock = EdgeClock::new(1200.0, 48_000.0, 0.5, 0.8).unwrap();
    let mut syms = 0usize;
    let n = count(|| {
        for i in 0..200 {
            sync.process_each(&x[..1024], |y| {
                syms += 1;
                Some(y.re as f64 * 0.01)
            });
            if i % 50 == 0 {
                sync.shift_half_symbol();
            }
            for &v in &xr[..1024] {
                clock.sample(u8::from(v > 0.0));
            }
        }
    });
    assert_eq!(n, 0, "symbol sync and bit clock");

    // Spectrum searches.
    let p: Vec<f32> = x[..1024].iter().map(|v| v.norm_sqr() + 1.0).collect();
    let fit = TemplateFit::new(250.0, 72e3, 0.6, 1024, 15e3).unwrap();
    let mut band = BandSearch::new(1024, 5e3, 100.0).unwrap();
    band.skip(1e3, 500.0);
    let n = count(|| {
        for _ in 0..200 {
            fit.fit(&p, 15e3).unwrap();
            band.find(&p, -20e3, 20e3);
        }
    });
    assert_eq!(n, 0, "spectrum searches");

    // DAB frame pieces at mode I sizes.
    let known: Vec<(i32, Complex32)> = (-768..=768)
        .filter(|&k| k != 0)
        .map(|k| (k, Complex32::new(if k % 3 == 0 { 1.0 } else { -1.0 }, 0.0)))
        .collect();
    let bins: Vec<usize> = known
        .iter()
        .map(|&(k, _)| k.rem_euclid(2048) as usize)
        .collect();
    let mut ir = ImpulseResponse::new(2048, &known).unwrap();
    let mut cleaner = DelayCleaner::new(2048, &bins).unwrap();
    let mut h: Vec<Complex32> = x[..1536].to_vec();
    let mut scratch = Vec::with_capacity(4096);
    let mut powers = vec![0f32; 2048];
    let n = count(|| {
        for _ in 0..20 {
            quietest_window(&x, 2048, 64, 128, 8192, &mut scratch).unwrap();
            ir.compute(&x[..2048], 2).unwrap();
            ir.peak();
            for (o, v) in powers.iter_mut().zip(ir.response()) {
                *o = v.norm_sqr();
            }
            place_window(&powers, 488, 16, 0.1, 12.0, &mut scratch).unwrap();
            cleaner.make_mask(&h, 12.0, 2, 0.25).unwrap();
            cleaner.clean(&mut h).unwrap();
        }
    });
    assert_eq!(n, 0, "DAB frame pieces");

    // Correlators, the grid interpolator and the continuous retune.
    let mut lag = LagCorrelator::with_shift(1024, 542, 1.0 / 1024.0, 0.9, 0.5).unwrap();
    let mut auto = Autocorrelation::new(8192, 2048).unwrap();
    let mut r = vec![Complex32::default(); 2049];
    let gi = GridInterp::new(3, 4).unwrap();
    let mut filled = vec![Complex32::default(); 3 * 599 + 1];
    let layout = OfdmLayout::ieee80211a();
    let mut fr = FftFramer::new(&layout, 4).unwrap();
    let mut cells = vec![Complex32::default(); layout.used()];
    let mut peaks = 0usize;
    let n = count(|| {
        for s in 0..50u64 {
            lag.process(&x[..4096], |_| peaks += 1);
            lag.energies();
            if s % 10 == 0 {
                auto.process(&x[..8192], &mut r).unwrap();
            }
            gi.fill(&x[..600], &mut filled).unwrap();
            fr.set_freq_continuous(1e-3 + s as f64 * 1e-6);
            fr.demodulate(&x, 20, s * 80, &mut cells).unwrap();
        }
    });
    assert_eq!(n, 0, "correlators, grid interpolator, continuous retune");

    // The OFDM clock loop on 802.11a-sized symbols.
    let mut ofdm = Vec::with_capacity(80 * 400);
    for s in 0..400usize {
        let body = &x[(s * 37) % 8000..(s * 37) % 8000 + 64];
        ofdm.extend_from_slice(&body[48..]);
        ofdm.extend_from_slice(body);
    }
    let mut cp = CpSync::new(&layout, CpSyncConfig::default()).unwrap();
    cp.enable_clock_loop(0.01, 1000.0).unwrap();
    let (mut out, mut events) = (Vec::with_capacity(80 * 400), Vec::with_capacity(4096));
    cp.process(&ofdm[..8000], &mut out, &mut events);
    let n = count(|| {
        for c in ofdm[8000..].chunks(800) {
            out.clear();
            events.clear();
            cp.process(c, &mut out, &mut events);
            cp.timing_error(0.01);
        }
    });
    assert_eq!(n, 0, "OFDM clock loop");

    // Tone loop gears and the normalised equaliser.
    let mut pll = ToneLoop::new(48_000.0, 1_000.0, 20.0, 50.0, 200.0).unwrap();
    pll.set_gears(80.0, 20.0).unwrap();
    let c = Constellation::new(PointSet::psk(8, 0.0).unwrap()).unwrap();
    let mut eq = AdaptiveEq::new(EqConfig::new(16, 1), c).unwrap();
    eq.set_input_normalisation(Some(0.002)).unwrap();
    eq.set_divergence_limit(16.0).unwrap();
    let mut eq_out = Vec::with_capacity(16_384);
    let n = count(|| {
        for _ in 0..20 {
            pll.process_real(&xr);
            eq_out.clear();
            eq.process(&x, &mut eq_out);
        }
    });
    assert_eq!(n, 0, "tone loop gears and normalised equaliser");

    // The budgeted code search with a reused job, the carrier loop that reports its frequency, the false-lock test and the
    // exponentially weighted M2M4.
    let period = 1024;
    let x = noise(8 * period);
    let code: Vec<f32> = x[..period].iter().map(|z| z.re.signum()).collect();
    let mut search = CodeSearch::new(period).unwrap();
    let replica = search.replica(&code).unwrap();
    let grid = SearchGrid::window(period, 0.0, 4.0 / period as f64, 4).unwrap();
    let mut cfg = SearchConfig::new(Integration {
        periods: 8,
        coherent: 2,
    });
    cfg.code_doppler = 0.002;
    let mut job = SearchJob::new();
    search.start(&mut job, &x, &grid, &cfg).unwrap();
    let mut budget = f64::INFINITY;
    search.step(&mut job, &replica, &mut budget).unwrap();
    let mut ccfg = CarrierConfig::new(0.015);
    ccfg.fll_bn_t = Some(0.03);
    let mut cs = CarrierSync::new(Costas::<2>, ccfg).unwrap();
    let mut fl = HalfRateFalseLock::new(10, 50).unwrap();
    let mut m = M2m4::psk().with_memory(100).unwrap();
    let n = count(|| {
        for _ in 0..3 {
            search.start(&mut job, &x, &grid, &cfg).unwrap();
            let mut done = None;
            while done.is_none() {
                // Each stage of this search costs 2 units or less.
                let mut budget = 2.0;
                done = search.step(&mut job, &replica, &mut budget).unwrap();
            }
            std::hint::black_box(done);
        }
        let mut f = 0.0;
        for &z in &x {
            let y = cs.push(z);
            f += cs.take_freq();
            if fl.push(y).is_some() {
                fl.reset();
            }
            m.push(y);
        }
        std::hint::black_box((f, m.estimate()));
    });
    assert_eq!(
        n, 0,
        "code search, carrier hand-off, false-lock test, weighted M2M4"
    );
}

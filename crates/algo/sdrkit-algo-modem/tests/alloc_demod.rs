//! Makes sure that the analogue demodulator, the PSK front end, carrier search and demodulator, and the framed OFDM modulator
//! allocate nothing in steady state. Each runs many times after a warm-up call has grown its buffers, under an allocator that
//! counts. (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined in this test only.)

use sdrkit_algo_modem::analog::{
    AgcConfig, AnalogConfig, AnalogDemod, AnalogMode, AudioFilter, BandPass, Gain, LowPass,
};
use sdrkit_algo_modem::ofdm::{FrameLayout, FrameModulator};
use sdrkit_algo_modem::psk::{PskDemodConfig, PskDemodulator, PskFrontEnd, PskOrder};
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
        .map(|i| Complex32::from_polar(0.3, (std::f64::consts::TAU * f * i as f64) as f32))
        .collect()
}

fn analog_demodulators() {
    let x = tone(2400, 0.05);
    let bp = BandPass {
        low_hz: 300.0,
        high_hz: 3000.0,
        transition_hz: 270.0,
        atten_db: 60.0,
    };
    for mode in [
        AnalogMode::Usb,
        AnalogMode::Nfm,
        AnalogMode::Am,
        AnalogMode::Cw,
    ] {
        let mut c = AnalogConfig::new(mode, 24e3);
        c.decimation = 3;
        if mode.is_sideband() {
            c.sideband = Some(bp);
        } else {
            c.channel = Some(LowPass::Taps {
                taps: 95,
                cutoff_hz: 6250.0,
            });
            c.audio = Some(AudioFilter::BandPass(bp));
        }
        c.gain = Gain::Auto(AgcConfig::from_factors(8e3, 0.5, 0.01, 0.0001, 1e4));
        c.dc_block_hz = Some(20.0);
        c.deemphasis_s = Some(75e-6);
        let mut d = AnalogDemod::new(c).unwrap();
        let (mut a, mut p) = (Vec::with_capacity(1000), Vec::with_capacity(1000));
        d.process(&x, &mut a, Some(&mut p));
        let n = count(|| {
            for _ in 0..200 {
                a.clear();
                p.clear();
                d.process(&x, &mut a, Some(&mut p));
            }
        });
        assert_eq!(n, 0, "{mode:?}");
    }
}

fn psk_chain() {
    let x = tone(4096, 0.01);
    let taps = sdrkit_algo_dsp::fir::lowpass(48e3, 4e3, 8e3, 60.0).unwrap();
    let mut fe = PskFrontEnd::new(48e3, 1e3, Some((&taps, 3)), None, true).unwrap();
    let mut out = Vec::with_capacity(2000);
    fe.process(&x, &mut out);
    let mut d = PskDemodulator::new(PskDemodConfig::new(PskOrder::Qpsk, 8.0)).unwrap();
    let mut symbols = Vec::with_capacity(1024);
    d.process(&x, &mut symbols);
    let n = count(|| {
        for _ in 0..50 {
            out.clear();
            fe.process(&x, &mut out);
            symbols.clear();
            d.process(&x, &mut symbols);
        }
    });
    assert_eq!(n, 0);
}

fn framed_modulator() {
    let l = FrameLayout {
        rate: 48e3,
        nfft: 256,
        cp: 32,
        cp0: 32,
        nsym: 4,
        kmin: -60,
        kmax: 60,
        known: vec![vec![(0, Complex32::new(1.0, 0.0))]; 4],
        null: vec![60],
        max_shift: 4,
        adv: 16,
        sync_threshold: 0.45,
        guard_threshold: 0.35,
        search_taps: None,
        pair_band: None,
    };
    let mut m = FrameModulator::new(l);
    let cells = vec![Complex32::new(0.7, 0.7); 4 * 121];
    let mut out = Vec::with_capacity(4 * 288);
    m.frame(&cells, &mut out);
    let n = count(|| {
        for _ in 0..100 {
            out.clear();
            m.frame(&cells, &mut out);
        }
    });
    assert_eq!(n, 0);
}

/// One test, so no other test thread allocates while the counter runs.
#[test]
fn steady_state_does_not_allocate() {
    analog_demodulators();
    psk_chain();
    framed_modulator();
}

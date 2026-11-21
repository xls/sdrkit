//! Checks that the steady-state paths of the shared DSP types allocate nothing: each is run 1,000 times after its buffers are made,
//! under an allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined in this test only.)

use sdrkit_algo_dsp::bank::{OverlapSaveBank, PolyphaseBank};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::cic::Cic;
use sdrkit_algo_dsp::decim::{ComplexDecimator, FirDecimator};
use sdrkit_algo_dsp::fir::{self, Convolver};
use sdrkit_algo_dsp::floor::{BinMedian, ChunkPercentile, MinFollower};
use sdrkit_algo_dsp::goertzel::{goertzel, Goertzel, ToneBank};
use sdrkit_algo_dsp::iir::{BiquadCoeffs, ComplexBiquad, ComplexOnePole, Deemphasis, PreEmphasis};
use sdrkit_algo_dsp::interp::{CatmullRom, Cubic, Polyphase, Reader};
use sdrkit_algo_dsp::linalg::LeastSquares;
use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
use sdrkit_algo_dsp::peak::{candan, line_fit_uniform, log_parabolic};
use sdrkit_algo_dsp::pulse::{taps_into, Norm, PulseTable, Shape};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::ring::{DelayLine, SampleRing};
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::stats::{quantile_f64, WindowSum};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::Complex32;
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
        .map(|i| Complex32::from_polar(0.5, (std::f64::consts::TAU * f * i as f64) as f32))
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    let x = tone(1024, 0.013);
    let xr: Vec<f32> = x.iter().map(|v| v.re).collect();

    // Decimators: per-sample push and blocks.
    let taps = fir::lowpass(1.0, 0.05, 0.15, 60.0).unwrap();
    let mut fd = FirDecimator::<Complex32>::new(&taps, 4);
    let ctaps = fir::bandpass_complex(48_000.0, 300.0, 2700.0, 400.0, 60.0).unwrap();
    let mut cd = ComplexDecimator::new(&ctaps, 4).unwrap();
    let mut cic = Cic::<Complex32>::new(4, 8).unwrap();
    let mut out = Vec::with_capacity(4096);
    // Warm up once (the output Vec reaches its size).
    fd.process(&x, &mut out);
    cd.process(&x, &mut out);
    out.clear();
    let n = count(|| {
        for _ in 0..1000 {
            out.clear();
            fd.process(&x[..64], &mut out);
            cd.process(&x[..64], &mut out);
            cic.process(&x[..64], &mut out);
            for &v in &x[..64] {
                let _ = fd.push(v);
                let _ = cd.push(v);
                let _ = cic.push(v);
            }
        }
    });
    assert_eq!(n, 0, "decimators");

    // Interpolators and the reader.
    let poly = Reader::new(Polyphase::<32, 64>::design(0.85, 60.0).unwrap());
    let mut sym = vec![Complex32::default(); 64];
    let n = count(|| {
        for k in 0..1000 {
            poly.read(&x, 40.3 + k as f64 * 0.01, 7.7, 120.0, &mut sym);
            let _ = Reader::new(Cubic).at(&x, 3.5);
            let _ = Reader::new(CatmullRom).at(&xr, -0.5);
        }
    });
    assert_eq!(n, 0, "interpolators");

    // Loops.
    let mut lf = LoopFilter::new(0.01, 0.707).unwrap();
    let mut acc = PhaseAcc::new(0.01);
    let n = count(|| {
        for i in 0..1000 {
            let z = acc.next_phasor();
            acc.steer(lf.update((z.im * 0.01 + i as f32 * 1e-6) as f64));
        }
    });
    assert_eq!(n, 0, "loops");

    // Goertzel, tone bank, IIR, floors, peaks, pulses.
    let mut g = Goertzel::new(0.1, 205).unwrap();
    let mut bank = ToneBank::new(&[0.08, 0.09, 0.1, 0.11, 0.15, 0.16, 0.18, 0.2], 205).unwrap();
    let mut cb = ComplexBiquad::from_real(BiquadCoeffs::lowpass(48_000.0, 1000.0, 0.707));
    let mut op = ComplexOnePole::resonator(48_000.0, 1000.0, 50.0).unwrap();
    let mut de = Deemphasis::bilinear(50e-6, 48_000.0).unwrap();
    let mut pre = PreEmphasis::new(50e-6, 48_000.0).unwrap();
    let mut bm = BinMedian::new(8).unwrap();
    bm.reserve(1024);
    let mut cp = ChunkPercentile::new(100, 0.5, 1, 0.3).unwrap();
    let mut mf = MinFollower::new(0.5, 0.1).unwrap();
    let conv = Convolver::<Complex32>::new(&taps).unwrap();
    let mut conv_out = vec![Complex32::default(); x.len()];
    let table = PulseTable::new(Shape::GaussianFreq { bt: 0.3 }, 3.0, 256).unwrap();
    let mut ptaps = Vec::with_capacity(1024);
    let mut buf = x.clone();
    let mut rbuf = xr.clone();
    let line = [1.0, 2.0, 2.9, 4.2];
    let n = count(|| {
        for i in 0..1000 {
            for &v in &xr[..32] {
                let _ = g.push(v);
            }
            bank.process(&xr[..64], |_, a| assert_eq!(a.len(), 8));
            cb.process(&mut buf[..32]);
            op.process(&mut buf[..32]);
            de.process(&mut rbuf[..32]);
            pre.process(&mut rbuf[..32]);
            let _ = goertzel(&xr[..100], 0.013);
            let _ = bm.estimate(xr.iter().map(|v| v * v));
            let _ = cp.push(xr[i % 1024].abs());
            let _ = mf.push(xr[i % 1024].abs());
            let _ = log_parabolic(0.5, 1.0, 0.7);
            let _ = candan(x[1], x[2], x[3], 64);
            let _ = line_fit_uniform(&line);
            let _ = table.value(i as f64 * 0.001);
            taps_into(
                Shape::Rrc { beta: 0.35 },
                4.0,
                8.0,
                Norm::Energy,
                &mut ptaps,
            )
            .unwrap();
            if i % 100 == 0 {
                conv.centred(&x, &mut conv_out).unwrap();
            }
        }
    });
    assert_eq!(n, 0, "tones, IIR, floors, peaks, pulses");

    // Filter banks and Welch.
    let mut pb = PolyphaseBank::new(16, 8, 2, 60.0, &[1, 5, 9]).unwrap();
    let mut pout: Vec<Vec<Complex32>> = (0..3).map(|_| Vec::with_capacity(8192)).collect();
    let mut ob = OverlapSaveBank::new(1e6, 1024).unwrap();
    ob.add_channel(100e3, 20e3, 40e3, 50.0).unwrap();
    ob.add_channel(-250e3, 50e3, 80e3, 50.0).unwrap();
    let mut oout: Vec<Vec<Complex32>> = (0..2).map(|_| Vec::with_capacity(8192)).collect();
    let mut welch = Welch::new(256, Window::Hann, 0.5, Scaling::Power).unwrap();
    let mut spec = vec![0.0; 256];
    let n = count(|| {
        for _ in 0..1000 {
            pout.iter_mut().for_each(|v| v.clear());
            pb.process(&x[..200], &mut pout).unwrap();
        }
    });
    assert_eq!(n, 0, "polyphase bank");
    let n = count(|| {
        for _ in 0..1000 {
            oout.iter_mut().for_each(|v| v.clear());
            ob.process(&x[..200], &mut oout).unwrap();
        }
    });
    assert_eq!(n, 0, "overlap-save bank");
    let n = count(|| {
        for _ in 0..10 {
            welch.compute(&x, &mut spec).unwrap();
        }
    });
    assert_eq!(n, 0, "Welch");

    // Shared-layer helpers: window sums, rings, delay lines, least squares, quantiles, the channeliser with a stop band.
    let mut ws = WindowSum::<Complex32>::new(64).unwrap();
    let mut ring = SampleRing::<Complex32>::new(256).unwrap();
    let mut dl = DelayLine::<f32>::new(17);
    let mut dl_buf = xr.clone();
    let mut ls = LeastSquares::new(8).unwrap();
    let mut sol = [0.0f64; 8];
    let vals: Vec<f64> = (0..300).map(|i| ((i * 37) % 101) as f64).collect();
    let mut qs = Vec::with_capacity(300);
    let mut cfg = ChannelConfig::new(2e6, 0.0, 80e3, 153.6e3);
    cfg.exact_rate = true;
    let mut ch = Channelizer::with_stop(cfg, 76.8e3).unwrap();
    let mut ch_out = Vec::with_capacity(4096);
    ch.process(&x, &mut ch_out);
    let mut copy = [Complex32::default(); 32];
    let n = count(|| {
        for i in 0..1000 {
            for &v in &x[..64] {
                ws.push(v);
                ring.push(v);
            }
            ring.copy(ring.start() + 3, &mut copy);
            dl.process(&mut dl_buf[..100]);
            let r = [1.0, i as f64, 0.5, -1.0, 2.0, 0.25, (i % 7) as f64, 1.5];
            ls.add_row(&r, 1.0, 1.0).unwrap();
            if i % 100 == 99 {
                ls.solve(1e-3, &mut sol).unwrap();
                quantile_f64(&vals, 0.84, &mut qs).unwrap();
            }
        }
    });
    assert_eq!(
        n, 0,
        "window sums, rings, delay lines, least squares, quantiles"
    );
    let n = count(|| {
        for _ in 0..1000 {
            ch_out.clear();
            ch.process(&x, &mut ch_out);
        }
    });
    assert_eq!(n, 0, "channeliser with a stop band");

    // Ring windows, the zero-padded FFT, the centred mean and the carrier spectrum.
    let mut wf =
        sdrkit_algo_dsp::fft::WindowedFft::new(256, 64, sdrkit_algo_dsp::window::Window::Hann)
            .unwrap();
    let mut cs = sdrkit_algo_dsp::spectrum::CarrierSpectrum::new(256, 48e3, 40e3).unwrap();
    let powers = vec![1.0f32; 256];
    let vals64: Vec<f64> = (0..300).map(|i| (i % 13) as f64).collect();
    let mut cm = Vec::with_capacity(300);
    let n = count(|| {
        for _ in 0..100 {
            ring.extend(&x[..64]);
            let w = ring.window(ring.end() - 64, 64).unwrap();
            wf.run_power(w);
            wf.run(ring.last(64));
            cs.noise_density(&powers, 4).unwrap();
            sdrkit_algo_dsp::stats::centred_mean(&vals64, 3, &mut cm);
        }
    });
    assert_eq!(
        n, 0,
        "ring windows, windowed FFT, centred mean, carrier spectrum"
    );

    // The fractional resampler with a frequency shift, streaming and reading a stored slice.
    let mut fr = FractionalResampler::<Complex32>::new(1e6, 250e3, 10e3, 30e3, 70.0).unwrap();
    fr.set_shift(100e3).unwrap();
    let mut out = Vec::with_capacity(4096);
    fr.process(&x, &mut out);
    fr.process_slice(&x, 300.3, 100, &mut out).unwrap();
    let n = count(|| {
        for i in 0..1000 {
            out.clear();
            fr.process(&x[..64], &mut out);
            fr.process_slice(&x, 300.3 + i as f64 * 0.01, 100, &mut out)
                .unwrap();
        }
    });
    assert_eq!(n, 0, "fractional resampler");
}

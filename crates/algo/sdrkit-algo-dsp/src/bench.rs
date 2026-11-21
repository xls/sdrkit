//! Benchmarks of the main blocks, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]). Each makes its test signal first,
//! then times the block on it; `items_per_second` counts input samples (transforms for the FFTs), and `cpu_fraction_of_realtime` is
//! for the rate in the name.

use crate::bank::{OverlapSaveBank, PolyphaseBank};
use crate::channel::{ChannelConfig, Channelizer};
use crate::cic::Cic;
use crate::decim::{ComplexDecimator, FirDecimator};
use crate::fft::{Fft, BACKENDS};
use crate::fm::{Discriminator, LagDiscriminator};
use crate::goertzel::ToneBank;
use crate::interp::{Cubic, Polyphase, Reader};
use crate::loopfilter::{LoopFilter, PhaseAcc};
use crate::nco::Nco;
use crate::resample::{FractionalResampler, Resampler};
use crate::spectrum::{cyclic_lines, Scaling, Welch};
use crate::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use crate::window::Window;
use crate::{fir, Complex32, Stereo};
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};

fn tone(n: usize, f: f64) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            Complex32::from_polar(0.5, (std::f64::consts::TAU * ((f * i as f64) % 1.0)) as f32)
        })
        .collect()
}

const BLOCK: usize = 65_536;

fn nco_mix(seconds: f64) -> BenchResult {
    let mut x = tone(BLOCK, 0.01);
    let mut n = Nco::new(2.4e6, 123_456.0);
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        n.mix_in_place(&mut x)
    })
}

fn decimate(seconds: f64) -> BenchResult {
    // The FM channel filter at 2.4 Msps: 114 taps, one output in five.
    let taps = fir::lowpass(2.4e6, 110e3, 190e3, 62.0).unwrap();
    let mut d = FirDecimator::<Complex32>::new(&taps, 5);
    let x = tone(BLOCK, 0.01);
    let mut out = Vec::with_capacity(BLOCK);
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        out.clear();
        d.process(&x, &mut out);
    })
}

fn channelizer(seconds: f64) -> BenchResult {
    let mut ch = Channelizer::new(ChannelConfig::new(2.4e6, 300e3, 20e3, 48e3)).expect("fits");
    let x = tone(BLOCK, 0.13);
    let mut out = Vec::with_capacity(BLOCK);
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        out.clear();
        ch.process(&x, &mut out);
    })
}

fn fft(seconds: f64, plain: bool) -> BenchResult {
    let n = 4096;
    let mut f = if plain {
        Fft::with_backend(n, false, BACKENDS.plain()).expect("the plain FFT plans every size")
    } else {
        Fft::forward(n)
    };
    let mut x = tone(n, 0.1);
    let reps = 64;
    // Real time: 30 transforms a second.
    measure(seconds, reps, reps as f64 / 30.0, || {
        for _ in 0..reps {
            f.process(&mut x);
        }
    })
}

fn spectrum(seconds: f64) -> BenchResult {
    let mut cfg = SpectrumConfig::new(65_536, 2.4e6);
    cfg.frames_per_second = 30.0;
    let mut s = SpectrumAnalyzer::new(cfg).expect("fits");
    let x = tone(240_000, 0.1);
    measure(seconds, x.len(), 0.1, || {
        s.push(&x);
    })
}

fn resample(seconds: f64) -> BenchResult {
    let mut r = Resampler::<Stereo>::new(44_100.0, 48_000.0).expect("fits");
    let x: Vec<Stereo> = (0..44_100)
        .map(|i| Stereo::new((i as f32 * 0.01).sin(), 0.0))
        .collect();
    let mut out = Vec::with_capacity(50_000);
    measure(seconds, x.len(), 1.0, || {
        out.clear();
        r.process(&x, &mut out);
    })
}

fn resample_shifted(seconds: f64) -> BenchResult {
    // 7.68 Msps to 1.92 Msps, a 1.3 MHz wide channel at +2 MHz moved to DC: flat to 325 kHz, 70 dB down from 975 kHz.
    let mut r =
        FractionalResampler::<Complex32>::new(7.68e6, 1.92e6, 325e3, 975e3, 70.0).expect("fits");
    r.set_shift(2e6).expect("finite");
    let x = tone(76_800, 0.27);
    let mut out = Vec::with_capacity(20_000);
    measure(seconds, x.len(), 0.01, || {
        out.clear();
        r.process(&x, &mut out);
    })
}

fn discriminator(seconds: f64) -> BenchResult {
    let mut d = Discriminator::new(500e3, 75e3).expect("constant parameters");
    let x = tone(BLOCK, 0.03);
    let mut out = vec![0.0; BLOCK];
    measure(seconds, BLOCK, BLOCK as f64 / 500e3, || {
        d.process_into(&x, &mut out)
    })
}

fn lag_discriminator(seconds: f64) -> BenchResult {
    let mut d = LagDiscriminator::new(4e6, 1.0, 4).expect("constant parameters");
    let x = tone(BLOCK, 0.03);
    let mut out = vec![0.0; BLOCK];
    measure(seconds, BLOCK, BLOCK as f64 / 4e6, || {
        d.process_into(&x, &mut out)
            .expect("the buffers have the same length")
    })
}

fn cic(seconds: f64) -> BenchResult {
    // Order 5 by 50: 2.4 Msps to 48 ksps.
    let mut c = Cic::<Complex32>::new(5, 50).expect("fits");
    let x = tone(BLOCK, 0.001);
    let mut out = Vec::with_capacity(BLOCK);
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        out.clear();
        c.process(&x, &mut out);
    })
}

fn complex_decimator(seconds: f64) -> BenchResult {
    // An SSB band-pass (300..2700 Hz) at 192 ksps, decimating by 4: complex taps, kept outputs only.
    let taps = fir::bandpass_complex(192e3, 300.0, 2700.0, 2000.0, 60.0).unwrap();
    let mut d = ComplexDecimator::new(&taps, 4).expect("valid");
    let x = tone(BLOCK, 0.01);
    let mut out = Vec::with_capacity(BLOCK);
    measure(seconds, BLOCK, BLOCK as f64 / 192e3, || {
        out.clear();
        d.process(&x, &mut out);
    })
}

fn decimator_push(seconds: f64) -> BenchResult {
    // The per-sample form of the FM channel decimator (114 taps, by 5).
    let taps = fir::lowpass(2.4e6, 110e3, 190e3, 62.0).unwrap();
    let mut d = FirDecimator::<Complex32>::new(&taps, 5);
    let x = tone(BLOCK, 0.01);
    let mut acc = Complex32::default();
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        for &v in &x {
            if let Some(y) = d.push(v) {
                acc += y;
            }
        }
        std::hint::black_box(acc);
    })
}

fn interp_read(seconds: f64, poly: bool) -> BenchResult {
    // Symbol reads from a stored burst at 4.37 samples per symbol with a clock error (items: reads).
    let x = tone(BLOCK, 0.03);
    let n = (BLOCK as f64 / 4.37) as usize - 20;
    let mut out = vec![Complex32::default(); n];
    let p = Reader::new(Polyphase::<32, 64>::design(0.85, 60.0).expect("valid"));
    let c = Reader::new(Cubic);
    measure(seconds, n, n as f64 / 1e6, || {
        if poly {
            p.read(&x, 20.0, 4.37, 50.0, &mut out);
        } else {
            c.read(&x, 20.0, 4.37, 50.0, &mut out);
        }
    })
}

fn tone_bank(seconds: f64) -> BenchResult {
    // The 16 SELCAL tones (312.6 to 1479.1 Hz) over 100 ms blocks at 8 ksps.
    let freqs = [
        312.6, 346.7, 384.6, 426.6, 473.2, 524.8, 582.1, 645.7, 716.1, 794.3, 881.0, 977.2, 1083.9,
        1202.3, 1333.5, 1479.1,
    ];
    let mut b = ToneBank::with_rate(8000.0, &freqs, 800).expect("valid");
    let x: Vec<f32> = tone(BLOCK, 0.05).iter().map(|v| v.re).collect();
    let mut blocks = 0u64;
    measure(seconds, BLOCK, BLOCK as f64 / 8000.0, || {
        b.process(&x, |_, _| blocks += 1);
    })
}

fn polyphase_bank(seconds: f64) -> BenchResult {
    // 64 bands of 37.5 kHz over 2.4 Msps, 2x oversampled, 8 taps per band, reading 8 bands.
    let read: Vec<usize> = (0..8).map(|i| i * 8).collect();
    let mut b = PolyphaseBank::new(64, 8, 2, 60.0, &read).expect("valid");
    let x = tone(BLOCK, 0.13);
    let mut out: Vec<Vec<Complex32>> = (0..8).map(|_| Vec::with_capacity(BLOCK)).collect();
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        out.iter_mut().for_each(|o| o.clear());
        b.process(&x, &mut out).expect("sizes");
    })
}

fn overlap_save_bank(seconds: f64) -> BenchResult {
    // Eight 125 kHz LoRa-style channels over 2.4 Msps, FFT 4096.
    let mut b = OverlapSaveBank::new(2.4e6, 4096).expect("valid");
    for k in 0..8 {
        b.add_channel(-900e3 + k as f64 * 250e3, 62.5e3, 100e3, 60.0)
            .expect("fits");
    }
    let x = tone(BLOCK, 0.13);
    let mut out: Vec<Vec<Complex32>> = (0..8).map(|_| Vec::with_capacity(BLOCK)).collect();
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        out.iter_mut().for_each(|o| o.clear());
        b.process(&x, &mut out).expect("sizes");
    })
}

fn welch(seconds: f64) -> BenchResult {
    // One second at 250 ksps, 4096-point Hann frames with half overlap.
    let x = tone(250_000, 0.1);
    let mut w = Welch::new(4096, Window::Hann, 0.5, Scaling::Power).expect("valid");
    let mut out = vec![0.0; 4096];
    measure(seconds, x.len(), 1.0, || {
        w.compute(&x, &mut out).expect("long enough");
    })
}

fn carrier_loop(seconds: f64) -> BenchResult {
    // A per-sample PLL: derotate, detect (imaginary part), loop filter, steer, at 1 Msps.
    let x = tone(BLOCK, 0.001);
    let mut lf = LoopFilter::new(0.001, 0.707).expect("valid");
    let mut acc = PhaseAcc::new(0.0);
    measure(seconds, BLOCK, BLOCK as f64 / 1e6, || {
        for &v in &x {
            let y = acc.derotate(v);
            acc.advance();
            acc.steer(lf.update(y.im as f64 * 0.1));
        }
        std::hint::black_box(acc.phase());
    })
}

/// The channeliser with a stop band at half the output rate: 2 Msps to exactly 153.6 kHz.
fn channelizer_stop(seconds: f64) -> BenchResult {
    let mut cfg = ChannelConfig::new(2e6, 100e3, 80e3, 153.6e3);
    cfg.exact_rate = true;
    let mut ch = Channelizer::with_stop(cfg, 76.8e3).expect("fits");
    let x = tone(BLOCK, 0.05);
    let mut out = Vec::with_capacity(BLOCK);
    measure(seconds, BLOCK, BLOCK as f64 / 2e6, || {
        out.clear();
        ch.process(&x, &mut out);
    })
}

/// A sliding complex window sum of 512 values (a cyclic prefix correlation's running sum) at 9.14 Msps.
fn window_sum(seconds: f64) -> BenchResult {
    let mut w = crate::stats::WindowSum::<Complex32>::new(512).expect("valid length");
    let x = tone(BLOCK, 0.01);
    let mut acc = num_complex::Complex64::default();
    measure(seconds, BLOCK, BLOCK as f64 / (64e6 / 7.0), || {
        for &v in &x {
            acc += w.push(v);
        }
    })
}

/// A sample ring of 4096 complex samples at 2.4 Msps, with a 256-sample window read every 256 samples.
fn sample_ring(seconds: f64) -> BenchResult {
    let mut r = crate::ring::SampleRing::<Complex32>::new(4096).expect("valid capacity");
    let x = tone(BLOCK, 0.01);
    let mut acc = Complex32::default();
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        for c in x.chunks(256) {
            r.extend(c);
            acc += r.last(256)[0];
        }
    })
}

/// A 1024-point FFT of a 256-sample Hann window, zero-padded: one transform per 256 samples at 48 ksps.
/// The symbol-rate line search of an acquisition: one call over 4 transforms of 65536 points at 16 MS/s.
fn cyclic_line_search(seconds: f64) -> BenchResult {
    let n = 4 * 65_536;
    let x = tone(n, 0.01);
    measure(seconds, n, n as f64 / 16e6, || {
        std::hint::black_box(cyclic_lines(&x, 16e6, 65_536, 0.3e6, 15e6, 30.0));
    })
}

fn windowed_fft(seconds: f64) -> BenchResult {
    let mut f = crate::fft::WindowedFft::new(1024, 256, Window::Hann).expect("fits");
    let x = tone(256, 0.1);
    let reps = 64;
    measure(seconds, reps, reps as f64 * 256.0 / 48_000.0, || {
        for _ in 0..reps {
            f.run_power(&x);
        }
    })
}

fn dot_complex(seconds: f64, taps: usize, plain: bool) -> BenchResult {
    // One dot product of real taps with complex samples at every sample of a block at 2.4 Msps: the kernel of the selected
    // FIR backend, or the plain dot product.
    let t: Vec<f32> = (0..taps).map(|i| (i as f32 * 0.37).sin()).collect();
    let kernel = fir::FirKernel::<Complex32>::new(&t);
    let x = tone(BLOCK + taps, 0.01);
    let mut acc = Complex32::default();
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        if plain {
            for i in 0..BLOCK {
                acc += <Complex32 as crate::Sample>::dot(&t, &x[i..i + taps]);
            }
        } else {
            for i in 0..BLOCK {
                acc += kernel.one(&x[i..i + taps]);
            }
        }
        std::hint::black_box(acc);
    })
}

fn fir_complex(seconds: f64) -> BenchResult {
    // A 64-tap filter on complex samples at 2.4 Msps, every output.
    let taps: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).sin() / 64.0).collect();
    let mut f = fir::Fir::<Complex32>::new(&taps);
    let x = tone(BLOCK, 0.01);
    let mut out = vec![Complex32::default(); BLOCK];
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        f.process(&x, &mut out);
    })
}

fn decimate_complex_by_4(seconds: f64) -> BenchResult {
    // A 64-tap filter on complex samples at 2.4 Msps, one output in four.
    let taps: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).sin() / 64.0).collect();
    let mut d = FirDecimator::<Complex32>::new(&taps, 4);
    let x = tone(BLOCK, 0.01);
    let mut out = Vec::with_capacity(BLOCK);
    measure(seconds, BLOCK, BLOCK as f64 / 2.4e6, || {
        out.clear();
        d.process(&x, &mut out);
    })
}

fn resample_complex(seconds: f64, in_rate: f64, out_rate: f64) -> BenchResult {
    // A 6 MHz television channel between 6.5 Msps and 21.52 Msps: flat to 3 MHz, 80 dB down from 3.25 MHz, no shift.
    let mut r = FractionalResampler::<Complex32>::new(in_rate, out_rate, 3.0e6, 3.25e6, 80.0)
        .expect("fits");
    let n = (in_rate * 0.01) as usize;
    let x = tone(n, 0.07);
    let mut out = Vec::with_capacity((out_rate * 0.011) as usize);
    measure(seconds, n, 0.01, || {
        out.clear();
        r.process(&x, &mut out);
    })
}

/// The benchmarks of this crate.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "dsp.dot_complex_32_taps",
            run: |s| dot_complex(s, 32, false),
        },
        Benchmark {
            name: "dsp.dot_complex_32_taps_plain",
            run: |s| dot_complex(s, 32, true),
        },
        Benchmark {
            name: "dsp.dot_complex_64_taps",
            run: |s| dot_complex(s, 64, false),
        },
        Benchmark {
            name: "dsp.dot_complex_64_taps_plain",
            run: |s| dot_complex(s, 64, true),
        },
        Benchmark {
            name: "dsp.dot_complex_128_taps",
            run: |s| dot_complex(s, 128, false),
        },
        Benchmark {
            name: "dsp.dot_complex_128_taps_plain",
            run: |s| dot_complex(s, 128, true),
        },
        Benchmark {
            name: "dsp.dot_complex_512_taps",
            run: |s| dot_complex(s, 512, false),
        },
        Benchmark {
            name: "dsp.dot_complex_512_taps_plain",
            run: |s| dot_complex(s, 512, true),
        },
        Benchmark {
            name: "dsp.fir_complex_64_taps_2400k",
            run: fir_complex,
        },
        Benchmark {
            name: "dsp.decimate_complex_by_4_64_taps_2400k",
            run: decimate_complex_by_4,
        },
        Benchmark {
            name: "dsp.resample_complex_21m52_to_6m5",
            run: |s| resample_complex(s, 21.52e6, 6.5e6),
        },
        Benchmark {
            name: "dsp.resample_complex_6m5_to_21m52",
            run: |s| resample_complex(s, 6.5e6, 21.52e6),
        },
        Benchmark {
            name: "dsp.sample_ring_window_256_2m4",
            run: sample_ring,
        },
        Benchmark {
            name: "dsp.cyclic_lines_4_of_65536_16m",
            run: cyclic_line_search,
        },
        Benchmark {
            name: "dsp.windowed_fft_256_in_1024_48k",
            run: windowed_fft,
        },
        Benchmark {
            name: "dsp.channelizer_stop_2m_to_153k6",
            run: channelizer_stop,
        },
        Benchmark {
            name: "dsp.window_sum_complex_512_9m14",
            run: window_sum,
        },
        Benchmark {
            name: "dsp.cic5_by_50_2400k",
            run: cic,
        },
        Benchmark {
            name: "dsp.complex_decimator_ssb_by_4_192k",
            run: complex_decimator,
        },
        Benchmark {
            name: "dsp.decimator_push_by_5_114_taps_2400k",
            run: decimator_push,
        },
        Benchmark {
            name: "dsp.interp_polyphase32_reads_1m",
            run: |s| interp_read(s, true),
        },
        Benchmark {
            name: "dsp.interp_cubic_reads_1m",
            run: |s| interp_read(s, false),
        },
        Benchmark {
            name: "dsp.tone_bank_16_selcal_8k",
            run: tone_bank,
        },
        Benchmark {
            name: "dsp.polyphase_bank_64x2_2400k",
            run: polyphase_bank,
        },
        Benchmark {
            name: "dsp.overlap_save_bank_8ch_2400k",
            run: overlap_save_bank,
        },
        Benchmark {
            name: "dsp.welch_4096_250k",
            run: welch,
        },
        Benchmark {
            name: "dsp.carrier_loop_1m",
            run: carrier_loop,
        },
        Benchmark {
            name: "dsp.nco_mix_2400k",
            run: nco_mix,
        },
        Benchmark {
            name: "dsp.decimate_by_5_114_taps_2400k",
            run: decimate,
        },
        Benchmark {
            name: "dsp.channelizer_2400k_to_48k",
            run: channelizer,
        },
        Benchmark {
            name: "dsp.fft_4096_30fps",
            run: |s| fft(s, false),
        },
        Benchmark {
            name: "dsp.fft_4096_30fps_plain",
            run: |s| fft(s, true),
        },
        Benchmark {
            name: "dsp.spectrum_65536_30fps_2400k",
            run: spectrum,
        },
        Benchmark {
            name: "dsp.resample_stereo_44k1_to_48k",
            run: resample,
        },
        Benchmark {
            name: "dsp.resample_shifted_7680k_to_1920k",
            run: resample_shifted,
        },
        Benchmark {
            name: "dsp.fm_discriminator_500k",
            run: discriminator,
        },
        Benchmark {
            name: "dsp.fm_lag_discriminator_4m",
            run: lag_discriminator,
        },
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_benchmark_runs() {
        for b in super::benchmarks() {
            let r = (b.run)(0.0);
            assert!(
                r.items_per_second > 0.0 && r.cpu_fraction_of_realtime > 0.0,
                "{}",
                b.name
            );
        }
    }
}

//! Benchmarks of the hot paths, for `sdrkit_benchmark` on the device (see `sdrkit_core::bench`). Each makes its test signal first,
//! then times the path on it; `items_per_second` counts input samples (symbols for symbol-rate paths), and
//! `cpu_fraction_of_realtime` is for the rate in the name.

use crate::carrier::{CarrierConfig, CarrierSync, Costas, DecisionDirected, ToneLoop};
use crate::freq::{kay, PowerLine};
use crate::level::M2m4;
use crate::squelch::{Gate, QuietingSquelch};
use crate::psk::{PskCarrier, PskDemodConfig, PskDemodulator, PskOrder, PskPulse};
use crate::timing::{
    Gardner, MatchedBank, MatchedInterp, RunSymbolSync, SymbolSync, SymbolSyncConfig,
};
use crate::{Complex32, Constellation, Demap};
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_dsp::interp::Polyphase;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};

const N: usize = 65_536;

/// Pseudo-random unit-power samples on QPSK-like points with a slow turn (cheap, repeatable, no library trigonometry).
fn signal(n: usize) -> Vec<Complex32> {
    let mut s = 0x9E37_79B9u32;
    let r = Complex32::from_polar(1.0, 0.001);
    let mut ph = Complex32::new(1.0, 0.0);
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ph *= r;
            let a = if s & 0x8000 != 0 { 0.707 } else { -0.707 };
            let b = if s & 0x4000 != 0 { 0.707 } else { -0.707 };
            Complex32::new(a, b) * ph + Complex32::new((s >> 22) as f32 * 1e-3 - 0.5e-3, 0.0)
        })
        .collect()
}

/// Pseudo-random complex noise of about unit power, each part a sum of four uniform values (repeatable, no library calls).
fn noise(n: usize) -> Vec<Complex32> {
    let mut s = 0x2545_F491u32;
    let mut part = || {
        (0..4)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .sum::<f32>()
            * 1.2247
    };
    (0..n).map(|_| Complex32::new(part(), part())).collect()
}

fn ok<T, E>(r: Result<T, E>) -> T {
    // The settings below are fixed and valid.
    r.unwrap_or_else(|_| unreachable!("benchmark settings are valid"))
}

/// 64-QAM max-log demapping at the DVB-T 8K cell rate (6048 cells per 1.12 ms symbol: 5.4 M cells/s).
fn demap_qam64(seconds: f64) -> BenchResult {
    let c = ok(Constellation::new(ok(PointSet::qam(64, QamMap::DVB))));
    let x = signal(8192);
    let mut out = vec![0f32; 8192 * 6];
    measure(seconds, x.len(), x.len() as f64 / 5.4e6, || {
        let _ = c.demap_block(&x, 0.05, Demap::MaxLog, &mut out);
    })
}

/// 32APSK exact demapping (the generic path) at a DVB-S2 rate of 10 MBd.
fn demap_apsk32(seconds: f64) -> BenchResult {
    let c = ok(Constellation::new(ok(PointSet::dvbs2_32apsk(2.84, 5.27))));
    let x = signal(8192);
    let mut out = vec![0f32; 8192 * 5];
    measure(seconds, x.len(), x.len() as f64 / 10e6, || {
        let _ = c.demap_block(&x, 0.05, Demap::Exact, &mut out);
    })
}

/// 32APSK max-log demapping at 10 MBd.
fn demap_apsk32_maxlog(seconds: f64) -> BenchResult {
    let c = ok(Constellation::new(ok(PointSet::dvbs2_32apsk(2.84, 5.27))));
    let x = signal(8192);
    let mut out = vec![0f32; 8192 * 5];
    measure(seconds, x.len(), x.len() as f64 / 10e6, || {
        let _ = c.demap_block(&x, 0.05, Demap::MaxLog, &mut out);
    })
}

/// Gardner timing with a 16-tap polyphase interpolator at 2 samples per symbol, 10 MBd (20 MS/s).
fn symbol_sync_2sps(seconds: f64) -> BenchResult {
    let mut cfg = SymbolSyncConfig::new(2.0);
    cfg.detector_gain = Some(1.0);
    let mut s = ok(SymbolSync::new(
        Gardner,
        ok(Polyphase::<16, 64>::design(0.8, 60.0)),
        cfg,
    ));
    let x = signal(N);
    let mut out = Vec::with_capacity(N);
    measure(seconds, N, N as f64 / 20e6, || {
        out.clear();
        s.process(&x, &mut out);
    })
}

/// Gardner timing with the 32-tap matched RRC interpolator at 4 samples per symbol, 1 MBd (4 MS/s).
fn symbol_sync_matched_4sps(seconds: f64) -> BenchResult {
    let mut cfg = SymbolSyncConfig::new(4.0);
    cfg.detector_gain = Some(1.0);
    let mi = ok(MatchedInterp::<32, 64>::new(Shape::Rrc { beta: 0.35 }, 4.0));
    let mut s = ok(SymbolSync::new(Gardner, mi, cfg));
    let x = signal(N);
    let mut out = Vec::with_capacity(N);
    measure(seconds, N, N as f64 / 4e6, || {
        out.clear();
        s.process(&x, &mut out);
    })
}

/// Gardner timing with the 64-tap matched RRC interpolator of 128 delays at 2.3 samples per symbol, two outputs per symbol,
/// 6.9 MBd (16 MS/s, a DVB-C receiver).
fn symbol_sync_matched_64_taps(seconds: f64) -> BenchResult {
    let mut cfg = SymbolSyncConfig::new(2.3);
    cfg.detector_gain = Some(1.0);
    cfg.two_per_symbol = true;
    let mi = ok(MatchedInterp::<64, 128>::new(
        Shape::Rrc { beta: 0.15 },
        2.3,
    ));
    let mut s = ok(SymbolSync::new(Gardner, mi, cfg));
    let x = signal(N);
    let mut out = Vec::with_capacity(N);
    measure(seconds, N, N as f64 / 16e6, || {
        out.clear();
        s.process(&x, &mut out);
    })
}

/// OQPSK Gardner timing with a cubic interpolator at 4 samples per symbol and a decision-directed carrier loop feeding its phase
/// back before the detector, 72 kBd (288 kS/s, a LEO weather satellite).
fn symbol_sync_oqpsk_carrier(seconds: f64) -> BenchResult {
    let mut cfg = SymbolSyncConfig::new(4.0);
    cfg.oqpsk = true;
    cfg.detector_gain = Some(1.0);
    let mut s = ok(SymbolSync::new(
        Gardner,
        sdrkit_algo_dsp::interp::Cubic,
        cfg,
    ));
    let c = ok(Constellation::new(PointSet::qpsk()));
    let mut cs = ok(CarrierSync::new(
        DecisionDirected::new(c),
        CarrierConfig::new(0.01),
    ));
    let x = signal(N);
    let mut n = 0usize;
    measure(seconds, N, N as f64 / 288e3, || {
        s.process_each(&x, |y| {
            n += 1;
            cs.push(y);
            Some(cs.phase())
        });
    })
}

/// The band-power search over a 1024-bin spectrum (a coarse carrier search), ten a second.
fn band_search(seconds: f64) -> BenchResult {
    let mut b = ok(crate::freq::BandSearch::new(1024, 5e3, 46.875));
    let p: Vec<f32> = signal(1024)
        .iter()
        .map(|z| z.norm_sqr() + z.re.abs())
        .collect();
    measure(seconds, 1, 0.1, || {
        let _ = b.find(&p, -18e3, 18e3);
    })
}

/// A Costas QPSK loop at 10 MBd.
fn costas(seconds: f64) -> BenchResult {
    let mut cs = ok(CarrierSync::new(Costas::<4>, CarrierConfig::new(0.01)));
    let x = signal(N);
    let mut out = vec![Complex32::default(); N];
    measure(seconds, N, N as f64 / 10e6, || {
        let _ = cs.process(&x, &mut out);
    })
}

/// Hard decisions of DVB-C QAM of `order` points at 6.9 MBd.
fn decide_qam(seconds: f64, order: usize) -> BenchResult {
    let c = ok(Constellation::new(ok(PointSet::dvb_c(order))));
    let x = signal(8192);
    let mut sum = 0u32;
    measure(seconds, x.len(), x.len() as f64 / 6.9e6, || {
        for &z in &x {
            sum = sum.wrapping_add(c.decide(z) as u32);
        }
        std::hint::black_box(sum);
    })
}

/// Making the DVB-C 64-QAM constellation from its point set, with its constant modulus radius, and dropping it: the cost for a
/// caller that keeps no constellation. The real-time figure is for one per symbol at 6.9 MBd.
fn build_qam64(seconds: f64) -> BenchResult {
    measure(seconds, 256, 256.0 / 6.9e6, || {
        for _ in 0..256 {
            let c = ok(Constellation::new(ok(PointSet::dvb_c(64))));
            std::hint::black_box(crate::equalizer::cma_radius(&c));
        }
    })
}

/// Making and dropping the DVB-C 64-QAM point set alone, one per symbol at 6.9 MBd.
fn build_points_qam64(seconds: f64) -> BenchResult {
    measure(seconds, 256, 256.0 / 6.9e6, || {
        for _ in 0..256 {
            std::hint::black_box(ok(PointSet::dvb_c(64)));
        }
    })
}

/// A decision-directed 64-QAM loop at 6.9 MBd (DVB-C).
fn dd_qam64(seconds: f64) -> BenchResult {
    let c = ok(Constellation::new(ok(PointSet::qam(64, QamMap::DVB))));
    let mut cs = ok(CarrierSync::new(
        DecisionDirected::new(c),
        CarrierConfig::new(0.01),
    ));
    let x = signal(N);
    let mut out = vec![Complex32::default(); N];
    measure(seconds, N, N as f64 / 6.9e6, || {
        let _ = cs.process(&x, &mut out);
    })
}

/// The 19 kHz pilot loop on an FM multiplex at 192 kHz.
fn tone_loop(seconds: f64) -> BenchResult {
    let mut t = ok(ToneLoop::new(192e3, 19e3, 20.0, 10.0, 2e3));
    let x: Vec<f32> = signal(N).iter().map(|z| z.re).collect();
    measure(seconds, N, N as f64 / 192e3, || t.process_real(&x))
}

/// A 4096-point 4th-power line search, ten a second.
fn power_line(seconds: f64) -> BenchResult {
    let mut p = ok(PowerLine::new(4, 4096));
    let x = signal(4096);
    measure(seconds, x.len(), 0.1, || {
        let _ = p.estimate(&x);
    })
}

/// Kay's estimator over 1024-sample preambles, a thousand a second.
fn kay_1024(seconds: f64) -> BenchResult {
    let x = signal(1024);
    measure(seconds, x.len(), 1e-3, || {
        let _ = std::hint::black_box(kay(std::hint::black_box(&x)));
    })
}

/// M2M4 SNR over symbols at 10 MBd.
fn m2m4(seconds: f64) -> BenchResult {
    let mut m = M2m4::psk();
    let x = signal(N);
    measure(seconds, N, N as f64 / 10e6, || {
        m.push_slice(std::hint::black_box(&x));
        let _ = std::hint::black_box(m.estimate());
    })
}

/// One satellite's code search: 16 periods of a 4092-sample code (1 ms at 4.092 MHz) in coherent pairs, over +-5 kHz in quarter
/// bins, with drift compensation. Real time is the 16 ms of signal searched.
fn code_search(seconds: f64) -> BenchResult {
    use crate::dsss::{CodeSearch, Integration, SearchConfig, SearchGrid, SearchJob};
    let n = 4092;
    let x = signal(16 * n);
    let code: Vec<f32> = x[..n].iter().map(|z| z.re.signum()).collect();
    let mut cs = ok(CodeSearch::new(n));
    let rep = ok(cs.replica(&code));
    let grid = ok(SearchGrid::window(n, 0.0, 5e3 / 4.092e6, 4));
    let mut cfg = SearchConfig::new(Integration {
        periods: 16,
        coherent: 2,
    });
    cfg.code_doppler = 4.092e6 / 1575.42e6;
    let mut job = SearchJob::new();
    measure(seconds, 16 * n, 16e-3, || {
        ok(cs.start(&mut job, &x, &grid, &cfg));
        let mut budget = f64::INFINITY;
        let _ = std::hint::black_box(ok(cs.step(&mut job, &rep, &mut budget)));
    })
}

/// The FM noise-quieting squelch on discriminator output at 24 kHz.
fn quieting(seconds: f64) -> BenchResult {
    let mut q = ok(QuietingSquelch::new(
        24e3,
        240,
        6e3,
        ok(Gate::new(6.0, 3.0, 2, 10)),
    ));
    let x: Vec<f32> = signal(N).iter().map(|z| z.im).collect();
    measure(seconds, N, N as f64 / 24e3, || {
        q.process(&x);
    })
}

/// The Morse key detector on an envelope at 1 kHz (the CW skimmer runs one per carrier).
fn key_detector(seconds: f64) -> BenchResult {
    let mut k = ok(crate::key::KeyDetector::new(1000.0));
    let x: Vec<f32> = signal(N)
        .iter()
        .map(|z| z.norm() * if z.re > 0.0 { 1.0 } else { 0.1 })
        .collect();
    measure(seconds, N, N as f64 / 1e3, || {
        for &e in &x {
            let _ = std::hint::black_box(k.step(e, 0.06));
        }
    })
}

/// A PSK signal of `n` samples at `sps` samples per symbol: pseudo-random points of `order` through root raised cosine
/// pulses of roll-off 0.5, at unit power.
fn psk_signal(order: PskOrder, sps: f64, n: usize) -> Vec<Complex32> {
    const SPAN: i64 = 6;
    let pulse = Shape::Rrc { beta: 0.5 };
    let mut s = 0x9E37_79B9u32;
    let symbols: Vec<Complex32> = (0..(n as f64 / sps) as usize + 2 * SPAN as usize + 2)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            order.point((s >> 16) as usize)
        })
        .collect();
    let offset = order == PskOrder::Oqpsk;
    (0..n)
        .map(|i| {
            let t = i as f64 / sps;
            let k0 = t.floor() as i64;
            let mut y = Complex32::new(0.0, 0.0);
            for k in k0 - SPAN..=k0 + SPAN {
                let a = symbols[(k + SPAN) as usize];
                let h = pulse.value(t - k as f64) as f32;
                if offset {
                    y += Complex32::new(a.re * h, a.im * pulse.value(t - k as f64 - 0.5) as f32);
                } else {
                    y += a * h;
                }
            }
            y * (1.0 / sps.sqrt() as f32)
        })
        .collect()
}

/// The settings of a decision-directed PSK receiver of a fast link: a matched filter over 8 symbols, timing at 0.004
/// per symbol, a carrier loop at 0.004 per symbol without a frequency loop.
fn psk_config(order: PskOrder, sps: f64) -> PskDemodConfig {
    let mut c = PskDemodConfig::new(order, sps);
    c.pulse = PskPulse::Rrc(0.5);
    c.mf_span = 8.0;
    c.timing_bn_t = 0.004;
    c.timing_gears = None;
    c.carrier = if order == PskOrder::Psk8 {
        PskCarrier::Costas
    } else {
        PskCarrier::DecisionDirected
    };
    c.carrier_bn_t = 0.004;
    c.fll_bn_t = None;
    c.handover = None;
    c.hold_symbols = 0;
    c
}

/// The PSK demodulator on `order` at `symbol_rate` symbols and `rate` samples a second.
fn psk_demod(seconds: f64, order: PskOrder, symbol_rate: f64, rate: f64) -> BenchResult {
    let x = psk_signal(order, rate / symbol_rate, 4 * N);
    let mut d = ok(PskDemodulator::new(psk_config(order, rate / symbol_rate)));
    let mut out = Vec::with_capacity(x.len());
    measure(seconds, x.len(), x.len() as f64 / rate, || {
        out.clear();
        d.process(&x, &mut out);
    })
}

/// The mixer of the PSK demodulator alone, in pieces of 6 samples (one run of 4 symbols at 1.46 samples per symbol).
fn psk_mixer(seconds: f64) -> BenchResult {
    let mut x = signal(4 * N);
    let mut nco = sdrkit_algo_dsp::nco::Nco::new(1.0, 1e-3);
    measure(seconds, x.len(), x.len() as f64 / 45e6, || {
        for piece in x.chunks_mut(6) {
            nco.mix_in_place(piece);
        }
    })
}

/// The timing loop of the PSK demodulator alone: a bank of matched filters of 16 taps and a clock step per 4 symbols,
/// BPSK at 30.72 Msym/s and 45 Msps, the input in pieces of 6 samples.
fn psk_timing(seconds: f64) -> BenchResult {
    let sps = 45.0 / 30.72;
    let x = psk_signal(PskOrder::Bpsk, sps, 4 * N);
    let mut c = SymbolSyncConfig::new(sps);
    c.bn_t = 0.004;
    c.pulse = Shape::Rc { beta: 0.5 };
    let bank = ok(MatchedBank::<16, 128>::new(Shape::Rrc { beta: 0.5 }, sps));
    let mut t = ok(RunSymbolSync::<Complex32, _>::new(bank, c, 4));
    let mut out = Vec::with_capacity(x.len());
    measure(seconds, x.len(), x.len() as f64 / 45e6, || {
        out.clear();
        for piece in x.chunks(6) {
            t.process(piece, &mut out);
        }
    })
}

/// The mixer of the PSK demodulator alone, in the pieces of 256 samples that the demodulator mixes.
fn psk_mixer_pieces(seconds: f64) -> BenchResult {
    let mut x = signal(4 * N);
    let mut nco = sdrkit_algo_dsp::nco::Nco::new(1.0, 1e-3);
    measure(seconds, x.len(), x.len() as f64 / 45e6, || {
        for piece in x.chunks_mut(256) {
            nco.mix_in_place(piece);
        }
    })
}

/// The timing loop as the PSK demodulator drives it on a low-orbit link: BPSK at 30.72 Msym/s and 45 Msps, a bank of
/// 16 taps, gears 0.01 then 0.0025 per symbol, a run at a time from pieces of 256 samples.
fn psk_timing_runs(seconds: f64) -> BenchResult {
    let sps = 45.0 / 30.72;
    let x = psk_signal(PskOrder::Bpsk, sps, 4 * N);
    let mut c = SymbolSyncConfig::new(sps);
    c.bn_t = 0.01;
    c.gears = Some(crate::lock::Gears {
        pull_in_bn_t: 0.01,
        track_bn_t: 0.0025,
    });
    c.max_ppm = 5000.0;
    c.pulse = Shape::Rc { beta: 0.5 };
    let bank = ok(MatchedBank::<16, 128>::new(Shape::Rrc { beta: 0.5 }, sps));
    let mut t = ok(RunSymbolSync::<Complex32, _>::new(bank, c, 8));
    let mut out = Vec::with_capacity(8);
    measure(seconds, x.len(), x.len() as f64 / 45e6, || {
        for piece in x.chunks(256) {
            let mut j = 0;
            loop {
                out.clear();
                let (m, made) = t.process_run(&piece[j..], &mut out);
                j += m;
                if !made {
                    break;
                }
            }
        }
    })
}

/// The timing loop while it searches, in runs of one symbol: a bank of 64 taps at 1.92 samples per symbol on wideband
/// noise, gears 0.01 then 0.001 per symbol, pieces of 8192 samples at 8 Msps.
fn timing_search(seconds: f64) -> BenchResult {
    let sps = 1.92;
    let x = noise(4 * N);
    let mut c = SymbolSyncConfig::new(sps);
    c.gears = Some(crate::lock::Gears {
        pull_in_bn_t: 0.01,
        track_bn_t: 0.001,
    });
    c.max_ppm = 300.0;
    c.pulse = Shape::Rc { beta: 0.25 };
    let bank = ok(MatchedBank::<64, 128>::new(Shape::Rrc { beta: 0.25 }, sps));
    let mut t = ok(RunSymbolSync::<Complex32, _>::new(bank, c, 32));
    let mut out = Vec::with_capacity(8192);
    measure(seconds, x.len(), x.len() as f64 / 8e6, || {
        for piece in x.chunks(8192) {
            out.clear();
            t.process(piece, &mut out);
        }
    })
}

/// The carrier loop as the PSK demodulator drives it on that link: decisions on BPSK, 0.01 per symbol, runs of 4 symbols
/// at 30.72 Msym/s.
fn psk_carrier_runs(seconds: f64) -> BenchResult {
    let mut cfg = CarrierConfig::new(0.01);
    cfg.max_freq = 0.25 / std::f64::consts::TAU;
    let mut cs = ok(CarrierSync::new(crate::psk::Decided(PskOrder::Bpsk), cfg));
    let x: Vec<Complex32> = signal(N)
        .iter()
        .map(|z| Complex32::new(z.re * 1.414, 0.1 * z.im))
        .collect();
    let mut run = [Complex32::default(); 4];
    measure(seconds, N, N as f64 / 30.72e6, || {
        for piece in x.as_chunks::<4>().0 {
            run = *piece;
            cs.push_run(&mut run);
            std::hint::black_box(&run);
        }
    })
}

/// Every benchmark of this crate.
pub fn benchmarks() -> Vec<Benchmark> {
    let mut v = vec![
        Benchmark {
            name: "modem.psk_mixer_pieces_of_256_45m",
            run: psk_mixer_pieces,
        },
        Benchmark {
            name: "modem.psk_timing_bank16_runs_45m",
            run: psk_timing_runs,
        },
        Benchmark {
            name: "modem.timing_bank64_search_8m",
            run: timing_search,
        },
        Benchmark {
            name: "modem.psk_carrier_decided_runs_30m72",
            run: psk_carrier_runs,
        },
        Benchmark {
            name: "modem.psk_demod_bpsk_30m72_at_45m",
            run: |s| psk_demod(s, PskOrder::Bpsk, 30.72e6, 45e6),
        },
        Benchmark {
            name: "modem.psk_demod_qpsk_93m3_at_140m",
            run: |s| psk_demod(s, PskOrder::Qpsk, 93.3e6, 140e6),
        },
        Benchmark {
            name: "modem.psk_demod_oqpsk_25m_at_40m",
            run: |s| psk_demod(s, PskOrder::Oqpsk, 25e6, 40e6),
        },
        Benchmark {
            name: "modem.psk_demod_8psk_10m_at_15m",
            run: |s| psk_demod(s, PskOrder::Psk8, 10e6, 15e6),
        },
        Benchmark {
            name: "modem.psk_mixer_pieces_of_6_45m",
            run: psk_mixer,
        },
        Benchmark {
            name: "modem.psk_timing_bank16_run4_45m",
            run: psk_timing,
        },
        Benchmark {
            name: "modem.demap_qam64_maxlog_5m4",
            run: demap_qam64,
        },
        Benchmark {
            name: "modem.decide_qam64_6m9",
            run: |s| decide_qam(s, 64),
        },
        Benchmark {
            name: "modem.decide_qam256_6m9",
            run: |s| decide_qam(s, 256),
        },
        Benchmark {
            name: "modem.constellation_build_qam64_6m9",
            run: build_qam64,
        },
        Benchmark {
            name: "modem.point_set_build_qam64_6m9",
            run: build_points_qam64,
        },
        Benchmark {
            name: "modem.code_search_l1_16ms",
            run: code_search,
        },
        Benchmark {
            name: "modem.demap_apsk32_exact_10m",
            run: demap_apsk32,
        },
        Benchmark {
            name: "modem.demap_apsk32_maxlog_10m",
            run: demap_apsk32_maxlog,
        },
        Benchmark {
            name: "modem.symbol_sync_gardner_2sps_20m",
            run: symbol_sync_2sps,
        },
        Benchmark {
            name: "modem.symbol_sync_matched_rrc_4sps_4m",
            run: symbol_sync_matched_4sps,
        },
        Benchmark {
            name: "modem.symbol_sync_matched_rrc_64_taps_16m",
            run: symbol_sync_matched_64_taps,
        },
        Benchmark {
            name: "modem.symbol_sync_oqpsk_carrier_288k",
            run: symbol_sync_oqpsk_carrier,
        },
        Benchmark {
            name: "modem.key_detector_1k",
            run: key_detector,
        },
        Benchmark {
            name: "modem.band_search_1024_10hz",
            run: band_search,
        },
        Benchmark {
            name: "modem.costas_qpsk_10m",
            run: costas,
        },
        Benchmark {
            name: "modem.dd_qam64_6m9",
            run: dd_qam64,
        },
        Benchmark {
            name: "modem.tone_loop_pilot_192k",
            run: tone_loop,
        },
        Benchmark {
            name: "modem.power_line_4096_10hz",
            run: power_line,
        },
        Benchmark {
            name: "modem.kay_1024_1khz",
            run: kay_1024,
        },
        Benchmark {
            name: "modem.m2m4_10m",
            run: m2m4,
        },
        Benchmark {
            name: "modem.quieting_squelch_24k",
            run: quieting,
        },
    ];
    v.extend(crate::ofdm::benchmarks());
    v.extend(crate::dsss::benchmarks());
    v.extend(crate::equalizer::benchmarks());
    v.extend(crate::correlate::benchmarks());
    v.extend(crate::burst::benchmarks());
    v.extend(crate::fsk::benchmarks());
    v.extend(crate::gmsk::benchmarks());
    v.extend(crate::msk::benchmarks());
    v.extend(crate::ook::benchmarks());
    v.extend(crate::chirp::benchmarks());
    v.extend(crate::tone::benchmarks());
    v.extend(crate::keysurvey::benchmarks());
    v
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

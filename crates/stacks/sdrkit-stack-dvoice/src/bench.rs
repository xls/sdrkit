//! The benchmarks of the digital voice stacks: a whole DMR receiver on its test signal at 96 ksps, the symbol layer alone, the
//! decoders of one NXDN channel and one DMR rate 3/4 block, a whole TETRA DMO receiver at 240 ksps and a whole DCR receiver
//! at 250 ksps, and whole FreeDV receivers on the 700D and the 700E test signals at 96 ksps.

use crate::symbols::{Sym, SymAnswer, SymbolLayer, SymbolSink, SyncAnswer, SyncPattern};
use sdrkit_core::bench::{measure, BenchResult};
use sdrkit_core::{Benchmark, InputInfo, Output, StackConfig, Values};

/// Every benchmark of the family.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "dvoice.dmr_receiver_96k",
            run: dmr_receiver,
        },
        Benchmark {
            name: "dvoice.symbol_layer_48k",
            run: symbol_layer,
        },
        Benchmark {
            name: "dvoice.nxdn_facch1",
            run: nxdn_channel,
        },
        Benchmark {
            name: "dvoice.dmr_trellis_3_4",
            run: dmr_trellis,
        },
        Benchmark {
            name: "tetra_dmo.receiver_240k",
            run: tetra_dmo_receiver,
        },
    ]
}

/// The benchmark of the DCR stack.
pub fn dcr_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "dcr.receiver_250k",
        run: dcr_receiver,
    }]
}

/// The benchmarks of the FreeDV stack.
pub fn freedv_benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "freedv.700d_receiver_96k",
            run: |s| freedv_receiver("700D", s),
        },
        Benchmark {
            name: "freedv.700e_receiver_96k",
            run: |s| freedv_receiver("700E", s),
        },
    ]
}

/// A whole FreeDV receiver of `submode` fed 6 s of the test signal of that submode at 96 ksps. Items are input samples.
fn freedv_receiver(submode: &str, seconds: f64) -> BenchResult {
    let d = &crate::FREEDV;
    let rate = 96_000.0;
    let settings = Values::defaults(&(d.settings)());
    let mut g = (d.make_generator.expect("a generator"))(
        &sdrkit_core::GenConfig::new(rate, settings.clone()).with_submode(submode),
    )
    .expect("the generator builds");
    let mut x = vec![sdrkit_core::Complex32::new(0.0, 0.0); (6.0 * rate) as usize];
    g.generate(&mut x);
    let mut rx = (d.make_receiver)(&StackConfig::new(settings).with_submode(submode))
        .expect("the receiver builds");
    let _ = rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording());
    let mut out = Output::default();
    let n = x.len();
    measure(seconds, n, n as f64 / rate, || {
        for c in x.chunks(4800) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// A whole DCR receiver fed 2 s of the first test signal of the standard at 250 ksps, the channel where the stack's tune
/// offset puts it. Items are input samples.
fn dcr_receiver(seconds: f64) -> BenchResult {
    let d = &crate::DCR;
    let rate = d.tuning.rate_hz;
    let mut x = crate::dcr::gen::render(&crate::dcr::gen::GenOptions {
        rate_hz: rate,
        channel_hz: -d.tuning.tune_offset_hz,
        ..Default::default()
    });
    x.truncate((2.0 * rate) as usize);
    let settings = Values::defaults(&(d.settings)());
    let mut rx = (d.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    // A stack without a default frequency takes the middle of its range.
    let f = if d.tuning.default_hz > 0.0 {
        d.tuning.default_hz
    } else {
        (d.tuning.min_hz + d.tuning.max_hz) / 2.0
    };
    let _ = rx.configure(&InputInfo::new(rate, f + d.tuning.tune_offset_hz, f).recording());
    let mut out = Output::default();
    let n = x.len();
    measure(seconds, n, n as f64 / rate, || {
        for c in x.chunks(4800) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// A whole TETRA DMO receiver fed 2 s of its test signal at 240 ksps, the channel where the stack's tune offset puts it.
/// Items are input samples.
fn tetra_dmo_receiver(seconds: f64) -> BenchResult {
    let d = &crate::TETRA_DMO;
    let rate = d.tuning.rate_hz;
    let settings = Values::defaults(&(d.settings)());
    let mut g = (d.make_generator.expect("a generator"))(&sdrkit_core::GenConfig::new(
        rate,
        settings.clone(),
    ))
    .expect("the generator builds");
    let mut x = vec![sdrkit_core::Complex32::new(0.0, 0.0); (2.0 * rate) as usize];
    g.generate(&mut x);
    let mut rx = (d.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    // A stack without a default frequency takes the middle of its range.
    let f = if d.tuning.default_hz > 0.0 {
        d.tuning.default_hz
    } else {
        (d.tuning.min_hz + d.tuning.max_hz) / 2.0
    };
    let _ = rx.configure(&InputInfo::new(rate, f + d.tuning.tune_offset_hz, f).recording());
    let mut out = Output::default();
    let n = x.len();
    measure(seconds, n, n as f64 / rate, || {
        for c in x.chunks(4800) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// A whole DMR receiver fed its test signal at 96 ksps. Items are input samples.
fn dmr_receiver(seconds: f64) -> BenchResult {
    let rate = 96_000.0;
    let sig = crate::dmr::DmrGen::default_schedule(1).render(1);
    let mut x = Vec::new();
    let mut r =
        sdrkit_algo_dsp::resample::Resampler::with_quality(crate::front::RATE, rate, 0.46, 70.0)
            .expect("a valid resampler");
    r.process(&sig[..(crate::front::RATE as usize).min(sig.len())], &mut x);
    let mut rx = (crate::DMR.make_receiver)(&StackConfig::new(Values::defaults(&(crate::DMR
        .settings)(
    ))))
    .expect("the DMR receiver builds");
    let _ = rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording());
    let mut out = Output::default();
    let n = x.len();
    measure(seconds, n, n as f64 / rate, || {
        for c in x.chunks(4800) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

struct Count(u64);
impl SymbolSink for Count {
    fn sync(&mut self, _: u8, _: bool, _: &[Sym], _: u64, _: bool) -> SyncAnswer {
        SyncAnswer::Take
    }
    fn symbol(&mut self, _: Sym, _: u64) -> SymAnswer {
        self.0 += 1;
        SymAnswer::Continue
    }
}

/// The symbol layer with the nine DMR syncs on one second of a 48 kHz level stream. Items are samples.
fn symbol_layer(seconds: f64) -> BenchResult {
    let pats: Vec<SyncPattern> = crate::dmr::SYNCS
        .iter()
        .enumerate()
        .map(|(i, (_, h))| SyncPattern::from_hex_dibits(i as u8, h, 0))
        .collect();
    let mut layer = SymbolLayer::new(10, false, &pats);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    let x: Vec<f32> = (0..48_000).map(|_| rng.normal_f32() * 600.0).collect();
    let mut c = Count(0);
    measure(seconds, x.len(), 1.0, || layer.process(&x, &mut c))
}

/// The FACCH1 decoder of NXDN (deinterleave, depuncture, Viterbi, CRC) on one block. Items are blocks.
fn nxdn_channel(seconds: f64) -> BenchResult {
    let mut coder = crate::nxdn::ChannelCoder::new(crate::nxdn::FACCH1);
    let info = vec![1u8; crate::nxdn::FACCH1.info];
    let soft: Vec<f32> = coder
        .encode(&info)
        .iter()
        .map(|&b| if b != 0 { 1.0 } else { -1.0 })
        .collect();
    measure(seconds, 1, 0.04, || {
        let _ = std::hint::black_box(coder.decode(&soft));
    })
}

/// The DMR rate 3/4 trellis decoder on the symbol levels of one block. Items are blocks.
fn dmr_trellis(seconds: f64) -> BenchResult {
    let mut codec = crate::dmr::Codecs::default().tcm;
    let info: Vec<u8> = (0..144).map(|i| (i % 3 == 0) as u8).collect();
    let mut sent = [0u8; 196];
    let _ = codec.encode(&info, &mut sent);
    let levels: Vec<f32> = sent
        .chunks(2)
        .map(|d| sdrkit_algo_fec::conv::tcm::dibit_symbol((d[0] << 1) | d[1]) as f32)
        .collect();
    let mut out = [0u8; 144];
    measure(seconds, 1, 0.03, || {
        let _ = std::hint::black_box(codec.decode_levels(&levels, &mut out));
    })
}

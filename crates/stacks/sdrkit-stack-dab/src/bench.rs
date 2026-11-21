//! Benchmarks of the DAB stack (for `sdrkit_benchmark`).

use crate::generator::{DabGenerator, GenParams};
use sdrkit_algo_sim::chain::{ChainConfig, Impaired, SignalInfo};
use sdrkit_algo_sim::noise::NoiseLevel;
use sdrkit_core::bench::measure;
use sdrkit_core::{
    BenchResult, Benchmark, Complex32, Generator, InputInfo, Output, Receiver, StackConfig,
};

/// The test signal at 20 dB C/N over 2.048 MHz, 1200 Hz off, at -14 dBFS, through the shared impairment chain.
fn signal(seconds: f64) -> Vec<Complex32> {
    let g = DabGenerator::new(GenParams::default());
    let cfg = ChainConfig {
        noise: NoiseLevel::SnrDb {
            snr_db: 20.0,
            bandwidth_hz: 2.048e6,
        },
        offset_hz: 1200.0,
        level_dbfs: -14.0,
        ..ChainConfig::default()
    };
    let mut v = vec![Complex32::default(); (seconds * 2.048e6) as usize];
    if let Ok(mut g) = Impaired::new(g, &cfg, SignalInfo::new(2.048e6)) {
        g.generate(&mut v);
    }
    v
}

fn receiver(service: &str) -> Option<sdrkit_graph::StackGraph> {
    use sdrkit_core::{owner, Owner, SettingSet};
    let schema = crate::DabSetting::schema();
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&schema));
    let mut r = crate::receiver::receiver(&cfg).ok()?;
    let id = r.commands().id("service.select").ok()?;
    let mut args = r.commands().def(id)?.new_args();
    args.set(&sdrkit_proto_mpegts::publish::SERVICE_ROW.field, service);
    owner::invoke(&mut r, id, &args).ok()?;
    r.configure(&InputInfo::new(2.048e6, 225.648e6, 225.648e6).recording())
        .ok()?;
    Some(r)
}

fn run_receiver(seconds: f64, service: &str) -> BenchResult {
    let x = signal(1.2);
    let Some(mut r) = receiver(service) else {
        return BenchResult::default();
    };
    let mut out = Output::default();
    r.feed(&x, &mut out);
    measure(seconds, x.len(), x.len() as f64 / 2.048e6, || {
        out.clear();
        r.feed(&x, &mut out);
    })
}

fn bench_dabplus(seconds: f64) -> BenchResult {
    run_receiver(seconds, "C103")
}

fn bench_mp2(seconds: f64) -> BenchResult {
    run_receiver(seconds, "C104")
}

fn bench_subchannel(seconds: f64) -> BenchResult {
    use crate::fec::{encode_profile, ProfileDecoder};
    let Some(p) = crate::tables::eep_profile(0, 3, 128) else {
        return BenchResult::default();
    };
    let data: Vec<u8> = (0..p.info_bits / 8).map(|i| (i * 37 % 251) as u8).collect();
    let mut coded = Vec::new();
    encode_profile(&p, &data, &mut coded);
    let llr: Vec<i8> = coded
        .iter()
        .map(|&b| if b == 0 { 40 } else { -40 })
        .collect();
    let Ok(mut dec) = ProfileDecoder::new(&p) else {
        return BenchResult::default();
    };
    let mut out = vec![0u8; data.len()];
    measure(seconds, 8 * data.len(), 0.024, || {
        let _ = dec.decode(&llr, &mut out);
    })
}

/// The benchmarks: the whole receiver on a DAB+ and on a Layer II service at 2.048 Msps, and the decoding of one 128 kbit/s EEP 3-A
/// logical frame (depuncturing, Viterbi decoding and energy dispersal; real time is 24 ms).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "dab.receiver_dabplus_2048k",
            run: bench_dabplus,
        },
        Benchmark {
            name: "dab.receiver_mp2_2048k",
            run: bench_mp2,
        },
        Benchmark {
            name: "dab.subchannel_eep3a_128k",
            run: bench_subchannel,
        },
    ]
}

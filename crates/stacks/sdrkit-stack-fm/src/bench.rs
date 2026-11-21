//! Benchmarks of the FM receiver and its main parts, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]). Each
//! makes the test station first (stereo, RDS, 30 dB C/N) and times the receiver, or one part, on it. `items_per_second` counts
//! input samples; `cpu_fraction_of_realtime` is for the input rate in the name (the FM budgets: under 3.5 % of one core at 2.4 Msps,
//! under 5 % at 4 Msps).

use crate::channel::ChannelSelect;
use crate::fmdemod::{FmDemod, Mpx};
use crate::generator::{FmGenerator, GenParams};
use crate::graph;
use crate::settings::FmSetting;
use crate::stereo::StereoDecoder;
use sdrkit_core::bench::measure;
use sdrkit_core::{
    BenchResult, Benchmark, Complex32, Generator, InputInfo, Output, Receiver, SettingSet,
    StackConfig, Values,
};

fn station(rate: f64, seconds: f64) -> Vec<Complex32> {
    let mut g = FmGenerator::new(GenParams {
        rate_hz: rate,
        ..Default::default()
    });
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    // 30 dB carrier to noise in 200 kHz.
    let p = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    let level = sdrkit_algo_sim::noise::NoiseLevel::SnrDb {
        snr_db: 30.0,
        bandwidth_hz: 200e3,
    };
    if let Ok(mut n) = sdrkit_algo_sim::noise::Awgn::complex(level, p, rate, 1) {
        n.process(&mut x);
    }
    x
}

fn receiver(seconds: f64, rate: f64) -> BenchResult {
    let x = station(rate, 1.0);
    let cfg = StackConfig::new(Values::defaults(&FmSetting::schema()));
    let Ok(mut rx) = graph::receiver(&cfg) else {
        return BenchResult::default();
    };
    if rx
        .configure(&InputInfo::new(rate, 100e6, 100e6).recording())
        .is_err()
    {
        return BenchResult::default();
    }
    let mut out = Output::default();
    measure(seconds, x.len(), 1.0, || {
        for b in x.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

fn channel(seconds: f64) -> BenchResult {
    let rate = 2.4e6;
    let x = station(rate, 0.25);
    let mut c = ChannelSelect::new(rate, 0.0, 0.0);
    let mut out = Vec::with_capacity(x.len());
    measure(seconds, x.len(), 0.25, || {
        out.clear();
        c.process(&x, &mut out);
    })
}

fn channel_samples(rate: f64) -> (Vec<Complex32>, f64) {
    let x = station(rate, 0.25);
    let mut c = ChannelSelect::new(rate, 0.0, 0.0);
    let mut out = Vec::new();
    c.process(&x, &mut out);
    (out, c.output_rate())
}

fn demod(seconds: f64) -> BenchResult {
    let (ch, fo) = channel_samples(2.4e6);
    let mut d = FmDemod::for_input(2.4e6).expect("2.4 MS/s decimates to a legal channel rate");
    let mut mpx = Mpx::default();
    measure(seconds, ch.len(), ch.len() as f64 / fo, || {
        mpx.clear();
        d.process(&ch, &mut mpx);
    })
}

fn stereo(seconds: f64) -> BenchResult {
    let (ch, fo) = channel_samples(2.4e6);
    let mut d = FmDemod::for_input(2.4e6).expect("2.4 MS/s decimates to a legal channel rate");
    let mut mpx = Mpx::default();
    d.process(&ch, &mut mpx);
    let mut s = StereoDecoder::new(fo / 2.0, Default::default());
    let (mut a, mut p) = (Vec::new(), Vec::new());
    measure(
        seconds,
        mpx.sound.len(),
        mpx.sound.len() as f64 / (fo / 2.0),
        || {
            a.clear();
            p.clear();
            s.process(&mpx.sound, 2, &mut a, &mut p);
        },
    )
}

/// The benchmarks of this crate.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "fm.receiver_2400k",
            run: |s| receiver(s, 2.4e6),
        },
        Benchmark {
            name: "fm.receiver_4000k",
            run: |s| receiver(s, 4e6),
        },
        Benchmark {
            name: "fm.channel_2400k",
            run: channel,
        },
        Benchmark {
            name: "fm.demod_2400k",
            run: demod,
        },
        Benchmark {
            name: "fm.stereo_2400k",
            run: stereo,
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

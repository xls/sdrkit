//! Benchmarks of graphs of the standard components, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]).
//! `items_per_second` counts input samples; `cpu_fraction_of_realtime` is for the input rate in the name.

use crate::components::*;
use crate::{Graph, GraphBuilder};
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark, Complex32, InputInfo, Output, Receiver};

fn signal(n: usize, rate: f64) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let t = i as f64 / rate;
            let e = 0.2 * (1.0 + 0.5 * (std::f64::consts::TAU * 1000.0 * t).cos());
            Complex32::from_polar(
                e as f32,
                (std::f64::consts::TAU * (30e3 * t).fract()) as f32,
            )
        })
        .collect()
}

fn run(seconds: f64, rate: f64, mut g: Graph) -> BenchResult {
    g.configure(&InputInfo::new(rate, 100e6, 100e6))
        .expect("configures");
    let n = (rate / 10.0) as usize;
    let x = signal(n, rate);
    let mut out = Output::default();
    measure(seconds, n, n as f64 / rate, || {
        out.clear();
        g.feed(&x, &mut out);
    })
}

/// An AM receiver: DC removal, a 10 kHz channel at an offset, the detector, AGC, audio at 48 kHz, plus a spectrum.
fn am_receiver(seconds: f64) -> BenchResult {
    let g = GraphBuilder::new()
        .add("dc", DcRemoval::default())
        .add(
            "chan",
            Channel::new(10e3, 24e3)
                .expect("legal values")
                .with_offset(30e3)
                .expect("legal values"),
        )
        .add("demod", AmDemod::default())
        .add("agc", AgcComponent::default())
        .add("audio", AudioResampler::new(48e3).expect("legal values"))
        .add(
            "spec",
            Spectrum::new(SpectrumSize::S4096, 30.0).expect("legal values"),
        )
        .add("sink", NullSink::default())
        .input("dc")
        .connect("dc", "chan")
        .connect("chan", "demod")
        .connect("demod", "agc")
        .connect("agc", "audio")
        .connect("dc", "spec")
        .connect("spec", "sink")
        .output("audio")
        .build()
        .expect("builds");
    run(seconds, 2.4e6, g)
}

/// The graph's own cost: three components that pass the samples on in place.
fn pass_through(seconds: f64) -> BenchResult {
    let g = GraphBuilder::new()
        .add("a", NullSink::default())
        .add("s", Shift::new(0.0).expect("legal values"))
        .add("d", DeemphasisComponent::new(0.0))
        .add("fm", FmDemod::new(75e3).expect("legal values"))
        .input("s")
        .input("a")
        .connect("s", "fm")
        .connect("fm", "d")
        .add("b", NullSink::default())
        .connect("d", "b")
        .build()
        .expect("builds");
    run(seconds, 2.4e6, g)
}

/// The merge of results from eight level meters, four of whose keys the stack declared (`peak_dbfs` from every meter, the
/// first meter giving it), as the graph does about 60 times a second of signal. `items_per_second` counts merges.
fn declared_merge(seconds: f64) -> BenchResult {
    const PEAK_DBFS: sdrkit_core::Field<sdrkit_core::publish::field::F64> =
        sdrkit_core::Field::new("peak_dbfs", "Peak", "dBFS");
    let decl = sdrkit_core::ResultsDecl::new().with_fields(&[&PEAK_DBFS]);
    let mut b = GraphBuilder::new();
    for i in 0..8 {
        let id = format!("m{i}");
        b = b.add(&id, PowerMeterSink::default()).input(&id);
    }
    let mut g = b.status_from("m0").declare(&decl).build().expect("builds");
    g.configure(&InputInfo::new(240e3, 100e6, 100e6))
        .expect("configures");
    let mut out = Output::default();
    g.feed(&signal(24_000, 240e3), &mut out);
    let merges = 600;
    measure(seconds, merges, merges as f64 / 60.0, || {
        for _ in 0..merges {
            g.refresh_results();
        }
    })
}

/// A rate change of stereo audio, 48 kHz to 44.1 kHz, after an FM discriminator and a duplicate to two channels.
fn stereo_resample(seconds: f64) -> BenchResult {
    let g = GraphBuilder::new()
        .add("fm", FmDemod::new(5e3).expect("legal values"))
        .add("audio", RealToAudio::default())
        .add("dup", Duplicate::default())
        .add("rs", Resample::new(44.1e3).expect("legal values"))
        .input("fm")
        .connect("fm", "audio")
        .connect("audio", "dup")
        .connect("dup", "rs")
        .output("rs")
        .build()
        .expect("builds");
    run(seconds, 48e3, g)
}

/// The benchmarks of this crate.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "graph.am_receiver_2400k",
            run: am_receiver,
        },
        Benchmark {
            name: "graph.shift_fm_demod_2400k",
            run: pass_through,
        },
        Benchmark {
            name: "graph.declared_results_merge",
            run: declared_merge,
        },
        Benchmark {
            name: "graph.stereo_resample_48k",
            run: stereo_resample,
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

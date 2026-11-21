//! Benchmarks of the crate's stacks on their own test signals, for `sdrkit bench` (see [`sdrkit_core::bench`]).
//! `items_per_second` counts input samples. `cpu_fraction_of_realtime` is for the input rate in the name.

use sdrkit_core::bench::measure;
use sdrkit_core::{
    BenchResult, Benchmark, Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor,
    Values,
};

/// Runs the receiver of `d` at its defaults over `signal` at `rate`, tuned to `centre`, for `seconds` of processor time.
pub fn run(
    d: &StackDescriptor,
    rate: f64,
    centre: f64,
    seconds: f64,
    signal: &[Complex32],
) -> BenchResult {
    let cfg = StackConfig::new(Values::defaults(&(d.settings)()));
    let Ok(mut rx) = (d.make_receiver)(&cfg) else {
        return BenchResult::default();
    };
    let _ = rx.configure(&InputInfo::new(rate, centre, centre).recording());
    let mut out = Output::default();
    let sig_s = signal.len() as f64 / rate;
    measure(seconds, signal.len(), sig_s, || {
        for b in signal.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// `secs` seconds of the test signal of `d` at `rate` with the default settings.
pub fn gen(d: &StackDescriptor, rate: f64, secs: f64) -> Vec<Complex32> {
    let cfg = GenConfig::new(rate, Values::defaults(&(d.settings)())).with_seed(1);
    let Some(Ok(mut g)) = d.make_generator.map(|make| make(&cfg)) else {
        return Vec::new();
    };
    let mut x = vec![Complex32::default(); (rate * secs) as usize];
    g.generate(&mut x);
    x
}

/// The EM4100 reader at 250 ksps: the front end to 31.25 ksps, the slicer and two half-bit readers.
fn rfid_lf_250k(s: f64) -> BenchResult {
    let d = &crate::rfid_lf::DESCRIPTOR;
    let x = gen(d, 250e3, 2.0);
    run(d, 250e3, 125e3, s, &x)
}

/// The Qi reader at 250 ksps: the front end to 16.7 ksps, the slicer, the bit clock and the packets.
fn qi_250k(s: f64) -> BenchResult {
    let d = &crate::qi::DESCRIPTOR;
    let x = gen(d, 250e3, 2.0);
    run(d, 250e3, 140e3, s, &x)
}

/// The Gen2 reader at 2 Msps: the front end to 1 Msps, the slicer and the frames of pulse intervals.
fn rfid_uhf_2000k(s: f64) -> BenchResult {
    let d = &crate::rfid_uhf::DESCRIPTOR;
    let x = gen(d, 2e6, 1.0);
    run(d, 2e6, 865e6, s, &x)
}

/// The benchmarks of the crate.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "tags.rfid_lf_250k",
            run: rfid_lf_250k,
        },
        Benchmark {
            name: "tags.rfid_uhf_2000k",
            run: rfid_uhf_2000k,
        },
        Benchmark {
            name: "tags.qi_250k",
            run: qi_250k,
        },
    ]
}

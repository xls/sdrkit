//! Runs the DSP benchmarks: `cargo run --release -p sdrkit-algo-dsp --example bench`. With the argument `plain`, the FIR
//! filters and the resamplers run on their plain backends.

fn main() {
    if std::env::args().nth(1).as_deref() == Some("plain") {
        let plain = sdrkit_algo_dsp::fir::BACKENDS.plain().name();
        sdrkit_algo_dsp::fir::BACKENDS
            .select(plain)
            .expect("registered");
        let plain = sdrkit_algo_dsp::resample::BACKENDS.plain().name();
        sdrkit_algo_dsp::resample::BACKENDS
            .select(plain)
            .expect("registered");
    }
    for b in sdrkit_algo_dsp::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:40} {:10.1} M items/s  {:7.3} % of real time",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

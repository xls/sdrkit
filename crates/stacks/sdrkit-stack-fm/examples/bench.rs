//! Runs the DSP, graph and FM benchmarks, as `sdrkit_benchmark` does on a device:
//! `cargo run --release -p sdrkit-stack-fm --example bench`.

fn main() {
    let all = sdrkit_algo_dsp::benchmarks()
        .into_iter()
        .chain(sdrkit_graph::benchmarks())
        .chain(sdrkit_stack_fm::benchmarks());
    for b in all {
        let r = (b.run)(1.0);
        println!(
            "{:40} {:10.1} M items/s  {:7.3} % of real time",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

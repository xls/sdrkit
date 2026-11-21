//! Runs the simulation benchmarks: `cargo run --release -p sdrkit-algo-sim --example bench`.

fn main() {
    for b in sdrkit_algo_sim::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:40} {:10.1} M items/s  {:7.3} % of real time",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

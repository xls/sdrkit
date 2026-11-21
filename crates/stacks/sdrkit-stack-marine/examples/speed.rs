//! Runs the marine benchmarks and prints the CPU share of real time: `cargo run --release --example speed -p sdrkit-stack-marine`.

fn main() {
    for b in sdrkit_stack_marine::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:28} {:>12.0} samples/s  {:>6.2} % of one core",
            b.name,
            r.items_per_second,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

//! Runs the family's benchmarks: `cargo run --release -p sdrkit-stack-avdata --example speed`.

fn main() {
    for b in sdrkit_stack_avdata::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:28} {:8.2} Msps  {:6.2} % of one core",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

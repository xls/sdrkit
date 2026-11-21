//! Runs the crate's benchmarks and prints them: `cargo run --release -p sdrkit-stack-general --example speed`.

fn main() {
    for b in sdrkit_stack_general::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:28} {:>8.1} Msps  {:>6.2} % of one core in real time",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

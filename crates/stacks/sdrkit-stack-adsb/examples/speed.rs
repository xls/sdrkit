//! Prints the speed of the receivers and the generator: `cargo run --release -p sdrkit-stack-adsb --example speed`.

fn main() {
    for b in sdrkit_stack_adsb::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:28} {:8.1} Msps   {:6.2} % of one core in real time   {:5.1}x real time",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime,
            1.0 / r.cpu_fraction_of_realtime
        );
    }
}

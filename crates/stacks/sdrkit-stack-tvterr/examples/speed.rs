//! Runs the benchmarks of this crate and prints the speed of each receiver against real time (build with --release).
fn main() {
    for b in sdrkit_stack_tvterr::benchmarks() {
        let r = (b.run)(2.0);
        println!(
            "{:28} {:8.2} Msamples/s  {:6.1} % of one core per second of signal",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

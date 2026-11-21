//! Runs the modem benchmarks: `cargo run --profile shipped -p sdrkit-algo-modem --example bench [text]`. With `text`,
//! only the benchmarks whose name contains it.

fn main() {
    let filter = std::env::args().nth(1).unwrap_or_default();
    for b in sdrkit_algo_modem::benchmarks() {
        if !b.name.contains(&filter) {
            continue;
        }
        let r = (b.run)(1.0);
        println!(
            "{:44} {:10.1} M items/s  {:8.3} % of real time",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

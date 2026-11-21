//! Runs the benchmarks of this crate: `cargo run --release -p sdrkit-proto-mpegts --example bench`.

fn main() {
    for b in sdrkit_proto_mpegts::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:28} {:10.2} M packets/s {:8.3} % of one core in real time",
            b.name,
            r.items_per_second / 1e6,
            r.cpu_fraction_of_realtime * 100.0
        );
    }
}

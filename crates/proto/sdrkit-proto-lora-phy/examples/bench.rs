//! Times the hot paths: `cargo run --release -p sdrkit-proto-lora-phy --example bench`. Each line reports the best run in millions of
//! items per second (samples for the receivers, symbols for the decoders) and the share of one core that real time needs.

fn main() {
    for b in sdrkit_proto_lora_phy::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:28} {:10.3} M/s {:8.3} % of one core in real time",
            b.name,
            r.items_per_second / 1e6,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

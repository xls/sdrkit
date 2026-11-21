//! Times the hot paths: `cargo run --release -p sdrkit-codec-image --example bench`. Each line reports millions of items per second
//! (the item of each benchmark is in the documentation of `sdrkit_codec_image::benchmarks`) and the share of one core at the real-time
//! rate the benchmark names.

fn main() {
    for b in sdrkit_codec_image::benchmarks() {
        let r = (b.run)(0.5);
        println!(
            "{:32} {:10.2} M items/s {:9.4} % of a core in real time",
            b.name,
            r.items_per_second / 1e6,
            r.cpu_fraction_of_realtime * 100.0
        );
    }
}

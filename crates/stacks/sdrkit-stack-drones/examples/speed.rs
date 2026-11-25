//! Times each stack's receiver on its own test signal (items are input samples): `cargo run --release --example speed`.

use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, Values};

fn main() {
    for desc in sdrkit_stack_drones::descriptors() {
        let settings = Values::defaults(&(desc.settings)());
        let rate = desc.tuning.rate_hz;
        let mut g = (desc.make_generator.unwrap())(&GenConfig::new(rate, settings.clone()).with_seed(1))
        .unwrap();
        let n = (rate * 0.5) as usize;
        let mut x = vec![Complex32::new(0.0, 0.0); n];
        let t0 = std::time::Instant::now();
        g.generate(&mut x);
        let tg = t0.elapsed().as_secs_f64();
        let mut rx = (desc.make_receiver)(&StackConfig::new(settings)).unwrap();
        rx.configure(
            &InputInfo::new(rate, desc.tuning.default_hz, desc.tuning.default_hz).recording(),
        )
        .unwrap();
        let mut out = Output::default();
        let t0 = std::time::Instant::now();
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
        let tr = t0.elapsed().as_secs_f64();
        println!(
            "{:12} generator {:6.2}x real time, receiver {:6.2}x real time",
            desc.id,
            0.5 / tg,
            0.5 / tr
        );
    }
    for b in sdrkit_stack_drones::benchmarks() {
        let r = (b.run)(1.0);
        println!(
            "{:26} {:5.1} % of one core at real time, {:.1} ms an item",
            b.name,
            100.0 * r.cpu_fraction_of_realtime,
            1e3 / r.items_per_second
        );
    }
}

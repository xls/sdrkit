//! Speed: run with `cargo test --release -p sdrkit-stack-dab --test speed -- --ignored --nocapture`.

#[test]
#[ignore]
fn benchmarks() {
    for b in sdrkit_stack_dab::benchmarks() {
        let r = (b.run)(2.0);
        eprintln!(
            "{:32} {:12.0} items/s  {:6.2} % of one core in real time",
            b.name,
            r.items_per_second,
            100.0 * r.cpu_fraction_of_realtime
        );
    }
}

#[test]
fn benchmarks_run() {
    for b in sdrkit_stack_dab::benchmarks() {
        let r = (b.run)(0.01);
        assert!(r.items_per_second > 0.0, "{}", b.name);
    }
}

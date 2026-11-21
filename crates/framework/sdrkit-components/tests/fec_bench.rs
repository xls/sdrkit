//! The `fec.*` benchmarks run and measure something. `FEC_BENCH_SECONDS` sets how long each runs (default: as short as
//! possible); with `--nocapture` the numbers are printed.

#[test]
fn every_benchmark_runs() {
    let seconds = std::env::var("FEC_BENCH_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let list = sdrkit_components::fec::benchmarks();
    assert!(list.len() >= 7);
    for b in list {
        assert!(b.name.starts_with("fec."), "{}", b.name);
        let r = (b.run)(seconds);
        assert!(
            r.items_per_second > 0.0 && r.cpu_fraction_of_realtime > 0.0,
            "{}",
            b.name
        );
        println!(
            "{:28} {:>14.0} items/s  {:>8.4} of real time",
            b.name, r.items_per_second, r.cpu_fraction_of_realtime
        );
    }
}

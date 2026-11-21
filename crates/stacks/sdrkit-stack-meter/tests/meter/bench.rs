//! The benchmarks run and keep up with real time.

#[test]
fn every_benchmark_runs_faster_than_real_time() {
    for b in sdrkit_stack_meter::bench::benchmarks() {
        let r = (b.run)(0.5);
        println!("{}: {:.2} % of one core in real time", b.name, 100.0 * r.cpu_fraction_of_realtime);
        assert!(r.cpu_fraction_of_realtime > 0.0 && r.cpu_fraction_of_realtime < 1.0, "{}", b.name);
    }
}

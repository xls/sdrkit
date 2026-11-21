//! Benchmarks: named, self-contained timing runs that each crate exposes (`sdrkit_algo_dsp::benchmarks()`, `sdrkit_graph::benchmarks()`,
//! a stack's `benchmarks()`), so a host can run them on the device that matters (the C API's `sdrkit_benchmark`).

use serde::{Deserialize, Serialize};

/// What one benchmark run measured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BenchResult {
    /// Items processed per second of CPU time (input samples for a sample stream; transforms for an FFT; the benchmark's own
    /// documentation says which).
    pub items_per_second: f64,
    /// CPU time spent per second of signal at the benchmark's nominal rate (0.035 is 3.5 % of one core). For work without a real-time
    /// rate it is relative to the rate the benchmark names.
    pub cpu_fraction_of_realtime: f64,
}

/// A named benchmark. `run(seconds)` runs it for about that much wall-clock time on the calling thread and returns what it
/// measured; it allocates its test signal first, outside the timed part.
#[derive(Clone, Copy, Debug)]
pub struct Benchmark {
    /// A stable name: `"<crate>.<what>"`, for example `"dsp.fft_4096"` or `"fm.receiver_2400k"`.
    pub name: &'static str,
    /// Runs it.
    pub run: fn(f64) -> BenchResult,
}

/// Times `step` (which processes `items` items, standing for `signal_s` seconds of signal) repeatedly for at least `seconds` (and at
/// least three times), after one untimed warm-up call, and returns the throughput of the fastest call (so other work on the device
/// does not count against it). A helper for writing benchmarks.
pub fn measure(seconds: f64, items: usize, signal_s: f64, mut step: impl FnMut()) -> BenchResult {
    use std::time::Instant;
    step();
    let start = Instant::now();
    let mut runs = 0u64;
    let mut best = f64::INFINITY;
    loop {
        let t0 = Instant::now();
        step();
        best = best.min(t0.elapsed().as_secs_f64());
        runs += 1;
        if runs >= 3 && start.elapsed().as_secs_f64() >= seconds {
            let per_run = best.max(1e-12);
            return BenchResult {
                items_per_second: items as f64 / per_run,
                cpu_fraction_of_realtime: if signal_s > 0.0 {
                    per_run / signal_s
                } else {
                    0.0
                },
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_counts_items_and_signal_time() {
        let mut n = 0u64;
        let r = measure(0.01, 1000, 1.0, || {
            for i in 0..1000u64 {
                n = n.wrapping_add(std::hint::black_box(i));
            }
        });
        assert!(r.items_per_second > 1e6);
        assert!(r.cpu_fraction_of_realtime > 0.0 && r.cpu_fraction_of_realtime < 0.1);
        assert!(n > 0);
    }
}

//! Benchmarks of the `ble` receiver on its own test signal at its preferred rate (8 Msps: channels 37 and 0), at 20 Msps and at 32 Msps
//! (channels 37, 0 to 11 and 38), for `sdrkit bench`, then the Wi-Fi and BLE receivers of the protocol crate. Items are input
//! samples. A benchmark whose receiver decodes nothing from its signal reports no throughput.

use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark, Complex32, GenConfig, InputInfo, Output, StackConfig, Values};

fn run(rate: f64, seconds: f64) -> BenchResult {
    let d = &super::BLE;
    let schema = (d.settings)();
    let cfg = GenConfig::new(rate, Values::defaults(&schema)).with_seed(1);
    let Some(Ok(mut g)) = d.make_generator.map(|make| make(&cfg)) else {
        return BenchResult::default();
    };
    let mut x = vec![Complex32::default(); (rate * 0.5) as usize];
    g.generate(&mut x);
    // The radio sits 1 MHz above channel 37: the test signal's channel 37 is at -1 MHz in the capture.
    let shift = -d.tuning.tune_offset_hz;
    for (n, s) in x.iter_mut().enumerate() {
        let ph = std::f64::consts::TAU * shift * n as f64 / rate;
        *s *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
    }
    let Ok(mut rx) = (d.make_receiver)(&StackConfig::new(Values::defaults(&schema))) else {
        return BenchResult::default();
    };
    // The radio tuned 1 MHz above channel 37, as the stack asks.
    let dial = d.tuning.default_hz;
    let _ = rx.configure(&InputInfo::new(rate, dial + d.tuning.tune_offset_hz, dial).recording());
    let mut out = Output::default();
    let mut heard = 0;
    for b in x.chunks(65_536) {
        out.clear();
        rx.feed(b, &mut out);
        heard += out.events.len();
    }
    if heard == 0 {
        return BenchResult::default();
    }
    let n = x.len();
    measure(seconds, n, n as f64 / rate, || {
        for b in x.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// The benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    let mut v = vec![
        Benchmark {
            name: "ble.receiver_8000k",
            run: |s| run(8e6, s),
        },
        Benchmark {
            name: "ble.receiver_20000k",
            run: |s| run(20e6, s),
        },
        Benchmark {
            name: "ble.receiver_32000k",
            run: |s| run(32e6, s),
        },
    ];
    v.extend(sdrkit_proto_wlan_phy::benchmarks());
    v
}

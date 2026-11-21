//! Runs the benchmarks of the hot paths: `cargo run --release -p sdrkit-proto-ookdev --example bench`.

use sdrkit_core::Complex32;
use sdrkit_proto_ookdev::guess::Guesser;
use sdrkit_proto_ookdev::{
    encode, BitRows, Coding, DetectorConfig, Modulation, PulseDetector, Slicer, Timing, Train,
};
use std::hint::black_box;
use std::time::Instant;

/// Calls `f` (which handles `items` items) for about half a second and prints items per second.
fn bench(name: &str, items: usize, unit: &str, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    let mut n = 0usize;
    while start.elapsed().as_secs_f64() < 0.5 {
        f();
        n += items;
    }
    let rate = n as f64 / start.elapsed().as_secs_f64();
    println!("{name:44} {:10.2} M {unit}/s", rate / 1e6);
}

fn main() {
    // 1 Msps: a 24-pulse PWM burst every 50 ms over a quiet floor (1 s of signal).
    let rate = 1e6;
    let mut x = vec![Complex32::new(1e-3, 0.0); rate as usize];
    let mut at = 1000usize;
    while at + 40_000 < x.len() {
        for k in 0..24 {
            let m = if k % 3 == 0 { 1404 } else { 464 };
            for v in &mut x[at..at + m] {
                *v = Complex32::new(0.3, 0.0);
            }
            at += 1868;
        }
        at += 50_000 - 24 * 1868;
    }
    let mut ook = DetectorConfig::new(rate);
    ook.modulation = Some(Modulation::Ook);
    let mut d = PulseDetector::new(ook).unwrap();
    bench("detector, OOK only (samples)", x.len(), "samples", || {
        d.process(&x, |t| {
            black_box(t.pulses.len());
        })
    });
    let mut d = PulseDetector::new(DetectorConfig::new(rate)).unwrap();
    bench(
        "detector, OOK and FSK (samples)",
        x.len(),
        "samples",
        || {
            d.process(&x, |t| {
                black_box(t.pulses.len());
            })
        },
    );

    // Slicers: a 64-bit PWM frame and a 400-bit NRZ frame.
    let bits: Vec<u8> = (0..64).map(|k| ((k * 5 + 1) % 3 == 0) as u8).collect();
    let mut pwm = Train::new(Modulation::Ook, rate);
    encode::pwm(&bits, (464.0, 1404.0), (1404.0, 464.0), &mut pwm);
    let mut s = Slicer::new();
    let tp = Timing::new(Coding::Pwm, 464.0, 1404.0, 5000.0);
    bench("slicer, PWM (pulses)", pwm.pulses.len(), "pulses", || {
        s.slice(&tp, &pwm, |r| {
            black_box(r.len(0));
        })
        .unwrap()
    });
    let nbits: Vec<u8> = (0..400).map(|k| ((k * 7 + k / 3) % 2) as u8).collect();
    let mut nrz = Train::new(Modulation::Fsk, rate);
    encode::nrz(&nbits, 52.0, &mut nrz);
    let tn = Timing::new(Coding::Nrz, 52.0, 52.0, 1000.0);
    bench("slicer, NRZ (bits)", nbits.len(), "bits", || {
        s.slice(&tn, &nrz, |r| {
            black_box(r.len(0));
        })
        .unwrap()
    });

    // Row decoders over 1024 chips.
    let mut rows = BitRows::default();
    for k in 0..1024 {
        rows.push(((k / 2 + k) % 2) as u8);
    }
    let mut out = BitRows::default();
    bench("rows, Manchester decode (chips)", 1024, "chips", || {
        black_box(rows.decode_manchester(
            0,
            0,
            4096,
            sdrkit_util_bits::line::ManchesterConv::Ieee,
            &mut out,
        ));
    });
    bench("rows, FM0 decode (chips)", 1024, "chips", || {
        black_box(rows.decode_fm0(0, 0, 4096, &mut out));
    });

    let mut g = Guesser::default();
    bench(
        "line-code guess (pulses)",
        pwm.pulses.len(),
        "pulses",
        || {
            black_box(g.guess(&pwm));
        },
    );
}

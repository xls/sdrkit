//! Benchmarks of the family, for `sdrkit_benchmark` on the device (see [`sdrkit_core::bench`]): the front end at 2
//! Msps, one slot of each decoder whose code sdrkit ships with 8 stations at -15 dB, the crowded FT8 slot, and Q65 slots of
//! 15 s and 60 s. `items_per_second` counts input samples (slots for the decoders), and
//! `cpu_fraction_of_realtime` is the time taken over the signal time.

use crate::encode::{audio, Transmission};
use crate::proto::codes::CodeStore;
use crate::front::{Demod, FrontEnd};
use crate::mode::{Mode, Sub};
use crate::runner::{decode_slot, DecodeState};
use crate::rx::slot::Slot;
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark, Complex32};
use std::sync::Arc;

const MSGS: [&str; 8] = [
    "CQ K1ABC FN42",
    "K1ABC W9XYZ -12",
    "W9XYZ K1ABC R-05",
    "CQ DX G4ABC IO91",
    "G4ABC DL1XYZ RR73",
    "CQ JA1XYZ PM95",
    "TNX BOB 73 GL",
    "CQ PJ4/K1ABC",
];

/// White noise of sigma 0.05, seeded.
fn noise(n: usize, seed: u64) -> Vec<f32> {
    let mut a = vec![0f32; n];
    if let Ok(mut w) = sdrkit_algo_sim::noise::Awgn::with_power(0.0025, seed) {
        w.process_real(&mut a);
    }
    a
}

/// A slot of `sub` with `n` stations at `snr` dB.
pub fn test_slot(sub: Sub, n: usize, snr: f64) -> Slot {
    let (w0, w1) = sub.window();
    let len = ((w1 - w0) * 8000.0) as usize;
    let mut a = noise(len, 1);
    let amp = (2.0 * 10f64.powf(snr / 10.0) * 0.0025 * 2500.0 / 4000.0).sqrt() as f32;
    let wspr = matches!(sub.mode, Mode::Wspr | Mode::Fst4w);
    for i in 0..n {
        let text = if wspr {
            [
                "K1ABC FN42 37",
                "G4XYZ IO91 23",
                "DL1XYZ JO62 10",
                "PJ4/K1ABC 30",
            ][i % 4]
                .to_string()
        } else {
            MSGS[i % 8].to_string()
        };
        let (lo, hi) = if wspr {
            (1420.0, 1580.0)
        } else {
            (300.0, 2700.0)
        };
        let hz = lo + (hi - lo) * i as f64 / n.max(1) as f64;
        if let Ok(s) = audio(
            &Transmission {
                sub,
                hz,
                amp,
                mirrored: false,
            },
            &text,
            8000.0,
            &CodeStore::default(),
        ) {
            let st = ((sub.t0() - w0) * 8000.0) as usize;
            for (j, v) in s.iter().enumerate() {
                if st + j < len {
                    a[st + j] += v;
                }
            }
        }
    }
    Slot {
        sub,
        utc: 0.0,
        audio: Arc::new(a),
        rate: 8000.0,
        t_first: w0,
        search: false,
        dial_hz: 0.0,
        index: 0,
        early: false,
        seq: 0,
    }
}

fn decoder(mode: Mode, n: usize, seconds: f64) -> BenchResult {
    let sub = Sub::new(mode);
    let slot = test_slot(sub, n, -15.0);
    let mut st = DecodeState::default();
    measure(seconds, 1, sub.period, || {
        st.clear();
        let _ = decode_slot(&slot, &mut st);
    })
}

/// One Q65-A slot of `period` seconds with 8 stations at -15 dB.
fn q65(period: f64, seconds: f64) -> BenchResult {
    let sub = Sub::new(Mode::Q65).with_period(period);
    let slot = test_slot(sub, 8, -15.0);
    let mut st = DecodeState::default();
    measure(seconds, 1, sub.period, || {
        st.clear();
        let _ = decode_slot(&slot, &mut st);
    })
}

fn front(seconds: f64) -> BenchResult {
    let rate = 2e6;
    let x: Vec<Complex32> = noise(2 * 200_000, 3)
        .chunks(2)
        .map(|c| Complex32::new(c[0], c[1]))
        .collect();
    let mut fe = FrontEnd::new(rate, -20e3, Demod::Usb).expect("a front end");
    let mut out = Vec::new();
    measure(seconds, x.len(), x.len() as f64 / rate, || {
        out.clear();
        fe.process(&x, &mut out);
    })
}

/// The family's benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "wsjt.front_2000k",
            run: front,
        },
        Benchmark {
            name: "wsjt.ft8_slot_8",
            run: |s| decoder(Mode::Ft8, 8, s),
        },
        Benchmark {
            name: "wsjt.ft8_slot_crowded",
            run: |s| decoder(Mode::Ft8, 100, s),
        },
        Benchmark {
            name: "wsjt.ft4_slot_8",
            run: |s| decoder(Mode::Ft4, 8, s),
        },
        Benchmark {
            name: "wsjt.ft2_slot_8",
            run: |s| decoder(Mode::Ft2, 8, s),
        },
        Benchmark {
            name: "wsjt.wspr_slot_8",
            run: |s| decoder(Mode::Wspr, 8, s),
        },
        Benchmark {
            name: "wsjt.jt65_slot_8",
            run: |s| decoder(Mode::Jt65, 8, s),
        },
        Benchmark {
            name: "wsjt.jt9_slot_8",
            run: |s| decoder(Mode::Jt9, 8, s),
        },
        Benchmark {
            name: "wsjt.jt4_slot_8",
            run: |s| decoder(Mode::Jt4, 8, s),
        },
        Benchmark {
            name: "wsjt.q65_15a_slot_8",
            run: |s| q65(15.0, s),
        },
        Benchmark {
            name: "wsjt.q65_60a_slot_8",
            run: |s| q65(60.0, s),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_front_end_is_quick() {
        let b = front(0.05);
        assert!(b.items_per_second > 0.0);
        assert_eq!(benchmarks().len(), 11);
    }
}

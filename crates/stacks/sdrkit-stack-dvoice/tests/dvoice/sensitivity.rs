//! Sensitivity of the narrow channels: the share of the good blocks (frames whose FEC or CRC checks) of a clean run that come
//! back in noise, at carrier offsets, over 8 noise seeds. The SNR is in the 12.5 kHz of the signal.
use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::Complex32;

const RATE: f64 = 96_000.0;
const TRIALS: u64 = 8;

/// The protocol results of stack `id` on `x`.
fn protocol(id: &str, x: &[Complex32]) -> serde_json::Value {
    let mut rx = receiver(id, &[]);
    let r = run(rx.as_mut(), RATE, x, 4800).0;
    r["detail"]["protocol"].clone()
}

fn good_blocks(id: &str, x: &[Complex32]) -> u64 {
    protocol(id, x)["blocks_ok"].as_u64().unwrap_or(0)
}

/// The share of the good blocks of the clean run on the centre that come back at `snr_db` and `offset_hz`, over the trials.
/// A trial counts at most the clean run's blocks: after a move of the carrier search the front replays 0.3 s, and the frames
/// in it can count twice.
fn share(id: &str, offset_hz: f64, snr_db: f64) -> f64 {
    let clean = good_blocks(id, &impaired_with_seed(id, RATE, 0.0, 0.0, 80.0, 1));
    assert!(clean > 0, "{id}: the clean run decodes");
    let got: u64 = (1..=TRIALS)
        .map(|seed| {
            good_blocks(
                id,
                &impaired_with_seed(id, RATE, offset_hz, 0.0, snr_db, seed),
            )
            .min(clean)
        })
        .sum();
    got as f64 / (TRIALS * clean) as f64
}

/// Checks the shares of `id` against `floors`: per carrier offset, the least share at each SNR.
fn check_shares(id: &str, floors: &[(f64, &[(f64, f64)])]) {
    let mut short = Vec::new();
    for &(offset, row) in floors {
        for &(snr, least) in row {
            let s = share(id, offset, snr);
            if s < least {
                short.push(format!(
                    "{offset} Hz, {snr} dB: {:.0} % < {:.0} %",
                    s * 100.0,
                    least * 100.0
                ));
            }
        }
    }
    assert!(short.is_empty(), "{id}: {}", short.join("; "));
}

/// System Fusion behind its 12.5 kHz channel with the coarse carrier search: at +3.1 kHz the search moves the carrier while
/// the frame layer already decodes the clipped signal.
#[test]
fn ysf_keeps_its_blocks_at_carrier_offsets() {
    check_shares(
        "ysf",
        &[
            (0.0, &[(12.0, 0.99), (10.0, 0.99), (8.0, 0.92), (6.0, 0.51)]),
            (
                3100.0,
                &[(12.0, 0.99), (10.0, 0.98), (8.0, 0.92), (6.0, 0.49)],
            ),
            (
                -6000.0,
                &[(12.0, 0.90), (10.0, 0.89), (8.0, 0.83), (6.0, 0.38)],
            ),
        ],
    );
    let n = (6.0 * RATE) as usize;
    for seed in 1..=TRIALS {
        let p = protocol("ysf", &crate::behaviour::noise(0.1, seed, n));
        assert_eq!(p["blocks_ok"], 0, "noise alone, seed {seed}");
    }
}

/// TETRAPOL: the sequence estimator on the GMSK symbols, the frames read on over a missed sync, and the channel of 4.5 kHz
/// pass band and 7 kHz stop band.
#[test]
fn tetrapol_keeps_its_blocks_at_carrier_offsets() {
    check_shares(
        "tetrapol",
        &[
            (0.0, &[(9.0, 0.77), (7.5, 0.56)]),
            (3100.0, &[(7.5, 0.38)]),
            (8000.0, &[(7.5, 0.36)]),
        ],
    );
}

/// `x` turned by a carrier that moves linearly from `from_hz` to `to_hz` over its length.
fn chirp(x: &mut [Complex32], from_hz: f64, to_hz: f64) {
    let n = x.len() as f64;
    let mut phase = 0.0f64;
    for (i, v) in x.iter_mut().enumerate() {
        let f = from_hz + (to_hz - from_hz) * i as f64 / n;
        phase = (phase + std::f64::consts::TAU * f / RATE) % std::f64::consts::TAU;
        *v *= Complex32::from_polar(1.0, phase as f32);
    }
}

/// TETRAPOL at 15 dB while the carrier drifts from -2.5 to +2.5 kHz in 8 s loses no signalling frame of the clean run.
#[test]
fn tetrapol_loses_no_signalling_frame_while_the_carrier_drifts() {
    let signal = |snr: f64| {
        sdrkit_dev_test::Harness::new(desc("tetrapol"))
            .rate(RATE)
            .seed(3)
            .seconds(8.0)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, snr)
            .generate()
            .expect("the test signal")
    };
    let frames = |x: &[Complex32]| {
        protocol("tetrapol", x)["tetrapol_signalling_frames"]
            .as_u64()
            .unwrap_or(0)
    };
    // Two passes of the schedule and a part of a third: 10 signalling frames a pass.
    let clean = frames(&signal(80.0));
    assert_eq!(clean, 20);
    let mut x = signal(15.0);
    chirp(&mut x, -2500.0, 2500.0);
    assert_eq!(frames(&x), clean);
}

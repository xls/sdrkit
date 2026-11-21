//! The FSK analyser: seven cases of levels, rate, deviation, BT, offset and clock error; a sweep of the noise; noise, a bare
//! carrier and FM voice-like audio.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::fsk_analyser::gen::{GEN_BAUD, GEN_BT, GEN_DEVIATION_HZ, GEN_LEVELS};
use sdrkit_stack_meter::fsk_analyser::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    FskAnalyserEventRow, FskAnalyserFields, FskLevels, FSK_ANALYSER_EVENTS,
};
use std::f64::consts::TAU;

const TUNED: f64 = 433.92e6;
const RATE: f64 = 250e3;

/// Levels, rate, deviation, BT, offset, clock ppm.
type Case = (i64, f64, f64, f64, f64, f64);

fn signal(cnr: f64, c: Case) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
        .set(&GEN_LEVELS, c.0)
        .set(&GEN_BAUD, c.1)
        .set(&GEN_DEVIATION_HZ, c.2)
        .set(&GEN_BT, c.3)
        .set(&GEN_OFFSET_HZ, c.4)
        .set(&GEN_CLOCK_PPM, c.5)
}

fn fields(r: &Streamed) -> FskAnalyserFields {
    FskAnalyserFields::read(r.results.fields()).unwrap()
}

#[test]
fn six_cases_read_levels_rate_shift_centre_and_index() {
    check(&[
        (2, 1200.0, 600.0, 0.0, 0.0, 0.0),
        (2, 9600.0, 2400.0, 0.5, 5000.0, 150.0),
        (2, 4800.0, 1200.0, 0.0, -7300.0, 0.0),
        (2, 19200.0, 9600.0, 0.5, 2000.0, -200.0),
        (4, 4800.0, 1800.0, 0.0, 1000.0, 0.0),
        (4, 2400.0, 900.0, 0.5, -3000.0, 100.0),
    ]);
}

#[test]
fn a_slow_narrow_fsk_reads_its_rate() {
    check(&[(2, 600.0, 300.0, 0.0, 1500.0, 50.0)]);
}

fn check(cases: &[Case]) {
    for (k, &c) in cases.iter().enumerate() {
        let seed = 1 + k as u64 + c.1 as u64;
        let f = fields(&stream(
            &DESCRIPTOR,
            &signal(30.0, c).seed(seed),
            RATE,
            TUNED,
            12.0,
            seed,
            |_, _| {},
        ));
        let g = if c.3 > 0.0 { 0.93 } else { 1.0 };
        let rate = c.1 / (1.0 + c.5 * 1e-6);
        let shift = if c.0 == 4 { 2.0 * c.2 / 3.0 } else { 2.0 * c.2 } * g;
        let h = 2.0 * c.2 / c.1 * g;
        println!(
            "case {k}: {:?} at {:?} Bd, shift {:?}, centre {:?}, h {:?}, eye {:?}",
            f.levels, f.symbol_rate_bd, f.shift_hz, f.centre_hz, f.mod_index, f.eye_snr_db
        );
        let want = if c.0 == 4 {
            FskLevels::Four
        } else {
            FskLevels::Two
        };
        assert_eq!(f.levels, Some(want), "case {k}: {f:?}");
        assert!(
            f.symbol_rate_bd
                .is_some_and(|r| (r - rate).abs() <= 0.004 * rate + 1.0),
            "case {k}: {f:?}"
        );
        assert!(
            f.shift_hz
                .is_some_and(|s| (s.0 - shift).abs() <= 0.05 * shift),
            "case {k}: {f:?}"
        );
        assert!(
            f.centre_hz.is_some_and(|x| (x.0 - c.4).abs() <= 40.0),
            "case {k}: {f:?}"
        );
        assert!(
            f.mod_index
                .is_some_and(|x| (x - h).abs() <= 0.06 * h + 0.02),
            "case {k}: {f:?}"
        );
        if c.0 == 2 && (2.0 * c.2 / c.1 - 0.5).abs() < 1e-9 {
            assert_eq!(f.msk_like, Some(true), "case {k}: {f:?}");
        }
    }
}

#[test]
fn the_9600_bd_gfsk_holds_down_to_18_db() {
    for (k, cnr) in [30.0, 22.0, 18.0, 15.0, 12.0].into_iter().enumerate() {
        let seed = 11 + k as u64;
        let c = (2, 9600.0, 2400.0, 0.5, 0.0, 0.0);
        let f = fields(&stream(
            &DESCRIPTOR,
            &signal(cnr, c).seed(seed),
            RATE,
            TUNED,
            12.0,
            seed,
            |_, _| {},
        ));
        println!(
            "{cnr} dB: {:?} at {:?} Bd, shift {:?}",
            f.levels, f.symbol_rate_bd, f.shift_hz
        );
        if cnr >= 18.0 {
            let want = 4800.0 * 0.93;
            assert_eq!(f.levels, Some(FskLevels::Two), "{cnr}: {f:?}");
            assert!(
                f.shift_hz
                    .is_some_and(|s| (s.0 - want).abs() <= 0.08 * want),
                "{cnr}: {f:?}"
            );
            assert!(
                f.symbol_rate_bd.is_some_and(|r| (r - 9600.0).abs() <= 40.0),
                "{cnr}: {f:?}"
            );
        }
    }
}

/// 20 s of noise of power 0.01 with `add` added, sample by sample at time `t`.
fn other(seed: u64, add: impl Fn(f64) -> Complex32) -> Vec<FskAnalyserEventRow> {
    let h = Harness::new(&DESCRIPTOR)
        .set(&GEN_LEVEL_DBFS, -150.0)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
        .seed(seed);
    let r = stream(&DESCRIPTOR, &h, RATE, TUNED, 20.0, seed, |t0, x| {
        for (i, z) in x.iter_mut().enumerate() {
            *z += add(t0 + i as f64 / RATE);
        }
    });
    FSK_ANALYSER_EVENTS
        .rows::<FskAnalyserEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

#[test]
fn noise_a_carrier_and_fm_voice_give_no_row() {
    for seed in [21, 22] {
        assert!(other(seed, |_| Complex32::default()).is_empty());
        let carrier = other(seed, |t| {
            Complex32::from_polar(1.0, (TAU * (3000.0 * t).fract()) as f32)
        });
        assert!(carrier.is_empty(), "{carrier:?}");
        // The phase of 2 kHz + 3 kHz sin(2 pi 440 t) + 1.5 kHz sin(2 pi 1130 t).
        let voice = other(seed, |t| {
            let ph = TAU * 2000.0 * t
                - 3000.0 / 440.0 * (TAU * 440.0 * t).cos()
                - 1500.0 / 1130.0 * (TAU * 1130.0 * t).cos();
            Complex32::from_polar(1.0, ph.rem_euclid(TAU) as f32)
        });
        assert!(voice.is_empty(), "{voice:?}");
    }
}

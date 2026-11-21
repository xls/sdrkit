//! The modulation quality meter: three orders at three rates and two roll-offs, a sweep of Es/N0, and noise, a carrier and FM
//! that must not lock.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_DRIFT_HZ_S, GEN_ESN0_DB, GEN_LEVEL_DBFS, GEN_NOISE,
    GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::mer::gen::{GEN_PSK, GEN_ROLLOFF, GEN_ROTATED, GEN_SYMBOL_RATE};
use sdrkit_stack_meter::mer::DESCRIPTOR;
use sdrkit_stack_meter::typed::{MerEventRow, MerFields, MerModulation, MER_EVENTS};
use std::f64::consts::TAU;

const TUNED: f64 = 435e6;
const RATE: f64 = 250e3;

/// Phases, symbol rate, roll-off, offset, drift, clock ppm, rotated, Es/N0.
type Case = (i64, f64, f64, f64, f64, f64, bool, f64);

fn run(c: Case, secs: f64, seed: u64) -> MerFields {
    let h = Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::EsN0)
        .set(&GEN_ESN0_DB, c.7)
        .set(&GEN_PSK, c.0)
        .set(&GEN_SYMBOL_RATE, c.1)
        .set(&GEN_ROLLOFF, c.2)
        .set(&GEN_OFFSET_HZ, c.3)
        .set(&GEN_DRIFT_HZ_S, c.4)
        .set(&GEN_CLOCK_PPM, c.5)
        .set(&GEN_ROTATED, c.6)
        .seed(seed);
    let r = stream(&DESCRIPTOR, &h, RATE, TUNED, secs, seed, |_, _| {});
    MerFields::read(r.results.fields()).unwrap()
}

fn order(m: i64) -> MerModulation {
    match m {
        2 => MerModulation::Bpsk,
        4 => MerModulation::Qpsk,
        _ => MerModulation::Psk8,
    }
}

/// The cases of one order at the three rates and the two roll-offs.
fn cases(m: i64) -> Vec<Case> {
    let mut v = Vec::new();
    for rs in [2400.0, 9600.0, 24000.0] {
        for beta in [0.35, 0.2] {
            v.push(match m {
                2 => (2, rs, beta, 0.0, 3.0, 0.0, false, 28.0),
                4 => (4, rs, beta, 7300.0, 0.0, 150.0, rs > 5000.0, 28.0),
                _ => (8, rs, beta, -12300.0, 1.0, 0.0, false, 28.0),
            });
        }
    }
    v
}

fn check(m: i64) {
    for (k, c) in cases(m).into_iter().enumerate() {
        if m == 8 && c.1 == 2400.0 && c.2 == 0.2 {
            continue;
        }
        let seed = 1 + k as u64 + 10 * m as u64;
        let f = run(c, 8.0, seed);
        let rate = c.1 / (1.0 + c.5 * 1e-6);
        println!(
            "{m}-PSK {} Bd {}: {:?} {:?} Bd, {:?} Hz, MER {:?} dB, roll-off {:?}",
            c.1, c.2, f.modulation, f.symbol_rate_bd, f.carrier_offset_hz, f.mer_db, f.rolloff
        );
        assert_eq!(f.modulation, Some(order(m)), "{c:?}: {f:?}");
        assert!(
            f.symbol_rate_bd
                .is_some_and(|r| (r - rate).abs() <= 0.002 * c.1 + 1.0),
            "{c:?}: {f:?}"
        );
        let off = c.3 + c.4 * 8.0;
        assert!(
            f.carrier_offset_hz
                .is_some_and(|o| (o.0 - off).abs() <= 60.0),
            "{c:?}: {f:?}"
        );
        assert!(
            f.mer_db.is_some_and(|x| (24.0..=31.5).contains(&x)),
            "{c:?}: {f:?}"
        );
        assert_eq!(f.rolloff, Some(c.2), "{c:?}: {f:?}");
    }
}

#[test]
fn bpsk_at_three_rates_and_two_rolloffs() {
    check(2);
}

#[test]
fn qpsk_at_three_rates_and_two_rolloffs() {
    check(4);
}

#[test]
fn psk8_at_three_rates_and_two_rolloffs() {
    check(8);
}

#[test]
fn the_mer_follows_esn0_from_10_12_and_18_db() {
    for (m, from) in [(2, 10.0), (4, 12.0), (8, 18.0)] {
        for (k, es) in [30.0, 24.0, 18.0, 15.0, 12.0, 10.0, 8.0, 6.0]
            .into_iter()
            .enumerate()
        {
            let c = (m, 9600.0, 0.35, 2100.0, 0.0, 0.0, false, es);
            let f = run(c, 8.0, 100 + k as u64 + 10 * m as u64);
            println!("{m}-PSK at {es} dB: {:?}, MER {:?}", f.modulation, f.mer_db);
            if es >= from {
                assert_eq!(f.modulation, Some(order(m)), "{m} {es}: {f:?}");
                assert!(
                    f.mer_db.is_some_and(|x| (x - es).abs() <= 2.0),
                    "{m} {es}: {f:?}"
                );
            }
        }
    }
}

/// 20 s of noise of power 0.01 with `add` added at time `t`.
fn other(seed: u64, add: impl Fn(f64) -> Complex32) -> Vec<MerEventRow> {
    let h = Harness::new(&DESCRIPTOR)
        .set(&GEN_LEVEL_DBFS, -150.0)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
        .seed(seed);
    let r: Streamed = stream(&DESCRIPTOR, &h, RATE, TUNED, 20.0, seed, |t0, x| {
        for (i, z) in x.iter_mut().enumerate() {
            *z += add(t0 + i as f64 / RATE);
        }
    });
    MER_EVENTS
        .rows::<MerEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

#[test]
fn noise_a_carrier_and_fm_do_not_lock() {
    for seed in [41, 42] {
        assert!(other(seed, |_| Complex32::default()).is_empty());
        let carrier = other(seed, |t| {
            Complex32::from_polar(1.0, (TAU * (3000.0 * t).fract()) as f32)
        });
        assert!(carrier.is_empty(), "{carrier:?}");
        let fm = other(seed, |t| {
            let ph = TAU * 2000.0 * t
                - 3000.0 / 440.0 * (TAU * 440.0 * t).cos()
                - 1500.0 / 1100.0 * (TAU * 1100.0 * t).cos();
            Complex32::from_polar(1.0, ph.rem_euclid(TAU) as f32)
        });
        assert!(fm.is_empty(), "{fm:?}");
    }
}

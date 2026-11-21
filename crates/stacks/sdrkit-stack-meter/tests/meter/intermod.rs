//! The intermodulation finder: products of a cubic receiver at three shifts, an independent tone at a product frequency,
//! steady tones, a sweep of the cubic term, one signal, and noise that swamps the tones.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ};
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::intermod::gen::{
    GEN_EXTRA_AMP, GEN_EXTRA_HZ, GEN_F2_HZ, GEN_K, GEN_SWING_DB,
};
use sdrkit_stack_meter::intermod::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    IntermodEventRow, IntermodKind, IntermodProductRow, IntermodVerdict, INTERMOD_EVENTS,
    INTERMOD_PRODUCTS,
};

const TUNED: f64 = 145e6;
const RATE: f64 = 1e6;

/// The scene with noise of `power` per sample.
fn scene(power: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, 10.0 * power.log10())
}

fn run(h: Harness, seconds: f64, seed: u64) -> Streamed {
    stream(
        &DESCRIPTOR,
        &h.seed(seed),
        RATE,
        TUNED,
        seconds,
        seed,
        |_, _| {},
    )
}

fn rows(r: &Streamed) -> Vec<IntermodEventRow> {
    INTERMOD_EVENTS
        .rows::<IntermodEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

/// The rows within 300 Hz of `hz`.
fn near(r: &[IntermodEventRow], hz: f64) -> Vec<IntermodEventRow> {
    r.iter()
        .filter(|e| (e.frequency.0 - hz).abs() <= 300.0)
        .cloned()
        .collect()
}

#[test]
fn the_products_of_a_cubic_receiver_follow_the_law() {
    for (seed, shift) in [(1, 0.0), (2, 777.0), (3, -3333.0)] {
        let r = rows(&run(scene(2e-6).set(&GEN_OFFSET_HZ, shift), 70.0, seed));
        for (hz, kind) in [
            (50e3, IntermodKind::TwoAMinusB),
            (260e3, IntermodKind::TwoBMinusA),
        ] {
            let at = near(&r, TUNED + hz + shift);
            println!(
                "shift {shift}, {hz}: {:?}",
                at.iter()
                    .map(|e| (e.verdict, e.below_db, e.slope, e.correlation))
                    .collect::<Vec<_>>()
            );
            assert!(
                at.iter()
                    .any(|e| e.verdict == IntermodVerdict::Follows && e.kind == kind),
                "{at:?}"
            );
            assert!(
                at.iter().all(|e| e.verdict != IntermodVerdict::Independent),
                "{at:?}"
            );
        }
    }
}

#[test]
fn an_independent_tone_at_a_product_frequency_does_not_follow() {
    for seed in [11, 12] {
        let h = scene(2e-6)
            .set(&GEN_K, 0.0)
            .set(&GEN_EXTRA_HZ, 50e3)
            .set(&GEN_EXTRA_AMP, 0.004);
        let at = near(&rows(&run(h, 70.0, seed)), TUNED + 50e3);
        println!(
            "independent: {:?}",
            at.iter()
                .map(|e| (e.verdict, e.slope, e.correlation))
                .collect::<Vec<_>>()
        );
        assert!(
            at.iter().all(|e| e.verdict != IntermodVerdict::Follows),
            "{at:?}"
        );
        assert!(
            at.iter().any(|e| e.verdict == IntermodVerdict::Independent),
            "{at:?}"
        );
    }
}

#[test]
fn steady_tones_cannot_tell() {
    for seed in [21, 22] {
        let at = near(
            &rows(&run(scene(2e-6).set(&GEN_SWING_DB, 0.0), 40.0, seed)),
            TUNED + 50e3,
        );
        assert!(
            at.iter().any(|e| e.verdict == IntermodVerdict::CannotTell),
            "{at:?}"
        );
        assert!(
            at.iter().all(|e| e.verdict != IntermodVerdict::Follows),
            "{at:?}"
        );
    }
}

#[test]
fn a_sweep_of_the_cubic_term_follows_from_0_3() {
    for (j, k) in [2.0, 0.8, 0.3, 0.1, 0.03].into_iter().enumerate() {
        let r = run(scene(2e-6).set(&GEN_K, k), 70.0, 31 + j as u64);
        let at = near(&rows(&r), TUNED + 50e3);
        let p: Vec<IntermodProductRow> = INTERMOD_PRODUCTS
            .rows::<IntermodProductRow>(&r.results)
            .map(|(_, e)| e)
            .collect();
        let now = p
            .iter()
            .find(|e| (e.frequency.0 - TUNED - 50e3).abs() <= 300.0);
        println!("k {k}: {:?}", now.map(|e| (e.verdict, e.below_db)));
        if k >= 0.3 {
            assert!(
                at.iter().any(|e| e.verdict == IntermodVerdict::Follows),
                "{k}: {at:?}"
            );
        }
    }
}

#[test]
fn one_signal_and_swamping_noise_give_no_product() {
    for seed in [41, 42] {
        let r = rows(&run(scene(2e-6).set(&GEN_F2_HZ, 120_001.0), 60.0, seed));
        assert!(r.is_empty(), "{r:?}");
        let r = rows(&run(scene(1.0), 40.0, seed));
        assert!(r.is_empty(), "{r:?}");
    }
}

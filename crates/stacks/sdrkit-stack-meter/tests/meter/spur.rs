//! The spur and image finder: a faulty radio at two shifts, a real signal at the image place, steady signals, a sweep of
//! the image rejection, no faults and noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS};
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::spur::gen::{
    mismatch_of, GEN_DC, GEN_H2, GEN_H3, GEN_IMAGE_DB, GEN_IMAGE_PHASE, GEN_REAL_AMP, GEN_REAL_HZ,
    GEN_SHIFT_HZ, GEN_SIGNALS, GEN_SPUR_AMP, GEN_SWING_DB,
};
use sdrkit_stack_meter::spur::DESCRIPTOR;
use sdrkit_stack_meter::typed::{SpurEventKind, SpurEventRow, SpurVerdict, SPUR_EVENTS};

const TUNED: f64 = 145e6;
const RATE: f64 = 1e6;

fn scene(power: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, 10.0 * power.log10())
}

fn run(h: Harness, seconds: f64, seed: u64) -> Vec<SpurEventRow> {
    let r: Streamed = stream(
        &DESCRIPTOR,
        &h.seed(seed),
        RATE,
        TUNED,
        seconds,
        seed,
        |_, _| {},
    );
    SPUR_EVENTS
        .rows::<SpurEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

fn of(r: &[SpurEventRow], kind: SpurEventKind, hz: f64) -> Vec<SpurEventRow> {
    r.iter()
        .filter(|e| e.kind == kind && (e.frequency.0 - TUNED - hz).abs() <= 300.0)
        .cloned()
        .collect()
}

#[test]
fn the_image_rejection_of_the_mixer_is_the_one_asked_for() {
    let beta = Complex32::from_polar(0.02, 0.7);
    let (g, p) = mismatch_of(beta);
    let iq = sdrkit_algo_sim::front::IqImbalance::new(g, p).unwrap();
    assert!(
        (iq.image_rejection_db() + 20.0 * 0.02f64.log10()).abs() < 1e-3,
        "{}",
        iq.image_rejection_db()
    );
}

#[test]
fn a_faulty_radio_shows_its_images_harmonics_spur_and_dc() {
    for (seed, s) in [(1, 0.0), (2, 555.0)] {
        let r = run(scene(2e-6).set(&GEN_SHIFT_HZ, s), 70.0, seed);
        for hz in [-130e3 - s, 210e3 - s] {
            let i = of(&r, SpurEventKind::Image, hz);
            println!(
                "shift {s}, image at {hz}: {:?}",
                i.iter()
                    .map(|e| (e.verdict, e.below_db))
                    .collect::<Vec<_>>()
            );
            let f: Vec<&SpurEventRow> = i
                .iter()
                .filter(|e| e.verdict == Some(SpurVerdict::Follows))
                .collect();
            assert!(!f.is_empty(), "{r:?}");
            assert!(
                f.iter().all(|e| (e.below_db.unwrap() - 34.0).abs() <= 2.5),
                "{f:?}"
            );
        }
        let h2 = of(&r, SpurEventKind::SecondHarmonic, 260e3 + 2.0 * s);
        let h3 = of(&r, SpurEventKind::ThirdHarmonic, 390e3 + 3.0 * s);
        let fixed = of(&r, SpurEventKind::FixedSpur, 250e3);
        let dc = r.iter().filter(|e| e.kind == SpurEventKind::Dc).count();
        println!(
            "h2 {:?}, h3 {:?}, fixed {}, dc {dc}",
            h2.first().map(|e| e.below_db),
            h3.first().map(|e| e.below_db),
            fixed.len()
        );
        assert!(
            !h2.is_empty() && !h3.is_empty() && !fixed.is_empty() && dc == 1,
            "{r:?}"
        );
    }
}

#[test]
fn a_real_signal_at_the_image_place_is_independent() {
    for seed in [11, 12] {
        let h = scene(2e-6)
            .set(&GEN_IMAGE_DB, 0.0)
            .set(&GEN_REAL_HZ, -130e3)
            .set(&GEN_REAL_AMP, 0.004);
        let i = of(&run(h, 70.0, seed), SpurEventKind::Image, -130e3);
        println!(
            "real: {:?}",
            i.iter()
                .map(|e| (e.verdict, e.slope, e.correlation))
                .collect::<Vec<_>>()
        );
        assert!(
            i.iter().all(|e| e.verdict != Some(SpurVerdict::Follows)),
            "{i:?}"
        );
        assert!(
            i.iter()
                .any(|e| e.verdict == Some(SpurVerdict::Independent)),
            "{i:?}"
        );
    }
}

#[test]
fn steady_signals_cannot_tell() {
    for seed in [21, 22] {
        let i = of(
            &run(scene(2e-6).set(&GEN_SWING_DB, 0.0), 40.0, seed),
            SpurEventKind::Image,
            -130e3,
        );
        assert!(
            i.iter().any(|e| e.verdict == Some(SpurVerdict::CannotTell)),
            "{i:?}"
        );
    }
}

#[test]
fn a_sweep_of_the_image_rejection_reads_it_to_50_db() {
    for (j, ir) in [20.0, 30.0, 40.0, 50.0, 55.0].into_iter().enumerate() {
        let h = scene(2e-6)
            .set(&GEN_IMAGE_DB, ir)
            .set(&GEN_IMAGE_PHASE, 0.3);
        let i = of(&run(h, 70.0, 31 + j as u64), SpurEventKind::Image, -130e3);
        println!(
            "image rejection {ir}: {:?}",
            i.iter()
                .map(|e| (e.verdict, e.below_db))
                .collect::<Vec<_>>()
        );
        if ir <= 50.0 {
            assert!(
                !i.is_empty() && i.iter().all(|e| (e.below_db.unwrap() - ir).abs() <= 2.5),
                "{ir}: {i:?}"
            );
        }
    }
}

#[test]
fn no_faults_and_noise_alone_give_no_row() {
    for seed in [41, 42] {
        let h = scene(2e-6)
            .set(&GEN_IMAGE_DB, 0.0)
            .set(&GEN_DC, 0.0)
            .set(&GEN_H2, 0.0)
            .set(&GEN_H3, 0.0)
            .set(&GEN_SPUR_AMP, 0.0);
        let r = run(h, 60.0, seed);
        assert!(r.is_empty(), "{r:?}");
        let h = scene(1.0)
            .set(&GEN_SIGNALS, "")
            .set(&GEN_DC, 0.0)
            .set(&GEN_SPUR_AMP, 0.0);
        let r = run(h, 40.0, seed);
        assert!(r.is_empty(), "{r:?}");
    }
}

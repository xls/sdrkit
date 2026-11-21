//! The noise floor logger: added band-limited noise beside a carrier; a step of the gain; a slow drift; steady noise.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::noise_floor::gen::GEN_EXTRA_POWER;
use sdrkit_stack_meter::noise_floor::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    NoiseFloorChange, NoiseFloorEvent, NoiseFloorFields, NOISE_FLOOR_EVENT,
};

const CENTRE: f64 = 145e6;

/// White noise of 0.001 per sample (-30 dBFS) under the test signal.
fn h() -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -30.0)
}

fn rows(r: &Streamed) -> Vec<NoiseFloorEvent> {
    r.events
        .iter()
        .filter_map(|e| NOISE_FLOOR_EVENT.read::<NoiseFloorEvent>(e))
        .collect()
}

fn scaled(x: &mut [Complex32], t0: f64, rate: f64, gain_db: impl Fn(f64) -> f64) {
    for (i, z) in x.iter_mut().enumerate() {
        *z *= 10f64.powf(gain_db(t0 + i as f64 / rate) / 20.0) as f32;
    }
}

#[test]
fn added_noise_beside_a_carrier_rises_and_falls_back_without_a_whole_band_row() {
    for seed in [1, 2] {
        let r = stream(
            &DESCRIPTOR,
            &h().seed(seed),
            2e6,
            CENTRE,
            200.0,
            seed,
            |_, _| {},
        );
        let rows = rows(&r);
        let rises: Vec<&NoiseFloorEvent> = rows
            .iter()
            .filter(|e| e.kind == NoiseFloorChange::Rise)
            .collect();
        let backs = rows
            .iter()
            .filter(|e| e.kind == NoiseFloorChange::Back)
            .count();
        assert!(!rises.is_empty(), "seed {seed}");
        assert_eq!(backs, rises.len(), "seed {seed}: every rise ends");
        assert!(
            rows.iter().all(|e| e.kind != NoiseFloorChange::WholeBand),
            "seed {seed}"
        );
        let top = rows
            .iter()
            .filter(|e| e.kind == NoiseFloorChange::Back)
            .max_by(|a, b| a.rise_db.total_cmp(&b.rise_db))
            .unwrap();
        let f = top.frequency.unwrap().0;
        println!(
            "seed {seed}: {} rises, largest {:.1} dB at {:.0} Hz",
            rises.len(),
            top.rise_db,
            f - CENTRE
        );
        assert!(
            (f - 145.3e6).abs() <= 150e3 && top.rise_db > 8.0,
            "seed {seed}: {top:?}"
        );
        assert!(
            rises
                .iter()
                .all(|e| (e.frequency.unwrap().0 - 144.6e6).abs() > 150e3),
            "seed {seed}: a rise at the carrier"
        );
    }
}

#[test]
fn a_gain_step_of_6_db_is_one_whole_band_row_and_no_rise() {
    let r = stream(
        &DESCRIPTOR,
        &h().set(&GEN_EXTRA_POWER, 0.0),
        2e6,
        CENTRE,
        180.0,
        3,
        |t, x| scaled(x, t, 2e6, |t| if t >= 120.0 { 6.0 } else { 0.0 }),
    );
    let rows = rows(&r);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].kind, NoiseFloorChange::WholeBand);
    assert!((rows[0].rise_db - 6.0).abs() < 0.5, "{rows:?}");
}

#[test]
fn a_drift_of_half_a_db_per_minute_gives_no_row() {
    let r = stream(
        &DESCRIPTOR,
        &h().set(&GEN_EXTRA_POWER, 0.0),
        2e6,
        CENTRE,
        180.0,
        4,
        |t, x| scaled(x, t, 2e6, |t| 0.5 * t / 60.0),
    );
    assert!(rows(&r).is_empty(), "{:?}", rows(&r));
}

#[test]
fn steady_noise_reads_its_floor_and_gives_no_row() {
    for seed in [5, 6, 7] {
        let r = stream(
            &DESCRIPTOR,
            &h().set(&GEN_EXTRA_POWER, 0.0),
            2e6,
            CENTRE,
            60.0,
            seed,
            |_, _| {},
        );
        let f = NoiseFloorFields::read(r.results.fields()).unwrap();
        let want = 10.0 * (0.001f64 / 4096.0).log10();
        let got = f.floor_db.unwrap();
        assert!(
            (got - want).abs() <= 0.3,
            "seed {seed}: {got:.2} dB, {want:.2} expected"
        );
        assert!(rows(&r).is_empty(), "seed {seed}");
        assert_eq!(f.raised, 0);
    }
}

//! The persistent carrier finder: the scene of five carriers with and without a radio shift; a carrier that goes and comes
//! back; a sweep of the level; noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::birdie::gen::{GEN_FM_AMP, GEN_TONES};
use sdrkit_stack_meter::birdie::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    BirdieCarrierRow, BirdieChange, BirdieClass, BirdieEvent, BIRDIE_CARRIERS, BIRDIE_EVENT,
};

const CENTRE: f64 = 145e6;

/// Noise of 2e-6 per sample under the test signal.
fn h(dbfs: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, dbfs)
}

fn run(h: Harness, secs: f64, seed: u64) -> Streamed {
    stream(
        &DESCRIPTOR,
        &h.seed(seed),
        1e6,
        CENTRE,
        secs,
        seed,
        |_, _| {},
    )
}

fn listed(r: &Streamed) -> Vec<BirdieCarrierRow> {
    BIRDIE_CARRIERS
        .rows::<BirdieCarrierRow>(&r.results)
        .map(|(_, c)| c)
        .collect()
}

fn rows(r: &Streamed) -> Vec<BirdieEvent> {
    r.events
        .iter()
        .filter_map(|e| BIRDIE_EVENT.read::<BirdieEvent>(e))
        .collect()
}

fn near(l: &[BirdieCarrierRow], hz: f64, tol: f64) -> Option<BirdieCarrierRow> {
    l.iter()
        .find(|c| (c.frequency.0 - CENTRE - hz).abs() <= tol)
        .cloned()
}

fn scene(shift: f64) {
    let r = run(h(-56.99).set(&GEN_OFFSET_HZ, shift), 100.0, 1);
    let l = listed(&r);
    let at = format!("shift {shift} Hz");
    let end = 123_456.7 + 0.5 * 100.0 + shift;
    let bare = near(&l, end, 40.0).unwrap_or_else(|| panic!("{at}: drifting carrier: {l:?}"));
    assert!(
        (bare.drift_hz_min - 30.0).abs() <= 3.0 && bare.class == BirdieClass::Bare,
        "{at}: {bare:?}"
    );
    let steady =
        near(&l, -75_000.3 + shift, 6.0).unwrap_or_else(|| panic!("{at}: steady carrier: {l:?}"));
    assert!(
        steady.class == BirdieClass::Bare && steady.drift_hz_min.abs() < 1.0,
        "{at}: {steady:?}"
    );
    assert!(
        near(&l, 300e3 + shift, 8.0).is_some(),
        "{at}: weak carrier: {l:?}"
    );
    assert!(
        near(&l, -240e3 + shift, 400.0).is_none(),
        "{at}: the keyed carrier is listed"
    );
    if let Some(fm) = near(&l, 450e3 + shift, 5e3) {
        assert_eq!(fm.class, BirdieClass::Modulated, "{at}: {fm:?}");
    }
    let new = rows(&r)
        .iter()
        .filter(|e| e.kind == BirdieChange::New)
        .count();
    assert!(new >= 3, "{at}: {new} carrier rows");
    println!(
        "{at}: drifting {:+.1} Hz {:.1} Hz/min, steady {:+.1} Hz, {} listed",
        bare.frequency.0 - CENTRE - end,
        bare.drift_hz_min,
        steady.frequency.0 - CENTRE + 75_000.3 - shift,
        l.len()
    );
}

#[test]
fn the_scene_without_a_shift() {
    scene(0.0);
}

#[test]
fn the_scene_shifted_by_1234_hz() {
    scene(1234.0);
}

#[test]
fn a_carrier_that_goes_and_comes_back_is_one_gone_row_and_two_carrier_rows() {
    let tones = "90000, 0.05, 0, 1, 0, 0, 0, 60; 90000, 0.05, 0, 1, 0, 0, 100, 1e9";
    let r = run(
        h(-56.99).set(&GEN_TONES, tones).set(&GEN_FM_AMP, 0.0),
        160.0,
        2,
    );
    let rows = rows(&r);
    let gone = rows.iter().filter(|e| e.kind == BirdieChange::Gone).count();
    let back = rows.iter().filter(|e| e.kind != BirdieChange::Gone).count();
    assert_eq!((gone, back), (1, 2), "{rows:?}");
}

#[test]
fn a_carrier_is_found_from_20_9_db_over_the_noise() {
    for amp in [0.0005, 0.0003, 0.0002, 0.00015, 0.0001] {
        let tones = format!("200000, {amp}");
        let r = run(
            h(-56.99).set(&GEN_TONES, &tones).set(&GEN_FM_AMP, 0.0),
            60.0,
            3,
        );
        let db = 10.0 * (amp * amp * 8192.0f64 * 8192.0 / (2e-6 * 6144.0)).log10();
        let found = near(&listed(&r), 200e3, 8.0).is_some();
        println!("{amp} ({db:.1} dB): found {found}");
        if amp >= 0.00015 {
            assert!(found, "{amp}");
        }
    }
}

#[test]
fn noise_alone_for_150_s_gives_no_row() {
    let r = run(h(0.0).set(&GEN_TONES, "").set(&GEN_FM_AMP, 0.0), 150.0, 4);
    assert!(rows(&r).is_empty(), "{:?}", rows(&r));
    assert_eq!(
        sdrkit_stack_meter::typed::BirdieFields::read(r.results.fields())
            .unwrap()
            .carriers,
        0
    );
}

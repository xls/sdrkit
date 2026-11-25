//! The noise locator: impulses at five rates and two input rates; a drifting mains with random impulses; Poisson impulses;
//! noise alone; a sweep of the impulse level.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::noise_locator::gen::{
    GEN_DROP, GEN_PEAK_DB, GEN_RANDOM_DB, GEN_RANDOM_RATE, GEN_RATE_DRIFT_HZ_S, GEN_RATE_HZ,
};
use sdrkit_stack_meter::noise_locator::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    NoiseLocatorChange, NoiseLocatorClass, NoiseLocatorFields, NoiseLocatorPatternEvent,
    NOISE_LOCATOR_PATTERN,
};

const CENTRE: f64 = 7.1e6;

/// Complex Gaussian noise of sigma 1 per quadrature (power 2, +3.01 dBFS) under the impulses.
fn h(rate_hz: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, 10.0 * 2f64.log10())
        .set(&GEN_RATE_HZ, rate_hz)
}

fn run(h: Harness, rate: f64, secs: f64, seed: u64) -> Streamed {
    stream(
        &DESCRIPTOR,
        &h.seed(seed),
        rate,
        CENTRE,
        secs,
        seed,
        |_, _| {},
    )
}

fn rows(r: &Streamed) -> Vec<NoiseLocatorPatternEvent> {
    r.events
        .iter()
        .filter_map(|e| NOISE_LOCATOR_PATTERN.read::<NoiseLocatorPatternEvent>(e))
        .collect()
}

#[test]
fn five_impulse_rates_at_two_input_rates_give_one_pattern_of_the_right_class() {
    let cases = [
        (100.0, NoiseLocatorClass::TwiceMains, Some(50.0)),
        (120.0, NoiseLocatorClass::TwiceMains, Some(60.0)),
        (50.0, NoiseLocatorClass::Mains, Some(50.0)),
        (60.0, NoiseLocatorClass::Mains, Some(60.0)),
        (173.0, NoiseLocatorClass::Steady, None),
    ];
    for rate in [250e3, 1e6] {
        for (k, (hz, class, mains)) in cases.into_iter().enumerate() {
            let r = run(h(hz), rate, 20.0, 1 + k as u64);
            let rows = rows(&r);
            let f = NoiseLocatorFields::read(r.results.fields()).unwrap();
            let at = format!("{hz} Hz at {rate} sps");
            assert_eq!(rows.len(), 1, "{at}: {rows:?}");
            assert_eq!(
                (rows[0].kind, f.class, f.mains_found_hz),
                (NoiseLocatorChange::Found, class, mains),
                "{at}"
            );
            let got = f.repetition_hz.unwrap();
            assert!((got - hz).abs() <= 0.005 * hz + 0.15, "{at}: {got}");
            println!(
                "{at}: {got:.3} Hz, {:.1} impulses/s, {:.1} dB",
                f.impulse_rate.unwrap(),
                f.impulse_level_db.unwrap()
            );
        }
    }
}

#[test]
fn a_drifting_mains_with_random_impulses_is_twice_the_50_hz() {
    let hh = h(99.6)
        .set(&GEN_RATE_DRIFT_HZ_S, 0.04)
        .set(&GEN_DROP, 0.3)
        .set(&GEN_RANDOM_RATE, 25.0)
        .set(&GEN_RANDOM_DB, 12.0);
    let r = run(hh, 250e3, 20.0, 7);
    let f = NoiseLocatorFields::read(r.results.fields()).unwrap();
    assert_eq!(
        (f.class, f.mains_found_hz),
        (NoiseLocatorClass::TwiceMains, Some(50.0)),
        "{:?}",
        rows(&r)
    );
}

#[test]
fn poisson_impulses_have_no_repetition() {
    for seed in [8, 9] {
        let hh = h(0.0)
            .set(&GEN_RANDOM_RATE, 100.0)
            .set(&GEN_RANDOM_DB, 14.0);
        let r = run(hh, 250e3, 25.0, seed);
        assert!(rows(&r).is_empty(), "seed {seed}: {:?}", rows(&r));
        let text = &r.results.status().summary;
        assert!(text.contains("without a repetition"), "seed {seed}: {text}");
    }
}

#[test]
fn noise_alone_for_15_minutes_gives_no_pattern() {
    let mut impulses = 0;
    for seed in 10..25 {
        let r = run(h(0.0), 96e3, 60.0, seed);
        assert!(rows(&r).is_empty(), "seed {seed}: {:?}", rows(&r));
        impulses += NoiseLocatorFields::read(r.results.fields())
            .unwrap()
            .impulses;
    }
    println!("{impulses} impulses in 15 minutes");
}

#[test]
fn impulses_are_found_from_14_db() {
    for db in [25.0, 18.0, 14.0, 10.0, 6.0, 3.0, 0.0, -3.0] {
        let found = (0..4)
            .filter(|k| {
                let hh = h(100.0).set(&GEN_DROP, 0.2).set(&GEN_PEAK_DB, db);
                let f = NoiseLocatorFields::read(run(hh, 250e3, 25.0, 30 + k).results.fields())
                    .unwrap();
                f.class == NoiseLocatorClass::TwiceMains
            })
            .count();
        println!("{db} dB: {found} of 4");
        if db >= 14.0 {
            assert_eq!(found, 4, "{db} dB");
        }
    }
}

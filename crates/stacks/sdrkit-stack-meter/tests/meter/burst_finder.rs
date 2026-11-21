//! The burst finder: the scene of four sources at two rates with and without a radio error; a sweep of the noise; noise
//! alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::burst_finder::gen::GEN_BURSTS;
use sdrkit_stack_meter::burst_finder::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    BurstClass, BurstFoundEvent, BurstRepeatingEvent, BURST_FOUND, BURST_REPEATING,
};

const CENTRE: f64 = 433.92e6;

fn at_cnr(cnr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
}

fn found(r: &Streamed) -> Vec<BurstFoundEvent> {
    r.events
        .iter()
        .filter_map(|e| BURST_FOUND.read::<BurstFoundEvent>(e))
        .collect()
}

fn near(b: &[BurstFoundEvent], hz: f64, tol: f64) -> Vec<BurstFoundEvent> {
    b.iter()
        .filter(|b| (b.frequency.0 - CENTRE - hz).abs() <= tol)
        .cloned()
        .collect()
}

fn scene(rate: f64, error: f64) {
    let r = stream(
        &DESCRIPTOR,
        &at_cnr(30.0).set(&GEN_OFFSET_HZ, error),
        rate,
        CENTRE,
        6.5,
        2,
        |_, _| {},
    );
    let at = format!("{rate} sps, {error} Hz");
    let all = found(&r);
    let fob = near(&all, 120e3 + error, 12e3);
    assert_eq!(fob.len(), 24, "{at}: key fob {fob:?}");
    assert!(fob.iter().all(|b| b.class == BurstClass::Narrow), "{at}");
    let mean = fob.iter().map(|b| b.length_ms).sum::<f64>() / 24.0;
    assert!((mean - 3.2).abs() <= 0.4, "{at}: key fob length {mean}");
    let rep: Vec<BurstRepeatingEvent> = r
        .events
        .iter()
        .filter_map(|e| BURST_REPEATING.read(e))
        .collect();
    assert!(
        rep.iter()
            .any(|s| (s.frequency.0 - CENTRE - 120e3 - error).abs() <= 12e3
                && (s.period_s.0 - 0.25).abs() <= 0.005),
        "{at}: {rep:?}"
    );
    assert_eq!(near(&all, -310e3 + error, 12e3).len(), 9, "{at}: sensor");
    assert!(
        rep.iter()
            .all(|s| (s.frequency.0 - CENTRE + 310e3).abs() > 30e3),
        "{at}: the sensor repeats"
    );
    let wide = near(&all, 500e3 + error, 60e3);
    assert_eq!(wide.len(), 6, "{at}: wide {wide:?}");
    assert!(
        wide.iter().all(|b| b.class == BurstClass::Wide),
        "{at}: {wide:?}"
    );
    let fast = near(&all, -700e3 + error, 12e3).len();
    assert!(fast >= 4, "{at}: fast {fast}");
    println!(
        "{at}: {} bursts, key fob {mean:.2} ms, {fast} fast, {} repeating",
        all.len(),
        rep.len()
    );
}

#[test]
fn the_scene_at_2_msps() {
    scene(2e6, 0.0);
    scene(2e6, 2500.0);
}

#[test]
fn the_scene_at_4_msps() {
    scene(4e6, 0.0);
    scene(4e6, 2500.0);
}

#[test]
fn ten_bursts_are_found_without_an_extra_one_from_14_db() {
    let bursts = "0.1, 0.002, 200000, 0, 1, 10, 0.4";
    for cnr in [30.0, 20.0, 14.0, 10.0, 7.0, 4.0] {
        let r = stream(
            &DESCRIPTOR,
            &at_cnr(cnr).set(&GEN_BURSTS, bursts),
            2e6,
            CENTRE,
            4.5,
            3,
            |_, _| {},
        );
        let all = found(&r);
        let hits = near(&all, 200e3, 12e3).len();
        println!("{cnr} dB: {hits} of 10, {} in all", all.len());
        if cnr >= 14.0 {
            assert_eq!((hits, all.len()), (10, 10), "{cnr} dB: {all:?}");
        }
    }
}

#[test]
fn noise_alone_for_120_s_gives_no_burst() {
    for seed in [4, 5] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_BURSTS, "")
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 2e6, CENTRE, 120.0, seed, |_, _| {});
        assert_eq!(found(&r).len(), 0, "seed {seed}: {:?}", found(&r));
    }
}

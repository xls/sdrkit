//! The frequency accuracy meter: offsets at three rates, a drift, a step, a gap, a sweep of C/N0, white frequency noise for
//! the Allan deviation, and noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_CN0_DBHZ, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_OFFSET_HZ};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::freq_accuracy::gen::{GEN_FM_NOISE_HZ, GEN_GAPS, GEN_STEPS};
use sdrkit_stack_meter::freq_accuracy::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    FreqAccuracyAllanRow, FreqAccuracyEvent, FreqAccuracyFields, FreqAccuracyKind,
    FREQ_ACCURACY_ALLAN, FREQ_ACCURACY_EVENT,
};

const TUNED: f64 = 10e6;

fn at(cn0: f64, offset: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Cn0)
        .set(&GEN_CN0_DBHZ, cn0)
        .set(&GEN_OFFSET_HZ, offset)
}

fn rows(r: &Streamed) -> Vec<FreqAccuracyEvent> {
    r.events
        .iter()
        .filter_map(|e| FREQ_ACCURACY_EVENT.read::<FreqAccuracyEvent>(e))
        .collect()
}

fn fields(r: &Streamed) -> FreqAccuracyFields {
    FreqAccuracyFields::read(r.results.fields()).unwrap()
}

#[test]
fn offsets_at_three_rates_read_within_a_tenth_of_a_hertz() {
    let mut seed = 0;
    for rate in [48e3, 96e3, 250e3] {
        for off in [0.0, 12.34, -1730.5, 2400.0] {
            seed += 1;
            let r = stream(
                &DESCRIPTOR,
                &at(60.0, off).seed(seed),
                rate,
                TUNED,
                20.0,
                seed,
                |_, _| {},
            );
            let f = fields(&r);
            let o = f.offset_hz.unwrap().0;
            let ppm = f.error_ppm.unwrap();
            let cn0 = f.cn0_dbhz.unwrap();
            println!("{rate} sps, {off} Hz: {o:.3} Hz, {ppm:.5} ppm, {cn0:.2} dB-Hz");
            assert!((o - off).abs() <= 0.1, "{rate} {off}: {o}");
            assert!(
                (ppm - off / TUNED * 1e6).abs() <= 0.01,
                "{rate} {off}: {ppm}"
            );
            assert!((cn0 - 60.0).abs() <= 2.0, "{rate} {off}: {cn0}");
        }
    }
}

#[test]
fn a_drift_of_1_hz_per_second_reads_360_ppm_per_hour() {
    for seed in [1, 2] {
        let h = at(60.0, 100.0).set(&GEN_DRIFT_HZ_S, 1.0).seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 120.0, seed, |_, _| {});
        let d = fields(&r).drift_ppm_h.unwrap();
        let want = 1.0 * 3600.0 / TUNED * 1e6;
        println!("seed {seed}: {d:.4} ppm/h ({want})");
        assert!((d - want).abs() <= 0.05 * want + 0.01, "{d}");
    }
}

#[test]
fn a_step_of_3_hz_is_one_step_row_and_a_gap_is_lost_and_returned() {
    for seed in [3, 4] {
        let h = at(60.0, 50.0).set(&GEN_STEPS, "40, 3").seed(seed);
        let r = rows(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 80.0, seed, |_, _| {}));
        let steps: Vec<_> = r
            .iter()
            .filter(|e| e.kind == FreqAccuracyKind::Step)
            .collect();
        assert_eq!(steps.len(), 1, "{steps:?}");
        let s = steps[0].step_hz.unwrap().0;
        assert!(
            (s - 3.0).abs() < 0.3 && (steps[0].time_s.0 - 41.0).abs() < 2.5,
            "{steps:?}"
        );
        let h = at(60.0, 50.0).set(&GEN_GAPS, "30, 50").seed(seed);
        let r = rows(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 80.0, seed, |_, _| {}));
        let kinds: Vec<_> = r
            .iter()
            .map(|e| e.kind)
            .filter(|k| matches!(k, FreqAccuracyKind::Lost | FreqAccuracyKind::Returned))
            .collect();
        assert_eq!(
            kinds,
            vec![FreqAccuracyKind::Lost, FreqAccuracyKind::Returned],
            "{r:?}"
        );
        assert!(r.iter().all(|e| e.kind != FreqAccuracyKind::Step), "{r:?}");
    }
}

#[test]
fn the_offset_holds_down_to_35_dbhz() {
    for (k, cn0) in [60.0, 50.0, 40.0, 35.0, 30.0, 27.0].into_iter().enumerate() {
        let seed = 10 + k as u64;
        let r = stream(
            &DESCRIPTOR,
            &at(cn0, 37.0).seed(seed),
            48e3,
            TUNED,
            60.0,
            seed,
            |_, _| {},
        );
        let f = fields(&r);
        println!("{cn0} dB-Hz: {:?} Hz, {:?} dB-Hz", f.offset_hz, f.cn0_dbhz);
        if cn0 >= 35.0 {
            let o = f.offset_hz.unwrap().0;
            assert!((o - 37.0).abs() <= 0.3, "{cn0}: {o}");
        }
    }
}

#[test]
fn white_frequency_noise_of_02_hz_gives_an_allan_deviation_of_2e8() {
    for seed in [5, 6] {
        let h = at(70.0, 37.0).set(&GEN_FM_NOISE_HZ, 0.2).seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 800.0, seed, |_, _| {});
        let a: Vec<FreqAccuracyAllanRow> = FREQ_ACCURACY_ALLAN
            .rows::<FreqAccuracyAllanRow>(&r.results)
            .map(|(_, a)| a)
            .collect();
        let first = a
            .iter()
            .min_by(|x, y| x.tau_s.0.total_cmp(&y.tau_s.0))
            .unwrap();
        let adev = first.adev.unwrap();
        println!(
            "seed {seed}: {adev:.3e} at {} s, {} rows",
            first.tau_s.0,
            a.len()
        );
        assert!((1.4e-8..=2.6e-8).contains(&adev), "{adev}");
    }
}

#[test]
fn noise_alone_for_10_minutes_gives_no_row() {
    for seed in [7, 8] {
        let h = at(60.0, 0.0).set(&GEN_GAPS, "0, 1e9").seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 600.0, seed, |_, _| {});
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
        assert_eq!(fields(&r).offset_hz, None);
    }
}

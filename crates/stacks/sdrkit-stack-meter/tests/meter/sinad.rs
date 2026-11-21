//! The SINAD meter: three rates and two noise levels against theory, distortion, a sweep of the noise, a crossing of the
//! mark, noise alone and a bare carrier.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::sinad::gen::{GEN_DEVIATION_AFTER, GEN_DEVIATION_HZ, GEN_H2, GEN_H3};
use sdrkit_stack_meter::sinad::DESCRIPTOR;
use sdrkit_stack_meter::typed::{SinadCrossing, SinadEventRow, SinadFields, SINAD_EVENTS};

const TUNED: f64 = 146.52e6;

fn at(cnr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
}

/// SINAD of a tone of `dev` Hz with the noise of a CNR in 16 kHz in 300 to 3000 Hz, flat.
fn theory(dev: f64, cnr: f64) -> f64 {
    let s = dev * dev / 2.0;
    let n = 10f64.powf(-cnr / 10.0) / 16e3 * (3000f64.powi(3) - 300f64.powi(3)) / 3.0;
    10.0 * ((s + n) / n).log10()
}

fn rows(r: &Streamed) -> Vec<SinadEventRow> {
    SINAD_EVENTS
        .rows::<SinadEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

fn fields(r: &Streamed) -> SinadFields {
    SinadFields::read(r.results.fields()).unwrap()
}

#[test]
fn three_rates_at_30_and_20_db_read_the_sinad_of_theory() {
    let mut seed = 0;
    for (rate, off) in [(48e3, 3000.0), (96e3, -1500.0), (250e3, 2500.0)] {
        for cnr in [30.0, 20.0] {
            seed += 1;
            let h = at(cnr)
                .set(&GEN_OFFSET_HZ, off)
                .set(&GEN_DRIFT_HZ_S, 5.0)
                .seed(seed);
            let f = fields(&stream(&DESCRIPTOR, &h, rate, TUNED, 10.0, seed, |_, _| {}));
            let (s, d, t, o) = (
                f.sinad_db.unwrap(),
                f.deviation_khz.unwrap(),
                f.tone_hz.unwrap().0,
                f.carrier_offset_hz.unwrap().0,
            );
            let want = theory(3000.0, cnr);
            println!("{rate} sps, {cnr} dB: SINAD {s:.2} (theory {want:.2}), {d:.4} kHz, {t:.2} Hz, offset {o:.1}");
            assert!((s - want).abs() <= 1.5, "{rate} {cnr}: {s} against {want}");
            assert!(
                (d - 3.0).abs() <= 0.1 && (t - 1000.0).abs() <= 1.0,
                "{rate} {cnr}: {d} {t}"
            );
            assert!((o - (off + 25.0)).abs() <= 60.0, "{rate} {cnr}: {o}");
        }
    }
}

#[test]
fn harmonics_of_2_and_1_percent_read_a_thd_of_224_percent() {
    for seed in [11, 12] {
        let h = at(60.0).set(&GEN_H2, 0.02).set(&GEN_H3, 0.01).seed(seed);
        let f = fields(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 10.0, seed, |_, _| {}));
        let (thd, s) = (f.thd_pct.unwrap(), f.sinad_db.unwrap());
        println!("seed {seed}: THD {thd:.3} %, SINAD {s:.2} dB");
        assert!(
            (thd - 2.236).abs() <= 0.2 && (s - 33.0).abs() <= 1.5,
            "{thd} {s}"
        );
    }
}

#[test]
fn a_sweep_of_the_noise_follows_theory_from_10_db() {
    for (k, cnr) in [25.0, 18.0, 14.0, 10.0, 8.0, 6.0].into_iter().enumerate() {
        let seed = 20 + k as u64;
        let f = fields(&stream(
            &DESCRIPTOR,
            &at(cnr).seed(seed),
            48e3,
            TUNED,
            10.0,
            seed,
            |_, _| {},
        ));
        let want = theory(3000.0, cnr);
        println!("{cnr} dB: SINAD {:?} (theory {want:.2})", f.sinad_db);
        if cnr >= 10.0 {
            let s = f.sinad_db.unwrap();
            assert!((s - want).abs() <= 2.0, "{cnr}: {s} against {want}");
        }
    }
}

#[test]
fn a_fall_of_the_deviation_is_one_row_under_the_mark() {
    for seed in [31, 32] {
        let h = at(14.0).set(&GEN_DEVIATION_AFTER, "8, 700").seed(seed);
        let r = rows(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 18.0, seed, |_, _| {}));
        let fell = r
            .iter()
            .filter(|e| e.crossing == SinadCrossing::Fell)
            .count();
        assert_eq!(fell, 1, "{r:?}");
    }
}

#[test]
fn noise_alone_and_a_bare_carrier_give_no_row() {
    for seed in [41, 42, 43] {
        let h = at(30.0)
            .set(&GEN_DEVIATION_HZ, 0.0)
            .set(&GEN_OFFSET_HZ, 500.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 10.0, seed, |_, _| {});
        assert!(
            rows(&r).is_empty() && fields(&r).sinad_db.is_none(),
            "{:?}",
            rows(&r)
        );
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
            .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -20.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 10.0, seed, |_, _| {});
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
    }
}

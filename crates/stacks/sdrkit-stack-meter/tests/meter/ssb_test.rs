//! The SSB test meter: one tone and two tones at three rates on both sidebands with a tuning error, unequal tones, a sweep of
//! the noise, noise alone and a carrier alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::ssb_test::gen::{GEN_CARRIER_DBC, GEN_SIDEBAND, GEN_TONES};
use sdrkit_stack_meter::ssb_test::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    SsbSideband, SsbTestEventRow, SsbTestFields, SsbTestKind, SSB_TEST_EVENTS,
};

const TUNED: f64 = 14.2e6;
const SETUPS: [(f64, f64); 3] = [(12e3, -20.0), (48e3, 35.0), (192e3, 8.0)];

fn at(cnr: f64, off: f64, side: SsbSideband) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
        .set(&GEN_OFFSET_HZ, off)
        .set(&GEN_SIDEBAND, side)
}

fn run(h: &Harness, rate: f64, seed: u64) -> Streamed {
    stream(
        &DESCRIPTOR,
        &h.clone().seed(seed),
        rate,
        TUNED,
        14.0,
        seed,
        |_, _| {},
    )
}

fn fields(r: &Streamed) -> SsbTestFields {
    SsbTestFields::read(r.results.fields()).unwrap()
}

fn rows(r: &Streamed) -> Vec<SsbTestEventRow> {
    SSB_TEST_EVENTS
        .rows::<SsbTestEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

fn near(v: Option<f64>, want: f64, tol: f64) -> bool {
    v.is_some_and(|v| (v - want).abs() <= tol)
}

#[test]
fn one_tone_reads_carrier_opposite_sideband_and_harmonics() {
    let mut seed = 0;
    for (rate, off) in SETUPS {
        for side in [SsbSideband::Upper, SsbSideband::Lower] {
            seed += 1;
            let f = fields(&run(&at(95.0, off, side), rate, seed));
            println!(
                "{rate} sps {side:?}: tone {:?}, carrier {:?}, opposite {:?}, h2 {:?}, h3 {:?}",
                f.tone1_hz, f.carrier_db, f.opposite_db, f.h2_db, f.h3_db
            );
            assert_eq!(
                (f.kind, f.sideband),
                (SsbTestKind::OneTone, Some(side)),
                "{f:?}"
            );
            assert!(near(f.tone1_hz.map(|h| h.0), 1000.0, 1.0), "{f:?}");
            assert!(
                near(f.carrier_db, 50.0, 1.5) && f.carrier_found == Some(true),
                "{f:?}"
            );
            assert!(near(f.opposite_db, 55.0, 1.5), "{f:?}");
            assert!(
                near(f.h2_db, 60.0, 1.5) && f.h2_under == Some(false),
                "{f:?}"
            );
            assert!(
                near(f.h3_db, 58.0, 1.5) && f.h3_under == Some(false),
                "{f:?}"
            );
        }
    }
}

#[test]
fn two_tones_read_the_products_under_one_tone_and_pep() {
    let tones = "1000, 0; 1700, 0; 300, -31; 2400, -34; 3100, -47; -1000, -52";
    let mut seed = 10;
    for (rate, off) in SETUPS {
        for side in [SsbSideband::Upper, SsbSideband::Lower] {
            seed += 1;
            let h = at(95.0, off, side)
                .set(&GEN_TONES, tones)
                .set(&GEN_CARRIER_DBC, -48.0);
            let f = fields(&run(&h, rate, seed));
            println!(
                "{rate} sps {side:?}: IMD3 {:?} ({:?} PEP), IMD5 {:?}, IMD7 {:?} {:?}, carrier {:?}, opposite {:?}",
                f.imd3_db, f.imd3_pep_db, f.imd5_db, f.imd7_db, f.imd7_under, f.carrier_db, f.opposite_db
            );
            assert_eq!(f.kind, SsbTestKind::TwoTones, "{f:?}");
            assert!(
                near(f.tone1_hz.map(|h| h.0), 1000.0, 1.0)
                    && near(f.tone2_hz.map(|h| h.0), 1700.0, 1.0),
                "{f:?}"
            );
            assert!(
                near(f.imd3_db, 31.0, 1.5) && near(f.imd3_pep_db, 37.0, 1.6),
                "{f:?}"
            );
            assert!(
                near(f.imd5_db, 47.0, 1.5) && f.imd7_under == Some(true),
                "{f:?}"
            );
            assert!(
                near(f.carrier_db, 48.0, 1.5) && near(f.opposite_db, 52.0, 1.5),
                "{f:?}"
            );
        }
    }
}

#[test]
fn unequal_tones_read_the_products_against_the_weaker() {
    for seed in [21, 22] {
        let h = at(95.0, 0.0, SsbSideband::Upper)
            .set(&GEN_TONES, "1200, 0; 2000, -5; 400, -36; 2800, -39");
        let f = fields(&run(&h, 48e3, seed));
        println!(
            "seed {seed}: IMD3 {:?}, difference {:?}",
            f.imd3_db, f.tone_level_diff_db
        );
        assert!(near(f.imd3_db, 31.0, 1.5), "{f:?}");
    }
}

#[test]
fn a_harmonic_at_50_db_holds_down_to_a_tone_cnr_of_70_db() {
    for (k, cnr) in [80.0, 70.0, 60.0, 55.0, 50.0].into_iter().enumerate() {
        let h = at(cnr, 0.0, SsbSideband::Upper).set(&GEN_TONES, "1000, 0; 2000, -50");
        let f = fields(&run(&h, 48e3, 31 + k as u64));
        println!(
            "{cnr} dB: h2 {:?} under {:?}, floor {:?}",
            f.h2_db, f.h2_under, f.floor_db
        );
        if cnr >= 70.0 {
            assert!(
                near(f.h2_db, 50.0, 1.5) && f.h2_under == Some(false),
                "{cnr}: {f:?}"
            );
        }
    }
}

#[test]
fn noise_alone_and_a_carrier_alone_give_no_row() {
    for seed in [41, 42, 43] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE_FLOOR_DBFS, -30.0);
        let r = run(&h, 48e3, seed);
        assert!(
            rows(&r).is_empty() && fields(&r).kind == SsbTestKind::NoTest,
            "{:?}",
            rows(&r)
        );
        let h = at(60.0, 5.0, SsbSideband::Upper)
            .set(&GEN_TONES, "")
            .set(&GEN_CARRIER_DBC, 0.0);
        let r = run(&h, 48e3, seed);
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
    }
}

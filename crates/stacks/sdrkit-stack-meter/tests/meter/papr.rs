//! The crest factor meter: Gaussian noise at two powers against theory, an FM carrier, QPSK against a direct count of the
//! samples, clipped noise and noise with impulses.

use crate::stream::{stream, Streamed};
use sdrkit_algo_dsp::ccdf::ccdf;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::papr::gen::{GEN_CLIP, GEN_KIND, GEN_POWER};
use sdrkit_stack_meter::papr::{BLOCK, DESCRIPTOR, SHARES};
use sdrkit_stack_meter::typed::{PaprFields, PaprGenKind, PaprReading};

const TUNED: f64 = 145e6;
const RATE: f64 = 1e6;

fn fields(r: &Streamed) -> PaprFields {
    PaprFields::read(r.results.fields()).unwrap()
}

fn run(h: &Harness, seconds: f64, seed: u64) -> PaprFields {
    fields(&stream(
        &DESCRIPTOR,
        &h.clone().seed(seed),
        RATE,
        TUNED,
        seconds,
        seed,
        |_, _| {},
    ))
}

#[test]
fn gaussian_noise_reads_the_levels_of_theory() {
    let bounds = [0.1, 0.15, 0.3, 0.6];
    let mut seed = 0;
    for power in [0.01, 0.4] {
        for _ in 0..2 {
            seed += 1;
            let f = run(
                &Harness::new(&DESCRIPTOR).set(&GEN_POWER, power),
                12.0,
                seed,
            );
            let got = [f.ccdf_10_db, f.ccdf_1_db, f.ccdf_01_db, f.ccdf_001_db].map(Option::unwrap);
            println!(
                "power {power}, seed {seed}: {got:?}, crest {:?}",
                f.crest_db
            );
            for ((t, b), l) in SHARES.iter().zip(bounds).zip(got) {
                let want = 10.0 * (-t.ln()).log10();
                assert!((l - want).abs() <= b, "{power} {t}: {l} against {want}");
            }
            assert_eq!(f.reading, Some(PaprReading::GaussianLike));
            assert!((f.mean_dbfs.unwrap() - 10.0 * power.log10()).abs() < 0.05);
        }
    }
}

#[test]
fn an_fm_carrier_has_a_constant_envelope() {
    for seed in [11, 12] {
        let f = run(
            &Harness::new(&DESCRIPTOR).set(&GEN_KIND, PaprGenKind::Fm),
            5.0,
            seed,
        );
        println!("FM: crest {:?}", f.crest_db);
        assert!(
            f.crest_db.unwrap() < 0.1 && f.reading == Some(PaprReading::ConstantEnvelope),
            "{f:?}"
        );
    }
}

#[test]
fn qpsk_reads_the_count_of_its_own_samples() {
    for seed in [21, 22] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_KIND, PaprGenKind::Qpsk)
            .seed(seed);
        let mut all: Vec<f64> = Vec::new();
        let r = stream(
            &DESCRIPTOR,
            &h,
            RATE,
            TUNED,
            12.0,
            seed,
            |_, x: &mut [Complex32]| {
                all.extend(x.iter().map(|z| f64::from(z.norm_sqr())));
            },
        );
        let f = fields(&r);
        // The last evaluation: block counts 1, 17, 33, ...; the 64 blocks up to it.
        let blocks = all.len() / BLOCK;
        let last = (blocks - 1) / 16 * 16 + 1;
        let window = &all[(last - 64) * BLOCK..last * BLOCK];
        let want = ccdf(window, &SHARES, &mut Vec::new()).unwrap();
        let (l1, l01) = (f.ccdf_1_db.unwrap(), f.ccdf_01_db.unwrap());
        println!(
            "QPSK: {l1:.3} / {l01:.3} dB; count {:.3} / {:.3}",
            want.levels_db[1], want.levels_db[2]
        );
        assert!((l1 - want.levels_db[1]).abs() <= 0.25 && (l01 - want.levels_db[2]).abs() <= 0.4);
        assert!(l01 < 7.0, "{l01}");
    }
}

#[test]
fn clipped_noise_is_less_peaky_and_impulses_are_peakier() {
    for seed in [31, 32] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_KIND, PaprGenKind::ClippedNoise)
            .set(&GEN_POWER, 1.0)
            .set(&GEN_CLIP, 1.0);
        let f = run(&h, 5.0, seed);
        println!("clipped: {:?} {:?}", f.ccdf_01_db, f.reading);
        assert!(f.ccdf_01_db.unwrap() < 3.2);
        assert!(matches!(
            f.reading,
            Some(PaprReading::LessPeaky | PaprReading::ConstantEnvelope)
        ));
        let h = Harness::new(&DESCRIPTOR).set(&GEN_KIND, PaprGenKind::NoiseWithImpulses);
        let f = run(&h, 5.0, seed);
        println!("impulses: {:?} {:?}", f.ccdf_01_db, f.reading);
        assert!(f.ccdf_01_db.unwrap() > 9.0 && f.reading == Some(PaprReading::Peakier));
    }
}

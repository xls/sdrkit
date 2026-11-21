//! The spectral kurtosis finder: noise alone, a steady and a pulsed tone at two shifts, a sweep of a weak pulsed tone and a
//! pulsed tone of 30 % duty.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::kurtosis::gen::GEN_TONES;
use sdrkit_stack_meter::kurtosis::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    KurtosisChange, KurtosisEventRow, KurtosisFields, KurtosisKind, KurtosisRegionRow,
    KURTOSIS_EVENTS, KURTOSIS_REGIONS,
};

const TUNED: f64 = 145e6;
const RATE: f64 = 2e6;

fn noisy() -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, 0.0)
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

fn regions(r: &Streamed) -> Vec<KurtosisRegionRow> {
    KURTOSIS_REGIONS
        .rows::<KurtosisRegionRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

fn rows(r: &Streamed) -> Vec<KurtosisEventRow> {
    KURTOSIS_EVENTS
        .rows::<KurtosisEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

/// The region of `kind` that holds `hz`.
fn at(r: &[KurtosisRegionRow], kind: KurtosisKind, hz: f64) -> Option<KurtosisRegionRow> {
    r.iter()
        .find(|g| g.kind == kind && g.low.0 - 2e3 <= hz && hz <= g.high.0 + 2e3)
        .cloned()
}

#[test]
fn noise_alone_has_no_region_and_a_clean_band() {
    for seed in [1, 2] {
        let r = run(noisy().set(&GEN_TONES, ""), 60.0, seed);
        let f = KurtosisFields::read(r.results.fields()).unwrap();
        println!(
            "seed {seed}: clean {:?} %, regions {:?}",
            f.clean_pct, f.regions
        );
        assert!(
            f.clean_pct.unwrap() > 99.0 && f.regions == Some(0) && rows(&r).is_empty(),
            "{:?}",
            rows(&r)
        );
    }
}

#[test]
fn a_steady_and_a_pulsed_tone_read_their_kurtosis() {
    for (seed, shift) in [(11, 0.0), (12, 777.0)] {
        let r = run(noisy().set(&GEN_OFFSET_HZ, shift), 60.0, seed);
        let g = regions(&r);
        let steady =
            at(&g, KurtosisKind::Steady, TUNED + 300e3 + shift).expect("the steady region");
        let pulsed =
            at(&g, KurtosisKind::Pulsed, TUNED - 400e3 + shift).expect("the pulsed region");
        println!(
            "shift {shift}: steady SK {:.3}, pulsed SK {:.3} ({:?} % at full power)",
            steady.sk, pulsed.sk, pulsed.full_power_pct
        );
        assert!(steady.sk < 0.4 && (2.5..=5.5).contains(&pulsed.sk), "{g:?}");
        let e = rows(&r);
        assert!(
            e.iter().any(|e| e.kind == KurtosisChange::Pulsed)
                && e.iter().any(|e| e.kind == KurtosisChange::Steady),
            "{e:?}"
        );
    }
}

#[test]
fn a_weak_pulsed_tone_is_found_from_0_2() {
    for (k, amp) in [1.0, 0.5, 0.3, 0.2, 0.1, 0.07].into_iter().enumerate() {
        let tone = format!("123456, {amp}, 0.0123, 0.05");
        let r = run(noisy().set(&GEN_TONES, &tone), 20.0, 21 + k as u64);
        let p = at(&regions(&r), KurtosisKind::Pulsed, TUNED + 123_456.0);
        println!(
            "amplitude {amp}: {:?}",
            p.as_ref().map(|p| (p.sk, p.over_noise_db))
        );
        if amp >= 0.2 {
            assert!(p.is_some(), "{amp}");
        }
    }
}

#[test]
fn a_pulsed_tone_of_30_percent_duty() {
    for seed in [31, 32] {
        let r = run(noisy().set(&GEN_TONES, "50000, 8, 0.0071, 0.3"), 40.0, seed);
        let p = at(&regions(&r), KurtosisKind::Pulsed, TUNED + 50e3).expect("the pulsed region");
        println!("SK {:.3}", p.sk);
        assert!((2.5..=6.5).contains(&p.sk), "{}", p.sk);
    }
}

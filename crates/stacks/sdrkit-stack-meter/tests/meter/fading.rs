//! The fading analyser: Rayleigh fading at two Dopplers against theory, Rician fading, a clean carrier, a sweep of the
//! noise, a carrier that goes and comes back, and noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::fading::gen::{GEN_FD_HZ, GEN_K, GEN_OFF};
use sdrkit_stack_meter::fading::DESCRIPTOR;
use sdrkit_stack_meter::typed::{FadingEventKind, FadingEventRow, FadingFields, FADING_EVENTS};

const TUNED: f64 = 144.4e6;
const RATE: f64 = 16e3;

fn floor() -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -40.0)
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

fn fields(r: &Streamed) -> FadingFields {
    FadingFields::read(r.results.fields()).unwrap()
}

fn rows(r: &Streamed) -> Vec<FadingEventRow> {
    let mut v: Vec<FadingEventRow> = FADING_EVENTS
        .rows::<FadingEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect();
    v.sort_by(|a, b| a.time_s.0.total_cmp(&b.time_s.0));
    v
}

#[test]
fn rayleigh_fading_reads_the_rates_of_theory() {
    for (seed, fd) in [(1, 5.0), (2, 50.0), (3, 5.0), (4, 50.0)] {
        let h = floor()
            .set(&GEN_FD_HZ, fd)
            .set(&GEN_OFFSET_HZ, 800.0)
            .set(&GEN_DRIFT_HZ_S, 3.0);
        let f = fields(&run(h, 75.0, seed));
        let (rate, len, dop, s4) = (
            f.fade_rate_hz.unwrap(),
            f.fade_length_ms.unwrap() / 1e3,
            f.doppler_hz.unwrap(),
            f.s4.unwrap(),
        );
        println!("fd {fd}: rate {rate:.3} ({:.3}), length {len:.4} ({:.4}), Doppler {dop:.2} ({:.2}), S4 {s4:.3}, K {:?}", 0.717 * fd, 0.1328 / fd, fd / 2f64.sqrt(), f.k_db);
        assert!((rate / (0.717 * fd) - 1.0).abs() <= 0.3, "{rate}");
        assert!((len / (0.1328 / fd) - 1.0).abs() <= 0.35, "{len}");
        assert!((dop / (fd / 2f64.sqrt()) - 1.0).abs() <= 0.3, "{dop}");
        assert!(f.rayleigh == Some(true) || f.k_db.unwrap() < 0.0, "{f:?}");
        assert!((s4 - 1.0).abs() <= 0.12, "{s4}");
    }
}

#[test]
fn rician_fading_reads_its_k_factor_and_s4() {
    for (seed, k_db) in [(11, 10.0), (12, 3.0), (13, 10.0), (14, 3.0)] {
        let k = 10f64.powf(k_db / 10.0);
        let h = floor()
            .set(&GEN_FD_HZ, 8.0)
            .set(&GEN_K, k)
            .set(&GEN_OFFSET_HZ, -1700.0)
            .set(&GEN_DRIFT_HZ_S, 2.0);
        let f = fields(&run(h, 75.0, seed));
        let want_s4 = ((2.0 * k + 1.0) / (k + 1.0).powi(2)).sqrt();
        println!("K {k_db} dB: {:?} dB, S4 {:?} ({want_s4:.3})", f.k_db, f.s4);
        assert!((f.k_db.unwrap() - k_db).abs() <= 2.0, "{:?}", f.k_db);
        assert!((f.s4.unwrap() - want_s4).abs() <= 0.07, "{:?}", f.s4);
    }
}

#[test]
fn a_clean_carrier_reads_no_fading() {
    for seed in [21, 22] {
        let f = fields(&run(floor().set(&GEN_K, 1e6), 50.0, seed));
        println!("clean: K {:?} dB", f.k_db);
        assert!(f.k_db.unwrap() >= 40.0, "{:?}", f.k_db);
    }
}

#[test]
fn a_sweep_of_the_noise_keeps_k_from_25_db() {
    for (j, cnr) in [40.0, 30.0, 25.0, 20.0, 15.0].into_iter().enumerate() {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, cnr)
            .set(&GEN_FD_HZ, 8.0)
            .set(&GEN_K, 10.0);
        let f = fields(&run(h, 75.0, 31 + j as u64));
        println!("CNR {cnr} dB: K {:?} dB", f.k_db);
        if cnr >= 25.0 {
            assert!((f.k_db.unwrap() - 10.0).abs() <= 2.5, "{cnr}: {:?}", f.k_db);
        }
    }
}

#[test]
fn a_carrier_that_goes_and_comes_back() {
    for seed in [41, 42] {
        let h = floor()
            .set(&GEN_FD_HZ, 5.0)
            .set(&GEN_K, 3.0)
            .set(&GEN_OFF, "60, 80");
        let r = rows(&run(h, 150.0, seed));
        let c: Vec<FadingEventKind> = r
            .iter()
            .map(|e| e.kind)
            .filter(|k| matches!(k, FadingEventKind::Found | FadingEventKind::Lost))
            .collect();
        assert!(
            c.len() >= 3
                && c[..3]
                    == [
                        FadingEventKind::Found,
                        FadingEventKind::Lost,
                        FadingEventKind::Found
                    ],
            "{c:?}"
        );
    }
}

#[test]
fn noise_alone_gives_no_row() {
    for seed in [51, 52, 53] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -40.0)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0);
        let r = run(h, 90.0, seed);
        assert!(
            rows(&r).is_empty() && fields(&r).s4.is_none(),
            "{:?}",
            rows(&r)
        );
    }
}

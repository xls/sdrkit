//! The phase noise meter: a Wiener oscillator with a white floor at three rates, a spur, the jitter, a quiet oscillator in
//! additive noise, and noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::phase_noise::gen::{GEN_FLOOR_DBC, GEN_LINEWIDTH_HZ, GEN_SPUR_HZ};
use sdrkit_stack_meter::phase_noise::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    PhaseNoiseEventRow, PhaseNoiseFields, PhaseNoiseSpurRow, PHASE_NOISE_EVENTS, PHASE_NOISE_SPURS,
};
use std::f64::consts::PI;

const TUNED: f64 = 10e6;

fn oscillator(lw: f64, floor: f64, off: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_LINEWIDTH_HZ, lw)
        .set(&GEN_FLOOR_DBC, floor)
        .set(&GEN_OFFSET_HZ, off)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -90.0)
}

fn fields(r: &Streamed) -> PhaseNoiseFields {
    PhaseNoiseFields::read(r.results.fields()).unwrap()
}

fn theory(lw: f64, floor: f64, f: f64) -> f64 {
    10.0 * (lw / (2.0 * PI * f * f) + 10f64.powf(floor / 10.0)).log10()
}

fn wiener_at(rate: f64, off: f64, seed: u64) {
    let h = oscillator(20.0, -125.0, off)
        .set(&GEN_DRIFT_HZ_S, 2.0)
        .seed(seed);
    let f = fields(&stream(&DESCRIPTOR, &h, rate, TUNED, 24.0, seed, |_, _| {}));
    let got = [f.l_100hz, f.l_1khz, f.l_10khz, f.l_100khz];
    println!(
        "{rate} sps, {off} Hz: {got:.2?}, jitter {:?} mrad",
        f.jitter_mrad
    );
    for (g, fo) in got.iter().zip([100.0, 1e3, 1e4, 1e5]) {
        let want = theory(20.0, -125.0, fo);
        assert!(
            g.is_some_and(|g| (g - want).abs() <= 1.5),
            "{rate} {off}: L({fo}) {g:?} against {want:.2}"
        );
    }
}

#[test]
fn a_wiener_oscillator_reads_its_l_at_250_and_1000_ksps() {
    wiener_at(250e3, 12345.6, 1);
    wiener_at(1e6, 3000.0, 3);
}

#[test]
fn a_wiener_oscillator_reads_its_l_at_500_ksps() {
    wiener_at(500e3, -48000.0, 2);
}

#[test]
fn a_spur_of_minus_62_dbc_is_listed() {
    for seed in [11, 12] {
        let h = oscillator(0.2, -130.0, 500.0)
            .set(&GEN_SPUR_HZ, 1500.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 250e3, TUNED, 24.0, seed, |_, _| {});
        let spurs: Vec<PhaseNoiseSpurRow> = PHASE_NOISE_SPURS
            .rows::<PhaseNoiseSpurRow>(&r.results)
            .map(|(_, s)| s)
            .collect();
        println!("seed {seed}: {spurs:?}");
        assert!(
            spurs.iter().any(
                |s| (s.offset_hz.0 - 1500.0).abs() <= 10.0 && (s.level_dbc + 62.0).abs() <= 2.0
            ),
            "{spurs:?}"
        );
    }
}

#[test]
fn the_jitter_of_a_wiener_oscillator_is_0252_rad() {
    let want = (2.0
        * (20.0 / (2.0 * PI) * (1.0 / 100.0 - 1.0 / 1e5) + 10f64.powf(-12.5) * (1e5 - 100.0)))
        .sqrt();
    for seed in [21, 22] {
        let h = oscillator(20.0, -125.0, 0.0).seed(seed);
        let j = fields(&stream(
            &DESCRIPTOR,
            &h,
            250e3,
            TUNED,
            24.0,
            seed,
            |_, _| {},
        ))
        .jitter_mrad
        .unwrap()
            / 1000.0;
        println!("seed {seed}: {j:.4} rad ({want:.4})");
        assert!((j - want).abs() <= 0.15 * want, "{j}");
    }
}

#[test]
fn a_quiet_oscillator_reads_the_additive_noise_at_100_khz() {
    for (k, cnr) in [70.0, 60.0, 50.0, 40.0].into_iter().enumerate() {
        let h = oscillator(0.01, -150.0, 0.0)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, cnr);
        let seed = 31 + k as u64;
        let l = fields(&stream(
            &DESCRIPTOR,
            &h.seed(seed),
            250e3,
            TUNED,
            24.0,
            seed,
            |_, _| {},
        ))
        .l_100khz;
        let want =
            10.0 * (10f64.powf(-cnr / 10.0) / 2e5 + 0.01 / (2.0 * PI * 1e10) + 1e-15).log10();
        println!("{cnr} dB: L(100 kHz) {l:?} ({want:.2})");
        assert!(l.is_some_and(|l| (l - want).abs() <= 1.5), "{cnr}: {l:?}");
    }
}

#[test]
fn noise_alone_gives_no_row() {
    for seed in [41, 42] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -30.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 250e3, TUNED, 24.0, seed, |_, _| {});
        let rows: Vec<PhaseNoiseEventRow> = PHASE_NOISE_EVENTS
            .rows::<PhaseNoiseEventRow>(&r.results)
            .map(|(_, e)| e)
            .collect();
        assert!(rows.is_empty() && fields(&r).l_1khz.is_none(), "{rows:?}");
    }
}

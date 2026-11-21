//! The beacon monitor: steady, fading and aurora-smeared beacons at three offsets and two levels, a drifting line, noise
//! alone and a beacon that goes and comes back.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CN0_DBHZ, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::beacon_monitor::gen::{GEN_FADE_HZ, GEN_OFF, GEN_SPREAD_HZ};
use sdrkit_stack_meter::beacon_monitor::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    BeaconMonitorEventRow, BeaconMonitorFields, BeaconPath, BEACON_MONITOR_EVENTS,
};

const TUNED: f64 = 144.4e6;
const RATE: f64 = 250e3;
/// The width of a bin of the working spectrum, Hz.
const BIN_HZ: f64 = 250e3 / 104.0 / 2048.0;

/// The beacon with its line `snr` dB over the noise of a bin.
fn at(snr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Cn0)
        .set(&GEN_CN0_DBHZ, snr + 10.0 * BIN_HZ.log10())
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

fn fields(r: &Streamed) -> BeaconMonitorFields {
    BeaconMonitorFields::read(r.results.fields()).unwrap()
}

fn rows(r: &Streamed) -> Vec<BeaconMonitorEventRow> {
    BEACON_MONITOR_EVENTS
        .rows::<BeaconMonitorEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

#[test]
fn steady_and_aurora_beacons_end_in_their_class() {
    let mut seed = 0;
    for off in [0.0, 180.0, -420.0] {
        for snr in [35.0, 30.0] {
            seed += 1;
            let f = fields(&run(at(snr).set(&GEN_OFFSET_HZ, off), 40.0, seed));
            let o = f.line_offset_hz.unwrap().0;
            println!(
                "{off} Hz, {snr} dB steady: {:?}, offset {o:.2}, spread {:?}, swing {:?}",
                f.class, f.spread_hz, f.swing_db
            );
            assert!(
                f.class == BeaconPath::Steady && (o - off).abs() <= 3.0,
                "{f:?}"
            );
            let f = fields(&run(
                at(snr).set(&GEN_OFFSET_HZ, off).set(&GEN_SPREAD_HZ, 120.0),
                40.0,
                seed,
            ));
            println!(
                "{off} Hz, {snr} dB aurora: {:?}, spread {:?}",
                f.class, f.spread_hz
            );
            assert_eq!(f.class, BeaconPath::AuroraLike, "{f:?}");
        }
    }
}

/// The fading beacon at `off` Hz and `snr` dB, with the seed of its case.
fn fading_case(off: f64, snr: f64, seed: u64) {
    let f = fields(&run(
        at(snr).set(&GEN_OFFSET_HZ, off).set(&GEN_FADE_HZ, 0.12),
        60.0,
        seed,
    ));
    println!(
        "{off} Hz, {snr} dB fading: {:?}, swing {:?}",
        f.class, f.swing_db
    );
    assert_eq!(f.class, BeaconPath::Fading, "{f:?}");
}

#[test]
fn fading_beacons_end_fading() {
    for (off, snr, seed) in [
        (0.0, 35.0, 1),
        (0.0, 30.0, 2),
        (180.0, 35.0, 3),
        (180.0, 30.0, 4),
        (-420.0, 30.0, 6),
    ] {
        fading_case(off, snr, seed);
    }
}

#[test]
fn a_fading_beacon_at_minus_420_hz_and_35_db_ends_fading() {
    fading_case(-420.0, 35.0, 5);
}

#[test]
fn a_drifting_line_ends_steady() {
    for seed in [11, 12] {
        let f = fields(&run(
            at(30.0)
                .set(&GEN_OFFSET_HZ, 100.0)
                .set(&GEN_DRIFT_HZ_S, 4.0),
            60.0,
            seed,
        ));
        println!("drift: {:?}, spread {:?}", f.class, f.spread_hz);
        assert_eq!(f.class, BeaconPath::Steady, "{f:?}");
    }
}

#[test]
fn noise_alone_gives_no_row() {
    for seed in [21, 22] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0);
        let r = run(h, 300.0, seed);
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
    }
}

#[test]
fn a_beacon_that_goes_and_comes_back_changes_class_three_times() {
    for seed in [31, 32] {
        let r = run(at(30.0).set(&GEN_OFF, "30, 60"), 90.0, seed);
        let n = rows(&r).len();
        println!(
            "changes: {:?}",
            rows(&r)
                .iter()
                .map(|e| (e.time_s.0, e.from_class, e.to_class))
                .collect::<Vec<_>>()
        );
        assert!(n >= 3, "{n}");
    }
}

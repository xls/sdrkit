//! The occupancy monitor: four keyed carriers at three rates with and without a radio error; two carriers over a sweep of
//! the noise; a steady carrier; noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::occupancy::gen::{DEFAULT_TONES, GEN_TONES};
use sdrkit_stack_meter::occupancy::DESCRIPTOR;
use sdrkit_stack_meter::scene::parse_tones;
use sdrkit_stack_meter::typed::{
    OccupancyChannelRow, OccupancyFields, OCCUPANCY_ACTIVITY_EVENT, OCCUPANCY_CHANNELS,
};

const CENTRE: f64 = 145e6;

fn run(rate: f64, secs: f64, seed: u64, h: Harness) -> Streamed {
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

fn at_cnr(cnr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
}

fn channels(r: &Streamed) -> Vec<OccupancyChannelRow> {
    OCCUPANCY_CHANNELS
        .rows::<OccupancyChannelRow>(&r.results)
        .map(|(_, c)| c)
        .collect()
}

/// The four carriers of the default scene at 25 dB in 25 kHz for 40 s.
fn four_carriers(rate: f64, offset: f64, drift: f64) {
    let r = run(
        rate,
        40.0,
        3,
        at_cnr(25.0)
            .set(&GEN_OFFSET_HZ, offset)
            .set(&GEN_DRIFT_HZ_S, drift),
    );
    let at = format!("{rate} sps, {offset} Hz, {drift} Hz/s");
    let list = channels(&r);
    let tones = parse_tones(DEFAULT_TONES).unwrap();
    assert_eq!(list.len(), 4, "{at}: {list:?}");
    for t in &tones {
        let want = CENTRE + t.offset_hz + offset + drift * 20.0;
        let c = list
            .iter()
            .find(|c| (c.frequency.0 - want).abs() <= 16e3)
            .unwrap_or_else(|| panic!("{at}: no channel near {want}: {list:?}"));
        assert!(
            (c.duty_pct - 100.0 * t.duty).abs() <= 3.5,
            "{at}: {t:?} duty {}",
            c.duty_pct
        );
        let n = 40.0 / t.period_s;
        assert!(
            (c.transmissions as f64 - n).abs() <= 1.0,
            "{at}: {t:?} {} transmissions",
            c.transmissions
        );
        println!(
            "{at}: {:.0} Hz duty {:.2} % {} transmissions",
            c.frequency.0 - CENTRE,
            c.duty_pct,
            c.transmissions
        );
    }
    let rows = r
        .events
        .iter()
        .filter(|e| e.is(&OCCUPANCY_ACTIVITY_EVENT))
        .count();
    assert!(rows > 20, "{at}: {rows} rows");
}

#[test]
fn four_keyed_carriers_at_2_msps_without_a_radio_error() {
    four_carriers(2e6, 0.0, 0.0);
}

#[test]
fn four_keyed_carriers_at_2_msps_3_khz_off() {
    four_carriers(2e6, 3e3, 0.0);
}

#[test]
fn four_keyed_carriers_at_2_msps_2_khz_off_and_drifting() {
    four_carriers(2e6, -2e3, 40.0);
}

#[test]
fn four_keyed_carriers_at_1_msps() {
    four_carriers(1e6, 0.0, 0.0);
}

#[test]
fn four_keyed_carriers_at_4_msps_1_5_khz_off() {
    four_carriers(4e6, 1.5e3, 0.0);
}

#[test]
fn two_carriers_are_found_without_a_false_channel_from_9_db() {
    let tones = "-111757.8, 1, 2, 0.5, 0.3; 218320.3, 1, 2, 0.5, 1.1";
    for cnr in [20.0, 12.0, 9.0, 7.0, 5.0] {
        let r = run(2e6, 30.0, 5, at_cnr(cnr).set(&GEN_TONES, tones));
        let list = channels(&r);
        let found = [-111_757.8, 218_320.3]
            .iter()
            .filter(|&&f| {
                list.iter()
                    .any(|c| (c.frequency.0 - CENTRE - f).abs() <= 16e3)
            })
            .count();
        println!(
            "{cnr} dB: {found} of 2 found, {} channels listed",
            list.len()
        );
        if cnr >= 9.0 {
            assert_eq!((found, list.len()), (2, 2), "{cnr} dB: {list:?}");
        }
    }
}

#[test]
fn a_steady_carrier_is_one_channel_busy_almost_all_the_time() {
    let r = run(2e6, 10.0, 7, at_cnr(25.0).set(&GEN_TONES, "55000, 1"));
    let list = channels(&r);
    assert_eq!(list.len(), 1, "{list:?}");
    assert!(list[0].duty_pct > 96.0, "{list:?}");
    assert!((list[0].frequency.0 - CENTRE - 55e3).abs() <= 16e3);
}

fn noise_alone(rate: f64, seed: u64) {
    let h = Harness::new(&DESCRIPTOR)
        .set(&GEN_TONES, "")
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -20.0);
    let r = run(rate, 150.0, seed, h);
    let f = OccupancyFields::read(r.results.fields()).unwrap();
    assert_eq!(
        r.events
            .iter()
            .filter(|e| e.is(&OCCUPANCY_ACTIVITY_EVENT))
            .count(),
        0,
        "{rate} sps seed {seed}"
    );
    assert_eq!(f.busy_now, 0, "{rate} sps seed {seed}");
    assert!(f.elapsed_s.0 > 149.0);
}

#[test]
fn noise_alone_for_150_s_at_2_msps_is_never_busy() {
    noise_alone(2e6, 11);
}

#[test]
fn noise_alone_for_150_s_at_1_msps_is_never_busy() {
    noise_alone(1e6, 12);
    noise_alone(1e6, 13);
}

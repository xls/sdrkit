//! The hopper analyser: four hoppers, with clock errors, jitter and a radio error; a sweep of the noise; signals that are
//! not hoppers.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::hopper::gen::{
    GEN_CHANNELS, GEN_DWELL_S, GEN_FSK_HZ, GEN_GUARD_S, GEN_JITTER, GEN_SPACING_HZ,
};
use sdrkit_stack_meter::hopper::DESCRIPTOR;
use sdrkit_stack_meter::typed::{HopperChangedEvent, HOPPER_CHANGED};
use std::f64::consts::TAU;

const CENTRE: f64 = 433.9e6;

fn hopper(channels: i64, spacing: f64, dwell: f64, guard: f64, cnr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
        .set(&GEN_CHANNELS, channels)
        .set(&GEN_SPACING_HZ, spacing)
        .set(&GEN_DWELL_S, dwell)
        .set(&GEN_GUARD_S, guard)
}

fn rows(r: &Streamed) -> Vec<HopperChangedEvent> {
    r.events
        .iter()
        .filter_map(|e| HOPPER_CHANGED.read::<HopperChangedEvent>(e))
        .collect()
}

/// A hopper case: the last row must give the rate, the channels, the spacing and the dwell.
fn case(h: Harness, rate: f64, channels: i64, spacing_hz: f64, dwell_s: f64, seed: u64) {
    let r = stream(
        &DESCRIPTOR,
        &h.seed(seed),
        2e6,
        CENTRE,
        4.0,
        seed,
        |_, _| {},
    );
    let rows = rows(&r);
    let at = format!("{channels} channels, {spacing_hz} Hz, {dwell_s} s");
    let last = rows.last().unwrap_or_else(|| panic!("{at}: no row"));
    println!(
        "{at}: {:.2} hops/s, {} channels, {:.2} kHz, dwell {:.3} ms, guard {:.3} ms, {} rows",
        last.hop_rate,
        last.channels,
        last.spacing_khz,
        last.dwell_ms,
        last.guard_ms,
        rows.len()
    );
    // The hop times are fitted over many hops, so the rate is not held to whole frame steps.
    assert!(
        (last.hop_rate / rate - 1.0).abs() <= 0.005,
        "{at}: rate {}",
        last.hop_rate
    );
    assert_eq!(last.channels, channels, "{at}");
    assert!(
        (last.spacing_khz * 1e3 / spacing_hz - 1.0).abs() <= 0.04,
        "{at}: spacing {}",
        last.spacing_khz
    );
    let tol = (0.2e-3f64).max(0.3 * dwell_s);
    assert!(
        (last.dwell_ms * 1e-3 - dwell_s).abs() <= tol,
        "{at}: dwell {}",
        last.dwell_ms
    );
}

#[test]
fn sixteen_channels_50_khz_apart_at_500_hops() {
    case(
        hopper(16, 50e3, 0.002, 0.0003, 30.0),
        500.0,
        16,
        50e3,
        0.0017,
        1,
    );
}

#[test]
fn eight_channels_with_jitter_a_clock_error_and_a_drifting_radio() {
    let h = hopper(8, 100e3, 0.010, 0.001, 30.0)
        .set(&GEN_JITTER, 0.02)
        .set(&GEN_CLOCK_PPM, 200.0)
        .set(&GEN_OFFSET_HZ, 3e3)
        .set(&GEN_DRIFT_HZ_S, 20.0);
    case(h, 99.98, 8, 100e3, 0.009, 2);
}

#[test]
fn thirty_two_channels_25_khz_apart_with_a_slow_clock() {
    let h = hopper(32, 25e3, 0.00125, 0.0002, 30.0)
        .set(&GEN_CLOCK_PPM, -300.0)
        .set(&GEN_OFFSET_HZ, -1.5e3);
    case(h, 800.2, 32, 25e3, 0.00105, 3);
}

#[test]
fn twelve_channels_60_khz_apart_with_jitter() {
    case(
        hopper(12, 60e3, 0.002, 0.0003, 30.0).set(&GEN_JITTER, 0.03),
        500.0,
        12,
        60e3,
        0.0017,
        4,
    );
}

#[test]
fn the_first_hopper_is_found_with_its_16_channels_from_12_db() {
    for cnr in [30.0, 20.0, 15.0, 12.0, 9.0, 6.0] {
        let r = stream(
            &DESCRIPTOR,
            &hopper(16, 50e3, 0.002, 0.0003, cnr).seed(5),
            2e6,
            CENTRE,
            4.0,
            5,
            |_, _| {},
        );
        let last = rows(&r).last().map(|r| r.channels);
        println!("{cnr} dB: {last:?} channels");
        if cnr >= 12.0 {
            assert_eq!(last, Some(16), "{cnr} dB");
        }
    }
}

/// 8 s of white noise at the level of 30 dB in 25 kHz with `add` laid over it gives no row.
fn not_a_hopper(what: &str, h: Harness, mut add: impl FnMut(f64) -> Complex32) {
    for seed in [6, 7] {
        let r = stream(
            &DESCRIPTOR,
            &h.clone().seed(seed),
            2e6,
            CENTRE,
            8.0,
            seed,
            |t0, x| {
                for (i, z) in x.iter_mut().enumerate() {
                    *z += add(t0 + i as f64 / 2e6);
                }
            },
        );
        assert!(rows(&r).is_empty(), "{what}, seed {seed}: {:?}", rows(&r));
    }
}

#[test]
fn noise_a_carrier_a_chirp_and_a_two_channel_toggler_are_not_hoppers() {
    let silent = || hopper(0, 50e3, 0.002, 0.0, 30.0);
    not_a_hopper("noise", silent(), |_| Complex32::default());
    not_a_hopper("a carrier", silent(), |t| {
        Complex32::from_polar(1.0, (TAU * (137e3 * t).fract()) as f32)
    });
    not_a_hopper("a chirp", silent(), |t| {
        // From -400 kHz at 100 kHz/s: the phase is the integral of the frequency.
        Complex32::from_polar(1.0, (TAU * (-400e3 * t + 50e3 * t * t).fract()) as f32)
    });
    let toggler = hopper(2, 80e3, 0.003, 0.0, 30.0).set(&GEN_FSK_HZ, 0.0);
    not_a_hopper("a two-channel toggler", toggler, |_| Complex32::default());
}

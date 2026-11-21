//! e-Czas through the receiver: clean, with the standard impairments, as an audio recording, and the clock comparison.

use sci::eczas::{EczasGenOptions, EczasGenerator};
use sci::typed::{EczasFields, ECZAS_TIME};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, Generator, Station, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_science as sci;
use sdrkit_util_geo::Position;

const START: f64 = 1_791_813_600.0;

fn f64_field(r: &Run, key: &str) -> f64 {
    match r.field(key) {
        Some(Value::F64(v) | Value::Time(v) | Value::Hz(v)) => *v,
        other => panic!("{key}: {other:?} ({})", r.results.status().summary),
    }
}

fn harness(seconds: f64) -> Harness {
    // The signal starts a second before a slot, so the first frame is whole.
    Harness::new(&sci::ECZAS)
        .rate(48e3)
        .seconds(seconds)
        .set(&sdrkit_stack_science::eczas::GEN_START_UTC, START - 1.0)
}

#[test]
fn time_packets_decode_clean() {
    let r = harness(40.0).run().unwrap();
    let ok = r.results.fields().get(&keys::FRAMES_OK).unwrap();
    assert!(ok >= 3, "{}", r.results.status().summary);
    assert_eq!(r.results.fields().get(&keys::FRAMES_BAD), Some(0));
    assert_eq!(r.results.fields().at(&EczasFields::CONFIRMED), Some(true));
    assert!((f64_field(&r, "utc") - (START - 1.0 + 40.0)).abs() < 1.0);
    let times: Vec<_> = r.events.iter().filter(|e| e.is(&ECZAS_TIME)).collect();
    assert_eq!(times.len() as i64, ok);
    assert!(times[0].get(&keys::TEXT).is_some());
}

#[test]
fn time_packets_decode_with_noise_and_an_offset() {
    let r = harness(40.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0)
        .set(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 30.0)
        .run()
        .unwrap();
    let ok = r.results.fields().get(&keys::FRAMES_OK).unwrap();
    assert!(ok >= 2, "{}", r.results.status().summary);
    assert!((f64_field(&r, "offset_hz") - 30.0).abs() < 5.0);
}

#[test]
fn an_audio_recording_decodes() {
    let o = EczasGenOptions {
        start_utc: START,
        audio_tone: Some(1000.0),
        ..Default::default()
    };
    let mut g = EczasGenerator::new(48e3, o);
    let mut x = vec![Complex32::new(0.0, 0.0); 48_000 * 40];
    g.generate(&mut x);
    let r = Harness::new(&sci::ECZAS)
        .rate(48e3)
        .set(&sdrkit_stack_science::eczas::AUDIO_INPUT, true)
        .feed(&x)
        .unwrap();
    let ok = r.results.fields().get(&keys::FRAMES_OK).unwrap();
    assert!(ok >= 3, "{}", r.results.status().summary);
}

#[test]
fn the_clock_offset_includes_the_path_delay() {
    let rx = Position::new(52.23, 21.01);
    let d = Position::new(sci::eczas::SITE.0, sci::eczas::SITE.1).distance_m(&rx);
    let delay = d / sdrkit_util_geo::SPEED_OF_LIGHT_M_S;
    let r = harness(40.0)
        .station(Station::at(rx))
        .set(
            &sdrkit_stack_science::clock::CLOCK_MODE,
            sdrkit_stack_science::clock::ClockChoice::Fixed,
        )
        .set(
            &sdrkit_stack_science::clock::CLOCK_ORIGIN,
            START - 1.0 + delay,
        )
        .run()
        .unwrap();
    assert!((f64_field(&r, "delay_ms") - delay * 1000.0).abs() < 1e-6);
    let avg = f64_field(&r, "clock_offset_avg_s");
    assert!(avg.abs() < 0.003, "{avg}");
}

#[test]
fn the_bit_slope_keeps_most_packets_at_25_db() {
    // 120 s holds 10 time packets.
    let r = harness(120.0)
        .seed(3)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0)
        .run()
        .unwrap();
    let ok = r.results.fields().get(&keys::FRAMES_OK).unwrap();
    assert!(ok >= 7, "{ok}: {}", r.results.status().summary);
}

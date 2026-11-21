//! Acceptance tests of the MDC-1200 stack: bursts and calls on several carriers at once, the test signal, and the rules of
//! every stack. The burst coder and decoder are tested in `acceptance.rs`.

use sdrkit_core::{Complex32, InputInfo, MemStorage, Receiver, Value};
use sdrkit_proto_tones::mdc::{burst_audio, Packet};
use sdrkit_stack_voice::typed;
use sdrkit_stack_voice::scene::{SceneConfig, Transmission};
use sdrkit_stack_voice::MDC1200;
use std::sync::Arc;

mod common;
use common::{feed, generated_of, receiver_of, samples};

const CENTRE: f64 = 155e6;

fn receiver(set: &[(&str, Value)]) -> Box<dyn Receiver> {
    receiver_of(&MDC1200, None, set, Arc::new(MemStorage::new()))
}

/// One row of the `bursts` table.
#[derive(Clone, Debug, PartialEq)]
struct BurstRow {
    unit: String,
    arg: String,
    hz: f64,
    text: String,
}

/// The bursts of `rx`, oldest first.
fn bursts(rx: &dyn Receiver) -> Vec<BurstRow> {
    let r = rx.results();
    let table = r
        .decl()
        .table_id("bursts")
        .and_then(|t| r.table(t))
        .unwrap();
    let mut rows: Vec<(i64, BurstRow)> = table
        .rows_newest_first()
        .map(|(_, rec)| {
            (
                rec.at(&typed::CallRow::SERIAL).unwrap(),
                BurstRow {
                    unit: rec.at(&typed::Mdc1200BurstRow::UNIT_ID).unwrap().to_owned(),
                    arg: rec.at(&typed::Mdc1200BurstRow::ARG).unwrap().to_owned(),
                    hz: rec.at(&typed::ChannelRow::HZ).unwrap(),
                    text: rec.at(&typed::Mdc1200BurstRow::TEXT).unwrap().to_owned(),
                },
            )
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    rows.into_iter().map(|r| r.1).collect()
}

fn rows(rx: &dyn Receiver, table: &str) -> usize {
    let r = rx.results();
    r.decl()
        .table_id(table)
        .and_then(|t| r.table(t))
        .map_or(0, |t| t.rows_newest_first().count())
}

fn field(
    rx: &dyn Receiver,
    f: &sdrkit_core::interface::Column<sdrkit_core::publish::field::I64>,
) -> i64 {
    rx.results().fields().at(f).unwrap_or(0)
}

/// A narrow FM transmission `offset_hz` from the centre: a PTT ID burst of `unit` with `arg`, then speech.
fn talker(offset_hz: f64, start_s: f64, arg: u8, unit: u16, amplitude: f32) -> Transmission {
    Transmission {
        amplitude,
        deviation: 0.7,
        burst: Some(burst_audio(&Packet::new(0x01, arg, unit), 48_000.0, 0.2, 7)),
        seed: u64::from(unit),
        ..Transmission::on(CENTRE + offset_hz).at(start_s, 1.6)
    }
}

fn scene(rate: f64, length_s: f64, noise_rms: f64, tx: Vec<Transmission>) -> SceneConfig {
    SceneConfig {
        rate_hz: rate,
        centre_hz: CENTRE,
        length_s,
        noise_rms,
        tx,
        seed: 7,
    }
}

/// Two carriers 75 kHz apart at 1 Msps, the second 1100 Hz off its raster frequency.
fn two_talkers(noise_rms: f64, amplitude: f32) -> SceneConfig {
    let mut second = talker(50e3, 1.0, 0x00, 0xBEEF, amplitude);
    second.offset_hz = 1100.0;
    scene(
        1e6,
        4.2,
        noise_rms,
        vec![talker(-25e3, 0.3, 0x80, 0x1234, amplitude), second],
    )
}

#[test]
fn two_carriers_each_with_a_burst_and_voice() {
    let cfg = two_talkers(0.005, 0.15);
    let mut rx = receiver(&[]);
    let h = feed(rx.as_mut(), &samples(&cfg), 1e6, CENTRE, 65_536);
    let got = bursts(rx.as_ref());
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!((got[0].unit.as_str(), got[0].arg.as_str()), ("1234", "80"));
    assert_eq!(got[0].text, "PTT ID (start of transmission)");
    assert!((got[0].hz - (CENTRE - 25e3)).abs() < 4e3, "{}", got[0].hz);
    assert_eq!((got[1].unit.as_str(), got[1].arg.as_str()), ("BEEF", "00"));
    assert_eq!(got[1].text, "PTT ID (end of transmission)");
    assert!((got[1].hz - (CENTRE + 51.1e3)).abs() < 4e3, "{}", got[1].hz);
    assert_eq!(field(rx.as_ref(), &typed::Mdc1200Fields::BURSTS), 2);
    assert_eq!(field(rx.as_ref(), &typed::VoiceChannelsFields::CALLS), 2);
    assert_eq!(rows(rx.as_ref(), "calls"), 2);
    assert_eq!(
        rx.results().status().summary,
        "MDC-1200: 0 channels active, 2 calls, 2 bursts"
    );
    // Every carrier is a carrier off the plan, labelled by its frequency.
    assert_eq!(rows(rx.as_ref(), "channels"), 2);
    let burst_events: Vec<_> = h.events.iter().filter(|e| e.is(&typed::MDC1200_BURST)).collect();
    assert_eq!(burst_events.len(), 2);
    assert_eq!(burst_events[0].fields.at(&typed::Mdc1200BurstRow::UNIT_ID), Some("1234"));
    assert_eq!(h.events.iter().filter(|e| e.is(&typed::MDC1200_CALL)).count(), 2);
    // The strongest open carrier plays.
    assert!(h.audio.iter().any(|v| *v != 0.0));
    let log: Vec<String> = rx.results().log_lines().iter().cloned().collect();
    // The graph puts the name of the component before a line that does not come from the call log.
    assert!(
        log.iter().any(
            |l| l == "bursts: MDC-1200 154.9750 MHz: unit 1234, PTT ID (start of transmission)"
        ),
        "{log:?}"
    );
}

#[test]
fn a_burst_has_the_time_of_its_signal() {
    // One transmission from 2.0 s: its burst ends 0.17 s later.
    let cfg = scene(
        1e6,
        4.5,
        0.005,
        vec![talker(-25e3, 2.0, 0x80, 0x1234, 0.15)],
    );
    let mut rx = receiver(&[("time_base", Value::Time(1_000_000.0))]);
    let h = feed(rx.as_mut(), &samples(&cfg), 1e6, CENTRE, 65_536);
    let burst = h.events.iter().find(|e| e.is(&typed::MDC1200_BURST)).unwrap();
    assert!((burst.time - 2.2).abs() < 0.1, "{}", burst.time);
    let wall = burst.fields.at(&typed::CallRow::TIME_UTC).unwrap();
    assert!((wall - 1_000_002.2).abs() < 0.1, "{wall}");
    let r = rx.results();
    let table = r
        .decl()
        .table_id("bursts")
        .and_then(|t| r.table(t))
        .unwrap();
    let (_, row) = table.rows_newest_first().next().unwrap();
    assert_eq!(row.at(&typed::CallRow::TIME_UTC), Some(wall));
}

#[test]
fn carrier_and_clock_errors() {
    for (offset, ppm) in [(2000.0, 50.0), (-2000.0, -50.0)] {
        let mut cfg = two_talkers(0.005, 0.15);
        for t in &mut cfg.tx {
            t.offset_hz += offset;
        }
        // The radio's clock is off: it samples at another rate than it states.
        cfg.rate_hz = 1e6 * (1.0 + ppm * 1e-6);
        let mut rx = receiver(&[]);
        feed(rx.as_mut(), &samples(&cfg), 1e6, CENTRE, 65_536);
        let units: Vec<String> = bursts(rx.as_ref()).into_iter().map(|b| b.unit).collect();
        assert_eq!(units, ["1234", "BEEF"], "{offset:+} Hz, {ppm:+} ppm");
    }
}

#[test]
fn noise_twelve_db_below_the_signal() {
    // Noise 12 dB below the signal in the 16 kHz that hold a narrow FM signal: at the FM threshold. Required and
    // accepted: the squelch stays closed, so nothing is decoded and no call is logged.
    let amplitude = 0.1f32;
    let noise_rms = (f64::from(amplitude * amplitude) / 10f64.powf(1.2) * 1e6 / 16e3 / 2.0).sqrt();
    let cfg = two_talkers(noise_rms, amplitude);
    let mut rx = receiver(&[]);
    feed(rx.as_mut(), &samples(&cfg), 1e6, CENTRE, 65_536);
    assert_eq!(bursts(rx.as_ref()).len(), 0);
    assert_eq!(rows(rx.as_ref(), "calls"), 0);
}

#[test]
fn the_test_signal_gives_two_bursts_at_default_settings() {
    // The 6 s of the scene once, at the rate of the stack.
    let x = generated_of(&MDC1200, 2e6, 6.0, 1);
    let mut rx = receiver(&[]);
    feed(rx.as_mut(), &x, 2e6, common::dial("mdc1200", None), 65_536);
    let got = bursts(rx.as_ref());
    let units: Vec<&str> = got.iter().map(|b| b.unit.as_str()).collect();
    assert_eq!(units, ["1A2B", "C0DE"], "{got:?}");
    assert!((got[0].hz - (155e6 - 37.5e3)).abs() < 4e3);
    assert!((got[1].hz - (155e6 + 62.5e3)).abs() < 4e3);
    assert_eq!(rows(rx.as_ref(), "calls"), 2);
}

#[test]
fn noise_alone_gives_no_burst_and_no_call() {
    for (rate, rms) in [(1e6, 0.02), (2e6, 0.05)] {
        let cfg = scene(rate, 6.0, rms, Vec::new());
        let mut rx = receiver(&[]);
        feed(rx.as_mut(), &samples(&cfg), rate, CENTRE, 65_536);
        assert_eq!(bursts(rx.as_ref()).len(), 0, "{rate}");
        assert_eq!(rows(rx.as_ref(), "calls"), 0, "{rate}");
        assert_eq!(field(rx.as_ref(), &typed::Mdc1200Fields::BURSTS), 0);
    }
}

#[test]
fn a_rate_below_the_lowest_is_refused_with_a_message() {
    let mut rx = receiver(&[]);
    let at = |rx: &mut dyn Receiver, rate: f64| {
        rx.configure(&InputInfo::new(rate, CENTRE, CENTRE).recording())
    };
    assert!(at(rx.as_mut(), 200e3).is_err());
    assert_eq!(
        rx.results().status().summary,
        "MDC-1200: the sample rate is too low (250 kHz at least)"
    );
    assert!(at(rx.as_mut(), 250e3).is_ok());
}

#[test]
fn each_setting_refuses_a_value_outside_its_range() {
    let mut rx = receiver(&[]);
    let mut set = |key: &str, v: Value| {
        let id = rx.schema().id(key).unwrap();
        sdrkit_core::owner::apply(rx.as_mut(), id, &v)
    };
    for (key, bad, good) in [
        ("squelch", Value::F64(101.0), Value::F64(100.0)),
        ("squelch", Value::F64(-1.0), Value::F64(0.0)),
        ("carriers", Value::I64(0), Value::I64(1)),
        ("carriers", Value::I64(25), Value::I64(24)),
        ("time_base", Value::Time(-1.0), Value::Time(0.0)),
        ("raster_step_hz", Value::Hz(-1.0), Value::Hz(25e3)),
        ("raster_step_hz", Value::Hz(250e3), Value::Hz(12.5e3)),
        ("raster_origin_hz", Value::Hz(-1.0), Value::Hz(155e6)),
        ("channel_width_hz", Value::Hz(60e3), Value::Hz(25e3)),
        ("gen_cnr_db", Value::F64(81.0), Value::F64(80.0)),
    ] {
        assert!(set(key, bad.clone()).is_err(), "{key} takes {bad:?}");
        assert!(set(key, good.clone()).is_ok(), "{key} refuses {good:?}");
    }
    // The stack has no plan, no channel to pick, no demodulator choice and no scan.
    for key in ["plan", "channel", "demod", "scan", "region"] {
        assert!(rx.schema().id(key).is_err(), "{key}");
    }
}

#[test]
fn clear_empties_the_lists() {
    let cfg = two_talkers(0.005, 0.15);
    let mut rx = receiver(&[]);
    let x: Vec<Complex32> = samples(&cfg);
    feed(rx.as_mut(), &x, 1e6, CENTRE, 65_536);
    assert_eq!(bursts(rx.as_ref()).len(), 2);
    assert_eq!(rows(rx.as_ref(), "calls"), 2);
    let id = rx.commands().id("clear").unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(rx.as_mut(), id, &args).unwrap();
    assert_eq!(bursts(rx.as_ref()).len(), 0);
    assert_eq!(rows(rx.as_ref(), "calls"), 0);
    assert_eq!(field(rx.as_ref(), &typed::Mdc1200Fields::BURSTS), 0);
    assert_eq!(field(rx.as_ref(), &typed::Mdc1200Fields::CHECK_FAILURES), 0);
    assert_eq!(field(rx.as_ref(), &typed::VoiceChannelsFields::CALLS), 0);
}

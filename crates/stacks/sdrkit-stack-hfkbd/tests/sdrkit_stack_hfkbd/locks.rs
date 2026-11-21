//! Every stack locks on its own test signal at default settings, with the radio 20 kHz above the dial as a session tunes it.

use crate::common::*;
use sdrkit_core::{InputInfo, LockState, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_hfkbd as hf;

/// A rate that holds the tune offset of the family and the audio band.
const TUNED_RATE: f64 = 48_000.0;
const DIAL: f64 = 14.07e6;

/// What a stack made of its test signal.
struct Seen {
    /// Seconds of signal before the first locked report, if any.
    locked_at: Option<f64>,
    /// The lock state at the end.
    last: LockState,
    /// The results at the end.
    results: sdrkit_core::Results,
}

/// Feeds `seconds` of the test signal of `desc` at default settings and an SNR of `snr_db` (none: no noise) to its receiver,
/// tuned as a session tunes it: the station [`hf::decoder::TUNE_OFFSET_HZ`] below the centre of the samples.
fn tuned_run(desc: &'static StackDescriptor, seconds: f64, snr_db: Option<f64>) -> Seen {
    let make = desc.make_generator.expect("a test signal");
    let plain = Harness::new(desc).rate(TUNED_RATE);
    let station = make(&sdrkit_core::GenConfig::new(
        TUNED_RATE,
        plain.values().unwrap(),
    ))
    .unwrap()
    .station_offset_hz(desc.tuning.tune_offset_hz);
    let mut h = plain
        .set(&GEN_OFFSET_HZ, -desc.tuning.tune_offset_hz - station)
        .seconds(seconds);
    if let Some(snr) = snr_db {
        h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, snr);
    }
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(TUNED_RATE, DIAL + desc.tuning.tune_offset_hz, DIAL).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    let mut locked_at = None;
    let piece = (TUNED_RATE / 10.0) as usize;
    for (i, c) in x.chunks(piece).enumerate() {
        out.clear();
        rx.feed(c, &mut out);
        if locked_at.is_none() && rx.results().status().lock_state == LockState::Locked {
            locked_at = Some((i + 1) as f64 * 0.1);
        }
    }
    Seen {
        locked_at,
        last: rx.results().status().lock_state,
        results: rx.results().clone(),
    }
}

/// The text of every row of table `table` of `res`, in one string.
fn table_text(res: &sdrkit_core::Results, table: &str) -> String {
    let Some(t) = res.decl().table_id(table).and_then(|id| res.table(id)) else {
        return String::new();
    };
    t.rows_newest_first()
        .map(|(_, row)| cell_text(row, "text") + "\n")
        .collect()
}

/// What the test signal of a stack must give at default settings.
enum Want {
    /// This text in the field `text`.
    Text(&'static str),
    /// This text in a row of the table `signals`.
    Signal(&'static str),
    /// A count field above 0.
    Count(&'static str),
    /// The lock alone.
    Lock,
}

/// Each stack locks on its test signal at 20 dB and reads what the signal sends.
#[test]
fn every_stack_locks_on_its_test_signal_and_reads_it() {
    let cases: [(&'static StackDescriptor, f64, Want); 20] = [
        (&hf::RTTY, 20.0, Want::Text("DE K0TST TEST 0123456789")),
        (
            &hf::SITOR,
            20.0,
            Want::Text("TEST MESSAGE FROM K0TST 0123456789"),
        ),
        (&hf::PSK, 20.0, Want::Signal("DE LY2ABC LY2ABC PSE K")),
        (&hf::CW, 20.0, Want::Signal("DE K0TST")),
        (&hf::MFSK, 30.0, Want::Text("K0TST TEST MESSAGE 0123456789")),
        (
            &hf::DOMINOEX,
            30.0,
            Want::Text("K0TST TEST MESSAGE 0123456789"),
        ),
        (&hf::THOR, 30.0, Want::Text("K0TST TEST MESSAGE 0123456789")),
        (
            &hf::OLIVIA,
            30.0,
            Want::Text("K0TST TEST MESSAGE 0123456789"),
        ),
        (
            &hf::CONTESTIA,
            30.0,
            Want::Text("K0TST TEST MESSAGE 0123456789"),
        ),
        (
            &hf::MT63,
            20.0,
            Want::Text("DE K0TST K0TST MT63 TEST PSE K"),
        ),
        (&hf::FSQ, 20.0, Want::Text("hello from the test signal")),
        (&hf::IFKP, 20.0, Want::Text("ifkp test message 0123456789")),
        (&hf::HELL, 10.0, Want::Count("columns")),
        (&hf::SSTV, 20.0, Want::Count("picture_rows")),
        (&hf::ARDOP, 20.0, Want::Text("TEST DATA TWO FROM K0TST")),
        (&hf::ROS, 20.0, Want::Count("detections")),
        (&hf::THROB, 30.0, Want::Text("CQ CQ DE K0TST")),
        (&hf::QRSS, 45.0, Want::Lock),
        (&hf::CP16, 30.0, Want::Text("CQ CP16 DE")),
        (&hf::WINMOR, 10.0, Want::Count("pilots")),
    ];
    for (d, seconds, want) in cases {
        let s = tuned_run(d, seconds, Some(20.0));
        assert!(s.locked_at.is_some(), "{}: only reached {:?}", d.id, s.last);
        let r = &s.results;
        match want {
            Want::Text(t) => {
                let got = field_text(r, "text");
                assert!(got.contains(t), "{}: {t:?} not in {got:?}", d.id);
            }
            Want::Signal(t) => {
                let got = table_text(r, "signals");
                assert!(got.contains(t), "{}: {t:?} not in {got:?}", d.id);
            }
            Want::Count(key) => assert!(field_num(r, key) > 0.0, "{}: no {key}", d.id),
            Want::Lock => {}
        }
    }
}

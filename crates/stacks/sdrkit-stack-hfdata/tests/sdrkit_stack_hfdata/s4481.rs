//! STANAG 4481 FSK: one channel at each speed, the multi-channel test signal, 16 channels at once, noise alone.

use crate::common::*;
use sdrkit_core::Complex32;
use sdrkit_stack_hfdata::s4481::rx::subcarrier;
use sdrkit_stack_hfdata::s4481::{start_stop, FskGen, Keyed, S4481};
use sdrkit_stack_hfdata::serial::TEST_TEXT;
use sdrkit_stack_hfdata::typed::S4481_CHANNELS;

fn channels(rx: &dyn sdrkit_core::Receiver) -> Vec<Row> {
    rows(rx.results(), &S4481_CHANNELS)
}

fn row(rows: &[Row], ch: i64) -> &Row {
    rows.iter()
        .find(|r| num(r, "channel") as i64 == ch)
        .expect("the channel row")
}

#[test]
fn one_channel_at_each_speed() {
    for (baud, key) in [(45.45, "45.45"), (50.0, "50"), (75.0, "75"), (100.0, "100")] {
        let secs = 0.5 + TEST_TEXT.len() as f64 * 8.0 / baud + 1.0;
        let x = test_signal(
            &S4481,
            24e3,
            &[("gen_multi", json!(false)), ("gen_baud", json!(key))],
            secs,
        );
        let ch = Channel {
            snr_db: Some(15.0),
            offset_hz: 23.0,
            seed: 3,
            ..Channel::default()
        };
        let (rx, out) = run_24k(&S4481, &through(&x, 24e3, &ch), 4801);
        let r = channels(rx.as_ref());
        let single = row(&r, 0);
        assert!(
            text(single, "last_text").contains(TEST_TEXT),
            "{baud}: {:?} {:?}",
            text(single, "last_text"),
            event_texts(&out, "s4481.event")
        );
        assert!((num(single, "mark_hz") - 1298.0).abs() < 10.0, "{baud}");
        assert!((num(single, "space_hz") - 2148.0).abs() < 10.0, "{baud}");
        assert_eq!(num(single, "speed_bd"), baud);
    }
}

#[test]
fn the_multi_channel_test_signal() {
    let x = test_signal(&S4481, 24e3, &[], 9.5);
    let ch = Channel {
        snr_db: Some(20.0),
        seed: 5,
        ..Channel::default()
    };
    let (rx, out) = run_24k(&S4481, &through(&x, 24e3, &ch), 4801);
    let r = channels(rx.as_ref());
    for c in [3, 11] {
        assert!(
            text(row(&r, c), "last_text").contains(TEST_TEXT),
            "{c}: {:?}",
            text(row(&r, c), "last_text")
        );
        assert_eq!(text(row(&r, c), "framing"), "start-stop");
    }
    let sync = row(&r, 7);
    assert_eq!(text(sync, "framing"), "sync");
    assert_eq!(num(sync, "speed_bd"), 75.0);
    let ev = event_texts(&out, "s4481.event");
    assert!(ev.iter().any(|e| e == "channel 3 (765 Hz) on"), "{ev:?}");
    assert!(
        ev.iter().any(|e| e == &format!("[ch 11] {TEST_TEXT}")),
        "{ev:?}"
    );
    assert_eq!(field_text(rx.as_ref(), "reception"), "multi");
}

#[test]
fn sixteen_channels_of_text() {
    let chans: Vec<Keyed> = (1..=16)
        .map(|n| Keyed {
            mark_hz: subcarrier(n) - 42.5,
            space_hz: subcarrier(n) + 42.5,
            elements: start_stop(TEST_TEXT, 50.0, 0.5),
        })
        .collect();
    let secs = 0.5 + TEST_TEXT.len() as f64 * 8.0 / 50.0 + 1.0;
    let mut g = FskGen::new(24e3, chans, secs);
    let x = generate(&mut g, secs);
    let ch = Channel {
        snr_db: Some(25.0),
        seed: 7,
        ..Channel::default()
    };
    let (rx, _) = run_24k(&S4481, &through(&x, 24e3, &ch), 4801);
    let r = channels(rx.as_ref());
    for c in 1..=16 {
        assert!(
            text(row(&r, c), "last_text").contains(TEST_TEXT),
            "{c}: {:?}",
            text(row(&r, c), "last_text")
        );
    }
}

#[test]
fn noise_gives_nothing() {
    let x = vec![Complex32::default(); 24_000 * 5];
    let ch = Channel {
        snr_db: Some(0.0),
        seed: 9,
        ..Channel::default()
    };
    let (rx, _) = run_24k(&S4481, &through(&x, 24e3, &ch), 4801);
    assert_eq!(field_text(rx.as_ref(), "text"), "");
    assert_eq!(field_text(rx.as_ref(), "reception"), "none");
}

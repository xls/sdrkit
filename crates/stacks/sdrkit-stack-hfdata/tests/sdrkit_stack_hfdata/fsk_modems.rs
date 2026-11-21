//! Telephone FSK modems: the worked example and the CHU frame of the documents, each standard in both sidebands with dial and
//! clock errors, CHU-like frames, bursts at stated levels, and no row on noise, tones that are not a standard, or a wrong speed.

use crate::common::*;
use crate::senders::*;
use sdrkit_stack_hfdata::fsk_modems::{self, Standard, FSK_MODEMS};
use sdrkit_stack_hfdata::senders::Tone;

/// A channel for a signal of amplitude 0.5: `snr` is the tone power over the noise in 3 kHz.
fn ch(snr: Option<f64>, offset_hz: f64, ppm: f64, seed: u64) -> Channel {
    Channel {
        snr_db: snr.map(|s| s + 10.0 * 4f64.log10()),
        offset_hz,
        ppm,
        seed,
        ..Channel::default()
    }
}

fn messages(seq: &[Tone], rate: f64, c: &Channel) -> Vec<(String, String)> {
    let dial = dial_of(rate);
    let x = signal(seq, rate, dial, c);
    let (rx, _) = run_at(&FSK_MODEMS, &x, rate, dial, &[]);
    sender_rows(rx.as_ref())
}

/// The bytes of the text of a message row.
fn bytes_of(text: &str) -> Vec<u8> {
    let hex = text.split(" | ").next().unwrap_or("");
    let hex = hex.split(": ").nth(1).unwrap_or("");
    hex.split(' ')
        .filter_map(|h| u8::from_str_radix(h, 16).ok())
        .collect()
}

#[test]
fn the_worked_example_reads_test() {
    let seq = fsk_modems::test_sequence(
        Standard::Bell103Ans,
        b"TEST",
        1,
        0,
        0.4,
        1.0,
        1.0,
        false,
        1.0,
    );
    let rows = messages(&seq, 24e3, &ch(None, 0.0, 0.0, 1));
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(
        rows[0].1.starts_with(
            "4 bytes: 54 45 53 54 | TEST | tones 2225 / 2025 Hz (upper sideband, offset +0 Hz), 0 framing errors,"
        ),
        "{rows:?}"
    );
    // A CHU frame: 10 characters of one start bit, eight data bits and two stop bits at 300 bit/s last 366.67 ms.
    let frame = fsk_modems::test_sequence(
        Standard::Bell103Ans,
        &[0u8; 10],
        2,
        0,
        0.0,
        0.0,
        0.0,
        false,
        1.0,
    );
    let secs: f64 = frame.iter().map(|t| t.secs).sum();
    assert!((secs - 0.36667).abs() < 1e-4, "{secs}");
}

#[test]
fn each_standard_in_both_sidebands_with_dial_and_clock_errors() {
    let bytes = fsk_modems::test_bytes(24, 3);
    let mut k = 0;
    for st in Standard::ALL {
        for lower in [false, true] {
            for (dial, ppm, gap) in [(0.0, 0.0, 0), (28.0, 10_000.0, 1), (-30.0, -10_000.0, 1)] {
                k += 1;
                let stops = 1 + k % 2;
                let seq = stretch(
                    &fsk_modems::test_sequence(st, &bytes, stops, gap, 0.4, 1.0, 1.0, lower, 1.0),
                    ppm,
                );
                let rows = messages(&seq, 48e3, &ch(Some(25.0), dial, 0.0, k as u64));
                let side = if lower { "lower" } else { "upper" };
                assert!(
                    rows.len() == 1
                        && rows[0].0 == "Message"
                        && bytes_of(&rows[0].1) == bytes
                        && rows[0].1.contains(&format!("({side} sideband"))
                        && sender_ids(&rows_rx(&seq, k)) == [st.name()],
                    "{st:?} {side} dial {dial} ppm {ppm}: {rows:?}"
                );
            }
        }
    }
}

/// The ids of the rows of a run (the standard).
fn sender_ids(ids: &[String]) -> Vec<&str> {
    ids.iter().map(String::as_str).collect()
}

/// The ids of the rows of `seq` at 48 ksps and 25 dB.
fn rows_rx(seq: &[Tone], seed: usize) -> Vec<String> {
    let dial = dial_of(48e3);
    let x = signal(seq, 48e3, dial, &ch(Some(25.0), 0.0, 0.0, seed as u64));
    let (rx, _) = run_at(&FSK_MODEMS, &x, 48e3, dial, &[]);
    use sdrkit_stack_hfdata::typed::{SenderRow, SENDER_ROWS};
    SENDER_ROWS
        .rows::<SenderRow>(rx.results())
        .map(|(_, r)| r.id.to_string())
        .collect()
}

#[test]
fn chu_like_frames_read_at_22_db() {
    let (mut perfect, mut near) = (0, 0);
    for k in 0..6u64 {
        let bytes = fsk_modems::test_bytes(10, 40 + k);
        let seq = fsk_modems::test_sequence(
            Standard::Bell103Ans,
            &bytes,
            2,
            0,
            0.06,
            1.0,
            1.0,
            false,
            1.0,
        );
        let dial = if k % 2 == 0 { 22.0 } else { -17.0 };
        let rows = messages(&seq, 24e3, &ch(Some(22.0), dial, 0.0, 40 + k));
        let got = rows.first().map(|r| bytes_of(&r.1)).unwrap_or_default();
        if got == bytes {
            perfect += 1;
            near += 1;
        } else if got.len() == 9 && (got[..] == bytes[1..] || got[..] == bytes[..9]) {
            near += 1;
        }
    }
    assert!(
        perfect >= 4 && near >= 5,
        "{perfect} perfect, {near} perfect or one missing"
    );
}

#[test]
fn bursts_at_stated_levels() {
    // Eb/N0 is the SNR in 3 kHz plus 10 dB (300 Bd) or 4 dB (1200 Bd): at 10 dB a burst of 24 bytes reads whole with a
    // probability near 1 for both; at 6 and 3 dB a burst may be missed but a message shown has every byte right.
    for st in [Standard::Bell103Ans, Standard::Bell202] {
        for snr in [30.0, 20.0, 14.0, 10.0, 6.0, 3.0] {
            let seeds = if snr <= 10.0 { 60..68u64 } else { 60..63 };
            let n = seeds.end - seeds.start;
            let (mut msgs, mut whole, mut wrong) = (0, 0, 0);
            for b in seeds {
                let bytes = fsk_modems::test_bytes(24, b);
                let seq = fsk_modems::test_sequence(st, &bytes, 1, 0, 0.4, 1.0, 1.0, false, 1.0);
                let rows = messages(&seq, 24e3, &ch(Some(snr), 0.0, 0.0, b));
                for r in &rows {
                    msgs += 1;
                    let got = bytes_of(&r.1);
                    let bad = bytes
                        .iter()
                        .zip(got.iter().chain(std::iter::repeat(&0u8)))
                        .filter(|(a, b)| a != b)
                        .count();
                    wrong += bad;
                    if bad == 0 && got.len() == bytes.len() {
                        whole += 1;
                    }
                }
                if rows.is_empty() {
                    wrong += 24;
                }
            }
            println!(
                "{st:?} {snr} dB: {msgs} messages, {whole} whole of {n}, {wrong} wrong bytes of {}",
                24 * n
            );
            if snr >= 14.0 {
                assert!(
                    msgs == 3 && wrong <= 3,
                    "{st:?} {snr} dB: {msgs} messages, {wrong} wrong"
                );
            } else if snr >= 10.0 {
                assert!(3 * whole >= 2 * n, "{st:?} {snr} dB: {whole} whole of {n}");
            } else {
                assert_eq!(msgs, whole, "{st:?} {snr} dB: a message with a wrong byte");
            }
        }
    }
}

#[test]
fn noise_tones_and_a_wrong_speed_give_no_message() {
    for (seq, what) in [
        (vec![Tone::silence(20.0)], "noise"),
        (
            vec![
                Tone::silence(1.0),
                Tone {
                    secs: 10.0,
                    hz: Some(1500.0),
                    amp: 0.5,
                },
                Tone::silence(1.0),
            ],
            "a tone",
        ),
        (
            vec![
                Tone::silence(1.0),
                Tone {
                    secs: 2.0,
                    hz: Some(1000.0),
                    amp: 0.5,
                },
                Tone {
                    secs: 2.0,
                    hz: Some(1700.0),
                    amp: 0.5,
                },
                Tone::silence(1.0),
            ],
            "two tones",
        ),
    ] {
        let seeds = if what == "noise" { 1..9u64 } else { 7..8 };
        for seed in seeds {
            let rows = messages(&seq, 24e3, &ch(Some(20.0), 0.0, 0.0, seed));
            assert!(rows.is_empty(), "{what} seed {seed}: {rows:?}");
        }
    }
    let bytes = fsk_modems::test_bytes(24, 9);
    let seq = fsk_modems::test_sequence(Standard::V21Ch1, &bytes, 1, 0, 0.4, 1.0, 1.0, false, 2.0);
    let rows = messages(&seq, 24e3, &ch(Some(25.0), 0.0, 0.0, 9));
    for r in &rows {
        let got = bytes_of(&r.1);
        let right = bytes.iter().zip(&got).filter(|(a, b)| a == b).count();
        assert!(right < 8, "600 baud: {right} bytes right: {r:?}");
    }
}

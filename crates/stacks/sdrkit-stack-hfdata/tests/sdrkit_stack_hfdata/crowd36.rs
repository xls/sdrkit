//! CROWD36: the character layer on the worked example, the receiver on its test signal at stated levels, and no rows on noise
//! or a carrier.

use crate::common::*;
use crate::senders::*;
use sdrkit_stack_hfdata::crowd36::{self, Read, Reader, CROWD36};
use sdrkit_stack_hfdata::senders::Tone;

/// The tone numbers of `text` in the letters case.
fn letters(text: &str) -> Vec<i64> {
    text.chars()
        .map(|c| crowd36::letter_tone(c).unwrap())
        .collect()
}

fn read_all(tones: &[i64], high: i64) -> Vec<Read> {
    let mut r = Reader::new(high);
    let mut out = Vec::new();
    for &t in tones {
        r.push(t, &mut out);
    }
    r.flush(&mut out);
    out
}

#[test]
fn the_character_layer_reads_the_worked_example() {
    let mut t: Vec<i64> = [24, 10].repeat(6);
    t.push(26);
    t.extend(letters("QUEEN "));
    assert_eq!(t[12..], [26, 0, 20, 4, 4, 29, 6]);
    t.extend([14, 0, 2, 4]);
    let got = read_all(&t, 24);
    assert_eq!(
        got,
        vec![Read::Sync(24, 10, 0), Read::Text("QUEEN 123".into())]
    );
    // The station's high tone is 25 and the characters are on their own tones: with the setting 24 every tone is read one
    // low (the letter shift reads as H, Q falls outside the table): the line H?ZVVO holds no group and is refused.
    let mut t: Vec<i64> = [25, 11].repeat(6);
    t.push(26);
    t.extend(letters("QUEEN"));
    let mut r = Reader::new(24);
    let mut got = Vec::new();
    t.iter().for_each(|&k| r.push(k, &mut got));
    r.flush(&mut got);
    assert_eq!((got, r.refused), (vec![Read::Sync(24, 10, 1)], 1));
    let got = read_all(&t, 25);
    assert_eq!(got, vec![Read::Sync(25, 11, 0), Read::Text("QUEEN".into())]);
}

#[test]
fn no_text_before_the_first_sync_and_sync_symbols_are_not_text() {
    let mut t = letters("QUEEN QUEEN");
    t.extend([24, 10].repeat(10));
    t.push(26);
    t.extend(letters("ABCDE"));
    let got = read_all(&t, 24);
    assert_eq!(got, vec![Read::Sync(24, 10, 0), Read::Text("ABCDE".into())]);
}

/// The test signal of `n` groups at `baud` with the sync tones 24 and 10: the groups and the tone sequence.
fn message(n: usize, seed: u64) -> (Vec<String>, Vec<Tone>) {
    let groups = crowd36::test_groups(n, seed);
    let mut seq = vec![Tone::silence(1.0)];
    seq.extend(
        crowd36::test_tones(&groups, 24, 10)
            .into_iter()
            .map(|k| Tone::new(1.0 / 40.0, crowd36::tone_hz(k))),
    );
    seq.push(Tone::silence(1.5));
    (groups, seq)
}

/// Groups read right of 40 sent, and Sync rows.
fn case(rate: f64, dial_off: f64, ch: &Channel, seed: u64) -> (usize, usize) {
    let (groups, seq) = message(40, seed);
    let x = signal(&seq, rate, dial_off, ch);
    let (rx, _) = run_at(&CROWD36, &x, rate, dial_off, &[]);
    let rows = sender_rows(rx.as_ref());
    (
        groups_right(&groups, &texts_of(&rows, "Text")),
        texts_of(&rows, "Sync").len(),
    )
}

#[test]
fn the_receiver_reads_its_test_signal_at_10_db() {
    let ch = |offset_hz, drift_hz_s, ppm| Channel {
        snr_db: Some(10.0),
        offset_hz,
        drift_hz_s,
        ppm,
        seed: 5,
        ..Channel::default()
    };
    for (rate, dial, c, what) in [
        (24e3, 0.0, ch(13.6, 0.0, 0.0), "dial +13.6 Hz"),
        (
            48e3,
            dial_of(48e3),
            ch(-800.0, 0.0, 0.0),
            "800 Hz low at 48 ksps",
        ),
        (
            24e3,
            0.0,
            ch(17.0, 0.2, -500.0),
            "clock -500 ppm, drift +0.2 Hz/s, dial +17 Hz",
        ),
        (
            250e3,
            45e3 - 9.0,
            ch(0.0, 0.0, 0.0),
            "250 ksps, receiver offset +45 kHz",
        ),
    ] {
        let (right, syncs) = case(rate, dial, &c, 3);
        assert!(
            right >= 39 && syncs >= 1,
            "{what}: {right} of 40, {syncs} sync rows"
        );
    }
}

#[test]
fn the_sweep_reaches_90_percent_at_minus_2_db() {
    let mut curve = Vec::new();
    for step in 0..8 {
        let snr = -16.0 + 2.0 * f64::from(step);
        let mut right = 0;
        for seed in [11u64, 12] {
            let ch = Channel {
                snr_db: Some(snr),
                offset_hz: -7.0,
                ppm: 100.0,
                seed,
                ..Channel::default()
            };
            right += case(24e3, 0.0, &ch, seed).0;
        }
        curve.push((snr, right as f64 / 80.0));
    }
    println!("crowd36 sweep: {curve:?}");
    assert!(curve[7].1 >= 0.9, "{curve:?}");
}

#[test]
fn noise_and_a_carrier_give_no_row() {
    for seed in [21u64, 22] {
        let x = signal(
            &[Tone::silence(60.0)],
            24e3,
            0.0,
            &Channel {
                snr_db: Some(0.0),
                seed,
                ..Channel::default()
            },
        );
        let (rx, _) = run_at(&CROWD36, &x, 24e3, 0.0, &[]);
        let rows = sender_rows(rx.as_ref());
        assert!(rows.is_empty(), "noise {seed}: {rows:?}");
    }
    let x = signal(
        &[Tone::new(40.0, 2000.0)],
        24e3,
        0.0,
        &Channel {
            snr_db: Some(0.0),
            seed: 23,
            ..Channel::default()
        },
    );
    let (rx, _) = run_at(&CROWD36, &x, 24e3, 0.0, &[]);
    let rows = sender_rows(rx.as_ref());
    assert!(rows.is_empty(), "carrier: {rows:?}");
}

#[test]
fn the_default_test_signal_locks() {
    let x = test_signal(&CROWD36, 24e3, &[], 14.0);
    let (rx, _) = run_at(&CROWD36, &x, 24e3, 0.0, &[]);
    let rows = sender_rows(rx.as_ref());
    assert!(texts_of(&rows, "Text").len() >= 4, "{rows:?}");
    assert_eq!(field(rx.as_ref(), "state"), Some(2.0));
}

#[test]
fn an_xpa_signal_gives_no_row() {
    use sdrkit_stack_hfdata::xpa::{self, Submode};
    let seq = xpa::test_sequence(Submode::Xpa, &xpa::test_groups(30, 1), 10.0, false);
    let x = signal(
        &seq,
        24e3,
        0.0,
        &Channel {
            snr_db: Some(10.0),
            seed: 24,
            ..Channel::default()
        },
    );
    let (rx, _) = run_at(&CROWD36, &x, 24e3, 0.0, &[]);
    let rows = sender_rows(rx.as_ref());
    assert!(rows.is_empty(), "{rows:?}");
}

#[test]
fn the_receiver_reads_20_bd_at_10_db() {
    let groups = crowd36::test_groups(40, 9);
    let mut seq = vec![Tone::silence(1.0)];
    seq.extend(
        crowd36::test_tones(&groups, 24, 10)
            .into_iter()
            .map(|k| Tone::new(1.0 / 20.0, crowd36::tone_hz(k))),
    );
    seq.push(Tone::silence(1.5));
    let c = Channel {
        snr_db: Some(10.0),
        offset_hz: 13.6,
        seed: 9,
        ..Channel::default()
    };
    let x = signal(&seq, 24e3, 0.0, &c);
    let (rx, _) = run_at(&CROWD36, &x, 24e3, 0.0, &[("speed", json!("20"))]);
    let rows = sender_rows(rx.as_ref());
    let right = groups_right(&groups, &texts_of(&rows, "Text"));
    assert!(right >= 39, "{right} of 40: {rows:?}");
}

#[test]
fn a_line_without_a_group_is_refused_and_counted() {
    // A group is five characters of the tables between spaces: a shorter word, or one with a tone of unknown meaning or
    // outside the tables, is none.
    let mut t: Vec<i64> = [24, 10].repeat(6);
    t.push(26);
    t.extend(letters("ABCD "));
    t.push(18);
    t.extend([10, 31, 0, 0, 0, 0]);
    t.push(18);
    t.extend(letters("ABCDE QX"));
    let mut r = Reader::new(24);
    let mut got = Vec::new();
    t.iter().for_each(|&k| r.push(k, &mut got));
    r.flush(&mut got);
    assert_eq!(
        got,
        vec![Read::Sync(24, 10, 0), Read::Text("ABCDE QX".into())]
    );
    assert_eq!(r.refused, 2);
    // Near the 50 % point the receiver reads lines broken by wrong tones; each row shown holds a group, and what is
    // refused is counted.
    let mut refused = 0.0;
    for seed in 11..15u64 {
        let ch = Channel {
            snr_db: Some(-6.0),
            offset_hz: -7.0,
            ppm: 100.0,
            seed,
            ..Channel::default()
        };
        let (_, seq) = message(40, seed);
        let x = signal(&seq, 24e3, 0.0, &ch);
        let (rx, _) = run_at(&CROWD36, &x, 24e3, 0.0, &[]);
        let rows = sender_rows(rx.as_ref());
        for t in texts_of(&rows, "Text") {
            assert!(
                t.split(' ')
                    .any(|w| w.chars().count() == 5 && !w.contains('<')),
                "seed {seed}: {t:?}"
            );
        }
        refused += field(rx.as_ref(), "refused").unwrap_or(0.0);
    }
    assert!(refused > 0.0, "{refused} refused");
}

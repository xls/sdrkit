//! XPA and XPA2: the reading layer on the worked examples and the documented sequences, the receiver on its test signal at
//! stated levels, offsets, clock errors and sidebands, and no rows on noise, a carrier or a CROWD36 signal.

use crate::common::*;
use crate::senders::*;
use sdrkit_stack_hfdata::senders::Tone;
use sdrkit_stack_hfdata::xpa::{self, Line, Reading, Submode, XPA};

fn read_all(sub: Submode, tones: &[i64]) -> Vec<Line> {
    let mut r = Reading::new(sub);
    let mut out = Vec::new();
    for &t in tones {
        r.push(t, &mut out);
    }
    r.flush(&mut out);
    out
}

#[test]
fn the_reading_layer_reads_the_worked_example() {
    let mut t = Vec::new();
    for _ in 0..5 {
        t.extend([0, 0, 19, 19]);
    }
    for _ in 0..8 {
        t.extend([2, 15]);
    }
    t.extend([16, 16, 7, 17, 8, 17, 9, 4, 15, 6, 17, 6, 17, 4, 5, 17]);
    let want = vec![
        Line::Start,
        Line::MessageStart,
        Line::Groups("11223 90000".into()),
        Line::End,
    ];
    assert_eq!(read_all(Submode::Xpa, &t), want);
    let mut r = Reading::new(Submode::Xpa);
    t.iter().for_each(|&k| r.push(k, &mut Vec::new()));
    assert_eq!(r.score, 2);
    // XPA2 tones 1000 Hz + 15.625 Hz k: the first group 1 R 2 R 3 space.
    let g = [7, 5, 8, 5, 9, 0];
    let hz: Vec<f64> = g.iter().map(|&k| Submode::Xpa2.tone_hz(k)).collect();
    assert_eq!(hz, [1109.375, 1078.125, 1125.0, 1078.125, 1140.625, 1000.0]);
}

#[test]
fn separators_and_a_null_message_are_rows_of_their_own() {
    let groups: Vec<String> = [
        "12345", "67890", "00000", "00000", "01234", "00001", "00000", "55555", "00000", "00000",
        "11111",
    ]
    .iter()
    .map(|g| g.to_string())
    .collect();
    // The description gives them for XPA; in XPA2 the groups are traffic.
    let t = tones_of(&xpa::test_pieces(Submode::Xpa2, &groups, 10.0), 10.0);
    let got = read_all(Submode::Xpa2, &t);
    assert!(
        !got.iter()
            .any(|l| matches!(l, Line::Separator | Line::NullMessage(_))),
        "{got:?}"
    );
    {
        let sub = Submode::Xpa;
        let t = tones_of(&xpa::test_pieces(sub, &groups, 10.0), 10.0);
        // The header holds the block sync and break strings.
        let got: Vec<Line> = read_all(sub, &t)
            .into_iter()
            .filter(|l| {
                !matches!(
                    l,
                    Line::Start | Line::MessageStart | Line::BlockSync | Line::Break
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                Line::Groups("12345 67890".into()),
                Line::Separator,
                Line::NullMessage("01234 00001 00000 55555".into()),
                Line::Separator,
                Line::Groups("11111".into()),
                Line::End,
            ],
            "{sub:?}"
        );
    }
    // Four groups of another form are traffic.
    let other: Vec<String> = ["11234", "00001", "00000", "55555"]
        .iter()
        .map(|g| g.to_string())
        .collect();
    let t = tones_of(&xpa::test_pieces(Submode::Xpa, &other, 10.0), 10.0);
    assert!(read_all(Submode::Xpa, &t).contains(&Line::Groups("11234 00001 00000 55555".into())));
}

/// The tones of `pieces` (the 400 ms and 1600 ms start tones taken as 4 and 16 symbols at 10 Bd).
fn tones_of(pieces: &[(f64, i64)], baud: f64) -> Vec<i64> {
    pieces
        .iter()
        .flat_map(|&(s, k)| std::iter::repeat_n(k, (s * baud).round() as usize))
        .collect()
}

#[test]
fn the_documented_sequences_read_as_one_message() {
    let groups = xpa::test_groups(70, 4);
    let t = tones_of(&xpa::test_pieces(Submode::Xpa, &groups, 10.0), 10.0);
    let got = read_all(Submode::Xpa, &t);
    let kinds: Vec<&str> = got.iter().map(Line::kind).collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "Start").count(),
        1,
        "{kinds:?}"
    );
    assert_eq!(kinds.iter().filter(|k| **k == "Message start").count(), 1);
    assert_eq!(kinds.iter().filter(|k| **k == "End").count(), 1);
    // The header string: block sync, break, block sync; the block sync after 64 groups.
    assert_eq!(kinds.iter().filter(|k| **k == "Block sync").count(), 3);
    assert_eq!(kinds.iter().filter(|k| **k == "Break").count(), 1);
    let lines: Vec<String> = got
        .iter()
        .filter_map(|l| match l {
            Line::Groups(g) => Some(g.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(groups_right(&groups, &lines), 70);
    assert!(
        lines.iter().all(|l| l.split(' ').all(|w| w.len() == 5)),
        "{lines:?}"
    );
    // XPA2 with the sync tones in either order and the end alternating.
    let groups = xpa::test_groups(24, 5);
    let base = tones_of(&xpa::test_pieces(Submode::Xpa2, &groups, 7.8125), 7.8125);
    for swap in [false, true] {
        let mut t = base.clone();
        if swap {
            for w in t.chunks_mut(2) {
                if w == [2, 5] {
                    w.swap(0, 1);
                }
            }
        }
        let got = read_all(Submode::Xpa2, &t);
        let kinds: Vec<&str> = got.iter().map(Line::kind).collect();
        let lines: Vec<String> = got
            .iter()
            .filter_map(|l| match l {
                Line::Groups(g) => Some(g.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(groups_right(&groups, &lines), 24, "{swap}: {got:?}");
        assert!(
            lines.iter().all(|l| l.split(' ').all(|w| w.len() == 5)),
            "{swap}: {lines:?}"
        );
        for (k, n) in [
            ("Start", 1),
            ("Message start", 1),
            ("End", 1),
            ("Block sync", 1),
        ] {
            assert_eq!(
                kinds.iter().filter(|x| **x == k).count(),
                n,
                "{swap} {k}: {kinds:?}"
            );
        }
    }
}

struct Run {
    right: usize,
    ends: usize,
    headline: String,
}

fn case(sub: Submode, n: usize, baud: f64, lower: bool, rate: f64, dial: f64, ch: &Channel) -> Run {
    let groups = xpa::test_groups(n, ch.seed);
    let seq = xpa::test_sequence(sub, &groups, baud, lower);
    let x = signal(&seq, rate, dial, ch);
    let set = [(
        "submode",
        json!(if sub == Submode::Xpa { "xpa" } else { "xpa2" }),
    )];
    let (rx, _) = run_at(&XPA, &x, rate, dial, &set);
    let rows = sender_rows(rx.as_ref());
    Run {
        right: groups_right(&groups, &texts_of(&rows, "Groups")),
        ends: texts_of(&rows, "End").len(),
        headline: rx.results().status().summary.clone(),
    }
}

fn ch(snr: f64, offset_hz: f64, drift_hz_s: f64, ppm: f64, seed: u64) -> Channel {
    Channel {
        snr_db: Some(snr),
        offset_hz,
        drift_hz_s,
        ppm,
        seed,
        ..Channel::default()
    }
}

#[test]
fn xpa_reads_its_test_signal_at_10_db() {
    for (baud, lower, rate, dial, c, what) in [
        (10.0, false, 24e3, 0.0, ch(10.0, 0.0, 0.0, 0.0, 1), "10 Bd"),
        (
            20.0,
            false,
            24e3,
            0.0,
            ch(10.0, 37.3, 0.0, 0.0, 2),
            "20 Bd dial +37.3 Hz",
        ),
        (
            10.0,
            false,
            48e3,
            dial_of(48e3),
            ch(10.0, 950.0, 0.0, 0.0, 3),
            "10 Bd 950 Hz off at 48 ksps",
        ),
        (
            20.0,
            false,
            24e3,
            0.0,
            ch(10.0, -61.0, 0.2, 400.0, 4),
            "20 Bd +400 ppm, +0.2 Hz/s, -61 Hz",
        ),
        (
            10.0,
            true,
            24e3,
            0.0,
            ch(10.0, -12.0, 0.0, 0.0, 5),
            "10 Bd lower sideband, -12 Hz",
        ),
        (
            20.0,
            true,
            24e3,
            0.0,
            ch(10.0, 25.0, 0.0, -300.0, 6),
            "20 Bd lower sideband, -300 ppm, +25 Hz",
        ),
        (
            20.0,
            false,
            250e3,
            -20e3 + 14.0,
            ch(10.0, 0.0, 0.0, 0.0, 7),
            "20 Bd at 250 ksps, offset -20 kHz",
        ),
    ] {
        let r = case(Submode::Xpa, 30, baud, lower, rate, dial, &c);
        let speed = format!("XPA {baud:.0} Bd");
        assert!(
            r.right >= 29 && r.ends >= 1 && r.headline.contains(&speed),
            "{what}: {} of 30, {} End rows, {}",
            r.right,
            r.ends,
            r.headline
        );
    }
}

#[test]
fn xpa2_reads_its_test_signal_at_0_db() {
    for (lower, rate, dial, c, what) in [
        (false, 24e3, 0.0, ch(0.0, 0.0, 0.0, 0.0, 1), "24 ksps"),
        (false, 24e3, 0.0, ch(0.0, 4.7, 0.0, 0.0, 2), "dial +4.7 Hz"),
        (
            false,
            48e3,
            dial_of(48e3),
            ch(0.0, 700.0, 0.0, 0.0, 3),
            "700 Hz off at 48 ksps",
        ),
        (
            false,
            24e3,
            0.0,
            ch(0.0, -33.0, -0.05, 300.0, 4),
            "+300 ppm, -0.05 Hz/s, -33 Hz",
        ),
        (
            true,
            24e3,
            0.0,
            ch(0.0, 3.0, 0.0, 0.0, 5),
            "lower sideband, +3 Hz",
        ),
    ] {
        let r = case(Submode::Xpa2, 24, 7.8125, lower, rate, dial, &c);
        assert!(
            r.right >= 23 && r.ends >= 1,
            "{what}: {} of 24, {} End rows, {}",
            r.right,
            r.ends,
            r.headline
        );
    }
}

fn sweep(sub: Submode, baud: f64, n: usize, from: f64, dial: f64) -> Vec<(f64, f64)> {
    (0..7)
        .map(|k| {
            let snr = from + 2.0 * k as f64;
            let right: usize = [31u64, 32]
                .iter()
                .map(|&seed| {
                    case(
                        sub,
                        n,
                        baud,
                        false,
                        24e3,
                        0.0,
                        &ch(snr, dial, 0.0, 100.0, seed),
                    )
                    .right
                })
                .sum();
            (snr, right as f64 / (2 * n) as f64)
        })
        .collect()
}

#[test]
fn xpa_sweeps_reach_90_percent_at_their_top_step() {
    for (baud, from) in [(10.0, -22.0), (20.0, -19.0)] {
        let c = sweep(Submode::Xpa, baud, 30, from, 23.0);
        println!("xpa {baud} Bd sweep: {c:?}");
        assert!(c[6].1 >= 0.9, "{baud} Bd: {c:?}");
    }
}

#[test]
fn xpa2_sweep_reaches_90_percent_at_minus_14_db() {
    let c = sweep(Submode::Xpa2, 7.8125, 24, -26.0, 2.1);
    println!("xpa2 sweep: {c:?}");
    assert!(c[6].1 >= 0.9, "{c:?}");
}

#[test]
fn noise_a_carrier_and_crowd36_give_no_row() {
    for sub in [Submode::Xpa, Submode::Xpa2] {
        let set = [(
            "submode",
            json!(if sub == Submode::Xpa { "xpa" } else { "xpa2" }),
        )];
        let carrier = if sub == Submode::Xpa { 900.0 } else { 1100.0 };
        let crowd = {
            let groups = sdrkit_stack_hfdata::crowd36::test_groups(40, 1);
            let mut seq = vec![Tone::silence(1.0)];
            seq.extend(
                sdrkit_stack_hfdata::crowd36::test_tones(&groups, 24, 10)
                    .into_iter()
                    .map(|k| Tone::new(0.025, sdrkit_stack_hfdata::crowd36::tone_hz(k))),
            );
            seq.push(Tone::silence(1.5));
            seq
        };
        for (seq, seed, what) in [
            (vec![Tone::silence(60.0)], 41u64, "noise"),
            (vec![Tone::silence(60.0)], 42, "noise"),
            (vec![Tone::new(40.0, carrier)], 43, "carrier"),
            (crowd, 44, "CROWD36 at 10 dB"),
        ] {
            let snr = if what.starts_with("CROWD") { 10.0 } else { 0.0 };
            let x = signal(&seq, 24e3, 0.0, &ch(snr, 0.0, 0.0, 0.0, seed));
            let (rx, _) = run_at(&XPA, &x, 24e3, 0.0, &set);
            let rows = sender_rows(rx.as_ref());
            assert!(rows.is_empty(), "{sub:?} {what}: {rows:?}");
        }
    }
}

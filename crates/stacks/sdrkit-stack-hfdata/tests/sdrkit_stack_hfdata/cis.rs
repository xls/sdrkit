//! The CIS stack: the format layers, each submode on its test signal through noise, offsets and clock errors, the identifiers on
//! every other signal, and the family front end at 250 ksps.

use crate::common::*;
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig};
use sdrkit_stack_hfdata::cis::{self, gen, CIS_HF};
use sdrkit_stack_hfdata::teleprinter::ToneSeqGen;

fn signal(sub: &str, set: &[(&str, Json)], secs: f64, rate: f64) -> Vec<Complex32> {
    let mut g = (CIS_HF.make_generator.unwrap())(
        &GenConfig::new(rate, values(&CIS_HF, set))
            .with_submode(sub)
            .with_seed(1),
    )
    .unwrap();
    generate(g.as_mut(), secs)
}

fn run(sub: &str, set: &[(&str, Json)], x: &[Complex32]) -> (Box<dyn Receiver>, Output) {
    let cfg = StackConfig::new(values(&CIS_HF, set)).with_submode(sub);
    let mut rx = (CIS_HF.make_receiver)(&cfg).unwrap();
    let f = CIS_HF.tuning.default_hz;
    rx.configure(&InputInfo::new(24e3, f, f).recording())
        .unwrap();
    let mut out = feed(rx.as_mut(), x, 4801);
    let mut e = end(rx.as_mut());
    out.events.append(&mut e.events);
    (rx, out)
}

fn chan(x: &[Complex32], snr_db: f64, offset_hz: f64, ppm: f64, seed: u64) -> Vec<Complex32> {
    through(
        x,
        24e3,
        &Channel {
            snr_db: Some(snr_db),
            offset_hz,
            ppm,
            seed,
            ..Channel::default()
        },
    )
}

#[test]
fn the_3_of_7_set_and_the_cis14_frame() {
    let v = gen::valid_chars();
    assert_eq!(v.len(), 35);
    assert!(!cis::fsk::valid(0x77));
    assert_eq!(0x77u8.count_ones(), 6);
    let f = gen::cis14_bits(2, 0b10110, 0, 1);
    let value = f
        .iter()
        .enumerate()
        .fold(0u32, |v, (i, &b)| v | (u32::from(b) << i));
    assert_eq!(value & 3, 2);
    assert_eq!(value >> 12, 1);
    assert_eq!((value >> 2) & 0x3FF, (1 << 0) | (1 << 4) | (1 << 6));
}

/// The characters of the messages in the text: the hex tokens after each message start.
fn message_chars(text: &str) -> Vec<Vec<String>> {
    let mut msgs = Vec::new();
    for line in text.lines() {
        let body = if let Some(i) = line.find(']') {
            if line.starts_with("[start") || line.starts_with("[follow-on") {
                &line[i + 1..]
            } else {
                continue;
            }
        } else {
            continue;
        };
        msgs.push(
            body.split_whitespace()
                .filter(|t| !t.starts_with('<'))
                .map(str::to_owned)
                .collect(),
        );
    }
    msgs
}

/// A BEE signal at `traffic` Bd and `shift` Hz: idle reversals, a message with its start, a follow-on message, idle.
fn bee_signal(idle: f64, traffic: f64, shift: f64, lower_one: bool) -> Vec<Complex32> {
    let c = 1700.0;
    let tone = |b: u8| {
        Some(if (b == 1) != lower_one {
            c + shift / 2.0
        } else {
            c - shift / 2.0
        })
    };
    let mut seq = Vec::new();
    for i in 0..(2.0 * idle) as usize {
        seq.push((1.0 / idle, tone((i % 2) as u8 ^ 1)));
    }
    for i in 0..(0.6 * traffic) as usize {
        seq.push((1.0 / traffic, tone((i % 2) as u8 ^ 1)));
    }
    for b in gen::bee_bits(true).into_iter().chain(gen::bee_bits(false)) {
        seq.push((1.0 / traffic, tone(b)));
    }
    let secs: f64 = seq.iter().map(|s| s.0).sum::<f64>() + 1.0;
    seq.push((5.0, None));
    let mut g = ToneSeqGen::new(24e3, &seq, 600.0);
    generate(&mut g, secs)
}

/// The share of the expected characters read right, over both messages.
fn bee_score(text: &str) -> f64 {
    let want: Vec<String> = gen::bee_message()
        .1
        .iter()
        .map(|c| format!("{c:02X}"))
        .collect();
    let msgs = message_chars(text);
    let mut right = 0;
    for m in msgs.iter().take(2) {
        right += m.iter().zip(&want).filter(|(a, b)| a == b).count();
    }
    right as f64 / (2 * want.len()) as f64
}

#[test]
fn bee_messages_at_50_and_100_bd() {
    for (idle, traffic, shift) in [(36.0, 50.0, 200.0), (50.0, 100.0, 250.0)] {
        for lower_one in [true, false] {
            for off in [-60.0, 0.0, 85.0] {
                for (snr, need) in [(30.0, 0.99), (12.0, 0.95)] {
                    let x = chan(
                        &bee_signal(idle, traffic, shift, lower_one),
                        snr,
                        off,
                        40.0,
                        3,
                    );
                    let (rx, _) = run("bee", &[], &x);
                    let text = field_text(rx.as_ref(), "text");
                    let score = bee_score(&text);
                    assert!(
                        score >= need,
                        "{traffic} {lower_one} {off} {snr}: {score} {text:?}"
                    );
                    let got = field(rx.as_ref(), "shift_hz").unwrap();
                    assert!((got - shift).abs() <= 0.2 * shift + 5.0, "{got}");
                }
            }
        }
    }
}

#[test]
fn bee_at_150_bd_with_speed_and_shift_set() {
    let x = chan(&bee_signal(50.0, 150.0, 500.0, true), 30.0, 0.0, 0.0, 4);
    let (rx, _) = run(
        "bee",
        &[("baud", json!(150.0)), ("shift_hz", json!(500.0))],
        &x,
    );
    let text = field_text(rx.as_ref(), "text");
    let want: Vec<String> = gen::bee_message()
        .1
        .iter()
        .take(50)
        .map(|c| format!("{c:02X}"))
        .collect();
    let msgs = message_chars(&text);
    let right = msgs
        .first()
        .map_or(0, |m| m.iter().zip(&want).filter(|(a, b)| a == b).count());
    assert!(right >= 49, "{right} {text:?}");
}

#[test]
fn bee_noise_has_no_message_start() {
    let x = chan(&vec![Complex32::default(); 24_000 * 20], 0.0, 0.0, 0.0, 5);
    let (rx, _) = run("bee", &[], &x);
    assert!(!field_text(rx.as_ref(), "text").contains("[start"));
}

#[test]
fn bee_test_signal_fills_the_message_table() {
    let x = signal("bee", &[], 26.0, 24e3);
    let (rx, out) = run("bee", &[], &x);
    let rows = rows(rx.results(), &sdrkit_stack_hfdata::typed::CIS_HF_MESSAGES);
    assert!(
        rows.iter()
            .any(|r| text(r, "ended_by") == "end of message" && num(r, "characters") == 120.0),
        "{rows:?}"
    );
    assert!(rows
        .iter()
        .any(|r| text(r, "start_word") == format!("{:011X}", gen::START_WORD)));
    assert!(!event_texts(&out, "cis.message").is_empty());
}

#[test]
fn cis14_frames() {
    for off in [-40.0, 0.0, 120.0] {
        for (snr, need) in [(30.0, 340), (14.0, 300)] {
            let x = chan(
                &signal("cis14", &[], 400.0 * 14.0 / 96.0 + 1.0, 24e3),
                snr,
                off,
                30.0,
                6,
            );
            let (rx, _) = run("cis14", &[], &x);
            let text = field_text(rx.as_ref(), "text");
            let right = text
                .lines()
                .filter(|l| {
                    let p: Vec<&str> = l.split(' ').collect();
                    if p.len() != 4 || p[0] != "01" {
                        return false;
                    }
                    let a = u8::from_str_radix(p[1], 16).unwrap_or(99);
                    let b = u8::from_str_radix(p[2], 16).unwrap_or(99);
                    let pp = format!("{}{}", (a % 4) & 1, (a % 4) >> 1);
                    a < 32 && b == (7 * a) % 32 && p[3] == pp
                })
                .count();
            assert!(right >= need, "{off} {snr}: {right}");
            assert_eq!(field_text(rx.as_ref(), "state_bits"), "01");
        }
    }
}

/// The step digits of the decoded CIS-12 symbols, aligned with the generator's, and the digit error rate over the symbols read.
fn cis12_errors(text: &str, order: u32) -> (usize, f64) {
    let want: Vec<String> = gen::cis12_steps(order, 1200)
        .iter()
        .map(|s| s.iter().map(|d| char::from(b'0' + d)).collect())
        .collect();
    let got: Vec<String> = text
        .split_whitespace()
        .filter(|g| g.len() >= 12 && g.chars().take(12).all(|c| c.is_ascii_digit() || c == '-'))
        .map(|g| g[..12].to_string())
        .collect();
    // The offset of the decoded symbols into the sent ones: the one that matches best over 40 symbols after the first 100 (the
    // first symbols read after the lock come from before the pilot was measured).
    if got.len() < 140 {
        return (0, 1.0);
    }
    let best = (0..want.len().saturating_sub(140))
        .max_by_key(|&o| (100..140).filter(|&i| got[i] == want[o + i - 100]).count())
        .unwrap_or(0)
        .wrapping_sub(100);
    let mut errors = 0;
    let mut n = 0;
    for (i, g) in got.iter().enumerate().skip(100) {
        let Some(w) = want.get(best.wrapping_add(i)) else {
            break;
        };
        n += 1;
        errors += g.chars().zip(w.chars()).filter(|(a, b)| a != b).count();
    }
    (n, errors as f64 / (12 * n.max(1)) as f64)
}

#[test]
fn cis12_steps_and_order() {
    for (order, name) in [("4", "DQPSK"), ("2", "DBPSK")] {
        for off in [-25.0, 0.0, 40.0] {
            for (snr, limit) in [(30.0, 0.01), (15.0, 0.05)] {
                let x = chan(
                    &signal("cis12", &[("gen_order", json!(order))], 10.5, 24e3),
                    snr,
                    off,
                    20.0,
                    7,
                );
                let (rx, _) = run("cis12", &[], &x);
                let o = field(rx.as_ref(), "offset_hz").unwrap();
                assert!((o - off).abs() <= 1.5, "{order} {off} {snr}: offset {o}");
                assert_eq!(
                    field_text(rx.as_ref(), "modulation"),
                    name,
                    "{order} {off} {snr}: m2 {:?} m4 {:?} {}",
                    field(rx.as_ref(), "m2"),
                    field(rx.as_ref(), "m4"),
                    field_text(rx.as_ref(), "found")
                );
                let (n, err) =
                    cis12_errors(&field_text(rx.as_ref(), "text"), order.parse().unwrap());
                assert!(
                    n >= 450 && err < limit,
                    "{order} {off} {snr}: {n} symbols, error {err}"
                );
            }
        }
    }
}

#[test]
fn cis36_identified_at_every_rate() {
    for rate in ["10", "20", "40"] {
        for off in [0.0, 70.0] {
            let x = chan(
                &signal("cis36", &[("gen_rate", json!(rate))], 30.0, 24e3),
                15.0,
                off,
                20.0,
                8,
            );
            let (rx, _) = run("cis36", &[], &x);
            assert_eq!(
                field_text(rx.as_ref(), "identified"),
                "CIS-36",
                "{rate} {off}: {}",
                field_text(rx.as_ref(), "found")
            );
            assert_eq!(
                field(rx.as_ref(), "symbol_rate"),
                Some(rate.parse::<f64>().unwrap())
            );
        }
    }
}

#[test]
fn ofdm_signals_are_named() {
    for (row, name) in [
        ("cis45_40", "CIS-45 (40 Bd)"),
        ("cis45_33", "CIS-45 (33.3 Bd)"),
        ("cis60", "CIS-60"),
        ("cis93", "CIS-93"),
    ] {
        for off in [0.0, 30.0] {
            let x = chan(
                &signal("ofdm_hdr", &[("gen_ofdm", json!(row))], 8.0, 24e3),
                18.0,
                off,
                10.0,
                9,
            );
            let (rx, _) = run("ofdm_hdr", &[], &x);
            assert_eq!(
                field_text(rx.as_ref(), "identified"),
                name,
                "{row} {off}: {}",
                field_text(rx.as_ref(), "found")
            );
        }
    }
}

#[test]
fn cis128_and_cis3000_are_identified() {
    let x = chan(&signal("cis128", &[], 8.0, 24e3), 25.0, 0.0, 0.0, 10);
    let (rx, _) = run("cis128", &[], &x);
    assert_eq!(
        field_text(rx.as_ref(), "identified"),
        "CIS-128",
        "{}",
        field_text(rx.as_ref(), "found")
    );
    for off in [0.0, -50.0] {
        let x = chan(&signal("cis3000", &[], 6.0, 24e3), 18.0, off, 10.0, 11);
        let (rx, _) = run("cis3000", &[], &x);
        assert_eq!(
            field_text(rx.as_ref(), "identified"),
            "CIS-3000",
            "{off}: {}",
            field_text(rx.as_ref(), "found")
        );
        assert_eq!(field_text(rx.as_ref(), "modulation"), "8-PSK");
    }
}

#[test]
fn no_identifier_claims_another_signal() {
    let subs = [
        "bee", "cis14", "cis12", "cis36", "ofdm_hdr", "cis128", "cis3000",
    ];
    let ids = ["cis36", "ofdm_hdr", "cis128", "cis3000"];
    for sig in subs {
        let x = chan(&signal(sig, &[], 14.0, 24e3), 25.0, 0.0, 0.0, 12);
        for id in ids.iter().filter(|&&i| i != sig) {
            let (rx, _) = run(id, &[], &x);
            assert_eq!(
                field_text(rx.as_ref(), "identified"),
                "",
                "{id} on {sig}: {}",
                field_text(rx.as_ref(), "found")
            );
        }
    }
}

#[test]
fn the_front_end_at_250k() {
    use sdrkit_stack_hfdata::serial::gen::test_dial;
    let rate = 250e3;
    for (sub, secs) in [("bee", 26.0), ("cis12", 6.0), ("cis3000", 6.0)] {
        let x = signal(sub, &[], secs, rate);
        let x = through(
            &x,
            rate,
            &Channel {
                snr_db: Some(25.0),
                offset_hz: 15.0,
                seed: 13,
                ..Channel::default()
            },
        );
        let cfg = StackConfig::new(values(&CIS_HF, &[])).with_submode(sub);
        let mut rx = (CIS_HF.make_receiver)(&cfg).unwrap();
        let centre = CIS_HF.tuning.default_hz;
        rx.configure(&InputInfo::new(rate, centre, centre + test_dial(rate)).recording())
            .unwrap();
        feed(rx.as_mut(), &x, 25_000);
        end(rx.as_mut());
        let rx = rx.as_ref();
        match sub {
            "bee" => {
                let text = field_text(rx, "text");
                let n = message_chars(&text).first().map_or(0, Vec::len);
                assert!(text.contains("[start") && n >= 50, "{text:?}");
            }
            "cis12" => {
                assert_eq!(field(rx, "state"), Some(2.0));
                assert!((field(rx, "offset_hz").unwrap() - 15.0).abs() <= 2.0);
            }
            _ => assert_eq!(field_text(rx, "identified"), "CIS-3000"),
        }
    }
}

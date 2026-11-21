//! Cross-slot combining, a-priori decoding from the calls heard on the band, and early decode of FT8.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, HostRequest, Output, Receiver, StackConfig, Value};
use sdrkit_stack_wsjt::bands::{hours_text, parse_bands, parse_cycle, parse_hours, HopPlan};
use sdrkit_stack_wsjt::encode::{audio, Transmission};
use sdrkit_stack_wsjt::proto::codes::CodeStore;
use sdrkit_stack_wsjt::runner::{decode_slot, DecodeState};
use sdrkit_stack_wsjt::rx::slot::{Decode, How};
use sdrkit_stack_wsjt::typed::{DecodeRow, DECODES};
use sdrkit_stack_wsjt::{Mode, Sub};

/// `seconds` of 8 kHz audio with noise (seed `seed`) and the stations of `per_slot[k]` in slot `k` of `sub` only.
fn audio_slots(sub: Sub, per_slot: &[Vec<St>], seconds: f64, seed: u64) -> Vec<f32> {
    let n = (seconds * 8000.0) as usize;
    let mut a = noise(n, SIGMA, seed);
    for (k, st) in per_slot.iter().enumerate() {
        for &(text, hz, snr, dt, mir) in st {
            let t = Transmission {
                sub,
                hz,
                amp: amp(snr),
                mirrored: mir,
            };
            let s = audio(&t, text, 8000.0, &CodeStore::default()).unwrap();
            let start = ((k as f64 * sub.period + sub.t0() + dt) * 8000.0).round() as i64;
            for (i, v) in s.iter().enumerate() {
                let j = start + i as i64;
                if j >= 0 && (j as usize) < n {
                    a[j as usize] += v;
                }
            }
        }
    }
    a
}

/// The decodes of every slot of `audio`, one decoder state across the slots.
fn decode_slots(sub: Sub, audio: &[f32]) -> Vec<Decode> {
    let mut state = DecodeState::default();
    slots(sub, audio)
        .iter()
        .flat_map(|s| decode_slot(s, &mut state).decodes)
        .collect()
}

#[test]
fn combining_decodes_a_repeated_message_below_the_threshold() {
    let sub = Sub::new(Mode::Ft8);
    let text = "W9XYZ K1ABC RR73";
    let mut decoded = 0;
    for snr in [-22.0, -23.0] {
        for s in 1..=3u64 {
            let st = [(
                text,
                1100.0 + 37.0 * s as f64,
                snr,
                0.15 * s as f64 - 0.2,
                false,
            )];
            let a = audio_of(sub, &st, 8.0 * sub.period, true, 1000 + 10 * s + (-snr) as u64);
            let d = decode_slots(sub, &a);
            let others: Vec<&str> = d
                .iter()
                .filter(|x| x.text != text)
                .map(|x| x.text.as_str())
                .collect();
            assert!(others.is_empty(), "{snr} dB seed {s}: {others:?}");
            let combined = d.iter().filter(|x| x.how == How::Combined).count();
            eprintln!("{snr} dB seed {s}: {} of 8 ({combined} combined)", d.len());
            decoded += d.len();
        }
    }
    assert!(decoded >= 6, "{decoded} of 48");
}

/// The conversation of the a-priori case: slots 0 to 2, and the weak message of slot 3 for `seed` at `snr`.
fn conversation(seed: u64, snr: f64) -> Vec<Vec<St<'static>>> {
    let weak = if seed % 2 == 1 {
        "K1ABC W9XYZ R-12"
    } else {
        "K1ABC W9XYZ R-09"
    };
    vec![
        vec![("CQ W9XYZ EN37", 1500.0, -8.0, 0.0, false)],
        vec![
            ("W9XYZ K1ABC FN42", 1900.0, -8.0, 0.0, false),
            ("JA1QRZ K1ABC PM95", 900.0, -10.0, 0.0, false),
        ],
        vec![("K1ABC W9XYZ -10", 1500.0, -10.0, 0.0, false)],
        vec![(weak, 1700.0 + 20.0 * seed as f64, snr, 0.0, false)],
    ]
}

#[test]
fn a_priori_decoding_from_the_calls_heard() {
    let sub = Sub::new(Mode::Ft8);
    for snr in [-21.0, -22.0] {
        let mut weak = 0;
        for seed in 1..=4u64 {
            let per_slot = conversation(seed, snr);
            let sent: Vec<&str> = per_slot.iter().flatten().map(|s| s.0).collect();
            let a = audio_slots(sub, &per_slot, 4.0 * sub.period, 2000 + seed);
            let d = decode_slots(sub, &a);
            let false_ones: Vec<&str> = d
                .iter()
                .map(|x| x.text.as_str())
                .filter(|t| !sent.contains(t))
                .collect();
            assert!(false_ones.is_empty(), "{snr} dB seed {seed}: {false_ones:?}");
            if let Some(x) = d.iter().find(|x| x.text == per_slot[3][0].0) {
                eprintln!("{snr} dB seed {seed}: {} {:?}", x.text, x.how);
                weak += 1;
            }
        }
        assert!(weak >= 3, "{snr} dB: {weak} of 4");
    }
}

/// The two calls of each of the three conversations of the a-priori run, and the audio frequency of the station heard.
const TALKS: [(&str, &str, f64); 3] = [
    ("K1ABC", "W9XYZ", 640.0),
    ("G4ABC", "DL1XYZ", 1430.0),
    ("JA1XYZ", "VK2ABC", 2210.0),
];

/// The last word of the message of conversation `i` in slot `k`: 30 different reports, so that no message repeats within a
/// run (a repeat would let cross-slot combining add to the decodes).
fn report(i: usize, k: usize) -> String {
    let r = (k + 10 * i) % 30;
    if r < 15 {
        format!("-{:02}", r + 6)
    } else {
        format!("R-{:02}", r - 9)
    }
}

/// What a run of 30 FT8 slots gives.
struct APrioriRun {
    /// Messages sent.
    sent: usize,
    /// Messages decoded.
    decoded: usize,
    /// Of those, decoded with the bits of a heard pair assumed.
    a_priori: usize,
    /// Decodes whose text was not sent in their slot.
    wrong: Vec<String>,
}

/// 30 slots of `mode` with one message of each conversation of [`TALKS`] at `snr` dB in 2500 Hz (none with `quiet`), and
/// the calls of the three pairs heard before every slot. `shifted`: the audio 40 Hz up (a dial error) and the sample clock
/// 100 ppm fast, applied to the audio that the slot decoder reads.
fn a_priori_run(mode: Mode, snr: f64, shifted: bool, quiet: bool, seed: u64) -> APrioriRun {
    let sub = Sub::new(mode);
    let df = if shifted { 40.0 } else { 0.0 };
    let mut r = rng(seed);
    let n = (30.0 * sub.period * 8000.0) as usize;
    let mut a = noise(n, SIGMA, seed);
    let mut sent: Vec<Vec<String>> = vec![Vec::new(); 30];
    for (k, texts) in sent.iter_mut().enumerate() {
        for (i, &(c1, c2, hz)) in TALKS.iter().enumerate() {
            let text = format!("{c1} {c2} {}", report(i, k));
            let dt = 0.6 * (r.uniform() - 0.5);
            if !quiet {
                let t = Transmission {
                    sub,
                    hz: hz + df,
                    amp: amp(snr),
                    mirrored: false,
                };
                let s = audio(&t, &text, 8000.0, &CodeStore::default()).unwrap();
                let start = ((k as f64 * sub.period + sub.t0() + dt) * 8000.0).round() as usize;
                for (o, v) in a[start..].iter_mut().zip(&s) {
                    *o += v;
                }
                texts.push(text);
            }
        }
    }
    if shifted {
        a = clock_error(&iq_of(&a), 100.0).iter().map(|c| c.re).collect();
    }
    let mut state = DecodeState::default();
    let mut run = APrioriRun {
        sent: sent.iter().map(Vec::len).sum(),
        decoded: 0,
        a_priori: 0,
        wrong: Vec::new(),
    };
    for (k, s) in slots(sub, &a).iter().enumerate().take(30) {
        for (c1, c2, _) in TALKS {
            state.ft.heard.take(s.utc - sub.period, &format!("{c1} {c2}"));
        }
        for d in decode_slot(s, &mut state).decodes {
            if sent[k].contains(&d.text) {
                run.decoded += 1;
                run.a_priori += usize::from(d.how == How::APriori);
            } else {
                run.wrong.push(format!("slot {k}: {} ({:?}, {} dB)", d.text, d.how, d.snr));
            }
        }
    }
    run
}

/// A-priori decoding of the calls heard makes at most one wrong text in 30 slots at each level from -12 to -22 dB, plain and
/// with a dial and a clock error, and decodes at least 100 % of the messages from -12 to -19 dB, 97 % at -20, 69 % at -21
/// and 7 % at -22. Noise alone gives no decode.
fn a_priori_without_wrong_texts(levels: &[f64], shifted: bool) {
    for &snr in levels {
        let run = a_priori_run(Mode::Ft8, snr, shifted, false, 5000 + (-snr) as u64);
        let share = run.decoded as f64 / run.sent as f64;
        eprintln!(
            "{snr} dB{}: {} of {} ({:.0} %), {} a priori, wrong {:?}",
            if shifted { " shifted" } else { "" },
            run.decoded,
            run.sent,
            100.0 * share,
            run.a_priori,
            run.wrong
        );
        assert!(run.wrong.len() <= 1, "{snr} dB: {:?}", run.wrong);
        if !shifted {
            let need = match snr as i32 {
                -20 => 0.97,
                -21 => 0.69,
                -22 => 0.07,
                _ => 1.0,
            };
            assert!(share >= need, "{snr} dB: {:.3} of the messages", share);
        }
    }
}

#[test]
fn a_priori_without_wrong_texts_at_minus_21_and_minus_22() {
    a_priori_without_wrong_texts(&[-21.0, -22.0], false);
}

#[test]
#[ignore = "9 levels of 30 slots (about 4 min); run with --ignored"]
fn a_priori_without_wrong_texts_from_minus_12_to_minus_20() {
    a_priori_without_wrong_texts(
        &[-12.0, -13.0, -14.0, -15.0, -16.0, -17.0, -18.0, -19.0, -20.0],
        false,
    );
}

#[test]
#[ignore = "11 levels of 30 slots (about 7 min); run with --ignored"]
fn a_priori_without_wrong_texts_with_dial_and_clock_errors() {
    a_priori_without_wrong_texts(
        &[-12.0, -13.0, -14.0, -15.0, -16.0, -17.0, -18.0, -19.0, -20.0, -21.0, -22.0],
        true,
    );
}

/// 30 slots of noise with the calls of three pairs heard: no decode in FT8, FT4 or FT2.
#[test]
fn a_priori_on_noise_alone_decodes_nothing() {
    for mode in [Mode::Ft8, Mode::Ft4, Mode::Ft2] {
        let run = a_priori_run(mode, 0.0, false, true, 5100);
        assert_eq!((run.decoded, run.wrong.len()), (0, 0), "{mode:?}: {:?}", run.wrong);
    }
}

/// Slots 0 to 2 of a conversation at -6 dB, then `quiet` slots of noise: no decode of a message that was not sent.
fn noise_after_a_conversation(quiet: usize) {
    let sub = Sub::new(Mode::Ft8);
    let talk: Vec<St> = vec![
        ("CQ W9XYZ EN37", 600.0, -6.0, 0.0, false),
        ("CQ DL1ABC JO62", 1000.0, -6.0, 0.0, false),
        ("W9XYZ K1ABC FN42", 1400.0, -6.0, 0.0, false),
        ("DL1ABC G4XYZ IO91", 1800.0, -6.0, 0.0, false),
        ("K1ABC W9XYZ -10", 2200.0, -6.0, 0.0, false),
    ];
    let sent: Vec<&str> = talk.iter().map(|s| s.0).collect();
    let per_slot = vec![talk.clone(), talk.clone(), talk];
    let a = audio_slots(sub, &per_slot, (3 + quiet) as f64 * sub.period, 3000);
    let d = decode_slots(sub, &a);
    let false_ones: Vec<&str> = d
        .iter()
        .map(|x| x.text.as_str())
        .filter(|t| !sent.contains(t))
        .collect();
    assert!(false_ones.is_empty(), "{false_ones:?}");
    assert!(d.len() >= 12, "{} decodes", d.len());
}

#[test]
fn noise_after_a_conversation_decodes_nothing_false() {
    noise_after_a_conversation(13);
}

#[test]
#[ignore = "the long run of 41 slots of noise (about 35 s); run with --ignored"]
fn noise_after_a_conversation_decodes_nothing_false_long() {
    noise_after_a_conversation(41);
}

/// The messages and early marks of the decode table, oldest first.
fn table_rows(rx: &sdrkit_graph::StackGraph) -> Vec<(String, bool)> {
    let mut v: Vec<(String, bool)> = DECODES
        .rows::<DecodeRow>(rx.results())
        .map(|(_, row)| (row.message.to_string(), row.early))
        .collect();
    v.reverse();
    v
}

/// Feeds IQ in pieces without finishing.
fn feed_open(rx: &mut sdrkit_graph::StackGraph, iq: &[Complex32], piece: usize) {
    let mut out = Output::default();
    for c in iq.chunks(piece) {
        rx.feed(c, &mut out);
        out.clear();
    }
}

#[test]
fn early_decode_then_the_whole_slot() {
    let sub = Sub::new(Mode::Ft8);
    let per_slot = vec![vec![
        ("CQ K1ABC FN42", 1200.0, -8.0, 0.1, false),
        ("W9XYZ DL1ABC JO62", 2000.0, -12.0, -0.3, false),
    ]];
    let iq = iq_of(&audio_slots(sub, &per_slot, 16.5, 4000));
    let mut rx = receiver(Some(Mode::Ft8), &[], 8000.0, 0.0);
    // 11.5 s of the slot, in pieces of 0.1 s.
    feed_open(&mut rx, &iq[..(11.5 * 8000.0) as usize], 800);
    let rows = table_rows(&rx);
    let cq: Vec<_> = rows.iter().filter(|r| r.0 == "CQ K1ABC FN42").collect();
    assert_eq!(cq.len(), 1, "{rows:?}");
    assert!(rows.iter().all(|r| r.1), "{rows:?}");
    assert_eq!(field_f64(rx.results(), "dec_ft8.slots"), 0.0);
    assert_eq!(field_f64(rx.results(), "decodes_total"), 0.0);
    // The rest of the slot.
    feed_open(&mut rx, &iq[(11.5 * 8000.0) as usize..], 800);
    let mut rows = table_rows(&rx);
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("CQ K1ABC FN42".to_string(), false),
            ("W9XYZ DL1ABC JO62".to_string(), false)
        ]
    );
    assert_eq!(field_f64(rx.results(), "dec_ft8.slots"), 1.0);
    assert_eq!(field_f64(rx.results(), "decodes_total"), 2.0);
}

/// The band list and the coordinated cycle of the data file of band hopping, as the texts of `hop.bands` and `hop.cycle`.
fn hop_lists() -> (String, String) {
    const FILE: &str = include_str!("../../../../../data/wsjt/band_hopping.csv");
    let mut bands: Vec<(usize, String)> = Vec::new();
    let mut cycle: Vec<(usize, String)> = Vec::new();
    for line in FILE.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        assert_eq!(c.len(), 3, "{line}");
        bands.push((c[0].parse().unwrap(), c[1].to_string()));
        if !c[2].is_empty() {
            cycle.push((c[2].parse().unwrap(), c[1].to_string()));
        }
    }
    bands.sort();
    cycle.sort();
    assert!(bands.iter().enumerate().all(|(i, b)| b.0 == i));
    let join = |v: Vec<(usize, String)>| v.into_iter().map(|x| x.1).collect::<Vec<_>>().join(",");
    (join(bands), join(cycle))
}

/// The settings `dials` and `band_edges` of the repository's data files, the dials of `rows` only (all when empty).
fn band_settings(rows: &[(&str, &str)]) -> [(&'static str, Value); 2] {
    use sdrkit_presets::wsjt;
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    let mut dials = wsjt::read_dials(&data.join(wsjt::DIALS)).unwrap();
    if !rows.is_empty() {
        dials.retain(|d| rows.iter().any(|&(m, b)| d.mode == m && d.band == b));
    }
    let bands = wsjt::read_bands(&data.join(wsjt::BANDS)).unwrap();
    [
        ("dials", Value::Text(wsjt::dials_text(&dials))),
        ("band_edges", Value::Text(wsjt::bands_text(&bands))),
    ]
}

/// The plan of the data file with these hour masks.
fn hop_plan(hours: [u16; 24], coordinated: bool) -> HopPlan {
    let (b, c) = hop_lists();
    let bands = parse_bands(&b).unwrap();
    HopPlan {
        cycle: parse_cycle(&c, &bands).unwrap(),
        bands,
        hours,
        coordinated,
    }
}

/// 2027-01-15 00:00 UTC.
const DAY: f64 = 1_799_971_200.0;

#[test]
fn band_hopping_follows_the_plan() {
    // 1. Every band of the cycle on in every hour: minute m gives band (m / 2) mod 10.
    let all = hop_plan([0x3FF; 24], true);
    for m in (0..40).step_by(2) {
        assert_eq!(all.band_at(DAY + 60.0 * m as f64), Some((m / 2) % 10), "minute {m}");
    }
    // 2. The edges of a slot.
    assert_eq!(all.band_at(DAY + 119.0), Some(0));
    assert_eq!(all.band_at(DAY + 120.0), Some(1));
    assert_eq!(HopPlan::next_change(DAY + 119.0), DAY + 120.0);
    assert_eq!(HopPlan::next_change(DAY + 120.0), DAY + 240.0);
    // 3. 60 m off: at 4 minutes, 40 m.
    let no60 = hop_plan([0x3FF & !(1 << 2); 24], true);
    assert_eq!(no60.band_at(DAY + 240.0), Some(3));
    // 4. Only 6 m on.
    let six = hop_plan([1 << 10; 24], true);
    assert_eq!(six.band_at(DAY), Some(10));
    assert_eq!(six.band_at(DAY + 600.0), Some(10));
    assert_eq!(six.name(10), Some("6m"));
    // 5. Not coordinated, 40, 20 and 10 m.
    let free = hop_plan([1 << 3 | 1 << 5 | 1 << 9; 24], false);
    let got: Vec<Option<usize>> = [0.0, 120.0, 240.0, 360.0, -120.0]
        .iter()
        .map(|s| free.band_at(DAY + s))
        .collect();
    assert_eq!(got, vec![Some(3), Some(5), Some(9), Some(3), Some(9)]);
    // 6. Hour 0: 80 m only; hour 12: 20 and 15 m; other hours none.
    let mut hours = [0u16; 24];
    hours[0] = 1 << 1;
    hours[12] = 1 << 5 | 1 << 7;
    let h = hop_plan(hours, true);
    assert_eq!(h.band_at(DAY + 600.0), Some(1));
    assert_eq!(h.band_at(DAY + 3600.0), None);
    assert_eq!(h.band_at(DAY + 12.0 * 3600.0 + 600.0), Some(5));
    assert_eq!(h.band_at(DAY + 12.0 * 3600.0 + 840.0), Some(7));
    assert_eq!(h.band_at(DAY + 12.0 * 3600.0 + 960.0), Some(5));
    // 7. The text of the masks: a round trip keeps them, and other text is refused.
    let text = hours_text(&hours);
    let back = parse_hours(&text).unwrap();
    assert_eq!((back[0], back[12], back[5]), (2, 0xA0, 0));
    assert_eq!(text.split(',').nth(12), Some("A0"));
    for bad in ["", "1,2,3", "G,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0", &format!("{text},0")] {
        assert!(parse_hours(bad).is_err(), "{bad}");
    }
    let schema = sdrkit_stack_wsjt::settings::schema_for(Some(Mode::Ft8));
    let mut v = settings(Some(Mode::Ft8), &[]);
    v.set(schema.id("hop.hours").unwrap(), &Value::Text("1,2,3".into()))
        .unwrap();
    assert!(sdrkit_stack_wsjt::receiver::receiver(Some(Mode::Ft8), &StackConfig::new(v)).is_err());
}

#[test]
fn band_hopping_asks_for_the_dial_of_the_band() {
    let (bands, cycle) = hop_lists();
    let mut on = vec!["0"; 24];
    on[0] = "20";
    let hours = on.join(",");
    let [dials, edges] = band_settings(&[]);
    let mut rx = receiver(
        Some(Mode::Ft8),
        &[
            ("hop.enabled", Value::Bool(true)),
            ("hop.bands", Value::Text(bands)),
            ("hop.cycle", Value::Text(cycle)),
            ("hop.hours", Value::Text(hours)),
            dials,
            edges,
        ],
        8000.0,
        0.0,
    );
    let mut out = Output::default();
    let mut asked = Vec::new();
    for _ in 0..10 {
        rx.feed(&vec![Complex32::default(); 800], &mut out);
        asked.append(&mut out.requests);
        out.clear();
    }
    // 2026-01-01 00:00: hour 0 has 20 m only; the receiver is tuned to 14 MHz.
    assert_eq!(
        asked,
        vec![HostRequest::Retune {
            tuned_hz: 14.074e6,
            reason: "band hopping: FT8 on 20m".into()
        }]
    );
    let r = rx.results();
    assert_eq!(field(r, "hop.band"), Some(&Value::Text("20m".into())));
    assert_eq!(field_f64(r, "hop.next_utc"), START + 120.0);
    // Off while the whole span is decoded.
    let mut rx = receiver(
        Some(Mode::Ft8),
        &[
            ("hop.enabled", Value::Bool(true)),
            ("hop.bands", Value::Text(hop_lists().0)),
            ("hop.hours", Value::Text(on.join(","))),
            ("wide", Value::Bool(true)),
        ],
        8000.0,
        0.0,
    );
    rx.feed(&vec![Complex32::default(); 800], &mut out);
    assert!(out.requests.is_empty());
}

/// The hop request of FT8 when the band of hour 0 is `band`, with the dials of `rows`.
fn hop_request(band: &str, rows: &[(&str, &str)]) -> Vec<HostRequest> {
    let (bands, cycle) = hop_lists();
    let bit = bands.split(',').position(|b| b == band).unwrap();
    let mut on = vec!["0".to_string(); 24];
    on[0] = format!("{:X}", 1u16 << bit);
    let [dials, edges] = band_settings(rows);
    let mut rx = receiver(
        Some(Mode::Ft8),
        &[
            ("hop.enabled", Value::Bool(true)),
            ("hop.bands", Value::Text(bands)),
            ("hop.cycle", Value::Text(cycle)),
            ("hop.hours", Value::Text(on.join(","))),
            dials,
            edges,
        ],
        8000.0,
        0.0,
    );
    let mut out = Output::default();
    let mut asked = Vec::new();
    for _ in 0..10 {
        rx.feed(&vec![Complex32::default(); 800], &mut out);
        asked.append(&mut out.requests);
        out.clear();
    }
    asked
}

#[test]
fn band_hopping_takes_the_dials_that_the_host_gives() {
    // Two rows of the dial file: the request is the dial of the row of the band.
    for (band, hz) in [("20m", 14.074e6), ("40m", 7.074e6)] {
        assert_eq!(
            hop_request(band, &[("ft8", "20m"), ("ft8", "40m")]),
            vec![HostRequest::Retune {
                tuned_hz: hz,
                reason: format!("band hopping: FT8 on {band}")
            }],
            "{band}"
        );
    }
    // No dial for the band: no request.
    assert!(hop_request("40m", &[("ft8", "20m")]).is_empty());
    // Every stack of the family takes the settings of the files.
    for d in sdrkit_stack_wsjt::descriptors() {
        let schema = (d.settings)();
        let mut v = sdrkit_core::Values::defaults(&schema);
        for (k, x) in band_settings(&[]) {
            if let Ok(id) = schema.id(k) {
                v.set(id, &x).unwrap();
            }
        }
        assert!((d.make_receiver)(&StackConfig::new(v)).is_ok(), "{}", d.id);
    }
}

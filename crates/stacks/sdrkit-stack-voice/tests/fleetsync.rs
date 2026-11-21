//! Acceptance tests of FleetSync: the frame, the block check and the FleetSync II code against the worked examples of the
//! specification, the burst decoder on audio, and the stack on several carriers at once.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Choice, InputInfo, MemStorage, Receiver, Value};
use sdrkit_proto_tones::mdc;
use sdrkit_stack_voice::fleetsync::code::{
    bits_audio, burst_audio, burst_bits, fs2_blocks, fs2_decode, BitRate, BurstDecoder,
    BurstOptions, Decoded, Kind, Message, FS2_WORDS,
};
use sdrkit_stack_voice::fleetsync::{GenFrame, GenRate, RateChoice, COVERAGE};
use sdrkit_stack_voice::scene::{SceneConfig, Transmission};
use sdrkit_stack_voice::typed;
use sdrkit_stack_voice::{FLEETSYNC, MDC1200};
use std::f64::consts::TAU;
use std::sync::Arc;

mod common;
use common::{feed, generated_of, receiver_of, samples};

const RATE: f64 = 48_000.0;
const CENTRE: f64 = 155e6;

fn decode(audio: &[f32], rate: f64, bit_rate: BitRate) -> (Vec<Decoded>, u64) {
    let mut d = BurstDecoder::new(rate, bit_rate).unwrap();
    let mut out = Vec::new();
    d.process(audio, |x| out.push(x));
    d.reset();
    (out, d.check_failures())
}

fn plain(fleet: u16, unit: u16) -> Message {
    Message::new(fleet, unit, 0, 0).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The frame and the codes.

#[test]
fn the_worked_examples_give_the_stated_words_and_identities() {
    for (fleet, unit, raw1, raw2, w1, w2) in [
        (107, 1772, 0, 0, 0x0000_0830, 0x5000_4854),
        (200, 3000, 0x1234, 0xABC, 0x1234_657D, 0x1ABC_AB6F),
        (99, 999, 0, 0, 0, 0x0000_0003),
    ] {
        let m = Message::new(fleet, unit, raw1, raw2).unwrap();
        assert_eq!((m.word1, m.word2), (w1, w2), "fleet {fleet} unit {unit}");
        let back = Message {
            word1: w1,
            word2: w2,
        };
        assert!(back.check_holds());
        assert_eq!((back.fleet(), back.unit()), (fleet, unit));
        assert_eq!((back.raw1(), back.raw2()), (raw1, raw2));
    }
    // Fleet 99 and unit 999 are on the wire but outside the maker's ranges.
    assert!(!plain(99, 999).in_maker_range());
    assert!(plain(107, 1772).in_maker_range());
    assert!(!plain(350, 1772).in_maker_range() && !plain(107, 5000).in_maker_range());
    assert!(Message::new(98, 1000, 0, 0).is_none() && Message::new(100, 5095, 0, 0).is_none());
}

#[test]
fn example_one_as_fleetsync_ii_has_the_stated_blocks() {
    assert_eq!(
        fs2_blocks(&plain(107, 1772)),
        [
            0x0000_0000_0000_0000,
            0x6881_0000_0000_7304,
            0x0000_150D_0000_0000,
            0x6881_440E_440E_150D
        ]
    );
}

/// The parity masks of the specification, bit 14 the first sent.
const MASKS: [u16; 8] = [
    0x4045, 0x2067, 0x1076, 0x083B, 0x0458, 0x022C, 0x0116, 0x008B,
];

fn syndrome(w: u16) -> u8 {
    MASKS
        .iter()
        .enumerate()
        .fold(0, |s, (i, &m)| s | (((w & m).count_ones() & 1) as u8) << i)
}

#[test]
fn the_code_words_and_syndromes_are_the_stated_ones() {
    const SINGLE: [u8; 15] = [
        0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x17, 0x2E, 0x5C, 0xB8, 0x67, 0xCE, 0x8B,
    ];
    for (nib, &w) in FS2_WORDS.iter().enumerate() {
        assert_eq!(usize::from((w >> 8) & 0xF), nib);
        assert_eq!(syndrome(w), 0, "{w:04X}");
        for (i, &s) in SINGLE.iter().enumerate() {
            assert_eq!(syndrome(w ^ (1 << (14 - i))), s, "c{}", 14 - i);
        }
    }
}

#[test]
fn every_single_wrong_code_word_bit_is_repaired() {
    let m = plain(107, 1772);
    let blocks = fs2_blocks(&m);
    let mut repaired = 0;
    for b in 0..4 {
        for slot in 0..4 {
            for bit in 0..15 {
                let mut x = blocks;
                x[b] ^= 1 << (48 - 16 * slot + bit);
                let (got, n) = fs2_decode(&x).expect("repaired");
                assert_eq!((got, n), (m, 1));
                repaired += 1;
            }
        }
    }
    assert_eq!(repaired, 240);
    // Two wrong bits in one code word fail the burst.
    let mut x = blocks;
    x[3] ^= 0b11 << 16;
    assert_eq!(fs2_decode(&x), None);
}

#[test]
fn a_plain_frame_with_wrong_data_bits_is_refused() {
    let m = Message::new(200, 3000, 0x1234, 0xABC).unwrap();
    for i in 16..64 {
        let one = Message::of_block(m.block() as u64 ^ (1 << i));
        assert!(!one.check_holds(), "bit {i}");
        for j in (16..64).filter(|&j| j != i).step_by(7) {
            let two = Message::of_block(m.block() as u64 ^ (1 << i) ^ (1 << j));
            assert!(!two.check_holds(), "bits {i} and {j}");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The burst decoder on audio.

#[test]
fn thirty_identities_come_back_plain_and_fleetsync_ii() {
    for kind in [Kind::Plain, Kind::Fs2] {
        for fleet in [99, 100, 107, 200, 354] {
            for unit in [999, 1000, 1015, 1772, 3000, 5094] {
                let m = Message::new(fleet, unit, 0x1234, 0xABC).unwrap();
                let o = BurstOptions {
                    kind,
                    ..BurstOptions::default()
                };
                let mut a = vec![0.0; 500];
                a.extend(burst_audio(&m, RATE, &o));
                a.extend([0.0; 500]);
                let (got, failed) = decode(&a, RATE, BitRate::B1200);
                assert_eq!(got.len(), 1, "{kind:?} {fleet} {unit}");
                assert_eq!(failed, 0);
                let g = got[0];
                assert_eq!((g.message, g.kind, g.repaired), (m, kind, 0));
                assert_eq!((g.message.fleet(), g.message.unit()), (fleet, unit));
                assert_eq!((g.message.raw1(), g.message.raw2()), (0x1234, 0xABC));
            }
        }
    }
}

#[test]
fn a_clean_burst_is_one_burst_at_both_tone_senses_and_both_audio_rates() {
    for rate in [48_000.0, 44_100.0] {
        for inverted in [false, true] {
            for kind in [Kind::Plain, Kind::Fs2] {
                let o = BurstOptions {
                    kind,
                    inverted,
                    ..BurstOptions::default()
                };
                let mut a = vec![0.0; 800];
                a.extend(burst_audio(&plain(107, 1772), rate, &o));
                a.extend([0.0; 800]);
                let (got, _) = decode(&a, rate, BitRate::B1200);
                assert_eq!(got.len(), 1, "{rate} {inverted} {kind:?}");
                assert_eq!(got[0].inverted, inverted);
                assert_eq!(got[0].message, plain(107, 1772));
            }
        }
    }
}

/// Voice-like audio: three tones with a gliding pitch at `level`.
fn voice(n: usize, level: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let f0 = rng.range(110.0, 220.0);
    let (mut p1, mut p2, mut p3) = (0.0f64, 0.0f64, 0.0f64);
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE;
            let f = f0 * (1.0 + 0.2 * (TAU * 3.0 * t).sin());
            p1 += 3.0 * f / RATE;
            p2 += 7.0 * f / RATE;
            p3 += 13.0 * f / RATE;
            level
                * (0.5 * (TAU * p1).sin() + 0.3 * (TAU * p2).sin() + 0.2 * (TAU * p3).sin()) as f32
        })
        .collect()
}

#[test]
fn forty_bursts_in_noise_with_clock_errors_and_voice_are_each_found_once() {
    let mut rng = Rng::new(11);
    let mut found = 0;
    for i in 0..40u64 {
        let ppm = -800.0 + 400.0 * (i % 5) as f64;
        let amplitude = 0.2 + 0.1 * (i % 3) as f32 / 2.0;
        let fleet = 100 + (i * 6) as u16;
        let unit = 1000 + (i * 97) as u16;
        let o = BurstOptions {
            kind: if i % 2 == 0 { Kind::Plain } else { Kind::Fs2 },
            amplitude,
            ..BurstOptions::default()
        };
        let m = plain(fleet, unit);
        let silence = 1000 + (i as usize * 137) % 5344;
        let mut a = vec![0.0f32; silence];
        a.extend(voice(9600, 0.2, i));
        // The sender's clock is off by `ppm`.
        a.extend(burst_audio(&m, RATE / (1.0 + ppm * 1e-6), &o));
        a.extend(voice(9600, 0.2, i + 100));
        for v in &mut a {
            *v += 0.02 * rng.normal_f32();
        }
        let (got, _) = decode(&a, RATE, BitRate::B1200);
        assert_eq!(got.len(), 1, "burst {i}, {ppm} ppm");
        assert_eq!(got[0].message, m, "burst {i}");
        found += 1;
    }
    assert_eq!(found, 40);
}

#[test]
fn three_bursts_in_a_row_are_three_in_order() {
    let mut a = Vec::new();
    for unit in [1001, 1002, 1003] {
        a.extend(burst_audio(
            &plain(150, unit),
            RATE,
            &BurstOptions::default(),
        ));
        a.extend([0.0; 3000]);
    }
    let (got, _) = decode(&a, RATE, BitRate::B1200);
    let units: Vec<u16> = got.iter().map(|d| d.message.unit()).collect();
    assert_eq!(units, [1001, 1002, 1003]);
}

#[test]
fn noise_alone_gives_no_burst() {
    let mut rng = Rng::new(5);
    let a: Vec<f32> = (0..(5.0 * RATE) as usize)
        .map(|_| 0.3 * rng.normal_f32())
        .collect();
    for r in [BitRate::B1200, BitRate::B2400] {
        assert!(decode(&a, RATE, r).0.is_empty(), "{r:?}");
    }
}

#[test]
fn a_damaged_burst_is_refused_and_counted() {
    let m = plain(107, 1772);
    for (kind, flips) in [
        // Two wrong bits, c14 and c7, in the first code word of the last block.
        (Kind::Fs2, [261, 268]),
        // Two wrong data bits of a plain frame; it is then read on to the length of FleetSync II and fails.
        (Kind::Plain, [80, 100]),
    ] {
        let o = BurstOptions {
            kind,
            ..BurstOptions::default()
        };
        let mut bits = burst_bits(&m, &o);
        for i in flips {
            bits[i] ^= 1;
        }
        let mut a = vec![0.0; 500];
        a.extend(bits_audio(&bits, RATE, &o));
        a.extend([0.0; 9000]);
        let (got, failed) = decode(&a, RATE, BitRate::B1200);
        assert!(got.is_empty(), "{kind:?} {got:?}");
        assert_eq!(failed, 1, "{kind:?}");
    }
}

#[test]
fn bursts_at_2400_bits_a_second() {
    for kind in [Kind::Plain, Kind::Fs2] {
        let o = BurstOptions {
            kind,
            bit_rate: BitRate::B2400,
            ..BurstOptions::default()
        };
        let mut a = vec![0.0; 600];
        a.extend(burst_audio(&plain(300, 4000), RATE, &o));
        a.extend([0.0; 600]);
        let (got, _) = decode(&a, RATE, BitRate::B2400);
        assert_eq!(got.len(), 1, "{kind:?}");
        assert_eq!(
            (got[0].message, got[0].bit_rate),
            (plain(300, 4000), BitRate::B2400)
        );
        // The decoder of the other rate does not read it.
        assert!(decode(&a, RATE, BitRate::B1200).0.is_empty());
    }
}

#[test]
fn an_mdc_1200_burst_is_not_fleetsync_and_the_reverse() {
    let mut a = vec![0.0; 600];
    a.extend(mdc::burst_audio(
        &mdc::Packet::new(0x01, 0x80, 0x1234),
        RATE,
        0.3,
        7,
    ));
    a.extend([0.0; 600]);
    for r in [BitRate::B1200, BitRate::B2400] {
        assert!(decode(&a, RATE, r).0.is_empty(), "{r:?}");
    }
    for kind in [Kind::Plain, Kind::Fs2] {
        let o = BurstOptions {
            kind,
            ..BurstOptions::default()
        };
        let mut a = vec![0.0; 600];
        a.extend(burst_audio(&plain(107, 1772), RATE, &o));
        a.extend([0.0; 600]);
        let mut d = mdc::BurstDecoder::new(RATE).unwrap();
        let mut n = 0;
        d.process(&a, |_| n += 1);
        assert_eq!(n, 0, "{kind:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The stack.

fn receiver(set: &[(&str, Value)]) -> Box<dyn Receiver> {
    receiver_of(&FLEETSYNC, None, set, Arc::new(MemStorage::new()))
}

/// One row of the `bursts` table.
#[derive(Clone, Debug, PartialEq)]
struct BurstRow {
    fleet: i64,
    unit: i64,
    kind: String,
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
                rec.at(&typed::FleetsyncBurstRow::SERIAL).unwrap(),
                BurstRow {
                    fleet: rec.at(&typed::FleetsyncBurstRow::FLEET).unwrap(),
                    unit: rec.at(&typed::FleetsyncBurstRow::UNIT).unwrap(),
                    kind: rec.at(&typed::FleetsyncBurstRow::KIND).unwrap().to_owned(),
                    hz: rec.at(&typed::FleetsyncBurstRow::HZ).unwrap(),
                    text: rec.at(&typed::FleetsyncBurstRow::TEXT).unwrap().to_owned(),
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

fn talker(offset_hz: f64, start_s: f64, burst: Vec<f32>, seed: u64) -> Transmission {
    Transmission {
        amplitude: 0.15,
        deviation: 0.7,
        burst: Some(burst),
        seed,
        ..Transmission::on(CENTRE + offset_hz).at(start_s, 1.6)
    }
}

fn scene(tx: Vec<Transmission>) -> SceneConfig {
    SceneConfig {
        rate_hz: 1e6,
        centre_hz: CENTRE,
        length_s: 4.2,
        noise_rms: 0.005,
        tx,
        seed: 7,
    }
}

#[test]
fn the_test_signal_gives_a_plain_and_a_fleetsync_ii_burst_at_default_settings() {
    let x = generated_of(&FLEETSYNC, 2e6, 6.0, 1);
    let mut rx = receiver(&[]);
    let h = feed(
        rx.as_mut(),
        &x,
        2e6,
        common::dial("fleetsync", None),
        65_536,
    );
    let got = bursts(rx.as_ref());
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(
        (got[0].fleet, got[0].unit, got[0].kind.as_str()),
        (107, 1772, "FleetSync")
    );
    assert_eq!(
        (got[1].fleet, got[1].unit, got[1].kind.as_str()),
        (107, 1773, "FleetSync II")
    );
    assert_eq!(got[0].text, "FleetSync fleet 107 unit 1772");
    assert!((got[0].hz - (155e6 - 37.5e3)).abs() < 4e3);
    assert!((got[1].hz - (155e6 + 62.5e3)).abs() < 4e3);
    assert_eq!(rows(rx.as_ref(), "units"), 2);
    assert_eq!(rows(rx.as_ref(), "calls"), 2);
    let f = rx.results().fields();
    assert_eq!(f.at(&typed::FleetsyncFields::BURSTS), Some(2));
    assert_eq!(f.at(&typed::FleetsyncFields::UNITS), Some(2));
    assert_eq!(f.at(&typed::FleetsyncFields::CHECK_FAILURES), Some(0));
    assert_eq!(
        h.events
            .iter()
            .filter(|e| e.is(&typed::FLEETSYNC_BURST))
            .count(),
        2
    );
    assert_eq!(
        h.events
            .iter()
            .filter(|e| e.is(&typed::FLEETSYNC_CALL))
            .count(),
        2
    );
    let summary = rx.results().status().summary.clone();
    assert!(
        summary.ends_with(&format!("2 bursts ({COVERAGE})")),
        "{summary}"
    );
    let log: Vec<String> = rx.results().log_lines().iter().cloned().collect();
    assert!(
        log.iter()
            .any(|l| l == "bursts: FleetSync 154.9625 MHz: fleet 107 unit 1772 (FleetSync)"),
        "{log:?}"
    );
}

#[test]
fn the_test_signal_at_2400_bits_a_second_is_read_with_automatic_rate() {
    let x = generated_of_with(&[
        ("gen_bit_rate", GenRate::B2400.index()),
        ("gen_frame", GenFrame::Fs2.index()),
    ]);
    let mut rx = receiver(&[]);
    feed(rx.as_mut(), &x, 2e6, CENTRE, 65_536);
    let got = bursts(rx.as_ref());
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.iter().all(|b| b.kind == "FleetSync II"));
    // Read at 1200 bit/s only, nothing is found.
    let mut rx = receiver(&[("bit_rate", Value::Choice(RateChoice::B1200.index()))]);
    feed(rx.as_mut(), &x, 2e6, CENTRE, 65_536);
    assert!(bursts(rx.as_ref()).is_empty());
}

/// The test signal of the stack at 2 Msps, 6 s, with the generator choices `set` (key and choice index).
fn generated_of_with(set: &[(&str, u16)]) -> Vec<sdrkit_core::Complex32> {
    let schema = sdrkit_components::sim::with_generator_settings(&(FLEETSYNC.settings)()).unwrap();
    let mut v = sdrkit_core::Values::defaults(&schema);
    for (k, i) in set {
        let id = schema.id(k).unwrap();
        v.set(id, &Value::Choice(*i)).unwrap();
    }
    let cfg = sdrkit_core::GenConfig::new(2e6, v).with_seed(1);
    let mut g = (FLEETSYNC.make_generator.unwrap())(&cfg).unwrap();
    let mut x = vec![sdrkit_core::Complex32::default(); 12_000_000];
    assert_eq!(g.generate(&mut x), x.len());
    x
}

#[test]
fn two_carriers_one_off_the_raster_are_each_reported_once() {
    let a = burst_audio(
        &plain(120, 2001),
        RATE,
        &BurstOptions {
            amplitude: 0.2,
            ..Default::default()
        },
    );
    let b = burst_audio(
        &plain(121, 2002),
        RATE,
        &BurstOptions {
            kind: Kind::Fs2,
            amplitude: 0.2,
            ..Default::default()
        },
    );
    let mut second = talker(50e3, 1.0, b, 2);
    second.offset_hz = 1100.0;
    let mut rx = receiver(&[]);
    feed(
        rx.as_mut(),
        &samples(&scene(vec![talker(-25e3, 0.3, a, 1), second])),
        1e6,
        CENTRE,
        65_536,
    );
    let got = bursts(rx.as_ref());
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(
        (got[0].fleet, got[0].unit, got[0].kind.as_str()),
        (120, 2001, "FleetSync")
    );
    assert!((got[0].hz - (CENTRE - 25e3)).abs() < 4e3, "{}", got[0].hz);
    assert_eq!(
        (got[1].fleet, got[1].unit, got[1].kind.as_str()),
        (121, 2002, "FleetSync II")
    );
    assert!((got[1].hz - (CENTRE + 51.1e3)).abs() < 4e3, "{}", got[1].hz);
}

#[test]
fn the_stacks_do_not_list_each_others_bursts() {
    // MDC-1200 bursts into the FleetSync stack.
    let mdc_burst = |unit| mdc::burst_audio(&mdc::Packet::new(0x01, 0x80, unit), RATE, 0.2, 7);
    let cfg = scene(vec![
        talker(-25e3, 0.3, mdc_burst(0x1234), 1),
        talker(50e3, 1.0, mdc_burst(0xBEEF), 2),
    ]);
    let mut rx = receiver(&[]);
    feed(rx.as_mut(), &samples(&cfg), 1e6, CENTRE, 65_536);
    assert!(bursts(rx.as_ref()).is_empty());
    assert_eq!(rows(rx.as_ref(), "calls"), 2);
    // FleetSync bursts into the MDC-1200 stack.
    let x = generated_of(&FLEETSYNC, 2e6, 6.0, 1);
    let mut rx = receiver_of(&MDC1200, None, &[], Arc::new(MemStorage::new()));
    feed(rx.as_mut(), &x, 2e6, CENTRE, 65_536);
    assert_eq!(rows(rx.as_ref(), "bursts"), 0);
    assert_eq!(rows(rx.as_ref(), "calls"), 2);
}

#[test]
fn clear_empties_the_lists_and_a_low_rate_is_refused() {
    let x = generated_of(&FLEETSYNC, 2e6, 6.0, 1);
    let mut rx = receiver(&[]);
    feed(rx.as_mut(), &x, 2e6, CENTRE, 65_536);
    assert_eq!(bursts(rx.as_ref()).len(), 2);
    let id = rx.commands().id("clear").unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(rx.as_mut(), id, &args).unwrap();
    assert!(bursts(rx.as_ref()).is_empty());
    assert_eq!(rows(rx.as_ref(), "units"), 0);
    assert_eq!(
        rx.results().fields().at(&typed::FleetsyncFields::BURSTS),
        Some(0)
    );
    let mut rx = receiver(&[]);
    assert!(rx
        .configure(&InputInfo::new(200e3, CENTRE, CENTRE).recording())
        .is_err());
    assert_eq!(
        rx.results().status().summary,
        "FleetSync: the sample rate is too low (250 kHz at least)"
    );
}

//! The codes of FST4, FST4W and MSK144: the CRC known answers, the printed tones and frames of known messages, and each
//! stack's test signal sent and received through the whole stack at a weak level with default settings.

use super::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::verify::{Outcome, VerifyOptions};
use sdrkit_core::{Complex32, GenConfig, LockState, Receiver, StackConfig};
use sdrkit_dev_test::Harness;
use sdrkit_stack_wsjt::encode::{self, channel_symbols};
use sdrkit_stack_wsjt::generator::{stations, WsjtGenerator};
use sdrkit_stack_wsjt::proto::msg77::pack77;
use sdrkit_stack_wsjt::{descriptors, Mode, Sub};
use sdrkit_util_bits::crc::cyclic::{remainder, FST4_CRC24, MSK144_CRC13};

/// The digits of a printed table, blanks and line labels left out: each line is a start index, then groups of digits.
fn digits(text: &str) -> Vec<u8> {
    text.lines()
        .flat_map(|l| l.split_whitespace().skip(1))
        .flat_map(|g| g.bytes().map(|b| b - b'0'))
        .collect()
}

/// The bits of the ASCII text `s`, most significant bit of each byte first.
fn ascii_bits(s: &str) -> Vec<u8> {
    s.bytes().flat_map(|c| (0..8).rev().map(move |k| c >> k & 1)).collect()
}

/// The `n` bits of a hex text, most significant bit first.
fn hex_bits(h: &str, n: usize) -> Vec<u8> {
    let mut v = Vec::new();
    sdrkit_util_bits::pack::hex_to_bits(h, &mut v).unwrap();
    v.truncate(n);
    v
}

#[test]
fn the_crcs_give_the_printed_values() {
    // Plain CRCs of "123456789": register 0, most significant bit first, no reflection, no final exclusive-or.
    let b = ascii_bits("123456789");
    assert_eq!(FST4_CRC24.compute(&b), 0xE6_C948);
    assert_eq!(remainder(&b, 13, 0x35D7, 13), 0x07B8);
    // FST4: the CRC of the scrambled 77 bits. The message of 77 zeros scrambles to the scrambling vector.
    let info = encode::fst4_info(&[0; 77], false);
    assert_eq!(info, hex_bits("4a5e89b4b08a7955be2ed36970", 101));
    let info = encode::fst4_info(&pack77("CQ K1ABC FN42").unwrap(), false);
    assert_eq!(info, hex_bits("4a5e8994fd6563dfa7a534a990", 101));
    assert_eq!(FST4_CRC24.compute(&info[..77]), 0xA6_9532);
    // FST4W: the CRC of the 50 bits, not scrambled.
    let info = encode::fst4_info(&hex_bits("09bde3550ccb0", 50), true);
    assert_eq!(info, hex_bits("09bde3550ccb18d5e6c", 74));
    assert_eq!(FST4_CRC24.compute(&hex_bits("f70c238b0d194", 50)), 0x45_5258);
    // MSK144: the remainder of the 77 bits and 19 zeros, the plain CRC of the 77 bits and 6 zeros.
    let m = pack77("CQ K1ABC FN42").unwrap();
    assert_eq!(MSK144_CRC13.compute(&m), 0x03C1);
    let mut m6 = m.clone();
    m6.extend([0; 6]);
    assert_eq!(remainder(&m6, 13, 0x35D7, 13), 0x03C1);
    assert_eq!(encode::msk144_info(&m), hex_bits("000000204def1a8a1988f04", 90));
}

#[test]
fn the_printed_tones_and_frames() {
    let fst4 = digits(
        "  0  0132102310 3311233031 3110222113 1113022123
         40  1032012233 1233110210 3331311303 3203010132
         80  1023321132 2021033021 0303223311 1000231032
        120  0102320213 0222323103 1213221131 0101321023",
    );
    assert_eq!(channel_symbols(&Sub::new(Mode::Fst4), "CQ K1ABC FN42").unwrap(), fst4);
    let fst4w = digits(
        "  0  0132102300 3132212302 1111002020 3201302123
         40  1032011123 1323232303 0132223312 1322100132
         80  1023000312 1120221323 2033112312 1102231032
        120  0110330000 0221010012 2331322123 0101321023",
    );
    assert_eq!(channel_symbols(&Sub::new(Mode::Fst4w), "K1ABC FN42 37").unwrap(), fst4w);
    let frame = digits(
        "  0  0111001000 0000000000 0000000000 0000100000
         40  0100110111 1011110111 0010000110 1010001010
         80  0001100110 0010001111 0000011001 0110100001
        120  0000000010 0110000010 1010",
    );
    let tones = digits(
        "  0  1100001101 0101010101 0101010101 0100110101
         40  1000001101 1001001100 0011011110 1011001011
         80  0111111111 0011000100 0101111110 1110110110
        120  0101010011 1111010010 1011",
    );
    let sub = Sub::new(Mode::Msk144);
    assert_eq!(channel_symbols(&sub, "CQ K1ABC FN42").unwrap(), frame);
    assert_eq!(encode::frame_tones(&sub, &frame), tones);
}

/// The strongest stations of `mode`'s test signal, sent by its generator with default settings and noise that puts the
/// strongest at `snr` dB in 2500 Hz, are decoded by its receiver with default settings, which locks; nothing else is decoded.
fn sends_and_receives(mode: Mode, snr: f64, seconds: f64, want: usize) {
    let s = settings(Some(mode), &[]);
    let mut g = WsjtGenerator::from_config(mode, &GenConfig::new(12000.0, s.clone()).with_seed(1));
    let mut iq = vec![Complex32::default(); (12000.0 * seconds) as usize];
    assert_eq!(g.generate(&mut iq), iq.len());
    add_noise(&mut iq, snr, 12000.0, 7);
    let mut rx = sdrkit_stack_wsjt::receiver::receiver(Some(mode), &StackConfig::new(s)).unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(12000.0, 14e6, 14e6).recording())
        .unwrap();
    let got = texts(&feed(&mut rx, &iq, 12000));
    let list = stations(mode);
    for st in &list[..want] {
        assert!(got.iter().any(|t| t == st.0), "{mode:?}: {} missing in {got:?}", st.0);
    }
    assert!(got.iter().all(|t| list.iter().any(|st| st.0 == t)), "{mode:?}: {got:?}");
    let st = sdrkit_core::owner::Owner::results(&rx).status();
    assert_eq!(st.lock_state, LockState::Locked, "{mode:?}: {}", st.summary);
}

#[test]
fn fst4_sends_and_receives_its_test_signal() {
    sends_and_receives(Mode::Fst4, -12.0, 125.0, 5);
}

#[test]
fn fst4w_sends_and_receives_its_test_signal() {
    sends_and_receives(Mode::Fst4w, -14.0, 245.0, 2);
}

#[test]
fn msk144_sends_and_receives_its_test_signal() {
    sends_and_receives(Mode::Msk144, -6.0, 31.0, 1);
}

#[test]
fn the_verify_cases_of_the_coded_stacks_pass() {
    let o = VerifyOptions {
        seconds: 1.0,
        clean: false,
        snr_db: 20.0,
    };
    for desc in descriptors().into_iter().filter(|d| ["fst4", "fst4w", "msk144"].contains(&d.id)) {
        let hooks = desc.verify.expect("a verify case");
        for case in (hooks.cases)(&o) {
            let mut h = Harness::new(desc).with_settings(&case.settings).seconds(case.seconds);
            if let Some(snr) = case.snr_db {
                h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, snr);
            }
            let r = h.run().expect("the run");
            for line in (case.check.expect("checks"))(&r.results) {
                assert_eq!(line.outcome, Outcome::Pass, "{} {}: {} {}", desc.id, case.name, line.key, line.detail);
            }
            assert_eq!(r.results.status().lock_state, LockState::Locked, "{}", desc.id);
        }
    }
}

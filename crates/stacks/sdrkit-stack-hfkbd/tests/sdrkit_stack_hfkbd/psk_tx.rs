//! The PSK sender: the QPSK phase table of the designer's description, in the code and measured on the samples, the bits and
//! turns of the specification's worked examples, every variant read back by the stack's own receiver at 12 dB, the pulse (the
//! phase at the symbol boundaries and the zero at mid symbol of a reversal), the occupied bandwidth, the frame of a burst,
//! `stop`, `tune`, the gate and the characters.

use crate::common::*;
use crate::tx_common::*;
use hf::psk::{qpsk_turns, PskCoder, Variant};
use hf::sender::{burst, Element, Envelope};
use sdrkit_core::{Complex32, Value};
use sdrkit_dev_test::Run;
use sdrkit_stack_hfkbd as hf;
use std::f64::consts::TAU;

/// The phase table of the designer's description (appendix "The convolutional code"): for each five-bit window, the left bit
/// sent first, the code of the phase shift: 0 none, 1 advance by 90 degrees, 2 reversal, 3 retard by 90 degrees.
const PHASE_TABLE: [u32; 32] = [
    2, 1, 3, 0, 3, 0, 2, 1, // 00000 to 00111
    0, 3, 1, 2, 1, 2, 0, 3, // 01000 to 01111
    1, 2, 0, 3, 0, 3, 1, 2, // 10000 to 10111
    3, 0, 2, 1, 2, 1, 3, 0, // 11000 to 11111
];

/// Every window of the K = 5 code turns the phase as the table states, an advance counted counter-clockwise.
#[test]
fn psk_tx_qpsk_follows_the_published_phase_table() {
    for w in 0..32u32 {
        let mut e = sdrkit_algo_fec::conv::Encoder::new(hf::fec::QPSK_K5);
        let mut sym = 0;
        for k in (0..5).rev() {
            sym = u32::from(e.push_bit(((w >> k) & 1) as u8));
        }
        assert_eq!(qpsk_turns(sym), PHASE_TABLE[w as usize], "window {w:05b}");
    }
}

/// QPSK31 "CQ" measured from the samples: the phase of the 1000 Hz tone at each symbol boundary, and the step between two
/// boundaries in quarter turns, an advance of the tone's phase counted positive. The steps are those of the published phase
/// table over the five-bit window of the bits: 32 zeros, the varicode of "CQ" with its gaps (worked example 1), then the
/// postamble of ones.
#[test]
fn psk_tx_qpsk31_phase_steps_match_the_published_table() {
    let set = settings(Variant::Qpsk31);
    let x = one_burst(&hf::PSK, &set, "CQ");
    let c = PskCoder::new(Variant::Qpsk31, 1000.0);
    let bits: Vec<u32> = std::iter::repeat_n(0, c.preamble())
        .chain("101011010011101110100".bytes().map(|b| u32::from(b - b'0')))
        .chain(std::iter::repeat_n(1, c.postamble()))
        .collect();
    let mut window = 0usize;
    let want: Vec<u32> = bits
        .iter()
        .map(|&b| {
            window = ((window << 1) | b as usize) & 31;
            PHASE_TABLE[window]
        })
        .collect();
    let lead = (0.15 * RATE) as usize;
    let sym = RATE / 31.25;
    let tone = |k: usize| {
        let n = lead + (k as f64 * sym).round() as usize;
        let p = -TAU * 1000.0 * (n - lead) as f64 / RATE;
        x[n] * Complex32::new(p.cos() as f32, p.sin() as f32)
    };
    // Boundary 1 ends the rise of the carrier; bit i turns the phase between boundaries i + 1 and i + 2.
    let got: Vec<u32> = (0..bits.len())
        .map(|i| {
            let step = f64::from((tone(i + 2) * tone(i + 1).conj()).arg());
            ((step / (TAU / 4.0)).round() as i64).rem_euclid(4) as u32
        })
        .collect();
    assert_eq!(got, want);
    assert!(
        want.contains(&1) && want.contains(&3),
        "both senses of a quarter turn"
    );
}

/// The phase turn of an element, in quarter turns counter-clockwise.
fn quarter_turns(e: &Element) -> u32 {
    let Envelope::Cosine(from, to) = e.env else {
        panic!("a PSK element");
    };
    let a = f64::from((to * from.conj()).arg());
    (((a / (TAU / 4.0)).round() as i64).rem_euclid(4)) as u32
}

/// Worked example 1: BPSK31 "CQ" is 32 reversals, the bits 101011010011101110100, then the steady carrier.
#[test]
fn psk_tx_bpsk31_worked_example() {
    let mut c = PskCoder::new(Variant::Bpsk31, 1000.0);
    let e = burst(&mut c, "CQ");
    assert_eq!(
        e[0].env,
        Envelope::Cosine(Complex32::default(), Complex32::new(1.0, 0.0))
    );
    assert_eq!(
        e.last().unwrap().env,
        Envelope::Cosine(
            match e[e.len() - 2].env {
                Envelope::Cosine(_, to) => to,
                _ => unreachable!(),
            },
            Complex32::default()
        )
    );
    let bits: String = e[1..e.len() - 1]
        .iter()
        .map(|x| if quarter_turns(x) == 0 { '1' } else { '0' })
        .collect();
    let want = format!(
        "{}{}{}",
        "0".repeat(32),
        "101011010011101110100",
        "1".repeat(16)
    );
    assert_eq!(bits, want);
    assert!(e
        .iter()
        .all(|x| (x.s - 0.032).abs() < 1e-12 && x.hz == 1000.0));
}

/// Worked example 2 with the designer's sign: QPSK31 "e" after the preamble. e is "11" then "00": the windows 00001, 00011,
/// 00110 and 01100 advance the phase by 1, 0, 2 and 1 quarter turns.
#[test]
fn psk_tx_qpsk31_worked_example() {
    let mut c = PskCoder::new(Variant::Qpsk31, 1000.0);
    let e = burst(&mut c, "e");
    let turns: Vec<u32> = e[1..].iter().map(quarter_turns).collect();
    assert!(turns[..32].iter().all(|&t| t == 2), "the preamble reverses");
    assert_eq!(&turns[32..36], &[1, 0, 2, 1]);
    // The postamble of ones after e and its two zero bits: the windows 11001 10011 00111 01111, then 11111 (no change).
    assert_eq!(c.postamble(), 56);
    let tail = &turns[36..turns.len() - 1];
    assert_eq!(&tail[..5], &[0, 3, 1, 3, 0]);
    assert!(tail[5..].iter().all(|&t| t == 0));
}

const SENT: &str = "CQ CQ DE N0CALL N0CALL PSE K\n";

/// The settings of a variant for the sender and the receiver.
fn settings(v: Variant) -> Vec<(&'static str, Value)> {
    vec![("modulation", Value::Choice(v.index()))]
}

/// The text of the receiver's signal within 20 Hz of 1000 Hz.
fn signal_text(r: &Run) -> String {
    signal(r).1
}

/// The offset and the text of the receiver's signal within 20 Hz of 1000 Hz (1000 Hz and no text when there is none).
fn signal(r: &Run) -> (f64, String) {
    rows(r, "signals")
        .iter()
        .find(|s| (cell_num(s, "offset_hz") - 1000.0).abs() < 20.0)
        .map(|s| (cell_num(s, "offset_hz"), cell_text(s, "text")))
        .unwrap_or((1000.0, String::new()))
}

/// The burst of `v` read by the stack's own receiver at `snr` dB in the baud bandwidth: the text sent, the offset of the
/// signal 2 s after the burst and the text read. The text is sent once per 31.25 baud of the variant, so that every burst
/// lasts about as long.
fn read_back(v: Variant, snr: f64, seed: u64) -> (String, f64, String) {
    let set = settings(v);
    let sent = SENT.repeat((v.baud() / 31.25) as usize);
    let x = one_burst(&hf::PSK, &set, &sent);
    let snr = snr - 10.0 * (2500.0 / v.baud()).log10();
    let r = feed(
        &harness(&hf::PSK).with_settings(&cases(&set)),
        &noisy(&x, snr, 2.0, seed),
    );
    let (offset, text) = signal(&r);
    (sent, offset, text)
}

/// The character errors of `got` against `want`: the fewest insertions, deletions and substitutions that turn `want` into a
/// prefix of `got` (the characters after the text, read from the noise after the burst, are not counted).
fn char_errors(want: &str, got: &str) -> usize {
    let (w, g): (Vec<char>, Vec<char>) = (want.chars().collect(), got.chars().collect());
    let mut row: Vec<usize> = (0..=g.len()).collect();
    for (i, &a) in w.iter().enumerate() {
        let mut next = vec![i + 1; g.len() + 1];
        for (j, &b) in g.iter().enumerate() {
            next[j + 1] = (row[j] + usize::from(a != b))
                .min(row[j + 1] + 1)
                .min(next[j] + 1);
        }
        row = next;
    }
    row.into_iter().min().unwrap_or(w.len())
}

/// Every variant at the specification's 12 dB in the baud bandwidth (the noise of 2.5 kHz scaled to the symbol rate), over
/// five noise seeds each: the stack's own receiver reads the whole text of every burst, BPSK31 to BPSK250 and QPSK31 with
/// nothing before it. QPSK63 and QPSK125 may lose or add a character at the start of a burst: they read one copy of the
/// text whole. The character error rate of each
/// variant and the largest distance of a signal from 1000 Hz after its burst are printed.
#[test]
fn psk_tx_loop() {
    for (v, exact) in [
        (Variant::Bpsk31, true),
        (Variant::Bpsk63, true),
        (Variant::Bpsk125, true),
        (Variant::Bpsk250, true),
        (Variant::Qpsk31, true),
        (Variant::Qpsk63, false),
        (Variant::Qpsk125, false),
    ] {
        let (mut errors, mut chars, mut off) = (0, 0, 0.0f64);
        for seed in 1..=5 {
            let (sent, offset, got) = read_back(v, 12.0, seed);
            errors += char_errors(&sent, &got);
            chars += sent.len();
            off = off.max((offset - 1000.0).abs());
            let whole = if exact {
                got.starts_with(&sent)
            } else {
                got.contains(SENT)
            };
            assert!(whole, "{} seed {seed}: {got:?}", v.name());
        }
        eprintln!(
            "{}: {errors} character errors in {chars}, {:.2} %; {off:.1} Hz from 1000 Hz at most",
            v.name(),
            100.0 * errors as f64 / chars as f64
        );
    }
}

/// Without noise the receiver prints the text sent and nothing else: no character before it from the rise of the carrier,
/// none after it from the steady carrier at the end.
#[test]
fn psk_tx_loop_clean() {
    for v in [Variant::Bpsk31, Variant::Bpsk125, Variant::Qpsk31] {
        let set = settings(v);
        let x = one_burst(&hf::PSK, &set, SENT);
        let r = feed(
            &harness(&hf::PSK).with_settings(&cases(&set)),
            &padded(&x, 2.0),
        );
        let t = signal_text(&r);
        assert_eq!(t, SENT, "{}", v.name());
    }
}

/// The pulse of BPSK31: between the boundaries of a symbol the phase of the envelope keeps (bit 1) or turns by 180 degrees
/// (bit 0) within 2 degrees; at mid symbol of a reversal the magnitude is under 0.02 of the peak. The level is the peak.
#[test]
fn psk_tx_bpsk31_pulse() {
    let x = one_burst(&hf::PSK, &[], "CQ");
    check_frame(&x, 0.5);
    let mut c = PskCoder::new(Variant::Bpsk31, 1000.0);
    let e = burst(&mut c, "CQ");
    let lead = (0.15 * RATE) as usize;
    let sym = RATE / 31.25;
    let at = |k: usize| lead + (k as f64 * sym).round() as usize;
    let base = |n: usize| {
        let p = -TAU * 1000.0 * (n - lead) as f64 / RATE;
        x[n] * Complex32::new(p.cos() as f32, p.sin() as f32)
    };
    for k in 1..e.len() - 1 {
        let (a, b) = (base(at(k)), base(at(k + 1)));
        let d = f64::from((b * a.conj()).arg()).to_degrees().abs();
        if quarter_turns(&e[k]) == 0 {
            assert!(d < 2.0, "symbol {k}: {d}");
        } else {
            assert!((d - 180.0).abs() < 2.0, "symbol {k}: {d}");
            let mid = x[(at(k) + at(k + 1)) / 2].norm();
            assert!(mid < 0.02 * 0.5, "symbol {k}: {mid} at mid symbol");
        }
    }
}

/// A text of letters, digits and spaces from a fixed seed.
fn random_text(n: usize) -> String {
    let set = b"ETAOINSHRDLUCMFWYPVBGKJQXZ0123456789     ";
    let mut s = 12345u64;
    (0..n)
        .map(|_| {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            set[((s >> 33) % set.len() as u64) as usize] as char
        })
        .collect()
}

/// The 99 % power bandwidth of a long text, within 10 % of the figures of the specification: BPSK31 and QPSK31 43 Hz, BPSK63
/// 86 Hz, BPSK250 352 Hz.
#[test]
fn psk_tx_bandwidth() {
    for (v, want, n) in [
        (Variant::Bpsk31, 43.0, 120),
        (Variant::Qpsk31, 43.0, 120),
        (Variant::Bpsk63, 86.0, 240),
        (Variant::Bpsk250, 352.0, 600),
    ] {
        let x = one_burst(&hf::PSK, &settings(v), &random_text(n));
        let (lo, hi) = occupied(&x);
        let bw = hi - lo;
        eprintln!("{}: {bw:.1} Hz", v.name());
        assert!((bw - want).abs() <= 0.1 * want, "{}: {bw} Hz", v.name());
    }
}

#[test]
fn psk_tx_stop_gate_and_tune() {
    check_stop(&hf::PSK, &[], &"CQ ".repeat(20), 2.0);
    check_gate(&hf::PSK, &[], "CQ CQ");
    check_tune(&hf::PSK, &[], 1000.0);
}

/// Bytes 128 and above refuse the text and are named.
#[test]
fn psk_tx_refuses_a_character() {
    let (mut rx, _) = station(&hf::PSK, &[], None);
    let e = send(rx.as_mut(), "CQ é").unwrap_err();
    assert!(e.to_string().contains("'é'"), "{e}");
    send(rx.as_mut(), "cq de t3st\r\n").unwrap();
}

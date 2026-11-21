//! The DominoEX and THOR senders: the tones of the specification's worked examples, the code of the MFSK description, every
//! variant read back by the stack's own receiver, the weak-signal cases, the lower sideband, the occupied bandwidth, the
//! continuous phase, the frame of a burst, `stop`, `tune`, the gate, the K = 15 variants refused and the characters.

use crate::common::*;
use crate::tx_common::*;
use hf::domino::{Family, IfkCoder, VARIANTS};
use hf::sender::{burst, Element};
use sdrkit_core::{Complex32, Value};
use sdrkit_stack_hfkbd as hf;
use std::f64::consts::TAU;

/// The tone numbers (0 to 17) of the elements of a burst around 1500 Hz with `spacing`.
fn tones(e: &[Element], spacing: f64) -> Vec<u32> {
    e.iter()
        .map(|x| {
            let k = (x.hz - 1500.0) / spacing + 8.5;
            assert!((k - k.round()).abs() < 1e-9, "{}", x.hz);
            k.round() as u32
        })
        .collect()
}

/// Worked example 1 of DominoEX: "CQ" after NUL CR STX CR gives the tones 3, 2, 13, 17, 14, 17, 16, 11, 15, 12, 17, 13, 3, 2;
/// then 16 spaces, two tones up each. Worked example 2: "e t a" steps by 3, 2, 4, 2 and 6 tones. `<` is the nibbles 0 10 8.
#[test]
fn dominoex_tx_worked_examples() {
    let v = Family::DominoEx.variant("11").unwrap();
    let mut c = IfkCoder::new(Family::DominoEx, v, 1500.0);
    let e = burst(&mut c, "CQ");
    let t = tones(&e, v.spacing());
    assert_eq!(
        &t[..14],
        &[3, 2, 13, 17, 14, 17, 16, 11, 15, 12, 17, 13, 3, 2]
    );
    assert_eq!(t.len(), 14 + 16);
    for k in 14..t.len() {
        assert_eq!(t[k], (t[k - 1] + 2) % 18);
    }
    assert!(e.iter().all(|x| (x.s - 1024.0 / 11025.0).abs() < 1e-12));
    let e = burst(&mut c, "e t a");
    let t = tones(&e, v.spacing());
    let steps: Vec<u32> = (10..15).map(|k| (t[k] + 18 - t[k - 1]) % 18).collect();
    assert_eq!(steps, [3, 2, 4, 2, 6]);
    assert_eq!(hf::domino::nibbles(b'<'), [0, 10, 8]);
}

/// Worked example 1 of THOR 16: 16 zero symbols, then NUL CR STX CR C Q through the code and the interleaver: the tones 2, 4,
/// ..., 16, 0, 2, ..., 14, then 6, 8, 0, 2, 12, 4, 6, 8, 0, 10, 16, 12, 8, 4. Worked example 2: the K = 7 code of "e" (1000)
/// from an empty register is 11 01 11 11.
#[test]
fn thor_tx_worked_examples() {
    let v = Family::Thor.variant("16").unwrap();
    let mut c = IfkCoder::new(Family::Thor, v, 1500.0);
    let e = burst(&mut c, "CQ");
    let t = tones(&e, v.spacing());
    let start: Vec<u32> = (1..=16).map(|k| (2 * k) % 18).collect();
    assert_eq!(&t[..16], &start[..]);
    assert_eq!(&t[16..30], &[6, 8, 0, 2, 12, 4, 6, 8, 0, 10, 16, 12, 8, 4]);
    let mut enc = sdrkit_algo_fec::conv::Encoder::new(hf::fec::MFSK_K7);
    let pairs: Vec<(u8, u8)> = [1u8, 0, 0, 0]
        .iter()
        .map(|&b| {
            let p = enc.push_bit(b);
            (p & 1, (p >> 1) & 1)
        })
        .collect();
    assert_eq!(pairs, [(1, 1), (0, 1), (1, 1), (1, 1)]);
    assert_eq!(c.tail_spaces(), 56);
}

/// The settings of a variant (an index of the family's variants) and an offset of the receiver's centre.
fn settings(variant: usize) -> Vec<(&'static str, Value)> {
    vec![("variant", Value::Choice(variant as u16))]
}

/// `x` moved by `hz`.
fn moved(x: &[Complex32], hz: f64) -> Vec<Complex32> {
    x.iter()
        .enumerate()
        .map(|(n, z)| {
            let p = TAU * hz * n as f64 / RATE;
            z * Complex32::new(p.cos() as f32, p.sin() as f32)
        })
        .collect()
}

const SENT: &str = "CQ DE N0CALL N0CALL K\n";
const SHORT: &str = "CQ DE N0CALL\n";

/// The burst of `desc` with `set` at `snr` dB in 2.5 kHz, moved by `offset` Hz, read by the stack's receiver.
fn read_back(
    desc: &'static sdrkit_core::StackDescriptor,
    set: &[(&'static str, Value)],
    text: &str,
    snr: f64,
    offset: f64,
    seed: u64,
) -> String {
    let x = one_burst(desc, set, text);
    let r = feed(
        &harness(desc).with_settings(&cases(set)),
        &noisy(&moved(&x, offset), snr, 3.0, seed),
    );
    crate::common::text(&r)
}

/// Every DominoEX variant at 30 dB: the whole text; DominoEX 11 at 0 dB, 100 Hz off either way. At 0 dB the receiver's signal
/// finder needs about 6 s to move 100 Hz, so that burst sends the line three times and the later lines are read whole.
#[test]
fn dominoex_tx_loop() {
    for (i, v) in Family::DominoEx.variants().iter().enumerate() {
        let text = if v.baud() < 6.0 { SHORT } else { SENT };
        let t = read_back(&hf::DOMINOEX, &settings(i), text, 30.0, 0.0, 90 + i as u64);
        assert!(t.contains(text), "DominoEX {}: {t:?}", v.tag);
    }
    for off in [100.0, -100.0] {
        let t = read_back(&hf::DOMINOEX, &settings(3), &SENT.repeat(3), 0.0, off, 97);
        assert!(t.contains(SENT), "DominoEX 11 at 0 dB, {off} Hz: {t:?}");
    }
}

/// Every THOR variant of the K = 7 code at 30 dB: the whole text; THOR 16 at 0 dB.
#[test]
fn thor_tx_loop() {
    for (i, v) in Family::Thor
        .variants()
        .iter()
        .enumerate()
        .filter(|v| v.1.k == 7)
    {
        let text = if v.baud() < 6.0 { SHORT } else { SENT };
        let t = read_back(&hf::THOR, &settings(i), text, 30.0, 0.0, 100 + i as u64);
        assert!(t.contains(text), "THOR {}: {t:?}", v.tag);
    }
    let t = read_back(&hf::THOR, &settings(4), SENT, 0.0, 0.0, 107);
    assert!(t.contains(SENT), "THOR 16 at 0 dB: {t:?}");
}

/// Without noise the receivers, set to the centre of the sender with the signal finder off, print exactly the text sent, after
/// the two line ends of the start (NUL, CR, STX, CR) and before the spaces of the end. With the finder on, the finder of the
/// THOR receiver moves the centre in the middle of this short clean burst and a part of the line is lost.
#[test]
fn dominoex_and_thor_tx_loop_clean() {
    for desc in [&hf::DOMINOEX, &hf::THOR] {
        let set = settings(4);
        let x = one_burst(desc, &set, SENT);
        let mut rx_set = set.clone();
        rx_set.push(("find_signal", Value::Bool(false)));
        let r = feed(
            &harness(desc).with_settings(&cases(&rx_set)),
            &padded(&x, 3.0),
        );
        let t = crate::common::text(&r);
        assert_eq!(
            t.trim_start_matches('\n').trim_end_matches(' '),
            SENT,
            "{}: {t:?}",
            desc.id
        );
    }
}

/// With LSB on both sides (the sideband the DominoEX description gives below 9 MHz) the text goes through; the tones are the
/// mirror image.
#[test]
fn dominoex_tx_lower_sideband() {
    let mut set = settings(3);
    set.push(("demod", Value::Choice(hf::front::Demod::Lsb.index())));
    let x = one_burst(&hf::DOMINOEX, &set, SENT);
    let (a, b) = signal_span(&x);
    let f = (0.01 * RATE) as usize;
    assert!(steps(&x[a + f..b - f]).iter().all(|v| *v < -1300.0));
    let r = feed(
        &harness(&hf::DOMINOEX).with_settings(&cases(&set)),
        &padded(&x, 3.0),
    );
    assert!(crate::common::text(&r).contains(SENT));
}

/// The phase is continuous: every phase step is one of the 18 tones; the 99 % power bandwidth of a long text is within 10 % of
/// the specification's figures, DominoEX 11 196 Hz and THOR 16 281 Hz.
#[test]
fn dominoex_and_thor_tx_bandwidth_and_phase() {
    let text = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789 ".repeat(2);
    for (desc, i, want) in [(&hf::DOMINOEX, 3, 196.0), (&hf::THOR, 4, 281.0)] {
        let v = VARIANTS[i];
        let x = one_burst(desc, &settings(i), &text);
        let (lo, hi) = occupied(&x);
        let bw = hi - lo;
        eprintln!("{}: {bw:.1} Hz", desc.id);
        assert!((bw - want).abs() <= 0.1 * want, "{}: {bw} Hz", desc.id);
        let (a, b) = signal_span(&x);
        let f = (0.01 * RATE) as usize;
        for s in steps(&x[a + f..b - f]) {
            let k = (s - 1500.0) / v.spacing() + 8.5;
            assert!(
                (k - k.round()).abs() < 1e-3 && (0.0..18.0).contains(&k.round()),
                "{s} Hz"
            );
        }
    }
}

#[test]
fn dominoex_and_thor_tx_stop_gate_and_tune() {
    for desc in [&hf::DOMINOEX, &hf::THOR] {
        check_stop(desc, &settings(4), &"CQ ".repeat(20), 3.0);
        check_gate(desc, &settings(4), "CQ CQ");
        check_tune(desc, &settings(4), 1500.0);
    }
}

/// THOR 25x4 and the other variants of the K = 15 code are not sent; a character above 255 refuses the text.
#[test]
fn thor_tx_refuses_the_long_code_and_a_character() {
    let (mut rx, sink) = station(&hf::THOR, &settings(6), None);
    let e = send(rx.as_mut(), "CQ").unwrap_err();
    assert!(e.to_string().contains("variant"), "{e}");
    let (mut rx2, _) = station(&hf::DOMINOEX, &settings(3), None);
    let e = send(rx2.as_mut(), "CQ €").unwrap_err();
    assert!(e.to_string().contains("'€'"), "{e}");
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
}

//! The RTTY sender: the bits of the specification's worked examples, read back by the stack's own receiver at every speed and
//! shift in both polarities, the occupied bandwidth, the continuous phase, the frame of a burst, `stop`, `tune`, the gate and
//! the characters refused.

use crate::common::*;
use crate::tx_common::*;
use hf::sender::{burst, Element, TxError};
use sdrkit_core::Value;
use sdrkit_stack_hfkbd as hf;

/// The bits of the elements of a burst, 1 for the mark: each bit time as one character, 20 and 10 bit times of mark as `M`
/// and `m`, 1.5 stop bits as `S`.
fn bit_string(e: &[Element], mark: f64, baud: f64) -> String {
    e.iter()
        .map(|x| {
            let bits = x.s * baud;
            let one = (x.hz - mark).abs() < 1e-9;
            match ((bits * 2.0).round() as u32, one) {
                (2, true) => '1',
                (2, false) => '0',
                (3, true) => 'S',
                (40, true) => 'M',
                (20, true) => 'm',
                b => panic!("an element of {bits} bits: {b:?}"),
            }
        })
        .collect()
}

/// A frame of five data bits written in the order sent.
fn frame(bits: &str) -> String {
    format!("0{bits}S")
}

/// Worked examples 1 and 2: "CQ" and "73" at the defaults. The mark is the upper tone (2295 Hz): the higher radio frequency.
#[test]
fn rtty_tx_worked_examples() {
    let m = hf::rtty::RttyModem::default();
    assert_eq!(m.tones(), (2295.0, 2125.0));
    let mut c = hf::rtty::RttyCoder::new(45.45, 2295.0, 2125.0);
    let e = burst(&mut c, "CQ");
    let want = format!(
        "M{}{}{}{}m",
        frame("11111"),
        frame("01110"),
        frame("11101"),
        frame("11111")
    );
    assert_eq!(bit_string(&e, 2295.0, 45.45), want);
    let e = burst(&mut c, "73");
    let want = format!(
        "M{}{}{}{}{}m",
        frame("11111"),
        frame("11011"),
        frame("11100"),
        frame("10000"),
        frame("11111")
    );
    assert_eq!(bit_string(&e, 2295.0, 45.45), want);
    let s: f64 = e.iter().map(|x| x.s).sum();
    assert!((s - 1.485).abs() < 1e-3, "{s} s");
    // The burst on air: 0.15 s, 1.485 s and 0.1 s.
    let x = one_burst(&hf::RTTY, &[], "73");
    let secs = x.len() as f64 / RATE;
    assert!(
        (secs - (0.25 + 67.5 / 45.45)).abs() < 1.5 / RATE,
        "{secs} s"
    );
    check_frame(&x, 0.5);
}

/// A line end is CR LF; figures after a space keep the figures case; the US figures case.
#[test]
fn rtty_tx_line_ends_and_figures() {
    let mut c = hf::rtty::RttyCoder::new(45.45, 2295.0, 2125.0);
    let e = burst(&mut c, "1 2\n$");
    let want = format!(
        "M{}{}{}{}{}{}{}{}{}m",
        frame("11111"),
        frame("11011"),
        frame("11101"),
        frame("00100"),
        frame("11001"),
        frame("00010"),
        frame("01000"),
        frame("10010"),
        frame("11111")
    );
    assert_eq!(bit_string(&e, 2295.0, 45.45), want);
}

const SENT: &str = "CQ CQ DE N0CALL N0CALL TEST 0123456789 $!&#\";' 73\n";

/// The settings of a polarity, speed and shift, for the sender and the receiver.
fn settings(baud: &str, shift: &str, reverse: bool) -> Vec<(&'static str, Value)> {
    vec![
        ("auto", Value::Bool(false)),
        (
            "baud",
            Value::Choice(hf::rtty::Baud::from_key(baud).unwrap().index()),
        ),
        (
            "shift",
            Value::Choice(hf::rtty::Shift::from_key(shift).unwrap().index()),
        ),
        ("reverse", Value::Bool(reverse)),
    ]
}

/// The receiver of the stack with `set`.
fn receiver(set: &[(&'static str, Value)]) -> sdrkit_dev_test::Harness {
    harness(&hf::RTTY).with_settings(&cases(set))
}

/// The loop: the burst with noise at 10 dB in 2.5 kHz, read by the stack's own receiver with the same settings: the whole
/// text, at most 6 framing errors, the tones within 8 Hz. Every speed and every shift, both polarities.
#[test]
fn rtty_tx_loop_every_speed_and_shift() {
    for (k, (baud, shift, rev)) in [
        ("45.45", "170", false),
        ("45.45", "170", true),
        ("50", "200", false),
        ("75", "425", true),
        ("100", "450", false),
        ("75", "850", false),
    ]
    .into_iter()
    .enumerate()
    {
        let set = settings(baud, shift, rev);
        let x = one_burst(&hf::RTTY, &set, SENT);
        let r = feed(&receiver(&set), &noisy(&x, 10.0, 1.0, 40 + k as u64));
        let t = text(&r);
        assert!(t.contains(SENT), "{baud}/{shift}/{rev}: {t:?}");
        let fe = num(&r, "framing_errors");
        assert!(fe <= 6.0, "{baud}/{shift}/{rev}: {fe} framing errors");
        let half: f64 = shift.parse::<f64>().unwrap() / 2.0;
        let (mark, space) = if rev {
            (2210.0 - half, 2210.0 + half)
        } else {
            (2210.0 + half, 2210.0 - half)
        };
        assert!(
            (num(&r, "mark_hz") - mark).abs() < 8.0,
            "{baud}/{shift}/{rev}: mark"
        );
        assert!(
            (num(&r, "space_hz") - space).abs() < 8.0,
            "{baud}/{shift}/{rev}: space"
        );
    }
}

/// Without noise the receiver prints the text exactly, with no framing error; lower case goes out in upper case.
#[test]
fn rtty_tx_loop_clean() {
    let set = settings("45.45", "170", false);
    let x = one_burst(&hf::RTTY, &set, "cq de t3st k\n");
    let r = feed(&receiver(&set), &padded(&x, 2.0));
    assert_eq!(text(&r), "CQ DE T3ST K\n");
    assert_eq!(num(&r, "framing_errors"), 0.0);
}

/// The 99 % power bandwidth at 45.45 Bd and 170 Hz shift is 252 Hz within 10 %; the phase is continuous: every phase step is
/// one of the two tones and the magnitude is the level outside the fades.
#[test]
fn rtty_tx_bandwidth_and_phase() {
    let text = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789 ".repeat(3);
    let x = one_burst(&hf::RTTY, &[], &text);
    let (lo, hi) = occupied(&x);
    let bw = hi - lo;
    assert!((bw - 252.0).abs() < 25.2, "{bw} Hz");
    let (a, b) = signal_span(&x);
    let f = (0.01 * RATE) as usize;
    let inner = &x[a + f..b - f];
    for (n, v) in steps(inner).into_iter().enumerate() {
        assert!(
            (v - 2125.0).abs() < 0.01 || (v - 2295.0).abs() < 0.01,
            "a phase step of {v} Hz at {n}"
        );
    }
    assert!(inner.iter().all(|z| (z.norm() - 0.5).abs() < 1e-4));
}

/// With LSB the signal is the mirror image: the tones at -2295 and -2125 Hz.
#[test]
fn rtty_tx_lower_sideband() {
    let x = one_burst(
        &hf::RTTY,
        &[("demod", Value::Choice(hf::front::Demod::Lsb.index()))],
        "RY",
    );
    let (a, b) = signal_span(&x);
    let f = (0.01 * RATE) as usize;
    for v in steps(&x[a + f..b - f]) {
        assert!(
            (v + 2125.0).abs() < 0.01 || (v + 2295.0).abs() < 0.01,
            "{v}"
        );
    }
}

#[test]
fn rtty_tx_stop_gate_and_tune() {
    check_stop(&hf::RTTY, &[], &"RYRYRYRY ".repeat(10), 1.0);
    check_gate(&hf::RTTY, &[], "RYRY");
    check_tune(&hf::RTTY, &[], 2210.0);
}

/// A character that ITA2 cannot carry refuses the whole text and names the character; text sent during a burst joins it.
#[test]
fn rtty_tx_refuses_a_character_and_queues_text() {
    let (mut rx, sink) = station(&hf::RTTY, &[], None);
    let e = send(rx.as_mut(), "CQ €").unwrap_err();
    assert!(
        e.to_string().contains(&TxError::Character('€').to_string()),
        "{e}"
    );
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
    send(rx.as_mut(), "CQ CQ ").unwrap();
    idle(rx.as_mut(), 0.5);
    send(rx.as_mut(), "DE T3ST\n").unwrap();
    let f = fields(rx.as_ref());
    assert!(f.tx_sending && f.tx_waiting > 0, "{f:?}");
    until_quiet(rx.as_mut(), 30.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1, "one burst");
    let set = settings("45.45", "170", false);
    let r = feed(&receiver(&set), &padded(&b[0].samples, 2.0));
    assert_eq!(text(&r), "CQ CQ DE T3ST\n");
}

/// A signal that does not fit 200 to 3800 Hz above the dial is refused: 850 Hz shift around 300 Hz.
#[test]
fn rtty_tx_refuses_a_signal_outside_the_band() {
    let mut set = settings("45.45", "850", false);
    set.push(("tx_centre_hz", Value::Hz(300.0)));
    let (mut rx, sink) = station(&hf::RTTY, &set, None);
    let e = send(rx.as_mut(), "RY").unwrap_err();
    assert!(e.to_string().contains(&TxError::Band.to_string()), "{e}");
    assert!(command(rx.as_mut(), "tune", &[]).is_err());
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
}

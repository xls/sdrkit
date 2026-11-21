//! The FSQ and IFKP senders: the check of the callsign, the symbols and tones of the specification's worked examples, a
//! directed FSQ message and every speed read back by the stack's own receiver, IFKP read back and not read by FSQ, the
//! occupied bandwidth, the continuous phase, the frame of a burst, `stop`, `tune`, the gate and the characters refused.

use crate::common::*;
use crate::tx_common::*;
use hf::fsq::{callsign_check, frame, FsqCoder, Kind, Trigger};
use hf::sender::{burst, Element};
use sdrkit_core::{Station, Value};
use sdrkit_stack_hfkbd as hf;

/// The tones (0 to 32) of the elements of a burst around 1500 Hz.
fn tones(e: &[Element], kind: Kind) -> Vec<u32> {
    e.iter()
        .map(|x| {
            let k = (x.hz - 1500.0) / kind.spacing() + 16.0;
            assert!((k - k.round()).abs() < 1e-9, "{}", x.hz);
            k.round() as u32
        })
        .collect()
}

/// The symbols that a sequence of tones carries: s = tone - previous - 1, mod 33.
fn symbols(t: &[u32]) -> Vec<u32> {
    t.windows(2).map(|w| (w[1] + 66 - w[0] - 1) % 33).collect()
}

/// The check of the specification: CRC-8 with the polynomial 0x07, initial 0.
#[test]
fn fsq_tx_callsign_check() {
    assert_eq!(callsign_check(b"123456789"), 0xf4);
    assert_eq!(callsign_check(b"k0tst"), 0xa0);
    assert_eq!(callsign_check(b"K0TST"), 0xc9);
}

/// Worked example 1: the directed line from k0tst to n0xyz with the trigger "?" and the text "hi". Worked example 2: the plain
/// line, its idle end and the idle tail rule.
#[test]
fn fsq_tx_worked_examples() {
    let line = frame("k0tst", Some(("n0xyz", '?')), "hi");
    assert_eq!(line, " \nk0tst:a0n0xyz? hi  \u{8}  ");
    let mut c = FsqCoder::new(Kind::Fsq, 1500.0, 4096);
    let t = tones(&burst(&mut c, &line), Kind::Fsq);
    assert_eq!(
        &symbols(&t)[..24],
        &[
            0, 28, 11, 10, 30, 20, 19, 20, 24, 30, 1, 10, 30, 14, 10, 30, 24, 25, 26, 28, 29, 0, 8,
            9
        ]
    );
    assert_eq!(
        &t[..25],
        &[
            5, 6, 2, 14, 25, 23, 11, 31, 19, 11, 9, 11, 22, 20, 2, 13, 11, 3, 29, 23, 19, 16, 17,
            26, 3
        ]
    );
    let line = frame("k0tst", None, "hi");
    assert_eq!(line, " \nk0tst:a0hi\n ");
    let t = tones(&burst(&mut c, &line), Kind::Fsq);
    assert_eq!(
        symbols(&t),
        [0, 28, 11, 10, 30, 20, 19, 20, 24, 30, 1, 10, 30, 8, 9, 28, 0, 28, 30, 28, 30]
    );
    assert_eq!(
        t,
        [5, 6, 2, 14, 25, 23, 11, 31, 19, 11, 9, 11, 22, 20, 29, 6, 2, 3, 32, 30, 26, 24]
    );
    // No tone repeats.
    assert!(t.windows(2).all(|w| w[0] != w[1]));
}

/// IFKP worked examples: "cq de k0tst" and a line end, then the idle 0 0; "K0TST" in upper case takes ten symbols.
#[test]
fn ifkp_tx_worked_examples() {
    let mut c = FsqCoder::new(Kind::Ifkp, 1500.0, 4096);
    let t = tones(&burst(&mut c, "cq de k0tst\n"), Kind::Ifkp);
    assert_eq!(
        symbols(&t),
        [3, 17, 28, 4, 5, 28, 11, 10, 30, 20, 19, 20, 28, 30, 0, 0]
    );
    assert_eq!(
        t,
        [5, 9, 27, 23, 28, 1, 30, 9, 20, 18, 6, 26, 14, 10, 8, 9, 10]
    );
    let t = tones(&burst(&mut c, "K0TST"), Kind::Ifkp);
    assert_eq!(
        &symbols(&t)[..10],
        &[11, 29, 10, 30, 20, 29, 19, 29, 20, 29]
    );
}

/// The receiver with the station callsign `call`.
fn receiver(desc: &'static sdrkit_core::StackDescriptor, call: &str) -> sdrkit_dev_test::Harness {
    harness(desc).station(Station {
        callsign: call.into(),
        ..Station::default()
    })
}

/// FSQ 3 at 10 dB: a directed message from the station to t3st asking for the report is parsed by the stack's own receiver,
/// at t3st: from N0CALL, the check good, to T3ST, the trigger, and to me.
#[test]
fn fsq_tx_loop_directed() {
    let (mut rx, sink) = station(&hf::FSQ, &[], None);
    command(
        rx.as_mut(),
        "send",
        &[
            ("text", Value::Text("snr please".into())),
            ("to", Value::Text("T3ST".into())),
            ("trigger", Value::Choice(Trigger::Report.index())),
        ],
    )
    .unwrap();
    until_quiet(rx.as_mut(), 120.0);
    let x = sink.bursts().remove(0).samples;
    let r = feed(&receiver(&hf::FSQ, "T3ST"), &noisy(&x, 10.0, 3.0, 120));
    let rows = rows(&r, "messages");
    let m = rows
        .first()
        .unwrap_or_else(|| panic!("a message: {:?}", text(&r)));
    assert_eq!(cell_text(m, "from"), "N0CALL");
    assert_eq!(cell_flag(m, "crc_ok"), Some(true));
    assert_eq!(cell_text(m, "to"), "T3ST");
    assert_eq!(cell_text(m, "trigger"), "Asks for the signal report");
    assert_eq!(cell_text(m, "text"), "snr please");
    assert_eq!(cell_flag(m, "to_me"), Some(true));
}

/// The FSQ speeds, choices of `speed` after auto.
const FSQ_SPEEDS: [&str; 5] = ["1.5", "2", "3", "4.5", "6"];

/// Every FSQ speed at 10 dB: the whole line to no one, from the station.
#[test]
fn fsq_tx_loop_every_speed() {
    for (i, sp) in FSQ_SPEEDS.iter().enumerate() {
        let set = vec![("speed", Value::Choice(i as u16 + 1))];
        let (mut rx, sink) = station(&hf::FSQ, &set, None);
        command(
            rx.as_mut(),
            "send",
            &[("text", Value::Text("cq cq de n0call k".into()))],
        )
        .unwrap();
        until_quiet(rx.as_mut(), 200.0);
        let x = sink.bursts().remove(0).samples;
        let r = feed(
            &receiver(&hf::FSQ, ""),
            &noisy(&x, 10.0, 3.0, 130 + i as u64),
        );
        let t = text(&r);
        assert!(
            t.contains("n0call:69cq cq de n0call k\n"),
            "FSQ {sp}: {t:?}"
        );
    }
}

/// IFKP 1.0 at 10 dB: the text and the speed of 3.906 baud; the FSQ stack reads nothing of an IFKP burst.
#[test]
fn ifkp_tx_loop() {
    let sent = "cq cq de n0call n0call k\n";
    let x = one_burst(&hf::IFKP, &[], sent);
    let r = feed(&receiver(&hf::IFKP, ""), &noisy(&x, 10.0, 3.0, 140));
    assert!(text(&r).contains(sent), "{:?}", text(&r));
    let baud = num(&r, "symbol_rate_hz");
    assert!((baud - 3.906).abs() < 0.01, "{baud}");
    let r = feed(&receiver(&hf::FSQ, ""), &noisy(&x, 10.0, 3.0, 141));
    assert_eq!(text(&r).trim(), "", "FSQ read an IFKP burst");
}

/// Without noise the receivers print the line exactly; the IFKP speeds 0.5 and 2.0 too. At 0.5 baud the receiver loses the
/// first character: an IFKP burst starts with the text after one tone, and the receiver finds the slowest speed only later.
#[test]
fn fsq_and_ifkp_tx_loop_clean() {
    let sent = "the quick brown fox 0123456789\n";
    for (desc, speed) in [(&hf::IFKP, 1u16), (&hf::IFKP, 3), (&hf::FSQ, 5)] {
        let set = vec![("speed", Value::Choice(speed))];
        let (mut rx, sink) = station(desc, &set, None);
        send(rx.as_mut(), sent).unwrap();
        until_quiet(rx.as_mut(), 200.0);
        let x = sink.bursts().remove(0).samples;
        let r = feed(&receiver(desc, ""), &padded(&x, 3.0));
        let want = if desc.id == "fsq" {
            format!(" \nn0call:69{sent}\n ")
        } else {
            sent.to_string()
        };
        let t = text(&r);
        let want = if speed == 1 && desc.id == "ifkp" {
            &want[1..]
        } else {
            want.trim()
        };
        assert!(t.contains(want), "{} {speed}: {t:?}", desc.id);
    }
}

/// The 99 % power bandwidth of FSQ 3 is within 10 % of 287 Hz and that of IFKP 1.0 within 10 % of 387 Hz (33 spacings); every
/// phase step is one of the 33 tones.
#[test]
fn fsq_and_ifkp_tx_bandwidth_and_phase() {
    let text = "the quick brown fox jumps over the lazy dog 0123456789 ".repeat(2);
    for (desc, kind, want) in [(&hf::FSQ, Kind::Fsq, 287.0), (&hf::IFKP, Kind::Ifkp, 387.0)] {
        let x = one_burst(desc, &[], &text);
        let (lo, hi) = occupied(&x);
        let bw = hi - lo;
        eprintln!("{}: {bw:.1} Hz", desc.id);
        assert!((bw - want).abs() <= 0.1 * want, "{}: {bw} Hz", desc.id);
        let (a, b) = signal_span(&x);
        let f = (0.01 * RATE) as usize;
        for s in steps(&x[a + f..b - f]) {
            let k = (s - 1500.0) / kind.spacing() + 16.0;
            assert!(
                (k - k.round()).abs() < 1e-3 && (0.0..33.0).contains(&k.round()),
                "{s} Hz"
            );
        }
    }
}

#[test]
fn fsq_and_ifkp_tx_stop_gate_and_tune() {
    for desc in [&hf::FSQ, &hf::IFKP] {
        check_stop(desc, &[], &"cq ".repeat(20), 3.0);
        check_gate(desc, &[], "cq cq");
        check_tune(desc, &[], 1500.0);
    }
}

/// A character without a code refuses the text; FSQ without a station callsign refuses to send.
#[test]
fn fsq_and_ifkp_tx_refuse() {
    let (mut rx, _) = station(&hf::IFKP, &[], None);
    let e = send(rx.as_mut(), "cq €").unwrap_err();
    assert!(e.to_string().contains("'€'"), "{e}");
    let (mut rx, sink) = station(&hf::FSQ, &[], None);
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<sdrkit_graph::Graph>())
        .unwrap();
    let mut ctx = g.context().clone();
    ctx.station = std::sync::Arc::new(Station::default());
    g.set_context(ctx);
    let e = send(rx.as_mut(), "cq").unwrap_err();
    assert!(e.to_string().contains("callsign"), "{e}");
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
}

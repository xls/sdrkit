//! The Olivia and Contestia senders: the symbols and tones of the specification's worked examples, the chip order, the tone
//! grid, every pair of tones and bandwidth read back by the stack's own receiver, the weak-signal cases over a fixed set of
//! seeds, the occupied bandwidth, the frame of a burst, `stop`, `tune`, the gate and the characters.

use crate::common::*;
use crate::tx_common::*;
use hf::olivia::{block_tones, Geometry, Kind, TxGrid};
use sdrkit_core::{StackDescriptor, Value};
use sdrkit_stack_hfkbd as hf;
use sdrkit_util_bits::gray::ungray;

/// The first 16 tones of a block of `chars`.
fn tones16(kind: Kind, chars: &[u8]) -> Vec<u32> {
    let mut t = Vec::new();
    block_tones(kind, chars, |x| t.push(x));
    t.truncate(16);
    t
}

/// Worked examples 1 and 2 of Olivia (8/250 "CQ" and 4/125 "K", first blocks with NUL first), the worked example of Contestia
/// (4/125, a block holding C and Q), the Walsh chip order and the Contestia values of "cq de k0tst".
#[test]
fn olivia_tx_worked_examples() {
    let t = tones16(Kind::Olivia, b"\0CQ");
    let s: Vec<u32> = t.iter().map(|&x| ungray(x)).collect();
    assert_eq!(s, [4, 5, 6, 5, 5, 5, 5, 2, 3, 0, 2, 0, 3, 3, 7, 6]);
    assert_eq!(t, [6, 7, 5, 7, 7, 7, 7, 3, 2, 0, 3, 0, 2, 2, 4, 5]);
    let t = tones16(Kind::Olivia, b"\0K");
    let s: Vec<u32> = t.iter().map(|&x| ungray(x)).collect();
    assert_eq!(s, [2, 0, 3, 3, 0, 2, 3, 3, 2, 0, 1, 0, 3, 2, 3, 1]);
    assert_eq!(t, [3, 0, 2, 2, 0, 3, 2, 2, 3, 0, 1, 0, 2, 3, 2, 1]);
    let t = tones16(Kind::Contestia, b"CQ");
    let s: Vec<u32> = t.iter().map(|&x| ungray(x)).collect();
    assert_eq!(s, [1, 0, 2, 3, 1, 2, 2, 2, 2, 2, 0, 2, 3, 0, 0, 1]);
    assert_eq!(t, [1, 0, 3, 2, 1, 3, 3, 3, 3, 3, 0, 3, 2, 0, 0, 1]);
    let chips =
        |v: u32| -> Vec<f32> { (0..8).map(|t| hf::util::walsh_chip(v, 7 - t, 3)).collect() };
    assert_eq!(chips(1), [-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0]);
    assert_eq!(chips(3), [1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0]);
    let v: Vec<u32> = b"cq de k0tst"
        .iter()
        .map(|&c| hf::olivia::char_value(Kind::Contestia, c))
        .collect();
    assert_eq!(v, [35, 49, 59, 36, 37, 59, 43, 16, 52, 51, 52]);
}

/// The lowest tone lies on the bin grid of the specification: Olivia 8/250 and 4/125 at 1500 Hz from bins 89 and 93, 32/1000
/// at 1500 Hz from 1015.625 Hz, 16/500 at 1200 Hz from 968.75 Hz; Contestia 16/500 at 1200 Hz from 953.125 Hz. At 2 tones and
/// 2000 Hz the tones are 1000 Hz apart, as the receiver expects.
#[test]
fn olivia_tx_tone_grid() {
    let grid = |k, tones, bw, c| TxGrid::new(k, Geometry::new(tones, bw), c);
    let g = grid(Kind::Olivia, 8, 250.0, 1500.0);
    assert_eq!((g.ns, g.b0, g.step), (512, 89, 2));
    assert_eq!((g.tone_hz(0), g.tone_hz(7)), (1390.625, 1609.375));
    let g = grid(Kind::Olivia, 4, 125.0, 1500.0);
    assert_eq!(g.b0, 93);
    assert_eq!(
        (0..4).map(|k| g.tone_hz(k)).collect::<Vec<_>>(),
        [1453.125, 1484.375, 1515.625, 1546.875]
    );
    assert_eq!(grid(Kind::Olivia, 32, 1000.0, 1500.0).tone_hz(0), 1015.625);
    assert_eq!(grid(Kind::Olivia, 32, 1000.0, 1500.0).tone_hz(31), 1984.375);
    assert_eq!(grid(Kind::Olivia, 16, 500.0, 1200.0).tone_hz(0), 968.75);
    assert_eq!(grid(Kind::Contestia, 16, 500.0, 1200.0).b0, 61);
    assert_eq!(grid(Kind::Contestia, 16, 500.0, 1200.0).tone_hz(0), 953.125);
    let g = grid(Kind::Olivia, 2, 2000.0, 1500.0);
    assert_eq!(g.tone_hz(1) - g.tone_hz(0), 1000.0);
}

/// The settings of a pair: indices of the `tones` and `bandwidth` choices.
fn settings(tones: usize, bw: usize) -> Vec<(&'static str, Value)> {
    vec![
        ("tones", Value::Choice(tones as u16)),
        ("bandwidth", Value::Choice(bw as u16)),
    ]
}

const SENT: &str = "CQ DE N0CALL K";

/// What a pair gave: the text read without noise (signal finder off) and at 30 dB in 2.5 kHz, the 99 % power bandwidth and the
/// level of the opposite sideband under the peak of the spectrum.
struct Pair {
    name: String,
    /// The tones' span plus three baud: the tones and 1.5 baud of keying sidebands each side, Hz.
    span: f64,
    baud: f64,
    clean: String,
    weak: String,
    occupied: f64,
    image: f64,
}

/// Every pair of `tones` (indices of the choices) and the five bandwidths of `desc`.
fn pairs(desc: &'static StackDescriptor, tones: &[usize]) -> Vec<Pair> {
    let mut out = Vec::new();
    for &ti in tones {
        for bi in 0..5 {
            let set = settings(ti, bi);
            let x = one_burst(desc, &set, SENT);
            let h = harness(desc).with_settings(&cases(&set));
            let mut off = set.clone();
            off.push(("find_signal", Value::Bool(false)));
            let h_off = harness(desc).with_settings(&cases(&off));
            let (lo, hi) = occupied(&x);
            let g = Geometry::new(2 << ti, f64::from(125u32 << bi));
            let baud = 8000.0 / (g.ns / 2) as f64;
            out.push(Pair {
                name: format!("{} {}/{}", desc.id, 2 << ti, 125 << bi),
                span: (g.tones - 1) as f64 * g.spacing() + 3.0 * baud,
                baud,
                clean: crate::common::text(&feed(&h_off, &followed(&x, 3.0))),
                weak: crate::common::text(&feed(
                    &h,
                    &noisy(&x, 30.0, 3.0, 500 + (ti * 5 + bi) as u64),
                )),
                occupied: hi - lo,
                image: psd_below_peak(&x, -3800.0, -200.0),
            });
        }
    }
    out
}

/// The pairs of `tones`: the text sent is read whole and unchanged without noise and at 30 dB; the 99 % power bandwidth is at
/// most the span of the tones and 1.5 baud each side; the opposite sideband is at least 60 dB down up to 31.25 baud and at
/// least 44 dB down above (the keying sidebands of the published window reach below the dial). The pairs where the receiver
/// also prints characters from the silence at the ends of the noise-free burst are listed.
fn loop_pairs(desc: &'static StackDescriptor, tones: &[usize]) -> Vec<String> {
    let mut bad = Vec::new();
    for p in pairs(desc, tones) {
        eprintln!(
            "{}: {:.1} Hz, opposite sideband {:.1} dB down{}",
            p.name,
            p.occupied,
            p.image,
            if p.clean == SENT {
                ""
            } else {
                "; characters beside the text without noise"
            }
        );
        if !p.clean.contains(SENT) {
            bad.push(format!("{} clean: {:?}", p.name, p.clean));
        }
        if !p.weak.contains(SENT) {
            bad.push(format!("{} 30 dB: {:?}", p.name, p.weak));
        }
        if p.occupied > p.span {
            bad.push(format!("{}: {:.1} Hz", p.name, p.occupied));
        }
        if p.image < if p.baud <= 31.25 { 60.0 } else { 44.0 } {
            bad.push(format!("{}: opposite sideband {:.1} dB", p.name, p.image));
        }
    }
    bad
}

/// Without noise the receiver prints exactly the text sent for every pair, and nothing beside it.
#[test]
fn olivia_and_contestia_tx_loop_exact() {
    let mut bad = Vec::new();
    for desc in [&hf::OLIVIA, &hf::CONTESTIA] {
        for p in pairs(desc, &[0, 1, 2, 3, 4, 5, 6, 7]) {
            if p.clean != SENT {
                bad.push(format!("{}: {:?}", p.name, p.clean));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn olivia_tx_loop_2_to_16_tones() {
    let bad = loop_pairs(&hf::OLIVIA, &[0, 1, 2, 3]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn olivia_tx_loop_32_and_64_tones() {
    let bad = loop_pairs(&hf::OLIVIA, &[4, 5]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn olivia_tx_loop_128_tones() {
    let bad = loop_pairs(&hf::OLIVIA, &[6]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn olivia_tx_loop_256_tones() {
    let bad = loop_pairs(&hf::OLIVIA, &[7]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn contestia_tx_loop_2_to_32_tones() {
    let bad = loop_pairs(&hf::CONTESTIA, &[0, 1, 2, 3, 4]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn contestia_tx_loop_64_to_256_tones() {
    let bad = loop_pairs(&hf::CONTESTIA, &[5, 6, 7]);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The pairs of `tones` (indices of the choices) and every bandwidth of both stacks, at the default settings, that print a
/// character from 60 s of white noise alone or from 5 s of digital silence.
fn chatter(tones: &[usize]) -> Vec<String> {
    let silence = vec![sdrkit_core::Complex32::default(); 5 * RATE as usize];
    let mut bad = Vec::new();
    for desc in [&hf::OLIVIA, &hf::CONTESTIA] {
        for &ti in tones {
            for bi in 0..5 {
                let h = harness(desc).with_settings(&cases(&settings(ti, bi)));
                let noise = noise_only(60.0, 700 + (ti * 5 + bi) as u64);
                for (what, x) in [("noise", &noise), ("silence", &silence)] {
                    let t = crate::common::text(&feed(&h, x));
                    if !t.is_empty() {
                        bad.push(format!(
                            "{} {}/{} {what}: {t:?}",
                            desc.id,
                            2 << ti,
                            125 << bi
                        ));
                    }
                }
            }
        }
    }
    bad
}

#[test]
fn olivia_and_contestia_quiet_2_and_4_tones() {
    let bad = chatter(&[0, 1]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn olivia_and_contestia_quiet_8_and_16_tones() {
    let bad = chatter(&[2, 3]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn olivia_and_contestia_quiet_32_and_64_tones() {
    let bad = chatter(&[4, 5]);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn olivia_and_contestia_quiet_128_and_256_tones() {
    let bad = chatter(&[6, 7]);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The share of the seeds 1 to 10 for which the receiver prints the whole text of `desc` with `set` at `snr` dB.
fn rate(desc: &'static StackDescriptor, set: &[(&'static str, Value)], snr: f64) -> usize {
    let x = one_burst(desc, set, SENT);
    let h = harness(desc).with_settings(&cases(set));
    (1..=10u64)
        .filter(|&seed| {
            crate::common::text(&feed(&h, &noisy(&x, snr, 3.0, 600 + seed))).contains(SENT)
        })
        .count()
}

/// The weak-signal targets of the receive specification over the seeds 1 to 10: Olivia 32/1000 at -8 dB in 2.5 kHz, Contestia
/// 16/500 at -4 dB.
#[test]
fn olivia_and_contestia_tx_weak_signal() {
    for (desc, set, name, snr, need) in [
        (&hf::OLIVIA, settings(4, 3), "Olivia 32/1000", -8.0, 9),
        (&hf::CONTESTIA, settings(3, 2), "Contestia 16/500", -4.0, 9),
    ] {
        let n = rate(desc, &set, snr);
        eprintln!("{name} at {snr} dB: {n} of 10");
        assert!(n >= need, "{name} at {snr} dB: {n} of 10");
    }
}

/// Contestia sends lower case as upper case and a character outside its alphabet as `?`; Olivia refuses a character above 255
/// and sends nothing.
#[test]
fn olivia_and_contestia_tx_characters() {
    let set = settings(3, 2);
    let x = one_burst(&hf::CONTESTIA, &set, "cq de n0call ~");
    let h = harness(&hf::CONTESTIA).with_settings(&cases(&set));
    assert_eq!(
        crate::common::text(&feed(&h, &followed(&x, 3.0))),
        "CQ DE N0CALL ?"
    );
    let (mut rx, sink) = station(&hf::OLIVIA, &settings(4, 3), None);
    let e = send(rx.as_mut(), "CQ \u{20ac}").unwrap_err();
    assert!(e.to_string().contains('\u{20ac}'), "{e}");
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
}

/// The frame of a burst with its peak at the level; the 99 % power bandwidth of Olivia 32/1000 within 10 % of the
/// specification's 986 Hz; the opposite sideband (-3800 to -200 Hz) at least 60 dB under the peak of the spectrum.
#[test]
fn olivia_and_contestia_tx_bandwidth_and_frame() {
    let text = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789 ";
    for (desc, want) in [(&hf::OLIVIA, Some(986.0)), (&hf::CONTESTIA, None)] {
        let x = one_burst(desc, &settings(4, 3), text);
        check_frame(&x, 0.5);
        let (lo, hi) = occupied(&x);
        let bw = hi - lo;
        let image = psd_below_peak(&x, -3800.0, -200.0);
        eprintln!(
            "{} 32/1000: {bw:.1} Hz, {lo:.1} to {hi:.1} Hz; opposite sideband {image:.1} dB down",
            desc.id
        );
        if let Some(w) = want {
            assert!((bw - w).abs() <= 0.1 * w, "{}: {bw} Hz", desc.id);
        }
        assert!(bw < 1000.0 * 1.1, "{}: {bw} Hz", desc.id);
        assert!(image >= 60.0, "{}: {image} dB", desc.id);
    }
}

#[test]
fn olivia_and_contestia_tx_stop_gate_and_tune() {
    for desc in [&hf::OLIVIA, &hf::CONTESTIA] {
        let set = settings(4, 3);
        check_stop(desc, &set, &"CQ ".repeat(20), 3.0);
        check_gate(desc, &set, "CQ CQ");
        check_tune(desc, &set, 1500.0);
    }
}

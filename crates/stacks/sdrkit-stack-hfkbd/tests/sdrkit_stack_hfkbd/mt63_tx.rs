//! The MT63 sender: the signs, Walsh words and carrier delays of the specification's worked examples, the burst's preamble and
//! tail, every width and interleave read back by the stack's own receiver, the weak-signal cases over a fixed set of seeds, the
//! carrier comb, the occupied bandwidth, the frame of a burst, `stop`, `tune`, the gate and the characters.

use crate::common::*;
use crate::tx_common::*;
use hf::mt63::{chips, delays, Mt63Coder, Mt63Signs};
use sdrkit_core::{Complex32, Value};
use sdrkit_stack_hfkbd as hf;
use std::f64::consts::TAU;

/// Worked example 1: NUL is every chip +1, so once the interleaver holds only NUL every carrier reverses at every symbol.
/// Worked example 2: "C" (67) is - + + - repeated from carrier 0. The delays of carriers 0 to 15 and the largest of each
/// interleave. The preamble and the tail of a burst.
#[test]
fn mt63_tx_worked_examples() {
    assert!(chips(0).iter().all(|&c| c == 1.0));
    let mut s = Mt63Signs::new(false);
    let mut prev = [1.0f32; 64];
    for _ in 0..40 {
        let now = *s.push(0);
        assert!(now.iter().zip(&prev).all(|(a, b)| *a == -*b));
        prev = now;
    }
    let c = chips(b'C');
    for (i, &x) in c.iter().enumerate() {
        let want = if i % 4 == 0 || i % 4 == 3 { -1.0 } else { 1.0 };
        assert_eq!(x, want, "carrier {i}");
    }
    let short = delays(false);
    assert_eq!(
        &short[..16],
        &[0, 4, 9, 15, 22, 26, 31, 5, 12, 16, 21, 27, 2, 6, 11, 17]
    );
    assert_eq!(*short.iter().max().unwrap(), 31);
    let long = delays(true);
    assert_eq!(
        &long[..16],
        &[0, 1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56]
    );
    assert_eq!(*long.iter().max().unwrap(), 63);
    let c = Mt63Coder::new(1000.0, false, 500.0);
    assert_eq!((c.preamble(), c.tail()), (55, 44));
    let c = Mt63Coder::new(500.0, true, 500.0);
    assert_eq!((c.preamble(), c.tail()), (80, 76));
    assert_eq!(Mt63Coder::new(2000.0, false, 500.0).preamble(), 110);
}

/// The settings of a width (index of the `bandwidth` choices: 1 for 500 Hz to 3 for 2000 Hz) and an interleave.
fn settings(width: usize, long: bool) -> Vec<(&'static str, Value)> {
    vec![
        ("bandwidth", Value::Choice(width as u16)),
        ("interleave", Value::Choice(if long { 2 } else { 1 })),
    ]
}

const SENT: &str = "CQ DE N0CALL N0CALL K\n";
const PAIRS: [(usize, bool); 6] = [
    (1, false),
    (1, true),
    (2, false),
    (2, true),
    (3, false),
    (3, true),
];

/// The name of a pair.
fn name(width: usize, long: bool) -> String {
    format!(
        "MT63-{} {}",
        250 << width,
        if long { "long" } else { "short" }
    )
}

/// Every width and interleave: without noise the receiver prints exactly the text sent, and nothing before it; at 30 dB in
/// 2.5 kHz, the whole text.
#[test]
fn mt63_tx_loop() {
    let mut bad = Vec::new();
    for (i, &(w, long)) in PAIRS.iter().enumerate() {
        let set = settings(w, long);
        let x = one_burst(&hf::MT63, &set, SENT);
        let h = harness(&hf::MT63).with_settings(&cases(&set));
        let clean = crate::common::text(&feed(&h, &followed(&x, 3.0)));
        let weak = crate::common::text(&feed(&h, &noisy(&x, 30.0, 3.0, 700 + i as u64)));
        if clean != SENT {
            bad.push(format!("{} clean: {clean:?}", name(w, long)));
        }
        if !weak.contains(SENT) {
            bad.push(format!("{} 30 dB: {weak:?}", name(w, long)));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The weak-signal targets of the receive specification over the seeds 1 to 10: MT63-1000 short at 4 dB in 2.5 kHz, long at
/// 2 dB, MT63-500 and MT63-2000 (both interleaves) at 8 dB.
#[test]
fn mt63_tx_weak_signal() {
    let mut bad = Vec::new();
    for (w, long, snr) in [
        (2, false, 4.0),
        (2, true, 2.0),
        (1, false, 8.0),
        (1, true, 8.0),
        (3, false, 8.0),
        (3, true, 8.0),
    ] {
        let set = settings(w, long);
        let x = one_burst(&hf::MT63, &set, SENT);
        let h = harness(&hf::MT63).with_settings(&cases(&set));
        let n = (1..=10u64)
            .filter(|&seed| {
                crate::common::text(&feed(&h, &noisy(&x, snr, 3.0, 800 + seed))).contains(SENT)
            })
            .count();
        eprintln!("{} at {snr} dB: {n} of 10", name(w, long));
        if n < 9 {
            bad.push(format!("{} at {snr} dB: {n} of 10", name(w, long)));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The magnitude of the spectrum of `x` at `hz`.
fn at(x: &[Complex32], hz: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (n, z) in x.iter().enumerate() {
        let p = -TAU * hz * n as f64 / RATE;
        let (s, c) = p.sin_cos();
        re += f64::from(z.re) * c - f64::from(z.im) * s;
        im += f64::from(z.re) * s + f64::from(z.im) * c;
    }
    re.hypot(im)
}

/// "@" (64) is every chip -1: no carrier changes sign, so a run of it shows the comb. Each of the 64 lines lies within 0.5 Hz
/// of the lowest carrier + i width / 64.
#[test]
fn mt63_tx_carrier_comb() {
    let set = settings(2, false);
    let x = one_burst(&hf::MT63, &set, &"@".repeat(32 + 60));
    // The steady part: 4 s from 5.5 s (the preamble) + 3.2 s (the interleave) + 0.15 s (the lead) on.
    let a = (9.0 * RATE) as usize;
    let seg = &x[a..a + (4.0 * RATE) as usize];
    for i in 0..64 {
        let f = 500.0 + i as f64 * 1000.0 / 64.0;
        let (best, _) = (-20..=20)
            .map(|k| {
                let h = f + k as f64 * 0.05;
                (h, at(seg, h))
            })
            .fold((f, 0.0), |b, c| if c.1 > b.1 { c } else { b });
        assert!((best - f).abs() <= 0.5, "carrier {i}: {best} Hz for {f} Hz");
    }
}

/// The frame of a burst with its peak at the level, the mean power 9 to 11 dB under the peak, the 99 % power bandwidth of each
/// width within 10 % of the width, and the opposite sideband (-3800 to -200 Hz) at least 60 dB under the peak of the spectrum.
#[test]
fn mt63_tx_bandwidth_and_frame() {
    let text = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789 ".repeat(2);
    for w in 1..=3 {
        let width = f64::from(250u32 << w);
        let x = one_burst(&hf::MT63, &settings(w, false), &text);
        check_frame(&x, 0.5);
        let (lo, hi) = occupied(&x);
        let bw = hi - lo;
        let image = psd_below_peak(&x, -3800.0, -200.0);
        let crest = 10.0 * (0.25 / signal_power(&x)).log10();
        eprintln!(
            "MT63-{width}: {bw:.1} Hz, {lo:.1} to {hi:.1} Hz; opposite sideband {image:.1} dB down; mean power {crest:.1} dB under the peak"
        );
        assert!((9.0..=11.0).contains(&crest), "MT63-{width}: {crest} dB");
        assert!((bw - width).abs() <= 0.1 * width, "MT63-{width}: {bw} Hz");
        assert!(image >= 60.0, "MT63-{width}: {image} dB");
    }
}

/// Stop, the gate and the tune carrier at the centre of the band (carrier 32: 1000 Hz for MT63-1000 from 500 Hz); a character
/// above 255 refuses the text.
#[test]
fn mt63_tx_stop_gate_tune_and_characters() {
    let set = settings(2, false);
    check_stop(&hf::MT63, &set, &"CQ ".repeat(20), 3.0);
    check_gate(&hf::MT63, &set, "CQ CQ");
    check_tune(&hf::MT63, &set, 1000.0);
    let (mut rx, sink) = station(&hf::MT63, &set, None);
    let e = send(rx.as_mut(), "CQ \u{20ac}").unwrap_err();
    assert!(e.to_string().contains('\u{20ac}'), "{e}");
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
}

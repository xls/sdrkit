//! The Hell sender: the columns of the specification's worked examples, the pixel timing and the keying edges, the black dot on
//! the higher tone of the FSK variants, the columns read back by the stack's own receiver, the occupied bandwidth, the frame of
//! a burst, `stop`, `tune` and the gate.

use crate::common::*;
use crate::tx_common::*;
use hf::hell::{tx_columns, HellCoder, VARIANTS, VARIANT_SETTING};
use hf::sender::{burst, Envelope};
use sdrkit_codec_image::font::ttf::TrueTypeFont;
use sdrkit_core::Value;
use sdrkit_stack_hfkbd as hf;

/// A column with ink on the pixels `px` (0 the bottom one).
fn col(px: &[u32]) -> u16 {
    px.iter().fold(0, |m, &p| m | 1 << p)
}

/// The pixels `a` to `b`.
fn run(a: u32, b: u32) -> Vec<u32> {
    (a..=b).collect()
}

/// Worked example 1: "C" with the shared glyph is a blank column, ink on pixels 4 to 9, three columns of ink on 2, 3, 10 and
/// 11, a blank column: six columns, 0.343 s in Feld Hell. Worked example 2: the start is three full stops (blank, ink on 2 and
/// 3, blank), then "T" is blank; 10, 11; 10, 11; 2 to 11; 10, 11; 10, 11; blank. A space is five blank columns.
#[test]
fn hell_tx_worked_examples() {
    let mut c = Vec::new();
    tx_columns('C', None, &mut c);
    let side = col(&[2, 3, 10, 11]);
    assert_eq!(c, [0, col(&run(4, 9)), side, side, side, 0]);
    let mut t = Vec::new();
    tx_columns('T', None, &mut t);
    let top = col(&[10, 11]);
    assert_eq!(t, [0, top, top, col(&run(2, 11)), top, top, 0]);
    let mut s = Vec::new();
    tx_columns(' ', None, &mut s);
    assert_eq!(s, [0; 5]);
    let v = VARIANTS[0];
    let mut coder = HellCoder::new(v, 1500.0);
    let e = burst(&mut coder, "C");
    // Three full stops (9 columns), "C" (6) and a space (5), 14 pixels each, a pixel 1 / 245 s.
    assert_eq!(e.len(), (9 + 6 + 5) * 14);
    let secs: f64 = e[9 * 14..15 * 14].iter().map(|x| x.s).sum();
    assert!((secs - 0.343).abs() < 0.001, "{secs}");
    let on: Vec<bool> = e[..3 * 14]
        .iter()
        .map(|x| !matches!(x.env, Envelope::Steady(a) if a.norm() == 0.0))
        .collect();
    let dot: Vec<bool> = (0..14).map(|p| p == 2 || p == 3).collect();
    assert_eq!(&on[..14], &[false; 14]);
    assert_eq!(&on[14..28], &dot[..]);
    assert_eq!(&on[28..42], &[false; 14]);
}

/// The settings of variant `name` with the receiver's signal finder off.
fn settings(name: &str) -> Vec<(&'static str, Value)> {
    let i = VARIANTS.iter().position(|v| v.name == name).unwrap();
    vec![
        ("variant", Value::Choice(i as u16)),
        ("find_signal", Value::Bool(false)),
    ]
}

/// Feld Hell: a run of ink rises from 10 % to 90 % in 2 to 3 ms. HellX5 rises within one sample.
#[test]
fn hell_tx_edges() {
    for (name, lo, hi) in [("Feld Hell", 2e-3, 3e-3), ("HellX5", 0.0, 1.0 / RATE)] {
        let x = one_burst(&hf::HELL, &settings(name), "I");
        let lead = (0.15 * RATE) as usize + (0.01 * RATE) as usize;
        let m: Vec<f32> = x[lead..].iter().map(|z| z.norm() / 0.5).collect();
        // The first rise after the fade: the first sample above 0.1 and the first above 0.9 after it.
        let a = m.iter().position(|&v| v > 0.1).unwrap();
        let b = a + m[a..].iter().position(|&v| v > 0.9).unwrap();
        let rise = (b - a) as f64 / RATE;
        assert!(rise >= lo && rise <= hi, "{name}: {rise} s");
    }
}

/// FSK Hell: the tone is always on, an inked pixel on the higher tone.
#[test]
fn hell_tx_fsk_ink_is_the_higher_tone() {
    for name in ["FSK Hell 245", "Hell 80"] {
        let v = *VARIANTS.iter().find(|v| v.name == name).unwrap();
        let mut c = HellCoder::new(v, 1500.0);
        let e = burst(&mut c, ".");
        // The second column of a full stop is inked on pixels 2 and 3.
        for (p, x) in e[14..28].iter().enumerate() {
            let want = if p == 2 || p == 3 {
                1500.0 + v.fsk
            } else {
                1500.0 - v.fsk
            };
            assert_eq!(x.hz, want, "{name}: pixel {p}");
        }
        let x = one_burst(&hf::HELL, &settings(name), "HI");
        let (a, b) = signal_span(&x);
        let f = (0.01 * RATE) as usize;
        for s in steps(&x[a + f..b - f]) {
            assert!(((s - 1500.0).abs() - v.fsk).abs() < 0.01, "{name}: {s} Hz");
        }
    }
}

/// The columns of the burst as the sender lays them out.
fn ideal(text: &str) -> Vec<u16> {
    ideal_with(text, None)
}

/// [`ideal`] with the font `font` for the characters the Hell font does not have.
fn ideal_with(text: &str, font: Option<&TrueTypeFont>) -> Vec<u16> {
    let mut v = Vec::new();
    for c in format!("...{text} ").chars() {
        tx_columns(c, font, &mut v);
    }
    v
}

/// The best share of equal pixels of the last 60 columns of `p` against the ideal columns, over every alignment along the
/// columns and one pixel up or down (Hell has no clock: the column boundary comes from the blank rows).
fn agreement(p: &sdrkit_codec_image::Raster, ideal: &[u16]) -> f64 {
    let n = 60usize.min(p.width() as usize);
    let mut best = 0.0f64;
    for v in -1i32..=1 {
        for shift in 0..ideal.len() {
            let mut eq = 0;
            for k in 0..n {
                let c = ideal.get(shift + k).copied().unwrap_or(0);
                let x = p.width() - n as u32 + k as u32;
                for px in 0..14i32 {
                    let src = px + v;
                    let want = (0..14).contains(&src) && (c >> src) & 1 == 1;
                    let got = grey(p, x, (13 - px) as u32) > 128;
                    eq += usize::from(got == want);
                }
            }
            best = best.max(eq as f64 / (n * 14) as f64);
        }
    }
    best
}

/// At 20 dB the last 60 columns the stack's own receiver draws match the sender's columns in more than 92 % of the pixels, in
/// every variant (Slow Hell with a short text: 2.2 columns a second).
#[test]
fn hell_tx_loop() {
    for (k, name) in VARIANTS.iter().map(|v| v.name).enumerate() {
        let text = if name == "Slow Hell" {
            "73"
        } else {
            "CQ DE N0CALL N0CALL K"
        };
        let set = settings(name);
        let x = one_burst(&hf::HELL, &set, text);
        let h = harness(&hf::HELL).with_settings(&cases(&set));
        let (mut rx, mut tap) = receiver_with_pictures(&h, RATE);
        feed_rx(rx.as_mut(), &noisy(&x, 20.0, 0.0, 150 + k as u64), 800);
        let p = pictures(&mut tap).pop().expect("a picture");
        let a = agreement(&p, &ideal(text));
        eprintln!("{name}: {a:.3}");
        assert!(a > 0.92, "{name}: {a}");
    }
}

/// The 99 % power bandwidth of Feld Hell text is under the specification's 492 Hz for random pixels with 35 % ink, plus 10 %:
/// text has longer runs.
#[test]
fn hell_tx_bandwidth() {
    let x = one_burst(
        &hf::HELL,
        &[],
        "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789",
    );
    let (lo, hi) = occupied(&x);
    let bw = hi - lo;
    eprintln!("Feld Hell: {bw:.1} Hz");
    assert!(bw < 1.1 * 492.0, "{bw} Hz");
}

#[test]
fn hell_tx_stop_gate_and_tune() {
    check_stop(&hf::HELL, &[], &"CQ ".repeat(20), 2.0);
    check_gate(&hf::HELL, &[], "CQ CQ");
    check_tune(&hf::HELL, &[], 1500.0);
    // The variant setting is the receiver's.
    assert_eq!(VARIANT_SETTING.key, "variant");
}

/// A TrueType font of the system, when there is one.
const FONT_FILE: &str = "/System/Library/Fonts/Supplemental/Arial.ttf";

/// The font file: a character the Hell font does not have (Ł, Ş) is sent with the font, 14 pixels high, between its blank
/// columns, and as `?` without it; ASCII keeps the Hell glyphs. Feld Hell with the font at 20 dB: the last 60 columns the
/// receiver draws match the sender's columns in more than 90 % of the pixels. A file that cannot be read sends `?`.
#[test]
fn hell_tx_font_file() {
    let Ok(bytes) = std::fs::read(FONT_FILE) else {
        return;
    };
    let font = TrueTypeFont::parse(bytes).unwrap();
    for c in ['Ł', 'Ş'] {
        let mut with = Vec::new();
        tx_columns(c, Some(&font), &mut with);
        let mut without = Vec::new();
        tx_columns(c, None, &mut without);
        let mut q = Vec::new();
        tx_columns('?', None, &mut q);
        assert_eq!(without, q, "{c}");
        assert_ne!(with, q, "{c}");
        assert_eq!((with[0], *with.last().unwrap()), (0, 0));
        assert!(with.iter().map(|m| m.count_ones()).sum::<u32>() >= 4);
    }
    let mut a = Vec::new();
    tx_columns('A', Some(&font), &mut a);
    let mut b = Vec::new();
    tx_columns('A', None, &mut b);
    assert_eq!(a, b);
    let text = "ŁÓDŹ ŞIŞLI ŁŞ";
    let mut set = settings("Feld Hell");
    set.push(("font", Value::Text(FONT_FILE.into())));
    let x = one_burst(&hf::HELL, &set, text);
    let h = harness(&hf::HELL).with_settings(&cases(&settings("Feld Hell")));
    let (mut rx, mut tap) = receiver_with_pictures(&h, RATE);
    feed_rx(rx.as_mut(), &noisy(&x, 20.0, 0.0, 171), 800);
    let p = pictures(&mut tap).pop().expect("a picture");
    let got = agreement(&p, &ideal_with(text, Some(&font)));
    eprintln!("Feld Hell with the font: {got:.3}");
    assert!(got > 0.90, "{got}");
    // The length of the burst (0.15 s of silence, the columns, 0.1 s of silence) follows the font's glyphs; a file that cannot
    // be read gives the columns of `?`.
    let burst_s = |cols: usize| 0.25 + cols as f64 * 14.0 / VARIANTS[0].pixel_rate();
    let on = x.len() as f64 / RATE;
    let want = burst_s(ideal_with(text, Some(&font)).len());
    assert!((on - want).abs() < 0.01, "{on} s for {want} s");
    let mut set = settings("Feld Hell");
    set.push(("font", Value::Text("/no/such/font.ttf".into())));
    let x = one_burst(&hf::HELL, &set, "Ł");
    let on = x.len() as f64 / RATE;
    let want = burst_s(ideal_with("Ł", None).len());
    assert!((on - want).abs() < 0.01, "{on} s for {want} s");
}

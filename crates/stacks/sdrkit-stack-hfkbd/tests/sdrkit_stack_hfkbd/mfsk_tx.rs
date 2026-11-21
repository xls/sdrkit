//! The MFSK sender: the tones of the specification's worked examples and the code check, every variant read back by the
//! stack's own receiver without noise and at 30 dB, the weak-signal cases over a fixed set of seeds, the lower sideband, the
//! occupied bandwidth, the continuous phase, the frame of a burst, `stop`, `tune` and the gate.

use crate::common::*;
use crate::tx_common::*;
use hf::mfsk::{MfskCoder, VARIANTS};
use hf::sender::{burst, Element};
use sdrkit_core::Value;
use sdrkit_stack_hfkbd as hf;

/// The tone numbers of the elements of a burst of `v` around 1500 Hz.
fn tones(e: &[Element], v: &hf::mfsk::MfskVariant) -> Vec<u32> {
    let low = v.low_hz(1500.0);
    e.iter()
        .map(|x| {
            let k = (x.hz - low) / v.spacing();
            assert!((k - k.round()).abs() < 1e-9, "{}", x.hz);
            assert!((x.s - v.symlen as f64 / v.rate).abs() < 1e-12);
            k.round() as u32
        })
        .collect()
}

/// The index of a variant.
fn index(name: &str) -> usize {
    VARIANTS.iter().position(|v| v.name == name).unwrap()
}

/// Worked example 1, MFSK16 with "CQ": 8 symbols of the lowest tone (the MFSK16 description), then the tones of the
/// specification after its zero symbols. Worked example 2, MFSK31: one zero code bit leads, 24 tones of 0, then the listed
/// tones. The code check: a single 1 into the empty code gives the pairs 3, 2, 3, 3, 0, 1, 3. MFSK16 tone k at 1500 Hz is at
/// 1382.8125 + 15.625 k Hz.
#[test]
fn mfsk_tx_worked_examples() {
    let v = VARIANTS[index("MFSK16")];
    assert_eq!(v.low_hz(1500.0), 1382.8125);
    let t = tones(&burst(&mut MfskCoder::new(v, 1500.0), "CQ"), &v);
    let mut want = vec![0; 8];
    want.extend([
        12, 0, 0, 0, 12, 0, 12, 12, 0, 12, 10, 12, 12, 10, 6, 0, 10, 6, 6, 0, 6, 3,
    ]);
    assert_eq!(&t[..30], &want[..]);
    let v = VARIANTS[index("MFSK31")];
    assert_eq!(v.tx_preamble(), (1, 35));
    let t = tones(&burst(&mut MfskCoder::new(v, 1500.0), "CQ"), &v);
    let mut want = vec![0; 24];
    want.extend([6, 0, 0, 6, 6, 6, 0, 6, 6, 6, 0, 0, 0, 5, 5, 3]);
    assert_eq!(&t[..40], &want[..]);
    let mut enc = sdrkit_algo_fec::conv::Encoder::new(hf::fec::MFSK_K7);
    let pairs: Vec<u8> = (0..7)
        .map(|i| {
            // The first code bit is bit 0 of the pair.
            enc.push_bit(u8::from(i == 0))
        })
        .collect();
    assert_eq!(pairs, [3, 2, 3, 3, 0, 1, 3]);
    assert_eq!(VARIANTS[index("MFSK16")].tx_flush(), 130);
}

/// The settings of variant `i`.
fn settings(i: usize) -> Vec<(&'static str, Value)> {
    vec![("variant", Value::Choice(i as u16))]
}

const SENT: &str = "CQ DE N0CALL N0CALL K\n";
const SHORT: &str = "CQ DE N0CALL\n";

/// The text of a variant: the short line below 10 baud.
fn text_of(v: &hf::mfsk::MfskVariant) -> &'static str {
    if v.spacing() < 10.0 {
        SHORT
    } else {
        SENT
    }
}

/// The burst of `desc` with `set` and `text` at `snr` dB in 2.5 kHz, read by the stack's receiver with `rx` added to `set`.
fn read_back(
    set: &[(&'static str, Value)],
    rx: &[(&'static str, Value)],
    text: &str,
    snr: Option<f64>,
    seed: u64,
) -> String {
    let x = one_burst(&hf::MFSK, set, text);
    let y = match snr {
        Some(s) => noisy(&x, s, 3.0, seed),
        None => padded(&x, 3.0),
    };
    let mut all = set.to_vec();
    all.extend_from_slice(rx);
    crate::common::text(&feed(&harness(&hf::MFSK).with_settings(&cases(&all)), &y))
}

/// Without noise, with the signal finder off, every variant prints exactly the text sent after the line ends of the start
/// (CR, STX, CR), and nothing printable after it.
#[test]
fn mfsk_tx_loop_clean() {
    let mut bad = Vec::new();
    for (i, v) in VARIANTS.iter().enumerate() {
        let text = text_of(v);
        let t = read_back(
            &settings(i),
            &[("find_signal", Value::Bool(false))],
            text,
            None,
            0,
        );
        if !t.ends_with(&format!("\n\n{text}")) {
            bad.push(format!("{}: {t:?}", v.name));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Every variant at 30 dB in 2.5 kHz, signal finder on: the whole text; the 99 % power bandwidth of the burst is at most the
/// span of the tones and 1.5 baud each side.
#[test]
fn mfsk_tx_loop_30db() {
    let mut bad = Vec::new();
    for (i, v) in VARIANTS.iter().enumerate() {
        let text = text_of(v);
        let x = one_burst(&hf::MFSK, &settings(i), text);
        let h = harness(&hf::MFSK).with_settings(&cases(&settings(i)));
        let t = crate::common::text(&feed(&h, &noisy(&x, 30.0, 3.0, 200 + i as u64)));
        let (lo, hi) = occupied(&x);
        let span = (v.tones() + 2) as f64 * v.spacing();
        eprintln!("{}: {:.1} Hz", v.name, hi - lo);
        if !t.contains(text) {
            bad.push(format!("{}: {t:?}", v.name));
        }
        if hi - lo > span {
            bad.push(format!("{}: {:.1} Hz", v.name, hi - lo));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The share of the seeds 1 to 10 for which the receiver prints the whole text of variant `name` at `snr` dB.
fn rate(name: &str, snr: f64) -> usize {
    let i = index(name);
    (1..=10u64)
        .filter(|&seed| read_back(&settings(i), &[], SENT, Some(snr), 300 + seed).contains(SENT))
        .count()
}

/// The weak-signal targets of the receive specification over the seeds 1 to 10: MFSK16 at -4 dB in 2.5 kHz, MFSK64 at 6 dB.
#[test]
fn mfsk_tx_weak_signal() {
    for (name, snr, need) in [("MFSK16", -4.0, 9), ("MFSK64", 6.0, 9)] {
        let n = rate(name, snr);
        eprintln!("{name} at {snr} dB: {n} of 10");
        assert!(n >= need, "{name} at {snr} dB: {n} of 10");
    }
}

/// With LSB on both sides the text goes through at 30 dB; the tones are the mirror image.
#[test]
fn mfsk_tx_lower_sideband() {
    let mut set = settings(index("MFSK16"));
    set.push(("demod", Value::Choice(hf::front::Demod::Lsb.index())));
    let x = one_burst(&hf::MFSK, &set, SENT);
    let (a, b) = signal_span(&x);
    let f = (0.01 * RATE) as usize;
    assert!(steps(&x[a + f..b - f]).iter().all(|v| *v < -1300.0));
    let r = feed(
        &harness(&hf::MFSK).with_settings(&cases(&set)),
        &noisy(&x, 30.0, 3.0, 41),
    );
    let t = crate::common::text(&r);
    assert!(t.contains(SENT), "{t:?}");
}

/// The phase is continuous: every phase step is one of the tones; the 99 % power bandwidth is within 10 % of the
/// specification's figures, MFSK16 252 Hz and MFSK32 504 Hz.
#[test]
fn mfsk_tx_bandwidth_and_phase() {
    let text = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789 ".repeat(2);
    for (name, want) in [("MFSK16", 252.0), ("MFSK32", 504.0)] {
        let i = index(name);
        let v = VARIANTS[i];
        let x = one_burst(&hf::MFSK, &settings(i), &text);
        check_frame(&x, 0.5);
        let (lo, hi) = occupied(&x);
        let bw = hi - lo;
        eprintln!("{name}: {bw:.1} Hz");
        assert!((bw - want).abs() <= 0.1 * want, "{name}: {bw} Hz");
        let (a, b) = signal_span(&x);
        let f = (0.01 * RATE) as usize;
        for s in steps(&x[a + f..b - f]) {
            let k = (s - v.low_hz(1500.0)) / v.spacing();
            assert!(
                (k - k.round()).abs() < 1e-3 && (0.0..v.tones() as f64).contains(&k.round()),
                "{s} Hz"
            );
        }
    }
}

#[test]
fn mfsk_tx_stop_gate_and_tune() {
    let set = settings(index("MFSK16"));
    check_stop(&hf::MFSK, &set, &"CQ ".repeat(20), 3.0);
    check_gate(&hf::MFSK, &set, "CQ CQ");
    check_tune(&hf::MFSK, &set, 1500.0);
}

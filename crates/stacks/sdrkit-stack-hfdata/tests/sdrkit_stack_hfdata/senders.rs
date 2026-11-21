//! Helpers of the tests of the HF sender stacks: a run through the whole stack at any rate and dial, the rows read, and the
//! count of groups read right.
#![allow(dead_code)]

use crate::common::*;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackDescriptor};
use sdrkit_stack_hfdata::senders::{render, Tone};
use sdrkit_stack_hfdata::typed::{SenderRow, SENDER_ROWS};

/// The centre of the samples in these tests, Hz.
pub const CENTRE: f64 = 6.5e6;

/// Runs `x` (at `rate`, the dial `dial_off` Hz from the centre of the samples) through a fresh receiver of `desc` with the
/// settings `set`, in pieces of 0.1 s, to the end of the input.
pub fn run_at(
    desc: &StackDescriptor,
    x: &[Complex32],
    rate: f64,
    dial_off: f64,
    set: &[(&str, Json)],
) -> (Box<dyn Receiver>, Output) {
    let mut rx = receiver(desc, set);
    rx.configure(&InputInfo::new(rate, CENTRE, CENTRE + dial_off).recording())
        .unwrap();
    let mut out = feed(rx.as_mut(), x, (rate / 10.0) as usize);
    let mut e = end(rx.as_mut());
    out.events.append(&mut e.events);
    (rx, out)
}

/// `seq` at `rate` with the dial `dial_off` Hz from the centre, through `ch` (SNR in 3 kHz for a unit-power signal).
pub fn signal(seq: &[Tone], rate: f64, dial_off: f64, ch: &Channel) -> Vec<Complex32> {
    through(&render(seq, rate, dial_off), rate, ch)
}

/// The rows of the sender table, oldest first: kind and text.
pub fn sender_rows(rx: &dyn Receiver) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = SENDER_ROWS
        .rows::<SenderRow>(rx.results())
        .map(|(_, r)| (r.kind.to_string(), r.text.to_string()))
        .collect();
    v.reverse();
    v
}

/// The texts of the rows of `kind`.
pub fn texts_of(rows: &[(String, String)], kind: &str) -> Vec<String> {
    rows.iter()
        .filter(|(k, _)| k == kind)
        .map(|(_, t)| t.clone())
        .collect()
}

/// How many of `sent` appear in order among the words of `lines`: the longest common subsequence of the groups sent and the
/// words read.
pub fn groups_right(sent: &[String], lines: &[String]) -> usize {
    let words: Vec<&str> = lines.iter().flat_map(|l| l.split(' ')).collect();
    let mut row = vec![0usize; words.len() + 1];
    for g in sent {
        let mut diag = 0;
        for (j, w) in words.iter().enumerate() {
            let up = row[j + 1];
            row[j + 1] = if *w == g.as_str() {
                diag + 1
            } else {
                up.max(row[j])
            };
            diag = up;
        }
    }
    row[words.len()]
}

/// The rate and dial of a case: 24 ksps with the dial at the centre, or a higher rate with the dial where the stacks of the crate
/// look.
pub fn dial_of(rate: f64) -> f64 {
    sdrkit_stack_hfdata::serial::gen::test_dial(rate)
}

/// `seq` sent by a clock `ppm` parts per million slow: every piece longer by `1 + ppm 1e-6` (the tones keep their frequency).
pub fn stretch(seq: &[Tone], ppm: f64) -> Vec<Tone> {
    seq.iter()
        .map(|t| Tone {
            secs: t.secs * (1.0 + ppm * 1e-6),
            ..*t
        })
        .collect()
}

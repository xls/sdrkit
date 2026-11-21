//! LTR: the recorded vector, the six words through the standard impairments, the word code and any cut of the input.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::Results;
use sdrkit_dev_test::Harness;
use sdrkit_stack_trunk::ltr::code::{check_of, encode, fields, talkgroup, LtrFields};
use sdrkit_stack_trunk::ltr::gen::{six_words, LtrSignal};
use sdrkit_stack_trunk::typed::{LTR_REPEATERS, LTR_WORDS};
use sdrkit_stack_trunk::{ltr, LTR};
use std::collections::BTreeSet;

fn talkgroups(r: &Results) -> BTreeSet<String> {
    rows(r, &LTR_WORDS)
        .into_iter()
        .map(|w| w["talkgroup"].clone())
        .collect()
}

#[test]
fn recorded_vector_matches_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("trunk/ltr_240k") else {
        return;
    };
    let exp = sdrkit_dev_test::sigmf::read_expected("trunk").unwrap();
    let e = &exp["recordings"]["ltr_240k"];
    let mut rx = make(&LTR, &[]);
    configure(
        rx.as_mut(),
        rec.rate_hz,
        rec.centre_hz,
        e["signal_offset_hz"].as_f64().unwrap(),
    );
    let fed = feed(rx.as_mut(), &rec.samples, 4096);
    let r = results(rx.as_ref());
    let good = num(r, "words_good");
    assert!((12.0..=18.0).contains(&good), "{good}");
    assert!(num(r, "words_bad") <= 1.0);
    assert_eq!(num(r, "area"), 0.0);
    assert_eq!(num(r, "locked"), 1.0);
    let want_words = e["words_unique"].as_array().unwrap();
    let want: BTreeSet<String> = want_words
        .iter()
        .map(|w| w["talkgroup"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(talkgroups(r), want);
    for w in rows(r, &LTR_WORDS) {
        let u = want_words
            .iter()
            .find(|u| u["talkgroup"].as_str() == Some(&w["talkgroup"]))
            .unwrap();
        let check: u8 = w["check"].parse().unwrap();
        assert_eq!(format!("{check:02X}"), u["check"].as_str().unwrap());
        let kind = match u["kind"].as_str().unwrap() {
            "" => "call",
            "callsign ID" => "callsign_id",
            k => k,
        };
        assert_eq!(w["kind"], kind);
        assert_eq!(w["goto"], u["goto"].to_string());
        assert_eq!(w["free"], u["free"].to_string());
    }
    let reps = rows(r, &LTR_REPEATERS);
    let want_reps = e["repeaters"].as_array().unwrap();
    assert_eq!(reps.len(), want_reps.len());
    for w in want_reps {
        let r = reps
            .iter()
            .find(|r| r["repeater"] == w["repeater"].to_string().as_str())
            .expect("repeater row");
        if good == 18.0 {
            assert_eq!(r["as_goto"], w["as_goto"].to_string());
            assert_eq!(r["as_home"], w["as_home"].to_string());
            assert_eq!(r["as_free"], w["as_free"].to_string());
        }
    }
    check_events(&kinds(&ltr::declaration()), &fed.events, true);
}

#[test]
fn the_word_check_of_the_six_words() {
    // US 6,529,486 B1: the check is the XOR of the check values of the information bits that are 1.
    let want = [0x55, 0x5C, 0x31, 0x04, 0x05, 0x1B];
    for (f, c) in six_words().iter().zip(want) {
        let w = encode(f);
        assert_eq!((w & 0x7F) as u8, c, "{}", talkgroup(f));
        assert_eq!(fields(w), *f);
    }
    // Each information bit alone produces its row of the table.
    let one = LtrFields {
        area: 1,
        ..Default::default()
    };
    assert_eq!(check_of(1 << 23), 0x38);
    assert_eq!(encode(&one) & 0x7F, 0x38);
}

#[test]
fn the_six_words_through_the_standard_impairments() {
    let s = LtrSignal::default();
    let run = Harness::new(&LTR)
        .rate(240e3)
        .seconds(s.iq().len() as f64 / 240e3)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 20.0)
        .set(&GEN_OFFSET_HZ, 400.0)
        .run()
        .unwrap();
    let want: BTreeSet<String> = six_words().iter().map(talkgroup).collect();
    assert_eq!(talkgroups(&run.results), want);
    assert_eq!(num(&run.results, "words_bad"), 0.0);
}

#[test]
fn any_cut_returns_the_same_words() {
    let x = LtrSignal {
        rate: 240e3,
        ..Default::default()
    }
    .iq();
    let run = |chunk: usize| {
        let mut rx = make(&LTR, &[]);
        configure(rx.as_mut(), 240e3, 460e6, 0.0);
        feed(rx.as_mut(), &x, chunk);
        feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 24000], chunk);
        rows(results(rx.as_ref()), &LTR_WORDS)
    };
    assert_eq!(run(4096), run(1001));
}

/// Good words of the third pass of the six words (the first two are the follow-up's settling time) and the words of the whole
/// run that were not sent, with complex noise of `sigma` for each component at 240 ksps and the carrier `offset_hz` from the
/// channel centre.
fn third_pass_words(sigma: f64, offset_hz: f64, seed: u64) -> (f64, usize) {
    let s = LtrSignal {
        offset_hz,
        repeats: 3,
        ..Default::default()
    };
    let mut x = s.iq();
    add_noise(&mut x, sigma, seed);
    let split = ((40 + 12 * 52 + 20) as f64 * s.rate / 300.0) as usize;
    let mut rx = make(&LTR, &[]);
    configure(rx.as_mut(), s.rate, 460e6, 0.0);
    feed(rx.as_mut(), &x[..split], 4096);
    let first = num(results(rx.as_ref()), "words_good");
    feed(rx.as_mut(), &x[split..], 4096);
    let r = results(rx.as_ref());
    let sent: Vec<(String, String, String)> = six_words()
        .iter()
        .map(|f| (talkgroup(f), f.goto.to_string(), f.free.to_string()))
        .collect();
    let not_sent = rows(r, &LTR_WORDS)
        .iter()
        .filter(|w| {
            let key = (w["talkgroup"].clone(), w["goto"].clone(), w["free"].clone());
            !sent.contains(&key)
        })
        .count();
    (num(r, "words_good") - first, not_sent)
}

#[test]
fn words_found_in_noise_with_a_carrier_error() {
    // Words found of 6 (mean of 60 trials). The specification gives the noise as sigma 1.5, 1.75 and 2 without a sample rate.
    // Here: complex noise of 1.3 x sigma for each component at 240 ksps on a carrier of amplitude 1, which is a carrier-to-noise
    // ratio of 4.0, 2.7 and 1.5 dB in 12.5 kHz (45.0, 43.7 and 42.5 dB-Hz).
    let trials = 60;
    let mut lines = Vec::new();
    let mut found = Vec::new();
    let mut not_sent = 0;
    for offset in [0.0, 3100.0] {
        for sigma in [1.5, 1.75, 2.0] {
            let mut sum = 0.0;
            for seed in 0..trials {
                let (words, wrong) = third_pass_words(1.3 * sigma, offset, 100 + seed);
                sum += words;
                not_sent += wrong;
            }
            let f = sum / trials as f64;
            lines.push(format!("carrier {offset} Hz, sigma {sigma}: {f:.2} of 6"));
            found.push(f);
        }
    }
    eprintln!("{}", lines.join("\n"));
    // Asked at carrier 0: 5.3, 2.7 and 0.8.
    assert!(
        found[0] >= 5.3 && found[1] >= 2.7 && found[2] >= 0.8,
        "{lines:?}"
    );
    // Asked at +3.1 kHz: 5.2, 2.2 and 0.2.
    assert!(
        found[3] >= 5.2 && found[4] >= 2.2 && found[5] >= 0.2,
        "{lines:?}"
    );
    // Every word listed in the 360 runs is one of the six.
    assert_eq!(not_sent, 0, "words that were not sent");
}

#[test]
fn noise_alone_has_no_word() {
    // 40 s of noise at the level of the last point above, and at 0.1.
    for (sigma, seed) in [(2.6, 7), (0.1, 8)] {
        let mut x = vec![Complex32::new(0.0, 0.0); 9_600_000];
        add_noise(&mut x, sigma, seed);
        let mut rx = make(&LTR, &[]);
        configure(rx.as_mut(), 240e3, 460e6, 0.0);
        let fed = feed(rx.as_mut(), &x, 65536);
        let r = results(rx.as_ref());
        assert_eq!(num(r, "words_good"), 0.0, "sigma {sigma}");
        assert!(fed.events.is_empty(), "sigma {sigma}");
    }
}

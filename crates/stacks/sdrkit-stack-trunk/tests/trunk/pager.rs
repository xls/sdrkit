//! The pagers: the recorded vector, generator round trips through the standard impairments, the corner cases and any cut of the
//! input.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::{Generator, Receiver, Results, Value};
use sdrkit_dev_test::{ClockError, FrequencyOffset, Harness};
use sdrkit_proto_pager::flex::Speed as FlexSpeed;
use sdrkit_stack_trunk::pager::gen::{
    cycle_pages, flex_tx, pocsag_tx, Content, PagerSignal, Segment, SPEEDS,
};
use sdrkit_stack_trunk::typed::PAGER_PAGES as PAGE_TABLE;
use sdrkit_stack_trunk::PAGER;

/// (protocol, address, type, text) of every page of a cycle.
fn sent(sig: &PagerSignal) -> Vec<(String, u32, String, String)> {
    let mut v = Vec::new();
    for s in SPEEDS {
        if sig.only.as_deref().is_some_and(|o| o != s) {
            continue;
        }
        let flex = s.starts_with("FLEX");
        if (flex && sig.content == Content::Pocsag) || (!flex && sig.content == Content::Flex) {
            continue;
        }
        for x in cycle_pages(s) {
            v.push((s.to_string(), x.address, x.kind.name().to_string(), x.text));
        }
    }
    v
}

/// (protocol, address, type, text, damaged) of every page received.
fn got(r: &Results) -> Vec<(String, u32, String, String, bool)> {
    rows(r, &PAGE_TABLE)
        .into_iter()
        .map(|p| {
            (
                p["protocol"].clone(),
                p["address"].parse().unwrap(),
                p["type"].clone(),
                p["message"].clone(),
                p["damaged"] == "yes" || p["damaged"] == "true",
            )
        })
        .collect()
}

/// The samples of one pass of a signal.
fn collect(sig: &PagerSignal) -> Vec<Complex32> {
    let mut g = sig.generator().once();
    let mut out = Vec::new();
    let mut buf = vec![Complex32::new(0.0, 0.0); 65536];
    loop {
        let n = g.generate(&mut buf);
        out.extend_from_slice(&buf[..n]);
        if n < buf.len() {
            return out;
        }
    }
}

fn run_samples(x: &[Complex32], rate: f64, settings: &[(&str, &str)]) -> (Box<dyn Receiver>, Fed) {
    let mut rx = make(&PAGER, settings);
    configure(rx.as_mut(), rate, 466.1e6, 0.0);
    let mut fed = feed(rx.as_mut(), x, 65536);
    let tail = vec![Complex32::new(0.0, 0.0); (rate * 0.5) as usize];
    fed.events.extend(feed(rx.as_mut(), &tail, 65536).events);
    (rx, fed)
}

fn check_all(sig: &PagerSignal, r: &Results) {
    let g = got(r);
    for (s, a, k, t) in sent(sig) {
        let key = (s, a, k, t, false);
        assert!(g.contains(&key), "missing {key:?}; got {g:#?}");
    }
    assert_eq!(g.len(), sent(sig).len(), "{g:#?}");
}

/// The signal of the vector trunk/pager_cycle_192k made by the generator of the crate: one cycle of the test signal at
/// 192 ksps, the channel 50 kHz below the centre (466.125 MHz) and the carrier 700 Hz above the channel, 20 dB carrier to noise
/// in 25 kHz, noise seed 1. The recording of that name came from a test signal with zero FLEX check sums and a long address
/// that is not one of the long-address sets, so the test builds the signal instead of reading it.
fn pager_cycle_signal() -> Vec<Complex32> {
    let mut x = collect(&PagerSignal {
        rate: ONE_CARRIER,
        ..Default::default()
    });
    FrequencyOffset::new(ONE_CARRIER, -50e3 + 700.0).process(&mut x);
    add_noise_snr(&mut x, ONE_CARRIER, 20.0, 25e3, 1);
    x
}

#[test]
fn recorded_vector_matches_expected() {
    let samples = pager_cycle_signal();
    let exp = sdrkit_dev_test::sigmf::read_expected("trunk").unwrap();
    let e = &exp["recordings"]["pager_cycle_192k"];
    let mut rx = make(&PAGER, &[("wide_input", "true")]);
    configure(
        rx.as_mut(),
        ONE_CARRIER,
        466.125e6,
        e["signal_offset_hz"].as_f64().unwrap(),
    );
    let mut fed = feed(rx.as_mut(), &samples, 4096);
    fed.events
        .extend(feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 96000], 4096).events);
    let r = results(rx.as_ref());
    let g = got(r);
    let want = e["pages"].as_array().unwrap();
    for w in want {
        let k = (
            w["protocol"].as_str().unwrap().to_string(),
            w["address"].as_u64().unwrap() as u32,
            w["type"].as_str().unwrap().to_string(),
            w["text"].as_str().unwrap().to_string(),
            false,
        );
        assert!(g.contains(&k), "missing {k:?}; got {g:#?}");
    }
    assert_eq!(g.len(), want.len(), "{g:#?}");
    let cfo = num(r, "offset_hz");
    assert!((cfo - 700.0).abs() < 250.0, "{cfo}");
    assert!(num(r, "codewords_lost") * 10.0 < num(r, "codewords_good"));
    check_events(
        &kinds(&sdrkit_stack_trunk::pager::declaration()),
        &fed.events,
        true,
    );
}

#[test]
fn one_cycle_at_2_msps_with_noise() {
    let sig = PagerSignal::default();
    let run = Harness::new(&PAGER)
        .rate(2e6)
        .seconds(sig.cycle_s() + 0.3)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 20.0)
        .set(&GEN_OFFSET_HZ, 800.0)
        .run()
        .unwrap();
    check_all(&sig, &run.results);
    check_events(
        &kinds(&sdrkit_stack_trunk::pager::declaration()),
        &run.events,
        true,
    );
}

#[test]
fn pager_events_are_pocsag_and_flex_pages() {
    let sig = PagerSignal::default();
    let x = collect(&PagerSignal { rate: 1e6, ..sig });
    let (_rx, fed) = run_samples(&x, 1e6, &[]);
    let got: Vec<&str> = fed.events.iter().map(|e| e.kind()).collect();
    assert!(
        got.contains(&"pocsag.page") && got.contains(&"flex.page"),
        "{got:?}"
    );
    assert!(
        got.iter().all(|k| *k == "pocsag.page" || *k == "flex.page"),
        "{got:?}"
    );
    let p = fed
        .events
        .iter()
        .find(|e| e.kind() == "pocsag.page")
        .unwrap();
    assert_eq!(field(p, "source"), "1234561");
    assert!(field(p, "baud").parse::<i64>().is_ok());
    check_events(
        &kinds(&sdrkit_stack_trunk::pager::declaration()),
        &fed.events,
        true,
    );
}

#[test]
fn inverted_pocsag_only_and_flex_only() {
    for (content, invert) in [
        (Content::Pocsag, true),
        (Content::Flex, false),
        (Content::Flex, true),
    ] {
        let sig = PagerSignal {
            rate: 1e6,
            content,
            invert,
            ..Default::default()
        };
        let (rx, _) = run_samples(&collect(&sig), 1e6, &[]);
        check_all(&sig, results(rx.as_ref()));
    }
}

#[test]
fn noise_alone_has_no_page() {
    let mut x = vec![Complex32::new(0.0, 0.0); 4_000_000];
    add_noise(&mut x, 0.3, 11);
    let (rx, fed) = run_samples(&x, 1e6, &[]);
    assert!(got(results(rx.as_ref())).is_empty());
    assert!(fed.events.is_empty());
}

#[test]
fn four_level_frame_with_a_bad_fiw_is_dropped() {
    let pages = cycle_pages("FLEX 3200/4");
    let sig = PagerSignal {
        rate: 1e6,
        custom: Some(vec![
            Segment::Silence(0.5),
            Segment::Tx(flex_tx(FlexSpeed::F3200_4, 9, &pages, 4800.0, false, true).unwrap()),
            Segment::Silence(0.5),
            Segment::Tx(flex_tx(FlexSpeed::F3200_4, 10, &pages, 4800.0, false, false).unwrap()),
            Segment::Silence(0.5),
        ]),
        ..Default::default()
    };
    let (rx, _) = run_samples(&collect(&sig), 1e6, &[]);
    let g = got(results(rx.as_ref()));
    assert_eq!(g.len(), 5, "{g:#?}");
}

#[test]
fn pocsag_idle_word_is_the_valid_one() {
    use sdrkit_proto_pager::{pocsag, word};
    // M.584-2 table 2 prints 0x7AC9C197, which fails its own check bits. The idle word that is a code word is 0x7A89C197.
    assert_eq!(pocsag::IDLE, 0x7A89_C197);
    assert!(word::is_valid(pocsag::IDLE));
    assert!(!word::is_valid(pocsag::IDLE_AS_PRINTED));
    // The receiver ends a page at an idle word: a page followed by idle words is read whole and not damaged.
    let sig = PagerSignal {
        rate: 1e6,
        only: Some("POCSAG 1200".into()),
        ..Default::default()
    };
    let (rx, _) = run_samples(&collect(&sig), 1e6, &[]);
    check_all(&sig, results(rx.as_ref()));
}

#[test]
fn read_by_address_refuses_a_bad_rule() {
    let mut rx = make(&PAGER, &[]);
    let text = |t: &str| Value::Text(t.into());
    assert!(set(rx.as_mut(), "read_by_address", &text("1234567:alpha")).is_ok());
    assert!(set(
        rx.as_mut(),
        "read_by_address",
        &text("1234567:alpha 99:sideways")
    )
    .is_err());
    assert!(set(rx.as_mut(), "read_by_address", &text("9999999:alpha")).is_err());
}

#[test]
fn german_characters_on_request() {
    use sdrkit_stack_trunk::pager::gen::TestPage;
    let page = TestPage::new(
        1234567,
        3,
        sdrkit_proto_pager::Kind::Alpha,
        "Gr}~e aus M}nchen",
    );
    let sig = PagerSignal {
        rate: 1e6,
        custom: Some(vec![
            Segment::Silence(0.5),
            Segment::Tx(pocsag_tx(&[page], 1200.0, 4500.0, false).unwrap()),
            Segment::Silence(0.5),
        ]),
        ..Default::default()
    };
    let (rx, _) = run_samples(&collect(&sig), 1e6, &[("charset", "german")]);
    let g = got(results(rx.as_ref()));
    assert_eq!(g[0].3, "Grüße aus München");
}

#[test]
fn clear_empties_the_list_and_state_survives_a_restart() {
    let sig = PagerSignal {
        rate: 1e6,
        only: Some("POCSAG 512".into()),
        ..Default::default()
    };
    let (mut rx, _) = run_samples(&collect(&sig), 1e6, &[]);
    let state = rx.save_state().expect("the pages are kept");
    invoke(rx.as_mut(), "clear");
    assert!(got(results(rx.as_ref())).is_empty());
    let mut back = make(&PAGER, &[]);
    configure(back.as_mut(), 1e6, 466.1e6, 0.0);
    back.restore_state(&state).unwrap();
    assert_eq!(got(results(back.as_ref())).len(), 2);
}

#[test]
fn any_cut_returns_the_same_pages() {
    let sig = PagerSignal {
        rate: 1e6,
        only: Some("FLEX 6400/4".into()),
        ..Default::default()
    };
    let x = collect(&sig);
    let run = |chunk: usize| {
        let mut rx = make(&PAGER, &[]);
        configure(rx.as_mut(), 1e6, 466.1e6, 0.0);
        feed(rx.as_mut(), &x, chunk);
        feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 500_000], chunk);
        got(results(rx.as_ref()))
    };
    assert_eq!(run(65536), run(4099));
}

#[test]
fn sensitivity_at_a_low_snr() {
    // One cycle (27 pages) over three noise seeds at 2 dB SNR in the signal's bandwidth.
    let sig = PagerSignal::default();
    let want = sent(&sig);
    let (mut right, mut wrong) = (0, 0);
    for seed in 1..=3 {
        let run = Harness::new(&PAGER)
            .rate(1e6)
            .seconds(sig.cycle_s() + 0.3)
            .seed(seed)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 2.0)
            .run()
            .unwrap();
        for g in got(&run.results) {
            let key = (g.0.clone(), g.1, g.2.clone(), g.3.clone());
            if !g.4 && want.contains(&key) {
                right += 1;
            } else if !want.iter().any(|w| w.0 == g.0 && w.1 == g.1) {
                wrong += 1;
            }
        }
    }
    eprintln!("sensitivity: {right} right of 81, {wrong} wrong");
    assert!(right >= 25, "{right} right of 81");
    assert!(wrong <= 2, "{wrong} pages that match nothing");
}

/// The pagers' test rate: one carrier at 192 ksps.
const ONE_CARRIER: f64 = 192e3;

/// `segments` after 0.3 s of silence at 192 ksps, the carrier `offset_hz` from the channel centre, the clock `ppm` off, with
/// noise at `snr_db` in 25 kHz.
fn noisy(segments: &[Segment], snr_db: f64, offset_hz: f64, ppm: f64, seed: u64) -> Vec<Complex32> {
    let mut all = vec![Segment::Silence(0.3)];
    all.extend_from_slice(segments);
    let sig = PagerSignal {
        rate: ONE_CARRIER,
        custom: Some(all),
        ..Default::default()
    };
    let mut y = Vec::new();
    ClockError::new(ONE_CARRIER, ppm).process(&collect(&sig), &mut y);
    FrequencyOffset::new(ONE_CARRIER, offset_hz).process(&mut y);
    add_noise_snr(&mut y, ONE_CARRIER, snr_db, 25e3, seed);
    y
}

/// The pages of `x` at 192 ksps.
fn pages_of(x: &[Complex32]) -> Vec<(String, u32, String, String, bool)> {
    let (rx, _) = run_samples(x, ONE_CARRIER, &[("wide_input", "true")]);
    got(results(rx.as_ref()))
}

/// The pages of `speeds` received right (sent, not damaged), and the false pages: an address that was not sent, or a page
/// that is not damaged and differs from the page sent (type or text).
fn right_and_false(speeds: &[&str], got: &[(String, u32, String, String, bool)]) -> (usize, usize) {
    let want: Vec<(String, u32, String, String)> = speeds
        .iter()
        .flat_map(|s| {
            cycle_pages(s)
                .into_iter()
                .map(move |p| (s.to_string(), p.address, p.kind.name().to_string(), p.text))
        })
        .collect();
    let same = |g: &(String, u32, String, String, bool), w: &(String, u32, String, String)| {
        (&g.0, g.1, &g.2, &g.3) == (&w.0, w.1, &w.2, &w.3)
    };
    let right = want
        .iter()
        .filter(|w| got.iter().any(|g| !g.4 && same(g, w)))
        .count();
    let wrong = got
        .iter()
        .filter(|g| {
            let address_sent = want.iter().any(|w| w.0 == g.0 && w.1 == g.1);
            !address_sent || (!g.4 && !want.iter().any(|w| same(g, w)))
        })
        .count();
    (right, wrong)
}

/// One POCSAG 1200 transmission of the cycle's three pages.
fn pocsag_1200() -> Vec<Segment> {
    let tx = pocsag_tx(&cycle_pages("POCSAG 1200"), 1200.0, 4500.0, false).unwrap();
    vec![Segment::Tx(tx), Segment::Silence(0.2)]
}

/// Transmissions of POCSAG 1200 (of 30) whose three pages are all received right.
fn pocsag_trials(snr_db: f64, offset_hz: f64, ppm: f64) -> usize {
    (0..30)
        .filter(|&seed| {
            let g = pages_of(&noisy(&pocsag_1200(), snr_db, offset_hz, ppm, 1000 + seed));
            right_and_false(&["POCSAG 1200"], &g).0 == 3
        })
        .count()
}

/// The share (%) of the pages of `content` received right over 6 noise seeds of the cycle, and the false pages.
fn cycle_share(content: Content, snr_db: f64, offset_hz: f64) -> (f64, usize) {
    let sig = PagerSignal {
        content,
        ..Default::default()
    };
    let speeds: Vec<&str> = SPEEDS
        .iter()
        .copied()
        .filter(|s| match content {
            Content::Pocsag => s.starts_with("POCSAG"),
            Content::Flex => s.starts_with("FLEX"),
            _ => true,
        })
        .collect();
    let n = sent(&sig).len() * 6;
    let (mut right, mut wrong) = (0, 0);
    for seed in 0..6 {
        let g = pages_of(&noisy(&sig.segments(), snr_db, offset_hz, 0.0, 2000 + seed));
        let (r, w) = right_and_false(&speeds, &g);
        right += r;
        wrong += w;
    }
    (100.0 * right as f64 / n as f64, wrong)
}

#[test]
fn pocsag_in_noise_with_tone_decisions_and_two_decoders() {
    // Transmissions of POCSAG 1200 (of 30) with all three pages right, SNR in 25 kHz. Asked: 30, 28, 18, 3 at -2, -3, -4,
    // -5 dB; at least 27 at -3 dB with a 50 ppm clock; a carrier 6.25 kHz and 5 kHz off as before (all 30 at 4 dB).
    let at = |snr| pocsag_trials(snr, 0.0, 0.0);
    let counts = [at(-2.0), at(-3.0), at(-4.0), at(-5.0)];
    let clock = pocsag_trials(-3.0, 0.0, 50.0);
    let offset = pocsag_trials(4.0, 6250.0, 0.0);
    let off_centre = pocsag_trials(4.0, 5000.0, 0.0);
    eprintln!(
        "POCSAG 1200 at -2 to -5 dB: {counts:?} of 30, 50 ppm {clock}, 6.25 kHz {offset}, 5 kHz {off_centre}"
    );
    assert!(
        counts[0] == 30 && counts[1] >= 28 && counts[2] >= 18 && counts[3] >= 3,
        "{counts:?}"
    );
    assert!(clock >= 27, "{clock}");
    assert_eq!((offset, off_centre), (30, 30));
    // The pages of the POCSAG cycle (512, 1200, 2400) over 6 seeds, 42 pages. Asked: 77, 67, 17 % at -3, -4.5, -6 dB, and
    // 100 % at 0 dB with the carrier +3.1 kHz off. At -4.5 dB 28 pages are read, 66.7 %.
    let share: Vec<f64> = [-3.0, -4.5, -6.0]
        .into_iter()
        .map(|snr| cycle_share(Content::Pocsag, snr, 0.0).0)
        .collect();
    let (off, off_false) = cycle_share(Content::Pocsag, 0.0, 3100.0);
    eprintln!("POCSAG cycle at -3, -4.5, -6 dB: {share:?} %, +3.1 kHz 0 dB {off} %");
    assert!(
        share[0] >= 77.0 && share[1] >= 66.6 && share[2] >= 17.0,
        "{share:?}"
    );
    assert_eq!((off, off_false), (100.0, 0));
}

#[test]
fn flex_in_noise_with_two_decoders() {
    // The pages of the four FLEX speeds over 6 seeds. Asked: 73 and 28 % at 0 and -1.5 dB.
    let share = [0.0, -1.5].map(|snr| cycle_share(Content::Flex, snr, 0.0).0);
    eprintln!("FLEX at 0 and -1.5 dB: {share:?} %");
    assert!(share[0] >= 95.0 && share[1] >= 80.0, "{share:?}");
}

/// The false pages of `got` (see [`right_and_false`]) for each protocol and speed of `speeds`.
fn false_by_speed(speeds: &[&str], got: &[(String, u32, String, String, bool)]) -> Vec<usize> {
    speeds
        .iter()
        .map(|s| {
            let mine: Vec<_> = got.iter().filter(|g| g.0 == *s).cloned().collect();
            right_and_false(&[s], &mine).1
        })
        .collect()
}

/// False pages near the threshold: 6 passes of the FLEX cycle (120 pages sent) and 6 of the POCSAG cycle (42 pages) at -1.5 dB
/// in 25 kHz, counted for each speed, apart for pages that a message check covers and pages that none covers (tone and short
/// numeric pages, and pages with a lost word).
///
/// A false page of a sent address is a message whose words a repair of 3 or more wrong bits changed. A word with 3 or more
/// wrong bits passes the hard repair into another code word with about 529/2048 = 0.26 (the soft repair adds only changes of
/// weak bits). The runs read about 6 x 20 = 120 messages; without the checks 12 of them came out changed (11 text, 1
/// numeric). A changed text fragment passes its 10-bit fragment check with 2^-10 and a whole message also its 7-bit
/// signature, 2^-7; a numeric page its 6-bit check with 2^-6. This leaves 11 x 2^-17 + 2^-6 < 0.02 pages: the bound of checked
/// FLEX pages is 0. A page with no message check needs an address word repaired into a short address of another capcode
/// (0.26 x 0.92) and a vector word that passes its code and 4-bit check (0.26 / 16), about 0.015 for each address word with
/// 3 or more wrong bits: the bound of unchecked pages is 1. POCSAG has no message check: a message word with 3 or more wrong
/// bits repaired into another message word (0.26 / 2) cannot be found, so its bound is 1 page.
#[test]
fn false_pages_near_the_threshold() {
    for (content, checked_bound, unchecked_bound) in
        [(Content::Flex, 0, 1), (Content::Pocsag, 1, 1)]
    {
        let speeds: Vec<&str> = SPEEDS
            .iter()
            .copied()
            .filter(|s| s.starts_with("FLEX") == (content == Content::Flex))
            .collect();
        let sig = PagerSignal {
            content,
            ..Default::default()
        };
        let (mut checked, mut unchecked, mut refused) =
            (vec![0; speeds.len()], vec![0; speeds.len()], 0.0);
        for seed in 0..6 {
            let x = noisy(&sig.segments(), -1.5, 0.0, 0.0, 2000 + seed);
            let (rx, _) = run_samples(&x, ONE_CARRIER, &[("wide_input", "true")]);
            let r = results(rx.as_ref());
            let g = got(r);
            assert!(g.iter().all(|p| speeds.contains(&p.0.as_str())), "{g:?}");
            // FLEX text and numeric pages that are not damaged passed a message check; a short numeric page holds at most 8
            // digits and has none.
            let has_check = |p: &(String, u32, String, String, bool)| {
                content == Content::Flex
                    && !p.4
                    && (p.2 == "alpha" || (p.2 == "numeric" && p.3.len() > 8))
            };
            let (a, b): (Vec<_>, Vec<_>) = g.into_iter().partition(has_check);
            for (w, n) in checked.iter_mut().zip(false_by_speed(&speeds, &a)) {
                *w += n;
            }
            for (w, n) in unchecked.iter_mut().zip(false_by_speed(&speeds, &b)) {
                *w += n;
            }
            refused += num(r, "pages_refused");
        }
        eprintln!(
            "{content:?} at -1.5 dB: false pages {speeds:?} checked {checked:?}, unchecked {unchecked:?}, {refused} refused"
        );
        assert!(
            checked.iter().sum::<usize>() <= checked_bound,
            "{checked:?}"
        );
        assert!(
            unchecked.iter().sum::<usize>() <= unchecked_bound,
            "{unchecked:?}"
        );
        if content == Content::Flex {
            assert!(refused > 0.0, "the checks refused nothing");
        }
    }
}

#[test]
fn no_false_page_at_0_db() {
    // The POCSAG and the FLEX cycle over 6 seeds each at 0 dB in 25 kHz: no page that is not damaged differs from a page sent.
    for content in [Content::Pocsag, Content::Flex] {
        let (share, wrong) = cycle_share(content, 0.0, 0.0);
        assert_eq!(wrong, 0, "{content:?}: {share} % right");
    }
}

#[test]
fn no_page_from_noise_alone() {
    // 300 POCSAG 1200 transmissions at -40 dB in 25 kHz: no page.
    let x: Vec<Complex32> = (0..300)
        .flat_map(|seed| noisy(&pocsag_1200(), -40.0, 0.0, 0.0, 5000 + seed))
        .collect();
    let g = pages_of(&x);
    assert!(g.is_empty(), "{g:?}");
}

/// False pages on 900 s of noise, for each protocol and speed. On noise a FLEX page needs a false sync 1, then a FIW that passes
/// the BCH code with at most 2 repaired bits (529 of 2048 words), its 4-bit check (1 in 16) and a cycle below 15, a block
/// information word that does the same, an address word in a page range and a vector word that passes its code, check and
/// values: about 2e-6 a false sync for a tone page, and 2^-10 less for a text page with its words. A POCSAG page needs a sync
/// word within 2 bits (529 of 2^32 places a bit) and 10 of 16 words of the first batch that pass the code (about 2e-3). Over
/// the words tried in 900 s this is far below one page: the bound is 0 at every speed.
#[test]
fn no_page_from_900_s_of_noise() {
    let mut rx = make(&PAGER, &[("wide_input", "true")]);
    configure(rx.as_mut(), ONE_CARRIER, 466.1e6, 0.0);
    let mut events = 0;
    for second in 0..900 {
        let mut x = vec![Complex32::new(0.0, 0.0); ONE_CARRIER as usize];
        add_noise(&mut x, 0.3, 9000 + second);
        events += feed(rx.as_mut(), &x, 65536).events.len();
    }
    let r = results(rx.as_ref());
    let g = got(r);
    let wrong = false_by_speed(&SPEEDS, &g);
    eprintln!(
        "900 s of noise: false pages {SPEEDS:?} {wrong:?}, {} refused",
        num(r, "pages_refused")
    );
    assert!(wrong.iter().all(|&n| n == 0), "{wrong:?}");
    assert!(g.is_empty(), "{g:?}");
    assert_eq!(events, 0);
}

#[test]
fn the_second_decoder_can_be_turned_off() {
    let x = noisy(&pocsag_1200(), 10.0, 0.0, 0.0, 1);
    for on in ["true", "false"] {
        let (rx, _) = run_samples(
            &x,
            ONE_CARRIER,
            &[("wide_input", "true"), ("narrow_decoder", on)],
        );
        let g = got(results(rx.as_ref()));
        assert_eq!(
            right_and_false(&["POCSAG 1200"], &g),
            (3, 0),
            "narrow_decoder {on}: {g:?}"
        );
        assert_eq!(g.len(), 3, "a page twice: {g:?}");
    }
}

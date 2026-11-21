//! The HF teleprinter stacks: EFR, G-TOR, ARQ-E, AUTOSPEC, Piccolo and Coquelet. The format layers without a radio, each stack on
//! its test signal through noise and offsets, noise alone, Clear and the data files.

use crate::common::*;
use sdrkit_core::{
    Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig, StackDescriptor,
};
use sdrkit_stack_hfdata::serial::TEST_TEXT;
use sdrkit_stack_hfdata::teleprinter::ToneSeqGen;
use sdrkit_stack_hfdata::{arqe, autospec, coquelet, efr, gtor, piccolo};

/// `secs` of the test signal of `desc` at 24 kHz with `set` and `submode`.
fn signal(
    desc: &StackDescriptor,
    set: &[(&str, Json)],
    submode: Option<&str>,
    secs: f64,
) -> Vec<Complex32> {
    let mut g = (desc.make_generator.unwrap())(
        &GenConfig::new(24e3, values(desc, set))
            .with_submode_opt(submode.map(String::from))
            .with_seed(1),
    )
    .unwrap();
    generate(g.as_mut(), secs)
}

/// Runs `x` (24 kHz, the dial at 0 Hz) through a receiver of `desc` with `set` and `submode` in pieces of `piece`.
fn run(
    desc: &StackDescriptor,
    set: &[(&str, Json)],
    submode: Option<&str>,
    x: &[Complex32],
    piece: usize,
) -> (Box<dyn Receiver>, Output) {
    let cfg = StackConfig::new(values(desc, set)).with_submode_opt(submode.map(String::from));
    let mut rx = (desc.make_receiver)(&cfg).unwrap();
    let f = desc.tuning.default_hz;
    rx.configure(&InputInfo::new(24e3, f, f).recording())
        .unwrap();
    let mut out = feed(rx.as_mut(), x, piece);
    let mut e = end(rx.as_mut());
    out.events.append(&mut e.events);
    (rx, out)
}

fn noisy(x: &[Complex32], snr_db: f64, offset_hz: f64, seed: u64) -> Vec<Complex32> {
    through(
        x,
        24e3,
        &Channel {
            snr_db: Some(snr_db),
            offset_hz,
            seed,
            ..Channel::default()
        },
    )
}

fn noise(secs: f64, seed: u64) -> Vec<Complex32> {
    noisy(
        &vec![Complex32::default(); (24e3 * secs) as usize],
        0.0,
        0.0,
        seed,
    )
}

fn text_of(rx: &dyn Receiver) -> String {
    field_text(rx, "text")
}

// ------------------------------------------------------------------------------------------------------------------------------
// EFR

#[test]
fn efr_worked_telegram() {
    use sdrkit_proto_link::ft12::{frame, Ft12Event, Ft12Reader};
    let user = [0x25, 0x0C, 0x22, 0x41, 0x42, 0x43];
    let f = frame(&user);
    assert_eq!(
        f,
        [0x68, 0x06, 0x06, 0x68, 0x25, 0x0C, 0x22, 0x41, 0x42, 0x43, 0x19, 0x16]
    );
    let mut bad = f.clone();
    bad[8] ^= 0x10;
    let mut r = Ft12Reader::new(3, 40);
    let got: Vec<Ft12Event> = bad.iter().filter_map(|&b| r.push(b)).collect();
    assert!(matches!(&got[..], [Ft12Event::BadChecksum(_)]));
}

#[test]
fn efr_reads_a_telegram_through_noise() {
    let bits = efr::test_bits(&[37, 12, 34, b'A', b'B', b'C']);
    let seq: Vec<(f64, Option<f64>)> = bits
        .iter()
        .map(|&b| (1.0 / 200.0, Some(if b == 1 { 1670.0 } else { 1330.0 })))
        .collect();
    let mut g = ToneSeqGen::new(24e3, &seq, 600.0);
    let x = generate(&mut g, 8.0);
    let (rx, out) = run(&efr::EFR, &[], None, &noisy(&x, 15.0, 40.0, 3), 4801);
    let ev = event_texts(&out, "efr.telegram");
    assert!(
        ev.iter().any(|e| e.starts_with("EFR telegram ok")
            && e.contains("address 12 / 34")
            && e.ends_with("414243")),
        "{ev:?}"
    );
    let rows = rows(rx.results(), &sdrkit_stack_hfdata::typed::EFR_TELEGRAMS);
    assert!(rows
        .iter()
        .any(|r| text(r, "payload") == "414243" && num(r, "message") == 37.0));
}

#[test]
fn efr_test_signal_and_clear() {
    let x = signal(&efr::EFR, &[], None, 6.0);
    let (mut rx, _) = run(&efr::EFR, &[], None, &x, 4801);
    let rows = rows(rx.results(), &sdrkit_stack_hfdata::typed::EFR_TELEGRAMS);
    let want: String = TEST_TEXT
        .bytes()
        .take(16)
        .map(|b| format!("{b:02X}"))
        .collect();
    assert!(rows.iter().any(|r| text(r, "payload") == want), "{rows:?}");
    invoke(rx.as_mut(), "clear", None);
    feed(rx.as_mut(), &[], 1);
    assert_eq!(text_of(rx.as_ref()), "");
}

// ------------------------------------------------------------------------------------------------------------------------------
// G-TOR

#[test]
fn gtor_crc_golay_and_interleave() {
    assert_eq!(gtor::crc(b"123456789"), 0x906E);
    let mut f = b"CQ".to_vec();
    f.extend([0x1E; 19]);
    f.push(0);
    assert_eq!(gtor::crc(&f), 0x99A6);
    assert_eq!(gtor::golay_parity(0x001), 0xFFE);
    assert_eq!(gtor::golay_parity(0x800), 0xDC5);
    assert_eq!(gtor::golay_parity(0xFFF), 0xFFF);
    assert!((0..4096u16).all(|v| gtor::golay_parity(gtor::golay_parity(v)) == v));
    let frame = gtor::data_frame(b"CQ", 24, 0);
    let g = gtor::groups(&frame);
    assert_eq!(g.len(), 16);
    let bits = gtor::interleave(&g);
    assert_eq!(bits[0], (g[0] >> 11) as u8 & 1);
    assert_eq!(bits[16], (g[0] >> 10) as u8 & 1);
    assert_eq!(gtor::bytes_of(&gtor::deinterleave(&bits, 16)), frame);
}

#[test]
fn gtor_every_speed_plain_and_golay() {
    for baud in ["100", "200", "300"] {
        for golay in [false, true] {
            let set = [("gen_baud", json!(baud)), ("gen_golay", json!(golay))];
            let x = signal(&gtor::GTOR, &set, None, 9.0);
            let (rx, out) = run(&gtor::GTOR, &[], None, &noisy(&x, 22.0, 25.0, 5), 4801);
            assert!(
                text_of(rx.as_ref()).contains(TEST_TEXT),
                "{baud} {golay}: {:?} {:?}",
                text_of(rx.as_ref()),
                event_texts(&out, "gtor.frame")
            );
            let ev = event_texts(&out, "gtor.frame");
            assert!(ev
                .iter()
                .any(|e| e.starts_with(&format!("G-TOR {baud} Bd data block 1"))
                    && e.contains("Golay form") == golay));
        }
    }
}

#[test]
fn gtor_at_0_db() {
    for (i, off) in [-30.0, 0.0, 30.0].into_iter().enumerate() {
        let x = signal(&gtor::GTOR, &[], None, 9.0);
        let (rx, _) = run(
            &gtor::GTOR,
            &[],
            None,
            &noisy(&x, 0.0, off, 10 + i as u64),
            4801,
        );
        assert!(
            text_of(rx.as_ref()).contains(TEST_TEXT),
            "{off}: {:?}",
            text_of(rx.as_ref())
        );
    }
}

// ------------------------------------------------------------------------------------------------------------------------------
// ARQ-E and ARQ-N

#[test]
fn arqe_worked_words() {
    use sdrkit_util_text::sevenunit::ident_parity::word;
    assert_eq!(word(3, false), 0x46);
    assert_eq!(word(1, false), 0x02);
    assert_eq!(word(4, false), 0x08);
    assert_eq!(!word(3, false) & 0x7F, 0x39);
}

/// The seconds of an ARQ test signal at `baud` up to the end of the text and a little more.
fn arq_secs(baud: f64) -> f64 {
    (200.0 + 70.0) * 7.0 / baud + 3.0
}

#[test]
fn arqe_cycles_speeds_and_offsets() {
    for (baud, shift, cycle, off, snr) in [
        (96.0, 400.0, "8", 35.0, 14.0),
        (184.6, 400.0, "4", -60.0, 16.0),
        (48.0, 170.0, "8", 20.0, 14.0),
    ] {
        let set = [
            ("gen_baud", json!(baud)),
            ("gen_shift_hz", json!(shift)),
            ("gen_cycle", json!(cycle)),
        ];
        let x = signal(&arqe::ARQE, &set, None, arq_secs(baud));
        let (rx, out) = run(&arqe::ARQE, &[], None, &noisy(&x, snr, off, 7), 4801);
        assert!(
            text_of(rx.as_ref()).contains(TEST_TEXT),
            "{baud}: {:?} {:?}",
            text_of(rx.as_ref()),
            event_texts(&out, "fsk.lock")
        );
        let lock = event_texts(&out, "fsk.lock");
        assert!(
            lock.iter()
                .any(|e| e.starts_with("ARQ-E word sync") && e.contains(&format!("cycle {cycle}"))),
            "{lock:?}"
        );
    }
}

#[test]
fn arqn_speeds() {
    for (baud, shift, off, snr) in [(96.0, 850.0, 25.0, 14.0), (192.0, 400.0, -40.0, 16.0)] {
        let set = [("gen_baud", json!(baud)), ("gen_shift_hz", json!(shift))];
        let x = signal(&arqe::ARQE, &set, Some("arqn"), arq_secs(baud));
        let (rx, out) = run(
            &arqe::ARQE,
            &[],
            Some("arqn"),
            &noisy(&x, snr, off, 8),
            4801,
        );
        assert!(
            text_of(rx.as_ref()).contains(TEST_TEXT),
            "{baud}: {:?}",
            text_of(rx.as_ref())
        );
        let lock = event_texts(&out, "fsk.lock");
        assert!(
            lock.iter()
                .any(|e| e.starts_with("ARQ-N word sync") && e.contains("no inversion")),
            "{lock:?}"
        );
    }
}

#[test]
fn arqe_near_the_threshold() {
    for (sub, set, snr) in [
        (None, vec![], -1.0),
        (
            None,
            vec![("gen_baud", json!(184.6)), ("gen_cycle", json!("4"))],
            2.0,
        ),
        (Some("arqn"), vec![("gen_shift_hz", json!(850.0))], -1.0),
    ] {
        let baud = set
            .iter()
            .find(|s| s.0 == "gen_baud")
            .map_or(96.0, |s| s.1.as_f64().unwrap());
        for (i, off) in [-30.0, 0.0, 30.0].into_iter().enumerate() {
            let x = signal(&arqe::ARQE, &set, sub, arq_secs(baud));
            let (rx, _) = run(
                &arqe::ARQE,
                &[],
                sub,
                &noisy(&x, snr, off, 30 + i as u64),
                4801,
            );
            assert!(
                text_of(rx.as_ref()).contains(TEST_TEXT),
                "{sub:?} {set:?} {off}: {:?}",
                text_of(rx.as_ref())
            );
        }
    }
}

#[test]
fn arqe_f342_code() {
    let set = [("gen_word_code", json!("f342"))];
    let x = signal(&arqe::ARQE, &set, None, arq_secs(96.0));
    let (rx, _) = run(
        &arqe::ARQE,
        &[("word_code", json!("f342"))],
        None,
        &noisy(&x, 14.0, 10.0, 9),
        4801,
    );
    assert!(
        text_of(rx.as_ref()).contains(TEST_TEXT),
        "{:?}",
        text_of(rx.as_ref())
    );
    let (rx, _) = run(&arqe::ARQE, &[], None, &noisy(&x, 14.0, 10.0, 9), 4801);
    assert!(!text_of(rx.as_ref()).contains("QUICK"));
}

#[test]
fn arqe_f342_frame_places() {
    use arqe::frame_place;
    // 4 channels: A and C element by element, then B and D element by element.
    let a: Vec<usize> = (0..7).map(|u| frame_place(4, 0, u)).collect();
    let c: Vec<usize> = (0..7).map(|u| frame_place(4, 2, u)).collect();
    let b: Vec<usize> = (0..7).map(|u| frame_place(4, 1, u)).collect();
    let d: Vec<usize> = (0..7).map(|u| frame_place(4, 3, u)).collect();
    assert_eq!(a, [0, 2, 4, 6, 8, 10, 12]);
    assert_eq!(c, [1, 3, 5, 7, 9, 11, 13]);
    assert_eq!(b, [14, 16, 18, 20, 22, 24, 26]);
    assert_eq!(d, [15, 17, 19, 21, 23, 25, 27]);
    // 2 channels: the characters in turn.
    assert_eq!(frame_place(2, 1, 0), 7);
    // The cycle: A and D invert the first character of a cycle, B and C the others.
    let u = arqe::test_units(4, arqe::WordCode::Parity, 4);
    let word = |k: usize, places: &[usize]| {
        places
            .iter()
            .enumerate()
            .fold(0u8, |w, (i, &p)| w | (u[28 * k + p] << i))
    };
    let odd = |w: u8| w.count_ones() % 2 == 1;
    for k in 0..8 {
        let first = k % 4 == 3;
        assert_eq!(odd(word(k, &a)), !first, "A {k}");
        assert_eq!(odd(word(k, &d)), !first, "D {k}");
        assert_eq!(odd(word(k, &b)), first, "B {k}");
        assert_eq!(odd(word(k, &c)), first, "C {k}");
    }
}

/// The seconds of an ARQ test signal of `n` channels at `baud` up to the end of the texts and a little more.
fn arq_channels_secs(n: usize, baud: f64) -> f64 {
    (400.0 / n as f64 + 90.0) * 7.0 * n as f64 / baud + 3.0
}

#[test]
fn arqe_f342_four_and_two_channels() {
    for (n, baud, cycle, code, shown, off, snr, seed) in [
        (4, 192.0, "4", "parity", "a", 30.0, 16.0, 21),
        (4, 192.0, "8", "parity", "c", -40.0, 16.0, 22),
        (4, 192.0, "8", "f342", "d", 15.0, 16.0, 23),
        (2, 96.0, "4", "parity", "b", 25.0, 14.0, 24),
    ] {
        let set = [
            ("gen_baud", json!(baud)),
            ("gen_cycle", json!(cycle)),
            ("gen_channels", json!(n.to_string())),
            ("gen_word_code", json!(code)),
        ];
        let x = signal(&arqe::ARQE, &set, None, arq_channels_secs(n, baud));
        // On a 4-channel signal that idles, A and C (B and D) send complementary units: the automatic speed finds half the
        // rate, so the speed is given.
        let rx_baud = if n == 4 { baud } else { 0.0 };
        let (rx, out) = run(
            &arqe::ARQE,
            &[
                ("word_code", json!(code)),
                ("channel", json!(shown)),
                ("baud", json!(rx_baud)),
            ],
            None,
            &noisy(&x, snr, off, seed),
            4801,
        );
        let ch = ["a", "b", "c", "d"]
            .iter()
            .position(|c| *c == shown)
            .unwrap();
        let want = arqe::channel_text(ch);
        assert!(
            text_of(rx.as_ref()).contains(&want),
            "{n} channels, {shown}: {:?} {:?}",
            text_of(rx.as_ref()),
            event_texts(&out, "fsk.lock")
        );
        let lock = event_texts(&out, "fsk.lock");
        assert!(
            lock.iter().any(|e| e.starts_with("ARQ-E word sync")
                && e.contains(&format!("cycle {cycle}, {n} channels"))),
            "{lock:?}"
        );
    }
    // A 4-channel signal read as one channel gives no text.
    let set = [
        ("gen_baud", json!(192.0)),
        ("gen_cycle", json!("4")),
        ("gen_channels", json!("4")),
    ];
    let x = signal(&arqe::ARQE, &set, None, arq_channels_secs(4, 192.0));
    let (rx, _) = run(
        &arqe::ARQE,
        &[("channels", json!("1")), ("baud", json!(192.0))],
        None,
        &noisy(&x, 16.0, 0.0, 25),
        4801,
    );
    assert!(!text_of(rx.as_ref()).contains("QUICK"));
}

// ------------------------------------------------------------------------------------------------------------------------------
// AUTOSPEC and SPREAD

#[test]
fn spread_layout() {
    assert_eq!(autospec::position(5, 3, 21), 113);
    // Every stream position holds one unit.
    for s in [1, 11, 21, 51] {
        let mut seen = vec![0u8; 10 * 200 + 9 * s];
        for n in 0..200 {
            for k in 0..10 {
                seen[autospec::position(n, k, s)] += 1;
            }
        }
        assert!(seen[9 * s..2000].iter().all(|&v| v == 1), "{s}");
    }
}

/// The seconds of an AUTOSPEC test signal at `baud` and depth `s` up to the end of the text and a little more.
fn spec_secs(baud: f64, s: f64) -> f64 {
    ((150.0 + 70.0) * 10.0 + 9.0 * s) / baud + 3.0
}

#[test]
fn autospec_and_spread() {
    for (sub, baud, depth, off, snr) in [
        ("autospec", 68.5, "11", 30.0, 12.0),
        ("autospec", 137.0, "11", -30.0, 14.0),
        ("spread", 102.7, "11", 15.0, 12.0),
        ("spread", 137.0, "21", -20.0, 14.0),
        ("spread", 68.5, "51", 10.0, 12.0),
    ] {
        let set = [("gen_baud", json!(baud)), ("gen_depth", json!(depth))];
        let s: f64 = if sub == "spread" {
            depth.parse().unwrap()
        } else {
            1.0
        };
        let x = signal(&autospec::AUTOSPEC, &set, Some(sub), spec_secs(baud, s));
        let (rx, out) = run(
            &autospec::AUTOSPEC,
            &[],
            Some(sub),
            &noisy(&x, snr, off, 11),
            4801,
        );
        assert!(
            text_of(rx.as_ref()).contains(TEST_TEXT),
            "{sub} {baud} {depth}: {:?} {:?}",
            text_of(rx.as_ref()),
            event_texts(&out, "fsk.lock")
        );
        let want = if sub == "spread" {
            format!("SPREAD {depth}")
        } else {
            "Mk 1".to_string()
        };
        assert_eq!(field_text(rx.as_ref(), "detail"), want);
    }
}

// ------------------------------------------------------------------------------------------------------------------------------
// Piccolo and Coquelet

#[test]
fn piccolo_matrix() {
    use piccolo::{pair_code, PairCode};
    let mut codes = Vec::new();
    let mut sync = 0;
    let mut mi = 0;
    for a in 0..6 {
        for b in 0..6 {
            match pair_code(a, b) {
                PairCode::Ita2(c) => codes.push(c),
                PairCode::Sync => sync += 1,
                PairCode::Mi => mi += 1,
                PairCode::Never => assert_eq!((a, b), (3, 3)),
                PairCode::Unused => assert_eq!((a, b), (0, 0)),
            }
        }
    }
    codes.sort_unstable();
    codes.dedup();
    // 26 letters, LF, CR, space, LTRS, FIGS; with SYNC twice and MI, 34 pairs.
    assert_eq!(codes.len(), 31);
    assert!(!codes.contains(&0));
    assert_eq!(codes.len() + sync + mi, 34);
}

#[test]
fn coquelet_table() {
    let pairs: std::collections::BTreeSet<(u8, u8)> = (0..32).map(coquelet::pair_of).collect();
    assert_eq!(pairs.len(), 32);
    assert_eq!(coquelet::pair_of(28), (1, 8));
    assert_eq!(coquelet::pair_of(1), (8, 6));
    assert_eq!(coquelet::pair_of(31), (1, 5));
    assert!((0..32).all(|c| {
        let (a, b) = coquelet::pair_of(c);
        coquelet::code_of(a, b) == Some(c)
    }));
    assert_eq!(coquelet::code_of(3, 4), None);
}

#[test]
fn piccolo_test_signal() {
    let x = signal(&piccolo::PICCOLO, &[], None, 16.0);
    let (rx, out) = run(
        &piccolo::PICCOLO,
        &[],
        None,
        &noisy(&x, 16.0, 40.0, 12),
        4801,
    );
    assert!(
        text_of(rx.as_ref()).contains(TEST_TEXT),
        "{:?} {:?}",
        text_of(rx.as_ref()),
        event_texts(&out, "fsk.lock")
    );
    assert_eq!(field_text(rx.as_ref(), "detail"), "50 ms tones");
    assert!((field(rx.as_ref(), "centre_hz").unwrap() - 550.0).abs() < 4.0);
    assert!(event_texts(&out, "piccolo.pairs")
        .iter()
        .any(|e| e == "Piccolo tone pairs found"));
}

#[test]
fn coquelet_tone_lengths() {
    for (ms, off, snr, label) in [
        ("37.5", -30.0, 16.0, "38 ms tones"),
        ("75", 20.0, 14.0, "75 ms tones"),
        ("50", 0.0, 16.0, "50 ms tones"),
    ] {
        let len: f64 = ms.parse::<f64>().unwrap() / 1000.0;
        let secs = 144.0 * 2.0 * len + 4.0;
        let x = signal(
            &coquelet::COQUELET,
            &[("gen_tone_ms", json!(ms))],
            None,
            secs,
        );
        let (rx, out) = run(
            &coquelet::COQUELET,
            &[],
            None,
            &noisy(&x, snr, off, 13),
            4801,
        );
        assert!(
            text_of(rx.as_ref()).contains(TEST_TEXT),
            "{ms}: {:?} {:?} {:?}",
            text_of(rx.as_ref()),
            event_texts(&out, "fsk.lock"),
            field(rx.as_ref(), "centre_hz")
        );
        assert_eq!(field_text(rx.as_ref(), "detail"), label);
    }
}

#[test]
fn tone_pairs_are_lost_when_the_signal_stops() {
    for d in [&piccolo::PICCOLO, &coquelet::COQUELET] {
        let mut x = signal(d, &[], None, 16.0);
        let n = x.len();
        x.extend(vec![Complex32::default(); (24e3 * 10.0) as usize]);
        let x = noisy(&x, 16.0, 0.0, 14);
        let (rx, _) = run(d, &[], None, &x[..n], 4801);
        let before = text_of(rx.as_ref()).chars().count();
        let (rx, out) = run(d, &[], None, &x, 4801);
        let after = text_of(rx.as_ref()).chars().count();
        assert!(after < before + 12, "{}: {before} then {after}", d.id);
        assert!(!event_texts(&out, "fsk.lost").is_empty(), "{}", d.id);
    }
}

// ------------------------------------------------------------------------------------------------------------------------------
// Every stack

/// The teleprinter stacks.
fn stacks() -> Vec<&'static StackDescriptor> {
    vec![
        &efr::EFR,
        &gtor::GTOR,
        &arqe::ARQE,
        &autospec::AUTOSPEC,
        &piccolo::PICCOLO,
        &coquelet::COQUELET,
    ]
}

#[test]
fn noise_alone_gives_almost_nothing() {
    for d in stacks() {
        let (rx, _) = run(d, &[], None, &noise(10.0, 21), 4801);
        let t = text_of(rx.as_ref());
        assert!(t.chars().count() < 12, "{}: {t:?}", d.id);
    }
}

#[test]
fn reset_keeps_the_text() {
    for d in stacks() {
        let secs = match d.id {
            "arqe" => arq_secs(96.0),
            "autospec" => spec_secs(68.5, 1.0),
            "piccolo" | "coquelet" => 16.0,
            _ => 9.0,
        };
        let x = signal(d, &[], None, secs);
        let (mut rx, _) = run(d, &[], None, &x, 24_000);
        let before = text_of(rx.as_ref());
        assert!(!before.is_empty(), "{}", d.id);
        invoke(rx.as_mut(), "reset", None);
        feed(rx.as_mut(), &[], 1);
        assert_eq!(text_of(rx.as_ref()), before, "{}", d.id);
    }
}

#[test]
fn the_data_files() {
    use sdrkit_stack_hfdata::descriptors;
    let dials = data_rows("hf_teleprinter_dials.csv");
    assert_eq!(dials.len(), 9);
    for row in &dials {
        let id = row[0].split(':').next().unwrap();
        let d = descriptors()
            .into_iter()
            .find(|d| d.id == id)
            .unwrap_or_else(|| panic!("no stack {id}"));
        let hz: f64 = row[1].parse().unwrap();
        assert!(hz >= d.tuning.min_hz && hz <= d.tuning.max_hz, "{}", row[0]);
        assert_ne!(
            hz, d.tuning.default_hz,
            "{}: the descriptor holds a dial",
            row[0]
        );
        if let Some(sub) = row[0].split(':').nth(1) {
            assert!(d.submodes.iter().any(|s| s.id == sub), "{}", row[0]);
        }
    }
    let efr = data_rows("efr_transmitters.csv");
    assert_eq!(efr.len(), 3);
    for r in efr.iter().filter(|r| !r[1].is_empty()) {
        let dial: f64 = r[1].parse().unwrap();
        let carrier: f64 = r[2].parse().unwrap();
        assert_eq!(carrier - dial, 1500.0, "{r:?}");
    }
}

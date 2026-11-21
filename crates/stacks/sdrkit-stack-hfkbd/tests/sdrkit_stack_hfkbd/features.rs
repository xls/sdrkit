//! What every stack shares: descriptors and settings, the rate refusal, the generators, cutting the input into pieces of any
//! size, Listen, text kept across a retune and a reconfigure, Clear, the declarations, the registry and the table facts.

use crate::common::*;
use sdrkit_core::{owner, Complex32, ConfigError, InputInfo};
use sdrkit_stack_hfkbd as hf;

#[test]
fn every_stack_has_a_descriptor_settings_and_a_generator() {
    let ids: Vec<&str> = hf::descriptors().iter().map(|d| d.id).collect();
    assert_eq!(
        ids,
        [
            "rtty",
            "psk",
            "cw",
            "olivia",
            "contestia",
            "mfsk",
            "dominoex",
            "thor",
            "hell",
            "mt63",
            "fsq",
            "ifkp",
            "sstv",
            "sitor",
            "ardop",
            "ros",
            "throb",
            "qrss",
            "cp16",
            "winmor"
        ]
    );
    for d in hf::descriptors() {
        assert_eq!(d.group, "HF and utility");
        assert_eq!(d.tuning.min_rate_hz, 12000.0);
        assert_eq!(d.tuning.rate_hz, 2e6);
        assert_eq!(d.tuning.tune_offset_hz, 20e3);
        let schema = (d.settings)();
        // Every stack has Listen: the front end's switch, or for CW its own choice of audio.
        assert!(schema.id("listen").is_ok(), "{}", d.id);
        // Every setting the descriptor lists is taken by the receiver at its default.
        let mut rx = harness(d).receiver().unwrap();
        for s in schema.settings() {
            let id = schema.id(&s.field.key).unwrap();
            owner::apply(rx.as_mut(), id, &s.default)
                .unwrap_or_else(|e| panic!("{} {}: {e}", d.id, s.field.key));
        }
        assert!(schema.id("no_such_setting").is_err(), "{}", d.id);
        assert!(d.make_generator.is_some());
    }
}

#[test]
fn a_rate_below_12_ksps_is_refused() {
    for d in hf::descriptors() {
        let mut rx = harness(d).receiver().unwrap();
        let e = rx
            .configure(&InputInfo::new(8000.0, 14e6, 14e6))
            .unwrap_err();
        assert!(
            matches!(e, ConfigError::RateTooLow { .. }) || e.to_string().contains("12 ksps"),
            "{}: {e}",
            d.id
        );
        rx.configure(&InputInfo::new(12000.0, 14e6, 14e6)).unwrap();
    }
}

#[test]
fn every_generator_runs_at_any_rate() {
    for d in hf::descriptors() {
        for rate in [12000.0, 48000.0] {
            let x = harness(d).rate(rate).generate().unwrap();
            assert_eq!(x.len(), rate as usize);
            assert!(
                x.iter().all(|s| s.re.is_finite() && s.im.is_finite()),
                "{}",
                d.id
            );
        }
    }
}

/// The text decoded from the same samples cut two ways.
fn cut_two_ways(desc: &'static sdrkit_core::StackDescriptor, seconds: f64) {
    let h = harness(desc).seconds(seconds);
    let x = h.generate().unwrap();
    let a = text(&feed(&h.clone().piece(7), &x));
    let b = text(&feed(&h.piece(4093), &x));
    assert_eq!(a, b, "{}", desc.id);
    assert!(!a.is_empty(), "{}", desc.id);
}

#[test]
fn results_do_not_depend_on_the_piece_size() {
    cut_two_ways(&hf::RTTY, 12.0);
    cut_two_ways(&hf::MFSK, 20.0);
    cut_two_ways(&hf::MT63, 15.0);
    cut_two_ways(&hf::SITOR, 15.0);
}

#[test]
fn listen_returns_audio_and_the_same_text() {
    for d in [
        &hf::RTTY,
        &hf::PSK,
        &hf::MT63,
        &hf::FSQ,
        &hf::IFKP,
        &hf::ARDOP,
    ] {
        let h = harness(d).seconds(10.0);
        let x = h.generate().unwrap();
        let quiet = feed(&h, &x);
        let loud = feed(&h.clone().set(&hf::front::LISTEN, true), &x);
        assert_eq!(text(&quiet), text(&loud), "{}", d.id);
        assert!(quiet.audio.iter().all(|&v| v == 0.0), "{}", d.id);
        let frames = loud.audio.len();
        assert!(
            (frames as f64 - 80000.0).abs() <= 500.0,
            "{}: {frames}",
            d.id
        );
    }
}

#[test]
fn text_survives_a_retune_and_clear_empties_it() {
    let h = harness(&hf::RTTY).seconds(12.0);
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    feed_rx(rx.as_mut(), &x, 800);
    let text_of = |rx: &dyn sdrkit_core::Receiver| field_text(rx.results(), "text");
    let before = text_of(rx.as_ref());
    assert!(before.contains("K0TST"));
    rx.retuned(&InputInfo::new(RATE, 14.1e6, 14.1e6).recording());
    let quiet = noise_only(1.0, 1);
    feed_rx(rx.as_mut(), &quiet, 800);
    assert!(text_of(rx.as_ref()).contains(&before[..20]));
    assert_eq!(
        rx.results().status().lock_state,
        sdrkit_core::LockState::Searching
    );
    // A reconfigure keeps it too.
    rx.configure(&InputInfo::new(24000.0, 14.1e6, 14.1e6))
        .unwrap();
    feed_rx(rx.as_mut(), &quiet, 800);
    assert!(text_of(rx.as_ref()).contains(&before[..20]));
    // Remove forgets one station. A station that is not there is refused.
    let has_station = |rx: &dyn sdrkit_core::Receiver| {
        let r = rx.results();
        let t = r.decl().table_id("stations").and_then(|id| r.table(id));
        t.is_some_and(|t| t.rows_newest_first().any(|(k, _)| k == "K0TST"))
    };
    assert!(has_station(rx.as_ref()));
    let remove = rx.commands().id("remove").unwrap();
    let mut args = rx.commands().def(remove).unwrap().new_args();
    let row = args.decl().id("row").unwrap();
    args.set_value(row, &sdrkit_core::Value::Text("K0TST".into()))
        .unwrap();
    owner::invoke(rx.as_mut(), remove, &args).unwrap();
    assert!(!has_station(rx.as_ref()));
    assert!(matches!(
        owner::invoke(rx.as_mut(), remove, &args),
        Err(sdrkit_core::CommandError::NoSuchRow { .. })
    ));
    let clear = rx.commands().id("clear").unwrap();
    let args = rx.commands().def(clear).unwrap().new_args();
    owner::invoke(rx.as_mut(), clear, &args).unwrap();
    feed_rx(rx.as_mut(), &vec![Complex32::default(); 2400], 800);
    assert_eq!(text_of(rx.as_ref()), "");
}

/// A change of frequency keeps the text, the callsign, the stations and the counts, and drops the tones of the signal.
#[test]
fn a_change_of_frequency_keeps_the_text_and_drops_the_tones() {
    let h = harness(&hf::RTTY).seconds(12.0);
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    feed_rx(rx.as_mut(), &x, 800);
    let stations = |rx: &dyn sdrkit_core::Receiver| {
        let r = rx.results();
        let t = r.decl().table_id("stations").and_then(|id| r.table(id));
        t.map_or(0, |t| t.rows_newest_first().count())
    };
    let text = field_text(rx.results(), "text");
    assert!(text.contains("K0TST"), "{text:?}");
    assert_eq!(field_text(rx.results(), "callsign"), "K0TST");
    assert!(field_num(rx.results(), "mark_hz") > 0.0);
    assert_eq!(stations(rx.as_ref()), 1);
    let characters = field_num(rx.results(), "characters");
    assert!(characters > 0.0);
    rx.forget();
    assert_eq!(field_num(rx.results(), "characters"), characters);
    assert_eq!(field_text(rx.results(), "text"), text);
    assert_eq!(field_text(rx.results(), "callsign"), "K0TST");
    assert_eq!(stations(rx.as_ref()), 1);
    assert!(field_num(rx.results(), "mark_hz").is_nan());
    assert!(field_num(rx.results(), "space_hz").is_nan());
}

/// The text of a PSK or CW signal is a column of the signals table. A change of frequency turns the signals into rows
/// of gone signals, which stay with their text. A new signal at the same audio frequency gets a row of its own.
#[test]
fn a_change_of_frequency_keeps_the_text_of_the_signals() {
    for desc in [&hf::PSK, &hf::CW] {
        let h = harness(desc).seconds(25.0);
        let x = h.generate().unwrap();
        let mut rx = h.receiver().unwrap();
        rx.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
            .unwrap();
        feed_rx(rx.as_mut(), &x, 800);
        // The id, the text and the gone flag of each row with text.
        let signals = |rx: &dyn sdrkit_core::Receiver| -> Vec<(String, String, bool)> {
            let r = rx.results();
            let t = r
                .decl()
                .table_id("signals")
                .and_then(|id| r.table(id))
                .expect("the signals table");
            let mut rows: Vec<(String, String, bool)> = t
                .rows_newest_first()
                .map(|(key, row)| {
                    (
                        key.to_owned(),
                        cell_text(row, "text"),
                        cell_flag(row, "gone") == Some(true),
                    )
                })
                .filter(|(_, text, _)| !text.is_empty())
                .collect();
            rows.sort();
            rows
        };
        let before = signals(rx.as_ref());
        assert!(!before.is_empty(), "{}: a signal with text", desc.id);
        rx.retuned(&InputInfo::new(RATE, 7e6, 7e6).recording());
        rx.forget();
        let kept: Vec<(String, String, bool)> = before
            .iter()
            .map(|(key, text, _)| (key.clone(), text.clone(), true))
            .collect();
        assert_eq!(signals(rx.as_ref()), kept, "{}", desc.id);
        // The same signal again, at the same audio frequency: the rows of the frequency before are as they were.
        feed_rx(rx.as_mut(), &x, 800);
        let after = signals(rx.as_ref());
        for row in &kept {
            assert!(after.contains(row), "{}: {row:?} in {after:?}", desc.id);
        }
        assert!(
            after
                .iter()
                .any(|(key, _, gone)| !gone && kept.iter().all(|k| k.0 != *key)),
            "{}: a new row for the new signal in {after:?}",
            desc.id
        );
    }
}

/// Every event a stack sends is declared, and no event carries a frequency of 0.
#[test]
fn results_and_events_are_declared() {
    for d in hf::descriptors() {
        let decl = declaration(d.id);
        let seconds = match d.id {
            "sstv" => 40.0,
            "olivia" | "contestia" | "mfsk" | "dominoex" | "thor" => 25.0,
            _ => 15.0,
        };
        let r = run(&harness(d)
            .seconds(seconds)
            .set(&GEN_NOISE, NoiseKind::Cn0)
            .set(&GEN_CN0_DBHZ, 70.0));
        for e in &r.events {
            assert!(
                decl.events().iter().any(|k| k.kind == e.kind()),
                "{}: {} is not declared",
                d.id,
                e.kind()
            );
            assert!(
                e.freq_hz != 0.0,
                "{}: {} without a frequency",
                d.id,
                e.kind()
            );
        }
    }
}

#[test]
fn the_registry_has_every_decoder() {
    let mut r = sdrkit_graph::Registry::new();
    hf::register(&mut r);
    for name in [
        "hfkbd.front",
        "hfkbd.rtty",
        "hfkbd.psk",
        "hfkbd.cw",
        "hfkbd.morse",
        "hfkbd.olivia",
        "hfkbd.contestia",
        "hfkbd.mfsk",
        "hfkbd.dominoex",
        "hfkbd.thor",
        "hfkbd.hell",
        "hfkbd.mt63",
        "hfkbd.fsq",
        "hfkbd.ifkp",
        "hfkbd.sstv",
        "hfkbd.sitor",
        "hfkbd.ardop",
        "hfkbd.ros",
        "hfkbd.throb",
        "hfkbd.qrss",
        "hfkbd.cp16",
        "hfkbd.mfsktx",
        "hfkbd.oliviatx",
        "hfkbd.contestiatx",
        "hfkbd.mt63tx",
        "hfkbd.ardoptx",
    ] {
        let c = r.make(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(c.info().name, name);
    }
}

#[test]
fn every_stack_has_a_verify_case() {
    for d in hf::descriptors() {
        let hooks = d.verify.expect("verify hooks");
        let cases = (hooks.cases)(&sdrkit_core::verify::VerifyOptions {
            seconds: 10.0,
            clean: true,
            snr_db: 20.0,
        });
        assert!(!cases.is_empty(), "{}", d.id);
    }
}

#[test]
fn table_facts() {
    assert_eq!(hf::mfsk::VARIANTS.len(), 11);
    assert_eq!(hf::domino::Family::Thor.variants().len(), 10);
    assert_eq!(hf::domino::Family::DominoEx.variants().len(), 6);
    assert_eq!(hf::hell::VARIANTS.len(), 7);
    assert_eq!(hf::olivia::Geometry::new(32, 1000.0).bw, 1000.0);
    assert_eq!(hf::olivia::Geometry::new(8, 250.0).bw, 250.0);
    assert!((hf::mfsk::variant("MFSK16").unwrap().bandwidth() - 15.0 * 15.625).abs() < 1e-9);
    assert!((hf::mfsk::variant("MFSK32").unwrap().bandwidth() - 15.0 * 31.25).abs() < 1e-9);
    assert!(
        (hf::domino::Family::Thor
            .variant("THOR 100")
            .unwrap()
            .bandwidth()
            - 1800.0)
            .abs()
            < 1e-9
    );
    assert!(
        (hf::domino::Family::DominoEx
            .variant("DominoEX 16")
            .unwrap()
            .bandwidth()
            - 18.0 * 15.625)
            .abs()
            < 1e-9
    );
    for (w, t) in [(500.0, 0.2), (1000.0, 0.1), (2000.0, 0.05)] {
        assert!((hf::mt63::SYMBOL as f64 / (2.0 * w) - t).abs() < 1e-12);
    }
}

fn declaration(id: &str) -> sdrkit_core::ResultsDecl {
    hf::typed::shape(id).results_decl()
}

/// The data file holds the dials of the slow stacks; the stacks hold none of them.
#[test]
fn the_dial_file_has_its_rows() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/hfkbd/slow_dials.csv");
    let text = std::fs::read_to_string(path).expect("the data file");
    let rows: Vec<Vec<&str>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .skip(1)
        .map(|l| l.split(',').collect())
        .collect();
    assert_eq!(rows.len(), 5);
    for r in &rows {
        let d = hf::descriptors()
            .into_iter()
            .find(|d| d.id == r[0])
            .expect("a stack of the family");
        let hz: f64 = r[2].parse().unwrap();
        assert!(hz >= d.tuning.min_hz && hz <= d.tuning.max_hz, "{}", d.id);
        assert_ne!(hz, d.tuning.default_hz, "{}", d.id);
    }
}

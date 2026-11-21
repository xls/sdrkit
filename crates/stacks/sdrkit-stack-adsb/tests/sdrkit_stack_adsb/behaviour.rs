//! Tests of single behaviours: country ranges, automatic gain, CPR zone changes, declarations, filters and the table.

use super::common::*;
use sdrkit_algo_fec::rs::{codes as rs_codes, Rs};
use sdrkit_core::{Complex32, Generator, Results};
use sdrkit_graph::Component;
use sdrkit_stack_adsb::checks::{CheckStats, FrameChecker, Msg};
use sdrkit_stack_adsb::demod::{DemodStats, Slice};
use sdrkit_stack_adsb::generator::*;
use sdrkit_stack_adsb::message::Frame;
use sdrkit_stack_adsb::table::Tracker;
use sdrkit_stack_adsb::uat::demod::UatCodec;
use serde_json::json;

/// Frames given to the checks and the tracker as sliced frames, and the tracker's results.
struct Bench {
    c: FrameChecker,
    t: Tracker,
}

impl Bench {
    fn new() -> Bench {
        Bench {
            c: FrameChecker::default(),
            t: Tracker::default(),
        }
    }
    fn feed(&mut self, f: &Frame, t: f64) {
        if let Some(a) = self
            .c
            .check_slice(&Slice::given(f.data()), t, Some(-30.0), Some(20.0))
        {
            self.t.take(&Msg::Frame(a));
        }
    }
    fn report(&mut self, t: f64) -> &Results {
        let st = CheckStats {
            demod: DemodStats {
                time: t,
                noise_dbfs: -60.0,
                ..Default::default()
            },
            ..self.c.stats().clone()
        };
        self.t.take(&Msg::Stats(st));
        self.t.results().unwrap()
    }
}

/// The cell `key` of the aircraft `icao` in the published table.
fn cell(r: &Results, icao: &str, key: &str) -> serde_json::Value {
    view(r)["aircraft"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["icao"] == icao)
        .map_or(serde_json::Value::Null, |a| a[key].clone())
}

/// Colombia's block is 0x0AC000 to 0x0ACFFF, 4,096 addresses (ICAO Annex 10 Vol. III, Part I, Chapter 9, Appendix Table 9-1).
#[test]
fn colombia_has_4096_addresses() {
    let mut b = Bench::new();
    b.feed(&df11(5, 0x0ACFFF, 0), 1.0);
    b.feed(&df11(5, 0x0AD000, 0), 1.1);
    let r = b.report(1.5);
    assert_eq!(cell(r, "0ACFFF", "country"), "Colombia");
    assert_ne!(cell(r, "0AD000", "country"), "Colombia");
}

/// Automatic gain is a fixed high gain in every stack of the family: the replies are microseconds long, and an AGC set from the
/// quiet between them leaves the distant aircraft in the converter's noise.
#[test]
fn auto_gain_is_a_fixed_high_gain() {
    for d in sdrkit_stack_adsb::descriptors() {
        assert_eq!(d.auto_gain, sdrkit_core::AutoGain::High, "{}", d.id);
    }
}

/// DO-260B A.1.7.7: a pair whose even and odd latitudes are in different NL zones is not decoded (the aircraft crossed a zone
/// boundary between the two frames).
#[test]
fn cpr_pair_with_different_nl_is_refused() {
    // NL changes from 59 to 58 at 10.4704713 degrees.
    assert_eq!(sdrkit_util_geo::cpr::nl(10.469), 59);
    assert_eq!(sdrkit_util_geo::cpr::nl(10.472), 58);
    let a = 0x4840D6;
    let alt = sdrkit_stack_adsb::codes::encode_ac12_25ft(30000).unwrap();
    let mut b = Bench::new();
    b.feed(&es_frame(17, 5, a, me_ident('A', 3, "KLM1023")), 1.0);
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt, false, 10.469, 50.0)),
        1.1,
    );
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt, true, 10.472, 50.0)),
        1.6,
    );
    let r = b.report(2.0);
    assert!(
        cell(r, "4840D6", "lat").is_null(),
        "{:?}",
        cell(r, "4840D6", "lat")
    );
    // The same pair in one zone decodes.
    let mut b = Bench::new();
    b.feed(&es_frame(17, 5, a, me_ident('A', 3, "KLM1023")), 1.0);
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt, false, 10.474, 50.0)),
        1.1,
    );
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt, true, 10.476, 50.0)),
        1.6,
    );
    let r = b.report(2.0);
    assert!((cell(r, "4840D6", "lat").as_f64().unwrap() - 10.476).abs() < 1e-3);
}

/// The aircraft identification uses the ICAO 6-bit set (Annex 10 Vol. IV 3.1.2.9.1.2) of `sdrkit-util-text`: codes that the AIS
/// set maps to a character (0 is `@`, 27 is `[`) are not characters here.
#[test]
fn identification_characters_are_the_icao_set() {
    use sdrkit_stack_adsb::codes::{callsign, callsign_defined, encode_callsign};
    assert_eq!(callsign(encode_callsign("KLM1023")), "KLM1023");
    // The first character's code set to 0.
    let at = encode_callsign(" ABC") & ((1 << 42) - 1);
    assert!(!callsign_defined(at));
    let bracket = 27u64 << 42 | encode_callsign(" ABC") & ((1 << 42) - 1);
    assert!(!callsign_defined(bracket));
    assert_eq!(callsign(bracket), " ABC");
}

/// Every correction the UAT decoder accepts produces a code word: decoding the result again corrects nothing. Words with more
/// errors than the code corrects are refused or decode to a code word.
#[test]
fn uat_rs_accepted_corrections_are_code_words() {
    let mut codec = UatCodec::default();
    let mut rs = Rs::new(rs_codes::UAT_48_34).unwrap();
    let mut rng = Rng::new(7);
    let mut accepted = 0;
    for trial in 0..3000 {
        let mut payload: Vec<u8> = (0..34).map(|_| rng.below(256) as u8).collect();
        payload[0] |= 0x08; // a long frame: payload type not 0
        let mut word = codec.encode_downlink(&payload).unwrap();
        let errors = 1 + trial % 12;
        for _ in 0..errors {
            let i = rng.below(48) as usize;
            word[i] ^= 1 + rng.below(255) as u8;
        }
        if let Some((_, out, _)) = codec.decode_downlink(&word) {
            accepted += 1;
            let mut again = codec.encode_downlink(&out).unwrap();
            let d = rs.decode(&mut again, &[]).unwrap();
            assert_eq!(d.corrected, 0, "trial {trial}");
        }
    }
    assert!(accepted > 1000, "{accepted}");
}

/// The UAT codes of `sdrkit-algo-fec` give the parity of the DO-282B example frame: its code word decodes with
/// nothing to correct.
#[test]
fn uat_parity_of_the_example_frame() {
    use sdrkit_stack_adsb::uat::generator::EXAMPLE_PAYLOAD;
    let mut codec = UatCodec::default();
    let word = codec.encode_downlink(&EXAMPLE_PAYLOAD).unwrap();
    assert_eq!(word.len(), 30);
    let (kind, out, n) = codec
        .decode_downlink(&[word.as_slice(), &[0u8; 18]].concat())
        .unwrap();
    assert_eq!(
        (kind, out.as_slice(), n),
        (
            sdrkit_stack_adsb::uat::demod::UatKind::Short,
            &EXAMPLE_PAYLOAD[..],
            0
        )
    );
}

/// The event `adsb.aircraft` is declared with its fields, and every event the receiver raises is of a declared kind.
#[test]
fn aircraft_events_are_declared_and_emitted() {
    use sdrkit_core::publish::keys;
    use sdrkit_stack_adsb::typed::{AircraftChange, AircraftSeen, AIRCRAFT_SEEN};
    let decl = sdrkit_stack_adsb::typed::shape("adsb").results_decl();
    let declared = |d: &sdrkit_core::ResultsDecl, e: &sdrkit_core::Event| {
        d.events().iter().any(|k| k.kind == e.kind())
    };
    assert!(decl.events().iter().any(|k| k.kind == "adsb.aircraft"));
    let rate = 4e6;
    let mut g = AdsbGenerator::airspace(rate, AirspaceOptions::default(), 2);
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 3.0) as usize];
    g.generate(&mut x);
    add_noise(&mut x, rate, 30.0, 2);
    let mut rx = receiver(&[]);
    at_station(&mut rx, CENTRE.0, CENTRE.1);
    let r = run(&mut rx, rate, &x, &[65536]);
    let ev: Vec<_> = r
        .output
        .events
        .iter()
        .filter(|e| e.is(&AIRCRAFT_SEEN))
        .collect();
    assert!(
        ev.iter()
            .filter(|e| e.at(&AircraftSeen::EVENT) == Some(AircraftChange::New))
            .count()
            >= 8
    );
    assert!(ev
        .iter()
        .any(|e| e.get(&keys::POSITION).is_some() && e.at(&AircraftSeen::DISTANCE_KM).is_some()));
    for e in &r.output.events {
        assert!(declared(&decl, e), "{}", e.kind());
    }
    // The other stacks' events are of declared kinds too.
    for id in ["uat", "modeac", "uplink1030"] {
        let d = sdrkit_stack_adsb::descriptors()
            .into_iter()
            .find(|d| d.id == id)
            .unwrap();
        let rate = d.tuning.rate_hz;
        let mut g = (d.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, config(d, &[]).settings).with_seed(1))
        .unwrap();
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * 1.5) as usize];
        g.generate(&mut x);
        let mut rx = (d.make_receiver)(&config(d, &[])).unwrap();
        let out = feed_all(rx.as_mut(), rate, &x, 65536);
        assert!(!out.events.is_empty(), "{id}");
        let decl = sdrkit_stack_adsb::typed::shape(id).results_decl();
        for e in &out.events {
            assert!(declared(&decl, e), "{id}: {}", e.kind());
        }
    }
}

/// What each stack publishes is what it declares: every field and table of its results is in the declaration, with the same
/// columns.
#[test]
fn results_match_their_declarations() {
    for d in sdrkit_stack_adsb::descriptors() {
        let rate = d.tuning.rate_hz;
        let mut g = (d.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, config(d, &[]).settings).with_seed(1))
        .unwrap();
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * 1.5) as usize];
        g.generate(&mut x);
        let mut rx = (d.make_receiver)(&config(
            d,
            &[("modeac", json!(true))]
                .iter()
                .filter(|_| d.id == "adsb")
                .cloned()
                .collect::<Vec<_>>(),
        ))
        .unwrap();
        rx.set_station(&sdrkit_core::Station::at(sdrkit_util_geo::Position::new(
            CENTRE.0, CENTRE.1,
        )));
        feed_all(rx.as_mut(), rate, &x, 65536);
        let decl = sdrkit_stack_adsb::typed::shape(d.id).results_decl();
        let got = Owner::results(rx.as_ref()).decl();
        for f in got.fields().fields() {
            assert!(decl.field_id(&f.key).is_some(), "{}: field {}", d.id, f.key);
        }
        for t in got.tables() {
            let mine = decl
                .table_id(&t.key)
                .and_then(|id| decl.table(id))
                .unwrap_or_else(|| panic!("{}: table {}", d.id, t.key));
            assert_eq!(mine.columns, t.columns, "{}: table {}", d.id, t.key);
        }
    }
}

/// The UAT demodulator filters a wideband input before it changes the rate. A strong tone at +2.5 MHz in an 8 Msps input lies
/// outside the UAT channel. Without an anti-alias filter it folds to +0.42 MHz, inside the channel, and the frame is lost.
#[test]
fn uat_wideband_input_rejects_an_out_of_band_tone() {
    use sdrkit_stack_adsb::uat::demod::{UatDemodulator, UatKind};
    use sdrkit_stack_adsb::uat::generator::{UatBurst, EXAMPLE_PAYLOAD};
    let rate = 8e6;
    let burst = UatBurst::downlink(100e-6, &EXAMPLE_PAYLOAD, 1.0, 0.0, 0.3);
    let lead = (100e-6 * rate) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); lead];
    x.extend(burst.samples(rate));
    x.resize(x.len() + 10 * lead, Complex32::new(0.0, 0.0));
    let w = std::f64::consts::TAU * 2.5e6 / rate;
    for (i, v) in x.iter_mut().enumerate() {
        *v += Complex32::from_polar(3.0, (w * i as f64) as f32);
    }
    let mut d = UatDemodulator::new(rate);
    let mut frames = Vec::new();
    for c in x.chunks(4096) {
        d.feed(c, &mut frames);
    }
    assert_eq!(frames.len(), 1, "{frames:?}");
    assert_eq!(frames[0].kind, UatKind::Short);
    assert_eq!(frames[0].bytes, EXAMPLE_PAYLOAD.to_vec());
    assert!(
        (frames[0].time - 100e-6 - 36.0 / sdrkit_stack_adsb::uat::demod::BIT_RATE).abs() < 2e-6,
        "{}",
        frames[0].time
    );
}

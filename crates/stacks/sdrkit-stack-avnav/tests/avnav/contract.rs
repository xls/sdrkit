//! The stack contract: descriptors, declarations, the registry, the rate check, retunes, settings, commands, events, and the
//! verify cases run the way a host runs them.

use crate::common::{get, rows, run};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::owner;
use sdrkit_core::verify::{Outcome, VerifyOptions};
use sdrkit_core::{
    Choice, CommandSet, ConfigError, InputInfo, LockState, SettingError, StackConfig, Value, Values,
};
use sdrkit_dev_test::Harness;
use sdrkit_graph::DecoderCommand;
use sdrkit_stack_avnav::carrier::BfoAudio;
use sdrkit_stack_avnav::typed::{
    DgpsFields, IdentFields, VorFields, DGPS_ALMANAC, DGPS_CORRECTIONS, DGPS_MESSAGES, IDENTS,
    NDB_SCAN,
};
use sdrkit_stack_avnav::vor::ToFrom;
use sdrkit_stack_avnav::{descriptors, registry, DGPS, DME, NDB, VOR};

#[test]
fn descriptors_are_complete() {
    let d = descriptors();
    let ids: Vec<&str> = d.iter().map(|m| m.id).collect();
    assert_eq!(ids, ["ndb", "vor", "ils", "dme", "loran", "dgps"]);
    for m in &d {
        assert_eq!(m.group, "Navigation");
        assert!(m.make_generator.is_some(), "{}", m.id);
        assert!(
            m.tuning.min_hz <= m.tuning.default_hz && m.tuning.default_hz <= m.tuning.max_hz,
            "{}",
            m.id
        );
        assert!(m.tuning.min_rate_hz <= m.tuning.rate_hz);
        for s in m.submodes {
            assert!(m.tuning.min_hz <= s.default_hz && s.default_hz <= m.tuning.max_hz);
        }
        // Every setting reaches the graph or belongs to the test signal, and every stack has the clear command.
        let schema = (m.settings)();
        let values = Values::defaults(&schema);
        let mut rx = (m.make_receiver)(&StackConfig::new(values.clone())).unwrap();
        for (i, s) in schema.settings().iter().enumerate() {
            let id = sdrkit_core::SettingId(i as u16);
            let r = owner::apply(rx.as_mut(), id, values.value(id).unwrap());
            assert!(r.is_ok(), "{}: {} refused: {r:?}", m.id, s.field.key);
        }
        assert!(rx.commands().id("clear").is_ok(), "{}", m.id);
        assert!(serde_json::to_string(&*schema).is_ok());
    }
    for m in &d {
        assert!(m.verify.is_some(), "{}", m.id);
    }
}

#[test]
fn every_component_is_registered() {
    let r = registry();
    for name in [
        "avnav.am_carrier",
        "avnav.bfo",
        "avnav.tone_keyer",
        "avnav.key_select",
        "avnav.morse_decoder",
        "avnav.carrier_keyer",
        "avnav.beacon_scan",
        "avnav.word_guard",
        "avnav.ident_folder",
        "avnav.ndb_report",
        "avnav.vor_bearing",
        "avnav.vor_report",
        "avnav.ils_ddm",
        "avnav.ils_report",
        "avnav.marker",
        "avnav.pulse_detect",
        "avnav.pulse_pairs",
        "avnav.dme_ident",
        "avnav.dme_report",
        "avnav.loran_tracker",
        "avnav.loran_report",
        "avnav.rtcm_framer",
        "avnav.rtcm_select",
        "avnav.dgps_report",
    ] {
        let c = r.make(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(c.info().name, name);
    }
}

#[test]
fn a_rate_too_low_is_refused() {
    for m in descriptors() {
        let mut rx =
            (m.make_receiver)(&StackConfig::new(Values::defaults(&(m.settings)()))).unwrap();
        let low = m.tuning.min_rate_hz / 2.0;
        let r = rx.configure(&InputInfo::new(
            low,
            m.tuning.default_hz,
            m.tuning.default_hz,
        ));
        assert!(
            matches!(r, Err(ConfigError::RateTooLow { .. })),
            "{}: {r:?}",
            m.id
        );
        assert!(rx
            .results()
            .status()
            .summary
            .contains("needs a sample rate"));
        let r = rx.configure(&InputInfo::new(
            m.tuning.rate_hz,
            m.tuning.default_hz,
            m.tuning.default_hz,
        ));
        assert!(r.is_ok(), "{}: {r:?}", m.id);
    }
}

fn feed_vor(h: &Harness) -> Box<dyn sdrkit_core::Receiver> {
    feed(h, 128e3, 113.1e6)
}

/// A receiver of `h` that has decoded its test signal at `rate` and `freq`.
fn feed(h: &Harness, rate: f64, freq: f64) -> Box<dyn sdrkit_core::Receiver> {
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, freq, freq).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    for c in x.chunks(8192) {
        out.clear();
        rx.feed(c, &mut out);
    }
    rx
}

#[test]
fn a_retune_and_the_clear_command_forget_the_station() {
    let h = Harness::new(&VOR).rate(128e3).seconds(12.0);
    let mut rx = feed_vor(&h);
    assert_eq!(get(rx.results(), &IdentFields::IDENT), Some("SYD"));
    rx.retuned(&InputInfo::new(128e3, 114.0e6, 114.0e6).recording());
    assert_eq!(get(rx.results(), &IdentFields::IDENT), None);
    assert_ne!(rx.results().status().lock_state, LockState::Locked);

    let mut rx = feed_vor(&h);
    assert_eq!(get(rx.results(), &IdentFields::IDENT), Some("SYD"));
    let id = rx.commands().id("clear").unwrap();
    let args = DecoderCommand::commands()
        .def(DecoderCommand::Clear.id())
        .unwrap()
        .new_args();
    owner::invoke(rx.as_mut(), id, &args).unwrap();
    assert_eq!(get(rx.results(), &IdentFields::IDENT), None);
}

/// A change of frequency keeps what belongs to no station: the band scan of NDB, and the almanac, the message counts
/// and the last text of DGPS. The station itself is forgotten. A retune that the host did not order does this, and so
/// does the forget call of the host on a second receiver.
#[test]
fn a_change_of_frequency_keeps_the_lists_and_forgets_the_station() {
    let kept = sdrkit_stack_avnav::typed::shape("ndb").kept_keys().unwrap();
    assert_eq!(kept, ["table scan"]);

    let h = Harness::new(&NDB).rate(250e3).freq(344e3).seconds(18.0);
    let mut rx = feed(&h, 250e3, 344e3);
    let scan = rows(rx.results(), &NDB_SCAN).len();
    assert_eq!(get(rx.results(), &IdentFields::IDENT), Some("BK"));
    assert!(scan > 1 && !rows(rx.results(), &IDENTS).is_empty());
    let check = |rx: &dyn sdrkit_core::Receiver, step: &str| {
        assert_eq!(rows(rx.results(), &NDB_SCAN).len(), scan, "{step}");
        assert_eq!(get(rx.results(), &IdentFields::IDENT), None, "{step}");
        assert_eq!(get(rx.results(), &IdentFields::LAST_IDENT), None, "{step}");
        assert!(rows(rx.results(), &IDENTS).is_empty(), "{step}");
    };
    rx.retuned(&InputInfo::new(250e3, 344e3, 346e3).recording());
    check(rx.as_ref(), "retune");
    let mut rx = feed(&h, 250e3, 344e3);
    rx.forget();
    check(rx.as_ref(), "forget");

    let h = Harness::new(&DGPS).rate(96e3).freq(304e3).seconds(12.0);
    let mut rx = feed(&h, 96e3, 304e3);
    let r = rx.results();
    let frames = get(r, &DgpsFields::FRAMES_OK).unwrap_or(0);
    let messages = rows(r, &DGPS_MESSAGES).len();
    assert_eq!(get(r, &DgpsFields::STATION_ID), Some(333));
    assert_eq!(rows(r, &DGPS_ALMANAC).len(), 1);
    assert!(frames > 0 && messages > 0 && !rows(r, &DGPS_CORRECTIONS).is_empty());
    let check = |rx: &dyn sdrkit_core::Receiver, step: &str| {
        let r = rx.results();
        assert_eq!(get(r, &DgpsFields::FRAMES_OK), Some(frames), "{step}");
        assert_eq!(rows(r, &DGPS_MESSAGES).len(), messages, "{step}");
        assert_eq!(rows(r, &DGPS_ALMANAC).len(), 1, "{step}");
        assert_eq!(
            get(r, &DgpsFields::TEXT),
            Some("SDRKIT TEST BEACON"),
            "{step}"
        );
        assert_eq!(get(r, &DgpsFields::STATION_ID), None, "{step}");
        assert_eq!(get(r, &DgpsFields::BASE_STATION), None, "{step}");
        assert!(rows(r, &DGPS_CORRECTIONS).is_empty(), "{step}");
    };
    rx.retuned(&InputInfo::new(96e3, 304e3, 305e3).recording());
    check(rx.as_ref(), "retune");
    let mut rx = feed(&h, 96e3, 304e3);
    rx.forget();
    check(rx.as_ref(), "forget");
}

#[test]
fn settings_are_strict_and_live() {
    let h = Harness::new(&VOR)
        .rate(128e3)
        .seconds(4.0)
        .set(&sdrkit_stack_avnav::vor::GEN_RADIAL, 200.0);
    let mut rx = feed_vor(&h);
    let obs = rx.schema().id("obs").unwrap();
    owner::apply(rx.as_mut(), obs, &Value::F64(20.0)).unwrap();
    let r = rx.results();
    assert_eq!(get(r, &VorFields::TO_FROM), Some(ToFrom::To));
    let cdi = get(r, &VorFields::CDI_DEG).unwrap_or(f64::NAN);
    assert!(cdi.abs() < 1.0, "{:?}", r.fields());
    // A value out of range is refused, never clamped.
    assert!(matches!(
        owner::apply(rx.as_mut(), obs, &Value::F64(400.0)),
        Err(SettingError::OutOfRange { .. })
    ));
    // A DME threshold and an NDB beat note are live too.
    let mut dme = Harness::new(&DME).receiver().unwrap();
    let t = dme.schema().id("threshold").unwrap();
    owner::apply(dme.as_mut(), t, &Value::F64(15.0)).unwrap();
    let mut ndb = Harness::new(&NDB).receiver().unwrap();
    let audio = ndb.schema().id("audio").unwrap();
    owner::apply(ndb.as_mut(), audio, &Value::Choice(BfoAudio::Cw.index())).unwrap();
    assert!(owner::apply(ndb.as_mut(), audio, &Value::Choice(9)).is_err());
    assert!(ndb.schema().id("nonsense").is_err());
}

/// Every stack's test signal produces events, and none carries a frequency of 0.
#[test]
fn events_have_a_frequency() {
    for m in descriptors() {
        let seconds = if m.id == "dgps" { 10.0 } else { 6.0 };
        let r = run(&Harness::new(m).seconds(seconds));
        for e in &r.events {
            assert!(e.freq_hz > 0.0, "{}: {} at 0 Hz", m.id, e.kind());
        }
        if m.id != "loran" {
            assert!(!r.events.is_empty(), "{}: no events", m.id);
        }
    }
}

#[test]
fn the_registered_cases_pass() {
    let o = VerifyOptions {
        seconds: 4.0,
        clean: false,
        snr_db: 20.0,
    };
    for m in descriptors() {
        let hooks = m.verify.unwrap();
        for case in (hooks.cases)(&o) {
            let mut h = Harness::new(m).seconds(case.seconds).seed(7);
            if let Some(s) = &case.submode {
                h = h.submode(s);
                if let Some(sm) = m.submodes.iter().find(|x| x.id == s.as_str()) {
                    h = h.freq(sm.default_hz);
                }
            }
            if let Some(f) = case.freq_hz {
                h = h.freq(f);
            }
            h = h.with_settings(&case.settings);
            if case.offset_hz != 0.0 {
                h = h.set(&GEN_OFFSET_HZ, case.offset_hz);
            }
            if let Some(snr) = case.snr_db {
                h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, snr);
            }
            let r = run(&h);
            if case.expect_lock {
                let lock_state = r.results.status().lock_state;
                assert!(
                    matches!(lock_state, LockState::Locked | LockState::Tracking),
                    "{} / {}: {lock_state:?}",
                    m.id,
                    case.name
                );
            }
            if let Some(a) = case.expect_audio {
                assert_eq!(!r.audio.is_empty(), a, "{} / {}: audio", m.id, case.name);
            }
            for line in (case.check.unwrap())(&r.results) {
                assert_eq!(
                    line.outcome,
                    Outcome::Pass,
                    "{} / {}: {} {}",
                    m.id,
                    case.name,
                    line.key,
                    line.detail
                );
            }
        }
    }
}

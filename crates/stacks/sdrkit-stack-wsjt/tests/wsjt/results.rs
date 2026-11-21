//! What the receiver publishes against its declaration: every event it writes is a declared kind, no event with a known
//! frequency carries 0, and the fields and tables of the components appear under their ids.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::Value;
use sdrkit_stack_wsjt::generator::stations;
use sdrkit_stack_wsjt::table::WATERFALL_SERIES;
use sdrkit_stack_wsjt::typed::{display_declaration as declaration, shape_of};
use sdrkit_stack_wsjt::{Mode, Sub};

#[test]
fn every_result_and_event_is_declared() {
    let sub = Sub::new(Mode::Ft8);
    let st: Vec<St> = stations(Mode::Ft8)
        .iter()
        .map(|s| (s.0, s.1, -6.0 + s.2, s.3, false))
        .collect();
    let iq = iq_of(&audio_of(sub, &st, 31.0, true, 12));
    let mut rx = receiver(None, &[("spots", Value::Bool(true))], 8000.0, 0.0);
    let ev = feed(&mut rx, &iq, 800);
    let d = declaration();
    let mut kinds = std::collections::BTreeSet::new();
    for e in &ev {
        assert!(
            d.events().iter().any(|k| k.kind == e.kind()),
            "{} is not declared",
            e.kind()
        );
        assert!(e.freq_hz > 1e6, "{} at {} Hz", e.kind(), e.freq_hz);
        kinds.insert(e.kind());
    }
    assert!(
        kinds.contains("wsjt.decode") && kinds.contains("wsjt.spot"),
        "{kinds:?}"
    );
    // Every field and table of the receiver, the components' under their ids.
    let r = rx.results();
    assert!(!table(r, "decodes").is_empty());
    assert!(!table(r, "stations").is_empty());
    let mine = shape_of(None).results_decl();
    for f in r.decl().fields().fields() {
        assert!(mine.field_id(&f.key).is_some(), "{} is not declared", f.key);
    }
    for t in r.decl().tables() {
        assert!(mine.table_id(&t.key).is_some(), "{} is not declared", t.key);
    }
    assert!(field(r, "dec_ft8.decodes").is_some() && field(r, "wide.channels").is_some());
    for d in sdrkit_stack_wsjt::descriptors() {
        let decl = shape_of(Mode::from_id(d.id)).results_decl();
        assert!(!decl.is_empty(), "{}", d.id);
    }
}

#[test]
fn js8_results_are_declared() {
    let d = declaration();
    let mut rx = receiver(Some(Mode::Js8), &[], 8000.0, 0.0);
    let _ = feed(&mut rx, &iq_of(&noise(20 * 8000, SIGMA, 4)), 8000);
    let r = rx.results();
    assert!(r.decl().series_id(WATERFALL_SERIES).is_some());
    for k in ["js8_state", "utc", "time_source"] {
        assert!(d.field_id(k).is_some() && field(r, k).is_some(), "{k}");
    }
}

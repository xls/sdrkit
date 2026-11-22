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
    let d = declaration(None);
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
    let d = declaration(None);
    let mut rx = receiver(Some(Mode::Js8), &[], 8000.0, 0.0);
    let _ = feed(&mut rx, &iq_of(&noise(20 * 8000, SIGMA, 4)), 8000);
    let r = rx.results();
    assert!(r.decl().series_id(WATERFALL_SERIES).is_some());
    for k in ["js8_state", "utc", "time_source"] {
        assert!(d.field_id(k).is_some() && field(r, k).is_some(), "{k}");
    }
}

#[test]
fn a_slot_stack_declares_only_what_its_modes_fill() {
    // (table, the stacks that have it). Every other slot stack has no such table.
    let tables: [(&str, &[&str]); 5] = [
        ("wspr_spots", &["wsjt", "wspr", "fst4w"]),
        ("js8_frames", &["wsjt", "js8"]),
        ("js8_messages", &["wsjt", "js8"]),
        ("js8_stations", &["wsjt", "js8"]),
        (
            "decodes",
            &[
                "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4", "fst4w",
                "msk144",
            ],
        ),
    ];
    let fields: [(&str, &[&str]); 3] = [
        ("js8_frames", &["wsjt", "js8"]),
        ("js8_state", &["wsjt", "js8"]),
        (
            "decodes_total",
            &[
                "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4", "fst4w",
                "msk144",
            ],
        ),
    ];
    let slot = [
        "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4", "fst4w",
        "msk144", "js8",
    ];
    for id in slot {
        let mode = Mode::from_id(id);
        let stack = shape_of(mode).results_decl();
        let display = declaration(mode);
        for (t, with) in tables {
            assert_eq!(
                stack.table_id(t).is_some(),
                with.contains(&id),
                "{id} table {t}"
            );
            assert_eq!(
                display.table_id(t).is_some(),
                with.contains(&id),
                "{id} display table {t}"
            );
        }
        for (f, with) in fields {
            assert_eq!(
                stack.field_id(f).is_some(),
                with.contains(&id),
                "{id} field {f}"
            );
        }
        let has = |kind: &str| stack.events().iter().any(|k| k.kind == kind);
        assert_eq!(has("js8.frame"), ["wsjt", "js8"].contains(&id), "{id}");
        assert_eq!(has("wsjt.decode"), id != "js8", "{id}");
        assert_eq!(
            stack.series_id(WATERFALL_SERIES).is_some(),
            ["wsjt", "js8"].contains(&id),
            "{id}"
        );
        // The stack has every table, field and event of its display.
        for t in display.tables() {
            assert!(stack.table_id(&t.key).is_some(), "{id} {}", t.key);
        }
        for e in display.events() {
            assert!(has(&e.kind), "{id} {}", e.kind);
        }
    }
}

#[test]
fn the_summary_of_ft8_says_what_its_numbers_count() {
    let sub = Sub::new(Mode::Ft8);
    let st: Vec<St> = stations(Mode::Ft8)
        .iter()
        .map(|s| (s.0, s.1, -6.0 + s.2, s.3, false))
        .collect();
    let iq = iq_of(&audio_of(sub, &st, 31.0, true, 12));
    let mut rx = receiver(Some(Mode::Ft8), &[], 8000.0, 0.0);
    let _ = feed(&mut rx, &iq, 800);
    let r = rx.results();
    let summary = r.status().summary.clone();
    let n = table(r, "decodes").len();
    assert!(n > 0);
    assert!(
        summary.starts_with("FT8: ")
            && summary.ends_with(&format!("in the last slot, {n} decoded")),
        "{summary}"
    );
    // The FT8 stack has no table that only another mode fills.
    assert!(
        r.decl().table_id("wspr_spots").is_none() && r.decl().table_id("js8_stations").is_none()
    );
}

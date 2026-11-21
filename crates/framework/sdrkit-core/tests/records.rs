//! Records, results, publishing and events, through the public API.

use sdrkit_core::publish::{field, keys, Declare, Field, Published, TableDef};
use sdrkit_core::record::RecordSeed;
use sdrkit_core::{Choice, EventCatalog, EventDef, Record, RecordDecl, Value};
use serde::de::DeserializeSeed;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// A test choice.
    pub enum Region {
        One = ("r1", "Region 1"),
        Two = ("r2", "Region 2"),
        Three = ("r3", "Region 3"),
    }
}

const STATION: Field<field::Text> = Field::new("station", "Station", "");
const REGION: Field<field::Choice<Region>> = Field::new("region", "Region", "");
const BLOCKS: Field<field::I64> = Field::new("blocks", "Blocks", "");
const SQUAWK: Field<field::Text> = Field::new("squawk", "Squawk", "");
const ALTITUDE_M: Field<field::F64> = Field::new("altitude_m", "Altitude", "m");
static AIRCRAFT: EventDef =
    EventDef::new("test.aircraft", "Aircraft seen", &[&SQUAWK, &ALTITUDE_M]);
static SPOT: EventDef = EventDef::spot("test.spot", "Spot");
const HEARD: TableDef = TableDef {
    key: "heard",
    label: "Heard",
    columns: &[&STATION, &keys::SNR_DB],
    cap: 2,
};

fn fields() -> [&'static dyn Declare; 4] {
    [&STATION, &REGION, &BLOCKS, &keys::SNR_DB]
}

fn record() -> Record {
    Record::new(Arc::new(RecordDecl::of(&fields())))
}

#[test]
fn a_choice_field_holds_the_index_of_its_variant() {
    let mut r = record();
    r.set(&REGION, Region::Three);
    let id = r.decl().id("region").unwrap();
    assert_eq!(r.value(id), Some(&Value::Choice(2)));
    assert_eq!(r.get(&REGION), Some(Region::Three));
    assert_eq!(r.text(id).unwrap().to_string(), "Region 3");
    assert_eq!(
        r.decl().field(id).unwrap().choices.len(),
        Region::CHOICES.len()
    );
}

#[test]
fn a_choice_list_built_at_run_time_is_written_by_index() {
    let plans = vec![
        sdrkit_core::ChoiceDecl::new("eu", "Europe"),
        sdrkit_core::ChoiceDecl::new("us", "USA"),
    ];
    let decl = RecordDecl::new(vec![sdrkit_core::FieldDecl::with_choices(
        "plan", "Plan", plans,
    )])
    .unwrap();
    let mut r = Record::new(Arc::new(decl));
    let id = r.decl().id("plan").unwrap();
    let us = r.decl().field(id).unwrap().choice_index("us").unwrap();
    r.set_value(id, &Value::Choice(us)).unwrap();
    assert_eq!(r.text(id).unwrap().to_string(), "USA");
    assert!(r.set_value(id, &Value::Choice(2)).is_err());
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "not declared")]
fn a_write_to_an_undeclared_field_panics_in_debug_builds() {
    let mut r = record();
    r.set(&keys::FRAMES_OK, 3);
}

#[test]
fn an_event_from_its_definition_is_matched_and_read() {
    let e = AIRCRAFT
        .event(12.0, 1_090e6)
        .set(&SQUAWK, "7000")
        .set(&ALTITUDE_M, 3_000.0);
    assert!(e.is(&AIRCRAFT));
    assert!(!e.is(&SPOT));
    assert_eq!(e.get(&SQUAWK), Some("7000"));
    assert_eq!(e.get(&ALTITUDE_M), Some(3_000.0));
    let spot = SPOT.event(1.0, 14_074_000.0).set(&keys::CALLSIGN, "VK2ABC");
    assert!(spot.is(&SPOT));
    assert_eq!(spot.get(&keys::CALLSIGN), Some("VK2ABC"));
}

#[test]
fn a_table_keeps_the_newest_rows_within_its_cap() {
    let mut p = Published::new(&[]).with_tables(&[&HEARD]);
    p.row_mut(&HEARD, "a").unwrap().set(&STATION, "QFA1");
    p.row_mut(&HEARD, "b").unwrap().set(&STATION, "VOZ2");
    p.row_mut(&HEARD, "a").unwrap().set(&keys::SNR_DB, 4.0);
    p.row_mut(&HEARD, "c").unwrap().set(&STATION, "JST3");
    let id = p.results().decl().table_id("heard").unwrap();
    let t = p.results().table(id).unwrap();
    assert_eq!(t.len(), 2);
    assert!(t.row("b").is_none());
    let keys: Vec<&str> = t.rows_newest_first().map(|(k, _)| k).collect();
    assert_eq!(keys, ["c", "a"]);
    assert_eq!(t.row("a").unwrap().get(&keys::SNR_DB), Some(4.0));
    assert_eq!(t.row("c").unwrap().get(&keys::SNR_DB), None);
}

const MANY: TableDef = TableDef {
    key: "many",
    label: "Many",
    columns: &[&BLOCKS],
    cap: 40,
};

#[test]
fn a_table_follows_inserts_moves_removals_and_its_cap_in_any_order() {
    let mut p = Published::new(&[]).with_tables(&[&MANY]);
    let id = p.results().decl().table_id("many").unwrap();
    // Newest last, as the table must keep them.
    let mut model: std::collections::VecDeque<(String, i64)> = std::collections::VecDeque::new();
    let mut seed = 7u64;
    for step in 0..20_000i64 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let key = format!("k{}", (seed >> 33) % 70);
        if (seed >> 20).is_multiple_of(4) {
            let there = model.iter().position(|(k, _)| *k == key);
            assert_eq!(p.remove_row(&MANY, &key), there.is_some());
            if let Some(i) = there {
                model.remove(i);
            }
        } else {
            p.row_mut(&MANY, &key).unwrap().set(&BLOCKS, step);
            match model.iter().position(|(k, _)| *k == key) {
                Some(i) => {
                    model.remove(i);
                }
                None if model.len() == 40 => {
                    model.pop_front();
                }
                None => {}
            }
            model.push_back((key, step));
        }
        let t = p.results().table(id).unwrap();
        assert_eq!(t.len(), model.len());
        let got: Vec<(&str, i64)> = t
            .rows_newest_first()
            .map(|(k, r)| (k, r.get(&BLOCKS).unwrap()))
            .collect();
        let want: Vec<(&str, i64)> = model.iter().rev().map(|(k, v)| (k.as_str(), *v)).collect();
        assert_eq!(got, want, "step {step}");
        for (k, v) in &model {
            assert_eq!(t.row(k).and_then(|r| r.get(&BLOCKS)), Some(*v));
        }
        if step % 997 == 0 {
            let mut copy = Published::new(&[]).with_tables(&[&MANY]);
            copy.results_mut().copy_from(p.results());
            assert_eq!(copy, p);
            if let Some((k, _)) = model.front() {
                assert!(copy.results().table(id).unwrap().row(k).is_some());
            }
        }
    }
}

#[test]
fn published_writes_fields_and_rows() {
    let mut p = Published::new(&fields()).with_tables(&[&HEARD]);
    p.set(&STATION, "Triple M");
    p.opt(&keys::SNR_DB, Some(12.25));
    p.row(
        &HEARD,
        "a",
        vec![Value::Text("VK2".into()), Value::F64(3.0)],
    );
    let r = p.results();
    assert_eq!(r.fields().get(&STATION), Some("Triple M"));
    let id = r.decl().table_id("heard").unwrap();
    assert_eq!(
        r.table(id).unwrap().row("a").unwrap().get(&keys::SNR_DB),
        Some(3.0)
    );
    p.clear_rows(&HEARD);
    assert!(p.results().table(id).unwrap().is_empty());
    let json = serde_json::to_string(p.results()).unwrap();
    assert!(
        json.contains("\"station\":{\"kind\":\"text\",\"value\":\"Triple M\"}"),
        "{json}"
    );
}

#[test]
fn a_record_reads_back_from_its_map_form() {
    let mut r = record();
    r.set(&STATION, "Triple M");
    r.set(&REGION, Region::Two);
    let json = serde_json::to_string(&r).unwrap();
    let mut de = serde_json::Deserializer::from_str(&json);
    let back = RecordSeed(r.decl()).deserialize(&mut de).unwrap();
    assert_eq!(back, r);
}

#[test]
fn the_catalog_returns_each_kind_an_id_and_shares_its_fields() {
    static DEFS: [&EventDef; 2] = [&AIRCRAFT, &SPOT];
    let catalog = EventCatalog::new(&DEFS).unwrap();
    let id = catalog.id_of(&AIRCRAFT).unwrap();
    let a = catalog
        .event(id, 1.0, 1_090e6)
        .unwrap()
        .set(&SQUAWK, "7000");
    let b = catalog.event(id, 2.0, 1_090e6).unwrap();
    assert!(a.is(&AIRCRAFT));
    assert_eq!(a.kind_id(), Some(id));
    assert!(std::sync::Arc::ptr_eq(a.fields.decl(), b.fields.decl()));
    assert_eq!(catalog.id("test.other"), None);
    static TWICE: [&EventDef; 2] = [&AIRCRAFT, &AIRCRAFT];
    assert!(EventCatalog::new(&TWICE).is_err());
}

#[test]
fn the_log_is_capped_and_a_retune_keeps_results() {
    let mut p = Published::new(&fields());
    let r = p.results_mut();
    r.set_log_cap(3);
    for i in 0..5 {
        r.log(format!("line {i}"));
    }
    assert_eq!(r.log_lines().front().map(String::as_str), Some("line 2"));
    r.update_status(|s| {
        s.lock_state = sdrkit_core::LockState::Locked;
        true
    });
    p.set(&STATION, "Triple M");
    p.results_mut().lost_signal();
    assert_eq!(
        p.results().status().lock_state,
        sdrkit_core::LockState::Searching
    );
    assert_eq!(p.results().fields().get(&STATION), Some("Triple M"));
}

#[test]
fn clone_from_keeps_the_text_buffers() {
    let mut a = Published::new(&fields()).with_tables(&[&HEARD]);
    a.set(&STATION, "Triple M");
    a.row(
        &HEARD,
        "a",
        vec![Value::Text("VK2".into()), Value::F64(3.0)],
    );
    let mut b = a.results().clone();
    let id = b.decl().fields().id("station").unwrap();
    let ptr = |r: &sdrkit_core::Results| match r.fields().value(id) {
        Some(Value::Text(t)) => t.as_ptr(),
        _ => std::ptr::null(),
    };
    let before = ptr(&b);
    a.set(&STATION, "Triple J");
    b.clone_from(a.results());
    assert_eq!(ptr(&b), before);
    assert_eq!(&b, a.results());
}

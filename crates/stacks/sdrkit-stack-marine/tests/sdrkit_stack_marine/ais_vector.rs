//! The AIS conformance vector, and the declaration of every event the stack emits.

use crate::common::*;
use sdrkit_stack_marine::ais::display::{AisNmeaEvent, NMEA};
use sdrkit_stack_marine::AIS;

#[test]
fn the_ais_vector_decodes_as_listed() {
    let Some(rec) = vector("ais_gpsd_250k") else {
        return;
    };
    let exp = expected("ais_gpsd_250k");
    let mut rx = receiver(&AIS, None, &[]);
    configure(rx.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let ev = run(rx.as_mut(), &rec.samples, 8192);
    let sentences: Vec<String> = ev
        .iter()
        .filter(|e| e.is(&NMEA))
        .map(|e| e.at(&AisNmeaEvent::SENTENCE).unwrap().to_string())
        .collect();
    let r = rx.results();
    println!("{sentences:#?}");
    assert_eq!(
        count(r, "frames_ok"),
        exp["expect"]["good_messages"].as_u64().unwrap()
    );
    assert_eq!(
        count(r, "frames_bad"),
        exp["expect"]["bad_bursts"].as_u64().unwrap()
    );
    assert_eq!(
        count(r, "stations"),
        exp["expect"]["stations"].as_u64().unwrap()
    );
    assert_eq!(sentences.len(), 14);
    for b in exp["bursts"].as_array().unwrap() {
        if let Some(s) = b["nmea"].as_str() {
            if b.get("expect").is_some() {
                assert!(!sentences.iter().any(|x| x == s));
            } else {
                assert!(sentences.iter().any(|x| x == s), "missing {s}");
            }
        }
    }
    let off = r.status().offset_hz.unwrap();
    assert!(off.abs() < 150.0, "{off}");
    // Every station of the list is in the table, with its nine digits.
    let table = r.table(r.decl().table_id("stations").unwrap()).unwrap();
    for m in exp["expect"]["stations_list"].as_array().unwrap() {
        let id = format!("{:09}", m.as_u64().unwrap());
        assert!(table.row(&id).is_some(), "station {id}");
    }
    // Every event is declared, with its kind.
    let decl = sdrkit_stack_marine::typed::shape("ais").results_decl();
    let kinds: std::collections::BTreeSet<&str> = ev.iter().map(|e| e.kind()).collect();
    assert!(
        kinds.contains("ais.vessel") && kinds.contains("ais.nmea"),
        "{kinds:?}"
    );
    check_events(&decl, &ev);
    assert!(!r.points().is_empty());
}

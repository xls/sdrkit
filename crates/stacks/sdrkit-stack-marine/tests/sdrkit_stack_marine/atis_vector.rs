//! The ATIS vector.

use crate::common::*;
use sdrkit_core::publish::keys;
use sdrkit_core::Value;
use sdrkit_stack_marine::atis::{AtisMessageEvent, AtisMessageRow, AtisStationRow, MESSAGE};
use sdrkit_stack_marine::ATIS;

#[test]
fn the_atis_vector_decodes_as_listed() {
    let Some(rec) = vector("atis_inland_48k") else {
        return;
    };
    let exp = expected("atis_inland_48k")["expect"].clone();
    let mut rx = receiver(&ATIS, None, &[]);
    configure(rx.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let ev = run(rx.as_mut(), &rec.samples, 4096);
    check_events(
        &sdrkit_stack_marine::typed::shape("atis").results_decl(),
        &ev,
    );
    let r = rx.results();
    let msgs: Vec<_> = ev.iter().filter(|e| e.is(&MESSAGE)).collect();
    for m in &msgs {
        println!("{:?}", m.fields);
    }
    println!("{}", r.status().summary);
    let got: Vec<String> = msgs
        .iter()
        .filter(|m| m.at(&AtisMessageEvent::CHECK_OK) == Some(true))
        .map(|m| m.get(&keys::SOURCE).unwrap().to_string())
        .collect();
    let want: Vec<String> = exp["messages_in_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(got, want);
    let stations = rows(r, "stations");
    assert_eq!(stations.len(), 5);
    assert_eq!(count(r, "frames_bad"), 0);
    for f in exp["fields"].as_array().unwrap() {
        let s = stations
            .iter()
            .find(|s| s.at(&AtisStationRow::DIGITS) == f["digits"].as_str())
            .expect("station");
        assert_eq!(s.get(&keys::CALLSIGN), f["callsign"].as_str());
        assert_eq!(s.at(&AtisStationRow::MMSI), f["mmsi"].as_str());
        assert_eq!(s.at(&AtisStationRow::COUNTRY), f["country"].as_str());
    }
    let m0 = rows(r, "messages").into_iter().next().unwrap();
    assert_eq!(
        (
            m0.at(&AtisMessageRow::CHECK_OK),
            m0.at(&AtisMessageRow::STANDARD_Z)
        ),
        (Some(true), Some(true))
    );
    let Some(b) = m0.at(&AtisMessageRow::BRANCH_HZ) else {
        panic!(
            "{:?}",
            m0.value(m0.decl().id("branch_hz").unwrap())
                .unwrap_or(&Value::None)
        );
    };
    assert!(b == 0.0 || b == 500.0, "{b}");
}

//! The marine HF conformance vectors: NAVTEX, DSC on MF/HF and VHF, weather fax.

use crate::common::*;
use sdrkit_core::publish::keys;
use sdrkit_core::{Event, Receiver};
use sdrkit_stack_marine::marine::{DscCallEvent, NavtexMessageEvent, DSC_CALL, NAVTEX_MESSAGE};
use sdrkit_stack_marine::MARINE;

fn run_vector(name: &str) -> Option<(Box<dyn Receiver>, Vec<Event>)> {
    let rec = vector(name)?;
    let mut rx = receiver(&MARINE, None, &[]);
    configure(rx.as_mut(), rec.rate_hz, rec.centre_hz, rec.centre_hz);
    let ev = run(rx.as_mut(), &rec.samples, 4096);
    check_events(
        &sdrkit_stack_marine::typed::shape("marine").results_decl(),
        &ev,
    );
    Some((rx, ev))
}

#[test]
fn navtex_vector() {
    let exp = expected("navtex_518k_12k");
    let Some((rx, ev)) = run_vector("navtex_518k_12k") else {
        return;
    };
    let msgs: Vec<_> = ev.iter().filter(|e| e.is(&NAVTEX_MESSAGE)).collect();
    let m = exp["expect"]["message"].clone();
    let complete: Vec<_> = msgs
        .iter()
        .filter(|e| e.at(&NavtexMessageEvent::COMPLETE) == Some(true))
        .collect();
    assert_eq!(complete.len(), 1);
    let c = complete[0];
    for (k, key) in [
        ("header", "header"),
        ("text", "text"),
        ("station", "source"),
        ("subject", "subject"),
        ("number", "number"),
        ("subject_name", "subject_name"),
    ] {
        assert_eq!(field_json(c, key), m[k], "{k}");
    }
    assert_eq!(field_json(c, "errors"), 0);
    for e in &msgs {
        assert_eq!(e.at(&NavtexMessageEvent::HEADER), Some("XA01"));
    }
    let r = rx.results();
    let shift = num(r, "shift_hz").unwrap();
    assert!((shift - 170.0).abs() <= 10.0, "{shift}");
    let baud = num(r, "symbol_rate_hz").unwrap();
    assert!((baud - 100.0).abs() <= 1.0, "{baud}");
    assert_eq!(count(r, "navtex_good"), 1);
    assert_eq!(rows(r, "navtex").len(), 1);
}

fn check_calls(name: &str) {
    let exp = expected(name);
    let Some((rx, ev)) = run_vector(name) else {
        return;
    };
    let calls: Vec<_> = ev.iter().filter(|e| e.is(&DSC_CALL)).collect();
    for c in &calls {
        println!("{}", c.get(&keys::TEXT).unwrap_or_default());
    }
    let want = exp["expect"]["calls_in_order"].as_array().unwrap();
    assert!(calls.len() >= want.len(), "{} calls", calls.len());
    for (w, got) in want.iter().zip(calls.iter()) {
        for (k, v) in w.as_object().unwrap() {
            match k.as_str() {
                "lat" | "lon" => assert!(
                    (field_json(got, "position")[k].as_f64().unwrap() - v.as_f64().unwrap()).abs()
                        < 1e-6,
                    "{k}"
                ),
                "from" => assert_eq!(&field_json(got, "source"), v),
                "to" => assert_eq!(&field_json(got, "target"), v),
                _ => assert_eq!(&field_json(got, k), v, "{name} {k} in {:?}", got.fields),
            }
        }
        if name.contains("vhf") {
            assert_eq!(field_json(got, "vhf"), true);
        }
    }
    assert!(calls
        .iter()
        .all(|c| c.at(&DscCallEvent::ECC_OK) == Some(true)));
    let r = rx.results();
    assert_eq!(rows(r, "dsc").len(), calls.len());
    assert_eq!(count(r, "dsc_good"), calls.len() as u64);
    // The distress call's position is on the map.
    assert!(r.points().contains_key("dsc:232123456"));
    shows_row(r, "dsc:232123456", "dsc");
}

/// Asserts that map point `id` shows a row of table `table` that is there.
fn shows_row(r: &sdrkit_core::Results, id: &str, table: &str) {
    let p = &r.points()[id];
    let link = p.row.as_ref().expect("a row");
    assert_eq!(Some(link.table), r.decl().table_id(table), "{id}");
    let t = r.table(link.table).expect("the table");
    assert!(t.row(&link.key).is_some(), "{id}: no row {}", link.key);
}

#[test]
fn dsc_mfhf_vector() {
    check_calls("dsc_mfhf_12k");
}

#[test]
fn dsc_vhf_vector() {
    check_calls("dsc_vhf_ch70_48k");
}

#[test]
fn wefax_vector() {
    let exp = expected("wefax_ioc576_120lpm_12k")["expect"].clone();
    let Some((rx, _)) = run_vector("wefax_ioc576_120lpm_12k") else {
        return;
    };
    let r = rx.results();
    assert_eq!(text(r, "fax_state").as_deref(), Some("stopped"));
    assert_eq!(num(r, "ioc"), exp["ioc"].as_f64());
    assert_eq!(num(r, "lpm"), exp["lpm"].as_f64());
    assert_eq!(num(r, "width"), exp["width"].as_f64());
    assert_eq!(num(r, "lines"), exp["height"].as_f64());
    assert!(count(r, "phasing_lines") >= 12);
    let black = num(r, "black_level_hz").unwrap();
    assert!((black - 1500.0).abs() <= 10.0, "{black}");
    assert!(num(r, "slant_ppm").unwrap().abs() <= 3.0);
}

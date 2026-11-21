//! The recorded test vector (tests/vectors/adsb): 1 s of the airspace at 2 Msps decodes as expected.json says.

use super::common::*;
use serde_json::Value as J;

fn rx_at_centre() -> sdrkit_graph::StackGraph {
    let mut rx = receiver(&[]);
    at_station(&mut rx, CENTRE.0, CENTRE.1);
    rx
}

fn close(a: &J, b: &J, tol: f64, what: &str, icao: &str) {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => assert!((x - y).abs() <= tol, "{icao} {what}: got {x}, expected {y}"),
        (None, None) => {}
        _ => panic!("{icao} {what}: got {a}, expected {b}"),
    }
}

#[test]
fn the_recording_decodes_as_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("adsb/adsb_airspace_2m") else {
        return;
    };
    let expected = sdrkit_dev_test::sigmf::read_expected("adsb").expect("expected.json");
    let txs = vector_transmissions();
    let tx: Vec<_> = txs.iter().map(|t| t.0.clone()).collect();
    let mut rx = rx_at_centre();
    let run = run(&mut rx, rec.rate_hz, &rec.samples, &[65536]);
    let (got, phantoms) = check_off(&run.frames, &tx);
    let decoded = got.iter().filter(|&&g| g).count();
    let decoded_not_over = txs
        .iter()
        .zip(&got)
        .filter(|(t, g)| !t.0.overlapped && **g)
        .count();
    eprintln!(
        "decoded {decoded} of {} ({decoded_not_over} not overlapped), phantoms {phantoms}",
        tx.len()
    );
    assert_eq!(phantoms, 0);
    assert!(
        decoded >= 380 && decoded_not_over >= 380,
        "{decoded} {decoded_not_over}"
    );
    // Every DF of the mix decodes.
    let by_df = &expected["counts"]["conforming_decoded_by_df"];
    for (df, n) in by_df.as_object().unwrap() {
        let df: u8 = df.parse().unwrap();
        let mine = run.frames.iter().filter(|a| a.frame.df() == df).count() as u64;
        assert!(
            mine as f64 >= n.as_u64().unwrap() as f64 * 0.9,
            "DF{df}: {mine} of {n}"
        );
    }
    // Time stamps.
    let mut err = Vec::new();
    for a in &run.frames {
        if let Some(t) = tx.iter().find(|t| same(&a.frame, a.time, t)) {
            err.push((a.time - t.time) * 1e9);
        }
    }
    let rms = (err.iter().map(|e| e * e).sum::<f64>() / err.len() as f64).sqrt();
    eprintln!("time stamp error rms {rms:.1} ns");
    assert!(rms <= 150.0, "{rms}");
    // The summary.
    let d = view(rx.results());
    let noise = d["noise_dbfs"].as_f64().unwrap();
    assert!((-52.0..=-48.0).contains(&noise), "{noise}");
    assert_eq!(d["state"], 2);
    assert_eq!(d["aircraft_count"], 12);
    assert_eq!(d["with_position"], 12);
    let max_range = d["max_range_nm"].as_f64().unwrap();
    assert!((max_range - 186.9).abs() < 2.0, "{max_range}");
    // Messages per second at the last report that a whole window of signal ended before the end (0.75 s).
    let r075 = run
        .reports
        .iter()
        .find(|r| (r["time"].as_f64().unwrap() - 0.75).abs() < 0.01)
        .expect("report at 0.75 s");
    let mps = r075["messages_per_second"].as_f64().unwrap();
    assert!((250.0..=350.0).contains(&mps), "{mps}");
    // Positions by 0.5 s, callsigns by 1.0 s.
    let first_all_pos = run
        .reports
        .iter()
        .find(|r| r["with_position"] == 12)
        .map(|r| r["time"].as_f64().unwrap());
    assert!(first_all_pos.is_some_and(|t| t <= 0.5), "{first_all_pos:?}");
    let all_cs = run.reports.iter().find(|r| {
        r["aircraft"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["callsign"].is_string())
            .count()
            == 12
    });
    assert!(all_cs.is_some_and(|r| r["time"].as_f64().unwrap() <= 1.0));
    // Per aircraft.
    let mine: std::collections::BTreeMap<String, J> = d["aircraft"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["icao"].as_str().unwrap().to_string(), a.clone()))
        .collect();
    for e in expected["aircraft"].as_array().unwrap() {
        let icao = e["icao"].as_str().unwrap();
        let a = mine.get(icao).unwrap_or_else(|| panic!("{icao} missing"));
        for k in [
            "callsign",
            "category",
            "squawk",
            "on_ground",
            "adsb_version",
            "nacp",
            "sil",
            "nic",
            "position_kind",
            "speed_kind",
            "heading_is_track",
        ] {
            assert_eq!(a.get(k).unwrap_or(&J::Null), &e[k], "{icao} {k}");
        }
        close(&a["lat"], &e["lat_deg"], 0.03, "lat", icao);
        close(&a["lon"], &e["lon_deg"], 0.03, "lon", icao);
        close(
            &a["baro_alt_ft"],
            &e["baro_alt_ft"],
            100.0,
            "altitude",
            icao,
        );
        close(&a["speed_kt"], &e["speed_kt"], 3.0, "speed", icao);
        let htol = if e["on_ground"] == true { 3.0 } else { 2.0 };
        close(&a["heading_deg"], &e["heading_deg"], htol, "heading", icao);
        close(
            a.get("vertical_rate_fpm").unwrap_or(&J::Null),
            &e["vertical_rate_fpm"],
            64.0,
            "vertical rate",
            icao,
        );
        close(&a["distance_nm"], &e["distance_nm"], 2.0, "distance", icao);
        close(&a["bearing_deg"], &e["bearing_deg"], 1.0, "bearing", icao);
        close(
            a.get("ias_kt").unwrap_or(&J::Null),
            &e["ias_kt"],
            2.0,
            "IAS",
            icao,
        );
        close(
            a.get("mach").unwrap_or(&J::Null),
            &e["mach"],
            0.008,
            "Mach",
            icao,
        );
        close(
            a.get("selected_alt_ft").unwrap_or(&J::Null),
            &e["selected_alt_ft"],
            32.0,
            "selected altitude",
            icao,
        );
        let msgs = a["messages"].as_f64().unwrap();
        assert!(
            msgs >= 0.85 * e["messages"].as_f64().unwrap(),
            "{icao} messages {msgs}"
        );
        close(&a["level_dbfs"], &e["level_dbfs"], 4.0, "level", icao);
        assert_eq!(a["tisb"], e["tisb"], "{icao} tisb");
    }
}

#[test]
fn without_the_receiver_position_eleven_have_a_position() {
    let Some(rec) = sdrkit_dev_test::read_recording("adsb/adsb_airspace_2m") else {
        return;
    };
    let mut rx = receiver(&[]);
    let _ = run(&mut rx, rec.rate_hz, &rec.samples, &[65536]);
    let d = &view(rx.results());
    assert_eq!(d["aircraft_count"], 12);
    assert_eq!(d["with_position"], 11);
}

#[test]
fn correction_settings_take_effect_on_the_recording() {
    let Some(rec) = sdrkit_dev_test::read_recording("adsb/adsb_airspace_2m") else {
        return;
    };
    let txs: Vec<_> = vector_transmissions().into_iter().map(|t| t.0).collect();
    let mut counts = Vec::new();
    for c in ["off", "1bit", "3bits"] {
        let mut rx = receiver(&[("correction", serde_json::json!(c))]);
        let run = run(&mut rx, rec.rate_hz, &rec.samples, &[65536]);
        let (got, phantoms) = check_off(&run.frames, &txs);
        assert_eq!(phantoms, 0, "{c}");
        counts.push(got.iter().filter(|&&g| g).count());
    }
    eprintln!("decoded with correction off, 1 bit, 3 bits: {counts:?}");
    assert!(counts[0] >= 375);
    assert!(counts[0] <= counts[1] && counts[1] <= counts[2]);
}

//! The recordings made off air in Sydney at 1090 MHz, 4 Msps: the aircraft, their callsigns, squawks, altitudes and positions
//! are those the receiver showed when they were recorded. One recording was made at a fixed gain, one with the automatic gain of
//! the stack (a fixed high gain).

use super::common::*;
use sdrkit_core::Complex32;

/// One aircraft as the table shows it at the end of a recording.
struct Want {
    icao: &'static str,
    callsign: &'static str,
    alt_ft: Option<i64>,
    squawk: &'static str,
    position: Option<(f64, f64)>,
    messages: u64,
}

/// The samples of a recording of 16-bit pairs. `None` when the recording is missing (the family gate never lets it be).
fn samples(name: &str) -> Option<Vec<Complex32>> {
    let raw = std::fs::read(sdrkit_dev_test::recording(name)?).expect("the recording reads");
    Some(
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                let i = i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0;
                let q = i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0;
                Complex32::new(i, q)
            })
            .collect(),
    )
}

/// Decodes a recording and compares the table with `want`, and the number of frames accepted with `frames`.
fn check(name: &str, want: &[Want], frames: usize) {
    let Some(x) = samples(name) else {
        return;
    };
    let mut rx = receiver(&[]);
    let r = run(&mut rx, 4e6, &x, &[65536]);
    assert_eq!(r.frames.len(), frames, "{name}: frames accepted");
    let d = view(rx.results());
    let got = d["aircraft"].as_array().unwrap();
    let mut icaos: Vec<&str> = got.iter().map(|a| a["icao"].as_str().unwrap()).collect();
    icaos.sort_unstable();
    let mut want_icaos: Vec<&str> = want.iter().map(|w| w.icao).collect();
    want_icaos.sort_unstable();
    assert_eq!(icaos, want_icaos, "{name}");
    for w in want {
        let a = got.iter().find(|a| a["icao"] == w.icao).unwrap();
        assert_eq!(
            a["callsign"].as_str().unwrap_or(""),
            w.callsign,
            "{name} {}",
            w.icao
        );
        assert_eq!(
            a["squawk"].as_str().unwrap_or(""),
            w.squawk,
            "{name} {}",
            w.icao
        );
        assert_eq!(a["baro_alt_ft"].as_i64(), w.alt_ft, "{name} {}", w.icao);
        assert_eq!(
            a["messages"].as_u64(),
            Some(w.messages),
            "{name} {}",
            w.icao
        );
        match w.position {
            Some((lat, lon)) => {
                let (la, lo) = (a["lat"].as_f64().unwrap(), a["lon"].as_f64().unwrap());
                assert!(
                    (la - lat).abs() < 1e-5 && (lo - lon).abs() < 1e-5,
                    "{name} {}: {la} {lo}",
                    w.icao
                );
                assert_eq!(a["position_kind"], "global", "{name} {}", w.icao);
            }
            None => assert!(a["lat"].is_null(), "{name} {}", w.icao),
        }
    }
}

#[test]
fn sydney_at_the_automatic_gain() {
    check(
        "real/adsb/adsb_1090_sydney_auto_gain",
        &[
            Want {
                icao: "7C7AA4",
                callsign: "",
                alt_ft: Some(1550),
                squawk: "",
                position: Some((-33.86137, 151.16548)),
                messages: 16,
            },
            Want {
                icao: "7CAD40",
                callsign: "VOZ978",
                alt_ft: Some(4450),
                squawk: "1157",
                position: Some((-33.70418, 151.12642)),
                messages: 40,
            },
            Want {
                icao: "7CAF1E",
                callsign: "VOZ560",
                alt_ft: Some(2700),
                squawk: "3717",
                position: Some((-33.78496, 151.13440)),
                messages: 91,
            },
            Want {
                icao: "7CF7CA",
                callsign: "",
                alt_ft: None,
                squawk: "",
                position: None,
                messages: 3,
            },
        ],
        151,
    );
}

#[test]
fn sydney_at_a_fixed_gain() {
    check(
        "real/adsb/adsb_1090_sydney",
        &[
            Want {
                icao: "7C6B0F",
                callsign: "",
                alt_ft: Some(3775),
                squawk: "",
                position: Some((-33.82494, 150.99520)),
                messages: 23,
            },
            Want {
                icao: "7C6DE4",
                callsign: "QFA480",
                alt_ft: Some(2750),
                squawk: "4076",
                position: Some((-33.74702, 151.12277)),
                messages: 74,
            },
            Want {
                icao: "7C7BC7",
                callsign: "",
                alt_ft: Some(36000),
                squawk: "1023",
                position: None,
                messages: 6,
            },
            Want {
                icao: "7CF7CA",
                callsign: "SSM1",
                alt_ft: Some(0),
                squawk: "7712",
                position: None,
                messages: 18,
            },
            Want {
                icao: "A40CFC",
                callsign: "ASA851",
                alt_ft: Some(8050),
                squawk: "2637",
                position: Some((-33.94061, 151.15685)),
                messages: 141,
            },
        ],
        268,
    );
}

#[test]
fn sydney_with_the_station_returns_distances() {
    let Some(x) = samples("real/adsb/adsb_1090_sydney_auto_gain") else {
        return;
    };
    let mut rx = receiver(&[]);
    // A station in Sydney: every aircraft with a position is within 30 NM of it.
    at_station(&mut rx, -33.87, 151.21);
    run(&mut rx, 4e6, &x, &[65536]);
    let d = view(rx.results());
    let with_pos: Vec<_> = d["aircraft"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["lat"].is_number())
        .collect();
    assert!(with_pos.len() >= 3, "{}", with_pos.len());
    for a in with_pos {
        let nm = a["distance_nm"].as_f64().unwrap();
        assert!(nm > 0.0 && nm < 30.0, "{}: {nm}", a["icao"]);
    }
    assert!(rx
        .results()
        .fields()
        .at(&sdrkit_stack_adsb::typed::TrackerFields::MAX_RANGE_M)
        .is_some());
}

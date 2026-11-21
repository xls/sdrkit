//! ARDOP: the test script with a tuning error and a clock error, the recording, and the frame check and Reed-Solomon code.

use crate::common::*;
use sdrkit_core::Value;
use sdrkit_dev_test::Run;
use sdrkit_stack_hfkbd as hf;
use serde_json::Value as Json;

/// The frames of a run, oldest first, as JSON objects by column key (the type as "0x30").
fn frames(r: &Run) -> Vec<Json> {
    let keys: Vec<String> = hf::typed::ARDOP_FRAMES
        .columns
        .iter()
        .map(|c| c.decl().key.into_owned())
        .collect();
    let mut v: Vec<Json> = rows(r, "frames")
        .into_iter()
        .map(|row| {
            let mut m = serde_json::Map::new();
            for k in &keys {
                let j = match cell(&row, k) {
                    None | Some(Value::None) => continue,
                    Some(Value::I64(i)) if k == "type" => serde_json::json!(format!("0x{i:02X}")),
                    Some(Value::Text(t)) => serde_json::json!(t),
                    Some(Value::Bool(b)) => serde_json::json!(b),
                    Some(Value::I64(i)) => serde_json::json!(i),
                    c => serde_json::json!(number(c)),
                };
                m.insert(k.clone(), j);
            }
            Json::Object(m)
        })
        .collect();
    v.sort_by(|a, b| {
        a["time_s"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["time_s"].as_f64().unwrap())
    });
    v
}

#[test]
fn ardop_script() {
    let h = snr_2500(harness(&hf::ARDOP), 15.0)
        .set(&GEN_OFFSET_HZ, 20.0)
        .set(&GEN_CLOCK_PPM, 50.0)
        .seconds(26.0);
    let r = run(&h);
    let f = frames(&r);
    let names: Vec<&str> = f.iter().map(|f| f["name"].as_str().unwrap()).collect();
    for want in [
        "IDFrame",
        "ConReq500M",
        "ConAck500",
        "Ping",
        "PingAck",
        "4FSK.200.50S.E",
        "DataACK",
        "4FSK.500.100S.E",
        "DataNAK",
        "DISC",
        "8PSK.200.100.E",
    ] {
        assert!(names.contains(&want), "{want}: {names:?}");
    }
    assert_eq!(num(&r, "stations_heard"), 2.0);
    assert_eq!(num(&r, "data_ok"), 2.0);
    for f in &f {
        let off = f["tone_offset_hz"].as_f64().unwrap();
        assert!((off - 20.0).abs() < 4.0, "{off}");
    }
}

#[test]
fn ardop_recording() {
    let name = "hfkbd_ardop_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        let r = recording(&harness(&hf::ARDOP).piece(piece), name);
        let f = frames(&r);
        let want = e["expect"]["frames"].as_array().unwrap();
        assert_eq!(f.len(), want.len());
        for (f, w) in f.iter().zip(want) {
            for (k, v) in w.as_object().unwrap() {
                match (f[k].as_f64(), v.as_f64()) {
                    (Some(a), Some(b)) => assert_eq!(a, b, "{} {k}", w["name"]),
                    _ => assert_eq!(&f[k], v, "{} {k}", w["name"]),
                }
            }
            assert!(in_range(
                f["tone_offset_hz"].as_f64().unwrap(),
                &e["expect"]["tone_offset_hz"]
            ));
        }
        assert_eq!(num(&r, "stations_heard"), 2.0);
        assert_eq!(num(&r, "data_ok"), 2.0);
        assert_eq!(num(&r, "data_bad"), 0.0);
        assert_eq!(num(&r, "unreadable_leaders"), 0.0);
    }
}

#[test]
fn ardop_frame_check_and_reed_solomon() {
    let d: Vec<u8> = (1..=12).collect();
    assert_eq!(hf::ardop::frame_check(&d), 0xD58C);
    let mut rs = hf::ardop::RsCodes::new();
    assert_eq!(rs.parity(&d, 4), [0x2B, 0xEA, 0x74, 0xA3]);
    let mut w = d.clone();
    w.extend(rs.parity(&d, 4));
    let good = w.clone();
    w[3] ^= 0x55;
    w[9] ^= 0x0F;
    assert_eq!(rs.correct(&mut w, 4), Some(2));
    assert_eq!(w, good);
}

//! Known answers and contracts of the codes, the locators and the spot events.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::publish::keys;
use sdrkit_core::LockState;
use sdrkit_stack_wsjt::encode::{self, EncodeError};
use sdrkit_stack_wsjt::proto::codes::CodeStore;
use sdrkit_stack_wsjt::proto::{js8msg, jt65, msg72};
use sdrkit_stack_wsjt::table::spot_event;
use sdrkit_stack_wsjt::typed::SPOT;
use sdrkit_stack_wsjt::{Mode, Sub};
use sdrkit_util_bits::crc::cyclic::{FST4_CRC24, FT8_CRC14, MSK144_CRC13};
use sdrkit_util_bits::pack::to_u64;

/// MSK144 encodes and decodes only with a table that the host loads, and its status says so otherwise.
#[test]
fn msk144_needs_a_loaded_table() {
    let none = CodeStore::default();
    assert!(matches!(
        encode::channel_symbols(&Sub::new(Mode::Msk144), "CQ K1ABC FN42", &none),
        Err(EncodeError::NoCode(_))
    ));
    let mut rx = receiver(Some(Mode::Msk144), &[], 8000.0, 0.0);
    let _ = feed(&mut rx, &iq_of(&noise(16 * 8000, SIGMA, 2)), 8000);
    let st = rx.results().status();
    assert_eq!(st.lock_state, LockState::Off);
    assert!(st.summary.contains("msk144_128_90.alist"), "{}", st.summary);
}

/// JS8 uses the published (174,87) code of `sdrkit-algo-fec`, so its tones equal the vector's, parity included. FST4 and
/// FST4W refuse to encode without their code.
#[test]
fn every_code_is_the_published_one() {
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            sdrkit_dev_test::sigmf::vectors_dir().join("wsjt/encode_vectors.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let js8 = v["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["mode"] == "JS8 Normal")
        .unwrap();
    let frame = js8msg::chars_to_bits(js8["chars"].as_str().unwrap()).unwrap();
    let want: Vec<u8> = js8["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as u8)
        .collect();
    let flags = js8["flags"].as_u64().unwrap() as u8;
    assert_eq!(encode::js8_tones(0, &frame, flags), want);
    let none = CodeStore::default();
    for (m, t) in [
        (Mode::Fst4, "CQ K1ABC FN42"),
        (Mode::Fst4w, "K1ABC FN42 37"),
    ] {
        assert!(matches!(
            encode::channel_symbols(&Sub::new(m), t, &none),
            Err(EncodeError::NoCode(_))
        ));
    }
}

/// The JT65 Reed-Solomon decoder refuses an erasure index of 63 or more.
#[test]
fn jt65_refuses_erasures_out_of_range() {
    let dat = msg72::pack72("CQ K1ABC FN42").unwrap();
    let cw = jt65::encode(&dat);
    assert_eq!(jt65::decode(&cw, &[]).map(|r| r.0), Some(dat));
    assert_eq!(jt65::decode(&cw, &[5, 62]).map(|r| r.0), Some(dat));
    assert!(jt65::decode(&cw, &[63]).is_none());
    assert!(jt65::decode(&cw, &[1000, 3]).is_none());
    assert!(jt65::decode(&cw, &(0..64).collect::<Vec<_>>()).is_none());
}

/// Locators have east-positive longitude, from `sdrkit-util-geo`. Only the packing of the 72-bit message turns it
/// west-positive, as the protocol defines.
#[test]
fn locators_use_east_positive_longitude() {
    // FN42 lies at 71 degrees west: its middle is at longitude -71.
    let (lat, lon) = sdrkit_stack_wsjt::rx::slot::locator_centre("FN42").unwrap();
    assert!((lat - 42.5).abs() < 1e-9 && (lon + 71.0).abs() < 1e-9);
    // The protocol's value: ((70 + 180) / 2) * 180 + 42 + 90 for the west-positive square of FN42 (QEX 2005).
    assert_eq!(msg72::grid_value("FN42"), Some(22_632));
    assert_eq!(msg72::grid_text(22_632).as_deref(), Some("FN42"));
    for g in ["JO62", "PM95", "AA00", "RR99", "IO91", "KO24"] {
        let v = msg72::grid_value(g).unwrap();
        assert_eq!(msg72::grid_text(v).as_deref(), Some(g));
        assert_eq!(
            js8msg::unpack_grid(js8msg::pack_grid(g)).as_deref(),
            Some(g)
        );
    }
}

/// The CRC bits of the encoder vectors equal the shared CRC-14, CRC-13 and CRC-24 of their message bits.
#[test]
fn crc14_crc13_crc24_match_published_vectors() {
    // FT8 "CQ K1ABC FN42": the 77 bits are 0x000000204def1a8a1988 (padded to 80 bits); the vector's tones carry the CRC 0x0B2E.
    let mut bits77 = Vec::new();
    sdrkit_util_bits::pack::hex_to_bits("000000204def1a8a1988", &mut bits77).unwrap();
    bits77.truncate(77);
    assert_eq!(FT8_CRC14.compute(&bits77), 0x0B2E);
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            sdrkit_dev_test::sigmf::vectors_dir().join("wsjt/encode_vectors.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let syms = |x: &serde_json::Value| -> Vec<u8> {
        x["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_u64().unwrap() as u8)
            .collect()
    };
    let (mut n14, mut n13, mut n24) = (0, 0, 0);
    for x in v["vectors"].as_array().unwrap() {
        let s = syms(x);
        match x["mode"].as_str().unwrap() {
            "FT8" => {
                let mut b = Vec::new();
                for (i, &t) in s.iter().enumerate() {
                    if i < 7 || (36..43).contains(&i) || i >= 72 {
                        continue;
                    }
                    let g = encode::GRAY8.iter().position(|&y| y == t).unwrap() as u8;
                    b.extend([g >> 2 & 1, g >> 1 & 1, g & 1]);
                }
                assert_eq!(FT8_CRC14.compute(&b[..77]), to_u64(&b[77..91]));
                n14 += 1;
            }
            "MSK144" => {
                let mut info = s[8..56].to_vec();
                info.extend(&s[64..106]);
                assert_eq!(MSK144_CRC13.compute(&info[..77]), to_u64(&info[77..90]));
                n13 += 1;
            }
            "FST4" | "FST4W" => {
                let k = if x["mode"] == "FST4" { 101 } else { 74 };
                let mut b = Vec::new();
                for w in 0..4 {
                    for &t in &s[38 * w + 8..38 * w + 38] {
                        let g = encode::GRAY4.iter().position(|&y| y == t).unwrap() as u8;
                        b.extend([g >> 1, g & 1]);
                    }
                }
                assert_eq!(FST4_CRC24.compute(&b[..k - 24]), to_u64(&b[k - 24..k]));
                n24 += 1;
            }
            _ => {}
        }
    }
    assert_eq!((n14, n13, n24), (21, 2, 4));
}

/// A spot event carries the standard spot fields, the source and the family.
#[test]
fn spots_use_the_standard_spot_fields() {
    let e = spot_event("FT8", START, 14_074_600.0, -12, "K1ABC", "FN42");
    assert!(e.is(&SPOT));
    assert_eq!(e.get(&keys::CALLSIGN), Some("K1ABC"));
    assert_eq!(e.get(&keys::LOCATOR), Some("FN42"));
    assert_eq!(e.get(&keys::SNR_DB), Some(-12.0));
    assert_eq!(e.get(&keys::FREQ_HZ), Some(14_074_600.0));
    assert_eq!(e.get(&keys::SPOT_MODE), Some("FT8"));
    assert_eq!(e.get(&keys::SOURCE), Some("K1ABC"));
    assert_eq!(e.get(&keys::FAMILY), Some(keys::Family::Message));
}

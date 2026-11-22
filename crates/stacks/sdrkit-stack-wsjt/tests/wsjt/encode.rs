//! The encoder vectors of `tests/vectors/wsjt/encode_vectors.json`, made by an independent encoder. FT8, FT4, WSPR, JT65, JT9,
//! JT4, JS8 and Q65 must be reproduced exactly (Q65 also here: the 78 message bits). For FST4, FST4W and MSK144, which have
//! no code table here, the sync symbols and the information part (the packed message, the CRC and the bit order) must match.

use sdrkit_stack_wsjt::encode::{self, channel_symbols};
use sdrkit_stack_wsjt::proto::codes::CodeStore;
use sdrkit_stack_wsjt::proto::js8msg::{self, Frame};
use sdrkit_stack_wsjt::proto::msg77::{pack77, unpack77, CallHashes};
use sdrkit_stack_wsjt::proto::wsprmsg::{pack50, unpack50, WsprHashes};
use sdrkit_stack_wsjt::{Mode, Sub};
use sdrkit_util_bits::crc::cyclic::{FST4_CRC24, FT8_CRC14, MSK144_CRC13};
use sdrkit_util_bits::pack::to_u64 as get_bits;
use serde_json::Value;

fn vectors() -> Vec<Value> {
    let path = sdrkit_dev_test::sigmf::vectors_dir().join("wsjt/encode_vectors.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    v["vectors"].as_array().unwrap().clone()
}

fn syms(v: &Value) -> Vec<u8> {
    v["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as u8)
        .collect()
}

fn hex_bits(h: &str, n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for c in h.chars() {
        let d = c.to_digit(16).unwrap();
        for b in (0..4).rev() {
            out.push(((d >> b) & 1) as u8);
        }
    }
    out.truncate(n);
    out
}

#[test]
fn exact_modes_reproduce_every_vector() {
    let mut checked = 0;
    for v in vectors() {
        let mode = v["mode"].as_str().unwrap();
        let msg = v["msg"].as_str().unwrap();
        let sub = match mode {
            "FT8" => Sub::new(Mode::Ft8),
            "FT4" => Sub::new(Mode::Ft4),
            "WSPR" => Sub::new(Mode::Wspr),
            "JT65" => Sub::new(Mode::Jt65),
            "JT9" => Sub::new(Mode::Jt9),
            "JT4" => Sub::new(Mode::Jt4),
            _ => continue,
        };
        assert_eq!(
            channel_symbols(&sub, msg, &CodeStore::default()).unwrap(),
            syms(&v),
            "{mode} {msg}"
        );
        if let Some(b) = v["bits77"].as_str() {
            assert_eq!(pack77(msg).unwrap(), hex_bits(b, 77), "{msg} bits77");
            let mut h = CallHashes::default();
            h.learn("PJ4/K1ABC");
            assert_eq!(unpack77(&hex_bits(b, 77), &h).unwrap().text, msg);
        }
        if let Some(b) = v["bits50"].as_str() {
            assert_eq!(
                pack50(msg).unwrap().to_vec(),
                hex_bits(b, 50),
                "{msg} bits50"
            );
            let mut h = WsprHashes::default();
            h.learn("K1ABC");
            assert_eq!(unpack50(&hex_bits(b, 50), &h).unwrap().text, msg);
        }
        checked += 1;
    }
    assert_eq!(checked, 41);
}

/// The 174 code bits of FT8 tones (Gray-decoded data symbols).
fn ft8_bits(t: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    for (i, &x) in t.iter().enumerate() {
        if i < 7 || (36..43).contains(&i) || i >= 72 {
            continue;
        }
        let v = encode::GRAY8.iter().position(|&g| g == x).unwrap() as u8;
        b.extend([v >> 2 & 1, v >> 1 & 1, v & 1]);
    }
    b
}

#[test]
fn crc14_matches_every_ft8_vector() {
    let mut n = 0;
    for v in vectors().iter().filter(|v| v["mode"] == "FT8") {
        let b = ft8_bits(&syms(v));
        assert_eq!(b.len(), 174);
        assert_eq!(
            FT8_CRC14.compute(&b[..77]),
            get_bits(&b[77..91]),
            "{}",
            v["msg"]
        );
        n += 1;
    }
    assert_eq!(n, 21);
}

#[test]
fn codes_without_tables_match_sync_and_information() {
    let none = CodeStore::default();
    let mut n = 0;
    for v in vectors() {
        let mode = v["mode"].as_str().unwrap();
        let msg = v["msg"].as_str().unwrap();
        let want = syms(&v);
        match mode {
            "Q65" => {
                for &p in &encode::Q65_SYNC {
                    assert_eq!(want[p], 0);
                }
                // The 13 message symbols come first among the data symbols: the 77 message bits and a zero.
                let data: Vec<u8> = (0..85)
                    .filter(|i| !encode::Q65_SYNC.contains(i))
                    .map(|i| want[i] - 1)
                    .collect();
                let mut bits = Vec::new();
                for &x in &data[..13] {
                    for k in (0..6).rev() {
                        bits.push(x >> k & 1);
                    }
                }
                assert_eq!(bits[..77], pack77(msg).unwrap()[..], "Q65 {msg}");
                assert_eq!(bits[77], 0);
                assert_eq!(channel_symbols(&Sub::new(Mode::Q65), msg, &none).unwrap(), want, "Q65 {msg}");
                n += 1;
            }
            "FST4" | "FST4W" => {
                let m = if mode == "FST4" {
                    Mode::Fst4
                } else {
                    Mode::Fst4w
                };
                let k = if m == Mode::Fst4 { 101 } else { 74 };
                for w in 0..5 {
                    assert_eq!(want[38 * w..38 * w + 8], encode::FST4_SYNC[w % 2]);
                }
                let mut bits = Vec::new();
                for w in 0..4 {
                    for &t in &want[38 * w + 8..38 * w + 38] {
                        let x = encode::GRAY4.iter().position(|&g| g == t).unwrap() as u8;
                        bits.push(x >> 1);
                        bits.push(x & 1);
                    }
                }
                let (msg_bits, info) = if m == Mode::Fst4 {
                    (
                        pack77(msg).unwrap(),
                        encode::fst4_info(&pack77(msg).unwrap(), false),
                    )
                } else {
                    let p = pack50(msg).unwrap().to_vec();
                    (p.clone(), encode::fst4_info(&p, true))
                };
                // The CRC-24 of the vector's message bits is the vector's own CRC (a known answer).
                let nm = k - 24;
                assert_eq!(
                    FST4_CRC24.compute(&bits[..nm]),
                    get_bits(&bits[nm..k]),
                    "{mode} {msg}"
                );
                assert_eq!(bits[..k], info[..], "{mode} {msg}: message and CRC-24");
                let _ = msg_bits;
                assert!(channel_symbols(&Sub::new(m), msg, &none).is_err());
                n += 1;
            }
            "MSK144" => {
                assert_eq!(want[..8], encode::MSK_SYNC);
                assert_eq!(want[56..64], encode::MSK_SYNC);
                let mut info = want[8..56].to_vec();
                info.extend(&want[64..106]);
                assert_eq!(info.len(), 90);
                assert_eq!(MSK144_CRC13.compute(&info[..77]), get_bits(&info[77..90]));
                assert_eq!(
                    info,
                    encode::msk144_info(&pack77(msg).unwrap()),
                    "MSK144 {msg}"
                );
                assert!(channel_symbols(&Sub::new(Mode::Msk144), msg, &none).is_err());
                n += 1;
            }
            m if m.starts_with("JS8") => {
                let sub = match m {
                    "JS8 Normal" => 0,
                    "JS8 Fast" => 1,
                    _ => 2,
                };
                let chars = v["chars"].as_str().unwrap();
                let flags = v["flags"].as_u64().unwrap() as u8;
                let frame = js8msg::chars_to_bits(chars).unwrap();
                let parsed = Frame::unpack(&frame).unwrap();
                assert_eq!(parsed.display(), msg, "{m}");
                assert_eq!(js8msg::bits_to_chars(&parsed.pack().unwrap()), chars);
                // The (174,87) code of sdrkit-algo-fec reproduces every tone, parity included.
                assert_eq!(encode::js8_tones(sub, &frame, flags), want, "{m} {msg}");
                let mut info = Vec::new();
                for &t in &want[43..72] {
                    info.extend([t >> 2 & 1, t >> 1 & 1, t & 1]);
                }
                assert_eq!(get_bits(&info[72..75]) as u8, flags);
                assert!(js8msg::check87(&info).is_some());
                n += 1;
            }
            _ => {}
        }
    }
    assert_eq!(n, 15);
}

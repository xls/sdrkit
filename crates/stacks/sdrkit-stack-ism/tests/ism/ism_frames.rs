//! Frame layouts of the `ism` protocols that have no test device in the scene generator: each frame is built from the layout,
//! decoded by its protocol's decoder, and checked field by field. A single flipped bit in a checked frame produces no reading,
//! and a frame with too few copies produces no reading.

use sdrkit_core::Value;
use sdrkit_proto_ookdev::{BitRows, Check, Ctx, Reading};
use sdrkit_stack_ism::ism::checks::*;
use sdrkit_stack_ism::ism::protocols::PROTOCOLS;
use sdrkit_util_bits::checksum::{lfsr_digest16, lfsr_digest8, lfsr_digest8_reflect, sum8, xor8};
use sdrkit_util_bits::crc::catalog::CRC16_UMTS;

/// Rows: `empty` empty rows, then `n` identical rows of the first `bits` bits of `bytes`.
fn rows(bytes: &[u8], bits: usize, n: usize, empty: usize) -> BitRows {
    let s: String = (0..bits)
        .map(|i| {
            if (bytes[i / 8] >> (7 - i % 8)) & 1 == 1 {
                '1'
            } else {
                '0'
            }
        })
        .collect();
    let mut v: Vec<&str> = vec![""; empty];
    v.extend(std::iter::repeat_n(s.as_str(), n));
    BitRows::parse(&v.join("/")).unwrap()
}

/// The readings of protocol `num` (1 to 46) from `bb`.
pub fn run(num: usize, bb: &BitRows) -> Vec<Reading> {
    let mut out = Vec::new();
    (PROTOCOLS[num - 1].decode)(bb, &mut Ctx::default(), &mut out);
    out
}

fn one(num: usize, bb: &BitRows) -> Reading {
    let r = run(num, bb);
    assert_eq!(r.len(), 1, "protocol {num}: {r:#?}");
    r.into_iter().next().unwrap()
}

/// The value of the measurement `key` of `r`.
fn value<'a>(r: &'a Reading, key: &str) -> Option<&'a Value> {
    r.fields.iter().find(|m| m.key() == key).map(|m| m.value())
}

/// The measurement `key` of `r` as a number: a flag is 1 or 0.
fn num(r: &Reading, key: &str) -> f64 {
    match value(r, key).unwrap_or_else(|| panic!("{}: no {key} in {}", r.model, r.line())) {
        Value::F64(v) | Value::Duration(v) => *v,
        Value::I64(v) => *v as f64,
        Value::Bool(b) => f64::from(u8::from(*b)),
        v => panic!("{key}: {v:?}"),
    }
}

fn text(r: &Reading, key: &str) -> String {
    match value(r, key).unwrap_or_else(|| panic!("{}: no {key}", r.model)) {
        Value::Text(t) => t.to_string(),
        v => panic!("{key}: {v:?}"),
    }
}

/// °F as °C.
fn f_to_c(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.051, "{a} vs {b}");
}

/// Every single-bit flip of the first `bits` bits produces no reading.
fn flips_bite(num: usize, bytes: &[u8], bits: usize, n: usize, empty: usize) {
    for i in 0..bits {
        let mut b = bytes.to_vec();
        b[i / 8] ^= 0x80 >> (i % 8);
        let r = run(num, &rows(&b, bits, n, empty));
        assert!(r.is_empty(), "protocol {num}, bit {i} flipped: {r:#?}");
    }
}

fn channel(r: &Reading) -> serde_json::Value {
    serde_json::to_value(&r.channel).unwrap()
}

#[test]
fn rubicson_and_solight() {
    // id 0x5A, battery ok, channel field 1, 21.3 °C
    let (b0, b1, b2) = (0x5A, 0x90, 0xD5);
    let c = CRC8_RUBICSON.checksum(&[b0, b1, b2, 0xF0]);
    let f = [b0, b1, b2, 0xF0 | (c >> 4), (c & 0xF) << 4];
    let r = one(2, &rows(&f, 36, 3, 0));
    assert_eq!((&*r.model, &*r.id), ("Rubicson-Temperature", "90"));
    assert_eq!(r.check, Check::Crc);
    assert_eq!(channel(&r), serde_json::json!(2));
    assert_eq!(r.battery_ok, Some(true));
    close(num(&r, "temperature_c"), 21.3);
    // the same frame is not a Nexus
    assert!(run(1, &rows(&f, 36, 3, 0)).is_empty());
    flips_bite(2, &f, 36, 3, 0);
    // Solight TE44: the Rubicson frame, 37 bits exactly
    let r = one(21, &rows(&f, 37, 3, 0));
    assert_eq!(r.model, "Solight-TE44");
    close(num(&r, "temperature_c"), 21.3);
    assert!(run(21, &rows(&f, 36, 3, 0)).is_empty());
}

#[test]
fn acurite_5n1() {
    // channel A (3), id 0x0ABC, message 0x38: 72.0 °F, 48 % humidity
    let raw_t = 720 + 400;
    let mut m = [
        0xC0 | 0x0A,
        0xBC,
        0x40 | 0x38,
        0x00,
        ((raw_t >> 7) & 0x0F) as u8,
        (raw_t & 0x7F) as u8,
        48,
        0,
    ];
    // even byte parity over bytes 2 to n-2: the top bit of each byte is its parity bit
    for x in m.iter_mut().take(7).skip(2) {
        if (*x & 0x7F).count_ones() % 2 == 1 {
            *x |= 0x80;
        }
    }
    m[7] = sum8(&m[..7]);
    let inv: Vec<u8> = m.iter().map(|x| !x).collect();
    let r = one(5, &rows(&inv, 64, 1, 0));
    assert_eq!((&*r.model, &*r.id), ("Acurite-5n1", "2748"));
    assert_eq!(r.check, Check::Checksum);
    assert_eq!(channel(&r), serde_json::json!("A"));
    assert_eq!(r.battery_ok, Some(true));
    close(num(&r, "temperature_c"), f_to_c(72.0));
    close(num(&r, "humidity_pct"), 48.0);
    flips_bite(5, &inv, 64, 1, 0);
}

#[test]
fn acurite_606tx_and_609txc() {
    let mut f = [0x33, 0x80, 0xE7, 0];
    f[3] = lfsr_digest8(&f[..3], 0x98, 0xF1);
    let r = one(9, &rows(&f, 32, 3, 0));
    assert_eq!((&*r.model, &*r.id), ("Acurite-606TX", "51"));
    assert_eq!(r.battery_ok, Some(true));
    close(num(&r, "temperature_c"), 23.1);
    flips_bite(9, &f, 32, 3, 0);

    let mut f = [0x77, 0x00, 0xC8, 50, 0];
    f[4] = sum8(&f[..4]);
    let r = one(10, &rows(&f, 40, 1, 0));
    assert_eq!((&*r.model, &*r.id), ("Acurite-609TXC", "119"));
    assert_eq!(r.battery_ok, Some(true));
    close(num(&r, "temperature_c"), 20.0);
    close(num(&r, "humidity_pct"), 50.0);
    flips_bite(10, &f, 40, 1, 0);
}

#[test]
fn prologue_repeat_rule() {
    let f = [0x9A, 0xB1, 0x0E, 0xA3, 0x70];
    let r = one(11, &rows(&f, 36, 4, 0));
    assert_eq!((&*r.model, &*r.id), ("Prologue-TH", "171"));
    assert_eq!(r.check, Check::Repeat);
    assert_eq!(channel(&r), serde_json::json!(2));
    close(num(&r, "temperature_c"), 23.4);
    close(num(&r, "humidity_pct"), 55.0);
    // fewer than four copies: nothing
    assert!(run(11, &rows(&f, 36, 3, 0)).is_empty());
    // humidity 0xCC is not reported
    let r = one(11, &rows(&[0x9A, 0xB1, 0x0E, 0xAC, 0xC0], 36, 4, 0));
    assert!(value(&r, "humidity_pct").is_none());
}

#[test]
fn kedsum() {
    // id 0x42, battery ok (b1:7-6 = 2), channel field 0, 68.0 °F -> raw 1580 = 0x62C, humidity 0x37 (nibbles swapped)
    let raw = 680 + 900;
    let b1 = 0x80 | (raw & 0xF) as u8;
    let b2 = (((raw >> 8) & 0xF) as u8) | ((((raw >> 4) & 0xF) as u8) << 4);
    let b3 = 0x73; // humidity 0x37 = 55
    let hi = 0x5;
    let lo = CRC4_KEDSUM.checksum(&[0x42, b1, b2, b3]) ^ hi;
    let msg = [0x42, b1, b2, b3, (hi << 4) | lo];
    // 2 leading bits, then 40 message bits
    let mut f = [0u8; 6];
    for i in 0..40 {
        if (msg[i / 8] >> (7 - i % 8)) & 1 == 1 {
            let j = i + 2;
            f[j / 8] |= 0x80 >> (j % 8);
        }
    }
    let r = one(12, &rows(&f, 42, 4, 5));
    assert_eq!((&*r.model, &*r.id), ("Kedsum-TH", "66"));
    assert_eq!(r.battery_ok, Some(true));
    assert_eq!(channel(&r), serde_json::json!(1));
    close(num(&r, "temperature_c"), 20.0);
    close(num(&r, "humidity_pct"), 55.0);
    // the leading empty rows are needed
    assert!(run(12, &rows(&f, 42, 4, 0)).is_empty());
    for i in 2..42 {
        let mut b = f;
        b[i / 8] ^= 0x80 >> (i % 8);
        assert!(run(12, &rows(&b, 42, 4, 5)).is_empty(), "bit {i}");
    }
}

#[test]
fn gt_wt_02() {
    let (b0, b1, b2) = (0x21u8, 0x10u8, 0xC3u8);
    let b3 = 45u8 << 1;
    let nib = |x: u8| (x >> 4) as u32 + (x & 0xF) as u32;
    let sum = (nib(b0) + nib(b1) + nib(b2) + (b3 >> 4) as u32 + (b3 & 0xE) as u32) % 64;
    let f = [b0, b1, b2, b3 | (sum >> 5) as u8, ((sum & 0x1F) as u8) << 3];
    let r = one(13, &rows(&f, 37, 2, 0));
    assert_eq!((&*r.model, &*r.id), ("GT-WT02", "33"));
    assert_eq!(channel(&r), serde_json::json!(2));
    assert_eq!(r.battery_ok, Some(true));
    close(num(&r, "temperature_c"), 19.5);
    close(num(&r, "humidity_pct"), 45.0);
    flips_bite(13, &f, 37, 2, 0);
}

#[test]
fn bresser_3ch() {
    // id 0x9C, channel 1, 70.0 °F, humidity 40
    let mut m = [0x9C, 0x10 | 0x06, 0x40, 40, 0];
    m[4] = m[..4].iter().fold(0u8, |a, &x| a.wrapping_add(x));
    let f: Vec<u8> = m.iter().map(|x| !x).collect();
    let r = one(14, &rows(&f, 40, 3, 0));
    assert_eq!((&*r.model, &*r.id), ("Bresser-3CH", "156"));
    assert_eq!(channel(&r), serde_json::json!(1));
    close(num(&r, "temperature_c"), (70.0 - 32.0) / 1.8);
    close(num(&r, "humidity_pct"), 40.0);
    flips_bite(14, &f, 40, 3, 0);
}

#[test]
fn fine_offset_wh2_family() {
    // WH2: 0xFF then 40 bits; id 0x4B -> b0 = 0x44, b1 high nibble 0xB; -5.2 °C (sign bit 11)
    let mut m = [0x44, 0xB8, 0x34, 61, 0];
    m[4] = CRC8_31.checksum(&m[..4]);
    let mut f = vec![0xFF];
    f.extend(m);
    let r = one(15, &rows(&f, 48, 1, 0));
    assert_eq!((&*r.model, &*r.id), ("Fineoffset-WH2", "75"));
    assert_eq!(r.check, Check::Crc);
    close(num(&r, "temperature_c"), -5.2);
    close(num(&r, "humidity_pct"), 61.0);
    flips_bite(15, &f, 48, 1, 0);
    // WH5: 47 bits, first byte 0xFE, 40 bits from bit 7; raw − 400
    let mut m = [0x44, 0xB1, 0xF4, 0xFF, 0];
    m[4] = CRC8_31.checksum(&m[..4]);
    let mut bits = vec![1u8; 7];
    for i in 0..40 {
        bits.push((m[i / 8] >> (7 - i % 8)) & 1);
    }
    let s: String = bits
        .iter()
        .map(|&b| if b == 1 { '1' } else { '0' })
        .collect();
    let r = one(15, &BitRows::parse(&s).unwrap());
    assert_eq!(r.model, "Fineoffset-WH5");
    close(num(&r, "temperature_c"), 10.0);
    assert!(value(&r, "humidity_pct").is_none());
}

#[test]
fn thermopro_tp11_and_tp12() {
    let mut f = [0x12, 0x30 | 0x1, 0x0E, 0];
    f[3] = lfsr_digest8_reflect(&f[..3], 0x51, 0x04);
    let r = one(16, &rows(&f, 32, 2, 0));
    assert_eq!((&*r.model, &*r.id), ("Thermopro-TP11", "291"));
    close(num(&r, "temperature_c"), (0x10E as f64 - 200.0) * 0.1);
    flips_bite(16, &f, 32, 2, 0);

    // food 25.0 (450 = 0x1C2), barbecue 180.0 (2000 = 0x7D0)
    let mut f = [0x5C, 0xC2, 0x17, 0xD0, 0, 0];
    f[4] = lfsr_digest8_reflect(&f[..4], 0x51, 0x04);
    let r = one(17, &rows(&f, 41, 3, 0));
    assert_eq!((&*r.model, &*r.id), ("Thermopro-TP12", "92"));
    close(num(&r, "temperature_1_c"), 25.0);
    close(num(&r, "temperature_2_c"), 180.0);
}

#[test]
fn ft_004_b_majority_vote() {
    // m0 = 0xF4, temperature 21.0 °C -> (21 + 40) / 0.05 = 1220 = 0x4C4
    let m = [0xF4u8, 0, 0, 0xC4, 0x04, 0];
    let bits: Vec<u8> = (0..46)
        .map(|i| (m[i / 8].reverse_bits() >> (7 - i % 8)) & 1)
        .collect();
    let mut all: Vec<u8> = bits.iter().chain(&bits).chain(&bits).copied().collect();
    // one copy damaged: the vote still holds
    all[50] ^= 1;
    let s: String = all
        .iter()
        .map(|&b| if b == 1 { '1' } else { '0' })
        .collect();
    let r = one(18, &BitRows::parse(&s).unwrap());
    assert_eq!((&*r.model, &*r.id), ("FT-004B", "0"));
    close(num(&r, "temperature_c"), 21.0);
}

#[test]
fn itron_ert_scm_and_idm() {
    let mut f = [0u8; 12];
    f[0] = 0xF9;
    f[1] = 0x53;
    f[2] = 0x02;
    f[3] = (1 << 6) | (7 << 2) | 2;
    f[4..7].copy_from_slice(&[0x01, 0x23, 0x45]);
    f[7..10].copy_from_slice(&[0x12, 0x34, 0x56]);
    let c = CRC16_ERT_SCM.checksum(&f[2..10]);
    f[10] = (c >> 8) as u8;
    f[11] = c as u8;
    let r = one(19, &rows(&f, 96, 1, 0));
    assert_eq!(r.model, "ERT-SCM");
    assert_eq!(r.id, ((2u32 << 23) | 0x123456).to_string());
    close(num(&r, "physical_tamper"), 1.0);
    close(num(&r, "ert_type"), 7.0);
    close(num(&r, "encoder_tamper"), 1.0);
    close(num(&r, "consumption"), 0x012345 as f64);
    // the CRC covers b2..b11
    for i in 16..96 {
        let mut g = f;
        g[i / 8] ^= 0x80 >> (i % 8);
        assert!(run(19, &rows(&g, 96, 1, 0)).is_empty(), "bit {i}");
    }

    let mut b = [0u8; 90];
    b[0] = 0x16;
    b[1] = 0xA3;
    b[2] = 0x1C;
    b[5] = 4;
    b[6] = 7;
    b[7..11].copy_from_slice(&1_550_000_123u32.to_be_bytes());
    b[11] = 9;
    b[27..31].copy_from_slice(&987_654u32.to_be_bytes());
    b[84] = 0x01;
    b[85] = 0x02;
    let c = CRC16_ERT_IDM.checksum(&b[2..88]);
    b[88] = (c >> 8) as u8;
    b[89] = c as u8;
    let mut f = vec![0x55, 0x55];
    f.extend(b);
    let r = one(20, &rows(&f, f.len() * 8, 1, 0));
    assert_eq!((&*r.model, &*r.id), ("ERT-IDM", "1550000123"));
    close(num(&r, "last_consumption"), 987_654.0);
    close(num(&r, "interval_count"), 9.0);
    close(num(&r, "transmit_offset"), 258.0);
    let mut g = f.clone();
    g[40] ^= 0x10;
    assert!(run(20, &rows(&g, g.len() * 8, 1, 0)).is_empty());
}

#[test]
fn kerui_states() {
    for (cmd, state, batt) in [
        (0xA, "motion", true),
        (0xE, "open", true),
        (0x7, "close", true),
        (0xB, "tamper", true),
        (0x5, "water", true),
        (0xF, "battery low", false),
    ] {
        let m = [0x12u8, 0x34, 0x50 | cmd];
        let f = [!m[0], !m[1], !m[2], 0x80];
        let r = one(23, &rows(&f, 25, 9, 0));
        assert_eq!((&*r.model, &*r.id), ("Kerui-Security", "12345"));
        assert_eq!(text(&r, "state"), state);
        assert_eq!(r.battery_ok, Some(batt));
        assert!(
            run(23, &rows(&f, 25, 8, 0)).is_empty(),
            "nine copies needed"
        );
    }
    // an unknown command is rejected
    let f = [!0x12u8, !0x34, !0x51, 0x80];
    assert!(run(23, &rows(&f, 25, 9, 0)).is_empty());
}

#[test]
fn springfield_soil() {
    let (b0, b1, b2) = (0x4C, 0x10, 0xBE);
    let x = b0 ^ b1 ^ b2;
    // moisture 60 %: b3 high nibble 6; the low nibble makes the folded XOR zero
    let hi = 6u8;
    let y = x ^ (hi << 4);
    let lo = (y >> 4) ^ (y & 0xF);
    let f = [b0, b1, b2, (hi << 4) | lo, 0];
    let r = one(24, &rows(&f, 36, 3, 0));
    assert_eq!((&*r.model, &*r.id), ("Springfield-Soil", "76"));
    assert_eq!(channel(&r), serde_json::json!(2));
    close(num(&r, "temperature_c"), 19.0);
    close(num(&r, "moisture_pct"), 60.0);
    assert!(
        run(24, &rows(&f, 36, 2, 0)).is_empty(),
        "three copies needed"
    );
}

#[test]
fn schrader_eg53ma4() {
    let mut m = [0x11u8, 0x22, 0x33, 0x44, 0xAB, 0xCD, 0xEF, 100, 77, 0];
    m[9] = sum8(&m[..9]);
    let mut f = vec![0u8; 5];
    f.extend(m);
    let r = one(26, &rows(&f, 120, 1, 0));
    assert_eq!((&*r.model, &*r.id), ("Schrader-EG53MA4", "ABCDEF"));
    close(num(&r, "pressure_kpa"), 250.0);
    close(num(&r, "temperature_c"), f_to_c(77.0));
    for i in 40..120 {
        let mut g = f.clone();
        g[i / 8] ^= 0x80 >> (i % 8);
        assert!(run(26, &rows(&g, 120, 1, 0)).is_empty(), "bit {i}");
    }
}

/// Bits of `bytes` (first `n` bits) as 0/1 values.
fn bits_of(bytes: &[u8], n: usize) -> Vec<u8> {
    (0..n).map(|i| (bytes[i / 8] >> (7 - i % 8)) & 1).collect()
}

/// Manchester pairs: 1 -> 01, 0 -> 10.
fn manchester(bits: &[u8]) -> Vec<u8> {
    bits.iter()
        .flat_map(|&b| if b == 1 { [0, 1] } else { [1, 0] })
        .collect()
}

fn row_of(bits: &[u8]) -> BitRows {
    let s: String = bits
        .iter()
        .map(|&b| if b == 1 { '1' } else { '0' })
        .collect();
    BitRows::parse(&s).unwrap()
}

/// A VDO family row as sent: preamble, Manchester data and a tail, all inverted.
fn vdo_row(pre: &[u8], data: &[u8], tail: usize) -> BitRows {
    let mut v = bits_of(&[0xAA, 0xAA], 16);
    v.extend(bits_of(pre, pre.len() * 8));
    v.extend(manchester(&bits_of(data, data.len() * 8)));
    v.extend(std::iter::repeat_n(1, tail));
    let inv: Vec<u8> = v.iter().map(|b| b ^ 1).collect();
    row_of(&inv)
}

#[test]
fn vdo_hyundai_citroen_abarth() {
    let mut m = [0x01u8, 0xDE, 0xAD, 0xBE, 0xEF, 0x23, 160, 70, 0x00, 0];
    m[9] = CRC8_HYUNDAI.checksum(&m[..9]);
    let bb = vdo_row(&[0xAA, 0xAA, 0xAA, 0xA9], &m, 8);
    let r = one(39, &bb);
    assert_eq!((&*r.model, &*r.id), ("Hyundai-VDO", "deadbeef"));
    close(num(&r, "pressure_kpa"), 160.0 * 1.375);
    close(num(&r, "temperature_c"), 20.0);
    close(num(&r, "repeat"), 3.0);
    let mut bad = m;
    bad[6] ^= 4;
    assert!(run(39, &vdo_row(&[0xAA, 0xAA, 0xAA, 0xA9], &bad, 8)).is_empty());

    // Citroen: XOR of b1..b9 is zero
    let mut m = [0x00u8, 0x12, 0x34, 0x56, 0x78, 0x51, 180, 72, 0x00, 0, 0];
    m[9] = xor8(&m[1..9]);
    let bb = vdo_row(&[0xAA, 0xA9], &m, 8);
    let r = one(36, &bb);
    assert_eq!((&*r.model, &*r.id), ("Citroen", "12345678"));
    close(num(&r, "pressure_kpa"), 180.0 * 1.364);
    close(num(&r, "temperature_c"), 22.0);
    close(num(&r, "repeat"), 1.0);
    close(num(&r, "flags"), 5.0);
    let mut bad = m;
    bad[3] ^= 0x10;
    assert!(run(36, &vdo_row(&[0xAA, 0xA9], &bad, 8)).is_empty());

    // Abarth: XOR of b0..b8 is zero, a row of 151 to 209 bits
    let mut m = [0xA1u8, 0xB2, 0xC3, 0xD4, 0x00, 180, 75, 0x02, 0];
    m[8] = xor8(&m[..8]);
    let bb = vdo_row(&[0xAA, 0xAA, 0xA9], &m, 8);
    let r = one(40, &bb);
    assert_eq!((&*r.model, &*r.id), ("Abarth-124Spider", "a1b2c3d4"));
    close(num(&r, "pressure_kpa"), 180.0 * 1.38);
    close(num(&r, "temperature_c"), 25.0);
    close(num(&r, "status"), 2.0);
    let mut bad = m;
    bad[1] ^= 1;
    assert!(run(40, &vdo_row(&[0xAA, 0xAA, 0xA9], &bad, 8)).is_empty());
}

#[test]
fn honeywell_crc_by_channel() {
    for (ch, poly) in [(8u8, 0x8005u16), (2, 0x8050)] {
        let mut m = [(ch << 4) | 0x0A, 0xBC, 0xDE, 0x80 | 0x40, 0, 0];
        let crc = |d: &[u8], p: u16| {
            if p == 0x8005 {
                CRC16_UMTS.checksum(d)
            } else {
                CRC16_HONEYWELL_8050.checksum(d)
            }
        };
        let c = crc(&m[..4], poly);
        m[4] = (c >> 8) as u8;
        m[5] = c as u8;
        let mut v = bits_of(&[0x55, 0x55, 0x55, 0x55, 0x56], 40);
        v.extend(manchester(&bits_of(&m, 48)));
        v.extend([0, 0, 0, 0, 0, 0, 0, 0]);
        let r = one(22, &row_of(&v));
        assert_eq!((&*r.model, &*r.id), ("Honeywell-Security", "abcde"));
        assert_eq!(channel(&r), serde_json::json!(ch));
        close(num(&r, "door_open"), 1.0);
        close(num(&r, "tamper"), 1.0);
        close(num(&r, "alarm"), 0.0);
        assert_eq!(r.battery_ok, Some(true));
        // the other polynomial does not pass
        let other = if poly == 0x8005 { 0x8050 } else { 0x8005 };
        let c = crc(&m[..4], other);
        m[4] = (c >> 8) as u8;
        m[5] = c as u8;
        let mut v = bits_of(&[0x55, 0x55, 0x55, 0x55, 0x56], 40);
        v.extend(manchester(&bits_of(&m, 48)));
        v.extend([0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(run(22, &row_of(&v)).is_empty());
    }
}

#[test]
fn generic_temperature_repeat_rule() {
    // id 0x21, battery bits 2, 23.5 °C: ((b1 & 0x3F) << 10 | b2 << 2) as i16 >> 4 = 235
    let v: i16 = 235 << 4;
    let b1 = 0x80 | ((v >> 10) as u8 & 0x3F);
    let b2 = ((v >> 2) & 0xFF) as u8;
    let f = [0x21, b1, b2];
    let r = one(3, &rows(&f, 24, 10, 0));
    assert_eq!((&*r.model, &*r.id), ("Generic-Temperature", "33"));
    assert_eq!(r.check, Check::Repeat);
    close(num(&r, "temperature_c"), 23.5);
    close(num(&r, "battery_bits"), 2.0);
    assert!(run(3, &rows(&f, 24, 9, 0)).is_empty(), "ten rows needed");
}

/// The Acurite tower frame of the test device: bytes 2 to 5 carry an even parity bit each, the last byte is the sum.
fn acurite_tower_frame() -> [u8; 7] {
    let par = |x: u8| if x.count_ones() % 2 == 1 { x | 0x80 } else { x };
    let t = 1225u16;
    let mut b = [
        0xD2u8,
        0x34,
        par(0x44),
        par(55),
        par((t >> 7) as u8 & 0x7F),
        par(t as u8 & 0x7F),
        0,
    ];
    b[6] = sum8(&b[..6]);
    b
}

/// Each Acurite byte carries its own parity bit. Bit 1 of byte 3 goes from 1 to 0 and bit 1 of byte 5 from 0 to 1: the sum and the
/// parity of the whole slice stay the same, and the parity of each of the two bytes breaks.
#[test]
fn acurite_parity_is_checked_in_each_byte() {
    let good = acurite_tower_frame();
    let inv = |b: &[u8]| b.iter().map(|x| !x).collect::<Vec<u8>>();
    let r = one(5, &rows(&inv(&good), 56, 1, 0));
    assert_eq!((&*r.model, &*r.id), ("Acurite-Tower", "4660"));
    let mut bad = good;
    assert!(bad[3] & 2 != 0 && bad[5] & 2 == 0);
    bad[3] ^= 2;
    bad[5] ^= 2;
    assert_eq!(sum8(&bad[..6]), bad[6]);
    assert_eq!(bad[2..6].iter().map(|x| x.count_ones()).sum::<u32>() % 2, 0);
    assert!(run(5, &rows(&inv(&bad), 56, 1, 0)).is_empty());
}

/// FM0 (biphase space) bits as levels: a change at each cell start, and one in the middle for a 0. `level` is the level before.
fn fm0_levels(bits: &[u8], mut level: u8) -> Vec<u8> {
    let mut v = Vec::new();
    for &b in bits {
        let a = 1 - level;
        let c = if b == 1 { a } else { 1 - a };
        v.extend([a, c]);
        level = c;
    }
    v
}

/// The FM0 (biphase space) decoder finds the even clock phase when the cells start at bit 0.
#[test]
fn fm0_finds_the_even_clock_phase_at_bit_0() {
    let data = [1u8, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 0];
    let lv = fm0_levels(&data, 0);
    let s: String = lv.iter().map(|&b| if b == 1 { '1' } else { '0' }).collect();
    let rows = BitRows::parse(&s).unwrap();
    let mut out = BitRows::new(1, 64);
    let n = rows.decode_fm0(0, 0, 64, &mut out);
    // The first cell needs the level before it, so decoding starts at the second cell.
    assert_eq!(n, data.len() - 1);
    for (i, &b) in data[1..].iter().enumerate() {
        assert_eq!(out.bit(0, i), b, "bit {i}");
    }
}

/// The Toyota TPMS frame: preamble 0xA9E then the data in FM0.
#[test]
fn toyota_tpms_reads_fm0() {
    let (p1, tt) = (157u8, 62u8);
    let mut b = [
        0xA1u8,
        0xB2,
        0xC3,
        0xD4,
        p1 >> 1,
        ((p1 & 1) << 7) | (tt >> 1),
        (tt & 1) << 7,
        !p1,
        0,
    ];
    b[8] = CRC8_TOYOTA.checksum(&b[..8]);
    let mut data: Vec<u8> = (0..72).map(|i| (b[i / 8] >> (7 - i % 8)) & 1).collect();
    data.extend([0, 0, 0, 0]);
    let mut lv: Vec<u8> = "01010101001111".bytes().map(|c| c - b'0').collect();
    lv.extend(fm0_levels(&data, 1));
    let s: String = lv.iter().map(|&b| if b == 1 { '1' } else { '0' }).collect();
    let r = one(34, &BitRows::parse(&s).unwrap());
    assert_eq!((&*r.model, &*r.id), ("Toyota", "a1b2c3d4"));
    close(num(&r, "pressure_kpa"), (157.0 * 0.25 - 7.0) * 6.894_757);
    close(num(&r, "temperature_c"), 22.0);
}

/// The (generator, key) pairs of the LFSR digests that the protocols use give their check values over "123456789".
#[test]
fn the_lfsr_digests_match_their_check_values() {
    let s = b"123456789";
    for (gen, key, check) in [(0x98u8, 0x3Eu8, 0xB5u8), (0x98, 0xF1, 0x54)] {
        assert_eq!(lfsr_digest8(s, gen, key), check);
    }
    for (gen, key, check) in [(0x31u8, 0xF4u8, 0x9Eu8), (0x51, 0x04, 0xA2)] {
        assert_eq!(lfsr_digest8_reflect(s, gen, key), check);
    }
    for (gen, key, check) in [(0x8810u16, 0x5412u16, 0xB746u16), (0x8810, 0xBA95, 0x0CC1)] {
        assert_eq!(lfsr_digest16(s, gen, key), check);
    }
}

/// The device CRCs on the shared engine match their check values, computed by the bit-serial definition of the CRC spec.
#[test]
fn the_device_crcs_match_their_check_values() {
    for spec in all() {
        assert_eq!(spec.compute(b"123456789"), spec.check, "{}", spec.name);
    }
    assert_eq!(CRC8_31.checksum(b"123456789"), 0xA2);
    assert_eq!(CRC8_RUBICSON.checksum(b"123456789"), 0x86);
    assert_eq!(CRC4_KEDSUM.checksum(b"123456789"), 0xE);
    assert_eq!(CRC16_ERT_SCM.checksum(b"123456789"), 0xBDF4);
    assert_eq!(CRC16_ERT_IDM.checksum(b"123456789"), 0x2322);
    assert_eq!(CRC16_HONEYWELL_8050.checksum(b"123456789"), 0xDA60);
}

//! `wmbus`: the link layer and the meters of the test signal, the declared results and the framework.

use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_core::publish::TableDef;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, Results, StackConfig, Value};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_dev_test::Harness;
use sdrkit_stack_ism::family::from_hex;
use sdrkit_stack_ism::typed::{
    WmbusRecordRow, WMBUS_CHANNELS as CHANNELS, WMBUS_FRAMES as FRAMES, WMBUS_METERS as METERS,
    WMBUS_RECORDS,
};
use sdrkit_stack_ism::wmbus::link::{self, air_len, strip_crcs, Format};
use sdrkit_stack_ism::wmbus::{self, GenParams, WMBUS};
use sdrkit_util_bits::crc::catalog::CRC16_EN_13757;
use sdrkit_util_bits::line::three_of_six;

fn fmt(phy: &str) -> Format {
    if phy == "C1 (B)" {
        Format::B
    } else {
        Format::A
    }
}

/// The cells of column `key` of a declared table as text for people, newest row first.
fn column(r: &Results, def: &TableDef, key: &str) -> Vec<String> {
    crate::util::table(r, def.key)
        .rows_newest_first()
        .map(|(_, row)| {
            let id = row
                .decl()
                .id(key)
                .unwrap_or_else(|| panic!("no column {key}"));
            row.text(id).map(|t| t.to_string()).unwrap_or_default()
        })
        .collect()
}

/// The row keys of the meter table, newest first.
fn meter_keys(r: &Results) -> Vec<String> {
    crate::util::table(r, METERS.key)
        .rows_newest_first()
        .map(|(k, _)| k.to_owned())
        .collect()
}

/// The data records of the meter with row key `meter`.
fn record_count(r: &Results, meter: &str) -> usize {
    crate::util::table(r, WMBUS_RECORDS.key)
        .rows_newest_first()
        .filter(|(_, row)| row.at(&WmbusRecordRow::METER) == Some(meter))
        .count()
}

fn field(r: &Results, key: &str) -> i64 {
    crate::util::number(r, key) as i64
}

#[test]
fn crc_check_value_and_manufacturer_code() {
    assert_eq!(CRC16_EN_13757.checksum(b"123456789"), 0xC2B7);
    assert_eq!(link::manufacturer(0x2C2D), "KAM");
    assert_eq!(link::manufacturer_code("KAM"), 0x2C2D);
}

#[test]
fn byte_vectors() {
    let exp = read_expected("ism").unwrap();
    for v in exp["byte_vectors"]["wmbus_frames_with_crcs"]
        .as_array()
        .unwrap()
    {
        let air = from_hex(v["air_hex"].as_str().unwrap()).unwrap();
        let f = fmt(v["phy"].as_str().unwrap());
        let d = strip_crcs(&air, f).unwrap_or_else(|| panic!("CRC {}", v["phy"]));
        let p = link::parse(&d).unwrap();
        assert_eq!(p.manufacturer, v["manufacturer"].as_str().unwrap());
        assert_eq!(p.id, v["id"].as_str().unwrap());
        assert_eq!(p.version as u64, v["version"].as_u64().unwrap());
        assert_eq!(p.type_name(), v["type"].as_str().unwrap());
        assert_eq!(format!("{:02X}", p.ci), v["ci"].as_str().unwrap());
        assert_eq!(p.encrypted, v["encrypted"].as_bool().unwrap());
        if let Some(r) = v["records"].as_array() {
            let got: Vec<String> = p.records.iter().map(|r| r.text()).collect();
            let want: Vec<String> = r.iter().map(|x| x.as_str().unwrap().to_string()).collect();
            assert_eq!(got, want, "{}", v["manufacturer"]);
        }
        if let Some(n) = v["records_min"].as_u64() {
            assert!(p.records.len() as u64 >= n, "{} records", p.records.len());
        }
        for i in 0..air.len() {
            let mut bad = air.clone();
            bad[i] ^= 0x01;
            assert!(
                strip_crcs(&bad, f).is_none() || i == 0,
                "byte {i} flipped still passes"
            );
        }
    }
}

#[test]
fn three_of_six_round_trip_and_unknown_words() {
    for n in 0..16 {
        assert_eq!(three_of_six::decode(three_of_six::encode(n)), Some(n));
    }
    assert_eq!(
        (0u8..64)
            .filter(|&c| three_of_six::decode(c).is_some())
            .count(),
        16
    );
}

#[test]
fn format_a_and_b_lengths() {
    assert_eq!(air_len(9, Format::A), Some(12));
    assert_eq!(air_len(10, Format::A), Some(15));
    assert_eq!(air_len(25, Format::A), Some(30));
    assert_eq!(air_len(26, Format::A), Some(33));
    assert_eq!(air_len(200, Format::A), Some(201 + 2 + 2 * 12));
    assert_eq!(air_len(127, Format::B), Some(128));
    assert_eq!(air_len(200, Format::B), Some(201));
    assert_eq!(air_len(128, Format::B), None);
    assert_eq!(air_len(129, Format::B), None);
}

/// A negative BCD value (a top nibble 0xF in the last byte) reads negative, and a nibble above 9 is not a number.
#[test]
fn bcd_records_follow_en_13757_3() {
    // DIF 0x0A (4-digit BCD), VIF 0x13 (volume, litres): 0x1234 negative is F2 34 little-endian.
    let mut d = vec![
        0u8, 0x44, 0x2D, 0x2C, 0x78, 0x56, 0x34, 0x12, 0x01, 0x07, 0x78,
    ];
    d.extend([0x0A, 0x13, 0x34, 0xF2]);
    d.extend([0x0A, 0x13, 0x3A, 0x12]);
    d[0] = (d.len() - 1) as u8;
    let p = link::parse(&d).unwrap();
    assert_eq!(p.records[0].value, "-0.234 m3");
    assert_eq!(p.records[1].value, "-");
}

fn rx(rate: f64, centre: f64) -> Box<dyn Receiver> {
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(WMBUS.settings)()));
    let mut r: Box<dyn Receiver> = Box::new(wmbus::receiver(&cfg).unwrap());
    r.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    r
}

fn feed(r: &mut dyn Receiver, x: &[Complex32], chunk: usize) -> Vec<sdrkit_core::Event> {
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(chunk) {
        r.feed(c, &mut out);
        events.append(&mut out.events);
        out.clear();
    }
    r.feed(&vec![Complex32::default(); 300_000], &mut out);
    events.append(&mut out.events);
    events
}

#[test]
fn conformance_vector() {
    let rec = sdrkit_dev_test::read_recording("ism/wmbus_868_625_1200k")
        .expect("the recording ism/wmbus_868_625_1200k");
    let exp = read_expected("ism").unwrap();
    let e = &exp["recordings"]["wmbus_868_625_1200k"]["expect"];
    let mut r = rx(rec.rate_hz, rec.centre_hz);
    feed(r.as_mut(), &rec.samples, 65536);
    let res = r.results();
    let mut labels: Vec<String> = Vec::new();
    let freqs = column(res, &CHANNELS, "frequency");
    let phys = column(res, &CHANNELS, "phy");
    for (f, p) in freqs.iter().zip(&phys) {
        labels.push(format!("{:.4} MHz {p}", f.parse::<f64>().unwrap() / 1e6));
    }
    let mut want: Vec<String> = e["channels_listed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    labels.sort();
    want.sort();
    assert_eq!(labels, want);
    assert!(field(res, "frames_ok") >= e["frames_good_min"].as_i64().unwrap());
    assert_eq!(field(res, "frames_bad"), 0);
    assert!(field(res, "encrypted_frames") >= e["encrypted_frames_min"].as_i64().unwrap());
    assert_eq!(field(res, "meters"), e["meters_exactly"].as_i64().unwrap());
    let ids = column(res, &METERS, "meter_id");
    let makers = column(res, &METERS, "maker");
    let modes = column(res, &METERS, "phy");
    let types = column(res, &METERS, "device_type");
    let enc = column(res, &METERS, "encrypted");
    let last = column(res, &METERS, "last_reading");
    let keys = meter_keys(res);
    for m in e["meters"].as_array().unwrap() {
        let i = ids
            .iter()
            .position(|x| x == m["id"].as_str().unwrap())
            .expect("meter");
        assert_eq!(makers[i], m["manufacturer"].as_str().unwrap());
        assert_eq!(modes[i], m["phy"].as_str().unwrap());
        assert_eq!(types[i], m["type"].as_str().unwrap());
        let encrypted = m["encrypted"].as_bool().unwrap();
        assert_eq!(enc[i] == "yes", encrypted, "{}", ids[i]);
        let n = record_count(res, &keys[i]);
        if let Some(rs) = m["records"].as_array() {
            if encrypted {
                assert_eq!(n, 0);
                assert_eq!(last[i], "encrypted payload");
            } else {
                assert_eq!(n, rs.len(), "{}", ids[i]);
            }
        }
        if let Some(min) = m["records_min"].as_u64() {
            assert!(n as u64 >= min);
        }
    }
}

fn gp(seed: u64) -> GenParams {
    GenParams {
        plan169: false,
        only_phy: None,
        encrypted: true,
        silence: false,
        seed,
    }
}

#[test]
fn generator_round_trip_868() {
    for fe in [-10e3, 10e3] {
        let run = Harness::new(&WMBUS)
            .rate(2e6)
            .seconds(30.0)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 15.0)
            .set(&GEN_OFFSET_HZ, fe)
            .run()
            .unwrap();
        let rx = &run.results;
        let ids = column(rx, &METERS, "meter_id");
        for id in [
            "12345678", "40112233", "50667788", "61000042", "72004711", "83001234",
        ] {
            assert!(ids.iter().any(|x| x == id), "{id} missing at {fe}: {ids:?}");
        }
        assert_eq!(ids.len(), 6, "{ids:?}");
        let i = ids.iter().position(|x| x == "50667788").unwrap();
        assert_eq!(column(rx, &METERS, "last_reading")[i], "encrypted payload");
        assert_eq!(record_count(rx, &meter_keys(rx)[i]), 0);
    }
}

#[test]
fn generator_round_trip_169() {
    let run = Harness::new(&WMBUS)
        .rate(1e6)
        .freq(wmbus::gen_centre(true))
        .set(&wmbus::GEN_PLAN, wmbus::GenPlan::Mhz169)
        .seconds(3.0)
        .run()
        .unwrap();
    let rx = &run.results;
    let ids = column(rx, &METERS, "meter_id");
    let modes = column(rx, &METERS, "phy");
    assert!(
        ids.iter()
            .zip(&modes)
            .any(|(i, m)| i == "94005566" && m == "N1"),
        "{ids:?}"
    );
}

#[test]
fn noise_only_returns_no_meter() {
    let run = Harness::new(&WMBUS)
        .rate(1.2e6)
        .seconds(60.0)
        .set(&wmbus::GEN_SILENCE, true)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, -30.0)
        .run()
        .unwrap();
    let rx = &run.results;
    assert_eq!(field(rx, "frames_ok"), 0);
    assert_eq!(field(rx, "meters"), 0);
}

#[test]
fn chunking_returns_the_same_frames() {
    let g = wmbus::generator(1.2e6, gp(5)).unwrap();
    let mut g: Box<dyn sdrkit_core::Generator> = Box::new(g);
    let mut x = vec![Complex32::default(); (1.2e6 * 2.5) as usize];
    g.generate(&mut x);
    let mut all = Vec::new();
    for c in [1usize, 777, 65536] {
        let mut r = rx(1.2e6, 868.625e6);
        feed(r.as_mut(), &x, c);
        all.push(column(r.results(), &FRAMES, "frame"));
    }
    assert!(!all[0].is_empty());
    assert_eq!(all[0], all[1]);
    assert_eq!(all[1], all[2]);
}

#[test]
fn every_event_is_declared_and_has_its_frequency() {
    let decl = sdrkit_stack_ism::typed::shape("wmbus").results_decl();
    let run = Harness::new(&WMBUS).seconds(5.0).run().unwrap();
    assert!(!run.events.is_empty());
    assert!(crate::util::declared(&decl, &run.events));
    for e in &run.events {
        assert!(e.freq_hz > 800e6, "{} sent at {} Hz", e.kind(), e.freq_hz);
    }
}

#[test]
fn a_bad_setting_value_is_refused() {
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(WMBUS.settings)()));
    let mut r = wmbus::receiver(&cfg).unwrap();
    assert!(crate::util::set(&mut r, "no_such_setting", Value::Bool(true)).is_err());
}

/// The 3-of-6 decoder takes every 6-chip word, and the words not in the code give `None`.
#[test]
fn three_of_six_decodes_by_table() {
    let mut found = [None; 64];
    for (c, f) in found.iter_mut().enumerate() {
        *f = three_of_six::decode(c as u8);
    }
    for (n, &w) in three_of_six::TABLE.iter().enumerate() {
        assert_eq!(found[w as usize], Some(n as u8));
        assert_eq!(w.count_ones(), 3);
    }
    assert_eq!(found.iter().filter(|f| f.is_some()).count(), 16);
}

/// A change of frequency keeps the meters, their records and the counts, and drops the channels of the present band.
#[test]
fn a_change_of_frequency_keeps_the_meters_and_drops_the_channels() {
    let h = Harness::new(&WMBUS).seconds(3.0);
    let t = &WMBUS.tuning;
    let mut r = rx(t.rate_hz, t.default_hz);
    feed(&mut *r, &h.generate().unwrap(), 65536);
    let meters = meter_keys(r.results()).len();
    let frames = field(r.results(), "frames_ok");
    assert!(meters >= 1 && frames > 0);
    assert!(!column(r.results(), &CHANNELS, "frequency").is_empty());
    r.forget();
    assert_eq!(meter_keys(r.results()).len(), meters);
    assert_eq!(field(r.results(), "frames_ok"), frames);
    assert!(column(r.results(), &CHANNELS, "frequency").is_empty());
    assert_eq!(r.results().status().level_dbfs, None);
}

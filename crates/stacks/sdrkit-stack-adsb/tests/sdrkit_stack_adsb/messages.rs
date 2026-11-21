//! The message layer against tests/vectors/adsb/messages.json, and the frame checks and the table with messages fed as sliced
//! frames.

use sdrkit_stack_adsb::checks::{CheckStats, Correction, FrameChecker, Msg};
use sdrkit_stack_adsb::codes;
use sdrkit_stack_adsb::commb::{self, Bds};
use sdrkit_stack_adsb::demod::{DemodStats, Slice};
use sdrkit_stack_adsb::generator::*;
use sdrkit_stack_adsb::message::{self, es, header, Es, Frame, SpeedKind};
use sdrkit_stack_adsb::table::{PosKind, Tracker};
use sdrkit_stack_adsb::uat::payload::decode_downlink;
use sdrkit_util_geo::cpr::{self, CprFormat, CprFrame};
use sdrkit_util_geo::Position;
use serde_json::Value as J;

fn messages() -> J {
    let p = sdrkit_dev_test::sigmf::vectors_dir()
        .join("adsb")
        .join("messages.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn f(h: &str) -> Frame {
    Frame::from_hex(h).unwrap()
}

#[test]
fn every_message_case_decodes() {
    let m = messages();
    for case in m["mode_s"].as_array().unwrap() {
        let Some(hex) = case["hex"].as_str() else {
            continue;
        };
        let e = &case["expect"];
        let fr = f(hex);
        let rem = fr.remainder();
        let h = header(&fr, rem);
        if let Some(df) = e["df"].as_u64() {
            assert_eq!(fr.df() as u64, df, "{hex}");
        }
        if let Some(r) = e["crc_remainder"].as_u64() {
            assert_eq!(rem as u64, r, "{hex}");
        }
        if let Some(a) = e["address_from_parity"].as_str() {
            assert_eq!(format!("{rem:06X}"), a, "{hex}");
        }
        if let Some(a) = e["icao"].as_str() {
            assert_eq!(format!("{:06X}", h.key & 0xFFFFFF), a, "{hex}");
        }
        if let Some(sq) = e["squawk"].as_str() {
            let s = h.squawk.or(match es(&fr) {
                Some(Es::Status { squawk, .. }) => squawk,
                _ => None,
            });
            assert_eq!(codes::squawk_text(s.unwrap()), sq, "{hex}");
        }
        if let Some(fs) = e["flight_status"].as_u64() {
            assert_eq!(h.code as u64, fs, "{hex}");
            let (g, alert, _) = message::flight_status(h.code);
            if let Some(og) = e["on_ground"].as_bool() {
                assert_eq!(g, Some(og), "{hex}");
            }
            if let Some(al) = e["alert"].as_bool() {
                assert_eq!(alert, al, "{hex}");
            }
        }
        if let Some(ic) = e["interrogator_code"].as_u64() {
            assert_eq!(h.ic.map(|x| x as u64), Some(ic), "{hex}");
        }
        if let Some(p) = e["parity_of_data"].as_str() {
            assert_eq!(
                format!(
                    "{:06X}",
                    sdrkit_stack_adsb::crc::parity(&fr.data()[..fr.data().len() - 3])
                ),
                p
            );
        }
        if let Some(rep) = e["repaired_hex"].as_str() {
            let i = sdrkit_stack_adsb::crc::syndromes()
                .locate(112, rem)
                .unwrap();
            assert_eq!(
                i + 1,
                e["single_bit_error_position_1_based"].as_u64().unwrap() as usize
            );
            let mut g = fr;
            g.flip(i);
            assert_eq!(g.hex(), rep);
        }
        let ee = es(&fr);
        match ee {
            Some(Es::Identification {
                set,
                code,
                ref callsign,
            }) => {
                if let Some(c) = e["callsign"].as_str() {
                    assert_eq!(callsign, c);
                }
                if let Some(c) = e["category"].as_str() {
                    assert_eq!(format!("{set}{code}"), c);
                }
                if let Some(c) = e["category_code"].as_u64() {
                    assert_eq!(code as u64, c);
                }
            }
            Some(Es::Airborne {
                alt_ft,
                gnss,
                cpr,
                ss,
                ..
            }) => {
                if let Some(a) = e["baro_alt_ft"].as_i64() {
                    assert!(!gnss);
                    assert_eq!(alt_ft, Some(a as i32), "{hex}");
                }
                if let Some(a) = e["gnss_height_ft"].as_i64() {
                    assert!(gnss);
                    assert_eq!(alt_ft, Some(a as i32));
                }
                if let Some(o) = e["cpr_odd"].as_bool() {
                    assert_eq!(cpr.odd, o);
                }
                if let Some(v) = e["cpr_lat"].as_u64() {
                    assert_eq!(cpr.lat as u64, v);
                    assert_eq!(cpr.lon as u64, e["cpr_lon"].as_u64().unwrap());
                }
                if let Some(v) = e["surveillance_status"].as_u64() {
                    assert_eq!(ss as u64, v);
                }
            }
            Some(Es::Surface {
                speed_kt,
                track_deg,
                cpr,
                ..
            }) => {
                assert_eq!(speed_kt, e["movement_kt"].as_f64());
                assert!((track_deg.unwrap() - e["track_deg"].as_f64().unwrap()).abs() < 1e-6);
                assert_eq!(cpr.lat as u64, e["cpr_lat"].as_u64().unwrap());
            }
            Some(Es::Velocity {
                speed_kt,
                speed_kind,
                heading_deg,
                is_track,
                vrate_fpm,
                vrate_baro,
                gnss_diff_ft,
                ..
            }) => {
                assert!((speed_kt.unwrap() - e["speed_kt"].as_f64().unwrap()).abs() < 0.11);
                let kind = match speed_kind {
                    SpeedKind::Ground => "ground",
                    SpeedKind::Ias => "ias",
                    SpeedKind::Tas => "tas",
                };
                assert_eq!(kind, e["speed_kind"].as_str().unwrap());
                let h = e["track_deg"]
                    .as_f64()
                    .or(e["heading_deg"].as_f64())
                    .unwrap();
                assert!((heading_deg.unwrap() - h).abs() < 0.011);
                assert_eq!(!is_track, e["heading_is_track"] == false);
                assert_eq!(vrate_fpm.map(|v| v as i64), e["vertical_rate_fpm"].as_i64());
                assert_eq!(
                    if vrate_baro { "baro" } else { "gnss" },
                    e["vertical_rate_source"].as_str().unwrap()
                );
                assert_eq!(
                    gnss_diff_ft.map(|v| v as i64),
                    e["gnss_minus_baro_ft"].as_i64()
                );
            }
            Some(Es::Status { emergency, .. }) => {
                assert_eq!(emergency as u64, e["emergency_state"].as_u64().unwrap());
                assert_eq!(
                    message::emergency_name(emergency),
                    e["emergency_name"].as_str().unwrap()
                );
            }
            Some(Es::NoPosition { alt_ft }) => {
                assert_eq!(alt_ft.map(|a| a as i64), e["baro_alt_ft"].as_i64())
            }
            Some(Es::TargetState {
                sel_alt_ft,
                sel_alt_fms,
                baro_hpa,
                sel_heading_deg,
                nacp,
                ..
            }) => {
                assert_eq!(sel_alt_ft.map(|v| v as i64), e["selected_alt_ft"].as_i64());
                assert_eq!(sel_alt_fms, e["selected_alt_source"] == "fms");
                assert!((baro_hpa.unwrap() - e["baro_setting_hpa"].as_f64().unwrap()).abs() < 0.05);
                assert!(
                    (sel_heading_deg.unwrap() - e["selected_heading_deg"].as_f64().unwrap()).abs()
                        < 0.5
                );
                assert_eq!(nacp as u64, e["nacp"].as_u64().unwrap());
            }
            Some(Es::OpStatus {
                version,
                nacp,
                sil,
                nic_a,
                gva,
                nic_baro,
                ..
            }) => {
                assert_eq!(version as u64, e["adsb_version"].as_u64().unwrap());
                assert_eq!(nacp.map(|v| v as u64), e["nacp"].as_u64());
                assert_eq!(sil.map(|v| v as u64), e["sil"].as_u64());
                assert_eq!(nic_a.map(|v| v as u64), e["nic_supplement_a"].as_u64());
                assert_eq!(gva.map(|v| v as u64), e["gva"].as_u64());
                assert_eq!(nic_baro.map(|v| v as u64), e["nic_baro"].as_u64());
            }
            _ => {}
        }
        if matches!(fr.df(), 0 | 4 | 16 | 20) {
            if let Some(a) = e["baro_alt_ft"].as_i64() {
                assert_eq!(h.alt_ft, Some(a as i32), "{hex}");
            }
        }
        if let Some(b) = e["bds"].as_str() {
            let alt = h.alt_ft;
            let reg = commb::infer(fr.me(), alt).unwrap_or_else(|| panic!("{hex}: no register"));
            assert_eq!(reg.id(), b, "{hex}");
            match reg {
                Bds::B20 { callsign } => assert_eq!(callsign, e["callsign"].as_str().unwrap()),
                Bds::B40 {
                    mcp_ft,
                    fms_ft,
                    baro_hpa,
                } => {
                    assert_eq!(mcp_ft.map(|v| v as i64), e["selected_alt_mcp_ft"].as_i64());
                    assert_eq!(fms_ft.map(|v| v as i64), e["selected_alt_fms_ft"].as_i64());
                    assert!(
                        (baro_hpa.unwrap() - e["baro_setting_hpa"].as_f64().unwrap()).abs() < 0.05
                    );
                }
                Bds::B30 {
                    threat,
                    ara,
                    rac,
                    terminated,
                    tti,
                    ..
                } => {
                    assert_eq!(
                        format!("{:06X}", threat.unwrap()),
                        e["threat_icao"].as_str().unwrap()
                    );
                    assert_eq!(
                        commb::advisory_text(ara, rac, terminated),
                        e["ra_text"].as_str().unwrap()
                    );
                    assert_eq!(tti as u64, e["threat_type"].as_u64().unwrap());
                }
                Bds::B17 { mask } => {
                    assert_eq!(format!("{mask:06X}"), e["gicb_mask"].as_str().unwrap());
                    // The registers follow the capability bit order of ICAO Doc 9871 (bit 1 = 0,5, bit 7 = 2,0, bit 9 = 4,0,
                    // bit 11 = 4,2, bit 16 = 5,0, bit 24 = 6,0); that order is the authority over the case's register list,
                    // which does not agree with its own mask.
                    let regs = commb::gicb_registers(mask);
                    let mut want: Vec<&str> = Vec::new();
                    for (i, r) in commb::GICB_REGISTERS.iter().enumerate() {
                        if mask >> (23 - i) & 1 == 1 {
                            want.push(r);
                        }
                    }
                    assert_eq!(regs, want);
                    if mask == 0x02A101 {
                        assert_eq!(regs, ["2,0", "4,0", "4,2", "5,0", "6,0"]);
                    }
                    for r in ["2,0", "5,0", "6,0"] {
                        assert!(regs.contains(&r), "{regs:?}");
                    }
                }
                Bds::B10 {
                    subnetwork_version,
                    acas_ra,
                } => {
                    assert_eq!(
                        subnetwork_version as u64,
                        e["subnetwork_version"].as_u64().unwrap()
                    );
                    assert_eq!(acas_ra, e["acas_generates_ra"].as_bool().unwrap());
                }
                _ => {}
            }
        }
    }
    // The UAT payload.
    for case in m["uat"].as_array().unwrap() {
        let hex = case["payload_hex"].as_str().unwrap();
        let b: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let d = decode_downlink(&b).unwrap();
        let e = &case["expect"];
        assert_eq!(format!("{:06X}", d.address), e["address"].as_str().unwrap());
        let (lat, lon) = d.sv.pos.unwrap();
        assert!(
            (lat - e["lat"].as_f64().unwrap()).abs() < 1e-9
                && (lon - e["lon"].as_f64().unwrap()).abs() < 1e-9
        );
        assert_eq!(d.sv.alt.map(|a| a.0 as i64), e["altitude_ft"].as_i64());
        assert_eq!(d.sv.nic as u64, e["nic"].as_u64().unwrap());
    }
    // Code tables.
    for c in m["codes"]["cpr_nl"].as_array().unwrap() {
        if let (Some(lat), Some(n)) = (c["lat"].as_f64(), c["nl"].as_u64()) {
            assert_eq!(cpr::nl(lat) as u64, n, "{lat}");
        }
    }
    for c in m["codes"]["gillham_11_bit_C1A1C2A2C4A4B1B2D2B4D4"]
        .as_array()
        .unwrap()
    {
        if let Some(code) = c["code"].as_str() {
            let v = u32::from_str_radix(code.trim_start_matches("0x"), 16).unwrap();
            assert_eq!(codes::gillham_ft(v).map(|a| a as i64), c["alt_ft"].as_i64());
        }
    }
}

#[test]
fn the_published_pairs_decode() {
    let e = CprFrame {
        odd: false,
        lat: 93000,
        lon: 51372,
    };
    let o = CprFrame {
        odd: true,
        lat: 74158,
        lon: 50194,
    };
    let p = cpr::decode_global(&CprFormat::AIRBORNE, &e, &o, true, None).unwrap();
    let (la, lo) = (p.lat_deg, p.lon_deg);
    assert!((la - 52.26578017412606).abs() < 1e-9 && (lo - 3.93891).abs() < 1e-4);
    // Local decodes from known positions 100 NM away give the same.
    let q =
        cpr::decode_local(&CprFormat::AIRBORNE, &o, &Position::new(la + 1.5, lo - 1.0)).unwrap();
    assert!((q.lat_deg - la).abs() < 1e-9 && (q.lon_deg - lo).abs() < 1e-9);
}

/// The checks and the table, fed with frames as sliced frames.
struct Bench {
    c: FrameChecker,
    t: Tracker,
    good: u64,
}

impl Bench {
    fn new() -> Bench {
        Bench {
            c: FrameChecker::default(),
            t: Tracker::default(),
            good: 0,
        }
    }
    fn rx(mut self, lat: f64, lon: f64) -> Bench {
        self.t.table.station = Some((lat, lon));
        self
    }
    fn feed(&mut self, fr: &Frame, t: f64, level: f32) -> bool {
        self.feed_slice(&Slice::given(fr.data()), t, level)
    }
    fn feed_slice(&mut self, s: &Slice, t: f64, level: f32) -> bool {
        match self.c.check_slice(s, t, Some(level), Some(20.0)) {
            Some(a) => {
                self.good += 1;
                self.t.take(&Msg::Frame(a));
                true
            }
            None => {
                self.c.count_failed();
                false
            }
        }
    }
    fn report(&mut self, t: f64) -> J {
        let st = CheckStats {
            demod: DemodStats {
                time: t,
                noise_dbfs: -60.0,
                ..Default::default()
            },
            ..self.c.stats().clone()
        };
        self.t.take(&Msg::Stats(st));
        use sdrkit_graph::Component;
        super::common::view(self.t.results().unwrap())
    }
    fn ac(&mut self, t: f64, icao: &str) -> J {
        let d = self.report(t);
        d["aircraft"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["icao"] == icao)
            .cloned()
            .unwrap_or(J::Null)
    }
}

#[test]
fn identification_position_and_velocity_make_three_aircraft() {
    let mut b = Bench::new();
    assert!(b.feed(&f("8D4840D6202CC371C32CE0576098"), 1.0, -30.0));
    assert!(b.feed(&f("8D40621D58C382D690C8AC2863A7"), 1.1, -30.0));
    assert!(b.feed(&f("8D40621D58C386435CC412692AD6"), 1.2, -30.0));
    assert!(b.feed(&f("8D485020994409940838175B284F"), 1.3, -30.0));
    let d = b.report(1.5);
    assert_eq!(d["aircraft_count"], 3);
    let a = b.ac(1.5, "4840D6");
    assert_eq!(a["callsign"], "KLM1023");
    assert!(a.get("category").is_none());
    let p = b.ac(1.5, "40621D");
    assert_eq!(p["baro_alt_ft"], 38000);
    assert_eq!(p["position_kind"], "global");
    assert!((p["lat"].as_f64().unwrap() - 52.26578017412606).abs() < 1e-9);
    let v = b.ac(1.5, "485020");
    assert!((v["speed_kt"].as_f64().unwrap() - 159.2).abs() < 0.1);
    assert_eq!(v["vertical_rate_fpm"], -832);
    assert_eq!(b.c.stats().by_df[17], 4);
    assert!(d["messages"][0]["message"]
        .as_str()
        .unwrap()
        .starts_with("Identification KLM1023"));
}

#[test]
fn address_parity_replies_need_a_known_address() {
    let mut b = Bench::new();
    let df4 = f("2000171806A983");
    assert!(!b.feed(&df4, 1.0, -30.0));
    assert_eq!(b.c.stats().failed, 1);
    assert!(b.feed(&df11(5, 0x4CA7E8, 0), 1.5, -30.0));
    assert!(b.feed(&df4, 2.0, -30.0));
    let a = b.ac(2.5, "4CA7E8");
    assert_eq!(a["baro_alt_ft"], 36000);
    assert_eq!(a["on_ground"], false);
}

#[test]
fn error_correction() {
    let good = f("8D4840D6202CC371C32CE0576098");
    // Unknown address: refused, repaired with "repair unknown addresses".
    let mut b = Bench::new();
    let mut one = good;
    one.flip(60);
    assert!(!b.feed(&one, 1.0, -30.0));
    b.c.repair_unknown = true;
    assert!(b.feed(&one, 1.1, -30.0));
    // Every single-bit error of a known aircraft's frame is repaired.
    let mut b = Bench::new();
    assert!(b.feed(&good, 1.0, -30.0));
    for i in 0..112 {
        let mut g = good;
        g.flip(i);
        assert!(b.feed(&g, 1.0 + i as f64 * 0.01, -30.0), "bit {i}");
    }
    assert_eq!(b.c.stats().repaired, 112);
    // Off refuses.
    b.c.correction = Correction::Off;
    let mut g = good;
    g.flip(40);
    assert!(!b.feed(&g, 3.0, -30.0));
    // Two errors: refused with 1 bit; repaired with up to 3 bits when the confidences point at them.
    b.c.correction = Correction::One;
    let mut two = good;
    two.flip(40);
    two.flip(77);
    assert!(!b.feed(&two, 3.1, -30.0));
    b.c.correction = Correction::Three;
    let mut s = Slice::given(two.data());
    s.conf[40] = 0.01;
    s.conf[77] = 0.02;
    assert!(b.feed_slice(&s, 3.2, -30.0));
    let blind = Slice::given(two.data());
    let mut blind2 = blind;
    for (i, c) in blind2.conf.iter_mut().enumerate() {
        *c = if i < 16 { 0.01 } else { 1.0 };
    }
    assert!(!b.feed_slice(&blind2, 3.3, -30.0));
    // DF17 -> DF19 is one bit.
    let mut d19 = good;
    d19.flip(3);
    assert_eq!(d19.df(), 19);
    assert!(b.feed(&d19, 3.4, -30.0));
    // A DF11 with interrogator code 9 is refused for an unknown address and accepted for a known one.
    let ic9 = f("5D4840D6F87406");
    let mut b2 = Bench::new();
    assert!(!b2.feed(&ic9, 1.0, -30.0));
    assert!(b.feed(&ic9, 3.5, -30.0));
    // An empty frame is refused.
    assert!(!b.feed_slice(&Slice::given(&[0u8; 14]), 3.6, -30.0));
}

#[test]
fn plausibility_of_replies() {
    let mut b = Bench::new();
    let a = 0x4840D6;
    let pos = es_frame(
        17,
        5,
        a,
        me_airborne(
            11,
            0,
            codes::encode_ac12_25ft(30000).unwrap(),
            false,
            47.0,
            8.5,
        ),
    );
    assert!(b.feed(&pos, 1.0, -30.0));
    assert!(b.feed(
        &df4_20(0, codes::encode_ac13_25ft(30200).unwrap(), None, a),
        2.0,
        -30.0
    ));
    assert!(!b.feed(
        &df4_20(0, codes::encode_ac13_25ft(38000).unwrap(), None, a),
        3.0,
        -30.0
    ));
    assert!(!b.feed(&df5_21(0, 0o1200, None, a), 4.0, -70.0));
    assert!(b.feed(&df5_21(0, 0o1200, None, a), 5.0, -45.0));
    assert!(!b.feed(&df5_21(6, 0o1200, None, a), 6.0, -30.0));
    // Keep the address known with all-call replies, 35 s after the last ADS-B altitude.
    assert!(b.feed(&df11(5, a, 0), 20.0, -30.0));
    assert!(b.feed(&df11(5, a, 0), 35.0, -30.0));
    assert!(b.feed(
        &df4_20(0, codes::encode_ac13_25ft(38000).unwrap(), None, a),
        36.0,
        -30.0
    ));
    for (df, n) in [
        (24u8, Some(112u8)),
        (27, Some(112)),
        (1, None),
        (22, None),
        (28, None),
        (31, None),
    ] {
        assert_eq!(message::df_bits(df << 3), n, "DF{df}");
    }
}

#[test]
fn positions() {
    let a = 0x3C6544;
    let alt = codes::encode_ac12_25ft(20000).unwrap();
    let ev = |t: f64, lat: f64, lon: f64| {
        (
            t,
            es_frame(17, 5, a, me_airborne(11, 0, alt, false, lat, lon)),
        )
    };
    let od = |t: f64, lat: f64, lon: f64| {
        (
            t,
            es_frame(17, 5, a, me_airborne(11, 0, alt, true, lat, lon)),
        )
    };
    // Without a receiver position one frame produces no position; the pair produces a global position.
    let mut b = Bench::new();
    let (t, x) = ev(1.0, 47.0, 8.5);
    b.feed(&x, t, -30.0);
    assert!(b.ac(1.2, "3C6544").get("lat").is_none());
    let (t, x) = od(1.5, 47.0005, 8.5005);
    b.feed(&x, t, -30.0);
    let p = b.ac(1.6, "3C6544");
    assert_eq!(p["position_kind"], "global");
    assert!(
        (p["lat"].as_f64().unwrap() - 47.0005).abs() < 1e-4
            && (p["lon"].as_f64().unwrap() - 8.5005).abs() < 2e-4
    );
    // 15 s later a single frame decodes locally (confirmed).
    let (t, x) = ev(16.51, 47.01, 8.51);
    b.feed(&x, t, -30.0);
    let p = b.ac(16.6, "3C6544");
    assert_eq!(p["position_kind"], "local");
    assert!((p["lat"].as_f64().unwrap() - 47.01).abs() < 1e-4);
    // A pair 11.5 s apart decodes, one 16 s apart does not.
    let mut b = Bench::new();
    b.feed(&ev(1.0, 47.0, 8.5).1, 1.0, -30.0);
    b.feed(&od(12.5, 47.001, 8.5).1, 12.5, -30.0);
    assert!(b.ac(12.6, "3C6544").get("lat").is_some());
    let mut b = Bench::new();
    b.feed(&ev(1.0, 47.0, 8.5).1, 1.0, -30.0);
    b.feed(&od(17.0, 47.001, 8.5).1, 17.0, -30.0);
    assert!(b.ac(17.1, "3C6544").get("lat").is_none());
    // With a receiver position the first frame produces an unconfirmed local position with range and bearing; the pair confirms
    // it.
    let mut b = Bench::new().rx(47.5, 9.0);
    b.feed(&ev(1.0, 47.0, 8.5).1, 1.0, -30.0);
    let p = b.ac(1.1, "3C6544");
    assert_eq!(p["position_kind"], "local_from_receiver");
    let d = sdrkit_stack_adsb::geo::distance_nm(47.5, 9.0, 47.0, 8.5);
    assert!((p["distance_nm"].as_f64().unwrap() - d).abs() < 0.1);
    b.feed(&od(1.5, 47.0, 8.5).1, 1.5, -30.0);
    let rep = b.report(1.6);
    let p = b.ac(1.6, "3C6544");
    assert_eq!(p["position_kind"], "global");
    assert!((rep["max_range_nm"].as_f64().unwrap() - d).abs() < 0.1);
    // A jump of 3 degrees after 1 s is dropped; the third in a row is believed.
    for k in 0..3 {
        b.feed(
            &ev(2.5 + k as f64 * 0.5, 50.0, 8.5).1,
            2.5 + k as f64 * 0.5,
            -30.0,
        );
        b.feed(
            &od(2.6 + k as f64 * 0.5, 50.0, 8.5).1,
            2.6 + k as f64 * 0.5,
            -30.0,
        );
        let p = b.ac(2.7 + k as f64 * 0.5, "3C6544");
        let moved = (p["lat"].as_f64().unwrap() - 50.0).abs() < 0.01;
        if k < 1 {
            assert!(!moved, "{k}");
        }
        if k == 2 {
            assert!(moved);
        }
    }
    // The southern hemisphere at the date line.
    let mut b = Bench::new();
    let s = 0x7C1234;
    b.feed(
        &es_frame(17, 5, s, me_airborne(11, 0, alt, false, -33.9, 179.9)),
        1.0,
        -30.0,
    );
    b.feed(
        &es_frame(17, 5, s, me_airborne(11, 0, alt, true, -33.9, 179.9)),
        1.4,
        -30.0,
    );
    let p = b.ac(1.5, "7C1234");
    assert!(
        (p["lat"].as_f64().unwrap() + 33.9).abs() < 1e-3
            && (p["lon"].as_f64().unwrap() - 179.9).abs() < 2e-3
    );
}

#[test]
fn surface_positions() {
    let a = 0x896111;
    let sf = |t: f64, odd: bool, lat: f64, lon: f64| {
        (
            t,
            es_frame(17, 4, a, me_surface(7, 17.0, Some(92.8125), odd, lat, lon)),
        )
    };
    // With the receiver position.
    let mut b = Bench::new().rx(25.25, 55.36);
    let (t, x) = sf(1.0, false, 25.2532, 55.3657);
    b.feed(&x, t, -30.0);
    let p = b.ac(1.1, "896111");
    assert!(
        (p["lat"].as_f64().unwrap() - 25.2532).abs() < 1e-4
            && (p["lon"].as_f64().unwrap() - 55.3657).abs() < 1e-4
    );
    assert_eq!(p["speed_kt"], 17.0);
    assert_eq!(p["heading_deg"], 92.8125);
    assert_eq!(p["on_ground"], true);
    // Without a station or a known position, nothing.
    let mut b = Bench::new();
    b.feed(&sf(1.0, false, 25.2532, 55.3657).1, 1.0, -30.0);
    b.feed(&sf(1.5, true, 25.2532, 55.3657).1, 1.5, -30.0);
    assert!(b.ac(1.6, "896111").get("lat").is_none());
    // After an airborne fix, a surface frame decodes from it.
    let alt = codes::encode_ac12_25ft(1000).unwrap();
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt, false, 25.30, 55.40)),
        2.0,
        -30.0,
    );
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt, true, 25.30, 55.40)),
        2.4,
        -30.0,
    );
    b.feed(&sf(3.0, false, 25.3002, 55.4003).1, 3.0, -30.0);
    let p = b.ac(3.1, "896111");
    assert!(
        (p["lat"].as_f64().unwrap() - 25.3002).abs() < 1e-3
            && (p["lon"].as_f64().unwrap() - 55.4003).abs() < 1e-3
    );
}

#[test]
fn status_squawks_and_comm_b() {
    let a = 0x111111;
    let mut b = Bench::new();
    assert!(b.feed(&df11(5, a, 0), 1.0, -30.0));
    assert!(b.feed(&f("28000AAA5BB5D8"), 1.1, -30.0)); // 7700
    assert_eq!(b.ac(1.2, "111111")["emergency"], 1);
    // The squawk confirmation sequence.
    let seq = [
        (0o1200, "7700"),
        (0o1200, "1200"),
        (0o2200, "1200"),
        (0o3300, "1200"),
        (0o2200, "1200"),
        (0o2200, "2200"),
    ];
    for (k, (sq, want)) in seq.iter().enumerate() {
        b.feed(&df5_21(0, *sq, None, a), 2.0 + k as f64, -30.0);
        assert_eq!(b.ac(2.1 + k as f64, "111111")["squawk"], *want, "{k}");
    }
    // TC 28 state 5 with 7500 reports 5, state 0 clears it.
    b.feed(&es_frame(17, 5, a, me_status(5, 0o7500)), 9.0, -30.0);
    assert_eq!(b.ac(9.1, "111111")["emergency"], 5);
    b.feed(&es_frame(17, 5, a, me_status(0, 0o2200)), 9.5, -30.0);
    assert_eq!(b.ac(9.6, "111111")["emergency"], 0);
    // FS 3: on the ground and alert.
    b.feed(&f("23000098EAE6CF"), 10.0, -30.0);
    let p = b.ac(10.1, "111111");
    assert_eq!(
        (p["on_ground"].as_bool(), p["alert"].as_bool()),
        (Some(true), Some(true))
    );
    // Comm-B.
    let alt = codes::encode_ac13_25ft(30000).unwrap();
    b.feed(
        &df4_20(0, alt, Some(commb::encode_20("UAE412")), a),
        11.0,
        -30.0,
    );
    assert_eq!(b.ac(11.1, "111111")["callsign"], "UAE412");
    b.feed(
        &df4_20(
            0,
            alt,
            Some(commb::encode_40(Some(35008), Some(35008), Some(1013.2))),
            a,
        ),
        11.5,
        -30.0,
    );
    let p = b.ac(11.6, "111111");
    assert_eq!(p["selected_alt_ft"], 35008);
    assert!((p["baro_setting_hpa"].as_f64().unwrap() - 1013.2).abs() < 0.05);
    b.feed(
        &df5_21(
            0,
            0o2200,
            Some(commb::encode_60(90.0, 280.0, 0.78, -640, -640)),
            a,
        ),
        12.0,
        -30.0,
    );
    let p = b.ac(12.1, "111111");
    assert_eq!(p["ias_kt"], 280.0);
    assert!((p["mach"].as_f64().unwrap() - 0.78).abs() < 0.005);
    // TC 29, 31, 20; DF18 CF1 kept apart; Gillham.
    let mut b = Bench::new();
    b.feed(&f("8D111111EA4668650120002119E1"), 1.0, -30.0);
    b.feed(&f("8D111111F8000000004A30B57E1C"), 1.1, -30.0);
    b.feed(
        &es_frame(
            17,
            5,
            a,
            me_airborne(
                11,
                0,
                codes::encode_ac12_25ft(30000).unwrap(),
                false,
                47.0,
                8.0,
            ),
        ),
        1.2,
        -30.0,
    );
    b.feed(&f("8D111111A0B510AAAA8000BCE31F"), 1.3, -30.0);
    let p = b.ac(1.4, "111111");
    assert_eq!(p["selected_alt_ft"], 36000);
    assert_eq!(p["adsb_version"], 2);
    assert_eq!(p["nacp"], 10);
    assert_eq!(p["gnss_alt_ft"], 35025);
    assert_eq!(p["baro_alt_ft"], 30000);
    assert!((p["selected_heading_deg"].as_f64().unwrap() - 90.0).abs() < 0.5);
    b.feed(&f("91222222235094C2C608201C5E99"), 1.5, -30.0);
    b.feed(
        &es_frame(17, 5, 0x222222, me_ident('A', 3, "OTHER")),
        1.6,
        -30.0,
    );
    let d = b.report(1.7);
    let ids: Vec<&str> = d["aircraft"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["icao"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"~222222") && ids.contains(&"222222"),
        "{ids:?}"
    );
    b.feed(&f("8D333333586612AAAA8000832F22"), 1.8, -30.0);
    assert_eq!(b.ac(1.9, "333333")["baro_alt_ft"], 35000);
}

#[test]
fn expiry_and_limits() {
    let mut b = Bench::new();
    b.feed(&df11(5, 0xAAAAAA, 0), 1.0, -30.0);
    b.feed(&df11(5, 0xBBBBBB, 0), 30.0, -30.0);
    assert_eq!(b.report(50.0)["aircraft_count"], 2);
    assert_eq!(b.report(61.5)["aircraft_count"], 1);
    assert_eq!(b.report(91.0)["aircraft_count"], 0);
    // A flood of addresses: at most 2000 in the table, 160 in the published table, 64 messages.
    let mut b = Bench::new();
    for i in 0..2500u32 {
        let a = 0x400000 + i;
        b.feed(
            &es_frame(17, 5, a, me_ident('A', 3, "ABCDEFGH")),
            1.0 + i as f64 * 1e-4,
            -30.0,
        );
    }
    let d = b.report(1.5);
    assert_eq!(d["aircraft_count"], 2000);
    assert_eq!(
        d["aircraft"].as_array().unwrap().len(),
        sdrkit_stack_adsb::typed::AIRCRAFT.cap
    );
    assert_eq!(d["messages"].as_array().unwrap().len(), 64);
    assert_eq!(d["aircraft"][0]["icao"], format!("{:06X}", 0x400000 + 2499));
    // 128 aircraft with the longest callsign and full trails stay under 400 kB of JSON (typed and text cells).
    let mut b = Bench::new();
    let alt = codes::encode_ac12_25ft(35000).unwrap();
    for k in 0..90 {
        for i in 0..128u32 {
            let t = k as f64 * 5.1 + i as f64 * 1e-3;
            let (lat, lon) = (
                40.0 + i as f64 * 0.01 + k as f64 * 0.001,
                10.0 + k as f64 * 0.002,
            );
            b.feed(
                &es_frame(17, 5, 0x500000 + i, me_ident('A', 3, "WWWWWWWW")),
                t,
                -30.0,
            );
            b.feed(
                &es_frame(
                    17,
                    5,
                    0x500000 + i,
                    me_airborne(11, 0, alt, false, lat, lon),
                ),
                t + 0.1,
                -30.0,
            );
            b.feed(
                &es_frame(17, 5, 0x500000 + i, me_airborne(11, 0, alt, true, lat, lon)),
                t + 0.2,
                -30.0,
            );
        }
    }
    let d = b.report(90.0 * 5.1);
    use sdrkit_graph::Component;
    let size = serde_json::to_string(b.t.results().unwrap()).unwrap().len();
    eprintln!("results of 128 aircraft with full trails: {size} bytes of JSON");
    assert!(size < 400_000, "{size}");
    assert!(crate::sdrkit_stack_adsb::common::trail_points(&d["aircraft"][0]["trail"]) == 80);
}

#[test]
fn derived_weather_and_advisories() {
    // An aircraft over Lithuania flying true north at TAS 400 kt, Mach 0.687, wind from 270 at 50 kt.
    let a = 0x503C11;
    let (lat, lon) = (54.7, 25.3);
    let mut b = Bench::new().rx(lat, lon);
    b.feed(&df11(5, a, 0), 0.5, -30.0);
    let decl = sdrkit_proto_aero::air::declination_deg(lat, lon).unwrap();
    let mag = (360.0 - decl).rem_euclid(360.0);
    let (gs, trk) = (403.1, 7.125);
    let alt = codes::encode_ac13_25ft(36000).unwrap();
    let r = 0.4;
    for k in 0..3 {
        let t = 1.0 + k as f64;
        b.feed(
            &df4_20(0, alt, Some(commb::encode_50(0.0, trk, gs, 0.0, 400.0)), a),
            t,
            -30.0,
        );
        b.feed(
            &df4_20(0, alt, Some(commb::encode_60(mag, 250.0, 0.687, 0, 0)), a),
            t + r,
            -30.0,
        );
    }
    let p = b.ac(4.0, "503C11");
    let (from, speed) = (
        p["wind_derived"][0].as_f64().unwrap(),
        p["wind_derived"][1].as_f64().unwrap(),
    );
    assert!(
        (from - 270.0).abs() < 4.0 && (speed - 50.0).abs() < 4.0,
        "{from} {speed}"
    );
    assert!((p["sat_derived_c"].as_f64().unwrap() + 50.0).abs() < 2.0);
    // In a turn, no wind.
    let mut b2 = Bench::new().rx(lat, lon);
    b2.feed(&df11(5, a, 0), 0.5, -30.0);
    b2.feed(
        &df4_20(0, alt, Some(commb::encode_50(20.0, trk, gs, 1.9, 400.0)), a),
        1.0,
        -30.0,
    );
    b2.feed(
        &df4_20(0, alt, Some(commb::encode_60(mag, 250.0, 0.687, 0, 0)), a),
        1.4,
        -30.0,
    );
    assert!(b2.ac(2.0, "503C11").get("wind_derived").is_none());
    // A stale pair, no wind.
    let mut b3 = Bench::new().rx(lat, lon);
    b3.feed(&df11(5, a, 0), 0.5, -30.0);
    b3.feed(
        &df4_20(0, alt, Some(commb::encode_50(0.0, trk, gs, 0.0, 400.0)), a),
        1.0,
        -30.0,
    );
    b3.feed(
        &df4_20(0, alt, Some(commb::encode_60(mag, 250.0, 0.687, 0, 0)), a),
        6.0,
        -30.0,
    );
    assert!(b3.ac(6.5, "503C11").get("wind_derived").is_none());
    // A reported 4,4 and a 3,0.
    b.feed(
        &df4_20(
            0,
            alt,
            Some(commb::encode_44(
                45.0,
                280.0,
                -52.25,
                Some(227.0),
                Some(1),
                Some(30.0),
            )),
            a,
        ),
        5.0,
        -30.0,
    );
    b.feed(
        &df4_20(
            0,
            alt,
            Some(commb::encode_30(0b11 << 12, 0, false, Some(0x896123))),
            a,
        ),
        5.5,
        -30.0,
    );
    let p = b.ac(6.0, "503C11");
    assert_eq!(p["sat_reported_c"], -52.25);
    assert!((p["wind_reported"][1].as_f64().unwrap() - 45.0).abs() < 1e-9);
    assert!(
        p["advisory"]
            .as_str()
            .unwrap()
            .starts_with("descend, corrective")
            || p["advisory"]
                .as_str()
                .unwrap()
                .starts_with("climb, corrective")
    );
    assert_eq!(p["advisory_threat"], "896123");
    b.feed(&df11(5, a, 0), 40.0, -30.0);
    b.feed(&df11(5, a, 0), 66.0, -30.0);
    assert!(b.ac(66.5, "503C11").get("advisory").is_none());
}

#[test]
fn df21_weather_needs_the_altitude() {
    let a = 0x4CA123;
    let m44 = commb::encode_44(45.0, 280.0, -52.25, Some(227.0), Some(1), None);
    let alt36 = codes::encode_ac12_25ft(36000).unwrap();
    // With an ADS-B altitude 5 s old.
    let mut b = Bench::new();
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt36, false, 47.0, 8.0)),
        1.0,
        -30.0,
    );
    b.feed(&df5_21(0, 0o1200, Some(m44), a), 6.0, -30.0);
    assert_eq!(b.ac(6.1, "4CA123")["sat_reported_c"], -52.25);
    // With a DF4 altitude.
    let mut b = Bench::new();
    b.feed(&df11(5, a, 0), 1.0, -30.0);
    b.feed(
        &df4_20(0, codes::encode_ac13_25ft(36000).unwrap(), None, a),
        2.0,
        -30.0,
    );
    b.feed(&df5_21(0, 0o1200, Some(m44), a), 3.0, -30.0);
    assert_eq!(b.ac(3.1, "4CA123")["sat_reported_c"], -52.25);
    // Without an altitude, with one 40 s old, or at 5000 ft.
    let mut b = Bench::new();
    b.feed(&df11(5, a, 0), 1.0, -30.0);
    b.feed(&df5_21(0, 0o1200, Some(m44), a), 2.0, -30.0);
    assert!(b.ac(2.1, "4CA123").get("sat_reported_c").is_none());
    let mut b = Bench::new();
    b.feed(
        &es_frame(17, 5, a, me_airborne(11, 0, alt36, false, 47.0, 8.0)),
        1.0,
        -30.0,
    );
    b.feed(&df11(5, a, 0), 30.0, -30.0);
    b.feed(&df5_21(0, 0o1200, Some(m44), a), 41.0, -30.0);
    assert!(b.ac(41.1, "4CA123").get("sat_reported_c").is_none());
    let mut b = Bench::new();
    b.feed(
        &es_frame(
            17,
            5,
            a,
            me_airborne(
                11,
                0,
                codes::encode_ac12_25ft(5000).unwrap(),
                false,
                47.0,
                8.0,
            ),
        ),
        1.0,
        -30.0,
    );
    b.feed(&df5_21(0, 0o1200, Some(m44), a), 2.0, -30.0);
    assert!(b.ac(2.1, "4CA123").get("sat_reported_c").is_none());
}

#[test]
fn coverage_bands_and_saved_state() {
    use sdrkit_stack_adsb::table::{band, Coverage};
    assert_eq!(band(Some(0), false), Some(0));
    assert_eq!(band(Some(9999), false), Some(0));
    assert_eq!(band(Some(10000), false), Some(1));
    assert_eq!(band(Some(35000), false), Some(3));
    assert_eq!(band(Some(40000), false), Some(4));
    assert_eq!(band(Some(45000), false), Some(4));
    assert_eq!(band(None, false), None);
    assert_eq!(band(None, true), Some(0));
    let mut c = Coverage::default();
    c.add(60.0, 0.5, Some(35000), false);
    c.add(30.0, 0.5, Some(5000), false);
    c.add(36.9, 90.0, Some(15000), false);
    c.add(60.0, 180.0, Some(35000), false);
    c.add(80.0, 270.0, None, false);
    assert_eq!(c.sectors[0][0], 60.0);
    assert_eq!(c.sectors[1][0], 30.0);
    assert_eq!(c.sectors[4][0], 60.0);
    assert_eq!(c.sectors[2][9], 36.9);
    assert_eq!(c.sectors[4][18], 60.0);
    assert_eq!(c.sectors[0][27], 80.0);
    assert!((1..6).all(|b| c.sectors[b][27] == 0.0));
    let saved = serde_json::to_value(&c).unwrap();
    let back: Coverage = serde_json::from_value(saved).unwrap();
    assert_eq!(back, c);
}

#[test]
fn messages_per_second_reads_the_last_second() {
    let mut b = Bench::new();
    for i in 0..40 {
        b.feed(&df11(5, 0x400001 + i, 0), 0.01 + i as f64 * 0.024, -30.0);
    }
    for k in 1..=4 {
        let st = CheckStats {
            demod: DemodStats {
                time: k as f64 * 0.25,
                ..Default::default()
            },
            good: b.c.stats().good.min(10 * k as u64),
            ..Default::default()
        };
        b.t.take(&Msg::Stats(st));
    }
    let mps = b.t.messages_per_second();
    assert!((mps - 40.0).abs() <= 0.5, "{mps}");
    // Clear empties the table and keeps the receiver position.
    let mut b = Bench::new().rx(10.0, 20.0);
    b.feed(&df11(5, 0x400001, 0), 1.0, -30.0);
    invoke(&mut b.t, "clear");
    let d = b.report(1.5);
    assert_eq!(d["aircraft_count"], 0);
    assert_eq!(b.t.table.station, Some((10.0, 20.0)));
    let _ = PosKind::Global;
}

#[test]
fn position_hint_gps_cells_csv_and_sessions() {
    use sdrkit_graph::Component;
    use sdrkit_stack_adsb::table::Coverage;
    let alt = codes::encode_ac12_25ft(30000).unwrap();
    let pair = |b: &mut Bench, a: u32, t: f64, lat: f64, lon: f64| {
        b.feed(
            &es_frame(17, 5, a, me_airborne(11, 0, alt, false, lat, lon)),
            t,
            -30.0,
        );
        b.feed(
            &es_frame(17, 5, a, me_airborne(11, 0, alt, true, lat, lon)),
            t + 0.5,
            -30.0,
        );
    };
    // Three aircraft more than 500 NM away offer their median position.
    let mut b = Bench::new().rx(10.0, -20.0);
    apply(&mut b.t, "gps_layer", sdrkit_core::Value::Bool(true));
    let pos = [(47.1, 8.6), (47.2, 8.7), (47.3, 8.8), (47.4, 8.9)];
    for (i, &(lat, lon)) in pos.iter().enumerate() {
        let a = 0x4B0000 + i as u32;
        // Two aircraft report a poor NACp (degraded), two a good one.
        b.feed(
            &es_frame(17, 5, a, me_op_status(2, if i < 2 { 5 } else { 9 }, 3)),
            1.0,
            -30.0,
        );
        b.feed(&es_frame(17, 5, a, me_ident('A', 3, "HINT")), 1.1, -30.0);
        pair(&mut b, a, 2.0 + i as f64, lat, lon);
    }
    let d = b.report(7.0);
    assert_eq!(d["with_position"], 4);
    let h = &d["position_hint"];
    assert!(
        (h[0].as_f64().unwrap() - 47.3).abs() < 1e-3 && (h[1].as_f64().unwrap() - 8.8).abs() < 1e-3,
        "{h}"
    );
    // One 0.5 degree cell with four aircraft, half degraded: level 2. No identities in it.
    let cells = d["gps_cells"].as_array().unwrap();
    assert_eq!(cells.len(), 1, "{cells:?}");
    assert_eq!(
        (
            &cells[0]["lat"],
            &cells[0]["lon"],
            &cells[0]["aircraft"],
            &cells[0]["level"]
        ),
        (
            &serde_json::json!(47.0),
            &serde_json::json!(8.5),
            &serde_json::json!(4),
            &serde_json::json!(2)
        )
    );
    // Near the aircraft there is no hint.
    let mut near = Bench::new().rx(47.0, 8.5);
    for (i, &(lat, lon)) in pos.iter().enumerate() {
        pair(&mut near, 0x4B0000 + i as u32, 2.0 + i as f64, lat, lon);
    }
    assert!(near.report(7.0)["position_hint"].is_null());

    // The saved state loads back onto another signal clock with the same ages.
    let saved = b.t.save_state().unwrap();
    let mut c = Bench::new();
    c.report(500.0);
    c.t.restore_state(&saved).unwrap();
    let d = c.report(500.25);
    assert_eq!(d["aircraft_count"], 4);
    let a = d["aircraft"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["icao"] == "4B0003")
        .unwrap();
    assert_eq!(a["callsign"], "HINT");
    let age =
        c.t.table
            .held
            .aircraft
            .rows()
            .seen(&sdrkit_proto_aero::aircraft::Key::Address(0x4B0003))
            .unwrap()
            .age(500.25);
    assert!((age - (7.25 - 5.5)).abs() < 0.3, "age {age}");
    // The statistics survive a save and load; "reset statistics" empties them.
    let mut s = Bench::new().rx(47.0, 8.5);
    pair(&mut s, 0x4B0010, 1.0, 47.9, 8.5);
    s.report(2.0);
    assert!(s.t.table.held.coverage.max_range_nm > 50.0);
    let saved = s.t.save_state().unwrap();
    let mut s2 = Bench::new();
    s2.t.restore_state(&saved).unwrap();
    // The ranges come back; the times move to the new signal clock.
    let same = |a: &Coverage, b: &Coverage| {
        a.sectors == b.sectors && a.histogram == b.histogram && a.positions == b.positions
    };
    assert!(same(&s2.t.table.held.coverage, &s.t.table.held.coverage));
    assert_eq!(s2.t.table.held.coverage.series[0].0, 0.0);
    // A state of another format is refused, and nothing changes.
    let other = sdrkit_core::StateBlob::new("adsb.other", saved.version(), saved.bytes().to_vec());
    assert!(s2.t.restore_state(&other).is_err());
    assert!(same(&s2.t.table.held.coverage, &s.t.table.held.coverage));
    invoke(&mut s.t, "reset_stats");
    assert_eq!(s.t.table.held.coverage, Coverage::default());
    assert_eq!(s.report(2.25)["aircraft_count"], 1);
    // Without a receiver position there are no ranges.
    let mut n = Bench::new();
    pair(&mut n, 0x4B0011, 1.0, 47.9, 8.5);
    n.report(2.0);
    assert_eq!(n.t.table.held.coverage.max_range_nm, 0.0);
}

/// Runs the command `key` of a component with its default arguments.
fn invoke(c: &mut dyn sdrkit_graph::Component, key: &str) {
    let (id, args) = {
        let cmds = c.commands();
        let id = cmds.id(key).unwrap();
        let args = cmds.check(id, &cmds.defs()[id.index()].new_args()).unwrap();
        (id, args)
    };
    c.invoke(id, &args).unwrap();
}

/// Sets the setting `key` of a component to `v`.
fn apply(c: &mut dyn sdrkit_graph::Component, key: &str, v: sdrkit_core::Value) {
    let id = c.info().settings.id(key).unwrap();
    c.apply(id, &v).unwrap();
}

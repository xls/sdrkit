//! The navigation message vectors of `tests/vectors/gnss/nav_bits.json`: LNAV subframes of G03 with their
//! ephemeris, iono and UTC; 32 SBAS messages of PRN 131 with the decoded state and the coded symbols of message 1.

use sdrkit_dev_test::sigmf::vectors_dir;
use sdrkit_stack_gnss::fec::{SoftViterbi, K7_171_133};
use sdrkit_stack_gnss::lnav::{self, LnavCollector, LnavContent};
use sdrkit_stack_gnss::sbas::{self, SbasContent};
use sdrkit_stack_gnss::system::System;

fn nav_bits() -> serde_json::Value {
    let p = vectors_dir().join("gnss").join("nav_bits.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-30) + 1e-30
}

#[test]
fn lnav_subframes_decode_to_the_expected_ephemeris() {
    let v = nav_bits();
    let l = &v["lnav"];
    let mut col = LnavCollector::default();
    let mut eph = None;
    let mut iono_utc = None;
    for s in l["subframes"].as_array().unwrap() {
        let bits = from_hex(s["hex"].as_str().unwrap(), 300);
        let sf = lnav::check_subframe(&bits).expect("parity");
        assert_eq!(sf.id as i64, s["id"].as_i64().unwrap());
        assert_eq!(sf.tow_count as i64, s["tow_count"].as_i64().unwrap());
        // The subframe began at (TOW - 1) x 6 s: its index in the week.
        assert_eq!(sf.start_tow(), s["subframe_index"].as_f64().unwrap() * 6.0);
        if s["sv_id"].as_i64().unwrap() >= 0 {
            assert_eq!(sf.sv_id() as i64, s["sv_id"].as_i64().unwrap());
        }
        let c = lnav::decode(&sf, System::Gps);
        if let LnavContent::IonoUtc(k, u) = &c {
            iono_utc = Some((*k, *u));
        }
        if let Some(e) = col.add(c, System::Gps, 3, |w| {
            sdrkit_stack_gnss::geo::resolve_week(w as i64, 1024, 2393)
        }) {
            eph = Some(e);
        }
    }
    let e = eph.expect("complete ephemeris");
    let x = &l["ephemeris"];
    assert_eq!(e.week, 2400);
    assert_eq!(e.week % 1024, x["wn10"].as_i64().unwrap());
    assert_eq!(e.iodc as i64, x["iodc"].as_i64().unwrap());
    assert_eq!(e.iode as i64, x["iode"].as_i64().unwrap());
    assert_eq!(e.health as i64, x["health"].as_i64().unwrap());
    assert_eq!(e.ura as i64, x["ura_index"].as_i64().unwrap());
    for (name, got) in [
        ("tgd", e.tgd),
        ("toc", e.toc),
        ("af0", e.af0),
        ("af1", e.af1),
        ("af2", e.af2),
        ("toe", e.toe),
        ("sqrtA", e.sqrt_a),
        ("e", e.e),
        ("m0", e.m0),
        ("delta_n", e.delta_n),
        ("omega0", e.omega0),
        ("i0", e.i0),
        ("omega", e.omega),
        ("omega_dot", e.omega_dot),
        ("idot", e.idot),
        ("cuc", e.cuc),
        ("cus", e.cus),
        ("crc", e.crc),
        ("crs", e.crs),
        ("cic", e.cic),
        ("cis", e.cis),
    ] {
        let want = x[name].as_f64().unwrap();
        assert!(close(got, want, 1e-12), "{name}: {got} != {want}");
    }
    let (k, u) = iono_utc.expect("page 18");
    let io = &l["iono"];
    for i in 0..4 {
        assert!(close(k.alpha[i], io["alpha"][i].as_f64().unwrap(), 1e-6));
        assert!(close(k.beta[i], io["beta"][i].as_f64().unwrap(), 1e-6));
    }
    let ut = &l["utc"];
    assert!(close(u.a0, ut["a0"].as_f64().unwrap(), 1e-6));
    assert!(close(u.a1, ut["a1"].as_f64().unwrap(), 1e-6));
    assert_eq!(u.tot, ut["tot"].as_f64().unwrap());
    assert_eq!(u.wnt as i64, ut["wnt"].as_i64().unwrap());
    assert_eq!(u.dtls as i64, ut["dtls"].as_i64().unwrap());
    assert_eq!(u.wnlsf as i64, ut["wnlsf"].as_i64().unwrap());
    assert_eq!(u.dn as i64, ut["dn"].as_i64().unwrap());
    assert_eq!(u.dtlsf as i64, ut["dtlsf"].as_i64().unwrap());
}

#[test]
fn sbas_messages_and_decoded_state() {
    let v = nav_bits();
    let s = &v["sbas"];
    let prn = s["prn"].as_u64().unwrap() as u16;
    let msgs = s["messages"].as_array().unwrap();
    let mut c = SbasContent::default();
    for (i, m) in msgs.iter().enumerate() {
        let bits = from_hex(m["hex"].as_str().unwrap(), 250);
        let (ty, pre) = sbas::check(&bits).expect("CRC");
        assert_eq!(ty as i64, m["type"].as_i64().unwrap());
        assert_eq!(pre as i64, m["preamble_index"].as_i64().unwrap());
        c.add(prn, &bits, i as f64);
    }
    // The coded symbols of message 1, with the encoder state left by message 0.
    let m0 = from_hex(msgs[0]["hex"].as_str().unwrap(), 250);
    let m1 = from_hex(msgs[1]["hex"].as_str().unwrap(), 250);
    let mut enc = sdrkit_algo_fec::conv::Encoder::new(K7_171_133);
    let mut scratch = Vec::new();
    enc.encode(&m0[244..], &mut scratch);
    let mut sym = Vec::new();
    enc.encode(&m1, &mut sym);
    let want = from_hex(msgs[1]["symbols_hex"].as_str().unwrap(), 500);
    assert_eq!(sym, want);
    // And they decode back.
    let soft: Vec<f32> = sym.iter().map(|&b| 1.0 - 2.0 * b as f32).collect();
    let mut out = Vec::new();
    SoftViterbi::new(K7_171_133, 600).decode(&soft, sdrkit_algo_fec::conv::Ends::Open, &mut out);
    assert_eq!(&out[..240], &m1[..240]);

    let d = &s["decoded_after_all"];
    let sat = &c.sats[&prn];
    assert_eq!(sat.messages as i64, d["messages"].as_i64().unwrap());
    let mask = sat.mask.as_ref().expect("mask");
    assert_eq!(mask.iodp as i64, d["iodp"].as_i64().unwrap());
    assert_eq!(mask.slots.len() as i64, d["masked"].as_i64().unwrap());
    assert_eq!(mask.slots[0] as i64, d["slot1_prn"].as_i64().unwrap());
    assert_eq!(mask.slots[13] as i64, d["slot14_prn"].as_i64().unwrap());
    assert_eq!(mask.slots[14] as i64, d["slot15_prn"].as_i64().unwrap());
    let (with, usable, _, _) = sat.fast_summary();
    assert_eq!(with as i64, d["fast_slots"].as_i64().unwrap());
    assert_eq!(usable as i64, d["usable"].as_i64().unwrap());
    let f1 = sat.fast[1].unwrap();
    assert_eq!(f1.prc.unwrap(), d["slot1_prc_m"].as_f64().unwrap());
    assert_eq!(f1.udrei as i64, d["slot1_udrei"].as_i64().unwrap());
    assert_eq!(sat.tlat.unwrap(), d["t_lat_s"].as_f64().unwrap());
    let g = sat.geo.unwrap();
    assert_eq!(g.iodn as i64, d["geo_iodn"].as_i64().unwrap());
    assert_eq!(g.ura as i64, d["geo_ura"].as_i64().unwrap());
    for i in 0..3 {
        assert!((g.pos[i] - d["geo_pos_m"][i].as_f64().unwrap()).abs() < 1e-3);
    }
    let t = sat.time.unwrap();
    assert_eq!(t.dtls as i64, d["utc_dtls"].as_i64().unwrap());
    assert_eq!(t.week as i64, d["utc_wn"].as_i64().unwrap());
    assert_eq!(t.utc_id as i64, d["utc_id"].as_i64().unwrap());
    assert!(close(t.a0, d["utc_a0_s"].as_f64().unwrap(), 1e-6));
    let prns: Vec<i64> = sat.almanac.iter().map(|a| a.prn as i64).collect();
    let want: Vec<i64> = d["almanac_prns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect();
    assert_eq!(prns, want);
    assert_eq!(sat.almanac[0].pos[0], d["almanac0_x_m"].as_f64().unwrap());
    let io = &sat.iono_summary()[0];
    assert_eq!(io.points as i64, d["iono_band0_igps"].as_i64().unwrap());
    assert_eq!(
        io.blocks_needed as i64,
        d["iono_band0_blocks_needed"].as_i64().unwrap()
    );
    assert_eq!(io.values as i64, d["iono_band0_values"].as_i64().unwrap());
    assert_eq!(io.min_m, d["iono_band0_min_m"].as_f64().unwrap());
    assert_eq!(io.max_m, d["iono_band0_max_m"].as_f64().unwrap());
    let deg = sat.degradation.as_ref().unwrap();
    assert_eq!(deg["brrc"] as i64, d["deg_brrc"].as_i64().unwrap());
    assert_eq!(deg["cer"] as i64, d["deg_cer"].as_i64().unwrap());
    assert_eq!(
        deg["ciono_ramp"] as i64,
        d["deg_ciono_ramp"].as_i64().unwrap()
    );
    assert_eq!(
        sat.long_term.len() as i64,
        d["long_term_slots"].as_i64().unwrap()
    );
}

/// `n` bits from hex digits, most significant first.
fn from_hex(hex: &str, n: usize) -> Vec<u8> {
    let mut bits = Vec::new();
    sdrkit_util_bits::pack::hex_to_bits(hex, &mut bits).unwrap();
    bits.truncate(n);
    bits
}

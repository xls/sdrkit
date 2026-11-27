//! The `wifi` stack: the element readers, the shared PHYs at every rate, generator to receiver round trips, the settings and the
//! recordings.

use super::help::{
    cnum, command, config, feed, impaired, num, rows, set, snr, text, value, wifi_domains,
    wifi_plan,
};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::{
    Choice, Complex32, InputInfo, Owner, Receiver, Results, SettingSet, StateBlob, Value,
};
use sdrkit_graph::StackGraph;
use sdrkit_proto_wlan_phy::{dsss, ofdm};
use sdrkit_stack_wlan::typed::{WifiBeaconRow as B, WifiNetworkRow as N, WifiRateRow};
use sdrkit_stack_wlan::wifi::frames::{
    beacon, data_frame, fils, probe_response, rsn_body, short_ssid, BssSpec, HeSpec, RnrSpec, Sec,
};
use sdrkit_stack_wlan::wifi::gen::{
    scene_24, scene_56, GenFrame, Phy, WifiGenParams, WifiGenerator, CLIENT,
};
use sdrkit_stack_wlan::wifi::mac::*;
use sdrkit_stack_wlan::wifi::plan::{all_channels, centre_mhz, wide_centre, Band};
use sdrkit_stack_wlan::wifi::table::{Network, Store};
use sdrkit_stack_wlan::wifi::{self, WifiSetting};

const AVM: &str = "AVM GmbH";
const UBNT: &str = "Ubiquiti Inc";

fn hex(t: &str) -> Vec<u8> {
    let mut v = Vec::new();
    sdrkit_core::base64::unhex_into(t, &mut v).unwrap();
    v
}

fn el(id: u8, d: &[u8]) -> Vec<u8> {
    let mut v = vec![id, d.len() as u8];
    v.extend_from_slice(d);
    v
}

fn ext(id: u8, d: &[u8]) -> Vec<u8> {
    let mut v = vec![id];
    v.extend_from_slice(d);
    el(255, &v)
}

fn ht_caps(info: u16, masks: &[u8]) -> Vec<u8> {
    let mut d = info.to_le_bytes().to_vec();
    d.push(0);
    let mut m = [0u8; 16];
    m[..masks.len()].copy_from_slice(masks);
    d.extend(m);
    d.extend([0u8; 7]);
    el(45, &d)
}

#[test]
fn ht_vht_he_eht_elements() {
    let mut b = ht_caps(0x0062, &[0xFF, 0xFF]);
    b.extend(el(61, &[36, 0x05, 0, 0, 0, 0]));
    let e = elements(&b);
    assert_eq!(e.count, 2);
    let h = e.ht.unwrap();
    assert!(h.forty && h.sgi);
    assert_eq!(h.streams, 2);
    assert_eq!(e.ht_op.unwrap().primary, 36);
    let c = combine(&e, Some(Band::G5));
    assert_eq!(
        (c.primary, c.width, c.standard.as_str()),
        (Some(36), 40, "a/n")
    );
    assert!(
        c.caps.contains("Wi-Fi 4") && c.caps.contains("2x2") && c.caps.contains("300 Mbit/s"),
        "{}",
        c.caps
    );
    let r10 = |x: f64| (x * 10.0).round() as i64;
    assert_eq!(
        [
            r10(ht_rate(20, false)),
            r10(ht_rate(20, true)),
            r10(ht_rate(40, false)),
            r10(2.0 * ht_rate(40, true))
        ],
        [650, 722, 1350, 3000]
    );
    // VHT
    let vc = hex("24000000FAFF0000FAFF0000");
    let mut b = el(191, &vc);
    b.extend(el(192, &[1, 42, 0, 0, 0]));
    b.extend(el(61, &[36, 0, 0, 0, 0, 0]));
    let e = elements(&b);
    let v = e.vht.unwrap();
    assert_eq!(
        (v.width_set, v.sgi80, v.streams, v.max_mcs),
        (1, true, 2, 9)
    );
    assert_eq!(e.vht_op, Some((1, 42, 0)));
    let c = combine(&e, Some(Band::G5));
    assert_eq!(
        (c.width, c.centre, c.standard.as_str()),
        (80, Some(42), "a/ac")
    );
    assert_eq!(r10(2.0 * vht_rate(160, 9)), 17333);
    assert_eq!(r10(vht_rate(80, 9)), 4333);
    assert_eq!(r10(vht_rate(20, 8)), 867);
    assert_eq!(r10(vht_rate(40, 9)), 2000);
    let mut b = el(191, &vc);
    b.extend(el(192, &[1, 42, 50, 0, 0]));
    let c = combine(&elements(&b), Some(Band::G5));
    assert_eq!((c.width, c.centre), (160, Some(50)));
    // HE
    let mut hc = vec![0x04, 0, 0, 0, 0, 0, 0x04];
    hc.extend([0u8; 10]);
    hc.extend([0xFA, 0xFF, 0xFA, 0xFF]);
    let mut b = ext(35, &hc);
    b.extend(ext(
        36,
        &[0x00, 0x00, 0x02, 0x05, 0xFC, 0xFF, 5, 0x02, 7, 0, 0],
    ));
    let e = elements(&b);
    let h = e.he.unwrap();
    assert!(!h.twt_req && h.twt_resp);
    assert_eq!((h.streams, h.max_mcs, h.width_set), (2, 11, 0x02));
    let o = e.he_op.unwrap();
    assert_eq!((o.color, o.color_disabled), (5, false));
    let s = o.six.unwrap();
    assert_eq!(
        (s.primary, s.width, s.ccfs0, s.ccfs1, s.reg, s.duplicate),
        (5, 80, 7, 0, 0, false)
    );
    let c = combine(&e, Some(Band::G6));
    assert_eq!(
        (c.primary, c.width, c.centre, c.standard.as_str()),
        (Some(5), 80, Some(7), "ax")
    );
    assert_eq!(
        [
            r10(he_rate(80, 11)),
            r10(he_rate(20, 11)),
            r10(he_rate(160, 11)),
            r10(he_rate(40, 11))
        ],
        [6005, 1434, 12010, 2868]
    );
    for want in ["Wi-Fi 6", "1201 Mbit/s", "TWT", "BSS color 5"] {
        assert!(c.caps.contains(want), "{want}: {}", c.caps);
    }
    let b = ext(
        36,
        &[
            0x00,
            0x00,
            0x02,
            0x83,
            0xFC,
            0xFF,
            37,
            0x03 | 0x04 | 1 << 3,
            47,
            47,
            0,
        ],
    );
    let e = elements(&b);
    let o = e.he_op.unwrap();
    assert_eq!((o.color, o.color_disabled), (3, true));
    let s = o.six.unwrap();
    assert_eq!((s.primary, s.width, s.duplicate, s.reg), (37, 160, true, 1));
    assert_eq!(combine(&e, Some(Band::G6)).centre, Some(47));
    // EHT
    let mut b = ext(106, &hex("0100000000041F0F"));
    b.extend(ext(108, &[0u8; 15]));
    let e = elements(&b);
    assert_eq!(e.eht_op, Some((320, 31, 15)));
    let c = combine(&e, Some(Band::G6));
    assert_eq!((c.width, c.standard.as_str()), (320, "be"));
    assert!(c.caps.contains("Wi-Fi 7"));
}

#[test]
fn rnr_rsn_country_and_more() {
    let mut d = hex("000C830510");
    d.extend([0x24, 0xA4, 0x3C, 0x60, 0x00, 0x01]);
    d.extend([0x78, 0x56, 0x34, 0x12, 0x62]);
    let e = elements(&el(201, &d));
    assert_eq!(e.rnr.len(), 1);
    let n = &e.rnr[0];
    assert_eq!((n.op_class, n.channel, n.band), (131, 5, Some(Band::G6)));
    assert_eq!(n.bssid, Some([0x24, 0xA4, 0x3C, 0x60, 0x00, 0x01]));
    assert_eq!(n.short_ssid, Some(0x1234_5678));
    assert!(n.co_located() && n.unsolicited() && n.same_ssid() && !n.ess_member());
    let mut d = vec![0x10, 7, 133, 21];
    for i in 0..2u8 {
        d.push(0xFF);
        d.extend([0x24, 0xA4, 0x3C, 0x60, 0x00, i]);
    }
    d.extend([0x00, 5, 134, 37, 0xFF, 1, 2, 3, 4]);
    let e = elements(&el(201, &d));
    assert_eq!(e.rnr.len(), 3);
    assert!(e.rnr[1].bssid.is_some() && e.rnr[1].params.is_none());
    assert_eq!(
        (e.rnr[2].short_ssid, e.rnr[2].bssid),
        (Some(0x0403_0201), None)
    );
    // RSN
    let e = elements(&el(48, &rsn_body(&[4], &[8, 2], true, false)));
    let s = security(&e, 0x11);
    assert_eq!((s.akms.as_str(), s.pmf.as_str()), ("SAE, PSK", "capable"));
    let s = security(&elements(&el(48, &rsn_body(&[4], &[18], true, true))), 0x01);
    assert_eq!(
        (s.akms.as_str(), s.pmf.as_str(), s.name.as_str()),
        ("OWE", "required", "OWE (Enhanced Open)")
    );
    let s = security(
        &elements(&el(48, &rsn_body(&[4], &[1, 5], false, false))),
        0x11,
    );
    assert_eq!(
        (s.akms.as_str(), s.pmf.as_str()),
        ("802.1X, 802.1X-SHA256", "no")
    );
    // Country, power constraint, mobility domain.
    let mut b = el(7, &[b'D', b'E', b'I', 36, 4, 23]);
    b.extend(el(32, &[3]));
    b.extend(el(54, &[0x34, 0x12, 0x01]));
    let e = elements(&b);
    assert_eq!(e.country.as_deref(), Some("DEI"));
    assert_eq!((e.power_constraint, e.mdid), (Some(3), Some(0x1234)));
    assert_eq!(centre_mhz(Band::G2, 6), Some(2437.0));
}

fn parse(f: &[u8]) -> Bss {
    parse_bss(&f[..f.len() - 4]).expect("a BSS frame")
}

#[test]
fn beacons_fils_and_probe_responses() {
    // A 5 GHz beacon parsed on 5 GHz.
    let mut a = BssSpec::new(
        [0x24, 0xA4, 0x3C, 0x01, 0x02, 0x03],
        "Five",
        Band::G5,
        44,
        Sec::Wpa2,
    );
    a.width = 80;
    a.ht = Some(2);
    a.vht = Some(2);
    a.he = Some(HeSpec {
        streams: 2,
        twt: false,
        color: 9,
        color_disabled: false,
        reg: 0,
        dup: false,
    });
    a.country = Some("LT ".into());
    a.power_constraint = Some(6);
    a.mdid = Some(0x00AB);
    let b = parse(&beacon(&a));
    let c = combine(&b.el, Some(Band::G5));
    assert_eq!(
        (c.channel, c.primary, c.width, c.centre),
        (Some(44), Some(44), 80, Some(42))
    );
    assert_eq!(c.standard, "a/n/ac/ax");
    assert_eq!(b.el.he_op.unwrap().color, 9);
    assert_eq!(b.el.country.as_deref(), Some("LT "));
    assert_eq!((b.el.power_constraint, b.el.mdid), (Some(6), Some(0xAB)));
    assert_eq!(security(&b.el, b.capability).name, "WPA2");
    assert_eq!(rates_text(&b.el.rates), "6 9 12 18 24 36 48 54");
    // The same AP on 6 GHz channel 37.
    let mut s = a.clone();
    s.band = Band::G6;
    s.channel = 37;
    s.width = 160;
    s.ht = None;
    s.vht = None;
    s.he = Some(HeSpec {
        streams: 2,
        twt: false,
        color: 9,
        color_disabled: false,
        reg: 1,
        dup: false,
    });
    s.eht = true;
    s.sec = Sec::Wpa3;
    let b = parse(&beacon(&s));
    let c = combine(&b.el, Some(Band::G6));
    assert_eq!(
        (c.channel, c.width, c.centre, c.standard.as_str()),
        (Some(37), 160, Some(47), "ax/be")
    );
    assert_eq!(b.el.he_op.unwrap().six.unwrap().reg, 1);
    let sec = security(&b.el, b.capability);
    assert_eq!((sec.name.as_str(), sec.pmf.as_str()), ("WPA3", "required"));
    let f = parse(&fils(&s));
    assert_eq!(
        (f.frame, f.ssid.as_str(), f.interval_tu),
        (BssFrame::Fils, "Five", 100)
    );
    assert_eq!(f.bssid, [0x24, 0xA4, 0x3C, 0x01, 0x02, 0x03]);
    assert_eq!(f.short_ssid, Some(short_ssid("Five")));
    assert!(parse(&probe_response(&a, [0xFF; 6])).unsolicited);
    assert!(!parse(&probe_response(&a, CLIENT)).unsolicited);
    let mut r = a.clone();
    r.rnr = vec![RnrSpec {
        class: 131,
        channel: 5,
        bssid: [1, 2, 3, 4, 5, 6],
        short_ssid: 7,
        params: 0x40,
    }];
    let b = parse(&beacon(&r));
    assert_eq!(b.el.rnr.len(), 1);
    assert!(b.el.rnr[0].co_located());
    // FILS body facts.
    let mut f = vec![0xD0, 0, 0, 0];
    f.extend([0xFF; 6]);
    f.extend([0x24, 0xA4, 0x3C, 1, 2, 3]);
    f.extend([0x24, 0xA4, 0x3C, 1, 2, 3]);
    f.extend([0, 0]);
    let mut body = vec![0x04, 0x22, 0x03, 0x00];
    body.extend(1..=8u8);
    body.extend([0x64, 0x00]);
    body.extend(b"Test");
    let mut whole = f.clone();
    whole.extend(&body);
    let x = parse_bss(&whole).unwrap();
    assert_eq!(
        (x.ssid.as_str(), x.interval_tu, x.timestamp),
        ("Test", 100, 0x0807_0605_0403_0201)
    );
    for cut in [10, 17] {
        let mut w = f.clone();
        w.extend(&body[..cut]);
        assert!(parse_bss(&w).is_none(), "{cut}");
    }
}

#[test]
fn every_security_parses_back_and_truncations_are_safe() {
    let cases = [
        (Sec::Open, false, "Open"),
        (Sec::Wep, false, "WEP"),
        (Sec::Wpa, false, "WPA"),
        (Sec::Wpa, true, "WPA-Enterprise"),
        (Sec::WpaWpa2, false, "WPA/WPA2"),
        (Sec::WpaWpa2, true, "WPA/WPA2-Enterprise"),
        (Sec::Wpa2, false, "WPA2"),
        (Sec::Wpa2, true, "WPA2-Enterprise"),
        (Sec::Wpa2Wpa3, false, "WPA2/WPA3"),
        (Sec::Wpa2Wpa3, true, "WPA2/WPA3-Enterprise"),
        (Sec::Wpa3, false, "WPA3"),
        (Sec::Wpa3, true, "WPA3-Enterprise"),
        (Sec::Owe, false, "OWE (Enhanced Open)"),
    ];
    for (sec, ent, name) in cases {
        let mut s = BssSpec::new([0x00, 0x04, 0x0E, 1, 2, 3], "Net", Band::G2, 6, sec);
        s.enterprise = ent;
        let b = parse(&beacon(&s));
        assert_eq!(security(&b.el, b.capability).name, name);
        assert_eq!(
            (b.ssid.as_str(), b.el.ds_channel, b.interval_tu),
            ("Net", Some(6), 100)
        );
        assert_eq!(rates_text(&b.el.rates), "1 2 5.5 11 6 9 12 18 24 36 48 54");
        assert_eq!(sdrkit_util_ids::oui::vendor_of_address(&b.bssid), Some(AVM));
    }
    let mut h = BssSpec::new(
        [2, 0x11, 0x22, 0x33, 0x44, 0x55],
        "Secret",
        Band::G2,
        6,
        Sec::Open,
    );
    h.hidden = true;
    let b = parse(&beacon(&h));
    assert!(b.hidden && b.ssid.is_empty());
    assert_eq!(parse(&probe_response(&h, CLIENT)).ssid, "Secret");
    let d = data_frame([0, 4, 0x0E, 1, 2, 3], CLIENT, 40, 1);
    let hd = header(&d[..d.len() - 4]).unwrap();
    assert_eq!(hd.ftype, 2);
    assert!(parse_bss(&d[..d.len() - 4]).is_none());
    // Every prefix of a rich beacon, and random bytes, parse safely.
    let [a, _, _] = sdrkit_stack_wlan::wifi::gen::scene_aps(Band::G5, 36);
    let f = beacon(&a);
    for n in 0..f.len() {
        let _ = parse_bss(&f[..n]);
        let _ = elements(&f[n.min(36)..n]);
    }
    let mut r = sdrkit_algo_sim::rng::Rng::new(3);
    for _ in 0..2000 {
        let n = r.below(300) as usize;
        let mut v = vec![0u8; n];
        r.fill_bytes(&mut v);
        if !v.is_empty() && r.bit() == 0 {
            v[0] = 0x80;
        }
        let _ = parse_bss(&v);
        let _ = elements(&v);
    }
}

/// The FILS short SSID is CRC-32/ISO-HDLC, not CRC-32/MPEG-2.
#[test]
fn fils_short_ssid_is_crc32_iso_hdlc() {
    // The check values of the CRC catalogue (Williams, "A painless guide to CRC error detection algorithms").
    assert_eq!(short_ssid("123456789"), 0xCBF4_3926);
    assert_ne!(short_ssid("123456789"), 0x0376_E6E7);
}

/// The saved state of the network table: the signal clock and the store.
#[derive(serde::Deserialize)]
struct Saved {
    data: Store,
}

/// The network records the table keeps, read from the saved state of the network table component.
fn networks(rx: &StackGraph) -> Vec<Network> {
    let Some(s) = rx.graph().component("table").and_then(|c| c.save_state()) else {
        return Vec::new();
    };
    let saved: Saved = serde_json::from_slice(s.bytes()).unwrap();
    saved
        .data
        .networks
        .iter()
        .map(|r| r.record.clone())
        .collect()
}

/// The band named by result field `band`.
fn band_label(r: &Results) -> String {
    match value(r, "band") {
        Some(Value::Choice(i)) => Band::CHOICES[usize::from(*i)].label.to_string(),
        _ => String::new(),
    }
}

/// The frames of each OFDM rate, from the `ofdm_rates` table, as "6 Mbit/s 1".
fn by_rate(r: &Results) -> Vec<String> {
    rows(r, "ofdm_rates")
        .into_iter()
        .map(|row| {
            format!(
                "{} Mbit/s {}",
                cnum(row, &WifiRateRow::RATE_MBPS),
                row.at(&WifiRateRow::FRAMES).unwrap_or(0)
            )
        })
        .collect()
}

fn net<'a>(d: &'a [Network], bssid: &str) -> &'a Network {
    d.iter().find(|n| n.bssid == bssid).unwrap_or_else(|| {
        panic!(
            "{bssid} missing: {:?}",
            d.iter().map(|n| &n.bssid).collect::<Vec<_>>()
        )
    })
}

fn rx() -> StackGraph {
    wifi::receiver(&config(WifiSetting::schema(), &[])).unwrap()
}

/// A signal of `p` through the impairments `imp`, `skip` samples dropped from its start.
fn signal(
    p: WifiGenParams,
    imp: &[(&str, Value)],
    seed: u64,
    seconds: f64,
    skip: usize,
) -> Vec<Complex32> {
    let mut x = impaired(
        Box::new(WifiGenerator::new(p)),
        imp,
        seed,
        seconds + skip as f64 / 20e6,
    );
    x.drain(..skip.min(x.len()));
    x
}

/// Feeds `x` at `rate` with the centre at `centre` and the receiver tuned to `tuned`, and returns the receiver back.
fn run(
    x: &[Complex32],
    rate: f64,
    centre: f64,
    tuned: f64,
    piece: usize,
) -> sdrkit_graph::StackGraph {
    let mut r = rx();
    r.configure(&InputInfo::new(rate, centre, tuned).recording())
        .unwrap();
    feed(&mut r, x, piece);
    r
}

fn imp(snr_db: f64, offset_hz: f64, ppm: f64) -> Vec<(&'static str, Value)> {
    let mut v = snr(snr_db);
    v.push(("gen_offset_hz", Value::F64(offset_hz)));
    v.push(("gen_clock_ppm", Value::F64(ppm)));
    v
}

#[test]
fn ofdm_at_every_rate_and_dsss_at_both_rates_through_the_receiver() {
    let psdu = beacon(&BssSpec::new(
        [0x24, 0xA4, 0x3C, 1, 2, 3],
        "Loop",
        Band::G5,
        36,
        Sec::Wpa2,
    ));
    let rates = [
        ofdm::Rate::R6,
        ofdm::Rate::R9,
        ofdm::Rate::R12,
        ofdm::Rate::R18,
        ofdm::Rate::R24,
        ofdm::Rate::R36,
        ofdm::Rate::R48,
        ofdm::Rate::R54,
    ];
    let frames: Vec<GenFrame> = rates
        .iter()
        .map(|&r| {
            GenFrame::new(psdu.clone(), Phy::Ofdm)
                .ofdm_rate(r)
                .gap(0.3e-3)
        })
        .collect();
    let x = signal(
        WifiGenParams {
            frames,
            looped: false,
            ..Default::default()
        },
        &imp(30.0, 20e3, 0.0),
        1,
        0.01,
        0,
    );
    let r = run(&x, 20e6, 5180e6, 5180e6, 65536);
    assert_eq!(num(Owner::results(&r), "frames_ok"), Some(8.0));
    let by_rate = by_rate(Owner::results(&r));
    for m in ["6 Mbit/s 1", "9 Mbit/s 1", "24 Mbit/s 1", "54 Mbit/s 1"] {
        assert!(by_rate.iter().any(|x| x == m), "{m} in {by_rate:?}");
    }
    let d = beacon(&BssSpec::new(
        [0x00, 0x04, 0x0E, 1, 2, 3],
        "Dsss",
        Band::G2,
        6,
        Sec::Wpa2,
    ));
    let frames = vec![
        GenFrame::new(d.clone(), Phy::Dsss),
        GenFrame::new(d.clone(), Phy::Dsss).dsss(dsss::Rate::Dqpsk2, dsss::Preamble::Long),
        GenFrame::new(d.clone(), Phy::Dsss).dsss(dsss::Rate::Dqpsk2, dsss::Preamble::Short),
        GenFrame::new(d, Phy::Dsss).dsss(dsss::Rate::Cck11, dsss::Preamble::Short),
    ];
    let mut p = WifiGenParams {
        frames,
        looped: false,
        ..Default::default()
    };
    // The CCK frame cannot be built here: only the three Barker frames are sent.
    p.frames.truncate(3);
    let x = signal(p, &imp(30.0, 0.0, 0.0), 1, 0.02, 0);
    let r = run(&x, 20e6, 2437e6, 2437e6, 65536);
    assert_eq!(num(Owner::results(&r), "frames_ok"), Some(3.0));
}

#[test]
fn the_24_ghz_scene() {
    let x = signal(WifiGenParams::default(), &imp(30.0, 0.0, 0.0), 1, 0.25, 0);
    let r = run(&x, 20e6, 2437e6, 2437e6, 65536);
    let d = networks(&r);
    let h = net(&d, "00:04:0E:12:34:56");
    assert_eq!(
        (h.ssid.as_str(), h.security.label(), h.vendor.as_str()),
        ("Test-Home", "WPA2", AVM)
    );
    assert_eq!((h.channel, h.heard_on, h.interval_tu), (6, 6, 100));
    assert!(
        h.beacons >= 12 && h.probe_responses >= 12,
        "{} {}",
        h.beacons,
        h.probe_responses
    );
    assert_eq!(h.rates, "1 2 5.5 11 6 9 12 18 24 36 48 54");
    assert_eq!(h.ciphers, "CCMP");
    let c = net(&d, "FC:EC:DA:AB:CD:EF");
    assert_eq!(
        (
            c.ssid.as_str(),
            c.security.label(),
            c.vendor.as_str(),
            c.channel
        ),
        ("Test-Cafe", "WPA2/WPA3", UBNT, 6)
    );
    let hid = net(&d, "02:11:22:33:44:55");
    assert!(hid.hidden && hid.ssid.is_empty());
    assert_eq!(
        (
            hid.security.label(),
            hid.interval_tu,
            hid.vendor.as_str(),
            hid.rates.as_str()
        ),
        ("Open", 200, "local", "1 2 5.5 11")
    );
    assert_eq!(d.len(), 3);
    let res = Owner::results(&r);
    assert_eq!(num(res, "frames_bad"), Some(0.0));
    assert_eq!(num(res, "dsss_headers_bad"), Some(0.0));
    assert!(num(res, "data_frames").unwrap() >= 12.0);
    assert_eq!(
        num(res, "mgmt_frames"),
        Some(num(res, "beacons").unwrap() + num(res, "probe_responses").unwrap())
    );
    assert_eq!(num(res, "channel"), Some(6.0));
    assert!(num(res, "snr_db").unwrap() > 20.0);
    let row = rows(res, "networks")
        .into_iter()
        .find(|c| text(c, &N::BSSID) == "02:11:22:33:44:55")
        .unwrap();
    assert_eq!(text(row, &N::SSID), "(hidden)");
    assert_eq!(text(row, &N::VENDOR), "(local)");
    assert!((cnum(row, &N::INTERVAL) - 0.2048).abs() < 1e-9);
}

#[test]
fn dsss_bad_fcs_offsets_clock_and_rates() {
    let mut f = scene_24();
    f[1] = f[1].clone().bad_fcs();
    let x = signal(
        WifiGenParams {
            frames: f,
            ..Default::default()
        },
        &[],
        1,
        0.26,
        0,
    );
    let r = run(&x, 20e6, 2437e6, 2437e6, 65536);
    assert!(networks(&r).iter().all(|n| n.bssid != "FC:EC:DA:AB:CD:EF"));
    net(&networks(&r), "00:04:0E:12:34:56");
    assert!(num(Owner::results(&r), "frames_bad").unwrap() >= 8.0);
    for off in [50e3, -50e3] {
        let x = signal(WifiGenParams::default(), &imp(12.0, off, 0.0), 7, 0.25, 0);
        let r = run(&x, 20e6, 2437e6, 2437e6, 65536);
        net(&networks(&r), "00:04:0E:12:34:56");
        net(&networks(&r), "FC:EC:DA:AB:CD:EF");
        let res = Owner::results(&r);
        assert!(
            num(res, "frames_ok").unwrap() >= 60.0 && num(res, "frames_bad").unwrap() <= 2.0,
            "{off}"
        );
        assert!((num(res, "offset_hz").unwrap() - off).abs() < 27e3, "{off}");
    }
    let mut f = scene_24();
    f.push(GenFrame::new(
        data_frame([0, 4, 0x0E, 0x12, 0x34, 0x56], CLIENT, 2000, 9),
        Phy::Dsss,
    ));
    let x = signal(
        WifiGenParams {
            frames: f,
            ..Default::default()
        },
        &imp(20.0, 0.0, 25.0),
        1,
        0.3,
        0,
    );
    let r = run(&x, 20e6, 2437e6, 2437e6, 65536);
    net(&networks(&r), "00:04:0E:12:34:56");
    net(&networks(&r), "FC:EC:DA:AB:CD:EF");
    assert_eq!(num(Owner::results(&r), "frames_bad"), Some(0.0));
    assert!(num(Owner::results(&r), "data_frames").unwrap() >= 10.0);
}

/// At 25 and 30 Msps the 2.4 GHz scene goes through the shared channel filter and resampler, with every frame decoded and none
/// failing its FCS.
#[test]
fn dsss_at_rates_other_than_20_msps_uses_a_filtered_resampler() {
    for rate in [25e6, 30e6, 43.5e6] {
        let x = signal(
            WifiGenParams {
                rate_hz: rate,
                ..Default::default()
            },
            &imp(25.0, 0.0, 0.0),
            2,
            0.1,
            0,
        );
        let r = run(&x, rate, 2437e6, 2437e6, 65536);
        net(&networks(&r), "00:04:0E:12:34:56");
        net(&networks(&r), "FC:EC:DA:AB:CD:EF");
        let res = Owner::results(&r);
        assert_eq!(num(res, "frames_bad"), Some(0.0), "{rate}");
        assert_eq!(num(res, "dsss_headers_bad"), Some(0.0), "{rate}");
        assert!(
            num(res, "frames_ok").unwrap() >= 25.0,
            "{rate}: {:?}",
            num(res, "frames_ok")
        );
    }
}

#[test]
fn noise_and_too_low_a_rate() {
    let floor = [
        ("gen_noise", Value::Text("floor".into())),
        ("gen_noise_floor_dbfs", Value::F64(-23.0)),
    ];
    let empty = WifiGenParams {
        frames: Vec::new(),
        ..Default::default()
    };
    let x = signal(empty.clone(), &floor, 3, 1.0, 0);
    let r = run(&x, 20e6, 2437e6, 2437e6, 65536);
    assert!(networks(&r).is_empty());
    assert_eq!(num(Owner::results(&r), "frames_ok"), Some(0.0));
    assert_eq!(
        num(Owner::results(&r), "dsss_headers_ok").unwrap()
            + num(Owner::results(&r), "dsss_headers_bad").unwrap(),
        0.0
    );
    let mut low = rx();
    assert!(low
        .configure(&InputInfo::new(3.5e6, 2437e6, 2437e6))
        .is_err());
    assert!(!low.ready());
    let mut y = signal(empty, &floor, 4, 0.1, 0);
    for (i, v) in y.iter_mut().enumerate() {
        if i % 3 == 0 {
            *v += Complex32::new(0.8, 0.0);
        }
    }
    let r = run(&y, 20e6, 5180e6, 5180e6, 65536);
    assert!(networks(&r).is_empty());
    assert_eq!(num(Owner::results(&r), "frames_ok"), Some(0.0));
}

fn check_56(r: &StackGraph, band: Band, p: u32) {
    let w = |want: u32| {
        if wide_centre(band, p, want).is_some() {
            want
        } else {
            20
        }
    };
    let res = Owner::results(r);
    let d = networks(r);
    assert_eq!(num(res, "channel"), Some(p as f64));
    assert_eq!(band_label(res), band.name());
    assert!(num(res, "frames_ok").unwrap() >= 6.0, "{p}");
    // The last frame can be cut by the end of the signal.
    assert!(num(res, "frames_bad").unwrap() <= 1.0, "{p}");
    if band == Band::G5 {
        let a = net(&d, "24:A4:3C:50:00:01");
        assert_eq!((a.ssid.as_str(), a.channel, a.heard_on), ("Test-5G", p, p));
        assert_eq!(
            (a.width_mhz, a.standard.as_str(), a.security.label()),
            (w(80), "a/n/ac/ax", "WPA2/WPA3")
        );
        assert_eq!(
            (a.vendor.as_str(), a.bss_color, a.twt),
            (UBNT, Some(17), true)
        );
        assert_eq!(
            (
                a.country.as_str(),
                a.power_constraint_db,
                a.mobility_domain.as_str()
            ),
            ("DE ", Some(3), "1234")
        );
        assert!(a.akm.contains("SAE"));
        for s in ["6 GHz ch 5", "co-located", "24:A4:3C:60:00:01"] {
            assert!(a.neighbors.contains(s), "{s}");
        }
        assert!(a.probe_responses >= 1 && !a.fils && !a.unsolicited_probe_responses);
        let b = net(&d, "00:04:0E:77:88:99");
        assert_eq!(
            (
                b.ssid.as_str(),
                b.standard.as_str(),
                b.width_mhz,
                b.security.label(),
                b.vendor.as_str()
            ),
            ("Test-Old", "a/n", 20, "WPA2", AVM)
        );
        let c = net(&d, "FC:EC:DA:12:34:56");
        assert_eq!(
            (c.ssid.as_str(), c.standard.as_str(), c.security.label()),
            ("Test-ac", "a/n/ac", "WPA2")
        );
    } else {
        let a = net(&d, "24:A4:3C:60:00:01");
        assert_eq!(
            (
                a.ssid.as_str(),
                a.standard.as_str(),
                a.security.label(),
                a.pmf.as_str()
            ),
            ("Test-6E", "ax/be", "WPA3", "required")
        );
        assert_eq!(
            (a.ap_power.as_str(), a.eht, a.akm.as_str()),
            ("indoor AP", true, "SAE")
        );
        assert!(a.fils && a.fils_frames >= 1 && a.unsolicited_probe_responses);
        assert_eq!((a.bss_color, a.twt), (Some(17), true));
        let b = net(&d, "00:04:0E:77:88:99");
        assert_eq!(
            (b.ssid.as_str(), b.security.label(), b.standard.as_str()),
            ("Test-6E-Open", "OWE (Enhanced Open)", "ax")
        );
        assert_eq!(
            (b.bss_color, b.pmf.as_str(), b.akm.as_str()),
            (Some(33), "required", "OWE")
        );
        assert_eq!(net(&d, "FC:EC:DA:12:34:56").security.label(), "WPA3");
    }
}

#[test]
fn every_channel_of_the_5_and_6_ghz_plans() {
    for band in [Band::G5, Band::G6] {
        for p in all_channels(band) {
            let f = centre_mhz(band, p).unwrap() * 1e6;
            let x = signal(
                WifiGenParams {
                    frames: scene_56(band, p as u8),
                    ..Default::default()
                },
                &imp(28.0, 0.0, 0.0),
                p as u64,
                0.04,
                0,
            );
            check_56(&run(&x, 20e6, f, f, 65536), band, p);
        }
    }
}

#[test]
fn impairments_on_every_seventh_channel() {
    // (offset Hz, SNR dB, clock ppm, rate Hz, position Hz, samples dropped, seed, seconds)
    type Case = (f64, f64, f64, f64, f64, usize, u64, f64);
    let cases: [Case; 9] = [
        (100e3, 25.0, 0.0, 20e6, 0.0, 0, 1, 0.04),
        (-120e3, 22.0, 0.0, 20e6, 0.0, 0, 1, 0.04),
        (0.0, 14.0, 0.0, 20e6, 0.0, 0, 3, 0.08),
        (20e3, 28.0, 40.0, 20e6, 0.0, 0, 1, 0.04),
        (0.0, 28.0, 0.0, 25e6, 0.0, 0, 1, 0.04),
        (-50e3, 28.0, 0.0, 30e6, 0.0, 0, 1, 0.04),
        (0.0, 28.0, 0.0, 25e6, 3e6, 0, 1, 0.04),
        (0.0, 28.0, 0.0, 20e6, 0.0, 1873, 1, 0.04),
        (60e3, 28.0, -30.0, 20e6, 0.0, 7001, 1, 0.04),
    ];
    for band in [Band::G5, Band::G6] {
        for p in all_channels(band).into_iter().step_by(7) {
            for (k, &(off, s, ppm, rate, pos, skip, seed, secs)) in cases.iter().enumerate() {
                let f = centre_mhz(band, p).unwrap() * 1e6;
                let mut frames = scene_56(band, p as u8);
                for fr in frames.iter_mut() {
                    fr.position_hz = pos;
                }
                let x = signal(
                    WifiGenParams {
                        frames,
                        rate_hz: rate,
                        ..Default::default()
                    },
                    &imp(s, off, ppm),
                    seed + p as u64,
                    secs,
                    skip,
                );
                let r = run(&x, rate, f - pos, f, 65536);
                if pos != 0.0 {
                    assert!(
                        networks(&r).len() >= 3
                            && num(Owner::results(&r), "frames_ok").unwrap() >= 6.0,
                        "{p} case {k}"
                    );
                } else {
                    check_56(&r, band, p);
                }
            }
        }
    }
}

#[test]
fn a_wide_capture_hears_two_channels_and_drops_duplicates() {
    let mut wide = BssSpec::new(
        [0x24, 0xA4, 0x3C, 0x0A, 0x0B, 0x0C],
        "Wide-36",
        Band::G5,
        36,
        Sec::Wpa2,
    );
    wide.width = 40;
    wide.ht = Some(2);
    let mut only = BssSpec::new(
        [0x00, 0x04, 0x0E, 0x0A, 0x0B, 0x0C],
        "Only-40",
        Band::G5,
        40,
        Sec::Wpa2,
    );
    only.ht = Some(1);
    let mut a = GenFrame::new(beacon(&wide), Phy::Ofdm).gap(0.8e-3);
    a.position_hz = -10e6;
    a.duplicates = vec![10e6];
    let mut b = GenFrame::new(beacon(&only), Phy::Ofdm)
        .gap(0.8e-3)
        .offset(15e3);
    b.position_hz = 10e6;
    let x = signal(
        WifiGenParams {
            rate_hz: 50e6,
            frames: vec![a, b],
            ..Default::default()
        },
        &imp(28.0, 0.0, 0.0),
        1,
        0.03,
        0,
    );
    let r = run(&x, 50e6, 5190e6, 5190e6, 65536);
    let res = Owner::results(&r);
    assert_eq!(num(res, "sub_channels"), Some(2.0));
    let d = networks(&r);
    assert_eq!(
        d.len(),
        2,
        "{:?}",
        d.iter().map(|n| &n.ssid).collect::<Vec<_>>()
    );
    let w = net(&d, "24:A4:3C:0A:0B:0C");
    assert_eq!(
        (w.ssid.as_str(), w.channel, w.heard_on, w.width_mhz),
        ("Wide-36", 36, 36, 40)
    );
    let o = net(&d, "00:04:0E:0A:0B:0C");
    assert_eq!((o.channel, o.heard_on), (40, 40));
    assert!(num(res, "duplicate_frames").unwrap() >= 8.0 && w.beacons >= 8);
}

#[test]
fn cuts_do_not_change_the_result() {
    let log = |r: &sdrkit_graph::StackGraph| {
        (
            num(Owner::results(r), "frames_ok"),
            rows(Owner::results(r), "beacons")
                .iter()
                .map(|c| (cnum(c, &B::SIGNAL_S), text(c, &B::DETAILS)))
                .collect::<Vec<_>>(),
        )
    };
    let x = signal(WifiGenParams::default(), &imp(30.0, 0.0, 0.0), 1, 0.08, 0);
    let a = run(&x, 20e6, 2437e6, 2437e6, 1013);
    let b = run(&x, 20e6, 2437e6, 2437e6, 65536);
    assert_eq!(networks(&a).len(), 3);
    assert_eq!(band_label(Owner::results(&a)), "2.4 GHz");
    assert_eq!(log(&a), log(&b));
    let x = signal(
        WifiGenParams {
            frames: scene_56(Band::G5, 36),
            ..Default::default()
        },
        &imp(30.0, 0.0, 0.0),
        1,
        0.03,
        0,
    );
    let a = run(&x, 20e6, 5180e6, 5180e6, 777);
    let b = run(&x, 20e6, 5180e6, 5180e6, 100_000);
    assert_eq!(log(&a), log(&b));
    assert!(num(Owner::results(&a), "frames_ok").unwrap() > 10.0);
}

#[test]
fn clock_error_save_restore_and_clear() {
    let x = signal(
        WifiGenParams {
            frames: scene_56(Band::G5, 149),
            ..Default::default()
        },
        &imp(30.0, 0.0, 20.0),
        1,
        0.05,
        0,
    );
    let mut r = run(&x, 20e6, 5745e6, 5745e6, 65536);
    check_56(&r, Band::G5, 149);
    let state = r.save_state().unwrap();
    let mut other = rx();
    other
        .configure(&InputInfo::new(20e6, 5745e6, 5745e6).recording())
        .unwrap();
    other.restore_state(&state).unwrap();
    assert_eq!(networks(&other).len(), 3);
    assert_eq!(rows(Owner::results(&other), "networks").len(), 3);
    assert!(other
        .restore_state(&StateBlob::new("garbage", 1, Vec::new()))
        .is_err());
    command(&mut r, "clear", &[]).unwrap();
    assert!(rows(Owner::results(&r), "networks").is_empty());
}

#[test]
fn display_filters_limit_the_network_rows() {
    let x = signal(WifiGenParams::default(), &imp(30.0, 0.0, 0.0), 1, 0.1, 0);
    let count = |s: &[(&str, Value)]| {
        let mut r = wifi::receiver(&config(WifiSetting::schema(), s)).unwrap();
        r.configure(&InputInfo::new(20e6, 2437e6, 2437e6).recording())
            .unwrap();
        feed(&mut r, &x, 65536);
        rows(Owner::results(&r), "networks").len()
    };
    assert_eq!(count(&[]), 3);
    assert_eq!(count(&[("filter_text", Value::Text("cafe".into()))]), 1);
    assert_eq!(count(&[("open_only", Value::Bool(true))]), 1);
    // Only the home network is sent at full power: the cafe is at 0.6 and the hidden network at 0.8 of its amplitude.
    assert_eq!(count(&[("min_level_dbfs", Value::F64(-1.0))]), 1);
    assert_eq!(count(&[("band_filter", Value::Text("5".into()))]), 0);
}

#[test]
fn channel_band_and_hop_ask_the_host() {
    let hops = |q: &[sdrkit_core::HostRequest]| -> Vec<(Vec<f64>, f64)> {
        q.iter()
            .filter_map(|q| match q {
                sdrkit_core::HostRequest::Hop { channels, dwell_s, .. } => Some((channels.clone(), *dwell_s)),
                _ => None,
            })
            .collect()
    };
    let quiet = vec![Complex32::default(); 200_000];
    // With no settings the stack hops by itself: channels 1 to 11 from the tuned channel, a beacon interval and a margin on
    // each.
    let mut r = rx();
    r.configure(&InputInfo::new(20e6, 2437e6, 2437e6)).unwrap();
    let fed = feed(&mut r, &quiet, 200_000);
    assert!(fed.requests.iter().all(|q| q.validate().is_ok()));
    let want: Vec<f64> = [6, 7, 8, 9, 10, 11, 1, 2, 3, 4, 5].iter().map(|&c| (2407.0 + 5.0 * f64::from(c)) * 1e6).collect();
    assert_eq!(hops(&fed.requests), [(want, 0.13)]);
    // A frequency on 5 GHz decides the band: the hop goes over the 5 GHz channels from the tuned one.
    let mut r = rx();
    r.configure(&InputInfo::new(20e6, 5745e6, 5745e6)).unwrap();
    let fed = feed(&mut r, &quiet, 200_000);
    let h = hops(&fed.requests);
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].0[0], h[0].0.contains(&5180e6)), (5745e6, true));
    // A channel list set before the first samples starts no hop over the band of the settings: the radio is on 5 GHz, and
    // the hop goes over its 5 GHz list from the tuned channel.
    let lists = [("channels_24", Value::Text("1 6 11".into())), ("channels_5", Value::Text("36 40 149".into()))];
    let mut r = wifi::receiver(&config(WifiSetting::schema(), &lists)).unwrap();
    r.configure(&InputInfo::new(20e6, 5200e6, 5200e6)).unwrap();
    let fed = feed(&mut r, &quiet, 200_000);
    assert_eq!(hops(&fed.requests), [(vec![5200e6, 5745e6, 5180e6], 0.13)]);
    // Band 5 with the 2.4 GHz default channel is not refused: the band's first channel is asked for, then the hop.
    let mut r = wifi::receiver(&config(WifiSetting::schema(), &[("band", Value::Text("5".into()))])).unwrap();
    r.configure(&InputInfo::new(20e6, 2437e6, 2437e6)).unwrap();
    let fed = feed(&mut r, &quiet, 200_000);
    match &fed.requests[..] {
        [sdrkit_core::HostRequest::Retune { tuned_hz, .. }, sdrkit_core::HostRequest::Hop { channels, .. }] => {
            assert_eq!((*tuned_hz, channels[0]), (5180e6, 5180e6));
        }
        q => panic!("{q:?}"),
    }
    // With the hop off from the start the radio stays where the host tuned it, on any band. A channel is then asked for
    // alone; a number the band lacks takes its first channel.
    let mut r = rx();
    set(&mut r, "hop", Value::Bool(false)).unwrap();
    r.configure(&InputInfo::new(20e6, 5180e6, 5180e6)).unwrap();
    assert!(feed(&mut r, &quiet, 200_000).requests.is_empty());
    let mut r = rx();
    set(&mut r, "hop", Value::Bool(false)).unwrap();
    r.configure(&InputInfo::new(20e6, 2437e6, 2437e6)).unwrap();
    set(&mut r, "channel", Value::F64(11.0)).unwrap();
    set(&mut r, "channel", Value::F64(36.0)).unwrap();
    let fed = feed(&mut r, &quiet, 200_000);
    let tuned: Vec<f64> = fed
        .requests
        .iter()
        .map(|q| match q {
            sdrkit_core::HostRequest::Retune { tuned_hz, .. } => *tuned_hz,
            q => panic!("{q:?}"),
        })
        .collect();
    // Channel 11, then the band's first channel for 36.
    assert_eq!(tuned, [2462e6, 2412e6]);
}

/// A hop whose dwell is shorter than the clock of the graph: each retune starts the clock again, and the results still come
/// out as the frames are decoded, with the status and the channel being heard.
#[test]
fn a_hop_with_a_short_dwell_shows_its_results_and_the_channel_while_it_runs() {
    let x = signal(WifiGenParams::default(), &snr(20.0), 1, 0.6, 0);
    let mut r = rx();
    let at = |ch: u32| {
        let hz = (2407.0 + 5.0 * f64::from(ch)) * 1e6;
        InputInfo::new(20e6, hz, hz).recording()
    };
    r.configure(&at(6)).unwrap();
    assert_eq!(Owner::results(&r).status().lock_state, sdrkit_core::LockState::Off);
    let mut out = sdrkit_core::Output::default();
    let mut hops = Vec::new();
    // 0.1 s on each channel; the clock of the graph is 0.25 s.
    for (i, dwell) in x.chunks(2_000_000).enumerate() {
        let ch = [6, 7, 8][i % 3];
        if i > 0 {
            r.retuned(&at(ch));
        }
        for p in dwell.chunks(65_536) {
            out.clear();
            r.feed(p, &mut out);
        }
        let res = Owner::results(&r);
        hops.push((res.status().lock_state, num(res, "channel"), rows(res, "networks").len()));
        if i >= 3 {
            assert!(res.status().summary.starts_with(&format!("hopping, on channel {ch}: ")), "{}", res.status().summary);
        }
    }
    use sdrkit_core::LockState::Locked;
    // The first network shows with the first piece of samples, before any tick of the clock.
    assert_eq!((hops[0].0, hops[0].1), (Locked, Some(6.0)));
    assert!(hops[0].2 >= 1);
    assert_eq!(hops[1].1, Some(7.0));
    assert_eq!(hops[5], (Locked, Some(8.0), 3));
}

/// On 5 GHz a beacon that names another channel than the tuned one is the copy of a wider network, and is left out. While
/// the radio hops it is left out too: after each move the stack is given samples of the new channel only.
#[test]
fn a_beacon_that_names_another_channel_is_left_out_while_the_radio_hops() {
    let p = WifiGenParams {
        frames: scene_56(Band::G5, 36),
        ..WifiGenParams::default()
    };
    let x = signal(p, &snr(25.0), 1, 0.6, 0);
    let at = |ch: u32| InputInfo::new(20e6, (5000.0 + 5.0 * f64::from(ch)) * 1e6, (5000.0 + 5.0 * f64::from(ch)) * 1e6).recording();
    let heard = |hop: bool| {
        let mut r = rx();
        r.configure(&at(40)).unwrap();
        let mut out = sdrkit_core::Output::default();
        for (i, dwell) in x.chunks(2_000_000).enumerate() {
            if hop && i > 0 {
                r.retuned(&at([40, 44][i % 2]));
            }
            for p in dwell.chunks(65_536) {
                out.clear();
                r.feed(p, &mut out);
            }
        }
        r.finish(&mut out);
        let res = Owner::results(&r);
        let channels: Vec<f64> = rows(res, "networks").iter().map(|n| cnum(n, &N::CHANNEL)).collect();
        (channels, num(res, "duplicate_frames").unwrap() > 0.0)
    };
    assert_eq!(heard(false), (vec![], true));
    assert_eq!(heard(true), (vec![], true));
}

/// The channel lists of two domains of the data file give the hop list of each band, with and without the DFS channels.
#[test]
fn the_hop_lists_of_two_domains() {
    let rows = wifi_domains();
    for domain in ["eu", "us"] {
        for skip_dfs in [false, true] {
            let plan = wifi_plan(domain, skip_dfs);
            let settings: Vec<(&str, Value)> =
                plan.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            let mut r = wifi::receiver(&config(WifiSetting::schema(), &settings)).unwrap();
            r.configure(&InputInfo::new(20e6, 2437e6, 2437e6)).unwrap();
            set(&mut r, "psc_only", Value::Bool(false)).unwrap();
            set(&mut r, "hop", Value::Bool(true)).unwrap();
            // The hop of the tuned band is asked for with the first samples, from the tuned channel; the hop of a band
            // that the band setting asks for, at once.
            let mut requests = Vec::new();
            let mut out = sdrkit_core::Output::default();
            r.feed(&vec![Complex32::default(); 200_000], &mut out);
            requests.append(&mut out.requests);
            for band in ["5", "6"] {
                set(&mut r, "band", Value::Text(band.into())).unwrap();
            }
            requests.append(&mut feed(&mut r, &vec![Complex32::default(); 200_000], 200_000).requests);
            let mut hops: Vec<Vec<f64>> = requests
                .iter()
                .filter_map(|q| match q {
                    sdrkit_core::HostRequest::Hop { channels, .. } => Some(channels.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(hops[0][0], 2437e6);
            hops[0].sort_by(f64::total_cmp);
            let want = |b: &str, band: Band| -> Vec<f64> {
                let row = rows
                    .iter()
                    .find(|r| r.domain == domain && r.band == b)
                    .unwrap();
                row.hop_list(skip_dfs)
                    .iter()
                    .map(|&c| centre_mhz(band, c).unwrap() * 1e6)
                    .collect()
            };
            assert_eq!(
                hops,
                vec![
                    want("2.4", Band::G2),
                    want("5", Band::G5),
                    want("6", Band::G6)
                ],
                "{domain} {skip_dfs}"
            );
        }
    }
    let mut r = rx();
    assert!(set(&mut r, "channels_5", Value::Text("36 37".into())).is_err());
    assert!(set(&mut r, "channels_24", Value::Text("1, 6 11".into())).is_ok());
}

#[test]
fn the_recordings_decode_as_expected() {
    let exp = sdrkit_dev_test::sigmf::read_expected("wlan").unwrap();
    for name in ["wifi_24g_dsss_20m", "wifi_5g_ofdm_20m", "wifi_6g_ofdm_20m"] {
        let e = &exp[name];
        let rec =
            sdrkit_dev_test::read_recording(&format!("wlan/{name}")).expect("the Wi-Fi recordings");
        let r = run(
            &rec.samples,
            rec.rate_hz,
            rec.centre_hz,
            rec.centre_hz,
            65536,
        );
        let res = Owner::results(&r);
        let d = networks(&r);
        assert_eq!(band_label(res), e["band"].as_str().unwrap(), "{name}");
        assert_eq!(num(res, "channel"), e["channel"].as_f64(), "{name}");
        assert_eq!(num(res, "frames_bad"), Some(0.0), "{name}");
        let c = &e["counters"];
        let lo = e["tolerance"]["frames_ok"][0].as_f64().unwrap();
        let hi = e["tolerance"]["frames_ok"][1].as_f64().unwrap();
        let ok = num(res, "frames_ok").unwrap();
        if name == "wifi_24g_dsss_20m" {
            // The recording holds four rounds of the five-frame scene: 20 frames with a good FCS.
            assert_eq!(ok, 20.0, "{name}");
        } else {
            assert!((lo..=hi).contains(&ok), "{name}: {ok}");
            for (k, f) in [
                ("beacons", "beacons"),
                ("probe_responses", "probe_responses"),
                ("fils", "fils_frames"),
            ] {
                if let Some(want) = c.get(k) {
                    let got = num(res, f).unwrap();
                    assert!(
                        (got - want.as_f64().unwrap()).abs() <= 1.0,
                        "{name} {k}: {got}"
                    );
                }
            }
        }
        let snr_min = if name == "wifi_24g_dsss_20m" {
            20.0
        } else {
            18.0
        };
        assert!(num(res, "snr_db").unwrap() >= snr_min, "{name}");
        let want_cfo = e["measurements"]["cfo_hz_last"].as_f64().unwrap();
        let tol = if name == "wifi_5g_ofdm_20m" {
            3800.0
        } else {
            3000.0
        };
        assert!(
            (num(res, "offset_hz").unwrap() - want_cfo).abs() <= tol,
            "{name}: {:?}",
            num(res, "offset_hz")
        );
        let nets = e["networks"].as_array().unwrap();
        assert_eq!(d.len(), nets.len(), "{name}");
        for w in nets {
            let b = w["bssid"].as_str().unwrap();
            let n = net(&d, b);
            let txt = |k: &str, got: &str| {
                if let Some(v) = w.get(k) {
                    assert_eq!(got, v.as_str().unwrap(), "{name} {b} {k}");
                }
            };
            txt("ssid", &n.ssid);
            txt("vendor", &n.vendor);
            txt("band", n.band.map_or("", Band::name));
            txt("standard", &n.standard);
            txt("security", n.security.label());
            txt("ciphers", &n.ciphers);
            txt("akm", &n.akm);
            txt("pmf", &n.pmf);
            txt("rates", &n.rates);
            txt("capability", &n.capability);
            txt("country", &n.country);
            txt("mobility_domain", &n.mobility_domain);
            txt("ap_power", &n.ap_power);
            txt("caps", &n.caps);
            txt("neighbors", &n.neighbors);
            let whole = |k: &str, got: u64| {
                if let Some(v) = w.get(k) {
                    assert_eq!(got, v.as_u64().unwrap(), "{name} {b} {k}");
                }
            };
            whole("channel", n.channel as u64);
            whole("heard_on", n.heard_on as u64);
            whole("width_mhz", n.width_mhz as u64);
            whole("interval_tu", n.interval_tu as u64);
            if let Some(v) = w.get("bss_color") {
                assert_eq!(n.bss_color.map(|x| x as u64), v.as_u64());
            }
            if let Some(v) = w.get("power_constraint_db") {
                assert_eq!(n.power_constraint_db.map(|x| x as u64), v.as_u64());
            }
            for (k, got) in [
                ("hidden", n.hidden),
                ("twt", n.twt),
                ("eht", n.eht),
                ("fils", n.fils),
                ("unsolicited_probe_responses", n.unsolicited_probe_responses),
            ] {
                if let Some(v) = w.get(k) {
                    assert_eq!(got, v.as_bool().unwrap(), "{name} {b} {k}");
                }
            }
            if name != "wifi_24g_dsss_20m" {
                for (k, got) in [
                    ("beacons", n.beacons),
                    ("probe_responses", n.probe_responses),
                    ("fils_frames", n.fils_frames),
                ] {
                    if let Some(v) = w.get(k) {
                        assert!(
                            (got as i64 - v.as_i64().unwrap()).abs() <= 1,
                            "{name} {b} {k}: {got}"
                        );
                    }
                }
            }
        }
        let log = rows(res, "beacons");
        if let Some(lines) = e.get("first_log_lines") {
            for w in lines.as_array().unwrap() {
                let info = w["info"].as_str().unwrap();
                let l = log
                    .iter()
                    .find(|l| text(l, &B::DETAILS) == info)
                    .unwrap_or_else(|| panic!("{info}"));
                assert_eq!(cnum(l, &B::BYTES), w["len"].as_f64().unwrap());
                assert_eq!(text(l, &N::BSSID), w["bssid"].as_str().unwrap());
                if let Some(t) = w.get("t") {
                    assert!(log.iter().any(|l| text(l, &B::DETAILS) == info
                        && (cnum(l, &B::SIGNAL_S) - t.as_f64().unwrap()).abs() < 5e-4));
                }
            }
        }
        if let Some(lines) = e.get("log_examples") {
            for l in lines.as_array().unwrap() {
                let s = l.as_str().unwrap();
                assert!(log.iter().any(|x| text(x, &B::DETAILS) == s), "{s}");
            }
        }
    }
}

#[test]
fn the_harness_runs_the_descriptor() {
    let d = &wifi::WIFI;
    assert_eq!(
        (d.tuning.min_hz, d.tuning.max_hz, d.tuning.min_rate_hz),
        (2400e6, 7125e6, 4e6)
    );
    let run = sdrkit_dev_test::Harness::new(d)
        .set(&wifi::GEN_SCENE, wifi::GenScene::G5)
        .set(&wifi::GEN_CHANNEL, 149)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 30.0)
        .freq(5745e6)
        .seconds(0.3)
        .run()
        .unwrap();
    assert!(num(&run.results, "frames_ok").unwrap() > 50.0);
    assert_eq!(num(&run.results, "channel"), Some(149.0));
}

#[test]
fn levels_are_the_signal_at_or_below_full_scale_and_ofdm_counts_its_subcarriers_only() {
    let levels = |x: &[Complex32]| {
        let mut r = wifi::receiver(&config(WifiSetting::schema(), &[])).unwrap();
        r.configure(&InputInfo::new(20e6, 2437e6, 2437e6).recording())
            .unwrap();
        feed(&mut r, x, 65536);
        let mut v: Vec<(String, f64)> = rows(Owner::results(&r), "networks")
            .into_iter()
            .map(|n| (text(n, &N::BSSID), cnum(n, &N::LEVEL_DBFS)))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    let clean = signal(WifiGenParams::default(), &[], 1, 0.1, 0);
    let a = levels(&clean);
    assert_eq!(a.len(), 3);
    assert!(a.iter().all(|(_, l)| *l <= 0.0), "{a:?}");
    assert!(a.iter().any(|(_, l)| *l > -0.1), "the network at full power: {a:?}");
    // With noise the level is the signal's own: the noise in the band is taken out, so it stays near the clean level.
    let noisy = levels(&signal(WifiGenParams::default(), &snr(30.0), 1, 0.1, 0));
    assert_eq!(noisy.len(), 3);
    for ((_, x), (_, y)) in a.iter().zip(&noisy) {
        assert!((x - y).abs() < 0.1, "{a:?} {noisy:?}");
    }
    // An OFDM frame with a tone at 9.5 MHz, between the last subcarrier and the band edge, 10 dB below it.
    let f = GenFrame::new(
        beacon(&BssSpec::new([2, 0, 0, 0, 0, 1], "edge", Band::G2, 6, Sec::Open)),
        Phy::Ofdm,
    );
    let frames = WifiGenParams {
        frames: vec![f],
        ..WifiGenParams::default()
    };
    let x = signal(frames.clone(), &[], 1, 0.05, 0);
    let mut ph = 0.0f64;
    let toned: Vec<Complex32> = x
        .iter()
        .map(|&s| {
            ph += std::f64::consts::TAU * 9.5e6 / 20e6;
            s + Complex32::new(ph.cos() as f32, ph.sin() as f32) * 0.316
        })
        .collect();
    let (c, t) = (levels(&x), levels(&toned));
    assert_eq!((c.len(), t.len()), (1, 1));
    assert!(c[0].1 <= 0.0 && (c[0].1 - t[0].1).abs() < 0.1, "{c:?} {t:?}");
}

#[test]
fn a_recording_of_unknown_centre_publishes_no_heard_channel() {
    let x = signal(WifiGenParams::default(), &[], 1, 0.1, 0);
    let mut r = wifi::receiver(&config(WifiSetting::schema(), &[])).unwrap();
    r.configure(&InputInfo::new(20e6, 0.0, 0.0).recording())
        .unwrap();
    feed(&mut r, &x, 65536);
    let res = Owner::results(&r);
    let nets = rows(res, "networks");
    assert_eq!(nets.len(), 3);
    for n in &nets {
        assert_eq!(super::help::cell(n, &N::HEARD_ON), None);
        // The DS Parameter Set of the beacon still names the channel.
        assert!(super::help::cell(n, &N::CHANNEL).is_some());
    }
    assert!(rows(res, "beacons")
        .iter()
        .all(|b| super::help::cell(b, &B::CHANNEL).is_none()));
}

#[test]
fn below_20_msps_the_dsss_beacons_of_2_4_ghz_are_received_and_the_status_says_what_is_not() {
    let x = signal(
        WifiGenParams {
            rate_hz: 22e6,
            ..WifiGenParams::default()
        },
        &snr(10.0),
        1,
        0.1,
        0,
    );
    for rate in [wifi::MIN_RATE_HZ, 11e6, 15e6] {
        let mut rs = sdrkit_algo_dsp::resample::FractionalResampler::with_rates(22e6, rate).unwrap();
        let mut y: Vec<Complex32> = Vec::new();
        rs.process(&x, &mut y);
        let mut r = wifi::receiver(&config(WifiSetting::schema(), &[])).unwrap();
        r.configure(&InputInfo::new(rate, 2437e6, 2437e6).recording())
            .unwrap();
        feed(&mut r, &y, 65536);
        assert_eq!(rows(Owner::results(&r), "networks").len(), 3, "{rate}");
        // One result says what the rate leaves out, and the status line repeats it.
        let want = "below 20 Msps only 802.11b DSSS frames are received";
        let res = Owner::results(&r);
        let said = match value(res, "quiet_reason") {
            Some(Value::Text(t)) => t.as_str(),
            v => panic!("{v:?}"),
        };
        assert!(said.starts_with(want), "{rate}: {said}");
        assert!(res.status().summary.contains(said), "{}", res.status().summary);
    }
    // At 20 Msps nothing is left out. On 5 GHz below 20 Msps nothing is received, and the result says so.
    let mut r = rx();
    r.configure(&InputInfo::new(20e6, 2437e6, 2437e6).recording()).unwrap();
    feed(&mut r, &x[..200_000], 65536);
    assert_eq!(value(Owner::results(&r), "quiet_reason"), None);
    let mut r = rx();
    r.configure(&InputInfo::new(12.5e6, 5180e6, 5180e6).recording()).unwrap();
    feed(&mut r, &x[..200_000], 65536);
    assert_eq!(
        value(Owner::results(&r), "quiet_reason"),
        Some(&Value::Text(
            "below 20 Msps nothing is received in this band: OFDM (802.11a/g/n/ac/ax) needs 20 Msps".into()
        ))
    );
    // The stack's own test signal at the least rate.
    let y = signal(
        WifiGenParams {
            rate_hz: wifi::MIN_RATE_HZ,
            ..WifiGenParams::default()
        },
        &snr(20.0),
        1,
        0.1,
        0,
    );
    let mut r = wifi::receiver(&config(WifiSetting::schema(), &[])).unwrap();
    r.configure(&InputInfo::new(wifi::MIN_RATE_HZ, 2437e6, 2437e6).recording())
        .unwrap();
    feed(&mut r, &y, 65536);
    assert_eq!(rows(Owner::results(&r), "networks").len(), 3);
    let mut r = wifi::receiver(&config(WifiSetting::schema(), &[])).unwrap();
    assert!(r
        .configure(&InputInfo::new(wifi::MIN_RATE_HZ * 0.9, 2437e6, 2437e6).recording())
        .is_err());
}

#[test]
fn the_dsss_level_stays_the_signal_level_down_to_a_weak_signal() {
    let level = |imp: &[(&str, Value)]| {
        let x = signal(WifiGenParams::default(), imp, 1, 0.1, 0);
        let mut r = wifi::receiver(&config(WifiSetting::schema(), &[])).unwrap();
        r.configure(&InputInfo::new(20e6, 2437e6, 2437e6).recording())
            .unwrap();
        feed(&mut r, &x, 65536);
        rows(Owner::results(&r), "networks")
            .into_iter()
            .map(|n| cnum(n, &N::LEVEL_DBFS))
            .fold(f64::MIN, f64::max)
    };
    let clean = level(&[]);
    // The noise estimate errs high (the correlation loss), so a weak signal reads a little low, never high.
    for (db, below) in [(5.0, 1.0), (0.0, 2.0)] {
        let l = level(&snr(db));
        assert!(l < clean + 0.1 && l > clean - below, "SNR {db} dB: {l} against {clean}");
    }
}



/// Live recordings of the `wifi` stack in the folder that `SDRKIT_LIVE_RECORDINGS` names (private: only counts are printed or
/// asserted). A file is not real time, so no data field is skipped. Skips when the variable is unset.
#[test]
#[ignore = "a long run over private recordings"]
fn live_recordings_skip_no_data_field() {
    let Some(dir) = std::env::var_os("SDRKIT_LIVE_RECORDINGS") else {
        eprintln!("SDRKIT_LIVE_RECORDINGS is not set: skipped");
        return;
    };
    let mut metas = Vec::new();
    let mut folders = vec![std::path::PathBuf::from(dir)];
    while let Some(d) = folders.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                folders.push(p);
            } else if p.extension().is_some_and(|x| x == "sigmf-meta") {
                metas.push(p);
            }
        }
    }
    metas.sort();
    let mut runs = 0;
    for meta in metas {
        let data = meta.with_extension("sigmf-data");
        let Ok(rec) = sdrkit_dev_test::sigmf::read_pair(&meta, &data) else {
            continue;
        };
        if rec.description != "wifi" {
            continue;
        }
        runs += 1;
        let r = run(&rec.samples, rec.rate_hz, rec.centre_hz, rec.centre_hz, 65536);
        let res = Owner::results(&r);
        let mut channels: Vec<u32> = networks(&r).iter().map(|n| n.channel).collect();
        channels.sort_unstable();
        channels.dedup();
        eprintln!(
            "{:.0} MHz: {} networks on channels {channels:?}; frames {:?} good, {:?} bad; HT/VHT decoded {:?}, skipped {:?}; channels at once {:?}",
            rec.centre_hz / 1e6,
            networks(&r).len(),
            num(res, "frames_ok"),
            num(res, "frames_bad"),
            num(res, "ppdu_decoded"),
            num(res, "ppdu_not_decoded_busy"),
            num(res, "sub_channels"),
        );
        assert!(!networks(&r).is_empty());
        assert_eq!(num(res, "ppdu_not_decoded_busy"), Some(0.0));
    }
    assert!(runs > 0, "no Wi-Fi recording in the folder");
}

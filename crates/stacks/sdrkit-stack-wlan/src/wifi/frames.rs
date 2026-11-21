//! Frame builders from the clause 9 layouts, for the generator and tests: beacons and probe responses with every element the
//! receiver reads, FILS Discovery frames and protected data frames.

use super::plan::{wide_centre, Band};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_proto_wlan_phy::mac::{self as phy, push_fcs, MgmtSubtype};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;

/// A security configuration to announce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sec {
    /// No security.
    Open,
    /// WEP (privacy bit only).
    Wep,
    /// WPA element only.
    Wpa,
    /// RSN and a WPA element.
    WpaWpa2,
    /// RSN.
    Wpa2,
    /// RSN, transition to WPA3.
    Wpa2Wpa3,
    /// RSN, WPA3 only.
    Wpa3,
    /// OWE.
    Owe,
}

/// HE settings of a BSS.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeSpec {
    /// Streams.
    pub streams: u8,
    /// TWT responder.
    pub twt: bool,
    /// BSS color.
    pub color: u8,
    /// Color disabled.
    pub color_disabled: bool,
    /// 6 GHz regulatory info (0 indoor, 1 standard power, 2 very low power).
    pub reg: u8,
    /// Duplicate beacons.
    pub dup: bool,
}

/// A neighbour for the Reduced Neighbor Report (TBTT information length 12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RnrSpec {
    /// Operating class.
    pub class: u8,
    /// Channel.
    pub channel: u8,
    /// BSSID.
    pub bssid: [u8; 6],
    /// Short SSID.
    pub short_ssid: u32,
    /// BSS parameters.
    pub params: u8,
}

/// A BSS to announce.
#[derive(Clone, Debug, PartialEq)]
pub struct BssSpec {
    /// BSSID.
    pub bssid: [u8; 6],
    /// SSID.
    pub ssid: String,
    /// Hidden (an empty SSID element in beacons).
    pub hidden: bool,
    /// Band.
    pub band: Band,
    /// Primary channel.
    pub channel: u8,
    /// Width, MHz.
    pub width: u32,
    /// Beacon interval, TU.
    pub interval: u16,
    /// Security.
    pub sec: Sec,
    /// Enterprise key management.
    pub enterprise: bool,
    /// Rates in 500 kbit/s units with the basic bit (None: the band's default list).
    pub rates: Option<Vec<u8>>,
    /// TSF.
    pub tsf: u64,
    /// Sequence number.
    pub seq: u16,
    /// Country text (three characters are sent).
    pub country: Option<String>,
    /// Power constraint.
    pub power_constraint: Option<u8>,
    /// Mobility domain.
    pub mdid: Option<u16>,
    /// HT streams (None: no HT).
    pub ht: Option<u8>,
    /// VHT streams (None: no VHT).
    pub vht: Option<u8>,
    /// HE settings.
    pub he: Option<HeSpec>,
    /// EHT.
    pub eht: bool,
    /// Neighbours.
    pub rnr: Vec<RnrSpec>,
}

impl BssSpec {
    /// A plain BSS: 20 MHz, interval 100, no modern elements.
    pub fn new(bssid: [u8; 6], ssid: &str, band: Band, channel: u8, sec: Sec) -> BssSpec {
        BssSpec {
            bssid,
            ssid: ssid.into(),
            hidden: false,
            band,
            channel,
            width: 20,
            interval: 100,
            sec,
            enterprise: false,
            rates: None,
            tsf: 0,
            seq: 0,
            country: None,
            power_constraint: None,
            mdid: None,
            ht: None,
            vht: None,
            he: None,
            eht: false,
            rnr: Vec::new(),
        }
    }
}

fn el(out: &mut Vec<u8>, id: u8, d: &[u8]) {
    phy::push_element(out, id, d).expect("an element of at most 255 bytes");
}

fn ext(out: &mut Vec<u8>, id: u8, d: &[u8]) {
    let mut v = vec![id];
    v.extend_from_slice(d);
    el(out, phy::element::EXTENSION, &v);
}

fn suite(oui: [u8; 3], t: u8) -> [u8; 4] {
    [oui[0], oui[1], oui[2], t]
}

/// An RSN element body.
pub fn rsn_body(pairwise: &[u8], akms: &[u8], mfpc: bool, mfpr: bool) -> Vec<u8> {
    let o = [0x00, 0x0F, 0xAC];
    let mut v = vec![1, 0];
    v.extend(suite(o, 4));
    v.extend((pairwise.len() as u16).to_le_bytes());
    for &p in pairwise {
        v.extend(suite(o, p));
    }
    v.extend((akms.len() as u16).to_le_bytes());
    for &a in akms {
        v.extend(suite(o, a));
    }
    let caps: u16 = (mfpr as u16) << 6 | (mfpc as u16) << 7;
    v.extend(caps.to_le_bytes());
    v
}

fn wpa_body(akm: u8) -> Vec<u8> {
    let o = [0x00, 0x50, 0xF2];
    let mut v = vec![0x00, 0x50, 0xF2, 0x01, 1, 0];
    v.extend(suite(o, 2));
    v.extend(1u16.to_le_bytes());
    v.extend(suite(o, 2));
    v.extend(1u16.to_le_bytes());
    v.extend(suite(o, akm));
    v
}

fn width_code_6g(w: u32) -> u8 {
    match w {
        40 => 1,
        80 => 2,
        160 => 3,
        _ => 0,
    }
}

/// The elements of a beacon (`probe` false) or probe response.
pub fn elements(b: &BssSpec, probe: bool) -> Vec<u8> {
    let mut e = Vec::new();
    if b.hidden && !probe {
        el(&mut e, 0, &[]);
    } else {
        el(&mut e, 0, b.ssid.as_bytes());
    }
    let rates = b.rates.clone().unwrap_or_else(|| match b.band {
        Band::G2 => vec![
            0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24, 0x30, 0x48, 0x60, 0x6C,
        ],
        _ => vec![0x8C, 0x12, 0x98, 0x24, 0xB0, 0x48, 0x60, 0x6C],
    });
    el(&mut e, 1, &rates[..rates.len().min(8)]);
    if b.band != Band::G6 {
        el(&mut e, 3, &[b.channel]);
    }
    if !probe {
        el(&mut e, 5, &[0, 1, 0, 0]);
    }
    if let Some(c) = &b.country {
        let mut d: Vec<u8> = c.bytes().chain(std::iter::repeat(b' ')).take(3).collect();
        d.extend([b.channel.max(1), 4, 23]);
        el(&mut e, 7, &d);
    }
    if let Some(p) = b.power_constraint {
        el(&mut e, 32, &[p]);
    }
    if rates.len() > 8 {
        el(&mut e, 50, &rates[8..]);
    }
    let pmf =
        |e: &mut Vec<u8>, akms: &[u8], c: bool, r: bool| el(e, 48, &rsn_body(&[4], akms, c, r));
    match (b.sec, b.enterprise) {
        (Sec::Wpa, ent) => el(&mut e, 221, &wpa_body(if ent { 1 } else { 2 })),
        (Sec::WpaWpa2, ent) => {
            pmf(&mut e, &[if ent { 1 } else { 2 }], false, false);
            el(&mut e, 221, &wpa_body(if ent { 1 } else { 2 }));
        }
        (Sec::Wpa2, false) => pmf(&mut e, &[2], false, false),
        (Sec::Wpa2, true) => pmf(&mut e, &[1], false, false),
        (Sec::Wpa2Wpa3, false) => pmf(&mut e, &[2, 8], true, false),
        (Sec::Wpa2Wpa3, true) => pmf(&mut e, &[1, 5], true, false),
        (Sec::Wpa3, false) => pmf(&mut e, &[8], true, true),
        (Sec::Wpa3, true) => pmf(&mut e, &[12], true, true),
        (Sec::Owe, _) => pmf(&mut e, &[18], true, true),
        _ => {}
    }
    if let Some(m) = b.mdid {
        let mut d = m.to_le_bytes().to_vec();
        d.push(0x01);
        el(&mut e, 54, &d);
    }
    let forty = b.width >= 40 && b.band != Band::G6;
    if let Some(n) = b.ht {
        let info: u16 = (forty as u16) << 1 | 0x60;
        let mut d = info.to_le_bytes().to_vec();
        d.push(0x17);
        let mut mcs = [0u8; 16];
        for m in mcs.iter_mut().take(n.min(4) as usize) {
            *m = 0xFF;
        }
        d.extend(mcs);
        d.extend([0u8; 7]);
        el(&mut e, 45, &d);
        let (off, wbit) = if forty {
            let c = wide_centre(b.band, b.channel as u32, 40).unwrap_or(b.channel as u32);
            (if c > b.channel as u32 { 1u8 } else { 3u8 }, 4u8)
        } else {
            (0, 0)
        };
        let mut d = vec![b.channel, off | wbit];
        d.extend([0u8; 20]);
        el(&mut e, 61, &d);
    }
    let map = |n: u8, v: u16| -> u16 {
        let mut m = 0xFFFFu16;
        for i in 0..n.min(8) {
            m &= !(3 << (2 * i));
            m |= v << (2 * i);
        }
        m
    };
    if let (Some(n), true) = (b.vht, b.band == Band::G5) {
        let info: u32 = (if b.width >= 160 { 1 } else { 0 }) << 2 | 1 << 5 | 1 << 11;
        let mut d = info.to_le_bytes().to_vec();
        let m = map(n, 2);
        d.extend(m.to_le_bytes());
        d.extend([0, 0]);
        d.extend(m.to_le_bytes());
        d.extend([0, 0]);
        el(&mut e, 191, &d);
        let (w, c0, c1) = match b.width {
            80 => (
                1,
                wide_centre(b.band, b.channel as u32, 80).unwrap_or(0) as u8,
                0,
            ),
            160 => (
                1,
                wide_centre(b.band, b.channel as u32, 80).unwrap_or(0) as u8,
                wide_centre(b.band, b.channel as u32, 160).unwrap_or(0) as u8,
            ),
            _ => (0, 0, 0),
        };
        el(&mut e, 192, &[w, c0, c1, 0xFC, 0xFF]);
    }
    if let Some(h) = b.he {
        let mut d = vec![(h.twt as u8) << 2, 0, 0, 0, 0, 0];
        let ws: u8 = match b.width {
            w if w >= 160 => 0x02 | 0x04,
            80 => 0x02,
            40 if b.band == Band::G2 => 0x01,
            40 => 0x02,
            _ => 0,
        };
        let mut phy = [0u8; 11];
        phy[0] = ws << 1;
        d.extend(phy);
        let m = map(h.streams, 2);
        d.extend(m.to_le_bytes());
        d.extend(m.to_le_bytes());
        ext(&mut e, 35, &d);
        let six = b.band == Band::G6;
        let params: u32 = if six { 1 << 17 } else { 0 };
        let mut d = vec![params as u8, (params >> 8) as u8, (params >> 16) as u8];
        d.push(h.color & 0x3F | (h.color_disabled as u8) << 7);
        d.extend([0xFC, 0xFF]);
        if six {
            let w = if b.width > 160 { 160 } else { b.width };
            let c80 = wide_centre(b.band, b.channel as u32, w.min(80)).unwrap_or(b.channel as u32);
            let (c0, c1) = if w == 160 {
                (c80, wide_centre(b.band, b.channel as u32, 160).unwrap_or(0))
            } else {
                (
                    wide_centre(b.band, b.channel as u32, w).unwrap_or(b.channel as u32),
                    0,
                )
            };
            d.extend([
                b.channel,
                width_code_6g(w) | (h.dup as u8) << 2 | (h.reg & 7) << 3,
                c0 as u8,
                c1 as u8,
                6,
            ]);
        }
        ext(&mut e, 36, &d);
    }
    if b.eht {
        let code = match b.width {
            40 => 1,
            80 => 2,
            160 => 3,
            320 => 4,
            _ => 0,
        };
        let w = b.width;
        let (c0, c1) = if w >= 160 {
            (
                wide_centre(b.band, b.channel as u32, 80).unwrap_or(0),
                wide_centre(b.band, b.channel as u32, w).unwrap_or(0),
            )
        } else {
            (
                wide_centre(b.band, b.channel as u32, w).unwrap_or(b.channel as u32),
                0,
            )
        };
        ext(&mut e, 106, &[1, 0, 0, 0, 0, code, c0 as u8, c1 as u8]);
        ext(&mut e, 108, &[0u8; 15]);
    }
    if !b.rnr.is_empty() {
        let mut d = Vec::new();
        for n in &b.rnr {
            d.extend([0x00, 0x0C, n.class, n.channel, 0xFF]);
            d.extend(n.bssid);
            d.extend(n.short_ssid.to_le_bytes());
            d.push(n.params);
        }
        el(&mut e, 201, &d);
    }
    e
}

fn capability(b: &BssSpec) -> u16 {
    let mut c = 0x0001 | 0x0020 | 0x0400;
    if !matches!(b.sec, Sec::Open | Sec::Owe) {
        c |= 0x0010;
    }
    c
}

fn mgmt_header(subtype: u8, addr1: [u8; 6], bssid: [u8; 6], seq: u16) -> Vec<u8> {
    let mut f = Vec::with_capacity(256);
    phy::push_mgmt_header(
        &mut f,
        MgmtSubtype::from_code(subtype),
        addr1,
        bssid,
        bssid,
        seq,
    );
    f
}

/// A beacon (FCS appended).
pub fn beacon(b: &BssSpec) -> Vec<u8> {
    let mut f = mgmt_header(8, [0xFF; 6], b.bssid, b.seq);
    phy::push_beacon_fields(&mut f, b.tsf, b.interval, capability(b));
    f.extend(elements(b, false));
    push_fcs(&mut f);
    f
}

/// A probe response to `to` (FCS appended); to the broadcast address it is unsolicited.
pub fn probe_response(b: &BssSpec, to: [u8; 6]) -> Vec<u8> {
    let mut f = mgmt_header(5, to, b.bssid, b.seq.wrapping_add(1));
    phy::push_beacon_fields(&mut f, b.tsf.wrapping_add(1024), b.interval, capability(b));
    f.extend(elements(b, true));
    push_fcs(&mut f);
    f
}

/// A FILS Discovery frame (FCS appended).
pub fn fils(b: &BssSpec) -> Vec<u8> {
    let mut f = mgmt_header(13, [0xFF; 6], b.bssid, b.seq.wrapping_add(2));
    let ssid = b.ssid.as_bytes();
    f.extend([4, 34]);
    let fc: u16 = (ssid.len().max(1) as u16 - 1) & 0x1F;
    f.extend(fc.to_le_bytes());
    f.extend(b.tsf.to_le_bytes());
    f.extend(b.interval.to_le_bytes());
    f.extend(ssid);
    push_fcs(&mut f);
    f
}

/// A protected data frame from the DS with a pseudo-random payload (FCS appended).
pub fn data_frame(bssid: [u8; 6], to: [u8; 6], payload: usize, seed: u64) -> Vec<u8> {
    let mut f = vec![0x08, 0x42, 0, 0];
    f.extend(to);
    f.extend(bssid);
    f.extend(bssid);
    f.extend([0x10, 0x00]);
    let mut r = Rng::new(seed);
    let at = f.len();
    f.resize(at + payload, 0);
    r.fill_bytes(&mut f[at..]);
    push_fcs(&mut f);
    f
}

/// The short SSID of a name: its CRC-32/ISO-HDLC (IEEE 802.11-2020 clause 9.4.2.170.3).
pub fn short_ssid(name: &str) -> u32 {
    CRC32_ISO_HDLC.checksum(name.as_bytes())
}

//! 802.11 MAC frames (IEEE 802.11-2020 clause 9, 802.11ax-2021, 802.11be-2024): frame control, beacons, probe responses, FILS
//! Discovery, the elements an access point announces, security, the combined channel and width, rates and the plain-words
//! capabilities. A truncated element is skipped and never over-read.

use super::plan::{class_band, valid, wide_centre, Band};
use crate::report::mac_text;
use sdrkit_proto_wlan_phy::mac as phy;
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Management subtype names.
pub const MGMT_NAMES: [&str; 16] = [
    "assoc req",
    "assoc resp",
    "reassoc req",
    "reassoc resp",
    "probe req",
    "probe resp",
    "timing adv",
    "mgmt 7",
    "beacon",
    "ATIM",
    "disassoc",
    "auth",
    "deauth",
    "action",
    "action no ack",
    "mgmt 15",
];

/// The type name of a frame for the log.
pub fn type_name(ftype: u8, subtype: u8) -> &'static str {
    match ftype {
        0 => MGMT_NAMES[subtype as usize & 15],
        1 => "control",
        2 => "data",
        _ => "extension",
    }
}

/// The MAC header fields every frame has.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// Type 0 management, 1 control, 2 data, 3 extension.
    pub ftype: u8,
    /// Subtype.
    pub subtype: u8,
    /// The second frame control byte.
    pub flags: u8,
    /// Address 1.
    pub addr1: [u8; 6],
    /// Address 2.
    pub addr2: [u8; 6],
    /// Address 3.
    pub addr3: [u8; 6],
}

/// Reads the header with the shared frame parser; `None` for a frame that does not parse (fewer than 10 bytes, another protocol
/// version, a header longer than the frame).
pub fn header(f: &[u8]) -> Option<Header> {
    let fr = phy::Frame::parse(f).ok()?;
    Some(Header {
        ftype: (f[0] >> 2) & 3,
        subtype: fr.fc.subtype,
        flags: f[1],
        addr1: fr.addr1,
        addr2: fr.addr2.unwrap_or_default(),
        addr3: fr.addr3.unwrap_or_default(),
    })
}

/// Cipher and key management suites of an RSN or WPA element.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Suites {
    /// Version.
    pub version: u16,
    /// Pairwise cipher types.
    pub pairwise: Vec<u8>,
    /// AKM types.
    pub akms: Vec<u8>,
    /// Management frame protection required.
    pub mfpr: bool,
    /// Management frame protection capable.
    pub mfpc: bool,
}

/// HT Capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HtCaps {
    /// 40 MHz supported.
    pub forty: bool,
    /// Short guard interval.
    pub sgi: bool,
    /// Spatial streams.
    pub streams: u8,
}

/// HT Operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HtOp {
    /// Primary channel.
    pub primary: u8,
    /// Secondary offset (1 above, 3 below).
    pub sec_offset: u8,
    /// STA channel width bit.
    pub width_bit: bool,
}

/// VHT Capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VhtCaps {
    /// Supported channel width set.
    pub width_set: u8,
    /// Short GI for 80 MHz.
    pub sgi80: bool,
    /// SU beamformer.
    pub su_bf: bool,
    /// MU beamformer.
    pub mu_bf: bool,
    /// Streams.
    pub streams: u8,
    /// Highest MCS.
    pub max_mcs: u8,
}

/// HE Capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeCaps {
    /// TWT requester.
    pub twt_req: bool,
    /// TWT responder.
    pub twt_resp: bool,
    /// Channel width set (7 bits).
    pub width_set: u8,
    /// Streams.
    pub streams: u8,
    /// Highest MCS.
    pub max_mcs: u8,
}

/// The 6 GHz Operation Information of an HE Operation element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SixGhzOp {
    /// Primary channel.
    pub primary: u8,
    /// Width, MHz.
    pub width: u32,
    /// Duplicate beacons.
    pub duplicate: bool,
    /// Regulatory info: 0 indoor AP, 1 standard power AP, 2 very low power AP.
    pub reg: u8,
    /// CCFS0.
    pub ccfs0: u8,
    /// CCFS1.
    pub ccfs1: u8,
    /// Minimum rate.
    pub min_rate: u8,
}

/// HE Operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeOp {
    /// TWT required.
    pub twt_required: bool,
    /// BSS color.
    pub color: u8,
    /// Color disabled.
    pub color_disabled: bool,
    /// 6 GHz Operation Information.
    pub six: Option<SixGhzOp>,
}

/// A Reduced Neighbor Report entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Neighbor {
    /// Operating class.
    pub op_class: u8,
    /// Channel.
    pub channel: u8,
    /// Band from the class.
    pub band: Option<Band>,
    /// BSSID.
    pub bssid: Option<[u8; 6]>,
    /// Short SSID.
    pub short_ssid: Option<u32>,
    /// BSS parameters.
    pub params: Option<u8>,
}

impl Neighbor {
    /// Same SSID (bit 1).
    pub fn same_ssid(&self) -> bool {
        self.params.is_some_and(|p| p & 0x02 != 0)
    }
    /// Member of an ESS with a co-located 2.4/5 GHz AP (bit 4).
    pub fn ess_member(&self) -> bool {
        self.params.is_some_and(|p| p & 0x10 != 0)
    }
    /// Unsolicited probe responses (bit 5).
    pub fn unsolicited(&self) -> bool {
        self.params.is_some_and(|p| p & 0x20 != 0)
    }
    /// Co-located AP (bit 6).
    pub fn co_located(&self) -> bool {
        self.params.is_some_and(|p| p & 0x40 != 0)
    }
    /// `"<band> ch <n>[ <BSSID>][ (co-located)][ (unsolicited probe responses)][ short SSID XXXXXXXX]"`.
    pub fn text(&self) -> String {
        let mut s = format!(
            "{} ch {}",
            self.band.unwrap_or(Band::G6).name(),
            self.channel
        );
        if let Some(b) = self.bssid {
            let _ = write!(s, " {}", mac_text(&b));
        }
        if self.co_located() {
            s.push_str(" (co-located)");
        }
        if self.unsolicited() {
            s.push_str(" (unsolicited probe responses)");
        }
        if let Some(x) = self.short_ssid {
            let _ = write!(s, " short SSID {x:08X}");
        }
        s
    }
}

/// Elements of a beacon, probe response or FILS Discovery frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Elements {
    /// SSID bytes (the first SSID element).
    pub ssid: Option<Vec<u8>>,
    /// Rates in 500 kbit/s units with the basic bit.
    pub rates: Vec<u8>,
    /// DS Parameter Set channel.
    pub ds_channel: Option<u8>,
    /// Country (three characters).
    pub country: Option<String>,
    /// Power constraint, dB.
    pub power_constraint: Option<u8>,
    /// HT Capabilities.
    pub ht: Option<HtCaps>,
    /// HT Operation.
    pub ht_op: Option<HtOp>,
    /// RSN.
    pub rsn: Option<Suites>,
    /// WPA (vendor element).
    pub wpa: Option<Suites>,
    /// Mobility domain.
    pub mdid: Option<u16>,
    /// VHT Capabilities.
    pub vht: Option<VhtCaps>,
    /// VHT Operation: (width, CCFS0, CCFS1).
    pub vht_op: Option<(u8, u8, u8)>,
    /// HE Capabilities.
    pub he: Option<HeCaps>,
    /// HE Operation.
    pub he_op: Option<HeOp>,
    /// EHT Operation with a width: (width MHz, CCFS0, CCFS1).
    pub eht_op: Option<(u32, u8, u8)>,
    /// EHT Capabilities present.
    pub eht_caps: bool,
    /// Reduced Neighbor Report entries.
    pub rnr: Vec<Neighbor>,
    /// Whether an RNR element was present.
    pub has_rnr: bool,
    /// Elements read.
    pub count: usize,
}

/// The suites of an RSN element body, or of a WPA element body after its OUI and type (the same layout), with only the suites
/// of `oui` kept. Standard: IEEE 802.11-2020 clause 9.4.2.24.
fn suites(b: &[u8], oui: [u8; 3]) -> Suites {
    let mut s = Suites::default();
    let Ok(r) = phy::Rsn::parse(b) else {
        if b.len() >= 2 {
            s.version = u16::from_le_bytes([b[0], b[1]]);
        }
        return s;
    };
    s.version = r.version;
    s.pairwise = r
        .pairwise_suites()
        .filter(|x| x.oui == oui)
        .map(|x| x.kind)
        .collect();
    for a in r.akm_suites().filter(|x| x.oui == oui) {
        if !s.akms.contains(&a.kind) {
            s.akms.push(a.kind);
        }
    }
    if let Some(caps) = r.capabilities {
        s.mfpr = caps & 0x40 != 0;
        s.mfpc = caps & 0x80 != 0;
    }
    s
}

fn mcs_map_streams(map: u16) -> (u8, u8) {
    let mut streams = 0u8;
    let mut max = 0u8;
    for i in 0..8 {
        let v = ((map >> (2 * i)) & 3) as u8;
        if v != 3 {
            streams = i as u8 + 1;
            max = max.max(v);
        }
    }
    (streams, max)
}

fn rnr(b: &[u8], out: &mut Vec<Neighbor>) {
    let mut at = 0usize;
    while at + 4 <= b.len() {
        let hdr = u16::from_le_bytes([b[at], b[at + 1]]);
        let count = ((hdr >> 4) & 0x0F) as usize + 1;
        let len = (hdr >> 8) as usize;
        let class = b[at + 2];
        let channel = b[at + 3];
        at += 4;
        if len == 0 || at + count * len > b.len() {
            return;
        }
        for _ in 0..count {
            let set = &b[at..at + len];
            at += len;
            let mut p = 1usize;
            let mut n = Neighbor {
                op_class: class,
                channel,
                band: class_band(class),
                ..Default::default()
            };
            if matches!(len, 7 | 8 | 11 | 12 | 13 | 16) && p + 6 <= set.len() {
                let mut a = [0u8; 6];
                a.copy_from_slice(&set[p..p + 6]);
                n.bssid = Some(a);
                p += 6;
            }
            if matches!(len, 5 | 6 | 11 | 12 | 13 | 16) && p + 4 <= set.len() {
                n.short_ssid = Some(u32::from_le_bytes([
                    set[p],
                    set[p + 1],
                    set[p + 2],
                    set[p + 3],
                ]));
                p += 4;
            }
            if matches!(len, 2 | 6 | 8 | 12 | 13 | 16) && p < set.len() {
                n.params = Some(set[p]);
            }
            if n.bssid.is_some() || n.short_ssid.is_some() {
                out.push(n);
            }
        }
    }
}

/// Reads the elements.
pub fn elements(b: &[u8]) -> Elements {
    let mut e = Elements::default();
    for el in phy::elements(b) {
        let (id, d) = (el.id, el.data);
        e.count += 1;
        match id {
            0 => {
                if e.ssid.is_none() {
                    e.ssid = Some(d.to_vec());
                }
            }
            1 | 50 => e.rates.extend_from_slice(d),
            3 => e.ds_channel = d.first().copied(),
            7 => {
                if d.len() >= 3 {
                    e.country = Some(
                        d[..3]
                            .iter()
                            .map(|&c| {
                                if (0x20..0x7F).contains(&c) {
                                    c as char
                                } else {
                                    '?'
                                }
                            })
                            .collect(),
                    );
                }
            }
            32 => e.power_constraint = d.first().copied(),
            45 => {
                if d.len() >= 26 {
                    let info = u16::from_le_bytes([d[0], d[1]]);
                    let mut streams = 0;
                    for (i, &m) in d[3..7].iter().enumerate() {
                        if m != 0 {
                            streams = i as u8 + 1;
                        }
                    }
                    e.ht = Some(HtCaps {
                        forty: info & 0x02 != 0,
                        sgi: info & 0x60 != 0,
                        streams,
                    });
                }
            }
            61 => {
                if d.len() >= 2 {
                    e.ht_op = Some(HtOp {
                        primary: d[0],
                        sec_offset: d[1] & 3,
                        width_bit: d[1] & 4 != 0,
                    });
                }
            }
            48 => e.rsn = Some(suites(d, [0x00, 0x0F, 0xAC])),
            54 => {
                if d.len() >= 3 {
                    e.mdid = Some(u16::from_le_bytes([d[0], d[1]]));
                }
            }
            191 => {
                if d.len() >= 12 {
                    let info = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
                    let (streams, max) = mcs_map_streams(u16::from_le_bytes([d[4], d[5]]));
                    e.vht = Some(VhtCaps {
                        width_set: ((info >> 2) & 3) as u8,
                        sgi80: info & (1 << 5) != 0,
                        su_bf: info & (1 << 11) != 0,
                        mu_bf: info & (1 << 19) != 0,
                        streams,
                        max_mcs: 7 + max,
                    });
                }
            }
            192 => {
                if d.len() >= 3 {
                    e.vht_op = Some((d[0], d[1], d[2]));
                }
            }
            201 => {
                e.has_rnr = true;
                rnr(d, &mut e.rnr);
            }
            221 => {
                if d.len() >= 4 && d[..4] == [0x00, 0x50, 0xF2, 0x01] {
                    e.wpa = Some(suites(&d[4..], [0x00, 0x50, 0xF2]));
                }
            }
            255 => {
                let Some(ext) = el.ext else {
                    continue;
                };
                let x = d;
                match ext {
                    35 => {
                        if x.len() >= 6 + 11 + 4 {
                            let (streams, max) =
                                mcs_map_streams(u16::from_le_bytes([x[17], x[18]]));
                            e.he = Some(HeCaps {
                                twt_req: x[0] & 0x02 != 0,
                                twt_resp: x[0] & 0x04 != 0,
                                width_set: x[6] >> 1,
                                streams,
                                max_mcs: [7, 9, 11][max.min(2) as usize],
                            });
                        }
                    }
                    36 => {
                        if x.len() >= 3 + 1 + 2 {
                            let p = x[0] as u32 | (x[1] as u32) << 8 | (x[2] as u32) << 16;
                            let mut op = HeOp {
                                twt_required: p & (1 << 3) != 0,
                                color: x[3] & 0x3F,
                                color_disabled: x[3] & 0x80 != 0,
                                six: None,
                            };
                            let mut at6 = 6;
                            if p & (1 << 14) != 0 {
                                at6 += 3;
                            }
                            if p & (1 << 15) != 0 {
                                at6 += 1;
                            }
                            if p & (1 << 17) != 0 && at6 + 5 <= x.len() {
                                let s = &x[at6..at6 + 5];
                                op.six = Some(SixGhzOp {
                                    primary: s[0],
                                    width: [20, 40, 80, 160][(s[1] & 3) as usize],
                                    duplicate: s[1] & 4 != 0,
                                    reg: (s[1] >> 3) & 7,
                                    ccfs0: s[2],
                                    ccfs1: s[3],
                                    min_rate: s[4],
                                });
                            }
                            e.he_op = Some(op);
                        }
                    }
                    106 => {
                        if x.len() > 4 && x[0] & 1 != 0 && x.len() >= 1 + 4 + 3 {
                            let w = [20, 40, 80, 160, 320, 0, 0, 0][(x[5] & 7) as usize];
                            if w > 0 {
                                e.eht_op = Some((w, x[6], x[7]));
                            }
                        }
                    }
                    108 => e.eht_caps = true,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    e
}

/// The kind of a frame that describes a BSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BssFrame {
    /// Beacon.
    Beacon,
    /// Probe response.
    ProbeResp,
    /// FILS Discovery.
    Fils,
}

/// What a beacon, probe response or FILS Discovery frame says about its BSS.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bss {
    /// The kind of frame.
    pub frame: BssFrame,
    /// BSSID.
    pub bssid: [u8; 6],
    /// SSID text ("" when hidden or absent).
    pub ssid: String,
    /// Hidden (empty or all-zero SSID).
    pub hidden: bool,
    /// Beacon interval in TU.
    pub interval_tu: u16,
    /// Timestamp.
    pub timestamp: u64,
    /// Capability information (0 for FILS).
    pub capability: u16,
    /// A probe response to the broadcast address.
    pub unsolicited: bool,
    /// FILS short SSID.
    pub short_ssid: Option<u32>,
    /// The elements.
    pub el: Elements,
}

fn ssid_text(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s
            .chars()
            .map(|c| {
                if (c as u32) < 0x20 || c as u32 == 0x7F {
                    '.'
                } else {
                    c
                }
            })
            .collect(),
        Err(_) => b
            .iter()
            .map(|&c| {
                if (0x20..0x7F).contains(&c) || c >= 0x80 {
                    c as char
                } else {
                    '.'
                }
            })
            .collect(),
    }
}

/// Parses a frame (FCS removed) that describes a BSS; `None` for any other frame or a malformed one.
pub fn parse_bss(f: &[u8]) -> Option<Bss> {
    let h = header(f)?;
    if h.ftype != 0 {
        return None;
    }
    match h.subtype {
        5 | 8 => {
            let body_at = if h.flags & 0x80 != 0 { 28 } else { 24 };
            let body = f.get(body_at..)?;
            if body.len() < 12 {
                return None;
            }
            let el = elements(&body[12..]);
            let (ssid, hidden) = match &el.ssid {
                Some(s) => (ssid_text(s), s.is_empty() || s.iter().all(|&c| c == 0)),
                None => (String::new(), true),
            };
            Some(Bss {
                frame: if h.subtype == 8 {
                    BssFrame::Beacon
                } else {
                    BssFrame::ProbeResp
                },
                bssid: h.addr3,
                ssid: if hidden { String::new() } else { ssid },
                hidden,
                timestamp: u64::from_le_bytes(body[0..8].try_into().ok()?),
                interval_tu: u16::from_le_bytes([body[8], body[9]]),
                capability: u16::from_le_bytes([body[10], body[11]]),
                unsolicited: h.subtype == 5 && h.addr1 == [0xFF; 6],
                short_ssid: None,
                el,
            })
        }
        13 => parse_fils(f, &h),
        _ => None,
    }
}

fn parse_fils(f: &[u8], h: &Header) -> Option<Bss> {
    if f.len() < 28 {
        return None;
    }
    let b = &f[24..];
    if b.len() < 2 || b[0] != 4 || b[1] != 34 {
        return None;
    }
    let need = 2 + 2 + 8 + 2;
    if b.len() < need {
        return None;
    }
    let fc = u16::from_le_bytes([b[2], b[3]]);
    let ts = u64::from_le_bytes(b[4..12].try_into().ok()?);
    let interval = u16::from_le_bytes([b[12], b[13]]);
    let rest = &b[14..];
    let (ssid, short) = if fc & (1 << 6) != 0 {
        if rest.len() < 4 {
            return None;
        }
        (
            String::new(),
            Some(u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]])),
        )
    } else {
        let n = (fc & 0x1F) as usize + 1;
        if rest.len() < n {
            return None;
        }
        let s = &rest[..n];
        (ssid_text(s), Some(CRC32_ISO_HDLC.checksum(s)))
    };
    Some(Bss {
        frame: BssFrame::Fils,
        bssid: h.addr2,
        hidden: ssid.is_empty(),
        ssid,
        interval_tu: interval,
        timestamp: ts,
        capability: 0,
        unsolicited: false,
        short_ssid: short,
        el: Elements::default(),
    })
}

/// AKM names.
pub fn akm_name(a: u8) -> String {
    match a {
        1 => "802.1X".into(),
        2 => "PSK".into(),
        3 => "FT-802.1X".into(),
        4 => "FT-PSK".into(),
        5 => "802.1X-SHA256".into(),
        6 => "PSK-SHA256".into(),
        8 => "SAE".into(),
        9 => "FT-SAE".into(),
        11 => "Suite-B".into(),
        12 => "Suite-B-192".into(),
        18 => "OWE".into(),
        24 => "SAE-EXT".into(),
        25 => "FT-SAE-EXT".into(),
        n => format!("AKM{n}"),
    }
}

/// Cipher names.
pub fn cipher_name(c: u8) -> String {
    match c {
        2 => "TKIP".into(),
        4 => "CCMP".into(),
        8 => "GCMP".into(),
        9 => "GCMP-256".into(),
        10 => "CCMP-256".into(),
        n => format!("cipher{n}"),
    }
}

sdrkit_core::choice_enum! {
    /// The security of a network, from its RSN and WPA elements and the privacy bit. Standard: IEEE Std 802.11-2020 clause
    /// 9.4.2.24 (RSN element) and the WPA3 specification of the Wi-Fi Alliance (transition modes).
    #[derive(Default, Serialize, Deserialize)]
    pub enum WifiSecurity {
        /// No security.
        #[default]
        Open = ("open", "Open"),
        /// WEP (the privacy bit without RSN or WPA).
        Wep = ("wep", "WEP"),
        /// WPA with a pre-shared key.
        Wpa = ("wpa", "WPA"),
        /// WPA and WPA2 together.
        WpaWpa2 = ("wpa_wpa2", "WPA/WPA2"),
        /// WPA2 with a pre-shared key.
        Wpa2 = ("wpa2", "WPA2"),
        /// WPA2 and WPA3 together (the transition mode).
        Wpa2Wpa3 = ("wpa2_wpa3", "WPA2/WPA3"),
        /// WPA3 (SAE).
        Wpa3 = ("wpa3", "WPA3"),
        /// Opportunistic Wireless Encryption (Enhanced Open).
        Owe = ("owe", "OWE (Enhanced Open)"),
        /// WPA with 802.1X.
        WpaEnterprise = ("wpa_enterprise", "WPA-Enterprise"),
        /// WPA and WPA2 with 802.1X.
        WpaWpa2Enterprise = ("wpa_wpa2_enterprise", "WPA/WPA2-Enterprise"),
        /// WPA2 with 802.1X.
        Wpa2Enterprise = ("wpa2_enterprise", "WPA2-Enterprise"),
        /// WPA2 and WPA3 with 802.1X (the transition mode).
        Wpa2Wpa3Enterprise = ("wpa2_wpa3_enterprise", "WPA2/WPA3-Enterprise"),
        /// WPA3 with 802.1X, the 192-bit Suite B mode included.
        Wpa3Enterprise = ("wpa3_enterprise", "WPA3-Enterprise"),
    }
}

impl WifiSecurity {
    /// The kind for a base name of [`Security::base`] and the enterprise flag. Unknown names are open.
    pub fn of(base: &str, enterprise: bool) -> WifiSecurity {
        use WifiSecurity as S;
        match (base, enterprise) {
            ("WEP", _) => S::Wep,
            ("WPA", false) => S::Wpa,
            ("WPA", true) => S::WpaEnterprise,
            ("WPA/WPA2", false) => S::WpaWpa2,
            ("WPA/WPA2", true) => S::WpaWpa2Enterprise,
            ("WPA2", false) => S::Wpa2,
            ("WPA2", true) => S::Wpa2Enterprise,
            ("WPA2/WPA3", false) => S::Wpa2Wpa3,
            ("WPA2/WPA3", true) => S::Wpa2Wpa3Enterprise,
            ("WPA3", false) => S::Wpa3,
            ("WPA3", true) => S::Wpa3Enterprise,
            (b, _) if b.starts_with("OWE") => S::Owe,
            _ => S::Open,
        }
    }

    /// Open or OWE: no key is needed to join.
    pub fn is_open(self) -> bool {
        matches!(self, WifiSecurity::Open | WifiSecurity::Owe)
    }

    /// WPA3 in any mode, or OWE.
    pub fn is_wpa3_or_owe(self) -> bool {
        use WifiSecurity as S;
        matches!(
            self,
            S::Wpa3 | S::Wpa2Wpa3 | S::Owe | S::Wpa3Enterprise | S::Wpa2Wpa3Enterprise
        )
    }
}

sdrkit_core::choice_enum! {
    /// The newest 802.11 amendment a network or a station declares, with the Wi-Fi Alliance generation number where one
    /// exists. Standard: IEEE Std 802.11-2020 clauses 15 to 21, 802.11ax-2021, 802.11be-2024.
    #[derive(Serialize, Deserialize)]
    pub enum WifiGeneration {
        /// 802.11b (DSSS and CCK).
        B = ("b", "802.11b"),
        /// 802.11a (OFDM at 5 GHz).
        A = ("a", "802.11a"),
        /// 802.11g (OFDM at 2.4 GHz).
        G = ("g", "802.11g"),
        /// Wi-Fi 4, 802.11n (HT).
        N = ("n", "Wi-Fi 4 (802.11n)"),
        /// Wi-Fi 5, 802.11ac (VHT).
        Ac = ("ac", "Wi-Fi 5 (802.11ac)"),
        /// Wi-Fi 6, 802.11ax (HE).
        Ax = ("ax", "Wi-Fi 6 (802.11ax)"),
        /// Wi-Fi 7, 802.11be (EHT).
        Be = ("be", "Wi-Fi 7 (802.11be)"),
    }
}

impl WifiGeneration {
    /// The generation of a Wi-Fi Alliance number: 4 HT, 5 VHT, 6 HE, 7 EHT. `None` for another number.
    pub fn of_number(n: u8) -> Option<WifiGeneration> {
        match n {
            4 => Some(WifiGeneration::N),
            5 => Some(WifiGeneration::Ac),
            6 => Some(WifiGeneration::Ax),
            7 => Some(WifiGeneration::Be),
            _ => None,
        }
    }
}

/// Security as announced.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Security {
    /// "Open", "WEP", "WPA", "WPA/WPA2", "WPA2", "WPA2/WPA3", "WPA3", "OWE (Enhanced Open)", with "-Enterprise" where it applies.
    pub name: String,
    /// The base name without "-Enterprise".
    pub base: String,
    /// Enterprise.
    pub enterprise: bool,
    /// Ciphers joined with "+".
    pub ciphers: String,
    /// AKMs joined with ", ".
    pub akms: String,
    /// "required", "capable" or "no".
    pub pmf: String,
    /// The kind, from the base name and the enterprise flag.
    pub kind: WifiSecurity,
}

/// The security of a BSS (from RSN, WPA and the privacy bit).
pub fn security(el: &Elements, capability: u16) -> Security {
    let mut s = Security::default();
    let has = |v: &[u8], xs: &[u8]| v.iter().any(|a| xs.contains(a));
    let src = el.rsn.as_ref().or(el.wpa.as_ref());
    if let Some(x) = src {
        s.ciphers = x
            .pairwise
            .iter()
            .map(|&c| cipher_name(c))
            .collect::<Vec<_>>()
            .join("+");
        s.akms = x
            .akms
            .iter()
            .map(|&a| akm_name(a))
            .collect::<Vec<_>>()
            .join(", ");
        s.pmf = if x.mfpr {
            "required"
        } else if x.mfpc {
            "capable"
        } else {
            "no"
        }
        .into();
    } else {
        s.pmf = "no".into();
    }
    const SAE: &[u8] = &[8, 9, 24, 25];
    const SUITE_B: &[u8] = &[11, 12];
    const PSK: &[u8] = &[2, 4, 6];
    const DOT1X: &[u8] = &[1, 3, 5];
    if let Some(r) = &el.rsn {
        let a = &r.akms;
        if !a.is_empty() && a.iter().all(|&x| x == 18) {
            s.base = "OWE (Enhanced Open)".into();
        } else if has(a, SAE) || has(a, SUITE_B) {
            let transition = has(a, PSK) || (has(a, DOT1X) && !has(a, SUITE_B));
            s.base = if transition { "WPA2/WPA3" } else { "WPA3" }.into();
        } else if el.wpa.is_some() {
            s.base = "WPA/WPA2".into();
        } else if a.contains(&1) && a.contains(&5) && r.mfpc {
            // 802.1X with its SHA-256 variant and PMF offered: the WPA3-Enterprise transition mode.
            s.base = "WPA2/WPA3".into();
        } else {
            s.base = "WPA2".into();
        }
        s.enterprise = (has(a, DOT1X) && !has(a, PSK) && !has(a, SAE)) || has(a, SUITE_B);
    } else if let Some(w) = &el.wpa {
        s.base = "WPA".into();
        s.enterprise = w.akms.contains(&1) && !w.akms.contains(&2);
    } else if capability & 0x0010 != 0 {
        s.base = "WEP".into();
    } else {
        s.base = "Open".into();
    }
    s.name = if s.enterprise {
        format!("{}-Enterprise", s.base)
    } else {
        s.base.clone()
    };
    s.kind = WifiSecurity::of(&s.base, s.enterprise);
    s
}

/// Rates as text: "1 2 5.5 11 6 9 …" (membership selectors left out).
pub fn rates_text(rates: &[u8]) -> String {
    let mut s = String::new();
    for &r in rates {
        let v = r & 0x7F;
        if v == 0 || v == 0x7F {
            continue;
        }
        if !s.is_empty() {
            s.push(' ');
        }
        if v % 2 == 0 {
            let _ = write!(s, "{}", v / 2);
        } else {
            let _ = write!(s, "{}.5", v / 2);
        }
    }
    s
}

/// One HT stream's top rate (MCS 7) in Mbit/s at 20 or 40 MHz.
pub fn ht_rate(width: u32, sgi: bool) -> f64 {
    let sd = if width >= 40 { 108.0 } else { 52.0 };
    sd * 5.0 / if sgi { 3.6 } else { 4.0 }
}

/// One VHT stream's rate in Mbit/s (short GI) at a width and MCS 7, 8 or 9.
pub fn vht_rate(width: u32, mcs: u8) -> f64 {
    let sd = match width {
        20 => 52.0,
        40 => 108.0,
        80 => 234.0,
        _ => 468.0,
    };
    let bits = match mcs {
        7 => 5.0,
        8 => 6.0,
        _ => 20.0 / 3.0,
    };
    sd * bits / 3.6
}

/// One HE stream's rate in Mbit/s (0.8 µs GI) at a width and MCS 7, 9 or 11.
pub fn he_rate(width: u32, mcs: u8) -> f64 {
    let sd = match width {
        20 => 234.0,
        40 => 468.0,
        80 => 980.0,
        _ => 1960.0,
    };
    let bits = match mcs {
        7 => 5.0,
        9 => 20.0 / 3.0,
        _ => 25.0 / 3.0,
    };
    sd * bits / 13.6
}

/// The combined view of a BSS on a band.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Combined {
    /// Primary channel.
    pub primary: Option<u8>,
    /// Channel shown (DS Parameter Set, else primary).
    pub channel: Option<u8>,
    /// Width, MHz.
    pub width: u32,
    /// CCFS0 (the centre).
    pub centre: Option<u8>,
    /// Standard text ("a/n/ac/ax").
    pub standard: String,
    /// The newest amendment of the standard text.
    pub generation: Option<WifiGeneration>,
    /// Capabilities in plain words.
    pub caps: String,
}

/// Combines the elements given the band of the radio.
pub fn combine(el: &Elements, band: Option<Band>) -> Combined {
    let mut c = Combined {
        primary: el
            .he_op
            .and_then(|h| h.six.map(|s| s.primary))
            .or(el.ht_op.map(|h| h.primary)),
        width: 20,
        ..Default::default()
    };
    c.centre = c.primary;
    if let Some(h) = el.ht_op {
        if h.width_bit && (h.sec_offset == 1 || h.sec_offset == 3) {
            c.width = 40;
            c.centre = Some(if h.sec_offset == 1 {
                h.primary.wrapping_add(2)
            } else {
                h.primary.wrapping_sub(2)
            });
        }
    }
    if let Some((w, c0, c1)) = el.vht_op {
        match w {
            1 => {
                if c1 != 0 && (c1 as i32 - c0 as i32).abs() == 8 {
                    c.width = 160;
                    c.centre = Some(c1);
                } else {
                    c.width = 80;
                    c.centre = Some(c0);
                }
            }
            2 => {
                c.width = 160;
                c.centre = Some(c0);
            }
            3 => {
                c.width = 80;
                c.centre = Some(c0);
            }
            _ => {}
        }
    }
    if let Some(s) = el.he_op.and_then(|h| h.six) {
        c.width = s.width;
        c.centre = Some(if s.width == 160 && s.ccfs1 != 0 {
            s.ccfs1
        } else {
            s.ccfs0
        });
    }
    if let Some((w, c0, c1)) = el.eht_op {
        c.width = w;
        c.centre = Some(if (w == 160 || w == 320) && c1 != 0 {
            c1
        } else {
            c0
        });
    }
    c.channel = el.ds_channel.or(c.primary);
    let mut parts: Vec<&str> = Vec::new();
    match band {
        Some(Band::G5) => parts.push("a"),
        Some(Band::G2) => parts.push("g"),
        _ => {}
    }
    if el.ht.is_some() {
        parts.push("n");
    }
    if el.vht.is_some() && band != Some(Band::G2) {
        parts.push("ac");
    }
    if el.he.is_some() {
        parts.push("ax");
    }
    if el.eht_caps {
        parts.push("be");
    }
    c.generation = parts
        .last()
        .and_then(|&p| <WifiGeneration as sdrkit_core::value::Choice>::from_key(p));
    c.standard = parts.join("/");
    c.caps = caps_text(el, c.width);
    c
}

/// A one-stream rate as shown, to 0.1 Mbit/s.
fn tenth(r: f64) -> f64 {
    (r * 10.0).round() / 10.0
}

fn caps_text(el: &Elements, combined_width: u32) -> String {
    let mut s = String::new();
    let (gen, streams, width, rate) = if el.eht_caps {
        ("Wi-Fi 7 (EHT)", None, combined_width, None)
    } else if let Some(h) = el.he {
        let w = if h.width_set & 0x0C != 0 {
            160
        } else if h.width_set & 0x02 != 0 {
            80
        } else if h.width_set & 0x01 != 0 {
            40
        } else {
            20
        };
        (
            "Wi-Fi 6 (HE)",
            Some(h.streams),
            w,
            Some(h.streams as f64 * tenth(he_rate(w, h.max_mcs))),
        )
    } else if let Some(v) = el.vht {
        let w = if v.width_set != 0 { 160 } else { 80 };
        (
            "Wi-Fi 5 (VHT)",
            Some(v.streams),
            w,
            Some(v.streams as f64 * tenth(vht_rate(w, v.max_mcs))),
        )
    } else if let Some(h) = el.ht {
        let w = if h.forty { 40 } else { 20 };
        (
            "Wi-Fi 4 (HT)",
            Some(h.streams),
            w,
            Some(h.streams as f64 * tenth(ht_rate(w, h.sgi))),
        )
    } else {
        return s;
    };
    s.push_str(gen);
    if let Some(n) = streams {
        let _ = write!(s, " {n}x{n}");
    }
    let _ = write!(s, ", up to {width} MHz wide");
    if let Some(r) = rate {
        let _ = write!(s, ", up to {} Mbit/s", (r + 1e-6).floor() as u64);
    }
    if let Some(h) = el.he {
        if h.twt_req || h.twt_resp {
            s.push_str(", TWT");
        }
    }
    if let Some(o) = el.he_op {
        let _ = write!(s, ", BSS color {}", o.color);
        if o.color_disabled {
            s.push_str(" (disabled)");
        }
    }
    s
}

/// The log summary of a BSS frame.
pub fn summary(b: &Bss, channel: Option<u8>, security: &str) -> String {
    match b.frame {
        BssFrame::Fils => {
            if b.ssid.is_empty() {
                format!(
                    "FILS discovery (short SSID {:08X}), {} TU",
                    b.short_ssid.unwrap_or(0),
                    b.interval_tu
                )
            } else {
                format!("FILS discovery \"{}\", {} TU", b.ssid, b.interval_tu)
            }
        }
        _ => {
            let mut s = String::from(if b.frame == BssFrame::Beacon {
                "beacon"
            } else {
                "probe resp"
            });
            if b.hidden {
                s.push_str(" (hidden)");
            } else {
                let _ = write!(s, " \"{}\"", b.ssid);
            }
            if let Some(c) = channel {
                let _ = write!(s, ", ch {c}");
            }
            let _ = write!(s, ", {security}, {} TU", b.interval_tu);
            s
        }
    }
}

/// Whether a frame heard on `heard` of a 5/6 GHz band is the non-HT duplicate of a wider BSS (3.9).
pub fn is_duplicate(b: &Bss, band: Option<Band>, heard: u32) -> bool {
    let Some(band) = band else {
        return false;
    };
    if band == Band::G2 || heard == 0 || b.frame == BssFrame::Fils {
        return false;
    }
    let named =
        b.el.ds_channel
            .or(b.el.he_op.and_then(|h| h.six.map(|s| s.primary)))
            .or(b.el.ht_op.map(|h| h.primary));
    match named {
        Some(c) => valid(band, c as u32) && c as u32 != heard,
        None => false,
    }
}

/// Whether a wider block exists (re-exported for the generator).
pub fn block_centre(band: Band, primary: u32, width: u32) -> Option<u32> {
    wide_centre(band, primary, width)
}

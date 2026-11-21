//! HaLow MAC frames: the S1G beacon, management beacons and probe responses, the names of other frames and the S1G elements.
//! The element walk, the RSN reader and the FCS are the shared ones. Builders for the generator and tests are at the end.
//! Standard: IEEE Std 802.11-2020 clause 9.3.4.3 (S1G beacon) and clause 9.4.2.

use crate::report::mac_text;
use crate::wifi::mac::MGMT_NAMES;
use sdrkit_proto_wlan_phy::mac::{self as phy, fcs_ok, push_fcs, IEEE_OUI};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;

/// Text with control bytes replaced by `bad` (other bytes as UTF-8 where valid).
fn printable(b: &[u8], bad: char) -> String {
    String::from_utf8_lossy(b)
        .chars()
        .map(|c| if c.is_control() { bad } else { c })
        .collect()
}
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// The kind of frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameKind {
    /// S1G beacon with elements.
    S1gBeacon,
    /// S1G short beacon (no elements).
    ShortBeacon,
    /// Management beacon.
    Beacon,
    /// Probe response.
    ProbeResp,
    /// Any other frame.
    Other,
}

/// The S1G Operation element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct S1gOp {
    /// Operating width, MHz.
    pub width: u32,
    /// Primary channel width, MHz (1 or 2).
    pub primary_width: u32,
    /// Operating class.
    pub class: u8,
    /// Primary channel number.
    pub primary: u8,
    /// Channel centre frequency number.
    pub channel: u8,
    /// Basic S1G-MCS for one stream: 0 MCS 0–2, 1 MCS 0–7, 2 MCS 0–9, 3 none.
    pub basic_mcs: Option<u8>,
}

/// A parsed frame (the `s1g.frame` stream, without the PPDU fields).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct S1gMac {
    /// Kind.
    pub kind: FrameKind,
    /// Type.
    pub ftype: u8,
    /// Subtype.
    pub subtype: u8,
    /// BSSID (MAC text) when the frame has one.
    pub bssid: Option<String>,
    /// SSID: Some("") when the element is present and empty (hidden).
    pub ssid: Option<String>,
    /// Compressed SSID.
    pub compressed_ssid: Option<u32>,
    /// Security name.
    pub security: String,
    /// Beacon interval, TU.
    pub interval_tu: Option<u16>,
    /// Timestamp (low 4 bytes for management beacons).
    pub timestamp: u64,
    /// S1G beacon BSS bandwidth indication.
    pub bw_indication: Option<u8>,
    /// S1G beacon security bit (or the privacy capability).
    pub security_bit: bool,
    /// S1G Operation.
    pub op: Option<S1gOp>,
    /// Long preamble supported (S1G Capabilities).
    pub long_preamble: Option<bool>,
    /// Country (two letters).
    pub country: Option<String>,
    /// DTIM period.
    pub dtim_period: Option<u8>,
    /// RPS present.
    pub raw: bool,
    /// TWT present.
    pub twt: bool,
    /// Power constraint, dB.
    pub power_constraint: Option<u8>,
    /// Vendor element OUIs.
    pub vendors: Vec<[u8; 3]>,
    /// Rates, Mbit/s.
    pub rates: Vec<f32>,
    /// Summary.
    pub summary: String,
}

#[derive(Default)]
struct El {
    ssid: Option<String>,
    rsn: Option<String>,
    interval: Option<u16>,
    op: Option<S1gOp>,
    long_preamble: Option<bool>,
    country: Option<String>,
    dtim: Option<u8>,
    raw: bool,
    twt: bool,
    pc: Option<u8>,
    vendors: Vec<[u8; 3]>,
    rates: Vec<f32>,
}

/// The security of an RSN element body, by its best key management suite (IEEE 802.11-2020 table 9-151).
fn rsn_security(d: &[u8]) -> String {
    let rank = |a: u8| match a {
        8 | 9 | 24 => 5,
        1 | 3 | 5 | 11 | 12 => 4,
        2 | 4 | 6 => 3,
        18 => 2,
        _ => 1,
    };
    let best = phy::Rsn::parse(d).map_or(0, |r| {
        r.akm_suites()
            .map(|a| if a.oui == IEEE_OUI { rank(a.kind) } else { 1 })
            .max()
            .unwrap_or(0)
    });
    match best {
        5 => "WPA3-SAE",
        4 => "WPA2-Enterprise",
        3 => "WPA2-PSK",
        2 => "OWE",
        _ => "RSN",
    }
    .into()
}

fn elements(b: &[u8]) -> El {
    let mut e = El::default();
    for x in phy::elements(b) {
        let (id, d) = (x.id, x.data);
        match id {
            0 => {
                if e.ssid.is_none() {
                    e.ssid = Some(if d.iter().all(|&c| c == 0) {
                        String::new()
                    } else {
                        printable(d, '.')
                    });
                }
            }
            1 | 50 => e.rates.extend(
                d.iter()
                    .filter(|&&r| r & 0x7F != 0x7F && r & 0x7F != 0)
                    .map(|&r| (r & 0x7F) as f32 / 2.0),
            ),
            5 if d.len() >= 3 => e.dtim = Some(d[1]),
            7 if d.len() >= 2 => e.country = Some(printable(&d[..2], '?')),
            32 => e.pc = d.first().copied(),
            48 if d.len() >= 8 => e.rsn = Some(rsn_security(d)),
            208 => e.raw = true,
            213 if d.len() >= 4 => e.interval = Some(u16::from_le_bytes([d[2], d[3]])),
            216 => e.twt = true,
            217 if !d.is_empty() => e.long_preamble = Some(d[0] & 1 != 0),
            221 if d.len() >= 3 => {
                let o = [d[0], d[1], d[2]];
                if !e.vendors.contains(&o) && e.vendors.len() < 8 {
                    e.vendors.push(o);
                }
            }
            232 if d.len() >= 4 => {
                let code = (d[0] >> 1) & 0x0F;
                let width = match code {
                    0 => 1,
                    1 | 2 => 2,
                    3..=6 => 4,
                    7..=14 => 8,
                    _ => 16,
                };
                e.op = Some(S1gOp {
                    width,
                    primary_width: if d[0] & 1 != 0 { 1 } else { 2 },
                    class: d[1],
                    primary: d[2],
                    channel: d[3],
                    basic_mcs: d.get(4).map(|v| v & 3),
                });
            }
            _ => {}
        }
    }
    e
}

fn ssid_part(s: &Option<String>) -> String {
    match s {
        Some(x) if x.is_empty() => " (hidden)".into(),
        Some(x) => format!(" {x}"),
        None => String::new(),
    }
}

/// Parses a PSDU (FCS included); `None` when the FCS fails or the frame says nothing.
pub fn parse(psdu: &[u8]) -> Option<S1gMac> {
    if !fcs_ok(psdu) {
        return None;
    }
    parse_body(&psdu[..psdu.len() - 4])
}

fn base(ftype: u8, subtype: u8) -> S1gMac {
    S1gMac {
        kind: FrameKind::Other,
        ftype,
        subtype,
        bssid: None,
        ssid: None,
        compressed_ssid: None,
        security: String::new(),
        interval_tu: None,
        timestamp: 0,
        bw_indication: None,
        security_bit: false,
        op: None,
        long_preamble: None,
        country: None,
        dtim_period: None,
        raw: false,
        twt: false,
        power_constraint: None,
        vendors: Vec::new(),
        rates: Vec::new(),
        summary: String::new(),
    }
}

fn fill(m: &mut S1gMac, e: El, privacy: bool) {
    m.security = match (&e.rsn, privacy) {
        (Some(s), _) => s.clone(),
        (None, true) => "Secured".into(),
        _ => "Open".into(),
    };
    m.ssid = e.ssid;
    if e.interval.is_some() {
        m.interval_tu = e.interval;
    }
    m.op = e.op;
    m.long_preamble = e.long_preamble;
    m.country = e.country;
    m.dtim_period = e.dtim;
    m.raw = e.raw;
    m.twt = e.twt;
    m.power_constraint = e.pc;
    m.vendors = e.vendors;
    m.rates = e.rates;
}

/// Parses a frame without its FCS.
pub fn parse_body(f: &[u8]) -> Option<S1gMac> {
    if f.len() < 2 || f[0] & 3 != 0 {
        return None;
    }
    let ftype = (f[0] >> 2) & 3;
    let subtype = f[0] >> 4;
    let flags = f[1];
    let mut m = base(ftype, subtype);
    let addr = |at: usize| f.get(at..at + 6).map(mac_text);
    match (ftype, subtype) {
        (3, 1) => {
            if f.len() < 15 {
                return None;
            }
            let mut at = 15usize;
            let mut take = |n: usize| -> Option<usize> {
                if at + n > f.len() {
                    None
                } else {
                    at += n;
                    Some(at - n)
                }
            };
            if flags & 1 != 0 {
                take(3)?;
            }
            let comp = if flags & 2 != 0 {
                let i = take(4)?;
                Some(u32::from_le_bytes([f[i], f[i + 1], f[i + 2], f[i + 3]]))
            } else {
                None
            };
            if flags & 4 != 0 {
                take(1)?;
            }
            m.bssid = addr(4);
            m.timestamp = u32::from_le_bytes([f[10], f[11], f[12], f[13]]) as u64;
            m.compressed_ssid = comp;
            m.bw_indication = Some((flags >> 3) & 7);
            m.security_bit = flags & 0x40 != 0;
            let rest = &f[at..];
            if rest.is_empty() {
                m.kind = FrameKind::ShortBeacon;
                m.security = if m.security_bit {
                    "Secured".into()
                } else {
                    "Open".into()
                };
                m.summary = "S1G short beacon".into();
            } else {
                m.kind = FrameKind::S1gBeacon;
                let e = elements(rest);
                let sb = m.security_bit;
                fill(&mut m, e, sb);
                m.summary = format!("S1G beacon{}", ssid_part(&m.ssid));
            }
        }
        (3, s) => {
            m.summary = format!("S1G extension {s}");
        }
        (0, 8) | (0, 5) => {
            if f.len() < 36 {
                return None;
            }
            m.bssid = addr(16);
            m.timestamp = u32::from_le_bytes([f[24], f[25], f[26], f[27]]) as u64;
            m.interval_tu = Some(u16::from_le_bytes([f[32], f[33]]));
            let cap = u16::from_le_bytes([f[34], f[35]]);
            m.security_bit = cap & 0x0010 != 0;
            let e = elements(&f[36..]);
            fill(&mut m, e, cap & 0x0010 != 0);
            if m.interval_tu.is_none() {
                m.interval_tu = Some(u16::from_le_bytes([f[32], f[33]]));
            }
            m.kind = if subtype == 8 {
                FrameKind::Beacon
            } else {
                FrameKind::ProbeResp
            };
            m.summary = format!(
                "{}{}",
                if subtype == 8 {
                    "beacon"
                } else {
                    "probe response"
                },
                ssid_part(&m.ssid)
            );
        }
        (0, s) => {
            m.bssid = addr(16);
            m.summary = match s {
                4 => {
                    let e = if f.len() > 24 {
                        elements(&f[24..])
                    } else {
                        El::default()
                    };
                    match e.ssid {
                        Some(x) if !x.is_empty() => format!("probe req {x}"),
                        _ => "probe req (any)".into(),
                    }
                }
                10 | 12 => {
                    let r = f
                        .get(24..26)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .unwrap_or(0);
                    format!("{}, reason {r}", MGMT_NAMES[s as usize])
                }
                _ => MGMT_NAMES[s as usize].into(),
            };
        }
        (1, s) => {
            m.summary = match s {
                7 => "control wrapper".into(),
                8 => "block ack req".into(),
                9 => "block ack".into(),
                10 => "PS-Poll".into(),
                11 => "RTS".into(),
                12 => "CTS".into(),
                13 => "ACK".into(),
                14 => "CF-End".into(),
                15 => "CF-End+Ack".into(),
                n => format!("control {n}"),
            };
            if matches!(s, 8..=11) {
                m.bssid = None;
            }
        }
        _ => {
            let to = flags & 1 != 0;
            let from = flags & 2 != 0;
            m.bssid = match (to, from) {
                (false, false) => addr(16),
                (true, false) => addr(4),
                (false, true) => addr(10),
                _ => None,
            };
            let mut s = String::from(if subtype & 8 != 0 { "QoS data" } else { "data" });
            if subtype & 4 != 0 {
                s.push_str(" null");
            }
            if flags & 0x40 != 0 {
                s.push_str(" (protected)");
            }
            m.summary = s;
        }
    }
    Some(m)
}

/// The vendor element text: `"AA:BB:CC[ <vendor>]"` joined with `", "`.
pub fn vendors_text(v: &[[u8; 3]]) -> String {
    v.iter()
        .map(|o| {
            let mut s = format!("{:02X}:{:02X}:{:02X}", o[0], o[1], o[2]);
            if let Some(name) = sdrkit_util_ids::oui::vendor_of_address(o) {
                let _ = write!(s, " {name}");
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A HaLow BSS to announce (generator and tests).
#[derive(Clone, Debug, PartialEq)]
pub struct HalowBss {
    /// BSSID.
    pub bssid: [u8; 6],
    /// SSID.
    pub ssid: String,
    /// Security: "Open", "WPA2-PSK", "WPA3-SAE", "OWE", "WPA2-Enterprise".
    pub security: &'static str,
    /// Operating width, MHz.
    pub width: u32,
    /// Primary width, MHz.
    pub primary_width: u32,
    /// Operating class.
    pub class: u8,
    /// Primary channel.
    pub primary: u8,
    /// Channel centre number.
    pub channel: u8,
    /// Beacon interval, TU.
    pub interval: u16,
    /// Send only the compressed SSID.
    pub compressed_only: bool,
    /// TIM, country, power constraint, a vendor element, RPS and TWT.
    pub extras: bool,
    /// A vendor element OUI.
    pub vendor: Option<[u8; 3]>,
    /// Timestamp.
    pub timestamp: u32,
}

impl HalowBss {
    /// A BSS with defaults (interval 100, primary 1 MHz, primary = channel).
    pub fn new(
        bssid: [u8; 6],
        ssid: &str,
        security: &'static str,
        width: u32,
        class: u8,
        channel: u8,
    ) -> HalowBss {
        HalowBss {
            bssid,
            ssid: ssid.into(),
            security,
            width,
            primary_width: if width == 1 { 1 } else { 2 },
            class,
            primary: channel,
            channel,
            interval: 100,
            compressed_only: false,
            extras: false,
            vendor: None,
            timestamp: 123_456,
        }
    }
}

fn el(out: &mut Vec<u8>, id: u8, d: &[u8]) {
    phy::push_element(out, id, d).expect("an element of at most 255 bytes");
}

fn width_code(w: u32) -> u8 {
    match w {
        1 => 0,
        2 => 1,
        4 => 3,
        8 => 7,
        _ => 15,
    }
}

fn bss_elements(b: &HalowBss, with_ssid: bool) -> Vec<u8> {
    let mut e = Vec::new();
    if with_ssid {
        el(&mut e, 0, b.ssid.as_bytes());
    }
    let mut compat = vec![0u8, 0];
    compat.extend(b.interval.to_le_bytes());
    compat.extend([0, 0, 0, 0]);
    el(&mut e, 213, &compat);
    el(
        &mut e,
        217,
        &[0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    el(
        &mut e,
        232,
        &[
            width_code(b.width) << 1 | (b.primary_width == 1) as u8,
            b.class,
            b.primary,
            b.channel,
            0xFE,
            0xFF,
        ],
    );
    let akm = match b.security {
        "WPA2-PSK" => Some(2u8),
        "WPA3-SAE" => Some(8),
        "OWE" => Some(18),
        "WPA2-Enterprise" => Some(1),
        _ => None,
    };
    if let Some(a) = akm {
        el(
            &mut e,
            48,
            &crate::wifi::frames::rsn_body(&[4], &[a], true, true),
        );
    }
    if b.extras {
        el(&mut e, 5, &[0, 3, 0, 0]);
        el(&mut e, 7, &[b'D', b'E', b' ', 1, 5, 14]);
        el(&mut e, 32, &[3]);
        el(&mut e, 208, &[0, 0, 0, 0, 0, 0]);
        el(&mut e, 216, &[0, 0, 0, 0]);
    }
    if let Some(o) = b.vendor {
        el(&mut e, 221, &[o[0], o[1], o[2], 1, 0]);
    }
    e
}

/// An S1G beacon (FCS appended).
pub fn s1g_beacon(b: &HalowBss) -> Vec<u8> {
    let secured = b.security != "Open";
    let flags: u8 = if b.compressed_only { 0x02 } else { 0 }
        | ((b.width.trailing_zeros() as u8 + 1) & 7) << 3
        | (secured as u8) << 6;
    let mut f = vec![0x1C, flags, 0, 0];
    f.extend(b.bssid);
    f.extend(b.timestamp.to_le_bytes());
    f.push(0);
    if b.compressed_only {
        f.extend(CRC32_ISO_HDLC.checksum(b.ssid.as_bytes()).to_le_bytes());
    }
    f.extend(bss_elements(b, !b.compressed_only));
    push_fcs(&mut f);
    f
}

/// A probe response with the full SSID (FCS appended).
pub fn probe_response(b: &HalowBss, to: [u8; 6]) -> Vec<u8> {
    let mut f = vec![0x50, 0, 0, 0];
    f.extend(to);
    f.extend(b.bssid);
    f.extend(b.bssid);
    f.extend([0, 0]);
    f.extend((b.timestamp as u64).to_le_bytes());
    f.extend(b.interval.to_le_bytes());
    let cap: u16 = 0x0001 | if b.security != "Open" { 0x0010 } else { 0 };
    f.extend(cap.to_le_bytes());
    f.extend(bss_elements(b, true));
    push_fcs(&mut f);
    f
}

/// A data frame from a station to its AP (FCS appended).
pub fn data_frame(bssid: [u8; 6], sta: [u8; 6], len: usize) -> Vec<u8> {
    let mut f = vec![0x08, 0x01, 0, 0];
    f.extend(bssid);
    f.extend(sta);
    f.extend(bssid);
    f.extend([0, 0]);
    f.extend((0..len.saturating_sub(28)).map(|i| (i * 37 % 251) as u8));
    push_fcs(&mut f);
    f
}

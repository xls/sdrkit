//! Wi-SUN FAN frames: PN9 whitening, the PHR, the FCS with least-reliable-bit correction, the channel plans, and the
//! header and payload information elements.
//! Standard: IEEE 802.15.4-2020 clause 19 (SUN FSK), Wi-SUN FAN 1.1 PHY and MAC specifications.

use crate::mac154::{self, eui, Addr};
use sdrkit_algo_fec::small::{chase_search, Chase};
use sdrkit_util_bits::crc::catalog::{CRC16_KERMIT, CRC32_ISO_HDLC};
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::{pack, seq, BitOrder, Pad};
use serde::{Deserialize, Serialize};

/// Whitens or de-whitens `data` in place with the SUN FSK PN9 sequence, least significant bit first in each octet. The sequence
/// is `sdrkit_util_bits::seq::pn9_802154g`. Real Wi-SUN frames pass their CRC-32 only with it (tests/vectors/ism/expected.json).
pub fn pn9(data: &mut [u8]) {
    Additive::new(seq::pn9_802154g()).apply_bytes(data, BitOrder::Lsb);
}

/// The PHR fields: (mode switch, FCS is 2 octets, whitened, PSDU length).
pub fn phr(v: u16) -> (bool, bool, bool, usize) {
    (
        v & 0x8000 != 0,
        v & 0x1000 != 0,
        v & 0x0800 != 0,
        (v & 0x7FF) as usize,
    )
}

/// Whether a de-whitened PSDU passes its FCS: CRC-16/KERMIT when `fcs2`, else CRC-32/ISO-HDLC, least significant octet first.
pub fn fcs_ok(p: &[u8], fcs2: bool) -> bool {
    if fcs2 {
        CRC16_KERMIT.verify(p, Trailer::Little)
    } else {
        CRC32_ISO_HDLC.verify(p, Trailer::Little)
    }
}

/// Appends the FCS (2 or 4 octets, least significant first).
pub fn add_fcs(p: &mut Vec<u8>, fcs2: bool) {
    let r = if fcs2 {
        CRC16_KERMIT.append(p, Trailer::Little)
    } else {
        CRC32_ISO_HDLC.append(p, Trailer::Little)
    };
    debug_assert!(r.is_ok(), "both FCS widths are whole octets");
}

/// The search of [`lrb_correct`]: every combination of the 14 least reliable bits.
pub const LRB_SEARCH: Chase = Chase {
    weakest: 14,
    max_flips: 14,
};

/// Corrects a frame with a CRC-32 FCS by the least-reliable-bits search of sdrkit-algo-fec. `p` holds the PSDU octets and `rel[i]` the
/// reliability of bit `i` as sent (least significant bit first in each octet). Returns the number of flipped bits (0 when the
/// FCS already passes), or `None` when no pattern passes. On `None`, `p` is unchanged.
pub fn lrb_correct(p: &mut [u8], rel: &[f32]) -> Option<u32> {
    if p.len() < 5 || rel.len() < 8 * p.len() {
        return None;
    }
    if fcs_ok(p, false) {
        return Some(0);
    }
    let mut bits = Vec::with_capacity(8 * p.len());
    pack::unpack(p, BitOrder::Lsb, &mut bits);
    let mut octets = Vec::with_capacity(p.len());
    let flips = chase_search(&mut bits, rel, LRB_SEARCH, |b| {
        octets.clear();
        pack::pack(b, BitOrder::Lsb, Pad::Zero, &mut octets);
        fcs_ok(&octets, false)
    })?;
    octets.clear();
    pack::pack(&bits, BitOrder::Lsb, Pad::Zero, &mut octets);
    p.copy_from_slice(&octets);
    Some(flips)
}

/// An operating class of a channel plan: channel k is centred at `first_hz + spacing_hz * k` for k below `count`.
#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    /// The centre of channel 0, Hz.
    pub first_hz: f64,
    /// The channel spacing, Hz.
    pub spacing_hz: f64,
    /// The number of channels.
    pub count: u32,
    /// The FSK modes: (bit rate in kbit/s, modulation index). Empty for a class that is not received.
    pub modes: Vec<(u32, f32)>,
}

/// A channel plan: the usable ranges and the operating classes in the order of their class numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// The usable ranges (low, high), Hz. Empty: no limit.
    pub ranges: Vec<(f64, f64)>,
    /// The operating classes; class number n is entry n - 1.
    pub classes: Vec<Class>,
}

/// The most entries of the `ranges` and `classes` settings.
pub const PLAN_ENTRIES: usize = 16;
/// The most channels of one class.
pub const CLASS_CHANNELS: u32 = 4096;

impl Plan {
    /// The plan of a capture centred on `centre_hz` when no plan is set: class 1 every 200 kHz at 50 kbit/s (index 1) and
    /// class 2 every 400 kHz at 100 and 150 kbit/s (index 0.5), on grids through the centre, 12.8 MHz to each side.
    pub fn around(centre_hz: f64) -> Plan {
        Plan {
            ranges: Vec::new(),
            classes: vec![
                Class {
                    first_hz: centre_hz - 12.8e6,
                    spacing_hz: 200e3,
                    count: 129,
                    modes: vec![(50, 1.0)],
                },
                Class {
                    first_hz: centre_hz - 12.8e6,
                    spacing_hz: 400e3,
                    count: 65,
                    modes: vec![(100, 0.5), (150, 0.5)],
                },
            ],
        }
    }

    /// The plan of the `ranges` and `classes` settings. `ranges`: entries separated by ";", each the lower and the upper
    /// edge in Hz separated by "-". `classes`: entries separated by ";", each the first channel in Hz, the spacing in Hz,
    /// the number of channels and the modes, separated by "/"; the modes are separated by spaces, each the bit rate in
    /// kbit/s and the modulation index separated by ":". `None` for a malformed text or more than [`PLAN_ENTRIES`]
    /// entries.
    pub fn parse(ranges: &str, classes: &str) -> Option<Plan> {
        Some(Plan {
            ranges: entries(ranges)?
                .into_iter()
                .map(parse_range)
                .collect::<Option<_>>()?,
            classes: entries(classes)?
                .into_iter()
                .map(parse_class)
                .collect::<Option<_>>()?,
        })
    }

    /// Whether `hz` lies in a usable range (every frequency when the plan has no range).
    pub fn usable(&self, hz: f64) -> bool {
        self.ranges.is_empty() || self.ranges.iter().any(|&(lo, hi)| hz >= lo && hz <= hi)
    }
}

/// The names of the regulatory domain values of the channel plan IE.
const DOMAINS: [(u8, &str); 17] = [
    (0x00, "Worldwide 2.4 GHz"),
    (0x01, "North America"),
    (0x02, "Japan"),
    (0x03, "Europe 863"),
    (0x04, "China 470"),
    (0x05, "India"),
    (0x06, "Mexico"),
    (0x07, "Brazil"),
    (0x08, "Australia / New Zealand"),
    (0x09, "Korea"),
    (0x0A, "Philippines"),
    (0x0B, "Malaysia"),
    (0x0C, "Hong Kong"),
    (0x0D, "Singapore (920 MHz)"),
    (0x0E, "Thailand"),
    (0x0F, "Vietnam"),
    (0x10, "Singapore (866 MHz)"),
];

/// The name of a regulatory domain value of the channel plan IE.
pub fn domain_name(value: u8) -> Option<&'static str> {
    DOMAINS.iter().find(|d| d.0 == value).map(|d| d.1)
}

/// The non-empty entries of a list separated by ";", or `None` for more than [`PLAN_ENTRIES`].
fn entries(text: &str) -> Option<Vec<&str>> {
    let v: Vec<&str> = text
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect();
    (v.len() <= PLAN_ENTRIES).then_some(v)
}

/// "low-high" in Hz.
fn parse_range(e: &str) -> Option<(f64, f64)> {
    let (lo, hi) = e.split_once('-')?;
    let (lo, hi): (f64, f64) = (lo.trim().parse().ok()?, hi.trim().parse().ok()?);
    (lo.is_finite() && hi.is_finite() && lo > 0.0 && hi >= lo).then_some((lo, hi))
}

/// "first/spacing/count/modes".
fn parse_class(e: &str) -> Option<Class> {
    let mut f = e.split('/');
    let first_hz: f64 = f.next()?.trim().parse().ok()?;
    let spacing_hz: f64 = f.next()?.trim().parse().ok()?;
    let count: u32 = f.next()?.trim().parse().ok()?;
    let modes = f
        .next()
        .unwrap_or("")
        .split_whitespace()
        .map(|m| {
            let (kbps, h) = m.split_once(':')?;
            let h: f32 = h.parse().ok()?;
            let kbps: u32 = kbps.parse().ok()?;
            (h.is_finite() && h > 0.0).then_some((kbps, h))
        })
        .collect::<Option<Vec<(u32, f32)>>>()?;
    let ok = f.next().is_none()
        && first_hz.is_finite()
        && first_hz > 0.0
        && spacing_hz.is_finite()
        && spacing_hz > 0.0
        && count <= CLASS_CHANNELS;
    ok.then_some(Class {
        first_hz,
        spacing_hz,
        count,
        modes,
    })
}

/// The usable channel centres of a plan within `half_span` Hz of `centre`: (Hz, modes as (bit rate, index)), de-duplicated.
pub fn channels(b: &Plan, centre: f64, half_span: f64) -> Vec<(f64, Vec<(u32, f32)>)> {
    let mut out: Vec<(f64, Vec<(u32, f32)>)> = Vec::new();
    for c in &b.classes {
        for k in 0..c.count {
            let hz = c.first_hz + c.spacing_hz * f64::from(k);
            if !b.usable(hz) || (hz - centre).abs() > half_span {
                continue;
            }
            let modes: Vec<(u32, f32)> = c.modes.iter().copied().filter(|m| m.0 <= 150).collect();
            if modes.is_empty() {
                continue;
            }
            match out.iter_mut().find(|c| (c.0 - hz).abs() < 1.0) {
                Some(c) => {
                    for m in modes {
                        if !c.1.contains(&m) {
                            c.1.push(m);
                        }
                    }
                }
                None => out.push((hz, modes)),
            }
        }
    }
    for c in out.iter_mut() {
        c.1.sort_by_key(|m| std::cmp::Reverse(m.0));
    }
    out
}

/// The channel number and operating class of a frequency in the plan of a bit rate: the nearest channel within half a spacing.
pub fn channel_number(b: &Plan, hz: f64, kbps: u32) -> Option<(usize, u32)> {
    for (ci, c) in b.classes.iter().enumerate() {
        if !c.modes.iter().any(|m| m.0 == kbps) {
            continue;
        }
        let k = ((hz - c.first_hz) / c.spacing_hz).round();
        if k >= 0.0
            && k < f64::from(c.count)
            && (hz - (c.first_hz + k * c.spacing_hz)).abs() <= c.spacing_hz / 2.0
        {
            return Some((ci + 1, k as u32));
        }
    }
    None
}

/// What the IEs say.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ies {
    /// The IE list ("UTT BT LBC HT1").
    pub list: Vec<String>,
    /// UTT frame type.
    pub utt: Option<u8>,
    /// UFSI.
    pub ufsi: Option<u32>,
    /// Broadcast slot number.
    pub bsn: Option<u16>,
    /// Broadcast interval offset (ms).
    pub bio: Option<u32>,
    /// RSL (dBm).
    pub rsl: Option<i16>,
    /// Authenticator EUI-64.
    pub ea: Option<u64>,
    /// LFN broadcast interval (ms).
    pub lbc: Option<u32>,
    /// PAN size, routing cost, flags.
    pub pan: Option<(u16, u16, u8)>,
    /// Network name.
    pub netname: Option<String>,
    /// PAN version.
    pub panver: Option<u16>,
    /// PHY modes.
    pub pom: Vec<u8>,
    /// Unicast schedule text.
    pub us: Option<String>,
    /// Broadcast schedule text.
    pub bs: Option<String>,
    /// EAPOL: "EAP request TLS", "EAPOL-Key" ...
    pub eap: Option<String>,
}

/// The UTT frame type name and its short name.
pub fn utt_name(t: u8) -> (&'static str, &'static str) {
    match t {
        0 => ("PAN Advert", "PA"),
        1 => ("PAN Advert Solicit", "PAS"),
        2 => ("PAN Config", "PC"),
        3 => ("PAN Config Solicit", "PCS"),
        4 => ("Data", "Data"),
        5 => ("Ack", "Ack"),
        6 => ("EAPOL", "EAPOL"),
        7 => ("LFN PAN Advert", "LPA"),
        8 => ("LFN PAN Advert Solicit", "LPAS"),
        9 => ("LFN PAN Config", "LPC"),
        10 => ("LFN PAN Config Solicit", "LPCS"),
        11 => ("Extended Data", "ExtData"),
        _ => ("UTT", "UTT"),
    }
}

fn le24(b: &[u8]) -> u32 {
    b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16
}

fn schedule_text(c: &[u8], bcast: bool) -> (String, usize) {
    let mut i = 0;
    let mut s = String::new();
    if bcast {
        if c.len() < 6 {
            return (s, c.len());
        }
        let bi = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        s.push_str(&format!("every {bi} ms, "));
        i = 6;
    }
    if c.len() < i + 4 {
        return (s, c.len());
    }
    let dwell = c[i];
    i += 3;
    let ctl = c[i];
    i += 1;
    let plan = ctl & 7;
    let func = (ctl >> 3) & 7;
    let excl = ctl >> 6;
    match plan {
        0 if c.len() >= i + 2 => {
            let dom = c[i];
            let class = c[i + 1];
            let name = domain_name(dom).unwrap_or("domain");
            s.push_str(&format!("{name} class {class}"));
            i += 2;
        }
        1 if c.len() >= i + 6 => {
            let ch0 = le24(&c[i..]) as f64 / 1000.0;
            let sp = [200, 400, 600, 100][(c[i + 3] & 3) as usize];
            let n = u16::from_le_bytes([c[i + 4], c[i + 5]]);
            s.push_str(&format!("{ch0:.3} MHz + {n} x {sp} kHz"));
            i += 6;
        }
        2 if c.len() >= i + 2 => {
            s.push_str(&format!("domain {} plan {}", c[i], c[i + 1]));
            i += 2;
        }
        _ => {}
    }
    match func {
        0 if c.len() >= i + 2 => {
            s.push_str(&format!(
                ", fixed ch {}",
                u16::from_le_bytes([c[i], c[i + 1]])
            ));
            i += 2;
        }
        1 => s.push_str(", TR51CF"),
        2 => s.push_str(", DH1CF"),
        3 => s.push_str(", vendor"),
        _ => {}
    }
    s.push_str(&format!(", dwell {dwell} ms"));
    if excl == 1 && i < c.len() {
        let n = c[i] as usize;
        let mut tot = 0u32;
        for r in 0..n {
            let a = i + 1 + 4 * r;
            if a + 4 <= c.len() {
                tot += u16::from_le_bytes([c[a + 2], c[a + 3]])
                    .saturating_sub(u16::from_le_bytes([c[a], c[a + 1]]))
                    as u32
                    + 1;
            }
        }
        s.push_str(&format!(", {tot} excluded"));
    }
    (s, c.len())
}

/// Parses the header IEs (and, when not secured, the payload IEs) after the MAC header. Returns the IEs and the number of octets
/// after them (the encrypted payload length when secured).
pub fn parse_ies(p: &[u8], mac: &mac154::Mac) -> (Ies, usize) {
    let mut ies = Ies::default();
    let mut i = mac.hdr_len;
    let mut payload_ies = false;
    if mac.ie_present {
        while i + 2 <= p.len() {
            let d = u16::from_le_bytes([p[i], p[i + 1]]);
            let len = (d & 0x7F) as usize;
            let id = ((d >> 7) & 0xFF) as u8;
            i += 2;
            let c = &p[i..(i + len).min(p.len())];
            i += len;
            match id {
                0x2A if !c.is_empty() => {
                    let sub = c[0];
                    let v = &c[1..];
                    let name = match sub {
                        0x01 if v.len() >= 4 => {
                            ies.utt = Some(v[0] & 0xF);
                            ies.ufsi = Some(le24(&v[1..]));
                            "UTT".to_string()
                        }
                        0x02 if v.len() >= 5 => {
                            ies.bsn = Some(u16::from_le_bytes([v[0], v[1]]));
                            ies.bio = Some(le24(&v[2..]));
                            "BT".into()
                        }
                        0x03 => "FC".into(),
                        0x04 if !v.is_empty() => {
                            ies.rsl = Some(v[0] as i16 - 174);
                            "RSL".into()
                        }
                        0x05 => "MHDS".into(),
                        0x06 => "VH".into(),
                        0x09 if v.len() >= 8 => {
                            ies.ea = Some(u64::from_be_bytes(
                                v[..8].try_into().expect("the range has the array's length"),
                            ));
                            "EA".into()
                        }
                        0x0A => "LUTT".into(),
                        0x0B => "LBT".into(),
                        0x0C => "NR".into(),
                        0x0D => "LUS".into(),
                        0x0E => "FLUS".into(),
                        0x0F => "LBS".into(),
                        0x10 => "LND".into(),
                        0x11 => "LTO".into(),
                        0x12 => "PANID".into(),
                        0x80 if v.len() >= 3 => {
                            ies.lbc = Some(le24(v));
                            "LBC".into()
                        }
                        s => format!("WH{s:02X}"),
                    };
                    ies.list.push(name);
                }
                0x7E => {
                    ies.list.push("HT1".into());
                    payload_ies = true;
                    break;
                }
                0x7F => {
                    ies.list.push("HT2".into());
                    break;
                }
                x => ies.list.push(format!("H{x:02X}")),
            }
        }
    }
    let rest = p.len().saturating_sub(i);
    if mac.encrypted() {
        return (ies, rest);
    }
    if payload_ies {
        while i + 2 <= p.len() {
            let d = u16::from_le_bytes([p[i], p[i + 1]]);
            let len = (d & 0x7FF) as usize;
            let gid = ((d >> 11) & 0xF) as u8;
            i += 2;
            let c = p[i..(i + len).min(p.len())].to_vec();
            i += len;
            match gid {
                0x4 => {
                    let mut j = 0;
                    while j + 2 <= c.len() {
                        let sd = u16::from_le_bytes([c[j], c[j + 1]]);
                        j += 2;
                        let (sid, sl, long) = if sd & 0x8000 != 0 {
                            (((sd >> 11) & 0xF) as u8, (sd & 0x7FF) as usize, true)
                        } else {
                            (((sd >> 8) & 0x7F) as u8, (sd & 0xFF) as usize, false)
                        };
                        let v = &c[j..(j + sl).min(c.len())];
                        j += sl;
                        let name = if long {
                            match sid {
                                1 => {
                                    ies.us = Some(schedule_text(v, false).0);
                                    "US".to_string()
                                }
                                2 => {
                                    ies.bs = Some(schedule_text(v, true).0);
                                    "BS".into()
                                }
                                3 => "VP".into(),
                                4 => "LCP".into(),
                                s => format!("WPL{s}"),
                            }
                        } else {
                            match sid {
                                0x04 if v.len() >= 5 => {
                                    ies.pan = Some((
                                        u16::from_le_bytes([v[0], v[1]]),
                                        u16::from_le_bytes([v[2], v[3]]),
                                        v[4],
                                    ));
                                    "PAN".into()
                                }
                                0x05 => {
                                    ies.netname = Some(String::from_utf8_lossy(v).into());
                                    "NETNAME".into()
                                }
                                0x06 if v.len() >= 2 => {
                                    ies.panver = Some(u16::from_le_bytes([v[0], v[1]]));
                                    "PANVER".into()
                                }
                                0x07 => "GTKHASH".into(),
                                0x08 if !v.is_empty() => {
                                    let n = (v[0] & 0xF) as usize;
                                    ies.pom = v[1..(1 + n).min(v.len())].to_vec();
                                    "POM".into()
                                }
                                0x40 => "LFNVER".into(),
                                0x41 => "LGTKHASH".into(),
                                s => format!("WP{s:02X}"),
                            }
                        };
                        ies.list.push(name);
                    }
                }
                0x3 => {
                    ies.list.push("MPX".into());
                    if c.len() >= 4
                        && c[0] & 7 <= 1
                        && u16::from_le_bytes([c[1], c[2]]) == 1
                        && c[3] == 1
                    {
                        let e = &c[4..];
                        ies.eap = match e.get(1) {
                            Some(0) => {
                                let code = e.get(4).copied().unwrap_or(0);
                                let cname = ["", "request", "response", "success", "failure"]
                                    .get(code as usize)
                                    .copied()
                                    .unwrap_or("");
                                let t = match e.get(8) {
                                    Some(1) => " identity",
                                    Some(13) => " TLS",
                                    _ => "",
                                };
                                Some(if code <= 2 {
                                    format!("EAP {cname}{t}")
                                } else {
                                    format!("EAP {cname}")
                                })
                            }
                            Some(3) => Some("EAPOL-Key".into()),
                            _ => Some("EAPOL".into()),
                        };
                    }
                }
                0xF => {
                    ies.list.push("PT".into());
                    break;
                }
                g => ies.list.push(format!("P{g}")),
            }
        }
    }
    (ies, p.len().saturating_sub(i))
}

/// The summary line of a frame.
pub fn summary(mac: &mac154::Mac, ies: &Ies, enc_len: usize) -> String {
    let mut s = match ies.utt {
        Some(t) => utt_name(t).0.to_string(),
        None => mac.type_name().to_string(),
    };
    if let Some(n) = &ies.netname {
        s.push_str(&format!(" \"{n}\""));
    }
    if let Some((size, cost, _)) = ies.pan {
        s.push_str(&format!(", PAN size {size}, routing cost {cost}"));
    }
    if let Some(v) = ies.panver {
        s.push_str(&format!(", PAN version {v}"));
    }
    if let Some(e) = &ies.eap {
        s.push_str(&format!(", {e}"));
    }
    if let Some(e) = ies.ea {
        s.push_str(&format!(", authenticator {}", eui(e)));
    }
    if let Some(r) = ies.rsl {
        s.push_str(&format!(", RSL {r} dBm"));
    }
    if mac.encrypted() {
        let _ = enc_len;
        s.push_str(&format!(
            ", encrypted (key {})",
            mac.aux.as_ref().and_then(|a| a.key_index).unwrap_or(0)
        ));
    }
    s
}

/// A header IE descriptor.
pub fn hie(id: u8, content: &[u8]) -> Vec<u8> {
    let d: u16 = (content.len() as u16 & 0x7F) | ((id as u16) << 7);
    let mut v = d.to_le_bytes().to_vec();
    v.extend_from_slice(content);
    v
}

/// A payload IE descriptor.
pub fn pie(gid: u8, content: &[u8]) -> Vec<u8> {
    let d: u16 = (content.len() as u16 & 0x7FF) | ((gid as u16) << 11) | 0x8000;
    let mut v = d.to_le_bytes().to_vec();
    v.extend_from_slice(content);
    v
}

/// A short WP sub-IE.
pub fn wp_short(sid: u8, v: &[u8]) -> Vec<u8> {
    let d: u16 = (v.len() as u16 & 0xFF) | ((sid as u16) << 8);
    let mut o = d.to_le_bytes().to_vec();
    o.extend_from_slice(v);
    o
}

/// A long WP sub-IE.
pub fn wp_long(sid: u8, v: &[u8]) -> Vec<u8> {
    let d: u16 = (v.len() as u16 & 0x7FF) | ((sid as u16) << 11) | 0x8000;
    let mut o = d.to_le_bytes().to_vec();
    o.extend_from_slice(v);
    o
}

/// The source EUI of a frame, if extended.
pub fn src_eui(m: &mac154::Mac) -> Option<u64> {
    match m.src {
        Addr::Ext(e) => Some(e),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrb_fixes_three_weak_bits() {
        let mut p: Vec<u8> = (0..40u8).collect();
        add_fcs(&mut p, false);
        let good = p.clone();
        let mut rel = vec![1.0f32; p.len() * 8];
        for &b in &[17usize, 100, 250] {
            p[b / 8] ^= 1 << (b % 8);
            rel[b] = 0.01;
        }
        for (k, &b) in [3usize, 60, 70, 90, 140, 200, 300, 310, 320, 330, 5]
            .iter()
            .enumerate()
        {
            rel[b] = 0.02 + k as f32 * 0.001;
        }
        assert_eq!(lrb_correct(&mut p, &rel), Some(3));
        assert_eq!(p, good);
    }
}

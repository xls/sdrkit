//! What the advertising data of a packet says: the common AD structures, the names of companies and services, the beacon
//! formats (iBeacon, Eddystone, Exposure Notification) and the one-line summary of a packet.
//! Standard: Core Specification Supplement v11 Part A; Bluetooth Core Specification v5.4 Vol 6 Part B clause 2.3.

use crate::report::hex;
use sdrkit_proto_wlan_phy::ble::ad::{self, AdValue};
use sdrkit_proto_wlan_phy::ble::{AdvPdu, Body, PduType};
use sdrkit_util_ids::bluetooth;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// What the advertising data of a packet says.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AdvInfo {
    /// Flags (the first byte).
    pub flags: Option<u8>,
    /// Service UUIDs as shown ("Battery (0x180F)", "0x12345678", a 128-bit UUID).
    pub services: Vec<String>,
    /// The local name.
    pub name: Option<String>,
    /// Whether the name is the complete one.
    pub name_complete: bool,
    /// TX power level, dBm.
    pub tx_power: Option<i8>,
    /// Appearance.
    pub appearance: Option<u16>,
    /// Company identifier of the manufacturer data.
    pub company: Option<u16>,
    /// Manufacturer data after the company identifier.
    pub mfr: Vec<u8>,
    /// The vendor (the company name, or the type of Apple's data).
    pub vendor: Option<String>,
    /// Beacon texts joined with "; ".
    pub beacon: Option<String>,
}

/// A 16-bit UUID as shown: `"<name> (0xXXXX)"`, or `"0xXXXX"` when the UUID has no assigned name.
pub fn uuid16_text(u: u16) -> String {
    match bluetooth::uuid16(u) {
        Some(n) => format!("{} (0x{u:04X})", n.name),
        None => format!("0x{u:04X}"),
    }
}

/// A 16-byte UUID, most significant byte first, in the 8-4-4-4-12 upper-case form.
pub fn uuid128_text(b: &[u8]) -> String {
    let h = hex(b);
    if h.len() < 32 {
        return h;
    }
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// The name of an Apple manufacturer data type (the first byte after the company identifier 0x004C).
pub fn apple_type(t: u8) -> Option<&'static str> {
    Some(match t {
        0x02 => "iBeacon",
        0x05 => "AirDrop",
        0x07 => "AirPods / Beats",
        0x09 => "AirPlay target",
        0x0C => "Handoff",
        0x0F => "Nearby Action",
        0x10 => "Nearby Info",
        0x12 => "Find My",
        _ => return None,
    })
}

/// Expands an Eddystone-URL: the scheme byte and the encoded bytes.
pub fn eddystone_url(scheme: u8, rest: &[u8]) -> String {
    let mut s = String::from(match scheme {
        0 => "http://www.",
        1 => "https://www.",
        2 => "http://",
        3 => "https://",
        _ => "?",
    });
    const EXP: [&str; 14] = [
        ".com/", ".org/", ".edu/", ".net/", ".info/", ".biz/", ".gov/", ".com", ".org", ".edu",
        ".net", ".info", ".biz", ".gov",
    ];
    for &b in rest {
        match b {
            0..=13 => s.push_str(EXP[b as usize]),
            0x21..=0x7E => s.push(b as char),
            _ => s.push('?'),
        }
    }
    s
}

fn service_beacon(uuid: u16, d: &[u8]) -> Option<String> {
    match uuid {
        0xFEAA => {
            let t = *d.first()?;
            Some(match t {
                0x00 if d.len() >= 18 => format!(
                    "Eddystone UID {} / {}, {} dBm at 0 m",
                    hex(&d[2..12]),
                    hex(&d[12..18]),
                    d[1] as i8
                ),
                0x10 if d.len() >= 3 => format!(
                    "Eddystone URL {}, {} dBm at 0 m",
                    eddystone_url(d[2], &d[3..]),
                    d[1] as i8
                ),
                0x20 if d.len() >= 14 && d[1] == 0 => {
                    let mv = u16::from_be_bytes([d[2], d[3]]);
                    let temp = i16::from_be_bytes([d[4], d[5]]) as f64 / 256.0;
                    let adv = u32::from_be_bytes([d[6], d[7], d[8], d[9]]);
                    let up =
                        (u32::from_be_bytes([d[10], d[11], d[12], d[13]]) as f64 / 10.0).round();
                    format!("Eddystone TLM {mv} mV, {temp:.1} C, {adv} adverts, up {up} s")
                }
                0x30 => "Eddystone EID".into(),
                _ => "Eddystone".into(),
            })
        }
        0xFD6F if d.len() >= 16 => Some(format!("Exposure Notification, RPI {}", hex(&d[..16]))),
        _ => None,
    }
}

fn add_service(a: &mut AdvInfo, s: String) {
    if !a.services.contains(&s) {
        a.services.push(s);
    }
}

fn manufacturer(a: &mut AdvInfo, company: u16, m: &[u8], beacons: &mut Vec<String>) {
    a.company = Some(company);
    a.mfr = m.to_vec();
    a.vendor = Some(match bluetooth::company(company) {
        Some(n) => n.to_string(),
        None => format!("company 0x{company:04X}"),
    });
    if company != 0x004C || m.is_empty() {
        return;
    }
    if m[0] == 0x02 && m.len() >= 23 && m[1] == 0x15 {
        let uuid = uuid128_text(&m[2..18]);
        let major = u16::from_be_bytes([m[18], m[19]]);
        let minor = u16::from_be_bytes([m[20], m[21]]);
        beacons.push(format!(
            "iBeacon {uuid} major {major} minor {minor}, {} dBm at 1 m",
            m[22] as i8
        ));
        a.vendor = Some("Apple: iBeacon".into());
    } else if let Some(n) = apple_type(m[0]) {
        a.vendor = Some(format!("Apple: {n}"));
    }
}

/// Reads advertising data (the AD structures of AdvData or ScanRspData). Structures this stack does not read are skipped.
pub fn parse_ad(data: &[u8]) -> AdvInfo {
    let mut a = AdvInfo::default();
    let mut beacons: Vec<String> = Vec::new();
    for s in ad::structures(data) {
        match s.value() {
            AdValue::Flags(f) => a.flags = Some(f),
            AdValue::Uuids { list, .. } => match list.width {
                2 => {
                    for u in list.values() {
                        add_service(&mut a, uuid16_text(u as u16));
                    }
                }
                4 => {
                    for u in list.values() {
                        add_service(&mut a, format!("0x{u:08X}"));
                    }
                }
                _ => {
                    for c in list.bytes.as_chunks::<16>().0 {
                        let mut r = [0u8; 16];
                        for (o, b) in r.iter_mut().zip(c.iter().rev()) {
                            *o = *b;
                        }
                        add_service(&mut a, uuid128_text(&r));
                    }
                }
            },
            AdValue::Name { complete, bytes } => {
                let n: String = String::from_utf8_lossy(bytes)
                    .chars()
                    .map(|c| if c.is_control() { '?' } else { c })
                    .collect();
                if complete {
                    a.name = Some(n);
                    a.name_complete = true;
                } else if a.name.is_none() {
                    a.name = Some(n);
                }
            }
            // 127 means "no power level given" (Supplement Part A clause 1.5 refers to Core Vol 4 Part E 7.8.6).
            AdValue::TxPower(p) if p != 127 => a.tx_power = Some(p),
            AdValue::Appearance(v) => a.appearance = Some(v),
            AdValue::ServiceData16 { uuid, data } => {
                add_service(&mut a, uuid16_text(uuid));
                if let Some(b) = service_beacon(uuid, data) {
                    beacons.push(b);
                }
            }
            AdValue::Manufacturer { company, data } => {
                manufacturer(&mut a, company, data, &mut beacons)
            }
            _ => {}
        }
    }
    if !beacons.is_empty() {
        a.beacon = Some(beacons.join("; "));
    }
    a
}

/// The address a packet belongs to in the device table and the log: ScanA for SCAN_REQ, InitA for CONNECT_IND, else AdvA.
pub fn sender(p: &AdvPdu<'_>) -> Option<sdrkit_proto_wlan_phy::ble::Addr> {
    match p.body {
        Body::ScanReq { scan_a, .. } => Some(scan_a),
        Body::Connect { init_a, .. } => Some(init_a),
        _ => p.adv_a(),
    }
}

/// The one-line summary of a packet (the Details column of the log).
pub fn summary(p: &AdvPdu<'_>, a: &AdvInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    match p.body {
        Body::ScanReq { scan_a, .. } => parts.push(format!("scan request from {}", scan_a.text())),
        Body::Connect { init_a, .. } => parts.push(format!(
            "connection request from {} (not followed)",
            init_a.text()
        )),
        Body::Direct { target_a, .. } => parts.push(format!("directed to {}", target_a.text())),
        Body::Extended { ext, .. } if p.header.pdu_type == PduType::AdvExtInd => {
            let mut s = String::from("extended");
            if let Some((ch, phy)) = ext.aux_ptr {
                let phy = match phy {
                    0 => "1M",
                    1 => "2M",
                    2 => "Coded",
                    _ => "?",
                };
                let _ = write!(s, ", data on channel {ch} ({phy} PHY, not followed)");
            }
            if let Some((_, sid)) = ext.adi {
                let _ = write!(s, ", set {sid}");
            }
            parts.push(s);
        }
        _ => {}
    }
    if let Some(n) = &a.name {
        parts.push(format!("\"{n}\""));
    }
    if let Some(v) = &a.vendor {
        parts.push(v.clone());
    }
    if let Some(b) = &a.beacon {
        parts.push(b.clone());
    }
    if let Some(t) = a.tx_power {
        parts.push(format!("tx {t} dBm"));
    }
    for s in a.services.iter().take(3) {
        parts.push(s.clone());
    }
    parts.join(", ")
}

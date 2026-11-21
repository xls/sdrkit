//! FANET frames: the header with its extension, and the payloads of each type.
//! Document: "FANET protocol", published with the open FANET firmware.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// The LoRa sync word of FANET.
pub const SYNC: u8 = 0xF1;
/// The spreading factor of every FANET plan.
pub const SF: u8 = 7;
/// Latitude units in one degree.
pub const LAT_SCALE: f64 = 93206.0;
/// Longitude units in one degree.
pub const LON_SCALE: f64 = 46603.0;

/// A station address: manufacturer and device id.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Address {
    /// The manufacturer id.
    pub manufacturer: u8,
    /// The device id.
    pub device: u16,
}

impl std::fmt::Display for Address {
    /// "MM:DDDD", upper-case hex.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02X}:{:04X}", self.manufacturer, self.device)
    }
}

impl Address {
    /// The name of the manufacturer, for the ids that the protocol document lists.
    pub fn manufacturer_name(&self) -> Option<&'static str> {
        Some(match self.manufacturer {
            0x01 => "Skytraxx",
            0x03 => "BitBroker.eu",
            0x06 => "Burnair",
            0x07 => "SoftRF",
            0x0A => "FLARM",
            0x11 => "FANET+",
            0x20 => "XC Tracer",
            0xE0 => "OGN tracker",
            0xFA => "various",
            0xFB => "Espressif base station",
            0xFC | 0xFD => "unregistered",
            _ => return None,
        })
    }
}

/// Why a payload is not a FANET frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FanetError {
    /// Fewer than the four header bytes.
    #[error("shorter than the header")]
    ShortHeader,
    /// The extension byte, the destination or the signature is cut off.
    #[error("the extended header is cut off")]
    ShortExtension,
    /// A tracking, ground tracking or service payload is shorter than its fixed part.
    #[error("the payload is too short")]
    ShortPayload,
}

/// The extended header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    /// 0 none, 1 requested, 2 requested through a forwarder.
    pub ack: u8,
    /// The destination of a unicast frame.
    pub destination: Option<Address>,
    /// The four signature bytes, little endian. The signature is not checked: the key is not known.
    pub signature: Option<u32>,
    /// Forwarded by position.
    pub geo_forward: bool,
}

/// The aircraft types of a tracking frame.
pub const AIRCRAFT_TYPES: [&str; 8] = [
    "other",
    "paraglider",
    "hang glider",
    "balloon",
    "glider",
    "powered aircraft",
    "helicopter",
    "UAV",
];

/// The name of ground state `s` (0 to 15).
pub fn ground_state(s: u8) -> &'static str {
    match s {
        0 => "other",
        1 => "walking",
        2 => "vehicle",
        3 => "bike",
        4 => "boot",
        8 => "need a ride",
        9 => "landed well",
        12 => "need technical support",
        13 => "need medical help",
        14 => "distress call",
        15 => "distress call automatically",
        _ => "not defined",
    }
}

/// A tracking payload (type 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tracking {
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Online tracking.
    pub online: bool,
    /// Aircraft type, 0 to 7.
    pub aircraft: u8,
    /// Altitude, metres.
    pub alt_m: f64,
    /// Speed, km/h.
    pub speed_kmh: f64,
    /// Climb, m/s.
    pub climb_m_s: f64,
    /// Heading, degrees.
    pub heading_deg: f64,
    /// Turn rate, degrees per second, clockwise positive.
    pub turn_deg_s: Option<f64>,
    /// QNE offset, metres.
    pub qne_offset_m: Option<f64>,
}

/// A service payload (type 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Service {
    /// Internet gateway.
    pub gateway: bool,
    /// Remote configuration supported.
    pub remote_config: bool,
    /// Latitude and longitude, degrees.
    pub position: Option<(f64, f64)>,
    /// Temperature, degrees Celsius.
    pub temperature_c: Option<f64>,
    /// Wind: direction in degrees, speed and gusts in km/h.
    pub wind: Option<(f64, f64, f64)>,
    /// Relative humidity, percent.
    pub humidity_pct: Option<f64>,
    /// Barometric pressure, hPa.
    pub pressure_hpa: Option<f64>,
    /// State of charge, percent.
    pub battery_pct: Option<f64>,
}

/// A ground tracking payload (type 7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// The state, 0 to 15.
    pub state: u8,
    /// Online tracking.
    pub online: bool,
}

impl Ground {
    /// The state requests help (12 to 15).
    pub fn distress(&self) -> bool {
        self.state >= 12
    }
}

/// A thermal payload (type 9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Thermal {
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Confidence, percent.
    pub confidence_pct: f64,
    /// Altitude, metres.
    pub alt_m: f64,
    /// Mean climb of the air, m/s.
    pub climb_m_s: f64,
    /// Mean wind speed, km/h.
    pub wind_kmh: f64,
    /// The direction that the wind comes from, degrees.
    pub wind_deg: f64,
}

/// The payload of a frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Payload {
    /// Type 0.
    Ack,
    /// Type 1.
    Tracking(Tracking),
    /// Type 2.
    Name(String),
    /// Type 3: the subheader and the text.
    Message(u8, String),
    /// Type 4.
    Service(Service),
    /// Type 7.
    Ground(Ground),
    /// Type 9.
    Thermal(Thermal),
    /// Types 8 and 10, as text.
    Hardware(String),
    /// Any other type: its number and the payload length.
    Other(u8, usize),
}

/// A parsed frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// The type, 0 to 63.
    pub kind: u8,
    /// Other stations may repeat the frame.
    pub forward: bool,
    /// The sender.
    pub source: Address,
    /// The extended header.
    pub extension: Option<Extension>,
    /// The payload.
    pub payload: Payload,
}

fn le16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

/// A 24-bit signed little-endian number.
fn s24(b: &[u8]) -> i32 {
    (i32::from(b[0]) | i32::from(b[1]) << 8 | i32::from(b[2]) << 16) << 8 >> 8
}

/// The position of six bytes: latitude then longitude in the absolute form.
fn position(b: &[u8]) -> (f64, f64) {
    (
        f64::from(s24(&b[..3])) / LAT_SCALE,
        f64::from(s24(&b[3..6])) / LON_SCALE,
    )
}

/// An unsigned 7-bit value with the scale bit: bit 7 multiplies by `factor`.
fn scaled(b: u8, factor: f64) -> f64 {
    f64::from(b & 0x7F) * if b & 0x80 != 0 { factor } else { 1.0 }
}

/// A signed 7-bit value (two's complement) with the scale bit.
fn scaled_signed(b: u8, factor: f64) -> f64 {
    f64::from((b << 1) as i8 >> 1) * if b & 0x80 != 0 { factor } else { 1.0 }
}

/// The altitude of the 12 low bits of a word: bit 11 multiplies the 11-bit value by 4.
fn altitude(w: u16) -> f64 {
    f64::from(w & 0x7FF) * if w & 0x800 != 0 { 4.0 } else { 1.0 }
}

/// Text of 8-bit characters up to an end zero: bytes outside 32 to 126 become ".".
fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    b[..end]
        .iter()
        .map(|&c| {
            if (32..127).contains(&c) {
                c as char
            } else {
                '.'
            }
        })
        .collect()
}

fn tracking(p: &[u8]) -> Result<Tracking, FanetError> {
    if p.len() < 11 {
        return Err(FanetError::ShortPayload);
    }
    let (lat, lon) = position(p);
    let w = le16(&p[6..]);
    Ok(Tracking {
        lat,
        lon,
        online: w & 0x8000 != 0,
        aircraft: ((w >> 12) & 7) as u8,
        alt_m: altitude(w),
        speed_kmh: scaled(p[8], 5.0) * 0.5,
        climb_m_s: scaled_signed(p[9], 5.0) * 0.1,
        heading_deg: f64::from(p[10]) * 360.0 / 256.0,
        turn_deg_s: p.get(11).map(|&b| scaled_signed(b, 4.0) * 0.25),
        qne_offset_m: p.get(12).map(|&b| scaled_signed(b, 4.0)),
    })
}

fn service(p: &[u8]) -> Result<Service, FanetError> {
    let Some(&flags) = p.first() else {
        return Ok(Service::default());
    };
    let mut s = Service {
        gateway: flags & 0x80 != 0,
        remote_config: flags & 0x04 != 0,
        ..Service::default()
    };
    let mut i = 1 + usize::from(flags & 0x01);
    let rest = p.get(i..).unwrap_or(&[]);
    if rest.len() < 6 {
        // A data field needs the position in front of it.
        return if flags & 0x7A != 0 {
            Err(FanetError::ShortPayload)
        } else {
            Ok(s)
        };
    }
    s.position = Some(position(rest));
    i += 6;
    let mut take = |n: usize| -> Option<&[u8]> {
        let f = p.get(i..i + n)?;
        i += n;
        Some(f)
    };
    if flags & 0x40 != 0 {
        s.temperature_c = take(1).map(|f| f64::from(f[0] as i8) * 0.5);
    }
    if flags & 0x20 != 0 {
        s.wind = take(3).map(|f| {
            (
                f64::from(f[0]) * 360.0 / 256.0,
                scaled(f[1], 5.0) * 0.2,
                scaled(f[2], 5.0) * 0.2,
            )
        });
    }
    if flags & 0x10 != 0 {
        s.humidity_pct = take(1).map(|f| f64::from(f[0]) * 0.4);
    }
    if flags & 0x08 != 0 {
        s.pressure_hpa = take(2).map(|f| f64::from(le16(f)) * 0.1 + 430.0);
    }
    if flags & 0x02 != 0 {
        s.battery_pct = take(1).map(|f| f64::from(f[0] & 0x0F) * 100.0 / 15.0);
    }
    Ok(s)
}

fn ground(p: &[u8]) -> Result<Ground, FanetError> {
    if p.len() < 7 {
        return Err(FanetError::ShortPayload);
    }
    let (lat, lon) = position(p);
    Ok(Ground {
        lat,
        lon,
        state: p[6] >> 4,
        online: p[6] & 1 != 0,
    })
}

fn thermal(p: &[u8]) -> Option<Thermal> {
    if p.len() < 11 {
        return None;
    }
    let (lat, lon) = position(p);
    let w = le16(&p[6..]);
    Some(Thermal {
        lat,
        lon,
        confidence_pct: f64::from((w >> 12) & 7) * 100.0 / 7.0,
        alt_m: altitude(w),
        climb_m_s: scaled_signed(p[8], 5.0) * 0.1,
        wind_kmh: scaled(p[9], 5.0) * 0.5,
        wind_deg: f64::from(p[10]) * 360.0 / 256.0,
    })
}

/// Hardware information of the old form (type 8) as text.
fn hardware_old(p: &[u8]) -> String {
    let mut s = String::from("hardware");
    if let Some(&t) = p.first() {
        let _ = write!(s, ", device type 0x{t:02X}");
    }
    if p.len() >= 3 {
        let w = le16(&p[1..]);
        let _ = write!(
            s,
            ", firmware {}-{:02}-{:02}{}",
            2019 + ((w >> 9) & 0x3F),
            (w >> 5) & 0xF,
            w & 0x1F,
            if w & 0x8000 != 0 {
                " (development)"
            } else {
                ""
            }
        );
    }
    if p.len() >= 5 {
        let _ = write!(s, ", uptime {} s", u32::from(le16(&p[3..]) >> 4) * 30);
    }
    s
}

/// Hardware information of the new form (type 10) as text.
fn hardware_new(p: &[u8]) -> String {
    let mut s = String::from("hardware");
    let Some(&flags) = p.first() else {
        return s;
    };
    if flags & 0x80 != 0 {
        s.push_str(", ping request");
    }
    let mut i = 1 + usize::from(flags & 0x01);
    if flags & 0x40 != 0 {
        if let Some(f) = p.get(i..i + 4) {
            let w = le16(&f[2..]);
            let _ = write!(
                s,
                ", subtype 0x{:04X}, firmware {}-{:02}-{:02}",
                le16(f),
                2019 + ((w >> 9) & 0x3F),
                (w >> 5) & 0xF,
                w & 0x1F
            );
        }
        i += 4;
    }
    if flags & 0x20 != 0 {
        if let Some(f) = p.get(i..i + 3) {
            let _ = write!(s, ", ICAO {:02X}{:02X}{:02X}", f[2], f[1], f[0]);
        }
        i += 3;
    }
    if flags & 0x10 != 0 {
        if let Some(f) = p.get(i..i + 2) {
            let _ = write!(s, ", uptime {} min", le16(f));
        }
    }
    s
}

/// Parses the payload of a LoRa frame as a FANET frame.
///
/// # Errors
///
/// [`FanetError`] for a frame shorter than its header, a cut-off extension, or a tracking, ground tracking or service payload
/// that is too short.
pub fn parse(b: &[u8]) -> Result<Frame, FanetError> {
    if b.len() < 4 {
        return Err(FanetError::ShortHeader);
    }
    let kind = b[0] & 0x3F;
    let source = Address {
        manufacturer: b[1],
        device: le16(&b[2..]),
    };
    let mut i = 4;
    let extension = if b[0] & 0x80 != 0 {
        let e = *b.get(i).ok_or(FanetError::ShortExtension)?;
        i += 1;
        let mut x = Extension {
            ack: e >> 6,
            geo_forward: e & 0x08 != 0,
            ..Extension::default()
        };
        if e & 0x20 != 0 {
            let d = b.get(i..i + 3).ok_or(FanetError::ShortExtension)?;
            x.destination = Some(Address {
                manufacturer: d[0],
                device: le16(&d[1..]),
            });
            i += 3;
        }
        if e & 0x10 != 0 {
            let s = b.get(i..i + 4).ok_or(FanetError::ShortExtension)?;
            x.signature = Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
            i += 4;
        }
        Some(x)
    } else {
        None
    };
    let p = &b[i..];
    let payload = match kind {
        0 => Payload::Ack,
        1 => Payload::Tracking(tracking(p)?),
        2 => Payload::Name(text(p)),
        3 => Payload::Message(
            p.first().copied().unwrap_or(0),
            text(p.get(1..).unwrap_or(&[])),
        ),
        4 => Payload::Service(service(p)?),
        7 => Payload::Ground(ground(p)?),
        8 => Payload::Hardware(hardware_old(p)),
        9 => thermal(p).map_or(Payload::Other(9, p.len()), Payload::Thermal),
        10 => Payload::Hardware(hardware_new(p)),
        n => Payload::Other(n, p.len()),
    };
    Ok(Frame {
        kind,
        forward: b[0] & 0x40 != 0,
        source,
        extension,
        payload,
    })
}

impl Frame {
    /// The position of the frame, latitude and longitude in degrees.
    pub fn position(&self) -> Option<(f64, f64)> {
        match &self.payload {
            Payload::Tracking(t) => Some((t.lat, t.lon)),
            Payload::Ground(g) => Some((g.lat, g.lon)),
            Payload::Thermal(t) => Some((t.lat, t.lon)),
            Payload::Service(s) => s.position,
            _ => None,
        }
    }

    /// The frame in one line.
    pub fn line(&self) -> String {
        let mut s = match &self.payload {
            Payload::Ack => "acknowledge".to_string(),
            Payload::Tracking(t) => format!(
                "{}, {:.0} m, {:.1} km/h, climb {:.1} m/s, heading {:.0}",
                AIRCRAFT_TYPES[usize::from(t.aircraft & 7)],
                t.alt_m,
                t.speed_kmh,
                t.climb_m_s,
                t.heading_deg
            ),
            Payload::Name(n) => format!("name: {n}"),
            Payload::Message(_, m) => format!("message: {m}"),
            Payload::Service(v) => {
                let mut s = String::from("service");
                if v.gateway {
                    s.push_str(", internet gateway");
                }
                if let Some(t) = v.temperature_c {
                    let _ = write!(s, ", {t:.1} C");
                }
                if let Some((d, v, g)) = v.wind {
                    let _ = write!(s, ", wind {d:.0} deg {v:.1} km/h gust {g:.1}");
                }
                if let Some(h) = v.humidity_pct {
                    let _ = write!(s, ", humidity {h:.0}%");
                }
                if let Some(p) = v.pressure_hpa {
                    let _ = write!(s, ", {p:.1} hPa");
                }
                if let Some(b) = v.battery_pct {
                    let _ = write!(s, ", battery {b:.0} %");
                }
                s
            }
            Payload::Ground(g) => format!("on the ground: {}", ground_state(g.state)),
            Payload::Thermal(t) => format!(
                "thermal, {:.0} m, climb {:.1} m/s, wind {:.0} deg {:.1} km/h, confidence {:.0} %",
                t.alt_m, t.climb_m_s, t.wind_deg, t.wind_kmh, t.confidence_pct
            ),
            Payload::Hardware(h) => h.clone(),
            Payload::Other(5, n) => format!("landmarks, {n} bytes"),
            Payload::Other(6, n) => format!("remote configuration, {n} bytes"),
            Payload::Other(9, _) => "thermal".to_string(),
            Payload::Other(t, n) => format!("type {t}, {n} bytes"),
        };
        if self.extension.is_some_and(|e| e.signature.is_some()) {
            s.push_str(", signed");
        }
        s
    }
}

/// The three bytes of a coordinate in the absolute form.
fn put24(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes()[..3]);
}

/// The six position bytes of `lat` and `lon` in degrees.
pub fn put_position(out: &mut Vec<u8>, lat: f64, lon: f64) {
    put24(out, (lat * LAT_SCALE).round() as i32);
    put24(out, (lon * LON_SCALE).round() as i32);
}

/// A 7-bit value with its scale bit for `v` units: the plain form when it fits, else the scaled form, held to its range.
fn put_scaled(v: f64, factor: f64, signed: bool) -> u8 {
    let (lo, hi) = if signed { (-64.0, 63.0) } else { (0.0, 127.0) };
    let r = v.round();
    if (lo..=hi).contains(&r) {
        (r as i8 as u8) & 0x7F
    } else {
        0x80 | ((v / factor).round().clamp(lo, hi) as i8 as u8) & 0x7F
    }
}

/// The four header bytes of a broadcast frame of type `kind` from `source`.
pub fn header(kind: u8, source: Address) -> Vec<u8> {
    let d = source.device.to_le_bytes();
    vec![kind & 0x3F, source.manufacturer, d[0], d[1]]
}

/// A tracking frame (type 1) with the 11 fixed payload bytes.
pub fn build_tracking(source: Address, t: &Tracking) -> Vec<u8> {
    let mut b = header(1, source);
    put_position(&mut b, t.lat, t.lon);
    let alt = t.alt_m.round().clamp(0.0, 8188.0) as u16;
    let alt = if alt > 2047 { 0x800 | (alt / 4) } else { alt };
    let w = u16::from(t.online) << 15 | u16::from(t.aircraft & 7) << 12 | alt;
    b.extend_from_slice(&w.to_le_bytes());
    b.push(put_scaled(t.speed_kmh * 2.0, 5.0, false));
    b.push(put_scaled(t.climb_m_s * 10.0, 5.0, true));
    b.push((t.heading_deg.rem_euclid(360.0) * 256.0 / 360.0).round() as u16 as u8);
    b
}

/// A name frame (type 2).
pub fn build_name(source: Address, name: &str) -> Vec<u8> {
    let mut b = header(2, source);
    b.extend_from_slice(name.as_bytes());
    b
}

/// A message frame (type 3) with the subheader 0, to `to` when it is given.
pub fn build_message(source: Address, to: Option<Address>, text: &str) -> Vec<u8> {
    let mut b = header(3, source);
    if let Some(d) = to {
        b[0] |= 0x80;
        b.push(0x20);
        b.push(d.manufacturer);
        b.extend_from_slice(&d.device.to_le_bytes());
    }
    b.push(0);
    b.extend_from_slice(text.as_bytes());
    b
}

/// A service frame (type 4) with the position and the fields of `s` that have a value.
pub fn build_service(source: Address, s: &Service) -> Vec<u8> {
    let mut b = header(4, source);
    let flags = u8::from(s.gateway) << 7
        | u8::from(s.temperature_c.is_some()) << 6
        | u8::from(s.wind.is_some()) << 5
        | u8::from(s.humidity_pct.is_some()) << 4
        | u8::from(s.pressure_hpa.is_some()) << 3
        | u8::from(s.remote_config) << 2
        | u8::from(s.battery_pct.is_some()) << 1;
    b.push(flags);
    let Some((lat, lon)) = s.position else {
        return b;
    };
    put_position(&mut b, lat, lon);
    if let Some(t) = s.temperature_c {
        b.push((t * 2.0).round().clamp(-128.0, 127.0) as i8 as u8);
    }
    if let Some((d, v, g)) = s.wind {
        b.push((d.rem_euclid(360.0) * 256.0 / 360.0).round() as u16 as u8);
        b.push(put_scaled(v * 5.0, 5.0, false));
        b.push(put_scaled(g * 5.0, 5.0, false));
    }
    if let Some(h) = s.humidity_pct {
        b.push((h / 0.4).round().clamp(0.0, 250.0) as u8);
    }
    if let Some(p) = s.pressure_hpa {
        let v = ((p - 430.0) * 10.0).round().clamp(0.0, 65535.0) as u16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    if let Some(c) = s.battery_pct {
        b.push((c * 15.0 / 100.0).round().clamp(0.0, 15.0) as u8);
    }
    b
}

/// A ground tracking frame (type 7).
pub fn build_ground(source: Address, g: &Ground) -> Vec<u8> {
    let mut b = header(7, source);
    put_position(&mut b, g.lat, g.lon);
    b.push(g.state << 4 | u8::from(g.online));
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    #[test]
    fn a_tracking_frame_gives_its_fields() {
        let f = parse(&hex("01 11 33 22 15 8E 43 25 22 FA B0 94 28 0C 40")).unwrap();
        assert_eq!(f.source.to_string(), "11:2233");
        let Payload::Tracking(t) = f.payload else {
            panic!("a tracking frame");
        };
        assert!((t.lat - 47.5).abs() < 2e-5 && (t.lon + 8.25).abs() < 3e-5);
        assert!(t.online);
        assert_eq!(AIRCRAFT_TYPES[usize::from(t.aircraft)], "paraglider");
        assert_eq!(t.alt_m, 1200.0);
        assert_eq!(t.speed_kmh, 20.0);
        assert!((t.climb_m_s - 1.2).abs() < 1e-9);
        assert_eq!(t.heading_deg, 90.0);
    }

    #[test]
    fn a_name_a_message_and_a_service_frame_give_their_fields() {
        let f = parse(&hex("02 01 02 00 42 6F 62")).unwrap();
        assert_eq!(f.source.to_string(), "01:0002");
        assert_eq!(f.payload, Payload::Name("Bob".into()));
        let f = parse(&hex("83 01 02 00 20 09 08 07 00 68 69")).unwrap();
        let x = f.extension.unwrap();
        assert_eq!(x.destination.unwrap().to_string(), "09:0708");
        assert_eq!(f.payload, Payload::Message(0, "hi".into()));
        let f = parse(&hex("04 01 02 00 E0 10 20 30 10 20 30 28 40 0A 14")).unwrap();
        let Payload::Service(s) = &f.payload else {
            panic!("a service frame");
        };
        assert!(s.gateway);
        let (lat, lon) = s.position.unwrap();
        assert!((lat - 33.8383).abs() < 1e-4 && (lon - 67.6767).abs() < 1e-4);
        assert_eq!(s.temperature_c, Some(20.0));
        let (d, v, g) = s.wind.unwrap();
        assert_eq!(d, 90.0);
        assert!((v - 2.0).abs() < 1e-9 && (g - 4.0).abs() < 1e-9);
        assert_eq!(
            f.line(),
            "service, internet gateway, 20.0 C, wind 90 deg 2.0 km/h gust 4.0"
        );
    }

    #[test]
    fn short_frames_are_refused() {
        assert_eq!(parse(&hex("02 01")), Err(FanetError::ShortHeader));
        assert_eq!(
            parse(&hex("83 01 02 00 20 09")),
            Err(FanetError::ShortExtension)
        );
        assert_eq!(
            parse(&hex("01 11 33 22 15 8E 43")),
            Err(FanetError::ShortPayload)
        );
        assert_eq!(
            parse(&hex("07 11 33 22 15 8E 43 25 22 FA")),
            Err(FanetError::ShortPayload)
        );
        // A temperature without the position in front of it.
        assert_eq!(
            parse(&hex("04 01 02 00 40 28")),
            Err(FanetError::ShortPayload)
        );
        // Only the gateway flag: the position may be left out.
        assert!(parse(&hex("04 01 02 00 80")).is_ok());
    }

    #[test]
    fn the_altitude_word_has_a_scale_bit() {
        let mut b = hex("01 11 33 22 15 8E 43 25 22 FA 8A CA 28 0C 40");
        let Payload::Tracking(t) = parse(&b).unwrap().payload else {
            panic!("a tracking frame");
        };
        assert!(t.online);
        assert_eq!(AIRCRAFT_TYPES[usize::from(t.aircraft)], "glider");
        assert_eq!(t.alt_m, 2600.0);
        // A negative climb with the scale: -20 units of 0.5 m/s.
        b[13] = 0x80 | 0x6C;
        let Payload::Tracking(t) = parse(&b).unwrap().payload else {
            panic!("a tracking frame");
        };
        assert!((t.climb_m_s + 10.0).abs() < 1e-9);
    }

    #[test]
    fn built_frames_parse_to_what_was_built() {
        let a = Address {
            manufacturer: 0x11,
            device: 0x2233,
        };
        let t = Tracking {
            lat: 47.5,
            lon: -8.25,
            online: true,
            aircraft: 1,
            alt_m: 1200.0,
            speed_kmh: 20.0,
            climb_m_s: 1.2,
            heading_deg: 90.0,
            turn_deg_s: None,
            qne_offset_m: None,
        };
        assert_eq!(
            build_tracking(a, &t),
            hex("01 11 33 22 15 8E 43 25 22 FA B0 94 28 0C 40")
        );
        let fast = Tracking {
            alt_m: 3000.0,
            speed_kmh: 150.0,
            climb_m_s: -9.0,
            ..t
        };
        let Payload::Tracking(r) = parse(&build_tracking(a, &fast)).unwrap().payload else {
            panic!("a tracking frame");
        };
        assert_eq!((r.alt_m, r.speed_kmh), (3000.0, 150.0));
        assert!((r.climb_m_s + 9.0).abs() < 1e-9);
        let g = Ground {
            lat: 10.0,
            lon: 20.0,
            state: 14,
            online: true,
        };
        let Payload::Ground(r) = parse(&build_ground(a, &g)).unwrap().payload else {
            panic!("a ground frame");
        };
        assert!(r.distress() && r.online && r.state == 14);
        let s = Service {
            gateway: true,
            position: Some((33.8383, 67.6767)),
            temperature_c: Some(20.0),
            wind: Some((90.0, 2.0, 4.0)),
            humidity_pct: Some(40.0),
            pressure_hpa: Some(1013.2),
            battery_pct: Some(100.0),
            remote_config: false,
        };
        let Payload::Service(r) = parse(&build_service(a, &s)).unwrap().payload else {
            panic!("a service frame");
        };
        assert_eq!(r.humidity_pct, Some(40.0));
        assert!((r.pressure_hpa.unwrap() - 1013.2).abs() < 0.05);
        assert_eq!(r.battery_pct, Some(100.0));
    }
}

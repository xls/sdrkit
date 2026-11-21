//! The AIS message layer (ITU-R M.1371-5 Annex 8, field offsets as in the public gpsd AIVDM document): every message type decoded
//! into one typed record, the acceptance rule, and the names shown for codes.

use super::binary;
use sdrkit_util_bits::pack::bits_to_hex;
use sdrkit_util_bits::reader::HardBits;
use sdrkit_util_geo::nmea::{self, aivdm};
use sdrkit_util_ids::mmsi::Mmsi;
use sdrkit_util_text::sixbit::{ais_char, ais_text_into};
use serde::{Deserialize, Serialize};

/// The class of station a message comes from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationClass {
    A,
    B,
    BaseStation,
    AidToNavigation,
    SarAircraft,
    #[default]
    Other,
}

impl StationClass {
    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            StationClass::A => "A",
            StationClass::B => "B",
            StationClass::BaseStation => "base station",
            StationClass::AidToNavigation => "aid to navigation",
            StationClass::SarAircraft => "SAR aircraft",
            StationClass::Other => "other",
        }
    }
    /// The bit of the filter's class set.
    pub fn bit(self) -> u32 {
        match self {
            StationClass::A => 1,
            StationClass::B => 2,
            StationClass::BaseStation => 4,
            StationClass::AidToNavigation => 8,
            StationClass::SarAircraft => 16,
            StationClass::Other => 32,
        }
    }
    /// The class of a message type, or from the MMSI for the other types.
    pub fn of(msg_type: u8, mmsi: u32) -> StationClass {
        match msg_type {
            1 | 2 | 3 | 5 | 27 => StationClass::A,
            18 | 19 | 24 => StationClass::B,
            4 => StationClass::BaseStation,
            21 => StationClass::AidToNavigation,
            9 => StationClass::SarAircraft,
            _ => match mmsi {
                111_000_000..=111_999_999 => StationClass::SarAircraft,
                990_000_000..=999_999_999 => StationClass::AidToNavigation,
                0..=9_999_999 => StationClass::BaseStation,
                _ => StationClass::Other,
            },
        }
    }
}

/// A decoded AIS message: the header, and every field the type carries (absent fields are `None`, "not available" codes become
/// `None` too).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AisMessage {
    #[serde(rename = "type")]
    pub msg_type: u8,
    pub repeat: u8,
    pub mmsi: u32,
    pub bits: usize,
    pub class: StationClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nav_status: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rot_deg_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sog_kn: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cog_deg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maneuver: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raim: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radio: Option<u32>,
    /// Base station / UTC response time, ISO 8601.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epfd: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ais_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imo: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callsign: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ship_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dim: Option<[u16; 4]>,
    /// ETA: month, day, hour, minute (hour 24 and minute 60 mean not available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<[u8; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draught_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dte: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude_m: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest_mmsi: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retransmit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dac: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fi: Option<u8>,
    /// Binary application data as hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_hex: Option<String>,
    /// Decoded application fields (IMO international function messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aid_type: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub off_position: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_aid: Option<bool>,
    /// Message 24: 0 for part A, 1 for part B.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mothership: Option<u32>,
    /// Message 27: GNSS position latency (false under 5 s).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<bool>,
    /// Class B flags (message 18): unit, display, DSC, band, message 22.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_b_flags: Option<[bool; 5]>,
    /// What the link-management types carry (acknowledgements, interrogations, assignments, reservations, channel and group
    /// management, the flags of binary messages 25 and 26).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub body: Option<Body>,
}

/// A station and a slot sequence number: one acknowledgement of message 7 or 13.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ack {
    /// The station acknowledged.
    pub mmsi: u32,
    /// The sequence number of the acknowledged message.
    pub seq: u8,
}

/// One request of an interrogation (message 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interrogation {
    /// The interrogated station.
    pub mmsi: u32,
    /// The requested message type.
    pub msg_type: u8,
    /// The slot offset of the reply.
    pub offset: u16,
}

/// One assignment of an assigned mode command (message 16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    /// The station assigned.
    pub mmsi: u32,
    /// The slot offset.
    pub offset: u16,
    /// The slot increment.
    pub increment: u16,
}

/// One reservation of a data link management message (message 20).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    /// The first slot reserved.
    pub offset: u16,
    /// The number of slots.
    pub number: u8,
    /// The time-out, minutes.
    pub timeout: u8,
    /// The slot increment.
    pub increment: u16,
}

/// A rectangle of positions in degrees: its north-east and south-west corners.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Area {
    /// The north-east corner, (latitude, longitude).
    pub ne: (f64, f64),
    /// The south-west corner, (latitude, longitude).
    pub sw: (f64, f64),
}

/// Where a channel management message (message 22) applies.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Applies {
    /// To two stations.
    Stations(u32, u32),
    /// To an area.
    Area(Area),
}

/// What the link-management types carry beside the header (ITU-R M.1371-5 Annex 8 clause 3.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Body {
    /// Message 7 or 13: the acknowledgements.
    Acks(Vec<Ack>),
    /// Message 15: the requests.
    Interrogations(Vec<Interrogation>),
    /// Message 16: the assignments.
    Assignments(Vec<Assignment>),
    /// Message 20: the reservations.
    Reservations(Vec<Reservation>),
    /// Message 22: channel management.
    ChannelManagement {
        /// The channel number of channel A.
        channel_a: u16,
        /// The channel number of channel B.
        channel_b: u16,
        /// The transmit and receive mode.
        txrx: u8,
        /// High power.
        power_high: bool,
        /// The stations or the area it applies to.
        applies: Applies,
        /// Channel A has a 12.5 kHz bandwidth.
        band_a: bool,
        /// Channel B has a 12.5 kHz bandwidth.
        band_b: bool,
        /// The size of the transition zone, nautical miles.
        zone_size: u8,
    },
    /// Message 23: group assignment.
    GroupAssignment {
        /// The area it applies to.
        area: Area,
        /// The station type it applies to.
        station_type: u8,
        /// The ship type it applies to.
        ship_type: u8,
        /// The transmit and receive mode.
        txrx: u8,
        /// The reporting interval code.
        interval: u8,
        /// The quiet time, minutes.
        quiet: u8,
    },
    /// Message 25 or 26: the flags of a binary message.
    Binary {
        /// Sent to one station.
        addressed: bool,
        /// Carries an application identifier.
        structured: bool,
    },
}

impl AisMessage {
    /// Whether it carries a valid position.
    pub fn position(&self) -> Option<(f64, f64)> {
        match (self.lat, self.lon) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }
}

/// Why a frame is not taken as AIS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    Short,
    Type,
    Mmsi,
    Length,
}

fn pos(lon_raw: i64, lat_raw: i64, scale: f64) -> (Option<f64>, Option<f64>) {
    let lon = lon_raw as f64 / scale;
    let lat = lat_raw as f64 / scale;
    if lon.abs() <= 180.0 && lat.abs() <= 90.0 {
        (Some(lat), Some(lon))
    } else {
        (None, None)
    }
}

fn sog10(raw: u64) -> Option<f64> {
    (raw != 1023).then_some(raw as f64 / 10.0)
}
fn cog10(raw: u64) -> Option<f64> {
    (raw < 3600).then_some(raw as f64 / 10.0)
}
fn heading(raw: u64) -> Option<u16> {
    (raw < 360).then_some(raw as u16)
}
fn second(raw: u64) -> Option<u8> {
    Some(raw as u8)
}

/// Rate of turn from its raw value: -128 not available, +-127 more than 720 deg/min (shown +-720), else sign x (raw / 4.733)^2.
pub fn rot(raw: i64) -> Option<f64> {
    match raw {
        -128 => None,
        127 => Some(720.0),
        -127 => Some(-720.0),
        r => {
            let v = (r as f64 / 4.733).powi(2);
            Some(if r < 0 { -v } else { v })
        }
    }
}

fn len_ok(t: u8, n: usize) -> bool {
    match t {
        1..=4 | 9 | 11 | 18 => n == 168,
        5 => (420..=424).contains(&n),
        6 => n >= 72,
        7 | 13 => (72..=168).contains(&n),
        8 => n >= 56,
        10 => n >= 72,
        12 => (72..=1008).contains(&n),
        14 => n >= 40,
        15 => (88..=160).contains(&n),
        16 => n == 96 || n == 144,
        17 => (80..=816).contains(&n),
        19 => n == 312,
        20 => (72..=160).contains(&n),
        21 => (272..=360).contains(&n),
        22 => n >= 168,
        23 => n >= 160,
        24 => (160..=168).contains(&n),
        25 => (40..=168).contains(&n),
        26 => (60..=1064).contains(&n),
        27 => n == 96 || n == 168,
        _ => false,
    }
}

/// The types whose length is checked for acceptance (the fixed layouts).
pub fn fixed_layout(t: u8) -> bool {
    matches!(t, 1..=6 | 8 | 9 | 11 | 14 | 18 | 19 | 21 | 24 | 27)
}

/// Decodes a frame's payload bits. A frame is accepted as AIS when its type is 1 to 27, its MMSI plausible, and (for the fixed
/// layouts) its length right.
pub fn decode(bits: &[u8]) -> Result<AisMessage, Reject> {
    if bits.len() < 38 {
        return Err(Reject::Short);
    }
    let r = Fields::new(bits);
    let t = r.u(0, 6) as u8;
    let mmsi = r.u32(8, 30);
    if !(1..=27).contains(&t) {
        return Err(Reject::Type);
    }
    if !Mmsi::new(mmsi).is_some_and(Mmsi::is_plausible_ais) {
        return Err(Reject::Mmsi);
    }
    let n = bits.len();
    if fixed_layout(t) && !len_ok(t, n) {
        return Err(Reject::Length);
    }
    let mut m = AisMessage {
        msg_type: t,
        repeat: r.u(6, 2) as u8,
        mmsi,
        bits: n,
        class: StationClass::of(t, mmsi),
        ..Default::default()
    };
    decode_body(&r, &mut m);
    Ok(m)
}

/// Decodes every field whatever the MMSI and length (for listeners of frames the acceptance rule refuses).
pub fn decode_unchecked(bits: &[u8]) -> Option<AisMessage> {
    if bits.len() < 38 {
        return None;
    }
    let r = Fields::new(bits);
    let t = r.u(0, 6) as u8;
    let mmsi = r.u32(8, 30);
    let mut m = AisMessage {
        msg_type: t,
        repeat: r.u(6, 2) as u8,
        mmsi,
        bits: bits.len(),
        class: StationClass::of(t, mmsi),
        ..Default::default()
    };
    decode_body(&r, &mut m);
    Some(m)
}

/// Decodes the header only (for frames the acceptance rule refuses, so other listeners can still see what they hold).
pub fn header(bits: &[u8]) -> Option<(u8, u32)> {
    (bits.len() >= 38).then(|| {
        let r = Fields::new(bits);
        (r.u(0, 6) as u8, r.u32(8, 30))
    })
}

fn dims(r: &Fields, at: usize) -> Option<[u16; 4]> {
    Some([
        r.u(at, 9) as u16,
        r.u(at + 9, 9) as u16,
        r.u(at + 18, 6) as u16,
        r.u(at + 24, 6) as u16,
    ])
}

fn area(r: &Fields, ne: usize, sw: usize) -> Area {
    Area {
        ne: (r.i(ne + 18, 17) as f64 / 600.0, r.i(ne, 18) as f64 / 600.0),
        sw: (r.i(sw + 18, 17) as f64 / 600.0, r.i(sw, 18) as f64 / 600.0),
    }
}

fn decode_body(r: &Fields, m: &mut AisMessage) {
    let n = r.len();
    match m.msg_type {
        1..=3 => {
            m.nav_status = Some(r.u(38, 4) as u8);
            m.rot_deg_min = rot(r.i(42, 8));
            m.sog_kn = sog10(r.u(50, 10));
            m.accuracy = Some(r.b(60));
            (m.lat, m.lon) = pos(r.i(61, 28), r.i(89, 27), 600_000.0);
            m.cog_deg = cog10(r.u(116, 12));
            m.heading = heading(r.u(128, 9));
            m.second = second(r.u(137, 6));
            m.maneuver = Some(r.u(143, 2) as u8);
            m.raim = Some(r.b(148));
            m.radio = Some(r.u32(149, 19));
        }
        4 | 11 => {
            let (y, mo, d, h, mi, s) = (
                r.u(38, 14),
                r.u(52, 4),
                r.u(56, 5),
                r.u(61, 5),
                r.u(66, 6),
                r.u(72, 6),
            );
            if y > 0 && mo > 0 && d > 0 && h < 24 && mi < 60 && s < 60 {
                m.utc = Some(format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z"));
            }
            m.accuracy = Some(r.b(78));
            (m.lat, m.lon) = pos(r.i(79, 28), r.i(107, 27), 600_000.0);
            m.epfd = Some(r.u(134, 4) as u8);
            m.raim = Some(r.b(148));
            m.radio = Some(r.u32(149, 19));
        }
        5 => {
            m.ais_version = Some(r.u(38, 2) as u8);
            m.imo = Some(r.u32(40, 30));
            m.callsign = Some(r.text(70, 7));
            m.name = Some(r.text(112, 20));
            m.ship_type = Some(r.u(232, 8) as u8);
            m.dim = dims(r, 240);
            m.epfd = Some(r.u(270, 4) as u8);
            m.eta = Some([
                r.u(274, 4) as u8,
                r.u(278, 5) as u8,
                r.u(283, 5) as u8,
                r.u(288, 6) as u8,
            ]);
            m.draught_m = Some(r.u(294, 8) as f64 / 10.0);
            m.destination = Some(r.text(302, 20));
            if n > 422 {
                m.dte = Some(r.b(422));
            }
        }
        6 => {
            m.seq = Some(r.u(38, 2) as u8);
            m.dest_mmsi = Some(r.u32(40, 30));
            m.retransmit = Some(r.b(70));
            if n >= 88 {
                let (dac, fi) = (r.u(72, 10) as u16, r.u(82, 6) as u8);
                m.dac = Some(dac);
                m.fi = Some(fi);
                m.data_hex = Some(r.hex_from(88));
                m.app = binary::decode(dac, fi, true, &bits_from(r, 88));
            } else {
                m.data_hex = Some(r.hex_from(72));
            }
        }
        7 | 13 => {
            let mut acks = Vec::new();
            let mut at = 40;
            while at + 32 <= n {
                acks.push(Ack {
                    mmsi: r.u32(at, 30),
                    seq: r.u(at + 30, 2) as u8,
                });
                at += 32;
            }
            m.body = Some(Body::Acks(acks));
        }
        8 => {
            let (dac, fi) = (r.u(40, 10) as u16, r.u(50, 6) as u8);
            m.dac = Some(dac);
            m.fi = Some(fi);
            m.data_hex = Some(r.hex_from(56));
            m.app = binary::decode(dac, fi, false, &bits_from(r, 56));
        }
        9 => {
            let alt = r.u(38, 12);
            m.altitude_m = (alt != 4095).then_some(alt as u16);
            let sog = r.u(50, 10);
            m.sog_kn = (sog != 1023).then_some(sog as f64);
            m.accuracy = Some(r.b(60));
            (m.lat, m.lon) = pos(r.i(61, 28), r.i(89, 27), 600_000.0);
            m.cog_deg = cog10(r.u(116, 12));
            m.second = second(r.u(128, 6));
            m.dte = Some(r.b(142));
            m.assigned = Some(r.b(146));
            m.raim = Some(r.b(147));
            m.radio = Some(r.u32(148, 20));
        }
        10 => {
            m.dest_mmsi = Some(r.u32(40, 30));
        }
        12 => {
            m.seq = Some(r.u(38, 2) as u8);
            m.dest_mmsi = Some(r.u32(40, 30));
            m.retransmit = Some(r.b(70));
            m.text = Some(r.text_to_end(72));
        }
        14 => {
            m.text = Some(r.text_to_end(40));
        }
        15 => {
            let ask = |mmsi: usize, at: usize| Interrogation {
                mmsi: r.u32(mmsi, 30),
                msg_type: r.u(at, 6) as u8,
                offset: r.u(at + 6, 12) as u16,
            };
            let mut q = vec![ask(40, 70)];
            if n >= 110 {
                q.push(ask(40, 90));
            }
            if n >= 160 {
                q.push(ask(110, 140));
            }
            m.body = Some(Body::Interrogations(q));
        }
        16 => {
            let assign = |at: usize| Assignment {
                mmsi: r.u32(at, 30),
                offset: r.u(at + 30, 12) as u16,
                increment: r.u(at + 42, 10) as u16,
            };
            let mut a = vec![assign(40)];
            if n >= 144 {
                a.push(assign(92));
            }
            m.body = Some(Body::Assignments(a));
        }
        17 => {
            (m.lat, m.lon) = pos(r.i(40, 18), r.i(58, 17), 600.0);
            m.data_hex = Some(r.hex_from(80));
        }
        18 => {
            m.sog_kn = sog10(r.u(46, 10));
            m.accuracy = Some(r.b(56));
            (m.lat, m.lon) = pos(r.i(57, 28), r.i(85, 27), 600_000.0);
            m.cog_deg = cog10(r.u(112, 12));
            m.heading = heading(r.u(124, 9));
            m.second = second(r.u(133, 6));
            m.class_b_flags = Some([r.b(141), r.b(142), r.b(143), r.b(144), r.b(145)]);
            m.assigned = Some(r.b(146));
            m.raim = Some(r.b(147));
            m.radio = Some(r.u32(148, 20));
        }
        19 => {
            m.sog_kn = sog10(r.u(46, 10));
            m.accuracy = Some(r.b(56));
            (m.lat, m.lon) = pos(r.i(57, 28), r.i(85, 27), 600_000.0);
            m.cog_deg = cog10(r.u(112, 12));
            m.heading = heading(r.u(124, 9));
            m.second = second(r.u(133, 6));
            m.name = Some(r.text(143, 20));
            m.ship_type = Some(r.u(263, 8) as u8);
            m.dim = dims(r, 271);
            m.epfd = Some(r.u(301, 4) as u8);
            m.raim = Some(r.b(305));
            m.dte = Some(r.b(306));
            m.assigned = Some(r.b(307));
        }
        20 => {
            let mut res = Vec::new();
            let mut at = 40;
            while at + 30 <= n {
                let (off, num) = (r.u(at, 12), r.u(at + 12, 4));
                if off > 0 || num > 0 {
                    res.push(Reservation {
                        offset: off as u16,
                        number: num as u8,
                        timeout: r.u(at + 16, 3) as u8,
                        increment: r.u(at + 19, 11) as u16,
                    });
                }
                at += 30;
            }
            m.body = Some(Body::Reservations(res));
        }
        21 => {
            m.aid_type = Some(r.u(38, 5) as u8);
            let mut name = r.text_raw(43, 20);
            if n > 272 {
                name.push_str(&r.text_raw(272, (n - 272) / 6));
            }
            let name = name.trim_end_matches(['@', ' ']).to_string();
            m.name = Some(name);
            m.accuracy = Some(r.b(163));
            (m.lat, m.lon) = pos(r.i(164, 28), r.i(192, 27), 600_000.0);
            m.dim = dims(r, 219);
            m.epfd = Some(r.u(249, 4) as u8);
            let sec = r.u(253, 6);
            m.second = second(sec);
            m.off_position = Some(r.b(259));
            m.raim = Some(r.b(268));
            m.virtual_aid = Some(r.b(269));
            m.assigned = Some(r.b(270));
        }
        22 => {
            let applies = if r.b(139) {
                Applies::Stations(r.u32(69, 30), r.u32(104, 30))
            } else {
                Applies::Area(area(r, 69, 104))
            };
            m.body = Some(Body::ChannelManagement {
                channel_a: r.u(40, 12) as u16,
                channel_b: r.u(52, 12) as u16,
                txrx: r.u(64, 4) as u8,
                power_high: r.b(68),
                applies,
                band_a: r.b(140),
                band_b: r.b(141),
                zone_size: r.u(142, 3) as u8,
            });
        }
        23 => {
            m.body = Some(Body::GroupAssignment {
                area: area(r, 40, 75),
                station_type: r.u(110, 4) as u8,
                ship_type: r.u(114, 8) as u8,
                txrx: r.u(144, 2) as u8,
                interval: r.u(146, 4) as u8,
                quiet: r.u(150, 4) as u8,
            });
        }
        24 => {
            let part = r.u(38, 2) as u8;
            m.part = Some(part);
            if part == 0 {
                m.name = Some(r.text(40, 20));
            } else {
                m.ship_type = Some(r.u(40, 8) as u8);
                m.vendor = Some(r.text(48, 3));
                m.model = Some(r.u(66, 4) as u8);
                m.serial = Some(r.u32(70, 20));
                m.callsign = Some(r.text(90, 7));
                if (980_000_000..990_000_000).contains(&m.mmsi) {
                    m.mothership = Some(r.u32(132, 30));
                } else {
                    m.dim = dims(r, 132);
                }
            }
        }
        25 | 26 => {
            let addressed = r.b(38);
            let structured = r.b(39);
            let mut at = 40;
            m.body = Some(Body::Binary {
                addressed,
                structured,
            });
            if addressed {
                m.dest_mmsi = Some(r.u32(at, 30));
                at += 30;
            }
            if structured && n >= at + 16 {
                m.dac = Some(r.u(at, 10) as u16);
                m.fi = Some(r.u(at + 10, 6) as u8);
                at += 16;
            }
            let end = if m.msg_type == 26 && n >= at + 20 {
                m.radio = Some(r.u32(n - 20, 20));
                n - 20
            } else {
                n
            };
            let data = bits_from(r, at);
            let data = &data[..end.saturating_sub(at).min(data.len())];
            m.data_hex = Some(hex(data));
            if let (Some(d), Some(f)) = (m.dac, m.fi) {
                m.app = binary::decode(d, f, addressed, data);
            }
        }
        27 => {
            m.accuracy = Some(r.b(38));
            m.raim = Some(r.b(39));
            m.nav_status = Some(r.u(40, 4) as u8);
            let (lon, lat) = (r.i(44, 18), r.i(62, 17));
            if lon != 181 * 600 && lat != 91 * 600 {
                (m.lat, m.lon) = pos(lon, lat, 600.0);
            }
            let sog = r.u(79, 6);
            m.sog_kn = (sog != 63).then_some(sog as f64);
            let cog = r.u(85, 9);
            m.cog_deg = (cog != 511 && cog < 360).then_some(cog as f64);
            m.latency = Some(r.b(94));
        }
        _ => {}
    }
}

/// Bits as upper-case hex, the last digit padded with zeros.
pub fn hex(bits: &[u8]) -> String {
    let mut s = String::with_capacity(bits.len().div_ceil(4));
    bits_to_hex(bits, &mut s);
    s
}

fn bits_from(r: &Fields, at: usize) -> Vec<u8> {
    r.0.get(at..).unwrap_or(&[]).to_vec()
}

/// The fields of a message's bits (one bit per byte, message-table order), read by position: unsigned and two's complement
/// numbers most significant bit first, flags and 6-bit text. Bits past the end read as 0.
#[derive(Clone, Copy, Debug)]
pub struct Fields<'a>(&'a [u8]);

impl<'a> Fields<'a> {
    /// The fields of `bits`.
    pub fn new(bits: &'a [u8]) -> Self {
        Fields(bits)
    }
    /// The number of bits.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Whether there are no bits.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// The unsigned field of `len` bits (at most 64) at `start`.
    pub fn u(&self, start: usize, len: usize) -> u64 {
        HardBits::at(self.0, start).u(len as u32)
    }
    /// The unsigned field of `len` bits (at most 32) at `start`.
    pub fn u32(&self, start: usize, len: usize) -> u32 {
        self.u(start, len) as u32
    }
    /// The two's complement field of `len` bits at `start`.
    pub fn i(&self, start: usize, len: usize) -> i64 {
        HardBits::at(self.0, start).i(len as u32)
    }
    /// The flag at `start`.
    pub fn b(&self, start: usize) -> bool {
        self.u(start, 1) == 1
    }
    /// `chars` 6-bit characters from `start`, with trailing `@` and spaces removed. Characters past the end are left out.
    pub fn text(&self, start: usize, chars: usize) -> String {
        let n = chars.min(self.len().saturating_sub(start) / 6);
        let mut s = String::with_capacity(n);
        if n > 0 {
            // The length is checked above, so the text cannot be short.
            let _ = ais_text_into(&self.0[start..], n, &mut s);
        }
        s
    }
    /// `chars` 6-bit characters from `start` with nothing removed. Characters past the end are left out.
    pub fn text_raw(&self, start: usize, chars: usize) -> String {
        (0..chars)
            .filter(|k| start + 6 * k + 6 <= self.len())
            .map(|k| ais_char(self.u(start + 6 * k, 6) as u8).unwrap_or('@'))
            .collect()
    }
    /// The whole 6-bit characters from `start` to the end.
    pub fn text_to_end(&self, start: usize) -> String {
        self.text(start, self.len().saturating_sub(start) / 6)
    }
    /// The bits from `start` to the end as upper-case hex (the last digit padded with zeros).
    pub fn hex_from(&self, start: usize) -> String {
        hex(&self.0[start.min(self.len())..])
    }
}

/// Appends to `out` the `!AIVDM` sentences of one message heard on `channel` (`A`, `B`, `C` or `D`): the bits armoured, at most
/// 60 payload characters per sentence, and the sequential message id `seq` (0 to 9) on a message of several sentences, after which
/// `seq` moves on. An error for a message longer than nine sentences.
///
/// Standard: IEC 61162-1 (VDM), ITU-R M.1371-5 Annex 8 (the 6-bit armour)
pub fn sentences(
    bits: &[u8],
    channel: char,
    seq: &mut u8,
    out: &mut Vec<String>,
) -> Result<(), sdrkit_util_geo::Error> {
    let mut payload = String::with_capacity(bits.len().div_ceil(6));
    let fill = aivdm::armour(bits, &mut payload)?;
    // The shared builder takes the channels of AIS 1 and 2 only: the long-range channels C and D are written into the field after.
    let shared = matches!(channel, 'A' | 'B');
    let id = *seq % 10;
    let mut count = 0;
    for f in aivdm::fragments("AIVDM", shared.then_some(channel), id, &payload, fill)? {
        let f = f?;
        count += 1;
        if shared {
            out.push(f.as_str().to_string());
            continue;
        }
        let s = nmea::parse(f.as_str())?;
        let mut b = nmea::Builder::new('!', s.address())?;
        for (i, v) in s.fields().enumerate() {
            if i == 3 {
                b.field_fmt(format_args!("{channel}"))?;
            } else {
                b.field(v)?;
            }
        }
        out.push(b.finish()?.as_str().to_string());
    }
    if count > 1 {
        *seq = (id + 1) % 10;
    }
    Ok(())
}

/// Ship and cargo types in words (M.1371 table 50).
pub fn ship_type_text(t: u8) -> &'static str {
    match t {
        0 => "Not available",
        1..=19 => "Reserved",
        20..=29 => "Wing in ground",
        30 => "Fishing",
        31 => "Towing",
        32 => "Towing, large",
        33 => "Dredging or underwater ops",
        34 => "Diving ops",
        35 => "Military ops",
        36 => "Sailing",
        37 => "Pleasure craft",
        38 | 39 => "Special",
        40..=49 => "High-speed craft",
        50 => "Pilot vessel",
        51 => "Search and rescue",
        52 => "Tug",
        53 => "Port tender",
        54 => "Anti-pollution",
        55 => "Law enforcement",
        58 => "Medical transport",
        59 => "Non-combatant ship",
        56 | 57 => "Special craft",
        60..=69 => "Passenger",
        70..=79 => "Cargo",
        80..=89 => "Tanker",
        _ => "Other",
    }
}

/// Navigational status names.
pub fn nav_status_text(s: u8) -> &'static str {
    const N: [&str; 16] = [
        "Under way (engine)",
        "At anchor",
        "Not under command",
        "Restricted manoeuvrability",
        "Constrained by draught",
        "Moored",
        "Aground",
        "Fishing",
        "Under way (sailing)",
        "Reserved 9",
        "Reserved 10",
        "Towing astern",
        "Pushing or towing alongside",
        "Reserved 13",
        "AIS-SART or similar",
        "Not defined",
    ];
    N.get(s as usize).copied().unwrap_or("Not defined")
}

/// Message type names for the counters.
pub fn type_name(t: u8) -> &'static str {
    match t {
        1..=3 => "Position report A",
        4 => "Base station report",
        5 => "Static and voyage data",
        6 => "Addressed binary",
        7 => "Binary acknowledge",
        8 => "Broadcast binary",
        9 => "SAR aircraft position",
        10 => "UTC inquiry",
        11 => "UTC response",
        12 => "Addressed safety text",
        13 => "Safety acknowledge",
        14 => "Safety broadcast",
        15 => "Interrogation",
        16 => "Assignment command",
        17 => "DGNSS broadcast",
        18 => "Position report B",
        19 => "Position report B, extended",
        20 => "Data link management",
        21 => "Aid to navigation",
        22 => "Channel management",
        23 => "Group assignment",
        24 => "Static data B",
        25 => "Single slot binary",
        26 => "Multi slot binary",
        27 => "Long range position",
        _ => "28 and up",
    }
}

/// Aid to navigation types (M.1371 table 74).
pub fn aid_type_text(t: u8) -> &'static str {
    const A: [&str; 32] = [
        "Not specified",
        "Reference point",
        "RACON",
        "Fixed structure off shore",
        "Spare",
        "Light, without sectors",
        "Light, with sectors",
        "Leading light front",
        "Leading light rear",
        "Beacon, cardinal N",
        "Beacon, cardinal E",
        "Beacon, cardinal S",
        "Beacon, cardinal W",
        "Beacon, port hand",
        "Beacon, starboard hand",
        "Beacon, preferred channel port hand",
        "Beacon, preferred channel starboard hand",
        "Beacon, isolated danger",
        "Beacon, safe water",
        "Beacon, special mark",
        "Cardinal mark N",
        "Cardinal mark E",
        "Cardinal mark S",
        "Cardinal mark W",
        "Port hand mark",
        "Starboard hand mark",
        "Preferred channel port hand",
        "Preferred channel starboard hand",
        "Isolated danger",
        "Safe water",
        "Special mark",
        "Light vessel / LANBY / rigs",
    ];
    A.get(t as usize).copied().unwrap_or("Not specified")
}

/// The ship-type group bit of the filter (bit 0 not received, 1 fishing, 2 towing / dredging / diving / military, 3 sailing and
/// pleasure, 4 high-speed, 5 pilot / tug / SAR / law enforcement, 6 passenger, 7 cargo, 8 tanker, 9 other).
pub fn type_group(t: Option<u8>) -> u32 {
    match t {
        None | Some(0) => 1,
        Some(30) => 1 << 1,
        Some(31..=35) => 1 << 2,
        Some(36 | 37) => 1 << 3,
        Some(40..=49) => 1 << 4,
        Some(50..=59) => 1 << 5,
        Some(60..=69) => 1 << 6,
        Some(70..=79) => 1 << 7,
        Some(80..=89) => 1 << 8,
        _ => 1 << 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bits of the message the sentences carry.
    pub(crate) fn bits_of(lines: &[&str]) -> Option<Vec<u8>> {
        let mut a = aivdm::Assembler::new();
        for l in lines {
            let s = nmea::parse(l).ok()?;
            let v = aivdm::Vdm::from_sentence(&s).ok()?;
            if let Some(m) = a.push(&v).ok()? {
                return Some(m.bits.to_vec());
            }
        }
        None
    }

    fn msg(lines: &[&str]) -> AisMessage {
        decode(&bits_of(lines).unwrap()).unwrap()
    }

    #[test]
    fn gpsd_examples() {
        let m = msg(&["!AIVDM,1,1,,A,15RTgt0PAso;90TKcjM8h6g208CQ,0*4A"]);
        assert_eq!((m.msg_type, m.mmsi, m.nav_status), (1, 371798000, Some(0)));
        assert_eq!(m.rot_deg_min, Some(-720.0));
        assert_eq!(m.sog_kn, Some(12.3));
        assert!((m.lat.unwrap() - 48.381_633_3).abs() < 2e-6);
        assert!((m.lon.unwrap() + 123.395_383_3).abs() < 2e-6);
        assert_eq!(
            (m.cog_deg, m.heading, m.second),
            (Some(224.0), Some(215), Some(33))
        );
        let m = msg(&["!AIVDM,1,1,,B,177KQJ5000G?tO`K>RA1wUbN0TKH,0*5C"]);
        assert_eq!(
            (m.mmsi, m.nav_status, m.heading, m.cog_deg, m.sog_kn),
            (477553000, Some(5), Some(181), Some(51.0), Some(0.0))
        );
        assert!(
            (m.lat.unwrap() - 47.582_83).abs() < 1e-5 && (m.lon.unwrap() + 122.345_83).abs() < 1e-5
        );
        let m = msg(&["!AIVDM,1,1,,A,403OviQuMGCqWrRO9>E6fE700@GO,0*4D"]);
        assert_eq!(m.utc.as_deref(), Some("2007-05-14T19:57:39Z"));
        assert_eq!(
            (m.mmsi, m.epfd, m.class),
            (3669702, Some(7), StationClass::BaseStation)
        );
        let m = msg(&[
            "!AIVDM,2,1,0,B,55?MbV02;H;s<HtKR20EHE:0@T4@Dn2222222216L961O5Gf0NSQEp6ClRp8,0*1E",
            "!AIVDM,2,2,0,B,88888888880,2*27",
        ]);
        assert_eq!(m.name.as_deref(), Some("EVER DIADEM"));
        assert_eq!(m.callsign.as_deref(), Some("3FOF8"));
        assert_eq!(
            (m.imo, m.ship_type, m.dim),
            (Some(9134270), Some(70), Some([225, 70, 1, 31]))
        );
        assert_eq!(m.destination.as_deref(), Some("NEW YORK"));
        assert_eq!(m.draught_m, Some(12.2));
        assert_eq!(m.eta, Some([5, 15, 14, 0]));
        let m = msg(&["!AIVDM,1,1,,B,>5?Per18=HB1U:1@E=B0m<L,2*52"]);
        assert_eq!(m.text.as_deref(), Some("RCVD YR TEST MSG"));
        let b = bits_of(&[
            "!AIVDM,1,1,,A,E1mg=5J1T4W0h97aRh6ba84<h2d;W:Te=eLvH50```q:D44QDlp0C1DU00,2*55",
        ])
        .unwrap();
        assert_eq!(decode(&b), Err(Reject::Mmsi));
        // The same frame from a plausible MMSI decodes the name with its extension.
        let mut b2 = b.clone();
        sdrkit_util_bits::pack::put_u64(&mut b2, 8, 244_000_001, 30).unwrap();
        let m = decode(&b2).unwrap();
        assert_eq!(m.name.as_deref(), Some("CHINA ROSE MURPHY EXPRESS ALERT"));
        assert_eq!(
            (m.aid_type, m.off_position, m.virtual_aid),
            (Some(20), Some(false), Some(false))
        );
        assert!((m.lat.unwrap() - 47.920_618_3).abs() < 2e-6);
        let m = msg(&["!AIVDM,1,1,,B,KCQ9r=hrFUnH7P00,0*42"]);
        assert_eq!((m.msg_type, m.mmsi, m.nav_status), (27, 236091959, Some(3)));
        assert!(
            (m.lat.unwrap() - 87.065).abs() < 1e-5 && (m.lon.unwrap() + 154.201_666_7).abs() < 1e-5
        );
        let mut bits = Vec::new();
        aivdm::dearmour("KC5E2b@U19PFdLbMuc5=ROv62<7m", 0, &mut bits).unwrap();
        let m = decode(&bits).unwrap();
        assert_eq!(
            (m.mmsi, m.nav_status, m.sog_kn, m.cog_deg),
            (206914217, Some(2), Some(57.0), Some(167.0))
        );
        assert!(
            (m.lon.unwrap() - 82214.0 / 600.0).abs() < 1e-9
                && (m.lat.unwrap() - 2904.0 / 600.0).abs() < 1e-9
        );
    }

    #[test]
    fn sentences_are_rebuilt_exactly_and_the_long_range_channels_are_written() {
        let s = "!AIVDM,1,1,,A,15RTgt0PAso;90TKcjM8h6g208CQ,0*4A";
        let b = bits_of(&[s]).unwrap();
        let mut out = Vec::new();
        let mut seq = 0;
        sentences(&b, 'A', &mut seq, &mut out).unwrap();
        assert_eq!(out, [s]);
        assert_eq!(seq, 0);
        let parts = [
            "!AIVDM,2,1,0,B,55?MbV02;H;s<HtKR20EHE:0@T4@Dn2222222216L961O5Gf0NSQEp6ClRp8,0*1E",
            "!AIVDM,2,2,0,B,88888888880,2*27",
        ];
        let b = bits_of(&parts).unwrap();
        out.clear();
        sentences(&b, 'B', &mut seq, &mut out).unwrap();
        assert_eq!(out, parts);
        assert_eq!(seq, 1);
        out.clear();
        let b = bits_of(&["!AIVDM,1,1,,B,KCQ9r=hrFUnH7P00,0*42"]).unwrap();
        sentences(&b, 'C', &mut seq, &mut out).unwrap();
        // The checksum of the same sentence with C in place of B: 0x42 ^ b'B' ^ b'C'.
        assert_eq!(out, ["!AIVDM,1,1,,C,KCQ9r=hrFUnH7P00,0*43"]);
        let p = nmea::parse(&out[0]).unwrap();
        assert_eq!(p.field(3), Some("C"));
    }
}

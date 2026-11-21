//! The safety messages of road vehicles in 802.11 data frames: the SAE J2735 BasicSafetyMessage (its core data) after the
//! EtherType of WSMP, and the ETSI cooperative awareness message (header and basic container) after the EtherType of
//! GeoNetworking. Both messages are unaligned PER. The WSMP and GeoNetworking headers are not parsed: the messages are found by
//! their first bytes, and the field ranges refuse most chance matches.
//! Standard: SAE J2735 (MessageFrame, BSMcoreData); ETSI EN 302 637-2 (CAM); IEEE 1609.3 (WSMP); ETSI EN 302 636-5-1 (BTP);
//! ITU-T X.691.

use sdrkit_util_bits::per::{PerIo, PerReader, PerWriter};

/// The messageId of the BasicSafetyMessage in a J2735 MessageFrame, as the two octets that start the frame.
pub const BSM_START: [u8; 2] = [0x00, 0x14];
/// The BTP-B destination port of the CAM.
pub const CAM_PORT: [u8; 2] = [0x07, 0xD1];
/// The BTP-B destination port of the DENM.
pub const DENM_PORT: [u8; 2] = [0x07, 0xD2];
/// The EtherType of WSMP.
pub const ETHERTYPE_WSMP: u16 = 0x88DC;
/// The EtherType of GeoNetworking.
pub const ETHERTYPE_GEONET: u16 = 0x8947;
/// The LLC/SNAP header before the EtherType.
pub const SNAP: [u8; 6] = [0xAA, 0xAA, 0x03, 0x00, 0x00, 0x00];
/// The fewest octets from the start of a BSM frame (`00 14`) that the scanner reads.
pub const BSM_MIN: usize = 30;
/// The fewest octets from the start of a CAM that the scanner reads.
pub const CAM_MIN: usize = 28;

/// Latitude offset of J2735 and of the position of an ETSI basic container: the coded value is the latitude in 1e-7 degree plus this.
const LAT_OFFSET: i64 = 900_000_000;
/// The coded latitude that means "unavailable".
const LAT_UNAVAILABLE: i64 = 1_800_000_001;
/// Longitude offset of J2735 (its range starts at -179.9999999 degrees).
const BSM_LON_OFFSET: i64 = 1_799_999_999;
/// Longitude offset of the position of an ETSI basic container.
const CAM_LON_OFFSET: i64 = 1_800_000_000;
/// The longitude 180.0000001 degrees, which means "unavailable" in both.
const LON_UNAVAILABLE_DEG7: i64 = 1_800_000_001;

/// The core data of a BasicSafetyMessage up to the heading.
#[derive(Clone, Debug, PartialEq)]
pub struct Bsm {
    /// msgCnt, 0 to 127.
    pub msg_count: u8,
    /// The temporary id (4 octets).
    pub id: u32,
    /// secMark, milliseconds within the minute.
    pub sec_mark: u16,
    /// Latitude and longitude, degrees; `None` when unavailable.
    pub position: Option<(f64, f64)>,
    /// Elevation, metres; `None` when unavailable.
    pub elevation_m: Option<f64>,
    /// TransmissionState, 0 to 7 (7 unavailable).
    pub transmission: u8,
    /// Speed, m/s; `None` when unavailable.
    pub speed_m_s: Option<f64>,
    /// Heading, degrees; `None` when unavailable.
    pub heading_deg: Option<f64>,
}

impl Bsm {
    /// The text of a row: "message 72, elevation 275.0 m, speed 6.1 m/s, heading 275.0".
    pub fn text(&self) -> String {
        let mut t = format!("message {}", self.msg_count);
        if let Some(e) = self.elevation_m {
            t.push_str(&format!(", elevation {e:.1} m"));
        }
        if let Some(s) = self.speed_m_s {
            t.push_str(&format!(", speed {s:.1} m/s"));
        }
        if let Some(h) = self.heading_deg {
            t.push_str(&format!(", heading {h:.1}"));
        }
        t
    }
}

/// The ItsPduHeader and the basic container of a CAM.
#[derive(Clone, Debug, PartialEq)]
pub struct Cam {
    /// protocolVersion, 1 to 3.
    pub protocol: u8,
    /// stationID.
    pub station_id: u32,
    /// generationDeltaTime, ms.
    pub generation_delta_ms: u16,
    /// stationType.
    pub station_type: u8,
    /// Latitude and longitude, degrees; `None` when unavailable.
    pub position: Option<(f64, f64)>,
}

impl Cam {
    /// The text of a row: "station type car".
    pub fn text(&self) -> String {
        format!("station type {}", station_type_name(self.station_type))
    }
}

/// The name of an ETSI stationType.
pub fn station_type_name(t: u8) -> &'static str {
    match t {
        0 => "unknown",
        1 => "pedestrian",
        2 => "cyclist",
        3 => "moped",
        4 => "motorcycle",
        5 => "car",
        6 => "bus",
        7 => "light truck",
        8 => "heavy truck",
        9 => "trailer",
        10 => "special vehicle",
        11 => "tram",
        15 => "roadside unit",
        _ => "reserved",
    }
}

fn bits<P: PerIo>(p: &mut P, v: &mut u64, n: u32) {
    p.bits(v, n);
}

fn position(lat: i64, lon: i64) -> Option<(f64, f64)> {
    (lat != 900_000_001 && lon != LON_UNAVAILABLE_DEG7)
        .then_some((lat as f64 * 1e-7, lon as f64 * 1e-7))
}

/// Reads a J2735 MessageFrame that holds a BasicSafetyMessage: `00 14`, the length of the open type (one octet below 128, two
/// octets `10xxxxxx xxxxxxxx` below 16384), then the message. `None` when the frame is something else, too short, or holds a
/// value outside its range.
pub fn read_bsm(frame: &[u8]) -> Option<Bsm> {
    if frame.get(..2)? != BSM_START {
        return None;
    }
    let (len, at) = match *frame.get(2)? {
        b if b & 0x80 == 0 => (usize::from(b), 3),
        b if b & 0xC0 == 0x80 => (
            (usize::from(b & 0x3F) << 8) | usize::from(*frame.get(3)?),
            4,
        ),
        _ => return None,
    };
    let body = frame.get(at..at + len)?;
    let mut r = PerReader::new(body);
    // Extension bit, partII present, regional present.
    let mut preamble = 0;
    bits(&mut r, &mut preamble, 3);
    let mut v = [0u64; 12];
    for (i, n) in [7, 32, 16, 31, 32, 16, 8, 8, 16, 3, 13, 15]
        .into_iter()
        .enumerate()
    {
        bits(&mut r, &mut v[i], n);
    }
    if r.failed() {
        return None;
    }
    let [count, id, sec, lat, lon, elev, _semi_major, _semi_minor, _orientation, tr, speed, heading] =
        v;
    let lat = lat as i64 - LAT_OFFSET;
    let lon = lon as i64 - BSM_LON_OFFSET;
    if lat.abs() > 900_000_001 || lon.abs() > LON_UNAVAILABLE_DEG7 || heading > 28_800 {
        return None;
    }
    Some(Bsm {
        msg_count: count as u8,
        id: id as u32,
        sec_mark: sec as u16,
        position: position(lat, lon),
        elevation_m: (elev != 0).then(|| (elev as f64 - 4096.0) / 10.0),
        transmission: tr as u8,
        speed_m_s: (speed != 8191).then_some(speed as f64 * 0.02),
        heading_deg: (heading != 28_800).then_some(heading as f64 * 0.0125),
    })
}

/// Reads the ItsPduHeader and the basic container of a CAM. `None` when the protocol version is not 1 to 3, the message is not
/// a CAM (messageID 2), the input is short, or the latitude or longitude is outside its range.
pub fn read_cam(pdu: &[u8]) -> Option<Cam> {
    let mut r = PerReader::new(pdu);
    let mut v = [0u64; 8];
    for (i, n) in [8, 8, 32, 16, 3, 1, 8, 31].into_iter().enumerate() {
        bits(&mut r, &mut v[i], n);
    }
    let mut lon = 0;
    bits(&mut r, &mut lon, 32);
    if r.failed() {
        return None;
    }
    let [protocol, message, station, delta, _preamble, _ext, station_type, lat] = v;
    if !(1..=3).contains(&protocol) || message != 2 {
        return None;
    }
    let lat = lat as i64 - LAT_OFFSET;
    let lon = lon as i64 - CAM_LON_OFFSET;
    if lat.abs() > 900_000_001 || lon.abs() > LON_UNAVAILABLE_DEG7 {
        return None;
    }
    Some(Cam {
        protocol: protocol as u8,
        station_id: station as u32,
        generation_delta_ms: delta as u16,
        station_type: station_type as u8,
        position: position(lat, lon),
    })
}

/// What one data frame holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    /// A BasicSafetyMessage.
    Bsm(Bsm),
    /// A cooperative awareness message.
    Cam(Cam),
    /// A decentralised environmental notification (BTP port 2002): counted, not decoded.
    Denm,
    /// A WSMP frame of this many octets with no BSM.
    Wsmp(usize),
    /// A GeoNetworking frame of this many octets with no CAM.
    GeoNet(usize),
}

/// Reads an MPDU (FCS removed): a data frame with a 24-octet header (26 for QoS data), LLC/SNAP and the EtherType of WSMP or
/// GeoNetworking. `None` for any other frame.
pub fn scan_mpdu(mpdu: &[u8]) -> Option<Found> {
    let fc = *mpdu.first()?;
    if fc & 0x0C != 0x08 {
        return None;
    }
    let header = if fc & 0x80 != 0 { 26 } else { 24 };
    let rest = mpdu.get(header..)?;
    if rest.get(..6)? != SNAP {
        return None;
    }
    let ethertype = u16::from_be_bytes([*rest.get(6)?, *rest.get(7)?]);
    let body = &rest[8..];
    match ethertype {
        ETHERTYPE_WSMP => Some(
            find_all(body, BSM_START)
                .filter(|&i| body.len() - i >= BSM_MIN)
                .find_map(|i| read_bsm(&body[i..]))
                .map_or(Found::Wsmp(body.len()), Found::Bsm),
        ),
        ETHERTYPE_GEONET => {
            let cam = find_all(body, CAM_PORT)
                .filter_map(|i| body.get(i + 4..))
                .filter(|c| c.len() >= CAM_MIN)
                .find_map(read_cam);
            if let Some(c) = cam {
                return Some(Found::Cam(c));
            }
            if find_all(body, DENM_PORT).next().is_some() {
                return Some(Found::Denm);
            }
            Some(Found::GeoNet(body.len()))
        }
        _ => None,
    }
}

fn find_all(hay: &[u8], pat: [u8; 2]) -> impl Iterator<Item = usize> + '_ {
    hay.windows(2)
        .enumerate()
        .filter(move |(_, w)| *w == pat)
        .map(|(i, _)| i)
}

/// The parts of the BSM core data after the heading that the writer sends: steering wheel angle, accelerations, brakes and
/// size, as coded values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BsmTail {
    /// Steering wheel angle, coded (127: unavailable).
    pub angle: u8,
    /// Longitudinal, lateral, vertical and yaw rate, coded.
    pub accel: [u16; 4],
    /// Wheel brakes (5 bits) and the five 2-bit states.
    pub brakes: [u8; 6],
    /// Width and length, cm.
    pub size_cm: [u16; 2],
}

impl Default for BsmTail {
    fn default() -> Self {
        BsmTail {
            angle: 127,
            accel: [2001, 2001, 0, 0],
            brakes: [0; 6],
            size_cm: [150, 530],
        }
    }
}

/// Appends a MessageFrame with the BasicSafetyMessage of `b` (no part II, no regional data) and the tail `t`.
pub fn write_bsm(b: &Bsm, t: &BsmTail, out: &mut Vec<u8>) {
    let mut body = Vec::new();
    let mut w = PerWriter::new(&mut body);
    let (lat, lon) = b.position.map_or(
        (LAT_UNAVAILABLE, LON_UNAVAILABLE_DEG7 + BSM_LON_OFFSET),
        |(la, lo)| {
            (
                (la * 1e7).round() as i64 + LAT_OFFSET,
                (lo * 1e7).round() as i64 + BSM_LON_OFFSET,
            )
        },
    );
    let elev = b
        .elevation_m
        .map_or(0, |e| (e * 10.0).round() as i64 + 4096);
    let speed = b.speed_m_s.map_or(8191, |s| (s / 0.02).round() as i64);
    let heading = b
        .heading_deg
        .map_or(28_800, |h| (h / 0.0125).round() as i64);
    let fields: [(i64, u32); 24] = [
        (0, 3),
        (i64::from(b.msg_count), 7),
        (i64::from(b.id), 32),
        (i64::from(b.sec_mark), 16),
        (lat, 31),
        (lon, 32),
        (elev, 16),
        (255, 8),
        (255, 8),
        (65_535, 16),
        (i64::from(b.transmission), 3),
        (speed, 13),
        (heading, 15),
        (i64::from(t.angle) + 126, 8),
        (i64::from(t.accel[0]) + 2000, 12),
        (i64::from(t.accel[1]) + 2000, 12),
        (i64::from(t.accel[2]), 8),
        (i64::from(t.accel[3]) + 32_767, 16),
        (i64::from(t.brakes[0]), 5),
        (i64::from(t.brakes[1]), 2),
        (i64::from(t.brakes[2]), 2),
        (i64::from(t.brakes[3]), 2),
        (i64::from(t.brakes[4]), 2),
        (i64::from(t.brakes[5]), 2),
    ];
    for (v, n) in fields {
        let mut u = v.max(0) as u64;
        w.bits(&mut u, n);
    }
    for (v, n) in [(t.size_cm[0], 10), (t.size_cm[1], 12)] {
        let mut u = u64::from(v);
        w.bits(&mut u, n);
    }
    w.finish();
    out.extend_from_slice(&BSM_START);
    if body.len() < 128 {
        out.push(body.len() as u8);
    } else {
        out.extend_from_slice(&(0x8000 | body.len() as u16).to_be_bytes());
    }
    out.extend_from_slice(&body);
}

/// Appends a CAM with the header and basic container of `c`, its other containers as zero bits, at least `min_len` octets.
pub fn write_cam(c: &Cam, min_len: usize, out: &mut Vec<u8>) {
    let start = out.len();
    let mut body = Vec::new();
    let mut w = PerWriter::new(&mut body);
    let (lat, lon) = c.position.map_or(
        (LAT_UNAVAILABLE, LON_UNAVAILABLE_DEG7 + CAM_LON_OFFSET),
        |(la, lo)| {
            (
                (la * 1e7).round() as i64 + LAT_OFFSET,
                (lo * 1e7).round() as i64 + CAM_LON_OFFSET,
            )
        },
    );
    let fields: [(i64, u32); 9] = [
        (i64::from(c.protocol), 8),
        (2, 8),
        (i64::from(c.station_id), 32),
        (i64::from(c.generation_delta_ms), 16),
        (0, 3),
        (0, 1),
        (i64::from(c.station_type), 8),
        (lat, 31),
        (lon, 32),
    ];
    for (v, n) in fields {
        let mut u = v.max(0) as u64;
        w.bits(&mut u, n);
    }
    w.finish();
    out.extend_from_slice(&body);
    while out.len() - start < min_len {
        out.push(0);
    }
}

/// Appends the 802.11 header of a QoS data frame broadcast outside the context of a BSS (OCB), LLC/SNAP and the EtherType.
/// `sender` is the transmitter address.
pub fn write_data_header(sender: [u8; 6], seq: u16, ethertype: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x88, 0x00, 0x00, 0x00]);
    out.extend_from_slice(&[0xFF; 6]);
    out.extend_from_slice(&sender);
    out.extend_from_slice(&[0xFF; 6]);
    out.extend_from_slice(&((seq & 0x0FFF) << 4).to_le_bytes());
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&SNAP);
    out.extend_from_slice(&ethertype.to_be_bytes());
}

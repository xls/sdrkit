//! ADS-C, Automatic Dependent Surveillance - Contract (ARINC 745, the FANS-1/A form of RTCA DO-258).
//!
//! A ground station sets up a contract with an aircraft. The aircraft then sends reports by the contract's rules. Both directions
//! travel in ARINC 622 envelopes ([`crate::arinc622`]): `ADS` for messages and `DIS` for a disconnect.
//!
//! A downlink (aircraft to ground) message is a sequence of groups. Each group starts with a tag octet, and its length follows
//! from the tag. [`groups`] reads them one by one without allocation. [`Group::encode`] writes each one back.
//!
//! An uplink (ground to aircraft) message is a request ([`Request`]) with a contract number, then request groups
//! ([`RequestGroup`]) that say what to report and when.
//!
//! Field scales (all MSB first):
//!
//! | Field | Bits | Scale |
//! |---|---|---|
//! | latitude, longitude | 21, two's complement | 90 / 2^19 degrees |
//! | altitude | 16, two's complement | 4 ft |
//! | time stamp | 15 | 0.125 s past the hour |
//! | track, heading | 12, two's complement | 90 / 1024 degrees, shown 0 to 360 |
//! | ground speed / Mach | 13 | 0.5 kt / 0.0005 |
//! | vertical rate | 12, two's complement | 16 ft/min |
//! | wind speed, direction | 9, 9 (two's complement) | 0.5 kt, 90 / 128 degrees |
//! | temperature | 12, two's complement | 0.25 °C |
//!
//! Standard: ARINC 745 (not freely available).
//!
//! ```
//! use sdrkit_proto_aero::adsc::{groups, Group, Report, ReportKind, Fom};
//! let r = Group::Report(Report {
//!     kind: ReportKind::Basic, lat_deg: 51.5, lon_deg: -0.4, alt_ft: 36000.0, sec_past_hour: 1234.5,
//!     fom: Fom { accuracy: 6, redundancy: true, tcas: true },
//! });
//! let mut bin = Vec::new();
//! r.encode(&mut bin).unwrap();
//! assert_eq!(bin.len(), 11);
//! let back = groups(&bin).next().unwrap().unwrap();
//! assert!(matches!(back, Group::Report(p) if (p.lat_deg - 51.5).abs() < 2e-4));
//! ```

use crate::arinc622::{self, Imi};
use crate::error::{Error, Item, Result};
use sdrkit_util_bits::{BitReader, BitWriter};
use sdrkit_util_geo::coord::{angle_from_raw, angle_to_raw};
use sdrkit_util_geo::position::wrap360;
use sdrkit_util_geo::Position;
use sdrkit_util_text::sixbit::{icao_char, icao_code};

/// The kind of a position report: the basic report or the event that caused it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportKind {
    /// Tag 7: basic report.
    Basic,
    /// Tag 9: emergency basic report.
    Emergency,
    /// Tag 10: lateral deviation change event.
    LateralDeviation,
    /// Tag 18: vertical rate change event.
    VerticalRate,
    /// Tag 19: altitude range event.
    AltitudeRange,
    /// Tag 20: waypoint change event.
    WaypointChange,
}

impl ReportKind {
    fn tag(self) -> u8 {
        match self {
            ReportKind::Basic => 7,
            ReportKind::Emergency => 9,
            ReportKind::LateralDeviation => 10,
            ReportKind::VerticalRate => 18,
            ReportKind::AltitudeRange => 19,
            ReportKind::WaypointChange => 20,
        }
    }

    fn from_tag(t: u8) -> Option<Self> {
        Some(match t {
            7 => ReportKind::Basic,
            9 => ReportKind::Emergency,
            10 => ReportKind::LateralDeviation,
            18 => ReportKind::VerticalRate,
            19 => ReportKind::AltitudeRange,
            20 => ReportKind::WaypointChange,
            _ => return None,
        })
    }
}

/// The figure of merit of a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fom {
    /// Position accuracy, 0 to 7 ([`ACCURACY`] holds the names).
    pub accuracy: u8,
    /// Navigation redundancy is available.
    pub redundancy: bool,
    /// TCAS is available.
    pub tcas: bool,
}

/// The names of the accuracy values 0 to 7.
pub const ACCURACY: [&str; 8] = [
    "navigation lost",
    "< 30 nm",
    "< 15 nm",
    "< 8 nm",
    "< 4 nm",
    "< 1 nm",
    "< 0.25 nm",
    "< 0.05 nm",
];

/// A position report (basic, emergency or event).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Report {
    /// Which report.
    pub kind: ReportKind,
    /// Degrees north.
    pub lat_deg: f64,
    /// Degrees east.
    pub lon_deg: f64,
    /// Altitude, feet.
    pub alt_ft: f64,
    /// The time of the position, seconds past the hour.
    pub sec_past_hour: f64,
    /// The figure of merit.
    pub fom: Fom,
}

/// A point of a predicted route or a projected intent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waypoint {
    /// Degrees north.
    pub lat_deg: f64,
    /// Degrees east.
    pub lon_deg: f64,
    /// Feet.
    pub alt_ft: f64,
}

/// One downlink group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Group {
    /// Tag 3: the contract request is accepted.
    Ack {
        /// The contract number.
        contract: u8,
    },
    /// Tag 4: the contract request is refused.
    Nack {
        /// The contract number.
        contract: u8,
        /// The reason code.
        reason: u8,
        /// Extra data that reasons 1, 2 and 7 carry.
        extended: Option<u8>,
    },
    /// Tag 5: the aircraft cannot do part of a contract.
    Noncompliance {
        /// The contract number.
        contract: u8,
        /// The number of groups it names.
        groups: u8,
    },
    /// Tag 6: the emergency mode ends.
    CancelEmergency,
    /// Tags 7, 9, 10, 18, 19, 20: a position report.
    Report(Report),
    /// Tag 12: the flight id (ICAO 6-bit characters, trailing spaces removed).
    FlightId {
        /// The characters (ASCII).
        id: [u8; 8],
        /// How many are used.
        len: u8,
    },
    /// Tag 13: the next waypoint, the time to it, and the one after.
    PredictedRoute {
        /// The next waypoint.
        next: Waypoint,
        /// Seconds to the next waypoint.
        eta_s: u32,
        /// The waypoint after it.
        then: Waypoint,
    },
    /// Tag 14: track over the ground.
    EarthReference {
        /// Degrees true, 0 to 360.
        track_deg: f64,
        /// The track is valid.
        valid: bool,
        /// Ground speed, knots.
        gs_kt: f64,
        /// Vertical rate, feet per minute.
        vs_fpm: f64,
    },
    /// Tag 15: heading through the air.
    AirReference {
        /// Degrees true, 0 to 360.
        heading_deg: f64,
        /// The heading is valid.
        valid: bool,
        /// Mach number.
        mach: f64,
        /// Vertical rate, feet per minute.
        vs_fpm: f64,
    },
    /// Tag 16: weather at the aircraft.
    Meteo {
        /// Wind speed, knots.
        wind_kt: f64,
        /// The wind direction is valid.
        valid: bool,
        /// Wind direction, degrees true, 0 to 360.
        wind_dir_deg: f64,
        /// Temperature, °C.
        temp_c: f64,
    },
    /// Tag 17: the 24-bit ICAO aircraft address.
    Airframe {
        /// The address.
        icao: u32,
    },
    /// Tag 22: intermediate projected intent.
    IntermediateIntent {
        /// Distance from the present position, nautical miles.
        distance_nm: f64,
        /// The track is valid.
        valid: bool,
        /// Track, degrees true.
        track_deg: f64,
        /// Feet.
        alt_ft: f64,
        /// Projected time, seconds.
        eta_s: u32,
    },
    /// Tag 23: fixed projected intent.
    FixedIntent {
        /// The point.
        point: Waypoint,
        /// Projected time, seconds.
        eta_s: u32,
    },
}

/// The name of a downlink tag, or "unknown".
pub fn tag_name(tag: u8) -> &'static str {
    match tag {
        3 => "acknowledgement",
        4 => "negative acknowledgement",
        5 => "noncompliance notification",
        6 => "cancel emergency",
        7 => "basic report",
        9 => "emergency basic report",
        10 => "lateral deviation change event",
        12 => "flight id",
        13 => "predicted route",
        14 => "earth reference",
        15 => "air reference",
        16 => "meteorological data",
        17 => "airframe id",
        18 => "vertical rate change event",
        19 => "altitude range event",
        20 => "waypoint change event",
        22 => "intermediate projected intent",
        23 => "fixed projected intent",
        _ => "unknown",
    }
}

/// The length of a downlink group after its tag, from the octets that follow the tag.
fn group_len(tag: u8, body: &[u8]) -> Result<usize> {
    Ok(match tag {
        3 => 1,
        4 => match body.get(1) {
            Some(1 | 2 | 7) => 3,
            Some(_) => 2,
            None => return Err(Error::Short),
        },
        5 => {
            let n = *body.get(1).ok_or(Error::Short)? as usize;
            let mut i = 2;
            for _ in 0..n {
                let flags = *body.get(i + 1).ok_or(Error::Short)?;
                i += 2 + ((flags & 0x0F) as usize).div_ceil(2);
            }
            i
        }
        6 => 0,
        7 | 9 | 10 | 18 | 19 | 20 => 10,
        12 => 6,
        13 => 17,
        14 | 15 => 5,
        16 => 4,
        17 => 3,
        22 => 8,
        23 => 9,
        _ => return Err(Error::BadField(Item::AdscDownlinkTag)),
    })
}

fn coord(r: &mut BitReader) -> f64 {
    angle_from_raw(r.i(21), 21, 360.0)
}

fn angle(r: &mut BitReader, bits: u32) -> f64 {
    wrap360(angle_from_raw(r.i(bits), bits, 360.0))
}

fn waypoint(r: &mut BitReader) -> Waypoint {
    let lat_deg = coord(r);
    let lon_deg = coord(r);
    Waypoint {
        lat_deg,
        lon_deg,
        alt_ft: r.i(16) as f64 * 4.0,
    }
}

fn parse_group(tag: u8, b: &[u8]) -> Result<Group> {
    let mut r = BitReader::new(b);
    let g = match tag {
        3 => Group::Ack { contract: b[0] },
        4 => Group::Nack {
            contract: b[0],
            reason: b[1],
            extended: b.get(2).copied(),
        },
        5 => Group::Noncompliance {
            contract: b[0],
            groups: b[1],
        },
        6 => Group::CancelEmergency,
        12 => {
            let mut id = [b' '; 8];
            for c in id.iter_mut() {
                *c = icao_char(r.u(6) as u8).map_or(b'?', |c| c as u8);
            }
            let len = id.iter().rposition(|&c| c != b' ').map_or(0, |i| i + 1) as u8;
            Group::FlightId { id, len }
        }
        13 => {
            let next = waypoint(&mut r);
            let eta_s = r.u(14) as u32;
            Group::PredictedRoute {
                next,
                eta_s,
                then: waypoint(&mut r),
            }
        }
        14 | 15 => {
            let valid = !r.bit();
            let a = angle(&mut r, 12);
            let s = r.u(13) as f64;
            let vs_fpm = r.i(12) as f64 * 16.0;
            if tag == 14 {
                Group::EarthReference {
                    track_deg: a,
                    valid,
                    gs_kt: s * 0.5,
                    vs_fpm,
                }
            } else {
                Group::AirReference {
                    heading_deg: a,
                    valid,
                    mach: s * 0.0005,
                    vs_fpm,
                }
            }
        }
        16 => {
            let wind_kt = r.u(9) as f64 * 0.5;
            let valid = !r.bit();
            let wind_dir_deg = angle(&mut r, 9);
            Group::Meteo {
                wind_kt,
                valid,
                wind_dir_deg,
                temp_c: r.i(12) as f64 * 0.25,
            }
        }
        17 => Group::Airframe {
            icao: r.u(24) as u32,
        },
        22 => {
            let distance_nm = r.u(16) as f64 * 0.125;
            let valid = !r.bit();
            let track_deg = angle(&mut r, 12);
            let alt_ft = r.i(16) as f64 * 4.0;
            Group::IntermediateIntent {
                distance_nm,
                valid,
                track_deg,
                alt_ft,
                eta_s: r.u(14) as u32,
            }
        }
        23 => {
            let point = waypoint(&mut r);
            Group::FixedIntent {
                point,
                eta_s: r.u(14) as u32,
            }
        }
        _ => {
            let kind = ReportKind::from_tag(tag).ok_or(Error::BadField(Item::AdscDownlinkTag))?;
            let lat_deg = coord(&mut r);
            let lon_deg = coord(&mut r);
            let alt_ft = r.i(16) as f64 * 4.0;
            let sec_past_hour = r.u(15) as f64 * 0.125;
            let v = r.u(7) as u8;
            Group::Report(Report {
                kind,
                lat_deg,
                lon_deg,
                alt_ft,
                sec_past_hour,
                fom: Fom {
                    accuracy: (v >> 1) & 7,
                    redundancy: v & 1 != 0,
                    tcas: (v >> 4) & 1 != 0,
                },
            })
        }
    };
    Ok(g)
}

/// The groups of a downlink message, read one by one (see [`groups`]).
#[derive(Clone, Debug)]
pub struct Groups<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Iterator for Groups<'_> {
    type Item = Result<Group>;

    /// The next group. After an error (an unknown tag or a short group) the iterator ends, because the next tag is unknown.
    fn next(&mut self) -> Option<Result<Group>> {
        let tag = *self.data.get(self.pos)?;
        let body = &self.data[self.pos + 1..];
        let item = group_len(tag, body).and_then(|n| {
            let b = body.get(..n).ok_or(Error::Short)?;
            self.pos += 1 + n;
            parse_group(tag, b)
        });
        if item.is_err() {
            self.pos = self.data.len();
        }
        Some(item)
    }
}

/// The groups of a downlink ADS-C message (the binary part of an `ADS` envelope, CRC removed).
pub fn groups(data: &[u8]) -> Groups<'_> {
    Groups { data, pos: 0 }
}

fn put_coord(w: &mut BitWriter, deg: f64) -> Result<()> {
    if !(-180.0..=180.0).contains(&deg) {
        return Err(Error::BadField(Item::LatitudeOrLongitude));
    }
    w.put(
        angle_to_raw(deg, 21, 360.0).map_err(|_| Error::BadField(Item::Angle))?,
        21,
    );
    Ok(())
}

fn put_angle(w: &mut BitWriter, deg: f64, bits: u32) -> Result<()> {
    w.put(
        angle_to_raw(deg, bits, 360.0).map_err(|_| Error::BadField(Item::Angle))?,
        bits,
    );
    Ok(())
}

/// Scales `v` by `1 / lsb` into a signed field of `bits` bits, or an error when it does not fit.
fn signed(v: f64, lsb: f64, bits: u32, what: Item) -> Result<i64> {
    let x = (v / lsb).round();
    let lim = (1i64 << (bits - 1)) as f64;
    if !x.is_finite() || x < -lim || x >= lim {
        return Err(Error::BadField(what));
    }
    Ok(x as i64)
}

/// Scales `v` by `1 / lsb` into an unsigned field of `bits` bits, or an error when it does not fit.
fn unsigned(v: f64, lsb: f64, bits: u32, what: Item) -> Result<u64> {
    let x = (v / lsb).round();
    if !x.is_finite() || x < 0.0 || x >= (1u64 << bits) as f64 {
        return Err(Error::BadField(what));
    }
    Ok(x as u64)
}

fn put_waypoint(w: &mut BitWriter, p: &Waypoint) -> Result<()> {
    put_coord(w, p.lat_deg)?;
    put_coord(w, p.lon_deg)?;
    w.put_signed(signed(p.alt_ft, 4.0, 16, Item::Altitude)?, 16);
    Ok(())
}

impl Group {
    /// The tag octet.
    pub fn tag(&self) -> u8 {
        match self {
            Group::Ack { .. } => 3,
            Group::Nack { .. } => 4,
            Group::Noncompliance { .. } => 5,
            Group::CancelEmergency => 6,
            Group::Report(r) => r.kind.tag(),
            Group::FlightId { .. } => 12,
            Group::PredictedRoute { .. } => 13,
            Group::EarthReference { .. } => 14,
            Group::AirReference { .. } => 15,
            Group::Meteo { .. } => 16,
            Group::Airframe { .. } => 17,
            Group::IntermediateIntent { .. } => 22,
            Group::FixedIntent { .. } => 23,
        }
    }

    /// Appends the tag and the group to `out`. A value outside its field's range is an error, and nothing is appended. A
    /// noncompliance group is written with no group entries.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        let start = out.len();
        out.push(self.tag());
        let r = self.encode_body(out);
        if r.is_err() {
            out.truncate(start);
        }
        r
    }

    fn encode_body(&self, out: &mut Vec<u8>) -> Result<()> {
        match *self {
            Group::Ack { contract } => out.push(contract),
            Group::Nack {
                contract,
                reason,
                extended,
            } => {
                out.extend_from_slice(&[contract, reason]);
                match (matches!(reason, 1 | 2 | 7), extended) {
                    (true, Some(x)) => out.push(x),
                    (false, None) => {}
                    _ => return Err(Error::BadField(Item::NegativeAckExtension)),
                }
            }
            Group::Noncompliance { contract, groups } => {
                if groups != 0 {
                    return Err(Error::BadField(Item::NoncomplianceGroups));
                }
                out.extend_from_slice(&[contract, 0]);
            }
            Group::CancelEmergency => {}
            Group::Report(r) => {
                let mut w = BitWriter::new(out);
                put_coord(&mut w, r.lat_deg)?;
                put_coord(&mut w, r.lon_deg)?;
                w.put_signed(signed(r.alt_ft, 4.0, 16, Item::Altitude)?, 16);
                w.put(unsigned(r.sec_past_hour, 0.125, 15, Item::TimeStamp)?, 15);
                if r.fom.accuracy > 7 {
                    return Err(Error::BadField(Item::Accuracy));
                }
                let v = u64::from(r.fom.redundancy)
                    | u64::from(r.fom.accuracy) << 1
                    | u64::from(r.fom.tcas) << 4;
                w.put(v, 7);
                w.finish();
            }
            Group::FlightId { id, len } => {
                let mut w = BitWriter::new(out);
                for (i, &c) in id.iter().enumerate() {
                    let c = if i < len as usize { c } else { b' ' };
                    w.put(
                        u64::from(icao_code(c as char).map_err(|_| Error::BadChar(c))?),
                        6,
                    );
                }
                w.finish();
            }
            Group::PredictedRoute { next, eta_s, then } => {
                let mut w = BitWriter::new(out);
                put_waypoint(&mut w, &next)?;
                w.put(unsigned(eta_s as f64, 1.0, 14, Item::TimeToGo)?, 14);
                put_waypoint(&mut w, &then)?;
                w.finish();
            }
            Group::EarthReference {
                track_deg: a,
                valid,
                gs_kt: s,
                vs_fpm,
            }
            | Group::AirReference {
                heading_deg: a,
                valid,
                mach: s,
                vs_fpm,
            } => {
                let lsb = if self.tag() == 14 { 0.5 } else { 0.0005 };
                let mut w = BitWriter::new(out);
                w.put(u64::from(!valid), 1);
                put_angle(&mut w, a, 12)?;
                w.put(unsigned(s, lsb, 13, Item::Speed)?, 13);
                w.put_signed(signed(vs_fpm, 16.0, 12, Item::VerticalRate)?, 12);
                w.finish();
            }
            Group::Meteo {
                wind_kt,
                valid,
                wind_dir_deg,
                temp_c,
            } => {
                let mut w = BitWriter::new(out);
                w.put(unsigned(wind_kt, 0.5, 9, Item::WindSpeed)?, 9);
                w.put(u64::from(!valid), 1);
                put_angle(&mut w, wind_dir_deg, 9)?;
                w.put_signed(signed(temp_c, 0.25, 12, Item::Temperature)?, 12);
                w.finish();
            }
            Group::Airframe { icao } => {
                if icao > 0xFF_FFFF {
                    return Err(Error::BadField(Item::IcaoAddress));
                }
                out.extend_from_slice(&icao.to_be_bytes()[1..]);
            }
            Group::IntermediateIntent {
                distance_nm,
                valid,
                track_deg,
                alt_ft,
                eta_s,
            } => {
                let mut w = BitWriter::new(out);
                w.put(unsigned(distance_nm, 0.125, 16, Item::Distance)?, 16);
                w.put(u64::from(!valid), 1);
                put_angle(&mut w, track_deg, 12)?;
                w.put_signed(signed(alt_ft, 4.0, 16, Item::Altitude)?, 16);
                w.put(unsigned(eta_s as f64, 1.0, 14, Item::ProjectedTime)?, 14);
                w.finish();
            }
            Group::FixedIntent { point, eta_s } => {
                let mut w = BitWriter::new(out);
                put_waypoint(&mut w, &point)?;
                w.put(unsigned(eta_s as f64, 1.0, 14, Item::ProjectedTime)?, 14);
                w.finish();
            }
        }
        Ok(())
    }

    /// A flight id group from text (at most 8 letters, digits and spaces).
    pub fn flight_id(text: &str) -> Result<Group> {
        let b = text.trim_end().as_bytes();
        if b.len() > 8 {
            return Err(Error::TooLong(8));
        }
        let mut id = [b' '; 8];
        for (d, &c) in id.iter_mut().zip(b) {
            icao_code(c as char).map_err(|_| Error::BadChar(c))?;
            *d = c.to_ascii_uppercase();
        }
        Ok(Group::FlightId {
            id,
            len: b.len() as u8,
        })
    }

    /// The flight id text of a flight id group.
    pub fn flight_id_text(&self) -> Option<&str> {
        match self {
            Group::FlightId { id, len } => std::str::from_utf8(&id[..*len as usize]).ok(),
            _ => None,
        }
    }
}

/// An uplink request from the ground station.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Tag 1: cancel all contracts and end the connection.
    CancelAll,
    /// Tag 2: cancel one contract.
    Cancel {
        /// The contract number.
        contract: u8,
    },
    /// Tag 6: cancel the emergency mode.
    CancelEmergency,
    /// Tag 7: a periodic contract.
    Periodic {
        /// The contract number.
        contract: u8,
    },
    /// Tag 8: an event contract.
    Event {
        /// The contract number.
        contract: u8,
    },
    /// Tag 9: an emergency periodic contract.
    EmergencyPeriodic {
        /// The contract number.
        contract: u8,
    },
}

/// A group of an uplink contract request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestGroup {
    /// Tag 10: report when the lateral deviation passes this threshold (0.125 nm units).
    LateralDeviation {
        /// The raw threshold.
        raw: u8,
    },
    /// Tag 11: the reporting interval: `rate` times 1, 8, 64 or 512 seconds by `scaling`.
    ReportingInterval {
        /// The scaling factor, 0 to 3.
        scaling: u8,
        /// The rate, 0 to 63.
        rate: u8,
    },
    /// Tags 12 to 17: include this group in every `modulus`-th report. The tags are 12 flight id, 13 predicted route,
    /// 14 earth reference, 15 air reference, 16 meteorological data and 17 airframe id.
    Modulus {
        /// The tag of the requested group.
        tag: u8,
        /// The modulus.
        modulus: u8,
    },
    /// Tag 18: report when the vertical rate passes this threshold (signed, 64 ft/min units).
    VerticalRateChange {
        /// The raw threshold.
        raw: u8,
    },
    /// Tag 19: report when the altitude leaves this range (4 ft units).
    AltitudeRange {
        /// The raw ceiling.
        ceiling: u16,
        /// The raw floor.
        floor: u16,
    },
    /// Tag 20: report at each waypoint change.
    WaypointChange,
    /// Tag 21: include the projected intent.
    AircraftIntent {
        /// The modulus.
        modulus: u8,
        /// The raw projection time.
        projection: u8,
    },
}

impl RequestGroup {
    /// The reporting interval in seconds, for a reporting interval group.
    pub fn interval_s(&self) -> Option<u32> {
        match *self {
            RequestGroup::ReportingInterval { scaling, rate } => {
                Some(u32::from(rate) * [1, 8, 64, 512][scaling as usize & 3])
            }
            _ => None,
        }
    }

    fn tag(&self) -> u8 {
        match *self {
            RequestGroup::LateralDeviation { .. } => 10,
            RequestGroup::ReportingInterval { .. } => 11,
            RequestGroup::Modulus { tag, .. } => tag,
            RequestGroup::VerticalRateChange { .. } => 18,
            RequestGroup::AltitudeRange { .. } => 19,
            RequestGroup::WaypointChange => 20,
            RequestGroup::AircraftIntent { .. } => 21,
        }
    }

    /// Appends the tag and the group to `out`.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<()> {
        out.push(self.tag());
        match *self {
            RequestGroup::LateralDeviation { raw } | RequestGroup::VerticalRateChange { raw } => {
                out.push(raw)
            }
            RequestGroup::ReportingInterval { scaling, rate } => {
                if scaling > 3 || rate > 63 {
                    out.pop();
                    return Err(Error::BadField(Item::ReportingInterval));
                }
                out.push(scaling << 6 | rate);
            }
            RequestGroup::Modulus { tag, modulus } => {
                if !(12..=17).contains(&tag) {
                    out.pop();
                    return Err(Error::BadField(Item::ModulusTag));
                }
                out.push(modulus);
            }
            RequestGroup::AltitudeRange { ceiling, floor } => {
                out.extend_from_slice(&ceiling.to_be_bytes());
                out.extend_from_slice(&floor.to_be_bytes());
            }
            RequestGroup::WaypointChange => {}
            RequestGroup::AircraftIntent {
                modulus,
                projection,
            } => out.extend_from_slice(&[modulus, projection]),
        }
        Ok(())
    }
}

/// The groups of an uplink contract request, read one by one.
#[derive(Clone, Debug)]
pub struct RequestGroups<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Iterator for RequestGroups<'_> {
    type Item = Result<RequestGroup>;

    fn next(&mut self) -> Option<Result<RequestGroup>> {
        let tag = *self.data.get(self.pos)?;
        let b = &self.data[self.pos + 1..];
        let n = match tag {
            10..=18 => 1,
            19 => 4,
            20 => 0,
            21 => 2,
            _ => {
                self.pos = self.data.len();
                return Some(Err(Error::BadField(Item::AdscRequestTag)));
            }
        };
        let Some(b) = b.get(..n) else {
            self.pos = self.data.len();
            return Some(Err(Error::Short));
        };
        self.pos += 1 + n;
        Some(Ok(match tag {
            10 => RequestGroup::LateralDeviation { raw: b[0] },
            11 => RequestGroup::ReportingInterval {
                scaling: b[0] >> 6,
                rate: b[0] & 0x3F,
            },
            12..=17 => RequestGroup::Modulus { tag, modulus: b[0] },
            18 => RequestGroup::VerticalRateChange { raw: b[0] },
            19 => RequestGroup::AltitudeRange {
                ceiling: u16::from_be_bytes([b[0], b[1]]),
                floor: u16::from_be_bytes([b[2], b[3]]),
            },
            20 => RequestGroup::WaypointChange,
            _ => RequestGroup::AircraftIntent {
                modulus: b[0],
                projection: b[1],
            },
        }))
    }
}

/// Reads an uplink message: the request and its groups.
pub fn request(data: &[u8]) -> Result<(Request, RequestGroups<'_>)> {
    let tag = *data.first().ok_or(Error::Short)?;
    let contract = || data.get(1).copied().ok_or(Error::Short);
    let (req, start) = match tag {
        1 => (Request::CancelAll, 1),
        2 => (
            Request::Cancel {
                contract: contract()?,
            },
            2,
        ),
        6 => (Request::CancelEmergency, 1),
        7 => (
            Request::Periodic {
                contract: contract()?,
            },
            2,
        ),
        8 => (
            Request::Event {
                contract: contract()?,
            },
            2,
        ),
        9 => (
            Request::EmergencyPeriodic {
                contract: contract()?,
            },
            2,
        ),
        _ => return Err(Error::BadField(Item::AdscRequestTag)),
    };
    Ok((
        req,
        RequestGroups {
            data: &data[start..],
            pos: 0,
        },
    ))
}

impl Request {
    /// Appends the request's tag and contract number to `out` (the groups follow with [`RequestGroup::encode`]).
    pub fn encode(&self, out: &mut Vec<u8>) {
        match *self {
            Request::CancelAll => out.push(1),
            Request::Cancel { contract } => out.extend_from_slice(&[2, contract]),
            Request::CancelEmergency => out.push(6),
            Request::Periodic { contract } => out.extend_from_slice(&[7, contract]),
            Request::Event { contract } => out.extend_from_slice(&[8, contract]),
            Request::EmergencyPeriodic { contract } => out.extend_from_slice(&[9, contract]),
        }
    }
}

/// What a decoded ADS-C message holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// Downlink groups. `complete` is false when a group was short or had an unknown tag.
    Downlink {
        /// The groups read.
        groups: Vec<Group>,
        /// Every octet was read.
        complete: bool,
    },
    /// An uplink request and its groups.
    Uplink {
        /// The request.
        request: Request,
        /// The groups read.
        groups: Vec<RequestGroup>,
        /// Every octet was read.
        complete: bool,
    },
    /// A disconnect (`DIS`) with its reason code (the high nibble of the first octet).
    Disconnect {
        /// The reason code.
        reason: u8,
    },
}

/// A decoded ADS-C message (owned).
#[derive(Clone, Debug, PartialEq)]
pub struct Adsc {
    /// The ground address.
    pub ground: String,
    /// `ADS` or `DIS`.
    pub imi: Imi,
    /// The aircraft registration (the padding dots removed).
    pub aircraft: String,
    /// The envelope CRC is good.
    pub crc_ok: bool,
    /// The message.
    pub content: Content,
}

/// Decodes the first ADS-C envelope (`ADS` or `DIS`) in an ACARS application text. `downlink` is the direction (labels B6 and
/// H1 downlink, A6 uplink). `registration` is the block's registration (it can be empty). Returns `None` when the text holds no
/// ADS-C envelope.
pub fn decode(text: &str, downlink: bool, registration: &str) -> Option<Adsc> {
    let e = arinc622::find(text, registration).filter(|e| !e.imi.is_cpdlc())?;
    let mut bin = vec![0u8; e.hex.len() / 2];
    let data = e.binary(&mut bin).ok()?;
    let content = if e.imi == Imi::Dis {
        Content::Disconnect {
            reason: data.first().map_or(0, |b| b >> 4),
        }
    } else if downlink {
        let mut complete = true;
        let groups = groups(data)
            .filter_map(|g| g.map_err(|_| complete = false).ok())
            .collect();
        Content::Downlink { groups, complete }
    } else {
        let (request, it) = request(data).ok()?;
        let mut complete = true;
        let groups = it
            .filter_map(|g| g.map_err(|_| complete = false).ok())
            .collect();
        Content::Uplink {
            request,
            groups,
            complete,
        }
    };
    Some(Adsc {
        ground: e.ground.to_owned(),
        imi: e.imi,
        aircraft: e.address.trim_start_matches('.').to_owned(),
        crc_ok: e.crc_ok,
        content,
    })
}

impl Adsc {
    /// The downlink groups (empty for an uplink or a disconnect).
    pub fn groups(&self) -> &[Group] {
        match &self.content {
            Content::Downlink { groups, .. } => groups,
            _ => &[],
        }
    }

    /// The first position report, when the CRC is good.
    pub fn report(&self) -> Option<&Report> {
        if !self.crc_ok {
            return None;
        }
        self.groups().iter().find_map(|g| match g {
            Group::Report(r) => Some(r),
            _ => None,
        })
    }

    /// The position of the first report, when the CRC is good and the position is sane ([`crate::textpos::sane`]). The height
    /// is not in it: see [`Report::alt_ft`].
    pub fn position(&self) -> Option<Position> {
        let r = self.report()?;
        crate::textpos::sane(r.lat_deg, r.lon_deg)
    }

    /// The earth reference (track, ground speed, vertical rate), when present and valid.
    pub fn earth_reference(&self) -> Option<(f64, f64, f64)> {
        self.groups().iter().find_map(|g| match *g {
            Group::EarthReference {
                track_deg,
                valid: true,
                gs_kt,
                vs_fpm,
            } => Some((track_deg, gs_kt, vs_fpm)),
            _ => None,
        })
    }
}

//! Mic-E (APRS 1.0.1 chapter 10): a position, course and speed packed into the destination address and the first nine bytes of
//! the information field, with a message code, optional telemetry, altitude and the radio's type code.

use super::position::{base91, take_altitude, take_dao};
use super::{text, Symbol};
use crate::Part;
use crate::{Error, Result};
use sdrkit_util_geo::Position;
use std::borrow::Cow;

/// The Mic-E message code from the three message bits of the destination (chapter 10, "Mic-E message codes").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MicEMessage {
    /// All three bits 0.
    Emergency,
    /// A standard message M0 to M6 (`0` is M0 "Off Duty").
    Standard(u8),
    /// A custom message C0 to C6.
    Custom(u8),
    /// The bits mix standard and custom encodings.
    Unknown,
}

/// The names of the standard messages M0 to M6.
pub const STANDARD_MESSAGES: [&str; 7] = [
    "Off Duty",
    "En Route",
    "In Service",
    "Returning",
    "Committed",
    "Special",
    "Priority",
];

impl MicEMessage {
    /// The message as text: "Emergency", a standard name, "Custom-0" to "Custom-6", or "Unknown".
    pub fn name(&self) -> String {
        match *self {
            MicEMessage::Emergency => "Emergency".into(),
            MicEMessage::Standard(n) => STANDARD_MESSAGES[n as usize % 7].into(),
            MicEMessage::Custom(n) => format!("Custom-{n}"),
            MicEMessage::Unknown => "Unknown".into(),
        }
    }
}

/// A decoded Mic-E report.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MicE {
    /// Latitude and longitude (`alt_m` is 0).
    pub position: Position,
    /// Position ambiguity, 0 to 4. If it is above 0, the position is the centre of the box left blank.
    pub ambiguity: u8,
    /// The map symbol.
    pub symbol: Symbol,
    /// Speed, knots.
    pub speed_kn: f64,
    /// Course, degrees (0 is "not known").
    pub course_deg: u16,
    /// The message code.
    pub message: MicEMessage,
    /// The data type byte: `` ` `` is a current GPS fix, `'` an old one (0x1C and 0x1D are the Rev 0 beta units).
    pub data_type: u8,
    /// Altitude, metres.
    pub altitude_m: Option<f64>,
    /// Telemetry channels 1 to 5 (`None` for channels not sent). The flags read here are `` ` `` for channels 1 and 3 and `'`
    /// for channels 1 to 5.
    pub telemetry: Option<[Option<u8>; 5]>,
    /// The radio, from the type code characters (APRS 1.2 "Mic-E types").
    pub device: Option<Cow<'static, str>>,
    /// The datum of a DAO extension in the comment.
    pub datum: Option<char>,
    /// The comment.
    pub comment: String,
}

/// One destination character: (digit or `None` for a blank, message bit, custom, north/+100/west flag).
fn dest_char(c: u8, i: usize) -> Option<(Option<u8>, u8, bool, bool)> {
    Some(match c {
        b'0'..=b'9' => (Some(c - b'0'), 0, false, false),
        b'A'..=b'J' if i < 3 => (Some(c - b'A'), 1, true, false),
        b'K' if i < 3 => (None, 1, true, false),
        b'L' => (None, 0, false, false),
        b'P'..=b'Y' => (Some(c - b'P'), 1, false, true),
        b'Z' => (None, 1, false, true),
        _ => return None,
    })
}

/// Hex digits to values.
fn hex_values(b: &[u8], out: &mut [Option<u8>]) -> bool {
    for (o, p) in out.iter_mut().zip(b.chunks(2)) {
        let s = std::str::from_utf8(p).ok();
        match s.and_then(|s| u8::from_str_radix(s, 16).ok()) {
            Some(v) => *o = Some(v),
            None => return false,
        }
    }
    true
}

/// Type-code suffixes (APRS 1.2 "Mic-E types", the two characters at the end of the status text).
const SUFFIXES: [(&str, &str); 9] = [
    ("_ ", "Yaesu VX-8"),
    ("_\"", "Yaesu FTM-350"),
    ("_#", "Yaesu VX-8G"),
    ("_$", "Yaesu FT1D"),
    ("_%", "Yaesu FTM-400DR"),
    ("_(", "Yaesu FT2D"),
    ("_)", "Yaesu FTM-100D"),
    ("|3", "Byonics TinyTrack3"),
    ("|4", "Byonics TinyTrack4"),
];

/// Decodes a Mic-E report from the destination callsign (without SSID) and the information field.
pub fn decode(destination: &str, info: &[u8]) -> Result<MicE> {
    let d = destination.as_bytes();
    if d.len() < 6 {
        return Err(Error::Syntax(Part::MiceDestination));
    }
    if info.len() < 9 {
        return Err(Error::Short);
    }
    let mut digits = [0u8; 6];
    let mut blanks = 0u8;
    let mut bits = [0u8; 3];
    let (mut custom, mut standard) = (false, false);
    let mut flags = [false; 6];
    for i in 0..6 {
        let (dig, bit, cus, flag) =
            dest_char(d[i], i).ok_or(Error::Syntax(Part::MiceDestination))?;
        match dig {
            Some(v) => {
                if blanks > 0 {
                    // A blank must not stand left of a digit.
                    return Err(Error::Syntax(Part::MiceAmbiguity));
                }
                digits[i] = v;
            }
            None => blanks += 1,
        }
        flags[i] = flag;
        if i < 3 {
            bits[i] = bit;
            if bit == 1 {
                if cus {
                    custom = true;
                } else {
                    standard = true;
                }
            }
        }
    }
    if blanks > 4 {
        return Err(Error::Syntax(Part::MiceAmbiguity));
    }
    let (north, offset100, west) = (flags[3], flags[4], flags[5]);
    let box_min = [0.01, 0.1, 1.0, 10.0, 60.0][blanks as usize];
    let half = if blanks > 0 { box_min / 2.0 } else { 0.0 };
    let lat_deg = (digits[0] * 10 + digits[1]) as f64;
    let lat_min = (digits[2] * 10 + digits[3]) as f64 + (digits[4] * 10 + digits[5]) as f64 / 100.0;
    if lat_deg > 90.0 || lat_min >= 60.0 {
        return Err(Error::Syntax(Part::MiceLatitude));
    }
    let mut lat = lat_deg + (lat_min + half) / 60.0;
    let v = |k: usize| (info[k] as i32) - 28;
    let mut deg = v(1);
    if offset100 {
        deg += 100;
    }
    if (180..=189).contains(&deg) {
        deg -= 80;
    } else if (190..=199).contains(&deg) {
        deg -= 190;
    }
    let mut min = v(2);
    if min >= 60 {
        min -= 60;
    }
    let hun = v(3);
    if !(0..=179).contains(&deg) || !(0..=59).contains(&min) || !(0..=99).contains(&hun) {
        return Err(Error::Syntax(Part::MiceLongitude));
    }
    let lon_min = min as f64 + hun as f64 / 100.0;
    let lon_box = if blanks > 0 {
        (lon_min / box_min).floor() * box_min
    } else {
        lon_min
    };
    let mut lon = deg as f64 + (lon_box + half) / 60.0;
    let (sp, dc, se) = (v(4), v(5), v(6));
    if sp < 0 || dc < 0 || se < 0 {
        return Err(Error::Syntax(Part::MiceSpeedCourse));
    }
    let mut speed = sp * 10 + dc / 10;
    let mut course = (dc % 10) * 100 + se;
    if speed >= 800 {
        speed -= 800;
    }
    if course >= 400 {
        course -= 400;
    }
    if !north {
        lat = -lat;
    }
    if west {
        lon = -lon;
    }
    let index = (bits[0] << 2) | (bits[1] << 1) | bits[2];
    let message = if index == 0 {
        MicEMessage::Emergency
    } else if custom && standard {
        MicEMessage::Unknown
    } else if custom {
        MicEMessage::Custom(7 - index)
    } else {
        MicEMessage::Standard(7 - index)
    };
    let mut m = MicE {
        position: Position {
            lat_deg: lat,
            lon_deg: lon,
            alt_m: 0.0,
        },
        ambiguity: blanks,
        symbol: Symbol {
            code: info[7] as char,
            table: info[8] as char,
        },
        speed_kn: speed as f64,
        course_deg: course as u16,
        message,
        data_type: info[0],
        altitude_m: None,
        telemetry: None,
        device: None,
        datum: None,
        comment: String::new(),
    };
    let mut rest = &info[9..];
    // A telemetry flag (chapter 10, "Mic-E telemetry data"): ` and four hex digits (channels 1 and 3), ' and ten hex digits
    // (channels 1 to 5), or 0x1D and five bytes. Otherwise the first character is a type code.
    let hex_run = rest
        .iter()
        .skip(1)
        .take_while(|c| c.is_ascii_hexdigit())
        .count();
    match rest.first() {
        Some(b'`') if hex_run == 4 => {
            let mut v = [None; 2];
            if hex_values(&rest[1..5], &mut v) {
                m.telemetry = Some([v[0], None, v[1], None, None]);
                rest = &rest[5..];
            }
        }
        Some(b'\'') if hex_run == 10 => {
            let mut v = [None; 5];
            if hex_values(&rest[1..11], &mut v) {
                m.telemetry = Some(v);
                rest = &rest[11..];
            }
        }
        Some(0x1D) if rest.len() >= 6 => {
            m.telemetry = Some([
                Some(rest[1]),
                Some(rest[2]),
                Some(rest[3]),
                Some(rest[4]),
                Some(rest[5]),
            ]);
            rest = &rest[6..];
        }
        Some(b'>') => {
            m.device = Some(Cow::Borrowed("Kenwood TH-D7A"));
            rest = &rest[1..];
        }
        Some(b']') => {
            m.device = Some(Cow::Borrowed("Kenwood TM-D700"));
            rest = &rest[1..];
        }
        Some(b'`') | Some(b'\'') => rest = &rest[1..],
        _ => {}
    }
    // Altitude: three base-91 characters and `}`, metres above -10 km.
    if rest.len() >= 4 && rest[3] == b'}' {
        if let Some(a) = base91(&rest[..3]) {
            m.altitude_m = Some(a as f64 - 10_000.0);
            rest = &rest[4..];
        }
    }
    let mut comment = text(rest);
    for (suffix, dev) in SUFFIXES {
        if comment.ends_with(suffix) {
            m.device = Some(Cow::Borrowed(dev));
            comment.truncate(comment.len() - suffix.len());
            break;
        }
    }
    if comment.ends_with('=') {
        let newer = match m.device.as_deref() {
            Some("Kenwood TM-D700") => Some("Kenwood TM-D710"),
            Some("Kenwood TH-D7A") => Some("Kenwood TH-D72"),
            _ => None,
        };
        if let Some(n) = newer {
            m.device = Some(Cow::Borrowed(n));
            comment.pop();
        }
    }
    if m.altitude_m.is_none() {
        m.altitude_m = take_altitude(&mut comment);
    }
    if let Some((datum, dlat, dlon)) = take_dao(&mut comment) {
        m.datum = Some(datum);
        if m.ambiguity == 0 {
            let s = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
            m.position.lat_deg += s(m.position.lat_deg) * dlat / 60.0;
            m.position.lon_deg += s(m.position.lon_deg) * dlon / 60.0;
        }
    }
    m.comment = comment;
    Ok(m)
}

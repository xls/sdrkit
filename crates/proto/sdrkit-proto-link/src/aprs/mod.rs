//! The APRS information field: positions, Mic-E, objects, items, messages, telemetry, weather, status and the other data
//! types of chapter 5.
//!
//! Standard: APRS 1.0.1 (2000), APRS 1.1 and 1.2 addenda

pub mod message;
pub mod mice;
pub mod position;
pub mod telemetry;
pub mod weather;

pub use message::{Content, Message};
pub use mice::{MicE, MicEMessage};
pub use position::{encode_compressed, encode_uncompressed, Bearing, Extension, Located, Station};
pub use telemetry::{Telemetry, TelemetryMeta};
pub use weather::Weather;

use crate::ax25::{Frame, Tnc2};
use crate::Part;
use crate::{Error, Result};
use std::fmt;

/// A map symbol: the table (`/` primary, `\` alternate, or an overlay character) and the code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Symbol {
    /// The table identifier or overlay.
    pub table: char,
    /// The symbol code.
    pub code: char,
}

/// A time stamp (chapter 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Timestamp {
    /// `DDHHMMz` (UTC) or `DDHHMM/` (the sender's local time).
    DayHourMinute {
        /// Day of the month.
        day: u8,
        /// Hour.
        hour: u8,
        /// Minute.
        minute: u8,
        /// UTC (`z`). False for local time (`/`).
        utc: bool,
    },
    /// `HHMMSSh`, UTC.
    HourMinuteSecond {
        /// Hour.
        hour: u8,
        /// Minute.
        minute: u8,
        /// Second.
        second: u8,
    },
    /// `MMDDHHMM`, UTC (positionless weather only).
    MonthDayHourMinute {
        /// Month.
        month: u8,
        /// Day of the month.
        day: u8,
        /// Hour.
        hour: u8,
        /// Minute.
        minute: u8,
    },
}

fn two(b: &[u8]) -> u8 {
    (b[0] - b'0') * 10 + (b[1] - b'0')
}

impl Timestamp {
    /// Reads a 7-character time stamp at the start of `b`.
    pub(crate) fn parse7(b: &[u8]) -> Option<Timestamp> {
        if b.len() < 7 || !b[..6].iter().all(u8::is_ascii_digit) {
            return None;
        }
        let (a, c, d) = (two(&b[0..2]), two(&b[2..4]), two(&b[4..6]));
        let t = match b[6] {
            b'z' | b'/' => Timestamp::DayHourMinute {
                day: a,
                hour: c,
                minute: d,
                utc: b[6] == b'z',
            },
            b'h' => Timestamp::HourMinuteSecond {
                hour: a,
                minute: c,
                second: d,
            },
            _ => return None,
        };
        t.is_valid().then_some(t)
    }

    fn is_valid(&self) -> bool {
        match *self {
            Timestamp::DayHourMinute {
                day, hour, minute, ..
            } => day <= 31 && hour < 24 && minute < 60,
            Timestamp::HourMinuteSecond {
                hour,
                minute,
                second,
            } => hour < 24 && minute < 60 && second < 61,
            Timestamp::MonthDayHourMinute {
                month,
                day,
                hour,
                minute,
            } => (1..=12).contains(&month) && day <= 31 && hour < 24 && minute < 60,
        }
    }
}

/// Whether an NMEA sentence carried a correct checksum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NmeaCheck {
    /// The `*hh` checksum is present and correct.
    Good,
    /// The `*hh` checksum is present and wrong. The position is not read.
    Bad,
    /// There is no checksum (old equipment). The position is read.
    Missing,
}

/// What a packet says. See the [module documentation](self) for the data type of each variant.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Body {
    /// A position report.
    Position {
        /// The time stamp (`/` and `@` reports).
        timestamp: Option<Timestamp>,
        /// The station can receive messages (`=` and `@`).
        messaging: bool,
        /// The position and what follows it.
        report: Located,
    },
    /// A Mic-E report.
    MicE(MicE),
    /// An object.
    Object {
        /// The name (up to nine characters, trailing blanks removed).
        name: String,
        /// False when the object is killed (`_`).
        live: bool,
        /// The time stamp.
        timestamp: Option<Timestamp>,
        /// The position and what follows it.
        report: Located,
    },
    /// An item.
    Item {
        /// The name (3 to 9 characters).
        name: String,
        /// False when the item is killed (`_`).
        live: bool,
        /// The position and what follows it.
        report: Located,
    },
    /// A status report.
    Status {
        /// The time stamp (`DDHHMMz` only).
        timestamp: Option<Timestamp>,
        /// A Maidenhead locator and symbol at the start of the status (`>IO91SX/G`).
        locator: Option<(String, Symbol)>,
        /// The status text.
        text: String,
    },
    /// A message, acknowledgement, bulletin or other addressed packet.
    Message(Message),
    /// Positionless weather.
    Weather {
        /// The time stamp `MMDDHHMM`.
        timestamp: Option<Timestamp>,
        /// The values.
        weather: Weather,
    },
    /// A telemetry report.
    Telemetry(Telemetry),
    /// A general query (`?APRS?`, `?IGATE?`, `?WX?`).
    Query {
        /// The query name, without the question marks.
        query: String,
        /// Text after the query (a footprint for `?APRS?`).
        target: String,
    },
    /// Station capabilities: `<IGATE,MSG_CNT=n,LOC_CNT=n`.
    Capabilities(Vec<(String, Option<String>)>),
    /// A raw NMEA sentence.
    RawGps {
        /// The sentence.
        sentence: String,
        /// The checksum result.
        check: NmeaCheck,
        /// The fix from a GGA, RMC or GLL sentence with a good or missing checksum.
        position: Option<sdrkit_util_geo::Position>,
        /// Speed over ground from RMC, knots.
        speed_kn: Option<f64>,
        /// Course from RMC, degrees.
        course_deg: Option<f64>,
        /// Altitude above mean sea level from GGA, metres.
        altitude_m: Option<f64>,
    },
    /// A third-party packet: the inner packet's TNC2 header, and its body.
    ThirdParty {
        /// The inner `SOURCE>DEST,PATH` text.
        header: String,
        /// The inner packet.
        packet: Box<Packet>,
    },
    /// User-defined data: `{` then the user identifier and the packet type characters.
    UserDefined {
        /// The user identifier.
        user: char,
        /// The user's packet type.
        kind: char,
        /// The rest.
        data: String,
    },
    /// A Maidenhead locator beacon `[IO91SX]comment`.
    Maidenhead {
        /// The locator.
        locator: String,
        /// The comment.
        comment: String,
    },
    /// Anything else (test data, Peet Bros and Ultimeter raw weather, data types the parser does not know).
    Other(String),
}

/// A parsed information field.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Packet {
    /// The data type identifier: the first byte of the information field.
    pub data_type: u8,
    /// What the packet says.
    pub body: Body,
}

/// Text from bytes: invalid UTF-8 and control characters become U+FFFD.
pub(crate) fn text(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len());
    for chunk in b.utf8_chunks() {
        for c in chunk.valid().chars() {
            s.push(if c.is_control() { '\u{FFFD}' } else { c });
        }
        if !chunk.invalid().is_empty() {
            s.push('\u{FFFD}');
        }
    }
    s
}

/// The position at the start of `b` (uncompressed, then compressed) and the bytes after it.
fn located(b: &[u8]) -> Option<(Located, &[u8])> {
    if let Some(l) = position::uncompressed(b) {
        return Some((l, &b[19..]));
    }
    position::compressed(b).map(|l| (l, &b[13..]))
}

fn located_with_rest(b: &[u8]) -> Option<Located> {
    let (mut l, rest) = located(b)?;
    position::after(rest, &mut l);
    Some(l)
}

/// The most nested third-party packets read.
const MAX_DEPTH: u32 = 4;

/// Reads an information field. `destination` is the AX.25 destination callsign without its SSID (Mic-E reads it).
///
/// ```
/// use sdrkit_proto_link::aprs::{self, Body};
/// let p = aprs::parse(b"!4903.50N/07201.75W-Test 001234", "APRS").unwrap();
/// let Body::Position { report, .. } = &p.body else { panic!() };
/// assert!((report.position.lat_deg - 49.058333).abs() < 1e-6);
/// assert!((report.position.lon_deg + 72.029167).abs() < 1e-6);
/// assert_eq!(report.comment, "Test 001234");
/// ```
pub fn parse(info: &[u8], destination: &str) -> Result<Packet> {
    parse_depth(info, destination, 0)
}

/// Reads the information field of an AX.25 frame.
pub fn parse_frame(frame: &Frame<'_>) -> Result<Packet> {
    parse(frame.info, frame.destination().call.as_str())
}

/// Reads the information field of a TNC2 line (from APRS-IS, a TNC monitor or a log).
pub fn parse_tnc2(line: &str) -> Result<Packet> {
    let t = Tnc2::parse(line)?;
    let dest = t.destination.split('-').next().unwrap_or("");
    parse(t.info.as_bytes(), dest)
}

fn parse_depth(info: &[u8], destination: &str, depth: u32) -> Result<Packet> {
    let b = info;
    let &t = b.first().ok_or(Error::Short)?;
    let body = match t {
        b'!' | b'=' | b'/' | b'@' => {
            let mut rest = &b[1..];
            let mut timestamp = None;
            if matches!(t, b'/' | b'@') {
                timestamp = Some(Timestamp::parse7(rest).ok_or(Error::Syntax(Part::TimeStamp))?);
                rest = &rest[7..];
            }
            match located_with_rest(rest) {
                Some(report) => Body::Position {
                    timestamp,
                    messaging: matches!(t, b'=' | b'@'),
                    report,
                },
                // A `!` that does not start a position is other text (a "!" can follow up to 40 characters).
                None if t == b'!' => scan_for_position(b),
                None => return Err(Error::Syntax(Part::Position)),
            }
        }
        b'`' | b'\'' | 0x1C | 0x1D => Body::MicE(mice::decode(destination, b)?),
        b';' => {
            if b.len() < 11 || !matches!(b[10], b'*' | b'_') {
                return Err(Error::Syntax(Part::ObjectName));
            }
            let name = text(&b[1..10]).trim_end().to_string();
            let mut rest = &b[11..];
            let timestamp = Timestamp::parse7(rest);
            if timestamp.is_some() {
                rest = &rest[7..];
            }
            Body::Object {
                name,
                live: b[10] == b'*',
                timestamp,
                report: located_with_rest(rest).ok_or(Error::Syntax(Part::ObjectPosition))?,
            }
        }
        b')' => {
            let end = b[1..b.len().min(11)]
                .iter()
                .position(|&c| c == b'!' || c == b'_')
                .filter(|&e| (3..=9).contains(&e))
                .ok_or(Error::Syntax(Part::ItemName))?;
            Body::Item {
                name: text(&b[1..1 + end]),
                live: b[1 + end] == b'!',
                report: located_with_rest(&b[2 + end..])
                    .ok_or(Error::Syntax(Part::ItemPosition))?,
            }
        }
        b'>' => status(&b[1..]),
        b':' => Body::Message(message::parse(b)?),
        b'_' => {
            if b.len() < 9 || !b[1..9].iter().all(u8::is_ascii_digit) {
                return Err(Error::Syntax(Part::WeatherTimeStamp));
            }
            let timestamp = Timestamp::MonthDayHourMinute {
                month: two(&b[1..3]),
                day: two(&b[3..5]),
                hour: two(&b[5..7]),
                minute: two(&b[7..9]),
            };
            let mut weather = Weather::default();
            weather.read_fields(&b[9..], true);
            Body::Weather {
                timestamp: timestamp.is_valid().then_some(timestamp),
                weather,
            }
        }
        b'T' if b.get(1) == Some(&b'#') => {
            Body::Telemetry(Telemetry::parse(&text(&b[2..])).ok_or(Error::Syntax(Part::Telemetry))?)
        }
        b'?' => {
            let s = text(&b[1..]);
            let (q, target) = match s.split_once('?') {
                Some((q, r)) => (q.to_string(), r.trim().to_string()),
                None => (s.trim().to_string(), String::new()),
            };
            Body::Query { query: q, target }
        }
        b'<' => Body::Capabilities(
            text(&b[1..])
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| match s.split_once('=') {
                    Some((k, v)) => (k.trim().to_string(), Some(v.trim().to_string())),
                    None => (s.trim().to_string(), None),
                })
                .collect(),
        ),
        b'$' => raw_gps(b),
        b'}' => {
            if depth >= MAX_DEPTH {
                return Err(Error::TooLong);
            }
            let inner = &b[1..];
            let colon = inner
                .iter()
                .position(|&c| c == b':')
                .ok_or(Error::Syntax(Part::ThirdPartyHeader))?;
            let header = text(&inner[..colon]);
            let (_, rest) = header
                .split_once('>')
                .ok_or(Error::Syntax(Part::ThirdPartyHeader))?;
            let dest = rest.split([',', '-']).next().unwrap_or("");
            let packet = parse_depth(&inner[colon + 1..], dest, depth + 1)?;
            Body::ThirdParty {
                header,
                packet: Box::new(packet),
            }
        }
        b'{' => {
            let s = text(&b[1..]);
            let mut cs = s.chars();
            let user = cs.next().ok_or(Error::Short)?;
            let kind = cs.next().ok_or(Error::Short)?;
            Body::UserDefined {
                user,
                kind,
                data: cs.as_str().to_string(),
            }
        }
        b'[' => {
            let s = text(&b[1..]);
            let (loc, comment) = s
                .split_once(']')
                .ok_or(Error::Syntax(Part::LocatorBeacon))?;
            sdrkit_util_geo::maidenhead::decode(loc).map_err(|_| Error::Syntax(Part::Locator))?;
            Body::Maidenhead {
                locator: loc.to_string(),
                comment: comment.trim().to_string(),
            }
        }
        _ => scan_for_position(b),
    };
    Ok(Packet { data_type: t, body })
}

/// A `!` position up to 40 characters into the field (chapter 5). Otherwise the field is [`Body::Other`].
fn scan_for_position(b: &[u8]) -> Body {
    for i in b
        .iter()
        .take(41)
        .enumerate()
        .filter(|(_, &c)| c == b'!')
        .map(|(i, _)| i)
    {
        if let Some(mut report) = located_with_rest(&b[i + 1..]) {
            let lead = text(&b[..i]);
            if !lead.trim().is_empty() {
                report.comment = if report.comment.is_empty() {
                    lead
                } else {
                    format!("{lead} {}", report.comment)
                };
            }
            return Body::Position {
                timestamp: None,
                messaging: false,
                report,
            };
        }
    }
    Body::Other(text(b))
}

fn status(b: &[u8]) -> Body {
    let mut rest = b;
    let mut timestamp = None;
    if rest.len() >= 7 && rest[6] == b'z' {
        if let Some(t) = Timestamp::parse7(rest) {
            timestamp = Some(t);
            rest = &rest[7..];
        }
    }
    let s = text(rest);
    // A Maidenhead locator (4 or 6 characters) and a symbol, then a blank or the end (chapter 16).
    let mut locator = None;
    let mut text_start = 0;
    for len in [6usize, 4] {
        if !s.is_char_boundary(len) || s.len() < len + 2 {
            continue;
        }
        let loc = &s[..len];
        let mut sym = s[len..].chars();
        let (Some(table), Some(code)) = (sym.next(), sym.next()) else {
            continue;
        };
        let ends = sym.next().is_none_or(|c| c == ' ');
        if matches!(table, '/' | '\\')
            && ends
            && loc.bytes().all(|c| c.is_ascii_alphanumeric())
            && sdrkit_util_geo::maidenhead::decode(loc).is_ok()
        {
            locator = Some((loc.to_string(), Symbol { table, code }));
            text_start = len + 1 + code.len_utf8();
            break;
        }
    }
    Body::Status {
        timestamp,
        locator,
        text: s[text_start..].trim_start().to_string(),
    }
}

fn raw_gps(b: &[u8]) -> Body {
    use sdrkit_util_geo::nmea::{self, ChecksumRule, Gga, Gll, Rmc};
    let sentence = text(b).trim_end().to_string();
    let mut body = Body::RawGps {
        sentence: sentence.clone(),
        check: NmeaCheck::Missing,
        position: None,
        speed_kn: None,
        course_deg: None,
        altitude_m: None,
    };
    let Body::RawGps {
        check,
        position,
        speed_kn,
        course_deg,
        altitude_m,
        ..
    } = &mut body
    else {
        return body;
    };
    let parsed = match nmea::parse_with(&sentence, ChecksumRule::IfPresent) {
        Ok(s) => s,
        Err(sdrkit_util_geo::Error::Checksum { .. }) => {
            *check = NmeaCheck::Bad;
            return body;
        }
        Err(_) => {
            *check = if sentence.contains('*') {
                NmeaCheck::Bad
            } else {
                NmeaCheck::Missing
            };
            return body;
        }
    };
    *check = if sentence.contains('*') {
        NmeaCheck::Good
    } else {
        NmeaCheck::Missing
    };
    match parsed.formatter() {
        "GGA" => {
            if let Ok(g) = Gga::from_sentence(&parsed) {
                *position = g
                    .position
                    .map(|p| sdrkit_util_geo::Position { alt_m: 0.0, ..p });
                *altitude_m = g.altitude_msl_m;
            }
        }
        "RMC" => {
            if let Ok(r) = Rmc::from_sentence(&parsed) {
                if r.valid {
                    *position = r.position;
                    *speed_kn = r.speed_kn;
                    *course_deg = r.course_deg;
                }
            }
        }
        "GLL" => {
            if let Ok(g) = Gll::from_sentence(&parsed) {
                if g.valid {
                    *position = g.position;
                }
            }
        }
        _ => {}
    }
    body
}

impl Body {
    /// A short name for the kind of packet: "Position", "Mic-E", "Object", "Item", "Status", "Message", "Weather", "Telemetry",
    /// "Query", "Capabilities", "Raw GPS", "Third-party", "User-defined", "Maidenhead" or "Other".
    pub fn kind(&self) -> &'static str {
        match self {
            Body::Position { report, .. } if report.weather.is_some() => "Weather",
            Body::Position { .. } => "Position",
            Body::MicE(_) => "Mic-E",
            Body::Object { .. } => "Object",
            Body::Item { .. } => "Item",
            Body::Status { .. } => "Status",
            Body::Message(_) => "Message",
            Body::Weather { .. } => "Weather",
            Body::Telemetry(_) => "Telemetry",
            Body::Query { .. } => "Query",
            Body::Capabilities(_) => "Capabilities",
            Body::RawGps { .. } => "Raw GPS",
            Body::ThirdParty { .. } => "Third-party",
            Body::UserDefined { .. } => "User-defined",
            Body::Maidenhead { .. } => "Maidenhead",
            Body::Other(_) => "Other",
        }
    }

    /// The position the packet provides, if any.
    pub fn position(&self) -> Option<sdrkit_util_geo::Position> {
        match self {
            Body::Position { report, .. }
            | Body::Object { report, .. }
            | Body::Item { report, .. } => Some(report.position),
            Body::MicE(m) => Some(m.position),
            Body::RawGps { position, .. } => *position,
            Body::ThirdParty { packet, .. } => packet.body.position(),
            Body::Status {
                locator: Some((l, _)),
                ..
            }
            | Body::Maidenhead { locator: l, .. } => sdrkit_util_geo::maidenhead::decode(l)
                .ok()
                .map(|s| s.centre()),
            _ => None,
        }
    }
}

fn fmt_located(f: &mut fmt::Formatter<'_>, l: &Located) -> fmt::Result {
    write!(
        f,
        "{:.4} {:.4} [{}{}]",
        l.position.lat_deg, l.position.lon_deg, l.symbol.table, l.symbol.code
    )?;
    if let (Some(c), Some(s)) = (l.course_deg, l.speed_kn) {
        write!(f, " {c} deg {s:.0} kn")?;
    }
    if let Some(a) = l.altitude_m {
        write!(f, " {a:.0} m")?;
    }
    if !l.comment.is_empty() {
        write!(f, " {}", l.comment)?;
    }
    Ok(())
}

impl fmt::Display for Packet {
    /// A one-line summary for logs and tables.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.body {
            Body::Position { report, .. } => fmt_located(f, report),
            Body::MicE(m) => {
                write!(
                    f,
                    "{:.4} {:.4} [{}{}] {} deg {:.0} kn {}",
                    m.position.lat_deg,
                    m.position.lon_deg,
                    m.symbol.table,
                    m.symbol.code,
                    m.course_deg,
                    m.speed_kn,
                    m.message.name()
                )?;
                if !m.comment.is_empty() {
                    write!(f, " {}", m.comment)?;
                }
                Ok(())
            }
            Body::Object { name, report, .. } | Body::Item { name, report, .. } => {
                write!(f, "{} {name} ", self.body.kind())?;
                fmt_located(f, report)
            }
            Body::Status { text, .. } => write!(f, "Status: {text}"),
            Body::Message(m) => match &m.content {
                Content::Text { text, id, .. } => {
                    write!(f, "Message to {}: {text}", m.addressee)?;
                    if let Some(i) = id {
                        write!(f, " {{{i}}}")?;
                    }
                    Ok(())
                }
                Content::Ack { id, .. } => write!(f, "Ack {id} to {}", m.addressee),
                Content::Rej { id } => write!(f, "Reject {id} to {}", m.addressee),
                Content::Bulletin { id, text, .. } => write!(f, "Bulletin {id}: {text}"),
                Content::Announcement { id, text } => write!(f, "Announcement {id}: {text}"),
                Content::Weather { text } => write!(f, "{}: {text}", m.addressee),
                Content::Telemetry(_) => write!(f, "Telemetry set-up for {}", m.addressee),
                Content::Query { query } => write!(f, "Query {query} to {}", m.addressee),
            },
            Body::Weather { .. } => f.write_str("Weather"),
            Body::Telemetry(t) => write!(f, "Telemetry {}", t.sequence),
            Body::Query { query, .. } => write!(f, "Query {query}"),
            Body::Capabilities(c) => write!(f, "Capabilities ({} items)", c.len()),
            Body::RawGps { sentence, .. } => write!(f, "Raw GPS {sentence}"),
            Body::ThirdParty { header, packet } => write!(f, "{header}: {packet}"),
            Body::UserDefined { user, kind, .. } => write!(f, "User-defined {user}{kind}"),
            Body::Maidenhead { locator, comment } => write!(f, "{locator} {comment}"),
            Body::Other(s) => f.write_str(s),
        }
    }
}

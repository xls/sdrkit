//! NMEA 0183 (IEC 61162-1) sentences: checksum, parsing without allocation, tag blocks, a builder, the GGA, RMC and GLL
//! sentences of position fixes, and AIVDM/AIVDO armouring for AIS ([`aivdm`]).
//!
//! A sentence is `$` (or `!` for encapsulated data such as AIS) followed by an address field (a two-letter talker and a
//! three-letter formatter, or `P` and a maker's code for proprietary sentences), comma-separated data fields, and `*hh`: the
//! exclusive OR of every character between the start mark and `*`, as two hexadecimal digits. With the closing CR LF a sentence
//! has at most 82 characters, so the text without CR LF has at most [`MAX_LEN`].
//!
//! A sentence may be preceded by a tag block (IEC 61162-450, NMEA 0183 4.10): `\s:station,c:1700000000*hh\`, which carries the
//! source and a time stamp; [`Sentence::tag`] reads its parameters.
//!
//! ```
//! use sdrkit_util_geo::nmea::{self, Gga};
//! let s = nmea::parse("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47").unwrap();
//! assert_eq!((s.talker(), s.formatter()), (Some("GP"), "GGA"));
//! let fix = Gga::from_sentence(&s).unwrap();
//! let p = fix.position.unwrap();
//! assert!((p.lat_deg - 48.1173).abs() < 1e-9 && fix.altitude_msl_m == Some(545.4));
//! // The position's height is above the ellipsoid: altitude plus geoid separation.
//! assert!((p.alt_m - (545.4 + 46.9)).abs() < 1e-9);
//! ```

pub mod aivdm;

use crate::coord::{format_ddmm, parse_ddmm, Axis};
use crate::error::finite;
use crate::time::Civil;
use crate::{Error, Position, Text};
use crate::{Quantity, SyntaxFault};
use std::fmt::Write;

/// The most characters of a sentence without its closing CR LF (82 with them).
pub const MAX_LEN: usize = 80;

/// The NMEA checksum of a sentence body: the XOR of its bytes.
///
/// ```
/// assert_eq!(sdrkit_util_geo::nmea::checksum(b"GPGLL,5300.97914,N,00259.98174,E,125926,A"), 0x28);
/// ```
pub fn checksum(body: &[u8]) -> u8 {
    body.iter().fold(0, |a, b| a ^ b)
}

/// Whether [`parse_with`] insists on the `*hh` checksum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChecksumRule {
    /// The checksum must be present and correct.
    Required,
    /// A sentence without a checksum is accepted; one that has a checksum must be correct. For old equipment that omits it.
    IfPresent,
}

/// A parsed sentence, borrowing the text it was parsed from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sentence<'a> {
    start: u8,
    address: &'a str,
    data: &'a str,
    has_data: bool,
    checksum: Option<u8>,
    tag_block: Option<&'a str>,
}

/// Parses a sentence, which must carry a correct checksum. Leading and trailing white space (including CR LF) is ignored.
pub fn parse(line: &str) -> Result<Sentence<'_>, Error> {
    parse_with(line, ChecksumRule::Required)
}

/// Parses a sentence with a choice of checksum rule.
pub fn parse_with(line: &str, rule: ChecksumRule) -> Result<Sentence<'_>, Error> {
    let mut s = line.trim();
    let mut tag_block = None;
    if let Some(rest) = s.strip_prefix('\\') {
        let end = rest
            .find('\\')
            .ok_or(Error::Syntax(SyntaxFault::UnterminatedTagBlock))?;
        let tag = &rest[..end];
        let (content, sum) = tag
            .rsplit_once('*')
            .ok_or(Error::Syntax(SyntaxFault::TagBlockChecksum))?;
        check_sum(content.as_bytes(), sum)?;
        tag_block = Some(content);
        s = &rest[end + 1..];
    }
    let b = s.as_bytes();
    let start = *b.first().ok_or(Error::Syntax(SyntaxFault::EmptySentence))?;
    if start != b'$' && start != b'!' {
        return Err(Error::Syntax(SyntaxFault::SentenceStart));
    }
    if let Some(at) = b.iter().position(|&c| !(0x20..=0x7e).contains(&c)) {
        return Err(Error::BadChar { at });
    }
    let (body, checksum) = match s[1..].rsplit_once('*') {
        Some((body, sum)) => (body, Some(check_sum(body.as_bytes(), sum)?)),
        None if rule == ChecksumRule::IfPresent => (&s[1..], None),
        None => return Err(Error::Syntax(SyntaxFault::NoChecksum)),
    };
    let (address, data, has_data) = match body.split_once(',') {
        Some((a, d)) => (a, d, true),
        None => (body, "", false),
    };
    if address.len() < 2
        || !address
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Err(Error::Syntax(SyntaxFault::AddressField));
    }
    Ok(Sentence {
        start,
        address,
        data,
        has_data,
        checksum,
        tag_block,
    })
}

fn check_sum(body: &[u8], sum: &str) -> Result<u8, Error> {
    let found = u8::from_str_radix(sum, 16)
        .ok()
        .filter(|_| sum.len() == 2)
        .ok_or(Error::Syntax(SyntaxFault::ChecksumDigits))?;
    let computed = checksum(body);
    if found != computed {
        return Err(Error::Checksum { found, computed });
    }
    Ok(found)
}

impl<'a> Sentence<'a> {
    /// The start mark: `$` or `!`.
    pub fn start(&self) -> char {
        self.start as char
    }

    /// The address field (`GPGGA`, `AIVDM`, `PGRMZ`).
    pub fn address(&self) -> &'a str {
        self.address
    }

    /// Whether it is a proprietary sentence (address starting with `P`).
    pub fn is_proprietary(&self) -> bool {
        self.address.starts_with('P')
    }

    /// The talker (`GP`, `GN`, `AI`), or `None` for a proprietary sentence.
    pub fn talker(&self) -> Option<&'a str> {
        if self.is_proprietary() || self.address.len() < 5 {
            None
        } else {
            Some(&self.address[..2])
        }
    }

    /// The sentence formatter: the last three characters of a standard address (`GGA`), or the whole address of a proprietary
    /// one.
    pub fn formatter(&self) -> &'a str {
        if self.is_proprietary() || self.address.len() < 5 {
            self.address
        } else {
            &self.address[2..]
        }
    }

    /// The data fields (after the address), in order. An empty sentence body has no fields.
    pub fn fields(&self) -> impl Iterator<Item = &'a str> + 'a {
        // "$XXABC*hh" has no fields; "$XXABC,*hh" has one empty field.
        let has = self.has_data;
        self.data.split(',').filter(move |_| has)
    }

    /// Data field `i` (0 is the first field after the address), or `None` when the sentence is shorter.
    pub fn field(&self, i: usize) -> Option<&'a str> {
        self.fields().nth(i)
    }

    /// The checksum carried, if any.
    pub fn checksum(&self) -> Option<u8> {
        self.checksum
    }

    /// The tag block's content without its checksum, if the line had one.
    pub fn tag_block(&self) -> Option<&'a str> {
        self.tag_block
    }

    /// The value of a tag block parameter (`s` source, `c` Unix time, `d` destination, `g` group, `n` line count, `r` relative
    /// time, `t` text).
    pub fn tag(&self, key: &str) -> Option<&'a str> {
        self.tag_block?.split(',').find_map(|kv| {
            kv.split_once(':')
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v)
        })
    }
}

/// Builds a sentence into a fixed buffer, adding the checksum. Fails with [`Error::TooLong`] beyond [`MAX_LEN`].
///
/// ```
/// let mut b = sdrkit_util_geo::nmea::Builder::new('$', "GPGLL").unwrap();
/// for f in ["5300.97914", "N", "00259.98174", "E", "125926", "A"] {
///     b.field(f).unwrap();
/// }
/// assert_eq!(b.finish().unwrap().as_str(), "$GPGLL,5300.97914,N,00259.98174,E,125926,A*28");
/// ```
#[derive(Clone, Debug)]
pub struct Builder {
    text: Text<MAX_LEN>,
}

impl Builder {
    /// Starts a sentence with `$` or `!` and an address of upper-case letters and digits.
    pub fn new(start: char, address: &str) -> Result<Builder, Error> {
        if start != '$' && start != '!' {
            return Err(Error::Syntax(SyntaxFault::SentenceStart));
        }
        if address.len() < 2
            || !address
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err(Error::Syntax(SyntaxFault::AddressField));
        }
        let mut text = Text::new();
        text.push(start)?;
        text.push_str(address)?;
        Ok(Builder { text })
    }

    /// Appends a field. Characters reserved by NMEA (`$ ! * , \ ^ ~`, CR, LF) and non-printable ones are refused.
    pub fn field(&mut self, value: &str) -> Result<&mut Builder, Error> {
        if let Some(at) = value
            .bytes()
            .position(|c| !(0x20..=0x7e).contains(&c) || b"$!*,\\^~".contains(&c))
        {
            return Err(Error::BadChar { at });
        }
        self.text.push(',')?;
        self.text.push_str(value)?;
        Ok(self)
    }

    /// Appends a formatted field: `b.field_fmt(format_args!("{:.1}", x))`.
    pub fn field_fmt(&mut self, args: std::fmt::Arguments<'_>) -> Result<&mut Builder, Error> {
        let mut t = Text::<MAX_LEN>::new();
        t.write_fmt(args).map_err(|_| Error::TooLong)?;
        self.field(&t)
    }

    /// Appends an optional number with `decimals` decimals, empty when `None`.
    pub fn number(&mut self, value: Option<f64>, decimals: usize) -> Result<&mut Builder, Error> {
        match value {
            Some(v) => {
                finite(&[v])?;
                self.field_fmt(format_args!("{v:.decimals$}"))
            }
            None => self.field(""),
        }
    }

    /// Adds `*hh` and returns the sentence (without CR LF).
    pub fn finish(mut self) -> Result<Text<MAX_LEN>, Error> {
        let sum = checksum(&self.text.as_bytes()[1..]);
        self.text
            .write_fmt(format_args!("*{sum:02X}"))
            .map_err(|_| Error::TooLong)?;
        Ok(self.text)
    }
}

/// A UTC time of day as NMEA writes it (`hhmmss.ss`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeOfDay {
    /// Hour.
    pub hour: u8,
    /// Minute.
    pub minute: u8,
    /// Seconds with fraction, 0 to below 61 (a leap second may show as 60).
    pub second: f64,
}

impl TimeOfDay {
    /// Seconds since midnight.
    pub fn seconds_of_day(&self) -> f64 {
        self.hour as f64 * 3600.0 + self.minute as f64 * 60.0 + self.second
    }
}

/// Parses an `hhmmss[.s…]` field; `None` for an empty field.
pub fn parse_time(field: &str) -> Result<Option<TimeOfDay>, Error> {
    if field.is_empty() {
        return Ok(None);
    }
    let b = field.as_bytes();
    if b.len() < 6
        || !b[..6].iter().all(u8::is_ascii_digit)
        || (b.len() > 6 && (b[6] != b'.' || !b[7..].iter().all(u8::is_ascii_digit)))
    {
        return Err(Error::Syntax(SyntaxFault::TimeField));
    }
    let n = |i: usize| (b[i] - b'0') * 10 + (b[i + 1] - b'0');
    let second: f64 = field[4..]
        .parse()
        .map_err(|_| Error::Syntax(SyntaxFault::Seconds))?;
    let t = TimeOfDay {
        hour: n(0),
        minute: n(2),
        second,
    };
    if t.hour > 23 || t.minute > 59 || t.second >= 61.0 {
        return Err(Error::OutOfRange(Quantity::TimeOfDay));
    }
    Ok(Some(t))
}

/// Parses a `ddmmyy` date field; `None` for an empty field. Two-digit years 80 to 99 are 1980 to 1999 (the GPS era), 00 to 79 are
/// 2000 to 2079.
pub fn parse_date(field: &str) -> Result<Option<(i32, u8, u8)>, Error> {
    if field.is_empty() {
        return Ok(None);
    }
    let b = field.as_bytes();
    if b.len() != 6 || !b.iter().all(u8::is_ascii_digit) {
        return Err(Error::Syntax(SyntaxFault::DateField));
    }
    let n = |i: usize| (b[i] - b'0') * 10 + (b[i + 1] - b'0');
    let (d, m, yy) = (n(0), n(2), n(4) as i32);
    let y = if yy >= 80 { 1900 + yy } else { 2000 + yy };
    crate::time::days_from_civil(y, m, d)?;
    Ok(Some((y, m, d)))
}

/// The `hhmmss.ss` field of a Unix time with `decimals` decimals (0 to 3), rounding with carry.
pub fn time_field(unix: f64, decimals: u8) -> Result<Text<16>, Error> {
    if decimals > 3 {
        return Err(Error::OutOfRange(Quantity::Decimals));
    }
    let scale = 10f64.powi(decimals as i32);
    let rounded = (unix * scale).round() / scale;
    let c = Civil::from_unix_f64(rounded)?;
    let mut t = Text::new();
    write!(t, "{:02}{:02}{:02}", c.hour, c.minute, c.second).map_err(|_| Error::TooLong)?;
    if decimals > 0 {
        let frac = ((c.nanos as f64 / 1e9) * scale).round() as u32;
        let d = decimals as usize;
        write!(t, ".{frac:0d$}").map_err(|_| Error::TooLong)?;
    }
    Ok(t)
}

/// The `ddmmyy` field of a Unix time.
pub fn date_field(unix: f64) -> Result<Text<8>, Error> {
    let c = Civil::from_unix_f64(unix)?;
    let mut t = Text::new();
    write!(t, "{:02}{:02}{:02}", c.day, c.month, c.year.rem_euclid(100))
        .map_err(|_| Error::TooLong)?;
    Ok(t)
}

fn opt_f64(field: Option<&str>) -> Result<Option<f64>, Error> {
    match field {
        None | Some("") => Ok(None),
        Some(f) => {
            let v: f64 = f
                .parse()
                .map_err(|_| Error::Syntax(SyntaxFault::NumberField))?;
            finite(&[v])?;
            Ok(Some(v))
        }
    }
}

fn lat_lon(
    lat: Option<&str>,
    ns: Option<&str>,
    lon: Option<&str>,
    ew: Option<&str>,
) -> Result<Option<(f64, f64)>, Error> {
    match (lat, ns, lon, ew) {
        (Some(la), Some(n), Some(lo), Some(e)) if !la.is_empty() && !lo.is_empty() => Ok(Some((
            parse_ddmm(la, n, Axis::Latitude)?,
            parse_ddmm(lo, e, Axis::Longitude)?,
        ))),
        _ => Ok(None),
    }
}

/// A GGA sentence: the fix of a GNSS receiver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gga {
    /// UTC time of the fix.
    pub time: Option<TimeOfDay>,
    /// Position; `alt_m` is the height above the ellipsoid (antenna altitude above mean sea level plus geoid separation, or the
    /// altitude alone when the separation is missing).
    pub position: Option<Position>,
    /// Fix quality: 0 invalid, 1 GNSS, 2 differential, 4 RTK fixed, 5 RTK float, 6 estimated.
    pub quality: u8,
    /// Satellites used.
    pub satellites: Option<u8>,
    /// Horizontal dilution of precision.
    pub hdop: Option<f64>,
    /// Antenna altitude above mean sea level, m.
    pub altitude_msl_m: Option<f64>,
    /// Geoid separation (geoid above the ellipsoid), m.
    pub geoid_separation_m: Option<f64>,
}

impl Gga {
    /// Reads a GGA sentence (any talker).
    pub fn from_sentence(s: &Sentence<'_>) -> Result<Gga, Error> {
        if s.formatter() != "GGA" {
            return Err(Error::Syntax(SyntaxFault::NotGga));
        }
        let f = |i| s.field(i);
        let quality: u8 = f(5).filter(|q| !q.is_empty()).map_or(Ok(0), |q| {
            q.parse()
                .map_err(|_| Error::Syntax(SyntaxFault::FixQuality))
        })?;
        let altitude_msl_m = opt_f64(f(8))?;
        let geoid_separation_m = opt_f64(f(10))?;
        let position = lat_lon(f(1), f(2), f(3), f(4))?.map(|(la, lo)| {
            Position::with_alt(
                la,
                lo,
                altitude_msl_m.unwrap_or(0.0) + geoid_separation_m.unwrap_or(0.0),
            )
        });
        Ok(Gga {
            time: parse_time(f(0).unwrap_or(""))?,
            position: if quality == 0 { None } else { position },
            quality,
            satellites: f(6)
                .filter(|v| !v.is_empty())
                .map(|v| {
                    v.parse()
                        .map_err(|_| Error::Syntax(SyntaxFault::Satellites))
                })
                .transpose()?,
            hdop: opt_f64(f(7))?,
            altitude_msl_m,
            geoid_separation_m,
        })
    }

    /// Writes a GGA sentence for `talker` (`GP`, `GN`) at Unix time `unix`, with 4 decimals of minutes (about 0.2 m).
    pub fn write(&self, talker: &str, unix: f64) -> Result<Text<MAX_LEN>, Error> {
        let mut addr = Text::<8>::new();
        addr.push_str(talker)?;
        addr.push_str("GGA")?;
        let mut b = Builder::new('$', &addr)?;
        b.field(&time_field(unix, 2)?)?;
        match self.position {
            Some(p) => {
                let (la, ns) = format_ddmm(p.lat_deg, Axis::Latitude, 4)?;
                let (lo, ew) = format_ddmm(p.lon_deg, Axis::Longitude, 4)?;
                b.field(&la)?
                    .field_fmt(format_args!("{ns}"))?
                    .field(&lo)?
                    .field_fmt(format_args!("{ew}"))?;
            }
            None => {
                b.field("")?.field("")?.field("")?.field("")?;
            }
        }
        b.field_fmt(format_args!("{}", self.quality))?;
        match self.satellites {
            Some(n) => b.field_fmt(format_args!("{n:02}"))?,
            None => b.field("")?,
        };
        b.number(self.hdop, 1)?;
        b.number(self.altitude_msl_m, 1)?.field("M")?;
        b.number(self.geoid_separation_m, 1)?.field("M")?;
        b.field("")?.field("")?;
        b.finish()
    }
}

/// An RMC sentence: the recommended minimum navigation data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rmc {
    /// UTC time.
    pub time: Option<TimeOfDay>,
    /// Status `A` (valid) rather than `V`.
    pub valid: bool,
    /// Position (height 0).
    pub position: Option<Position>,
    /// Speed over the ground, knots.
    pub speed_kn: Option<f64>,
    /// Course over the ground, degrees true.
    pub course_deg: Option<f64>,
    /// UTC date (year, month, day).
    pub date: Option<(i32, u8, u8)>,
    /// Magnetic variation, degrees, east positive.
    pub magnetic_variation_deg: Option<f64>,
    /// Mode indicator (NMEA 2.3 and later): `A` autonomous, `D` differential, `E` estimated, `N` not valid.
    pub mode: Option<char>,
}

impl Rmc {
    /// Reads an RMC sentence (any talker).
    pub fn from_sentence(s: &Sentence<'_>) -> Result<Rmc, Error> {
        if s.formatter() != "RMC" {
            return Err(Error::Syntax(SyntaxFault::NotRmc));
        }
        let f = |i| s.field(i);
        let valid = f(1) == Some("A");
        let variation = match (opt_f64(f(9))?, f(10)) {
            (Some(v), Some("W")) => Some(-v),
            (Some(v), Some("E")) => Some(v),
            (Some(_), _) => return Err(Error::Syntax(SyntaxFault::MagneticVariationDirection)),
            (None, _) => None,
        };
        Ok(Rmc {
            time: parse_time(f(0).unwrap_or(""))?,
            valid,
            position: lat_lon(f(2), f(3), f(4), f(5))?.map(|(la, lo)| Position::new(la, lo)),
            speed_kn: opt_f64(f(6))?,
            course_deg: opt_f64(f(7))?,
            date: parse_date(f(8).unwrap_or(""))?,
            magnetic_variation_deg: variation,
            mode: f(11).and_then(|m| m.chars().next()),
        })
    }

    /// The Unix time of the fix, when both the date and the time are present.
    pub fn unix(&self) -> Option<f64> {
        let (y, m, d) = self.date?;
        let t = self.time?;
        Some(crate::time::days_from_civil(y, m, d).ok()? as f64 * 86_400.0 + t.seconds_of_day())
    }

    /// Writes an RMC sentence for `talker` at Unix time `unix` (which returns both the time and the date fields).
    pub fn write(&self, talker: &str, unix: f64) -> Result<Text<MAX_LEN>, Error> {
        let mut addr = Text::<8>::new();
        addr.push_str(talker)?;
        addr.push_str("RMC")?;
        let mut b = Builder::new('$', &addr)?;
        b.field(&time_field(unix, 2)?)?;
        b.field(if self.valid { "A" } else { "V" })?;
        match self.position {
            Some(p) => {
                let (la, ns) = format_ddmm(p.lat_deg, Axis::Latitude, 4)?;
                let (lo, ew) = format_ddmm(p.lon_deg, Axis::Longitude, 4)?;
                b.field(&la)?
                    .field_fmt(format_args!("{ns}"))?
                    .field(&lo)?
                    .field_fmt(format_args!("{ew}"))?;
            }
            None => {
                b.field("")?.field("")?.field("")?.field("")?;
            }
        }
        b.number(self.speed_kn, 1)?.number(self.course_deg, 1)?;
        // The date of the rounded time, so 23:59:59.996 writes the next day's date with 00:00:00.00.
        b.field(&date_field((unix * 100.0).round() / 100.0)?)?;
        match self.magnetic_variation_deg {
            Some(v) => b
                .number(Some(v.abs()), 1)?
                .field(if v < 0.0 { "W" } else { "E" })?,
            None => b.field("")?.field("")?,
        };
        if let Some(m) = self.mode {
            b.field_fmt(format_args!("{m}"))?;
        }
        b.finish()
    }
}

/// A GLL sentence: position, time and status.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gll {
    /// Position (height 0).
    pub position: Option<Position>,
    /// UTC time.
    pub time: Option<TimeOfDay>,
    /// Status `A` rather than `V`.
    pub valid: bool,
}

impl Gll {
    /// Reads a GLL sentence (any talker).
    pub fn from_sentence(s: &Sentence<'_>) -> Result<Gll, Error> {
        if s.formatter() != "GLL" {
            return Err(Error::Syntax(SyntaxFault::NotGll));
        }
        let f = |i| s.field(i);
        Ok(Gll {
            position: lat_lon(f(0), f(1), f(2), f(3))?.map(|(la, lo)| Position::new(la, lo)),
            time: parse_time(f(4).unwrap_or(""))?,
            valid: f(5) == Some("A"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_sentences_parse_and_check() {
        // The GLL example of the NMEA 0183 documentation, and the widely published GGA and RMC examples.
        let gll = parse("$GPGLL,5300.97914,N,00259.98174,E,125926,A*28").unwrap();
        let g = Gll::from_sentence(&gll).unwrap();
        assert!(g.valid && (g.position.unwrap().lat_deg - (53.0 + 0.97914 / 60.0)).abs() < 1e-12);
        assert_eq!(
            g.time.unwrap().seconds_of_day(),
            12.0 * 3600.0 + 59.0 * 60.0 + 26.0
        );
        let rmc =
            parse("$GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W*6A").unwrap();
        let r = Rmc::from_sentence(&rmc).unwrap();
        assert!(r.valid);
        assert_eq!(
            (r.speed_kn, r.course_deg, r.date, r.magnetic_variation_deg),
            (Some(22.4), Some(84.4), Some((1994, 3, 23)), Some(-3.1))
        );
        assert_eq!(r.unix(), Some(764_426_119.0)); // 1994-03-23 12:35:19 UTC
        let gga =
            parse("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47").unwrap();
        let f = Gga::from_sentence(&gga).unwrap();
        assert_eq!(
            (f.quality, f.satellites, f.hdop, f.geoid_separation_m),
            (1, Some(8), Some(0.9), Some(46.9))
        );
        assert!((f.position.unwrap().alt_m - 592.3).abs() < 1e-9);
        assert!(Rmc::from_sentence(&gga).is_err());
        assert_eq!(gga.field(13), Some(""));
        assert_eq!(gga.field(14), None);
    }

    #[test]
    fn checksums_and_layout() {
        assert_eq!(
            parse("$GPGLL,5300.97914,N,00259.98174,E,125926,A*29"),
            Err(Error::Checksum {
                found: 0x29,
                computed: 0x28
            })
        );
        assert!(parse("$GPGLL,5300.97914,N,00259.98174,E,125926,A").is_err());
        assert!(parse_with(
            "$GPGLL,5300.97914,N,00259.98174,E,125926,A",
            ChecksumRule::IfPresent
        )
        .is_ok());
        assert!(parse_with("$GPGLL,1*00", ChecksumRule::IfPresent).is_err());
        assert!(parse("GPGLL*28").is_err());
        assert!(parse("$gpgll*00").is_err());
        assert!(parse("$GPGLL,\u{7f}*28").is_err());
        assert!(parse("$GPGLL*2").is_err());
        assert!(parse("").is_err());
        let p = parse("$PGRMZ,246,f,3*1B").unwrap();
        assert!(p.is_proprietary() && p.talker().is_none() && p.formatter() == "PGRMZ");
        let p = parse("$GPXYZ*").unwrap_err();
        assert!(matches!(p, Error::Syntax(_)));
        // A sentence with an empty body has no fields; a trailing comma produces one empty field.
        let empty = parse_with("$GPABC", ChecksumRule::IfPresent).unwrap();
        assert_eq!(empty.fields().count(), 0);
        let one = parse_with("$GPABC,", ChecksumRule::IfPresent).unwrap();
        assert_eq!(one.fields().count(), 1);
    }

    #[test]
    fn tag_blocks() {
        let line = "\\s:2573345,c:1671620143*0E\\!AIVDM,1,1,,A,15RTgt0PAso;90TKcjM8h6g208CQ,0*4A";
        let s = parse(line).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(s.tag("s"), Some("2573345"));
        assert_eq!(s.tag("c"), Some("1671620143"));
        assert_eq!(s.tag("x"), None);
        assert!(parse(
            "\\s:2573345,c:1671620143*0F\\!AIVDM,1,1,,A,15RTgt0PAso;90TKcjM8h6g208CQ,0*4A"
        )
        .is_err());
        assert!(parse("\\s:1*00!AIVDM").is_err());
    }

    #[test]
    fn building() {
        let fix = Gga {
            time: None,
            position: Some(Position::with_alt(48.1173, 11.516_666_666_7, 592.3)),
            quality: 1,
            satellites: Some(8),
            hdop: Some(0.9),
            altitude_msl_m: Some(545.4),
            geoid_separation_m: Some(46.9),
        };
        let t = crate::time::days_from_civil(1994, 3, 23).unwrap() as f64 * 86_400.0
            + 12.0 * 3600.0
            + 35.0 * 60.0
            + 19.0;
        let s = fix.write("GP", t).unwrap();
        assert_eq!(
            s.as_str(),
            "$GPGGA,123519.00,4807.0380,N,01131.0000,E,1,08,0.9,545.4,M,46.9,M,,*69"
        );
        let back = Gga::from_sentence(&parse(&s).unwrap()).unwrap();
        assert!((back.position.unwrap().lat_deg - 48.1173).abs() < 1e-6);
        let rmc = Rmc {
            time: None,
            valid: true,
            position: Some(Position::new(-33.8688, 151.2093)),
            speed_kn: Some(5.0),
            course_deg: Some(270.0),
            date: None,
            magnetic_variation_deg: Some(12.5),
            mode: Some('A'),
        };
        // 23:59:59.996 rounds to the next day: time and date carry together.
        let late =
            crate::time::days_from_civil(2026, 12, 31).unwrap() as f64 * 86_400.0 + 86_399.996;
        let s = rmc.write("GN", late).unwrap();
        assert!(
            s.starts_with("$GNRMC,000000.00,A,3352.1280,S,15112.5580,E,5.0,270.0,010127,12.5,E,A*"),
            "{s}"
        );
        let back = Rmc::from_sentence(&parse(&s).unwrap()).unwrap();
        assert_eq!(back.date, Some((2027, 1, 1)));
        // Reserved characters and over-long sentences are refused.
        let mut b = Builder::new('$', "GPTXT").unwrap();
        assert!(b.field("a,b").is_err());
        assert!(b.field(&"x".repeat(80)).is_err());
        assert!(Builder::new('#', "GPTXT").is_err());
        assert!(Builder::new('$', "gp").is_err());
        assert!(b.number(Some(f64::NAN), 1).is_err());
    }

    #[test]
    fn fields_of_time_and_date() {
        assert_eq!(parse_time("123519.25").unwrap().unwrap().second, 19.25);
        assert_eq!(parse_time("").unwrap(), None);
        assert!(parse_time("2359").is_err());
        assert!(parse_time("246000").is_err());
        assert!(parse_time("120000.x").is_err());
        assert_eq!(parse_date("230394").unwrap(), Some((1994, 3, 23)));
        assert_eq!(parse_date("010126").unwrap(), Some((2026, 1, 1)));
        assert!(parse_date("310226").is_err());
        assert!(parse_date("0101").is_err());
        assert_eq!(time_field(59.9996, 3).unwrap(), "000100.000");
        assert_eq!(time_field(1.5, 0).unwrap(), "000002");
        assert!(time_field(0.0, 4).is_err());
        assert_eq!(date_field(0.0).unwrap(), "010170");
    }
}

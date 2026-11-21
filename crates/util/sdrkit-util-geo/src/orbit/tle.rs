//! Two-line element sets (TLE), the format NORAD and CelesTrak publish mean elements in: two lines of 69 fixed columns, each with
//! a modulo-10 checksum, optionally after a name line of up to 24 characters. Catalogue numbers above 99 999 use the Alpha-5 form
//! (a letter A–Z, without I and O, for the ten-thousands digit).
//!
//! Standard: Spacetrack Report #3 (Hoots and Roehrich, 1980); CelesTrak, "NORAD Two-Line Element Set Format".

use crate::time::days_from_civil;
use crate::{Error, Text};
use crate::{Quantity, SyntaxFault};
use std::fmt::Write;

/// Whether [`Tle::parse`] checks the checksum of each line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checksum {
    /// Refuse a line whose last column does not match its checksum.
    Require,
    /// Do not look at the last column (it may be missing). For hand-made or test element sets.
    Ignore,
}

/// A parsed two-line element set. Angles are in degrees and rates as the TLE provides them; [`crate::orbit::Sgp4`] converts them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tle {
    /// The name line without a leading `0 `, or empty.
    pub name: Text<32>,
    /// NORAD catalogue number (Alpha-5 decoded: `A0000` is 100 000).
    pub catalog_number: u32,
    /// Classification: `U` unclassified, `C` classified, `S` secret.
    pub classification: char,
    /// International designator (`98067A`), or empty.
    pub international_designator: Text<8>,
    /// Epoch year, four digits (two-digit years 57 to 99 are 1957 to 1999, 00 to 56 are 2000 to 2056).
    pub epoch_year: i32,
    /// Epoch day of the year with fraction (1.0 is 1 January 00:00 UTC).
    pub epoch_day: f64,
    /// First derivative of the mean motion divided by two, revolutions per day².
    pub mean_motion_dot: f64,
    /// Second derivative of the mean motion divided by six, revolutions per day³.
    pub mean_motion_ddot: f64,
    /// B* drag term, inverse Earth radii.
    pub bstar: f64,
    /// Ephemeris type (0 in published sets).
    pub ephemeris_type: u8,
    /// Element set number.
    pub element_set_number: u16,
    /// Inclination, degrees.
    pub inclination_deg: f64,
    /// Right ascension of the ascending node, degrees.
    pub raan_deg: f64,
    /// Eccentricity.
    pub eccentricity: f64,
    /// Argument of perigee, degrees.
    pub arg_perigee_deg: f64,
    /// Mean anomaly, degrees.
    pub mean_anomaly_deg: f64,
    /// Mean motion, revolutions per day.
    pub mean_motion_rev_day: f64,
    /// Revolution number at epoch.
    pub revolution_number: u32,
}

/// The TLE checksum of a line: the sum of its digits, with 1 for each minus sign, over the first 68 columns, modulo 10.
pub fn checksum(line: &str) -> u8 {
    line.bytes()
        .take(68)
        .map(|c| match c {
            b'0'..=b'9' => (c - b'0') as u32,
            b'-' => 1,
            _ => 0,
        })
        .sum::<u32>() as u8
        % 10
}

impl Tle {
    /// Parses a TLE from two lines, or three with a name line first. Blank lines and trailing spaces are ignored.
    ///
    /// ```
    /// use sdrkit_util_geo::orbit::tle::{Checksum, Tle};
    /// let iss = "ISS (ZARYA)
    /// 1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927
    /// 2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537";
    /// let t = Tle::parse(iss, Checksum::Require).unwrap();
    /// assert_eq!((t.catalog_number, t.name.as_str(), t.epoch_year), (25544, "ISS (ZARYA)", 2008));
    /// assert_eq!(t.bstar, -0.11606e-4);
    /// ```
    pub fn parse(text: &str, check: Checksum) -> Result<Tle, Error> {
        let mut lines = text
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty());
        let a = lines.next().ok_or(Error::Syntax(SyntaxFault::NoTleLines))?;
        let b = lines
            .next()
            .ok_or(Error::Syntax(SyntaxFault::TleLineCount))?;
        let c = lines.next();
        if lines.next().is_some() {
            return Err(Error::Syntax(SyntaxFault::TooManyLines));
        }
        match c {
            None => Self::from_lines(None, a, b, check),
            Some(c) => Self::from_lines(Some(a), b, c, check),
        }
    }

    /// Parses a TLE from its lines.
    pub fn from_lines(
        name: Option<&str>,
        line1: &str,
        line2: &str,
        check: Checksum,
    ) -> Result<Tle, Error> {
        let l1 = line1.trim_end().as_bytes();
        let l2 = line2.trim_end().as_bytes();
        let need = if check == Checksum::Require { 69 } else { 68 };
        if l1.len() < need || l2.len() < need || l1.len() > 69 || l2.len() > 69 {
            return Err(Error::Syntax(SyntaxFault::TleLineLength));
        }
        if !l1.is_ascii() || !l2.is_ascii() {
            return Err(Error::Syntax(SyntaxFault::TleNotAscii));
        }
        if l1[0] != b'1' || l2[0] != b'2' {
            return Err(Error::Syntax(SyntaxFault::LineNumbers));
        }
        if check == Checksum::Require {
            for (line, text) in [(l1, line1), (l2, line2)] {
                let found = line[68].wrapping_sub(b'0');
                let computed = checksum(text);
                if found != computed {
                    return Err(Error::Checksum { found, computed });
                }
            }
        }
        // Both lines are ASCII, so these slices are on character boundaries.
        let s1 = std::str::from_utf8(l1).map_err(|_| Error::Syntax(SyntaxFault::Ascii))?;
        let s2 = std::str::from_utf8(l2).map_err(|_| Error::Syntax(SyntaxFault::Ascii))?;
        let catalog_number = alpha5(&s1[2..7])?;
        if alpha5(&s2[2..7])? != catalog_number {
            return Err(Error::Syntax(SyntaxFault::SatelliteMismatch));
        }
        let name_text = match name {
            Some(n) => {
                let n = n.trim();
                let n = n.strip_prefix("0 ").unwrap_or(n);
                let mut t = Text::new();
                for ch in n.chars() {
                    if t.push(ch).is_err() {
                        break;
                    }
                }
                t
            }
            None => Text::new(),
        };
        let yy = int(&s1[18..20], SyntaxFault::EpochYear)?;
        let epoch_year = if yy < 57 {
            2000 + yy as i32
        } else {
            1900 + yy as i32
        };
        let epoch_day = float(&s1[20..32], SyntaxFault::EpochDay)?;
        let leap = crate::time::is_leap_year(epoch_year);
        if !(1.0..(if leap { 367.0 } else { 366.0 })).contains(&epoch_day) {
            return Err(Error::OutOfRange(Quantity::EpochDay));
        }
        let eccentricity = float(&format_implied(&s2[26..33])?, SyntaxFault::Eccentricity)?;
        let tle = Tle {
            name: name_text,
            catalog_number,
            classification: (l1[7] as char).to_ascii_uppercase(),
            international_designator: Text::try_from_str(s1[9..17].trim())?,
            epoch_year,
            epoch_day,
            mean_motion_dot: float(&s1[33..43], SyntaxFault::MeanMotionDot)?,
            mean_motion_ddot: exponent_field(&s1[44..52], SyntaxFault::MeanMotionDdot)?,
            bstar: exponent_field(&s1[53..61], SyntaxFault::Bstar)?,
            ephemeris_type: blank_or(&s1[62..63], 0, SyntaxFault::EphemerisType)? as u8,
            element_set_number: blank_or(&s1[64..68], 0, SyntaxFault::ElementSetNumber)? as u16,
            inclination_deg: float(&s2[8..16], SyntaxFault::Inclination)?,
            raan_deg: float(&s2[17..25], SyntaxFault::RightAscension)?,
            eccentricity,
            arg_perigee_deg: float(&s2[34..42], SyntaxFault::ArgumentOfPerigee)?,
            mean_anomaly_deg: float(&s2[43..51], SyntaxFault::MeanAnomaly)?,
            mean_motion_rev_day: float(&s2[52..63], SyntaxFault::MeanMotion)?,
            revolution_number: blank_or(&s2[63..68], 0, SyntaxFault::RevolutionNumber)?,
        };
        if !(0.0..=180.0).contains(&tle.inclination_deg) {
            return Err(Error::OutOfRange(Quantity::Inclination));
        }
        if !(0.0..=360.0).contains(&tle.raan_deg)
            || !(0.0..=360.0).contains(&tle.arg_perigee_deg)
            || !(0.0..=360.0).contains(&tle.mean_anomaly_deg)
        {
            return Err(Error::OutOfRange(Quantity::Angle));
        }
        if tle.mean_motion_rev_day <= 0.0 {
            return Err(Error::OutOfRange(Quantity::MeanMotion));
        }
        Ok(tle)
    }

    /// The epoch as UTC Unix seconds.
    pub fn epoch_unix(&self) -> f64 {
        self.epoch_days_since_1970() * 86_400.0
    }

    /// The epoch as days (with fraction) since 1970-01-01 00:00 UTC.
    pub fn epoch_days_since_1970(&self) -> f64 {
        // The year was checked when parsing, so the date is valid.
        let jan1 = days_from_civil(self.epoch_year, 1, 1).unwrap_or(0) as f64;
        jan1 + (self.epoch_day - 1.0)
    }
}

fn int(s: &str, what: SyntaxFault) -> Result<u32, Error> {
    let t = s.trim();
    if t.is_empty() || !t.bytes().all(|c| c.is_ascii_digit()) {
        return Err(Error::Syntax(what));
    }
    t.parse().map_err(|_| Error::Syntax(what))
}

fn blank_or(s: &str, default: u32, what: SyntaxFault) -> Result<u32, Error> {
    if s.trim().is_empty() {
        Ok(default)
    } else {
        int(s, what)
    }
}

fn float(s: &str, what: SyntaxFault) -> Result<f64, Error> {
    let t = s.trim();
    if t.is_empty()
        || !t
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'+'))
    {
        return Err(Error::Syntax(what));
    }
    let v: f64 = t.parse().map_err(|_| Error::Syntax(what))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(Error::Syntax(what))
    }
}

/// The eccentricity field: seven digits with an implied leading decimal point.
fn format_implied(s: &str) -> Result<Text<10>, Error> {
    let t = s.trim();
    if t.is_empty() || !t.bytes().all(|c| c.is_ascii_digit()) {
        return Err(Error::Syntax(SyntaxFault::Eccentricity));
    }
    let mut out = Text::new();
    out.push_str("0.")?;
    out.push_str(t)?;
    Ok(out)
}

/// A field like ` 12345-3` or `-11606-4`: a signed mantissa with an implied leading decimal point and a signed power of ten.
fn exponent_field(s: &str, what: SyntaxFault) -> Result<f64, Error> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(0.0);
    }
    let (sign, rest) = match t.as_bytes()[0] {
        b'-' => (-1.0, &t[1..]),
        b'+' => (1.0, &t[1..]),
        _ => (1.0, t),
    };
    let split = rest.rfind(['-', '+']).ok_or(Error::Syntax(what))?;
    let (mant, exp) = rest.split_at(split);
    let mant = mant.trim();
    if mant.is_empty() || !mant.bytes().all(|c| c.is_ascii_digit()) {
        return Err(Error::Syntax(what));
    }
    let e: i32 = exp.parse().map_err(|_| Error::Syntax(what))?;
    // Parse "0.<mantissa>e<exponent>" so the value is the correctly rounded decimal the field writes.
    let mut t = Text::<32>::new();
    write!(t, "0.{mant}e{e}").map_err(|_| Error::Syntax(what))?;
    let m: f64 = t.parse().map_err(|_| Error::Syntax(what))?;
    Ok(sign * m)
}

/// A five-character catalogue number, digits or Alpha-5.
fn alpha5(s: &str) -> Result<u32, Error> {
    let b = s.as_bytes();
    let t = s.trim_start();
    if t.is_empty() {
        return Err(Error::Syntax(SyntaxFault::CatalogueNumber));
    }
    if b[0].is_ascii_alphabetic() {
        let c = b[0].to_ascii_uppercase();
        let v = match c {
            b'A'..=b'H' => c - b'A' + 10,
            b'J'..=b'N' => c - b'J' + 18,
            b'P'..=b'Z' => c - b'P' + 23,
            _ => return Err(Error::BadChar { at: 2 }),
        } as u32;
        Ok(v * 10_000 + int(&s[1..], SyntaxFault::CatalogueNumber)?)
    } else {
        int(t, SyntaxFault::CatalogueNumber)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISS_1: &str = "1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927";
    const ISS_2: &str = "2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537";

    #[test]
    fn the_published_example() {
        // The example set of the CelesTrak / Wikipedia description of the format.
        assert_eq!(checksum(ISS_1), 7);
        assert_eq!(checksum(ISS_2), 7);
        let t = Tle::from_lines(Some("0 ISS (ZARYA)"), ISS_1, ISS_2, Checksum::Require).unwrap();
        assert_eq!(t.name, "ISS (ZARYA)");
        assert_eq!(t.catalog_number, 25544);
        assert_eq!(t.classification, 'U');
        assert_eq!(t.international_designator, "98067A");
        assert_eq!((t.epoch_year, t.epoch_day), (2008, 264.517_825_28));
        assert_eq!(t.mean_motion_dot, -0.000_021_82);
        assert_eq!(t.mean_motion_ddot, 0.0);
        assert_eq!(t.bstar, -0.11606e-4);
        assert_eq!((t.ephemeris_type, t.element_set_number), (0, 292));
        assert_eq!(
            (t.inclination_deg, t.raan_deg, t.eccentricity),
            (51.6416, 247.4627, 0.000_670_3)
        );
        assert_eq!(
            (t.arg_perigee_deg, t.mean_anomaly_deg, t.mean_motion_rev_day),
            (130.5360, 325.0288, 15.721_253_91)
        );
        assert_eq!(t.revolution_number, 56353);
        // Day 264.51782528 of 2008 is 20 September 12:25:40.104 UTC.
        let c = crate::time::Civil::from_unix_f64(t.epoch_unix()).unwrap();
        assert_eq!(
            (c.month, c.day, c.hour, c.minute, c.second),
            (9, 20, 12, 25, 40)
        );
    }

    #[test]
    fn checksums_and_layout_are_checked() {
        let bad = ISS_2.replace("563537", "563538");
        assert_eq!(
            Tle::from_lines(None, ISS_1, &bad, Checksum::Require),
            Err(Error::Checksum {
                found: 8,
                computed: 7
            })
        );
        assert!(Tle::from_lines(None, ISS_1, &bad, Checksum::Ignore).is_ok());
        assert!(Tle::from_lines(None, ISS_2, ISS_1, Checksum::Ignore).is_err());
        assert!(Tle::from_lines(None, &ISS_1[..60], ISS_2, Checksum::Ignore).is_err());
        let other = ISS_2.replacen("25544", "25545", 1);
        assert!(Tle::from_lines(None, ISS_1, &other, Checksum::Ignore).is_err());
        let ecc = ISS_2.replacen("0006703", "00x6703", 1);
        assert!(Tle::from_lines(None, ISS_1, &ecc, Checksum::Ignore).is_err());
        assert!(Tle::parse("", Checksum::Ignore).is_err());
        assert!(Tle::parse(ISS_1, Checksum::Ignore).is_err());
        assert!(Tle::parse(&format!("a\n{ISS_1}\n{ISS_2}\nextra"), Checksum::Ignore).is_err());
        assert!(Tle::parse(&format!("\n{ISS_1}\n\n{ISS_2}\n"), Checksum::Require).is_ok());
        let nonascii = ISS_1.replacen("98067A", "98067é", 1);
        assert!(Tle::from_lines(None, &nonascii, ISS_2, Checksum::Ignore).is_err());
    }

    #[test]
    fn fields() {
        assert_eq!(
            exponent_field(" 12345-3", SyntaxFault::Bstar),
            Ok(0.12345e-3)
        );
        assert_eq!(
            exponent_field("-30915-6", SyntaxFault::Bstar),
            Ok(-0.30915e-6)
        );
        assert_eq!(exponent_field(" 00000+0", SyntaxFault::Bstar), Ok(0.0));
        assert_eq!(exponent_field("        ", SyntaxFault::Bstar), Ok(0.0));
        assert!(exponent_field(" 12a45-3", SyntaxFault::Bstar).is_err());
        assert!(exponent_field(" 12345", SyntaxFault::Bstar).is_err());
        // Alpha-5: A0000 is 100 000; I and O are skipped, so J is 18 and Z is 33.
        assert_eq!(alpha5("A0000"), Ok(100_000));
        assert_eq!(alpha5("H9999"), Ok(179_999));
        assert_eq!(alpha5("J0000"), Ok(180_000));
        assert_eq!(alpha5("Z9999"), Ok(339_999));
        assert!(alpha5("I0000").is_err());
        assert!(alpha5("O0000").is_err());
        assert_eq!(alpha5("00005"), Ok(5));
        assert_eq!(alpha5("    5"), Ok(5));
    }
}

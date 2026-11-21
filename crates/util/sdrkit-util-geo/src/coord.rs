//! Coordinates as text (signed decimal, decimal with hemisphere, DMS, DDM and the NMEA `ddmm.mmmm` form, latitude first), and
//! angles as fixed-point binary fields ([`angle_from_raw`], [`angle_to_raw`]): an angle is `raw × full_scale / 2^bits`.
//!
//! A formatter rounds the whole value to the requested decimals before it splits it, so 59.9996 seconds becomes the next minute.

use crate::error::finite;
use crate::{maidenhead, Error, Position, Text};
use crate::{Quantity, SyntaxFault};
use std::fmt::Write;

/// Which coordinate an angle is: it decides the hemisphere letters, the degree digits and the valid range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Latitude: `N`/`S`, two degree digits, ±90.
    Latitude,
    /// Longitude: `E`/`W`, three degree digits, ±180.
    Longitude,
}

impl Axis {
    fn letters(self) -> (u8, u8) {
        match self {
            Axis::Latitude => (b'N', b'S'),
            Axis::Longitude => (b'E', b'W'),
        }
    }
    fn limit(self) -> f64 {
        match self {
            Axis::Latitude => 90.0,
            Axis::Longitude => 180.0,
        }
    }
    fn degree_digits(self) -> usize {
        match self {
            Axis::Latitude => 2,
            Axis::Longitude => 3,
        }
    }
    fn quantity(self) -> Quantity {
        match self {
            Axis::Latitude => Quantity::Latitude,
            Axis::Longitude => Quantity::Longitude,
        }
    }
}

/// A text style for [`format_angle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Signed decimal degrees, no symbol: `-33.868800`.
    Signed,
    /// Decimal degrees with the hemisphere: `33.868800°S`.
    Decimal,
    /// Degrees, minutes and decimal seconds: `33°52'07.68"S`.
    Dms,
    /// Degrees and decimal minutes: `33°52.1280'S`.
    Ddm,
}

/// Formats an angle. `decimals` (0 to 9) applies to the last part: degrees, seconds or minutes.
///
/// ```
/// use sdrkit_util_geo::coord::{format_angle, Axis, Style};
/// assert_eq!(format_angle(-33.8688, Axis::Latitude, Style::Dms, 2).unwrap().as_str(), "33°52'07.68\"S");
/// assert_eq!(format_angle(151.2093, Axis::Longitude, Style::Ddm, 3).unwrap().as_str(), "151°12.558'E");
/// // Rounding carries: 0.99999999 degrees at 2 decimals of seconds is one whole degree.
/// assert_eq!(format_angle(0.99999999, Axis::Latitude, Style::Dms, 2).unwrap().as_str(), "1°00'00.00\"N");
/// ```
pub fn format_angle(deg: f64, axis: Axis, style: Style, decimals: u8) -> Result<Text<32>, Error> {
    check_angle(deg, axis)?;
    if decimals > 9 {
        return Err(Error::OutOfRange(Quantity::Decimals));
    }
    let d = decimals as usize;
    let (pos, neg) = axis.letters();
    let hemi = if deg < 0.0 { neg as char } else { pos as char };
    let mut t = Text::new();
    let w = |r: std::fmt::Result| r.map_err(|_| Error::TooLong);
    match style {
        Style::Signed => w(write!(t, "{deg:.d$}"))?,
        Style::Decimal => w(write!(t, "{:.d$}°{hemi}", deg.abs()))?,
        Style::Dms => {
            let (whole, sub) = split_units(deg.abs(), 3600.0, decimals);
            let s = sub as f64 / 10f64.powi(decimals as i32);
            let width = if d == 0 { 2 } else { d + 3 };
            w(write!(
                t,
                "{}°{:02}'{:0width$.d$}\"{hemi}",
                whole / 3600,
                (whole / 60) % 60,
                (whole % 60) as f64 + s
            ))?
        }
        Style::Ddm => {
            let (whole, sub) = split_units(deg.abs(), 60.0, decimals);
            let m = sub as f64 / 10f64.powi(decimals as i32);
            let width = if d == 0 { 2 } else { d + 3 };
            w(write!(
                t,
                "{}°{:0width$.d$}'{hemi}",
                whole / 60,
                (whole % 60) as f64 + m
            ))?
        }
    }
    Ok(t)
}

/// Splits a non-negative angle into whole units (`per_degree` units in a degree: 60 for minutes, 3600 for seconds) and a
/// fraction of a unit in steps of 10^-decimals, rounding the whole value first so the parts carry.
fn split_units(abs_deg: f64, per_degree: f64, decimals: u8) -> (u64, u64) {
    let scale = 10u64.pow(decimals as u32);
    let total = (abs_deg * per_degree * scale as f64).round() as u64;
    (total / scale, total % scale)
}

fn check_angle(deg: f64, axis: Axis) -> Result<(), Error> {
    finite(&[deg])?;
    if deg.abs() > axis.limit() {
        return Err(Error::OutOfRange(axis.quantity()));
    }
    Ok(())
}

/// The NMEA 0183 / APRS field of an angle: degrees (2 digits for latitude, 3 for longitude) followed by minutes with `decimals`
/// decimals (`4807.038`), and the hemisphere letter. Rounding carries into the degrees.
///
/// ```
/// use sdrkit_util_geo::coord::{format_ddmm, Axis};
/// let (f, h) = format_ddmm(48.1173, Axis::Latitude, 3).unwrap();
/// assert_eq!((f.as_str(), h), ("4807.038", 'N'));
/// let (f, h) = format_ddmm(-11.516_666_67, Axis::Longitude, 3).unwrap();
/// assert_eq!((f.as_str(), h), ("01131.000", 'W'));
/// ```
pub fn format_ddmm(deg: f64, axis: Axis, decimals: u8) -> Result<(Text<16>, char), Error> {
    check_angle(deg, axis)?;
    if decimals > 9 {
        return Err(Error::OutOfRange(Quantity::Decimals));
    }
    let (pos, neg) = axis.letters();
    let (whole, sub) = split_units(deg.abs(), 60.0, decimals);
    let mut t = Text::new();
    let dd = axis.degree_digits();
    write!(t, "{:0dd$}{:02}", whole / 60, whole % 60).map_err(|_| Error::TooLong)?;
    if decimals > 0 {
        let d = decimals as usize;
        write!(t, ".{sub:0d$}").map_err(|_| Error::TooLong)?;
    }
    Ok((t, if deg < 0.0 { neg as char } else { pos as char }))
}

/// Parses an NMEA 0183 / APRS angle field (`4807.038` with hemisphere `N`). The degrees are all digits before the last two
/// before the decimal point, so both `0730.5` and `00730.5` work. Minutes must be below 60 and the hemisphere must match the
/// axis. Empty fields are an error, as NMEA uses them for "no data".
///
/// ```
/// use sdrkit_util_geo::coord::{parse_ddmm, Axis};
/// assert!((parse_ddmm("4807.038", "N", Axis::Latitude).unwrap() - 48.1173).abs() < 1e-9);
/// assert!((parse_ddmm("01131.000", "W", Axis::Longitude).unwrap() + 11.516_666_666).abs() < 1e-8);
/// assert!(parse_ddmm("4867.0", "N", Axis::Latitude).is_err());
/// ```
pub fn parse_ddmm(field: &str, hemisphere: &str, axis: Axis) -> Result<f64, Error> {
    let b = field.as_bytes();
    if b.is_empty() {
        return Err(Error::Syntax(SyntaxFault::EmptyCoordinate));
    }
    let point = b.iter().position(|&c| c == b'.').unwrap_or(b.len());
    if point < 3 {
        return Err(Error::Syntax(SyntaxFault::MinutesDigits));
    }
    if let Some(at) = b
        .iter()
        .enumerate()
        .position(|(i, &c)| !(c.is_ascii_digit() || (c == b'.' && i == point)))
    {
        return Err(Error::BadChar { at });
    }
    let deg: f64 = field[..point - 2]
        .parse()
        .map_err(|_| Error::Syntax(SyntaxFault::Degrees))?;
    let min: f64 = field[point - 2..]
        .parse()
        .map_err(|_| Error::Syntax(SyntaxFault::Minutes))?;
    if min >= 60.0 {
        return Err(Error::OutOfRange(Quantity::Minutes));
    }
    let (pos, neg) = axis.letters();
    let sign = match hemisphere.as_bytes() {
        [c] if c.eq_ignore_ascii_case(&pos) => 1.0,
        [c] if c.eq_ignore_ascii_case(&neg) => -1.0,
        _ => return Err(Error::Syntax(SyntaxFault::HemisphereLetter)),
    };
    let v = sign * (deg + min / 60.0);
    check_angle(v, axis)?;
    Ok(v)
}

/// Parses one angle written as decimal degrees, degrees and minutes, or degrees, minutes and seconds.
///
/// Accepted: a sign or a hemisphere letter (before or after, any case); parts separated by `°`, `º`, `'`, `′`, `’`, `"`, `″`,
/// `”`, `:` or spaces; a decimal fraction on the last part only. Minutes and seconds must be below 60.
///
/// ```
/// use sdrkit_util_geo::coord::{parse_angle, Axis};
/// for s in ["-33.8688", "33.8688S", "S 33.8688", "33°52'07.68\"S", "33 52.128 S", "-33:52:07.68"] {
///     assert!((parse_angle(s, Axis::Latitude).unwrap() + 33.8688).abs() < 1e-6, "{s}");
/// }
/// assert!(parse_angle("33°75'S", Axis::Latitude).is_err());
/// ```
pub fn parse_angle(text: &str, axis: Axis) -> Result<f64, Error> {
    let s = text.trim();
    let b = s.as_bytes();
    if b.is_empty() {
        return Err(Error::Syntax(SyntaxFault::EmptyAngle));
    }
    let (pos, neg) = axis.letters();
    let is_hemi = |c: u8| c.eq_ignore_ascii_case(&pos) || c.eq_ignore_ascii_case(&neg);
    let mut sign = 1.0;
    let mut start = 0;
    let mut end = b.len();
    let mut have_hemi = false;
    if is_hemi(b[0]) {
        sign = if b[0].eq_ignore_ascii_case(&neg) {
            -1.0
        } else {
            1.0
        };
        start = 1;
        have_hemi = true;
    } else if is_hemi(b[end - 1]) {
        sign = if b[end - 1].eq_ignore_ascii_case(&neg) {
            -1.0
        } else {
            1.0
        };
        end -= 1;
        have_hemi = true;
    }
    let body = s[start..end].trim();
    let body = match body.as_bytes().first() {
        Some(b'-') if !have_hemi => {
            sign = -1.0;
            &body[1..]
        }
        Some(b'+') if !have_hemi => &body[1..],
        _ => body,
    };
    // Split into at most three numeric parts.
    let mut parts = [0.0f64; 3];
    let mut n = 0;
    let mut fraction_seen = false;
    for tok in body.split(|c: char| {
        matches!(
            c,
            '°' | 'º' | '\'' | '′' | '’' | '"' | '″' | '”' | ':' | ' ' | '\t'
        )
    }) {
        if tok.is_empty() {
            continue;
        }
        if n == 3 || fraction_seen {
            return Err(Error::Syntax(SyntaxFault::AngleParts));
        }
        if let Some(at) = tok.bytes().position(|c| !(c.is_ascii_digit() || c == b'.')) {
            // Byte offset of the bad character in the caller's text.
            return Err(Error::BadChar {
                at: tok.as_ptr() as usize - text.as_ptr() as usize + at,
            });
        }
        if tok.contains('.') {
            fraction_seen = true;
        }
        parts[n] = tok
            .parse()
            .map_err(|_| Error::Syntax(SyntaxFault::Number))?;
        n += 1;
    }
    if n == 0 {
        return Err(Error::Syntax(SyntaxFault::NoNumber));
    }
    if n >= 2 && parts[1] >= 60.0 {
        return Err(Error::OutOfRange(Quantity::Minutes));
    }
    if n == 3 && parts[2] >= 60.0 {
        return Err(Error::OutOfRange(Quantity::Seconds));
    }
    let v = sign * (parts[0] + parts[1] / 60.0 + parts[2] / 3600.0);
    check_angle(v, axis)?;
    Ok(v)
}

/// A position parsed from text by [`parse_position`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parsed {
    /// The position (the centre of the square for a locator).
    pub position: Position,
    /// Whether the text was a Maidenhead locator rather than coordinates.
    pub from_locator: bool,
}

/// Parses a position written as two angles (latitude first) or as a Maidenhead locator.
///
/// The two angles may be separated by a comma, a semicolon, or nothing but the hemisphere letters; each angle accepts every form
/// of [`parse_angle`]. Two plain signed numbers separated by spaces also work.
///
/// ```
/// use sdrkit_util_geo::coord::parse_position;
/// for s in ["54.687, 25.280", "54.687N 25.280E", "N54.687 E25.280", "54°41'13.2\"N 25°16'48\"E", "54.687 25.28"] {
///     let p = parse_position(s).unwrap().position;
///     assert!((p.lat_deg - 54.687).abs() < 1e-6 && (p.lon_deg - 25.28).abs() < 1e-6, "{s}");
/// }
/// assert!(parse_position("KO24pq").unwrap().from_locator);
/// ```
pub fn parse_position(text: &str) -> Result<Parsed, Error> {
    let s = text.trim();
    if s.is_empty() {
        return Err(Error::Syntax(SyntaxFault::EmptyPosition));
    }
    if let Ok(sq) = maidenhead::decode(s) {
        return Ok(Parsed {
            position: sq.centre(),
            from_locator: true,
        });
    }
    let (lat, lon) = split_pair(s)?;
    Ok(Parsed {
        position: Position::new(
            parse_angle(lat, Axis::Latitude)?,
            parse_angle(lon, Axis::Longitude)?,
        ),
        from_locator: false,
    })
}

fn split_pair(s: &str) -> Result<(&str, &str), Error> {
    if let Some(i) = s.find([',', ';']) {
        return Ok((&s[..i], &s[i + 1..]));
    }
    let b = s.as_bytes();
    let ns = |c: u8| matches!(c.to_ascii_uppercase(), b'N' | b'S');
    let ew = |c: u8| matches!(c.to_ascii_uppercase(), b'E' | b'W');
    if ns(b[0]) {
        // Prefix style: N54.687 E25.280.
        if let Some(i) = b.iter().position(|&c| ew(c)) {
            return Ok((&s[..i], &s[i..]));
        }
    } else if let Some(i) = b.iter().position(|&c| ns(c)) {
        // Suffix style: 54.687N 25.280E.
        return Ok((&s[..=i], &s[i + 1..]));
    }
    let mut it = s.split_whitespace();
    match (it.next(), it.next(), it.next()) {
        (Some(a), Some(b), None) => Ok((a, b)),
        _ => Err(Error::Syntax(SyntaxFault::LatitudeEnd)),
    }
}

/// Formats a position as `lat, lon` in signed decimal degrees with `decimals` decimals, the form [`parse_position`] reads back.
pub fn format_position(p: &Position, decimals: u8) -> Result<Text<48>, Error> {
    check_angle(p.lat_deg, Axis::Latitude)?;
    check_angle(p.lon_deg, Axis::Longitude)?;
    if decimals > 9 {
        return Err(Error::OutOfRange(Quantity::Decimals));
    }
    let d = decimals as usize;
    let mut t = Text::new();
    write!(t, "{:.d$}, {:.d$}", p.lat_deg, p.lon_deg).map_err(|_| Error::TooLong)?;
    Ok(t)
}

/// One semicircle in degrees: GPS (IS-GPS-200) and Galileo give angles in semicircles, where 1 semicircle = 180°.
pub const SEMICIRCLE_DEG: f64 = 180.0;

/// The angle of a fixed-point field: `raw × full_scale_deg / 2^bits`. `raw` is the field already sign-extended where it is
/// signed (see [`sign_extend`]). For example ADS-B UAT latitude: `bits = 24`, `full_scale_deg = 360`; GPS semicircles over 32
/// bits: `bits = 31`, `full_scale_deg = 180`.
pub fn angle_from_raw(raw: i64, bits: u32, full_scale_deg: f64) -> f64 {
    raw as f64 * full_scale_deg / (2f64).powi(bits.min(1023) as i32)
}

/// The nearest fixed-point value of an angle (the inverse of [`angle_from_raw`]), wrapped into `bits` bits as two's complement,
/// so a longitude of -1° over 360° full scale becomes a large unsigned field as the standards expect. Errors for non-finite
/// angles and for `bits` outside 1 to 63.
pub fn angle_to_raw(deg: f64, bits: u32, full_scale_deg: f64) -> Result<u64, Error> {
    finite(&[deg, full_scale_deg])?;
    if !(1..=63).contains(&bits) || full_scale_deg == 0.0 {
        return Err(Error::OutOfRange(Quantity::BitsOrFullScale));
    }
    let scaled = (deg / full_scale_deg * (2f64).powi(bits as i32)).round();
    if scaled.abs() > 9.0e18 {
        return Err(Error::OutOfRange(Quantity::Angle));
    }
    Ok((scaled as i64 as u64) & ((1u64 << bits) - 1))
}

/// Sign-extends the low `bits` bits of `raw` (two's complement). `bits` of 0 returns 0; 64 or more returns `raw` as is.
pub fn sign_extend(raw: u64, bits: u32) -> i64 {
    match bits {
        0 => 0,
        64.. => raw as i64,
        _ => {
            let shift = 64 - bits;
            ((raw << shift) as i64) >> shift
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        let f = |d, a, s, n| format_angle(d, a, s, n).unwrap().as_str().to_string();
        assert_eq!(f(-33.8688, Axis::Latitude, Style::Signed, 4), "-33.8688");
        assert_eq!(f(-33.8688, Axis::Latitude, Style::Decimal, 3), "33.869°S");
        assert_eq!(f(151.2093, Axis::Longitude, Style::Dms, 0), "151°12'33\"E");
        assert_eq!(f(0.0, Axis::Longitude, Style::Ddm, 0), "0°00'E");
        // Carry from seconds to minutes to degrees.
        assert_eq!(
            f(
                10.0 + 59.0 / 60.0 + 59.9996 / 3600.0,
                Axis::Latitude,
                Style::Dms,
                3
            ),
            "11°00'00.000\"N"
        );
        assert_eq!(
            f(10.999_999_9, Axis::Latitude, Style::Ddm, 4),
            "11°00.0000'N"
        );
        assert!(format_angle(91.0, Axis::Latitude, Style::Dms, 2).is_err());
        assert!(format_angle(1.0, Axis::Latitude, Style::Dms, 10).is_err());
        assert!(format_angle(f64::INFINITY, Axis::Longitude, Style::Dms, 2).is_err());
        assert_eq!(
            format_position(&Position::new(1.5, -2.25), 3).unwrap(),
            "1.500, -2.250"
        );
    }

    #[test]
    fn nmea_fields() {
        // The worked example of NMEA 0183 GGA: 4807.038,N,01131.000,E is 48°07.038'N 11°31.000'E.
        assert!(
            (parse_ddmm("4807.038", "N", Axis::Latitude).unwrap() - (48.0 + 7.038 / 60.0)).abs()
                < 1e-12
        );
        assert!(
            (parse_ddmm("01131.000", "E", Axis::Longitude).unwrap() - (11.0 + 31.0 / 60.0)).abs()
                < 1e-12
        );
        assert_eq!(
            format_ddmm(48.0 + 7.038 / 60.0, Axis::Latitude, 3).unwrap(),
            (Text::try_from_str("4807.038").unwrap(), 'N')
        );
        assert_eq!(
            format_ddmm(11.0 + 31.0 / 60.0, Axis::Longitude, 3)
                .unwrap()
                .0,
            "01131.000"
        );
        // APRS writes two decimals: 4903.50N 07201.75W.
        assert_eq!(
            format_ddmm(49.0 + 3.5 / 60.0, Axis::Latitude, 2).unwrap().0,
            "4903.50"
        );
        assert_eq!(
            format_ddmm(-(72.0 + 1.75 / 60.0), Axis::Longitude, 2).unwrap(),
            (Text::try_from_str("07201.75").unwrap(), 'W')
        );
        // 59.99996 minutes at 4 decimals carries to the next degree, never "60.0000".
        assert_eq!(
            format_ddmm(12.0 + 59.999_96 / 60.0, Axis::Latitude, 4)
                .unwrap()
                .0,
            "1300.0000"
        );
        assert_eq!(format_ddmm(5.0, Axis::Latitude, 0).unwrap().0, "0500");
        assert!(parse_ddmm("", "N", Axis::Latitude).is_err());
        assert!(parse_ddmm("4807.038", "E", Axis::Latitude).is_err());
        assert!(parse_ddmm("48x7.038", "N", Axis::Latitude).is_err());
        assert!(parse_ddmm("9100.000", "N", Axis::Latitude).is_err());
        assert!(parse_ddmm("07.0", "N", Axis::Latitude).is_err());
        assert!(
            (parse_ddmm("4807.", "S", Axis::Latitude).unwrap() + (48.0 + 7.0 / 60.0)).abs() < 1e-12
        );
    }

    #[test]
    fn parses_angles() {
        let lat = |s| parse_angle(s, Axis::Latitude);
        assert_eq!(lat("12"), Ok(12.0));
        assert_eq!(lat("n12"), Ok(12.0));
        assert_eq!(lat("12 30"), Ok(12.5));
        assert_eq!(lat("+12°30'"), Ok(12.5));
        assert!(lat("12.5 30").is_err()); // a fraction before the last part
        assert!(lat("1 2 3 4").is_err());
        assert!(lat("12 30 60").is_err());
        assert!(lat("-12S").is_err()); // sign and hemisphere together
        assert!(lat("12E").is_err());
        assert!(lat("").is_err());
        assert!(lat("N").is_err());
        assert!(matches!(lat("12x"), Err(Error::BadChar { .. })));
        assert!(parse_angle("180.0001", Axis::Longitude).is_err());
        assert_eq!(parse_angle("W 0°30'", Axis::Longitude), Ok(-0.5));
    }

    #[test]
    fn parses_positions() {
        let p = parse_position("-33.8688; 151.2093").unwrap();
        assert_eq!(p.position, Position::new(-33.8688, 151.2093));
        assert!(!p.from_locator);
        let p = parse_position("JO62qm").unwrap();
        assert!(p.from_locator && (p.position.lat_deg - 52.52).abs() < 0.03);
        assert!(parse_position("").is_err());
        assert!(parse_position("1 2 3").is_err());
        assert!(parse_position("95, 10").is_err());
    }

    #[test]
    fn fixed_point_angles() {
        // ADS-B UAT (DO-282B 2.2.4.5.2.1): latitude and longitude are 24-bit fractions of 360°.
        let raw = angle_to_raw(-122.0, 24, 360.0).unwrap();
        assert_eq!(raw, 0xA93E94); // (360 - 122) / 360 · 2^24, rounded
        assert!(
            (angle_from_raw(sign_extend(raw, 24), 24, 360.0) + 122.0).abs() < 360.0 / 16_777_216.0
        );
        // GPS semicircles: 2^31 counts to one semicircle (180°).
        assert_eq!(angle_from_raw(1 << 30, 31, SEMICIRCLE_DEG), 90.0);
        assert_eq!(sign_extend(0x80, 8), -128);
        assert_eq!(sign_extend(0x7f, 8), 127);
        assert_eq!(sign_extend(u64::MAX, 64), -1);
        assert_eq!(sign_extend(5, 0), 0);
        assert!(angle_to_raw(f64::NAN, 24, 360.0).is_err());
        assert!(angle_to_raw(1.0, 0, 360.0).is_err());
        assert!(angle_to_raw(1.0, 24, 0.0).is_err());
    }
}

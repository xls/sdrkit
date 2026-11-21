//! Civil dates and times, Modified Julian Day, Julian day, ISO 8601 text, leap seconds and the satellite navigation time scales.
//!
//! Times are Unix seconds on the UTC scale, as `f64` or as a whole `i64` second with nanoseconds. Unix time has no leap seconds;
//! [`LeapSeconds`] holds them for the conversions in [`gnss`]. Dates use the proleptic Gregorian calendar with the day-count
//! algorithms of H. Hinnant ("chrono-Compatible Low-Level Date Algorithms", 2013).

pub mod gnss;
mod leap;

pub use leap::LeapSeconds;

use crate::error::finite;
use crate::{Error, Text};
use crate::{Quantity, SyntaxFault};
use std::fmt::Write;

/// Seconds in a day.
pub const SECONDS_PER_DAY: i64 = 86_400;
/// The Modified Julian Day of the Unix epoch 1970-01-01.
pub const MJD_UNIX_EPOCH: i64 = 40_587;
/// Julian day minus Modified Julian Day (MJD = JD − 2 400 000.5).
pub const JD_MINUS_MJD: f64 = 2_400_000.5;
/// The Julian day of J2000.0, 2000-01-01 12:00.
pub const JD_J2000: f64 = 2_451_545.0;

/// A day of the week. ISO 8601 numbers them from Monday = 1; [`Weekday::iso_number`] returns that number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[allow(missing_docs)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    const ALL: [Weekday; 7] = [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ];

    /// The weekday of a day count since 1970-01-01 (a Thursday).
    pub fn from_days(days: i64) -> Weekday {
        Self::ALL[(days + 3).rem_euclid(7) as usize]
    }

    /// The ISO 8601 number: Monday 1 to Sunday 7.
    pub fn iso_number(self) -> u8 {
        self as u8 + 1
    }

    /// Days since Monday: Monday 0 to Sunday 6.
    pub fn days_from_monday(self) -> u8 {
        self as u8
    }

    /// The English three-letter abbreviation (`Mon`).
    pub fn short_name(self) -> &'static str {
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][self as usize]
    }
}

/// Whether a year of the proleptic Gregorian calendar is a leap year.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// The number of days in a month (1 to 12).
pub fn days_in_month(year: i32, month: u8) -> Result<u8, Error> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
        4 | 6 | 9 | 11 => Ok(30),
        2 => Ok(if is_leap_year(year) { 29 } else { 28 }),
        _ => Err(Error::OutOfRange(Quantity::Month)),
    }
}

/// The number of days from 1970-01-01 to a date. Errors for a month or day that does not exist.
///
/// ```
/// use sdrkit_util_geo::time::days_from_civil;
/// assert_eq!(days_from_civil(1970, 1, 1), Ok(0));
/// assert_eq!(days_from_civil(2000, 3, 1), Ok(11_017));
/// assert!(days_from_civil(2023, 2, 29).is_err());
/// ```
pub fn days_from_civil(year: i32, month: u8, day: u8) -> Result<i64, Error> {
    if day == 0 || day > days_in_month(year, month)? {
        return Err(Error::OutOfRange(Quantity::Day));
    }
    let y = year as i64 - (month <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468)
}

/// The date (year, month, day) of a day count since 1970-01-01. Errors when the year does not fit an `i32`.
pub fn civil_from_days(days: i64) -> Result<(i32, u8, u8), Error> {
    let z = days
        .checked_add(719_468)
        .ok_or(Error::OutOfRange(Quantity::DayCount))?;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = era
        .checked_mul(400)
        .and_then(|y| y.checked_add(yoe + (month <= 2) as i64))
        .ok_or(Error::OutOfRange(Quantity::Year))?;
    let year = i32::try_from(year).map_err(|_| Error::OutOfRange(Quantity::Year))?;
    Ok((year, month, day))
}

/// The month and day of a day of the year (1 = 1 January).
pub fn date_from_day_of_year(year: i32, day_of_year: u16) -> Result<(u8, u8), Error> {
    let n = if is_leap_year(year) { 366 } else { 365 };
    if day_of_year == 0 || day_of_year > n {
        return Err(Error::OutOfRange(Quantity::DayOfYear));
    }
    let (_, m, d) = civil_from_days(days_from_civil(year, 1, 1)? + day_of_year as i64 - 1)?;
    Ok((m, d))
}

/// A calendar date and time of day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Civil {
    /// Year (proleptic Gregorian; 0 is 1 BC).
    pub year: i32,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day of the month, 1 to 31.
    pub day: u8,
    /// Hour, 0 to 23.
    pub hour: u8,
    /// Minute, 0 to 59.
    pub minute: u8,
    /// Second, 0 to 59.
    pub second: u8,
    /// Nanoseconds, 0 to 999 999 999.
    pub nanos: u32,
    /// Day of the week.
    pub weekday: Weekday,
    /// Day of the year, 1 to 366.
    pub day_of_year: u16,
}

impl Civil {
    /// A date and time after checking every field (seconds 0 to 59: Unix time cannot hold a leap second).
    pub fn new(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        nanos: u32,
    ) -> Result<Civil, Error> {
        if hour > 23 || minute > 59 || second > 59 || nanos > 999_999_999 {
            return Err(Error::OutOfRange(Quantity::TimeOfDay));
        }
        let days = days_from_civil(year, month, day)?;
        Ok(Civil {
            year,
            month,
            day,
            hour,
            minute,
            second,
            nanos,
            weekday: Weekday::from_days(days),
            day_of_year: (days - days_from_civil(year, 1, 1)? + 1) as u16,
        })
    }

    /// The UTC date and time of Unix seconds `secs` plus `nanos`.
    ///
    /// ```
    /// use sdrkit_util_geo::time::{Civil, Weekday};
    /// let c = Civil::from_unix(1_780_272_000, 0).unwrap();
    /// assert_eq!((c.year, c.month, c.day, c.weekday, c.day_of_year), (2026, 6, 1, Weekday::Monday, 152));
    /// ```
    pub fn from_unix(secs: i64, nanos: u32) -> Result<Civil, Error> {
        Self::from_unix_local(secs, nanos, 0)
    }

    /// The local date and time of Unix seconds, `offset_min` minutes ahead of UTC (the host provides the offset; this crate never
    /// reads the system's time zone).
    pub fn from_unix_local(secs: i64, nanos: u32, offset_min: i32) -> Result<Civil, Error> {
        if nanos > 999_999_999 {
            return Err(Error::OutOfRange(Quantity::Nanoseconds));
        }
        let t = secs
            .checked_add(offset_min as i64 * 60)
            .ok_or(Error::OutOfRange(Quantity::Time))?;
        let days = t.div_euclid(SECONDS_PER_DAY);
        let sod = t.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days)?;
        Ok(Civil {
            year,
            month,
            day,
            hour: (sod / 3600) as u8,
            minute: (sod / 60 % 60) as u8,
            second: (sod % 60) as u8,
            nanos,
            weekday: Weekday::from_days(days),
            day_of_year: (days - days_from_civil(year, 1, 1)? + 1) as u16,
        })
    }

    /// The UTC date and time of `f64` Unix seconds, rounded to the nearest nanosecond.
    pub fn from_unix_f64(t: f64) -> Result<Civil, Error> {
        let (s, n) = split_f64(t)?;
        Self::from_unix(s, n)
    }

    /// Unix seconds and nanoseconds of this date and time, read as UTC.
    pub fn to_unix(&self) -> Result<(i64, u32), Error> {
        let days = days_from_civil(self.year, self.month, self.day)?;
        Ok((
            days * SECONDS_PER_DAY
                + self.hour as i64 * 3600
                + self.minute as i64 * 60
                + self.second as i64,
            self.nanos,
        ))
    }

    /// Unix seconds of this date and time as `f64`, read as UTC.
    pub fn to_unix_f64(&self) -> Result<f64, Error> {
        let (s, n) = self.to_unix()?;
        Ok(s as f64 + n as f64 * 1e-9)
    }
}

/// Splits `f64` seconds into whole seconds (floor) and nanoseconds, rounding to the nearest nanosecond.
pub fn split_f64(t: f64) -> Result<(i64, u32), Error> {
    finite(&[t])?;
    if t.abs() > 9.0e15 {
        return Err(Error::OutOfRange(Quantity::Time));
    }
    let s = t.floor();
    let mut secs = s as i64;
    let mut nanos = ((t - s) * 1e9).round() as i64;
    if nanos >= 1_000_000_000 {
        secs += 1;
        nanos -= 1_000_000_000;
    }
    Ok((secs, nanos as u32))
}

/// The Modified Julian Day (with fraction) of Unix seconds.
pub fn mjd_from_unix(t: f64) -> f64 {
    t / SECONDS_PER_DAY as f64 + MJD_UNIX_EPOCH as f64
}

/// Unix seconds of a Modified Julian Day (with fraction).
pub fn unix_from_mjd(mjd: f64) -> f64 {
    (mjd - MJD_UNIX_EPOCH as f64) * SECONDS_PER_DAY as f64
}

/// The Julian day (with fraction) of Unix seconds. Read on the UTC scale; astronomical work on TT or UT1 adds its own offset.
pub fn jd_from_unix(t: f64) -> f64 {
    mjd_from_unix(t) + JD_MINUS_MJD
}

/// Unix seconds of a Julian day (with fraction).
pub fn unix_from_jd(jd: f64) -> f64 {
    unix_from_mjd(jd - JD_MINUS_MJD)
}

/// The whole Modified Julian Day of a date (the day number DVB, DAB and RDS send).
///
/// ```
/// // EN 300 468 Annex C: 1993-10-13 is MJD 49 273 (0xC079).
/// assert_eq!(sdrkit_util_geo::time::mjd_from_date(1993, 10, 13), Ok(0xC079));
/// ```
pub fn mjd_from_date(year: i32, month: u8, day: u8) -> Result<i64, Error> {
    Ok(days_from_civil(year, month, day)? + MJD_UNIX_EPOCH)
}

/// The date of a whole Modified Julian Day.
pub fn date_from_mjd(mjd: i64) -> Result<(i32, u8, u8), Error> {
    civil_from_days(
        mjd.checked_sub(MJD_UNIX_EPOCH)
            .ok_or(Error::OutOfRange(Quantity::Mjd))?,
    )
}

/// ISO 8601 / RFC 3339 text of UTC Unix seconds with `decimals` (0 to 9) decimals of the second and a `Z`.
///
/// The time is rounded to the decimals first, so the seconds carry into minutes, hours and the date (59.9996 s at three
/// decimals is the next minute).
///
/// ```
/// use sdrkit_util_geo::time::iso8601;
/// assert_eq!(iso8601(1_794_134_527.25, 3).unwrap().as_str(), "2026-11-08T10:42:07.250Z");
/// assert_eq!(iso8601(1_794_182_399.9996, 3).unwrap().as_str(), "2026-11-09T00:00:00.000Z");
/// ```
pub fn iso8601(t: f64, decimals: u8) -> Result<Text<40>, Error> {
    let (s, n) = split_f64(t)?;
    format_iso8601(s, n, decimals, None)
}

/// ISO 8601 text of Unix seconds and nanoseconds. `offset_min` of `None` writes UTC with `Z`; `Some(m)` writes the local time
/// `m` minutes ahead of UTC with its offset (`+11:00`).
pub fn format_iso8601(
    secs: i64,
    nanos: u32,
    decimals: u8,
    offset_min: Option<i32>,
) -> Result<Text<40>, Error> {
    if decimals > 9 {
        return Err(Error::OutOfRange(Quantity::Decimals));
    }
    if nanos > 999_999_999 {
        return Err(Error::OutOfRange(Quantity::Nanoseconds));
    }
    if offset_min.is_some_and(|m| m.abs() >= 24 * 60) {
        return Err(Error::OutOfRange(Quantity::UtcOffset));
    }
    // Round to the requested decimals, carrying into the seconds.
    let unit = 10u32.pow(9 - decimals as u32);
    let mut frac = (nanos + unit / 2) / unit;
    let mut secs = secs;
    if frac >= 10u32.pow(decimals as u32) {
        frac -= 10u32.pow(decimals as u32);
        secs = secs
            .checked_add(1)
            .ok_or(Error::OutOfRange(Quantity::Time))?;
    }
    let c = Civil::from_unix_local(secs, 0, offset_min.unwrap_or(0))?;
    let mut t = Text::new();
    let w = |r: std::fmt::Result| r.map_err(|_| Error::TooLong);
    if (0..=9999).contains(&c.year) {
        w(write!(t, "{:04}", c.year))?;
    } else {
        // ISO 8601 expanded years carry an explicit sign.
        w(write!(t, "{:+05}", c.year))?;
    }
    w(write!(
        t,
        "-{:02}-{:02}T{:02}:{:02}:{:02}",
        c.month, c.day, c.hour, c.minute, c.second
    ))?;
    if decimals > 0 {
        let d = decimals as usize;
        w(write!(t, ".{frac:0d$}"))?;
    }
    match offset_min {
        None => w(t.write_char('Z'))?,
        Some(m) => w(write!(
            t,
            "{}{:02}:{:02}",
            if m < 0 { '-' } else { '+' },
            m.abs() / 60,
            m.abs() % 60
        ))?,
    }
    Ok(t)
}

/// Parses ISO 8601 / RFC 3339 text to UTC Unix seconds and nanoseconds.
///
/// Accepted: `YYYY-MM-DD`; `YYYY-MM-DDThh:mm`, `…:ss` and `…:ss.fffffffff` (1 to 9 decimals, `.` or `,`); a space instead of
/// `T`; the basic forms `YYYYMMDD` and `YYYYMMDDThhmmss[.f]`; then `Z`, `±hh:mm`, `±hhmm` or `±hh`. Text without a zone is read
/// as UTC. A leap second `23:59:60` is read as the first instant of the next day, as Unix time cannot hold it.
///
/// ```
/// use sdrkit_util_geo::time::parse_iso8601;
/// assert_eq!(parse_iso8601("2026-11-08T10:42:07.25Z"), Ok((1_794_134_527, 250_000_000)));
/// assert_eq!(parse_iso8601("2026-11-08T21:42:07.25+11:00"), Ok((1_794_134_527, 250_000_000)));
/// assert!(parse_iso8601("2026-02-29").is_err());
/// ```
pub fn parse_iso8601(text: &str) -> Result<(i64, u32), Error> {
    let b = text.trim().as_bytes();
    let mut p = Cursor { b, i: 0 };
    let year = p.digits(4)? as i32;
    let extended = p.eat(b'-');
    let month = p.digits(2)? as u8;
    if extended && !p.eat(b'-') {
        return Err(Error::Syntax(SyntaxFault::DateSeparator));
    }
    let day = p.digits(2)? as u8;
    let (mut hour, mut minute, mut second, mut nanos) = (0u32, 0u32, 0u32, 0u32);
    if p.eat(b'T') || p.eat(b't') || p.eat(b' ') {
        hour = p.digits(2)?;
        if extended && !p.eat(b':') {
            return Err(Error::Syntax(SyntaxFault::TimeSeparator));
        }
        minute = p.digits(2)?;
        let has_seconds = if extended {
            p.eat(b':')
        } else {
            p.peek().is_some_and(|c| c.is_ascii_digit())
        };
        if has_seconds {
            second = p.digits(2)?;
            if p.eat(b'.') || p.eat(b',') {
                let start = p.i;
                let mut scale = 100_000_000u32;
                while let Some(c) = p.peek().filter(u8::is_ascii_digit) {
                    if p.i - start == 9 {
                        return Err(Error::Syntax(SyntaxFault::TooManyDecimals));
                    }
                    nanos += (c - b'0') as u32 * scale;
                    scale /= 10;
                    p.i += 1;
                }
                if p.i == start {
                    return Err(Error::Syntax(SyntaxFault::DecimalPointWithoutDigits));
                }
            }
        }
    }
    let offset_s: i64 = match p.peek() {
        None => 0,
        Some(b'Z') | Some(b'z') => {
            p.i += 1;
            0
        }
        Some(c @ (b'+' | b'-')) => {
            p.i += 1;
            let h = p.digits(2)? as i64;
            let m = if p.eat(b':') || p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.digits(2)? as i64
            } else {
                0
            };
            if h > 23 || m > 59 {
                return Err(Error::OutOfRange(Quantity::UtcOffset));
            }
            let s = (h * 60 + m) * 60;
            if c == b'-' {
                -s
            } else {
                s
            }
        }
        Some(_) => return Err(Error::BadChar { at: p.i }),
    };
    if p.i != b.len() {
        return Err(Error::BadChar { at: p.i });
    }
    let leap = second == 60;
    if leap && !(hour == 23 && minute == 59) && offset_s == 0 {
        return Err(Error::OutOfRange(Quantity::Second));
    }
    if hour > 23 || minute > 59 || second > 60 {
        return Err(Error::OutOfRange(Quantity::TimeOfDay));
    }
    let days = days_from_civil(year, month, day)?;
    let local = days * SECONDS_PER_DAY + hour as i64 * 3600 + minute as i64 * 60 + second as i64;
    if leap {
        nanos = 0;
    }
    Ok((local - offset_s, nanos))
}

/// Parses ISO 8601 text (see [`parse_iso8601`]) to `f64` Unix seconds.
pub fn parse_iso8601_f64(text: &str) -> Result<f64, Error> {
    let (s, n) = parse_iso8601(text)?;
    Ok(s as f64 + n as f64 * 1e-9)
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn digits(&mut self, n: usize) -> Result<u32, Error> {
        let mut v = 0u32;
        for _ in 0..n {
            match self.peek() {
                Some(c) if c.is_ascii_digit() => {
                    v = v * 10 + (c - b'0') as u32;
                    self.i += 1;
                }
                Some(_) => return Err(Error::BadChar { at: self.i }),
                None => return Err(Error::Syntax(SyntaxFault::TextEndsEarly)),
            }
        }
        Ok(v)
    }
}

/// A compact UTC stamp for file names, `YYYYMMDD-hhmmss`, of whole Unix seconds.
///
/// ```
/// assert_eq!(sdrkit_util_geo::time::format_compact(1_794_134_527).unwrap().as_str(), "20261108-104207");
/// ```
pub fn format_compact(secs: i64) -> Result<Text<24>, Error> {
    let c = Civil::from_unix(secs, 0)?;
    let mut t = Text::new();
    write!(
        t,
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
    .map_err(|_| Error::TooLong)?;
    Ok(t)
}

/// A short English age for displays: `12 s`, `5 min`, `3 h 20 min`, `2 d 4 h`; negative ages (clock differences) are shown as
/// `0 s`. Whole units, rounded down.
///
/// ```
/// use sdrkit_util_geo::time::format_age;
/// assert_eq!(format_age(12.7).unwrap().as_str(), "12 s");
/// assert_eq!(format_age(3.0 * 3600.0 + 20.0 * 60.0 + 5.0).unwrap().as_str(), "3 h 20 min");
/// ```
pub fn format_age(seconds: f64) -> Result<Text<24>, Error> {
    finite(&[seconds])?;
    let s = seconds.clamp(0.0, 1e15) as u64;
    let mut t = Text::new();
    let r = match s {
        0..=59 => write!(t, "{s} s"),
        60..=3_599 => write!(t, "{} min", s / 60),
        3_600..=86_399 => {
            if (s / 60).is_multiple_of(60) {
                write!(t, "{} h", s / 3600)
            } else {
                write!(t, "{} h {} min", s / 3600, s / 60 % 60)
            }
        }
        _ => {
            if (s / 3600).is_multiple_of(24) {
                write!(t, "{} d", s / 86_400)
            } else {
                write!(t, "{} d {} h", s / 86_400, s / 3600 % 24)
            }
        }
    };
    r.map_err(|_| Error::TooLong)?;
    Ok(t)
}

/// The time now, Unix seconds on the system clock (0 if the clock reads before 1970). The only place this crate reads the clock.
pub fn now_unix() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_counts_and_weekdays() {
        assert_eq!(days_from_civil(1970, 1, 1), Ok(0));
        assert_eq!(civil_from_days(0), Ok((1970, 1, 1)));
        assert_eq!(Weekday::from_days(0), Weekday::Thursday);
        assert_eq!(days_from_civil(2000, 3, 1), Ok(11_017));
        assert_eq!(civil_from_days(-1), Ok((1969, 12, 31)));
        // 2000 is a leap year, 1900 and 2100 are not.
        assert!(
            is_leap_year(2000) && !is_leap_year(1900) && !is_leap_year(2100) && is_leap_year(2024)
        );
        assert!(days_from_civil(1900, 2, 29).is_err());
        assert!(days_from_civil(2024, 13, 1).is_err());
        assert!(days_from_civil(2024, 4, 31).is_err());
        assert!(days_from_civil(2024, 1, 0).is_err());
        // Round trip across the whole i32 year range, and refusal beyond it.
        for &y in &[
            i32::MIN,
            -4713,
            -1,
            0,
            1,
            1582,
            1600,
            1970,
            2038,
            2100,
            9999,
            10_000,
            i32::MAX,
        ] {
            for &(m, d) in &[(1u8, 1u8), (2, 28), (3, 1), (12, 31)] {
                let n = days_from_civil(y, m, d).unwrap();
                assert_eq!(civil_from_days(n), Ok((y, m, d)), "{y}-{m}-{d}");
            }
        }
        assert!(civil_from_days(i64::MAX).is_err());
        assert!(civil_from_days(i64::MIN).is_err());
        assert!(civil_from_days(days_from_civil(i32::MAX, 12, 31).unwrap() + 1).is_err());
        // Every day of four centuries round-trips and the weekdays advance one by one.
        let start = days_from_civil(1900, 1, 1).unwrap();
        let mut wd = Weekday::from_days(start).iso_number();
        assert_eq!(wd, 1); // 1900-01-01 was a Monday
        for n in start..start + 146_097 {
            let (y, m, d) = civil_from_days(n).unwrap();
            assert_eq!(days_from_civil(y, m, d), Ok(n));
            assert_eq!(Weekday::from_days(n).iso_number(), wd);
            wd = wd % 7 + 1;
        }
        assert_eq!(date_from_day_of_year(2024, 60), Ok((2, 29)));
        assert_eq!(date_from_day_of_year(2026, 365), Ok((12, 31)));
        assert!(date_from_day_of_year(2026, 366).is_err());
        assert_eq!(Weekday::Sunday.days_from_monday(), 6);
        assert_eq!(Weekday::Sunday.short_name(), "Sun");
    }

    #[test]
    fn civil_times() {
        let c = Civil::from_unix(0, 0).unwrap();
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.weekday, c.day_of_year),
            (1970, 1, 1, 0, Weekday::Thursday, 1)
        );
        let c = Civil::from_unix(-1, 999_999_999).unwrap();
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute, c.second),
            (1969, 12, 31, 23, 59, 59)
        );
        // The 32-bit rollover: 2038-01-19 03:14:08 UTC.
        let c = Civil::from_unix(1 << 31, 0).unwrap();
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute, c.second),
            (2038, 1, 19, 3, 14, 8)
        );
        let c = Civil::new(2026, 11, 8, 10, 42, 7, 250_000_000).unwrap();
        assert_eq!(c.to_unix(), Ok((1_794_134_527, 250_000_000)));
        assert_eq!(c.weekday, Weekday::Sunday);
        assert!(Civil::new(2026, 11, 8, 24, 0, 0, 0).is_err());
        assert!(Civil::new(2026, 11, 8, 0, 0, 60, 0).is_err());
        assert!(Civil::from_unix(0, 1_000_000_000).is_err());
        assert!(Civil::from_unix(i64::MAX, 0).is_err());
        assert!(Civil::from_unix_f64(f64::NAN).is_err());
        // An offset of +11 h moves 2026-11-08 13:00 UTC to the next local day.
        let c = Civil::from_unix_local(1_794_142_800, 0, 11 * 60).unwrap();
        assert_eq!((c.day, c.hour), (9, 0));
        assert!((Civil::from_unix_f64(1.5).unwrap().to_unix_f64().unwrap() - 1.5).abs() < 1e-9);
        assert_eq!(split_f64(-0.25), Ok((-1, 750_000_000)));
        assert_eq!(split_f64(0.999_999_999_9), Ok((1, 0)));
    }

    #[test]
    fn julian_days() {
        // J2000.0 is JD 2 451 545.0 = 2000-01-01 12:00 (here on the UTC scale).
        let t = Civil::new(2000, 1, 1, 12, 0, 0, 0)
            .unwrap()
            .to_unix_f64()
            .unwrap();
        assert_eq!(jd_from_unix(t), JD_J2000);
        assert_eq!(mjd_from_unix(t), 51_544.5);
        assert_eq!(unix_from_jd(JD_J2000), t);
        // Meeus, Astronomical Algorithms, example 7.a: 1957 October 4.81 is JD 2 436 116.31.
        let sputnik = days_from_civil(1957, 10, 4).unwrap() as f64 * 86_400.0 + 0.81 * 86_400.0;
        assert!((jd_from_unix(sputnik) - 2_436_116.31).abs() < 1e-6);
        // MJD 0 is 1858-11-17; EN 300 468 Annex C: MJD 0xC079 is 1993-10-13.
        assert_eq!(date_from_mjd(0), Ok((1858, 11, 17)));
        assert_eq!(date_from_mjd(0xC079), Ok((1993, 10, 13)));
        assert_eq!(mjd_from_date(2000, 1, 1), Ok(51_544));
        assert_eq!(mjd_from_date(2023, 2, 25), Ok(60_000));
        assert_eq!(unix_from_mjd(40_587.0), 0.0);
        assert!(date_from_mjd(i64::MIN).is_err());
    }

    #[test]
    fn iso_formatting() {
        let f = |t: f64, d: u8| iso8601(t, d).unwrap().as_str().to_string();
        assert_eq!(f(0.0, 0), "1970-01-01T00:00:00Z");
        assert_eq!(f(1_794_134_527.25, 2), "2026-11-08T10:42:07.25Z");
        assert_eq!(f(59.9996, 3), "1970-01-01T00:01:00.000Z");
        assert_eq!(f(-0.5, 1), "1969-12-31T23:59:59.5Z");
        assert_eq!(
            format_iso8601(0, 999_999_999, 9, None).unwrap(),
            "1970-01-01T00:00:00.999999999Z"
        );
        assert_eq!(
            format_iso8601(0, 999_999_999, 8, None).unwrap(),
            "1970-01-01T00:00:01.00000000Z"
        );
        assert_eq!(
            format_iso8601(1_794_134_527, 0, 0, Some(11 * 60)).unwrap(),
            "2026-11-08T21:42:07+11:00"
        );
        assert_eq!(
            format_iso8601(1_794_134_527, 0, 0, Some(-(5 * 60 + 30))).unwrap(),
            "2026-11-08T05:12:07-05:30"
        );
        let far = days_from_civil(-12_345, 6, 7).unwrap() * 86_400;
        assert_eq!(
            format_iso8601(far, 0, 0, None).unwrap(),
            "-12345-06-07T00:00:00Z"
        );
        let far = days_from_civil(12_345, 6, 7).unwrap() * 86_400;
        assert_eq!(
            format_iso8601(far, 0, 0, None).unwrap(),
            "+12345-06-07T00:00:00Z"
        );
        assert!(iso8601(f64::NAN, 3).is_err());
        assert!(iso8601(0.0, 10).is_err());
        assert!(format_iso8601(0, 0, 0, Some(24 * 60)).is_err());
    }

    #[test]
    fn iso_parsing() {
        let p = |s: &str| parse_iso8601(s);
        assert_eq!(p("1970-01-01"), Ok((0, 0)));
        assert_eq!(p("19700101T000001Z"), Ok((1, 0)));
        assert_eq!(p("1970-01-01T00:01"), Ok((60, 0)));
        assert_eq!(p("1970-01-01 00:00:00,5Z"), Ok((0, 500_000_000)));
        assert_eq!(p("1970-01-01T01:00:00+01"), Ok((0, 0)));
        assert_eq!(p("1970-01-01T01:00:00+0100"), Ok((0, 0)));
        assert_eq!(p("1969-12-31T23:00:00-01:00"), Ok((0, 0)));
        assert_eq!(p("2016-12-31T23:59:60Z"), Ok((1_483_228_800, 0)));
        assert_eq!(p(" 1970-01-01T00:00:00.123456789Z "), Ok((0, 123_456_789)));
        for bad in [
            "",
            "1970",
            "1970-01",
            "1970-1-01",
            "1970-01-01T",
            "1970-01-01T25:00",
            "1970-01-01T00:60",
            "1970-01-01T00:00:00.",
            "1970-01-01T00:00:00.1234567890",
            "1970-01-01T00:00:00+24:00",
            "1970-01-01T00:00:00Q",
            "1970-01-01T12:00:60Z",
            "1970-01-01T00:00:00Zjunk",
            "1970-01-01T00:00:61Z",
            "1970-02-30",
        ] {
            assert!(p(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_iso8601_f64("1970-01-01T00:00:01.5Z"), Ok(1.5));
        // Format then parse is the identity.
        for t in [-1e10, -1.0, 0.0, 1.5e9, 4e9, 2.5e11] {
            let s = iso8601(t, 9).unwrap();
            assert!(
                (parse_iso8601_f64(&s).unwrap() - t).abs() < 1e-6 * t.abs().max(1.0),
                "{s}"
            );
        }
        assert!(now_unix() > 1.7e9);
        assert_eq!(format_age(-5.0).unwrap(), "0 s");
        assert_eq!(format_age(59.9).unwrap(), "59 s");
        assert_eq!(format_age(60.0).unwrap(), "1 min");
        assert_eq!(format_age(7_200.0).unwrap(), "2 h");
        assert_eq!(
            format_age(86_400.0 * 2.0 + 3_600.0 * 4.0).unwrap(),
            "2 d 4 h"
        );
        assert_eq!(format_age(86_400.0).unwrap(), "1 d");
        assert!(format_age(f64::NAN).is_err());
        assert_eq!(format_compact(0).unwrap(), "19700101-000000");
    }
}

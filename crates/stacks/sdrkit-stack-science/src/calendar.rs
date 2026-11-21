//! Calendar helpers of the crate on top of `sdrkit_util_geo::time`: broken-down UTC with the field types the time codes use, Unix
//! times of dates, and the summer-time rules that the European and US time stations announce.

use sdrkit_util_geo::time::{self as gt, Weekday};

/// A broken-down UTC (or local) time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    /// The year.
    pub year: i64,
    /// The month, 1 to 12.
    pub month: u32,
    /// The day of the month, 1 to 31.
    pub day: u32,
    /// The hour, 0 to 23.
    pub hour: u32,
    /// The minute, 0 to 59.
    pub minute: u32,
    /// The second, 0 to 59.
    pub second: u32,
    /// The day of the week.
    pub weekday: Weekday,
    /// The day of the year, 1 is 1 January.
    pub yday: u32,
}

/// Breaks whole Unix seconds into their civil parts. A time outside the range of `sdrkit_util_geo::time` returns 1970-01-01.
pub fn civil(unix: i64) -> Civil {
    let c = gt::Civil::from_unix(unix, 0)
        .or_else(|_| gt::Civil::from_unix(0, 0))
        .expect("1970-01-01 is in range");
    Civil {
        year: c.year as i64,
        month: c.month as u32,
        day: c.day as u32,
        hour: c.hour as u32,
        minute: c.minute as u32,
        second: c.second as u32,
        weekday: c.weekday,
        yday: c.day_of_year as u32,
    }
}

/// The Unix time of a civil date and time, or `None` when a part is out of range.
pub fn unix(y: i64, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Option<i64> {
    if h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let y = i32::try_from(y).ok()?;
    let days = gt::days_from_civil(y, u8::try_from(mo).ok()?, u8::try_from(d).ok()?).ok()?;
    Some(days * gt::SECONDS_PER_DAY + h as i64 * 3600 + mi as i64 * 60 + s as i64)
}

/// Whether `y` is a leap year.
pub fn is_leap(y: i64) -> bool {
    i32::try_from(y).is_ok_and(gt::is_leap_year)
}

/// The days in a month, or 0 for a month out of range.
pub fn month_days(y: i64, m: u32) -> u32 {
    match (i32::try_from(y), u8::try_from(m)) {
        (Ok(y), Ok(m)) => gt::days_in_month(y, m).map_or(0, u32::from),
        _ => 0,
    }
}

/// The month and day of a day of the year, or `None` when the day is out of range for the year.
pub fn from_yday(y: i64, yday: u32) -> Option<(u32, u32)> {
    let (m, d) =
        gt::date_from_day_of_year(i32::try_from(y).ok()?, u16::try_from(yday).ok()?).ok()?;
    Some((m as u32, d as u32))
}

/// ISO 8601 text of whole Unix seconds ("2026-10-12T14:02:00Z"), or an empty text when out of range.
pub fn iso(unix: i64) -> String {
    gt::iso8601(unix as f64, 0).map_or_else(|_| String::new(), |t| t.as_str().to_string())
}

/// The compact stamp "YYYYMMDD-hhmmss" of whole Unix seconds, or an empty text when out of range.
pub fn compact(unix: i64) -> String {
    gt::format_compact(unix).map_or_else(|_| String::new(), |t| t.as_str().to_string())
}

/// The Unix time of the last Sunday of a month at a UTC hour.
fn last_sunday(y: i64, m: u32, hour: u32) -> i64 {
    let last = month_days(y, m);
    let Some(t) = unix(y, m, last, hour, 0, 0) else {
        return 0;
    };
    let back = (civil(t).weekday.days_from_monday() as i64 + 1) % 7;
    t - back * 86_400
}

/// The Unix time of the `n`-th Sunday (from 1) of a month at a UTC hour.
fn nth_sunday(y: i64, m: u32, n: u32, hour_utc: i64) -> i64 {
    let Some(first) = unix(y, m, 1, 0, 0, 0) else {
        return 0;
    };
    let ahead = (6 - civil(first).weekday.days_from_monday() as i64).rem_euclid(7);
    first + (ahead + 7 * (n as i64 - 1)) * 86_400 + hour_utc * 3600
}

/// Whether European summer time is in force at a UTC instant. It runs from 01:00 UTC on the last Sunday of March to 01:00 UTC on
/// the last Sunday of October (Directive 2000/84/EC, articles 1 to 3).
pub fn eu_summer(unix: i64) -> bool {
    let y = civil(unix).year;
    unix >= last_sunday(y, 3, 1) && unix < last_sunday(y, 10, 1)
}

/// Whether US daylight time is in force at a UTC instant, for a zone `std_offset_h` hours from UTC. It runs from 02:00 local on
/// the second Sunday of March to 02:00 local daylight time on the first Sunday of November (15 U.S.C. 260a).
pub fn us_dst(unix: i64, std_offset_h: i64) -> bool {
    let y = civil(unix).year;
    let start = nth_sunday(y, 3, 2, 2 - std_offset_h);
    let end = nth_sunday(y, 11, 1, 1 - std_offset_h);
    unix >= start && unix < end
}

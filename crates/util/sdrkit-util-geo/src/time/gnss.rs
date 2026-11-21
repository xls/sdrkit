//! TAI and the satellite navigation time scales: GPS, Galileo, BeiDou and GLONASS.
//!
//! | Scale | Relation | Week 0 begins | Source |
//! |---|---|---|---|
//! | TAI | UTC + (TAI − UTC) from [`LeapSeconds`] | — | IERS Bulletin C |
//! | GPS time | TAI − 19 s | 1980-01-06 00:00:00 UTC | IS-GPS-200N 3.3.4, 20.3.3.5.2.4 |
//! | Galileo System Time | GPS time (same seconds; week − 1024) | 1999-08-22 00:00:00 UTC − 13 s | Galileo OS SIS ICD 2.1, 5.1.2 |
//! | BeiDou Time | GPS time − 14 s (week − 1356) | 2006-01-01 00:00:00 UTC | BDS-SIS-ICD-B1I-3.0, 3.2 |
//! | GLONASS time | UTC(SU) + 3 h, with UTC's leap seconds | four-year intervals from 1996 | GLONASS ICD 5.1, 3.3.3 |
//!
//! The functions take UTC as Unix seconds (`f64`) and a leap-second table, so a host with a newer table gets the right answer.
//! Week numbers here are full week counts; [`resolve_week`] restores them from the truncated numbers that satellites send.

use super::{days_from_civil, LeapSeconds, SECONDS_PER_DAY};
use crate::error::finite;
use crate::Error;
use crate::Quantity;

/// Unix time of the GPS epoch, 1980-01-06 00:00:00 UTC.
pub const GPS_EPOCH_UNIX: i64 = 315_964_800;
/// Seconds in a week.
pub const SECONDS_PER_WEEK: f64 = 604_800.0;
/// TAI − GPS time, s (IS-GPS-200: GPS time was set to UTC at its epoch, when TAI − UTC was 19 s).
pub const TAI_MINUS_GPS_S: i32 = 19;
/// GPS week in which Galileo System Time week 0 begins.
pub const GALILEO_WEEK_IN_GPS: u32 = 1024;
/// GPS week in which BeiDou Time week 0 begins.
pub const BEIDOU_WEEK_IN_GPS: u32 = 1356;
/// GPS time − BeiDou Time, s.
pub const GPS_MINUS_BEIDOU_S: f64 = 14.0;
/// GLONASS time − UTC, s (Moscow time, UTC + 3 h).
pub const GLONASS_MINUS_UTC_S: f64 = 10_800.0;

/// A week number and the seconds into that week, the form in which GPS, Galileo and BeiDou count time.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeekTime {
    /// Full week number since the system's epoch (not truncated).
    pub week: u32,
    /// Seconds of the week, 0 to 604 800.
    pub tow_s: f64,
}

impl WeekTime {
    /// Seconds since the system's epoch.
    pub fn seconds(&self) -> f64 {
        self.week as f64 * SECONDS_PER_WEEK + self.tow_s
    }

    /// The week and seconds of `seconds` since the epoch; an error before the epoch.
    pub fn from_seconds(seconds: f64) -> Result<WeekTime, Error> {
        finite(&[seconds])?;
        if seconds < 0.0 {
            return Err(Error::OutOfRange(Quantity::TimeBeforeEpoch));
        }
        let week = (seconds / SECONDS_PER_WEEK).floor();
        if week > u32::MAX as f64 {
            return Err(Error::OutOfRange(Quantity::Week));
        }
        Ok(WeekTime {
            week: week as u32,
            tow_s: seconds - week * SECONDS_PER_WEEK,
        })
    }
}

/// TAI as "Unix seconds on the TAI scale" (UTC Unix seconds + TAI − UTC) of a UTC Unix time.
pub fn tai_from_utc(utc_unix: f64, leap: &LeapSeconds) -> Result<f64, Error> {
    finite(&[utc_unix])?;
    Ok(utc_unix + leap.tai_minus_utc(utc_unix.floor() as i64)? as f64)
}

/// The UTC Unix time of a TAI-scale time (the inverse of [`tai_from_utc`]). An inserted leap second maps onto a repeat of the
/// last second of its day, as in POSIX.
pub fn utc_from_tai(tai: f64, leap: &LeapSeconds) -> Result<f64, Error> {
    finite(&[tai])?;
    Ok(tai - leap.tai_minus_utc_at_tai(tai.floor() as i64)? as f64)
}

/// GPS − UTC in whole seconds at a UTC Unix time (18 s since 2017).
pub fn gps_minus_utc(utc_unix: f64, leap: &LeapSeconds) -> Result<i32, Error> {
    finite(&[utc_unix])?;
    Ok(leap.tai_minus_utc(utc_unix.floor() as i64)? - TAI_MINUS_GPS_S)
}

/// Seconds of GPS time since the GPS epoch at a UTC Unix time.
pub fn gps_seconds_from_utc(utc_unix: f64, leap: &LeapSeconds) -> Result<f64, Error> {
    Ok(tai_from_utc(utc_unix, leap)? - TAI_MINUS_GPS_S as f64 - GPS_EPOCH_UNIX as f64)
}

/// The UTC Unix time of seconds of GPS time since the GPS epoch.
pub fn utc_from_gps_seconds(gps_seconds: f64, leap: &LeapSeconds) -> Result<f64, Error> {
    utc_from_tai(
        gps_seconds + GPS_EPOCH_UNIX as f64 + TAI_MINUS_GPS_S as f64,
        leap,
    )
}

/// GPS week and time of week at a UTC Unix time.
///
/// ```
/// use sdrkit_util_geo::time::{gnss, LeapSeconds};
/// // The first week rollover: GPS week 1024 began at 1999-08-21 23:59:47 UTC (GPS − UTC was 13 s).
/// let w = gnss::gps_from_utc(935_279_987.0, &LeapSeconds::BUILTIN).unwrap();
/// assert_eq!((w.week, w.tow_s), (1024, 0.0));
/// ```
pub fn gps_from_utc(utc_unix: f64, leap: &LeapSeconds) -> Result<WeekTime, Error> {
    WeekTime::from_seconds(gps_seconds_from_utc(utc_unix, leap)?)
}

/// The UTC Unix time of a GPS week and time of week.
pub fn utc_from_gps(t: &WeekTime, leap: &LeapSeconds) -> Result<f64, Error> {
    utc_from_gps_seconds(t.seconds(), leap)
}

/// Galileo System Time week and time of week at a UTC Unix time.
pub fn galileo_from_utc(utc_unix: f64, leap: &LeapSeconds) -> Result<WeekTime, Error> {
    let g = gps_seconds_from_utc(utc_unix, leap)?;
    WeekTime::from_seconds(g - GALILEO_WEEK_IN_GPS as f64 * SECONDS_PER_WEEK)
}

/// The UTC Unix time of a Galileo System Time week and time of week.
pub fn utc_from_galileo(t: &WeekTime, leap: &LeapSeconds) -> Result<f64, Error> {
    utc_from_gps_seconds(
        t.seconds() + GALILEO_WEEK_IN_GPS as f64 * SECONDS_PER_WEEK,
        leap,
    )
}

/// BeiDou Time week and seconds of week at a UTC Unix time.
///
/// ```
/// use sdrkit_util_geo::time::{gnss, LeapSeconds};
/// // BeiDou Time began at 2006-01-01 00:00:00 UTC.
/// let w = gnss::beidou_from_utc(1_136_073_600.0, &LeapSeconds::BUILTIN).unwrap();
/// assert_eq!((w.week, w.tow_s), (0, 0.0));
/// ```
pub fn beidou_from_utc(utc_unix: f64, leap: &LeapSeconds) -> Result<WeekTime, Error> {
    let g = gps_seconds_from_utc(utc_unix, leap)?;
    WeekTime::from_seconds(g - BEIDOU_WEEK_IN_GPS as f64 * SECONDS_PER_WEEK - GPS_MINUS_BEIDOU_S)
}

/// The UTC Unix time of a BeiDou Time week and seconds of week.
pub fn utc_from_beidou(t: &WeekTime, leap: &LeapSeconds) -> Result<f64, Error> {
    utc_from_gps_seconds(
        t.seconds() + BEIDOU_WEEK_IN_GPS as f64 * SECONDS_PER_WEEK + GPS_MINUS_BEIDOU_S,
        leap,
    )
}

/// Restores a full week number from one truncated to `bits` bits (10 for the GPS LNAV week, 13 for CNAV, 12 for Galileo, 13 for
/// BeiDou, 8 for GPS almanac weeks): the full week congruent to `truncated` that is nearest `near_week` (a recent known
/// week, for example from the host's clock).
///
/// ```
/// use sdrkit_util_geo::time::gnss::resolve_week;
/// assert_eq!(resolve_week(352, 10, 2393), Ok(2400));
/// assert_eq!(resolve_week(1000, 10, 2393), Ok(2024));
/// ```
pub fn resolve_week(truncated: u32, bits: u32, near_week: u32) -> Result<u32, Error> {
    if !(1..=31).contains(&bits) || truncated >> bits != 0 {
        return Err(Error::OutOfRange(Quantity::TruncatedWeek));
    }
    let m = 1i64 << bits;
    let r = near_week as i64;
    let base = r - r.rem_euclid(m) + truncated as i64;
    let best = [base - m, base, base + m]
        .into_iter()
        .filter(|w| *w >= 0)
        .min_by_key(|w| (w - r).abs());
    best.and_then(|w| u32::try_from(w).ok())
        .ok_or(Error::OutOfRange(Quantity::Week))
}

/// A GLONASS time: the four-year interval since 1996 (N4 = 1 is 1996 to 1999), the day within it (NT = 1 is 1 January of the
/// leap year that starts it), and the time of day, all in Moscow time (UTC + 3 h).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GlonassTime {
    /// Four-year interval number, from 1.
    pub n4: u16,
    /// Day within the interval, 1 to 1461.
    pub nt: u16,
    /// Seconds of the day, 0 to 86 400 (86 400 and above only during an inserted leap second).
    pub tod_s: f64,
}

fn glonass_origin_days() -> i64 {
    // 1996-01-01; the date is fixed, so the calendar call cannot fail.
    days_from_civil(1996, 1, 1).unwrap_or(9496)
}

/// GLONASS time at a UTC Unix time (GLONASS time follows UTC's leap seconds, so no table is needed).
///
/// ```
/// use sdrkit_util_geo::time::gnss::glonass_from_utc;
/// // 2020-01-01 00:00 Moscow time (2019-12-31 21:00 UTC) is the first day of interval 7.
/// let g = glonass_from_utc(1_577_826_000.0).unwrap();
/// assert_eq!((g.n4, g.nt, g.tod_s), (7, 1, 0.0));
/// ```
pub fn glonass_from_utc(utc_unix: f64) -> Result<GlonassTime, Error> {
    finite(&[utc_unix])?;
    let msk = utc_unix + GLONASS_MINUS_UTC_S;
    let days = (msk / SECONDS_PER_DAY as f64).floor();
    let tod = msk - days * SECONDS_PER_DAY as f64;
    let since = days as i64 - glonass_origin_days();
    if since < 0 || since / 1461 >= u16::MAX as i64 {
        return Err(Error::OutOfRange(Quantity::GlonassTime));
    }
    Ok(GlonassTime {
        n4: (since / 1461 + 1) as u16,
        nt: (since % 1461 + 1) as u16,
        tod_s: tod,
    })
}

/// The UTC Unix time of a GLONASS time.
pub fn utc_from_glonass(t: &GlonassTime) -> Result<f64, Error> {
    finite(&[t.tod_s])?;
    if t.n4 == 0 || t.nt == 0 || t.nt > 1461 {
        return Err(Error::OutOfRange(Quantity::GlonassDayFields));
    }
    let days = glonass_origin_days() + (t.n4 as i64 - 1) * 1461 + t.nt as i64 - 1;
    Ok(days as f64 * SECONDS_PER_DAY as f64 + t.tod_s - GLONASS_MINUS_UTC_S)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unix(y: i32, m: u8, d: u8, h: i64, mi: i64, s: i64) -> f64 {
        (days_from_civil(y, m, d).unwrap() * 86_400 + h * 3600 + mi * 60 + s) as f64
    }

    const L: &LeapSeconds = &LeapSeconds::BUILTIN;

    #[test]
    fn gps_epoch_and_rollovers() {
        // IS-GPS-200: GPS time zero is 1980-01-06 00:00:00 UTC.
        assert_eq!(
            gps_from_utc(unix(1980, 1, 6, 0, 0, 0), L).unwrap(),
            WeekTime {
                week: 0,
                tow_s: 0.0
            }
        );
        // First rollover: 1999-08-22 00:00:00 GPS time, 1999-08-21 23:59:47 UTC (GPS − UTC = 13 s).
        assert_eq!(
            gps_from_utc(unix(1999, 8, 21, 23, 59, 47), L).unwrap(),
            WeekTime {
                week: 1024,
                tow_s: 0.0
            }
        );
        // Second rollover: 2019-04-07 00:00:00 GPS time, 2019-04-06 23:59:42 UTC (18 s).
        assert_eq!(
            gps_from_utc(unix(2019, 4, 6, 23, 59, 42), L).unwrap(),
            WeekTime {
                week: 2048,
                tow_s: 0.0
            }
        );
        assert_eq!(gps_minus_utc(unix(2026, 11, 8, 0, 0, 0), L), Ok(18));
        let t = unix(2026, 11, 8, 10, 42, 7) + 0.25;
        let w = gps_from_utc(t, L).unwrap();
        assert!((utc_from_gps(&w, L).unwrap() - t).abs() < 1e-6);
        assert!(gps_from_utc(unix(1980, 1, 5, 0, 0, 0), L).is_err());
        assert!(gps_from_utc(f64::NAN, L).is_err());
    }

    #[test]
    fn tai_and_the_leap_second() {
        let before = unix(2016, 12, 31, 23, 59, 59);
        assert_eq!(tai_from_utc(before, L), Ok(before + 36.0));
        assert_eq!(tai_from_utc(before + 1.0, L), Ok(before + 1.0 + 37.0));
        // The inserted second (TAI-scale before + 37) maps onto a repeat of 23:59:59.
        assert_eq!(utc_from_tai(before + 37.0, L), Ok(before));
        assert_eq!(utc_from_tai(before + 38.0, L), Ok(before + 1.0));
        assert!(tai_from_utc(0.0, L).is_err());
    }

    #[test]
    fn galileo_and_beidou_epochs() {
        // Galileo OS SIS ICD 5.1.2: GST week 0 began 13 s before 1999-08-22 00:00:00 UTC.
        assert_eq!(
            galileo_from_utc(unix(1999, 8, 21, 23, 59, 47), L).unwrap(),
            WeekTime {
                week: 0,
                tow_s: 0.0
            }
        );
        // BDS ICD 3.2: BDT began at 2006-01-01 00:00:00 UTC, then GPS week 1356 second 14.
        assert_eq!(
            beidou_from_utc(unix(2006, 1, 1, 0, 0, 0), L).unwrap(),
            WeekTime {
                week: 0,
                tow_s: 0.0
            }
        );
        assert_eq!(
            gps_from_utc(unix(2006, 1, 1, 0, 0, 0), L).unwrap(),
            WeekTime {
                week: 1356,
                tow_s: 14.0
            }
        );
        let t = unix(2026, 3, 4, 5, 6, 7);
        assert_eq!(utc_from_galileo(&galileo_from_utc(t, L).unwrap(), L), Ok(t));
        assert_eq!(utc_from_beidou(&beidou_from_utc(t, L).unwrap(), L), Ok(t));
        // GST and GPS time tick together: same seconds of week.
        assert_eq!(
            galileo_from_utc(t, L).unwrap().tow_s,
            gps_from_utc(t, L).unwrap().tow_s
        );
        assert_eq!(
            galileo_from_utc(t, L).unwrap().week + 1024,
            gps_from_utc(t, L).unwrap().week
        );
    }

    #[test]
    fn glonass_days() {
        let g = glonass_from_utc(unix(1996, 1, 1, 0, 0, 0) - 3.0 * 3600.0).unwrap();
        assert_eq!((g.n4, g.nt, g.tod_s), (1, 1, 0.0));
        // 1999-12-31 is the last day (1461) of interval 1.
        let g = glonass_from_utc(unix(1999, 12, 31, 12, 0, 0)).unwrap();
        assert_eq!((g.n4, g.nt, g.tod_s), (1, 1461, 15.0 * 3600.0));
        let t = unix(2026, 11, 8, 10, 42, 7);
        assert_eq!(utc_from_glonass(&glonass_from_utc(t).unwrap()), Ok(t));
        assert!(glonass_from_utc(unix(1995, 12, 31, 0, 0, 0)).is_err());
        assert!(utc_from_glonass(&GlonassTime {
            n4: 1,
            nt: 1462,
            tod_s: 0.0
        })
        .is_err());
    }

    #[test]
    fn week_resolution() {
        assert_eq!(resolve_week(352, 10, 2393), Ok(2400));
        assert_eq!(resolve_week(1000, 10, 2393), Ok(2024));
        assert_eq!(resolve_week(100, 10, 2393), Ok(2148));
        assert_eq!(resolve_week(5, 10, 0), Ok(5));
        assert_eq!(resolve_week(1020, 10, 2), Ok(1020));
        assert!(resolve_week(1024, 10, 2393).is_err());
        assert!(resolve_week(0, 0, 2393).is_err());
    }
}

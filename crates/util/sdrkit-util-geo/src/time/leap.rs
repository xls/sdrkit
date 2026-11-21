//! The one leap-second table: TAI − UTC since 1972.

use crate::Error;
use crate::{Quantity, SyntaxFault};

/// The most entries a table can hold (28 are in use; room for many decades more).
const CAPACITY: usize = 64;

/// Seconds from the NTP epoch 1900-01-01 to the Unix epoch 1970-01-01 (RFC 5905).
const NTP_UNIX_OFFSET: i64 = 2_208_988_800;

/// A table of leap seconds: the Unix time at which each new value of TAI − UTC applies, and when the table stops being
/// known to be complete.
///
/// [`LeapSeconds::BUILTIN`] holds every leap second announced by the IERS up to Bulletin C 71 (January 2026), which announced no
/// leap second at the end of June 2026, so the table is complete until 2026-12-28 ([`LeapSeconds::expires_unix`]).
///
/// **Updating.** A host that has a newer list provides it to the engine in one of three ways, and passes the result wherever a
/// `&LeapSeconds` is taken:
///
/// 1. [`LeapSeconds::from_iers_list`] reads the IERS/IETF `leap-seconds.list` file (published by the IERS at
///    `hpiers.obspm.fr/iers/bul/bulc/ntp/leap-seconds.list` and shipped by most operating systems' time-zone data), with its
///    expiry date.
/// 2. [`LeapSeconds::with_leap`] adds one leap second, for example from the UTC parameters a GNSS receiver decodes (GPS ΔtLSF and
///    WNLSF, Galileo, BeiDou), and [`LeapSeconds::with_expiry`] extends the validity.
/// 3. A new release of this crate updates [`LeapSeconds::BUILTIN`]: the entries and the expiry are the two constants below it in
///    `time/leap.rs`.
///
/// ```
/// use sdrkit_util_geo::time::LeapSeconds;
/// let t = LeapSeconds::BUILTIN;
/// assert_eq!(t.tai_minus_utc(1_483_228_799), Ok(36)); // 2016-12-31 23:59:59
/// assert_eq!(t.tai_minus_utc(1_483_228_800), Ok(37)); // 2017-01-01 00:00:00
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeapSeconds {
    /// (Unix time of UTC midnight when the value applies, TAI − UTC from then on).
    entries: [(i64, i32); CAPACITY],
    len: usize,
    expires_unix: i64,
}

/// The IERS leap-second history (IERS Bulletin C; the `leap-seconds.list` file): UTC dates and TAI − UTC from that date.
const BUILTIN_ENTRIES: [(i32, u8, i32); 28] = [
    (1972, 1, 10),
    (1972, 7, 11),
    (1973, 1, 12),
    (1974, 1, 13),
    (1975, 1, 14),
    (1976, 1, 15),
    (1977, 1, 16),
    (1978, 1, 17),
    (1979, 1, 18),
    (1980, 1, 19),
    (1981, 7, 20),
    (1982, 7, 21),
    (1983, 7, 22),
    (1985, 7, 23),
    (1988, 1, 24),
    (1990, 1, 25),
    (1991, 1, 26),
    (1992, 7, 27),
    (1993, 7, 28),
    (1994, 7, 29),
    (1996, 1, 30),
    (1997, 7, 31),
    (1999, 1, 32),
    (2006, 1, 33),
    (2009, 1, 34),
    (2012, 7, 35),
    (2015, 7, 36),
    (2017, 1, 37),
];

/// The built-in table is known complete until this date (2026-12-28 00:00 UTC, the expiry of `leap-seconds.list` after IERS
/// Bulletin C 71).
const BUILTIN_EXPIRES: (i32, u8, u8) = (2026, 12, 28);

/// Days from 1970-01-01 to the first day of a month, for the const table (months 1 to 12 only; checked by a test against the
/// general calendar code).
const fn days_to_month_start(year: i32, month: u8) -> i64 {
    let y = year as i64 - (month <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

const fn build_builtin() -> LeapSeconds {
    let mut entries = [(0i64, 0i32); CAPACITY];
    let mut i = 0;
    while i < BUILTIN_ENTRIES.len() {
        let (y, m, v) = BUILTIN_ENTRIES[i];
        entries[i] = (days_to_month_start(y, m) * 86_400, v);
        i += 1;
    }
    let (ey, em, ed) = BUILTIN_EXPIRES;
    LeapSeconds {
        entries,
        len: BUILTIN_ENTRIES.len(),
        expires_unix: (days_to_month_start(ey, em) + ed as i64 - 1) * 86_400,
    }
}

impl LeapSeconds {
    /// The table built into this release (see the type's documentation for what it holds and how to update it).
    pub const BUILTIN: LeapSeconds = build_builtin();

    /// The entries in force: (Unix time when the value applies, TAI − UTC from then on), oldest first.
    pub fn entries(&self) -> &[(i64, i32)] {
        &self.entries[..self.len]
    }

    /// The Unix time until which the table is known to be complete. Conversions past it still use the last entry; the host
    /// should warn or fetch a newer list when [`LeapSeconds::is_expired`] says so.
    pub fn expires_unix(&self) -> i64 {
        self.expires_unix
    }

    /// Whether `now_unix` is past the expiry, so a leap second may be missing.
    pub fn is_expired(&self, now_unix: i64) -> bool {
        now_unix >= self.expires_unix
    }

    /// TAI − UTC in seconds at a UTC Unix time. [`Error::BeforeLeapTable`] before 1972-01-01.
    pub fn tai_minus_utc(&self, utc_unix: i64) -> Result<i32, Error> {
        let e = self.entries();
        match e.iter().rposition(|&(t, _)| utc_unix >= t) {
            Some(i) => Ok(e[i].1),
            None => Err(Error::BeforeLeapTable),
        }
    }

    /// TAI − UTC at a TAI instant written as "Unix seconds on the TAI scale" (UTC Unix seconds plus TAI − UTC). During an
    /// inserted leap second the new value is returned, so the UTC Unix time repeats the last second of the day, as POSIX does.
    pub fn tai_minus_utc_at_tai(&self, tai_unix: i64) -> Result<i32, Error> {
        let e = self.entries();
        // An entry takes effect at TAI = t + (previous value) + 1 for an inserted second: the leap second itself belongs to the
        // new value.
        let first = e.first().ok_or(Error::BeforeLeapTable)?;
        if tai_unix < first.0 + first.1 as i64 {
            return Err(Error::BeforeLeapTable);
        }
        let mut v = first.1;
        for w in e.windows(2) {
            let (t, new) = w[1];
            let old = w[0].1;
            if tai_unix >= t + old as i64 {
                v = new;
            }
        }
        Ok(v)
    }

    /// A copy with one more leap second: from `utc_unix` (a UTC midnight) TAI − UTC is `tai_minus_utc`. The entry must be later
    /// than the last one.
    pub fn with_leap(mut self, utc_unix: i64, tai_minus_utc: i32) -> Result<LeapSeconds, Error> {
        if self.len == CAPACITY {
            return Err(Error::TooLong);
        }
        if utc_unix.rem_euclid(86_400) != 0 {
            return Err(Error::OutOfRange(Quantity::LeapSecondNotAtMidnight));
        }
        if let Some(&(t, v)) = self.entries().last() {
            if utc_unix <= t || (tai_minus_utc - v).abs() != 1 {
                return Err(Error::OutOfRange(Quantity::LeapSecondOrder));
            }
        }
        self.entries[self.len] = (utc_unix, tai_minus_utc);
        self.len += 1;
        self.expires_unix = self.expires_unix.max(utc_unix);
        Ok(self)
    }

    /// A copy whose table is known complete until `expires_unix` (it never moves the expiry earlier).
    pub fn with_expiry(mut self, expires_unix: i64) -> LeapSeconds {
        self.expires_unix = self.expires_unix.max(expires_unix);
        self
    }

    /// Reads the IERS/IETF `leap-seconds.list` format: lines of `NTP-seconds TAI−UTC [# comment]`, comments starting with `#`, and
    /// the expiry on a line `#@ NTP-seconds`. NTP seconds count from 1900-01-01. If the expiry line is missing, the expiry is the
    /// date of the last entry.
    ///
    /// ```
    /// use sdrkit_util_geo::time::LeapSeconds;
    /// let list = "#@\t3976214400\n2272060800\t10\t# 1 Jan 1972\n2287785600\t11\t# 1 Jul 1972\n";
    /// let t = LeapSeconds::from_iers_list(list).unwrap();
    /// assert_eq!(t.tai_minus_utc(80_000_000), Ok(11));
    /// assert_eq!(t.expires_unix(), 3_976_214_400 - 2_208_988_800);
    /// ```
    pub fn from_iers_list(text: &str) -> Result<LeapSeconds, Error> {
        let mut t = LeapSeconds {
            entries: [(0, 0); CAPACITY],
            len: 0,
            expires_unix: i64::MIN,
        };
        let mut expiry = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("#@") {
                let ntp: i64 = rest
                    .trim()
                    .parse()
                    .map_err(|_| Error::Syntax(SyntaxFault::ExpiryLine))?;
                expiry = Some(ntp - NTP_UNIX_OFFSET);
                continue;
            }
            let data = line.split('#').next().unwrap_or("").trim();
            if data.is_empty() {
                continue;
            }
            let mut f = data.split_whitespace();
            let ntp: i64 = f
                .next()
                .and_then(|x| x.parse().ok())
                .ok_or(Error::Syntax(SyntaxFault::NtpSeconds))?;
            let v: i32 = f
                .next()
                .and_then(|x| x.parse().ok())
                .ok_or(Error::Syntax(SyntaxFault::TaiMinusUtc))?;
            let unix = ntp - NTP_UNIX_OFFSET;
            if t.len == 0 {
                if unix.rem_euclid(86_400) != 0 {
                    return Err(Error::OutOfRange(Quantity::LeapSecondNotAtMidnight));
                }
                t.entries[0] = (unix, v);
                t.len = 1;
            } else {
                t = t.with_leap(unix, v)?;
            }
        }
        if t.len == 0 {
            return Err(Error::Syntax(SyntaxFault::NoEntries));
        }
        t.expires_unix = expiry.unwrap_or(t.entries[t.len - 1].0);
        Ok(t)
    }
}

impl Default for LeapSeconds {
    fn default() -> Self {
        Self::BUILTIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::days_from_civil;

    #[test]
    fn builtin_table_matches_the_calendar() {
        let t = LeapSeconds::BUILTIN;
        for (e, &(y, m, v)) in t.entries().iter().zip(BUILTIN_ENTRIES.iter()) {
            assert_eq!(e.0, days_from_civil(y, m, 1).unwrap() * 86_400);
            assert_eq!(e.1, v);
        }
        assert_eq!(t.entries().len(), 28);
        assert_eq!(
            t.expires_unix(),
            days_from_civil(2026, 12, 28).unwrap() * 86_400
        );
        // Each step is exactly one second.
        for w in t.entries().windows(2) {
            assert_eq!(w[1].1 - w[0].1, 1);
        }
    }

    #[test]
    fn lookups() {
        let t = LeapSeconds::BUILTIN;
        assert_eq!(t.tai_minus_utc(63_071_999), Err(Error::BeforeLeapTable)); // 1971-12-31 23:59:59
        assert_eq!(t.tai_minus_utc(63_072_000), Ok(10));
        // The GPS epoch 1980-01-06: TAI − UTC = 19, so GPS time began equal to UTC.
        assert_eq!(t.tai_minus_utc(315_964_800), Ok(19));
        // 2026: 37 s.
        assert_eq!(t.tai_minus_utc(1_794_134_527), Ok(37));
        // On the TAI scale, the inserted second 2016-12-31 23:59:60 UTC already carries the new value.
        let leap_tai = 1_483_228_800 + 36; // the TAI-scale instant of 23:59:60
        assert_eq!(t.tai_minus_utc_at_tai(leap_tai - 1), Ok(36));
        assert_eq!(t.tai_minus_utc_at_tai(leap_tai), Ok(37));
        assert_eq!(t.tai_minus_utc_at_tai(0), Err(Error::BeforeLeapTable));
        assert!(t.is_expired(t.expires_unix()) && !t.is_expired(t.expires_unix() - 1));
    }

    #[test]
    fn updates() {
        let t = LeapSeconds::BUILTIN;
        let next = days_from_civil(2030, 1, 1).unwrap() * 86_400;
        let u = t.with_leap(next, 38).unwrap();
        assert_eq!(u.tai_minus_utc(next - 1), Ok(37));
        assert_eq!(u.tai_minus_utc(next), Ok(38));
        assert!(u.expires_unix() >= next);
        assert!(t.with_leap(next + 1, 38).is_err());
        assert!(t.with_leap(next, 39).is_err());
        assert!(t.with_leap(0, 38).is_err());
        assert_eq!(t.with_expiry(0).expires_unix(), t.expires_unix());
        // The real file's layout.
        let list = "#\tUpdated through IERS Bulletin C 71\n#$\t 3945196800\n#@\t3975868800\n#\n\
                    2272060800\t10\t# 1 Jan 1972\n2287785600\t11\t# 1 Jul 1972\n2303683200\t12\t# 1 Jan 1973\n";
        let l = LeapSeconds::from_iers_list(list).unwrap();
        assert_eq!(
            l.entries(),
            &[(63_072_000, 10), (78_796_800, 11), (94_694_400, 12)]
        );
        assert_eq!(l.expires_unix(), 3_975_868_800 - NTP_UNIX_OFFSET);
        assert!(LeapSeconds::from_iers_list("").is_err());
        assert!(LeapSeconds::from_iers_list("abc 10\n").is_err());
        assert!(LeapSeconds::from_iers_list("2272060800 10\n2272060800 11\n").is_err());
    }
}

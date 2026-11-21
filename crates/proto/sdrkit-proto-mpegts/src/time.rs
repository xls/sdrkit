//! DVB time coding (EN 300 468 Annex C): Modified Julian Date and BCD times, and UTC text.

/// The Unix time (seconds since 1970) of a 40-bit MJD + BCD hh:mm:ss field. `None` when undefined (all bits 1), the BCD is bad, the
/// time out of range, or the date before 1970.
pub fn mjd_bcd_to_unix(b: &[u8]) -> Option<i64> {
    if b.len() < 5 {
        return None;
    }
    if b[..5].iter().all(|&x| x == 0xFF) {
        return None;
    }
    let mjd = u16::from_be_bytes([b[0], b[1]]) as i64;
    let h = bcd(b[2])?;
    let m = bcd(b[3])?;
    let s = bcd(b[4])?;
    if h > 23 || m > 59 || s > 59 || mjd < 40587 {
        return None;
    }
    Some((mjd - 40587) * 86_400 + h as i64 * 3600 + m as i64 * 60 + s as i64)
}

/// A BCD byte (`None` when a digit is above 9).
pub fn bcd(x: u8) -> Option<u32> {
    let (hi, lo) = (x >> 4, x & 0x0F);
    (hi <= 9 && lo <= 9).then_some(hi as u32 * 10 + lo as u32)
}

/// A 24-bit BCD duration hh:mm:ss in seconds.
pub fn bcd_duration(b: &[u8]) -> Option<i64> {
    if b.len() < 3 {
        return None;
    }
    let h = bcd(b[0])? as i64;
    let m = bcd(b[1])? as i64;
    let s = bcd(b[2])? as i64;
    (m <= 59 && s <= 59).then_some(h * 3600 + m * 60 + s)
}

/// The 40-bit MJD + BCD field of a Unix time.
pub fn unix_to_mjd_bcd(t: i64) -> [u8; 5] {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let mjd = (days + 40587) as u16;
    let to_bcd = |v: i64| (((v / 10) << 4) | (v % 10)) as u8;
    let [a, b] = mjd.to_be_bytes();
    [
        a,
        b,
        to_bcd(secs / 3600),
        to_bcd(secs / 60 % 60),
        to_bcd(secs % 60),
    ]
}

/// A BCD byte of `v` (0 to 99).
pub fn to_bcd(v: u32) -> u8 {
    (((v / 10) << 4) | (v % 10)) as u8
}

/// Year, month, day, hour, minute, second of a Unix time (UTC).
pub fn civil(t: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400) as u32;
    // Howard Hinnant's days-from-civil inverse.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (
        if m <= 2 { y + 1 } else { y },
        m,
        d,
        secs / 3600,
        secs / 60 % 60,
        secs % 60,
    )
}

/// "YYYY-MM-DD hh:mm:ss UTC".
pub fn utc_text(t: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// The weekday of a Unix time, 0 Monday to 6 Sunday.
pub fn weekday(t: i64) -> u32 {
    ((t.div_euclid(86_400) + 3).rem_euclid(7)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn annex_c_example() {
        let t = mjd_bcd_to_unix(&[0xC0, 0x79, 0x12, 0x45, 0x00]).unwrap();
        assert_eq!(utc_text(t), "1993-10-13 12:45:00 UTC");
        assert_eq!(unix_to_mjd_bcd(t), [0xC0, 0x79, 0x12, 0x45, 0x00]);
        assert_eq!(mjd_bcd_to_unix(&[0xFF; 5]), None);
        assert_eq!(mjd_bcd_to_unix(&[0xC0, 0x79, 0x1A, 0x45, 0x00]), None);
        assert_eq!(mjd_bcd_to_unix(&[0xC0, 0x79, 0x24, 0x00, 0x00]), None);
        assert_eq!(mjd_bcd_to_unix(&[0x00, 0x10, 0x00, 0x00, 0x00]), None);
        assert_eq!(bcd_duration(&[0x01, 0x30, 0x00]), Some(5400));
        assert_eq!(utc_text(1_768_469_400), "2026-01-15 09:30:00 UTC");
        assert_eq!(weekday(1_768_469_400), 3); // a Thursday
    }
}

//! GLONASS L1OF strings (GLONASS ICD 4.3 to 4.5): the fields of strings 1 to 5 (immediate data, Table 4.5; string 5, Table 4.11),
//! the frame collector that makes an ephemeris, and GLONASS time on the GPS scale.
//!
//! A string is 85 bits; `s[0]` is bit 85 (sent first) and `s[85 - k]` is bit k, as in [`crate::fec::glonass`].

use crate::orbit::GloEphemeris;
use sdrkit_util_bits::HardBits;
use sdrkit_util_geo::time::gnss::{
    utc_from_glonass, GlonassTime, GLONASS_MINUS_UTC_S, GPS_EPOCH_UNIX,
};

fn f_u(s: &[u8; 85], hi: usize, lo: usize) -> u64 {
    HardBits::at(s, 85 - hi).u((hi - lo + 1) as u32)
}
fn f_sm(s: &[u8; 85], hi: usize, lo: usize) -> i64 {
    HardBits::at(s, 85 - hi).sm((hi - lo + 1) as u32)
}

/// The string number m (bits 84 to 81).
pub fn string_number(s: &[u8; 85]) -> u32 {
    f_u(s, 84, 81) as u32
}

/// tk of string 1 as seconds of day.
pub fn tk_seconds(s: &[u8; 85]) -> f64 {
    let h = f_u(s, 76, 72) as f64;
    let m = f_u(s, 71, 66) as f64;
    let half = f_u(s, 65, 65) as f64;
    h * 3600.0 + m * 60.0 + half * 30.0
}

/// String 5's time parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GloTime {
    pub na: u32,
    pub tau_c: f64,
    pub n4: u32,
    pub tau_gps: f64,
}

/// Decodes string 5.
pub fn string5(s: &[u8; 85]) -> GloTime {
    GloTime {
        na: f_u(s, 80, 70) as u32,
        tau_c: f_sm(s, 69, 38) as f64 * 2f64.powi(-31),
        n4: f_u(s, 36, 32) as u32,
        tau_gps: f_sm(s, 31, 10) as f64 * 2f64.powi(-30),
    }
}

/// Collects strings 1 to 4 of one frame.
#[derive(Clone, Debug, Default)]
pub struct GloCollector {
    strings: [Option<(i64, [u8; 85])>; 4],
}

impl GloCollector {
    /// Strings of 1 to 4 held for the newest frame.
    pub fn parts(&self) -> usize {
        self.strings.iter().filter(|s| s.is_some()).count()
    }
    /// Adds string m (1 to 4) that started at `tod` (seconds of day); returns the ephemeris when strings 1 to 4 of one frame are
    /// held.
    pub fn add(&mut self, s: &[u8; 85], tod: f64, k: i8) -> Option<GloEphemeris> {
        let m = string_number(s) as usize;
        if !(1..=4).contains(&m) {
            return None;
        }
        let frame = ((tod - 2.0 * (m as f64 - 1.0)) / 30.0).round() as i64;
        self.strings[m - 1] = Some((frame, *s));
        let mut ss = [[0u8; 85]; 4];
        for (i, x) in self.strings.iter().enumerate() {
            match x {
                Some((f, st)) if *f == frame => ss[i] = *st,
                _ => return None,
            }
        }
        let km = 1000.0;
        let p20 = 2f64.powi(-20) * km;
        let p30 = 2f64.powi(-30) * km;
        let p11 = 2f64.powi(-11) * km;
        let v = |s: &[u8; 85]| {
            (
                f_sm(s, 64, 41) as f64 * p20,
                f_sm(s, 40, 36) as f64 * p30,
                f_sm(s, 35, 9) as f64 * p11,
            )
        };
        let (vx, ax, x) = v(&ss[0]);
        let (vy, ay, y) = v(&ss[1]);
        let (vz, az, z) = v(&ss[2]);
        Some(GloEphemeris {
            k,
            slot: f_u(&ss[3], 15, 11) as u16,
            tb: f_u(&ss[1], 76, 70) as f64 * 900.0,
            tk: tk_seconds(&ss[0]),
            pos: [x, y, z],
            vel: [vx, vy, vz],
            acc: [ax, ay, az],
            tau_n: f_sm(&ss[3], 80, 59) as f64 * 2f64.powi(-30),
            gamma_n: f_sm(&ss[2], 79, 69) as f64 * 2f64.powi(-40),
            delta_tau_n: f_sm(&ss[3], 58, 54) as f64 * 2f64.powi(-30),
            bn: f_u(&ss[1], 80, 78) as u32,
            p1: f_u(&ss[0], 78, 77) as u32,
            p2: f_u(&ss[1], 77, 77) as u32,
            p3: f_u(&ss[2], 80, 80) as u32,
            p4: f_u(&ss[3], 34, 34) as u32,
            p: f_u(&ss[2], 67, 66) as u32,
            ln: f_u(&ss[2], 65, 65) as u32,
            ft: f_u(&ss[3], 33, 30) as u32,
            nt: f_u(&ss[3], 26, 16) as u32,
            m: f_u(&ss[3], 10, 9) as u32,
            en: f_u(&ss[3], 53, 49) as u32,
        })
    }
}

/// The day number since 1980-01-06 (GPS epoch) of GLONASS day NT of four-year interval N4 (N4 = 1 is 1996 to 1999). `None` for
/// an N4 or NT out of range.
pub fn glo_day_since_gps_epoch(n4: u32, nt: u32) -> Option<i64> {
    let t = GlonassTime {
        n4: u16::try_from(n4).ok()?,
        nt: u16::try_from(nt).ok()?,
        tod_s: 0.0,
    };
    let utc = utc_from_glonass(&t).ok()?;
    // Moscow midnight, counted in days from the GPS epoch.
    Some(((utc + GLONASS_MINUS_UTC_S - GPS_EPOCH_UNIX as f64) / 86_400.0).round() as i64)
}

/// GPS week and seconds of a GLONASS time of day on day `day` (since the GPS epoch, Moscow date): minus 3 h, plus the leap
/// seconds, plus τGPS.
pub fn glo_to_gps(day: i64, tod: f64, leap: f64, tau_gps: f64) -> (i64, f64) {
    let total = day as f64 * 86_400.0 + tod - 10_800.0 + leap + tau_gps;
    let w = (total / 604_800.0).floor();
    (w as i64, total - w * 604_800.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glonass_days() {
        // 1996-01-01 (Moscow) is day 1 of interval 1: 5839 days after 1980-01-06.
        let d = glo_day_since_gps_epoch(1, 1).unwrap();
        assert_eq!(d, 5839);
        assert!(glo_day_since_gps_epoch(0, 1).is_none());
        let (w, s) = glo_to_gps(d, 10_800.0, 0.0, 0.0);
        let t = crate::geo::gps_unix(w, s);
        assert_eq!(
            sdrkit_util_geo::time::iso8601(t, 0).unwrap().as_str(),
            "1996-01-01T00:00:00Z"
        );
    }
}

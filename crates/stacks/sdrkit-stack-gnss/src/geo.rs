//! Coordinates, time and the atmosphere models of the solver, in the forms that the solver uses (arrays in radians, GPS week
//! and seconds).
//!
//! Standard: IS-GPS-200 clause 20.3.3.5.2.5 (Klobuchar).

use crate::system::{C, GPS_PI, WEEK_S};
use sdrkit_util_geo::time::gnss::{self as gtime, GPS_EPOCH_UNIX};
use sdrkit_util_geo::time::LeapSeconds;
use sdrkit_util_geo::{Ecef, Ellipsoid, Observer, Position};

/// Geodetic latitude, longitude (radians) and height (m) on WGS-84 of an ECEF point. A point that is not finite returns NaN.
pub fn ecef_to_geodetic(p: [f64; 3]) -> [f64; 3] {
    match Ellipsoid::WGS84.from_ecef(&Ecef::new(p[0], p[1], p[2])) {
        Ok(g) => [g.lat_deg.to_radians(), g.lon_deg.to_radians(), g.alt_m],
        Err(_) => [f64::NAN; 3],
    }
}

/// The WGS-84 position of geodetic latitude, longitude (radians) and height (m).
fn position(g: [f64; 3]) -> Position {
    Position {
        lat_deg: g[0].to_degrees(),
        lon_deg: g[1].to_degrees(),
        alt_m: g[2],
    }
}

/// ECEF of a geodetic point (radians, m) on WGS-84.
pub fn geodetic_to_ecef(g: [f64; 3]) -> [f64; 3] {
    Ellipsoid::WGS84.to_ecef(&position(g)).to_array()
}

/// The east, north, up components of an ECEF vector at a geodetic position (radians, m).
pub fn ecef_to_enu(pos: [f64; 3], d: [f64; 3]) -> [f64; 3] {
    let e = Observer::new(position(pos)).rotate_to_enu(d);
    [e.east_m, e.north_m, e.up_m]
}

/// Azimuth and elevation (radians) of `sat` (ECEF) seen from a receiver at `rx_geo` (geodetic, radians and m). `rx` is the same
/// receiver in ECEF; the look angles are taken from `rx_geo`.
pub fn az_el(rx_geo: [f64; 3], _rx: [f64; 3], sat: [f64; 3]) -> (f64, f64) {
    let l = Observer::new(position(rx_geo)).look(&Ecef::new(sat[0], sat[1], sat[2]));
    (l.azimuth_deg.to_radians(), l.elevation_deg.to_radians())
}

/// Distance of two ECEF points, m.
pub fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    Ecef::new(a[0], a[1], a[2]).distance_m(&Ecef::new(b[0], b[1], b[2]))
}

/// The Klobuchar parameters (IS-GPS-200 20.3.3.5.2.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Klobuchar {
    pub alpha: [f64; 4],
    pub beta: [f64; 4],
}

impl Klobuchar {
    /// The L1 ionospheric delay (seconds) at GPS time of week `tow`, receiver at `geo` (radians), satellite at `az`, `el`.
    pub fn delay_s(&self, tow: f64, geo: [f64; 3], az: f64, el: f64) -> f64 {
        let e = el / GPS_PI; // semicircles
        let psi = 0.0137 / (e + 0.11) - 0.022;
        let mut phi_i = geo[0] / GPS_PI + psi * az.cos();
        phi_i = phi_i.clamp(-0.416, 0.416);
        let lam_i = geo[1] / GPS_PI + psi * az.sin() / (phi_i * GPS_PI).cos();
        let phi_m = phi_i + 0.064 * ((lam_i - 1.617) * GPS_PI).cos();
        let mut t = 4.32e4 * lam_i + tow;
        t = t.rem_euclid(86_400.0);
        let f = 1.0 + 16.0 * (0.53 - e).powi(3);
        let mut amp = 0.0;
        let mut per = 0.0;
        for n in 0..4 {
            amp += self.alpha[n] * phi_m.powi(n as i32);
            per += self.beta[n] * phi_m.powi(n as i32);
        }
        amp = amp.max(0.0);
        per = per.max(72_000.0);
        let x = std::f64::consts::TAU * (t - 50_400.0) / per;
        if x.abs() < 1.57 {
            f * (5e-9 + amp * (1.0 - x * x / 2.0 + x.powi(4) / 24.0))
        } else {
            f * 5e-9
        }
    }
}

/// Saastamoinen troposphere delay (m) with a standard atmosphere at height `h` (m), latitude `lat` and elevation `el` (radians).
pub fn saastamoinen(lat: f64, h: f64, el: f64) -> f64 {
    if !(-100.0..=1e4).contains(&h) || el <= 0.0 {
        return 0.0;
    }
    let hgt = h.max(0.0);
    let pres = 1013.25 * (1.0 - 2.2557e-5 * hgt).powf(5.2568);
    let temp = 15.0 - 6.5e-3 * hgt + 273.16;
    let humi = 0.7;
    let e = 6.108 * humi * ((17.15 * temp - 4684.0) / (temp - 38.45)).exp();
    let z = std::f64::consts::FRAC_PI_2 - el;
    let trph =
        0.0022768 * pres / (1.0 - 0.00266 * (2.0 * lat).cos() - 0.00028 * hgt / 1e3) / z.cos();
    let trpw = 0.002277 * (1255.0 / temp + 0.05) * e / z.cos();
    trph + trpw
}

/// Seconds since 1970 on the GPS scale (no leap seconds) of GPS week `week` and seconds `tow`.
pub fn gps_unix(week: i64, tow: f64) -> f64 {
    GPS_EPOCH_UNIX as f64 + week as f64 * WEEK_S + tow
}

/// UTC (seconds since 1970) of GPS week `week` and seconds `tow`, with `leap_s` leap seconds (GPS - UTC).
pub fn utc_unix(week: i64, tow: f64, leap_s: i32) -> f64 {
    gps_unix(week, tow) - leap_s as f64
}

/// GPS - UTC from a leap-second table at the GPS time `week`, `tow`. An error when the time is before the table.
pub fn leap_at(week: i64, tow: f64, table: &LeapSeconds) -> Result<i32, sdrkit_util_geo::Error> {
    let gps = week as f64 * WEEK_S + tow;
    let utc = gtime::utc_from_gps_seconds(gps, table)?;
    Ok((GPS_EPOCH_UNIX as f64 + gps - utc).round() as i32)
}

/// The GPS week of a UTC time (seconds since 1970) by the leap-second table; week 2400 if the time is before late 2023 (a
/// computer clock that was never set).
pub fn week_from_clock(unix_s: f64, table: &LeapSeconds) -> i64 {
    match gtime::gps_from_utc(unix_s, table) {
        Ok(w) if w.week >= 2280 => w.week as i64,
        _ => 2400,
    }
}

/// Resolves a week number counted modulo `modulus` (a power of two: 256, 1024 or 8192) to the full week nearest `near`.
/// An unresolvable week returns `near`.
pub fn resolve_week(wn: i64, modulus: i64, near: i64) -> i64 {
    let bits = modulus.trailing_zeros();
    let w = wn.rem_euclid(modulus) as u32;
    gtime::resolve_week(w, bits, near.max(0) as u32)
        .map(|w| w as i64)
        .unwrap_or(near)
}

/// Wraps a time difference to ±302 400 s.
pub fn wrap_week(dt: f64) -> f64 {
    if dt > 302_400.0 {
        dt - WEEK_S
    } else if dt < -302_400.0 {
        dt + WEEK_S
    } else {
        dt
    }
}

/// Seconds of light travel for `m` metres.
pub fn m_to_s(m: f64) -> f64 {
    m / C
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_and_coordinates() {
        // The GPS epoch, and the week rollover of 1999-08-22 at 13 leap seconds.
        assert_eq!(gps_unix(0, 0.0), 315_964_800.0);
        let t = utc_unix(1024, 0.0, 13);
        assert_eq!(
            sdrkit_util_geo::time::iso8601(t, 0).unwrap().as_str(),
            "1999-08-21T23:59:47Z"
        );
        let table = LeapSeconds::BUILTIN;
        assert_eq!(leap_at(2400, 0.0, &table), Ok(18));
        assert_eq!(leap_at(1024, 0.0, &table), Ok(13));
        assert_eq!(resolve_week(352, 1024, 2393), 2400);
        assert_eq!(resolve_week(1000, 1024, 2393), 2024);
        assert_eq!(resolve_week(100, 1024, 2393), 2148);
        let e = geodetic_to_ecef([0.0, 0.0, 0.0]);
        assert!((e[0] - 6_378_137.0).abs() < 1e-6 && e[1].abs() < 1e-6 && e[2].abs() < 1e-6);
        let g = [0.44, 0.97, 123.4];
        let back = ecef_to_geodetic(geodetic_to_ecef(g));
        assert!(dist(geodetic_to_ecef(back), geodetic_to_ecef(g)) < 1e-5);
        let rx = geodetic_to_ecef(g);
        let up = geodetic_to_ecef([g[0], g[1], g[2] + 20_000e3]);
        assert!(az_el(g, rx, up).1.to_degrees() > 89.99);
        let north = geodetic_to_ecef([g[0] + 0.01, g[1], g[2] + 1000.0]);
        let az = az_el(g, rx, north).0.to_degrees();
        assert!(!(0.5..=359.5).contains(&az), "{az}");
        // Klobuchar at night: 5 ns times the obliquity factor at 90 degrees.
        let k = Klobuchar {
            alpha: [1e-8, 0.0, 0.0, 0.0],
            beta: [90_000.0, 0.0, 0.0, 0.0],
        };
        let night = k.delay_s(0.0, [0.0, 0.0, 0.0], 0.0, std::f64::consts::FRAC_PI_2);
        assert!((night - 5e-9 * (1.0 + 16.0 * (0.53f64 - 0.5).powi(3))).abs() < 1e-15);
        let z = saastamoinen(0.4, 10.0, std::f64::consts::FRAC_PI_2);
        assert!((2.2..2.7).contains(&z), "{z}");
        assert!(saastamoinen(0.4, 10.0, 10f64.to_radians()) > 3.6 * z);
    }
}

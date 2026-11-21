//! Time and sky: sidereal time, galactic and equatorial coordinates, precession, horizontal coordinates, the Sun and the Moon,
//! rise and set times, and the velocity of the observer towards a source (LSR).
//! Standard: J. Meeus, Astronomical Algorithms, 2nd edition

use std::f64::consts::PI;

const D: f64 = PI / 180.0;

/// The Julian date of a Unix time (`sdrkit_util_geo::time::jd_from_unix`).
pub fn jd_unix(unix: f64) -> f64 {
    sdrkit_util_geo::time::jd_from_unix(unix)
}

/// Greenwich mean sidereal time, degrees, at a Julian date (`sdrkit_util_geo::orbit::gmst_rad`).
pub fn gmst_deg(jd: f64) -> f64 {
    sdrkit_util_geo::orbit::gmst_rad(sdrkit_util_geo::time::unix_from_jd(jd))
        .to_degrees()
        .rem_euclid(360.0)
}

/// Local mean sidereal time, degrees.
pub fn lmst_deg(jd: f64, lon: f64) -> f64 {
    (gmst_deg(jd) + lon).rem_euclid(360.0)
}

const RA_GP: f64 = 192.859_48;
const DEC_GP: f64 = 27.128_25;
const L_NCP: f64 = 122.931_92;

/// J2000 RA, Dec (degrees) → galactic l, b.
pub fn eq_to_gal(ra: f64, dec: f64) -> (f64, f64) {
    let (sd, cd) = (dec * D).sin_cos();
    let (sg, cg) = (DEC_GP * D).sin_cos();
    let da = (ra - RA_GP) * D;
    let sb = sd * sg + cd * cg * da.cos();
    let b = sb.clamp(-1.0, 1.0).asin();
    let y = cd * da.sin();
    let x = sd * cg - cd * sg * da.cos();
    let l = (L_NCP - y.atan2(x) / D).rem_euclid(360.0);
    (l, b / D)
}

/// Galactic l, b → J2000 RA, Dec.
pub fn gal_to_eq(l: f64, b: f64) -> (f64, f64) {
    let (sb, cb) = (b * D).sin_cos();
    let (sg, cg) = (DEC_GP * D).sin_cos();
    let dl = (L_NCP - l) * D;
    let sd = sb * sg + cb * cg * dl.cos();
    let dec = sd.clamp(-1.0, 1.0).asin();
    let y = cb * dl.sin();
    let x = sb * cg - cb * sg * dl.cos();
    let ra = (RA_GP + y.atan2(x) / D).rem_euclid(360.0);
    (ra, dec / D)
}

/// IAU 1976 precession of J2000 coordinates to the equinox of `jd`.
pub fn precess(ra: f64, dec: f64, jd: f64) -> (f64, f64) {
    let t = (jd - 2_451_545.0) / 36525.0;
    let s = |a: f64, b: f64, c: f64| (a * t + b * t * t + c * t * t * t) / 3600.0 * D;
    let zeta = s(2306.2181, 0.30188, 0.017998);
    let z = s(2306.2181, 1.09468, 0.018203);
    let theta = s(2004.3109, -0.42665, -0.041833);
    let (ra0, d0) = (ra * D, dec * D);
    let a = d0.cos() * (ra0 + zeta).sin();
    let b = theta.cos() * d0.cos() * (ra0 + zeta).cos() - theta.sin() * d0.sin();
    let c = theta.sin() * d0.cos() * (ra0 + zeta).cos() + theta.cos() * d0.sin();
    let ra1 = (a.atan2(b) + z) / D;
    (ra1.rem_euclid(360.0), c.clamp(-1.0, 1.0).asin() / D)
}

/// Equatorial (of date) → horizontal: azimuth from north and elevation, degrees; optional refraction.
pub fn horizontal(ra: f64, dec: f64, jd: f64, lat: f64, lon: f64, refraction: bool) -> (f64, f64) {
    let h = (lmst_deg(jd, lon) - ra) * D;
    let (sl, cl) = (lat * D).sin_cos();
    let (sd, cd) = (dec * D).sin_cos();
    let alt = (sl * sd + cl * cd * h.cos()).clamp(-1.0, 1.0).asin();
    let az = (-cd * h.sin()).atan2(sd * cl - cd * sl * h.cos());
    let mut el = alt / D;
    if refraction && el > -1.0 {
        el += 1.02 / ((el + 10.3 / (el + 5.11)) * D).tan() / 60.0;
    }
    ((az / D).rem_euclid(360.0), el)
}

/// The Sun's apparent RA, Dec (degrees, low precision, Meeus 25).
pub fn sun(jd: f64) -> (f64, f64) {
    let t = (jd - 2_451_545.0) / 36525.0;
    let l0 = 280.46646 + 36000.76983 * t;
    let m = (357.52911 + 35999.05029 * t) * D;
    let c = (1.914602 - 0.004817 * t) * m.sin()
        + 0.019993 * (2.0 * m).sin()
        + 0.000289 * (3.0 * m).sin();
    let lam = (l0 + c) * D;
    let eps = (23.439291 - 0.0130042 * t) * D;
    let ra = (eps.cos() * lam.sin()).atan2(lam.cos()) / D;
    let dec = (eps.sin() * lam.sin()).asin() / D;
    (ra.rem_euclid(360.0), dec)
}

/// The Moon's RA, Dec (degrees, the main terms of Meeus 47, about 0.3° accuracy).
pub fn moon(jd: f64) -> (f64, f64) {
    let t = (jd - 2_451_545.0) / 36525.0;
    let lp = 218.3164477 + 481267.88123421 * t;
    let d = (297.8501921 + 445267.1114034 * t) * D;
    let m = (357.5291092 + 35999.0502909 * t) * D;
    let mp = (134.9633964 + 477198.8675055 * t) * D;
    let f = (93.2720950 + 483202.0175233 * t) * D;
    let lon = lp
        + 6.288774 * mp.sin()
        + 1.274027 * (2.0 * d - mp).sin()
        + 0.658314 * (2.0 * d).sin()
        + 0.213618 * (2.0 * mp).sin()
        - 0.185116 * m.sin()
        - 0.114332 * (2.0 * f).sin();
    let lat = 5.128122 * f.sin()
        + 0.280602 * (mp + f).sin()
        + 0.277693 * (mp - f).sin()
        + 0.173237 * (2.0 * d - f).sin();
    let eps = 23.439291 * D;
    let (lo, la) = (lon * D, lat * D);
    let ra = (lo.sin() * eps.cos() - la.tan() * eps.sin()).atan2(lo.cos()) / D;
    let dec = (la.sin() * eps.cos() + la.cos() * eps.sin() * lo.sin()).asin() / D;
    (ra.rem_euclid(360.0), dec)
}

/// Named calibrators (J2000).
pub const CALIBRATORS: [(&str, f64, f64); 3] = [
    ("Cas A", 350.8584, 58.8117),
    ("Cyg A", 299.8682, 40.7339),
    ("Sgr A*", 266.416_83, -29.007_81),
];

/// Transit, rise and set (fractions of a day after `jd0`) of a fixed J2000 source; `None` for rise/set when it never sets or
/// never rises.
pub fn rise_transit_set(
    ra: f64,
    dec: f64,
    jd0: f64,
    lat: f64,
    lon: f64,
    h0: f64,
) -> (f64, Option<f64>, Option<f64>) {
    let sidereal = 1.002_737_909_35;
    // Transit: local hour angle zero.
    let lst0 = lmst_deg(jd0, lon);
    let transit = ((ra - lst0).rem_euclid(360.0) / 360.0) / sidereal;
    let (sl, cl) = (lat * D).sin_cos();
    let (sd, cd) = (dec * D).sin_cos();
    let ch = ((h0 * D).sin() - sl * sd) / (cl * cd);
    if !(-1.0..=1.0).contains(&ch) {
        return (transit, None, None);
    }
    let h = ch.acos() / D / 360.0 / sidereal;
    let rise = (transit - h).rem_euclid(1.0 / sidereal).min(1.0);
    let set = (transit + h).rem_euclid(1.0 / sidereal).min(1.0);
    (transit, Some(rise), Some(set))
}

/// A unit vector of galactic l, b.
fn gal_vec(l: f64, b: f64) -> [f64; 3] {
    let (sl, cl) = (l * D).sin_cos();
    let (sb, cb) = (b * D).sin_cos();
    [cb * cl, cb * sl, sb]
}

/// The kinematic (IAU standard) solar motion: 20 km/s towards galactic l 56.1°, b 22.8°.
pub const APEX_KINEMATIC: (f64, f64, f64) = (56.1, 22.8, 20.0);
/// The dynamical solar motion (Schönrich et al. 2010): U, V, W km/s.
pub const SCHOENRICH: [f64; 3] = [11.1, 12.24, 7.25];

/// The Sun's velocity component towards (l, b), km/s (positive: the Sun moves towards the source).
pub fn sun_term(l: f64, b: f64, dynamical: bool) -> f64 {
    let n = gal_vec(l, b);
    if dynamical {
        SCHOENRICH[0] * n[0] + SCHOENRICH[1] * n[1] + SCHOENRICH[2] * n[2]
    } else {
        let a = gal_vec(APEX_KINEMATIC.0, APEX_KINEMATIC.1);
        APEX_KINEMATIC.2 * (a[0] * n[0] + a[1] * n[1] + a[2] * n[2])
    }
}

/// The Earth's orbital speed at a distance r (AU), km/s.
pub fn orbit_speed(r_au: f64) -> f64 {
    let gm = 1.327_124_400_18e11; // km³/s²
    let au = 1.495_978_707e8;
    (gm * (2.0 / (r_au * au) - 1.0 / au)).sqrt()
}

/// The Earth's orbital velocity component towards a J2000 source at `jd`, km/s (heliocentric, the Sun's reflex neglected).
pub fn orbit_term(ra: f64, dec: f64, jd: f64) -> f64 {
    let t = (jd - 2_451_545.0) / 36525.0;
    let (sra, sdec) = sun(jd);
    let m = (357.52911 + 35999.05029 * t) * D;
    let e = 0.016708634;
    let nu = m + (2.0 * e * m.sin());
    let r = 1.000001018 * (1.0 - e * e) / (1.0 + e * nu.cos());
    let v = orbit_speed(r);
    // The Earth moves 90° behind the Sun's direction (in ecliptic longitude) : about along the apex of the Sun minus 90°.
    let lam_sun = {
        let eps = 23.439291 * D;
        let (a, d) = (sra * D, sdec * D);
        (a.sin() * eps.cos() + d.tan() * eps.sin()).atan2(a.cos())
    };
    let apex = lam_sun - PI / 2.0;
    let eps = 23.439291 * D;
    let dir = [apex.cos(), apex.sin() * eps.cos(), apex.sin() * eps.sin()];
    let (a, d) = (ra * D, dec * D);
    let n = [d.cos() * a.cos(), d.cos() * a.sin(), d.sin()];
    v * (dir[0] * n[0] + dir[1] * n[1] + dir[2] * n[2])
}

/// The Earth-rotation component towards a source, km/s (0.4651 km/s × cos latitude at most).
pub fn rotation_term(ra: f64, dec: f64, jd: f64, lat: f64, lon: f64) -> f64 {
    let h = (lmst_deg(jd, lon) - ra) * D;
    0.465_1 * (lat * D).cos() * (dec * D).cos() * h.sin()
}

/// The observer's velocity towards the source (rotation, orbit, Sun): v_LSR = v_topo + total.
pub fn lsr_terms(
    ra: f64,
    dec: f64,
    jd: f64,
    lat: f64,
    lon: f64,
    dynamical: bool,
) -> (f64, f64, f64) {
    let (l, b) = eq_to_gal(ra, dec);
    (
        rotation_term(ra, dec, jd, lat, lon),
        orbit_term(ra, dec, jd),
        sun_term(l, b, dynamical),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meeus_examples() {
        // 1987-04-10 00:00 UT (Meeus example 12.a).
        let jd = 2_446_895.5;
        assert_eq!(jd, 2_446_895.5);
        let g = gmst_deg(jd) / 15.0;
        let want = 13.0 + 10.0 / 60.0 + 46.3668 / 3600.0;
        assert!((g - want).abs() * 3600.0 < 0.01, "{g}");
        let (ra, dec) = gal_to_eq(0.0, 0.0);
        let (l, b) = eq_to_gal(ra, dec);
        assert!((l.rem_euclid(360.0) - 0.0).abs() < 1e-6 || (l - 360.0).abs() < 1e-6);
        assert!(b.abs() < 1e-6);
        let (l, b) = eq_to_gal(266.416_83, -29.007_81);
        assert!(
            (l - 359.944).abs() < 0.001 && (b + 0.046).abs() < 0.001,
            "{l} {b}"
        );
        assert!((sun_term(0.0, 0.0, false) - 10.3).abs() < 0.3);
        assert!((sun_term(0.0, 0.0, true) - 11.1).abs() < 1e-9);
        assert!((orbit_speed(0.98329) - 30.29).abs() < 0.02);
        assert!((orbit_speed(1.01671) - 29.29).abs() < 0.02);
        // The pole's altitude is the latitude.
        let jd = 2_460_000.5;
        let (_, el) = horizontal(0.0, 90.0, jd, 52.0, 5.0, false);
        assert!((el - 52.0).abs() < 1e-9);
        // Transit elevation = 90 − (lat − dec).
        let (tr, rise, set) = rise_transit_set(100.0, 20.0, jd, 52.0, 5.0, 0.0);
        let (_, el) = horizontal(100.0, 20.0, jd + tr, 52.0, 5.0, false);
        assert!((el - (90.0 - (52.0 - 20.0))).abs() < 0.01, "{el}");
        assert!(rise.is_some() && set.is_some());
        let (_, el_r) = horizontal(100.0, 20.0, jd + rise.unwrap(), 52.0, 5.0, false);
        assert!(el_r.abs() < 0.5, "{el_r}");
        // Cas A is circumpolar at high latitude.
        assert!(rise_transit_set(350.8584, 58.8117, jd, 60.0, 0.0, 0.0)
            .1
            .is_none());
    }
}

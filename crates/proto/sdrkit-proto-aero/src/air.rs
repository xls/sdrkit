//! Air data from aircraft reports: the ICAO standard atmosphere, the static air temperature from true airspeed and Mach, the
//! wind triangle and the magnetic declination that turns a magnetic heading into a true heading.
//!
//! Standard: ICAO Doc 7488/3 (Manual of the ICAO Standard Atmosphere); IAGA IGRF-13 (epoch 2020.0)

use crate::error::{Error, Item, Result};

/// The ISA temperature at mean sea level, kelvin (Doc 7488 table A).
pub const T0_K: f64 = 288.15;
/// The ISA pressure at mean sea level, hectopascals (Doc 7488 table A).
pub const P0_HPA: f64 = 1013.25;
/// The speed of sound at ISA mean sea level, knots (340.294 m/s, Doc 7488 table A).
pub const A0_KT: f64 = 340.294 * 3600.0 / 1852.0;
/// The lowest pressure altitude the ISA functions take, metres.
pub const ISA_MIN_M: f64 = -5_000.0;
/// The highest pressure altitude the ISA functions take, metres (the top of the third layer).
pub const ISA_MAX_M: f64 = 32_000.0;

/// Metres per foot.
const M_PER_FT: f64 = 0.3048;
/// Standard gravity, m/s² (Doc 7488).
const G0: f64 = 9.806_65;
/// The gas constant of dry air, J/(kg K) (Doc 7488).
const R_AIR: f64 = 287.052_87;
/// The ISA layers from mean sea level: base altitude (m), base temperature (K), lapse rate (K/m). Doc 7488 table D.
const LAYERS: [(f64, f64, f64); 3] = [
    (0.0, 288.15, -0.0065),
    (11_000.0, 216.65, 0.0),
    (20_000.0, 216.65, 0.001),
];

/// The temperature and pressure of the standard atmosphere at one altitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    /// Temperature, degrees Celsius.
    pub temp_c: f64,
    /// Pressure, hectopascals.
    pub pressure_hpa: f64,
}

/// The ISA temperature and pressure at a pressure altitude in metres (geopotential). The lowest layer extends below sea level.
///
/// An error for an altitude that is not finite or outside [`ISA_MIN_M`] to [`ISA_MAX_M`].
///
/// ```
/// let a = sdrkit_proto_aero::air::isa_m(11_000.0).unwrap();
/// assert!((a.temp_c + 56.5).abs() < 1e-9);
/// assert!((a.pressure_hpa - 226.32).abs() < 0.01);
/// ```
pub fn isa_m(alt_m: f64) -> Result<Atmosphere> {
    if !(ISA_MIN_M..=ISA_MAX_M).contains(&alt_m) {
        return Err(Error::BadField(Item::PressureAltitude));
    }
    let mut p_pa = P0_HPA * 100.0;
    for (i, &(h0, t0, lapse)) in LAYERS.iter().enumerate() {
        let top = LAYERS.get(i + 1).map_or(ISA_MAX_M, |l| l.0);
        let h = alt_m.min(top);
        let t = t0 + lapse * (h - h0);
        p_pa *= if lapse == 0.0 {
            (-G0 * (h - h0) / (R_AIR * t0)).exp()
        } else {
            (t / t0).powf(-G0 / (lapse * R_AIR))
        };
        if alt_m <= top {
            return Ok(Atmosphere {
                temp_c: t - 273.15,
                pressure_hpa: p_pa / 100.0,
            });
        }
    }
    Err(Error::BadField(Item::PressureAltitude))
}

/// The ISA temperature and pressure at a pressure altitude in feet. See [`isa_m`].
pub fn isa_ft(alt_ft: f64) -> Result<Atmosphere> {
    isa_m(alt_ft * M_PER_FT)
}

/// The static air temperature (degrees Celsius) from the true airspeed (knots) and the Mach number. The speed of sound goes
/// with the square root of the temperature, so the temperature is `T0 (TAS / (M a0))²`.
///
/// An error for a speed that is not finite or below zero, or a Mach number that is not above zero.
///
/// ```
/// let sat = sdrkit_proto_aero::air::sat_from_tas_mach(sdrkit_proto_aero::air::A0_KT, 1.0).unwrap();
/// assert!((sat - 15.0).abs() < 1e-9);
/// ```
pub fn sat_from_tas_mach(tas_kt: f64, mach: f64) -> Result<f64> {
    if !(tas_kt.is_finite() && tas_kt >= 0.0) {
        return Err(Error::BadField(Item::TrueAirspeed));
    }
    if !(mach.is_finite() && mach > 0.0) {
        return Err(Error::BadField(Item::MachNumber));
    }
    Ok(T0_K * (tas_kt / (mach * A0_KT)).powi(2) - 273.15)
}

/// A wind: the direction it blows from and its speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    /// The direction the wind blows from, degrees true, 0 to 360.
    pub from_deg: f64,
    /// Speed, knots.
    pub speed_kt: f64,
}

/// The wind from the wind triangle: the ground vector (track and ground speed) minus the air vector (true heading and true
/// airspeed). Angles are degrees true, speeds knots.
///
/// An error for a value that is not finite or a speed below zero.
///
/// ```
/// use sdrkit_proto_aero::air::wind_triangle;
/// // Heading north at 100 kt, carried east of north: a 50 kt wind from the west.
/// let w = wind_triangle(26.565_051, 111.803_399, 0.0, 100.0).unwrap();
/// assert!((w.from_deg - 270.0).abs() < 1e-4 && (w.speed_kt - 50.0).abs() < 1e-4);
/// ```
pub fn wind_triangle(
    track_deg: f64,
    gs_kt: f64,
    heading_true_deg: f64,
    tas_kt: f64,
) -> Result<Wind> {
    if !(track_deg.is_finite() && heading_true_deg.is_finite()) {
        return Err(Error::BadField(Item::Direction));
    }
    if !(gs_kt.is_finite() && gs_kt >= 0.0 && tas_kt.is_finite() && tas_kt >= 0.0) {
        return Err(Error::BadField(Item::Speed));
    }
    let (gt, ht) = (track_deg.to_radians(), heading_true_deg.to_radians());
    let east = gs_kt * gt.sin() - tas_kt * ht.sin();
    let north = gs_kt * gt.cos() - tas_kt * ht.cos();
    Ok(Wind {
        from_deg: (-east).atan2(-north).to_degrees().rem_euclid(360.0),
        speed_kt: east.hypot(north),
    })
}

// IGRF-13 main field, epoch 2020.0, nanotesla: (n, m, g, h).
const IGRF: [(usize, usize, f64, f64); 9] = [
    (1, 0, -29404.8, 0.0),
    (1, 1, -1450.9, 4652.5),
    (2, 0, -2499.6, 0.0),
    (2, 1, 2982.0, -2991.6),
    (2, 2, 1677.0, -734.6),
    (3, 0, 1363.2, 0.0),
    (3, 1, -2381.2, -82.1),
    (3, 2, 1236.2, 241.9),
    (3, 3, 525.7, -543.4),
];

/// The magnetic declination (degrees, east positive) at the surface, from the IGRF-13 main field at epoch 2020.0 cut at degree
/// 3. The cut leaves an error of a few degrees away from the magnetic poles. Add it to a magnetic heading to get a true heading.
///
/// An error for a latitude outside -90 to 90 degrees or a longitude that is not finite.
pub fn declination_deg(lat_deg: f64, lon_deg: f64) -> Result<f64> {
    if !(-90.0..=90.0).contains(&lat_deg) || !lon_deg.is_finite() {
        return Err(Error::BadField(Item::Position));
    }
    let th = (90.0 - lat_deg).to_radians();
    let (x, s) = (th.cos(), th.sin().max(1e-9));
    let ph = lon_deg.to_radians();
    let (r3, r6, r10, r15) = (3f64.sqrt(), 6f64.sqrt(), 10f64.sqrt(), 15f64.sqrt());
    // Schmidt semi-normalised P(n, m) and dP/dtheta.
    let p = |n: usize, m: usize| -> (f64, f64) {
        match (n, m) {
            (1, 0) => (x, -s),
            (1, 1) => (s, x),
            (2, 0) => ((3.0 * x * x - 1.0) / 2.0, -3.0 * x * s),
            (2, 1) => (r3 * x * s, r3 * (x * x - s * s)),
            (2, 2) => (r3 / 2.0 * s * s, r3 * s * x),
            (3, 0) => (
                (5.0 * x * x * x - 3.0 * x) / 2.0,
                -s * (15.0 * x * x - 3.0) / 2.0,
            ),
            (3, 1) => (
                r6 / 4.0 * s * (5.0 * x * x - 1.0),
                r6 / 4.0 * (5.0 * x * x * x - x - 10.0 * x * s * s),
            ),
            (3, 2) => (
                r15 / 2.0 * x * s * s,
                r15 / 2.0 * (2.0 * x * x * s - s * s * s),
            ),
            (3, 3) => (r10 / 4.0 * s * s * s, r10 / 4.0 * 3.0 * s * s * x),
            _ => (0.0, 0.0),
        }
    };
    let (mut bx, mut by) = (0.0, 0.0);
    for &(n, m, g, h) in &IGRF {
        let (pv, dp) = p(n, m);
        let (sm, cm) = ((m as f64) * ph).sin_cos();
        bx += (g * cm + h * sm) * dp;
        by += m as f64 * (g * sm - h * cm) * pv / s;
    }
    Ok(by.atan2(bx).to_degrees())
}

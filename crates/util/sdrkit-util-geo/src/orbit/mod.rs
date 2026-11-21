//! Satellite orbits: TLE parsing, SGP4/SDP4, two-body orbits, sidereal time, the TEME to Earth-fixed rotation, look angles,
//! Doppler shift and pass prediction.
//!
//! ```
//! use sdrkit_util_geo::orbit::{Satellite, tle::{Checksum, Tle}};
//! use sdrkit_util_geo::{Observer, Position};
//! let tle = Tle::parse("ISS (ZARYA)
//! 1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927
//! 2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537", Checksum::Require).unwrap();
//! let sat = Satellite::from_tle(&tle).unwrap();
//! let here = Observer::new(Position::new(51.5, -0.1));
//! let t0 = tle.epoch_unix();
//! let pass = sat.next_pass(&here, t0, t0 + 86_400.0, 0.0).unwrap().expect("the ISS passes over London every day");
//! assert!(pass.los_unix > pass.aos_unix && pass.max_elevation_deg > 0.0);
//! let doppler = sat.doppler_hz(&here, pass.max_unix, 145.8e6).unwrap();
//! assert!(doppler.abs() < 4_000.0);
//! ```

pub mod dopplermatch;
pub mod kepler;
pub mod sgp4;
pub mod tle;

pub use sgp4::{Gravity, OpsMode, Sgp4, TemeState};
pub use tle::Tle;

use crate::time::jd_from_unix;
use crate::{Ecef, Error, Look, Observer, Position, SPEED_OF_LIGHT_M_S};

/// The Earth's rotation rate, rad/s (IERS; the value Vallado uses with SGP4's TEME frame).
pub const EARTH_ROTATION_RAD_S: f64 = 7.292_115_146_706_979e-5;

/// Greenwich mean sidereal time (IAU 1982, as SGP4's TEME frame defines it) at a Unix time on the UT1 scale, radians 0..2π.
/// UTC may be given instead of UT1: the difference is under 0.9 s, about 0.004° of rotation.
pub fn gmst_rad(unix_ut1: f64) -> f64 {
    sgp4::gstime(jd_from_unix(unix_ut1))
}

/// Rotates a TEME state into the Earth-fixed frame (pseudo-Earth-fixed: polar motion, under 15 m, is left out), giving the ECEF
/// position and the ECEF velocity in m/s, at the Unix time (UT1, or UTC within 0.9 s) of the state.
pub fn teme_to_ecef(state: &TemeState, unix_ut1: f64) -> (Ecef, [f64; 3]) {
    let g = gmst_rad(unix_ut1);
    let (s, c) = g.sin_cos();
    let r = state.r_km;
    let v = state.v_km_s;
    let x = (c * r[0] + s * r[1]) * 1e3;
    let y = (-s * r[0] + c * r[1]) * 1e3;
    let z = r[2] * 1e3;
    // v_ecef = R·v_teme − ω × r_ecef
    let vx = (c * v[0] + s * v[1]) * 1e3 + EARTH_ROTATION_RAD_S * y;
    let vy = (-s * v[0] + c * v[1]) * 1e3 - EARTH_ROTATION_RAD_S * x;
    let vz = v[2] * 1e3;
    (Ecef::new(x, y, z), [vx, vy, vz])
}

/// The Doppler shift of a carrier seen across a range rate (positive range rate, moving apart, lowers the frequency): the
/// first-order shift −f·ṙ/c, accurate to (ṙ/c)² ≈ 10⁻⁹ of the carrier for satellites.
pub fn doppler_hz(carrier_hz: f64, range_rate_m_s: f64) -> f64 {
    -carrier_hz * range_rate_m_s / SPEED_OF_LIGHT_M_S
}

/// The straight-line distance from a ground station to a satellite at height `alt_m` seen at elevation `elevation_deg`, m
/// (spherical Earth of the mean radius).
pub fn slant_range_m(alt_m: f64, elevation_deg: f64) -> f64 {
    let re = crate::EARTH_MEAN_RADIUS_M;
    let r = re + alt_m;
    let e = elevation_deg.to_radians();
    (r * r - (re * e.cos()).powi(2)).sqrt() - re * e.sin()
}

/// The ground distance across the area that sees a satellite at height `alt_m` above `elevation_deg`, m (spherical Earth).
pub fn swath_width_m(alt_m: f64, elevation_deg: f64) -> f64 {
    let re = crate::EARTH_MEAN_RADIUS_M;
    let e = elevation_deg.to_radians();
    // The Earth-central angle from the sub-satellite point to the edge.
    let lambda = (re * e.cos() / (re + alt_m)).clamp(-1.0, 1.0).acos() - e;
    2.0 * lambda * re
}

/// The largest Doppler shift of a carrier from a circular orbit at height `alt_m`, seen from the horizon, Hz (μ of
/// [`kepler::MU_EARTH_M3_S2`], spherical Earth).
pub fn max_doppler_hz(carrier_hz: f64, alt_m: f64) -> f64 {
    let re = crate::EARTH_MEAN_RADIUS_M;
    let r = re + alt_m;
    let v = (kepler::MU_EARTH_M3_S2 / r).sqrt();
    // On the horizon the line of sight is tangent to the Earth: its component of the orbital velocity is v·Re/r.
    carrier_hz * v * re / r / SPEED_OF_LIGHT_M_S
}

/// One pass of a satellite over an observer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pass {
    /// Acquisition of signal: when the elevation rises through the minimum, Unix seconds (the window start if it was already up).
    pub aos_unix: f64,
    /// Loss of signal: when the elevation falls through the minimum (the window end if it was still up).
    pub los_unix: f64,
    /// Time of the highest elevation.
    pub max_unix: f64,
    /// The highest elevation, degrees.
    pub max_elevation_deg: f64,
    /// Azimuth at AOS, degrees.
    pub aos_azimuth_deg: f64,
    /// Azimuth at LOS, degrees.
    pub los_azimuth_deg: f64,
    /// The satellite was already above the minimum at the start of the search window.
    pub began_before_window: bool,
    /// The satellite was still above the minimum at the end of the search window.
    pub ends_after_window: bool,
}

/// A satellite propagated by SGP4: positions, look angles, Doppler and passes at Unix times (UTC).
#[derive(Clone, Debug)]
pub struct Satellite {
    sgp4: Sgp4,
}

impl Satellite {
    /// A satellite from a TLE (WGS-72 constants, improved mode).
    pub fn from_tle(tle: &Tle) -> Result<Satellite, Error> {
        Ok(Satellite {
            sgp4: Sgp4::from_tle(tle)?,
        })
    }

    /// A satellite from an initialised propagator.
    pub fn new(sgp4: Sgp4) -> Satellite {
        Satellite { sgp4 }
    }

    /// The propagator.
    pub fn sgp4(&self) -> &Sgp4 {
        &self.sgp4
    }

    /// ECEF position and velocity (m, m/s) at a Unix time.
    pub fn ecef_at(&self, unix: f64) -> Result<(Ecef, [f64; 3]), Error> {
        let s = self.sgp4.propagate_unix(unix)?;
        Ok(teme_to_ecef(&s, unix))
    }

    /// The point below the satellite and its height above the WGS-84 ellipsoid.
    pub fn position_at(&self, unix: f64) -> Result<Position, Error> {
        self.ecef_at(unix)?.0.to_position()
    }

    /// Azimuth, elevation, range and range rate from an observer.
    pub fn look(&self, observer: &Observer, unix: f64) -> Result<Look, Error> {
        let (p, v) = self.ecef_at(unix)?;
        Ok(observer.look_moving(&p, &v))
    }

    /// The Doppler shift of a carrier transmitted by the satellite as the observer receives it, Hz.
    pub fn doppler_hz(
        &self,
        observer: &Observer,
        unix: f64,
        carrier_hz: f64,
    ) -> Result<f64, Error> {
        Ok(doppler_hz(
            carrier_hz,
            self.look(observer, unix)?.range_rate_m_s,
        ))
    }

    fn elevation(&self, observer: &Observer, unix: f64) -> Result<f64, Error> {
        Ok(self.look(observer, unix)?.elevation_deg)
    }

    /// The first pass above `min_elevation_deg` that is in progress or starts between `from_unix` and `until_unix`, or `None`.
    ///
    /// The search steps through the window (one hundredth of an orbit, at most 60 s), refines AOS and LOS by bisection to 10 ms
    /// and the maximum by golden-section search. A local maximum between steps is also examined, so passes shorter than a step
    /// are not missed.
    pub fn next_pass(
        &self,
        observer: &Observer,
        from_unix: f64,
        until_unix: f64,
        min_elevation_deg: f64,
    ) -> Result<Option<Pass>, Error> {
        crate::error::finite(&[from_unix, until_unix, min_elevation_deg])?;
        if until_unix <= from_unix {
            return Ok(None);
        }
        let step = (self.sgp4.period_min() * 60.0 / 100.0).clamp(1.0, 60.0);
        let above =
            |t: f64| -> Result<f64, Error> { Ok(self.elevation(observer, t)? - min_elevation_deg) };
        let mut t0 = from_unix;
        let mut e0 = above(t0)?;
        if e0 >= 0.0 {
            return self
                .finish_pass(observer, until_unix, from_unix, true, min_elevation_deg)
                .map(Some);
        }
        let mut e_prev = f64::NEG_INFINITY;
        while t0 < until_unix {
            let t1 = (t0 + step).min(until_unix);
            let e1 = above(t1)?;
            if e1 >= 0.0 {
                let aos = self.bisect(observer, t0, t1, min_elevation_deg, true)?;
                return self
                    .finish_pass(observer, until_unix, aos, false, min_elevation_deg)
                    .map(Some);
            }
            // A peak between samples that might poke above the minimum.
            if e0 > e_prev && e0 > e1 {
                let lo = (t0 - step).max(from_unix);
                let (tm, em) = self.golden_max(observer, lo, t1)?;
                if em >= min_elevation_deg {
                    let aos = self.bisect(observer, lo, tm, min_elevation_deg, true)?;
                    return self
                        .finish_pass(observer, until_unix, aos, false, min_elevation_deg)
                        .map(Some);
                }
            }
            e_prev = e0;
            t0 = t1;
            e0 = e1;
        }
        Ok(None)
    }

    fn finish_pass(
        &self,
        observer: &Observer,
        until: f64,
        aos: f64,
        began_before: bool,
        min_el: f64,
    ) -> Result<Pass, Error> {
        let step = (self.sgp4.period_min() * 60.0 / 100.0).clamp(1.0, 60.0);
        let mut t = aos;
        let mut los = until;
        let mut ends_after = true;
        while t < until {
            let t1 = (t + step).min(until);
            if self.elevation(observer, t1)? < min_el {
                los = self.bisect(observer, t, t1, min_el, false)?;
                ends_after = false;
                break;
            }
            t = t1;
        }
        let (max_unix, max_elevation_deg) = self.golden_max(observer, aos, los)?;
        Ok(Pass {
            aos_unix: aos,
            los_unix: los,
            max_unix,
            max_elevation_deg,
            aos_azimuth_deg: self.look(observer, aos)?.azimuth_deg,
            los_azimuth_deg: self.look(observer, los)?.azimuth_deg,
            began_before_window: began_before,
            ends_after_window: ends_after,
        })
    }

    /// The crossing of `min_el` between `lo` and `hi` (rising when `rising`), to 10 ms.
    fn bisect(
        &self,
        observer: &Observer,
        mut lo: f64,
        mut hi: f64,
        min_el: f64,
        rising: bool,
    ) -> Result<f64, Error> {
        while hi - lo > 0.01 {
            let mid = 0.5 * (lo + hi);
            let up = self.elevation(observer, mid)? >= min_el;
            if up == rising {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Ok(if rising { hi } else { lo })
    }

    /// The highest elevation between `lo` and `hi` by golden-section search, to 10 ms.
    fn golden_max(
        &self,
        observer: &Observer,
        mut lo: f64,
        mut hi: f64,
    ) -> Result<(f64, f64), Error> {
        let g = 0.5 * (5f64.sqrt() - 1.0);
        let mut a = hi - g * (hi - lo);
        let mut b = lo + g * (hi - lo);
        let mut fa = self.elevation(observer, a)?;
        let mut fb = self.elevation(observer, b)?;
        while hi - lo > 0.01 {
            if fa > fb {
                hi = b;
                b = a;
                fb = fa;
                a = hi - g * (hi - lo);
                fa = self.elevation(observer, a)?;
            } else {
                lo = a;
                a = b;
                fa = fb;
                b = lo + g * (hi - lo);
                fb = self.elevation(observer, b)?;
            }
        }
        let t = 0.5 * (lo + hi);
        Ok((t, self.elevation(observer, t)?))
    }

    /// Every pass between two times, in order, without allocating: an iterator that yields `Result<Pass, Error>` and stops after
    /// the window or at the first error.
    pub fn passes<'a>(
        &'a self,
        observer: &'a Observer,
        from_unix: f64,
        until_unix: f64,
        min_elevation_deg: f64,
    ) -> Passes<'a> {
        Passes {
            sat: self,
            observer,
            next: from_unix,
            until: until_unix,
            min_el: min_elevation_deg,
            done: false,
        }
    }
}

/// The iterator of [`Satellite::passes`].
#[derive(Debug)]
pub struct Passes<'a> {
    sat: &'a Satellite,
    observer: &'a Observer,
    next: f64,
    until: f64,
    min_el: f64,
    done: bool,
}

impl Iterator for Passes<'_> {
    type Item = Result<Pass, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self
            .sat
            .next_pass(self.observer, self.next, self.until, self.min_el)
        {
            Ok(Some(p)) => {
                if p.ends_after_window {
                    self.done = true;
                }
                // Continue just after LOS.
                self.next = p.los_unix + 1.0;
                Some(Ok(p))
            }
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tle::Checksum;
    use super::*;

    const ISS: &str = "ISS (ZARYA)
1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927
2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537";

    #[test]
    fn teme_to_ecef_keeps_lengths_and_matches_the_rotation() {
        let s = TemeState {
            r_km: [7000.0, 0.0, 100.0],
            v_km_s: [0.0, 7.5, 0.0],
        };
        let t = 1_000_000_000.0;
        let (p, v) = teme_to_ecef(&s, t);
        let r = (p.x_m.powi(2) + p.y_m.powi(2) + p.z_m.powi(2)).sqrt();
        assert!((r - (7000.0f64.powi(2) + 100.0f64.powi(2)).sqrt() * 1e3).abs() < 1e-6);
        let g = gmst_rad(t);
        assert!(
            (p.y_m.atan2(p.x_m) + g)
                .rem_euclid(std::f64::consts::TAU)
                .min((p.y_m.atan2(p.x_m) + g - std::f64::consts::TAU).abs())
                < 1e-12
        );
        // A point fixed on the Earth has zero ECEF velocity.
        let w = EARTH_ROTATION_RAD_S;
        let fixed = TemeState {
            r_km: [6378.0, 0.0, 0.0],
            v_km_s: [0.0, w * 6378.0, 0.0],
        };
        let (_, v0) = teme_to_ecef(&fixed, 0.0);
        assert!(v0.iter().all(|x| x.abs() < 1e-9), "{v0:?}");
        assert!(v.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn spherical_geometry() {
        assert!((slant_range_m(500e3, 90.0) - 500e3).abs() < 1e-6);
        // On the horizon the slant range is the tangent length sqrt((Re + h)² − Re²).
        let re = crate::EARTH_MEAN_RADIUS_M;
        assert!((slant_range_m(500e3, 0.0) - ((re + 500e3).powi(2) - re * re).sqrt()).abs() < 1e-6);
        let s = swath_width_m(500e3, 0.0);
        assert!((s - 2.0 * re * (re / (re + 500e3)).acos()).abs() < 1e-6);
        let d = max_doppler_hz(162e6, 500e3);
        assert!((3_600.0..4_200.0).contains(&d), "{d}");
    }

    #[test]
    fn doppler_sign_and_size() {
        // Approaching at 7 km/s: +3.4 kHz at 145.8 MHz.
        let d = doppler_hz(145.8e6, -7_000.0);
        assert!((d - 145.8e6 * 7_000.0 / SPEED_OF_LIGHT_M_S).abs() < 1e-9 && d > 3_000.0);
    }

    #[test]
    fn iss_passes() {
        let tle = Tle::parse(ISS, Checksum::Require).unwrap();
        let sat = Satellite::from_tle(&tle).unwrap();
        let obs = Observer::new(Position::with_alt(51.5, -0.1, 20.0));
        let t0 = tle.epoch_unix();
        let mut n = 0;
        let mut last_los = t0;
        for p in sat.passes(&obs, t0, t0 + 2.0 * 86_400.0, 10.0) {
            let p = p.unwrap();
            n += 1;
            assert!(
                p.aos_unix >= last_los
                    && p.los_unix > p.aos_unix
                    && p.los_unix - p.aos_unix < 900.0,
                "{p:?}"
            );
            assert!(p.max_unix >= p.aos_unix && p.max_unix <= p.los_unix);
            assert!(p.max_elevation_deg >= 10.0 && p.max_elevation_deg <= 90.0);
            // The edges are at the minimum elevation, and the satellite is approaching at AOS and receding at LOS.
            assert!((sat.look(&obs, p.aos_unix).unwrap().elevation_deg - 10.0).abs() < 0.01);
            assert!((sat.look(&obs, p.los_unix).unwrap().elevation_deg - 10.0).abs() < 0.01);
            assert!(sat.doppler_hz(&obs, p.aos_unix, 145.8e6).unwrap() > 0.0);
            assert!(sat.doppler_hz(&obs, p.los_unix, 145.8e6).unwrap() < 0.0);
            // At the highest point the range rate passes through zero.
            assert!(sat.look(&obs, p.max_unix).unwrap().range_rate_m_s.abs() < 300.0);
            last_los = p.los_unix;
        }
        // The ISS (51.6° inclination) is seen above 10° from London several times a day.
        assert!((4..=14).contains(&n), "{n}");
        let h = sat.position_at(t0).unwrap().alt_m;
        assert!((300_000.0..450_000.0).contains(&h), "{h}");
        assert!(sat.next_pass(&obs, t0, t0, 0.0).unwrap().is_none());
    }
}

//! Two-body (Keplerian) orbits: classical elements from a state vector and back, and propagation by Kepler's equation, for
//! closed (elliptical) orbits. For satellites with a NORAD element set use [`crate::orbit::Sgp4`].
//!
//! Standard: Vallado, *Fundamentals of Astrodynamics and Applications*, algorithms 9 (`rv2coe`) and 10 (`coe2rv`).

use crate::error::finite;
use crate::Error;
use crate::Quantity;
use std::f64::consts::TAU;

/// The Earth's gravitational parameter μ = GM, m³/s² (IERS Conventions 2010, as used by WGS-84 (G1762) and Galileo).
pub const MU_EARTH_M3_S2: f64 = 3.986_004_418e14;

/// Classical elements of an elliptical orbit at an epoch, in an inertial frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kepler {
    /// Semi-major axis, m.
    pub a_m: f64,
    /// Eccentricity, 0 to below 1.
    pub e: f64,
    /// Inclination, radians.
    pub i_rad: f64,
    /// Right ascension of the ascending node, radians.
    pub raan_rad: f64,
    /// Argument of perigee, radians.
    pub argp_rad: f64,
    /// Mean anomaly at the epoch, radians.
    pub m0_rad: f64,
    /// Epoch, Unix seconds.
    pub epoch_unix: f64,
    /// Gravitational parameter, m³/s².
    pub mu: f64,
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: &[f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

impl Kepler {
    /// The elements of the orbit through position `r_m` with velocity `v_m_s` at `epoch_unix`. Errors for a parabolic or
    /// hyperbolic orbit, a zero vector, or non-finite input. Circular and equatorial orbits are handled (the undefined angles are
    /// set to 0 and the anomaly measured from the node or the x axis).
    ///
    /// ```
    /// use sdrkit_util_geo::orbit::kepler::{Kepler, MU_EARTH_M3_S2};
    /// // Vallado, example 2-5.
    /// let k = Kepler::from_state(&[6_524_834.0, 6_862_875.0, 6_448_296.0], &[4_901.327, 5_533.756, -1_976.341], 0.0,
    ///     MU_EARTH_M3_S2).unwrap();
    /// assert!((k.a_m - 36_127_343.0).abs() < 1_000.0 && (k.e - 0.832_853).abs() < 1e-6);
    /// ```
    pub fn from_state(
        r_m: &[f64; 3],
        v_m_s: &[f64; 3],
        epoch_unix: f64,
        mu: f64,
    ) -> Result<Kepler, Error> {
        finite(&[
            r_m[0], r_m[1], r_m[2], v_m_s[0], v_m_s[1], v_m_s[2], epoch_unix, mu,
        ])?;
        let r = norm(r_m);
        let v = norm(v_m_s);
        if r == 0.0 || mu <= 0.0 {
            return Err(Error::OutOfRange(Quantity::StateVector));
        }
        let h = cross(r_m, v_m_s);
        let hn = norm(&h);
        if hn == 0.0 {
            return Err(Error::OutOfRange(Quantity::RadialTrajectory));
        }
        let n = [-h[1], h[0], 0.0];
        let nn = norm(&n);
        let rv = dot(r_m, v_m_s);
        let c1 = v * v - mu / r;
        let ev = [
            (c1 * r_m[0] - rv * v_m_s[0]) / mu,
            (c1 * r_m[1] - rv * v_m_s[1]) / mu,
            (c1 * r_m[2] - rv * v_m_s[2]) / mu,
        ];
        let e = norm(&ev);
        let energy = v * v / 2.0 - mu / r;
        if e >= 1.0 || energy >= 0.0 {
            return Err(Error::OutOfRange(Quantity::OrbitNotClosed));
        }
        let a = -mu / (2.0 * energy);
        let i = (h[2] / hn).clamp(-1.0, 1.0).acos();
        let small = 1e-11;
        let equatorial = nn < small * hn;
        let circular = e < small;
        let raan = if equatorial {
            0.0
        } else {
            (n[1]).atan2(n[0]).rem_euclid(TAU)
        };
        // Argument of perigee and true anomaly (or the substitutes for circular and equatorial orbits).
        let (argp, nu) = if !circular {
            let argp = if equatorial {
                // Longitude of perigee, measured from x.
                let w = ev[1].atan2(ev[0]);
                if h[2] < 0.0 {
                    TAU - w
                } else {
                    w
                }
            } else {
                let c = (dot(&n, &ev) / (nn * e)).clamp(-1.0, 1.0).acos();
                if ev[2] < 0.0 {
                    TAU - c
                } else {
                    c
                }
            };
            let c = (dot(&ev, r_m) / (e * r)).clamp(-1.0, 1.0).acos();
            (argp, if rv < 0.0 { TAU - c } else { c })
        } else if !equatorial {
            // Argument of latitude, from the node.
            let c = (dot(&n, r_m) / (nn * r)).clamp(-1.0, 1.0).acos();
            (0.0, if r_m[2] < 0.0 { TAU - c } else { c })
        } else {
            // True longitude, from x.
            let l = r_m[1].atan2(r_m[0]);
            (0.0, if h[2] < 0.0 { TAU - l } else { l })
        };
        // True to mean anomaly.
        let ea = ((1.0 - e * e).sqrt() * nu.sin()).atan2(e + nu.cos());
        let m0 = (ea - e * ea.sin()).rem_euclid(TAU);
        Ok(Kepler {
            a_m: a,
            e,
            i_rad: i,
            raan_rad: raan,
            argp_rad: argp.rem_euclid(TAU),
            m0_rad: m0,
            epoch_unix,
            mu,
        })
    }

    /// Mean motion, radians per second.
    pub fn mean_motion_rad_s(&self) -> f64 {
        (self.mu / (self.a_m * self.a_m * self.a_m)).sqrt()
    }

    /// Orbital period, s.
    pub fn period_s(&self) -> f64 {
        TAU / self.mean_motion_rad_s()
    }

    /// Position (m) and velocity (m/s) in the inertial frame of the elements at Unix time `unix`.
    pub fn state_at(&self, unix: f64) -> Result<([f64; 3], [f64; 3]), Error> {
        finite(&[
            unix,
            self.a_m,
            self.e,
            self.i_rad,
            self.raan_rad,
            self.argp_rad,
            self.m0_rad,
            self.mu,
        ])?;
        if !(0.0..1.0).contains(&self.e) || self.a_m <= 0.0 || self.mu <= 0.0 {
            return Err(Error::OutOfRange(Quantity::Elements));
        }
        let e = self.e;
        let n = self.mean_motion_rad_s();
        let m = (self.m0_rad + n * (unix - self.epoch_unix)).rem_euclid(TAU);
        // Kepler's equation by Newton's method from a starting value that converges for every eccentricity below 1.
        let mut ea = if e < 0.8 { m } else { std::f64::consts::PI };
        let mut converged = false;
        for _ in 0..50 {
            let f = ea - e * ea.sin() - m;
            let step = f / (1.0 - e * ea.cos());
            ea -= step;
            if step.abs() < 1e-15 {
                converged = true;
                break;
            }
        }
        if !converged {
            return Err(Error::NoConvergence);
        }
        let (se, ce) = ea.sin_cos();
        let b = self.a_m * (1.0 - e * e).sqrt();
        // Position and velocity in the perifocal frame.
        let r = self.a_m * (1.0 - e * ce);
        let pf = [self.a_m * (ce - e), b * se];
        // dE/dt = n·a / r, so the perifocal velocity is (−a·sin E, b·cos E)·n·a / r.
        let k = self.a_m * n / r;
        let vf = [-k * self.a_m * se, k * b * ce];
        let (so, co) = self.raan_rad.sin_cos();
        let (sw, cw) = self.argp_rad.sin_cos();
        let (si, ci) = self.i_rad.sin_cos();
        let p = [co * cw - so * sw * ci, so * cw + co * sw * ci, sw * si];
        let q = [-co * sw - so * cw * ci, -so * sw + co * cw * ci, cw * si];
        let rot = |x: f64, y: f64| {
            [
                p[0] * x + q[0] * y,
                p[1] * x + q[1] * y,
                p[2] * x + q[2] * y,
            ]
        };
        Ok((rot(pf[0], pf[1]), rot(vf[0], vf[1])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MU_KM: f64 = 398_600.441_8;

    #[test]
    fn vallado_example_2_5_state_to_elements() {
        let k = Kepler::from_state(
            &[6_524_834.0, 6_862_875.0, 6_448_296.0],
            &[4_901.327, 5_533.756, -1_976.341],
            0.0,
            MU_EARTH_M3_S2,
        )
        .unwrap();
        // Published: p = 11 067.790 km, a = 36 127.343 km, e = 0.832 853, i = 87.870°, Ω = 227.898°, ω = 53.38°, ν = 92.335°.
        assert!((k.a_m / 1e3 - 36_127.343).abs() < 0.5, "{k:?}");
        assert!(
            (k.a_m / 1e3 * (1.0 - k.e * k.e) - 11_067.790).abs() < 0.05,
            "{k:?}"
        );
        assert!((k.e - 0.832_853).abs() < 1e-6);
        assert!((k.i_rad.to_degrees() - 87.870).abs() < 1e-3);
        assert!((k.raan_rad.to_degrees() - 227.898).abs() < 1e-3);
        assert!((k.argp_rad.to_degrees() - 53.38).abs() < 0.01);
        let (r, v) = k.state_at(0.0).unwrap();
        for (a, b) in r.iter().zip([6_524_834.0, 6_862_875.0, 6_448_296.0]) {
            assert!((a - b).abs() < 1e-3, "{r:?}");
        }
        for (a, b) in v.iter().zip([4_901.327, 5_533.756, -1_976.341]) {
            assert!((a - b).abs() < 1e-6, "{v:?}");
        }
    }

    #[test]
    fn vallado_example_2_4_kepler_problem() {
        // Vallado, example 2-4: propagate 40 minutes.
        let r0 = [1_131_340.0, -2_282_343.0, 6_672_423.0];
        let v0 = [-5_643.05, 4_303.33, 2_428.79];
        let k = Kepler::from_state(&r0, &v0, 0.0, MU_KM * 1e9).unwrap();
        let (r, v) = k.state_at(2_400.0).unwrap();
        let want_r = [-4_219_752.7, 4_363_029.2, -3_958_766.6];
        let want_v = [3_689.866, -1_916.735, -6_112.511];
        for (a, b) in r.iter().zip(want_r) {
            assert!((a - b).abs() < 1.0, "{r:?}");
        }
        for (a, b) in v.iter().zip(want_v) {
            assert!((a - b).abs() < 1e-3, "{v:?}");
        }
    }

    #[test]
    fn agrees_with_numerical_integration() {
        // An independent check: integrate r'' = -μ r / |r|³ with fourth-order Runge-Kutta.
        let r0 = [7_000_000.0, -1_200_000.0, 300_000.0];
        let v0 = [1_000.0, 7_200.0, 2_500.0];
        let k = Kepler::from_state(&r0, &v0, 0.0, MU_EARTH_M3_S2).unwrap();
        let acc = |r: &[f64; 3]| {
            let d = norm(r).powi(3);
            [
                -MU_EARTH_M3_S2 * r[0] / d,
                -MU_EARTH_M3_S2 * r[1] / d,
                -MU_EARTH_M3_S2 * r[2] / d,
            ]
        };
        let (mut r, mut v) = (r0, v0);
        let h = 0.5;
        for _ in 0..(6_000.0 / h) as usize {
            let add = |a: &[f64; 3], b: &[f64; 3], s: f64| {
                [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2]]
            };
            let (k1r, k1v) = (v, acc(&r));
            let (k2r, k2v) = (add(&v, &k1v, h / 2.0), acc(&add(&r, &k1r, h / 2.0)));
            let (k3r, k3v) = (add(&v, &k2v, h / 2.0), acc(&add(&r, &k2r, h / 2.0)));
            let (k4r, k4v) = (add(&v, &k3v, h), acc(&add(&r, &k3r, h)));
            for i in 0..3 {
                r[i] += h / 6.0 * (k1r[i] + 2.0 * k2r[i] + 2.0 * k3r[i] + k4r[i]);
                v[i] += h / 6.0 * (k1v[i] + 2.0 * k2v[i] + 2.0 * k3v[i] + k4v[i]);
            }
        }
        let (rk, vk) = k.state_at(6_000.0).unwrap();
        for i in 0..3 {
            assert!((rk[i] - r[i]).abs() < 0.01, "{rk:?} {r:?}");
            assert!((vk[i] - v[i]).abs() < 1e-5, "{vk:?} {v:?}");
        }
        assert!((k.period_s() - TAU / k.mean_motion_rad_s()).abs() < 1e-9);
    }

    #[test]
    fn special_and_bad_orbits() {
        // Circular equatorial: every angle collapses into the true longitude.
        let r = 7_000_000.0;
        let v = (MU_EARTH_M3_S2 / r).sqrt();
        let k = Kepler::from_state(&[0.0, r, 0.0], &[-v, 0.0, 0.0], 0.0, MU_EARTH_M3_S2).unwrap();
        assert!(k.e < 1e-12 && k.i_rad.abs() < 1e-12);
        let (p, _) = k.state_at(0.0).unwrap();
        assert!((p[0]).abs() < 1e-3 && (p[1] - r).abs() < 1e-3);
        // Circular inclined.
        let k = Kepler::from_state(&[r, 0.0, 0.0], &[0.0, 0.0, v], 0.0, MU_EARTH_M3_S2).unwrap();
        assert!((k.i_rad.to_degrees() - 90.0).abs() < 1e-9);
        let (p, _) = k.state_at(k.period_s() / 4.0).unwrap();
        assert!(p[0].abs() < 1e-3 && (p[2] - r).abs() < 1e-3);
        // Escape speed: refused.
        assert!(
            Kepler::from_state(&[r, 0.0, 0.0], &[0.0, v * 1.5, 0.0], 0.0, MU_EARTH_M3_S2).is_err()
        );
        assert!(Kepler::from_state(&[0.0; 3], &[0.0, 1.0, 0.0], 0.0, MU_EARTH_M3_S2).is_err());
        assert!(Kepler::from_state(&[r, 0.0, 0.0], &[1.0, 0.0, 0.0], 0.0, MU_EARTH_M3_S2).is_err());
        assert!(
            Kepler::from_state(&[f64::NAN, 0.0, 0.0], &[0.0, 1.0, 0.0], 0.0, MU_EARTH_M3_S2)
                .is_err()
        );
        // A very eccentric orbit still solves Kepler's equation.
        let k = Kepler {
            a_m: 4e7,
            e: 0.999,
            i_rad: 0.1,
            raan_rad: 0.2,
            argp_rad: 0.3,
            m0_rad: 0.0,
            epoch_unix: 0.0,
            mu: MU_EARTH_M3_S2,
        };
        assert!(k.state_at(123.0).is_ok());
        assert!(Kepler { e: 1.0, ..k }.state_at(0.0).is_err());
    }
}

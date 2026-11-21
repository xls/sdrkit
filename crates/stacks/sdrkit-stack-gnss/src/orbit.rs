//! Broadcast ephemerides and the orbit and clock models: the Kepler set of GPS, QZSS, Galileo and BeiDou (with BeiDou's GEO
//! algorithm), the GLONASS state vector integrated by fourth-order Runge-Kutta, and the GPS almanac.
//!
//! Standard: IS-GPS-200 Table 20-IV, BDS SIS ICD B1I Table 5-11, GLONASS ICD Appendix A.3.1.2.

use crate::geo::wrap_week;
use crate::system::*;
use serde::{Deserialize, Serialize};

/// A broadcast Kepler ephemeris with its clock. Angles in radians, times in seconds of the system's week.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kepler {
    pub system: Option<System>,
    pub prn: u16,
    /// The full week of toe (GPS numbering for GPS and QZSS, GST week + 1024 for Galileo, BDT week for BeiDou).
    pub week: i64,
    pub toe: f64,
    pub toc: f64,
    pub sqrt_a: f64,
    pub e: f64,
    pub m0: f64,
    pub delta_n: f64,
    pub omega0: f64,
    pub i0: f64,
    pub omega: f64,
    pub omega_dot: f64,
    pub idot: f64,
    pub cuc: f64,
    pub cus: f64,
    pub crc: f64,
    pub crs: f64,
    pub cic: f64,
    pub cis: f64,
    pub af0: f64,
    pub af1: f64,
    pub af2: f64,
    /// The group delay applied for this signal (TGD, BGD(E1,E5b), TGD1), s.
    pub tgd: f64,
    /// IODE (GPS), IODnav (Galileo), AODE (BeiDou).
    pub iode: u32,
    pub iodc: u32,
    /// Non-zero means unhealthy.
    pub health: u32,
    pub ura: u32,
    pub fit: u32,
    /// BeiDou GEO: the GEO algorithm applies.
    pub geo: bool,
}

/// A satellite position, its clock and how it was computed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SatState {
    /// ECEF at the requested instant (earth-fixed frame of that instant), m.
    pub pos: [f64; 3],
    /// ECEF velocity, m/s.
    pub vel: [f64; 3],
    /// Satellite clock offset (including the relativistic term, without the group delay), s.
    pub clock: f64,
    /// Clock drift, s/s.
    pub drift: f64,
}

impl Kepler {
    fn constants(&self) -> (f64, f64, f64) {
        match self.system {
            Some(System::Galileo) => (MU_GAL, OMEGA_E, F_GAL),
            Some(System::Beidou) => (MU_GAL, OMEGA_E_BDS, F_GAL),
            _ => (MU_GPS, OMEGA_E, F_GPS),
        }
    }

    /// Eccentric anomaly at `t` (seconds of week), and tk.
    fn anomaly(&self, t: f64) -> (f64, f64) {
        let (mu, _, _) = self.constants();
        let a = self.sqrt_a * self.sqrt_a;
        let tk = wrap_week(t - self.toe);
        let n = (mu / (a * a * a)).sqrt() + self.delta_n;
        let m = self.m0 + n * tk;
        let mut e = m;
        for _ in 0..30 {
            let de = (e - self.e * e.sin() - m) / (1.0 - self.e * e.cos());
            e -= de;
            if de.abs() < 1e-14 {
                break;
            }
        }
        (e, tk)
    }

    /// The ECEF position at `t` (seconds of the system's week), the frame being that of the instant `t`.
    pub fn position(&self, t: f64) -> [f64; 3] {
        let (_, we, _) = self.constants();
        let a = self.sqrt_a * self.sqrt_a;
        let (ek, tk) = self.anomaly(t);
        let nu = ((1.0 - self.e * self.e).sqrt() * ek.sin()).atan2(ek.cos() - self.e);
        let phi = nu + self.omega;
        let (s2, c2) = (2.0 * phi).sin_cos();
        let u = phi + self.cus * s2 + self.cuc * c2;
        let r = a * (1.0 - self.e * ek.cos()) + self.crs * s2 + self.crc * c2;
        let i = self.i0 + self.idot * tk + self.cis * s2 + self.cic * c2;
        let (x, y) = (r * u.cos(), r * u.sin());
        if self.geo {
            let om = self.omega0 + self.omega_dot * tk - we * self.toe;
            let (so, co) = om.sin_cos();
            let (si, ci) = i.sin_cos();
            let xg = x * co - y * ci * so;
            let yg = x * so + y * ci * co;
            let zg = y * si;
            let (sf, cf) = (-5f64.to_radians()).sin_cos();
            // Rx(-5 deg)
            let (x1, y1, z1) = (xg, cf * yg + sf * zg, -sf * yg + cf * zg);
            let (sw, cw) = (we * tk).sin_cos();
            return [cw * x1 + sw * y1, -sw * x1 + cw * y1, z1];
        }
        let om = self.omega0 + (self.omega_dot - we) * tk - we * self.toe;
        let (so, co) = om.sin_cos();
        let (si, ci) = i.sin_cos();
        [x * co - y * ci * so, x * so + y * ci * co, y * si]
    }

    /// The satellite clock offset at `t` (af0 + af1·dt + af2·dt² + the relativistic term), s.
    pub fn clock(&self, t: f64) -> f64 {
        let (_, _, f) = self.constants();
        let (ek, _) = self.anomaly(t);
        let dt = wrap_week(t - self.toc);
        self.af0 + self.af1 * dt + self.af2 * dt * dt + f * self.e * self.sqrt_a * ek.sin()
    }

    /// Position, velocity and clock at `t`.
    pub fn state(&self, t: f64) -> SatState {
        let h = 0.01;
        let (a, b) = (self.position(t - h), self.position(t + h));
        let (ca, cb) = (self.clock(t - h), self.clock(t + h));
        SatState {
            pos: self.position(t),
            vel: [
                (b[0] - a[0]) / (2.0 * h),
                (b[1] - a[1]) / (2.0 * h),
                (b[2] - a[2]) / (2.0 * h),
            ],
            clock: self.clock(t),
            drift: (cb - ca) / (2.0 * h),
        }
    }

    /// Seconds from the ephemeris epoch (toe) to `t`, wrapped.
    pub fn age(&self, t: f64) -> f64 {
        wrap_week(t - self.toe)
    }
}

/// A GPS almanac entry (IS-GPS-200 20.3.3.5.1.2), angles in radians.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Almanac {
    pub prn: u16,
    pub e: f64,
    pub toa: f64,
    pub delta_i: f64,
    pub omega_dot: f64,
    pub sqrt_a: f64,
    pub omega0: f64,
    pub omega: f64,
    pub m0: f64,
    pub af0: f64,
    pub af1: f64,
    pub health: u32,
    /// The almanac week (full).
    pub week: i64,
}

impl Almanac {
    /// The Kepler set it stands for (inclination of 0.3 semicircles plus δi, no harmonic terms).
    pub fn kepler(&self) -> Kepler {
        Kepler {
            system: Some(System::Gps),
            prn: self.prn,
            week: self.week,
            toe: self.toa,
            toc: self.toa,
            sqrt_a: self.sqrt_a,
            e: self.e,
            m0: self.m0,
            omega0: self.omega0,
            i0: 0.3 * GPS_PI + self.delta_i,
            omega: self.omega,
            omega_dot: self.omega_dot,
            af0: self.af0,
            af1: self.af1,
            health: self.health,
            ..Default::default()
        }
    }
    /// ECEF position at `t` (seconds of week).
    pub fn position(&self, t: f64) -> [f64; 3] {
        self.kepler().position(t)
    }
}

/// A GLONASS immediate ephemeris (strings 1 to 4), PZ-90, metres and seconds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GloEphemeris {
    /// Frequency channel.
    pub k: i8,
    /// Slot number n (1 to 24; 0 unknown).
    pub slot: u16,
    /// tb, seconds of the GLONASS day (Moscow time).
    pub tb: f64,
    /// tk, seconds of the day at the start of the frame.
    pub tk: f64,
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub acc: [f64; 3],
    pub tau_n: f64,
    pub gamma_n: f64,
    pub delta_tau_n: f64,
    pub bn: u32,
    pub p1: u32,
    pub p2: u32,
    pub p3: u32,
    pub p4: u32,
    pub p: u32,
    pub ln: u32,
    pub ft: u32,
    pub nt: u32,
    pub m: u32,
    pub en: u32,
}

/// GLONASS equations of motion: derivative of (pos, vel).
pub fn glo_deriv(s: &[f64; 6], acc: &[f64; 3]) -> [f64; 6] {
    let (x, y, z) = (s[0], s[1], s[2]);
    let (vx, vy, vz) = (s[3], s[4], s[5]);
    let r2 = x * x + y * y + z * z;
    let r = r2.sqrt();
    let r3 = r2 * r;
    let a = 1.5 * J2_GLO * MU_GLO * AE_GLO * AE_GLO / (r2 * r3);
    let zz = 5.0 * z * z / r2;
    let w2 = OMEGA_E_GLO * OMEGA_E_GLO;
    [
        vx,
        vy,
        vz,
        -MU_GLO / r3 * x - a * x * (1.0 - zz) + w2 * x + 2.0 * OMEGA_E_GLO * vy + acc[0],
        -MU_GLO / r3 * y - a * y * (1.0 - zz) + w2 * y - 2.0 * OMEGA_E_GLO * vx + acc[1],
        -MU_GLO / r3 * z - a * z * (3.0 - zz) + acc[2],
    ]
}

impl GloEphemeris {
    /// Position and velocity at `t` (seconds of the GLONASS day), integrated from tb by RK4 in steps of at most `max_step` s.
    pub fn integrate(&self, t: f64, max_step: f64) -> ([f64; 3], [f64; 3]) {
        let mut dt = t - self.tb;
        if dt > 43_200.0 {
            dt -= 86_400.0;
        } else if dt < -43_200.0 {
            dt += 86_400.0;
        }
        let mut s = [
            self.pos[0],
            self.pos[1],
            self.pos[2],
            self.vel[0],
            self.vel[1],
            self.vel[2],
        ];
        let n = (dt.abs() / max_step).ceil().max(1.0) as usize;
        let h = dt / n as f64;
        if dt != 0.0 {
            for _ in 0..n {
                let k1 = glo_deriv(&s, &self.acc);
                let s2: [f64; 6] = std::array::from_fn(|i| s[i] + h / 2.0 * k1[i]);
                let k2 = glo_deriv(&s2, &self.acc);
                let s3: [f64; 6] = std::array::from_fn(|i| s[i] + h / 2.0 * k2[i]);
                let k3 = glo_deriv(&s3, &self.acc);
                let s4: [f64; 6] = std::array::from_fn(|i| s[i] + h * k3[i]);
                let k4 = glo_deriv(&s4, &self.acc);
                for i in 0..6 {
                    s[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
                }
            }
        }
        ([s[0], s[1], s[2]], [s[3], s[4], s[5]])
    }
    /// Satellite clock offset at `t` (seconds of day): −τn + γn·(t − tb).
    pub fn clock(&self, t: f64) -> f64 {
        -self.tau_n + self.gamma_n * (t - self.tb)
    }
    /// Healthy when Bn's high bit and ln are 0.
    pub fn healthy(&self) -> bool {
        self.bn & 4 == 0 && self.ln == 0
    }
    /// Seconds from tb to `t` (seconds of day), wrapped to ±12 h.
    pub fn age(&self, t: f64) -> f64 {
        let mut d = t - self.tb;
        if d > 43_200.0 {
            d -= 86_400.0;
        } else if d < -43_200.0 {
            d += 86_400.0;
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circular() -> Kepler {
        Kepler {
            system: Some(System::Gps),
            sqrt_a: 26_560_000f64.sqrt(),
            i0: 0.96,
            omega0: 1.0,
            toe: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn orbit_models() {
        let k = circular();
        // Circular orbit: radius constant and equal to a; matches the closed form at t.
        let a = 26_560_000.0;
        for t in [0.0, 1000.0, 20_000.0] {
            let p = k.position(t);
            let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((r - a).abs() < 1e-6, "{r}");
            let n = (MU_GPS / (a * a * a)).sqrt();
            let u = n * t;
            let om = 1.0 - OMEGA_E * t;
            let (x, y) = (a * u.cos(), a * u.sin());
            let cf = [
                x * om.cos() - y * 0.96f64.cos() * om.sin(),
                x * om.sin() + y * 0.96f64.cos() * om.cos(),
                y * 0.96f64.sin(),
            ];
            assert!(crate::geo::dist(cf, p) < 1e-6);
        }
        // Eccentric: radius from Kepler's equation; energy conserved.
        let mut k = circular();
        k.e = 0.01;
        let mut energies = Vec::new();
        for t in (0..40_000).step_by(4000) {
            let s = k.state(t as f64);
            // Inertial speed: add the earth rotation back.
            let v = [
                s.vel[0] - OMEGA_E * s.pos[1],
                s.vel[1] + OMEGA_E * s.pos[0],
                s.vel[2],
            ];
            let r = (s.pos[0].powi(2) + s.pos[1].powi(2) + s.pos[2].powi(2)).sqrt();
            let (e, _) = k.anomaly(t as f64);
            assert!((r - a * (1.0 - 0.01 * e.cos())).abs() < 1e-3);
            energies.push((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) / 2.0 - MU_GPS / r);
        }
        let e0 = energies[0];
        assert!(energies.iter().all(|e| ((e - e0) / e0).abs() < 2e-5));
    }

    #[test]
    fn glonass_integration_holds_a_circular_orbit() {
        let r = 25_510_000.0;
        let v = (MU_GLO / r).sqrt();
        // In the rotating frame: inertial velocity v along y minus the frame's rotation.
        let e = GloEphemeris {
            pos: [r, 0.0, 0.0],
            vel: [0.0, v - OMEGA_E_GLO * r, 0.0],
            ..Default::default()
        };
        let (p, _) = e.integrate(900.0, 30.0);
        let rr = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        // Within J2's effect (some km), close to the radius.
        assert!((rr - r).abs() < 20_000.0, "{rr}");
        // Step size does not matter much.
        let (q, _) = e.integrate(900.0, 10.0);
        assert!(crate::geo::dist(p, q) < 0.05);
    }
}

//! The orbit arithmetic of the test signal, apart from the receiver's: Kepler orbits by a Halley iteration and rotation
//! matrices, GLONASS orbits by fourth-order Runge-Kutta on a dense table with Hermite interpolation.

/// A 3-vector.
pub type V3 = [f64; 3];

fn mat_vec(m: &[[f64; 3]; 3], v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn rot_z(a: f64) -> [[f64; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn rot_x(a: f64) -> [[f64; 3]; 3] {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

/// Orbit constants of a system as the generator sees them.
#[derive(Clone, Copy, Debug)]
pub struct Consts {
    pub mu: f64,
    pub we: f64,
    pub f: f64,
}

pub const GPS_C: Consts = Consts {
    mu: 3.986005e14,
    we: 7.2921151467e-5,
    f: -4.442807633e-10,
};
pub const GAL_C: Consts = Consts {
    mu: 3.986004418e14,
    we: 7.2921151467e-5,
    f: -4.442807309e-10,
};
pub const BDS_C: Consts = Consts {
    mu: 3.986004418e14,
    we: 7.2921150e-5,
    f: -4.442807309e-10,
};

/// The broadcast Kepler set as the generator builds it (already at the broadcast resolution). Angles in radians.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Elements {
    pub sqrt_a: f64,
    pub e: f64,
    pub i0: f64,
    pub omega0: f64,
    pub omega: f64,
    pub m0: f64,
    pub delta_n: f64,
    pub omega_dot: f64,
    pub idot: f64,
    pub cuc: f64,
    pub cus: f64,
    pub crc: f64,
    pub crs: f64,
    pub cic: f64,
    pub cis: f64,
    pub toe: f64,
    /// BeiDou GEO: the orbit frame is tilted by −5° about X.
    pub geo: bool,
}

impl Elements {
    /// Eccentric anomaly by Halley's method.
    fn ecc_anomaly(&self, m: f64) -> f64 {
        let e = self.e;
        let mut x = if e > 0.8 { std::f64::consts::PI } else { m };
        for _ in 0..12 {
            let f = x - e * x.sin() - m;
            let f1 = 1.0 - e * x.cos();
            let f2 = e * x.sin();
            let step = f / (f1 - 0.5 * f * f2 / f1);
            x -= step;
            if step.abs() < 1e-15 {
                break;
            }
        }
        x
    }

    /// Earth-fixed position at `dt` seconds from toe, and the relativistic clock term.
    pub fn ecef(&self, dt: f64, k: &Consts) -> (V3, f64) {
        let a = self.sqrt_a * self.sqrt_a;
        let n = (k.mu / a.powi(3)).sqrt() + self.delta_n;
        let ea = self.ecc_anomaly(self.m0 + n * dt);
        let (se, ce) = ea.sin_cos();
        let b = (1.0 - self.e * self.e).sqrt();
        let nu = (b * se).atan2(ce - self.e);
        let arg = nu + self.omega;
        let (s2, c2) = (2.0 * arg).sin_cos();
        let u = arg + self.cuc * c2 + self.cus * s2;
        let r = a * (1.0 - self.e * ce) + self.crc * c2 + self.crs * s2;
        let inc = self.i0 + self.idot * dt + self.cic * c2 + self.cis * s2;
        let in_plane = [r * u.cos(), r * u.sin(), 0.0];
        let rel = k.f * self.e * self.sqrt_a * se;
        if self.geo {
            let node = self.omega0 + self.omega_dot * dt - k.we * self.toe;
            let p = mat_vec(&rot_z(node), mat_vec(&rot_x(inc), in_plane));
            // R_X(-5 deg) of the ICD, whose R_X(phi) rotates the other way from `rot_x`.
            let tilted = mat_vec(&rot_x(5f64.to_radians()), p);
            return (mat_vec(&rot_z(-k.we * dt), tilted), rel);
        }
        let node = self.omega0 + (self.omega_dot - k.we) * dt - k.we * self.toe;
        (mat_vec(&rot_z(node), mat_vec(&rot_x(inc), in_plane)), rel)
    }
}

/// Geodetic (radians, m) to ECEF, WGS-84.
pub fn llh_to_xyz(lat: f64, lon: f64, h: f64) -> V3 {
    crate::geo::geodetic_to_ecef([lat, lon, h])
}

/// Elevation, azimuth (radians) and range (m) of `sat` from `rx` at geodetic `lat`, `lon` (WGS-84).
pub fn look(rx: V3, lat: f64, lon: f64, sat: V3) -> (f64, f64, f64) {
    let h = crate::geo::ecef_to_geodetic(rx)[2];
    let (az, el) = crate::geo::az_el([lat, lon, h], rx, sat);
    (el, az, crate::geo::dist(rx, sat))
}

/// The GLONASS force model as the generator writes it (PZ-90 constants), state (x, y, z, vx, vy, vz).
fn glo_f(s: &[f64; 6], acc: &V3) -> [f64; 6] {
    const MU: f64 = 398_600.44e9;
    const AE: f64 = 6_378_136.0;
    const J2: f64 = 1082.6257e-6;
    const W: f64 = 7.292115e-5;
    let r = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
    let mu_r3 = MU / (r * r * r);
    let k = 1.5 * J2 * MU * AE * AE / r.powi(5);
    let z2 = 5.0 * (s[2] / r).powi(2);
    [
        s[3],
        s[4],
        s[5],
        -mu_r3 * s[0] - k * s[0] * (1.0 - z2) + W * W * s[0] + 2.0 * W * s[4] + acc[0],
        -mu_r3 * s[1] - k * s[1] * (1.0 - z2) + W * W * s[1] - 2.0 * W * s[3] + acc[1],
        -mu_r3 * s[2] - k * s[2] * (3.0 - z2) + acc[2],
    ]
}

/// A GLONASS truth orbit: states every second from `t0`, integrated from the broadcast state at tb with RK4 (1 s steps).
#[derive(Clone, Debug)]
pub struct GloTruth {
    pub t0: f64,
    states: Vec<[f64; 6]>,
}

impl GloTruth {
    /// Integrates from (`tb`, `state`) to cover `[t_from, t_to]` (seconds of the GLONASS day).
    pub fn new(tb: f64, state: [f64; 6], acc: V3, t_from: f64, t_to: f64) -> GloTruth {
        let step = |s: &[f64; 6], h: f64| {
            let k1 = glo_f(s, &acc);
            let a: [f64; 6] = std::array::from_fn(|i| s[i] + 0.5 * h * k1[i]);
            let k2 = glo_f(&a, &acc);
            let b: [f64; 6] = std::array::from_fn(|i| s[i] + 0.5 * h * k2[i]);
            let k3 = glo_f(&b, &acc);
            let c: [f64; 6] = std::array::from_fn(|i| s[i] + h * k3[i]);
            let k4 = glo_f(&c, &acc);
            std::array::from_fn(|i| s[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]))
        };
        let t0 = t_from.floor() - 2.0;
        let t1 = t_to.ceil() + 2.0;
        // Integrate from tb to t0 (1 s steps), then table every second to t1.
        let mut s = state;
        let mut t = tb;
        let target = t0;
        while (target - t).abs() > 1e-9 {
            let h = (target - t).clamp(-1.0, 1.0);
            s = step(&s, h);
            t += h;
        }
        let mut states = vec![s];
        let n = (t1 - t0) as usize;
        for _ in 0..n {
            s = step(&s, 1.0);
            states.push(s);
        }
        GloTruth { t0, states }
    }

    /// Position at `t` by cubic Hermite interpolation of the table.
    pub fn pos(&self, t: f64) -> V3 {
        let x = (t - self.t0).clamp(0.0, (self.states.len() - 2) as f64);
        let i = x.floor() as usize;
        let u = x - i as f64;
        let (a, b) = (&self.states[i], &self.states[i + 1]);
        let h00 = 2.0 * u.powi(3) - 3.0 * u * u + 1.0;
        let h10 = u.powi(3) - 2.0 * u * u + u;
        let h01 = -2.0 * u.powi(3) + 3.0 * u * u;
        let h11 = u.powi(3) - u * u;
        std::array::from_fn(|k| h00 * a[k] + h10 * a[k + 3] + h01 * b[k] + h11 * b[k + 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halley_solves_kepler() {
        let el = Elements {
            sqrt_a: 5153.6,
            e: 0.3,
            ..Default::default()
        };
        for m in [0.1, 1.0, 3.0, 5.5] {
            let e = el.ecc_anomaly(m);
            assert!((e - 0.3 * e.sin() - m).abs() < 1e-12);
        }
    }
}

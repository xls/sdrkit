//! Ellipsoids, Earth-centred Earth-fixed (ECEF) coordinates, local east-north-up (ENU) coordinates, look angles, and Vincenty's
//! formulae for distances on the ellipsoid.
//!
//! Geodetic to ECEF is the closed form of NIMA TR8350.2 section 4 (EPSG method 9602). ECEF to geodetic iterates Bowring's formula
//! (Survey Review 23, 1976). Distances and destinations use Vincenty's formulae (Survey Review 23, 1975); the inverse returns
//! [`Error::NoConvergence`] for nearly antipodal points.

use crate::error::finite;
use crate::position::{wrap180, wrap360};
use crate::{Error, Position};

/// An Earth ellipsoid: semi-major axis and flattening.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ellipsoid {
    /// Semi-major (equatorial) axis, m.
    pub a_m: f64,
    /// Flattening f = (a - b) / a.
    pub f: f64,
}

impl Ellipsoid {
    /// WGS-84 (NIMA TR8350.2): a = 6 378 137 m, 1/f = 298.257 223 563. The ellipsoid of GPS and of [`Position`].
    pub const WGS84: Ellipsoid = Ellipsoid {
        a_m: 6_378_137.0,
        f: 1.0 / 298.257_223_563,
    };
    /// GRS 80 (Moritz 1980): a = 6 378 137 m, 1/f = 298.257 222 101. Also used by ETRS89 and GDA.
    pub const GRS80: Ellipsoid = Ellipsoid {
        a_m: 6_378_137.0,
        f: 1.0 / 298.257_222_101,
    };
    /// WGS-72: a = 6 378 135 m, 1/f = 298.26. The ellipsoid of NORAD element sets (SGP4).
    pub const WGS72: Ellipsoid = Ellipsoid {
        a_m: 6_378_135.0,
        f: 1.0 / 298.26,
    };
    /// PZ-90 (GLONASS ICD 5.1): a = 6 378 136 m, 1/f = 298.257 84.
    pub const PZ90: Ellipsoid = Ellipsoid {
        a_m: 6_378_136.0,
        f: 1.0 / 298.257_84,
    };
    /// CGCS2000 (BeiDou ICD): a = 6 378 137 m, 1/f = 298.257 222 101.
    pub const CGCS2000: Ellipsoid = Ellipsoid {
        a_m: 6_378_137.0,
        f: 1.0 / 298.257_222_101,
    };

    /// Semi-minor (polar) axis b = a (1 - f), m.
    pub fn b_m(&self) -> f64 {
        self.a_m * (1.0 - self.f)
    }

    /// First eccentricity squared e² = f (2 - f).
    pub fn e2(&self) -> f64 {
        self.f * (2.0 - self.f)
    }

    /// ECEF of a geodetic position on this ellipsoid.
    pub fn to_ecef(&self, p: &Position) -> Ecef {
        let e2 = self.e2();
        let (sl, cl) = p.lat_deg.to_radians().sin_cos();
        let (so, co) = p.lon_deg.to_radians().sin_cos();
        let n = self.a_m / (1.0 - e2 * sl * sl).sqrt();
        Ecef {
            x_m: (n + p.alt_m) * cl * co,
            y_m: (n + p.alt_m) * cl * so,
            z_m: (n * (1.0 - e2) + p.alt_m) * sl,
        }
    }

    /// The geodetic position of an ECEF point on this ellipsoid. Errors only for non-finite input.
    pub fn from_ecef(&self, e: &Ecef) -> Result<Position, Error> {
        finite(&[e.x_m, e.y_m, e.z_m])?;
        let a = self.a_m;
        let b = self.b_m();
        let e2 = self.e2();
        let ep2 = e2 / (1.0 - e2);
        let p = e.x_m.hypot(e.y_m);
        let lon = e.y_m.atan2(e.x_m);
        if p < 1e-9 * a {
            // On the axis: a pole (or the centre, given as the north pole at height -b).
            let lat = if e.z_m >= 0.0 { 90.0 } else { -90.0 };
            return Ok(Position {
                lat_deg: lat,
                lon_deg: 0.0,
                alt_m: e.z_m.abs() - b,
            });
        }
        // Bowring: start from the reduced latitude of the point, iterate until the latitude stops changing.
        let mut beta = (a * e.z_m).atan2(b * p);
        let mut lat = 0.0;
        for _ in 0..8 {
            let (sb, cb) = beta.sin_cos();
            let next = (e.z_m + ep2 * b * sb * sb * sb).atan2(p - e2 * a * cb * cb * cb);
            let done = (next - lat).abs() < 1e-15;
            lat = next;
            if done {
                break;
            }
            beta = ((1.0 - self.f) * lat.tan()).atan();
        }
        let (sl, cl) = lat.sin_cos();
        let n = a / (1.0 - e2 * sl * sl).sqrt();
        // Height: the better-conditioned of the two forms for the latitude.
        let h = if cl.abs() > 0.5 {
            p / cl - n
        } else {
            e.z_m / sl - n * (1.0 - e2)
        };
        Ok(Position {
            lat_deg: lat.to_degrees(),
            lon_deg: lon.to_degrees(),
            alt_m: h,
        })
    }

    /// Vincenty's inverse problem: the distance and the azimuths of the geodesic between two positions on this ellipsoid
    /// (heights ignored). Accurate to well under a millimetre; returns [`Error::NoConvergence`] for nearly antipodal points, where
    /// the method fails.
    ///
    /// ```
    /// use sdrkit_util_geo::{Ellipsoid, Position};
    /// // Vincenty's worked example as published by Geoscience Australia: Flinders Peak to Buninyong on GRS 80.
    /// let dms = |d: f64, m: f64, s: f64| d.signum() * (d.abs() + m / 60.0 + s / 3600.0);
    /// let flinders = Position::new(dms(-37.0, 57.0, 3.72030), dms(144.0, 25.0, 29.52440));
    /// let buninyong = Position::new(dms(-37.0, 39.0, 10.15610), dms(143.0, 55.0, 35.38390));
    /// let g = Ellipsoid::GRS80.inverse(&flinders, &buninyong).unwrap();
    /// assert!((g.distance_m - 54_972.271).abs() < 0.001);
    /// ```
    pub fn inverse(&self, from: &Position, to: &Position) -> Result<Geodesic, Error> {
        finite(&[from.lat_deg, from.lon_deg, to.lat_deg, to.lon_deg])?;
        let a = self.a_m;
        let f = self.f;
        let b = self.b_m();
        let l = (to.lon_deg - from.lon_deg).to_radians();
        let u1 = ((1.0 - f) * from.lat_deg.to_radians().tan()).atan();
        let u2 = ((1.0 - f) * to.lat_deg.to_radians().tan()).atan();
        let (su1, cu1) = u1.sin_cos();
        let (su2, cu2) = u2.sin_cos();
        let mut lambda = l;
        let mut iter = 0;
        let (mut sin_sigma, mut cos_sigma, mut sigma, mut cos_sq_alpha, mut cos2sm, mut sl, mut cl);
        loop {
            (sl, cl) = lambda.sin_cos();
            let t = cu2 * sl;
            let u = cu1 * su2 - su1 * cu2 * cl;
            sin_sigma = (t * t + u * u).sqrt();
            if sin_sigma == 0.0 {
                // Coincident points.
                return Ok(Geodesic {
                    distance_m: 0.0,
                    initial_bearing_deg: 0.0,
                    final_bearing_deg: 0.0,
                });
            }
            cos_sigma = su1 * su2 + cu1 * cu2 * cl;
            sigma = sin_sigma.atan2(cos_sigma);
            let sin_alpha = cu1 * cu2 * sl / sin_sigma;
            cos_sq_alpha = 1.0 - sin_alpha * sin_alpha;
            cos2sm = if cos_sq_alpha != 0.0 {
                cos_sigma - 2.0 * su1 * su2 / cos_sq_alpha
            } else {
                0.0
            };
            let c = f / 16.0 * cos_sq_alpha * (4.0 + f * (4.0 - 3.0 * cos_sq_alpha));
            let prev = lambda;
            lambda = l
                + (1.0 - c)
                    * f
                    * sin_alpha
                    * (sigma
                        + c * sin_sigma
                            * (cos2sm + c * cos_sigma * (-1.0 + 2.0 * cos2sm * cos2sm)));
            iter += 1;
            if (lambda - prev).abs() < 1e-12 {
                break;
            }
            if iter >= 200 || lambda.abs() > std::f64::consts::PI * 1.000_001 {
                return Err(Error::NoConvergence);
            }
        }
        let u_sq = cos_sq_alpha * (a * a - b * b) / (b * b);
        let ka = 1.0 + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
        let kb = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));
        let ds = kb
            * sin_sigma
            * (cos2sm
                + kb / 4.0
                    * (cos_sigma * (-1.0 + 2.0 * cos2sm * cos2sm)
                        - kb / 6.0
                            * cos2sm
                            * (-3.0 + 4.0 * sin_sigma * sin_sigma)
                            * (-3.0 + 4.0 * cos2sm * cos2sm)));
        let distance_m = b * ka * (sigma - ds);
        let a1 = (cu2 * sl).atan2(cu1 * su2 - su1 * cu2 * cl);
        let a2 = (cu1 * sl).atan2(-su1 * cu2 + cu1 * su2 * cl);
        Ok(Geodesic {
            distance_m,
            initial_bearing_deg: wrap360(a1.to_degrees()),
            final_bearing_deg: wrap360(a2.to_degrees()),
        })
    }

    /// Vincenty's direct problem: the position reached from `from` after `distance_m` along the geodesic leaving on
    /// `bearing_deg`, and the bearing on arrival. The height of `from` is kept.
    pub fn direct(
        &self,
        from: &Position,
        bearing_deg: f64,
        distance_m: f64,
    ) -> Result<(Position, f64), Error> {
        finite(&[from.lat_deg, from.lon_deg, bearing_deg, distance_m])?;
        let a = self.a_m;
        let f = self.f;
        let b = self.b_m();
        let (sa1, ca1) = bearing_deg.to_radians().sin_cos();
        let tu1 = (1.0 - f) * from.lat_deg.to_radians().tan();
        let cu1 = 1.0 / (1.0 + tu1 * tu1).sqrt();
        let su1 = tu1 * cu1;
        let sigma1 = tu1.atan2(ca1);
        let sin_alpha = cu1 * sa1;
        let cos_sq_alpha = 1.0 - sin_alpha * sin_alpha;
        let u_sq = cos_sq_alpha * (a * a - b * b) / (b * b);
        let ka = 1.0 + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
        let kb = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));
        let mut sigma = distance_m / (b * ka);
        let (mut ss, mut cs, mut cos2sm);
        let mut iter = 0;
        loop {
            cos2sm = (2.0 * sigma1 + sigma).cos();
            (ss, cs) = sigma.sin_cos();
            let ds = kb
                * ss
                * (cos2sm
                    + kb / 4.0
                        * (cs * (-1.0 + 2.0 * cos2sm * cos2sm)
                            - kb / 6.0
                                * cos2sm
                                * (-3.0 + 4.0 * ss * ss)
                                * (-3.0 + 4.0 * cos2sm * cos2sm)));
            let prev = sigma;
            sigma = distance_m / (b * ka) + ds;
            iter += 1;
            if (sigma - prev).abs() < 1e-12 {
                break;
            }
            if iter >= 200 {
                return Err(Error::NoConvergence);
            }
        }
        (ss, cs) = sigma.sin_cos();
        cos2sm = (2.0 * sigma1 + sigma).cos();
        let x = su1 * ss - cu1 * cs * ca1;
        let lat2 =
            (su1 * cs + cu1 * ss * ca1).atan2((1.0 - f) * (sin_alpha * sin_alpha + x * x).sqrt());
        let lambda = (ss * sa1).atan2(cu1 * cs - su1 * ss * ca1);
        let c = f / 16.0 * cos_sq_alpha * (4.0 + f * (4.0 - 3.0 * cos_sq_alpha));
        let l = lambda
            - (1.0 - c)
                * f
                * sin_alpha
                * (sigma + c * ss * (cos2sm + c * cs * (-1.0 + 2.0 * cos2sm * cos2sm)));
        let a2 = sin_alpha.atan2(-x);
        Ok((
            Position {
                lat_deg: lat2.to_degrees(),
                lon_deg: wrap180(from.lon_deg + l.to_degrees()),
                alt_m: from.alt_m,
            },
            wrap360(a2.to_degrees()),
        ))
    }
}

/// The result of [`Ellipsoid::inverse`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Geodesic {
    /// Length of the geodesic, m.
    pub distance_m: f64,
    /// Azimuth at the start, degrees from true north.
    pub initial_bearing_deg: f64,
    /// Azimuth on arrival (the direction of travel at the end), degrees from true north.
    pub final_bearing_deg: f64,
}

/// A point in Earth-centred Earth-fixed coordinates: x towards 0° latitude 0° longitude, z towards the north pole, metres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ecef {
    /// x, m.
    pub x_m: f64,
    /// y, m.
    pub y_m: f64,
    /// z, m.
    pub z_m: f64,
}

impl Ecef {
    /// A point from its coordinates.
    pub const fn new(x_m: f64, y_m: f64, z_m: f64) -> Self {
        Ecef { x_m, y_m, z_m }
    }

    /// The coordinates as an array.
    pub fn to_array(&self) -> [f64; 3] {
        [self.x_m, self.y_m, self.z_m]
    }

    /// The straight-line distance to another point, m.
    pub fn distance_m(&self, o: &Ecef) -> f64 {
        ((self.x_m - o.x_m).powi(2) + (self.y_m - o.y_m).powi(2) + (self.z_m - o.z_m).powi(2))
            .sqrt()
    }

    /// The geodetic WGS-84 position of this point.
    pub fn to_position(&self) -> Result<Position, Error> {
        Ellipsoid::WGS84.from_ecef(self)
    }
}

impl Position {
    /// ECEF on WGS-84.
    ///
    /// ```
    /// use sdrkit_util_geo::Position;
    /// let e = Position::new(0.0, 0.0).to_ecef();
    /// assert_eq!((e.x_m, e.y_m, e.z_m), (6_378_137.0, 0.0, 0.0));
    /// ```
    pub fn to_ecef(&self) -> Ecef {
        Ellipsoid::WGS84.to_ecef(self)
    }
}

/// A vector in local east, north, up coordinates (a plane tangent to the ellipsoid at an origin), metres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Enu {
    /// East, m (or m/s for a velocity).
    pub east_m: f64,
    /// North, m.
    pub north_m: f64,
    /// Up along the ellipsoid normal, m.
    pub up_m: f64,
}

/// Where a target appears from an observer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Look {
    /// Azimuth, degrees clockwise from true north, 0 to 360.
    pub azimuth_deg: f64,
    /// Elevation above the local horizontal plane (the plane normal to the ellipsoid), degrees, -90 to 90.
    pub elevation_deg: f64,
    /// Straight-line distance, m.
    pub range_m: f64,
    /// Rate of change of the range, m/s, positive when the target moves away; 0 when no velocity was given.
    pub range_rate_m_s: f64,
}

/// A fixed observer with its ECEF position and local axes computed once, for fast repeated ENU and look-angle sums.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observer {
    position: Position,
    ecef: Ecef,
    east: [f64; 3],
    north: [f64; 3],
    up: [f64; 3],
}

impl Observer {
    /// An observer at a WGS-84 position.
    pub fn new(position: Position) -> Self {
        let (sl, cl) = position.lat_deg.to_radians().sin_cos();
        let (so, co) = position.lon_deg.to_radians().sin_cos();
        Observer {
            position,
            ecef: position.to_ecef(),
            east: [-so, co, 0.0],
            north: [-sl * co, -sl * so, cl],
            up: [cl * co, cl * so, sl],
        }
    }

    /// The observer's position.
    pub fn position(&self) -> Position {
        self.position
    }

    /// The observer's ECEF position.
    pub fn ecef(&self) -> Ecef {
        self.ecef
    }

    /// The ENU components of an ECEF difference vector (or velocity) at the observer.
    pub fn rotate_to_enu(&self, d: [f64; 3]) -> Enu {
        let dot = |a: &[f64; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
        Enu {
            east_m: dot(&self.east),
            north_m: dot(&self.north),
            up_m: dot(&self.up),
        }
    }

    /// The ECEF components of an ENU vector (or velocity) at the observer: the inverse of [`Observer::rotate_to_enu`].
    pub fn rotate_from_enu(&self, v: &Enu) -> [f64; 3] {
        let c =
            |i: usize| self.east[i] * v.east_m + self.north[i] * v.north_m + self.up[i] * v.up_m;
        [c(0), c(1), c(2)]
    }

    /// The ENU coordinates of an ECEF point relative to the observer.
    pub fn enu(&self, target: &Ecef) -> Enu {
        self.rotate_to_enu([
            target.x_m - self.ecef.x_m,
            target.y_m - self.ecef.y_m,
            target.z_m - self.ecef.z_m,
        ])
    }

    /// The ECEF point at ENU offset `v` from the observer.
    pub fn ecef_of(&self, v: &Enu) -> Ecef {
        let d = self.rotate_from_enu(v);
        Ecef {
            x_m: self.ecef.x_m + d[0],
            y_m: self.ecef.y_m + d[1],
            z_m: self.ecef.z_m + d[2],
        }
    }

    /// Azimuth, elevation and range of an ECEF point.
    pub fn look(&self, target: &Ecef) -> Look {
        self.look_moving(target, &[0.0; 3])
    }

    /// Azimuth, elevation, range and range rate of an ECEF point moving at `velocity_m_s` (ECEF axes, m/s).
    pub fn look_moving(&self, target: &Ecef, velocity_m_s: &[f64; 3]) -> Look {
        let d = [
            target.x_m - self.ecef.x_m,
            target.y_m - self.ecef.y_m,
            target.z_m - self.ecef.z_m,
        ];
        let range = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if range < 1e-9 {
            return Look {
                azimuth_deg: 0.0,
                elevation_deg: 90.0,
                range_m: range,
                range_rate_m_s: 0.0,
            };
        }
        let e = self.rotate_to_enu(d);
        let horiz = e.east_m.hypot(e.north_m);
        Look {
            azimuth_deg: wrap360(e.east_m.atan2(e.north_m).to_degrees()),
            elevation_deg: e.up_m.atan2(horiz).to_degrees(),
            range_m: range,
            range_rate_m_s: (d[0] * velocity_m_s[0]
                + d[1] * velocity_m_s[1]
                + d[2] * velocity_m_s[2])
                / range,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dms(d: f64, m: f64, s: f64) -> f64 {
        d.signum() * (d.abs() + m / 60.0 + s / 3600.0)
    }

    #[test]
    fn iogp_guidance_note_7_2_geocentric_example() {
        // IOGP Publication 373-7-2, EPSG method 9602 worked example (WGS 84): 53°48'33.820"N 2°07'46.380"E h = 73.0 m.
        let p = Position::with_alt(dms(53.0, 48.0, 33.820), dms(2.0, 7.0, 46.380), 73.0);
        let e = p.to_ecef();
        assert!((e.x_m - 3_771_793.968).abs() < 0.001, "{e:?}");
        assert!((e.y_m - 140_253.342).abs() < 0.001, "{e:?}");
        assert!((e.z_m - 5_124_304.349).abs() < 0.001, "{e:?}");
        let back = e.to_position().unwrap();
        assert!(
            (back.lat_deg - p.lat_deg).abs() < 1e-11 && (back.lon_deg - p.lon_deg).abs() < 1e-12
        );
        assert!((back.alt_m - 73.0).abs() < 1e-6);
    }

    #[test]
    fn axes_and_poles() {
        let pole = Position::new(90.0, 0.0).to_ecef();
        assert!((pole.z_m - 6_356_752.314_245).abs() < 1e-5, "{pole:?}");
        let p = Ecef::new(0.0, 0.0, -6_356_752.314_245 - 100.0)
            .to_position()
            .unwrap();
        assert_eq!(p.lat_deg, -90.0);
        assert!((p.alt_m - 100.0).abs() < 1e-6);
        assert_eq!(
            Ecef::new(f64::NAN, 0.0, 0.0).to_position(),
            Err(Error::NotFinite)
        );
    }

    #[test]
    fn round_trips_from_below_ground_to_geostationary() {
        for &lat in &[-89.999, -60.0, -30.0, -1e-9, 0.0, 12.3, 45.0, 75.0, 89.9999] {
            for &h in &[-10_000.0, 0.0, 400_000.0, 20_200_000.0, 35_786_000.0] {
                let p = Position::with_alt(lat, 123.456, h);
                let q = p.to_ecef().to_position().unwrap();
                assert!(
                    q.to_ecef().distance_m(&p.to_ecef()) < 1e-6,
                    "{p:?} -> {q:?}"
                );
            }
        }
    }

    #[test]
    fn vincenty_published_example() {
        // Geoscience Australia's worked example of Vincenty's formulae (GRS 80).
        let flinders = Position::new(dms(-37.0, 57.0, 3.72030), dms(144.0, 25.0, 29.52440));
        let buninyong = Position::new(dms(-37.0, 39.0, 10.15610), dms(143.0, 55.0, 35.38390));
        let g = Ellipsoid::GRS80.inverse(&flinders, &buninyong).unwrap();
        assert!((g.distance_m - 54_972.271).abs() < 0.001, "{g:?}");
        assert!(
            (g.initial_bearing_deg - dms(306.0, 52.0, 5.37)).abs() < 0.01 / 3600.0,
            "{g:?}"
        );
        // The published reverse azimuth is 127°10'25.07"; the direction of travel on arrival is its opposite.
        assert!(
            (g.final_bearing_deg - wrap360(dms(127.0, 10.0, 25.07) + 180.0)).abs() < 0.01 / 3600.0,
            "{g:?}"
        );
        let (p, a2) = Ellipsoid::GRS80
            .direct(&flinders, dms(306.0, 52.0, 5.37), 54_972.271)
            .unwrap();
        assert!(
            (p.lat_deg - buninyong.lat_deg).abs() < 1e-8
                && (p.lon_deg - buninyong.lon_deg).abs() < 1e-8,
            "{p:?}"
        );
        assert!((a2 - g.final_bearing_deg).abs() < 1e-5);
    }

    #[test]
    fn vincenty_edges() {
        let w = Ellipsoid::WGS84;
        // A quarter meridian on WGS-84 is 10 001 965.729 m.
        let g = w
            .inverse(&Position::new(0.0, 0.0), &Position::new(90.0, 0.0))
            .unwrap();
        assert!((g.distance_m - 10_001_965.729).abs() < 0.001, "{g:?}");
        assert_eq!(
            w.inverse(&Position::new(1.0, 2.0), &Position::new(1.0, 2.0))
                .unwrap()
                .distance_m,
            0.0
        );
        assert_eq!(
            w.inverse(&Position::new(0.0, 0.0), &Position::new(0.5, 179.7)),
            Err(Error::NoConvergence)
        );
        assert!(w.direct(&Position::new(0.0, 0.0), f64::NAN, 1.0).is_err());
    }

    #[test]
    fn enu_and_look_angles() {
        let obs = Observer::new(Position::with_alt(25.0, 50.0, 100.0));
        let up = Position::with_alt(25.0, 50.0, 1_100.0).to_ecef();
        let l = obs.look(&up);
        assert!((l.elevation_deg - 90.0).abs() < 1e-9 && (l.range_m - 1_000.0).abs() < 1e-6);
        let e = Enu {
            east_m: 1_000.0,
            north_m: 0.0,
            up_m: 0.0,
        };
        let l = obs.look(&obs.ecef_of(&e));
        assert!((l.azimuth_deg - 90.0).abs() < 1e-9 && l.elevation_deg.abs() < 1e-9);
        let back = obs.enu(&obs.ecef_of(&Enu {
            east_m: 3.0,
            north_m: -4.0,
            up_m: 5.0,
        }));
        assert!(
            (back.east_m - 3.0).abs() < 1e-6
                && (back.north_m + 4.0).abs() < 1e-6
                && (back.up_m - 5.0).abs() < 1e-6
        );
        // Moving straight away along the line of sight at 7 m/s.
        let d = obs.ecef_of(&Enu {
            east_m: 0.0,
            north_m: 1_000.0,
            up_m: 0.0,
        });
        let o = obs.ecef();
        let r = d.distance_m(&o);
        let v = [
            7.0 * (d.x_m - o.x_m) / r,
            7.0 * (d.y_m - o.y_m) / r,
            7.0 * (d.z_m - o.z_m) / r,
        ];
        let l = obs.look_moving(&d, &v);
        assert!((l.range_rate_m_s - 7.0).abs() < 1e-6, "{l:?}");
        assert!(l.azimuth_deg.min(360.0 - l.azimuth_deg) < 1e-6, "{l:?}");
    }
}

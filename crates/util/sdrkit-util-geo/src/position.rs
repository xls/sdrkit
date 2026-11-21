//! [`Position`] and the spherical-Earth sums on it: great-circle distance, bearings, destination, midpoint, cross-track and
//! along-track distance, the closest point of approach of two moving objects, and fences.
//!
//! All spherical sums use the IUGG mean Earth radius R1 = 6 371 008.8 m of GRS 80 ([`EARTH_MEAN_RADIUS_M`]; Moritz, Bulletin
//! Géodésique 54, 1980), within about 0.5 % of the ellipsoid. For survey-grade distances use [`crate::ellipsoid::Ellipsoid::inverse`].

use crate::error::finite;
use crate::Error;
use crate::Quantity;

/// The IUGG mean Earth radius R1 = (2a + b) / 3 of GRS 80, m. Every spherical sum in this crate uses it.
pub const EARTH_MEAN_RADIUS_M: f64 = 6_371_008.8;

/// A point on or above the Earth.
///
/// Latitude and longitude are geodetic (WGS-84) degrees, north and east positive; latitude always comes first. `alt_m` is the
/// height above the WGS-84 ellipsoid in metres (0 when unknown or on the surface); it is ignored by the spherical sums and used by
/// [`Position::to_ecef`] and the look angles.
///
/// ```
/// use sdrkit_util_geo::Position;
/// let p = Position::checked(-33.8688, 151.2093, 58.0).unwrap();
/// assert_eq!(p.lat_deg, -33.8688);
/// assert!(Position::checked(91.0, 0.0, 0.0).is_err());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    /// Geodetic latitude, degrees, -90 to 90, north positive.
    pub lat_deg: f64,
    /// Longitude, degrees, -180 to 180, east positive.
    pub lon_deg: f64,
    /// Height above the WGS-84 ellipsoid, metres.
    pub alt_m: f64,
}

/// The result of the closest-point-of-approach sum ([`closest_approach`]).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Approach {
    /// The distance at the closest point of approach (CPA), m.
    pub distance_m: f64,
    /// Seconds from now to the closest point (TCPA); 0 when the objects are already moving apart.
    pub time_s: f64,
}

/// Something moving in a straight line: where it is, its speed over the ground and its course.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Track {
    /// Where it is now.
    pub position: Position,
    /// Speed over the ground, m/s (multiply knots by [`crate::KNOT_M_S`]).
    pub speed_m_s: f64,
    /// Course over the ground, degrees from true north.
    pub course_deg: f64,
}

impl Position {
    /// A position on the ellipsoid (height 0). The values are not checked; use [`Position::checked`] for input from outside.
    pub const fn new(lat_deg: f64, lon_deg: f64) -> Self {
        Position {
            lat_deg,
            lon_deg,
            alt_m: 0.0,
        }
    }

    /// A position with a height. The values are not checked.
    pub const fn with_alt(lat_deg: f64, lon_deg: f64, alt_m: f64) -> Self {
        Position {
            lat_deg,
            lon_deg,
            alt_m,
        }
    }

    /// A position after checking that every value is finite, the latitude is within ±90 and the longitude within ±180.
    pub fn checked(lat_deg: f64, lon_deg: f64, alt_m: f64) -> Result<Self, Error> {
        let p = Position {
            lat_deg,
            lon_deg,
            alt_m,
        };
        p.validate()?;
        Ok(p)
    }

    /// Checks the values: finite, latitude within ±90, longitude within ±180.
    pub fn validate(&self) -> Result<(), Error> {
        finite(&[self.lat_deg, self.lon_deg, self.alt_m])?;
        if self.lat_deg.abs() > 90.0 {
            return Err(Error::OutOfRange(Quantity::Latitude));
        }
        if self.lon_deg.abs() > 180.0 {
            return Err(Error::OutOfRange(Quantity::Longitude));
        }
        Ok(())
    }

    /// The great-circle distance to `other` on the mean-radius sphere, m (haversine formula, accurate for every separation).
    ///
    /// ```
    /// use sdrkit_util_geo::{Position, EARTH_MEAN_RADIUS_M};
    /// let d = Position::new(0.0, 0.0).distance_m(&Position::new(0.0, 1.0));
    /// assert!((d - EARTH_MEAN_RADIUS_M * 1f64.to_radians()).abs() < 1e-6);
    /// ```
    pub fn distance_m(&self, other: &Position) -> f64 {
        EARTH_MEAN_RADIUS_M * self.central_angle_rad(other)
    }

    /// The angle at the Earth's centre between the two positions, radians.
    pub fn central_angle_rad(&self, other: &Position) -> f64 {
        let (p1, p2) = (self.lat_deg.to_radians(), other.lat_deg.to_radians());
        let dp = p2 - p1;
        let dl = (other.lon_deg - self.lon_deg).to_radians();
        let a = (dp * 0.5).sin().powi(2) + p1.cos() * p2.cos() * (dl * 0.5).sin().powi(2);
        2.0 * a.sqrt().min(1.0).asin()
    }

    /// The initial bearing (forward azimuth) of the great circle towards `other`, degrees from true north, 0 to 360. Undefined
    /// (returns 0) when the points coincide or `self` is a pole.
    pub fn initial_bearing_deg(&self, other: &Position) -> f64 {
        let (p1, p2) = (self.lat_deg.to_radians(), other.lat_deg.to_radians());
        let dl = (other.lon_deg - self.lon_deg).to_radians();
        let y = dl.sin() * p2.cos();
        let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
        wrap360(y.atan2(x).to_degrees())
    }

    /// The bearing on arrival at `other` along the great circle from `self`, degrees, 0 to 360.
    pub fn final_bearing_deg(&self, other: &Position) -> f64 {
        wrap360(other.initial_bearing_deg(self) + 180.0)
    }

    /// The point reached after `distance_m` along the great circle that leaves on `bearing_deg`. The height is kept.
    ///
    /// ```
    /// use sdrkit_util_geo::Position;
    /// let start = Position::new(51.0, -1.0);
    /// let end = start.destination(90.0, 100_000.0);
    /// assert!((start.distance_m(&end) - 100_000.0).abs() < 1e-6);
    /// ```
    pub fn destination(&self, bearing_deg: f64, distance_m: f64) -> Position {
        let d = distance_m / EARTH_MEAN_RADIUS_M;
        let b = bearing_deg.to_radians();
        let p1 = self.lat_deg.to_radians();
        let l1 = self.lon_deg.to_radians();
        let sin_p2 = (p1.sin() * d.cos() + p1.cos() * d.sin() * b.cos()).clamp(-1.0, 1.0);
        let p2 = sin_p2.asin();
        let l2 = l1 + (b.sin() * d.sin() * p1.cos()).atan2(d.cos() - p1.sin() * sin_p2);
        Position {
            lat_deg: p2.to_degrees(),
            lon_deg: wrap180(l2.to_degrees()),
            alt_m: self.alt_m,
        }
    }

    /// The point halfway along the great circle to `other`. The height is the mean of the two.
    pub fn midpoint(&self, other: &Position) -> Position {
        let (p1, p2) = (self.lat_deg.to_radians(), other.lat_deg.to_radians());
        let l1 = self.lon_deg.to_radians();
        let dl = (other.lon_deg - self.lon_deg).to_radians();
        let bx = p2.cos() * dl.cos();
        let by = p2.cos() * dl.sin();
        let p3 = (p1.sin() + p2.sin()).atan2(((p1.cos() + bx).powi(2) + by * by).sqrt());
        let l3 = l1 + by.atan2(p1.cos() + bx);
        Position {
            lat_deg: p3.to_degrees(),
            lon_deg: wrap180(l3.to_degrees()),
            alt_m: 0.5 * (self.alt_m + other.alt_m),
        }
    }

    /// The signed distance of `self` from the great circle through `start` and `end`, m: positive when `self` is to the right of
    /// the path (seen travelling from `start` towards `end`), negative to the left.
    pub fn cross_track_m(&self, start: &Position, end: &Position) -> f64 {
        let d13 = start.central_angle_rad(self);
        let t13 = start.initial_bearing_deg(self).to_radians();
        let t12 = start.initial_bearing_deg(end).to_radians();
        let s = (d13.sin() * (t13 - t12).sin()).clamp(-1.0, 1.0);
        EARTH_MEAN_RADIUS_M * s.asin()
    }

    /// The distance from `start`, along the great circle through `start` and `end`, to the point on it closest to `self`, m;
    /// negative when that point is behind `start`.
    pub fn along_track_m(&self, start: &Position, end: &Position) -> f64 {
        let d13 = start.central_angle_rad(self);
        let dxt = self.cross_track_m(start, end) / EARTH_MEAN_RADIUS_M;
        let c = dxt.cos();
        if c.abs() < 1e-15 {
            return 0.0;
        }
        let dat = (d13.cos() / c).clamp(-1.0, 1.0).acos();
        let t13 = start.initial_bearing_deg(self).to_radians();
        let t12 = start.initial_bearing_deg(end).to_radians();
        EARTH_MEAN_RADIUS_M * dat * (t12 - t13).cos().signum()
    }
}

/// Wraps an angle to -180 (exclusive) .. 180 (inclusive) degrees.
pub fn wrap180(deg: f64) -> f64 {
    let w = deg.rem_euclid(360.0);
    if w > 180.0 {
        w - 360.0
    } else {
        w
    }
}

/// Wraps an angle to 0 (inclusive) .. 360 (exclusive) degrees.
pub fn wrap360(deg: f64) -> f64 {
    let w = deg.rem_euclid(360.0);
    if w >= 360.0 {
        0.0
    } else {
        w
    }
}

/// The 16-point compass name of a bearing (`N`, `NNE`, `NE` … `NNW`), each covering 22.5° centred on its direction.
///
/// ```
/// assert_eq!(sdrkit_util_geo::position::compass_point(0.0), "N");
/// assert_eq!(sdrkit_util_geo::position::compass_point(236.0), "SW");
/// assert_eq!(sdrkit_util_geo::position::compass_point(-10.0), "N");
/// ```
pub fn compass_point(bearing_deg: f64) -> &'static str {
    const NAMES: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    if !bearing_deg.is_finite() {
        return "";
    }
    NAMES[((wrap360(bearing_deg) + 11.25) / 22.5) as usize % 16]
}

/// The closest point of approach of two objects moving in straight lines at constant speed (`own` may have speed 0 for a fixed
/// observer). The sum is made in a local flat plane centred on `own`, which is good to well under 1 % within a few hundred
/// kilometres, the range where a CPA matters.
///
/// ```
/// use sdrkit_util_geo::position::{closest_approach, Track};
/// use sdrkit_util_geo::{Position, KNOT_M_S};
/// let me = Track { position: Position::new(0.0, 0.0), speed_m_s: 0.0, course_deg: 0.0 };
/// // A ship 10 km west and 5 km north of me, steaming east at 10 knots, passes 5 km north of me.
/// let ship = Position::new(0.0, 0.0).destination(0.0, 5_000.0).destination(270.0, 10_000.0);
/// let a = closest_approach(&me, &Track { position: ship, speed_m_s: 10.0 * KNOT_M_S, course_deg: 90.0 });
/// assert!((a.distance_m - 5_000.0).abs() < 50.0);
/// ```
pub fn closest_approach(own: &Track, other: &Track) -> Approach {
    let d = own.position.distance_m(&other.position);
    let b = own
        .position
        .initial_bearing_deg(&other.position)
        .to_radians();
    // The other object relative to own, metres east and north.
    let (x, y) = (d * b.sin(), d * b.cos());
    let (so, co) = own.course_deg.to_radians().sin_cos();
    let (st, ct) = other.course_deg.to_radians().sin_cos();
    let vx = other.speed_m_s * st - own.speed_m_s * so;
    let vy = other.speed_m_s * ct - own.speed_m_s * co;
    let vv = vx * vx + vy * vy;
    if !vv.is_finite() || vv <= f64::EPSILON {
        return Approach {
            distance_m: d,
            time_s: 0.0,
        };
    }
    let t = (-(x * vx + y * vy) / vv).max(0.0);
    let (cx, cy) = (x + vx * t, y + vy * t);
    Approach {
        distance_m: (cx * cx + cy * cy).sqrt(),
        time_s: t,
    }
}

/// Whether `p` is inside the polygon whose corners are given in order (the last joins the first), by ray casting with longitude
/// as x. The polygon must not cross the 180° meridian or contain a pole. Fewer than three corners contain nothing.
pub fn polygon_contains(corners: &[Position], p: &Position) -> bool {
    if corners.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = corners.len() - 1;
    for i in 0..corners.len() {
        let (a, b) = (&corners[i], &corners[j]);
        if (a.lat_deg > p.lat_deg) != (b.lat_deg > p.lat_deg) {
            let x = (b.lon_deg - a.lon_deg) * (p.lat_deg - a.lat_deg) / (b.lat_deg - a.lat_deg)
                + a.lon_deg;
            if p.lon_deg < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// A named area for alerts: a circle or a polygon.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "shape", rename_all = "snake_case"))]
pub enum Fence {
    /// Everything within `radius_m` (great-circle distance) of `centre`.
    Circle {
        /// The centre.
        centre: Position,
        /// The radius, m.
        radius_m: f64,
    },
    /// A polygon (see [`polygon_contains`] for its limits).
    Polygon {
        /// The corners, in order.
        corners: Vec<Position>,
    },
}

impl Fence {
    /// Whether the position is inside the area.
    pub fn contains(&self, p: &Position) -> bool {
        match self {
            Fence::Circle { centre, radius_m } => centre.distance_m(p) <= *radius_m,
            Fence::Polygon { corners } => polygon_contains(corners, p),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn distance_on_the_mean_sphere() {
        // One degree of arc is R * pi / 180 by definition of the sphere.
        let one_deg = EARTH_MEAN_RADIUS_M * std::f64::consts::PI / 180.0;
        assert!(close(
            Position::new(0.0, 0.0).distance_m(&Position::new(0.0, 1.0)),
            one_deg,
            1e-6
        ));
        assert!(close(
            Position::new(10.0, 20.0).distance_m(&Position::new(11.0, 20.0)),
            one_deg,
            1e-6
        ));
        // Pole to pole is half the circumference; antipodes do not overflow the arcsine.
        let half = EARTH_MEAN_RADIUS_M * std::f64::consts::PI;
        assert!(close(
            Position::new(90.0, 0.0).distance_m(&Position::new(-90.0, 0.0)),
            half,
            1e-6
        ));
        assert!(close(
            Position::new(0.0, 0.0).distance_m(&Position::new(0.0, 180.0)),
            half,
            1e-6
        ));
        assert_eq!(
            Position::new(12.0, 34.0).distance_m(&Position::new(12.0, 34.0)),
            0.0
        );
    }

    #[test]
    fn bearings() {
        let o = Position::new(0.0, 0.0);
        assert!(close(
            o.initial_bearing_deg(&Position::new(0.0, 1.0)),
            90.0,
            1e-12
        ));
        assert!(close(
            o.initial_bearing_deg(&Position::new(1.0, 0.0)),
            0.0,
            1e-12
        ));
        assert!(close(
            o.initial_bearing_deg(&Position::new(0.0, -1.0)),
            270.0,
            1e-12
        ));
        assert!(close(
            o.initial_bearing_deg(&Position::new(-1.0, 0.0)),
            180.0,
            1e-12
        ));
        // From 45N 0E due east along the great circle: the final bearing exceeds 90 as the circle bends south.
        let a = Position::new(45.0, 0.0);
        let b = a.destination(90.0, 1_000_000.0);
        assert!(close(a.initial_bearing_deg(&b), 90.0, 1e-9));
        assert!(a.final_bearing_deg(&b) > 90.0);
        // Spherical-trigonometry identity: sin(az1)·cos(lat1) = sin(az2)·cos(lat2) (Clairaut).
        let c1 = a.initial_bearing_deg(&b).to_radians().sin() * a.lat_deg.to_radians().cos();
        let c2 = a.final_bearing_deg(&b).to_radians().sin() * b.lat_deg.to_radians().cos();
        assert!(close(c1, c2, 1e-12));
    }

    #[test]
    fn destination_inverts_distance_and_bearing() {
        let sydney = Position::new(-33.8688, 151.2093);
        let melbourne = Position::new(-37.8136, 144.9631);
        let d = sydney.distance_m(&melbourne);
        let b = sydney.initial_bearing_deg(&melbourne);
        assert!(close(d, 713_400.0, 2_000.0), "{d}");
        let back = sydney.destination(b, d);
        assert!(back.distance_m(&melbourne) < 1e-3);
        // Crossing the antimeridian wraps the longitude.
        let p = Position::new(0.0, 179.5).destination(90.0, 111_195.0);
        assert!(close(p.lon_deg, -179.5, 1e-3), "{p:?}");
    }

    #[test]
    fn midpoint_cross_and_along_track() {
        let a = Position::new(0.0, 0.0);
        let b = Position::new(0.0, 10.0);
        let m = a.midpoint(&b);
        assert!(close(m.lat_deg, 0.0, 1e-12) && close(m.lon_deg, 5.0, 1e-12));
        let one_deg = EARTH_MEAN_RADIUS_M * 1f64.to_radians();
        // North of an eastbound path is to its left: negative.
        let p = Position::new(1.0, 5.0);
        assert!(close(p.cross_track_m(&a, &b), -one_deg, 1e-6));
        assert!(close(
            Position::new(-1.0, 5.0).cross_track_m(&a, &b),
            one_deg,
            1e-6
        ));
        assert!(close(p.along_track_m(&a, &b), 5.0 * one_deg, 1e-3));
        assert!(Position::new(0.0, -2.0).along_track_m(&a, &b) < 0.0);
    }

    #[test]
    fn closest_approach_cases() {
        let me = Track {
            position: Position::new(0.0, 0.0),
            speed_m_s: 0.0,
            course_deg: 0.0,
        };
        let at = Position::new(0.0, 0.0)
            .destination(0.0, 5_000.0)
            .destination(270.0, 10_000.0);
        let v = 10.0 * crate::KNOT_M_S;
        let a = closest_approach(
            &me,
            &Track {
                position: at,
                speed_m_s: v,
                course_deg: 90.0,
            },
        );
        assert!(close(a.distance_m, 5_000.0, 50.0), "{a:?}");
        assert!(close(a.time_s, 10_000.0 / v, 10_000.0 / v * 0.01), "{a:?}");
        // Moving away: the closest point is now.
        let away = closest_approach(
            &me,
            &Track {
                position: at,
                speed_m_s: v,
                course_deg: 270.0,
            },
        );
        assert_eq!(away.time_s, 0.0);
        // Two ships on reciprocal courses 1 km apart abeam meet at CPA 1 km after equal times.
        let a0 = Track {
            position: Position::new(0.0, 0.0),
            speed_m_s: 5.0,
            course_deg: 0.0,
        };
        let b0 = Track {
            position: Position::new(0.0, 0.0)
                .destination(90.0, 1_000.0)
                .destination(0.0, 10_000.0),
            speed_m_s: 5.0,
            course_deg: 180.0,
        };
        let r = closest_approach(&a0, &b0);
        assert!(
            close(r.distance_m, 1_000.0, 5.0) && close(r.time_s, 1_000.0, 10.0),
            "{r:?}"
        );
        // NaN speed is treated as no relative motion, not a panic.
        let n = closest_approach(
            &me,
            &Track {
                position: at,
                speed_m_s: f64::NAN,
                course_deg: 0.0,
            },
        );
        assert_eq!(n.time_s, 0.0);
    }

    #[test]
    fn fences_and_checks() {
        let c = Fence::Circle {
            centre: Position::new(0.0, 0.0),
            radius_m: 100_000.0,
        };
        assert!(c.contains(&Position::new(0.5, 0.5)));
        assert!(!c.contains(&Position::new(1.0, 1.0)));
        let sq = Fence::Polygon {
            corners: vec![
                Position::new(0.0, 0.0),
                Position::new(0.0, 1.0),
                Position::new(1.0, 1.0),
                Position::new(1.0, 0.0),
            ],
        };
        assert!(sq.contains(&Position::new(0.5, 0.5)));
        assert!(!sq.contains(&Position::new(1.5, 0.5)));
        assert!(!polygon_contains(&[], &Position::new(0.0, 0.0)));
        assert_eq!(
            Position::checked(91.0, 0.0, 0.0),
            Err(Error::OutOfRange(Quantity::Latitude))
        );
        assert_eq!(
            Position::checked(0.0, 181.0, 0.0),
            Err(Error::OutOfRange(Quantity::Longitude))
        );
        assert_eq!(Position::checked(f64::NAN, 0.0, 0.0), Err(Error::NotFinite));
        assert_eq!(wrap180(540.0), 180.0);
        assert_eq!(wrap180(-180.0), 180.0);
        assert_eq!(wrap360(-1e-20), 0.0);
        assert_eq!(wrap360(720.5), 0.5);
        assert_eq!(compass_point(11.24), "N");
        assert_eq!(compass_point(11.25), "NNE");
        assert_eq!(compass_point(348.76), "N");
        assert_eq!(compass_point(f64::NAN), "");
    }
}

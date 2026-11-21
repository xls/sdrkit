//! Distances on the WGS-84 ellipsoid and the travel time of a radio wave between two points.
//!
//! Distances use Vincenty's inverse formula on WGS-84 (`sdrkit_util_geo::Ellipsoid::WGS84`). For nearly antipodal points, where it
//! does not converge, the haversine distance on the mean radius 6 371 008.8 m replaces it. The travel time is the ground wave at
//! `velocity_factor` times the speed of light, or, when hops are on and the path is 1500 km or longer, the path of equal hops
//! between the ground and a reflecting layer at `hop_height_m`.

use sdrkit_util_geo::position::wrap180;
use sdrkit_util_geo::{Ellipsoid, Position, EARTH_MEAN_RADIUS_M};

use crate::LocateError;

/// The speed of light in vacuum, m/s.
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// With hops on, paths of this length or longer take the hop model, m.
pub const HOP_MIN_DISTANCE_M: f64 = 1_500_000.0;

/// The longest ground distance that one hop covers, m.
pub const HOP_SPAN_M: f64 = 3_000_000.0;

/// The geodesic distance between two positions on WGS-84, m (heights ignored). Vincenty's inverse formula; the haversine distance
/// on the mean radius when it does not converge.
///
/// ```
/// use sdrkit_algo_locate::propagation::distance_m;
/// use sdrkit_util_geo::Position;
/// let d = distance_m(&Position::new(48.0, 8.0), &Position::new(48.0, 9.0));
/// assert!((d - 74_625.0).abs() < 50.0);
/// ```
pub fn distance_m(from: &Position, to: &Position) -> f64 {
    match Ellipsoid::WGS84.inverse(from, &beside(from, to)) {
        Ok(g) => g.distance_m,
        Err(_) => from.distance_m(to),
    }
}

/// The position reached from `from` after `distance_m` along the geodesic that leaves on `bearing_deg`. Vincenty's direct formula
/// on WGS-84; the great circle on the mean radius when it does not converge.
pub fn destination(from: &Position, bearing_deg: f64, distance_m: f64) -> Position {
    match Ellipsoid::WGS84.direct(from, bearing_deg, distance_m) {
        Ok((p, _)) => p,
        Err(_) => from.destination(bearing_deg, distance_m),
    }
}

/// The offset of `to` from `from` as (north, east) metres: the geodesic distance resolved along the initial bearing.
pub fn north_east_m(from: &Position, to: &Position) -> (f64, f64) {
    let (d, bearing) = match Ellipsoid::WGS84.inverse(from, &beside(from, to)) {
        Ok(g) => (g.distance_m, g.initial_bearing_deg),
        Err(_) => (from.distance_m(to), from.initial_bearing_deg(to)),
    };
    let (s, c) = bearing.to_radians().sin_cos();
    (d * c, d * s)
}

/// `to` with its longitude moved by whole turns to within 180 degrees of the longitude of `from`, so that the inverse formula
/// takes the short way across the 180 degree meridian.
fn beside(from: &Position, to: &Position) -> Position {
    Position {
        lon_deg: from.lon_deg + wrap180(to.lon_deg - from.lon_deg),
        ..*to
    }
}

/// The propagation model: ground wave, with optional ionosphere hops on long paths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Propagation {
    /// Ground-wave speed over the speed of light (above 0, at most 1). Default 0.9993.
    pub velocity_factor: f64,
    /// Paths of [`HOP_MIN_DISTANCE_M`] or longer travel by hops. Default off.
    pub hops: bool,
    /// Height of the reflecting layer for hops, m (above 0, at most 1000 km). Default 80 km.
    pub hop_height_m: f64,
}

impl Default for Propagation {
    fn default() -> Self {
        Propagation {
            velocity_factor: 0.9993,
            hops: false,
            hop_height_m: 80_000.0,
        }
    }
}

impl Propagation {
    /// Errors when the velocity factor or the hop height is out of range.
    pub fn validate(&self) -> Result<(), LocateError> {
        if !(self.velocity_factor > 0.0 && self.velocity_factor <= 1.0) {
            return LocateError::range("velocity factor", self.velocity_factor);
        }
        if !(self.hop_height_m > 0.0 && self.hop_height_m <= 1.0e6) {
            return LocateError::range("hop height", self.hop_height_m);
        }
        Ok(())
    }

    /// The ground-wave travel time over `distance_m`, s: `distance_m / (velocity_factor c)`.
    pub fn ground_time_s(&self, distance_m: f64) -> f64 {
        distance_m / (self.velocity_factor * SPEED_OF_LIGHT_M_S)
    }

    /// The travel time over a geodesic of `distance_m`, s. With hops on and a path of [`HOP_MIN_DISTANCE_M`] or more: `n =
    /// max(1, ceil(d / HOP_SPAN_M))` equal hops, each of two straight legs between the ground (mean radius R) and the layer at
    /// height h, a leg of length `sqrt(R^2 + (R + h)^2 - 2 R (R + h) cos(d / (2 n R)))`, at the speed of light. Otherwise the
    /// ground wave.
    pub fn travel_time_s(&self, distance_m: f64) -> f64 {
        if self.hops && distance_m >= HOP_MIN_DISTANCE_M {
            let r = EARTH_MEAN_RADIUS_M;
            let h = self.hop_height_m;
            let n = (distance_m / HOP_SPAN_M).ceil().max(1.0);
            let half = distance_m / (r * n * 2.0);
            // The law of cosines in the form that keeps its precision for small angles.
            let s = (half / 2.0).sin();
            let leg = (h * h + 4.0 * r * (r + h) * s * s).sqrt();
            2.0 * n * leg / SPEED_OF_LIGHT_M_S
        } else {
            self.ground_time_s(distance_m)
        }
    }

    /// The travel time between two positions, s.
    pub fn delay_s(&self, from: &Position, to: &Position) -> f64 {
        self.travel_time_s(distance_m(from, to))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    fn dms(d: f64, m: f64, s: f64) -> f64 {
        d.signum() * (d.abs() + m / 60.0 + s / 3600.0)
    }

    #[test]
    fn vincenty_example_on_wgs84() {
        let flinders = Position::new(dms(-37.0, 57.0, 3.72030), dms(144.0, 25.0, 29.52440));
        let buninyong = Position::new(dms(-37.0, 39.0, 10.15610), dms(143.0, 55.0, 35.38390));
        let d = distance_m(&flinders, &buninyong);
        assert!((d - 54_972.271).abs() <= 0.01, "{d:.4}");
    }

    #[test]
    fn direct_and_inverse_round_trips() {
        let mut rng = Rng::new(0x10ca7e);
        for _ in 0..200 {
            let from = Position::new(rng.range(-80.0, 80.0), rng.range(-180.0, 180.0));
            let bearing = rng.range(0.0, 360.0);
            let d = rng.range(1.0, 8_000_000.0);
            let to = destination(&from, bearing, d);
            let back = distance_m(&from, &to);
            assert!((back - d).abs() < 1e-3, "{from:?} {bearing} {d} {back}");
        }
        // Nearly antipodal points still have a distance, close to half the circumference.
        let d = distance_m(&Position::new(0.0, 0.0), &Position::new(0.5, 179.7));
        assert!((19_800e3..20_100e3).contains(&d), "{d}");
        let (n, e) = north_east_m(&Position::new(48.0, 8.0), &Position::new(48.0, 8.01));
        assert!(n.abs() < 1.0 && (e - 746.0).abs() < 1.0, "{n} {e}");
    }

    #[test]
    fn hops_apply_to_long_paths_only() {
        let p = Propagation {
            hops: true,
            ..Propagation::default()
        };
        p.validate().unwrap();
        let d = 1_000_000.0;
        assert_eq!(p.travel_time_s(d), d / (0.9993 * SPEED_OF_LIGHT_M_S));
        let d = 2_000_000.0;
        assert!(p.travel_time_s(d) > d / SPEED_OF_LIGHT_M_S);
        // Two hops from 3000 km on; without hops the ground wave everywhere.
        assert!(p.travel_time_s(3_000_001.0) > p.travel_time_s(3_000_000.0));
        let ground = Propagation::default();
        assert_eq!(ground.travel_time_s(d), ground.ground_time_s(d));
        assert!(Propagation {
            velocity_factor: 1.2,
            ..ground
        }
        .validate()
        .is_err());
    }
}

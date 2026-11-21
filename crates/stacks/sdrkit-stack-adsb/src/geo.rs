//! Distances and bearings in nautical miles on `sdrkit-util-geo`. The air data (standard atmosphere, wind triangle, magnetic
//! declination) is in `sdrkit_proto_aero::air`.

use sdrkit_util_geo::{Position, NAUTICAL_MILE_M};

/// The great-circle distance in nautical miles (`sdrkit_util_geo::Position::distance_m`).
pub fn distance_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    Position::new(lat1, lon1).distance_m(&Position::new(lat2, lon2)) / NAUTICAL_MILE_M
}

/// The initial great-circle bearing from the first point to the second, 0 to 360 degrees.
pub fn bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    Position::new(lat1, lon1).initial_bearing_deg(&Position::new(lat2, lon2))
}

/// The point `dist_nm` from a start along the initial bearing `brg_deg`, and the track on arrival there.
pub fn destination(lat: f64, lon: f64, brg_deg: f64, dist_nm: f64) -> (f64, f64, f64) {
    let a = Position::new(lat, lon);
    let b = a.destination(brg_deg, dist_nm * NAUTICAL_MILE_M);
    (b.lat_deg, b.lon_deg, a.final_bearing_deg(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_bearings() {
        assert!((distance_nm(0.0, 0.0, 0.0, 1.0) - 60.04).abs() < 0.05);
        assert!((bearing_deg(0.0, 0.0, 0.0, 1.0) - 90.0).abs() < 1e-9);
        assert!(bearing_deg(0.0, 0.0, 1.0, 0.0).abs() < 1e-9);
        assert!((bearing_deg(1.0, 0.0, 0.0, 0.0) - 180.0).abs() < 1e-9);
        assert!((bearing_deg(0.0, 1.0, 0.0, 0.0) - 270.0).abs() < 1e-9);
        let (la, lo, _) = destination(25.25, 55.36, 45.0, 100.0);
        assert!((distance_nm(25.25, 55.36, la, lo) - 100.0).abs() < 1e-6);
    }
}

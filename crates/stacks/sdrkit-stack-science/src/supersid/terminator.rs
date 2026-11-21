//! Terminator marks of a path: for the transmitter, the great-circle midpoint and the receiver, the times of one UTC day at
//! which the Sun crosses the ground limit (−0.833°) and the D-region limit (−acos(6371 / 6456), about −9.3°), and the share of
//! the path whose D region is sunlit.

use crate::astro::sky;
use sdrkit_util_geo::Position;

/// The Sun's elevation at sunrise and sunset at the ground, degrees.
pub const GROUND_DEG: f64 = -0.833;
/// The Earth's radius and the radius of the D region (85 km up), km.
pub const RADII_KM: (f64, f64) = (6371.0, 6456.0);
/// The search step, s.
pub const STEP_S: f64 = 60.0;
/// The bisections that refine a crossing.
pub const BISECTIONS: u32 = 25;
/// The points of the path whose sunlight is counted.
pub const PATH_POINTS: usize = 21;

/// The Sun's elevation when it sets at the D region, degrees (about −9.3).
pub fn d_region_deg() -> f64 {
    -(RADII_KM.0 / RADII_KM.1).acos().to_degrees()
}

/// The Sun's elevation at `p` at Unix time `unix`, degrees, without refraction.
pub fn sun_elevation(p: &Position, unix: f64) -> f64 {
    let jd = sky::jd_unix(unix);
    let (ra, dec) = sky::sun(jd);
    sky::horizontal(ra, dec, jd, p.lat_deg, p.lon_deg, false).1
}

/// Which crossing a mark is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    /// The Sun rises through −0.833°.
    SunriseGround,
    /// The Sun sets through −0.833°.
    SunsetGround,
    /// The Sun rises through the D-region limit.
    SunriseD,
    /// The Sun sets through the D-region limit.
    SunsetD,
}

impl MarkKind {
    /// The words of the mark.
    pub fn text(self) -> &'static str {
        match self {
            MarkKind::SunriseGround => "sunrise, ground",
            MarkKind::SunsetGround => "sunset, ground",
            MarkKind::SunriseD => "sunrise, D region",
            MarkKind::SunsetD => "sunset, D region",
        }
    }
}

/// Which point of the path a mark belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathPoint {
    /// The transmitter.
    Transmitter,
    /// The great-circle midpoint.
    Midpoint,
    /// The receiver.
    Receiver,
}

impl PathPoint {
    /// The word of the point.
    pub fn text(self) -> &'static str {
        match self {
            PathPoint::Transmitter => "transmitter",
            PathPoint::Midpoint => "midpoint",
            PathPoint::Receiver => "receiver",
        }
    }
}

/// One crossing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    /// The point of the path.
    pub point: PathPoint,
    /// The crossing.
    pub kind: MarkKind,
    /// Its Unix time, s.
    pub unix: f64,
}

/// The crossings of `limit_deg` at `p` during the 24 hours from `day_start` (Unix s): each found in 60 s steps and refined by
/// 25 bisections. Each item is (time, rising).
pub fn crossings(p: &Position, day_start: f64, limit_deg: f64) -> Vec<(f64, bool)> {
    let f = |t: f64| sun_elevation(p, t) - limit_deg;
    let mut out = Vec::new();
    let steps = (86_400.0 / STEP_S) as usize;
    let mut t0 = day_start;
    let mut v0 = f(t0);
    for i in 1..=steps {
        let t1 = day_start + i as f64 * STEP_S;
        let v1 = f(t1);
        if (v0 < 0.0) != (v1 < 0.0) {
            let (mut a, mut b, va) = (t0, t1, v0);
            for _ in 0..BISECTIONS {
                let m = 0.5 * (a + b);
                if (f(m) < 0.0) == (va < 0.0) {
                    a = m;
                } else {
                    b = m;
                }
            }
            out.push((0.5 * (a + b), v1 >= 0.0));
        }
        t0 = t1;
        v0 = v1;
    }
    out
}

/// The `n` points from `a` to `b` along the great circle, both ends included.
pub fn path_points(a: &Position, b: &Position, n: usize) -> Vec<Position> {
    let d = a.distance_m(b);
    let bearing = a.initial_bearing_deg(b);
    (0..n)
        .map(|i| {
            let f = i as f64 / (n.max(2) - 1) as f64;
            if f == 0.0 {
                *a
            } else if f == 1.0 {
                *b
            } else {
                a.destination(bearing, d * f)
            }
        })
        .collect()
}

/// The marks of the path from the transmitter `tx` to the receiver `rx` on the UTC day that starts at `day_start`, in time
/// order. A point where the Sun does not cross a limit that day has no mark for it.
pub fn path_marks(tx: &Position, rx: &Position, day_start: f64) -> Vec<Mark> {
    let mid = tx.midpoint(rx);
    let mut out = Vec::new();
    for (point, p) in [
        (PathPoint::Transmitter, tx),
        (PathPoint::Midpoint, &mid),
        (PathPoint::Receiver, rx),
    ] {
        for (limit, kinds) in [
            (GROUND_DEG, (MarkKind::SunriseGround, MarkKind::SunsetGround)),
            (d_region_deg(), (MarkKind::SunriseD, MarkKind::SunsetD)),
        ] {
            for (unix, rising) in crossings(p, day_start, limit) {
                out.push(Mark {
                    point,
                    kind: if rising { kinds.0 } else { kinds.1 },
                    unix,
                });
            }
        }
    }
    out.sort_by(|a, b| a.unix.total_cmp(&b.unix));
    out
}

/// The share (0 to 1) of the 21 equally spaced points of the path from `tx` to `rx` where the Sun stands above the D-region
/// limit at Unix time `unix`.
pub fn sunlit_share(tx: &Position, rx: &Position, unix: f64) -> f64 {
    let limit = d_region_deg();
    let pts = path_points(tx, rx, PATH_POINTS);
    let lit = pts
        .iter()
        .filter(|p| sun_elevation(p, unix) > limit)
        .count();
    lit as f64 / pts.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_d_region_limit_and_a_day_of_marks() {
        assert!((d_region_deg() + 9.3).abs() < 0.05);
        // 2026-03-20, Greenwich: the ground sunrise near 06:00 UTC and sunset near 18:10 UTC.
        let day = 1_773_964_800.0;
        let c = crossings(&Position::new(51.48, 0.0), day, GROUND_DEG);
        assert_eq!(c.len(), 2);
        assert!(c[0].1 && !c[1].1);
        assert!((c[0].0 - day - 6.0 * 3600.0).abs() < 1800.0, "{}", c[0].0 - day);
        assert!((c[1].0 - day - 18.2 * 3600.0).abs() < 1800.0, "{}", c[1].0 - day);
        let tx = Position::new(54.91, -3.28);
        let rx = Position::new(52.5, 13.4);
        let m = path_marks(&tx, &rx, day);
        assert_eq!(m.len(), 12);
        assert_eq!(sunlit_share(&tx, &rx, day + 12.0 * 3600.0), 1.0);
        assert_eq!(sunlit_share(&tx, &rx, day), 0.0);
    }
}

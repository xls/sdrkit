//! Trails of positions.

use crate::error::Error;
use crate::persist::Shift;
use sdrkit_util_geo::Position;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// One point of a trail: where, when, and whatever the caller keeps with it (an altitude band, "on the ground").
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrailPoint<X> {
    /// Signal time, seconds.
    pub t: f64,
    /// Where.
    pub position: Position,
    /// The caller's data.
    pub extra: X,
}

/// When a new position becomes a trail point: at least `min_s` seconds **and** at least `min_m` metres after the last point kept.
/// Zero for either means no limit of that kind.
///
/// Aircraft trails space points in time (a point every 5 s). Ship tracks space them in distance (a point when the ship has moved),
/// so a ship at anchor does not fill its track with the same place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Spacing {
    /// The least time between points, seconds.
    pub min_s: f64,
    /// The least distance between points, metres (great circle, on the mean sphere of `sdrkit-util-geo`).
    pub min_m: f64,
}

impl Spacing {
    /// Points at least `s` seconds apart.
    pub const fn seconds(s: f64) -> Spacing {
        Spacing {
            min_s: s,
            min_m: 0.0,
        }
    }
    /// Points at least `m` metres apart.
    pub const fn metres(m: f64) -> Spacing {
        Spacing {
            min_s: 0.0,
            min_m: m,
        }
    }
}

/// A trail of positions with a point cap and a minimum spacing. If the trail is full, the oldest point leaves.
///
/// ```
/// use sdrkit_util_geo::Position;
/// use sdrkit_util_objects::{Spacing, Trail};
///
/// let mut trail: Trail = Trail::new(80, Spacing::seconds(5.0));
/// assert!(trail.push(0.0, Position::new(-33.95, 151.18), ()).unwrap());
/// assert!(!trail.push(2.0, Position::new(-33.96, 151.18), ()).unwrap()); // too soon after the last point
/// assert!(trail.push(5.0, Position::new(-33.97, 151.18), ()).unwrap());
/// assert_eq!(trail.len(), 2);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Trail<X = ()> {
    points: VecDeque<TrailPoint<X>>,
    cap: usize,
    spacing: Spacing,
}

impl<X> Trail<X> {
    /// An empty trail of at most `cap` points (at least 1), spaced by `spacing`.
    pub fn new(cap: usize, spacing: Spacing) -> Self {
        let cap = cap.max(1);
        Trail {
            points: VecDeque::with_capacity(cap),
            cap,
            spacing,
        }
    }

    /// Adds a position heard at signal time `t` when it is far enough, in time and distance, from the last point kept. Returns
    /// whether it was kept. A time that is not finite, or a position out of range, is refused with an error.
    pub fn push(&mut self, t: f64, position: Position, extra: X) -> Result<bool, Error> {
        if !t.is_finite() {
            return Err(Error::BadTime);
        }
        if position.validate().is_err() {
            return Err(Error::BadPosition);
        }
        if let Some(last) = self.points.back() {
            if (t - last.t).abs() < self.spacing.min_s {
                return Ok(false);
            }
            if self.spacing.min_m > 0.0 && last.position.distance_m(&position) < self.spacing.min_m
            {
                return Ok(false);
            }
        }
        if self.points.len() >= self.cap {
            self.points.pop_front();
        }
        self.points.push_back(TrailPoint { t, position, extra });
        Ok(true)
    }

    /// The points, oldest first.
    pub fn points(&self) -> impl DoubleEndedIterator<Item = &TrailPoint<X>> + ExactSizeIterator {
        self.points.iter()
    }

    /// The newest point.
    pub fn last(&self) -> Option<&TrailPoint<X>> {
        self.points.back()
    }

    /// Points now.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether there are no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The most points kept.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// The spacing.
    pub fn spacing(&self) -> Spacing {
        self.spacing
    }

    /// Forgets every point.
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// Removes the points older than `max_age_s` seconds at signal time `now_s`. Returns how many left.
    pub fn expire(&mut self, now_s: f64, max_age_s: f64) -> usize {
        let mut n = 0;
        while self.points.front().is_some_and(|p| now_s - p.t > max_age_s) {
            self.points.pop_front();
            n += 1;
        }
        n
    }

    /// The segments to draw, as pairs of consecutive points, leaving out a segment that crosses the date line (its longitudes
    /// differ by more than 180°). On a flat map such a segment runs the wrong way round the world.
    ///
    /// ```
    /// use sdrkit_util_geo::Position;
    /// use sdrkit_util_objects::{Spacing, Trail};
    /// let mut t: Trail = Trail::new(10, Spacing::default());
    /// for (i, lon) in [178.0, 179.5, -179.5, -178.0].into_iter().enumerate() {
    ///     t.push(i as f64, Position::new(-17.0, lon), ()).unwrap();
    /// }
    /// assert_eq!(t.segments().count(), 2); // 178 -> 179.5 and -179.5 -> -178
    /// ```
    pub fn segments(&self) -> impl Iterator<Item = (&TrailPoint<X>, &TrailPoint<X>)> {
        self.points
            .iter()
            .zip(self.points.iter().skip(1))
            .filter(|(a, b)| (b.position.lon_deg - a.position.lon_deg).abs() <= 180.0)
    }

    /// Takes the points of `saved` (a restored state), keeping this trail's cap and spacing (the newest points stay).
    pub fn adopt(&mut self, saved: Trail<X>) {
        self.points.clear();
        let skip = saved.points.len().saturating_sub(self.cap);
        self.points.extend(saved.points.into_iter().skip(skip));
    }
}

impl<X> Shift for Trail<X> {
    fn shift_times(&mut self, by_s: f64) {
        for p in &mut self.points {
            p.t += by_s;
        }
    }
}

#[derive(Serialize)]
struct TrailOut<'a, X> {
    cap: usize,
    spacing: Spacing,
    points: &'a VecDeque<TrailPoint<X>>,
}

#[derive(Deserialize)]
struct TrailIn<X> {
    cap: usize,
    spacing: Spacing,
    points: VecDeque<TrailPoint<X>>,
}

/// Written as `{"cap", "spacing", "points": [{"t", "position", "extra"}, ...]}`, oldest first.
impl<X: Serialize> Serialize for Trail<X> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TrailOut {
            cap: self.cap,
            spacing: self.spacing,
            points: &self.points,
        }
        .serialize(s)
    }
}

/// Reads what [`Serialize`] wrote. Points with a time that is not finite or a position out of range are refused.
impl<'de, X: Deserialize<'de>> Deserialize<'de> for Trail<X> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let t = TrailIn::<X>::deserialize(d)?;
        if t.points
            .iter()
            .any(|p| !p.t.is_finite() || p.position.validate().is_err())
        {
            return Err(D::Error::custom("a trail point out of range"));
        }
        let cap = t.cap.max(1);
        let mut points = t.points;
        while points.len() > cap {
            points.pop_front();
        }
        points.reserve(cap - points.len());
        Ok(Trail {
            points,
            cap,
            spacing: t.spacing,
        })
    }
}

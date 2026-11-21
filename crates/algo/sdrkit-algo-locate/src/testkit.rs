//! Receiver layouts and simulated arrival times for the tests.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_util_geo::Position;

use crate::propagation::{destination, Propagation};
use crate::tdoa::Observation;

/// `n` receivers on a circle of `radius_m` around `centre`, evenly spaced from north.
pub(crate) fn ring(centre: &Position, radius_m: f64, n: usize) -> Vec<Position> {
    (0..n)
        .map(|k| destination(centre, 360.0 * k as f64 / n as f64, radius_m))
        .collect()
}

/// Six receivers on a circle of `radius_m`, every third at 0.7 of the radius, the odd ones turned by 10 degrees.
pub(crate) fn uneven_ring(centre: &Position, radius_m: f64) -> Vec<Position> {
    (0..6)
        .map(|k| {
            let r = if k % 3 == 0 { 0.7 * radius_m } else { radius_m };
            let turn = if k % 2 == 1 { 10.0 } else { 0.0 };
            destination(centre, 60.0 * k as f64 + turn, r)
        })
        .collect()
}

/// A position at a distance drawn uniformly from 0 to `max_m` from `centre`, in a direction drawn uniformly.
pub(crate) fn around(rng: &mut Rng, centre: &Position, max_m: f64) -> Position {
    let r = max_m * rng.uniform();
    destination(centre, rng.range(0.0, 360.0), r)
}

/// Arrival times of a signal sent at time 0 from `source`, with Gaussian errors of `sigma_s`; one clock group, no ambiguity.
pub(crate) fn observations(
    rng: &mut Rng,
    stations: &[Position],
    source: &Position,
    prop: &Propagation,
    sigma_s: f64,
) -> Vec<Observation> {
    stations
        .iter()
        .map(|p| Observation {
            position: *p,
            time_s: prop.delay_s(source, p) + sigma_s * rng.normal(),
            sigma_s,
            group: 0,
            ambiguity_s: 0.0,
        })
        .collect()
}

/// The median (upper middle of an even count); NaN for no values.
pub(crate) fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

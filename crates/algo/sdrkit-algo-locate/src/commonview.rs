//! Common-view alignment: receiver clocks set to one time scale by the symbol boundaries of a transmitter that all of them hear.
//!
//! The transmitter's symbols start on a grid `E + k T` (T the symbol period); each receiver reads a boundary on its own clock.
//! The true arrival at receiver i is `E + k T + d_i / v`, `d_i` the geodesic distance from the transmitter and `v` the
//! ground-wave speed. Receivers whose clocks are absolute (GPS or GNSS) fix the epoch `E`; every other receiver gets the
//! correction that puts its clock on that scale, known up to whole periods.

use sdrkit_util_geo::Position;

use crate::propagation::{distance_m, Propagation};
use crate::LocateError;

/// The path speed is known to this fraction; it adds `PATH_SPEED_ERROR x d / v` to a correction's sigma.
const PATH_SPEED_ERROR: f64 = 2e-4;

/// One receiver's reading of a symbol boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryReading {
    /// The receiver's position.
    pub position: Position,
    /// A symbol boundary on the receiver's clock, ns.
    pub boundary_ns: i64,
    /// The 1-sigma of the reading, s.
    pub sigma_s: f64,
    /// Whether the receiver's clock is absolute (tied to UTC by GPS or GNSS).
    pub absolute: bool,
}

/// The result of [`align`]; the vectors follow the order of the readings.
#[derive(Clone, Debug, PartialEq)]
pub struct Alignment {
    /// The boundary reading of the first receiver: the origin of the epoch, ns.
    pub origin_ns: i64,
    /// The epoch E of the symbol grid relative to `origin_ns`, s, in (-T/2, T/2]. 0 without an absolute receiver.
    pub epoch_s: f64,
    /// Whether an absolute receiver fixed the epoch (the corrected clocks are then UTC up to whole periods).
    pub tied_to_utc: bool,
    /// The correction to add to each receiver's clock, s, in (-T/2, T/2]; 0 for an absolute receiver.
    pub corrections_s: Vec<f64>,
    /// The 1-sigma of each correction, s; 0 for an absolute receiver.
    pub sigmas_s: Vec<f64>,
}

/// Aligns the receivers that read boundaries of one transmitter at `transmitter` with the symbol period `period_s`.
///
/// The epoch is the circular mean over the absolute receivers of the phase `2 pi ((b_i - b_0) mod T - d_i / v) / T`. A receiver
/// that is not absolute gets the correction `wrap(E + d_i / v - ((b_i - b_0) mod T))` into (-T/2, T/2] with the sigma `sqrt(s_i^2 +
/// s_E^2 + (2e-4 d_i / v)^2)`, `s_E = sqrt(sum of s^2) / n` over the n absolute receivers. Without an absolute receiver the epoch
/// is 0 and the corrected clocks form a time scale of their own. `v` is the ground-wave speed of `propagation`. Errors for no
/// readings, a period that is not above 0, or a reading sigma that is negative or not finite.
pub fn align(
    readings: &[BoundaryReading],
    transmitter: &Position,
    period_s: f64,
    propagation: &Propagation,
) -> Result<Alignment, LocateError> {
    if readings.is_empty() {
        return LocateError::range("number of readings", 0.0);
    }
    if !(period_s > 0.0 && period_s.is_finite()) {
        return LocateError::range("symbol period", period_s);
    }
    propagation.validate()?;
    if let Some(r) = readings
        .iter()
        .find(|r| !(r.sigma_s >= 0.0 && r.sigma_s.is_finite()))
    {
        return LocateError::range("reading sigma", r.sigma_s);
    }
    let t = period_s;
    let origin_ns = readings[0].boundary_ns;
    // The reading's place in the period, and the path delay, for each receiver.
    let place =
        |r: &BoundaryReading| (r.boundary_ns.saturating_sub(origin_ns) as f64 * 1e-9).rem_euclid(t);
    let delay: Vec<f64> = readings
        .iter()
        .map(|r| propagation.ground_time_s(distance_m(transmitter, &r.position)))
        .collect();
    let (mut sin, mut cos, mut var, mut n_abs) = (0.0, 0.0, 0.0, 0usize);
    for (r, d) in readings.iter().zip(&delay) {
        if r.absolute {
            let phase = std::f64::consts::TAU * (place(r) - d) / t;
            sin += phase.sin();
            cos += phase.cos();
            var += r.sigma_s * r.sigma_s;
            n_abs += 1;
        }
    }
    let wrap = |x: f64| {
        let y = x.rem_euclid(t);
        if y > t / 2.0 {
            y - t
        } else {
            y
        }
    };
    let epoch_s = if n_abs > 0 {
        wrap(sin.atan2(cos) / std::f64::consts::TAU * t)
    } else {
        0.0
    };
    let s_epoch = if n_abs > 0 {
        var.sqrt() / n_abs as f64
    } else {
        0.0
    };
    let mut corrections_s = Vec::with_capacity(readings.len());
    let mut sigmas_s = Vec::with_capacity(readings.len());
    for (r, d) in readings.iter().zip(&delay) {
        if r.absolute {
            corrections_s.push(0.0);
            sigmas_s.push(0.0);
        } else {
            corrections_s.push(wrap(epoch_s + d - place(r)));
            let path = PATH_SPEED_ERROR * d;
            sigmas_s.push((r.sigma_s * r.sigma_s + s_epoch * s_epoch + path * path).sqrt());
        }
    }
    Ok(Alignment {
        origin_ns,
        epoch_s,
        tied_to_utc: n_abs > 0,
        corrections_s,
        sigmas_s,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_receiver_clock_ahead_is_corrected_modulo_the_period() {
        let tx = Position::new(52.0, 10.0);
        let prop = Propagation::default();
        let period = 5e-3;
        // Boundaries leave the transmitter at UTC grid points E_true + k T.
        let grid_ns: i64 = 1_700_000_000_000_123_456;
        let sites = [
            (Position::new(48.1, 11.6), true, 0.0, 3),
            (Position::new(50.9, 6.9), true, 0.0, 17),
            (Position::new(53.6, 10.0), false, 7.3e-3, 40),
            (Position::new(47.4, 8.5), false, 0.0, 2),
        ];
        let readings: Vec<BoundaryReading> = sites
            .iter()
            .map(|&(p, absolute, ahead, k)| {
                let arrival = prop.ground_time_s(distance_m(&tx, &p)) + ahead + k as f64 * period;
                BoundaryReading {
                    position: p,
                    boundary_ns: grid_ns + (arrival * 1e9).round() as i64,
                    sigma_s: 1e-6,
                    absolute,
                }
            })
            .collect();
        let al = align(&readings, &tx, period, &prop).unwrap();
        assert!(al.tied_to_utc);
        assert_eq!(&al.corrections_s[..2], &[0.0, 0.0]);
        // 7.3 ms ahead is -7.3 ms, which is -2.3 ms modulo 5 ms; a clock already right needs nothing.
        assert!(
            (al.corrections_s[2] + 2.3e-3).abs() < 5e-9,
            "{:?}",
            al.corrections_s
        );
        assert!(al.corrections_s[3].abs() < 5e-9, "{:?}", al.corrections_s);
        let path = 2e-4 * prop.ground_time_s(distance_m(&tx, &sites[2].0));
        let want = (1e-12 + 0.5e-12 + path * path).sqrt();
        assert!((al.sigmas_s[2] - want).abs() < 1e-15, "{:?}", al.sigmas_s);
        // Without an absolute receiver the epoch is 0 and the clocks form their own scale.
        let own: Vec<BoundaryReading> = readings
            .iter()
            .map(|r| BoundaryReading {
                absolute: false,
                ..*r
            })
            .collect();
        let al = align(&own, &tx, period, &prop).unwrap();
        assert!(!al.tied_to_utc && al.epoch_s == 0.0);
        assert!(align(&[], &tx, period, &prop).is_err());
        assert!(align(&readings, &tx, 0.0, &prop).is_err());
    }
}

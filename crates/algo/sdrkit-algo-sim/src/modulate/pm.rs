//! Phase modulation with a residual carrier: shaped data move the phase of a carrier of constant amplitude.

use super::Linear;
use crate::{Param, SimError};
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_core::Complex32;

/// A phase modulator with a residual carrier: `y(t) = exp(j index d(t))`, where `d(t)` is a train of root raised cosine pulses
/// of sign +1 or -1 by the data, each of unit energy a symbol. A share of the power stays in the carrier line when the index
/// is below 90 degrees. Split-phase data are sent as two symbols (chips) for each bit.
///
/// ```
/// use sdrkit_algo_sim::modulate::PhaseMod;
/// let mut m = PhaseMod::new(1.0, 12.0, 4.0, 60f64.to_radians()).unwrap();
/// let mut out = Vec::new();
/// m.push(&[1, 0, 1, 1], &mut out);
/// assert!(out.iter().all(|z| (z.norm() - 1.0).abs() < 1e-3));
/// ```
#[derive(Clone, Debug)]
pub struct PhaseMod {
    shaper: Linear,
    index: f32,
    levels: Vec<Complex32>,
    shaped: Vec<Complex32>,
}

impl PhaseMod {
    /// A modulator with root raised cosine pulses of roll-off `beta` over `span` symbols at `sps` samples a symbol, and a phase
    /// index of `index_rad` radians.
    ///
    /// # Errors
    ///
    /// [`SimError`] if the pulse shaper refuses its options or the index is not between 0 and pi.
    pub fn new(beta: f64, span: f64, sps: f64, index_rad: f64) -> Result<Self, SimError> {
        if !(0.0..=std::f64::consts::PI).contains(&index_rad) {
            return Param::ModIndex.range(0.0, std::f64::consts::PI, index_rad);
        }
        Ok(PhaseMod {
            shaper: Linear::rrc(beta, span, sps)?,
            index: index_rad as f32,
            levels: Vec::new(),
            shaped: Vec::new(),
        })
    }

    /// Appends the samples that the symbols so far determine. A symbol of 1 moves the phase up, a symbol of 0 down.
    pub fn push(&mut self, symbols: &[u8], out: &mut Vec<Complex32>) {
        self.levels.clear();
        self.levels.extend(
            symbols
                .iter()
                .map(|&b| Complex32::new(if b & 1 == 1 { 1.0 } else { -1.0 }, 0.0)),
        );
        self.shaped.clear();
        self.shaper.push(&self.levels, &mut self.shaped);
        out.extend(self.shaped.iter().map(|d| {
            let (s, c) = sin_cos(f64::from(self.index * d.re));
            Complex32::new(c, s)
        }));
    }
}

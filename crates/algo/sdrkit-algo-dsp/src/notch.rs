//! Carrier notch: two one-pole notches in cascade that remove one strong carrier at a known offset from the tuned centre (for
//! example a VLF transmitter in front of an impulse detector, whose floor the carrier would lift).
//!
//! Each stage keeps `l1 <- (1 - r) x + r e^{jw} l1` and outputs `y = x - l1` (with `l1` already updated), so its response
//! `r (1 - e^{jw} z^-1) / (1 - r e^{jw} z^-1)` has an exact zero at `w = 2 pi offset / rate`. The width sets the pole radius
//! `r = exp(-2 pi width / rate)`; far from the carrier the gain of each stage is `r`.
//!
//! ```
//! use sdrkit_algo_dsp::notch::CarrierNotch;
//! use sdrkit_algo_dsp::Complex32;
//! let mut n = CarrierNotch::new(250_000.0, 40_000.0, 300.0).unwrap();
//! let w = std::f64::consts::TAU * 40_000.0 / 250_000.0;
//! let mut x: Vec<Complex32> = (0..25_000)
//!     .map(|i| Complex32::from_polar(1.0, (w * i as f64 % std::f64::consts::TAU) as f32))
//!     .collect();
//! n.process(&mut x);
//! assert!(x[24_999].norm() < 1e-3);
//! ```

use crate::{Complex32, DspError, Param};
use num_complex::Complex64;
use std::f64::consts::TAU;

/// The double carrier notch (module documentation).
#[derive(Clone, Debug)]
pub struct CarrierNotch {
    rate: f64,
    offset_hz: f64,
    width_hz: f64,
    r: f64,
    /// `r e^{jw}`.
    pole: Complex64,
    l1: [Complex64; 2],
}

impl CarrierNotch {
    /// A notch at `offset_hz` from the tuned centre for input at `rate`, with width `width_hz` (the pole radius
    /// `exp(-2 pi width / rate)`). Errors when `rate` is not above 0, the offset is outside `+-rate/2`, or the width is not
    /// above 0 and under `rate / 4`.
    pub fn new(rate: f64, offset_hz: f64, width_hz: f64) -> Result<Self, DspError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        if !(width_hz > 0.0 && width_hz < rate / 4.0) {
            return Param::Bandwidth.range(0.0, rate / 4.0, width_hz);
        }
        let r = (-TAU * width_hz / rate).exp();
        let mut n = CarrierNotch {
            rate,
            offset_hz: 0.0,
            width_hz,
            r,
            pole: Complex64::new(r, 0.0),
            l1: [Complex64::default(); 2],
        };
        n.set_offset(offset_hz)?;
        Ok(n)
    }

    /// The carrier offset, Hz.
    pub fn offset_hz(&self) -> f64 {
        self.offset_hz
    }

    /// The width, Hz.
    pub fn width_hz(&self) -> f64 {
        self.width_hz
    }

    /// Moves the notch to `offset_hz` from the next sample on; the state is kept. Errors outside `+-rate/2`.
    pub fn set_offset(&mut self, offset_hz: f64) -> Result<(), DspError> {
        if offset_hz.is_nan() || offset_hz.abs() > self.rate / 2.0 {
            return Param::OffsetHz.range(-self.rate / 2.0, self.rate / 2.0, offset_hz);
        }
        self.offset_hz = offset_hz;
        self.pole = Complex64::from_polar(self.r, TAU * offset_hz / self.rate);
        Ok(())
    }

    /// Clears the state of both stages.
    pub fn reset(&mut self) {
        self.l1 = [Complex64::default(); 2];
    }

    /// One sample through both stages. A sample that is not finite counts as 0, so the state stays finite.
    #[inline]
    pub fn push(&mut self, x: Complex32) -> Complex32 {
        let mut v = if x.re.is_finite() && x.im.is_finite() {
            Complex64::new(x.re as f64, x.im as f64)
        } else {
            Complex64::default()
        };
        let g = 1.0 - self.r;
        for l in &mut self.l1 {
            *l = v * g + self.pole * *l;
            v -= *l;
        }
        Complex32::new(v.re as f32, v.im as f32)
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for v in buf {
            *v = self.push(*v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{camp, ctone, db};

    #[test]
    fn removes_the_carrier_and_passes_the_rest() {
        let rate = 250_000.0;
        let mut n = CarrierNotch::new(rate, 40_000.0, 300.0).unwrap();
        let len = 50_000;
        let carrier = ctone(len, 40_000.0 / rate, 1.0);
        let other = ctone(len, 45_000.0 / rate, 0.01);
        let mut x: Vec<Complex32> = carrier.iter().zip(&other).map(|(a, b)| a + b).collect();
        n.process(&mut x);
        // After 10 ms the carrier is more than 80 dB down; a signal 5 kHz away loses under 0.2 dB.
        let tail = &x[len / 2..];
        assert!(db(camp(tail, 40_000.0 / rate)) < -80.0);
        let pass = db(camp(tail, 45_000.0 / rate) / 0.01);
        assert!(pass.abs() < 0.2, "{pass}");
        // Pieces of any size give the same output.
        let mut m = CarrierNotch::new(rate, 40_000.0, 300.0).unwrap();
        let mut y: Vec<Complex32> = carrier.iter().zip(&other).map(|(a, b)| a + b).collect();
        for piece in y.chunks_mut(777) {
            m.process(piece);
        }
        assert_eq!(x, y);
        m.reset();
        assert_eq!(m.push(Complex32::new(f32::NAN, 0.0)), Complex32::default());
    }

    #[test]
    fn settings_are_checked() {
        assert!(CarrierNotch::new(0.0, 0.0, 300.0).is_err());
        assert!(CarrierNotch::new(48_000.0, 30_000.0, 300.0).is_err());
        assert!(CarrierNotch::new(48_000.0, 1_000.0, 0.0).is_err());
        assert!(CarrierNotch::new(48_000.0, 1_000.0, 20_000.0).is_err());
        let mut n = CarrierNotch::new(48_000.0, 1_000.0, 300.0).unwrap();
        assert!(n.set_offset(f64::NAN).is_err());
        assert!(n.set_offset(-2_000.0).is_ok());
        assert_eq!(n.offset_hz(), -2_000.0);
        assert_eq!(n.width_hz(), 300.0);
    }
}

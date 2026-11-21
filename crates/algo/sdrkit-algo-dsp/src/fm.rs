//! FM demodulation: a fast `atan2`, the quadrature discriminator, and the droop correction for a discriminator that runs on a
//! decimated channel.

use crate::{Complex32, DspError, Param};
use std::f64::consts::TAU;

pub use crate::approx::{atan2 as fast_atan2, ATAN2_MAX_ERROR as FAST_ATAN2_MAX_ERROR};

/// The quadrature discriminator: the angle of `x[n] conj(x[n-1])` (the phase step, which is the instantaneous frequency), scaled so
/// a deviation of `deviation_hz` is output as 1.0. It keeps the last sample between calls, so the output does not depend on how the
/// input is split.
#[derive(Clone, Debug)]
pub struct Discriminator {
    prev: Complex32,
    gain: f32,
    rate: f64,
    deviation: f64,
}

impl Discriminator {
    /// For samples at `rate_hz` and a peak deviation of `deviation_hz` (75 kHz for broadcast FM). Errors unless both values are
    /// positive and finite.
    ///
    /// ```
    /// use sdrkit_algo_dsp::fm::Discriminator;
    /// assert!(Discriminator::new(48_000.0, 5_000.0).is_ok());
    /// assert!(Discriminator::new(48_000.0, 0.0).is_err());
    /// ```
    pub fn new(rate_hz: f64, deviation_hz: f64) -> Result<Self, DspError> {
        if !(rate_hz > 0.0 && rate_hz.is_finite()) {
            return Param::Rate.positive(rate_hz);
        }
        if !(deviation_hz > 0.0 && deviation_hz.is_finite()) {
            return Param::Deviation.positive(deviation_hz);
        }
        Ok(Discriminator {
            prev: Complex32::default(),
            gain: (rate_hz / (TAU * deviation_hz)) as f32,
            rate: rate_hz,
            deviation: deviation_hz,
        })
    }

    /// The same as [`Discriminator::new`]. It stays until its last users move to `new`.
    #[doc(hidden)]
    pub fn try_new(rate_hz: f64, deviation_hz: f64) -> Result<Self, DspError> {
        Self::new(rate_hz, deviation_hz)
    }

    /// Changes the deviation that is output as 1.0. A deviation that is not positive and finite is ignored.
    pub fn set_deviation(&mut self, deviation_hz: f64) {
        let _ = self.try_set_deviation(deviation_hz);
    }

    /// Changes the deviation that is output as 1.0. Errors (and keeps the old deviation) unless it is positive and finite.
    pub fn try_set_deviation(&mut self, deviation_hz: f64) -> Result<(), DspError> {
        let d = Self::new(self.rate, deviation_hz)?;
        self.gain = d.gain;
        self.deviation = d.deviation;
        Ok(())
    }

    /// The deviation that is output as 1.0, in Hz.
    pub fn deviation(&self) -> f64 {
        self.deviation
    }

    /// The gain: output per radian of phase step.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// The last sample taken (the reference for the next phase step).
    pub fn last(&self) -> Complex32 {
        self.prev
    }

    /// Sets the last sample (to continue from another discriminator's state without a step).
    pub fn set_last(&mut self, x: Complex32) {
        self.prev = x;
    }

    /// Forgets the last sample.
    pub fn reset(&mut self) {
        self.prev = Complex32::default();
    }

    /// One sample.
    #[inline]
    pub fn process_sample(&mut self, x: Complex32) -> f32 {
        let d = x * self.prev.conj();
        self.prev = x;
        fast_atan2(d.im, d.re) * self.gain
    }

    /// Demodulates `x` into `out` (the same length).
    ///
    /// # Panics
    /// If the lengths differ.
    pub fn process_into(&mut self, x: &[Complex32], out: &mut [f32]) {
        assert_eq!(x.len(), out.len());
        let Some(&last) = x.last() else { return };
        let g = self.gain;
        out[0] = {
            let d = x[0] * self.prev.conj();
            fast_atan2(d.im, d.re) * g
        };
        for (o, w) in out[1..].iter_mut().zip(x.windows(2)) {
            let d = w[1] * w[0].conj();
            *o = fast_atan2(d.im, d.re) * g;
        }
        self.prev = last;
    }

    /// Appends the demodulated samples of `x` to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        let start = out.len();
        out.resize(start + x.len(), 0.0);
        self.process_into(x, &mut out[start..]);
    }
}

/// A phase discriminator over a lag of `lag` samples: the angle of `x[n] conj(x[n - lag])`, scaled so a deviation of
/// `deviation_hz` is output as 1.0. A longer lag averages the frequency over more samples and lowers the noise, for example over one
/// bit at 4 samples per bit. It keeps the last `lag` samples between calls, so the output does not depend on how the input is split.
#[derive(Clone, Debug)]
pub struct LagDiscriminator {
    hist: Vec<Complex32>,
    pos: usize,
    gain: f32,
}

impl LagDiscriminator {
    /// For samples at `rate_hz`, a deviation of `deviation_hz` that is output as 1.0 (with 1.0 the output is in Hz), and a lag of `lag`
    /// samples. Errors unless the rate and deviation are positive and finite and `lag` is at least 1.
    ///
    /// ```
    /// use sdrkit_algo_dsp::fm::LagDiscriminator;
    /// assert!(LagDiscriminator::new(4e6, 1.0, 4).is_ok());
    /// assert!(LagDiscriminator::new(4e6, 1.0, 0).is_err());
    /// ```
    pub fn new(rate_hz: f64, deviation_hz: f64, lag: usize) -> Result<Self, DspError> {
        if lag == 0 {
            return Param::Lag.at_least(1.0, 0.0);
        }
        let d = Discriminator::new(rate_hz, deviation_hz)?;
        Ok(LagDiscriminator {
            hist: vec![Complex32::default(); lag],
            pos: 0,
            gain: d.gain / lag as f32,
        })
    }

    /// The lag, samples.
    pub fn lag(&self) -> usize {
        self.hist.len()
    }

    /// Forgets the past samples.
    pub fn reset(&mut self) {
        self.hist.fill(Complex32::default());
        self.pos = 0;
    }

    /// One sample. The first `lag` outputs after a reset compare with zero samples and give 0.
    #[inline]
    pub fn process_sample(&mut self, x: Complex32) -> f32 {
        let old = std::mem::replace(&mut self.hist[self.pos], x);
        self.pos += 1;
        if self.pos == self.hist.len() {
            self.pos = 0;
        }
        let d = x * old.conj();
        fast_atan2(d.im, d.re) * self.gain
    }

    /// Demodulates `x` into `out`. Errors if the lengths differ.
    pub fn process_into(&mut self, x: &[Complex32], out: &mut [f32]) -> Result<(), DspError> {
        if x.len() != out.len() {
            return Param::Output.length(x.len(), out.len());
        }
        for (o, &v) in out.iter_mut().zip(x) {
            *o = self.process_sample(v);
        }
        Ok(())
    }
}

/// The droop correction for a discriminator on a channel decimated by `d` from where the signal was made: `[-a, 1 + 2a, -a]` with
/// `a = (1 - 1/d^2) / 24`.
///
/// The discriminator measures the mean frequency over one channel sample, so the output droops like `sinc(f / fo)` (about 1 % at
/// 38 kHz on a 500 kHz channel), which costs stereo separation. For a signal decimated by `d` the droop is that of a `d`-sample
/// average, which this 3-tap filter undoes to second order. `d = 1` is the identity.
pub fn droop_correction(d: usize) -> [f32; 3] {
    let a = (1.0 - 1.0 / (d * d) as f32) / 24.0;
    [-a, 1.0 + 2.0 * a, -a]
}

/// `taps` with the [`droop_correction`] for a decimation of `d` folded in (two taps longer, unless `d = 1`).
pub fn droop_compensated(taps: &[f32], d: usize) -> Vec<f32> {
    let k = droop_correction(d);
    if k[0] == 0.0 {
        return taps.to_vec();
    }
    let mut out = vec![0.0f32; taps.len() + 2];
    for (i, &t) in taps.iter().enumerate() {
        for (j, &c) in k.iter().enumerate() {
            out[i + j] += t * c;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn fast_atan2_is_within_its_bound_all_round() {
        let mut worst = 0.0f32;
        for i in 0..200_000 {
            let t = i as f64 / 200_000.0 * TAU - std::f64::consts::PI;
            for r in [1e-3f64, 0.7, 1e3] {
                let (y, x) = ((r * t.sin()) as f32, (r * t.cos()) as f32);
                let e = (fast_atan2(y, x) - y.atan2(x)).abs();
                let e = e.min((e - 2.0 * PI).abs());
                worst = worst.max(e);
            }
        }
        assert!(worst <= FAST_ATAN2_MAX_ERROR, "worst error {worst}");
        assert_eq!(fast_atan2(0.0, 0.0), 0.0);
        assert_eq!(fast_atan2(0.0, 1.0), 0.0);
        assert!((fast_atan2(1.0, 0.0) - FRAC_PI_2).abs() < 1e-6);
        assert!((fast_atan2(0.0, -1.0) - PI).abs() < 1e-6);
    }

    #[test]
    fn the_discriminator_reads_frequency_and_does_not_care_about_cuts() {
        let rate = 500e3;
        let mut ph = 0.0f64;
        let x: Vec<Complex32> = (0..10_000)
            .map(|i| {
                let f = 20e3 + 30e3 * (TAU * 1000.0 * i as f64 / rate).sin();
                ph += TAU * f / rate;
                Complex32::from_polar(0.3, ph as f32)
            })
            .collect();
        let mut d = Discriminator::new(rate, 75e3).unwrap();
        let mut whole = Vec::new();
        d.process(&x, &mut whole);
        let mean = whole[1..].iter().map(|&v| v as f64).sum::<f64>() / (whole.len() - 1) as f64;
        assert!((mean * 75e3 - 20e3).abs() < 50.0, "{}", mean * 75e3);
        let mut d = Discriminator::new(rate, 75e3).unwrap();
        let mut cut = Vec::new();
        for c in x.chunks(333) {
            d.process(c, &mut cut);
        }
        assert_eq!(cut, whole);
        let mut d = Discriminator::new(rate, 75e3).unwrap();
        let one: Vec<f32> = x.iter().map(|&v| d.process_sample(v)).collect();
        assert_eq!(one, whole);
    }

    #[test]
    fn bad_settings_give_errors_not_panics() {
        assert!(Discriminator::new(0.0, 75e3).is_err());
        assert!(Discriminator::new(500e3, -1.0).is_err());
        assert!(Discriminator::new(f64::NAN, 75e3).is_err());
        assert!(Discriminator::new(500e3, f64::INFINITY).is_err());
        let mut d = Discriminator::new(500e3, 75e3).unwrap();
        // The gain is rate / (2 pi deviation): 1.0 out for a phase step of 2 pi 75 kHz / 500 kHz.
        assert!((d.gain() as f64 - 500e3 / (TAU * 75e3)).abs() < 1e-6);
        assert!(d.try_set_deviation(0.0).is_err());
        d.set_deviation(-5.0);
        assert_eq!(d.deviation(), 75e3);
        d.try_set_deviation(150e3).unwrap();
        assert!((d.gain() as f64 - 500e3 / (TAU * 150e3)).abs() < 1e-6);
    }

    #[test]
    fn the_lag_discriminator_reads_the_mean_frequency_over_its_lag() {
        let rate = 4e6;
        // A tone at 250 kHz: a phase step of pi / 8 per sample, pi / 2 over 4 samples.
        let x: Vec<Complex32> = (0..64)
            .map(|i| Complex32::from_polar(1.0, (i as f64 * TAU * 250e3 / rate) as f32))
            .collect();
        let mut d = LagDiscriminator::new(rate, 1.0, 4).unwrap();
        let mut whole = vec![0.0; x.len()];
        d.process_into(&x, &mut whole).unwrap();
        assert_eq!(whole[..4], [0.0; 4]);
        for &f in &whole[4..] {
            assert!((f - 250e3).abs() < 250e3 * 2e-3, "{f}");
        }
        let mut d = LagDiscriminator::new(rate, 1.0, 4).unwrap();
        let mut cut = vec![0.0; x.len()];
        for (c, o) in x.chunks(5).zip(cut.chunks_mut(5)) {
            d.process_into(c, o).unwrap();
        }
        assert_eq!(cut, whole);
        let mut one = LagDiscriminator::new(rate, 1.0, 1).unwrap();
        let mut plain = Discriminator::new(rate, 1.0).unwrap();
        for &v in &x {
            assert_eq!(one.process_sample(v), plain.process_sample(v));
        }
        assert!(d.process_into(&x, &mut [0.0; 3]).is_err());
        assert!(LagDiscriminator::new(rate, 0.0, 4).is_err());
    }

    #[test]
    fn droop_correction_undoes_the_average_of_a_decimated_channel() {
        // A channel decimated by 4 sees the frequency averaged over 4 input samples; with the correction the MPX is flat within
        // 0.005 dB to 0.08 of the channel rate (38 kHz on a 500 kHz channel), against 0.08 dB without.
        let d = 4;
        assert_eq!(droop_correction(1), [0.0, 1.0, 0.0]);
        let k = droop_correction(d);
        assert!((k[1] + 2.0 * k[0] - 1.0).abs() < 1e-7, "unity at DC");
        let avg = vec![1.0 / d as f32; d];
        for x in [0.02, 0.05, 0.076] {
            let droop = crate::fir::response(&avg, x / d as f64).norm();
            let fixed = droop * crate::fir::response(&k, x).norm();
            assert!((fixed - 1.0).abs() < 6e-4, "{x}: {fixed}");
            assert!((fixed - 1.0).abs() < (droop - 1.0).abs() / 10.0, "{x}");
        }
        let taps = [0.2f32, 0.6, 0.2];
        assert_eq!(droop_compensated(&taps, 1), taps.to_vec());
        assert_eq!(droop_compensated(&taps, 4).len(), 5);
    }
}

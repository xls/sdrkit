//! The second-order loop filter that every tracking loop uses (carrier, timing, pilot, chirp timing), with its gains from a noise
//! bandwidth and a damping factor, and a long-running phase accumulator a loop can steer every sample.
//!
//! **Gains.** A loop with a proportional-plus-integral filter and an accumulator (the NCO or the timing counter) is a type-2 loop. For a
//! noise bandwidth `Bn` normalised to the update rate (`bn_t = Bn T`, one-sided, in cycles per update), a damping factor `zeta` and a
//! total gain `K` of the detector and the accumulator (`K = Kd K0`, 1 when the detector reads the error in the accumulator's own units):
//!
//! ```text
//! theta = bn_t / (zeta + 1 / (4 zeta))
//! d     = 1 + 2 zeta theta + theta^2
//! kp    = 4 zeta theta / (d K)
//! ki    = 4 theta^2 / (d K)
//! ```
//!
//! (M. Rice, *Digital Communications: A Discrete-Time Approach*, appendix C, equations C.56 to C.61; F. M. Gardner, *Phaselock
//! Techniques*.) For small `bn_t` the loop behaves as the analog second-order loop with natural frequency `wn T = 2 bn_t / (zeta + 1 /
//! (4 zeta))`; the tests check the step response against that loop and the noise bandwidth against `bn_t`.
//!
//! ```
//! use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
//! // A carrier loop with a noise bandwidth of 1 % of the symbol rate.
//! let mut lf = LoopFilter::new(0.01, std::f64::consts::FRAC_1_SQRT_2).unwrap();
//! let mut nco = PhaseAcc::new(0.0);
//! let err = 0.1; // a phase error in cycles, from a detector
//! nco.steer(lf.update(err));
//! ```

use crate::{approx, Complex32, DspError, Param};
use std::f64::consts::TAU;

/// The gains `(kp, ki)` of a second-order loop (module docs) for `bn_t`, `zeta` and total gain `k`. Errors unless `0 < bn_t < 0.5`,
/// `zeta > 0` and `k` is non-zero (all finite).
pub fn loop_gains(bn_t: f64, zeta: f64, k: f64) -> Result<(f64, f64), DspError> {
    if !(bn_t > 0.0 && bn_t < 0.5) {
        return Param::LoopBandwidth.range(0.0, 0.5, bn_t);
    }
    if !(zeta > 0.0 && zeta.is_finite()) {
        return Param::Damping.positive(zeta);
    }
    if !k.is_finite() {
        return Param::Gain.not_finite();
    }
    if k == 0.0 {
        return Param::Gain.not_allowed(k);
    }
    let theta = bn_t / (zeta + 1.0 / (4.0 * zeta));
    let d = 1.0 + 2.0 * zeta * theta + theta * theta;
    Ok((4.0 * zeta * theta / (d * k), 4.0 * theta * theta / (d * k)))
}

/// A proportional-plus-integral loop filter: `out = kp e + I`, `I += ki e` (the integrator is updated first, so a step error moves
/// the output by `kp + ki` at once). The integrator holds the loop's estimate of the frequency (or clock rate) and can be clamped,
/// read, set (a Doppler hand-over) and frozen (while no signal is there). All state is `f64`.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopFilter {
    kp: f64,
    ki: f64,
    integ: f64,
    clamp: f64,
    frozen: bool,
}

impl LoopFilter {
    /// A loop of noise bandwidth `bn_t` (per update) and damping `zeta`, for a detector and accumulator of total gain 1.
    pub fn new(bn_t: f64, zeta: f64) -> Result<Self, DspError> {
        Self::with_gain(bn_t, zeta, 1.0)
    }

    /// A loop for a detector and accumulator of total gain `k` (for example a Gardner detector's S-curve slope).
    pub fn with_gain(bn_t: f64, zeta: f64, k: f64) -> Result<Self, DspError> {
        let (kp, ki) = loop_gains(bn_t, zeta, k)?;
        Ok(Self::from_gains(kp, ki))
    }

    /// A loop with these gains directly (for a design given as `kp, ki`).
    pub fn from_gains(kp: f64, ki: f64) -> Self {
        LoopFilter {
            kp,
            ki,
            integ: 0.0,
            clamp: f64::INFINITY,
            frozen: false,
        }
    }

    /// New bandwidth and damping (a gear shift from pull-in to tracking), keeping the integrator.
    pub fn set_bandwidth(&mut self, bn_t: f64, zeta: f64, k: f64) -> Result<(), DspError> {
        let (kp, ki) = loop_gains(bn_t, zeta, k)?;
        self.kp = kp;
        self.ki = ki;
        Ok(())
    }

    /// The gains `(kp, ki)`.
    pub fn gains(&self) -> (f64, f64) {
        (self.kp, self.ki)
    }

    /// Limits the integrator to `+-limit` (the frequency or clock range the loop may pull to). Infinite by default.
    pub fn set_clamp(&mut self, limit: f64) {
        self.clamp = limit.abs();
        self.integ = self.integ.clamp(-self.clamp, self.clamp);
    }

    /// The integrator (the loop's frequency or rate estimate, per update).
    pub fn integrator(&self) -> f64 {
        self.integ
    }

    /// Sets the integrator (within the clamp), for example to a frequency found by a search.
    pub fn set_integrator(&mut self, v: f64) {
        self.integ = v.clamp(-self.clamp, self.clamp);
    }

    /// While frozen, [`update`](Self::update) returns the integrator alone and learns nothing.
    pub fn set_frozen(&mut self, frozen: bool) {
        self.frozen = frozen;
    }

    /// Clears the integrator.
    pub fn reset(&mut self) {
        self.integ = 0.0;
    }

    /// One update with the detector's error; returns the correction for the accumulator. A NaN error is ignored (the output is then
    /// the integrator alone).
    #[inline]
    pub fn update(&mut self, err: f64) -> f64 {
        if self.frozen || err.is_nan() {
            return self.integ;
        }
        self.integ = (self.integ + self.ki * err).clamp(-self.clamp, self.clamp);
        self.kp * err + self.integ
    }
}

/// `p` reduced to `[0, 1)` (a tiny negative value would round up to 1.0; that becomes 0).
#[inline]
fn wrap(p: f64) -> f64 {
    let r = p - p.floor();
    if r < 1.0 {
        r
    } else {
        0.0
    }
}

/// A phase accumulator in `f64` cycles, kept in `[0, 1)` so it stays exact over hours, with a frequency in cycles per sample that a
/// loop may change every sample. The phasor comes from [`approx::sin_cos`] (no library `sin` or `cos`).
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseAcc {
    phase: f64,
    freq: f64,
}

impl PhaseAcc {
    /// An accumulator at phase 0 advancing `freq` cycles per sample.
    pub fn new(freq: f64) -> Self {
        PhaseAcc { phase: 0.0, freq }
    }

    /// The phase in cycles, in `[0, 1)`.
    pub fn phase(&self) -> f64 {
        self.phase
    }

    /// Sets the phase (cycles, any value).
    pub fn set_phase(&mut self, cycles: f64) {
        self.phase = cycles.rem_euclid(1.0);
    }

    /// The frequency in cycles per sample.
    pub fn freq(&self) -> f64 {
        self.freq
    }

    /// Sets the frequency in cycles per sample.
    pub fn set_freq(&mut self, freq: f64) {
        self.freq = freq;
    }

    /// Moves the phase by `delta` cycles (a loop's correction), without changing the frequency.
    #[inline]
    pub fn steer(&mut self, delta: f64) {
        if delta.is_finite() {
            self.phase = wrap(self.phase + delta);
        }
    }

    /// Advances one sample at the current frequency.
    #[inline]
    pub fn advance(&mut self) {
        self.phase = wrap(self.phase + self.freq);
    }

    /// `e^(j 2 pi phase)`, within [`approx::SIN_COS_MAX_ERROR`].
    #[inline]
    pub fn phasor(&self) -> Complex32 {
        let (s, c) = approx::sin_cos(TAU * self.phase);
        Complex32::new(c, s)
    }

    /// `x e^(-j 2 pi phase)`: `x` moved down by the accumulator's phase (a derotator).
    #[inline]
    pub fn derotate(&self, x: Complex32) -> Complex32 {
        x * self.phasor().conj()
    }

    /// The phasor of the current sample, then advances (an oscillator output).
    #[inline]
    pub fn next_phasor(&mut self) -> Complex32 {
        let p = self.phasor();
        self.advance();
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs a phase-locked loop on an input phase sequence `phi[n]` (cycles); returns the error `phi - theta` per update.
    fn run(lf: &mut LoopFilter, phi: impl Fn(usize) -> f64, n: usize) -> Vec<f64> {
        let mut theta = 0.0;
        (0..n)
            .map(|i| {
                let e = phi(i) - theta;
                theta += lf.update(e);
                e
            })
            .collect()
    }

    #[test]
    fn gains_match_the_formula() {
        let (kp, ki) = loop_gains(0.01, 0.707, 1.0).unwrap();
        let th = 0.01 / (0.707 + 1.0 / (4.0 * 0.707));
        let d = 1.0 + 2.0 * 0.707 * th + th * th;
        assert!((kp - 4.0 * 0.707 * th / d).abs() < 1e-15);
        assert!((ki - 4.0 * th * th / d).abs() < 1e-15);
        let (kp2, ki2) = loop_gains(0.01, 0.707, 2.0).unwrap();
        assert!((kp2 * 2.0 - kp).abs() < 1e-15 && (ki2 * 2.0 - ki).abs() < 1e-15);
        assert!(loop_gains(0.0, 0.7, 1.0).is_err());
        assert!(loop_gains(0.01, -1.0, 1.0).is_err());
        assert!(loop_gains(0.01, 0.7, 0.0).is_err());
        assert!(loop_gains(f64::NAN, 0.7, 1.0).is_err());
    }

    #[test]
    fn step_response_is_the_analog_second_order_one() {
        // Phase step of 1: the analog loop's error is e^(-zeta wn t) (cos wd t - zeta / sqrt(1 - zeta^2) sin wd t).
        for zeta in [0.5, std::f64::consts::FRAC_1_SQRT_2, 0.9] {
            let bn_t = 0.002;
            let mut lf = LoopFilter::new(bn_t, zeta).unwrap();
            let e = run(&mut lf, |_| 1.0, 3000);
            let wn = 2.0 * bn_t / (zeta + 1.0 / (4.0 * zeta));
            let wd = wn * (1.0 - zeta * zeta).sqrt();
            let mut worst = 0.0f64;
            for (n, &v) in e.iter().enumerate() {
                let t = n as f64;
                let want = (-zeta * wn * t).exp()
                    * ((wd * t).cos() - zeta / (1.0 - zeta * zeta).sqrt() * (wd * t).sin());
                worst = worst.max((v - want).abs());
            }
            assert!(worst < 0.01, "zeta {zeta}: {worst}");
        }
    }

    #[test]
    fn noise_bandwidth_is_the_one_asked_for() {
        // Closed loop H(z) = G / (1 + G), G = (kp + ki / (1 - z^-1)) z^-1 / (1 - z^-1); Bn T = (1 / 2 pi) int_0^pi |H|^2 dw.
        use num_complex::Complex64;
        for bn_t in [0.001, 0.005, 0.02] {
            let lf = LoopFilter::new(bn_t, 0.707).unwrap();
            let (kp, ki) = lf.gains();
            let n = 200_000;
            let mut acc = 0.0;
            for i in 1..n {
                let w = std::f64::consts::PI * i as f64 / n as f64;
                let z1 = Complex64::from_polar(1.0, -w);
                let one = Complex64::new(1.0, 0.0);
                let g = (kp + ki / (one - z1)) * z1 / (one - z1);
                acc += (g / (one + g)).norm_sqr();
            }
            let got = acc / n as f64 / 2.0;
            assert!((got / bn_t - 1.0).abs() < 0.05, "bn_t {bn_t}: {got}");
        }
    }

    #[test]
    fn a_frequency_ramp_is_tracked_with_no_lasting_error() {
        let mut lf = LoopFilter::new(0.01, 0.707).unwrap();
        let f = 0.003;
        let e = run(&mut lf, |n| f * n as f64, 5000);
        assert!(e[4999].abs() < 1e-9);
        assert!((lf.integrator() - f).abs() < 1e-9);
        // Clamped below the frequency, it cannot follow.
        let mut lf = LoopFilter::new(0.01, 0.707).unwrap();
        lf.set_clamp(0.001);
        let e = run(&mut lf, |n| f * n as f64, 5000);
        // The proportional path carries the rest: a lasting error of (f - 0.001) / kp.
        assert_eq!(lf.integrator(), 0.001);
        assert!(
            (e[4999] - (f - 0.001) / lf.gains().0).abs() < 1e-6,
            "{}",
            e[4999]
        );
        // Frozen: the output is the integrator and the error is ignored.
        lf.set_frozen(true);
        assert_eq!(lf.update(5.0), 0.001);
        assert_eq!(lf.update(f64::NAN), 0.001);
    }

    #[test]
    fn phase_acc_stays_exact_and_returns_its_phasor() {
        let mut p = PhaseAcc::new(0.123_456_789);
        for _ in 0..10_000_000 {
            p.advance();
        }
        let want = (0.123_456_789f64 * 1e7).rem_euclid(1.0);
        assert!((p.phase() - want).abs() < 1e-6, "{} vs {want}", p.phase());
        let mut q = PhaseAcc::new(0.0);
        q.set_phase(0.25);
        let z = q.phasor();
        assert!((z.re).abs() < 1e-6 && (z.im - 1.0).abs() < 1e-6);
        let d = q.derotate(Complex32::new(0.0, 2.0));
        assert!((d.re - 2.0).abs() < 1e-6 && d.im.abs() < 1e-6);
        q.steer(-0.5);
        assert!((q.phase() - 0.75).abs() < 1e-12);
        q.steer(f64::NAN);
        assert!((q.phase() - 0.75).abs() < 1e-12);
        q.set_phase(0.0);
        q.steer(-1e-18);
        assert!(q.phase() >= 0.0 && q.phase() < 1.0);
    }
}

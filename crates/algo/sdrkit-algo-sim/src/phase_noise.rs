//! Oscillator phase noise: a random walk of the phase (the `1/f^2` region of a free-running oscillator, a Lorentzian line) plus an
//! optional white phase floor.
//!
//! # Model
//!
//! The phase is `phi(n) = w(n) + v(n)`, where `w` is a random walk whose steps are Gaussian with variance `sigma_w^2` per sample,
//! and `v` is white Gaussian with variance `sigma_v^2`. At a sample rate `fs`:
//!
//! - the random walk has the single-sideband phase noise `L(f) = sigma_w^2 fs / (4 pi^2 f^2)` (dBc/Hz), so a level `L` at an
//!   offset `f_m` sets `sigma_w^2 = 4 pi^2 f_m^2 10^(L/10) / fs`;
//! - it is also a Lorentzian line of full width at half maximum `linewidth = sigma_w^2 fs / (2 pi)`, and the carrier's
//!   autocorrelation is `E[e^{j(phi(n+k) - phi(n))}] = e^{-sigma_w^2 |k| / 2} = e^{-pi linewidth |k| / fs}`;
//! - the white floor `L_floor` (dBc/Hz) sets `sigma_v^2 = 10^(L_floor/10) fs`.
//!
//! (`L(f)` here is the IEEE 1139 single-sideband ratio, which equals the two-sided phase spectral density for small angles.)

use crate::rng::Rng;
use crate::{Param, SimError};
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_core::Complex32;
use std::f64::consts::{PI, TAU};

/// Phase noise applied to a signal, seeded.
///
/// ```
/// use sdrkit_algo_sim::phase_noise::PhaseNoise;
/// // -80 dBc/Hz at 10 kHz from the carrier, at 1 MHz: a Lorentzian line about 6.3 Hz wide.
/// let p = PhaseNoise::from_dbc_hz(1e6, -80.0, 10e3, None, 1).unwrap();
/// assert!((p.linewidth_hz() - 6.283).abs() < 0.001);
/// ```
#[derive(Clone, Debug)]
pub struct PhaseNoise {
    rng: Rng,
    rate: f64,
    sigma_w: f64,
    sigma_v: f64,
    phi: f64,
}

impl PhaseNoise {
    /// A random walk of standard deviation `sigma_w_rad` per sample and a white floor of `sigma_v_rad`, both in radians.
    pub fn new(
        rate_hz: f64,
        sigma_w_rad: f64,
        sigma_v_rad: f64,
        seed: u64,
    ) -> Result<Self, SimError> {
        if !(rate_hz > 0.0 && rate_hz.is_finite()) {
            return Param::Rate.positive(rate_hz);
        }
        Param::SigmaW.check_in(sigma_w_rad, 0.0, f64::INFINITY)?;
        Param::SigmaV.check_in(sigma_v_rad, 0.0, f64::INFINITY)?;
        Ok(PhaseNoise {
            rng: Rng::new(seed),
            rate: rate_hz,
            sigma_w: sigma_w_rad,
            sigma_v: sigma_v_rad,
            phi: 0.0,
        })
    }

    /// A Lorentzian line `linewidth_hz` wide (full width at half maximum), with no floor.
    pub fn from_linewidth(rate_hz: f64, linewidth_hz: f64, seed: u64) -> Result<Self, SimError> {
        Param::Linewidth.check_in(linewidth_hz, 0.0, f64::INFINITY)?;
        Param::Rate.check_positive(rate_hz)?;
        Self::new(rate_hz, (TAU * linewidth_hz / rate_hz).sqrt(), 0.0, seed)
    }

    /// Phase noise of `dbc_hz` at `offset_hz` from the carrier (on the `1/f^2` slope), with an optional white floor in dBc/Hz.
    pub fn from_dbc_hz(
        rate_hz: f64,
        dbc_hz: f64,
        offset_hz: f64,
        floor_dbc_hz: Option<f64>,
        seed: u64,
    ) -> Result<Self, SimError> {
        Param::Rate.check_positive(rate_hz)?;
        Param::PhaseNoiseOffset.check_positive(offset_hz)?;
        Param::PhaseNoise.check_finite(dbc_hz)?;
        let var_w = 4.0 * PI * PI * offset_hz * offset_hz * 10f64.powf(dbc_hz / 10.0) / rate_hz;
        let var_v = match floor_dbc_hz {
            Some(f) if f.is_finite() => 10f64.powf(f / 10.0) * rate_hz,
            Some(_) => return Param::Floor.not_finite(),
            None => 0.0,
        };
        Self::new(rate_hz, var_w.sqrt(), var_v.sqrt(), seed)
    }

    /// The Lorentzian linewidth of the random walk, Hz.
    pub fn linewidth_hz(&self) -> f64 {
        self.sigma_w * self.sigma_w * self.rate / TAU
    }

    /// The single-sideband level of the random walk at `offset_hz`, dBc/Hz.
    pub fn dbc_hz_at(&self, offset_hz: f64) -> f64 {
        10.0 * (self.sigma_w * self.sigma_w * self.rate / (4.0 * PI * PI * offset_hz * offset_hz))
            .log10()
    }

    /// Turns `buf` by the noisy phase, sample by sample.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for x in buf {
            self.phi += self.sigma_w * self.rng.normal();
            if self.phi.abs() > PI {
                self.phi -= TAU * (self.phi / TAU).round();
            }
            let p = if self.sigma_v > 0.0 {
                self.phi + self.sigma_v * self.rng.normal()
            } else {
                self.phi
            };
            let (s, c) = sin_cos(p);
            *x *= Complex32::new(c, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_carrier_autocorrelation_decays_as_a_lorentzian_line() {
        // A 50 Hz linewidth at 10 kHz: E[z(n+k) z*(n)] = exp(-pi 50 k / 1e4). Averaged over a long run.
        let rate = 1e4;
        let lw = 50.0;
        let mut p = PhaseNoise::from_linewidth(rate, lw, 3).unwrap();
        let mut z = vec![Complex32::new(1.0, 0.0); 2_000_000];
        p.process(&mut z);
        for k in [5usize, 20, 60] {
            let mut acc = 0.0f64;
            for i in 0..z.len() - k {
                acc += (z[i + k] * z[i].conj()).re as f64;
            }
            let got = acc / (z.len() - k) as f64;
            let want = (-PI * lw * k as f64 / rate).exp();
            assert!((got - want).abs() < 0.01, "lag {k}: {got} want {want}");
        }
        assert!(z.iter().all(|v| (v.norm() - 1.0).abs() < 1e-5));
    }

    #[test]
    fn the_level_and_the_linewidth_agree_and_the_floor_has_its_variance() {
        let p = PhaseNoise::from_dbc_hz(1e6, -80.0, 10e3, None, 1).unwrap();
        assert!((p.dbc_hz_at(10e3) + 80.0).abs() < 1e-9);
        assert!(
            (p.dbc_hz_at(20e3) + 86.0206).abs() < 1e-3,
            "20 dB per decade"
        );
        // L(f) = linewidth / (2 pi f^2) for a Lorentzian well outside its width.
        let lw = p.linewidth_hz();
        assert!((10.0 * (lw / (TAU * 1e8)).log10() + 80.0).abs() < 1e-9);
        // A white floor of -60 dBc/Hz at 10 kHz: phase variance 1e-6 * 1e4 = 0.01 rad^2.
        let mut f = PhaseNoise::from_dbc_hz(1e4, -300.0, 1.0, Some(-60.0), 2).unwrap();
        let mut z = vec![Complex32::new(1.0, 0.0); 200_000];
        f.process(&mut z);
        let var = z
            .iter()
            .map(|v| (v.im as f64).atan2(v.re as f64).powi(2))
            .sum::<f64>()
            / z.len() as f64;
        assert!((var / 0.01 - 1.0).abs() < 0.02, "{var}");
        assert!(PhaseNoise::from_dbc_hz(0.0, -80.0, 1.0, None, 0).is_err());
        assert!(PhaseNoise::from_linewidth(1.0, -1.0, 0).is_err());
    }
}

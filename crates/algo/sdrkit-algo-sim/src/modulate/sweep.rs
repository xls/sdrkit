//! Linear frequency sweeps: one chirp, or a sawtooth of chirps that repeats with a pause (FMCW ocean radars, ionosondes, the test
//! sweep of a receiver).

use crate::{Param, SimError};
use sdrkit_core::{Complex32, Generator};
use std::f64::consts::TAU;

/// Samples between exact phase updates; inside a run the phase steps by complex products.
const RUN: u64 = 64;

/// A linear frequency sweep from `from_hz` to `to_hz` (up or down) over `sweep_s`, at unit amplitude, as a [`Generator`].
///
/// Once: it ends after the sweep. Repeating ([`Sweep::repeating`]): a new sweep starts every `period_s`, with silence after each
/// sweep when the period is longer (a sawtooth). The phase is continuous within a sweep. Each sample is the exact quadratic phase
/// at that instant, updated by complex products and set again every 64 samples, so it uses no library trigonometry per sample.
///
/// ```
/// use sdrkit_algo_sim::modulate::Sweep;
/// use sdrkit_core::Generator;
/// // 0.1 s from -1 kHz to +1 kHz at 8 kHz, then 0.05 s of silence, repeating.
/// let mut s = Sweep::new(8000.0, -1000.0, 1000.0, 0.1).unwrap().repeating(0.15).unwrap();
/// assert_eq!(s.freq_at(0.05), Some(0.0));
/// assert_eq!(s.freq_at(0.12), None);
/// let mut out = vec![Default::default(); 2400];
/// assert_eq!(s.generate(&mut out), 2400);
/// ```
#[derive(Clone, Debug)]
pub struct Sweep {
    rate: f64,
    from: f64,
    slope: f64,
    n_sweep: u64,
    n_period: Option<u64>,
    k: u64,
    start_cycles: f64,
    p: Complex32,
    r: Complex32,
    q: Complex32,
    ended: bool,
}

impl Sweep {
    /// One sweep from `from_hz` to `to_hz` over `sweep_s`, for samples at `rate_hz`. A frequency outside half the rate either
    /// side of 0 aliases, as it does in a sampled radio. Errors: a rate that is not positive, a frequency that is not a number,
    /// or a sweep shorter than two samples.
    pub fn new(rate_hz: f64, from_hz: f64, to_hz: f64, sweep_s: f64) -> Result<Self, SimError> {
        if !(rate_hz.is_finite() && rate_hz > 0.0) {
            return Param::Rate.positive(rate_hz);
        }
        Param::FromHz.check_finite(from_hz)?;
        Param::ToHz.check_finite(to_hz)?;
        let n = (sweep_s * rate_hz).round();
        if !(n.is_finite() && n >= 2.0) {
            return Param::Duration.at_least(2.0, n);
        }
        let n_sweep = n as u64;
        let slope = (to_hz - from_hz) / n_sweep as f64;
        Ok(Sweep {
            rate: rate_hz,
            from: from_hz,
            slope,
            n_sweep,
            n_period: None,
            k: 0,
            start_cycles: 0.0,
            p: Complex32::new(1.0, 0.0),
            r: Complex32::new(1.0, 0.0),
            q: cis(slope / rate_hz),
            ended: false,
        })
    }

    /// Repeats the sweep every `period_s` (not shorter than the sweep), silent between the end of a sweep and the next start.
    pub fn repeating(mut self, period_s: f64) -> Result<Self, SimError> {
        let n = (period_s * self.rate).round();
        if !(n.is_finite() && n >= self.n_sweep as f64) {
            return Param::Period.at_least(self.n_sweep as f64, n);
        }
        self.n_period = Some(n as u64);
        Ok(self)
    }

    /// The frequency at `t_s` seconds after the start, or `None` while silent or after a single sweep.
    pub fn freq_at(&self, t_s: f64) -> Option<f64> {
        if !(t_s.is_finite() && t_s >= 0.0) {
            return None;
        }
        let mut k = (t_s * self.rate).floor() as u64;
        match self.n_period {
            Some(p) => k %= p,
            None if k >= self.n_sweep => return None,
            None => {}
        }
        (k < self.n_sweep).then_some(self.from + self.slope * k as f64)
    }

    /// The phase in cycles at sample `k` of a sweep, from the start of that sweep.
    fn cycles(&self, k: u64) -> f64 {
        let k = k as f64;
        (self.from * k + 0.5 * self.slope * k * k) / self.rate
    }
}

/// `exp(j 2 pi x)` for `x` in cycles.
fn cis(x: f64) -> Complex32 {
    let (s, c) = (TAU * x.rem_euclid(1.0)).sin_cos();
    Complex32::new(c as f32, s as f32)
}

impl Generator for Sweep {
    fn rate_hz(&self) -> f64 {
        self.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for (i, o) in out.iter_mut().enumerate() {
            if self.ended {
                return i;
            }
            let k = self.k;
            *o = if k < self.n_sweep {
                if k.is_multiple_of(RUN) {
                    // The exact phase and step at the start of each run.
                    self.p = cis(self.start_cycles + self.cycles(k));
                    self.r = cis((self.from + self.slope * (k as f64 + 0.5)) / self.rate);
                }
                let y = self.p;
                self.p *= self.r;
                self.r *= self.q;
                y
            } else {
                Complex32::default()
            };
            self.k += 1;
            match self.n_period {
                Some(p) if self.k >= p => {
                    self.start_cycles =
                        (self.start_cycles + self.cycles(self.n_sweep)).rem_euclid(1.0);
                    self.k = 0;
                }
                None if self.k >= self.n_sweep => self.ended = true,
                _ => {}
            }
        }
        out.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each sample against the closed form exp(j 2 pi (f0 t + c t^2 / 2)) of a linear chirp, and the instantaneous frequency
    /// from the phase step.
    #[test]
    fn samples_follow_the_closed_form_chirp() {
        let rate = 48_000.0;
        let (f0, f1, ts) = (-6000.0, 9000.0, 0.25);
        let mut s = Sweep::new(rate, f0, f1, ts).unwrap();
        let mut y = vec![Complex32::default(); 20_000];
        assert_eq!(s.generate(&mut y), 12_000);
        let c = (f1 - f0) / ts;
        for (k, v) in y[..12_000].iter().enumerate() {
            let t = k as f64 / rate;
            let want = cis(f0 * t + 0.5 * c * t * t);
            assert!((*v - want).norm() < 1e-3, "sample {k}");
        }
        for k in [100usize, 6000, 11_900] {
            let f = (y[k + 1] * y[k].conj()).arg() as f64 / TAU * rate;
            let want = f0 + c * (k as f64 + 0.5) / rate;
            assert!((f - want).abs() < 1.0, "{k}: {f} vs {want}");
        }
    }

    #[test]
    fn a_sawtooth_repeats_with_its_pause_and_any_cut_returns_the_same_samples() {
        let mk = || {
            Sweep::new(10_000.0, 2000.0, -2000.0, 0.1)
                .unwrap()
                .repeating(0.125)
                .unwrap()
        };
        let mut a = mk();
        let mut ya = vec![Complex32::default(); 5000];
        a.generate(&mut ya);
        let mut b = mk();
        let mut yb = vec![Complex32::default(); 5000];
        for piece in yb.chunks_mut(333) {
            b.generate(piece);
        }
        assert_eq!(ya, yb);
        // Silent from 0.1 s to 0.125 s, sounding again after.
        assert!(ya[1000..1250].iter().all(|v| v.norm() == 0.0));
        assert!((ya[1250].norm() - 1.0).abs() < 1e-5);
        assert_eq!(mk().freq_at(0.13), Some(1800.0));
        assert!(Sweep::new(10_000.0, f64::NAN, 0.0, 0.1).is_err());
        assert!(Sweep::new(10_000.0, 0.0, 10.0, 0.0001).is_err());
        assert!(mk().repeating(0.05).is_err());
    }
}

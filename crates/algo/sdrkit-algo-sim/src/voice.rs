//! Speech-like audio: Gaussian noise limited to the voice band, in syllables with pauses, limited to full scale. Fed to
//! [`crate::modulate::Fm`] or [`crate::modulate::Am`], it gives a voice channel whose spectrum is filled, not a set of
//! lines as a steady tone gives.

use crate::rng::Rng;
use crate::{Param, SimError};
use std::f64::consts::TAU;

/// The lower edge of the voice band, Hz: the corner of the one-pole high-pass.
pub const VOICE_LOW_HZ: f64 = 300.0;
/// The upper edge of the voice band, Hz: the corner of the two one-pole low-passes.
pub const VOICE_HIGH_HZ: f64 = 2700.0;
/// The rate of the syllables, Hz.
pub const SYLLABLE_HZ: f64 = 4.0;

/// Speech-like audio. The noise passes a one-pole high-pass at [`VOICE_LOW_HZ`] and two one-pole low-passes at
/// [`VOICE_HIGH_HZ`], is scaled to an RMS value, multiplied by the positive half of a sine at [`SYLLABLE_HZ`] (pauses as
/// long as the syllables), and limited to `[-1, 1]`. It keeps its state between calls and never allocates.
///
/// ```
/// use sdrkit_algo_sim::rng::Rng;
/// use sdrkit_algo_sim::voice::Voice;
/// let mut v = Voice::new(48_000.0, 0.4, Rng::new(1)).unwrap();
/// let mut a = [0f32; 4800];
/// v.fill(&mut a);
/// assert!(a.iter().all(|x| x.abs() <= 1.0));
/// ```
#[derive(Clone, Debug)]
pub struct Voice {
    rng: Rng,
    high: f64,
    low: f64,
    slow: f64,
    a: f64,
    b: f64,
    gain: f64,
    step: f64,
    phase: f64,
    syllables: bool,
}

impl Voice {
    /// Voice at `rate_hz` with the RMS value `level` before the syllables and the limit, drawing from `rng`. The
    /// syllables start at the start of a syllable. Errors when the rate does not hold the voice band or `level` is not
    /// above 0.
    pub fn new(rate_hz: f64, level: f64, rng: Rng) -> Result<Self, SimError> {
        Param::Rate.check_positive(rate_hz)?;
        if rate_hz <= 2.0 * VOICE_HIGH_HZ {
            return Param::Rate.at_least(2.0 * VOICE_HIGH_HZ, rate_hz);
        }
        Param::Level.check_positive(level)?;
        let pole = |hz: f64| 1.0 - (-TAU * hz / rate_hz).exp();
        let mut v = Voice {
            rng,
            high: pole(VOICE_LOW_HZ),
            low: pole(VOICE_HIGH_HZ),
            slow: 0.0,
            a: 0.0,
            b: 0.0,
            gain: 1.0,
            step: SYLLABLE_HZ / rate_hz,
            phase: 0.0,
            syllables: true,
        };
        v.gain = level / v.unit_rms(rate_hz);
        Ok(v)
    }

    /// The same voice with no pauses: the noise of the voice band at its RMS value, limited.
    pub fn steady(mut self) -> Self {
        self.syllables = false;
        self
    }

    /// The same voice starting at `cycles` of the syllable cycle (0 to 1; 0.5 starts in a pause).
    pub fn with_phase(mut self, cycles: f64) -> Self {
        self.phase = cycles.rem_euclid(1.0);
        self
    }

    /// Fills `out` with the next samples.
    pub fn fill(&mut self, out: &mut [f32]) {
        for o in out.iter_mut() {
            let x = self.rng.normal();
            let y = self.filter(x) * self.gain;
            let env = if self.syllables {
                (TAU * self.phase).sin().max(0.0)
            } else {
                1.0
            };
            self.phase = (self.phase + self.step).rem_euclid(1.0);
            *o = (y * env).clamp(-1.0, 1.0) as f32;
        }
    }

    /// The band filter: one sample in, one out.
    fn filter(&mut self, x: f64) -> f64 {
        self.slow += self.high * (x - self.slow);
        self.a += self.low * (x - self.slow - self.a);
        self.b += self.low * (self.a - self.b);
        self.b
    }

    /// The RMS output of the band filter for white noise of variance 1: the root of the energy of its impulse
    /// response, summed until 40 time constants of its slowest pole. The filter state is left at zero.
    fn unit_rms(&mut self, rate_hz: f64) -> f64 {
        let n = (40.0 * rate_hz / (TAU * VOICE_LOW_HZ)).ceil() as usize;
        let mut energy = 0.0;
        for i in 0..n {
            let y = self.filter(if i == 0 { 1.0 } else { 0.0 });
            energy += y * y;
        }
        (self.slow, self.a, self.b) = (0.0, 0.0, 0.0);
        energy.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }

    #[test]
    fn a_steady_voice_has_its_level() {
        for rate in [48e3, 2e6] {
            let mut v = Voice::new(rate, 0.3, Rng::new(5)).unwrap().steady();
            let mut a = vec![0f32; (rate * 2.0) as usize];
            v.fill(&mut a);
            let r = rms(&a);
            assert!((r - 0.3).abs() < 0.02, "{rate}: {r}");
        }
    }

    #[test]
    fn the_syllables_have_pauses() {
        let rate = 48e3;
        let mut v = Voice::new(rate, 0.4, Rng::new(2)).unwrap();
        // One syllable cycle in 8 parts: the first half speaks, the second is silent.
        let mut a = vec![0f32; (rate / SYLLABLE_HZ) as usize];
        v.fill(&mut a);
        let part = a.len() / 8;
        let parts: Vec<f64> = a.chunks(part).map(rms).collect();
        assert!(parts[1] > 0.2 && parts[2] > 0.2, "{parts:?}");
        assert!(parts[5] == 0.0 && parts[6] == 0.0, "{parts:?}");
    }

    #[test]
    fn the_band_is_the_voice_band() {
        let rate = 48e3;
        let mut v = Voice::new(rate, 0.3, Rng::new(9)).unwrap().steady();
        let mut a = vec![0f32; 1 << 16];
        v.fill(&mut a);
        let power = |hz: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, x) in a.iter().enumerate() {
                let p = TAU * hz * i as f64 / rate;
                re += *x as f64 * p.cos();
                im += *x as f64 * p.sin();
            }
            re * re + im * im
        };
        let band: f64 = [800.0, 1200.0, 1600.0].iter().map(|f| power(*f)).sum();
        let out: f64 = [9000.0, 12000.0, 15000.0].iter().map(|f| power(*f)).sum();
        assert!(band > 20.0 * out, "{band} {out}");
    }

    #[test]
    fn pieces_of_any_size_give_the_same_audio() {
        let mut one = Voice::new(48e3, 0.4, Rng::new(3)).unwrap().with_phase(0.3);
        let mut two = one.clone();
        let mut a = vec![0f32; 10_000];
        one.fill(&mut a);
        let mut b = vec![0f32; 10_000];
        for c in b.chunks_mut(777) {
            two.fill(c);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn bad_settings_are_refused() {
        assert!(Voice::new(5000.0, 0.4, Rng::new(1)).is_err());
        assert!(Voice::new(48e3, 0.0, Rng::new(1)).is_err());
        assert!(Voice::new(f64::NAN, 0.4, Rng::new(1)).is_err());
    }
}

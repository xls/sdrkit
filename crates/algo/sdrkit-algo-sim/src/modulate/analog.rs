//! Analogue modulators from real audio to complex baseband at the same rate: AM (with or without carrier), FM (with optional
//! pre-emphasis) and SSB (upper or lower sideband).

use crate::{Param, SimError};
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_algo_dsp::fir::{hilbert_design, Fir};
use sdrkit_algo_dsp::iir::PreEmphasis;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

fn same_len(a: usize, b: usize) -> Result<(), SimError> {
    if a == b {
        Ok(())
    } else {
        Param::Output.length(a, b)
    }
}

/// Amplitude modulation: `y = 1 + m a(t)` (double sideband with full carrier), or `y = a(t)` (suppressed carrier), at 0 Hz.
///
/// ```
/// use sdrkit_algo_sim::modulate::Am;
/// use sdrkit_core::Complex32;
/// let am = Am::new(0.5).unwrap();
/// let mut y = [Complex32::default(); 2];
/// am.process(&[1.0, -1.0], &mut y).unwrap();
/// assert_eq!(y, [Complex32::new(1.5, 0.0), Complex32::new(0.5, 0.0)]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Am {
    depth: f32,
    carrier: bool,
}

impl Am {
    /// Full-carrier AM of depth `depth` (0 to 1 for no overmodulation; up to 10 allowed).
    pub fn new(depth: f64) -> Result<Self, SimError> {
        if !(0.0..=10.0).contains(&depth) {
            return Param::AmDepth.range(0.0, 10.0, depth);
        }
        Ok(Am {
            depth: depth as f32,
            carrier: true,
        })
    }

    /// Double sideband with suppressed carrier.
    pub fn dsb_sc() -> Self {
        Am {
            depth: 1.0,
            carrier: false,
        }
    }

    /// Modulates `audio` into `out` (the same length).
    pub fn process(&self, audio: &[f32], out: &mut [Complex32]) -> Result<(), SimError> {
        same_len(audio.len(), out.len())?;
        let c = if self.carrier { 1.0 } else { 0.0 };
        for (y, &a) in out.iter_mut().zip(audio) {
            *y = Complex32::new(c + self.depth * a, 0.0);
        }
        Ok(())
    }
}

/// Frequency modulation: the frequency is `deviation_hz a(t)`, the phase its exact running sum in `f64` cycles.
///
/// ```
/// use sdrkit_algo_sim::modulate::Fm;
/// use sdrkit_core::Complex32;
/// let mut fm = Fm::new(48_000.0, 12_000.0).unwrap();
/// let mut y = [Complex32::default(); 5];
/// fm.process(&[1.0; 5], &mut y).unwrap();
/// // 12 kHz at 48 kHz: a quarter turn per sample.
/// assert!((y[1].im - 1.0).abs() < 1e-5);
/// ```
#[derive(Clone, Debug)]
pub struct Fm {
    k: f64,
    phase: f64,
    pre: Option<PreEmphasis>,
}

impl Fm {
    /// FM of `deviation_hz` per unit of audio, for samples at `rate_hz`.
    pub fn new(rate_hz: f64, deviation_hz: f64) -> Result<Self, SimError> {
        Param::Rate.check_positive(rate_hz)?;
        if !(deviation_hz.is_finite() && deviation_hz.abs() < rate_hz / 2.0) {
            return Param::Deviation.range(-rate_hz / 2.0, rate_hz / 2.0, deviation_hz);
        }
        Ok(Fm {
            k: deviation_hz / rate_hz,
            phase: 0.0,
            pre: None,
        })
    }

    /// Adds pre-emphasis of time constant `tau_s` (50 us in Europe, 75 us in the Americas), the exact inverse of
    /// `sdrkit_algo_dsp::iir::Deemphasis::new` at this rate.
    pub fn with_preemphasis(mut self, tau_s: f64, rate_hz: f64) -> Result<Self, SimError> {
        self.pre = Some(PreEmphasis::new(tau_s, rate_hz)?);
        Ok(self)
    }

    /// Modulates `audio` into `out` (the same length).
    pub fn process(&mut self, audio: &[f32], out: &mut [Complex32]) -> Result<(), SimError> {
        same_len(audio.len(), out.len())?;
        for (y, &a) in out.iter_mut().zip(audio) {
            let a = match &mut self.pre {
                Some(p) => p.process_sample(a),
                None => a,
            };
            let (s, c) = sin_cos(TAU * self.phase);
            *y = Complex32::new(c, s);
            self.phase = (self.phase + self.k * a as f64).rem_euclid(1.0);
        }
        Ok(())
    }
}

/// Which sideband.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sideband {
    /// Upper: audio at `+f` appears at `+f`.
    Upper,
    /// Lower: audio at `+f` appears at `-f`.
    Lower,
}

/// Single-sideband modulation by the phasing method: `y = a(t - D) +- j H{a}(t)`, with `H` a Hilbert transformer from
/// `sdrkit_algo_dsp::fir` (flat from `transition_hz` to `rate/2 - transition_hz`, the other sideband suppressed by its design
/// attenuation) and `D` its delay.
#[derive(Clone, Debug)]
pub struct Ssb {
    hilbert: Fir<f32>,
    delay: Vec<f32>,
    pos: usize,
    sign: f32,
}

impl Ssb {
    /// SSB on `side` for audio at `rate_hz`, the Hilbert transformer flat from `transition_hz` (for example 150 Hz for speech) and
    /// suppressing the other sideband by `atten_db`.
    pub fn new(
        side: Sideband,
        rate_hz: f64,
        transition_hz: f64,
        atten_db: f64,
    ) -> Result<Self, SimError> {
        Param::Rate.check_positive(rate_hz)?;
        if !(transition_hz > 0.0 && transition_hz < rate_hz / 4.0) {
            return Param::Transition.range(0.0, rate_hz / 4.0, transition_hz);
        }
        Param::AttenuationDb.check_in(atten_db, 20.0, 120.0)?;
        let taps = hilbert_design(rate_hz, transition_hz, atten_db)?;
        if taps.len() > 1 << 16 {
            return Param::Transition.degenerate();
        }
        let d = (taps.len() - 1) / 2;
        Ok(Ssb {
            hilbert: Fir::new(&taps),
            delay: vec![0.0; d + 1],
            pos: 0,
            sign: match side {
                Sideband::Upper => 1.0,
                Sideband::Lower => -1.0,
            },
        })
    }

    /// The delay from audio to output, samples.
    pub fn delay(&self) -> usize {
        self.delay.len() - 1
    }

    /// Modulates `audio` into `out` (the same length).
    pub fn process(&mut self, audio: &[f32], out: &mut [Complex32]) -> Result<(), SimError> {
        same_len(audio.len(), out.len())?;
        let n = self.delay.len();
        for (y, &a) in out.iter_mut().zip(audio) {
            let q = self.hilbert.process_sample(a);
            self.delay[self.pos] = a;
            self.pos = (self.pos + 1) % n;
            let i = self.delay[self.pos];
            *y = Complex32::new(i, self.sign * q);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bin(x: &[Complex32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let p = -TAU * (f * i as f64).rem_euclid(1.0);
            re += v.re as f64 * p.cos() - v.im as f64 * p.sin();
            im += v.re as f64 * p.sin() + v.im as f64 * p.cos();
        }
        (re * re + im * im).sqrt() / x.len() as f64
    }

    fn audio(n: usize, f: f64, a: f64) -> Vec<f32> {
        (0..n)
            .map(|i| (a * (TAU * f * i as f64).cos()) as f32)
            .collect()
    }

    #[test]
    fn am_envelope_returns_the_audio_back() {
        let a = audio(4800, 1000.0 / 48_000.0, 0.8);
        let mut y = vec![Complex32::default(); a.len()];
        Am::new(0.5).unwrap().process(&a, &mut y).unwrap();
        for (v, s) in y.iter().zip(&a) {
            assert!(((v.norm() - 1.0) / 0.5 - s).abs() < 1e-6);
        }
        Am::dsb_sc().process(&a, &mut y).unwrap();
        assert!(y.iter().zip(&a).all(|(v, s)| v.re == *s && v.im == 0.0));
        assert!(Am::new(0.5).unwrap().process(&a, &mut y[..3]).is_err());
    }

    #[test]
    fn fm_phase_differences_give_the_audio_back() {
        // Independent demodulation: arg(y[n+1] y*[n]) * rate / 2 pi = deviation * a[n].
        let rate = 48_000.0;
        let a = audio(4800, 700.0 / rate, 0.9);
        let mut fm = Fm::new(rate, 5000.0).unwrap();
        let mut y = vec![Complex32::default(); a.len()];
        for (ac, yc) in a.chunks(333).zip(y.chunks_mut(333)) {
            fm.process(ac, yc).unwrap();
        }
        for n in 0..a.len() - 1 {
            let d = y[n + 1] * y[n].conj();
            let f = (d.im as f64).atan2(d.re as f64) * rate / TAU;
            assert!((f - 5000.0 * a[n] as f64).abs() < 0.2, "sample {n}: {f}");
        }
        // Pre-emphasis then the matching de-emphasis is the identity.
        let mut p = Fm::new(rate, 5000.0)
            .unwrap()
            .with_preemphasis(50e-6, rate)
            .unwrap();
        let mut z = vec![Complex32::default(); a.len()];
        p.process(&a, &mut z).unwrap();
        let mut de = sdrkit_algo_dsp::iir::Deemphasis::new(50e-6, rate);
        let mut worst = 0.0f64;
        for n in 0..a.len() - 1 {
            let d = z[n + 1] * z[n].conj();
            let f = (d.im as f64).atan2(d.re as f64) * rate / TAU / 5000.0;
            let back = de.process_sample(f as f32);
            worst = worst.max((back - a[n]).abs() as f64);
        }
        assert!(worst < 1e-3, "{worst}");
        assert!(Fm::new(rate, 30_000.0).is_err());
    }

    #[test]
    fn ssb_puts_the_tone_on_one_side_only() {
        // A 1 kHz tone at 8 kHz: USB puts it at +1 kHz, LSB at -1 kHz, the other side more than 55 dB down.
        let rate = 8000.0;
        let a = audio(16_000, 1000.0 / rate, 0.5);
        for (side, sign) in [(Sideband::Upper, 1.0), (Sideband::Lower, -1.0)] {
            let mut m = Ssb::new(side, rate, 200.0, 60.0).unwrap();
            let mut y = vec![Complex32::default(); a.len()];
            m.process(&a, &mut y).unwrap();
            let y = &y[m.delay() * 2..];
            let want = bin(y, sign * 1000.0 / rate);
            let image = bin(y, -sign * 1000.0 / rate);
            assert!((want - 0.5).abs() < 0.01, "{side:?}: {want}");
            assert!(
                20.0 * (want / image).log10() > 55.0,
                "{side:?}: image {image}"
            );
        }
        assert!(Ssb::new(Sideband::Upper, 8000.0, 0.0, 60.0).is_err());
    }
}

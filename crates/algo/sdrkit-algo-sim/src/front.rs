//! Receiver and transmitter imperfections that act sample by sample: gain, spectral inversion, mains hum, DC offset, IQ imbalance,
//! impulse noise and the ADC quantiser.
//!
//! Full scale: a complex sample whose I and Q both lie in `[-1, 1]` is in range; a mean power of 1 is 0 dBFS.

use crate::rng::Rng;
use crate::{Param, SimError};
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The amplitude gain for `db` decibels of power (`10^(db/20)`).
pub fn gain_from_db(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}

/// Multiplies `buf` by `gain`.
pub fn scale(buf: &mut [Complex32], gain: f32) {
    for x in buf {
        *x *= gain;
    }
}

/// Inverts the spectrum (complex conjugate): a component at `+f` moves to `-f`, as after a tuner that mixes from above.
pub fn invert(buf: &mut [Complex32]) {
    for x in buf {
        x.im = -x.im;
    }
}

/// Adds `other` to `buf`, sample by sample. The shorter length wins.
pub fn add_into(buf: &mut [Complex32], other: &[Complex32]) {
    for (a, b) in buf.iter_mut().zip(other) {
        *a += *b;
    }
}

/// Mains hum: ripple at the mains frequency and its harmonics, either modulating the signal's amplitude (a transmitter or receiver
/// with a poor supply) or added to audio.
///
/// The ripple is `h(t) = sum_k a_k cos(2 pi k f t)` with `a_1 = 1` and the harmonics given (each relative to the fundamental). The
/// phasor of each harmonic is a power of the fundamental's, so there is one `sin_cos` per sample, not one per harmonic.
#[derive(Clone, Debug)]
pub struct Hum {
    rate: f64,
    freq: f64,
    phase: f64,
    harmonics: [f32; 8],
    count: usize,
    depth: f32,
}

impl Hum {
    /// Ripple at `freq_hz` (50 or 60 Hz mains, 100 or 120 Hz after a full-wave rectifier) with amplitude-modulation depth `depth`
    /// (`0..=1`: the signal is multiplied by `1 + depth h(t)`; for [`process_audio`](Self::process_audio), the peak amplitude of
    /// the fundamental added).
    pub fn new(rate_hz: f64, freq_hz: f64, depth: f64) -> Result<Self, SimError> {
        Param::Rate.check_positive(rate_hz)?;
        if !(freq_hz > 0.0 && freq_hz < rate_hz / 2.0) {
            return Param::HumFreq.range(0.0, rate_hz / 2.0, freq_hz);
        }
        Param::HumDepth.check_in(depth, 0.0, 1.0)?;
        let mut harmonics = [0.0; 8];
        harmonics[0] = 1.0;
        Ok(Hum {
            rate: rate_hz,
            freq: freq_hz,
            phase: 0.0,
            harmonics,
            count: 1,
            depth: depth as f32,
        })
    }

    /// Adds harmonics 2, 3, ... with these amplitudes relative to the fundamental (up to 7 of them).
    pub fn with_harmonics(mut self, rel: &[f32]) -> Result<Self, SimError> {
        if rel.len() > 7 {
            return Param::Harmonics.range(0.0, 7.0, rel.len() as f64);
        }
        if rel.iter().any(|a| !a.is_finite()) {
            return Param::Harmonics.not_finite();
        }
        self.harmonics[1..=rel.len()].copy_from_slice(rel);
        self.count = rel.len() + 1;
        Ok(self)
    }

    #[inline]
    fn next(&mut self) -> f32 {
        let (s, c) = sin_cos(TAU * self.phase);
        self.phase = (self.phase + self.freq / self.rate).rem_euclid(1.0);
        let z = Complex32::new(c, s);
        let mut p = z;
        let mut h = 0.0;
        for a in &self.harmonics[..self.count] {
            h += a * p.re;
            p *= z;
        }
        h
    }

    /// Modulates `buf` by `1 + depth h(t)`.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for x in buf {
            let g = 1.0 + self.depth * self.next();
            *x *= g;
        }
    }

    /// Adds `depth h(t)` to real audio.
    pub fn process_audio(&mut self, buf: &mut [f32]) {
        for x in buf {
            *x += self.depth * self.next();
        }
    }
}

/// IQ imbalance of a quadrature mixer whose Q branch has gain `g` and phase error `phi` relative to I: `y = mu x + nu x*`, with
/// `mu = (1 + g e^(-j phi)) / 2` and `nu = (1 - g e^(j phi)) / 2`. A tone at `+f` gets an image at `-f`, weaker by the image
/// rejection ratio `|mu|^2 / |nu|^2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IqImbalance {
    mu: Complex32,
    nu: Complex32,
}

impl IqImbalance {
    /// From the Q branch's gain error in dB and phase error in degrees.
    pub fn new(gain_db: f64, phase_deg: f64) -> Result<Self, SimError> {
        Param::IqGain.check_finite(gain_db)?;
        if !(phase_deg.is_finite() && phase_deg.abs() < 90.0) {
            return Param::IqPhase.range(-90.0, 90.0, phase_deg);
        }
        let g = 10f64.powf(gain_db / 20.0);
        let p = phase_deg.to_radians();
        let mu = Complex32::new(
            ((1.0 + g * p.cos()) / 2.0) as f32,
            (-g * p.sin() / 2.0) as f32,
        );
        let nu = Complex32::new(
            ((1.0 - g * p.cos()) / 2.0) as f32,
            (-g * p.sin() / 2.0) as f32,
        );
        Ok(IqImbalance { mu, nu })
    }

    /// The image rejection ratio, dB (infinite when balanced).
    pub fn image_rejection_db(&self) -> f64 {
        10.0 * (self.mu.norm_sqr() as f64 / self.nu.norm_sqr() as f64).log10()
    }

    /// Applies the imbalance in place.
    pub fn process(&self, buf: &mut [Complex32]) {
        for x in buf {
            *x = self.mu * *x + self.nu * x.conj();
        }
    }
}

/// Adds a constant `dc` to every sample (a receiver's DC offset, a transmitter's carrier leak).
pub fn add_dc(buf: &mut [Complex32], dc: Complex32) {
    for x in buf {
        *x += dc;
    }
}

/// An ADC of `bits` bits clipping at full scale 1: each of I and Q is clipped to `[-1, 1]` and rounded to one of `2^bits` levels
/// spaced `2 / (2^bits - 1)` apart (mid-rise: no level at 0, as in an offset-binary converter such as an RTL-SDR's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantiser {
    step: f32,
    bits: u32,
}

impl Quantiser {
    /// A quantiser of `bits` bits (1 to 24).
    pub fn new(bits: u32) -> Result<Self, SimError> {
        if !(1..=24).contains(&bits) {
            return Param::AdcBits.range(1.0, 24.0, bits as f64);
        }
        Ok(Quantiser {
            step: 2.0 / ((1u32 << bits) - 1) as f32,
            bits,
        })
    }

    /// The bits.
    pub fn bits(&self) -> u32 {
        self.bits
    }

    #[inline]
    fn q(&self, v: f32) -> f32 {
        if v.is_nan() {
            return 0.0;
        }
        let c = v.clamp(-1.0, 1.0);
        (((c + 1.0) / self.step).round() * self.step - 1.0).clamp(-1.0, 1.0)
    }

    /// Clips and rounds `buf` in place; returns how many components were clipped.
    pub fn process(&self, buf: &mut [Complex32]) -> usize {
        let mut clipped = 0;
        for x in buf {
            clipped += (x.re.abs() > 1.0) as usize + (x.im.abs() > 1.0) as usize;
            *x = Complex32::new(self.q(x.re), self.q(x.im));
        }
        clipped
    }
}

/// Impulse noise: bursts of complex Gaussian noise arriving at random (a Poisson process), as from ignition, switching or lightning.
///
/// Arrivals have mean rate `rate_hz`; each burst lasts `width_s` and has mean power `level_dbfs` while it lasts. Inter-arrival
/// times are exponential and counted from the end of the previous burst.
#[derive(Clone, Debug)]
pub struct ImpulseNoise {
    rng: Rng,
    mean_gap: f64,
    width: u64,
    power: f64,
    /// Samples until the next burst starts, and samples left in the current one.
    wait: u64,
    left: u64,
    bursts: u64,
}

impl ImpulseNoise {
    /// Impulses at a mean `rate_hz` per second, each `width_s` long at `level_dbfs`, for samples at `sample_rate_hz`.
    pub fn new(
        sample_rate_hz: f64,
        rate_hz: f64,
        width_s: f64,
        level_dbfs: f64,
        seed: u64,
    ) -> Result<Self, SimError> {
        Param::Rate.check_positive(sample_rate_hz)?;
        Param::ImpulseRate.check_positive(rate_hz)?;
        Param::ImpulseWidth.check_positive(width_s)?;
        Param::Level.check_finite(level_dbfs)?;
        let mut n = ImpulseNoise {
            rng: Rng::new(seed),
            mean_gap: sample_rate_hz / rate_hz,
            width: ((width_s * sample_rate_hz).round() as u64).max(1),
            power: 10f64.powf(level_dbfs / 10.0),
            wait: 0,
            left: 0,
            bursts: 0,
        };
        n.wait = n.gap();
        Ok(n)
    }

    fn gap(&mut self) -> u64 {
        (self.rng.exponential() * self.mean_gap).round().min(1e18) as u64
    }

    /// How many bursts have started.
    pub fn bursts(&self) -> u64 {
        self.bursts
    }

    /// Adds the impulses to `buf`.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        let mut i = 0;
        while i < buf.len() {
            if self.left == 0 {
                let skip = (self.wait.min((buf.len() - i) as u64)) as usize;
                self.wait -= skip as u64;
                i += skip;
                if self.wait == 0 && i < buf.len() {
                    self.left = self.width;
                    self.bursts += 1;
                }
                continue;
            }
            let k = (self.left.min((buf.len() - i) as u64)) as usize;
            for x in &mut buf[i..i + k] {
                *x += self.rng.complex_normal(self.power);
            }
            self.left -= k as u64;
            i += k;
            if self.left == 0 {
                self.wait = self.gap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, f: f64) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = TAU * (f * i as f64).rem_euclid(1.0);
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect()
    }

    fn bin(x: &[Complex32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let p = -TAU * (f * i as f64).rem_euclid(1.0);
            re += v.re as f64 * p.cos() - v.im as f64 * p.sin();
            im += v.re as f64 * p.sin() + v.im as f64 * p.cos();
        }
        (re * re + im * im) / (x.len() as f64).powi(2)
    }

    #[test]
    fn iq_imbalance_makes_the_image_its_formula_predicts() {
        // 1 dB and 3 degrees: image rejection from the closed form 10 log10 |1 + g e^-jp|^2 / |1 - g e^jp|^2, measured on a tone.
        let iq = IqImbalance::new(1.0, 3.0).unwrap();
        let g = 10f64.powf(0.05);
        let p = 3f64.to_radians();
        let num = (1.0 + g * p.cos()).powi(2) + (g * p.sin()).powi(2);
        let den = (1.0 - g * p.cos()).powi(2) + (g * p.sin()).powi(2);
        let want = 10.0 * (num / den).log10();
        assert!((iq.image_rejection_db() - want).abs() < 1e-4);
        let mut x = tone(4000, 0.1);
        iq.process(&mut x);
        let meas = 10.0 * (bin(&x, 0.1) / bin(&x, -0.1)).log10();
        assert!((meas - want).abs() < 0.01, "{meas} want {want}");
        assert!((IqImbalance::new(0.0, 0.0).unwrap().image_rejection_db()).is_infinite());
        assert!(IqImbalance::new(0.0, 95.0).is_err());
    }

    #[test]
    fn a_full_scale_sine_quantised_returns_six_db_per_bit() {
        // SQNR of a full-scale sine through a uniform quantiser: 6.02 b + 1.76 dB (Bennett). Each rail of a complex tone is a
        // sine, so the complex SQNR is the same.
        for bits in [6u32, 8, 10] {
            let q = Quantiser::new(bits).unwrap();
            let x = tone(100_000, 0.012_345_6);
            let mut y = x.clone();
            assert_eq!(q.process(&mut y), 0);
            let e: f64 = x
                .iter()
                .zip(&y)
                .map(|(a, b)| (a - b).norm_sqr() as f64)
                .sum();
            let s: f64 = x.iter().map(|a| a.norm_sqr() as f64).sum();
            let sqnr = 10.0 * (s / e).log10();
            let want = 6.02 * bits as f64 + 1.76;
            assert!(
                (sqnr - want).abs() < 0.3,
                "{bits} bits: {sqnr} dB, want {want}"
            );
        }
        let q = Quantiser::new(3).unwrap();
        let mut c = [Complex32::new(2.0, -3.0), Complex32::new(f32::NAN, 0.01)];
        assert_eq!(q.process(&mut c), 2);
        assert_eq!(c[0], Complex32::new(1.0, -1.0));
        assert!(c[1].re.is_finite());
        let levels: std::collections::BTreeSet<i32> = (0..1000)
            .map(|i| (q.q(i as f32 / 500.0 - 1.0) * 1e5) as i32)
            .collect();
        assert_eq!(levels.len(), 8);
        assert!(Quantiser::new(0).is_err() && Quantiser::new(25).is_err());
    }

    #[test]
    fn hum_modulates_at_its_frequency_and_harmonics() {
        let mut h = Hum::new(8000.0, 50.0, 0.1)
            .unwrap()
            .with_harmonics(&[0.5])
            .unwrap();
        let mut x = vec![Complex32::new(1.0, 0.0); 8000];
        h.process(&mut x);
        let env: Vec<Complex32> = x.iter().map(|v| Complex32::new(v.re - 1.0, 0.0)).collect();
        // A real cosine of amplitude a shows a / 2 in each of the two bins.
        assert!((bin(&env, 50.0 / 8000.0).sqrt() - 0.05).abs() < 1e-4);
        assert!((bin(&env, 100.0 / 8000.0).sqrt() - 0.025).abs() < 1e-4);
        let mut a = vec![0.0f32; 160];
        Hum::new(8000.0, 50.0, 0.2).unwrap().process_audio(&mut a);
        assert!((a[0] - 0.2).abs() < 1e-6 && (a[80] + 0.2).abs() < 1e-5);
        assert!(Hum::new(8000.0, 5000.0, 0.1).is_err());
    }

    #[test]
    fn impulses_arrive_at_their_rate_with_their_power() {
        // 200 impulses a second for 50 s at 1 MHz, 20 us each at 0 dBFS: about 10000 bursts (Poisson, sd 100).
        let rate = 1e6;
        let mut n = ImpulseNoise::new(rate, 200.0, 20e-6, 0.0, 4).unwrap();
        let mut buf = vec![Complex32::default(); 1 << 16];
        let mut energy = 0.0f64;
        let mut total = 0usize;
        while total < 50_000_000 {
            buf.iter_mut().for_each(|v| *v = Complex32::default());
            n.process(&mut buf);
            energy += buf.iter().map(|v| v.norm_sqr() as f64).sum::<f64>();
            total += buf.len();
        }
        let b = n.bursts() as f64;
        let expect = 200.0 * total as f64 / rate / (1.0 + 200.0 * 20e-6);
        assert!(
            (b - expect).abs() < 4.0 * expect.sqrt(),
            "{b} bursts, expected {expect}"
        );
        let per = energy / (b * 20.0);
        assert!(
            (per - 1.0).abs() < 0.03,
            "power per sample in a burst {per}"
        );
        assert!(ImpulseNoise::new(rate, 0.0, 1e-3, 0.0, 0).is_err());
    }

    #[test]
    fn small_helpers() {
        assert!((gain_from_db(-6.0206) - 0.5).abs() < 1e-5);
        let mut x = vec![Complex32::new(1.0, 2.0)];
        invert(&mut x);
        add_dc(&mut x, Complex32::new(0.5, 0.0));
        scale(&mut x, 2.0);
        add_into(&mut x, &[Complex32::new(0.0, 1.0)]);
        assert_eq!(x[0], Complex32::new(3.0, -3.0));
    }
}

//! Deprecated impairments, kept as thin wrappers over `sdrkit-algo-sim` so the tests of modes not yet moved still build. New tests use
//! `sdrkit_algo_sim` directly, or the [`Harness`](crate::Harness), which applies the standard `gen_*` impairments.
//!
//! The random sequence is unchanged: [`Rng`] draws from `sdrkit_algo_sim::rng::Rng` (the same xoshiro256** seeded by SplitMix64) and
//! keeps its Box-Muller normals, so a seeded test sees the same noise as before.

use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// Deprecated: use `sdrkit_algo_sim::rng::Rng`. A repeatable random number generator (xoshiro256** seeded by SplitMix64).
#[derive(Clone, Debug)]
pub struct Rng {
    inner: sdrkit_algo_sim::rng::Rng,
    spare: Option<f64>,
}

impl Rng {
    /// A generator seeded with `seed` (any value, including 0).
    pub fn new(seed: u64) -> Self {
        Rng {
            inner: sdrkit_algo_sim::rng::Rng::new(seed),
            spare: None,
        }
    }
    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }
    /// Uniform in `[0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    /// Uniform integer in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        ((self.next_u64() as u128 * n as u128) >> 64) as u64
    }
    /// A standard normal value (mean 0, variance 1), by the Box-Muller method.
    pub fn gauss(&mut self) -> f64 {
        if let Some(v) = self.spare.take() {
            return v;
        }
        let u1 = 1.0 - self.uniform(); // (0, 1]
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let (s, c) = (TAU * u2).sin_cos();
        self.spare = Some(r * s);
        r * c
    }
}

/// Deprecated: use `sdrkit_algo_sim::noise` (or the harness's `gen_noise`). Additive white Gaussian noise, complex and circular, seeded.
///
/// The noise power that falls in `bandwidth_hz` is `signal_power / 10^(snr_db / 10)`; the total over the sample rate is that times
/// `rate / bandwidth`.
#[derive(Clone, Debug)]
pub struct Awgn {
    rng: Rng,
    sigma: f64,
}

impl Awgn {
    /// Noise with standard deviation `sigma` per component (I and Q), so the total power is `2 sigma^2`.
    pub fn with_sigma(sigma: f64, seed: u64) -> Self {
        Awgn {
            rng: Rng::new(seed),
            sigma,
        }
    }
    /// Noise for a signal of power `signal_power` at `snr_db` measured in `bandwidth_hz`, for samples at `rate_hz`.
    pub fn new(signal_power: f64, snr_db: f64, bandwidth_hz: f64, rate_hz: f64, seed: u64) -> Self {
        assert!(bandwidth_hz > 0.0 && rate_hz > 0.0);
        let noise_in_band = signal_power / 10f64.powf(snr_db / 10.0);
        let total = noise_in_band * rate_hz / bandwidth_hz;
        Self::with_sigma((total / 2.0).sqrt(), seed)
    }
    /// The standard deviation per component.
    pub fn sigma(&self) -> f64 {
        self.sigma
    }
    /// The total noise power (mean `|n|^2`).
    pub fn power(&self) -> f64 {
        2.0 * self.sigma * self.sigma
    }
    /// One noise sample.
    pub fn sample(&mut self) -> Complex32 {
        let i = self.rng.gauss() * self.sigma;
        let q = self.rng.gauss() * self.sigma;
        Complex32::new(i as f32, q as f32)
    }
    /// Adds noise to `buf`.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for x in buf {
            *x += self.sample();
        }
    }
    /// `n` samples of noise alone.
    pub fn noise(&mut self, n: usize) -> Vec<Complex32> {
        (0..n).map(|_| self.sample()).collect()
    }
}

/// Deprecated: use `sdrkit_algo_sim::offset::FrequencyOffset`. Moves a signal up by `offset_hz`, phase continuous across blocks.
#[derive(Clone, Debug)]
pub struct FrequencyOffset {
    inner: sdrkit_algo_sim::offset::FrequencyOffset,
}

impl FrequencyOffset {
    /// An offset of `offset_hz` (positive moves the signal up) for samples at `rate_hz`.
    ///
    /// # Panics
    /// When the rate is not positive or a value is not finite (a test's own mistake).
    pub fn new(rate_hz: f64, offset_hz: f64) -> Self {
        FrequencyOffset {
            inner: sdrkit_algo_sim::offset::FrequencyOffset::new(rate_hz, offset_hz, 0.0)
                .expect("a positive rate and a finite offset"),
        }
    }
    /// Shifts `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        self.inner.process(buf);
    }
}

/// Deprecated: use `sdrkit_algo_sim::clock::ClockError`. A sample-clock error: the signal as a receiver whose clock runs `ppm` fast
/// sees it (about `1 + ppm 1e-6` output samples per input sample).
///
/// Unlike the other wrappers this one keeps its own resampler (`sdrkit_algo_dsp::resample::FractionalResampler`): `sdrkit-algo-sim`'s clock
/// error uses a different interpolator and delay, and the tests of the modes not yet moved (satcom GOES and Inmarsat-C, wlan
/// HaLow) are tuned to this one's exact output. Moving those tests to `sdrkit-algo-sim` is part of moving their family.
#[derive(Clone, Debug)]
pub struct ClockError {
    res: sdrkit_algo_dsp::resample::FractionalResampler<Complex32>,
}

impl ClockError {
    /// A clock error of `ppm` for samples at `rate_hz`. The signal must fit within 0.35 of the rate either side of 0 Hz.
    ///
    /// # Panics
    /// When the rate is not positive (a test's own mistake).
    pub fn new(rate_hz: f64, ppm: f64) -> Self {
        let mut res = sdrkit_algo_dsp::resample::FractionalResampler::new(
            rate_hz,
            rate_hz,
            0.35 * rate_hz,
            0.5 * rate_hz,
            70.0,
        )
        .expect("a unity resampler always fits");
        res.set_correction(1.0 / (1.0 + ppm * 1e-6));
        ClockError { res }
    }
    /// Appends the resampled `input` to `out`.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        self.res.process(input, out);
    }
    /// The delay the resampler adds, in samples.
    pub fn delay(&self) -> f64 {
        self.res.delay()
    }
}

/// Deprecated: use `sdrkit_algo_sim::fading::Profile::Echo` (or `gen_fading` = echo). The signal plus one echo,
/// `y[n] = x[n] + g x[n - delay]`.
#[derive(Clone, Debug)]
pub struct Echo {
    gain: Complex32,
    line: Vec<Complex32>,
    pos: usize,
}

impl Echo {
    /// An echo `delay` samples late, `level_db` relative to the direct signal, in phase with it.
    pub fn new(delay: usize, level_db: f64) -> Self {
        Self::with_phase(delay, level_db, 0.0)
    }
    /// An echo with its carrier phase turned by `phase_rad`.
    ///
    /// # Panics
    /// When `delay` is 0.
    pub fn with_phase(delay: usize, level_db: f64, phase_rad: f64) -> Self {
        assert!(delay >= 1, "an echo needs a delay of at least one sample");
        let g = 10f64.powf(level_db / 20.0);
        let gain = Complex32::new((g * phase_rad.cos()) as f32, (g * phase_rad.sin()) as f32);
        Echo {
            gain,
            line: vec![Complex32::default(); delay],
            pos: 0,
        }
    }
    /// Adds the echo to `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for x in buf {
            let old = self.line[self.pos];
            self.line[self.pos] = *x;
            self.pos = (self.pos + 1) % self.line.len();
            *x += old * self.gain;
        }
    }
}

/// Deprecated: use `sdrkit_algo_sim::front::Quantiser`. An ADC of `bits` bits clipping at `full_scale`.
#[derive(Clone, Copy, Debug)]
pub struct Quantizer {
    inner: sdrkit_algo_sim::front::Quantiser,
    full_scale: f32,
}

impl Quantizer {
    /// A quantizer of `bits` bits (2 to 24) clipping at `full_scale`.
    ///
    /// # Panics
    /// Outside those ranges (a test's own mistake).
    pub fn new(bits: u32, full_scale: f32) -> Self {
        assert!((2..=24).contains(&bits) && full_scale > 0.0);
        Quantizer {
            inner: sdrkit_algo_sim::front::Quantiser::new(bits).expect("2 to 24 bits"),
            full_scale,
        }
    }
    /// Clips and rounds `buf` in place.
    pub fn process(&self, buf: &mut [Complex32]) {
        sdrkit_algo_sim::front::scale(buf, 1.0 / self.full_scale);
        self.inner.process(buf);
        sdrkit_algo_sim::front::scale(buf, self.full_scale);
    }
}

/// Deprecated: use `sdrkit_algo_sim::front::add_into`. Adds `other` to `buf`, sample by sample; the shorter length wins.
pub fn add_into(buf: &mut [Complex32], other: &[Complex32]) {
    sdrkit_algo_sim::front::add_into(buf, other)
}

/// Deprecated: use `sdrkit_algo_sim::front::scale`. Multiplies `buf` by `gain`.
pub fn scale(buf: &mut [Complex32], gain: f32) {
    sdrkit_algo_sim::front::scale(buf, gain)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generator these wrappers replaced, kept here only to show the sequence is unchanged.
    fn old_first_words(seed: u64) -> [u64; 3] {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        let mut s = [next(), next(), next(), next()];
        let mut out = [0; 3];
        for o in &mut out {
            *o = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
            let t = s[1] << 17;
            s[2] ^= s[0];
            s[3] ^= s[1];
            s[1] ^= s[2];
            s[0] ^= s[3];
            s[2] ^= t;
            s[3] = s[3].rotate_left(45);
        }
        out
    }

    #[test]
    fn the_seeded_sequence_is_unchanged() {
        for seed in [0, 1, 7, u64::MAX] {
            let mut r = Rng::new(seed);
            assert_eq!(
                [r.next_u64(), r.next_u64(), r.next_u64()],
                old_first_words(seed),
                "seed {seed}"
            );
        }
    }
}

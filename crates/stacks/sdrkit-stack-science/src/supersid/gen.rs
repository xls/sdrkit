//! The SuperSID test signal: unmodulated carriers at fixed offsets from the tuned frequency over complex white noise of power
//! 1; optionally the first carrier 3 dB stronger from 17 minutes on, as a flare would make it.

use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, SignalInfo};
use std::f64::consts::TAU;

/// The carriers of the test signal: offset from the tuned frequency, Hz, and amplitude.
pub const TEST_CARRIERS: [(f64, f64); 4] = [(2000.0, 0.5), (2800.0, 0.315), (-600.0, 0.2), (1400.0, 0.16)];
/// When the flare of the test signal starts, s.
pub const FLARE_S: f64 = 17.0 * 60.0;
/// The amplitude factor of the flare (+3 dB).
pub const FLARE_GAIN: f64 = 1.41;

/// One carrier of a test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carrier {
    /// Its offset from the centre of the samples, Hz.
    pub offset_hz: f64,
    /// Its amplitude.
    pub amplitude: f64,
    /// From this time (s) on, its amplitude is multiplied by the factor.
    pub step: Option<(f64, f64)>,
}

/// The generator of carriers over white noise.
pub struct SidGenerator {
    rate: f64,
    carriers: Vec<Carrier>,
    noise: Option<Awgn>,
    n: u64,
}

impl SidGenerator {
    /// `carriers` at `rate` over complex white noise of power `noise_power` (0: none).
    pub fn new(rate: f64, carriers: Vec<Carrier>, noise_power: f64, seed: u64) -> Self {
        SidGenerator {
            rate,
            carriers,
            noise: (noise_power > 0.0)
                .then(|| Awgn::with_power(noise_power, seed ^ 0x5349_4400).ok())
                .flatten(),
            n: 0,
        }
    }

    /// The test signal at `rate`: [`TEST_CARRIERS`] over noise of power 1, with the flare when `flare` is set.
    pub fn test_signal(rate: f64, flare: bool, seed: u64) -> Self {
        let carriers = TEST_CARRIERS
            .iter()
            .enumerate()
            .map(|(i, &(offset_hz, amplitude))| Carrier {
                offset_hz,
                amplitude,
                step: (flare && i == 0).then_some((FLARE_S, FLARE_GAIN)),
            })
            .collect();
        SidGenerator::new(rate, carriers, 1.0, seed)
    }

    /// The generator of the settings of `cfg`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] for a rate that does not hold the carriers.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, ConfigError> {
        let need = 2.0 * 2800.0 / super::meter::SPAN_SHARE;
        if cfg.rate_hz < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: cfg.rate_hz,
            });
        }
        Ok(SidGenerator::test_signal(
            cfg.rate_hz,
            cfg.settings.get(&super::GEN_FLARE),
            cfg.seed,
        ))
    }
}

impl Generator for SidGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        // The phase of each carrier from the sample index, so that a long signal keeps its precision.
        let start = self.n;
        out.fill(Complex32::default());
        for c in &self.carriers {
            let cycles = c.offset_hz / self.rate;
            let mut i = start;
            let mut rest = &mut out[..];
            while !rest.is_empty() {
                // Exact phase every 4096 samples, a rotation in between.
                let m = rest.len().min(4096);
                let t = i as f64 / self.rate;
                let a = match c.step {
                    Some((from, gain)) if t >= from => c.amplitude * gain,
                    _ => c.amplitude,
                };
                let a_end = match c.step {
                    Some((from, gain)) if (i + m as u64) as f64 / self.rate > from => {
                        c.amplitude * gain
                    }
                    _ => c.amplitude,
                };
                let ph = TAU * (cycles * i as f64).fract();
                let mut z = Complex32::from_polar(a as f32, ph as f32);
                let rot = Complex32::from_polar(1.0, (TAU * cycles) as f32);
                let switch = if a == a_end {
                    usize::MAX
                } else {
                    c.step
                        .map(|(from, _)| ((from * self.rate).ceil() as u64 - i) as usize)
                        .unwrap_or(usize::MAX)
                };
                for (k, o) in rest[..m].iter_mut().enumerate() {
                    if k == switch {
                        z *= (a_end / a) as f32;
                    }
                    *o += z;
                    z *= rot;
                }
                i += m as u64;
                rest = &mut rest[m..];
            }
        }
        if let Some(nz) = self.noise.as_mut() {
            nz.process(out);
        }
        self.n += out.len() as u64;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let p: f64 = self.carriers.iter().map(|c| c.amplitude.powi(2)).sum();
        Some(SignalInfo::new(120.0).with_power(p.max(1e-12)))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

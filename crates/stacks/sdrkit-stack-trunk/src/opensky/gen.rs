//! The OpenSky test signal: 4-level FSK at 9600 baud with the levels ±1/3 and ±1 of 4800 Hz, smoothed by a one-pole low-pass at
//! 0.8 × the baud, in frames of 4 slots of 7.5 ms with the first 2 on, amplitude 0.4. Other shapes for the checks that the
//! identifier tells them apart: P25-like, EDACS-like and 3600-baud 2-level FSK, all continuous.

use num_complex::Complex32;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

sdrkit_core::choice_enum! {
    /// The shape of the test signal.
    pub enum Shape {
        /// The OpenSky description.
        OpenSky = ("opensky", "OpenSky: 9600 baud, 4 levels, 4800 Hz, 2 of 4 slots"),
        /// 4800 baud, 4 levels, 1944 Hz, continuous.
        P25Like = ("p25", "4800 baud, 4 levels, 1944 Hz, continuous"),
        /// 9600 baud, 2 levels, 2500 Hz, continuous.
        EdacsLike = ("edacs", "9600 baud, 2 levels, 2500 Hz, continuous"),
        /// 3600 baud, 2 levels, 1200 Hz, continuous.
        Baud3600 = ("3600", "3600 baud, 2 levels, 1200 Hz, continuous"),
    }
}

/// The shape of the test signal.
pub const GEN_SHAPE: Setting<field::Choice<Shape>> =
    Setting::choice("gen_shape", "Test signal shape", Shape::OpenSky)
        .section("Test signal")
        .advanced();

/// The test signal settings. The framework adds the standard `gen_*` settings.
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_SHAPE];

/// What a shape sends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeParams {
    /// Symbols a second.
    pub baud: f64,
    /// 2 or 4 levels.
    pub levels: u8,
    /// The outer deviation, Hz.
    pub deviation_hz: f64,
    /// Bursts of 2 of 4 slots of 7.5 ms.
    pub bursts: bool,
}

impl Shape {
    /// What the shape sends.
    pub fn params(self) -> ShapeParams {
        match self {
            Shape::OpenSky => ShapeParams {
                baud: 9600.0,
                levels: 4,
                deviation_hz: 4800.0,
                bursts: true,
            },
            Shape::P25Like => ShapeParams {
                baud: 4800.0,
                levels: 4,
                deviation_hz: 1944.0,
                bursts: false,
            },
            Shape::EdacsLike => ShapeParams {
                baud: 9600.0,
                levels: 2,
                deviation_hz: 2500.0,
                bursts: false,
            },
            Shape::Baud3600 => ShapeParams {
                baud: 3600.0,
                levels: 2,
                deviation_hz: 1200.0,
                bursts: false,
            },
        }
    }
}

/// The generator.
pub struct OpenSkyGenerator {
    rate: f64,
    p: ShapeParams,
    channel_hz: f64,
    rng: Rng,
    level: f64,
    y: f64,
    alpha: f64,
    pos: f64,
    phase: f64,
    n: u64,
}

impl OpenSkyGenerator {
    /// A generator of `shape` at `rate`, the channel `channel_hz` from the centre.
    pub fn new(rate: f64, shape: Shape, channel_hz: f64, seed: u64) -> OpenSkyGenerator {
        let p = shape.params();
        OpenSkyGenerator {
            rate,
            p,
            channel_hz,
            rng: Rng::new(seed),
            level: 0.0,
            y: 0.0,
            alpha: 1.0 - (-TAU * 0.8 * p.baud / rate).exp(),
            pos: 1.0,
            phase: 0.0,
            n: 0,
        }
    }

    /// The test signal of the settings of `cfg`, the channel where a radio tuned `tune_offset_hz` above it sees it.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] if the rate cannot hold the channel.
    pub fn from_config(
        cfg: &GenConfig,
        tune_offset_hz: f64,
    ) -> Result<OpenSkyGenerator, ConfigError> {
        let need = 2.0 * (tune_offset_hz + 12.5e3);
        if cfg.rate_hz < need {
            return Err(ConfigError::RateTooLow {
                need,
                got: cfg.rate_hz,
            });
        }
        Ok(OpenSkyGenerator::new(
            cfg.rate_hz,
            cfg.settings.get(&GEN_SHAPE),
            -tune_offset_hz,
            cfg.seed,
        ))
    }
}

impl Generator for OpenSkyGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let step = self.p.baud / self.rate;
        for z in out.iter_mut() {
            if self.pos >= 1.0 {
                self.pos -= 1.0;
                let k = (self.rng.next_u64() % u64::from(self.p.levels)) as f64;
                self.level = if self.p.levels == 2 {
                    2.0 * k - 1.0
                } else {
                    (2.0 * k - 3.0) / 3.0
                };
            }
            self.pos += step;
            self.y += self.alpha * (self.level * self.p.deviation_hz - self.y);
            self.phase = (self.phase + TAU * (self.channel_hz + self.y) / self.rate) % TAU;
            let t_ms = self.n as f64 / self.rate * 1e3;
            let on = !self.p.bursts || t_ms % 30.0 < 15.0;
            *z = if on {
                Complex32::from_polar(0.4, self.phase as f32)
            } else {
                Complex32::new(0.0, 0.0)
            };
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SNR of the test signal is the carrier over the noise in 25 kHz.
        Some(
            SignalInfo::new(25e3)
                .with_symbol_rate(self.p.baud)
                .with_bit_rate(self.p.baud * f64::from(self.p.levels / 2))
                .with_power(0.16),
        )
    }
}

//! The riometer test signal: complex white noise of amplitude 0.1 per part, in chunks of 20 ms; a share of the chunks (3 % by
//! default) three times stronger (bursts of +9.5 dB); the level 0.5 dB lower in the second half of every 10 minutes. Options
//! for tests: a step of the level at a time and a carrier.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The share of burst chunks of the test signal.
pub const GEN_BURST_SHARE: Setting<field::F64> =
    Setting::new("gen_burst_share", "Test signal burst share", "", 0.03)
        .range(0.0, 0.5, 0.0)
        .help("The share of 20 ms chunks of the test signal that are three times stronger.")
        .section("Test signal")
        .advanced();

/// The amplitude of the noise per part.
pub const AMPLITUDE: f64 = 0.1;
/// The length of a chunk, s.
pub const CHUNK_S: f64 = 0.02;
/// The amplitude factor of a burst chunk.
pub const BURST_FACTOR: f64 = 3.0;

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// The amplitude of the noise per part.
    pub amplitude: f64,
    /// The share of burst chunks.
    pub burst_share: f64,
    /// The level change in the second half of every `period_s`, dB; 0 for none.
    pub half_step_db: f64,
    /// The period of the level change, s.
    pub period_s: f64,
    /// A step of the level from a time on: (time s, dB).
    pub step: Option<(f64, f64)>,
    /// A carrier: (offset Hz, amplitude).
    pub carrier: Option<(f64, f64)>,
}

impl GenParams {
    /// The default signal with a burst share.
    pub fn standard(burst_share: f64) -> GenParams {
        GenParams {
            amplitude: AMPLITUDE,
            burst_share,
            half_step_db: -0.5,
            period_s: 600.0,
            step: None,
            carrier: None,
        }
    }
}

/// The riometer test signal.
pub struct RiometerGenerator {
    rate: f64,
    p: GenParams,
    rng: Rng,
    n: u64,
    chunk: u64,
    factor: f64,
}

impl RiometerGenerator {
    /// A generator at `rate` of the signal `p`.
    pub fn new(rate: f64, p: GenParams, seed: u64) -> Self {
        RiometerGenerator {
            rate,
            p,
            rng: Rng::new(seed ^ 0x5249_4f4d),
            n: 0,
            chunk: u64::MAX,
            factor: 1.0,
        }
    }
}

impl Generator for RiometerGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let b = (t / CHUNK_S).floor() as u64;
            if b != self.chunk {
                self.chunk = b;
                let mut db = 0.0;
                if self.p.half_step_db != 0.0 && t.rem_euclid(self.p.period_s) >= self.p.period_s / 2.0 {
                    db += self.p.half_step_db;
                }
                if let Some((at, step)) = self.p.step {
                    if t >= at {
                        db += step;
                    }
                }
                let burst = self.rng.uniform() < self.p.burst_share;
                self.factor = 10f64.powf(db / 20.0) * if burst { BURST_FACTOR } else { 1.0 };
            }
            let a = self.p.amplitude * self.factor;
            let mut y = Complex32::new((a * self.rng.normal()) as f32, (a * self.rng.normal()) as f32);
            if let Some((f, amp)) = self.p.carrier {
                let ph = (TAU * f * t).rem_euclid(TAU);
                y += Complex32::new((amp * ph.cos()) as f32, (amp * ph.sin()) as f32);
            }
            *z = y;
        }
        out.len()
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate).with_power(2.0 * self.p.amplitude * self.p.amplitude))
    }

    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

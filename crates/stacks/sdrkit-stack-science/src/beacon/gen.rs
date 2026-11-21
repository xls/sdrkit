//! The beacon test signal: complex white noise and a carrier that opens and closes. While open, its frequency walks around
//! the offset (an Ornstein-Uhlenbeck process of standard deviation spread / 2.5 and correlation time 0.05 s) and its level
//! walks in dB (standard deviation `flutter_db`, correlation time 3 s).
//!
//! The default: a carrier at +60 Hz, 30 dB over the noise in a 2 Hz bin, open at 5 s and 40 s of every 70 s for 18 s, a spread
//! of 120 Hz and a flutter of 3 dB.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The level of the carrier of the test signal.
pub const GEN_LEVEL_DB: Setting<field::F64> = Setting::new("gen_level_db", "Test signal level", "dB", 30.0)
    .range(-10.0, 60.0, 0.0)
    .help("The carrier over the noise in a 2 Hz bin.")
    .section("Test signal")
    .advanced();
/// The spread of the carrier of the test signal.
pub const GEN_SPREAD_HZ: Setting<field::F64> =
    Setting::new("gen_spread_hz", "Test signal spread", "Hz", 120.0)
        .range(0.0, 400.0, 0.0)
        .help("The Doppler spread of the carrier while open.")
        .section("Test signal")
        .advanced();
/// The flutter of the carrier of the test signal.
pub const GEN_FLUTTER_DB: Setting<field::F64> =
    Setting::new("gen_flutter_db", "Test signal flutter", "dB", 3.0)
        .range(0.0, 20.0, 0.0)
        .help("The standard deviation of the level of the carrier while open.")
        .section("Test signal")
        .advanced();

/// The noise power.
pub const NOISE_POWER: f64 = 0.0625;
/// The correlation time of the frequency walk, s.
pub const SPREAD_TAU_S: f64 = 0.05;
/// The correlation time of the level walk, s.
pub const FLUTTER_TAU_S: f64 = 3.0;

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// The noise power.
    pub noise_power: f64,
    /// The openings: (start s, length s), in the cycle when the signal repeats.
    pub openings: Vec<(f64, f64)>,
    /// The repeat, s; `None` for once.
    pub cycle_s: Option<f64>,
    /// The carrier at the start, Hz from the centre.
    pub offset_hz: f64,
    /// The drift of the carrier, Hz/s.
    pub drift_hz_s: f64,
    /// The carrier over the noise in 2 Hz, dB.
    pub level_db: f64,
    /// The spread, Hz.
    pub spread_hz: f64,
    /// The flutter, dB.
    pub flutter_db: f64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            noise_power: NOISE_POWER,
            openings: vec![(5.0, 18.0), (40.0, 18.0)],
            cycle_s: Some(70.0),
            offset_hz: 60.0,
            drift_hz_s: 0.0,
            level_db: GEN_LEVEL_DB.default,
            spread_hz: GEN_SPREAD_HZ.default,
            flutter_db: GEN_FLUTTER_DB.default,
        }
    }
}

/// The beacon test signal.
pub struct BeaconGenerator {
    rate: f64,
    p: GenParams,
    rng: Rng,
    n: u64,
    phase: f64,
    walk_hz: f64,
    walk_db: f64,
    amp: f64,
    own_noise: bool,
}

impl BeaconGenerator {
    /// A generator at `rate` of the signal `p`.
    pub fn new(rate: f64, p: GenParams, seed: u64) -> Self {
        let amp = (10f64.powf(p.level_db / 10.0) * p.noise_power * 2.0 / rate).sqrt();
        BeaconGenerator {
            rate,
            p,
            rng: Rng::new(seed ^ 0x4245_4143),
            n: 0,
            phase: 0.0,
            walk_hz: 0.0,
            walk_db: 0.0,
            amp,
            own_noise: true,
        }
    }

    /// Keeps (`true`, the default) or leaves out the generator's own noise floor. The stack leaves it out when the framework
    /// adds noise (see [`crate::family::own_noise`]), so that noise is not added twice and an SNR is referred to the signal alone.
    pub fn with_own_noise(mut self, on: bool) -> Self {
        self.own_noise = on;
        self
    }

    fn open(&self, t: f64) -> bool {
        let tc = match self.p.cycle_s {
            Some(c) if c > 0.0 => t.rem_euclid(c),
            _ => t,
        };
        self.p.openings.iter().any(|&(s, l)| tc >= s && tc < s + l)
    }
}

impl Generator for BeaconGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let sigma = if self.own_noise { (self.p.noise_power / 2.0).sqrt() } else { 0.0 };
        let dt = 1.0 / self.rate;
        let ks = (2.0 * dt / SPREAD_TAU_S).sqrt() * self.p.spread_hz / 2.5;
        let kf = (2.0 * dt / FLUTTER_TAU_S).sqrt() * self.p.flutter_db;
        for z in out.iter_mut() {
            let t = self.n as f64 * dt;
            self.n += 1;
            let mut y = Complex32::new(
                (sigma * self.rng.normal()) as f32,
                (sigma * self.rng.normal()) as f32,
            );
            if self.open(t) {
                self.walk_hz += -self.walk_hz * dt / SPREAD_TAU_S + ks * self.rng.normal();
                self.walk_db += -self.walk_db * dt / FLUTTER_TAU_S + kf * self.rng.normal();
                let f = self.p.offset_hz + self.p.drift_hz_s * t + self.walk_hz;
                self.phase = (self.phase + TAU * f * dt).rem_euclid(TAU);
                let a = self.amp * 10f64.powf(self.walk_db / 20.0);
                y += Complex32::new((a * self.phase.cos()) as f32, (a * self.phase.sin()) as f32);
            }
            *z = y;
        }
        out.len()
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        // The open carrier, with the noise when the generator makes it: the test source puts this power at its nominal level.
        let noise = if self.own_noise { self.p.noise_power } else { 0.0 };
        Some(SignalInfo::new(2.0).with_power(noise + self.amp * self.amp))
    }

    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

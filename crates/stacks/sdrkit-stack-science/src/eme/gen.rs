//! The moon echo test signal: complex white noise; the station's own carrier keyed in pulses, strong over the noise; its echo a
//! set delay later, moved by a Doppler shift and spread by a random frequency walk (an Ornstein-Uhlenbeck process of standard
//! deviation spread / 2.5 and correlation time 0.3 s).
//!
//! The default: the carrier at +30 Hz, 50 dB over the noise in 4 Hz, keyed at 5 s and 12 s of every 14 s for 1.5 s (the detector learns the noise in the first 3.84 s); the echo
//! 2.55 s later, +120 Hz, 4 Hz of spread, 10 dB over the noise in 12 Hz.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The delay of the echo of the test signal.
pub const GEN_DELAY_S: Setting<field::F64> = Setting::new("gen_moon_delay_s", "Test signal echo delay", "s", 2.55)
    .range(2.0, 3.0, 0.0)
    .help("The delay of the echo after the carrier.")
    .section("Test signal")
    .advanced();
/// The Doppler shift of the echo of the test signal.
pub const GEN_DOPPLER_HZ: Setting<field::F64> =
    Setting::new("gen_moon_doppler_hz", "Test signal echo Doppler", "Hz", 120.0)
        .range(-450.0, 450.0, 0.0)
        .help("The shift of the echo from the carrier.")
        .section("Test signal")
        .advanced();
/// The spread of the echo of the test signal.
pub const GEN_SPREAD_HZ: Setting<field::F64> =
    Setting::new("gen_moon_spread_hz", "Test signal echo spread", "Hz", 4.0)
        .range(0.0, 50.0, 0.0)
        .help("The Doppler spread of the echo.")
        .section("Test signal")
        .advanced();
/// The level of the echo of the test signal.
pub const GEN_ECHO_DB: Setting<field::F64> = Setting::new("gen_moon_level_db", "Test signal echo level", "dB", 10.0)
    .range(-60.0, 40.0, 0.0)
    .help("The echo over the noise in 12 Hz.")
    .section("Test signal")
    .advanced();

/// The noise power of the default signal.
pub const NOISE_POWER: f64 = 0.0625;
/// The correlation time of the frequency walk of the echo, s.
pub const WALK_TAU_S: f64 = 0.3;

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// The noise power.
    pub noise_power: f64,
    /// The starts of the pulses, s (in the cycle when the signal repeats).
    pub pulses: Vec<f64>,
    /// The length of a pulse, s.
    pub pulse_s: f64,
    /// The repeat of the pulses, s; `None` for once.
    pub cycle_s: Option<f64>,
    /// The carrier at the start, Hz.
    pub carrier_hz: f64,
    /// The drift of the carrier, Hz/s.
    pub carrier_drift_hz_s: f64,
    /// The carrier over the noise in 4 Hz, dB.
    pub carrier_db: f64,
    /// The delay of the echo, s.
    pub delay_s: f64,
    /// The shift of the echo, Hz.
    pub doppler_hz: f64,
    /// The spread of the echo, Hz.
    pub spread_hz: f64,
    /// The echo over the noise in 12 Hz, dB.
    pub echo_db: f64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            noise_power: NOISE_POWER,
            pulses: vec![5.0, 12.0],
            pulse_s: 1.5,
            cycle_s: Some(14.0),
            carrier_hz: 30.0,
            carrier_drift_hz_s: 0.0,
            carrier_db: 50.0,
            delay_s: GEN_DELAY_S.default,
            doppler_hz: GEN_DOPPLER_HZ.default,
            spread_hz: GEN_SPREAD_HZ.default,
            echo_db: GEN_ECHO_DB.default,
        }
    }
}

/// The moon echo test signal.
pub struct EmeGenerator {
    rate: f64,
    p: GenParams,
    rng: Rng,
    n: u64,
    carrier_phase: f64,
    echo_phase: f64,
    walk_hz: f64,
    carrier_amp: f64,
    echo_amp: f64,
    own_noise: bool,
}

impl EmeGenerator {
    /// A generator at `rate` of the signal `p`.
    pub fn new(rate: f64, p: GenParams, seed: u64) -> Self {
        let carrier_amp = (10f64.powf(p.carrier_db / 10.0) * p.noise_power * 4.0 / rate).sqrt();
        let echo_amp = (10f64.powf(p.echo_db / 10.0) * p.noise_power * 12.0 / rate).sqrt();
        EmeGenerator {
            rate,
            p,
            rng: Rng::new(seed ^ 0x454d_4521),
            n: 0,
            carrier_phase: 0.0,
            echo_phase: 0.0,
            walk_hz: 0.0,
            carrier_amp,
            echo_amp,
            own_noise: true,
        }
    }

    /// Keeps (`true`, the default) or leaves out the generator's own noise floor. The stack leaves it out when the framework
    /// adds noise (see [`crate::family::own_noise`]), so that noise is not added twice and an SNR is referred to the signal alone.
    pub fn with_own_noise(mut self, on: bool) -> Self {
        self.own_noise = on;
        self
    }

    /// Whether the carrier is keyed at `t`.
    fn keyed(&self, t: f64) -> bool {
        let tc = match self.p.cycle_s {
            Some(c) if c > 0.0 => t.rem_euclid(c),
            _ => t,
        };
        self.p.pulses.iter().any(|&s| tc >= s && tc < s + self.p.pulse_s)
    }

    fn carrier_at(&self, t: f64) -> f64 {
        self.p.carrier_hz + self.p.carrier_drift_hz_s * t
    }
}

impl Generator for EmeGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let sigma = if self.own_noise { (self.p.noise_power / 2.0).sqrt() } else { 0.0 };
        let dt = 1.0 / self.rate;
        let walk_sigma = self.p.spread_hz / 2.5;
        let walk_k = (2.0 * dt / WALK_TAU_S).sqrt();
        for z in out.iter_mut() {
            let t = self.n as f64 * dt;
            self.n += 1;
            let mut y = Complex32::new(
                (sigma * self.rng.normal()) as f32,
                (sigma * self.rng.normal()) as f32,
            );
            self.carrier_phase = (self.carrier_phase + TAU * self.carrier_at(t) * dt).rem_euclid(TAU);
            if self.keyed(t) {
                y += Complex32::new(
                    (self.carrier_amp * self.carrier_phase.cos()) as f32,
                    (self.carrier_amp * self.carrier_phase.sin()) as f32,
                );
            }
            self.walk_hz += -self.walk_hz * dt / WALK_TAU_S + walk_sigma * walk_k * self.rng.normal();
            let sent = t - self.p.delay_s;
            let f = self.carrier_at(sent) + self.p.doppler_hz + self.walk_hz;
            self.echo_phase = (self.echo_phase + TAU * f * dt).rem_euclid(TAU);
            if sent >= 0.0 && self.keyed(sent) {
                y += Complex32::new(
                    (self.echo_amp * self.echo_phase.cos()) as f32,
                    (self.echo_amp * self.echo_phase.sin()) as f32,
                );
            }
            *z = y;
        }
        out.len()
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        // The keyed carrier, with the noise when the generator makes it: the test source puts this power at its nominal level.
        let noise = if self.own_noise { self.p.noise_power } else { 0.0 };
        Some(SignalInfo::new(12.0).with_power(noise + self.carrier_amp * self.carrier_amp))
    }

    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

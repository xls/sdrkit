//! The pulse radar test signal: complex noise of power 1 and pulses at the repetition interval, each with a random phase and
//! the gain of a rotating beam, −12 (Δaz / beamwidth)² dB, clamped to the side-lobe level. A sample holds the part of a pulse
//! that falls in its interval, so a pulse shorter than a sample still reaches one sample. The beam points at the receiver a
//! quarter of a rotation after sample 0 and once a rotation after; rotation k has the peak `peaks_db[k mod len]`. The output is
//! that signal times [`GenParams::scale`], stated at a power of 1.
//!
//! Marine: 24 rpm, 2.0° beam, 0.8 µs, 30 dB peak, side lobes −28 dB, 1000 Hz. Weather: 10 rpm, 1.0° beam, 2.0 µs, a volume of
//! four sweeps at 38, 34, 30 and 26 dB, side lobes −40 dB, intervals alternating 1/1200 and 1/900 s.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The rotation of the test signal; 0 for the submode's.
pub const GEN_RPM: Setting<field::F64> = Setting::new("gen_rpm", "Test signal rotation", "rpm", 0.0)
    .range(0.0, 120.0, 0.0)
    .zero_means("the submode's: 24 rpm marine, 10 rpm weather")
    .help("Rotations per minute of the beam of the test signal.")
    .section("Test signal")
    .advanced();
/// The repetition frequency of the test signal; 0 for the submode's.
pub const GEN_PRF_HZ: Setting<field::Hz> = Setting::new("gen_prf_hz", "Test signal PRF", "Hz", 0.0)
    .range(0.0, 10_000.0, 0.0)
    .zero_means("the submode's: 1000 Hz marine, 1200 Hz weather")
    .help("Pulses per second of the test signal; with a staggered train, the higher of the two.")
    .section("Test signal")
    .advanced();

sdrkit_core::choice_enum! {
    /// One repetition frequency or two in turn.
    pub enum DualPrf {
        /// The submode's: one for marine, two for weather.
        Auto = ("auto", "The submode's"),
        /// One repetition frequency.
        Single = ("single", "Single"),
        /// Intervals alternating 1 / PRF and 1 / (0.75 PRF).
        Dual = ("dual", "Dual"),
    }
}

/// A staggered train in the test signal.
pub const GEN_DUAL_PRF: Setting<field::Choice<DualPrf>> =
    Setting::choice("gen_dual_prf", "Test signal staggered PRF", DualPrf::Auto)
        .help("Dual: the intervals alternate 1 / PRF and 1 / (0.75 PRF), as 1200 and 900 Hz.")
        .section("Test signal")
        .advanced();

/// The second repetition frequency of a staggered train, as a share of the first.
pub const SECOND_PRF: f64 = 0.75;
/// The output is scaled so that the strongest pulse peak has this amplitude: the 8-bit model of the `synth` source (full scale
/// 4.0) keeps it. The noise is then 0.1 (marine, about 2.3 steps of 4/128 on each rail) or 0.04 (weather, about 0.9 step: 38 dB
/// of peak leave no more room in 8 bits).
pub const PEAK_AMPLITUDE: f64 = 3.2;
/// Pulses this far below the noise are left out, dB.
const FLOOR_DB: f64 = -30.0;

/// The kind of radar of the test signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A marine navigation radar.
    Marine,
    /// A weather radar.
    Weather,
}

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// Rotations per minute.
    pub rpm: f64,
    /// The beamwidth, degrees.
    pub beam_deg: f64,
    /// The pulse length, s.
    pub pulse_s: f64,
    /// The peak over the noise of each rotation in turn, dB.
    pub peaks_db: Vec<f64>,
    /// The side-lobe level under the peak, dB; `None` for no side lobes.
    pub side_lobe_db: Option<f64>,
    /// The repetition frequency, Hz.
    pub prf_hz: f64,
    /// The intervals alternate 1 / PRF and 1 / ([`SECOND_PRF`] × PRF).
    pub dual: bool,
    /// The output scale.
    pub scale: f64,
}

impl GenParams {
    /// The test signal of `kind` (module docs).
    pub fn of(kind: Kind) -> Self {
        let mut p = match kind {
            Kind::Marine => GenParams {
                rpm: 24.0,
                beam_deg: 2.0,
                pulse_s: 0.8e-6,
                peaks_db: vec![30.0],
                side_lobe_db: Some(-28.0),
                prf_hz: 1000.0,
                dual: false,
                scale: 1.0,
            },
            Kind::Weather => GenParams {
                rpm: 10.0,
                beam_deg: 1.0,
                pulse_s: 2.0e-6,
                peaks_db: vec![38.0, 34.0, 30.0, 26.0],
                side_lobe_db: Some(-40.0),
                prf_hz: 1200.0,
                dual: true,
                scale: 1.0,
            },
        };
        p.scale = p.default_scale();
        p
    }

    /// The scale that puts the strongest peak at [`PEAK_AMPLITUDE`].
    pub fn default_scale(&self) -> f64 {
        let top = self.peaks_db.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        PEAK_AMPLITUDE / 10f64.powf(top.max(0.0) / 20.0)
    }

    /// The rotation period, s.
    pub fn period_s(&self) -> f64 {
        60.0 / self.rpm
    }

    /// The time of the first beam crossing, s.
    pub fn first_crossing_s(&self) -> f64 {
        0.25 * self.period_s()
    }
}

/// The kind of radar of a submode: `weather`, else marine.
pub fn kind_of(submode: Option<&str>) -> Kind {
    if submode == Some("weather") {
        Kind::Weather
    } else {
        Kind::Marine
    }
}

/// A pulse being written: its start and end, s, and its complex amplitude.
#[derive(Clone, Copy, Debug)]
struct Pending {
    t0: f64,
    t1: f64,
    a: Complex32,
}

/// The pulse radar generator.
pub struct RadarGenerator {
    rate: f64,
    p: GenParams,
    noise: Rng,
    phases: Rng,
    n: u64,
    next_pulse_s: f64,
    pulses: u64,
    pending: Vec<Pending>,
    own_noise: bool,
}

impl RadarGenerator {
    /// A generator at `rate` of the signal `p`.
    pub fn new(rate: f64, p: GenParams, seed: u64) -> Self {
        RadarGenerator {
            rate,
            p,
            noise: Rng::stream(seed, 1),
            phases: Rng::stream(seed, 2),
            n: 0,
            next_pulse_s: 0.0,
            pulses: 0,
            pending: Vec::new(),
            own_noise: true,
        }
    }

    /// Keeps (`true`, the default) or leaves out the generator's own noise floor. The stack leaves it out when the framework
    /// adds noise (see [`crate::family::own_noise`]), so that noise is not added twice and an SNR is referred to the signal alone.
    pub fn with_own_noise(mut self, on: bool) -> Self {
        self.own_noise = on;
        self
    }

    /// The generator of `cfg`: the submode's signal with the settings' rotation, repetition and stagger, and its own noise floor
    /// only while the framework adds no noise.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoRate`] for a rate that is not a positive number.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, ConfigError> {
        if !(cfg.rate_hz.is_finite() && cfg.rate_hz > 0.0) {
            return Err(ConfigError::NoRate);
        }
        let mut p = GenParams::of(kind_of(cfg.submode.as_deref()));
        let rpm = cfg.settings.get(&GEN_RPM);
        if rpm > 0.0 {
            p.rpm = rpm;
        }
        let prf = cfg.settings.get(&GEN_PRF_HZ);
        if prf > 0.0 {
            p.prf_hz = prf;
        }
        match cfg.settings.get(&GEN_DUAL_PRF) {
            DualPrf::Auto => {}
            DualPrf::Single => p.dual = false,
            DualPrf::Dual => p.dual = true,
        }
        Ok(RadarGenerator::new(cfg.rate_hz, p, cfg.seed).with_own_noise(crate::family::own_noise(cfg)))
    }

    /// The parameters.
    pub fn params(&self) -> &GenParams {
        &self.p
    }

    /// The amplitude of a pulse at `t` s over the noise: the peak of the rotation of `t` and the beam gain.
    fn amplitude(&self, t: f64) -> f64 {
        let period = self.p.period_s();
        let turns = (t - self.p.first_crossing_s()) / period;
        let k = turns.round();
        let off_deg = (turns - k) * 360.0;
        let mut gain = -12.0 * (off_deg / self.p.beam_deg).powi(2);
        if let Some(s) = self.p.side_lobe_db {
            gain = gain.max(s);
        }
        let n = self.p.peaks_db.len().max(1) as i64;
        let peak = self
            .p
            .peaks_db
            .get((k as i64).rem_euclid(n) as usize)
            .copied()
            .unwrap_or(0.0);
        let db = peak + gain;
        if db < FLOOR_DB {
            0.0
        } else {
            10f64.powf(db / 20.0)
        }
    }

    /// Adds the part of each pending pulse that falls in `out` (samples from `self.n`) and drops the pulses that end in it.
    fn write_pending(&mut self, out: &mut [Complex32]) {
        let dt = 1.0 / self.rate;
        let n0 = self.n;
        let n1 = n0 + out.len() as u64;
        self.pending.retain(|p| {
            let first = ((p.t0 * self.rate).floor().max(0.0) as u64).max(n0);
            let last = ((p.t1 * self.rate).ceil() as u64).min(n1);
            for k in first..last {
                let s0 = k as f64 * dt;
                let share = ((p.t1.min(s0 + dt) - p.t0.max(s0)) / dt).clamp(0.0, 1.0);
                out[(k - n0) as usize] += p.a * share as f32;
            }
            (p.t1 * self.rate).ceil() as u64 > n1
        });
    }
}

impl Generator for RadarGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let power = self.p.scale * self.p.scale;
        let keep = if self.own_noise { 1.0 } else { 0.0 };
        for z in out.iter_mut() {
            *z = self.noise.complex_normal(power) * keep;
        }
        let end_s = (self.n + out.len() as u64) as f64 / self.rate;
        while self.next_pulse_s < end_s {
            let t = self.next_pulse_s;
            let phase = self.phases.uniform() * TAU;
            let a = self.amplitude(t) * self.p.scale;
            if a > 0.0 {
                self.pending.push(Pending {
                    t0: t,
                    t1: t + self.p.pulse_s,
                    a: Complex32::from_polar(a as f32, phase as f32),
                });
            }
            let second = self.p.dual && self.pulses % 2 == 1;
            let prf = if second {
                self.p.prf_hz * SECOND_PRF
            } else {
                self.p.prf_hz
            };
            self.next_pulse_s += 1.0 / prf.max(1e-3);
            self.pulses += 1;
        }
        self.write_pending(out);
        self.n += out.len() as u64;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The nominal level, not the mean power: see PEAK_AMPLITUDE.
        Some(SignalInfo::new(1e6).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

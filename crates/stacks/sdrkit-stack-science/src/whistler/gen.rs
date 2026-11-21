//! The whistler test signal, real (in the in-phase part): white noise of standard deviation 1; sferics, 2 ms of decaying noise,
//! at gaps of 0.5 to 0.9 s away from the placed events; whistlers that fall from 9 kHz to 1.5 kHz after their causative
//! sferic with the phase `-2 pi D^2 / tau` (frequency `D^2 / tau^2`, `tau` the time since the sferic), raised in 30 ms and
//! lowered in 100 ms; tweeks: a sferic, then the first two waveguide modes with `f(dt) = n fc / sqrt(1 - 1 / (1 + dt / 10 ms)^2)`,
//! amplitudes 1 and 0.6, decaying with 50 ms, for 120 ms. The output is that signal times [`OUTPUT_SCALE`].

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::{PI, TAU};

/// The dispersion of the whistlers of the test signal.
pub const GEN_DISPERSION: Setting<field::F64> =
    Setting::new("gen_dispersion", "Test signal dispersion", "s√Hz", 60.0)
        .range(5.0, 300.0, 0.0)
        .help(
            "D of the whistlers of the test signal. 60 is a test value: documents give about 25 to 40 s√Hz for night-time \
             whistlers at low latitude, more by day and at higher latitude.",
        )
        .section("Test signal")
        .advanced();
/// The cut-off of the first waveguide mode of the tweeks of the test signal.
pub const GEN_CUTOFF_HZ: Setting<field::Hz> =
    Setting::new("gen_cutoff_hz", "Test signal tweek cut-off", "Hz", 1800.0)
        .range(1000.0, 3000.0, 0.0)
        .help("Documents give 1.6 to 1.9 kHz for the first mode (about 1.8 kHz for a reflection height of 83 km).")
        .section("Test signal")
        .advanced();
/// The level of the whistlers of the test signal.
pub const GEN_TONE_SNR_DB: Setting<field::F64> =
    Setting::new("gen_tone_snr_db", "Test signal whistler SNR", "dB", 15.0)
        .range(-20.0, 40.0, 0.0)
        .help("The power of a whistler over the power of the noise in 1 kHz.")
        .section("Test signal")
        .advanced();

/// The output is the signal of the module docs times this. The stated power is 1, so a test source that scales the signal to
/// its stated power and quantises it with 12 dB of headroom (an 8-bit radio model) keeps the sferic peaks.
pub const OUTPUT_SCALE: f64 = 1.0 / 16.0;
/// The noise bandwidth that [`GEN_TONE_SNR_DB`] refers to, Hz.
pub const TONE_SNR_BAND_HZ: f64 = 1000.0;
/// The amplitude of a sferic at its start (noise of standard deviation 1).
pub const SFERIC_AMPLITUDE: f64 = 30.0;
/// The length of a sferic, s.
pub const SFERIC_S: f64 = 0.002;
/// The decay time of a sferic, s.
pub const SFERIC_TAU_S: f64 = 0.0005;
/// The gaps between the random sferics, s.
pub const SFERIC_GAP_S: (f64, f64) = (0.5, 0.9);
/// No random sferic this close to an event, s.
pub const GUARD_S: f64 = 0.3;
/// The whistler band, Hz.
pub const WHISTLER_HZ: (f64, f64) = (1500.0, 9000.0);
/// The rise and the fall of a whistler, s.
pub const WHISTLER_RAMPS_S: (f64, f64) = (0.03, 0.1);
/// The tweek: the time constant of the dispersion, the decay and the length, s.
pub const TWEEK_S: (f64, f64, f64) = (0.01, 0.05, 0.12);
/// The amplitudes of the first two modes of a tweek.
pub const TWEEK_AMPLITUDES: [f64; 2] = [1.0, 0.6];
/// The events of one cycle of the default test signal: (time in the cycle, s; whistler).
pub const DEFAULT_CYCLE: [(f64, bool); 5] = [
    (3.0, true),
    (6.0, false),
    (11.0, false),
    (14.0, true),
    (18.0, false),
];
/// The length of the cycle, s.
pub const CYCLE_S: f64 = 20.0;

/// A placed event: a whistler or a tweek whose causative sferic comes at `t` seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenEvent {
    /// The time of the causative sferic, s.
    pub t: f64,
    /// A whistler (`true`) or a tweek.
    pub whistler: bool,
}

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// The dispersion of the whistlers, s√Hz.
    pub dispersion: f64,
    /// The cut-off of the first mode of the tweeks, Hz.
    pub cutoff_hz: f64,
    /// The power of a whistler over the noise in [`TONE_SNR_BAND_HZ`], dB.
    pub tone_snr_db: f64,
    /// The events. With `cycle_s`, they repeat every `cycle_s` seconds.
    pub events: Vec<GenEvent>,
    /// The repeat of the events, s; `None` for once.
    pub cycle_s: Option<f64>,
    /// Random sferics between the events.
    pub sferics: bool,
    /// The noise power multiplies by this (1: standard deviation 1).
    pub noise_power: f64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            dispersion: GEN_DISPERSION.default,
            cutoff_hz: GEN_CUTOFF_HZ.default,
            tone_snr_db: GEN_TONE_SNR_DB.default,
            events: DEFAULT_CYCLE
                .iter()
                .map(|&(t, whistler)| GenEvent { t, whistler })
                .collect(),
            cycle_s: Some(CYCLE_S),
            sferics: true,
            noise_power: 1.0,
        }
    }
}

/// The whistler and tweek generator.
pub struct WhistlerGenerator {
    rate: f64,
    p: GenParams,
    amplitude: f64,
    top_hz: f64,
    rng: Rng,
    n: u64,
    next_sferic: f64,
    /// Start times of the sferics that may still sound.
    sferics: Vec<f64>,
}

impl WhistlerGenerator {
    /// A generator at `rate` of the signal `p`.
    pub fn new(rate: f64, p: GenParams, seed: u64) -> Self {
        let amplitude =
            (2.0 * 10f64.powf(p.tone_snr_db / 10.0) * TONE_SNR_BAND_HZ / (rate / 2.0)).sqrt();
        let mut rng = Rng::new(seed ^ 0x5748_4953);
        let next_sferic = rng.range(SFERIC_GAP_S.0, SFERIC_GAP_S.1);
        WhistlerGenerator {
            rate,
            amplitude,
            top_hz: WHISTLER_HZ.1.min(0.45 * rate),
            p,
            rng,
            n: 0,
            next_sferic,
            sferics: Vec::new(),
        }
    }

    /// The generator of the settings of `cfg`: the default events at the dispersion, cut-off and level of the settings.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below the stack's least rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, ConfigError> {
        if cfg.rate_hz < super::MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: super::MIN_RATE_HZ,
                got: cfg.rate_hz,
            });
        }
        let p = GenParams {
            dispersion: cfg.settings.get(&GEN_DISPERSION),
            cutoff_hz: cfg.settings.get(&GEN_CUTOFF_HZ),
            tone_snr_db: cfg.settings.get(&GEN_TONE_SNR_DB),
            ..GenParams::default()
        };
        Ok(WhistlerGenerator::new(cfg.rate_hz, p, cfg.seed))
    }

    /// The time an event keeps sounding after its sferic, s.
    fn span(&self, whistler: bool) -> f64 {
        if whistler {
            self.p.dispersion / WHISTLER_HZ.0.sqrt()
        } else {
            TWEEK_S.2
        }
    }

    /// Calls `f` with the time of each event whose sferic comes from `from` to `to` (s).
    fn events_between(&self, from: f64, to: f64, mut f: impl FnMut(f64, bool)) {
        match self.p.cycle_s {
            Some(c) if c > 0.0 => {
                let k0 = (from / c).floor().max(0.0) as u64;
                let k1 = (to / c).floor().max(0.0) as u64;
                for k in k0..=k1 {
                    for e in &self.p.events {
                        let t = k as f64 * c + e.t;
                        if t >= from && t <= to {
                            f(t, e.whistler);
                        }
                    }
                }
            }
            _ => {
                for e in &self.p.events {
                    if e.t >= from && e.t <= to {
                        f(e.t, e.whistler);
                    }
                }
            }
        }
    }

    /// Whether a random sferic at `t` falls near an event.
    fn near_event(&self, t: f64) -> bool {
        let reach = self.span(true).max(self.span(false)) + GUARD_S;
        let mut near = false;
        self.events_between(t - reach, t + GUARD_S, |e, w| {
            if t >= e - GUARD_S && t <= e + self.span(w) + GUARD_S {
                near = true;
            }
        });
        near
    }

    /// The whistler of a sferic at `t0`, at time `t`.
    fn whistler(&self, t0: f64, t: f64) -> f64 {
        let d = self.p.dispersion;
        let tau = t - t0;
        let (start, end) = (d / self.top_hz.sqrt(), d / WHISTLER_HZ.0.sqrt());
        if tau < start || tau > end {
            return 0.0;
        }
        let ramp = |x: f64, len: f64| {
            if x >= len {
                1.0
            } else {
                0.5 - 0.5 * (PI * x / len).cos()
            }
        };
        let env = ramp(tau - start, WHISTLER_RAMPS_S.0) * ramp(end - tau, WHISTLER_RAMPS_S.1);
        let phase = (-TAU * d * d / tau).rem_euclid(TAU);
        self.amplitude * env * phase.cos()
    }

    /// The tweek of a sferic at `t0`, at time `t`: modes 1 and 2 below 0.45 of the rate.
    fn tweek(&self, t0: f64, t: f64) -> f64 {
        let dt = t - t0;
        let (tau0, decay, len) = TWEEK_S;
        if dt <= 0.0 || dt > len {
            return 0.0;
        }
        let u = 1.0 + dt / tau0;
        let root = (u * u - 1.0).sqrt();
        let mut y = 0.0;
        for (n, a) in TWEEK_AMPLITUDES.iter().enumerate() {
            let fc = (n + 1) as f64 * self.p.cutoff_hz;
            // f = fc u / sqrt(u^2 - 1); its integral over dt is fc tau0 sqrt(u^2 - 1).
            if fc * u / root >= 0.45 * self.rate {
                continue;
            }
            let phase = (TAU * fc * tau0 * root).rem_euclid(TAU);
            y += a * (-dt / decay).exp() * phase.cos();
        }
        y
    }

    /// The sferics sounding at `t`.
    fn sferic(&mut self, t: f64) -> f64 {
        self.sferics.retain(|&s| t - s < SFERIC_S);
        let mut y = 0.0;
        for &s in &self.sferics {
            if t >= s {
                y += SFERIC_AMPLITUDE * (-(t - s) / SFERIC_TAU_S).exp() * self.rng.normal();
            }
        }
        y
    }
}

impl Generator for WhistlerGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let sigma = self.p.noise_power.max(0.0).sqrt();
        let reach = self.span(true).max(self.span(false));
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let dt = 1.0 / self.rate;
            self.n += 1;
            // The sferics that start in this sample: the random ones and those of the events.
            if self.p.sferics {
                while self.next_sferic <= t {
                    let s = self.next_sferic;
                    self.next_sferic += self.rng.range(SFERIC_GAP_S.0, SFERIC_GAP_S.1);
                    if !self.near_event(s) {
                        self.sferics.push(s);
                    }
                }
            }
            let mut starts = Vec::new();
            self.events_between(t - dt, t, |e, _| starts.push(e));
            for e in starts {
                if e > t - dt {
                    self.sferics.push(e);
                }
            }
            let mut y = sigma * self.rng.normal() + self.sferic(t);
            let mut tones = 0.0;
            self.events_between(t - reach, t, |e, w| {
                tones += if w {
                    self.whistler(e, t)
                } else {
                    self.tweek(e, t)
                };
            });
            y += tones;
            *z = Complex32::new((OUTPUT_SCALE * y) as f32, 0.0);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The nominal level, not the mean power: see OUTPUT_SCALE.
        Some(SignalInfo::new(10e3).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

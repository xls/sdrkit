//! The test signal of the burst monitors: complex white noise with bursts. A burst is a comb of tones about 5 kHz apart with
//! random phases, all within its width; the comb's power density is `level_db` over the noise density, so every spectrogram
//! bin it covers rises by 10 log10(1 + 10^(level/10)) dB. A burst can drift: its centre moves at a constant rate. Options:
//! a gain drift over the run (it scales noise and bursts) and a keyed narrowband interferer.
//!
//! The comb repeats every `p = round(rate / 5000)` samples, so it is one table of `p` samples per burst, moved to the burst
//! centre by a running phase.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The level of the bursts of the test signal.
pub const GEN_LEVEL_DB: Setting<field::F64> =
    Setting::new("gen_level_db", "Test signal burst level", "dB", 6.0)
        .range(-10.0, 30.0, 0.0)
        .help("The power density of a burst over the noise density.")
        .section("Test signal")
        .advanced();
/// The drift of the bursts of the `callisto` test signal.
pub const GEN_DRIFT_HZ_S: Setting<field::F64> =
    Setting::new("gen_burst_drift_hz_s", "Test signal burst drift", "Hz/s", 0.0)
        .range(-1e7, 1e7, 0.0)
        .help("The drift of the bursts; 0 for 0.2 × rate over a quarter of the cycle.")
        .section("Test signal")
        .advanced();

/// The spacing of the tones of a comb, Hz.
pub const TONE_SPACING_HZ: f64 = 5000.0;
/// The noise power of the test signal. The stated power of the signal is this: the test source puts it at its nominal level.
pub const NOISE_POWER: f64 = 0.125;

/// One burst of the test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenBurst {
    /// Start, s (in the cycle when the signal repeats).
    pub start_s: f64,
    /// Length, s.
    pub len_s: f64,
    /// Width, Hz.
    pub width_hz: f64,
    /// Centre at the start, Hz from the centre of the capture.
    pub centre_hz: f64,
    /// Drift of the centre, Hz/s.
    pub drift_hz_s: f64,
}

/// A narrowband interferer: a carrier at `offset_hz` of amplitude `amplitude`, on for `on_s` every `every_s` from 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interferer {
    /// Offset, Hz.
    pub offset_hz: f64,
    /// Amplitude (the noise has the power [`GenParams::noise_power`]).
    pub amplitude: f64,
    /// Time on, s.
    pub on_s: f64,
    /// Period, s.
    pub every_s: f64,
}

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// The comb density over the noise density, dB.
    pub level_db: f64,
    /// The bursts.
    pub bursts: Vec<GenBurst>,
    /// The repeat of the bursts, s; `None` for once.
    pub cycle_s: Option<f64>,
    /// The noise power.
    pub noise_power: f64,
    /// The gain change over `gain_over_s`, dB (linear in time).
    pub gain_drift_db: f64,
    /// The time of the gain change, s.
    pub gain_over_s: f64,
    /// A narrowband interferer.
    pub interferer: Option<Interferer>,
    /// Bursts that would start before this time are left out, s: the detector learns its background from the first rows.
    pub lead_s: f64,
}

impl GenParams {
    /// The length of the default cycle at `rate`: clamp(8e6 / rate, 4, 60) s.
    pub fn cycle_len(rate: f64) -> f64 {
        (8e6 / rate).clamp(4.0, 60.0)
    }

    /// The default `jove` signal: bursts 0.8 × rate wide at the centre, at 0.2 and 0.55 of the cycle, 0.15 of it long; none
    /// in the first 3 s (30 rows of 0.1 s).
    pub fn jove(rate: f64, level_db: f64) -> GenParams {
        let l = Self::cycle_len(rate);
        let b = |s: f64| GenBurst {
            start_s: s * l,
            len_s: 0.15 * l,
            width_hz: 0.8 * rate,
            centre_hz: 0.0,
            drift_hz_s: 0.0,
        };
        GenParams {
            level_db,
            bursts: vec![b(0.2), b(0.55)],
            cycle_s: Some(l),
            lead_s: 3.0,
            ..GenParams::quiet()
        }
    }

    /// The default `callisto` signal: a cycle of clamp(8e6 / rate, 8, 60) s; bursts 0.2 × rate wide starting at -0.25 × rate,
    /// drifting `drift_hz_s` (0 for 0.2 × rate over 0.25 of the cycle), at 0.2 and 0.6 of the cycle, 0.25 of it long; none in
    /// the first 7.5 s (30 rows of 0.25 s).
    pub fn callisto(rate: f64, level_db: f64, drift_hz_s: f64) -> GenParams {
        // At least 8 s: in a 4 s cycle the two bursts lie 0.6 s apart, which 0.25 s rows do not separate.
        let l = Self::cycle_len(rate).max(8.0);
        let drift = if drift_hz_s != 0.0 {
            drift_hz_s
        } else {
            0.2 * rate / (0.25 * l)
        };
        let b = |s: f64| GenBurst {
            start_s: s * l,
            len_s: 0.25 * l,
            width_hz: 0.2 * rate,
            centre_hz: -0.25 * rate,
            drift_hz_s: drift,
        };
        GenParams {
            level_db,
            bursts: vec![b(0.2), b(0.6)],
            cycle_s: Some(l),
            lead_s: 7.5,
            ..GenParams::quiet()
        }
    }

    /// Noise only.
    pub fn quiet() -> GenParams {
        GenParams {
            level_db: GEN_LEVEL_DB.default,
            bursts: Vec::new(),
            cycle_s: None,
            noise_power: NOISE_POWER,
            gain_drift_db: 0.0,
            gain_over_s: 1.0,
            interferer: None,
            lead_s: 0.0,
        }
    }
}

/// The burst test signal.
pub struct BurstGenerator {
    rate: f64,
    p: GenParams,
    tables: Vec<Vec<Complex32>>,
    rng: Rng,
    n: u64,
    own_noise: bool,
}

impl BurstGenerator {
    /// A generator at `rate` of the signal `p`.
    pub fn new(rate: f64, p: GenParams, seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0x4255_5253);
        let period = ((rate / TONE_SPACING_HZ).round() as usize).max(1);
        let spacing = rate / period as f64;
        let density = 10f64.powf(p.level_db / 10.0) * p.noise_power / rate;
        let amp = (density * spacing).sqrt();
        let tables = p
            .bursts
            .iter()
            .map(|b| {
                let half = (b.width_hz / 2.0 / spacing).floor() as i64;
                let tones: Vec<(f64, f64)> = (-half..=half)
                    .map(|k| (k as f64, rng.range(0.0, TAU)))
                    .collect();
                (0..period)
                    .map(|i| {
                        let mut z = Complex32::default();
                        for &(k, ph) in &tones {
                            let a = TAU * k * i as f64 / period as f64 + ph;
                            z += Complex32::new((amp * a.cos()) as f32, (amp * a.sin()) as f32);
                        }
                        z
                    })
                    .collect()
            })
            .collect();
        BurstGenerator {
            rate,
            p,
            tables,
            rng,
            n: 0,
            own_noise: true,
        }
    }

    /// Keeps (`true`, the default) or leaves out the generator's own noise floor. The stack leaves it out when the framework
    /// adds noise (see [`crate::family::own_noise`]), so that noise is not added twice and an SNR is referred to the signal alone.
    pub fn with_own_noise(mut self, on: bool) -> Self {
        self.own_noise = on;
        self
    }
}

impl Generator for BurstGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let sigma = if self.own_noise { (self.p.noise_power / 2.0).sqrt() } else { 0.0 };
        for z in out.iter_mut() {
            let i = self.n;
            self.n += 1;
            let t = i as f64 / self.rate;
            let mut y = Complex32::new(
                (sigma * self.rng.normal()) as f32,
                (sigma * self.rng.normal()) as f32,
            );
            let tc = match self.p.cycle_s {
                Some(c) if c > 0.0 => t.rem_euclid(c),
                _ => t,
            };
            for (b, table) in self.p.bursts.iter().zip(&self.tables) {
                let u = tc - b.start_s;
                if u < 0.0 || u >= b.len_s || t - u < self.p.lead_s {
                    continue;
                }
                let phase = (TAU * (b.centre_hz * u + 0.5 * b.drift_hz_s * u * u)).rem_euclid(TAU);
                let k = (u * self.rate).round() as usize % table.len();
                y += table[k] * Complex32::new(phase.cos() as f32, phase.sin() as f32);
            }
            if let Some(f) = self.p.interferer {
                if t.rem_euclid(f.every_s) < f.on_s {
                    let ph = (TAU * f.offset_hz * t).rem_euclid(TAU);
                    y += Complex32::new(
                        (f.amplitude * ph.cos()) as f32,
                        (f.amplitude * ph.sin()) as f32,
                    );
                }
            }
            if self.p.gain_drift_db != 0.0 {
                let g = 10f64.powf(self.p.gain_drift_db * (t / self.p.gain_over_s) / 20.0);
                y *= g as f32;
            }
            *z = y;
        }
        out.len()
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        let width = self
            .p
            .bursts
            .iter()
            .map(|b| b.width_hz)
            .fold(0.0, f64::max)
            .max(self.rate / 100.0);
        // The noise power: the level the bursts are given against, whether the generator makes the noise or not.
        Some(SignalInfo::new(width).with_power(self.p.noise_power))
    }

    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

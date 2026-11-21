//! The Schumann test signal, real (in the in-phase part). At the working rate of about 200 sps: white noise of standard deviation
//! 1, and the five resonances at 7.83, 14.1, 20.3, 26.3 and 32.5 Hz scaled by the fundamental, each 21 steady tones over 30 % of
//! its frequency with a Gaussian weight and Rayleigh amplitudes, whose spectrum peaks at 10, 7, 5, 4 and 3 times the noise
//! density; an optional 12 Hz transient every 60 s from 70 s. Each working sample is held over a group of input samples, so the
//! receiver's front end returns it. 50 Hz mains of amplitude 0.5 is added at the input rate. The output is that signal times
//! [`OUTPUT_SCALE`].

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The fundamental of the test signal.
pub const GEN_FUNDAMENTAL_HZ: Setting<field::Hz> =
    Setting::new("gen_fundamental_hz", "Test signal fundamental", "Hz", 7.83)
        .range(5.0, 12.0, 0.0)
        .help("The first resonance of the test signal; the others follow in proportion.")
        .section("Test signal")
        .advanced();
/// The level of the transients of the test signal.
pub const GEN_TRANSIENTS: Setting<field::F64> =
    Setting::new("gen_transients", "Test signal transients", "", 0.0)
        .range(0.0, 10.0, 0.0)
        .zero_means("none")
        .help(
            "A 12 Hz transient every 60 s from 70 s, Gaussian envelope of 0.25 s: its amplitude is 6 × this level × the rms \
             of the background. 0 for none.",
        )
        .section("Test signal")
        .advanced();

/// The output is the signal of the module docs times this. The stated power is 1, so a test source that scales the signal to
/// its stated power and quantises it with 12 dB of headroom (an 8-bit radio model) keeps the transients of level 2.
pub const OUTPUT_SCALE: f64 = 0.25;
/// The published resonance frequencies, Hz.
pub const RESONANCES_HZ: [f64; 5] = [7.83, 14.1, 20.3, 26.3, 32.5];
/// The peak of each resonance over the noise density.
pub const PEAKS: [f64; 5] = [10.0, 7.0, 5.0, 4.0, 3.0];
/// Tones of a resonance.
const TONES: usize = 21;
/// The tones span this part of the frequency of a resonance (1.5 × 20 %).
const SPAN: f64 = 0.3;
/// The width at half height of the Gaussian weight, a part of the frequency.
const WIDTH: f64 = 0.2;
/// The transient: frequency (Hz), envelope sigma (s), amplitude per level, first (s), repeat (s).
pub const TRANSIENT: (f64, f64, f64, f64, f64) = (12.0, 0.25, 6.0, 70.0, 60.0);
/// The mains hum: frequency (Hz) and amplitude.
pub const MAINS: (f64, f64) = (50.0, 0.5);

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GenParams {
    /// The fundamental at the start, Hz.
    pub fundamental_hz: f64,
    /// Its change per second, Hz/s.
    pub drift_hz_per_s: f64,
    /// The background over its standard level (standard deviation 1), dB.
    pub background_db: f64,
    /// The transient level; 0 for none.
    pub transients: f64,
    /// The amplitude of the mains hum.
    pub mains: f64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            fundamental_hz: GEN_FUNDAMENTAL_HZ.default,
            drift_hz_per_s: 0.0,
            background_db: 0.0,
            transients: GEN_TRANSIENTS.default,
            mains: MAINS.1,
        }
    }
}

/// One tone of a resonance: its frequency per hertz of fundamental, its amplitude and its phase.
#[derive(Clone, Debug)]
struct Tone {
    per_hz: f64,
    amplitude: f64,
    phase: f64,
}

/// The Schumann generator.
pub struct SchumannGenerator {
    rate: f64,
    dec: usize,
    fs: f64,
    p: GenParams,
    tones: Vec<Tone>,
    sigma: f64,
    rng: Rng,
    n: u64,
    k: u64,
    held: f64,
    mains_phase: f64,
}

impl SchumannGenerator {
    /// A generator at `rate` of the signal `p`.
    pub fn new(rate: f64, p: GenParams, seed: u64) -> Self {
        let dec = super::decimation(rate);
        let fs = rate / dec as f64;
        let mut rng = Rng::new(seed ^ 0x5343_484d);
        // White noise of variance 1 has the one-sided density 2 / fs. A tone of mean power s^2 every `step` Hz gives the
        // density s^2 / step.
        let density = 2.0 / fs;
        let sigma_g = WIDTH / (2.0 * (2.0 * 2f64.ln()).sqrt());
        let mut tones = Vec::with_capacity(RESONANCES_HZ.len() * TONES);
        for (&f, &peak) in RESONANCES_HZ.iter().zip(&PEAKS) {
            let step = SPAN * f / (TONES - 1) as f64;
            for j in 0..TONES {
                let x = (j as f64 / (TONES - 1) as f64 - 0.5) * SPAN;
                let w = (-0.5 * (x / sigma_g).powi(2)).exp();
                let s = (peak * density * step * w).sqrt();
                let a = s * (-2.0 * rng.uniform_open().ln()).sqrt();
                tones.push(Tone {
                    per_hz: f * (1.0 + x) / RESONANCES_HZ[0],
                    amplitude: a,
                    phase: rng.range(0.0, TAU),
                });
            }
        }
        SchumannGenerator {
            rate,
            dec,
            fs,
            sigma: 10f64.powf(p.background_db / 20.0),
            p,
            tones,
            rng,
            n: 0,
            k: 0,
            held: 0.0,
            mains_phase: 0.0,
        }
    }

    /// The generator of the settings of `cfg`.
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
            fundamental_hz: cfg.settings.get(&GEN_FUNDAMENTAL_HZ),
            transients: cfg.settings.get(&GEN_TRANSIENTS),
            ..GenParams::default()
        };
        Ok(SchumannGenerator::new(cfg.rate_hz, p, cfg.seed))
    }

    /// The next working sample.
    fn working_sample(&mut self) -> f64 {
        let t = self.k as f64 / self.fs;
        self.k += 1;
        let f1 = self.p.fundamental_hz + self.p.drift_hz_per_s * t;
        let mut y = self.sigma * self.rng.normal();
        for tone in &mut self.tones {
            y += tone.amplitude * tone.phase.cos();
            tone.phase = (tone.phase + TAU * tone.per_hz * f1 / self.fs).rem_euclid(TAU);
        }
        let (f, sigma, per_level, first, every) = TRANSIENT;
        if self.p.transients > 0.0 && t > first - 4.0 * sigma {
            let k = ((t - first) / every).round().max(0.0);
            let dt = t - (first + k * every);
            if dt.abs() < 4.0 * sigma {
                let env = (-0.5 * (dt / sigma).powi(2)).exp();
                y += per_level * self.p.transients * self.sigma * env * (TAU * f * dt).cos();
            }
        }
        y
    }
}

impl Generator for SchumannGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let w = TAU * MAINS.0 / self.rate;
        for z in out.iter_mut() {
            if self.n.is_multiple_of(self.dec as u64) {
                self.held = self.working_sample();
            }
            self.n += 1;
            let y = self.held + self.p.mains * self.mains_phase.cos();
            self.mains_phase = (self.mains_phase + w).rem_euclid(TAU);
            *z = Complex32::new((OUTPUT_SCALE * y) as f32, 0.0);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The nominal level, not the mean power: see OUTPUT_SCALE.
        Some(SignalInfo::new(100.0).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

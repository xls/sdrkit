//! The noise floor test signal: band-limited noise (white noise through a one-pole low-pass, moved to an offset) for a
//! stretch of time, and a steady carrier. The band-limited noise is the source under test; the white background noise
//! comes from the standard impairments (`gen_noise` at a fixed floor).

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The centre of the band-limited noise from the centre.
pub const GEN_EXTRA_HZ: Setting<field::F64> =
    Setting::new("gen_extra_hz", "Added noise centre", "Hz", 300e3)
        .range(-1e8, 1e8, 0.0)
        .section(SECTION);
/// The width of the band-limited noise: twice the cut-off of the low-pass.
pub const GEN_EXTRA_BW_HZ: Setting<field::F64> =
    Setting::new("gen_extra_bw_hz", "Added noise width", "Hz", 250e3)
        .range(1.0, 1e8, 0.0)
        .section(SECTION)
        .help("Twice the cut-off of the one-pole low-pass that shapes the added noise.");
/// The power of the band-limited noise. The default 0.002 is about 10 dB over a background of 0.001 per sample at the
/// centre of the added band at 2 Msps.
pub const GEN_EXTRA_POWER: Setting<field::F64> =
    Setting::new("gen_extra_power", "Added noise power", "", 0.002)
        .range(0.0, 100.0, 0.0)
        .section(SECTION);
/// The start of the band-limited noise.
pub const GEN_EXTRA_FROM_S: Setting<field::F64> =
    Setting::new("gen_extra_from_s", "Added noise from", "s", 90.0)
        .range(0.0, 1e6, 0.0)
        .section(SECTION);
/// The end of the band-limited noise.
pub const GEN_EXTRA_TO_S: Setting<field::F64> =
    Setting::new("gen_extra_to_s", "Added noise until", "s", 150.0)
        .range(0.0, 1e6, 0.0)
        .section(SECTION);
/// The offset of the carrier.
pub const GEN_TONE_HZ: Setting<field::F64> =
    Setting::new("gen_tone_hz", "Carrier offset", "Hz", -400e3)
        .range(-1e8, 1e8, 0.0)
        .section(SECTION);
/// The amplitude of the carrier.
pub const GEN_TONE_AMP: Setting<field::F64> =
    Setting::new("gen_tone_amp", "Carrier amplitude", "", 0.05)
        .range(0.0, 10.0, 0.0)
        .section(SECTION);

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct NoiseFloorGenerator {
    rate: f64,
    extra_hz: f64,
    from_s: f64,
    to_s: f64,
    pole: f64,
    gain: f64,
    state: Complex32,
    tone_hz: f64,
    tone_amp: f64,
    rng: Rng,
    n: u64,
}

impl NoiseFloorGenerator {
    /// The signal of the settings `v` at `rate_hz` with the seed `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> NoiseFloorGenerator {
        let pole = (-TAU * 0.5 * v.get(&GEN_EXTRA_BW_HZ) / rate_hz).exp();
        // The output of y = a y + (1 - a) w has the power (1 - a) / (1 + a) of w.
        let gain = (v.get(&GEN_EXTRA_POWER) * (1.0 + pole) / (1.0 - pole)).sqrt();
        NoiseFloorGenerator {
            rate: rate_hz,
            extra_hz: v.get(&GEN_EXTRA_HZ),
            from_s: v.get(&GEN_EXTRA_FROM_S),
            to_s: v.get(&GEN_EXTRA_TO_S),
            pole,
            gain,
            state: Complex32::default(),
            tone_hz: v.get(&GEN_TONE_HZ),
            tone_amp: v.get(&GEN_TONE_AMP),
            rng: Rng::new(seed ^ 0x6e6f_6973_655f_666c),
            n: 0,
        }
    }
}

impl Generator for NoiseFloorGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let a = self.pole as f32;
        let w = (1.0 - self.pole) as f32 * (self.gain / std::f64::consts::SQRT_2) as f32;
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let tone = (TAU * (self.tone_hz * t).fract()) as f32;
            *z = Complex32::from_polar(self.tone_amp as f32, tone);
            if t >= self.from_s && t < self.to_s {
                let x = Complex32::new(self.rng.normal_f32(), self.rng.normal_f32()) * w;
                self.state = self.state * a + x;
                let ph = (TAU * (self.extra_hz * t).fract()) as f32;
                *z += self.state * Complex32::from_polar(1.0, ph);
            }
            self.n += 1;
        }
        out.len()
    }
    /// The signal has no reference power of its own: the background noise is given as a fixed floor.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

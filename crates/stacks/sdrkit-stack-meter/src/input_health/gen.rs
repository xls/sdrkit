//! The test signal of `input_health`: complex noise of a stated power per sample and tones at stated offsets with
//! amplitudes in units of the noise amplitude, each component clipped at a stated level and then scaled. The noise is the
//! measured signal itself. The DC offset and the IQ mismatch are those of the standard impairments (`gen_dc_i`, `gen_dc_q`,
//! `gen_iq_gain_db`, `gen_iq_phase_deg`), which act after the clip; their phase error leans Q towards -I, so a phase of
//! -2.5 degrees there reads +2.5 degrees here.

use crate::ptt_log::gen::parse_spans;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The noise power per sample.
pub const GEN_POWER: Setting<field::F64> = Setting::new("gen_power", "Noise power", "", 0.0025)
    .range(0.0, 4.0, 0.0)
    .section(SECTION);
/// The tones.
pub const GEN_TONES: Setting<field::Text> =
    Setting::new("gen_tones", "Tones", "", "123456, 0.3; -300000, 0.3")
        .section(SECTION)
        .help("Entries separated by ';', each 'offset Hz, amplitude' with the amplitude in units of the square root of the noise power.");
/// The clip level of each component.
pub const GEN_CLIP: Setting<field::F64> = Setting::new("gen_clip", "Clip at", "", 1.0)
    .range(0.01, 10.0, 0.0)
    .section(SECTION);
/// The gain after the clip.
pub const GEN_SCALE: Setting<field::F64> =
    Setting::new("gen_scale", "Gain after the clip", "", 1.0)
        .range(0.0, 100.0, 0.0)
        .section(SECTION);

/// The test signal generator.
pub struct InputHealthGenerator {
    rate: f64,
    power: f64,
    tones: Vec<(f64, f64)>,
    clip: f32,
    scale: f32,
    rng: Rng,
    n: u64,
}

impl InputHealthGenerator {
    /// The signal of the settings `v` at `rate_hz`, its noise drawn from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> InputHealthGenerator {
        let power = v.get(&GEN_POWER);
        let tones = parse_spans(v.get(&GEN_TONES))
            .unwrap_or_default()
            .into_iter()
            .map(|(f, a)| (f, a * power.sqrt()))
            .collect();
        InputHealthGenerator {
            rate: rate_hz,
            power,
            tones,
            clip: v.get(&GEN_CLIP) as f32,
            scale: v.get(&GEN_SCALE) as f32,
            rng: Rng::new(seed ^ 0x6865_616c),
            n: 0,
        }
    }
}

impl Generator for InputHealthGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut x = self.rng.complex_normal(self.power);
            for &(f, a) in &self.tones {
                let ph = (TAU * f * t).rem_euclid(TAU);
                x += Complex32::from_polar(a as f32, ph as f32);
            }
            let c = self.clip;
            *z = Complex32::new(x.re.clamp(-c, c), x.im.clamp(-c, c)) * self.scale;
            self.n += 1;
        }
        out.len()
    }
    /// The noise is the signal; it passes the impairments at its own power.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

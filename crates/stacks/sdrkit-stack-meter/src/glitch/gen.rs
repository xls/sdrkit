//! The test signal of `glitch`: a steady tone of a stated frequency and amplitude whose stream has faults put in at stated
//! times: samples dropped or repeated, runs of zeros in place of samples, a block of samples repeated from the block before,
//! and steps of the gain that stay. The drift and the noise are those of the standard impairments, which act after the
//! faults: a zero run or a repeated block keeps its exact samples only with the noise off.

use crate::ptt_log::gen::parse_spans;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::collections::VecDeque;
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The tone.
pub const GEN_TONE_HZ: Setting<field::F64> = Setting::new("gen_tone_hz", "Tone", "Hz", 123_456.0)
    .range(-1e7, 1e7, 0.0)
    .section(SECTION);
/// Its amplitude.
pub const GEN_TONE_AMP: Setting<field::F64> =
    Setting::new("gen_tone_amp", "Tone amplitude", "", 1.0)
        .range(0.0, 1.0, 0.0)
        .section(SECTION);
/// Samples dropped (positive) or repeated (negative).
pub const GEN_DROPS: Setting<field::Text> = Setting::new(
    "gen_drops",
    "Drops and repeats",
    "",
    "1.0, 1; 1.5, 2; 2.0, 3; 2.5, 5; 3.0, -1; 3.5, -2",
)
.section(SECTION)
.help("Entries separated by ';', each 'time s, samples'; negative samples are repeated.");
/// Runs of zeros.
pub const GEN_ZERO_RUNS: Setting<field::Text> = Setting::new("gen_zero_runs", "Zero runs", "", "")
    .section(SECTION)
    .help("Entries separated by ';', each 'time s, samples'.");
/// Repeated blocks.
pub const GEN_REPEAT_BLOCKS: Setting<field::Text> = Setting::new("gen_repeat_blocks", "Repeated blocks", "", "")
    .section(SECTION)
    .help("Times in s separated by ';': the 256 samples after each time plus 256 are those from the time.");
/// Gain steps.
pub const GEN_GAIN_STEPS: Setting<field::Text> =
    Setting::new("gen_gain_steps", "Gain steps", "", "")
        .section(SECTION)
        .help("Entries separated by ';', each 'time s, dB': the gain from that time on.");

/// The length of a repeated block.
pub const REPEAT_BLOCK: u64 = 256;

/// The test signal generator.
pub struct GlitchGenerator {
    rate: f64,
    f: f64,
    amp: f64,
    drops: Vec<(u64, i64)>,
    zeros: Vec<(u64, u64)>,
    repeats: Vec<u64>,
    gains: Vec<(u64, f64)>,
    gain: f32,
    src: i64,
    n: u64,
    past: VecDeque<Complex32>,
}

impl GlitchGenerator {
    /// The signal of the settings `v` at `rate_hz`.
    pub fn new(rate_hz: f64, v: &Values) -> GlitchGenerator {
        let at = |t: f64| (t * rate_hz).round().max(0.0) as u64;
        let pairs = |s: &str| parse_spans(s).unwrap_or_default();
        let repeats = v
            .get(&GEN_REPEAT_BLOCKS)
            .split(';')
            .filter_map(|e| e.trim().parse::<f64>().ok())
            .map(at)
            .collect();
        GlitchGenerator {
            rate: rate_hz,
            f: v.get(&GEN_TONE_HZ),
            amp: v.get(&GEN_TONE_AMP),
            drops: pairs(v.get(&GEN_DROPS))
                .into_iter()
                .map(|(t, n)| (at(t), n as i64))
                .collect(),
            zeros: pairs(v.get(&GEN_ZERO_RUNS))
                .into_iter()
                .map(|(t, n)| (at(t), n as u64))
                .collect(),
            repeats,
            gains: pairs(v.get(&GEN_GAIN_STEPS))
                .into_iter()
                .map(|(t, d)| (at(t), d))
                .collect(),
            gain: 1.0,
            src: 0,
            n: 0,
            past: VecDeque::new(),
        }
    }
}

impl Generator for GlitchGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let b = REPEAT_BLOCK as usize;
        for z in out.iter_mut() {
            let k = self.n;
            for &(t, d) in &self.drops {
                if t == k {
                    self.src += d;
                }
            }
            for &(t, g) in &self.gains {
                if t == k {
                    self.gain = 10f64.powf(g / 20.0) as f32;
                }
            }
            let ph = (TAU * self.f * self.src as f64 / self.rate).rem_euclid(TAU);
            let mut s = Complex32::from_polar(self.amp as f32, ph as f32) * self.gain;
            if self.zeros.iter().any(|&(t, n)| k >= t && k < t + n) {
                s = Complex32::default();
            }
            if self
                .repeats
                .iter()
                .any(|&t| k >= t + REPEAT_BLOCK && k < t + 2 * REPEAT_BLOCK)
                && self.past.len() >= b
            {
                s = self.past[self.past.len() - b];
            }
            self.past.push_back(s);
            if self.past.len() > b {
                self.past.pop_front();
            }
            *z = s;
            self.src += 1;
            self.n += 1;
        }
        out.len()
    }
    /// The noise is stated against a tone of power 1.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

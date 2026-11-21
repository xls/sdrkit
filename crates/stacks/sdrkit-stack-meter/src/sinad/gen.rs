//! The test signal of `sinad`: an FM carrier of amplitude 1 modulated by a test tone with optional second and third
//! harmonics, its deviation changing at stated times. The offset, the drift and the noise are the standard impairments; an SNR
//! is the CNR in 16 kHz.

use crate::ptt_log::gen::parse_spans;
use crate::txtest::FmTones;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The test tone.
pub const GEN_TONE_HZ: Setting<field::F64> = Setting::new("gen_tone_hz", "Test tone", "Hz", 1000.0)
    .range(50.0, 8000.0, 0.0)
    .section(SECTION);
/// The peak deviation.
pub const GEN_DEVIATION_HZ: Setting<field::F64> =
    Setting::new("gen_deviation_hz", "Deviation", "Hz", 3000.0)
        .range(0.0, 10_000.0, 0.0)
        .section(SECTION);
/// The second harmonic as a share of the audio.
pub const GEN_H2: Setting<field::F64> = Setting::new("gen_h2", "Second harmonic", "", 0.0)
    .range(0.0, 1.0, 0.0)
    .section(SECTION);
/// The third harmonic as a share of the audio.
pub const GEN_H3: Setting<field::F64> = Setting::new("gen_h3", "Third harmonic", "", 0.0)
    .range(0.0, 1.0, 0.0)
    .section(SECTION);
/// Later deviations.
pub const GEN_DEVIATION_AFTER: Setting<field::Text> =
    Setting::new("gen_deviation_after", "Later deviations", "", "")
        .section(SECTION)
        .help("Changes separated by ';', each 'time s, deviation Hz'.");

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 16e3;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct SinadGenerator {
    rate: f64,
    tone: f64,
    dev: f64,
    h2: f64,
    h3: f64,
    after: Vec<(f64, f64)>,
    fm: FmTones,
    n: u64,
}

impl SinadGenerator {
    /// The signal of the settings `v` at `rate_hz`.
    pub fn new(rate_hz: f64, v: &Values) -> SinadGenerator {
        let mut after = parse_spans(v.get(&GEN_DEVIATION_AFTER)).unwrap_or_default();
        after.sort_by(|a, b| a.0.total_cmp(&b.0));
        SinadGenerator {
            rate: rate_hz,
            tone: v.get(&GEN_TONE_HZ),
            dev: v.get(&GEN_DEVIATION_HZ),
            h2: v.get(&GEN_H2),
            h3: v.get(&GEN_H3),
            after,
            fm: FmTones::default(),
            n: 0,
        }
    }
}

impl Generator for SinadGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let dev = self
                .after
                .iter()
                .rev()
                .find(|a| t >= a.0)
                .map_or(self.dev, |a| a.1);
            let w = TAU * self.tone * t;
            let f = dev * (w.sin() + self.h2 * (2.0 * w).sin() + self.h3 * (3.0 * w).sin());
            *z = self.fm.next(f, self.rate, 1.0);
            self.n += 1;
        }
        out.len()
    }
    /// The noise is stated against a carrier of power 1; an SNR is stated in 16 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

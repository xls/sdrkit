//! The test signal of `kurtosis`: tones at stated offsets and amplitudes, each on for a stated share of its period (always
//! on for a period of 0). The noise is that of the standard impairments (`gen_noise` with a floor of 0 dBFS gives a power
//! of 1 per sample) and the shift is their offset.

use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The tones.
pub const GEN_TONES: Setting<field::Text> =
    Setting::new("gen_tones", "Tones", "", "300000, 8, 0, 1; -400000, 40, 0.004, 0.1")
        .section(SECTION)
        .help("Entries separated by ';', each 'offset Hz, amplitude, period s, duty'; a period of 0 is always on.");

/// One tone: offset, amplitude, period, duty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PulsedTone {
    /// The offset, Hz.
    pub hz: f64,
    /// The amplitude.
    pub amp: f64,
    /// The period, s (0: always on).
    pub period_s: f64,
    /// The share of the period that it is on.
    pub duty: f64,
}

/// The tones of a list as `GEN_TONES` states it; `None` when an entry does not read.
pub fn parse_pulsed(text: &str) -> Option<Vec<PulsedTone>> {
    text.split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| {
            let v: Vec<f64> = e
                .split(',')
                .map(|x| x.trim().parse().ok())
                .collect::<Option<_>>()?;
            (v.len() == 4).then(|| PulsedTone {
                hz: v[0],
                amp: v[1],
                period_s: v[2],
                duty: v[3],
            })
        })
        .collect()
}

/// The test signal generator.
pub struct KurtosisGenerator {
    rate: f64,
    tones: Vec<PulsedTone>,
    n: u64,
}

impl KurtosisGenerator {
    /// The signal of the settings `v` at `rate_hz`.
    pub fn new(rate_hz: f64, v: &Values) -> KurtosisGenerator {
        KurtosisGenerator {
            rate: rate_hz,
            tones: parse_pulsed(v.get(&GEN_TONES)).unwrap_or_default(),
            n: 0,
        }
    }
}

impl Generator for KurtosisGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut s = Complex32::default();
            for tone in &self.tones {
                let on = tone.period_s <= 0.0 || (t / tone.period_s).fract() < tone.duty;
                if on {
                    s += Complex32::from_polar(
                        tone.amp as f32,
                        (TAU * tone.hz * t).rem_euclid(TAU) as f32,
                    );
                }
            }
            *z = s;
            self.n += 1;
        }
        out.len()
    }
    /// The noise of the impairments is stated against a power of 1.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

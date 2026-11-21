//! The test signal of `ssb_test`: the audio tones of an SSB transmitter on one sideband of a suppressed carrier at the
//! centre, each at a stated level relative to a tone of amplitude 1, with a carrier leak. The tuning error of the radio is the
//! standard offset `gen_offset_hz`; the noise is the standard noise, an SNR being the tone CNR in 3 kHz.

use crate::typed::SsbSideband;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The default tones: one tone at 1000 Hz, the opposite sideband and harmonics 2 to 5.
pub const DEFAULT_TONES: &str = "1000, 0; -1000, -55; 2000, -60; 3000, -58; 4000, -70; 5000, -66";

/// The audio tones.
pub const GEN_TONES: Setting<field::Text> = Setting::new("gen_tones", "Tones", "", DEFAULT_TONES)
    .section(SECTION)
    .help("Tones separated by ';', each 'audio Hz, dBc'; a negative audio frequency is on the opposite sideband.");
/// The sideband.
pub const GEN_SIDEBAND: Setting<field::Choice<SsbSideband>> =
    Setting::choice("gen_sideband", "Sideband", SsbSideband::Upper).section(SECTION);
/// The carrier leak.
pub const GEN_CARRIER_DBC: Setting<field::F64> =
    Setting::new("gen_carrier_dbc", "Carrier leak", "dBc", -50.0)
        .range(-200.0, 0.0, 0.0)
        .section(SECTION);

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 3000.0;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct SsbGenerator {
    rate: f64,
    tones: Vec<(f64, f64, f64)>,
    carrier: f64,
    n: u64,
}

impl SsbGenerator {
    /// The signal of the settings `v` at `rate_hz`; a list that cannot be read is empty.
    pub fn new(rate_hz: f64, v: &Values) -> SsbGenerator {
        let s = if v.get(&GEN_SIDEBAND) == SsbSideband::Upper {
            1.0
        } else {
            -1.0
        };
        let tones = crate::ptt_log::gen::parse_spans(v.get(&GEN_TONES))
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(k, (f, dbc))| (s * f, 10f64.powf(dbc / 20.0), 1.3 * k as f64))
            .collect();
        SsbGenerator {
            rate: rate_hz,
            tones,
            carrier: 10f64.powf(v.get(&GEN_CARRIER_DBC) / 20.0),
            n: 0,
        }
    }
}

impl Generator for SsbGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut s = Complex32::new(self.carrier as f32, 0.0);
            for &(f, a, p) in &self.tones {
                s += Complex32::from_polar(a as f32, (TAU * (f * t).fract() + p) as f32);
            }
            *z = s;
            self.n += 1;
        }
        out.len()
    }
    /// The noise is stated against a tone of amplitude 1; an SNR is stated in 3 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

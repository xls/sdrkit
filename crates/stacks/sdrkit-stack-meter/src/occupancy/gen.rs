//! The occupancy test signal: keyed carriers at offsets from the centre. The default is four carriers 2.5 kHz above the
//! centres of channels -12, -3, +6 and +15 from the middle of a grid of 25 390.625 Hz channels (26 bins of a 2048-point FFT
//! at 2 Msps), keyed with duties of 50, 20, 90 and 35 % and periods of 2, 1, 5 and 1.6 s. The noise reference is a carrier of
//! power 1 in 25 kHz.

use crate::scene::{parse_tones, ToneScene};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};

const SECTION: &str = "Test signal";

/// The default carriers: offset Hz, amplitude, period s, duty, first key-down s.
pub const DEFAULT_TONES: &str =
    "-289507.8, 1, 2, 0.5, 0.3; -60976.6, 1, 1, 0.2, 0.1; 167539.1, 1, 5, 0.9, 0.7; \
396054.7, 1, 1.6, 0.35, 0.45";

/// The carriers of the test signal.
pub const GEN_TONES: Setting<field::Text> = Setting::new("gen_tones", "Carriers", "", DEFAULT_TONES)
    .section(SECTION)
    .help(
        "Carriers separated by ';', each 'offset Hz, amplitude, period s, duty, first key-down s, drift Hz/s, from s, to \
         s'; the values after the amplitude can be left out.",
    );

/// The band that the noise of the impairments is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 25e3;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct OccupancyGenerator {
    scene: ToneScene,
}

impl OccupancyGenerator {
    /// The scene of the settings `v` at `rate_hz`; a list that cannot be read is no carrier.
    pub fn new(rate_hz: f64, v: &Values) -> OccupancyGenerator {
        let tones = parse_tones(v.get(&GEN_TONES)).unwrap_or_default();
        OccupancyGenerator {
            scene: ToneScene::new(rate_hz, tones),
        }
    }
}

impl Generator for OccupancyGenerator {
    fn rate_hz(&self) -> Hz {
        self.scene.rate()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        self.scene.add_into(out);
        out.len()
    }
    /// A carrier of power 1 is the reference; the noise is stated in 25 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

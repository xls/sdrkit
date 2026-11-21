//! The persistent carrier test signal: keyed or steady carriers at offsets from the centre, and a carrier frequency-modulated
//! by low-passed noise. The default is a bare carrier at +123 456.7 Hz drifting 0.5 Hz/s (amplitude 0.05), a steady one at
//! -75 000.3 Hz (0.02), a weak one at +300 kHz (0.0015), one keyed 50 % with a 4 s period at -240 kHz (0.05) and the
//! modulated one at +450 kHz (0.03). Noise comes from the standard impairments.

use crate::scene::{parse_tones, ToneScene};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The default carriers: offset Hz, amplitude, period s, duty, first key-down s, drift Hz/s.
pub const DEFAULT_TONES: &str =
    "123456.7, 0.05, 0, 1, 0, 0.5; -75000.3, 0.02; 300000, 0.0015; -240000, 0.05, 4, 0.5";

/// The carriers of the test signal.
pub const GEN_TONES: Setting<field::Text> = Setting::new("gen_tones", "Carriers", "", DEFAULT_TONES)
    .section(SECTION)
    .help(
        "Carriers separated by ';', each 'offset Hz, amplitude, period s, duty, first key-down s, drift Hz/s, from s, to \
         s'; the values after the amplitude can be left out.",
    );
/// The offset of the modulated carrier.
pub const GEN_FM_HZ: Setting<field::F64> =
    Setting::new("gen_fm_hz", "Modulated carrier offset", "Hz", 450e3)
        .range(-1e8, 1e8, 0.0)
        .section(SECTION);
/// The amplitude of the modulated carrier; 0 for none.
pub const GEN_FM_AMP: Setting<field::F64> =
    Setting::new("gen_fm_amp", "Modulated carrier amplitude", "", 0.03)
        .range(0.0, 10.0, 0.0)
        .section(SECTION);
/// The RMS deviation of the modulated carrier.
pub const GEN_FM_DEV_HZ: Setting<field::F64> = Setting::new(
    "gen_fm_dev_hz",
    "Modulated carrier deviation",
    "Hz",
    3000.0,
)
.range(0.0, 1e6, 0.0)
.section(SECTION)
.help("The RMS deviation; the modulation is white noise through a one-pole low-pass of 300 Hz.");

/// The cut-off of the low-pass of the modulating noise, Hz.
const FM_CUT_HZ: f64 = 300.0;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct BirdieGenerator {
    scene: ToneScene,
    fm_hz: f64,
    fm_amp: f32,
    pole: f64,
    gain: f64,
    m: f64,
    phase: f64,
    rng: Rng,
}

impl BirdieGenerator {
    /// The scene of the settings `v` at `rate_hz`; the modulation from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> BirdieGenerator {
        let pole = (-TAU * FM_CUT_HZ / rate_hz).exp();
        BirdieGenerator {
            scene: ToneScene::new(rate_hz, parse_tones(v.get(&GEN_TONES)).unwrap_or_default()),
            fm_hz: v.get(&GEN_FM_HZ),
            fm_amp: v.get(&GEN_FM_AMP) as f32,
            pole,
            gain: v.get(&GEN_FM_DEV_HZ) * ((1.0 + pole) / (1.0 - pole)).sqrt(),
            m: 0.0,
            phase: 0.0,
            rng: Rng::new(seed ^ 0x6269_7264_6965),
        }
    }
}

impl Generator for BirdieGenerator {
    fn rate_hz(&self) -> Hz {
        self.scene.rate()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        if self.fm_amp > 0.0 {
            let rate = self.scene.rate();
            for z in out.iter_mut() {
                self.m = self.pole * self.m + (1.0 - self.pole) * self.gain * self.rng.normal();
                self.phase = (self.phase + TAU * (self.fm_hz + self.m) / rate).rem_euclid(TAU);
                *z = Complex32::from_polar(self.fm_amp, self.phase as f32);
            }
        }
        self.scene.add_into(out);
        out.len()
    }
    /// A carrier of power 1 is the reference; the noise is given as a fixed floor.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.scene.rate()).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

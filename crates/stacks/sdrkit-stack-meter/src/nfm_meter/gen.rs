//! The test signal of `nfm_meter`: an FM carrier of amplitude 1 with a four-tone voice
//! `(0.45 sin(2 pi 523 t) + 0.3 sin(2 pi 1117 t + 1) + 0.2 sin(2 pi 1931 t + 2) + 0.1 sin(2 pi 2707 t + 0.3)) x scale` and a CTCSS
//! tone `deviation x sin(2 pi f t + 0.5)` that can stop at a stated time. The offset, the drift and the noise are the standard
//! impairments; an SNR is the CNR in 16 kHz.

use crate::txtest::FmTones;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The scale of the four-tone voice.
pub const GEN_VOICE_HZ: Setting<field::F64> =
    Setting::new("gen_voice_hz", "Voice scale", "Hz", 1600.0)
        .range(0.0, 20_000.0, 0.0)
        .section(SECTION);
/// The CTCSS tone (0 for none).
pub const GEN_CTCSS_HZ: Setting<field::F64> =
    Setting::new("gen_ctcss_hz", "CTCSS tone", "Hz", 100.0)
        .range(0.0, 300.0, 0.0)
        .section(SECTION);
/// The CTCSS deviation.
pub const GEN_CTCSS_DEV_HZ: Setting<field::F64> =
    Setting::new("gen_ctcss_dev_hz", "CTCSS deviation", "Hz", 400.0)
        .range(0.0, 5000.0, 0.0)
        .section(SECTION);
/// The time the CTCSS tone stops (0 for never).
pub const GEN_CTCSS_OFF_S: Setting<field::F64> =
    Setting::new("gen_ctcss_off_s", "CTCSS off at", "s", 0.0)
        .range(0.0, 1e6, 0.0)
        .section(SECTION);

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 16e3;

/// The four voice tones: share, frequency Hz, phase rad.
pub const VOICE: [(f64, f64, f64); 4] = [
    (0.45, 523.0, 0.0),
    (0.3, 1117.0, 1.0),
    (0.2, 1931.0, 2.0),
    (0.1, 2707.0, 0.3),
];

/// The voice of scale 1 at `t`.
pub fn voice(t: f64) -> f64 {
    VOICE
        .iter()
        .map(|&(a, f, p)| a * (TAU * f * t + p).sin())
        .sum()
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct NfmGenerator {
    rate: f64,
    voice_hz: f64,
    ctcss_hz: f64,
    ctcss_dev: f64,
    ctcss_off: f64,
    fm: FmTones,
    n: u64,
}

impl NfmGenerator {
    /// The signal of the settings `v` at `rate_hz`.
    pub fn new(rate_hz: f64, v: &Values) -> NfmGenerator {
        let off = v.get(&GEN_CTCSS_OFF_S);
        NfmGenerator {
            rate: rate_hz,
            voice_hz: v.get(&GEN_VOICE_HZ),
            ctcss_hz: v.get(&GEN_CTCSS_HZ),
            ctcss_dev: v.get(&GEN_CTCSS_DEV_HZ),
            ctcss_off: if off > 0.0 { off } else { f64::INFINITY },
            fm: FmTones::default(),
            n: 0,
        }
    }
}

impl Generator for NfmGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut f = self.voice_hz * voice(t);
            if self.ctcss_hz > 0.0 && t < self.ctcss_off {
                f += self.ctcss_dev * (TAU * self.ctcss_hz * t + 0.5).sin();
            }
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

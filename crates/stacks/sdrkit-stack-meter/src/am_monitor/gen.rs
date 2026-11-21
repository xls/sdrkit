//! The AM monitor test signal: a carrier at the centre whose envelope is `1 + m(t)`, with `m` two tones, a raised positive
//! half for asymmetry, clipping at zero carrier and a cut of the carrier. Noise, mains hum, the carrier offset and its drift
//! come from the standard impairments of the test signal.

use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

/// The section of the test signal settings.
const SECTION: &str = "Test signal";

/// The depth of the first tone.
pub const GEN_DEPTH1: Setting<field::F64> = Setting::new("gen_depth1", "First tone depth", "", 0.5)
    .range(0.0, 2.0, 0.0)
    .section(SECTION)
    .help("The peak of the first tone as a share of the carrier.");
/// The depth of the second tone.
pub const GEN_DEPTH2: Setting<field::F64> = Setting::new("gen_depth2", "Second tone depth", "", 0.2)
    .range(0.0, 2.0, 0.0)
    .section(SECTION)
    .help("The peak of the second tone as a share of the carrier.");
/// The frequency of the first tone.
pub const GEN_TONE1_HZ: Setting<field::F64> = Setting::new("gen_tone1_hz", "First tone", "Hz", 1000.0)
    .range(20.0, 10_000.0, 0.0)
    .section(SECTION);
/// The frequency of the second tone.
pub const GEN_TONE2_HZ: Setting<field::F64> = Setting::new("gen_tone2_hz", "Second tone", "Hz", 3300.0)
    .range(20.0, 10_000.0, 0.0)
    .section(SECTION);
/// The asymmetry: the positive half of the modulation is raised by this share.
pub const GEN_ASYMMETRY: Setting<field::F64> = Setting::new("gen_asymmetry", "Asymmetry", "", 0.0)
    .range(0.0, 2.0, 0.0)
    .section(SECTION)
    .help("The positive half of the modulation is raised by this share of itself.");
/// The start of the cut of the carrier.
pub const GEN_CUT_FROM_S: Setting<field::F64> = Setting::new("gen_cut_from_s", "Carrier cut from", "s", -1.0)
    .range(-1.0, 1e6, 0.0)
    .section(SECTION)
    .help("The carrier is off from this time; a negative value is no cut.");
/// The end of the cut of the carrier.
pub const GEN_CUT_UNTIL_S: Setting<field::F64> = Setting::new("gen_cut_until_s", "Carrier cut until", "s", -1.0)
    .range(-1.0, 1e6, 0.0)
    .section(SECTION)
    .help("The carrier is back from this time.");
/// Clipping of the envelope at zero carrier.
pub const GEN_CLIP: Setting<field::Bool> = Setting::new("gen_clip", "Clip at zero carrier", "", true).section(SECTION);

/// The test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct AmSignal {
    /// The depth and frequency of the first tone.
    pub tone1: (f64, f64),
    /// The depth and frequency of the second tone.
    pub tone2: (f64, f64),
    /// The asymmetry.
    pub asymmetry: f64,
        /// The cut of the carrier: from, until (s).
    pub cut: Option<(f64, f64)>,
    /// Clipping at zero carrier.
    pub clip: bool,
}

impl Default for AmSignal {
    fn default() -> Self {
        AmSignal {
            tone1: (GEN_DEPTH1.default, GEN_TONE1_HZ.default),
            tone2: (GEN_DEPTH2.default, GEN_TONE2_HZ.default),
            asymmetry: 0.0,
            cut: None,
            clip: true,
        }
    }
}

impl AmSignal {
    /// The signal that the settings `v` give.
    pub fn from_values(v: &Values) -> AmSignal {
        let from = v.get(&GEN_CUT_FROM_S);
        let until = v.get(&GEN_CUT_UNTIL_S);
        AmSignal {
            tone1: (v.get(&GEN_DEPTH1), v.get(&GEN_TONE1_HZ)),
            tone2: (v.get(&GEN_DEPTH2), v.get(&GEN_TONE2_HZ)),
            asymmetry: v.get(&GEN_ASYMMETRY),
            cut: (from >= 0.0 && until > from).then_some((from, until)),
            clip: v.get(&GEN_CLIP),
        }
    }

    /// The envelope at time `t`.
    pub fn envelope(&self, t: f64) -> f64 {
        if self.cut.is_some_and(|(a, b)| t >= a && t < b) {
            return 0.0;
        }
        let mut m = self.tone1.0 * (TAU * self.tone1.1 * t).sin() + self.tone2.0 * (TAU * self.tone2.1 * t + 1.0).sin();
        m += self.asymmetry * m.max(0.0);
        let e = 1.0 + m;
        if self.clip {
            e.max(0.0)
        } else {
            e
        }
    }
}

/// Makes the test signal at a rate.
#[derive(Clone, Debug)]
pub struct AmGenerator {
    rate: f64,
    signal: AmSignal,
    n: u64,
}

impl AmGenerator {
    /// The signal `signal` at `rate_hz`.
    pub fn new(rate_hz: f64, signal: AmSignal) -> AmGenerator {
        AmGenerator {
            rate: rate_hz,
            signal,
            n: 0,
        }
    }
}

impl Generator for AmGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            *z = Complex32::new(self.signal.envelope(t) as f32, 0.0);
        }
        out.len()
    }
    /// The carrier power is 1; the noise of the impairments is set against it in the 12 kHz of the channel.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(12e3).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

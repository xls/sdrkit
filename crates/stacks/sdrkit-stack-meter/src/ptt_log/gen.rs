//! The PTT test signal: a carrier keyed on and off with raised-cosine edges of 5 ms, with fades (the carrier off for a
//! short time inside a transmission). The default is a session of 15 transmissions over 330 s with 4 fades. The noise
//! reference is the carrier of power 1 in 12.5 kHz.

use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::{PI, TAU};

const SECTION: &str = "Test signal";

/// The default transmissions: from s, to s.
pub const DEFAULT_KEYED: &str =
    "10, 14.2; 20, 22.5; 30, 31.0; 40.3, 40.6; 50, 60.5; 75, 76.2; 90, 98; 100, 101.5; \
110, 111; 111.7, 113; 130, 135; 150, 150.35; 170, 171.3; 200, 215; 300, 303";
/// The default fades: from s, to s.
pub const DEFAULT_FADES: &str = "12.0, 12.12; 55, 55.12; 94, 94.15; 132, 132.2";

/// The transmissions of the test signal.
pub const GEN_KEYED: Setting<field::Text> =
    Setting::new("gen_keyed", "Transmissions", "", DEFAULT_KEYED)
        .section(SECTION)
        .help("Transmissions separated by ';', each 'from s, to s'.");
/// The fades of the test signal.
pub const GEN_FADES: Setting<field::Text> = Setting::new("gen_fades", "Fades", "", DEFAULT_FADES)
    .section(SECTION)
    .help("Fades separated by ';', each 'from s, to s': the carrier is off.");
/// The offset of the carrier.
pub const GEN_TONE_HZ: Setting<field::F64> =
    Setting::new("gen_tone_hz", "Carrier offset", "Hz", 0.0)
        .range(-1e6, 1e6, 0.0)
        .section(SECTION);

/// The length of an edge, s.
pub const EDGE_S: f64 = 0.005;
/// The band that the noise of the impairments is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 12.5e3;

/// Reads a list of `from, to` pairs separated by `;`; `None` for a value that is not a number.
pub fn parse_spans(text: &str) -> Option<Vec<(f64, f64)>> {
    text.split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| {
            let v: Vec<f64> = e
                .split(',')
                .map(|x| x.trim().parse().ok())
                .collect::<Option<_>>()?;
            (v.len() == 2).then(|| (v[0], v[1]))
        })
        .collect()
}

/// The raised-cosine gate of the spans at `t`: 1 inside, 0 outside, edges of [`EDGE_S`] starting at each boundary.
fn gate(spans: &[(f64, f64)], t: f64) -> f64 {
    let rise = |u: f64| 0.5 - 0.5 * (PI * (u / EDGE_S).clamp(0.0, 1.0)).cos();
    spans
        .iter()
        .map(|&(a, b)| {
            if t < a || t >= b + EDGE_S {
                0.0
            } else {
                rise(t - a).min(1.0 - rise(t - b))
            }
        })
        .fold(0.0, f64::max)
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct PttGenerator {
    rate: f64,
    keyed: Vec<(f64, f64)>,
    fades: Vec<(f64, f64)>,
    tone_hz: f64,
    n: u64,
}

impl PttGenerator {
    /// The session of the settings `v` at `rate_hz`; a list that cannot be read is empty.
    pub fn new(rate_hz: f64, v: &Values) -> PttGenerator {
        PttGenerator {
            rate: rate_hz,
            keyed: parse_spans(v.get(&GEN_KEYED)).unwrap_or_default(),
            fades: parse_spans(v.get(&GEN_FADES)).unwrap_or_default(),
            tone_hz: v.get(&GEN_TONE_HZ),
            n: 0,
        }
    }
}

impl Generator for PttGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let a = gate(&self.keyed, t) * (1.0 - gate(&self.fades, t));
            *z = Complex32::from_polar(a as f32, (TAU * (self.tone_hz * t).fract()) as f32);
            self.n += 1;
        }
        out.len()
    }
    /// The carrier of power 1 is the reference; the noise is stated in 12.5 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

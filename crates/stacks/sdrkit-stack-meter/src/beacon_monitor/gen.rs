//! The test signal of `beacon_monitor`: a beacon carrier of amplitude 1, optionally faded by a gain of
//! `10^(depth / 20 x 0.5 (sin(2 pi f t + a0) + 0.6 sin(2 pi 1.7 f t + a1)) / 1.6)`, optionally smeared by a phase
//! modulation with 24 sines of 0.35 rad spread up to a stated frequency with random phases, and off in stated spans. The
//! offset, the drift and the noise are those of the standard impairments (a C/N0 of `SNR + 10 log10(bin)` puts the line
//! SNR dB over the noise of a bin).

use crate::ptt_log::gen::parse_spans;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The fading rate (0: none).
pub const GEN_FADE_HZ: Setting<field::F64> = Setting::new("gen_fade_hz", "Fading rate", "Hz", 0.0)
    .range(0.0, 10.0, 0.0)
    .section(SECTION);
/// The fading depth.
pub const GEN_FADE_DEPTH_DB: Setting<field::F64> =
    Setting::new("gen_fade_depth_db", "Fading depth", "dB", 14.0)
        .range(0.0, 40.0, 0.0)
        .section(SECTION);
/// The aurora spread (0: none).
pub const GEN_SPREAD_HZ: Setting<field::F64> =
    Setting::new("gen_spread_hz", "Aurora spread", "Hz", 0.0)
        .range(0.0, 500.0, 0.0)
        .section(SECTION);
/// The spans without beacon.
pub const GEN_OFF: Setting<field::Text> = Setting::new("gen_off", "No beacon", "", "")
    .section(SECTION)
    .help("Spans separated by ';', each 'from s, to s'.");

/// The sines of the aurora smear.
pub const SMEAR_SINES: usize = 24;
/// Their peak phase, rad.
pub const SMEAR_RAD: f64 = 0.35;

/// The test signal generator.
pub struct BeaconMonitorGenerator {
    rate: f64,
    fade: f64,
    depth: f64,
    phases: [f64; 2],
    smear: Vec<(f64, f64)>,
    off: Vec<(f64, f64)>,
    n: u64,
}

impl BeaconMonitorGenerator {
    /// The signal of the settings `v` at `rate_hz`, its random phases drawn from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> BeaconMonitorGenerator {
        let mut rng = Rng::new(seed ^ 0x6265_6163);
        let spread = v.get(&GEN_SPREAD_HZ);
        let smear = if spread > 0.0 {
            (0..SMEAR_SINES)
                .map(|i| {
                    (
                        spread * (i + 1) as f64 / SMEAR_SINES as f64,
                        TAU * rng.uniform(),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        BeaconMonitorGenerator {
            rate: rate_hz,
            fade: v.get(&GEN_FADE_HZ),
            depth: v.get(&GEN_FADE_DEPTH_DB),
            phases: [TAU * rng.uniform(), TAU * rng.uniform()],
            smear,
            off: parse_spans(v.get(&GEN_OFF)).unwrap_or_default(),
            n: 0,
        }
    }
}

impl Generator for BeaconMonitorGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            if self.off.iter().any(|&(a, b)| t >= a && t < b) {
                *z = Complex32::default();
                continue;
            }
            let g = if self.fade > 0.0 {
                let w = (TAU * self.fade * t + self.phases[0]).sin()
                    + 0.6 * (TAU * 1.7 * self.fade * t + self.phases[1]).sin();
                10f64.powf(self.depth / 20.0 * 0.5 * w / 1.6)
            } else {
                1.0
            };
            let ph: f64 = self
                .smear
                .iter()
                .map(|&(f, a)| SMEAR_RAD * (TAU * f * t + a).sin())
                .sum();
            *z = Complex32::from_polar(g as f32, ph as f32);
        }
        out.len()
    }
    /// The beacon has a power of 1 while on.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(10.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

//! The test signal of `freq_accuracy`: a carrier of amplitude 1 at the centre that steps to other frequencies, goes off for
//! stretches, and carries white frequency noise held constant over each hold. The offset, the drift and the noise are the
//! standard impairments (`gen_offset_hz`, `gen_drift_hz_s`, `gen_noise` with `gen_cn0_dbhz`).

use crate::ptt_log::gen::parse_spans;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The steps of the carrier.
pub const GEN_STEPS: Setting<field::Text> = Setting::new("gen_steps", "Steps", "", "")
    .section(SECTION)
    .help("Steps separated by ';', each 'time s, frequency Hz': from that time the carrier is that far from its start.");
/// The stretches without a carrier.
pub const GEN_GAPS: Setting<field::Text> = Setting::new("gen_gaps", "Gaps", "", "")
    .section(SECTION)
    .help("Stretches separated by ';', each 'from s, to s': the carrier is off.");
/// The white frequency noise.
pub const GEN_FM_NOISE_HZ: Setting<field::F64> =
    Setting::new("gen_fm_noise_hz", "Frequency noise", "Hz", 0.0)
        .range(0.0, 1000.0, 0.0)
        .section(SECTION)
        .help("Standard deviation of a frequency held constant over each hold; 0 for none.");
/// The hold of the frequency noise.
pub const GEN_FM_HOLD_S: Setting<field::F64> =
    Setting::new("gen_fm_hold_s", "Frequency noise hold", "s", 1.024)
        .range(0.001, 100.0, 0.0)
        .section(SECTION);

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 1000.0;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct FreqAccuracyGenerator {
    rate: f64,
    steps: Vec<(f64, f64)>,
    gaps: Vec<(f64, f64)>,
    fm_hz: f64,
    hold_s: f64,
    held: f64,
    hold_index: i64,
    rng: Rng,
    phase: f64,
    n: u64,
}

impl FreqAccuracyGenerator {
    /// The signal of the settings `v` at `rate_hz` with the random stream of `seed`; a list that cannot be read is empty.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> FreqAccuracyGenerator {
        let mut steps = parse_spans(v.get(&GEN_STEPS)).unwrap_or_default();
        steps.sort_by(|a, b| a.0.total_cmp(&b.0));
        FreqAccuracyGenerator {
            rate: rate_hz,
            steps,
            gaps: parse_spans(v.get(&GEN_GAPS)).unwrap_or_default(),
            fm_hz: v.get(&GEN_FM_NOISE_HZ),
            hold_s: v.get(&GEN_FM_HOLD_S),
            held: 0.0,
            hold_index: -1,
            rng: Rng::stream(seed, 41),
            phase: 0.0,
            n: 0,
        }
    }
}

impl Generator for FreqAccuracyGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let hold = (t / self.hold_s).floor() as i64;
            if hold != self.hold_index {
                self.hold_index = hold;
                self.held = self.fm_hz * self.rng.normal();
            }
            let step = self
                .steps
                .iter()
                .rev()
                .find(|s| t >= s.0)
                .map_or(0.0, |s| s.1);
            self.phase = (self.phase + TAU * (step + self.held) / self.rate).rem_euclid(TAU);
            let on = !self.gaps.iter().any(|&(a, b)| t >= a && t < b);
            *z = if on {
                Complex32::from_polar(1.0, self.phase as f32)
            } else {
                Complex32::default()
            };
            self.n += 1;
        }
        out.len()
    }
    /// The noise is stated against a carrier of power 1; an SNR is stated in 1 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

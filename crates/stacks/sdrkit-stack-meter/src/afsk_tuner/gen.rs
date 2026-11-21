//! The test signal of `afsk_tuner`: an FM carrier of amplitude 1, on all the time, that carries bursts of Bell 202 AFSK
//! (mark 1200 Hz, space 2200 Hz, continuous phase, random bits that each toggle the tone with probability one half). The louder
//! tone has the full deviation and the other `10^(twist / 20)` of it. The offset, the clock error and the noise are the
//! standard impairments; an SNR is the CNR in 16 kHz.

use crate::txtest::FmTones;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The mark tone of Bell 202, Hz.
pub const MARK_HZ: f64 = 1200.0;
/// The space tone of Bell 202, Hz.
pub const SPACE_HZ: f64 = 2200.0;

/// The deviation of the louder tone.
pub const GEN_DEVIATION_HZ: Setting<field::F64> =
    Setting::new("gen_deviation_hz", "Deviation", "Hz", 3000.0)
        .range(0.0, 10_000.0, 0.0)
        .section(SECTION);
/// The twist, space over mark.
pub const GEN_TWIST_DB: Setting<field::F64> =
    Setting::new("gen_twist_db", "Twist, space over mark", "dB", 0.0)
        .range(-20.0, 20.0, 0.0)
        .section(SECTION);
/// The bit rate.
pub const GEN_BAUD: Setting<field::F64> = Setting::new("gen_baud", "Bit rate", "Bd", 1200.0)
    .range(50.0, 9600.0, 0.0)
    .section(SECTION);
/// The burst period (0 for one stream that does not stop).
pub const GEN_PERIOD_S: Setting<field::F64> =
    Setting::new("gen_period_s", "Burst period", "s", 2.0)
        .range(0.0, 600.0, 0.0)
        .section(SECTION);
/// The burst length.
pub const GEN_BURST_S: Setting<field::F64> = Setting::new("gen_burst_s", "Burst length", "s", 0.5)
    .range(0.01, 600.0, 0.0)
    .section(SECTION);
/// The first burst.
pub const GEN_FIRST_S: Setting<field::F64> =
    Setting::new("gen_first_s", "First burst at", "s", 0.5)
        .range(0.0, 600.0, 0.0)
        .section(SECTION);

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 16e3;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct AfskGenerator {
    rate: f64,
    mark_dev: f64,
    space_dev: f64,
    baud: f64,
    period: f64,
    burst: f64,
    first: f64,
    rng: Rng,
    bit: i64,
    space: bool,
    tone_phase: f64,
    fm: FmTones,
    n: u64,
}

impl AfskGenerator {
    /// The signal of the settings `v` at `rate_hz` with the random bits of `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> AfskGenerator {
        let dev = v.get(&GEN_DEVIATION_HZ);
        let twist = 10f64.powf(v.get(&GEN_TWIST_DB) / 20.0);
        let (mark_dev, space_dev) = if twist <= 1.0 {
            (dev, dev * twist)
        } else {
            (dev / twist, dev)
        };
        AfskGenerator {
            rate: rate_hz,
            mark_dev,
            space_dev,
            baud: v.get(&GEN_BAUD),
            period: v.get(&GEN_PERIOD_S),
            burst: v.get(&GEN_BURST_S),
            first: v.get(&GEN_FIRST_S),
            rng: Rng::stream(seed, 43),
            bit: -1,
            space: false,
            tone_phase: 0.0,
            fm: FmTones::default(),
            n: 0,
        }
    }

    fn in_burst(&self, t: f64) -> bool {
        if t < self.first {
            return false;
        }
        let u = if self.period > 0.0 {
            (t - self.first) % self.period
        } else {
            t - self.first
        };
        u < self.burst
    }
}

impl Generator for AfskGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let f = if self.in_burst(t) {
                let bit = (t * self.baud).floor() as i64;
                if bit != self.bit {
                    self.bit = bit;
                    if self.rng.bit() == 1 {
                        self.space = !self.space;
                    }
                }
                let (tone, dev) = if self.space {
                    (SPACE_HZ, self.space_dev)
                } else {
                    (MARK_HZ, self.mark_dev)
                };
                self.tone_phase = (self.tone_phase + TAU * tone / self.rate).rem_euclid(TAU);
                dev * self.tone_phase.sin()
            } else {
                0.0
            };
            *z = self.fm.next(f, self.rate, 1.0);
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

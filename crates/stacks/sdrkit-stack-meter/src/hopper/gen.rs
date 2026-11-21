//! The hopper test signal: one frequency hopper that picks a random channel of an evenly spaced set for each dwell, sends
//! binary FSK inside the dwell, and is silent for the guard at the end of each dwell. The hop clock can have jitter; a
//! clock error of the whole signal comes from the standard impairments (`gen_clock_ppm`). Without channels the signal is
//! silent. The noise reference is a carrier of power 1 in 25 kHz.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The channels of the hopper.
pub const GEN_CHANNELS: Setting<field::I64> = Setting::new("gen_channels", "Channels", "", 16)
    .range(0.0, 256.0, 1.0)
    .section(SECTION);
/// The channel spacing.
pub const GEN_SPACING_HZ: Setting<field::F64> =
    Setting::new("gen_spacing_hz", "Channel spacing", "Hz", 50e3)
        .range(1e3, 1e7, 0.0)
        .section(SECTION);
/// The dwell, guard included.
pub const GEN_DWELL_S: Setting<field::F64> = Setting::new("gen_dwell_s", "Dwell", "s", 0.002)
    .range(1e-4, 10.0, 0.0)
    .section(SECTION);
/// The silence at the end of each dwell.
pub const GEN_GUARD_S: Setting<field::F64> = Setting::new("gen_guard_s", "Guard", "s", 0.0003)
    .range(0.0, 10.0, 0.0)
    .section(SECTION);
/// The jitter of the hop clock: each dwell is longer or shorter by up to this share.
pub const GEN_JITTER: Setting<field::F64> = Setting::new("gen_jitter", "Hop clock jitter", "", 0.0)
    .range(0.0, 0.5, 0.0)
    .section(SECTION);
/// The FSK deviation.
pub const GEN_FSK_HZ: Setting<field::F64> =
    Setting::new("gen_fsk_hz", "FSK deviation", "Hz", 3000.0)
        .range(0.0, 1e6, 0.0)
        .section(SECTION);
/// The FSK bit rate.
pub const GEN_FSK_BPS: Setting<field::F64> =
    Setting::new("gen_fsk_bps", "FSK bit rate", "bit/s", 10_000.0)
        .range(1.0, 1e7, 0.0)
        .section(SECTION);

/// The band that the noise of the impairments is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 25e3;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct HopperGenerator {
    rate: f64,
    channels: usize,
    spacing: f64,
    dwell: f64,
    guard: f64,
    jitter: f64,
    fsk_hz: f64,
    bit_s: f64,
    rng: Rng,
    t: f64,
    dwell_end: f64,
    channel_hz: f64,
    bit_end: f64,
    bit: f64,
    phase: f64,
}

impl HopperGenerator {
    /// The hopper of the settings `v` at `rate_hz`, its channels and bits from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> HopperGenerator {
        HopperGenerator {
            rate: rate_hz,
            channels: v.get(&GEN_CHANNELS).max(0) as usize,
            spacing: v.get(&GEN_SPACING_HZ),
            dwell: v.get(&GEN_DWELL_S),
            guard: v.get(&GEN_GUARD_S),
            jitter: v.get(&GEN_JITTER),
            fsk_hz: v.get(&GEN_FSK_HZ),
            bit_s: 1.0 / v.get(&GEN_FSK_BPS),
            rng: Rng::new(seed ^ 0x686f_7070_6572),
            t: 0.0,
            dwell_end: 0.0,
            channel_hz: 0.0,
            bit_end: 0.0,
            bit: 1.0,
            phase: 0.0,
        }
    }
}

impl Generator for HopperGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let step = 1.0 / self.rate;
        for z in out.iter_mut() {
            *z = Complex32::default();
            if self.channels > 0 {
                if self.t >= self.dwell_end {
                    let len = self.dwell * (1.0 + self.jitter * (2.0 * self.rng.uniform() - 1.0));
                    self.dwell_end += len;
                    let k = (self.rng.uniform() * self.channels as f64) as usize;
                    self.channel_hz = self.spacing
                        * (k.min(self.channels - 1) as f64 - 0.5 * (self.channels as f64 - 1.0));
                }
                if self.t >= self.bit_end {
                    self.bit_end += self.bit_s;
                    self.bit = if self.rng.uniform() < 0.5 { -1.0 } else { 1.0 };
                }
                if self.t < self.dwell_end - self.guard {
                    *z = Complex32::from_polar(1.0, self.phase as f32);
                }
                let f = self.channel_hz + self.bit * self.fsk_hz;
                self.phase = (self.phase + TAU * f * step).rem_euclid(TAU);
            }
            self.t += step;
        }
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

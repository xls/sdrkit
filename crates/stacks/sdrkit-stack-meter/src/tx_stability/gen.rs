//! The test signal of `tx_stability`: a carrier of amplitude 1 keyed on and off with linear ramps, whose frequency during a
//! transmission is `final + start_offset exp(-u / tau) + drift u + mod sin(2 pi 1000 t)`, `u` the time since key-up. The
//! offset of the whole signal and the noise are the standard impairments; an SNR is the CNR in 16 kHz.

use crate::ptt_log::gen::parse_spans;
use crate::txtest::FmTones;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The default transmissions: from s, to s.
pub const DEFAULT_KEYED: &str = "4, 10; 14, 14.4; 18, 22";

/// The transmissions.
pub const GEN_KEYED: Setting<field::Text> =
    Setting::new("gen_keyed", "Transmissions", "", DEFAULT_KEYED)
        .section(SECTION)
        .help("Transmissions separated by ';', each 'from s, to s'.");
/// The final frequency.
pub const GEN_FINAL_HZ: Setting<field::F64> =
    Setting::new("gen_final_hz", "Final frequency", "Hz", 500.0)
        .range(-1e5, 1e5, 0.0)
        .section(SECTION);
/// The start offset over the final frequency.
pub const GEN_START_OFFSET_HZ: Setting<field::F64> =
    Setting::new("gen_start_offset_hz", "Start offset", "Hz", 250.0)
        .range(-1e5, 1e5, 0.0)
        .section(SECTION);
/// The time constant of the settling.
pub const GEN_TAU_S: Setting<field::F64> =
    Setting::new("gen_tau_s", "Settling time constant", "s", 0.15)
        .range(0.001, 100.0, 0.0)
        .section(SECTION);
/// The drift during a transmission.
pub const GEN_TX_DRIFT_HZ_S: Setting<field::F64> =
    Setting::new("gen_tx_drift_hz_s", "Drift in a transmission", "Hz/s", 0.0)
        .range(-1e4, 1e4, 0.0)
        .section(SECTION);
/// The deviation of a 1 kHz modulating tone.
pub const GEN_MOD_HZ: Setting<field::F64> =
    Setting::new("gen_mod_hz", "1 kHz tone deviation", "Hz", 0.0)
        .range(0.0, 10_000.0, 0.0)
        .section(SECTION);
/// The length of the key ramps.
pub const GEN_RAMP_S: Setting<field::F64> = Setting::new("gen_ramp_s", "Key ramp", "s", 0.004)
    .range(0.0, 1.0, 0.0)
    .section(SECTION);

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 16e3;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct TxStabilityGenerator {
    rate: f64,
    keyed: Vec<(f64, f64)>,
    final_hz: f64,
    start_hz: f64,
    tau: f64,
    drift: f64,
    mod_hz: f64,
    ramp: f64,
    fm: FmTones,
    n: u64,
}

impl TxStabilityGenerator {
    /// The signal of the settings `v` at `rate_hz`.
    pub fn new(rate_hz: f64, v: &Values) -> TxStabilityGenerator {
        TxStabilityGenerator {
            rate: rate_hz,
            keyed: parse_spans(v.get(&GEN_KEYED)).unwrap_or_default(),
            final_hz: v.get(&GEN_FINAL_HZ),
            start_hz: v.get(&GEN_START_OFFSET_HZ),
            tau: v.get(&GEN_TAU_S),
            drift: v.get(&GEN_TX_DRIFT_HZ_S),
            mod_hz: v.get(&GEN_MOD_HZ),
            ramp: v.get(&GEN_RAMP_S),
            fm: FmTones::default(),
            n: 0,
        }
    }
}

impl Generator for TxStabilityGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let Some(&(a, b)) = self.keyed.iter().find(|&&(a, b)| t >= a && t < b) else {
                *z = Complex32::default();
                continue;
            };
            let u = t - a;
            let gain = if self.ramp > 0.0 {
                (u / self.ramp).min((b - t) / self.ramp).min(1.0)
            } else {
                1.0
            };
            let f = self.final_hz
                + self.start_hz * (-u / self.tau).exp()
                + self.drift * u
                + self.mod_hz * (TAU * 1000.0 * t).sin();
            *z = self.fm.next(f, self.rate, gain);
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

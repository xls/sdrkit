//! The test signal of `noise_figure`: the receiver's noise `P0` times a gain drift `10^(d t / 10)`, and in every odd period
//! the noise of a source that adds `P0 (10^(Y / 10) - 1)`. The noise is the measured signal itself.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};

const SECTION: &str = "Test signal";

/// The step of the source.
pub const GEN_Y_DB: Setting<field::F64> = Setting::new("gen_y_db", "Y-factor", "dB", 10.0)
    .range(0.0, 40.0, 0.0)
    .section(SECTION);
/// The time between switchings of the source.
pub const GEN_PERIOD_S: Setting<field::F64> =
    Setting::new("gen_period_s", "Source toggles every", "s", 10.0)
        .range(0.5, 1000.0, 0.0)
        .section(SECTION);
/// The gain drift of the receiver.
pub const GEN_DRIFT_DB_S: Setting<field::F64> =
    Setting::new("gen_drift_db_s", "Gain drift", "dB/s", 0.01)
        .range(-1.0, 1.0, 0.0)
        .section(SECTION);
/// The receiver's noise power per sample.
pub const GEN_P0: Setting<field::F64> = Setting::new("gen_p0", "Receiver noise", "", 1.0)
    .range(1e-12, 4.0, 0.0)
    .section(SECTION);

/// The test signal generator.
pub struct NoiseFigureGenerator {
    rate: f64,
    y: f64,
    period: f64,
    drift: f64,
    p0: f64,
    rng: Rng,
    n: u64,
}

impl NoiseFigureGenerator {
    /// The signal of the settings `v` at `rate_hz`, its noise drawn from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> NoiseFigureGenerator {
        NoiseFigureGenerator {
            rate: rate_hz,
            y: 10f64.powf(v.get(&GEN_Y_DB) / 10.0),
            period: v.get(&GEN_PERIOD_S),
            drift: v.get(&GEN_DRIFT_DB_S),
            p0: v.get(&GEN_P0),
            rng: Rng::new(seed ^ 0x6e66),
            n: 0,
        }
    }
}

impl Generator for NoiseFigureGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let on = ((t / self.period).floor() as u64) % 2 == 1;
            let p = self.p0 * 10f64.powf(self.drift * t / 10.0) * if on { self.y } else { 1.0 };
            *z = self.rng.complex_normal(p);
            self.n += 1;
        }
        out.len()
    }
    /// The noise is the signal; it passes the impairments at its own power.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

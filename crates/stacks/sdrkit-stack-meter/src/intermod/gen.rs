//! The test signal of `intermod`: two tones whose levels swing as `swing sin(2 pi r1 t)` and `swing sin(2 pi r2 t + 1)` dB,
//! through a receiver `y = x + k x |x|^2` whose cubic term makes the third-order products, and an optional steady tone
//! after the receiver. For this receiver the product at `2 f1 - f2` stands `k a^2` under the weaker tone in amplitude. The
//! noise and the shift of the scene are those of the standard impairments.

use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The first tone.
pub const GEN_F1_HZ: Setting<field::F64> = Setting::new("gen_f1_hz", "First tone", "Hz", 120_000.0)
    .range(-1e7, 1e7, 0.0)
    .section(SECTION);
/// The second tone.
pub const GEN_F2_HZ: Setting<field::F64> =
    Setting::new("gen_f2_hz", "Second tone", "Hz", 190_000.0)
        .range(-1e7, 1e7, 0.0)
        .section(SECTION);
/// Their amplitude.
pub const GEN_AMP: Setting<field::F64> = Setting::new("gen_amp", "Tone amplitude", "", 0.2)
    .range(0.0, 1.0, 0.0)
    .section(SECTION);
/// Their swing.
pub const GEN_SWING_DB: Setting<field::F64> =
    Setting::new("gen_swing_db", "Level swing", "dB", 5.0)
        .range(0.0, 20.0, 0.0)
        .section(SECTION);
/// The rates of the swings.
pub const GEN_SWING_HZ: Setting<field::Text> =
    Setting::new("gen_swing_hz", "Swing rates", "Hz", "0.071, 0.113")
        .section(SECTION)
        .help("The rates of the two swings separated by ','.");
/// The cubic term of the receiver.
pub const GEN_K: Setting<field::F64> = Setting::new("gen_k", "Cubic term of the receiver", "", 0.8)
    .range(0.0, 10.0, 0.0)
    .section(SECTION);
/// A steady independent tone.
pub const GEN_EXTRA_HZ: Setting<field::F64> =
    Setting::new("gen_extra_hz", "Independent tone", "Hz", 0.0)
        .range(-1e7, 1e7, 0.0)
        .section(SECTION);
/// Its amplitude (0: none).
pub const GEN_EXTRA_AMP: Setting<field::F64> =
    Setting::new("gen_extra_amp", "Independent tone amplitude", "", 0.0)
        .range(0.0, 1.0, 0.0)
        .section(SECTION);

/// The test signal generator.
pub struct IntermodGenerator {
    rate: f64,
    f: [f64; 2],
    amp: f64,
    swing: f64,
    rates: [f64; 2],
    k: f32,
    extra: (f64, f64),
    n: u64,
}

impl IntermodGenerator {
    /// The signal of the settings `v` at `rate_hz`.
    pub fn new(rate_hz: f64, v: &Values) -> IntermodGenerator {
        let r: Vec<f64> = v
            .get(&GEN_SWING_HZ)
            .split(',')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
        IntermodGenerator {
            rate: rate_hz,
            f: [v.get(&GEN_F1_HZ), v.get(&GEN_F2_HZ)],
            amp: v.get(&GEN_AMP),
            swing: v.get(&GEN_SWING_DB),
            rates: [
                r.first().copied().unwrap_or(0.071),
                r.get(1).copied().unwrap_or(0.113),
            ],
            k: v.get(&GEN_K) as f32,
            extra: (v.get(&GEN_EXTRA_HZ), v.get(&GEN_EXTRA_AMP)),
            n: 0,
        }
    }
}

impl Generator for IntermodGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let mut x = Complex32::default();
            for (j, (f, ph0)) in self.f.iter().zip([0.0, 1.0]).enumerate() {
                let db = self.swing * (TAU * self.rates[j] * t + ph0).sin();
                let a = self.amp * 10f64.powf(db / 20.0);
                x += Complex32::from_polar(a as f32, (TAU * f * t).rem_euclid(TAU) as f32);
            }
            let mut y = x + x * (self.k * x.norm_sqr());
            if self.extra.1 > 0.0 {
                y += Complex32::from_polar(
                    self.extra.1 as f32,
                    (TAU * self.extra.0 * t).rem_euclid(TAU) as f32,
                );
            }
            *z = y;
            self.n += 1;
        }
        out.len()
    }
    /// The noise of the impairments is stated against a power of 1.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

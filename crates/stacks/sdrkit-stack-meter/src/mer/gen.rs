//! The test signal of `mer`: M-PSK of random symbols with root-raised-cosine pulses at unit power, the points on the axes or
//! turned by `pi / M`. The offset, the drift, the clock error and the noise are the standard impairments; an Es/N0 is stated
//! for the symbol rate.

use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::{PI, TAU};

const SECTION: &str = "Test signal";

/// The number of phases (2, 4 or 8).
pub const GEN_PSK: Setting<field::I64> = Setting::new("gen_psk", "Phases", "", 4)
    .range(2.0, 8.0, 1.0)
    .section(SECTION);
/// The symbol rate.
pub const GEN_SYMBOL_RATE: Setting<field::F64> =
    Setting::new("gen_symbol_rate", "Symbol rate", "Bd", 9600.0)
        .range(10.0, 1e6, 0.0)
        .section(SECTION);
/// The roll-off of the pulses.
pub const GEN_ROLLOFF: Setting<field::F64> = Setting::new("gen_rolloff", "Roll-off", "", 0.35)
    .range(0.05, 1.0, 0.0)
    .section(SECTION);
/// The points turned by `pi / M`.
pub const GEN_ROTATED: Setting<field::Bool> =
    Setting::new("gen_rotated", "Points turned by pi / M", "", false).section(SECTION);

/// The span of the pulses, symbols.
pub const SPAN: f64 = 16.0;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct MerGenerator {
    rate: f64,
    m: u64,
    baud: f64,
    rot: f64,
    rng: Rng,
    modu: Option<Linear>,
    buf: Vec<Complex32>,
    read: usize,
}

impl MerGenerator {
    /// The signal of the settings `v` at `rate_hz` with the random symbols of `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> MerGenerator {
        let m = match v.get(&GEN_PSK) {
            ..=2 => 2,
            3..=4 => 4,
            _ => 8,
        };
        let baud = v.get(&GEN_SYMBOL_RATE);
        MerGenerator {
            rate: rate_hz,
            m,
            baud,
            rot: if v.get(&GEN_ROTATED) {
                PI / m as f64
            } else {
                0.0
            },
            rng: Rng::stream(seed, 59),
            modu: Linear::rrc(v.get(&GEN_ROLLOFF), SPAN, rate_hz / baud).ok(),
            buf: Vec::new(),
            read: 0,
        }
    }
}

impl Generator for MerGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let Some(modu) = self.modu.as_mut() else {
            out.fill(Complex32::default());
            return out.len();
        };
        let mut filled = 0;
        let mut sym = Vec::with_capacity(64);
        while filled < out.len() {
            if self.read == self.buf.len() {
                self.buf.clear();
                self.read = 0;
                sym.clear();
                for _ in 0..64 {
                    let k = self.rng.below(self.m) as f64;
                    sym.push(Complex32::from_polar(
                        1.0,
                        (TAU * k / self.m as f64 + self.rot) as f32,
                    ));
                }
                modu.push(&sym, &mut self.buf);
            }
            let n = (self.buf.len() - self.read).min(out.len() - filled);
            out[filled..filled + n].copy_from_slice(&self.buf[self.read..self.read + n]);
            self.read += n;
            filled += n;
        }
        out.len()
    }
    /// Unit power; an Es/N0 is stated at the symbol rate and an SNR in the occupied band.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(self.baud)
                .with_power(1.0)
                .with_symbol_rate(self.baud),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

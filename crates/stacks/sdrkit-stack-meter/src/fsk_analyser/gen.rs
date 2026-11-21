//! The test signal of `fsk_analyser`: an FM carrier of amplitude 1 whose frequency is FSK or GFSK of random symbols: two
//! levels at `+-deviation` or four at `+-deviation / 3` and `+-deviation`, each symbol a rectangle of one symbol filtered
//! by a Gaussian of `sigma = sqrt(ln 2) / (2 pi BT) T`, or of `0.12 T` for plain FSK (BT 0). The offset, the clock error
//! and the noise are the standard impairments; an SNR is the CNR in 50 kHz.

use crate::txtest::FmTones;
use sdrkit_algo_dsp::pulse::erf;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::collections::VecDeque;
use std::f64::consts::{LN_2, PI, SQRT_2};

const SECTION: &str = "Test signal";

/// The number of levels (2 or 4).
pub const GEN_LEVELS: Setting<field::I64> = Setting::new("gen_levels", "Levels", "", 2)
    .range(2.0, 4.0, 2.0)
    .section(SECTION);
/// The symbol rate.
pub const GEN_BAUD: Setting<field::F64> = Setting::new("gen_baud", "Symbol rate", "Bd", 9600.0)
    .range(10.0, 100_000.0, 0.0)
    .section(SECTION);
/// The peak deviation.
pub const GEN_DEVIATION_HZ: Setting<field::F64> =
    Setting::new("gen_deviation_hz", "Deviation", "Hz", 2400.0)
        .range(0.0, 50_000.0, 0.0)
        .section(SECTION);
/// The Gaussian BT (0 for plain FSK).
pub const GEN_BT: Setting<field::F64> = Setting::new("gen_bt", "Gaussian BT", "", 0.5)
    .range(0.0, 2.0, 0.0)
    .section(SECTION);

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 50e3;

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct FskGenerator {
    rate: f64,
    levels: i64,
    baud: f64,
    dev: f64,
    sigma: f64,
    rng: Rng,
    symbols: VecDeque<(i64, f64)>,
    fm: FmTones,
    n: u64,
}

impl FskGenerator {
    /// The signal of the settings `v` at `rate_hz` with the random symbols of `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> FskGenerator {
        let baud = v.get(&GEN_BAUD);
        let bt = v.get(&GEN_BT);
        let t = 1.0 / baud;
        FskGenerator {
            rate: rate_hz,
            levels: if v.get(&GEN_LEVELS) >= 4 { 4 } else { 2 },
            baud,
            dev: v.get(&GEN_DEVIATION_HZ),
            sigma: if bt > 0.0 {
                LN_2.sqrt() / (2.0 * PI * bt) * t
            } else {
                0.12 * t
            },
            rng: Rng::stream(seed, 53),
            symbols: VecDeque::new(),
            fm: FmTones::default(),
            n: 0,
        }
    }

    fn level(&mut self) -> f64 {
        match self.levels {
            4 => [-1.0, -1.0 / 3.0, 1.0 / 3.0, 1.0][self.rng.below(4) as usize],
            _ => [-1.0, 1.0][self.rng.below(2) as usize],
        }
    }
}

impl Generator for FskGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let t_sym = 1.0 / self.baud;
        let reach = (0.5 * t_sym + 5.0 * self.sigma) * self.baud;
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let k = t * self.baud;
            let last = (k + reach).ceil() as i64;
            while self.symbols.back().is_none_or(|s| s.0 < last) {
                let next = self
                    .symbols
                    .back()
                    .map_or((k - reach).floor() as i64, |s| s.0 + 1);
                let a = self.level();
                self.symbols.push_back((next, a));
            }
            while self
                .symbols
                .front()
                .is_some_and(|s| (s.0 as f64) < k - reach - 1.0)
            {
                self.symbols.pop_front();
            }
            let d = self.sigma * SQRT_2;
            let f: f64 = self
                .symbols
                .iter()
                .map(|&(j, a)| {
                    let c = (j as f64 + 0.5) * t_sym;
                    a * 0.5 * (erf((t - c + 0.5 * t_sym) / d) - erf((t - c - 0.5 * t_sym) / d))
                })
                .sum();
            *z = self.fm.next(self.dev * f, self.rate, 1.0);
        }
        out.len()
    }
    /// The noise is stated against a carrier of power 1; an SNR is stated in 50 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(NOISE_BAND_HZ)
                .with_power(1.0)
                .with_symbol_rate(self.baud),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

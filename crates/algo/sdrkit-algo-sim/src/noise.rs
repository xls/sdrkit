//! Additive white Gaussian noise, with its level stated the way a standard states it, and the conversions between those ways.
//!
//! # Definitions
//!
//! Every level comes down to `N0`, the one-sided noise power spectral density (power per hertz, in the same units as the signal
//! power `C`; with sdrkit's full-scale convention a mean power of 1 is 0 dBFS):
//!
//! | Stated as | Meaning | `N0` |
//! |---|---|---|
//! | SNR in a bandwidth `B` (C/N when `B` is the channel bandwidth) | `C / (N0 B)` | `C / (B 10^(SNR/10))` |
//! | C/N0 (dB-Hz) | `C / N0` | `C / 10^(CN0/10)` |
//! | Es/N0 at `Rs` symbols per second | `(C / Rs) / N0` | `C / (Rs 10^(EsN0/10))` |
//! | Eb/N0 at `Rb` information bits per second | `(C / Rb) / N0` | `C / (Rb 10^(EbN0/10))` |
//! | a floor: total noise power over the sample rate, dBFS | (absolute) | `10^(dBFS/10) / fs` (complex) |
//!
//! So `C/N0 = SNR + 10 log10(B) = Es/N0 + 10 log10(Rs) = Eb/N0 + 10 log10(Rb)`, and `Es/N0 = Eb/N0 + 10 log10(bits per symbol)`
//! where "bits per symbol" counts information bits, so it includes the code rate (QPSK at rate 1/2 carries 1 bit per symbol).
//!
//! **Complex and real noise.** For complex baseband at `fs` samples per second the noise occupies `fs` hertz, so its total power is
//! `N0 fs`, split equally between I and Q. For a real signal (audio) at `fs` the noise occupies `0..fs/2`, so its power is
//! `N0 fs / 2`. Either way, the noise in any band `B` inside the signal's spectrum is `N0 B`, which is what the standards measure.

use crate::rng::Rng;
use crate::{Param, SimError};
use sdrkit_core::Complex32;

/// How a noise level is stated (see the module documentation for the definitions).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoiseLevel {
    /// No noise.
    Off,
    /// Signal-to-noise ratio in dB, the noise measured in `bandwidth_hz` (carrier-to-noise ratio C/N when that is the channel
    /// bandwidth).
    SnrDb {
        /// The ratio, dB.
        snr_db: f64,
        /// The bandwidth the noise is measured in, Hz.
        bandwidth_hz: f64,
    },
    /// Carrier-to-noise-density ratio C/N0, dB-Hz.
    Cn0DbHz(f64),
    /// Energy per symbol over `N0`, dB, at `symbol_rate_hz` symbols per second.
    EsN0Db {
        /// The ratio, dB.
        esn0_db: f64,
        /// Symbols per second.
        symbol_rate_hz: f64,
    },
    /// Energy per information bit over `N0`, dB, at `bit_rate_hz` information bits per second.
    EbN0Db {
        /// The ratio, dB.
        ebn0_db: f64,
        /// Information bits per second.
        bit_rate_hz: f64,
    },
    /// An absolute floor, independent of the signal: the total noise power over the whole sample rate, dBFS (complex samples).
    FloorDbfs(f64),
}

impl NoiseLevel {
    /// The one-sided noise density `N0` for a signal of mean power `signal_power`, and a sample rate `rate_hz` (needed only for
    /// [`FloorDbfs`](NoiseLevel::FloorDbfs)). 0 for [`Off`](NoiseLevel::Off). Errors for non-finite levels, or a bandwidth or rate
    /// that is not positive.
    pub fn n0(&self, signal_power: f64, rate_hz: f64) -> Result<f64, SimError> {
        let fin = |x: f64| -> Result<f64, SimError> {
            if x.is_finite() {
                Ok(x)
            } else {
                Param::NoiseLevel.not_finite()
            }
        };
        let c = if signal_power >= 0.0 && signal_power.is_finite() {
            signal_power
        } else {
            return Param::SignalPower.at_least(0.0, signal_power);
        };
        let db = |x: f64| 10f64.powf(x / 10.0);
        Ok(match *self {
            NoiseLevel::Off => 0.0,
            NoiseLevel::SnrDb {
                snr_db,
                bandwidth_hz,
            } => c / (Param::Bandwidth.check_positive(bandwidth_hz)? * db(fin(snr_db)?)),
            NoiseLevel::Cn0DbHz(x) => c / db(fin(x)?),
            NoiseLevel::EsN0Db {
                esn0_db,
                symbol_rate_hz,
            } => c / (Param::SymbolRate.check_positive(symbol_rate_hz)? * db(fin(esn0_db)?)),
            NoiseLevel::EbN0Db {
                ebn0_db,
                bit_rate_hz,
            } => c / (Param::BitRate.check_positive(bit_rate_hz)? * db(fin(ebn0_db)?)),
            NoiseLevel::FloorDbfs(x) => db(fin(x)?) / Param::Rate.check_positive(rate_hz)?,
        })
    }
}

/// `C/N0` (dB-Hz) from an SNR (dB) measured in `bandwidth_hz`.
pub fn cn0_from_snr(snr_db: f64, bandwidth_hz: f64) -> f64 {
    snr_db + 10.0 * bandwidth_hz.log10()
}

/// The SNR (dB) in `bandwidth_hz` from `C/N0` (dB-Hz).
pub fn snr_from_cn0(cn0_dbhz: f64, bandwidth_hz: f64) -> f64 {
    cn0_dbhz - 10.0 * bandwidth_hz.log10()
}

/// `Es/N0` (dB) from `C/N0` at `symbol_rate_hz`.
pub fn esn0_from_cn0(cn0_dbhz: f64, symbol_rate_hz: f64) -> f64 {
    cn0_dbhz - 10.0 * symbol_rate_hz.log10()
}

/// `C/N0` (dB-Hz) from `Es/N0` at `symbol_rate_hz`.
pub fn cn0_from_esn0(esn0_db: f64, symbol_rate_hz: f64) -> f64 {
    esn0_db + 10.0 * symbol_rate_hz.log10()
}

/// `Es/N0` (dB) from `Eb/N0` with `bits_per_symbol` information bits per symbol (modulation bits times code rate).
pub fn esn0_from_ebn0(ebn0_db: f64, bits_per_symbol: f64) -> f64 {
    ebn0_db + 10.0 * bits_per_symbol.log10()
}

/// `Eb/N0` (dB) from `Es/N0` with `bits_per_symbol` information bits per symbol.
pub fn ebn0_from_esn0(esn0_db: f64, bits_per_symbol: f64) -> f64 {
    esn0_db - 10.0 * bits_per_symbol.log10()
}

/// The SNR (dB) in `bandwidth_hz` from `Es/N0` at `symbol_rate_hz`: `Es/N0 + 10 log10(Rs / B)`.
pub fn snr_from_esn0(esn0_db: f64, symbol_rate_hz: f64, bandwidth_hz: f64) -> f64 {
    esn0_db + 10.0 * (symbol_rate_hz / bandwidth_hz).log10()
}

/// Additive white Gaussian noise, seeded and repeatable, for complex or real samples.
///
/// ```
/// use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
/// // A unit-power signal at 10 dB SNR in 10 kHz, sampled at 100 kHz: the noise power over the whole rate is 0.1 * 10 = 1.
/// let n = Awgn::complex(NoiseLevel::SnrDb { snr_db: 10.0, bandwidth_hz: 10e3 }, 1.0, 100e3, 1).unwrap();
/// assert!((n.power() - 1.0).abs() < 1e-12);
/// ```
#[derive(Clone, Debug)]
pub struct Awgn {
    rng: Rng,
    sigma: f32,
    power: f64,
}

impl Awgn {
    /// Noise of total power `power` (mean `|n|^2` for complex samples, mean `n^2` for real ones).
    pub fn with_power(power: f64, seed: u64) -> Result<Self, SimError> {
        if !(power >= 0.0 && power.is_finite()) {
            return Param::NoisePower.at_least(0.0, power);
        }
        Ok(Awgn {
            rng: Rng::new(seed),
            sigma: power.sqrt() as f32,
            power,
        })
    }

    /// Noise for complex samples at `rate_hz` at `level`, relative to a signal of mean power `signal_power`: total power `N0 fs`.
    pub fn complex(
        level: NoiseLevel,
        signal_power: f64,
        rate_hz: f64,
        seed: u64,
    ) -> Result<Self, SimError> {
        let n0 = level.n0(signal_power, rate_hz)?;
        if !(rate_hz > 0.0 && rate_hz.is_finite()) {
            return Param::Rate.positive(rate_hz);
        }
        Self::with_power(n0 * rate_hz, seed)
    }

    /// Noise for real samples at `rate_hz` at `level`: total power `N0 fs / 2` (the noise fills `0..fs/2`).
    pub fn real(
        level: NoiseLevel,
        signal_power: f64,
        rate_hz: f64,
        seed: u64,
    ) -> Result<Self, SimError> {
        let n0 = level.n0(signal_power, rate_hz)?;
        if !(rate_hz > 0.0 && rate_hz.is_finite()) {
            return Param::Rate.positive(rate_hz);
        }
        Self::with_power(n0 * rate_hz / 2.0, seed)
    }

    /// The total noise power.
    pub fn power(&self) -> f64 {
        self.power
    }

    /// Adds complex noise to `buf` (each of I and Q gets half the power).
    pub fn process(&mut self, buf: &mut [Complex32]) {
        if self.power == 0.0 {
            return;
        }
        let s = self.sigma * std::f32::consts::FRAC_1_SQRT_2;
        for x in buf {
            x.re += self.rng.normal_f32() * s;
            x.im += self.rng.normal_f32() * s;
        }
    }

    /// Adds real noise to `buf`.
    pub fn process_real(&mut self, buf: &mut [f32]) {
        if self.power == 0.0 {
            return;
        }
        for x in buf {
            *x += self.rng.normal_f32() * self.sigma;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power(x: &[Complex32]) -> f64 {
        x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64
    }

    #[test]
    fn every_way_of_stating_the_level_returns_the_same_density() {
        // C = 2 (3 dBFS). SNR 10 dB in 10 kHz <=> C/N0 50 dB-Hz <=> Es/N0 10 dB at 10 kBd <=> Eb/N0 = 10 - 10 log10(2) at 2
        // bits per symbol.
        let c = 2.0;
        let n0 = NoiseLevel::SnrDb {
            snr_db: 10.0,
            bandwidth_hz: 10e3,
        }
        .n0(c, 1e6)
        .unwrap();
        assert!((n0 - 2e-5).abs() < 1e-18);
        let cn0 = cn0_from_snr(10.0, 10e3);
        assert!((cn0 - 50.0).abs() < 1e-12);
        assert!((NoiseLevel::Cn0DbHz(cn0).n0(c, 1e6).unwrap() - n0).abs() < 1e-18);
        let esn0 = esn0_from_cn0(cn0, 10e3);
        assert!((esn0 - 10.0).abs() < 1e-12);
        let es = NoiseLevel::EsN0Db {
            esn0_db: esn0,
            symbol_rate_hz: 10e3,
        };
        assert!((es.n0(c, 1e6).unwrap() - n0).abs() < 1e-18);
        let ebn0 = ebn0_from_esn0(esn0, 2.0);
        assert!((ebn0 - (10.0 - 10.0 * 2f64.log10())).abs() < 1e-12);
        let eb = NoiseLevel::EbN0Db {
            ebn0_db: ebn0,
            bit_rate_hz: 20e3,
        };
        assert!((eb.n0(c, 1e6).unwrap() - n0).abs() < 1e-17);
        assert!((esn0_from_ebn0(ebn0, 2.0) - esn0).abs() < 1e-12);
        assert!((cn0_from_esn0(esn0, 10e3) - cn0).abs() < 1e-12);
        assert!((snr_from_cn0(cn0, 10e3) - 10.0).abs() < 1e-12);
        assert!((snr_from_esn0(10.0, 10e3, 5e3) - (10.0 + 10.0 * 2f64.log10())).abs() < 1e-12);
        // A floor of -30 dBFS over 1 MHz.
        assert!((NoiseLevel::FloorDbfs(-30.0).n0(c, 1e6).unwrap() - 1e-9).abs() < 1e-22);
        assert_eq!(NoiseLevel::Off.n0(c, 1e6).unwrap(), 0.0);
        // Bad values are refused.
        assert!(NoiseLevel::SnrDb {
            snr_db: 1.0,
            bandwidth_hz: 0.0
        }
        .n0(1.0, 1.0)
        .is_err());
        assert!(NoiseLevel::Cn0DbHz(f64::NAN).n0(1.0, 1.0).is_err());
        assert!(NoiseLevel::Cn0DbHz(1.0).n0(-1.0, 1.0).is_err());
        assert!(NoiseLevel::FloorDbfs(0.0).n0(1.0, 0.0).is_err());
    }

    #[test]
    fn measured_noise_power_matches_the_setting() {
        // Complex: 20 dB SNR in 25 kHz at 250 kHz for unit power: total 0.01 * 10 = 0.1.
        let mut n = Awgn::complex(
            NoiseLevel::SnrDb {
                snr_db: 20.0,
                bandwidth_hz: 25e3,
            },
            1.0,
            250e3,
            9,
        )
        .unwrap();
        assert!((n.power() - 0.1).abs() < 1e-15);
        let mut x = vec![Complex32::default(); 400_000];
        n.process(&mut x);
        let p = power(&x);
        assert!((10.0 * (p / 0.1).log10()).abs() < 0.03, "{p}");
        let pi = x.iter().map(|v| (v.re as f64).powi(2)).sum::<f64>() / x.len() as f64;
        assert!((pi / p - 0.5).abs() < 0.005, "I carries half");
        // Real: the same N0 produces half the power.
        let mut r = Awgn::real(
            NoiseLevel::SnrDb {
                snr_db: 20.0,
                bandwidth_hz: 25e3,
            },
            1.0,
            250e3,
            9,
        )
        .unwrap();
        assert!((r.power() - 0.05).abs() < 1e-15);
        let mut y = vec![0f32; 400_000];
        r.process_real(&mut y);
        let pr = y.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / y.len() as f64;
        assert!((10.0 * (pr / 0.05).log10()).abs() < 0.03);
        assert!(Awgn::with_power(-1.0, 0).is_err());
    }

    #[test]
    fn noise_is_repeatable_and_independent_of_the_cuts() {
        let mut a = Awgn::with_power(0.5, 5).unwrap();
        let mut b = Awgn::with_power(0.5, 5).unwrap();
        let mut x1 = vec![Complex32::default(); 1000];
        a.process(&mut x1);
        let mut x2 = vec![Complex32::default(); 1000];
        for c in x2.chunks_mut(37) {
            b.process(c);
        }
        assert_eq!(x1, x2);
    }
}

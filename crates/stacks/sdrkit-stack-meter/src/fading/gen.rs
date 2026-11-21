//! The test signal of `fading`: a carrier of power 1 through a flat channel of the classical Doppler spectrum of a largest
//! Doppler `fd`, with a direct path of Rice factor K (the fader of `sdrkit-algo-sim::fading`; K = 0 is Rayleigh fading), and
//! no carrier in stated spans. The offset, the drift and the noise are those of the standard impairments; an SNR is the CNR
//! in 1 kHz.

use crate::ptt_log::gen::parse_spans;
use sdrkit_algo_sim::fading::{Doppler, Fader};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};

const SECTION: &str = "Test signal";

/// The largest Doppler.
pub const GEN_FD_HZ: Setting<field::F64> = Setting::new("gen_fd_hz", "Largest Doppler", "Hz", 5.0)
    .range(0.01, 200.0, 0.0)
    .section(SECTION);
/// The Rice factor (power ratio).
pub const GEN_K: Setting<field::F64> = Setting::new("gen_k", "Rice factor", "", 0.0)
    .range(0.0, 1e9, 0.0)
    .section(SECTION)
    .help("Direct over scattered power as a ratio; 0 is Rayleigh fading.");
/// The spans without carrier.
pub const GEN_OFF: Setting<field::Text> = Setting::new("gen_off", "No carrier", "", "")
    .section(SECTION)
    .help("Spans separated by ';', each 'from s, to s'.");

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 1000.0;

/// The test signal generator.
pub struct FadingGenerator {
    rate: f64,
    fader: Option<Fader>,
    off: Vec<(f64, f64)>,
    n: u64,
}

impl FadingGenerator {
    /// The signal of the settings `v` at `rate_hz`, its channel drawn from `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> FadingGenerator {
        let (fd, k) = (v.get(&GEN_FD_HZ), v.get(&GEN_K));
        let doppler = if k > 0.0 {
            Doppler::Rice {
                max_hz: fd,
                los_hz: 0.0,
                k,
            }
        } else {
            Doppler::Jakes { max_hz: fd }
        };
        FadingGenerator {
            rate: rate_hz,
            fader: Fader::new(doppler, rate_hz, &mut Rng::new(seed ^ 0x6661_6465)).ok(),
            off: parse_spans(v.get(&GEN_OFF)).unwrap_or_default(),
            n: 0,
        }
    }
}

impl Generator for FadingGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let g = self
                .fader
                .as_mut()
                .map_or(Complex32::new(1.0, 0.0), Fader::next);
            *z = if self.off.iter().any(|&(a, b)| t >= a && t < b) {
                Complex32::default()
            } else {
                g
            };
            self.n += 1;
        }
        out.len()
    }
    /// The carrier has a mean power of 1; an SNR is stated in 1 kHz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

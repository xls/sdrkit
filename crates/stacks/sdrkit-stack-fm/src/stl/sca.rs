//! The SCA meter of the STL stack (`stl.sca`): the power of the multiplex around the subcarriers at 67 and 92 kHz against
//! the noise there, every 0.5 s. The noise of an FM multiplex grows with the square of the frequency, so the noise of a
//! band is the noise density of the band from 100 to 108 kHz (where no subcarrier is) scaled by f².
//!
//! A subcarrier is present when its band stands 10 dB over that noise and holds at least 0.1 % of the power of a
//! full-deviation sine (-30 dB, a third of the amplitude of a 10 % injection), so the skirts of RDS on a clean link are no
//! subcarrier.

use super::link::REPORT_TICKS;
use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed};

/// The centres of the SCA subcarriers, Hz.
pub const SCA_HZ: [f64; 2] = crate::generator::SCA_HZ;
/// Half the band measured around a subcarrier, Hz.
pub const HALF_BAND_HZ: f64 = 7.5e3;
/// The band that measures the noise, Hz.
pub const NOISE_BAND_HZ: (f64, f64) = (100e3, 108e3);
/// The SNR a subcarrier needs, dB.
pub const MIN_SNR_DB: f64 = 10.0;
/// The power a subcarrier needs against a full-deviation sine, dB.
pub const MIN_LEVEL_DB: f64 = -30.0;

const FFT: usize = 1024;

/// The SCA meter's report: the SNR of each subcarrier in [`SCA_HZ`], `None` when it is absent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScaReport {
    /// The SNR of each subcarrier, dB.
    pub snr_db: [Option<f64>; 2],
}
impl ScaReport {
    /// A subcarrier is present.
    pub fn present(&self) -> bool {
        self.snr_db.iter().any(Option::is_some)
    }
}
impl Item for ScaReport {
    const KIND: &'static str = "stl.sca_report";
}

/// The SNR of each subcarrier in the averaged power spectrum `power` of a real multiplex (display order, `bin_hz` apart,
/// 1.0 is full deviation), `None` where it is absent.
pub fn measure(power: &[f32], bin_hz: f64) -> [Option<f64>; 2] {
    let half = power.len() / 2;
    // Both halves of the spectrum of a real signal: the sum is the mean power of the band.
    let band = |lo: f64, hi: f64| -> (f64, f64) {
        let (mut p, mut f2) = (0.0, 0.0);
        for (i, &x) in power.iter().enumerate() {
            let f = (i as f64 - half as f64).abs() * bin_hz;
            if (lo..hi).contains(&f) {
                p += x as f64;
                f2 += f * f;
            }
        }
        (p, f2)
    };
    let (noise, noise_f2) = band(NOISE_BAND_HZ.0, NOISE_BAND_HZ.1);
    // Noise power per unit of f², from the noise band.
    let density = if noise_f2 > 0.0 { noise / noise_f2 } else { 0.0 };
    let full = 0.5;
    SCA_HZ.map(|fc| {
        let (p, f2) = band(fc - HALF_BAND_HZ, fc + HALF_BAND_HZ);
        let n = (density * f2).max(f64::MIN_POSITIVE);
        let snr = 10.0 * ((p - n).max(f64::MIN_POSITIVE) / n).log10();
        let level = 10.0 * (p.max(f64::MIN_POSITIVE) / full).log10();
        (snr >= MIN_SNR_DB && level >= MIN_LEVEL_DB).then_some(snr)
    })
}

/// The SCA meter (`stl.sca`): the multiplex in, a [`ScaReport`] every [`REPORT_TICKS`] ticks.
#[derive(Default)]
pub struct ScaComponent {
    rate: f64,
    spectrum: Option<SpectrumAnalyzer>,
    ticks: u32,
}

impl Component for ScaComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "stl.sca",
            vec![Port::real("in")],
            vec![Port::typed::<ScaReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.ticks = 0;
        self.spectrum = (self.rate > 0.0)
            .then(|| {
                let cfg = SpectrumConfig {
                    frames_per_second: self.rate / FFT as f64,
                    average: usize::MAX,
                    ..SpectrumConfig::spec(FFT, self.rate)
                };
                SpectrumAnalyzer::new(cfg).ok()
            })
            .flatten();
        Ok(vec![Format::new(Kind::typed::<ScaReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(a) = &mut self.spectrum else { return };
        a.push_real(io.real(0));
        if !io.tick() {
            return;
        }
        self.ticks += 1;
        if !self.ticks.is_multiple_of(REPORT_TICKS) {
            return;
        }
        let snr_db = if a.frames() > 0 && self.rate > 2.0 * NOISE_BAND_HZ.1 {
            measure(a.power(), self.rate / FFT as f64)
        } else {
            [None; 2]
        };
        a.reset();
        io.items_out::<ScaReport>(0).push(Timed {
            time: 0.0,
            value: ScaReport { snr_db },
        });
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

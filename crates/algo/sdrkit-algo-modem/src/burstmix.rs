//! The channel of one burst: the burst's samples mixed so that its signal sits at 0 Hz, low-pass filtered (a Hamming-windowed
//! sinc) and decimated. The oscillator runs from the stream index of the burst's first sample, so bursts of one signal stay
//! phase continuous. The filter starts empty for each burst.
//!
//! ```
//! use sdrkit_algo_modem::burstmix::BurstMixer;
//! use sdrkit_algo_modem::Complex32;
//! let mut m = BurstMixer::new(1e6, 4, 60e3, 33).unwrap();
//! let x: Vec<Complex32> = (0..4000)
//!     .map(|i| {
//!         let p = std::f32::consts::TAU * 0.1 * i as f32;
//!         Complex32::new(p.cos(), p.sin())
//!     })
//!     .collect();
//! let mut out = Vec::new();
//! m.run(&x, 0, 100e3, &mut out);
//! assert_eq!(out.len(), 1000);
//! assert!((out[500] - out[501]).norm() < 1e-3);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;

/// The mixer, filter and decimator of bursts at one input rate.
pub struct BurstMixer {
    rate_hz: f64,
    decim: usize,
    fir: FirDecimator<Complex32>,
    mixed: Vec<Complex32>,
}

impl BurstMixer {
    /// For input at `rate_hz`, decimation by `decim`, a cut-off of `cutoff_hz` and `taps` taps (made odd). Errors for a rate or
    /// cut-off that is not positive, a decimation of 0, or a filter the design refuses.
    pub fn new(
        rate_hz: f64,
        decim: usize,
        cutoff_hz: f64,
        taps: usize,
    ) -> Result<BurstMixer, ModemError> {
        Param::Rate.check_positive(rate_hz)?;
        Param::Cutoff.check_positive(cutoff_hz)?;
        Param::Decimation.check_count(decim, 1, 1 << 20)?;
        let t = design_by_taps(taps | 1, (cutoff_hz / rate_hz).min(0.5), Window::Hamming)
            .map_err(ModemError::Dsp)?;
        Ok(BurstMixer {
            rate_hz,
            decim,
            fir: FirDecimator::new(&t, decim),
            mixed: Vec::new(),
        })
    }

    /// The input rate, Hz.
    pub fn rate_hz(&self) -> f64 {
        self.rate_hz
    }

    /// The decimation.
    pub fn decimation(&self) -> usize {
        self.decim
    }

    /// The output rate, Hz.
    pub fn out_rate_hz(&self) -> f64 {
        self.rate_hz / self.decim as f64
    }

    /// Mixes `samples` (whose first sample has stream index `start`) by `-offset_hz`, filters and decimates them into `out`
    /// (cleared first).
    pub fn run(&mut self, samples: &[Complex32], start: u64, offset_hz: f64, out: &mut Vec<Complex32>) {
        out.clear();
        let mut nco = Nco::new(self.rate_hz, -offset_hz);
        nco.set_phase_cycles((-offset_hz * start as f64 / self.rate_hz).rem_euclid(1.0));
        self.mixed.resize(samples.len(), Complex32::default());
        nco.mix(samples, &mut self.mixed);
        self.fir.reset();
        self.fir.process(&self.mixed, out);
    }
}

impl std::fmt::Debug for BurstMixer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BurstMixer")
            .field("rate_hz", &self.rate_hz)
            .field("decim", &self.decim)
            .finish()
    }
}

//! The sub-audio band (below 300 Hz) of real audio at about 1 kHz: what the CTCSS and DCS decoders read.

use crate::{Param, TonesError};
use sdrkit_algo_dsp::decim::FirDecimator;

/// The highest frequency passed flat, Hz.
pub const PASS_HZ: f64 = 260.0;
/// Aliases fall above this frequency, Hz.
pub const ALIAS_FREE_HZ: f64 = 300.0;

/// Low-pass and decimation to the sub-audio rate. Streaming. It allocates nothing after [`SubAudio::new`].
#[derive(Clone, Debug)]
pub struct SubAudio {
    dec: FirDecimator<f32>,
    out_rate: f64,
}

impl SubAudio {
    /// For audio at `rate` Hz (at least 1000).
    pub fn new(rate: f64) -> Result<Self, TonesError> {
        if !(rate.is_finite() && rate >= 1000.0) {
            return Err(TonesError::BadParameter(Param::SubAudioRate));
        }
        let factor = ((rate / 1000.0).floor() as usize).max(1);
        Ok(SubAudio {
            dec: FirDecimator::design(rate, factor, PASS_HZ, ALIAS_FREE_HZ, 60.0)
                .map_err(TonesError::Dsp)?,
            out_rate: rate / factor as f64,
        })
    }

    /// The output rate, Hz (between 1000 and 2000).
    pub fn rate(&self) -> f64 {
        self.out_rate
    }

    /// The decimation factor.
    pub fn factor(&self) -> usize {
        self.dec.factor()
    }

    /// The filter's delay, input samples.
    pub fn delay(&self) -> f64 {
        self.dec.delay()
    }

    /// Takes one input sample. Returns a sub-audio sample every `factor` inputs.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<f32> {
        self.dec.push(if x.is_finite() { x } else { 0.0 })
    }

    /// Forgets the history.
    pub fn reset(&mut self) {
        self.dec.reset();
    }
}

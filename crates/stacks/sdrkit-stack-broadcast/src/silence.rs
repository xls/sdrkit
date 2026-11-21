//! The generator that sends silence, and the error of a test signal that cannot be made.

use sdrkit_core::Complex32;

/// A generator that sends silence.
pub struct Silent(pub f64);

impl sdrkit_core::Generator for Silent {
    fn rate_hz(&self) -> f64 {
        self.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        out.len()
    }
}

/// The test signal cannot be made with the settings at this sample rate.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
#[error("the test signal cannot be made with these settings at {rate_hz} samples a second")]
pub struct NoSignal {
    /// The sample rate, Hz.
    pub rate_hz: f64,
}

impl NoSignal {
    /// The error as a [`sdrkit_core::ConfigError`].
    pub fn refused(self) -> sdrkit_core::ConfigError {
        sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(self))
    }
}

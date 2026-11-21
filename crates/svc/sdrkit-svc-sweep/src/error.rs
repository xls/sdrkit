//! The errors of the sweep service.

use sdrkit_core::{StateError, StorageError, StorePath};

/// An error of the sweep service.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SweepError {
    /// The range is empty, or an edge is not a finite frequency.
    #[error("the range {low_hz} Hz to {high_hz} Hz is empty")]
    EmptyRange {
        /// The low edge, Hz.
        low_hz: f64,
        /// The high edge, Hz.
        high_hz: f64,
    },
    /// The range lies outside what the radio tunes.
    #[error("the range {low_hz} Hz to {high_hz} Hz is outside the radio")]
    OutsideRadio {
        /// The requested low edge, Hz.
        low_hz: f64,
        /// The requested high edge, Hz.
        high_hz: f64,
    },
    /// The resolution is not a positive frequency.
    #[error("the resolution {bin_hz} Hz is not positive")]
    BadResolution {
        /// The resolution given, Hz.
        bin_hz: f64,
    },
    /// The sample rate is not a positive frequency.
    #[error("the rate {rate_hz} Hz is not positive")]
    BadRate {
        /// The rate given, Hz.
        rate_hz: f64,
    },
    /// Every hop of the range is excluded.
    #[error("every hop is excluded")]
    AllExcluded,
    /// A spectrum frame does not have the bins or the span that the sweep declares.
    #[error("a frame of {bins} bins over {span_hz} Hz is not the spectrum the sweep needs")]
    FrameShape {
        /// The bins of the frame.
        bins: usize,
        /// The span of the frame, Hz.
        span_hz: f64,
    },
    /// No preset has this name.
    #[error("there is no preset {name}")]
    NoPreset {
        /// The name given.
        name: String,
    },
    /// No sweep has run, so there is nothing to export or save.
    #[error("no sweep has run")]
    NoSweep,
    /// A file of the service did not read or write.
    #[error("{path}: {source}")]
    Storage {
        /// The file.
        path: StorePath,
        /// What went wrong.
        source: StorageError,
    },
    /// The file is not a survey file of this version.
    #[error("{path} is not a survey file")]
    NotASurvey {
        /// The file.
        path: StorePath,
    },
    /// The file is not a saved noise floor.
    #[error("{path} is not a saved noise floor: {source}")]
    NotAFloor {
        /// The file.
        path: StorePath,
        /// Why the saved state does not open.
        source: StateError,
    },
    /// The saved state did not encode.
    #[error("the saved state could not be written")]
    Encode,
}

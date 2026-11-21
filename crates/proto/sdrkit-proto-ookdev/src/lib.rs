//! The building blocks of OOK and FSK device decoders: pulse detection, pulse trains, slicers to bit rows, the protocol
//! list, device readings, a line-code guess and an encoder for generators.
//!
//! Lengths are in samples at the rate of the train. Device timings are in microseconds. Bad input returns an [`Error`], never a
//! panic.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod components;
pub mod detect;
pub mod encode;
pub mod guess;
pub mod protocol;
pub mod reading;
pub mod rows;
pub mod slicer;
pub mod train;

pub use components::{register, register_decoder};
pub use detect::{burst_tones, level_train, tone_train, DetectorConfig, PulseDetector};
pub use protocol::{Ctx, DecodeFn, Decoding, Protocol, ProtocolDef, ProtocolList};
pub use reading::{Channel, Check, Measure, Measurement, Reading};
pub use rows::{BitRows, Row};
pub use slicer::{Coding, Slicer, Timing};
pub use train::{Modulation, Pulse, Train};

/// A parameter that a call refused. [`Error::BadParameter`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// The detector settings: a positive rate, positive lengths and at least 2 pulses.
    Detector,
    /// The thresholds of the pulse gate: the close level must be at most the open level.
    Thresholds,
}

impl Param {
    /// The parameter and its limits in plain words.
    pub fn describe(self) -> &'static str {
        match self {
            Param::Detector => {
                "detector settings: a positive rate, positive lengths, at least 2 pulses"
            }
            Param::Thresholds => "detector thresholds: the close level at most the open level",
        }
    }
}

/// What went wrong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    /// A train has a length that is not finite or is negative, or a rate that is not positive.
    BadTrain,
    /// A timing has a width that is not finite, is negative, or is 0 where the coding needs it.
    BadTiming,
    /// A setting is out of its range.
    BadParameter(Param),
    /// The pulse gate cannot be made.
    Modem(sdrkit_algo_modem::ModemError),
    /// The frequency discriminator cannot be made.
    Dsp(sdrkit_algo_dsp::DspError),
    /// The input is shorter than the operation needs.
    InputTooShort,
    /// An output slice is too short. `need` items are necessary.
    OutputTooShort {
        /// Items necessary.
        need: usize,
    },
    /// The text holds more rows or bits than the buffer can hold.
    Capacity,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadTrain => f.write_str("the train has a bad length or rate"),
            Error::BadTiming => f.write_str("the timing has a bad width"),
            Error::BadParameter(p) => write!(f, "{} out of range", p.describe()),
            Error::Modem(e) => e.fmt(f),
            Error::Dsp(e) => e.fmt(f),
            Error::InputTooShort => f.write_str("the input is too short"),
            Error::OutputTooShort { need } => write!(f, "the output needs {need} items"),
            Error::Capacity => f.write_str("too many rows or bits for the buffer"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Modem(e) => Some(e),
            Error::Dsp(e) => Some(e),
            Error::BadTrain
            | Error::BadTiming
            | Error::BadParameter(_)
            | Error::InputTooShort
            | Error::OutputTooShort { .. }
            | Error::Capacity => None,
        }
    }
}

/// A component whose detector cannot be made refuses the format with this error.
impl From<Error> for sdrkit_core::ConfigError {
    fn from(e: Error) -> Self {
        sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
    }
}

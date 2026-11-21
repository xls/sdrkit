//! The error type of every fallible function in this crate.

use std::fmt;

/// What went wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// There is no scale factor band table for this sampling rate or frame length.
    SampleRate,
    /// An input slice is shorter than one frame.
    InputLength,
    /// The data before the raw data block, or the smallest block, does not fit the budget.
    Budget,
    /// A frame breaks the bitstream syntax.
    Syntax,
    /// A frame or a configuration uses a tool or an element that this decoder does not decode.
    Unsupported,
}

/// `Result` with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::SampleRate => "no scale factor band table for this sampling rate",
            Error::InputLength => "input shorter than one frame",
            Error::Budget => "the block does not fit its budget",
            Error::Syntax => "the frame breaks the bitstream syntax",
            Error::Unsupported => "the stream uses a tool this decoder does not decode",
        })
    }
}

impl std::error::Error for Error {}

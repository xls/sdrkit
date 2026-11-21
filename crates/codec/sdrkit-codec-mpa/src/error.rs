//! The error type of every fallible function in this crate.

use std::fmt;

/// What went wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The input is shorter than its header or the frame needs.
    Truncated,
    /// The 12-bit syncword is not all ones.
    Sync,
    /// The layer code is the reserved value 00.
    Layer,
    /// The bit rate index is 0 (free format) or the forbidden 15, or a bit rate has no index.
    BitRate,
    /// The sampling frequency code is the reserved value 3, or a sampling rate has no code.
    SampleRate,
    /// The emphasis code is the reserved value 2.
    Emphasis,
    /// The header is MPEG-2.5, which is not an ISO format.
    Mpeg25,
    /// A header field is out of its range.
    Field,
    /// The frame is not Layer II.
    NotLayer2,
    /// The CRC of the header and side information does not match.
    Crc,
    /// A scale factor index is the forbidden value 63.
    ScaleFactor,
    /// An input slice is shorter than one frame needs.
    InputLength,
}

/// `Result` with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::Truncated => "input shorter than the frame",
            Error::Sync => "no MPEG audio syncword",
            Error::Layer => "reserved layer code",
            Error::BitRate => "free format, forbidden or unknown bit rate",
            Error::SampleRate => "reserved or unknown sampling frequency",
            Error::Emphasis => "reserved emphasis code",
            Error::Mpeg25 => "MPEG-2.5 is not an ISO format",
            Error::Field => "header field out of range",
            Error::NotLayer2 => "not a Layer II frame",
            Error::Crc => "CRC failed",
            Error::ScaleFactor => "forbidden scale factor index 63",
            Error::InputLength => "input shorter than one frame",
        })
    }
}

impl std::error::Error for Error {}

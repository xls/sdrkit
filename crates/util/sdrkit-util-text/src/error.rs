//! The error type of this crate.

use std::fmt;

/// Why a text operation failed.
///
/// Every decoder and encoder in this crate returns this type instead of panicking on bad input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A character has no code in the chosen alphabet.
    Unencodable(char),
    /// A code has no character in the chosen alphabet.
    UnknownCode(u32),
    /// A value is outside the range the format can carry.
    OutOfRange,
    /// The input is shorter than the format needs.
    TooShort,
    /// The input is longer than the format allows.
    TooLong,
    /// A byte failed its parity check.
    Parity,
    /// A byte or word is not a valid code word (too many bit errors, or a byte outside the alphabet).
    Invalid,
    /// The output buffer is too small.
    BufferFull,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unencodable(c) => write!(f, "character {c:?} has no code in this alphabet"),
            Error::UnknownCode(v) => write!(f, "code {v:#x} has no character in this alphabet"),
            Error::OutOfRange => f.write_str("value out of range"),
            Error::TooShort => f.write_str("input too short"),
            Error::TooLong => f.write_str("input too long"),
            Error::Parity => f.write_str("parity error"),
            Error::Invalid => f.write_str("not a valid code word"),
            Error::BufferFull => f.write_str("output buffer full"),
        }
    }
}

impl std::error::Error for Error {}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;

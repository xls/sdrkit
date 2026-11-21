//! The error type of this crate.

/// A parameter or an input value that this crate cannot use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LocateError {
    /// A value lies outside its allowed range (or is not finite).
    OutOfRange {
        /// The name of the value in plain words.
        what: &'static str,
        /// The value given.
        got: f64,
    },
}

impl LocateError {
    pub(crate) fn range<T>(what: &'static str, got: f64) -> Result<T, LocateError> {
        Err(LocateError::OutOfRange { what, got })
    }
}

impl std::fmt::Display for LocateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocateError::OutOfRange { what, got } => write!(f, "{what} out of range: {got}"),
        }
    }
}

impl std::error::Error for LocateError {}

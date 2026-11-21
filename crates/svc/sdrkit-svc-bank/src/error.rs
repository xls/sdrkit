//! The errors of the bank service.

use sdrkit_core::StateError;

/// An error of the bank service.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum BankError {
    /// The bank has no channel with this row key.
    #[error("there is no channel {key}")]
    NoChannel {
        /// The row key given.
        key: String,
    },
    /// The readings do not match the watched channels: the engine must make the carrier scan again.
    #[error("{got} readings for {expected} watched channels")]
    Readings {
        /// The readings that the watched channels need.
        expected: usize,
        /// The readings given.
        got: usize,
    },
    /// The saved state cannot be read.
    #[error(transparent)]
    State(#[from] StateError),
    /// The state cannot be written.
    #[error("the state cannot be written")]
    Encode,
}

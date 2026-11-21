//! The error type of the crate.

use sdrkit_core::StateError;

/// Why a table, trail, joiner or saved state refused something.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum Error {
    /// A time that is not a finite number of seconds.
    #[error("a time that is not a finite number")]
    BadTime,
    /// A position outside ±90° latitude or ±180° longitude, or not finite.
    #[error("a position out of range")]
    BadPosition,
    /// The table is at its cap and every row is a favourite, so no row can make room.
    #[error("the table is full of favourites")]
    Full,
    /// The data cannot be written as saved state (a map whose keys are not text, for example).
    #[error("the data cannot be saved")]
    NotSaved,
    /// A saved state that is refused.
    #[error(transparent)]
    State(#[from] StateError),
}

/// A refused saved state becomes the refusal of `restore_state`. A bad time in the state makes it damaged.
impl From<Error> for StateError {
    fn from(e: Error) -> Self {
        match e {
            Error::State(s) => s,
            Error::BadTime | Error::BadPosition | Error::Full | Error::NotSaved => {
                StateError::Damaged
            }
        }
    }
}

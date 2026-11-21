//! The errors of the store.

use sdrkit_core::{StorageError, StorePath};

/// The reason the store could not carry out a request.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A file in the host's storage could not be read or written.
    #[error("store file {path}: {source}")]
    Storage {
        /// The file.
        path: StorePath,
        /// What the storage said.
        source: StorageError,
    },
    /// A time is not a date (not finite, or too far from 1970).
    #[error("{unix_time} s is not a date")]
    Time {
        /// The time, Unix seconds.
        unix_time: f64,
    },
    /// A message could not be written as JSON.
    #[error("store JSON: {0}")]
    Json(#[from] serde_json::Error),
}

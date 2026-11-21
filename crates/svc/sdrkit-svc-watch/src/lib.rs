//! Watches and alerts: a watch looks for a thing, a field value, a frequency or a word in each event, and a match produces an
//! alert within the limits. Alerts are events, and push channels send them on.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod entry;
mod push;
pub mod watch;

pub use entry::WatchWhat;
pub use watch::{Channel, Outcome, Watch, WatchCommand, WatchSetting, KEY, WATCH_EVENTS};

/// The reason the watch could not carry out a request.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// A name in a watch does not resolve.
    #[error("{arg}: {error}")]
    Name {
        /// The argument that holds the name.
        arg: &'static str,
        /// What does not resolve.
        error: sdrkit_svc_events::NameError,
    },
    /// The address of a push channel is refused.
    #[error("{0}")]
    Output(#[from] sdrkit_svc_outputs::OutputError),
    /// A file in the host's storage could not be read or written.
    #[error("watch file {path}: {source}")]
    Storage {
        /// The file.
        path: sdrkit_core::StorePath,
        /// What the storage said.
        source: sdrkit_core::StorageError,
    },
    /// The saved watches could not be read or written as JSON.
    #[error("watch JSON: {0}")]
    Json(#[from] serde_json::Error),
}

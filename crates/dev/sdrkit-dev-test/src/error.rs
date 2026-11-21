//! The error of the test helpers.

use sdrkit_components::sim::SimError;
use sdrkit_core::settings::SchemaError;
use sdrkit_core::{ConfigError, SettingError};
use std::path::PathBuf;

/// Why a recording, a vector or a harness run failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file could not be read.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The error of the system.
        source: std::io::Error,
    },
    /// A JSON file does not parse.
    #[error("{}: {source}", path.display())]
    Json {
        /// The file.
        path: PathBuf,
        /// The parse error.
        source: serde_json::Error,
    },
    /// The `core:datatype` of a SigMF recording is not `cf32_le` or `ci16_le`.
    #[error("{}: datatype {found:?} is not cf32_le or ci16_le", path.display())]
    Datatype {
        /// The `.sigmf-meta` file.
        path: PathBuf,
        /// The datatype in the file.
        found: String,
    },
    /// A SigMF recording has no `core:sample_rate`.
    #[error("{}: no core:sample_rate", path.display())]
    NoRate {
        /// The `.sigmf-meta` file.
        path: PathBuf,
    },
    /// The stack has no generator.
    #[error("{stack} has no generator")]
    NoGenerator {
        /// The stack id.
        stack: &'static str,
    },
    /// A key of the stack is also a generator key.
    #[error("the settings: {0}")]
    Schema(SchemaError),
    /// A setting value is refused, or the settings have no such key.
    #[error("a setting: {0}")]
    Settings(#[from] SettingError),
    /// The impairment chain refuses the settings.
    #[error("the impairments: {0}")]
    Impair(SimError),
    /// The stack refuses to make its receiver or its generator, or the receiver refuses the input.
    #[error("the stack: {0}")]
    Config(ConfigError),
}

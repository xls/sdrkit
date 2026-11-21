//! The errors of the crate.

use std::path::PathBuf;

/// A data folder or one of its files cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// The folder or the file does not exist.
    #[error("missing data: {}", path.display())]
    Missing {
        /// The path that does not exist.
        path: PathBuf,
    },
    /// The folder or the file exists and cannot be read.
    #[error("cannot read {}: {source}", path.display())]
    Read {
        /// The path.
        path: PathBuf,
        /// The error of the file system.
        #[source]
        source: std::io::Error,
    },
    /// The file does not have its format.
    #[error("{}: {reason}", path.display())]
    Format {
        /// The path.
        path: PathBuf,
        /// What is wrong, with the line or the JSON path.
        reason: String,
    },
}

/// A text is not a region.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegionError {
    /// The text is not an ISO 3166 alpha-2 country code.
    #[error("`{0}` is not a country code of two letters")]
    NotACountry(String),
}

/// No preset can be made.
#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    /// The data has no plan and no station for the stack in the region.
    #[error("no data for the stack `{stack}` in the region {region}")]
    NoData {
        /// The stack id.
        stack: String,
        /// The region, as [`crate::Region`] displays it.
        region: String,
    },
    /// A file of the data folder cannot be used.
    #[error(transparent)]
    Data(#[from] DataError),
    /// The channel number is above the slots of the preset in the band; a running stack refuses it too.
    #[error(transparent)]
    ChannelNumTooHigh(#[from] sdrkit_proto_meshtastic::ChannelNumTooHigh),
}

/// A stack does not start from a preset.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// No preset can be made.
    #[error(transparent)]
    Preset(#[from] PresetError),
    /// A value of the preset is not a value of the stack's setting.
    #[error("the stack `{stack}` does not take {key}={value}")]
    Setting {
        /// The stack id.
        stack: String,
        /// The setting key.
        key: String,
        /// The value, as text.
        value: String,
    },
    /// The engine refused a setting or the start.
    #[error(transparent)]
    Engine(Box<sdrkit_host_engine::EngineError>),
}

impl From<sdrkit_host_engine::EngineError> for StartError {
    fn from(e: sdrkit_host_engine::EngineError) -> Self {
        StartError::Engine(Box::new(e))
    }
}

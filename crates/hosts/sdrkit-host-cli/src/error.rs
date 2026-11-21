//! The one error of the tool and its exit code.

use sdrkit_core::{CommandError, StorageError, ValueKind};
use sdrkit_graph::GraphError;
use sdrkit_host_engine::EngineError;

/// The text that lists the forms a number with a unit can take.
pub const NUMBER_FORMS: &str =
    "write a number with an optional unit: 919.875M, 919.875MHz, 919875000, 2.4M, 48k, 1.2G";

/// Everything that can go wrong in a command.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// The engine refused a call.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// A graph description did not build.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// The host's storage refused a read or a write.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// A file or the terminal refused a read or a write.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// A JSON text did not read, or a value did not write.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// A WAV file did not open or write.
    #[error(transparent)]
    Wav(#[from] hound::Error),
    /// The sound output did not start.
    #[error(transparent)]
    Sound(#[from] SoundError),
    /// The Ctrl-C handler did not install.
    #[error(transparent)]
    CtrlC(#[from] ctrlc::Error),
    /// The data folder cannot be read, or a stack does not start from its preset.
    #[error(transparent)]
    Preset(#[from] sdrkit_presets::StartError),
    /// The value of `--region` is not a country code.
    #[error("--region: {0}")]
    Region(#[from] sdrkit_presets::RegionError),
    /// The data folder has no plans for the country of `--region`.
    #[error("--region: the data has no plans for `{region}`. The regions of the data:\n{list}")]
    UnknownRegion {
        /// The value given.
        region: String,
        /// The regions of the data, one a line: the code and the name.
        list: String,
    },
    /// No stack has this id.
    #[error("no stack has the id `{id}`")]
    UnknownStack {
        /// The id given.
        id: String,
    },
    /// No owner has this name.
    #[error("no owner has the name `{name}`")]
    UnknownOwner {
        /// The name given.
        name: String,
    },
    /// No driver has this key.
    #[error("no radio driver has the key `{key}`")]
    UnknownDriver {
        /// The key given.
        key: String,
    },
    /// The owners named have no setting with this key.
    #[error("{owner} has no setting `{key}`")]
    UnknownSetting {
        /// The owners that were searched.
        owner: String,
        /// The key given.
        key: String,
    },
    /// The owner declares no table with this key.
    #[error("`{owner}` has no table `{table}`")]
    UnknownTable {
        /// The owner name given.
        owner: String,
        /// The table key given.
        table: String,
    },
    /// The text is not a number with a unit.
    #[error("`{text}` is not a number ({NUMBER_FORMS})")]
    BadNumber {
        /// The text given.
        text: String,
    },
    /// The text is not `key=value`.
    #[error("`{text}` is not key=value")]
    NotKeyValue {
        /// The text given.
        text: String,
    },
    /// The text is not a value of the kind of the setting or argument.
    #[error("`{text}` is not a value of `{key}` (kind {kind:?}{forms})")]
    BadValue {
        /// The key of the setting or argument.
        key: String,
        /// The text given.
        text: String,
        /// The kind it must have.
        kind: ValueKind,
        /// The forms or choices it can take.
        forms: String,
    },
    /// The owner exists only while `run` runs.
    #[error("`{name}` exists only while `run` runs (type it on the console of `run`)")]
    NotRunning {
        /// The owner name given.
        name: String,
    },
    /// A test did not pass.
    #[error("{failed} of {total} checks failed")]
    ChecksFailed {
        /// The checks that failed.
        failed: usize,
        /// Every check.
        total: usize,
    },
}

/// Why the sound output did not start.
#[derive(Debug, thiserror::Error)]
pub enum SoundError {
    /// The computer has no sound output.
    #[error("no sound output device")]
    NoDevice,
    /// The device refused the stream's form.
    #[error(transparent)]
    Build(#[from] cpal::BuildStreamError),
    /// The device did not start the stream.
    #[error(transparent)]
    Play(#[from] cpal::PlayStreamError),
}

impl CliError {
    /// The exit code: 1 if a check failed or a call failed, 2 for bad usage.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::UnknownStack { .. }
            | CliError::UnknownOwner { .. }
            | CliError::UnknownDriver { .. }
            | CliError::UnknownSetting { .. }
            | CliError::UnknownTable { .. }
            | CliError::BadNumber { .. }
            | CliError::NotKeyValue { .. }
            | CliError::BadValue { .. }
            | CliError::NotRunning { .. }
            | CliError::Region(_)
            | CliError::UnknownRegion { .. }
            | CliError::Engine(
                EngineError::NoSuchStack { .. } | EngineError::NoSuchDriver { .. },
            )
            | CliError::Engine(
                EngineError::Setting { .. }
                | EngineError::Command {
                    error: CommandError::BadArgument { .. },
                    ..
                },
            ) => 2,
            _ => 1,
        }
    }
}

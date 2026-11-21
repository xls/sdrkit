//! The errors of the record service.

use sdrkit_core::storage::PathError;
use sdrkit_core::{StorageError, StorePath};

/// Why a recording file does not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// The file name has no extension that names a recording.
    #[error("the file name does not name a recording")]
    UnknownName,
    /// The file ends before its header does.
    #[error("the file is shorter than its header")]
    Short,
    /// A WAV file without the RIFF, WAVE, fmt or data parts.
    #[error("the file is not a WAV file")]
    NotWav,
    /// A WAV file whose sample encoding is not 16-bit integer or 32-bit float.
    #[error("the WAV file holds samples of format tag {tag} with {bits} bits")]
    WavEncoding {
        /// The format tag of the fmt part.
        tag: u16,
        /// The bits per sample.
        bits: u16,
    },
    /// A file without the FLAC marker or STREAMINFO.
    #[error("the file is not a FLAC file")]
    NotFlac,
    /// SigMF metadata that is not JSON.
    #[error("the SigMF metadata is not JSON (line {line}, column {column})")]
    SigmfJson {
        /// The line of the first error.
        line: usize,
        /// The column of the first error.
        column: usize,
    },
    /// SigMF metadata without a field that a recording needs.
    #[error("the SigMF metadata has no valid {0}")]
    SigmfField(SigmfField),
    /// A transport stream file that does not start with the sync byte.
    #[error("the file is not a transport stream")]
    NotTs,
}

/// A field of SigMF metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigmfField {
    /// `global.core:datatype`, or a datatype that is not a complex type this crate reads.
    Datatype,
    /// `global.core:sample_rate`.
    SampleRate,
}

impl std::fmt::Display for SigmfField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SigmfField::Datatype => "core:datatype",
            SigmfField::SampleRate => "core:sample_rate",
        })
    }
}

/// Why a cron rule does not parse. `at` is the byte position of the field in the rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CronError {
    /// The rule does not have five fields.
    #[error("a cron rule has five fields")]
    Fields,
    /// A field that is not a number, a name, `*`, a range, a step or a list.
    #[error("the cron field at {at} does not read")]
    Field {
        /// The byte position of the field.
        at: usize,
    },
    /// A value outside the range of its field.
    #[error("the cron field at {at} is out of range")]
    Range {
        /// The byte position of the field.
        at: usize,
    },
}

/// The errors of the record service.
#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    /// The storage failed on `path`.
    #[error("the storage failed on {path}: {source}")]
    Storage {
        /// The file.
        path: StorePath,
        /// What the storage said.
        source: StorageError,
    },
    /// A file name made from the settings is not a store path.
    #[error("the name is not a store path: {0}")]
    Name(PathError),
    /// Less than `need` bytes are free beside the reserve.
    #[error("{free} bytes are free, {need} are needed beside the reserve")]
    NoSpace {
        /// The bytes needed.
        need: u64,
        /// The bytes free.
        free: u64,
    },
    /// The stream has no rate yet.
    #[error("the stream has no rate yet")]
    NoRate,
    /// The rewind buffer holds no samples.
    #[error("the rewind buffer holds no samples")]
    NothingHeld,
    /// The engine dropped the feed of the recording.
    #[error("the feed of the recording was dropped")]
    FeedGone,
    /// The system refused a writer thread.
    #[error("the system refused a writer thread: {kind}")]
    Thread {
        /// What the system said.
        kind: std::io::ErrorKind,
    },
    /// A recording file does not read.
    #[error("{path} does not read: {error}")]
    Format {
        /// The file.
        path: StorePath,
        /// Why.
        error: FormatError,
    },
    /// A cron rule does not parse.
    #[error(transparent)]
    Cron(#[from] CronError),
    /// The saved schedules do not read.
    #[error("the saved schedules do not read (line {line}, column {column})")]
    Saved {
        /// The line of the first error.
        line: usize,
        /// The column of the first error.
        column: usize,
    },
}

impl RecordError {
    /// A storage failure on `path`.
    pub(crate) fn storage(path: &StorePath, source: StorageError) -> Self {
        RecordError::Storage {
            path: path.clone(),
            source,
        }
    }
}

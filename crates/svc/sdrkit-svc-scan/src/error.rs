//! The errors of the scan service, and the kinds of problem that an imported scan list can have.

use sdrkit_core::{StorageError, StorePath};

sdrkit_core::choice_enum! {
    /// What is wrong with one line of an imported scan list.
    pub enum ScanProblem {
        /// The frequency cell does not read as a frequency.
        NotAFrequency = ("not_a_frequency", "Not a frequency"),
        /// The dwell cell is not a number above zero.
        NotADwell = ("not_a_dwell", "Not a dwell time"),
        /// The settings cell is not a JSON list of settings with their owners and values.
        NotSettings = ("not_settings", "Not a list of settings"),
        /// The list is full.
        Full = ("full", "List full"),
    }
}

/// One problem of an imported file: its line (1 is the first) and its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineProblem {
    /// The line, 1 for the first.
    pub line: usize,
    /// What is wrong.
    pub kind: ScanProblem,
}

/// An error of the scan service.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ScanError {
    /// The band has no start above 0, a stop below the start, a step below 1 Hz, or too many steps.
    #[error("the band cannot be scanned")]
    BadBand,
    /// No stack has this id.
    #[error("there is no stack {id}")]
    NoStack {
        /// The id given.
        id: String,
    },
    /// There is nothing to scan.
    #[error("there is nothing to scan")]
    Nothing,
    /// A survey has no rate: none was given, and the input of the session has no known rate.
    #[error("the survey has no sample rate: the input of the session has none; give rate_hz")]
    NoRate,
    /// The engine has told the service nothing of this session: it does not run.
    #[error("there is no session {session}")]
    NoSession {
        /// The session key given.
        session: String,
    },
    /// The stack that a span or survey scan judges by has no result that a scan can watch under this key.
    #[error("the stack {stack} has no result {watch} to watch; it has: {}", .valid.join(", "))]
    NoWatch {
        /// The key given.
        watch: String,
        /// The stack id.
        stack: String,
        /// The keys of the results of the stack that a scan can watch.
        valid: Vec<String>,
    },
    /// The detector of a survey refused the frames of the rate.
    #[error("the survey cannot look at blocks of this rate")]
    BadRate,
    /// The dwell on a candidate of a survey is shorter than the look at a block.
    #[error("the dwell of {dwell_s} s is shorter than the look of {look_s} s: give a dwell_s of at least the look, or set a shorter look")]
    DwellUnderLook {
        /// The dwell given, s.
        dwell_s: f64,
        /// The look set, s.
        look_s: f64,
    },
    /// A file of the service could not be read or written.
    #[error("{path}: {source}")]
    Storage {
        /// The file.
        path: StorePath,
        /// What went wrong.
        source: StorageError,
    },
    /// The data could not be written as JSON.
    #[error("the data could not be written as JSON")]
    Encode,
}

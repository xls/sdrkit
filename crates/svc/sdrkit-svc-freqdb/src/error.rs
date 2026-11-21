//! The errors of the frequency database, and the kinds of problem that an entry or a line of an imported file can have.

use sdrkit_core::{StorageError, StorePath};

sdrkit_core::choice_enum! {
    /// What is wrong with an entry, or with one line of an imported file.
    pub enum Problem {
        /// The entry has no name.
        NoName = ("no_name", "No name"),
        /// The name or the notes are too long.
        TooLong = ("too_long", "Too long"),
        /// The frequency is not above 0 and below 300 GHz.
        BadFrequency = ("bad_frequency", "Bad frequency"),
        /// The upper edge is not above the frequency.
        BadUpperEdge = ("bad_upper_edge", "Bad upper edge"),
        /// The bandwidth is not above 0 and at most 100 MHz.
        BadBandwidth = ("bad_bandwidth", "Bad bandwidth"),
        /// The text is not JSON of an entry list.
        NotJson = ("not_json", "Not JSON"),
        /// A JSON item is not an object.
        NotAnEntry = ("not_an_entry", "Not an entry"),
        /// A line has no frequency.
        NoFrequency = ("no_frequency", "No frequency"),
        /// A settings cell is not a JSON list of settings with their owners and values.
        NotSettings = ("not_settings", "Not a list of settings"),
        /// The user list is full.
        Full = ("full", "List full"),
    }
}

/// One problem of an imported file: its line (1 is the first, or the place in a JSON list) and its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineProblem {
    /// The line.
    pub line: usize,
    /// What is wrong.
    pub kind: Problem,
}

/// An error of the frequency database.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DbError {
    /// The entry breaks a rule.
    #[error("the entry is not valid: {0:?}")]
    Invalid(Problem),
    /// No entry has this id.
    #[error("there is no entry {id}")]
    NoEntry {
        /// The id.
        id: String,
    },
    /// Only the entries of the user can be changed or deleted.
    #[error("entry {id} is built in")]
    BuiltIn {
        /// The id.
        id: String,
    },
    /// The user list holds its most entries.
    #[error("the user list is full")]
    Full,
    /// A file could not be read or written.
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

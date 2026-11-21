//! The errors of the memory service, and the kinds of problem that an imported file can have.

use sdrkit_core::{StorageError, StorePath};

sdrkit_core::choice_enum! {
    /// What is wrong with one line of an imported file.
    pub enum Problem {
        /// The text is not JSON of a memory list.
        NotJson = ("not_json", "Not JSON"),
        /// A JSON item is not a memory.
        NotAMemory = ("not_a_memory", "Not a memory"),
        /// The CSV header has no frequency column.
        NoFrequencyColumn = ("no_frequency_column", "No frequency column"),
        /// The row has no frequency.
        NoFrequency = ("no_frequency", "No frequency"),
        /// A frequency cell does not read as a frequency.
        NotAFrequency = ("not_a_frequency", "Not a frequency"),
        /// A bandwidth cell does not read as a frequency.
        NotABandwidth = ("not_a_bandwidth", "Not a bandwidth"),
        /// A number cell does not read as a number.
        NotANumber = ("not_a_number", "Not a number"),
        /// The frequency is not above zero and below 1 THz.
        FrequencyOutOfRange = ("frequency_out_of_range", "Frequency out of range"),
        /// The colour is not `#rrggbb`.
        NotAColour = ("not_a_colour", "Not a colour"),
        /// A rate, bandwidth or dwell time is not above zero.
        NotPositive = ("not_positive", "Not above zero"),
        /// The end of a range is below its start.
        EndBelowStart = ("end_below_start", "End below start"),
        /// The list is full.
        Full = ("full", "List full"),
        /// A settings cell is not a JSON list of settings with their owners and values.
        NotSettings = ("not_settings", "Not a list of settings"),
    }
}

/// One problem of an imported file: its line (1 is the first) and its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineProblem {
    /// The line, 1 for the first. For a JSON item, the position in the list.
    pub line: usize,
    /// What is wrong.
    pub kind: Problem,
}

/// An error of the memory service.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MemoryError {
    /// The list holds its most items.
    #[error("the list is full ({cap} items)")]
    Full {
        /// The most items.
        cap: usize,
    },
    /// No item has this id.
    #[error("there is no item {id}")]
    NoItem {
        /// The id.
        id: u64,
    },
    /// The item is not valid.
    #[error("the item is not valid: {0:?}")]
    Invalid(Problem),
    /// A preset name is empty or another preset of the stack has it.
    #[error("the preset name is empty or used")]
    PresetName,
    /// A built-in preset cannot be renamed or deleted.
    #[error("a built-in preset cannot be renamed or deleted")]
    Builtin,
    /// The preset belongs to another stack.
    #[error("the preset belongs to another stack")]
    OtherStack,
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

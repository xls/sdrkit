//! The errors of the codec and of the codebook loader.

use crate::codebooks::Table;

/// An error of the codec or of the codebook loader.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The mode needs this table, and the codebooks do not hold it.
    MissingTable(Table),
    /// The data for a table has the wrong number of values.
    TableSize {
        /// The table.
        table: Table,
        /// The number of `f32` values that the table must hold.
        expected: usize,
        /// The number of values that the data holds.
        got: usize,
    },
    /// The table does not exist: the index in the variant is out of range.
    UnknownTable(Table),
    /// The SHA-256 of a table file is not the published value.
    TableHash(Table),
    /// A table value is not finite.
    TableValue {
        /// The table.
        table: Table,
        /// The index of the first bad value.
        index: usize,
    },
    /// A table file cannot be read. The text is the reason reported by the system.
    Io {
        /// The table.
        table: Table,
        /// The reason.
        reason: String,
    },
    /// A buffer has the wrong length for the mode.
    Length {
        /// What the buffer holds: "speech input", "speech output", "frame input" or "frame output".
        what: &'static str,
        /// The length that the mode needs.
        expected: usize,
        /// The length of the buffer.
        got: usize,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MissingTable(t) => {
                write!(f, "codec2: the table {} is not loaded", t.file_name())
            }
            Error::UnknownTable(t) => write!(f, "codec2: there is no table {t:?}"),
            Error::TableHash(t) => write!(
                f,
                "codec2: the SHA-256 of {} is not the published value",
                t.file_name()
            ),
            Error::TableSize {
                table,
                expected,
                got,
            } => write!(
                f,
                "codec2: the table {} must hold {expected} values, not {got}",
                table.file_name()
            ),
            Error::TableValue { table, index } => write!(
                f,
                "codec2: value {index} of table {} is not finite",
                table.file_name()
            ),
            Error::Io { table, reason } => {
                write!(
                    f,
                    "codec2: cannot read table {}: {reason}",
                    table.file_name()
                )
            }
            Error::Length {
                what,
                expected,
                got,
            } => write!(
                f,
                "codec2: the {what} must hold {expected} values, not {got}"
            ),
        }
    }
}

impl std::error::Error for Error {}

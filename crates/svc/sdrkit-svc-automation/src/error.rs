//! The errors of the automation.

use sdrkit_core::{StorageError, StorePath};
use sdrkit_svc_events::NameError;

use crate::expr::ExprError;

/// What is wrong with a piece of user text.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum TextError {
    /// A kind, family or field name does not resolve.
    #[error("{0}")]
    Name(#[from] NameError),
    /// The condition does not parse.
    #[error("{0}")]
    Expr(#[from] ExprError),
    /// A time of day is not `HH:MM`.
    #[error("not a time of day (at {at})")]
    Time {
        /// The byte position.
        at: usize,
    },
    /// A day is not a number from 0 (Monday) to 6 (Sunday).
    #[error("not a day of the week (at {at})")]
    Day {
        /// The byte position.
        at: usize,
    },
    /// A corner is not `lat lon` in range.
    #[error("not a corner (at {at})")]
    Corner {
        /// The byte position.
        at: usize,
    },
}

impl TextError {
    /// The byte position of the problem in its text.
    pub fn at(&self) -> usize {
        match self {
            TextError::Name(
                NameError::UnknownField { at, .. }
                | NameError::UnknownKind { at, .. }
                | NameError::UnknownFamily { at, .. },
            )
            | TextError::Time { at }
            | TextError::Day { at }
            | TextError::Corner { at } => *at,
            TextError::Expr(e) => e.at(),
        }
    }
}

/// The reason the automation could not carry out a request.
#[derive(Debug, thiserror::Error)]
pub enum AutomationError {
    /// The text of an argument does not resolve.
    #[error("{arg}: {error}")]
    Text {
        /// The key of the argument or setting.
        arg: &'static str,
        /// What is wrong.
        error: TextError,
    },
    /// The automation holds the most named areas already.
    #[error("there are {cap} areas already")]
    TooManyFences {
        /// The most areas.
        cap: usize,
    },
    /// A file in the host's storage could not be read or written.
    #[error("automation file {path}: {source}")]
    Storage {
        /// The file.
        path: StorePath,
        /// What the storage said.
        source: StorageError,
    },
    /// The saved rules could not be read or written as JSON.
    #[error("automation JSON: {0}")]
    Json(#[from] serde_json::Error),
}

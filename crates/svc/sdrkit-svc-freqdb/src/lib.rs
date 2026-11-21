//! A frequency database: well-known frequencies with names, bandwidths, stacks, categories, tags, scopes, notes and sources,
//! the channels of chosen plans, and the entries of the user. A host reads and changes it through [`FreqDbService`] as an
//! owner. The engine reads [`FreqDb`] for what is on a frequency and for the labels of a spectrum.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod data;
mod db;
mod entry;
mod error;
mod io;
mod labels;
mod service;

pub use data::supplied_entries;
pub use db::{
    parse_frequency, plan_entries, validate, FreqDb, Hit, Query, UserState, MAX_USER_ENTRIES,
};
pub use entry::{Category, Entry};
pub use error::{DbError, LineProblem, Problem};
pub use labels::{
    auto_labels, gather, spectrum_labels, Label, LabelInput, LabelSource, Labels, View,
};
pub use service::{
    ExportFormat, FreqDbCommand, FreqDbService, Which, ENTRIES, ENTRY_SCOPES,
    ENTRY_SETTINGS, ENTRY_TAGS, KEY, PROBLEMS, ROW, SESSION,
};

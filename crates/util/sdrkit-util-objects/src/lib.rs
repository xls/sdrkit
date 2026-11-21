//! Objects that a stack hears over time (aircraft, ships, stations, calls), kept in tables with caps and ages, and published
//! as result tables, map points and events.
//!
//! Every time the crate keeps is signal time in seconds, as the stack counts it in the samples. A [`Stamp`] can also carry the
//! wall-clock time for display.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod activity;
pub mod calls;
pub mod commands;
pub mod datafile;
mod dedup;
mod error;
mod joiner;
mod log;
mod persist;
pub mod publish;
pub mod rows;
mod seen;
pub mod shaped;
mod stats;
mod table;
mod trail;

pub use activity::{Activity, Closed, Touch};
pub use calls::{Call, CallEnd, CallEvent, CallLog, SameCall};
pub use commands::Tables;
pub use dedup::{fingerprint, Dedup};
pub use error::Error;
pub use joiner::{JoinRules, Joinable, Joined, Joiner, Numbering, Part};
pub use log::{LogLine, LogRing};
pub use persist::{Format, Shift};
pub use publish::Object;
pub use seen::{Seen, Stamp};
pub use stats::{Stats, Tally};
pub use table::{Evicted, ObjectTable, RowRef, Upsert};
pub use trail::{Spacing, Trail, TrailPoint};

//! Scanners: an activity scan of a list, the memories or a band; a channel scan judged by the lock profile of a stack; a
//! span-block scan judged by a result of the stack; and a survey of a wide range that looks for keyed carriers before it
//! visits them. Each says its plan before it starts. A host reads and controls them through [`ScanService`] as an owner.
//! The engine steps the scan that runs and feeds a survey its spectrum frames.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod activity;
mod channel;
mod error;
mod hits;
mod host;
mod list;
mod lock;
mod observe;
mod plan;
mod service;
mod span;
mod survey;
mod table;

pub use activity::{ActivityConfig, ActivityScanner};
pub use channel::{ChannelScanConfig, ChannelScanner};
pub use error::{LineProblem, ScanError, ScanProblem};
pub use hits::{Hit, HitLog};
pub use host::{EndReason, Progress, ScanMachine, ScanState, Spectrum, Step, Target};
pub use list::{BandSearch, Channels, ScanEntry, ScanList};
pub use lock::{Channel, EmptyReason, Found, LockTest, ProfileLockTest, Verdict};
pub use observe::{Activity, Observation, Observer, StopOn};
pub use plan::ScanPlan;
pub use sdrkit_svc_sweep::{SpectrumFrame, SpectrumNeed};
pub use service::{
    Action, ExportWhat, MemoryLookup, PlanChannels, ResultsLookup, ScanCommand, ScanKind,
    ScanService, ScanSetting, SessionView, CANDIDATES, ENTRIES, ENTRY_SETTINGS, HITS, HIT_VALUES, KEY, LOCKOUTS,
    OCCUPIED, PROBLEMS, ROW, SESSION, STATUS_FIELDS, TABLE, TABLE_SETTINGS, TABLE_VALUES,
};
pub use span::{blocks, AfterFind, FindConfig, SpanConfig, SpanScanner, MAX_BLOCKS};
pub use survey::{
    CandidateRow, CandidateState, OccupiedChannel, SurveyScanConfig, SurveyScanner,
    CENTRE_GUARD_HZ, FRAME_S, MAX_BINS, MAX_CANDIDATES, MAX_OCCUPIED, MOST_VISITS, USED_SHARE,
};
pub use table::{ScanRow, ScanTable};

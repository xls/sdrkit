//! Recording for sdrkit: audio, IQ and transport stream recordings, the rewind buffer, time-shift, scheduled recordings and
//! the disk space rules, as the owner [`RecordService`], and the recorder and player components of a graph.

#![warn(missing_docs)]

mod audio;
mod components;
mod error;
mod flac;
mod format;
mod iq;
mod rewind;
mod sched;
mod service;
mod sigmf;
mod space;
mod spool;
mod timeshift;
mod ts;
mod tsplay;
mod wav;

pub use audio::{AudioFeed, Gate, Split};
pub use components::{
    register, AudioRecorder, AudioRecorderSetting, GateBlock, IqRecorder, IqRecorderSetting,
    RecorderCommand, RewindCommand, RewindRecorder, RewindSetting, TsFilePlayer, TsPlayCommand,
    TsPlaySetting, TsRecorder, TsRecorderSetting,
};
pub use error::{CronError, FormatError, RecordError, SigmfField};
pub use format::{
    read_recording, AudioContainer, Container, IqContainer, IqSample, MediaKind, Recording,
};
pub use iq::IqFeed;
pub use rewind::RewindFeed;
pub use sched::Outcome;
pub use service::{
    Feed, Problem, RecordCommand, RecordService, RecordSetting, RepeatKind, RunState, KEY,
    RECORDINGS, RECORDING_CUT, RECORDING_ENDED, RECORDING_FAILED, RECORDING_STARTED, RECORD_EVENTS,
    ROW, RUNNING, SCHEDULES, SCHEDULE_RUNS, SESSION, TS_FILES,
};
pub use timeshift::ShiftFeed;
pub use ts::{transport_packets, TsFeed};

//! The record service as an owner: the recordings, the running recordings, the transport stream files and the schedules as
//! tables, and the commands that start and stop recordings. The engine passes data to the recordings through feeds.

use crate::audio::{self, AudioControl, AudioFeed, AudioSpec, Gate, Split};
use crate::error::RecordError;
use crate::format::{
    container_of, read_recording, AudioContainer, Container, IqContainer, IqSample, MediaKind,
};
use crate::iq::{self, IqControl, IqFeed, IqSpec};
use crate::rewind::{self, RewindControl, RewindFeed, RewindSpec, SaveNote, SaveOrder};
use crate::sched::{Job, Outcome, Repeat, SchedAction, Scheduler, Target};
use crate::space;
use crate::spool::{Live, Note};
use crate::timeshift::{self, ShiftFeed, ShiftSpec, TimeShift};
use crate::ts::{self, TsControl, TsFeed, TsSpec};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::event::{Event, EventDef};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::record::Kind;
use sdrkit_core::settings::{Argument, SettingError, SettingSet};
use sdrkit_core::stack::StageError;
use sdrkit_core::TuneTarget;
use sdrkit_core::{
    Clock, HostRequest, Owner, OwnerKey, Record, Results, ResultsDecl, Schema, Setting, SettingId,
    Storage, StorageError, StorePath, Value, Values,
};
use sdrkit_proto_mpegts::packet::TS_SIZE;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// The owner key of the record service.
pub const KEY: OwnerKey = OwnerKey::of("service.record");
const OWNER: &str = "service.record";
/// The saved schedules.
const SCHEDULE_FILE: &str = "record/schedules.json";
/// The folder of the time-shift segments, under the recordings folder.
const SHIFT_FOLDER: &str = "timeshift";
const MAX_RECORDINGS: usize = 100_000;
const MAX_RUNNING: usize = 64;
const MAX_JOBS: usize = 1000;
const MAX_RUNS: usize = 500;
const MIB: i64 = 1 << 20;

sdrkit_core::choice_enum! {
    /// The state of a running recording.
    pub enum RunState {
        /// A file is open and data goes to it.
        Recording = ("recording", "Recording"),
        /// It waits for a transmission, or for its first data.
        Waiting = ("waiting", "Waiting"),
        /// It ends: the last data goes to storage.
        Ending = ("ending", "Ending"),
        /// A write failed. It ends.
        Failed = ("failed", "Failed"),
    }
}

sdrkit_core::choice_enum! {
    /// Why storage failed.
    pub enum Problem {
        /// No problem.
        None = ("none", "None"),
        /// The storage is full, or the free space fell below the reserve.
        NoSpace = ("no_space", "No space"),
        /// A file or folder is not there.
        NotFound = ("not_found", "Not found"),
        /// The storage refused access.
        Denied = ("denied", "Denied"),
        /// A file of that name exists.
        AlreadyExists = ("already_exists", "Already exists"),
        /// A name is not a valid path.
        InvalidPath = ("invalid_path", "Invalid path"),
        /// Another input or output error.
        Io = ("io", "Input or output error"),
    }
}

impl Problem {
    /// The problem of a storage error.
    pub fn of(e: &StorageError) -> Problem {
        match e {
            StorageError::NotFound => Problem::NotFound,
            StorageError::AlreadyExists => Problem::AlreadyExists,
            StorageError::NoSpace => Problem::NoSpace,
            StorageError::Denied => Problem::Denied,
            StorageError::InvalidPath(_) => Problem::InvalidPath,
            StorageError::Io { .. } => Problem::Io,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How a schedule repeats.
    pub enum RepeatKind {
        /// Once.
        Once = ("once", "Once"),
        /// Every period.
        Every = ("every", "Every period"),
        /// Every day at the local time of the start.
        Daily = ("daily", "Daily"),
        /// On the chosen weekdays at the local time of the start.
        Weekly = ("weekly", "Weekly"),
        /// By a cron rule.
        Cron = ("cron", "Cron rule"),
    }
}

// Settings.
pub(crate) const FOLDER: Setting<field::Text> = Setting::new("folder", "Folder", "", "recordings")
    .help("The folder of the recordings in the storage of the host, as a relative path.");
pub(crate) const PREFIX: Setting<field::Text> = Setting::new("prefix", "Name prefix", "", "sdrkit");
pub(crate) const UTC_OFFSET: Setting<field::I64> =
    Setting::new("utc_offset_min", "Local time offset", "min", 0)
        .range(-1440.0, 1440.0, 1.0)
        .help("The offset of local time from UTC, for file names and schedules.");
pub(crate) const RESERVE: Setting<field::I64> = Setting::new("reserve_bytes", "Reserve", "B", 256 * MIB)
    .range(0.0, 1e15, 1.0)
    .help("Bytes that stay free. A recording that would pass it does not start, and one that runs is cut.");
pub(crate) const BUDGET: Setting<field::I64> =
    Setting::new("budget_bytes", "Folder budget", "B", 0)
        .range(0.0, 1e15, 1.0)
        .help("The most bytes that the recordings hold. The oldest go first. 0 sets no budget.");
pub(crate) const QUEUE: Setting<field::I64> =
    Setting::new("queue_bytes", "Write queue", "B", 16 * MIB)
        .range(1e6, 4e9, 1.0)
        .help("Bytes that can wait for storage before data is left out.");
pub(crate) const AUDIO_CONTAINER: Setting<field::Choice<AudioContainer>> =
    Setting::choice("audio_container", "Audio file", AudioContainer::Wav);
pub(crate) const MONO: Setting<field::Bool> =
    Setting::new("mono", "Mono", "", false).help("Mixes the channels of the audio to one.");
pub(crate) const SPLIT: Setting<field::Choice<Split>> =
    Setting::choice("split", "Files", Split::Whole);
pub(crate) const GATE: Setting<field::Choice<Gate>> =
    Setting::choice("gate", "Transmission gate", Gate::Squelch);
pub(crate) const OPEN_DB: Setting<field::F64> =
    Setting::new("open_db", "Open level", "dBFS", -40.0).range(-200.0, 0.0, 0.0);
pub(crate) const CLOSE_DB: Setting<field::F64> =
    Setting::new("close_db", "Close level", "dBFS", -45.0).range(-200.0, 0.0, 0.0);
pub(crate) const PREROLL: Setting<field::Duration> = Setting::new("preroll", "Pre-roll", "s", 0.5)
    .range(0.0, 60.0, 0.0)
    .help("Audio from before a transmission opens.");
pub(crate) const HANG: Setting<field::Duration> = Setting::new("hang", "Hang", "s", 2.0)
    .range(0.0, 600.0, 0.0)
    .help(
        "How long a file stays open after the gate closes. A reopening goes on in the same file.",
    );
pub(crate) const MIN_LENGTH: Setting<field::Duration> =
    Setting::new("min_length", "Shortest kept", "s", 1.0)
        .range(0.0, 3600.0, 0.0)
        .help("A transmission shorter than this, without pre-roll and hang, is deleted.");
pub(crate) const MAX_FILE: Setting<field::Duration> =
    Setting::new("max_file", "Longest file", "s", 0.0)
        .range(0.0, 1e7, 0.0)
        .help("A longer file goes on in a next part. 0 sets no limit.");
pub(crate) const LOG: Setting<field::Bool> = Setting::new("log", "Log of kept files", "", false)
    .help("Adds a line for each kept audio file to the CSV file <prefix>.csv in the folder.");
pub(crate) const TITLE: Setting<field::Text> = Setting::new("title", "Title tag", "", "");
pub(crate) const COMMENT: Setting<field::Text> = Setting::new("comment", "Comment tag", "", "");
pub(crate) const CHANNEL_IQ: Setting<field::Bool> =
    Setting::new("channel_iq", "Channel IQ", "", false)
        .help("Writes the channel IQ beside each audio file.");
pub(crate) const IQ_CONTAINER: Setting<field::Choice<IqContainer>> =
    Setting::choice("iq_container", "IQ file", IqContainer::Sigmf);
pub(crate) const IQ_SAMPLE: Setting<field::Choice<IqSample>> =
    Setting::choice("iq_sample", "IQ samples", IqSample::Cs16);
pub(crate) const TS_DROP_NULL: Setting<field::Bool> =
    Setting::new("ts_drop_null", "Leave out null packets", "", true);
pub(crate) const TS_MAX: Setting<field::I64> =
    Setting::new("ts_max_bytes", "Largest stream file", "B", 0)
        .range(0.0, 1e15, 1.0)
        .help("A larger file goes on in a next part. 0 sets no limit.");
pub(crate) const TS_SIDECAR: Setting<field::Bool> =
    Setting::new("ts_sidecar", "Stream sidecar", "", true)
        .help("Writes a JSON file beside each stream file with its service, part and time.");
pub(crate) const REWIND: Setting<field::Bool> = Setting::new("rewind", "Rewind buffer", "", false)
    .help("Keeps the last seconds of the radio's samples in memory, for a save.");
pub(crate) const REWIND_BUDGET: Setting<field::I64> =
    Setting::new("rewind_bytes", "Rewind memory", "B", 128 * MIB).range(65_536.0, 1e11, 1.0);
pub(crate) const REWIND_LONG: Setting<field::I64> =
    Setting::new("rewind_long_factor", "Centre history factor", "", 0)
        .range(0.0, 1e6, 1.0)
        .zero_means("Off")
        .help("Averages every this many samples into a longer history of the centre of the band.");
pub(crate) const REWIND_LONG_BUDGET: Setting<field::I64> =
    Setting::new("rewind_long_bytes", "Centre history memory", "B", 16 * MIB)
        .range(65_536.0, 1e11, 1.0);
pub(crate) const SHIFT_BUDGET: Setting<field::I64> =
    Setting::new("timeshift_bytes", "Time-shift size", "B", 2048 * MIB).range(65_536.0, 1e15, 1.0);
pub(crate) const SHIFT_SEGMENT: Setting<field::I64> = Setting::new(
    "timeshift_segment_bytes",
    "Time-shift segment",
    "B",
    32 * MIB,
)
.range(32_768.0, 1e12, 1.0);

sdrkit_core::settings! {
    /// The settings of the record service. A recording takes them when it starts.
    pub enum RecordSetting {
        /// The folder of the recordings.
        Folder = FOLDER,
        /// The start of file names.
        Prefix = PREFIX,
        /// The local time offset.
        UtcOffset = UTC_OFFSET,
        /// The bytes that stay free.
        Reserve = RESERVE,
        /// The folder budget.
        Budget = BUDGET,
        /// The write queue.
        Queue = QUEUE,
        /// The audio container.
        AudioContainer = AUDIO_CONTAINER,
        /// Mono audio.
        Mono = MONO,
        /// Whole or per transmission.
        Split = SPLIT,
        /// The transmission gate.
        Gate = GATE,
        /// The open level of the level gate.
        OpenDb = OPEN_DB,
        /// The close level of the level gate.
        CloseDb = CLOSE_DB,
        /// The pre-roll.
        Preroll = PREROLL,
        /// The hang.
        Hang = HANG,
        /// The shortest transmission kept.
        MinLength = MIN_LENGTH,
        /// The longest file.
        MaxFile = MAX_FILE,
        /// The log of kept files.
        Log = LOG,
        /// The title tag.
        Title = TITLE,
        /// The comment tag.
        Comment = COMMENT,
        /// The channel IQ beside audio.
        ChannelIq = CHANNEL_IQ,
        /// The IQ container.
        IqContainer = IQ_CONTAINER,
        /// The IQ sample type.
        IqSample = IQ_SAMPLE,
        /// Null packets left out.
        TsDropNull = TS_DROP_NULL,
        /// The largest stream file.
        TsMax = TS_MAX,
        /// The stream sidecar.
        TsSidecar = TS_SIDECAR,
        /// The rewind buffer.
        Rewind = REWIND,
        /// The rewind memory.
        RewindBudget = REWIND_BUDGET,
        /// The centre history factor.
        RewindLong = REWIND_LONG,
        /// The centre history memory.
        RewindLongBudget = REWIND_LONG_BUDGET,
        /// The time-shift size.
        ShiftBudget = SHIFT_BUDGET,
        /// The time-shift segment size.
        ShiftSegment = SHIFT_SEGMENT,
    }
}

// Results.
const FREE: Field<field::I64> = Field::new("free_bytes", "Free", "B");
const USED: Field<field::I64> = Field::new("used_bytes", "Recordings", "B");
const REWIND_S: Field<field::Duration> = Field::new("rewind_s", "Rewind held", "s");
const REWIND_CAP: Field<field::Duration> = Field::new("rewind_capacity_s", "Rewind capacity", "s");
const SHIFT_BEHIND: Field<field::Duration> = Field::new("timeshift_behind_s", "Behind live", "s");
const SHIFT_HELD: Field<field::Duration> = Field::new("timeshift_held_s", "Time-shift held", "s");
const SHIFT_PAUSED: Field<field::Bool> = Field::new("timeshift_paused", "Paused", "");

/// What a recording holds.
pub const KIND: Field<field::Choice<MediaKind>> = Field::new("kind", "Kind", "");
const CONTAINER: Field<field::Choice<Container>> = Field::new("container", "Container", "");
const BYTES: Field<field::I64> = Field::new("bytes", "Size", "B");
/// The length of a recording.
pub const SECONDS: Field<field::Duration> = Field::new("seconds", "Length", "s");
const RATE: Field<field::Hz> = Field::new("rate_hz", "Rate", "Hz");
const CENTRE: Field<field::Hz> = Field::new("centre_hz", "Centre", "Hz");
const CHANNELS: Field<field::I64> = Field::new("channels", "Channels", "");
const MODIFIED: Field<field::Time> = Field::new("modified", "Modified", "s");
const SESSION_F: Field<field::Text> =
    Field::new("session", "Session", "").refers_to("engine", "sessions");
const STATE: Field<field::Choice<RunState>> = Field::new("state", "State", "");
/// The file of a recording: its row key in the recordings table.
pub const FILE: Field<field::Text> = Field::new("file", "File", "").refers_to(OWNER, "recordings");
const FILES: Field<field::I64> = Field::new("files", "Files kept", "");
const FRAMES: Field<field::I64> = Field::new("frames", "Written", "");
const DROPPED: Field<field::I64> = Field::new("dropped", "Left out", "");
/// Why storage failed.
pub const PROBLEM: Field<field::Choice<Problem>> = Field::new("problem", "Problem", "");
const STARTED: Field<field::Time> = Field::new("started", "Started", "s");
/// The running recording: its row key in the running table.
pub const RECORDING: Field<field::Text> =
    Field::new("recording", "Recording", "").refers_to(OWNER, "running");
const SERVICE: Field<field::I64> = Field::new("service", "Service", "");
const PART: Field<field::I64> = Field::new("part", "Part", "");
const OPEN: Field<field::Bool> = Field::new("open", "Open", "");
const NAME: Field<field::Text> = Field::new("name", "Name", "");
const ENABLED: Field<field::Bool> = Field::new("enabled", "Enabled", "");
const FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
const START: Field<field::Time> = Field::new("start", "Start", "s");
const DURATION: Field<field::Duration> = Field::new("duration", "Length", "s");
const REPEAT: Field<field::Choice<RepeatKind>> = Field::new("repeat", "Repeat", "");
const PERIOD: Field<field::Duration> = Field::new("period", "Period", "s");
const DAYS: Field<field::Text> = Field::new("days", "Weekdays", "");
const CRON: Field<field::Text> = Field::new("cron", "Cron rule", "");
const PAD_BEFORE: Field<field::Duration> = Field::new("pad_before", "Start early", "s");
const PAD_AFTER: Field<field::Duration> = Field::new("pad_after", "End late", "s");
const PRIORITY: Field<field::I64> = Field::new("priority", "Priority", "");
const RESOURCE: Field<field::Text> = Field::new("resource", "Resource", "");
const NEXT: Field<field::Time> = Field::new("next", "Next", "s");
const RUNS: Field<field::I64> = Field::new("runs", "Runs", "");
const JOB: Field<field::Text> = Field::new("job", "Schedule", "").refers_to(OWNER, "schedules");
const END: Field<field::Time> = Field::new("end", "End", "s");
const OUTCOME: Field<field::Choice<Outcome>> = Field::new("outcome", "Outcome", "");

/// The recording files in the folder. The row key is the store path of the data file.
pub const RECORDINGS: TableDef = TableDef {
    key: "recordings",
    label: "Recordings",
    columns: &[
        &KIND, &CONTAINER, &BYTES, &SECONDS, &RATE, &CENTRE, &CHANNELS, &MODIFIED,
    ],
    cap: MAX_RECORDINGS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The recordings that run. The row key is a number.
pub const RUNNING: TableDef = TableDef {
    key: "running",
    label: "Running recordings",
    columns: &[
        &KIND, &SESSION_F, &STATE, &FILE, &FILES, &FRAMES, &DROPPED, &BYTES, &PROBLEM, &STARTED,
    ],
    cap: MAX_RUNNING,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The files of the running transport stream recordings. The row key is the store path.
pub const TS_FILES: TableDef = TableDef {
    key: "ts_files",
    label: "Stream files",
    columns: &[&RECORDING, &SERVICE, &PART, &BYTES, &OPEN],
    cap: MAX_RECORDINGS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The scheduled recordings. The row key is the schedule id.
pub const SCHEDULES: TableDef = TableDef {
    key: "schedules",
    label: "Schedules",
    columns: &[
        &NAME,
        &ENABLED,
        &KIND,
        &SESSION_F,
        &FREQ,
        &STACK,
        &START,
        &DURATION,
        &REPEAT,
        &PERIOD,
        &DAYS,
        &CRON,
        &PAD_BEFORE,
        &PAD_AFTER,
        &PRIORITY,
        &RESOURCE,
        &NEXT,
        &RUNS,
    ],
    cap: MAX_JOBS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The history of the scheduled runs, newest last. The row key is a number.
pub const SCHEDULE_RUNS: TableDef = TableDef {
    key: "schedule_runs",
    label: "Scheduled runs",
    columns: &[&JOB, &NAME, &START, &END, &OUTCOME, &FILE, &BYTES],
    cap: MAX_RUNS,
    marks: sdrkit_core::RowMarks::NONE,
};

// Events.
/// A recording file opened.
pub static RECORDING_STARTED: EventDef = EventDef::new(
    "record.started",
    "Recording started",
    &[&RECORDING, &KIND, &FILE],
);
/// A recording file closed and was kept.
pub static RECORDING_ENDED: EventDef = EventDef::new(
    "record.ended",
    "Recording ended",
    &[&RECORDING, &KIND, &FILE, &SECONDS, &BYTES],
);
/// A write failed. The recording ends, and its file keeps what came before.
pub static RECORDING_FAILED: EventDef = EventDef::new(
    "record.failed",
    "Recording failed",
    &[&RECORDING, &KIND, &FILE, &PROBLEM],
);
/// The free space fell below the reserve, and the recording was cut.
pub static RECORDING_CUT: EventDef = EventDef::new(
    "record.cut",
    "Recording cut",
    &[&RECORDING, &KIND, &FILE, &FREE],
);
/// Every event kind of the service, for the engine's catalog.
pub static RECORD_EVENTS: [&EventDef; 4] = [
    &RECORDING_STARTED,
    &RECORDING_ENDED,
    &RECORDING_FAILED,
    &RECORDING_CUT,
];

// Arguments.
/// The row key that a command acts on.
pub const ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Row", "")).help("The row key, as the table shows it.");
/// The session to record: its row key in the engine's sessions table.
pub const SESSION: Argument<field::Text> = Argument::new(SESSION_F);
const A_LABEL: Setting<field::Text> =
    Setting::new("label", "Label", "", "").help("A word added to the file names.");
const A_SECONDS: Setting<field::Duration> = Setting::new("seconds", "Length", "s", 0.0)
    .range(0.0, 1e7, 0.0)
    .help("The recording ends after this. 0 records until it is stopped.");
const A_SERVICES: Setting<field::Text> = Setting::new("services", "Services", "", "")
    .help("Service ids separated by commas, one file each. Empty records the multiplex.");
const A_PAUSED: Setting<field::Bool> = Setting::new("paused", "Paused", "", true);
const A_BEHIND: Argument<field::Duration> =
    Argument::new(Field::new("behind_s", "Behind live", "s"));
pub(crate) const A_PRE: Setting<field::Duration> =
    Setting::new("pre_s", "Before", "s", 30.0).range(0.0, 86_400.0, 0.0);
pub(crate) const A_POST: Setting<field::Duration> =
    Setting::new("post_s", "After", "s", 0.0).range(0.0, 86_400.0, 0.0);
pub(crate) const A_LONG: Setting<field::Bool> = Setting::new("long", "Centre history", "", true);
pub(crate) const A_REASON: Setting<field::Text> = Setting::new("reason", "Reason", "", "button");
const A_NAME: Argument<field::Text> = Argument::new(NAME);
const A_KIND: Setting<field::Choice<MediaKind>> = Setting::choice("kind", "Kind", MediaKind::Audio);
const A_FREQ: Setting<field::Hz> = Setting::new("freq_hz", "Frequency", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .zero_means("No change")
    .help("The frequency to tune to first.");
const A_STACK: Setting<field::Text> = Setting::new("stack", "Stack", "", "")
    .help("The stack to start first. Empty keeps the running stack.");
const A_START: Argument<field::Time> = Argument::new(START);
const A_DURATION: Argument<field::Duration> = Argument::new(DURATION);
const A_REPEAT: Setting<field::Choice<RepeatKind>> =
    Setting::choice("repeat", "Repeat", RepeatKind::Once);
const A_PERIOD: Setting<field::Duration> =
    Setting::new("period", "Period", "s", 3600.0).range(1.0, 1e9, 0.0);
const A_DAYS: Setting<field::Text> = Setting::new("days", "Weekdays", "", "")
    .help("Weekdays separated by commas: mon, tue, wed, thu, fri, sat, sun.");
const A_CRON: Setting<field::Text> = Setting::new("cron", "Cron rule", "", "")
    .help("Minute, hour, day of month, month and weekday, or @daily and the like.");
const A_PAD_BEFORE: Setting<field::Duration> =
    Setting::new("pad_before", "Start early", "s", 0.0).range(0.0, 86_400.0, 0.0);
const A_PAD_AFTER: Setting<field::Duration> =
    Setting::new("pad_after", "End late", "s", 0.0).range(0.0, 86_400.0, 0.0);
const A_PRIORITY: Setting<field::I64> =
    Setting::new("priority", "Priority", "", 0).range(-1000.0, 1000.0, 1.0);
const A_RESOURCE: Setting<field::Text> = Setting::new("resource", "Resource", "", "radio")
    .help("The radio or tuner it needs. Jobs of one resource share it by priority.");

// Replies.
const R_RECORDING: Field<field::Text> =
    Field::new("recording", "Recording", "").refers_to(OWNER, "running");
const R_ROW: Field<field::Text> = Field::new("row", "Schedule", "").refers_to(OWNER, "schedules");

const C_AUDIO_START: Command = Command::new("audio.start", "Record audio")
    .help("Records the audio of the session with the audio settings.")
    .args(&[&SESSION, &A_LABEL, &A_SECONDS])
    .reply(&[&R_RECORDING]);
const C_AUDIO_STOP: Command = Command::new("audio.stop", "Stop audio recording").args(&[&ROW]);
const C_IQ_START: Command = Command::new("iq.start", "Record IQ")
    .help("Records the samples of the session with the IQ settings.")
    .args(&[&SESSION, &A_SECONDS])
    .reply(&[&R_RECORDING]);
const C_IQ_STOP: Command = Command::new("iq.stop", "Stop IQ recording").args(&[&ROW]);
const C_TS_START: Command = Command::new("ts.start", "Record stream")
    .help("Records the transport stream of the session: the multiplex, or one file per service.")
    .args(&[&SESSION, &A_SERVICES])
    .reply(&[&R_RECORDING]);
const C_TS_STOP: Command = Command::new("ts.stop", "Stop stream recording")
    .help("Stops a stream recording or the time-shift buffer.")
    .args(&[&ROW]);
const C_SHIFT_START: Command = Command::new("timeshift.start", "Start time-shift")
    .help("Buffers the transport stream of the session on storage for pause and seek.")
    .args(&[&SESSION])
    .reply(&[&R_RECORDING]);
const C_SHIFT_PAUSE: Command = Command::new("timeshift.pause", "Pause").args(&[&A_PAUSED]);
const C_SHIFT_SEEK: Command = Command::new("timeshift.seek", "Seek")
    .help("Plays from this many seconds behind live, limited to what the buffer holds.")
    .args(&[&A_BEHIND]);
const C_SHIFT_LIVE: Command = Command::new("timeshift.live", "Back to live");
const C_REWIND_SAVE: Command = Command::new("rewind.save", "Save rewind")
    .help("Saves the last seconds of the rewind buffer and the seconds that follow as a SigMF recording.")
    .args(&[&A_PRE, &A_POST, &A_LONG, &A_REASON])
    .reply(&[&R_RECORDING]);
const C_SCHEDULE_ADD: Command = Command::new("schedule.add", "Add schedule")
    .help("Adds a timed recording. When it is due, the session is tuned and the recording starts.")
    .args(&[
        &A_NAME,
        &A_KIND,
        &SESSION,
        &A_FREQ,
        &A_STACK,
        &A_START,
        &A_DURATION,
        &A_REPEAT,
        &A_PERIOD,
        &A_DAYS,
        &A_CRON,
        &A_PAD_BEFORE,
        &A_PAD_AFTER,
        &A_PRIORITY,
        &A_RESOURCE,
    ])
    .reply(&[&R_ROW]);
const C_SCHEDULE_REMOVE: Command = Command::new("schedule.remove", "Remove schedule").args(&[&ROW]);
const C_DELETE: Command = Command::new("delete", "Delete recording")
    .help("Deletes a recording file and the metadata beside it.")
    .args(&[&ROW]);

sdrkit_core::commands! {
    /// The commands of the record service.
    pub enum RecordCommand {
        /// Starts an audio recording.
        AudioStart = C_AUDIO_START,
        /// Stops an audio recording.
        AudioStop = C_AUDIO_STOP,
        /// Starts an IQ recording.
        IqStart = C_IQ_START,
        /// Stops an IQ recording.
        IqStop = C_IQ_STOP,
        /// Starts a transport stream recording.
        TsStart = C_TS_START,
        /// Stops a transport stream recording or the time-shift buffer.
        TsStop = C_TS_STOP,
        /// Starts the time-shift buffer.
        ShiftStart = C_SHIFT_START,
        /// Pauses or resumes time-shift playback.
        ShiftPause = C_SHIFT_PAUSE,
        /// Moves time-shift playback.
        ShiftSeek = C_SHIFT_SEEK,
        /// Moves time-shift playback to live.
        ShiftLive = C_SHIFT_LIVE,
        /// Saves the rewind buffer.
        RewindSave = C_REWIND_SAVE,
        /// Adds a schedule.
        ScheduleAdd = C_SCHEDULE_ADD,
        /// Removes a schedule.
        ScheduleRemove = C_SCHEDULE_REMOVE,
        /// Deletes a recording.
        Delete = C_DELETE,
    }
}

/// The feed of a recording, for the engine to hand it data on the signal path.
#[derive(Debug)]
pub enum Feed {
    /// Audio of a session.
    Audio(Box<AudioFeed>),
    /// IQ samples of a session.
    Iq(Box<IqFeed>),
    /// The transport stream of a session.
    Ts(Box<TsFeed>),
    /// The transport stream of a session, for the time-shift buffer.
    TimeShift(Box<ShiftFeed>),
    /// The samples of the radio, for the rewind buffer.
    Rewind(Box<RewindFeed>),
}

/// The control side of one running recording.
enum Side {
    Audio(AudioControl),
    Iq(IqControl),
    Ts(TsControl, Vec<Option<u16>>),
    Shift(Box<TimeShift>),
    Save,
}

struct Running {
    id: u64,
    key: String,
    kind: MediaKind,
    session: String,
    side: Side,
    open: Vec<Option<StorePath>>,
    last_file: Option<StorePath>,
    files: u64,
    bytes: u64,
    problem: Problem,
    state: RunState,
    started: f64,
    run_id: Option<u64>,
    gaps: u64,
    done: bool,
}

impl Running {
    fn live(&self) -> Option<&Live> {
        match &self.side {
            Side::Audio(c) => Some(&c.live),
            Side::Iq(c) => Some(&c.live),
            Side::Ts(c, _) => Some(&c.live),
            Side::Shift(_) | Side::Save => None,
        }
    }
    /// The bytes in the files of the recording, the open one included, if a writer of its own writes them.
    fn written(&self) -> Option<u64> {
        match &self.side {
            Side::Audio(c) => Some(c.writer.written()),
            Side::Iq(c) => Some(c.writer.written()),
            Side::Ts(c, _) => Some(c.writer.written()),
            Side::Shift(_) | Side::Save => None,
        }
    }
    fn note(&mut self) -> Option<Note> {
        match &mut self.side {
            Side::Audio(c) => c.writer.note(),
            Side::Iq(c) => c.writer.note(),
            Side::Ts(c, _) => c.writer.note(),
            Side::Shift(_) | Side::Save => None,
        }
    }
    fn stop(&mut self) {
        if let Some(l) = self.live() {
            l.stop.store(true, Ordering::Relaxed);
        }
        if let Side::Shift(s) = &mut self.side {
            s.stop();
            self.done = true;
        }
        if self.state != RunState::Failed {
            self.state = RunState::Ending;
        }
    }
    fn file(&self) -> Option<&StorePath> {
        self.open
            .iter()
            .flatten()
            .next()
            .or(self.last_file.as_ref())
    }
}

/// The record service.
pub struct RecordService {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    p: Published,
    running: Vec<Running>,
    next_id: u64,
    feeds: Vec<Feed>,
    requests: Vec<HostRequest>,
    events: Vec<Event>,
    rewind: Option<RewindControl>,
    sched: Scheduler,
    run_rows: u64,
}

impl std::fmt::Debug for RecordService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordService")
            .field("running", &self.running.len())
            .finish_non_exhaustive()
    }
}

fn decl() -> Arc<ResultsDecl> {
    let mut d = ResultsDecl::new().with_fields(&[
        &FREE,
        &USED,
        &REWIND_S,
        &REWIND_CAP,
        &SHIFT_BEHIND,
        &SHIFT_HELD,
        &SHIFT_PAUSED,
    ]);
    for t in [&RECORDINGS, &RUNNING, &TS_FILES, &SCHEDULES, &SCHEDULE_RUNS] {
        d = d.with_table(t);
    }
    Arc::new(RECORD_EVENTS.iter().fold(d, |d, e| d.event(e)))
}

fn count(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn failed(e: RecordError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// The error of an argument that the service refuses: [`CommandError::BadArgument`] with `error` made from its id.
fn refused<K: Kind>(
    args: &Values,
    a: &Argument<K>,
    error: impl Fn(SettingId) -> SettingError,
) -> CommandError {
    match args.schema().id(a.field.key) {
        Ok(arg) => CommandError::BadArgument {
            arg,
            error: error(arg),
        },
        Err(_) => CommandError::NotAnArgument { key: a.field.key },
    }
}

/// The error of a setting argument that the service refuses.
fn refused_setting<K: sdrkit_core::settings::SettingKind>(
    args: &Values,
    s: &Setting<K>,
    at: usize,
) -> CommandError {
    match args.schema().id(s.field.key) {
        Ok(id) => CommandError::BadArgument {
            arg: id,
            error: SettingError::Malformed { id, at: Some(at) },
        },
        Err(_) => CommandError::NotAnArgument { key: s.field.key },
    }
}

/// Reads a list of numbers separated by commas. The error is the byte position of the first item that does not read.
fn numbers(text: &str) -> Result<Vec<u16>, usize> {
    let mut out = Vec::new();
    let mut at = 0;
    for part in text.split(',') {
        let t = part.trim();
        if !t.is_empty() {
            out.push(t.parse::<u16>().map_err(|_| at)?);
        }
        at += part.len() + 1;
    }
    Ok(out)
}

/// Reads weekdays separated by commas (0 Monday to 6 Sunday). The error is the byte position of the first item that does not
/// read.
fn weekdays(text: &str) -> Result<Vec<u8>, usize> {
    const NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    let mut out = Vec::new();
    let mut at = 0;
    for part in text.split(',') {
        let t = part.trim();
        if !t.is_empty() {
            let d = NAMES
                .iter()
                .position(|n| n.eq_ignore_ascii_case(t))
                .ok_or(at)?;
            out.push(d as u8);
        }
        at += part.len() + 1;
    }
    Ok(out)
}

/// Makes sure that `value`, the value of the folder setting `id`, is a store path.
pub(crate) fn check_folder(id: SettingId, value: &Value) -> Result<(), SettingError> {
    match StorePath::new(FOLDER.read(id, value)?) {
        Ok(_) => Ok(()),
        Err(_) => Err(SettingError::Malformed { id, at: None }),
    }
}

fn int(v: &Values, s: &Setting<field::I64>) -> u64 {
    u64::try_from(v.get(s)).unwrap_or(0)
}

/// The folder of the recordings in `v`.
pub(crate) fn folder_of(v: &Values) -> Result<StorePath, RecordError> {
    StorePath::new(v.get(&FOLDER)).map_err(RecordError::Name)
}

/// How to record audio by the settings `v`, with a `label` in the names, for at most `max_s` seconds (0: no limit).
pub(crate) fn audio_spec(v: &Values, label: &str, max_s: f64) -> Result<AudioSpec, RecordError> {
    Ok(AudioSpec {
        folder: folder_of(v)?,
        prefix: v.get(&PREFIX).into(),
        container: v.get(&AUDIO_CONTAINER),
        mono: v.get(&MONO),
        split: v.get(&SPLIT),
        gate: v.get(&GATE),
        open_db: v.get(&OPEN_DB),
        close_db: v.get(&CLOSE_DB),
        preroll_s: v.get(&PREROLL),
        hang_s: v.get(&HANG),
        min_s: v.get(&MIN_LENGTH),
        max_file_s: v.get(&MAX_FILE),
        max_s,
        log: v.get(&LOG),
        label: label.into(),
        title: v.get(&TITLE).into(),
        comment: v.get(&COMMENT).into(),
        stack_id: String::new(),
        rf_hz: 0.0,
        utc_offset_min: i32::try_from(v.get(&UTC_OFFSET)).unwrap_or(0),
        channel_iq: v.get(&CHANNEL_IQ),
        queue_bytes: int(v, &QUEUE),
    })
}

/// How to record IQ by the settings `v`, for at most `max_s` seconds (0: no limit).
pub(crate) fn iq_spec(v: &Values, max_s: f64) -> Result<IqSpec, RecordError> {
    Ok(IqSpec {
        folder: folder_of(v)?,
        prefix: v.get(&PREFIX).into(),
        container: v.get(&IQ_CONTAINER),
        sample: v.get(&IQ_SAMPLE),
        stack_id: String::new(),
        utc_offset_min: i32::try_from(v.get(&UTC_OFFSET)).unwrap_or(0),
        max_s,
        queue_bytes: int(v, &QUEUE),
    })
}

/// How to record a transport stream by the settings `v`: the `services`, or the multiplex when it is empty.
pub(crate) fn ts_spec(v: &Values, services: Vec<u16>) -> Result<TsSpec, RecordError> {
    Ok(TsSpec {
        folder: folder_of(v)?,
        prefix: v.get(&PREFIX).into(),
        services,
        drop_null: v.get(&TS_DROP_NULL),
        rf_hz: 0.0,
        max_bytes: int(v, &TS_MAX),
        sidecar: v.get(&TS_SIDECAR),
        utc_offset_min: i32::try_from(v.get(&UTC_OFFSET)).unwrap_or(0),
        queue_bytes: int(v, &QUEUE),
    })
}

/// The limits of the rewind buffer by the settings `v`.
pub(crate) fn rewind_spec(v: &Values) -> RewindSpec {
    RewindSpec {
        budget_bytes: usize::try_from(int(v, &REWIND_BUDGET)).unwrap_or(usize::MAX),
        chunk_samples: 16_384,
        long_factor: u32::try_from(int(v, &REWIND_LONG)).unwrap_or(0),
        long_budget_bytes: usize::try_from(int(v, &REWIND_LONG_BUDGET)).unwrap_or(usize::MAX),
    }
}

/// A rewind save by the settings `v` and the arguments of the save command `args`. The id and the time are left at 0.
pub(crate) fn save_order(v: &Values, args: &Values) -> Result<SaveOrder, RecordError> {
    Ok(SaveOrder {
        id: 0,
        folder: folder_of(v)?,
        prefix: format!("{}-rewind", v.get(&PREFIX)),
        pre_s: args.get(&A_PRE),
        post_s: args.get(&A_POST),
        long: args.get(&A_LONG),
        reason: args.get(&A_REASON).into(),
        utc_offset_min: i32::try_from(v.get(&UTC_OFFSET)).unwrap_or(0),
        now_unix: 0.0,
    })
}

impl RecordService {
    /// Opens the service on the host's `storage`, with the host's wall `clock` for file names and schedules. It lists the
    /// recordings in the folder and reads the saved schedules.
    ///
    /// # Errors
    ///
    /// [`RecordError::Storage`] if the storage fails, [`RecordError::Saved`] if the saved schedules do not read.
    pub fn open(storage: Arc<dyn Storage>, clock: Clock) -> Result<Self, RecordError> {
        let path = StorePath::new(SCHEDULE_FILE).map_err(RecordError::Name)?;
        let sched = match storage.read(&path) {
            Ok(bytes) => Scheduler::from_json(&bytes)?,
            Err(StorageError::NotFound) => Scheduler::new(),
            Err(e) => return Err(RecordError::storage(&path, e)),
        };
        let mut s = RecordService {
            storage,
            clock,
            values: Values::defaults(&RecordSetting::schema()),
            p: Published::from_decl(decl()),
            running: Vec::new(),
            next_id: 1,
            feeds: Vec::new(),
            requests: Vec::new(),
            events: Vec::new(),
            rewind: None,
            sched,
            run_rows: 0,
        };
        s.list_recordings()?;
        s.publish_schedules();
        Ok(s)
    }

    /// Takes the feeds of the recordings that started since the last call. The engine passes each feed the data of its session
    /// and drops it once it has ended.
    pub fn take_feeds(&mut self) -> Vec<Feed> {
        std::mem::take(&mut self.feeds)
    }

    /// Takes the requests to the engine that schedules made since the last call (tune a session, start a stack).
    pub fn take_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.requests)
    }

    /// Advances the service to wall time `now_unix`: reads what the writers report, applies the disk space rules, runs the
    /// schedules, and appends the events to `events`. The engine calls it about once a second.
    pub fn tick(&mut self, now_unix: f64, events: &mut Vec<Event>) {
        self.poll(now_unix);
        self.space_rules(now_unix);
        self.run_schedules(now_unix);
        self.publish_status();
        events.append(&mut self.events);
    }

    /// The packets of time-shift playback due at wall time `now_unix`, at most `max`, appended to `out`. Packets come out at
    /// the pace of their PCR, so the engine can call it at any interval. Returns true if more are due now.
    pub fn read_timeshift(
        &mut self,
        now_unix: f64,
        out: &mut Vec<[u8; TS_SIZE]>,
        max: usize,
    ) -> bool {
        self.shift()
            .is_some_and(|s| s.read(now_unix, out, max) == crate::tsplay::ReadState::More)
    }

    fn shift(&mut self) -> Option<&mut TimeShift> {
        self.running.iter_mut().find_map(|r| match &mut r.side {
            Side::Shift(s) if !r.done => Some(&mut **s),
            _ => None,
        })
    }

    fn folder(&self) -> Result<StorePath, RecordError> {
        StorePath::new(self.values.get(&FOLDER)).map_err(RecordError::Name)
    }

    fn queue(&self) -> u64 {
        u64::try_from(self.values.get(&QUEUE)).unwrap_or(1 << 20)
    }

    fn offset(&self) -> i32 {
        i32::try_from(self.values.get(&UTC_OFFSET)).unwrap_or(0)
    }

    fn reserve(&self) -> u64 {
        u64::try_from(self.values.get(&RESERVE)).unwrap_or(0)
    }

    /// Adds `r` to the running recordings and returns its row key.
    fn add_running(&mut self, kind: MediaKind, session: &str, side: Side, slots: usize) -> String {
        let id = self.next_id;
        self.next_id += 1;
        let key = id.to_string();
        self.running.push(Running {
            id,
            key: key.clone(),
            kind,
            session: session.into(),
            side,
            open: vec![None; slots],
            last_file: None,
            files: 0,
            bytes: 0,
            problem: Problem::None,
            state: RunState::Waiting,
            started: self.clock.now(),
            run_id: None,
            gaps: 0,
            done: false,
        });
        self.publish_running(self.running.len() - 1);
        key
    }

    fn start_audio(
        &mut self,
        session: &str,
        label: &str,
        max_s: f64,
    ) -> Result<String, RecordError> {
        space::check(&*self.storage, self.reserve(), 0)?;
        let spec = audio_spec(&self.values, label, max_s)?;
        let (feed, control) =
            audio::start(spec, session, self.storage.clone(), self.clock.clone())?;
        self.feeds.push(Feed::Audio(Box::new(feed)));
        Ok(self.add_running(MediaKind::Audio, session, Side::Audio(control), 2))
    }

    fn start_iq(&mut self, session: &str, max_s: f64) -> Result<String, RecordError> {
        space::check(&*self.storage, self.reserve(), 0)?;
        let spec = iq_spec(&self.values, max_s)?;
        let (feed, control) = iq::start(spec, session, self.storage.clone(), self.clock.clone())?;
        self.feeds.push(Feed::Iq(Box::new(feed)));
        Ok(self.add_running(MediaKind::Iq, session, Side::Iq(control), 1))
    }

    fn start_ts(&mut self, session: &str, services: Vec<u16>) -> Result<String, RecordError> {
        space::check(&*self.storage, self.reserve(), 0)?;
        let slots: Vec<Option<u16>> = if services.is_empty() {
            vec![None]
        } else {
            services.iter().map(|&s| Some(s)).collect()
        };
        let spec = ts_spec(&self.values, services)?;
        let (feed, control) = ts::start(spec, session, self.storage.clone(), self.clock.clone())?;
        self.feeds.push(Feed::Ts(Box::new(feed)));
        let n = slots.len();
        Ok(self.add_running(MediaKind::Ts, session, Side::Ts(control, slots), n))
    }

    fn start_shift(&mut self, session: &str) -> Result<String, RecordError> {
        let budget = u64::try_from(self.values.get(&SHIFT_BUDGET)).unwrap_or(0);
        let folder = self
            .folder()?
            .join(SHIFT_FOLDER)
            .map_err(RecordError::Name)?;
        space::check(&*self.storage, self.reserve(), budget)?;
        let spec = ShiftSpec {
            folder,
            budget_bytes: budget,
            segment_bytes: u64::try_from(self.values.get(&SHIFT_SEGMENT)).unwrap_or(0),
            queue_bytes: self.queue(),
        };
        let (feed, control) = timeshift::start(spec, session, self.storage.clone())?;
        self.feeds.push(Feed::TimeShift(Box::new(feed)));
        let key = self.add_running(MediaKind::Ts, session, Side::Shift(Box::new(control)), 0);
        if let Some(r) = self.running.last_mut() {
            r.state = RunState::Recording;
        }
        Ok(key)
    }

    /// Turns the rewind buffer on with the settings, or off.
    fn set_rewind(&mut self) {
        if let Some(old) = self.rewind.take() {
            old.live.stop.store(true, Ordering::Relaxed);
        }
        if !self.values.get(&REWIND) {
            return;
        }
        let (feed, control) = rewind::start(rewind_spec(&self.values), self.storage.clone());
        self.feeds.push(Feed::Rewind(Box::new(feed)));
        self.rewind = Some(control);
    }

    fn save_rewind(&mut self, args: &Values) -> Result<String, CommandError> {
        let order = save_order(&self.values, args).map_err(failed)?;
        let Some(control) = &self.rewind else {
            return Err(self.not_now(&REWIND_S));
        };
        let rate = control.live.rate();
        let (pre, post) = (order.pre_s, order.post_s);
        let held = control.live.held.load(Ordering::Relaxed) as f64 / rate.max(1.0);
        let need = ((pre.min(held) + post) * rate * 2.0) as u64;
        space::check(&*self.storage, self.reserve(), need).map_err(failed)?;
        let id = self.next_id;
        let order = SaveOrder {
            id,
            now_unix: self.clock.now(),
            ..order
        };
        rewind::order(control, order).map_err(failed)?;
        Ok(self.add_running(MediaKind::Iq, "", Side::Save, 1))
    }

    fn not_now<K: Kind>(&self, f: &Field<K>) -> CommandError {
        let d = self.p.results().decl();
        CommandError::NotNow {
            field: d.field_id(f.key).unwrap_or(sdrkit_core::FieldId(0)),
            state: Value::None,
        }
    }

    fn no_row(&self, t: &TableDef, key: &str) -> CommandError {
        CommandError::NoSuchRow {
            table: self
                .p
                .results()
                .decl()
                .table_id(t.key)
                .unwrap_or(sdrkit_core::TableId(0)),
            key: key.into(),
        }
    }

    /// Stops running recording `key` if it is of `kind`. A time-shift buffer counts as a stream recording.
    fn stop(&mut self, key: &str, kind: MediaKind) -> Result<(), CommandError> {
        match self
            .running
            .iter_mut()
            .find(|r| r.key == key && r.kind == kind && !matches!(r.side, Side::Save))
        {
            Some(r) => {
                r.stop();
                Ok(())
            }
            None => Err(self.no_row(&RUNNING, key)),
        }
    }

    /// Reads what the writers report.
    fn poll(&mut self, now: f64) {
        let mut changed = Vec::new();
        for i in 0..self.running.len() {
            while let Some(n) = self.running[i].note() {
                self.on_note(i, n);
                changed.push(i);
            }
            let r = &mut self.running[i];
            if let Some(live) = r.live() {
                let ended = live.ended.load(Ordering::Relaxed);
                let gate = live.gate_open.load(Ordering::Relaxed);
                if ended && r.state == RunState::Recording {
                    r.state = RunState::Ending;
                } else if !ended && matches!(r.state, RunState::Recording | RunState::Waiting) {
                    r.state = if r.open.iter().any(Option::is_some)
                        && (gate || r.kind != MediaKind::Audio)
                    {
                        RunState::Recording
                    } else {
                        RunState::Waiting
                    };
                }
            }
            changed.push(i);
        }
        self.poll_saves();
        changed.dedup();
        for i in changed {
            if i < self.running.len() {
                self.publish_running(i);
            }
        }
        let mut i = 0;
        while i < self.running.len() {
            if self.running[i].done {
                let r = self.running.remove(i);
                self.p.remove_row(&RUNNING, &r.key);
                if let Side::Ts(_, _) = r.side {
                    for f in r.open.iter().flatten().chain(r.last_file.iter()) {
                        self.p.remove_row(&TS_FILES, f.as_str());
                    }
                }
                if let Some(run) = r.run_id {
                    let file = r.last_file.as_ref().map_or("", |f| f.as_str());
                    self.sched
                        .report(run, now as i64, file, r.bytes, r.state == RunState::Failed);
                    self.publish_schedules();
                }
            } else {
                i += 1;
            }
        }
    }

    fn on_note(&mut self, i: usize, n: Note) {
        match n {
            Note::Opened { slot, path } => {
                let r = &mut self.running[i];
                if let Some(s) = r.open.get_mut(slot) {
                    *s = Some(path.clone());
                }
                if slot == 0 || r.kind == MediaKind::Ts {
                    let e = RECORDING_STARTED
                        .event(self.clock.now(), 0.0)
                        .set(&RECORDING, &r.key)
                        .set(&KIND, r.kind)
                        .set(&FILE, path.as_str());
                    self.events.push(e);
                }
                if let Side::Ts(_, services) = &r.side {
                    let service = services.get(slot).copied().flatten();
                    let part = r.files + 1;
                    let key = r.key.clone();
                    if let Some(mut row) = self.p.row_mut(&TS_FILES, path.as_str()) {
                        row.set(&RECORDING, &key);
                        row.opt(&SERVICE, service.map(i64::from));
                        row.set(&PART, count(part));
                        row.set(&BYTES, 0);
                        row.set(&OPEN, true);
                    }
                }
            }
            Note::Closed {
                slot,
                path,
                bytes,
                kept,
            } => {
                let r = &mut self.running[i];
                if let Some(s) = r.open.get_mut(slot) {
                    *s = None;
                }
                if !kept {
                    return;
                }
                r.files += 1;
                r.bytes += bytes;
                if slot == 0 || r.kind == MediaKind::Ts {
                    r.last_file = Some(path.clone());
                }
                let (key, kind) = (r.key.clone(), r.kind);
                if let Side::Ts(_, _) = r.side {
                    if let Some(mut row) = self.p.row_mut(&TS_FILES, path.as_str()) {
                        row.set(&BYTES, count(bytes));
                        row.set(&OPEN, false);
                    }
                }
                let seconds = self.publish_recording(&path);
                if slot == 0 || kind == MediaKind::Ts {
                    let e = RECORDING_ENDED
                        .event(self.clock.now(), 0.0)
                        .set(&RECORDING, &key)
                        .set(&KIND, kind)
                        .set(&FILE, path.as_str())
                        .opt(&SECONDS, seconds)
                        .set(&BYTES, count(bytes));
                    self.events.push(e);
                }
                self.trim();
            }
            Note::Failed { path, error, .. } => {
                let r = &mut self.running[i];
                r.problem = Problem::of(&error);
                r.stop();
                r.state = RunState::Failed;
                let e = RECORDING_FAILED
                    .event(self.clock.now(), 0.0)
                    .set(&RECORDING, &r.key)
                    .set(&KIND, r.kind)
                    .set(&FILE, path.as_str())
                    .set(&PROBLEM, r.problem);
                self.events.push(e);
            }
            Note::Done => self.running[i].done = true,
        }
    }

    fn poll_saves(&mut self) {
        let Some(control) = &self.rewind else {
            return;
        };
        let notes: Vec<SaveNote> = control.notes.try_iter().collect();
        for n in notes {
            let id = match &n {
                SaveNote::Opened { id, .. }
                | SaveNote::Done { id, .. }
                | SaveNote::Failed { id, .. } => *id,
            };
            let Some(i) = self.running.iter().position(|r| r.id == id) else {
                continue;
            };
            match n {
                SaveNote::Opened { path, .. } => self.on_note(i, Note::Opened { slot: 0, path }),
                SaveNote::Done {
                    path,
                    long,
                    samples,
                    gaps,
                    ..
                } => {
                    self.running[i].gaps = gaps;
                    let bytes = samples * 2;
                    self.on_note(
                        i,
                        Note::Closed {
                            slot: 0,
                            path,
                            bytes,
                            kept: true,
                        },
                    );
                    if let Some(l) = long {
                        self.publish_recording(&l);
                    }
                    self.running[i].done = true;
                }
                SaveNote::Failed { path, error, .. } => {
                    self.on_note(i, Note::Failed { path, error });
                    self.running[i].done = true;
                }
            }
        }
    }

    /// Cuts the recordings when the free space falls below the reserve.
    fn space_rules(&mut self, now: f64) {
        let free = self.storage.free_space().ok().flatten();
        self.p.opt(&FREE, free.map(count));
        let Some(free) = free else {
            return;
        };
        if free >= self.reserve() {
            return;
        }
        for r in self.running.iter_mut().filter(|r| {
            matches!(r.state, RunState::Recording | RunState::Waiting)
                && !matches!(r.side, Side::Save)
        }) {
            r.stop();
            r.problem = Problem::NoSpace;
            let file = r.file().map_or("", |f| f.as_str()).to_string();
            let e = RECORDING_CUT
                .event(now, 0.0)
                .set(&RECORDING, &r.key)
                .set(&KIND, r.kind)
                .set(&FILE, &file)
                .set(&FREE, count(free));
            self.events.push(e);
        }
    }

    /// Keeps the recordings folder within the budget, oldest first. Files of running recordings stay.
    fn trim(&mut self) {
        let budget = u64::try_from(self.values.get(&BUDGET)).unwrap_or(0);
        let Ok(folder) = self.folder() else { return };
        if budget > 0 {
            let open: Vec<StorePath> = self
                .running
                .iter()
                .flat_map(|r| r.open.iter().flatten().cloned())
                .collect();
            if let Ok(rep) = space::trim(&*self.storage, &folder, budget, |p| open.contains(p)) {
                for f in &rep.deleted {
                    self.p.remove_row(&RECORDINGS, f.as_str());
                }
            }
        }
        if let Ok(used) = space::folder_bytes(&*self.storage, &folder) {
            self.p.set(&USED, count(used));
        }
    }

    /// Lists the recordings in the folder into the recordings table.
    fn list_recordings(&mut self) -> Result<(), RecordError> {
        let folder = self.folder()?;
        let entries = match self.storage.list(&folder) {
            Ok(e) => e,
            Err(StorageError::NotFound) => Vec::new(),
            Err(e) => return Err(RecordError::storage(&folder, e)),
        };
        self.p.clear_rows(&RECORDINGS);
        for e in entries.into_iter().filter(|e| !e.is_dir) {
            let Ok(path) = folder.join(&e.name) else {
                continue;
            };
            if container_of(&path).is_some() && path.extension() != Some("sigmf-meta") {
                self.publish_recording(&path);
            }
        }
        if let Ok(used) = space::folder_bytes(&*self.storage, &folder) {
            self.p.set(&USED, count(used));
        }
        Ok(())
    }

    /// Writes the row of recording file `path`. Returns its length in seconds, if known.
    fn publish_recording(&mut self, path: &StorePath) -> Option<f64> {
        let rec = read_recording(&*self.storage, path).ok()?;
        let stat = self.storage.stat(&rec.data).ok()?;
        let seconds = rec.seconds();
        let mut row = self.p.row_mut(&RECORDINGS, rec.data.as_str())?;
        row.set(&KIND, rec.kind());
        row.set(&CONTAINER, rec.container);
        row.set(&BYTES, count(stat.size));
        row.opt(&SECONDS, seconds);
        row.opt(&RATE, (rec.format.rate > 0.0).then_some(rec.format.rate));
        row.opt(
            &CENTRE,
            (rec.format.centre_hz > 0.0).then_some(rec.format.centre_hz),
        );
        row.set(&CHANNELS, i64::from(rec.format.channels));
        row.opt(&MODIFIED, stat.modified);
        seconds
    }

    fn publish_running(&mut self, i: usize) {
        let r = &self.running[i];
        let (frames, dropped) = r.live().map_or((r.bytes / 2, r.gaps), |l| {
            (
                l.frames.load(Ordering::Relaxed),
                l.dropped.load(Ordering::Relaxed),
            )
        });
        let file = r.file().map(|f| f.as_str().to_string());
        let (key, kind, session, state, files, bytes, problem, started) = (
            r.key.clone(),
            r.kind,
            r.session.clone(),
            r.state,
            r.files,
            r.written().unwrap_or(r.bytes),
            r.problem,
            r.started,
        );
        if let Some(mut row) = self.p.row_mut(&RUNNING, &key) {
            row.set(&KIND, kind);
            row.set(&SESSION_F, &session);
            row.set(&STATE, state);
            row.opt(&FILE, file.as_deref());
            row.set(&FILES, count(files));
            row.set(&FRAMES, count(frames));
            row.set(&DROPPED, count(dropped));
            row.set(&BYTES, count(bytes));
            row.set(&PROBLEM, problem);
            row.set(&STARTED, started);
        }
    }

    fn publish_status(&mut self) {
        let rewind = self.rewind.as_ref().map(|c| {
            let rate = c.live.rate().max(1.0);
            (
                c.live.held.load(Ordering::Relaxed) as f64 / rate,
                c.live.capacity.load(Ordering::Relaxed) as f64 / rate,
            )
        });
        self.p.opt(&REWIND_S, rewind.map(|r| r.0));
        self.p.opt(&REWIND_CAP, rewind.map(|r| r.1));
        let shift = self.shift().map(TimeShift::status);
        self.p
            .opt(&SHIFT_BEHIND, shift.as_ref().map(|s| s.behind_s));
        self.p
            .opt(&SHIFT_HELD, shift.as_ref().map(|s| s.live_s - s.start_s));
        self.p
            .opt(&SHIFT_PAUSED, shift.as_ref().map(|s| s.play.paused));
    }

    fn publish_schedules(&mut self) {
        let now = self.clock.now() as i64;
        let next: Vec<(String, i64)> = self
            .sched
            .upcoming(now, now + 86_400 * 366)
            .into_iter()
            .map(|o| (o.job_id, o.start))
            .collect();
        self.p.clear_rows(&SCHEDULES);
        for j in self.sched.jobs.clone() {
            let Some(mut row) = self.p.row_mut(&SCHEDULES, &j.id) else {
                continue;
            };
            let (repeat, period, days, cron) = match &j.repeat {
                Repeat::Once => (RepeatKind::Once, None, String::new(), String::new()),
                Repeat::Every { seconds } => (
                    RepeatKind::Every,
                    Some(*seconds as f64),
                    String::new(),
                    String::new(),
                ),
                Repeat::Daily => (RepeatKind::Daily, None, String::new(), String::new()),
                Repeat::Weekly { days } => {
                    const NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
                    let d: Vec<&str> = days
                        .iter()
                        .filter_map(|&d| NAMES.get(usize::from(d)).copied())
                        .collect();
                    (RepeatKind::Weekly, None, d.join(","), String::new())
                }
                Repeat::Cron { rule } => (RepeatKind::Cron, None, String::new(), rule.clone()),
            };
            row.set(&NAME, &j.name);
            row.set(&ENABLED, j.enabled);
            row.set(&KIND, j.target.kind);
            row.set(&SESSION_F, &j.target.session);
            row.opt(&FREQ, j.target.freq_hz);
            row.set(&STACK, &j.target.stack);
            row.set(&START, j.start as f64);
            row.set(&DURATION, j.duration_s as f64);
            row.set(&REPEAT, repeat);
            row.opt(&PERIOD, period);
            row.set(&DAYS, &days);
            row.set(&CRON, &cron);
            row.set(&PAD_BEFORE, j.pad_before_s as f64);
            row.set(&PAD_AFTER, j.pad_after_s as f64);
            row.set(&PRIORITY, i64::from(j.priority));
            row.set(&RESOURCE, &j.resource);
            row.opt(&NEXT, next.iter().find(|n| n.0 == j.id).map(|n| n.1 as f64));
            row.set(&RUNS, i64::from(j.runs));
        }
        let skip = self.sched.history.len().saturating_sub(MAX_RUNS);
        let history: Vec<_> = self.sched.history.iter().skip(skip).cloned().collect();
        self.p.clear_rows(&SCHEDULE_RUNS);
        for (k, h) in history.iter().enumerate() {
            if let Some(mut row) = self.p.row_mut(&SCHEDULE_RUNS, &k.to_string()) {
                row.set(&JOB, &h.job_id);
                row.set(&NAME, &h.name);
                row.set(&START, h.planned.0 as f64);
                row.set(&END, h.planned.1 as f64);
                row.set(&OUTCOME, h.outcome);
                row.opt(&FILE, (!h.file.is_empty()).then_some(h.file.as_str()));
                row.set(&BYTES, count(h.bytes));
            }
        }
        self.run_rows = history.len() as u64;
    }

    fn save_schedules(&self) -> Result<(), RecordError> {
        let path = StorePath::new(SCHEDULE_FILE).map_err(RecordError::Name)?;
        self.storage
            .write(&path, &self.sched.to_json())
            .map_err(|e| RecordError::storage(&path, e))
    }

    /// Starts and stops the scheduled recordings that are due.
    fn run_schedules(&mut self, now: f64) {
        if self.sched.jobs.is_empty() && self.sched.running.is_empty() {
            return;
        }
        self.sched.set_utc_offset(self.offset() * 60);
        let free = self
            .storage
            .free_space()
            .ok()
            .flatten()
            .map(|f| f.saturating_sub(self.reserve()));
        let actions = self.sched.tick(now as i64, free);
        if actions.is_empty() {
            return;
        }
        for a in actions {
            match a {
                SchedAction::Start(run) => {
                    let t = &run.target;
                    if t.freq_hz.is_some() || !t.stack.is_empty() {
                        self.requests.push(HostRequest::Tune {
                            session: t.session.clone(),
                            freq_hz: t.freq_hz,
                            target: TuneTarget::of_stack(&t.stack),
                            submode: String::new(),
                            rate_hz: None,
                            bandwidth_hz: None,
                            settings: Vec::new(),
                        });
                    }
                    let seconds = (run.end - run.start).max(1) as f64;
                    let started = match t.kind {
                        MediaKind::Audio => self.start_audio(&t.session, &run.file_name, seconds),
                        MediaKind::Iq => self.start_iq(&t.session, seconds),
                        MediaKind::Ts => self.start_ts(&t.session, Vec::new()),
                    };
                    match started {
                        Ok(_) => {
                            if let Some(r) = self.running.last_mut() {
                                r.run_id = Some(run.id);
                            }
                        }
                        Err(_) => self.sched.report(run.id, now as i64, "", 0, true),
                    }
                }
                SchedAction::Stop { run_id, .. } => {
                    if let Some(r) = self.running.iter_mut().find(|r| r.run_id == Some(run_id)) {
                        r.stop();
                    }
                }
            }
        }
        // The schedules keep their runs on the next start. A failure here leaves the last saved form.
        let _ = self.save_schedules();
        self.publish_schedules();
    }

    fn add_schedule(&mut self, args: &Values) -> Result<String, CommandError> {
        let repeat = match args.get(&A_REPEAT) {
            RepeatKind::Once => Repeat::Once,
            RepeatKind::Every => Repeat::Every {
                seconds: args.get(&A_PERIOD).round() as i64,
            },
            RepeatKind::Daily => Repeat::Daily,
            RepeatKind::Weekly => Repeat::Weekly {
                days: weekdays(args.get(&A_DAYS))
                    .map_err(|at| refused_setting(args, &A_DAYS, at))?,
            },
            RepeatKind::Cron => Repeat::Cron {
                rule: args.get(&A_CRON).into(),
            },
        };
        let freq = args.get(&A_FREQ);
        let id = format!(
            "s{}",
            self.clock.now().max(0.0) as u64 * 1000 + self.sched.jobs.len() as u64
        );
        let job = Job {
            id: id.clone(),
            name: args.arg(&A_NAME)?.into(),
            start: args.arg(&A_START)?.round() as i64,
            duration_s: args.arg(&A_DURATION)?.round().max(1.0) as i64,
            repeat,
            pad_before_s: args.get(&A_PAD_BEFORE).round() as i64,
            pad_after_s: args.get(&A_PAD_AFTER).round() as i64,
            priority: i32::try_from(args.get(&A_PRIORITY)).unwrap_or(0),
            resource: args.get(&A_RESOURCE).into(),
            target: Target {
                kind: args.get(&A_KIND),
                session: args.arg(&SESSION)?.into(),
                freq_hz: (freq > 0.0).then_some(freq),
                stack: args.get(&A_STACK).into(),
            },
            ..Job::default()
        };
        if self.sched.jobs.len() >= MAX_JOBS {
            return Err(self.not_now(&USED));
        }
        self.sched.upsert(job).map_err(|e| {
            let at = match e {
                crate::error::CronError::Fields => 0,
                crate::error::CronError::Field { at } | crate::error::CronError::Range { at } => at,
            };
            refused_setting(args, &A_CRON, at)
        })?;
        self.save_schedules().map_err(failed)?;
        self.publish_schedules();
        Ok(id)
    }

    fn delete(&mut self, key: &str) -> Result<(), CommandError> {
        if self
            .p
            .results()
            .decl()
            .table_id(RECORDINGS.key)
            .and_then(|t| self.p.results().table(t))
            .and_then(|t| t.row(key))
            .is_none()
        {
            return Err(self.no_row(&RECORDINGS, key));
        }
        let path = StorePath::new(key).map_err(|_| self.no_row(&RECORDINGS, key))?;
        if self
            .running
            .iter()
            .any(|r| r.open.iter().flatten().any(|f| *f == path))
        {
            return Err(self.not_now(&USED));
        }
        let rm = |p: &StorePath| match self.storage.remove(p) {
            Ok(()) | Err(StorageError::NotFound) => Ok(()),
            Err(e) => Err(failed(RecordError::storage(p, e))),
        };
        rm(&path)?;
        for ext in ["sigmf-meta", "json"] {
            if let Ok(side) = path.with_extension(ext) {
                rm(&side)?;
            }
        }
        self.p.remove_row(&RECORDINGS, key);
        if let Ok(folder) = self.folder() {
            if let Ok(used) = space::folder_bytes(&*self.storage, &folder) {
                self.p.set(&USED, count(used));
            }
        }
        Ok(())
    }

    fn run_command(
        &mut self,
        cmd: RecordCommand,
        args: &Values,
    ) -> Result<Option<Record>, CommandError> {
        let mut reply = RecordCommand::commands()
            .def(cmd.id())
            .map(|d| d.new_reply())
            .filter(|r| !r.decl().is_empty());
        let mut answer = |f: &Field<field::Text>, v: &str| {
            if let Some(r) = reply.as_mut() {
                r.set(f, v);
            }
        };
        match cmd {
            RecordCommand::AudioStart => {
                let key = self
                    .start_audio(
                        args.arg(&SESSION)?,
                        args.get(&A_LABEL),
                        args.get(&A_SECONDS),
                    )
                    .map_err(failed)?;
                answer(&R_RECORDING, &key);
            }
            RecordCommand::IqStart => {
                let key = self
                    .start_iq(args.arg(&SESSION)?, args.get(&A_SECONDS))
                    .map_err(failed)?;
                answer(&R_RECORDING, &key);
            }
            RecordCommand::TsStart => {
                let services = numbers(args.get(&A_SERVICES))
                    .map_err(|at| refused_setting(args, &A_SERVICES, at))?;
                let key = self
                    .start_ts(args.arg(&SESSION)?, services)
                    .map_err(failed)?;
                answer(&R_RECORDING, &key);
            }
            RecordCommand::ShiftStart => {
                if self.shift().is_some() {
                    return Err(self.not_now(&SHIFT_HELD));
                }
                let key = self.start_shift(args.arg(&SESSION)?).map_err(failed)?;
                answer(&R_RECORDING, &key);
            }
            RecordCommand::AudioStop => self.stop(args.arg(&ROW)?, MediaKind::Audio)?,
            RecordCommand::IqStop => self.stop(args.arg(&ROW)?, MediaKind::Iq)?,
            RecordCommand::TsStop => self.stop(args.arg(&ROW)?, MediaKind::Ts)?,
            RecordCommand::ShiftPause => {
                let paused = args.get(&A_PAUSED);
                let s = self.shift().ok_or_else(|| CommandError::NotNow {
                    field: sdrkit_core::FieldId(0),
                    state: Value::None,
                })?;
                s.set_paused(paused);
            }
            RecordCommand::ShiftSeek => {
                let behind = args.arg(&A_BEHIND)?;
                if behind < 0.0 {
                    return Err(refused(args, &A_BEHIND, |id| SettingError::OutOfRange {
                        id,
                        min: 0.0,
                        max: f64::MAX,
                        got: behind,
                    }));
                }
                match self.shift() {
                    Some(s) => s.seek_behind(behind),
                    None => return Err(self.not_now(&SHIFT_HELD)),
                }
            }
            RecordCommand::ShiftLive => match self.shift() {
                Some(s) => s.go_live(),
                None => return Err(self.not_now(&SHIFT_HELD)),
            },
            RecordCommand::RewindSave => {
                let key = self.save_rewind(args)?;
                answer(&R_RECORDING, &key);
            }
            RecordCommand::ScheduleAdd => {
                let id = self.add_schedule(args)?;
                answer(&R_ROW, &id);
            }
            RecordCommand::ScheduleRemove => {
                let key = args.arg(&ROW)?;
                if !self.sched.jobs.iter().any(|j| j.id == key) {
                    return Err(self.no_row(&SCHEDULES, key));
                }
                let now = self.clock.now() as i64;
                for a in self.sched.remove(key, now) {
                    if let SchedAction::Stop { run_id, .. } = a {
                        if let Some(r) = self.running.iter_mut().find(|r| r.run_id == Some(run_id))
                        {
                            r.stop();
                        }
                    }
                }
                self.save_schedules().map_err(failed)?;
                self.publish_schedules();
            }
            RecordCommand::Delete => self.delete(args.arg(&ROW)?)?,
        }
        Ok(reply)
    }
}

impl Owner for RecordService {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match RecordSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RecordSetting::Folder => {
                check_folder(id, value)?;
                self.values.set(id, value)?;
                // The table shows the new folder. A folder that does not list shows no rows.
                let _ = self.list_recordings();
                Ok(())
            }
            RecordSetting::Rewind
            | RecordSetting::RewindBudget
            | RecordSetting::RewindLong
            | RecordSetting::RewindLongBudget => {
                self.values.set(id, value)?;
                self.set_rewind();
                Ok(())
            }
            RecordSetting::Prefix
            | RecordSetting::UtcOffset
            | RecordSetting::Reserve
            | RecordSetting::Budget
            | RecordSetting::Queue
            | RecordSetting::AudioContainer
            | RecordSetting::Mono
            | RecordSetting::Split
            | RecordSetting::Gate
            | RecordSetting::OpenDb
            | RecordSetting::CloseDb
            | RecordSetting::Preroll
            | RecordSetting::Hang
            | RecordSetting::MinLength
            | RecordSetting::MaxFile
            | RecordSetting::Log
            | RecordSetting::Title
            | RecordSetting::Comment
            | RecordSetting::ChannelIq
            | RecordSetting::IqContainer
            | RecordSetting::IqSample
            | RecordSetting::TsDropNull
            | RecordSetting::TsMax
            | RecordSetting::TsSidecar
            | RecordSetting::ShiftBudget
            | RecordSetting::ShiftSegment => self.values.set(id, value),
        }
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.p.results().decl()
    }
    fn results(&self) -> &Results {
        self.p.results()
    }
    fn problem(&mut self, text: &str) {
        self.p.results_mut().problem(text);
    }
    fn commands(&self) -> &Commands {
        RecordCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = RecordCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.run_command(cmd, args)
    }
}

impl Drop for RecordService {
    fn drop(&mut self) {
        for r in &mut self.running {
            r.stop();
        }
        if let Some(c) = &self.rewind {
            c.live.stop.store(true, Ordering::Relaxed);
        }
    }
}

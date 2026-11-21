//! The scan service as an owner: the scan list, the hits of the running scan, the candidates of a survey and the saved scan
//! tables as tables, and the commands that start (or plan), control and stop a scan. The engine steps the scan that runs.

use crate::activity::{ActivityConfig, ActivityScanner};
use crate::channel::{ChannelScanConfig, ChannelScanner};
use crate::error::{LineProblem, ScanError, ScanProblem};
use crate::hits::{Hit, HitLog};
use crate::host::{EndReason, Progress, ScanMachine, ScanState, Spectrum, Step, Target};
use crate::list::{BandSearch, Channels, ScanList};
use crate::lock::{Channel, EmptyReason, ProfileLockTest};
use crate::observe::{Activity, Observation, Observer, StopOn};
use crate::plan::ScanPlan;
use crate::span::{AfterFind, FindConfig, SpanConfig, SpanScanner};
use crate::survey::{CandidateState, SurveyScanConfig, SurveyScanner};
use crate::table::{ScanRow, ScanTable};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::record::Kind;
use sdrkit_core::results::RowMut;
use sdrkit_core::settings::{Argument, SettingError, SettingSet};
use sdrkit_core::stack::StageError;
use sdrkit_core::{
    Clock, HostRequest, Owner, OwnerKey, OwnerSetting, Record, Results, ResultsDecl, Schema,
    Setting, SettingId, StackDescriptor, StackLookup, Storage, StorageError, StorePath, TableId,
    TuneTarget, Value, Values,
};
use sdrkit_svc_memory::{FilterKind, Memory, MemoryFilter};
use sdrkit_svc_sweep::{SpectrumFrame, SpectrumNeed};
use std::collections::HashSet;
use std::sync::Arc;

/// The owner key of the scan service.
pub const KEY: OwnerKey = OwnerKey::of("service.scan");
const OWNER: &str = "service.scan";

/// The channels that the host gives, by stack id: the name (empty for a raster point) and the frequency of each
/// channel, lowest first. The host sets them with [`ScanService::set_plan_channels`].
pub type PlanChannels = std::collections::HashMap<String, Vec<(String, f64)>>;

/// What one session receives and shows, as the engine tells the service with [`ScanService::set_view`].
#[derive(Clone, Debug)]
pub struct SessionView {
    /// The frequency where the session listens, Hz.
    pub listened_hz: f64,
    /// The width of the samples of its input, Hz: the rate that arrives, or before the first samples the rate that the
    /// session asked of its radio or the rate of its host feed. 0 if not known.
    pub span_hz: f64,
    /// The id of the stack that the session runs. Empty for none.
    pub stack: String,
    /// The results that the session shows.
    pub results: Arc<ResultsDecl>,
}

/// The views of the sessions, by session key.
type Views = std::collections::HashMap<String, SessionView>;

/// The results that a session of a stack shows, by stack id. The engine supplies it with
/// [`ScanService::set_stack_results`].
pub type ResultsLookup = sdrkit_core::Lookup<Arc<ResultsDecl>>;

/// The most channels of a scan of the whole range of a stack. A stack with a wider range and no channel plan is scanned
/// in what its session receives.
const WHOLE_RANGE_CHANNELS: f64 = 2000.0;
/// The part of the samples of a session, from the middle, that such a scan covers.
const VIEW_SHARE: f64 = 0.9;

/// Returns the memories that a filter takes, for a memory scan. The engine supplies it when it makes the service.
pub type MemoryLookup = Arc<dyn Fn(&MemoryFilter) -> Vec<Memory> + Send + Sync>;

const LIST_FILE: &str = "scan/list.json";
const MAX_ENTRIES: usize = 10_000;
const MAX_HITS: usize = 10_000;
const MAX_ITEMS: usize = 100_000;
const MAX_PAGE: usize = 1000;
const MAX_PROBLEMS: usize = 1000;

sdrkit_core::choice_enum! {
    /// The kind of scan that runs or ran last.
    pub enum ScanKind {
        /// No scan has run.
        None = ("none", "None"),
        /// An activity scan of a list, the memories or a band.
        Activity = ("activity", "Activity"),
        /// A channel scan judged by the lock profile of a stack.
        Channels = ("channels", "Channels"),
        /// A range in blocks of the span of a stack, judged by a result of the stack.
        Span = ("span", "Span blocks"),
        /// A wide range: keyed carriers found in wide blocks, then visited as span blocks.
        Survey = ("survey", "Survey"),
    }
}

sdrkit_core::choice_enum! {
    /// What the command control does to an activity scan.
    pub enum Action {
        /// Leaves the channel now.
        Skip = ("skip", "Skip"),
        /// Locks out the channel for this scan and leaves it.
        Lockout = ("lockout", "Lock out"),
        /// Stays on the channel with sound.
        Pause = ("pause", "Pause"),
        /// Goes on after a pause.
        Resume = ("resume", "Resume"),
        /// Lifts the lockout of the frequency `freq_hz`.
        Unlock = ("unlock", "Unlock"),
    }
}

sdrkit_core::choice_enum! {
    /// What the command export writes.
    pub enum ExportWhat {
        /// The scan list.
        List = ("list", "Scan list"),
        /// The hits of the last activity scan.
        Hits = ("hits", "Hits"),
        /// The saved scan table that the table view shows.
        Table = ("table", "Scan table"),
    }
}

// Settings.
const SETTLE: Setting<field::Duration> = Setting::new("settle", "Settle time", "s", 0.15)
    .range(0.0, 10.0, 0.0)
    .help("After a retune, the time before the channel is judged.");
const DWELL: Setting<field::Duration> = Setting::new("dwell", "Dwell", "s", 0.5)
    .range(0.0, 3600.0, 0.0)
    .help("How long to listen to a quiet channel. The dwell of an entry wins.");
const HANG: Setting<field::Duration> = Setting::new("hang", "Resume delay", "s", 2.0)
    .range(0.0, 3600.0, 0.0)
    .help("How long to stay after activity ends.");
const MAX_HOLD: Setting<field::Duration> = Setting::new("max_hold", "Longest hold", "s", 0.0)
    .range(0.0, 86_400.0, 0.0)
    .help("The longest hold before the scan goes on. 0 holds while the channel is active.");
const STOP_ON: Setting<field::Choice<StopOn>> =
    Setting::choice("stop_on", "Stop on", StopOn::Squelch);
const THRESHOLD: Setting<field::F64> = Setting::new("threshold_db", "Threshold", "dB", 6.0)
    .range(-200.0, 200.0, 0.0)
    .help("The SNR in dB, or the level in dBFS for a stop on level.");
const PRIORITY_INTERVAL: Setting<field::Duration> =
    Setting::new("priority_interval", "Priority interval", "s", 2.0)
        .range(0.1, 3600.0, 0.0)
        .help("How often the priority channel is checked.");
const PRIORITY_DWELL: Setting<field::Duration> =
    Setting::new("priority_dwell", "Priority dwell", "s", 0.3)
        .range(0.0, 60.0, 0.0)
        .help("How long the priority channel is listened to on a check.");
const PASSES: Setting<field::I64> = Setting::new("passes", "Passes", "", 0)
    .range(0.0, 1e6, 1.0)
    .help("The passes over the channels before the scan ends. 0 scans until it is stopped.");
const HIT_CAP: Setting<field::I64> =
    Setting::new("hit_cap", "Most hits", "", 1000).range(1.0, MAX_HITS as f64, 1.0);
const KEEP_EMPTY: Setting<field::Bool> =
    Setting::new("keep_empty", "Keep empty channels", "", false)
        .help("A channel scan keeps the empty channels in its table, with the reason.");
const AFTER_FIND: Setting<field::Choice<AfterFind>> =
    Setting::choice("after_find", "After a find", AfterFind::Resume).help(
        "What a span or survey scan does after a find: stay, or go on once the watched result is quiet.",
    );
const QUIET: Setting<field::Duration> = Setting::new("quiet", "Quiet time", "s", 30.0)
    .range(0.0, 86_400.0, 0.0)
    .help(
        "After a find, how long the watched result must not pass its test before the scan goes on.",
    );
const REJECT_HOLD: Setting<field::Duration> =
    Setting::new("reject_hold", "Rejection hold", "s", 1800.0)
        .range(0.0, 604_800.0, 0.0)
        .help("How long a survey skips candidates near one on which the stack found nothing.");
const LOOK: Setting<field::Duration> = Setting::new("look", "Look time", "s", 2.0)
    .range(1.0, 10.0, 0.0)
    .common(&[2.0, 3.0, 4.0, 6.0])
    .help("How long a survey looks at each wide block for keyed carriers: a longer look finds more slow Morse and more weak stations, and each pass takes longer.");

sdrkit_core::settings! {
    /// The settings of the scan service. A scan takes them when it starts.
    pub enum ScanSetting {
        /// The settle time.
        Settle = SETTLE,
        /// The dwell time.
        Dwell = DWELL,
        /// The resume delay.
        Hang = HANG,
        /// The longest hold.
        MaxHold = MAX_HOLD,
        /// What counts as activity.
        StopOn = STOP_ON,
        /// The threshold of the activity rule.
        Threshold = THRESHOLD,
        /// How often the priority channel is checked.
        PriorityInterval = PRIORITY_INTERVAL,
        /// How long the priority channel is listened to.
        PriorityDwell = PRIORITY_DWELL,
        /// The passes before the scan ends.
        Passes = PASSES,
        /// The most hits kept.
        HitCap = HIT_CAP,
        /// A channel scan keeps empty channels.
        KeepEmpty = KEEP_EMPTY,
        /// What a span or survey scan does after a find.
        AfterFind = AFTER_FIND,
        /// The quiet time before a span or survey scan goes on after a find.
        Quiet = QUIET,
        /// How long a survey skips candidates near a rejected one.
        RejectHold = REJECT_HOLD,
        /// How long a survey looks at each wide block.
        Look = LOOK,
    }
}

// Results.
const STATE: Field<field::Choice<ScanState>> = Field::new("state", "State", "");
const KIND: Field<field::Choice<ScanKind>> = Field::new("kind", "Kind", "");
const INDEX: Field<field::I64> = Field::new("index", "Channel", "");
const COUNT: Field<field::I64> = Field::new("count", "Channels", "");
const FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
const NAME: Field<field::Text> = Field::new("name", "Name", "");
const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
const AT_PRIORITY: Field<field::Bool> = Field::new("at_priority", "At priority", "");
const PASS: Field<field::I64> = Field::new("pass", "Pass", "");
const STEPS: Field<field::I64> = Field::new("steps", "Steps", "");
const FOUND: Field<field::I64> = Field::new("found", "Found", "");
const END: Field<field::Choice<EndReason>> = Field::new("end", "End", "");
const SESSION_F: Field<field::Text> =
    Field::new("session", "Session", "").refers_to("engine", "sessions");
const TOTAL: Field<field::I64> = Field::new("total", "Rows of the scan table", "");
const TIME_LEFT: Field<field::Duration> = Field::new("time_left_s", "Time left", "s");
const PASS_TIME: Field<field::Duration> = Field::new("pass_s", "Time of a pass", "s");
const PASSES_F: Field<field::F64> = Field::new("passes", "Passes", "");
const WAITING: Field<field::I64> = Field::new("candidates", "Candidates waiting", "");
const WATCHED: Field<field::F64> = Field::new("watched", "Watched value", "");
const TEXT: Field<field::Text> = Field::new("text", "Text", "");
const SCORE: Field<field::F64> = Field::new("score", "Score", "");
const WPM: Field<field::F64> = Field::new("wpm", "Speed", "WPM");
const CAND_STATE: Field<field::Choice<CandidateState>> = Field::new("state", "State", "");
const UNTIL: Field<field::Time> = Field::new("skip_until", "Skipped until", "s");

const POSITION: Field<field::I64> = Field::new("position", "Position", "");
const SUBMODE: Field<field::Text> = Field::new("submode", "Demodulator", "");
const DWELL_COL: Field<field::Duration> = Field::new("dwell_s", "Dwell", "s");
const LOCKOUT: Field<field::Bool> = Field::new("lockout", "Locked out", "");
const PRIORITY: Field<field::Bool> = Field::new("priority", "Priority", "");
const BANDWIDTH: Field<field::Hz> = Field::new("bandwidth_hz", "Bandwidth", "Hz");
const WIDTH: Field<field::Hz> = Field::new("width_hz", "Width", "Hz");
const LOCKED: Field<field::Bool> = Field::new("locked", "Locked", "");
const SNR: Field<field::F64> = Field::new("snr_db", "SNR", "dB");
const LEVEL: Field<field::F64> = Field::new("level_dbfs", "Level", "dBFS");
const START: Field<field::Time> = Field::new("start", "Start", "s");
const STOP: Field<field::Time> = Field::new("end_time", "End", "s");
const TONE: Field<field::Text> = Field::new("tone", "Tone", "");
const MEMORY: Field<field::Text> =
    Field::new("memory", "Memory", "").refers_to("service.memory", "memories");
const CHANNEL: Field<field::Text> = Field::new("channel", "Channel", "");
const FIRST_SEEN: Field<field::Time> = Field::new("first_seen", "First seen", "s");
const LAST_SEEN: Field<field::Time> = Field::new("last_seen", "Last seen", "s");
const SEEN: Field<field::I64> = Field::new("seen", "Seen", "");
const EMPTY: Field<field::Choice<EmptyReason>> = Field::new("empty", "Empty because", "");
const ITEM_OWNER: Field<field::Text> = Field::new("owner", "Owner", "");
const ITEM_KEY: Field<field::Text> = Field::new("key", "Key", "");
const ITEM_VALUE: Field<field::Any> = Field::new("value", "Value", "");
const OF_ENTRY: Field<field::Text> = Field::new("entry", "Entry", "").refers_to(OWNER, "entries");
const OF_HIT: Field<field::Text> = Field::new("hit", "Hit", "").refers_to(OWNER, "hits");
const OF_ROW: Field<field::Text> = Field::new("row", "Row", "").refers_to(OWNER, "table");
const LINE: Field<field::I64> = Field::new("line", "Line", "");
const PROBLEM: Field<field::Choice<ScanProblem>> = Field::new("kind", "Problem", "");

/// The scan list, in scan order. The row key is the place in the list.
pub const ENTRIES: TableDef = TableDef {
    key: "entries",
    label: "Scan list",
    columns: &[
        &FREQ, &STACK, &SUBMODE, &NAME, &DWELL_COL, &LOCKOUT, &PRIORITY, &BANDWIDTH,
    ],
    cap: MAX_ENTRIES,
};
/// The settings of the entries of [`ENTRIES`]: the owner key, the setting key and the typed value.
pub const ENTRY_SETTINGS: TableDef = TableDef {
    key: "entry_settings",
    label: "Scan list settings",
    columns: &[&OF_ENTRY, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
    cap: MAX_ITEMS,
};
/// What the running or last scan found: the stops of an activity scan, or the channels of a channel scan.
pub const HITS: TableDef = TableDef {
    key: "hits",
    label: "Scan hits",
    columns: &[
        &FREQ, &NAME, &STACK, &LOCKED, &SNR, &LEVEL, &START, &STOP, &PRIORITY, &TONE, &MEMORY,
        &CHANNEL, &EMPTY, &PASS, &TEXT,
    ],
    cap: MAX_HITS,
};
/// The candidates of the running or last survey: what stage one saw and what the visit gave. The row key is the sequence
/// number of the candidate.
pub const CANDIDATES: TableDef = TableDef {
    key: "candidates",
    label: "Survey candidates",
    columns: &[
        &FREQ,
        &SCORE,
        &WPM,
        &SNR,
        &CAND_STATE,
        &PASS,
        &FIRST_SEEN,
        &UNTIL,
    ],
    cap: crate::survey::MAX_CANDIDATES,
};
/// The occupied channels that stage one of the running or last survey saw: power over the noise across 2 to 30 kHz, of
/// any modulation. A lone carrier, a run at the end of a block and the edge of a wider signal are not listed. The row
/// key is the number of the channel; a channel seen again within half its width keeps its row. The rows are not visited.
pub const OCCUPIED: TableDef = TableDef {
    key: "occupied",
    label: "Occupied channels of a survey, 2 to 30 kHz wide; a lone carrier is not listed",
    columns: &[&FREQ, &WIDTH, &SNR, &PASS, &SEEN],
    cap: crate::survey::MAX_OCCUPIED,
};
/// The result fields that the lock profile of the stack keeps for each hit of a channel scan, by key, typed.
pub const HIT_VALUES: TableDef = TableDef {
    key: "hit_values",
    label: "Hit values",
    columns: &[&OF_HIT, &ITEM_KEY, &ITEM_VALUE],
    cap: MAX_ITEMS,
};
/// One page of the saved scan table of the stack that the command table names.
pub const TABLE: TableDef = TableDef {
    key: "table",
    label: "Scan table",
    columns: &[
        &POSITION,
        &FREQ,
        &STACK,
        &CHANNEL,
        &NAME,
        &LOCKED,
        &SNR,
        &LEVEL,
        &FIRST_SEEN,
        &LAST_SEEN,
        &SEEN,
        &EMPTY,
    ],
    cap: MAX_PAGE,
};
/// The settings that the rows of [`TABLE`] were found with.
pub const TABLE_SETTINGS: TableDef = TableDef {
    key: "table_settings",
    label: "Scan table settings",
    columns: &[&OF_ROW, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
    cap: MAX_ITEMS,
};
/// The kept result fields of the rows of [`TABLE`].
pub const TABLE_VALUES: TableDef = TableDef {
    key: "table_values",
    label: "Scan table values",
    columns: &[&OF_ROW, &ITEM_KEY, &ITEM_VALUE],
    cap: MAX_ITEMS,
};
/// The frequencies locked out in the running activity scan.
pub const LOCKOUTS: TableDef = TableDef {
    key: "lockouts",
    label: "Lockouts",
    columns: &[&FREQ],
    cap: MAX_ENTRIES,
};
/// The problems of the last import, one row for each line that was not read.
pub const PROBLEMS: TableDef = TableDef {
    key: "problems",
    label: "Import problems",
    columns: &[&LINE, &PROBLEM],
    cap: MAX_PROBLEMS,
};

// Arguments.
/// The row key of the entry of the scan list that a command acts on.
pub const ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Row", "")).help("The row key, as the table shows it.");
const A_LOCKOUT: Setting<field::Bool> = Setting::new("lockout", "Locked out", "", false);
const A_PRIORITY: Setting<field::Bool> = Setting::new("priority", "Priority", "", false);
/// The session that a scan tunes: its row key in the engine's sessions table.
pub const SESSION: Argument<field::Text> = Argument::new(SESSION_F);
const A_FILTER: Setting<field::Choice<FilterKind>> =
    Setting::choice("filter", "Memories", FilterKind::InScan);
const A_GROUP: Setting<field::Text> = Setting::new("group", "Group", "", "");
const A_LO: Setting<field::Hz> = Setting::new("lo_hz", "From", "Hz", 0.0).range(0.0, 1e12, 0.0);
const A_HI: Setting<field::Hz> = Setting::new("hi_hz", "To", "Hz", 1e12).range(0.0, 1e12, 0.0);
const A_START: Argument<field::Hz> = Argument::new(Field::new("start_hz", "Start", "Hz"));
const A_STOP: Argument<field::Hz> = Argument::new(Field::new("stop_hz", "Stop", "Hz"));
const A_STEP: Argument<field::Hz> = Argument::new(Field::new("step_hz", "Step", "Hz"));
const A_STACK: Setting<field::Text> =
    Setting::new("stack", "Stack", "", "").help("The stack id. Empty for the running stack.");
const A_STACK_REQ: Argument<field::Text> = Argument::new(STACK);
const A_BANDWIDTH: Setting<field::Hz> = Setting::new("bandwidth_hz", "Bandwidth", "Hz", 0.0)
    .range(0.0, 1e10, 0.0)
    .zero_means("The bandwidth of the stack");
const A_ACTION: Argument<field::Choice<Action>> = Argument::new(Field::new("action", "Action", ""));
const A_UNLOCK: Setting<field::Hz> = Setting::new("freq_hz", "Frequency", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .help("The frequency to unlock.");
const A_PATH: Argument<field::Text> = Argument::new(Field::new("path", "File", ""))
    .help("A file in the storage of the host, as a relative path.");
const A_WHAT: Setting<field::Choice<ExportWhat>> =
    Setting::choice("what", "What", ExportWhat::Hits);
const A_LIMIT: Setting<field::I64> =
    Setting::new("limit", "Rows", "", 100).range(1.0, MAX_PAGE as f64, 1.0);
const A_OFFSET: Setting<field::I64> =
    Setting::new("offset", "First row", "", 0).range(0.0, 1e6, 1.0);
const A_ALL: Setting<field::Bool> = Setting::new("all", "Every row", "", false)
    .help("Stores every row, not only the rows where the stack locked.");
const A_DRY: Setting<field::Bool> = Setting::new("dry", "Plan only", "", false)
    .help("Returns the plan of the scan and starts nothing.");
const A_SPAN: Argument<field::Hz> = Argument::new(Field::new("span_hz", "Span", "Hz"))
    .help("The span that the stack watches: the width of a block. The stack gives it as a result.");
const A_WATCH: Argument<field::Text> = Argument::new(Field::new("watch", "Watched result", ""))
    .help("The key of the result field of the stack whose value over `above` is a find: a field that holds a number, or a text that starts with one. The stack is `stack`, else the stack of the session. The command refuses a key that the stack does not have, with the list of the keys it has.");
const A_ABOVE: Setting<field::F64> = Setting::new("above", "Find over", "", 0.0)
    .range(-1e12, 1e12, 0.0)
    .help("A find is a watched value over this.");
const A_BLOCK_DWELL: Setting<field::Duration> = Setting::new("dwell_s", "Dwell", "s", 10.0)
    .range(0.1, 86_400.0, 0.0)
    .help("How long a block or a candidate is held.");
const A_TOTAL: Setting<field::Duration> = Setting::new("total_s", "Total time", "s", 3600.0)
    .range(0.0, 604_800.0, 0.0)
    .zero_means("Until stopped");
const A_RATE: Setting<field::Hz> = Setting::new("rate_hz", "Widest rate", "Hz", 0.0)
    .range(0.0, 1e10, 0.0)
    .zero_means("The width that the session receives");
const A_HIT: Setting<field::Text> =
    Setting::new("hit", "Hit", "", "").help("The row key of the hit. Empty for the newest find.");
const A_TEXT: Argument<field::Text> = Argument::new(Field::new("text", "Text", ""));

// Replies.
const R_CHANNELS: Field<field::I64> = Field::new("channels", "Channels", "");
const R_ADDED: Field<field::I64> = Field::new("added", "Added", "");
const R_PROBLEMS: Field<field::I64> = Field::new("problems", "Problems", "");
const R_WRITTEN: Field<field::I64> = Field::new("written", "Rows written", "");
const R_STORED: Field<field::I64> = Field::new("stored", "Memories asked for", "");
const R_KIND: Field<field::Choice<ScanKind>> = Field::new("kind", "Kind", "");
const R_DWELL: Field<field::Duration> = Field::new("dwell_s", "Dwell", "s");
const R_PASS: Field<field::Duration> = Field::new("pass_s", "Time of a pass", "s");
const R_TOTAL: Field<field::Duration> = Field::new("total_s", "Total time", "s");
const R_PASSES: Field<field::F64> = Field::new("passes", "Passes", "");
const R_VISIT: Field<field::Duration> = Field::new("visit_s", "Time of a visit", "s");
const R_SPAN_PASS: Field<field::Duration> =
    Field::new("span_pass_s", "Time of a pass of span blocks", "s");
/// The reply of every command that starts a scan: the channels or blocks, then the plan ([`ScanPlan`]).
const PLAN_REPLY: &[&dyn sdrkit_core::Declare] = &[
    &R_CHANNELS,
    &R_KIND,
    &R_DWELL,
    &R_PASS,
    &R_TOTAL,
    &R_PASSES,
    &R_VISIT,
    &R_SPAN_PASS,
];

const C_MEMORIES: Command = Command::new("memories", "Scan memories")
    .help("Starts an activity scan of the memories that the filter takes. Their hits are counted on them when it ends.")
    .args(&[&SESSION, &A_FILTER, &A_GROUP, &A_LO, &A_HI, &A_DRY])
    .reply(PLAN_REPLY);
const C_ENTRIES: Command = Command::new("entries", "Scan the list")
    .help("Starts an activity scan of the scan list.")
    .args(&[&SESSION, &A_DRY])
    .reply(PLAN_REPLY);
const C_BAND: Command = Command::new("band", "Scan a band")
    .help("Starts an activity scan of a band, step by step.")
    .args(&[
        &SESSION,
        &A_START,
        &A_STOP,
        &A_STEP,
        &A_STACK,
        &A_BANDWIDTH,
        &A_DRY,
    ])
    .reply(PLAN_REPLY);
const C_CHANNELS: Command = Command::new("channels", "Scan channels")
    .help("Starts a channel scan of a raster, judged by the lock profile of the stack. Its table is merged into the saved table of the stack when it ends.")
    .args(&[&SESSION, &A_STACK_REQ, &A_START, &A_STOP, &A_STEP, &A_DRY])
    .reply(PLAN_REPLY);
const C_START: Command = Command::new("start", "Scan the band of the stack")
    .help("Starts a channel scan of the stack, judged by its lock profile: the channels that the host gave for the stack, else one channel for each bandwidth of the range of the stack. If that range has more than 2000 channels, the scan takes the part of it that the session receives now, around its frequency.")
    .args(&[&SESSION, &A_STACK_REQ, &A_DRY])
    .reply(PLAN_REPLY);
const C_SPAN: Command = Command::new("span", "Scan span blocks")
    .help("Starts a scan of the range from `start_hz` to `stop_hz` in blocks of `span_hz`, each held for `dwell_s` and judged by the result `watch` of the stack: a value over `above` is a find, which holds the scan (the setting after_find). The passes repeat until `total_s` is over.")
    .args(&[
        &SESSION,
        &A_START,
        &A_STOP,
        &A_SPAN,
        &A_WATCH,
        &A_ABOVE,
        &A_STACK,
        &A_BLOCK_DWELL,
        &A_TOTAL,
        &A_DRY,
    ])
    .reply(PLAN_REPLY);
const C_SURVEY: Command = Command::new("survey", "Survey a wide range")
    .help("Starts a survey of the range from `start_hz` to `stop_hz`. Stage one steps the radio in blocks of `rate_hz` and looks for keyed carriers for the time of the setting look each, without the stack; stage two visits each candidate as a span block and lets the result `watch` of the stack decide. Stage one also lists the occupied channels of each block in the table occupied: power 3 dB or more over the local noise across 2 to 30 kHz, of any modulation, with centre, width and level; a lone carrier or spur, a ripple of the passband, a run at the end of a block and the edge of a wider signal are refused, and the rows are not visited. When one pass of span blocks fits into `total_s` (without a total time: when it is not longer than one pass of stage one), the survey runs span blocks; the reply says which. A survey with a `dwell_s` shorter than the look is refused. Without `rate_hz` the survey takes the width that the session receives, which is known as soon as the session has started: before its first samples it is the rate that the session asked of its radio, or the rate of its host feed. A survey can start at once after its session.")
    .args(&[
        &SESSION,
        &A_START,
        &A_STOP,
        &A_SPAN,
        &A_WATCH,
        &A_ABOVE,
        &A_STACK,
        &A_BLOCK_DWELL,
        &A_TOTAL,
        &A_RATE,
        &A_DRY,
    ])
    .reply(PLAN_REPLY);
const C_HIT_TEXT: Command = Command::new("hit.text", "Attach text to a hit")
    .help("Attaches a text, such as the text that the stack decoded, to a find of the running or last span or survey scan.")
    .args(&[&A_TEXT, &A_HIT]);
const C_ENTRY_SET: Command = Command::new("entry.set", "Change list entry")
    .help("Sets the lockout and priority marks of an entry of the scan list.")
    .args(&[&ROW, &A_LOCKOUT, &A_PRIORITY]);
const C_ENTRY_REMOVE: Command = Command::new("entry.remove", "Delete list entry").args(&[&ROW]);
const C_STOP: Command = Command::new("stop", "Stop scan");
const C_CONTROL: Command = Command::new("control", "Control scan")
    .help("Skips, locks out, pauses or resumes the running activity scan, or unlocks a frequency. A span or survey scan takes skip, pause and resume (resume also leaves a find); a survey takes lockout as the rejection of the candidate that it visits.")
    .args(&[&A_ACTION, &A_UNLOCK]);
const C_IMPORT: Command = Command::new("import", "Import scan list")
    .help("Replaces the scan list with the CSV file `path`. The problems table lists the lines not read.")
    .args(&[&A_PATH])
    .reply(&[&R_ADDED, &R_PROBLEMS]);
const C_EXPORT: Command = Command::new("export", "Export")
    .help("Writes the scan list, the hits or the shown scan table to the CSV file `path`.")
    .args(&[&A_PATH, &A_WHAT])
    .reply(&[&R_WRITTEN]);
const C_TABLE: Command = Command::new("table", "Show scan table")
    .help("Sets which saved scan table the table view shows, and the page.")
    .args(&[&A_STACK_REQ, &A_LIMIT, &A_OFFSET]);
const C_STORE: Command = Command::new("store", "Store as memories")
    .help("Asks the engine to store the rows of the saved scan table of the stack as memories in the group.")
    .args(&[&A_STACK_REQ, &A_GROUP, &A_ALL])
    .reply(&[&R_STORED]);

sdrkit_core::commands! {
    /// The commands of the scan service.
    pub enum ScanCommand {
        /// Starts a scan of the memories.
        Memories = C_MEMORIES,
        /// Starts a scan of the scan list.
        Entries = C_ENTRIES,
        /// Starts a scan of a band.
        Band = C_BAND,
        /// Starts a channel scan.
        Channels = C_CHANNELS,
        /// Starts a channel scan of the band of a stack.
        Start = C_START,
        /// Changes the marks of an entry of the scan list.
        EntrySet = C_ENTRY_SET,
        /// Deletes an entry of the scan list.
        EntryRemove = C_ENTRY_REMOVE,
        /// Stops the scan.
        Stop = C_STOP,
        /// Controls the running activity scan.
        Control = C_CONTROL,
        /// Imports a scan list.
        Import = C_IMPORT,
        /// Exports the list, the hits or a table.
        Export = C_EXPORT,
        /// Sets which scan table is shown.
        Table = C_TABLE,
        /// Stores a scan table as memories.
        Store = C_STORE,
        /// Starts a span-block scan.
        Span = C_SPAN,
        /// Starts a survey.
        Survey = C_SURVEY,
        /// Attaches text to a hit.
        HitText = C_HIT_TEXT,
    }
}

/// The scanner that runs.
enum Machine {
    Activity(Box<ActivityScanner>),
    Channel(Box<ChannelScanner>),
    Span(Box<SpanScanner>),
    Survey(Box<SurveyScanner>),
}

impl Machine {
    fn get(&self) -> &dyn ScanMachine {
        match self {
            Machine::Activity(m) => &**m,
            Machine::Channel(m) => &**m,
            Machine::Span(m) => &**m,
            Machine::Survey(m) => &**m,
        }
    }
    fn get_mut(&mut self) -> &mut dyn ScanMachine {
        match self {
            Machine::Activity(m) => &mut **m,
            Machine::Channel(m) => &mut **m,
            Machine::Span(m) => &mut **m,
            Machine::Survey(m) => &mut **m,
        }
    }
    /// The finds of a span or survey scan.
    fn finds(&self) -> Option<&HitLog> {
        match self {
            Machine::Span(m) => Some(m.hits()),
            Machine::Survey(m) => Some(m.hits()),
            _ => None,
        }
    }
}

/// What a start command requested. The scanner is made at the first step, when the clock of the engine is known.
enum Plan {
    Activity(Channels),
    Channel {
        channels: Vec<Channel>,
        stack: &'static StackDescriptor,
    },
    Span(SpanConfig),
    Survey(SurveyScanConfig),
}

/// A scan that a command set up and the engine runs.
struct Run {
    session: String,
    scan_plan: ScanPlan,
    plan: Option<Plan>,
    machine: Option<Machine>,
    pending: Option<Step>,
    finished: bool,
}

/// The table view: which saved table, and the page.
#[derive(Clone, Debug, Default)]
struct View {
    stack: String,
    limit: usize,
    offset: usize,
    table: ScanTable,
}

/// The field ids of the progress fields, found once.
#[derive(Clone, Copy, Debug)]
struct Ids {
    state: sdrkit_core::FieldId,
    entries: TableId,
    hits: TableId,
}

/// The scan service.
pub struct ScanService {
    storage: Arc<dyn Storage>,
    clock: Clock,
    stacks: StackLookup,
    memories: MemoryLookup,
    plan_channels: PlanChannels,
    views: Views,
    stack_results: ResultsLookup,
    requests: Vec<HostRequest>,
    values: Values,
    list: ScanList,
    run: Option<Run>,
    observer: Observer,
    obs: Observation,
    now_s: f64,
    shown: Option<Progress>,
    hits_shown: u64,
    hits_first: u64,
    candidates_shown: u64,
    view: View,
    p: Published,
    ids: Ids,
}

impl std::fmt::Debug for ScanService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanService")
            .field("entries", &self.list.entries.len())
            .field("running", &self.run.is_some())
            .finish()
    }
}

/// The status fields of the scan service, in the order of its results.
pub const STATUS_FIELDS: [&dyn sdrkit_core::Declare; 19] = [
    &STATE,
    &KIND,
    &INDEX,
    &COUNT,
    &FREQ,
    &NAME,
    &STACK,
    &AT_PRIORITY,
    &PASS,
    &STEPS,
    &FOUND,
    &END,
    &SESSION_F,
    &TOTAL,
    &TIME_LEFT,
    &PASS_TIME,
    &PASSES_F,
    &WAITING,
    &WATCHED,
];

fn decl() -> Arc<ResultsDecl> {
    let mut d = ResultsDecl::new().with_fields(&STATUS_FIELDS);
    for t in [
        &ENTRIES,
        &ENTRY_SETTINGS,
        &HITS,
        &HIT_VALUES,
        &TABLE,
        &TABLE_SETTINGS,
        &TABLE_VALUES,
        &LOCKOUTS,
        &PROBLEMS,
        &CANDIDATES,
        &OCCUPIED,
    ] {
        d = d.with_table(t);
    }
    Arc::new(d)
}

fn path(p: &'static str) -> StorePath {
    StorePath::new(p).expect("the file names of the service are store paths")
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Writes the count `n` into the field `f` of a reply.
fn put(reply: &mut Option<Record>, f: &Field<field::I64>, n: usize) {
    if let Some(r) = reply.as_mut() {
        r.set(f, count(n));
    }
}

fn failed(e: ScanError) -> CommandError {
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

fn file_arg(args: &Values) -> Result<StorePath, CommandError> {
    StorePath::new(args.arg(&A_PATH)?)
        .map_err(|_| refused(args, &A_PATH, |id| SettingError::Malformed { id, at: None }))
}

fn table_path(stack: &str) -> Option<StorePath> {
    StorePath::new(&format!("scan/tables/{stack}.json")).ok()
}

impl ScanService {
    /// Opens the service on the host's `storage`, with the host's wall `clock` for the times of hits, the `stacks` of the
    /// engine for the lock profiles, and the `memories` that a memory scan reads. A list file that does not parse is left and
    /// the service starts with an empty list.
    ///
    /// # Errors
    ///
    /// [`ScanError::Storage`] if the storage fails.
    pub fn open(
        storage: Arc<dyn Storage>,
        clock: Clock,
        stacks: StackLookup,
        memories: MemoryLookup,
    ) -> Result<Self, ScanError> {
        let list_path = path(LIST_FILE);
        let (list, unread) = match storage.read(&list_path) {
            Ok(bytes) => match serde_json::from_slice::<ScanList>(&bytes) {
                Ok(l) => (l, false),
                Err(_) => (ScanList::default(), true),
            },
            Err(StorageError::NotFound) => (ScanList::default(), false),
            Err(source) => {
                return Err(ScanError::Storage {
                    path: list_path,
                    source,
                })
            }
        };
        let p = Published::from_decl(decl());
        let d = p.results().decl();
        let ids = Ids {
            state: d
                .field_id(STATE.key)
                .expect("the service declares its fields"),
            entries: d
                .table_id(ENTRIES.key)
                .expect("the service declares its tables"),
            hits: d
                .table_id(HITS.key)
                .expect("the service declares its tables"),
        };
        let mut s = ScanService {
            storage,
            clock,
            stacks,
            memories,
            plan_channels: PlanChannels::new(),
            views: Views::new(),
            stack_results: Arc::new(|_: &str| None),
            requests: Vec::new(),
            values: Values::defaults(&ScanSetting::schema()),
            list,
            run: None,
            observer: Observer::default(),
            obs: Observation::default(),
            now_s: 0.0,
            shown: None,
            hits_shown: 0,
            hits_first: 0,
            candidates_shown: 0,
            view: View {
                limit: 100,
                ..View::default()
            },
            p,
            ids,
        };
        if unread {
            s.p.results_mut()
                .log("The scan list file did not read. The list starts empty.");
        }
        s.p.set(&STATE, ScanState::Idle);
        s.p.set(&KIND, ScanKind::None);
        s.p.set(&END, EndReason::None);
        s.publish_list();
        Ok(s)
    }

    /// Steps the scan that runs, at `now_s` seconds on the signal clock of the engine. `results` are the results of the tuned
    /// stack, if the engine has new ones. Returns what the engine must do: tune to [`ScanService::target`], or tune back.
    /// The engine calls it at every results snapshot and at its ticks. Once a scan runs, this call allocates nothing until a
    /// hit opens or closes.
    pub fn step(&mut self, now_s: f64, results: Option<&Results>) -> Option<Step> {
        self.now_s = now_s;
        let run = self.run.as_mut().filter(|r| !r.finished)?;
        if let Some(r) = results {
            self.observer.read(r, &mut self.obs);
        }
        let step = if let Some(s) = run.pending.take() {
            Some(s)
        } else if let Some(plan) = run.plan.take() {
            let mut m = make(&self.values, &self.clock, plan, now_s);
            let s = m.get_mut().start(now_s);
            run.machine = Some(m);
            Some(s)
        } else {
            run.machine
                .as_mut()
                .and_then(|m| m.get_mut().tick(now_s, &self.obs, results))
        };
        if step == Some(Step::Done) {
            if let Some(r) = self.run.as_mut() {
                r.finished = true;
            }
            self.finish();
        }
        self.publish_progress();
        step
    }

    /// Passes the scan that runs a spectrum of the samples of the session it tunes. The engine calls it for each new
    /// frame, before [`ScanService::step`].
    pub fn spectrum(&mut self, now_s: f64, spectrum: &Spectrum<'_>) {
        if let Some(m) = self
            .run
            .as_mut()
            .filter(|r| !r.finished)
            .and_then(|r| r.machine.as_mut())
        {
            m.get_mut().spectrum(now_s, spectrum);
        }
    }

    /// The spectrum frames that the scan that runs needs: a survey needs back-to-back frames of the whole band of the radio
    /// at its widest rate. While this is `Some`, the engine puts such a spectrum on the feed of the session that the scan
    /// tunes and passes every frame, in order, to [`ScanService::frame`].
    pub fn spectrum_need(&self) -> Option<SpectrumNeed> {
        let run = self.run.as_ref().filter(|r| !r.finished)?;
        match (&run.plan, &run.machine) {
            (Some(Plan::Survey(cfg)), _) => Some(cfg.need()),
            (_, Some(Machine::Survey(s))) => s.need(),
            _ => None,
        }
    }

    /// Passes the scan that runs a frame of the spectrum of [`ScanService::spectrum_need`]. The engine calls it for each
    /// frame, in order, before [`ScanService::step`].
    pub fn frame(&mut self, frame: &SpectrumFrame<'_>) {
        if let Some(Machine::Survey(s)) = self
            .run
            .as_mut()
            .filter(|r| !r.finished)
            .and_then(|r| r.machine.as_mut())
        {
            s.frame(frame);
        }
    }

    /// The plan of the scan that runs or ran last.
    pub fn plan(&self) -> Option<ScanPlan> {
        self.run.as_ref().map(|r| r.scan_plan)
    }

    /// The channel of the last [`Step::Tune`].
    pub fn target(&self) -> Option<Target<'_>> {
        self.machine()?.target()
    }

    /// True if sound must play now: a stop is held, or no scan runs.
    pub fn audio_wanted(&self) -> bool {
        self.machine()
            .is_none_or(|m| m.finished() || m.audio_wanted())
    }

    /// The session that the scan tunes, while a scan runs.
    pub fn session(&self) -> Option<&str> {
        self.run
            .as_ref()
            .filter(|r| !r.finished)
            .map(|r| r.session.as_str())
    }

    fn machine(&self) -> Option<&dyn ScanMachine> {
        self.run.as_ref()?.machine.as_ref().map(Machine::get)
    }

    /// Takes the requests to the engine since the last call: the hits of a memory scan, and the memories of the command
    /// store.
    pub fn take_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.requests)
    }
}

/// Makes the scanner of `plan` with the settings `v`, at `now_s` on the signal clock.
fn make(v: &Values, clock: &Clock, plan: Plan, now_s: f64) -> Machine {
    let offset = clock.now() - now_s;
    match plan {
        Plan::Activity(channels) => {
            let max_hold = v.get(&MAX_HOLD);
            let passes = v.get(&PASSES);
            let cfg = ActivityConfig {
                settle_s: v.get(&SETTLE),
                dwell_s: v.get(&DWELL),
                hang_s: v.get(&HANG),
                max_hold_s: (max_hold > 0.0).then_some(max_hold),
                activity: Activity {
                    stop_on: v.get(&STOP_ON),
                    threshold_db: v.get(&THRESHOLD) as f32,
                },
                priority_interval_s: v.get(&PRIORITY_INTERVAL),
                priority_dwell_s: v.get(&PRIORITY_DWELL),
                passes: u32::try_from(passes).ok().filter(|&p| p > 0),
                clock_offset_s: offset,
                hit_cap: usize::try_from(v.get(&HIT_CAP)).unwrap_or(1),
            };
            Machine::Activity(Box::new(ActivityScanner::new(channels, cfg)))
        }
        Plan::Channel { channels, stack } => Machine::Channel(Box::new(ChannelScanner::new(
            channels,
            Box::new(ProfileLockTest::new(stack.lock_profile)),
            ChannelScanConfig {
                keep_empty: v.get(&KEEP_EMPTY),
                clock_offset_s: offset,
            },
        ))),
        Plan::Span(mut cfg) => {
            find_settings(v, offset, &mut cfg.find);
            Machine::Span(Box::new(SpanScanner::new(cfg)))
        }
        Plan::Survey(mut cfg) => {
            find_settings(v, offset, &mut cfg.find);
            cfg.reject_hold_s = v.get(&REJECT_HOLD);
            match SurveyScanner::new(cfg.clone()) {
                Ok(m) => Machine::Survey(Box::new(m)),
                // The command made the same survey: it does not fail here. A span scan of the range stands in.
                Err(_) => Machine::Span(Box::new(SpanScanner::new(SpanConfig {
                    start_hz: cfg.start_hz,
                    stop_hz: cfg.stop_hz,
                    span_hz: 0.0,
                    find: cfg.find,
                }))),
            }
        }
    }
}

/// Fills the parts of `find` that come from the settings `v`, with `offset` from the scan clock to seconds since 1970.
fn find_settings(v: &Values, offset: f64, find: &mut FindConfig) {
    find.settle_s = v.get(&SETTLE);
    find.after = v.get(&AFTER_FIND);
    find.quiet_s = v.get(&QUIET);
    find.clock_offset_s = offset;
    find.hit_cap = usize::try_from(v.get(&HIT_CAP)).unwrap_or(1);
}

impl ScanService {
    /// Asks the engine to count the hits of an activity scan on their memories, or merges the table of a channel scan into
    /// the saved table of its stack.
    fn finish(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let outcome = match m {
            Machine::Activity(a) => {
                self.requests.extend(a.hits().memory_hits());
                Ok(())
            }
            Machine::Channel(c) => {
                let table = c.table().clone();
                self.merge_table(table)
            }
            Machine::Span(_) | Machine::Survey(_) => Ok(()),
        };
        if let Err(e) = outcome {
            self.p
                .results_mut()
                .log(format!("The scan results were not kept: {e}"));
        }
    }

    fn merge_table(&mut self, found: ScanTable) -> Result<(), ScanError> {
        let Some(stack) = found.rows.first().map(|r| r.stack.clone()) else {
            return Ok(());
        };
        let mut saved = self.load_table(&stack)?;
        saved.merge(found);
        self.save_table(&stack, &saved)?;
        if self.view.stack == stack {
            self.view.table = saved;
            self.publish_table();
        }
        Ok(())
    }

    fn load_table(&self, stack: &str) -> Result<ScanTable, ScanError> {
        let Some(path) = table_path(stack) else {
            return Ok(ScanTable::default());
        };
        match self.storage.read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
            Err(StorageError::NotFound) => Ok(ScanTable::default()),
            Err(source) => Err(ScanError::Storage { path, source }),
        }
    }

    fn save_table(&self, stack: &str, t: &ScanTable) -> Result<(), ScanError> {
        let path = table_path(stack).ok_or_else(|| ScanError::NoStack { id: stack.into() })?;
        self.write(
            &path,
            &serde_json::to_vec(t).map_err(|_| ScanError::Encode)?,
        )
    }

    fn save_list(&self) -> Result<(), ScanError> {
        let bytes = serde_json::to_vec(&self.list).map_err(|_| ScanError::Encode)?;
        self.write(&path(LIST_FILE), &bytes)
    }

    fn write(&self, path: &StorePath, bytes: &[u8]) -> Result<(), ScanError> {
        self.storage
            .write(path, bytes)
            .map_err(|source| ScanError::Storage {
                path: path.clone(),
                source,
            })
    }

    /// Writes the progress fields if they changed, and the new hits.
    fn publish_progress(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let pr = m.get().progress();
        let plan = self.run.as_ref().map(|r| r.scan_plan).unwrap_or_default();
        let (left, waiting, watched) = match m {
            Machine::Span(s) => (s.time_left(self.now_s), None, s.watched()),
            Machine::Survey(s) => (
                s.time_left(self.now_s),
                Some(count(s.waiting())),
                s.watched(),
            ),
            _ => (None, None, None),
        };
        {
            let p = &mut self.p;
            p.opt(&TIME_LEFT, left);
            p.opt(&WAITING, waiting);
            p.opt(&WATCHED, watched);
        }
        if self.shown != Some(pr) {
            self.shown = Some(pr);
            let p = &mut self.p;
            p.set(&STATE, pr.state);
            p.set(&INDEX, count(pr.index));
            p.set(&COUNT, count(pr.count));
            p.set(&AT_PRIORITY, pr.at_priority);
            p.set(&PASS, i64::from(pr.pass));
            p.set(&STEPS, i64::try_from(pr.steps).unwrap_or(i64::MAX));
            p.set(&FOUND, count(pr.found));
            p.set(&END, pr.end);
            p.set(&PASS_TIME, plan.pass_s);
            p.set(&PASSES_F, plan.passes);
            if let Some(t) = m.get().target() {
                p.set(&FREQ, t.freq_hz);
                p.set(&NAME, t.name);
                p.set(&STACK, t.stack);
            }
        }
        match m {
            Machine::Activity(a) => {
                if a.hits().pushed() != self.hits_shown {
                    self.publish_hits();
                }
            }
            Machine::Channel(c) => {
                if c.table().rows.len() as u64 != self.hits_shown {
                    self.publish_hits();
                }
            }
            Machine::Span(s) => {
                if s.hits().revision() != self.hits_shown {
                    self.publish_hits();
                }
            }
            Machine::Survey(s) => {
                let hits = s.hits().revision() != self.hits_shown;
                let candidates = s.revision() != self.candidates_shown;
                if hits {
                    self.publish_hits();
                }
                if candidates {
                    self.publish_candidates();
                }
            }
        }
    }

    /// Writes the candidates and the occupied channels of the survey that runs or ran last.
    fn publish_candidates(&mut self) {
        let Some(Machine::Survey(s)) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let p = &mut self.p;
        p.clear_rows(&CANDIDATES);
        for c in s.candidates().collect::<Vec<_>>().into_iter().rev() {
            if let Some(mut r) = p.row_mut(&CANDIDATES, &c.seq.to_string()) {
                r.set(&FREQ, c.freq_hz);
                r.set(&SCORE, c.score);
                r.set(&WPM, c.wpm);
                r.set(&SNR, c.snr_db);
                r.set(&CAND_STATE, c.state);
                r.set(&PASS, i64::from(c.pass));
                r.set(&FIRST_SEEN, c.seen_s);
                r.opt(&UNTIL, c.until_s);
            }
        }
        p.clear_rows(&OCCUPIED);
        for c in s.occupied() {
            if let Some(mut r) = p.row_mut(&OCCUPIED, &c.id.to_string()) {
                r.set(&FREQ, c.freq_hz);
                r.set(&WIDTH, c.width_hz);
                r.set(&SNR, c.snr_db);
                r.set(&PASS, i64::from(c.pass));
                r.set(&SEEN, i64::from(c.seen));
            }
        }
        self.candidates_shown = s.revision();
    }

    fn publish_hits(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let p = &mut self.p;
        match m {
            Machine::Activity(a) => {
                let first = a.hits().pushed() - a.hits().len() as u64;
                if let Some(id) = p.results().decl().table_id(HITS.key) {
                    if let Some(mut t) = p.results_mut().table_mut(id) {
                        for k in self.hits_first..first {
                            t.remove(&k.to_string());
                        }
                    }
                }
                self.hits_first = first;
                let new = self.hits_shown.max(first);
                for (i, h) in a.hits().hits().enumerate().skip((new - first) as usize) {
                    let key = (first + i as u64).to_string();
                    if let Some(mut r) = p.row_mut(&HITS, &key) {
                        write_hit(&mut r, h);
                    }
                }
                p.clear_rows(&LOCKOUTS);
                for hz in a.lockouts() {
                    if let Some(mut r) = p.row_mut(&LOCKOUTS, &format!("{hz:.0}")) {
                        r.set(&FREQ, hz);
                    }
                }
                self.hits_shown = a.hits().pushed();
            }
            Machine::Channel(c) => {
                for row in c.table().rows.iter().rev() {
                    let key = format!("{:.0}", row.freq_hz);
                    if let Some(mut r) = p.row_mut(&HITS, &key) {
                        write_row(&mut r, row);
                        r.set(&START, row.first_seen_s);
                        r.set(&STOP, row.last_seen_s);
                    }
                    values(p, &HIT_VALUES, &OF_HIT, &key, &row.values);
                }
                self.hits_shown = c.table().rows.len() as u64;
            }
            Machine::Span(_) | Machine::Survey(_) => {
                let Some(log) = m.finds() else { return };
                p.clear_rows(&HITS);
                let first = log.pushed() - log.len() as u64;
                for (i, h) in log.hits().enumerate().rev() {
                    if let Some(mut r) = p.row_mut(&HITS, &(first + i as u64).to_string()) {
                        write_hit(&mut r, h);
                    }
                }
                self.hits_shown = log.revision();
            }
        }
    }

    fn publish_list(&mut self) {
        let p = &mut self.p;
        p.clear_rows(&ENTRIES);
        p.clear_rows(&ENTRY_SETTINGS);
        for (i, e) in self.list.entries.iter().enumerate().rev() {
            let key = i.to_string();
            if let Some(mut r) = p.row_mut(&ENTRIES, &key) {
                r.set(&FREQ, e.freq_hz);
                r.set(&STACK, &e.stack);
                r.set(&SUBMODE, &e.submode);
                r.set(&NAME, &e.name);
                r.opt(&DWELL_COL, e.dwell_s);
                r.set(&LOCKOUT, e.lockout);
                r.set(&PRIORITY, e.priority);
                r.opt(&BANDWIDTH, e.bandwidth_hz);
            }
            settings(p, &ENTRY_SETTINGS, &OF_ENTRY, &key, &e.settings);
        }
    }

    fn publish_table(&mut self) {
        let v = &self.view;
        let p = &mut self.p;
        p.set(&TOTAL, count(v.table.rows.len()));
        p.clear_rows(&TABLE);
        p.clear_rows(&TABLE_SETTINGS);
        p.clear_rows(&TABLE_VALUES);
        let page = v.table.rows.iter().skip(v.offset).take(v.limit);
        for (i, row) in page.enumerate().collect::<Vec<_>>().into_iter().rev() {
            let key = format!("{:.0}", row.freq_hz);
            if let Some(mut r) = p.row_mut(&TABLE, &key) {
                r.set(&POSITION, count(i));
                write_row(&mut r, row);
                r.set(&FIRST_SEEN, row.first_seen_s);
                r.set(&LAST_SEEN, row.last_seen_s);
                r.set(&SEEN, i64::from(row.seen));
            }
            settings(p, &TABLE_SETTINGS, &OF_ROW, &key, &row.settings);
            values(p, &TABLE_VALUES, &OF_ROW, &key, &row.values);
        }
    }

    fn publish_problems(&mut self, problems: &[LineProblem]) {
        self.p.clear_rows(&PROBLEMS);
        for (i, pr) in problems.iter().take(MAX_PROBLEMS).enumerate().rev() {
            if let Some(mut r) = self.p.row_mut(&PROBLEMS, &i.to_string()) {
                r.set(&LINE, count(pr.line));
                r.set(&PROBLEM, pr.kind);
            }
        }
    }

    /// Sets up a scan for the engine. A scan that runs ends first.
    fn begin(
        &mut self,
        args: &Values,
        plan: Plan,
        scan_plan: ScanPlan,
    ) -> Result<(), CommandError> {
        let kind = scan_plan.kind;
        self.end_run();
        self.run = Some(Run {
            session: args.arg(&SESSION)?.to_owned(),
            scan_plan,
            plan: Some(plan),
            machine: None,
            pending: None,
            finished: false,
        });
        self.shown = None;
        self.hits_shown = 0;
        self.hits_first = 0;
        self.candidates_shown = 0;
        let p = &mut self.p;
        p.clear_rows(&HITS);
        p.clear_rows(&HIT_VALUES);
        p.clear_rows(&LOCKOUTS);
        p.clear_rows(&CANDIDATES);
        p.clear_rows(&OCCUPIED);
        p.set(&PASS_TIME, scan_plan.pass_s);
        p.set(&PASSES_F, scan_plan.passes);
        p.set(&KIND, kind);
        p.set(&STATE, ScanState::Settle);
        p.set(&END, EndReason::None);
        p.set(&SESSION_F, args.arg(&SESSION)?);
        Ok(())
    }

    /// Stops a scan that runs and keeps its results.
    fn end_run(&mut self) {
        let now = self.now_s;
        let Some(run) = self.run.as_mut().filter(|r| !r.finished) else {
            return;
        };
        run.finished = true;
        if let Some(m) = run.machine.as_mut() {
            match m {
                Machine::Activity(a) => a.stop(now),
                Machine::Channel(c) => c.stop(),
                Machine::Span(s) => s.stop(now),
                Machine::Survey(s) => s.stop(now),
            };
            self.finish();
            self.publish_progress();
        } else {
            self.p.set(&STATE, ScanState::Done);
            self.p.set(&END, EndReason::Stopped);
        }
    }

    fn not_now(&self) -> CommandError {
        let state = self
            .p
            .results()
            .fields()
            .value(self.ids.state)
            .cloned()
            .unwrap_or_default();
        CommandError::NotNow {
            field: self.ids.state,
            state,
        }
    }

    /// Sets the channels of each stack. The command `start` scans them for a stack that they name.
    pub fn set_plan_channels(&mut self, channels: PlanChannels) {
        self.plan_channels = channels;
    }

    /// Sets what session `session` receives and shows, or with `None` forgets the session. The engine calls it when a
    /// session starts, before the start returns, when what it receives or shows changes, and when it ends. The commands
    /// read it: `start` scans what a session receives for a stack that has no channels and a range too wide to scan
    /// whole, a survey takes its rate, and a span or survey scan checks its `watch` against the results.
    pub fn set_view(&mut self, session: &str, view: Option<SessionView>) {
        match view {
            Some(v) => {
                self.views.insert(session.to_owned(), v);
            }
            None => {
                self.views.remove(session);
            }
        }
    }

    /// Sets how the service finds the results that a session of a stack shows, for a span or survey scan that names a
    /// stack other than the one of its session.
    pub fn set_stack_results(&mut self, results: ResultsLookup) {
        self.stack_results = results;
    }

    /// The view of the session of a command.
    fn view(&self, args: &Values) -> Result<&SessionView, CommandError> {
        let session = args.arg(&SESSION)?;
        self.views.get(session).ok_or_else(|| {
            failed(ScanError::NoSession {
                session: session.to_owned(),
            })
        })
    }

    /// Checks the `watch` of a span or survey command against the results of the stack that judges: the stack of
    /// `stack` if given, else that of the session.
    fn check_watch(&self, args: &Values) -> Result<(), CommandError> {
        let view = self.view(args)?;
        let watch = args.arg(&A_WATCH)?;
        let named = args.get(&A_STACK);
        let (stack, decl) = if named.is_empty() || named == view.stack {
            (view.stack.as_str(), Arc::clone(&view.results))
        } else {
            let decl = (self.stack_results)(named).ok_or_else(|| {
                failed(ScanError::NoStack {
                    id: named.to_owned(),
                })
            })?;
            (named, decl)
        };
        if crate::span::watched_field(&decl, watch).is_some() {
            return Ok(());
        }
        Err(failed(ScanError::NoWatch {
            watch: watch.to_owned(),
            stack: stack.to_owned(),
            valid: crate::span::watched_keys(&decl),
        }))
    }

    fn stack(&self, args: &Values, id: &str) -> Result<&'static StackDescriptor, CommandError> {
        (self.stacks)(id)
            .ok_or_else(|| refused(args, &A_STACK_REQ, |id| SettingError::NotPermitted { id }))
    }

    /// Sets up the scan `plan` with the plan `said`, or with `dry` only says it. Returns what the reply states.
    fn start_or_plan(
        &mut self,
        args: &Values,
        plan: Plan,
        said: ScanPlan,
    ) -> Result<ScanPlan, CommandError> {
        if !args.get(&A_DRY) {
            self.begin(args, plan, said)?;
        }
        Ok(said)
    }

    /// The activity plan of `steps` channels with the settings of the service.
    fn activity_plan(&self, steps: usize) -> ScanPlan {
        let v = &self.values;
        ScanPlan::activity(
            steps,
            v.get(&SETTLE),
            v.get(&DWELL),
            u32::try_from(v.get(&PASSES)).unwrap_or(0),
        )
    }

    /// The find settings of a span or survey command, with its `watch` checked; the settings of the service fill the rest
    /// when the scan starts.
    fn find_args(&self, args: &Values) -> Result<FindConfig, CommandError> {
        self.check_watch(args)?;
        Ok(FindConfig {
            stack: args.get(&A_STACK).to_owned(),
            watch: args.arg(&A_WATCH)?.to_owned(),
            above: args.get(&A_ABOVE),
            settle_s: 0.0,
            dwell_s: args.get(&A_BLOCK_DWELL),
            after: AfterFind::Resume,
            quiet_s: 0.0,
            total_s: args.get(&A_TOTAL),
            clock_offset_s: 0.0,
            hit_cap: 1,
        })
    }

    fn run_command(
        &mut self,
        cmd: ScanCommand,
        args: &Values,
    ) -> Result<Option<Record>, CommandError> {
        let mut reply = ScanCommand::commands()
            .def(cmd.id())
            .map(|d| d.new_reply())
            .filter(|r| !r.decl().is_empty());
        let mut said: Option<ScanPlan> = None;
        match cmd {
            ScanCommand::Memories => {
                let filter = match args.get(&A_FILTER) {
                    FilterKind::All => MemoryFilter::All,
                    FilterKind::InScan => MemoryFilter::InScan,
                    FilterKind::Group => MemoryFilter::Group {
                        name: args.get(&A_GROUP).to_owned(),
                    },
                    FilterKind::Range => MemoryFilter::Range {
                        lo_hz: args.get(&A_LO),
                        hi_hz: args.get(&A_HI),
                    },
                    FilterKind::Starred => MemoryFilter::Starred,
                };
                let list = ScanList::from_memories((self.memories)(&filter));
                let n = list.entries.len();
                if n == 0 {
                    return Err(failed(ScanError::Nothing));
                }
                let p = self.activity_plan(n);
                said = Some(self.start_or_plan(
                    args,
                    Plan::Activity(Channels::List(list.entries)),
                    p,
                )?);
            }
            ScanCommand::Entries => {
                let n = self.list.entries.len();
                if n == 0 {
                    return Err(failed(ScanError::Nothing));
                }
                let entries = self.list.entries.clone();
                let p = self.activity_plan(n);
                said =
                    Some(self.start_or_plan(args, Plan::Activity(Channels::List(entries)), p)?);
            }
            ScanCommand::Band => {
                let mut band = BandSearch::new(
                    args.arg(&A_START)?,
                    args.arg(&A_STOP)?,
                    args.arg(&A_STEP)?,
                    args.get(&A_STACK),
                )
                .map_err(failed)?;
                let bw = args.get(&A_BANDWIDTH);
                band.bandwidth_hz = (bw > 0.0).then_some(bw);
                // A band search runs one pass.
                let p = ScanPlan {
                    passes: 1.0,
                    ..self.activity_plan(band.len())
                };
                let p = ScanPlan {
                    total_s: p.pass_s,
                    ..p
                };
                said = Some(self.start_or_plan(args, Plan::Activity(Channels::Band(band)), p)?);
            }
            ScanCommand::Channels | ScanCommand::Start => {
                let stack = self.stack(args, args.arg(&A_STACK_REQ)?)?;
                let planned = self
                    .plan_channels
                    .get(stack.id)
                    .filter(|c| cmd == ScanCommand::Start && !c.is_empty());
                let channels = match planned {
                    Some(c) => c
                        .iter()
                        .map(|(name, hz)| Channel::new(name, *hz, stack.id))
                        .collect(),
                    None => {
                        let (start, stop, step) = if cmd == ScanCommand::Start {
                            let t = &stack.tuning;
                            let whole = (t.max_hz - t.min_hz) / t.bandwidth_hz;
                            let view = self
                                .views
                                .get(args.arg(&SESSION)?)
                                .map(|v| (v.listened_hz, v.span_hz));
                            match view {
                                _ if whole <= WHOLE_RANGE_CHANNELS => {
                                    (t.min_hz, t.max_hz, t.bandwidth_hz)
                                }
                                // Channels on both sides of the frequency of the session, as far as its samples go.
                                Some((at, span)) if span > 0.0 && t.bandwidth_hz > 0.0 => {
                                    let each = (span * VIEW_SHARE / 2.0 / t.bandwidth_hz).floor();
                                    let reach = each * t.bandwidth_hz;
                                    (
                                        (at - reach).max(t.min_hz),
                                        (at + reach).min(t.max_hz),
                                        t.bandwidth_hz,
                                    )
                                }
                                _ => return Err(failed(ScanError::BadBand)),
                            }
                        } else {
                            (args.arg(&A_START)?, args.arg(&A_STOP)?, args.arg(&A_STEP)?)
                        };
                        let steps = (stop - start) / step;
                        if !(steps >= 0.0 && steps < crate::list::MAX_BAND_STEPS as f64) {
                            return Err(failed(ScanError::BadBand));
                        }
                        Channel::raster(start, stop, step, stack.id)
                    }
                };
                let n = channels.len();
                if n == 0 {
                    return Err(failed(ScanError::BadBand));
                }
                let lp = &stack.lock_profile;
                let p = ScanPlan::channels(n, lp.settle_s, lp.timeout_s);
                said = Some(self.start_or_plan(args, Plan::Channel { channels, stack }, p)?);
            }
            ScanCommand::Span | ScanCommand::Survey => {
                let (start, stop) = (args.arg(&A_START)?, args.arg(&A_STOP)?);
                let span_hz = args.arg(&A_SPAN)?;
                let find = self.find_args(args)?;
                let settle = self.values.get(&SETTLE);
                let blocks = crate::span::blocks(start, stop, span_hz).len();
                if blocks == 0 {
                    return Err(failed(ScanError::BadBand));
                }
                let span_plan = ScanPlan::span(blocks, settle, find.dwell_s, find.total_s);
                let span = SpanConfig {
                    start_hz: start,
                    stop_hz: stop,
                    span_hz,
                    find,
                };
                if cmd == ScanCommand::Span {
                    said = Some(self.start_or_plan(args, Plan::Span(span), span_plan)?);
                } else {
                    let rate = match args.get(&A_RATE) {
                        r if r > 0.0 => r,
                        _ => Some(self.view(args)?.span_hz)
                            .filter(|w| *w > 0.0)
                            .ok_or_else(|| failed(ScanError::NoRate))?,
                    };
                    let survey = SurveyScanConfig {
                        start_hz: start,
                        stop_hz: stop,
                        rate_hz: rate,
                        look_s: self.values.get(&LOOK),
                        reject_hold_s: self.values.get(&REJECT_HOLD),
                        find: span.find.clone(),
                    };
                    let looked = SurveyScanner::new(survey.clone())
                        .map_err(|_| failed(ScanError::BadRate))?
                        .blocks()
                        .len();
                    let p = ScanPlan::survey(
                        looked,
                        survey.look_s,
                        settle,
                        span.find.dwell_s,
                        span_plan,
                    );
                    let plan = if p.kind == ScanKind::Span {
                        Plan::Span(span)
                    } else if span.find.dwell_s < survey.look_s {
                        return Err(failed(ScanError::DwellUnderLook {
                            dwell_s: span.find.dwell_s,
                            look_s: survey.look_s,
                        }));
                    } else {
                        Plan::Survey(survey)
                    };
                    said = Some(self.start_or_plan(args, plan, p)?);
                }
            }
            ScanCommand::HitText => {
                let text = args.arg(&A_TEXT)?;
                let key = args.get(&A_HIT);
                let seq = match key {
                    "" => None,
                    k => Some(k.parse::<u64>().map_err(|_| self.no_hit(k))?),
                };
                let done = match self.run.as_mut().and_then(|r| r.machine.as_mut()) {
                    Some(Machine::Span(s)) => s.set_text(seq, text),
                    Some(Machine::Survey(s)) => s.set_text(seq, text),
                    _ => false,
                };
                if !done {
                    return Err(self.no_hit(key));
                }
                self.publish_progress();
            }
            ScanCommand::EntrySet | ScanCommand::EntryRemove => {
                let row = args.arg(&ROW)?;
                let i = row
                    .parse::<usize>()
                    .ok()
                    .filter(|&i| i < self.list.entries.len())
                    .ok_or_else(|| CommandError::NoSuchRow {
                        table: self.ids.entries,
                        key: row.to_owned(),
                    })?;
                if cmd == ScanCommand::EntryRemove {
                    self.list.entries.remove(i);
                } else {
                    let e = &mut self.list.entries[i];
                    e.lockout = args.get(&A_LOCKOUT);
                    e.priority = args.get(&A_PRIORITY);
                }
                self.save_list().map_err(failed)?;
                self.publish_list();
            }
            ScanCommand::Stop => self.end_run(),
            ScanCommand::Control => self.control(args)?,
            ScanCommand::Import => {
                let file = file_arg(args)?;
                let bytes = self
                    .storage
                    .read(&file)
                    .map_err(|source| failed(ScanError::Storage { path: file, source }))?;
                let (mut list, mut problems) = ScanList::from_csv(&String::from_utf8_lossy(&bytes));
                if list.entries.len() > MAX_ENTRIES {
                    list.entries.truncate(MAX_ENTRIES);
                    problems.push(LineProblem {
                        line: 0,
                        kind: ScanProblem::Full,
                    });
                }
                put(&mut reply, &R_ADDED, list.entries.len());
                put(&mut reply, &R_PROBLEMS, problems.len());
                self.list = list;
                self.save_list().map_err(failed)?;
                self.publish_list();
                self.publish_problems(&problems);
            }
            ScanCommand::Export => {
                let path = file_arg(args)?;
                let (text, n) = match args.get(&A_WHAT) {
                    ExportWhat::List => (self.list.to_csv(), self.list.entries.len()),
                    ExportWhat::Table => (self.view.table.to_csv(), self.view.table.rows.len()),
                    ExportWhat::Hits => match self.run.as_ref().and_then(|r| r.machine.as_ref()) {
                        Some(Machine::Activity(a)) => (a.hits().to_csv(), a.hits().len()),
                        Some(Machine::Channel(c)) => (c.table().to_csv(), c.table().rows.len()),
                        Some(m @ (Machine::Span(_) | Machine::Survey(_))) => m
                            .finds()
                            .map_or((String::new(), 0), |l| (l.to_csv(), l.len())),
                        None => (ScanTable::default().to_csv(), 0),
                    },
                };
                self.write(&path, text.as_bytes()).map_err(failed)?;
                put(&mut reply, &R_WRITTEN, n);
            }
            ScanCommand::Table => {
                let stack = args.arg(&A_STACK_REQ)?;
                if table_path(stack).is_none() {
                    return Err(refused(args, &A_STACK_REQ, |id| SettingError::Malformed {
                        id,
                        at: None,
                    }));
                }
                let table = self.load_table(stack).map_err(failed)?;
                self.view = View {
                    stack: stack.to_owned(),
                    limit: usize::try_from(args.get(&A_LIMIT)).unwrap_or(1),
                    offset: usize::try_from(args.get(&A_OFFSET)).unwrap_or(0),
                    table,
                };
                self.publish_table();
            }
            ScanCommand::Store => {
                let stack = args.arg(&A_STACK_REQ)?;
                let group = match args.get(&A_GROUP) {
                    "" => stack,
                    g => g,
                };
                let rows = self.load_table(stack).map_err(failed)?;
                let list = rows.to_memories(group, args.get(&A_ALL));
                put(&mut reply, &R_STORED, list.len());
                self.requests
                    .extend(list.into_iter().map(|m| HostRequest::StoreMemory {
                        name: m.name,
                        freq_hz: m.freq_hz,
                        target: TuneTarget::Stack(m.stack),
                        bandwidth_hz: m.bandwidth_hz,
                        settings: m.settings,
                        group: m.group,
                    }));
            }
        }
        if let (Some(r), Some(p)) = (reply.as_mut(), said) {
            r.set(&R_CHANNELS, count(p.steps));
            r.set(&R_KIND, p.kind);
            r.set(&R_DWELL, p.dwell_s);
            r.set(&R_PASS, p.pass_s);
            r.set(&R_TOTAL, p.total_s);
            r.set(&R_PASSES, p.passes);
            r.set(&R_VISIT, p.visit_s);
            r.set(&R_SPAN_PASS, p.span_pass_s);
        }
        Ok(reply)
    }

    fn no_hit(&self, key: &str) -> CommandError {
        CommandError::NoSuchRow {
            table: self.ids.hits,
            key: key.to_owned(),
        }
    }

    fn control(&mut self, args: &Values) -> Result<(), CommandError> {
        let now = self.now_s;
        let action = args.arg(&A_ACTION)?;
        let unlock_hz = args.get(&A_UNLOCK);
        let not_now = self.not_now();
        let Some(run) = self.run.as_mut().filter(|r| !r.finished) else {
            return Err(not_now);
        };
        let step = match (run.machine.as_mut(), action) {
            (Some(Machine::Activity(a)), _) => match action {
                Action::Skip => a.skip(now),
                Action::Lockout => a.lockout_current(now),
                Action::Pause => {
                    a.pause();
                    None
                }
                Action::Resume => {
                    a.resume(now);
                    None
                }
                Action::Unlock => {
                    a.unlock(unlock_hz);
                    None
                }
            },
            (Some(Machine::Span(s)), Action::Skip) => s.skip(now),
            (Some(Machine::Span(s)), Action::Pause) => {
                s.pause();
                None
            }
            (Some(Machine::Span(s)), Action::Resume) => s.resume(now),
            (Some(Machine::Survey(s)), Action::Skip) => s.skip(now),
            (Some(Machine::Survey(s)), Action::Lockout) => s.reject(now),
            (Some(Machine::Survey(s)), Action::Pause) => {
                s.pause();
                None
            }
            (Some(Machine::Survey(s)), Action::Resume) => s.resume(now),
            _ => return Err(not_now),
        };
        if step.is_some() {
            run.pending = step;
        }
        self.publish_progress();
        self.publish_hits();
        Ok(())
    }
}

impl Owner for ScanService {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match ScanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ScanSetting::Settle
            | ScanSetting::Dwell
            | ScanSetting::Hang
            | ScanSetting::MaxHold
            | ScanSetting::StopOn
            | ScanSetting::Threshold
            | ScanSetting::PriorityInterval
            | ScanSetting::PriorityDwell
            | ScanSetting::Passes
            | ScanSetting::HitCap
            | ScanSetting::KeepEmpty
            | ScanSetting::AfterFind
            | ScanSetting::Quiet
            | ScanSetting::RejectHold
            | ScanSetting::Look => self.values.set(id, value),
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
        ScanCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = ScanCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.run_command(cmd, args)
    }
}

/// Writes a hit of an activity, span or survey scan.
fn write_hit(r: &mut RowMut<'_>, h: &Hit) {
    r.set(&FREQ, h.freq_hz);
    r.set(&NAME, &h.name);
    r.set(&STACK, &h.stack);
    r.set(&LOCKED, true);
    r.opt(&SNR, h.peak_snr_db.map(f64::from));
    r.set(&START, h.start_s);
    r.set(&STOP, h.end_s);
    r.set(&PRIORITY, h.priority);
    r.set(&TONE, &h.tone);
    r.opt(&MEMORY, h.memory_id.map(|id| id.to_string()).as_deref());
    r.set(&PASS, i64::from(h.pass));
    r.set(&TEXT, &h.text);
}

/// Writes the columns that the hits table and the scan table share.
fn write_row(r: &mut RowMut<'_>, row: &ScanRow) {
    r.set(&FREQ, row.freq_hz);
    r.set(&STACK, &row.stack);
    r.set(&CHANNEL, &row.channel);
    r.set(&NAME, &row.name);
    r.set(&LOCKED, row.locked);
    r.opt(&SNR, row.snr_db.map(f64::from));
    r.opt(&LEVEL, row.level_dbfs.map(f64::from));
    r.opt(&EMPTY, row.empty);
}

/// Writes stored settings of row `of_row` into `def`, keyed `<row>/<owner>/<key>`.
fn settings(
    p: &mut Published,
    def: &TableDef,
    of: &Field<field::Text>,
    of_row: &str,
    list: &[OwnerSetting],
) {
    for s in list {
        if let Some(mut r) = p.row_mut(def, &format!("{of_row}/{}/{}", s.owner.as_str(), s.key)) {
            r.set(of, of_row);
            r.set(&ITEM_OWNER, s.owner.as_str());
            r.set(&ITEM_KEY, &s.key);
            r.set(&ITEM_VALUE, &s.value);
        }
    }
}

/// Writes kept values of row `of_row` into `def`, keyed `<row>/<key>`.
fn values(
    p: &mut Published,
    def: &TableDef,
    of: &Field<field::Text>,
    of_row: &str,
    list: &[(String, Value)],
) {
    let mut seen = HashSet::new();
    for (k, v) in list {
        if !seen.insert(k) {
            continue;
        }
        if let Some(mut r) = p.row_mut(def, &format!("{of_row}/{k}")) {
            r.set(of, of_row);
            r.set(&ITEM_KEY, k);
            r.set(&ITEM_VALUE, v);
        }
    }
}

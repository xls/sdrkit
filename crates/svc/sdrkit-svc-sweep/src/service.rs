//! The sweep service as an owner: the profile as settings, what the sweep found as fields, tables, series and a raster, and
//! the commands that start, control and stop a sweep. The engine steps the sweep that runs and feeds it spectrum frames.

use crate::channels::{ActionOutcome, Channel, ChannelType, HitAction};
use crate::files;
use crate::floor::FloorSnapshot;
use crate::health::{default_bands, Level};
use crate::hits::HitEvent;
use crate::hunter::{SourceClass, Trend};
use crate::machine::{
    Machine, Phase, RadioInfo, RadioSweepUse, SpectrumFrame, SpectrumNeed, Step, Watches,
};
use crate::profile::{
    self, preset_bin_hz, Band, Purpose, ScanProfile, SweepSetting, PRESET_RANGES,
};
use crate::record::{RecordKind, RecordRequest};
use crate::survey::{BandSurvey, ChangeKind, SurveySignal};
use crate::traces::SERIES;
use crate::SweepError;
use sdrkit_algo_dsp::Complex32;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::results::RowMut;
use sdrkit_core::settings::{Argument, SettingError, SettingSet};
use sdrkit_core::stack::StageError;
use sdrkit_core::TuneTarget;
use sdrkit_core::{
    Choice, Clock, Event, EventDef, HostRequest, LockState, Owner, OwnerKey, RasterId, Record,
    Results, ResultsDecl, Schema, SeriesId, Setting, SettingId, StateBlob, Status, Storage,
    StorePath, TableId, Value, Values, WriteMode,
};
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Arc;

/// The owner key of the sweep service.
pub const KEY: OwnerKey = OwnerKey::of("service.sweep");
const OWNER: &str = "service.sweep";

/// The most rows of a page of the findings table.
pub const MAX_PAGE: usize = 1000;
const MAX_BURSTS: usize = 500;
const MAX_DRIFT: usize = 500;
const MAX_INTERFERENCE: usize = 256;
const MAX_PERSISTENT: usize = 1000;
const MAX_QUIET: usize = 100;
const MAX_DIFFS: usize = 1000;
const MAX_RECORDINGS: usize = 64;
const MAX_ADVICE: usize = 32;
const MAX_HEALTH_BANDS: usize = 16;
const MAX_PRESETS: usize = 64;
/// The columns of the survey picture.
const RASTER_WIDTH: usize = 1024;
/// The tag and the version of a saved noise floor.
const FLOOR_STATE: &str = "service.sweep.floor";
const FLOOR_VERSION: u32 = 1;

sdrkit_core::choice_enum! {
    /// How a sweep ended.
    pub enum EndReason {
        /// The sweep has not ended.
        None = ("none", "Not ended"),
        /// Every sweep of the schedule is done, or the sweep was finished.
        Finished = ("finished", "Finished"),
        /// The sweep was stopped.
        Stopped = ("stopped", "Stopped"),
        /// The sweep did not start. The log says why.
        Failed = ("failed", "Failed"),
    }
}

sdrkit_core::choice_enum! {
    /// What happened to a hit.
    pub enum HitState {
        /// A new signal opened.
        Started = ("started", "Started"),
        /// A known signal opened again.
        Returned = ("returned", "Returned"),
        /// A signal ended.
        Ended = ("ended", "Ended"),
    }
}

sdrkit_core::choice_enum! {
    /// What the command export writes.
    pub enum ExportFormat {
        /// The last sweep in the rtl_power layout.
        RtlPower = ("rtl_power", "rtl_power CSV"),
        /// The live, peak and average traces as CSV.
        Traces = ("traces", "Traces CSV"),
        /// The confirmed channels as CSV.
        Signals = ("signals", "Signal log CSV"),
        /// The statistics of the survey as CSV.
        SurveyCsv = ("survey_csv", "Survey CSV"),
        /// The survey as a compact file that the command diff reads.
        Survey = ("survey", "Survey file"),
    }
}

// Results.
const STATE: Field<field::Choice<Phase>> = Field::new("state", "State", "");
const PURPOSE: Field<field::Choice<Purpose>> = Field::new("purpose", "Purpose", "");
const PROFILE: Field<field::Text> = Field::new("profile", "Profile", "");
const END: Field<field::Choice<EndReason>> = Field::new("end", "End", "");
const SESSION_F: Field<field::Text> =
    Field::new("session", "Session", "").refers_to("engine", "sessions");
const SWEEPS_F: Field<field::I64> = Field::new("sweeps", "Sweeps", "");
const HOP: Field<field::I64> = Field::new("hop", "Hop", "");
const HOPS: Field<field::I64> = Field::new("hops", "Hops per sweep", "");
const HOPS_DONE: Field<field::I64> = Field::new("hops_done", "Hops done", "");
const LOW: Field<field::Hz> = Field::new("low_hz", "Low edge", "Hz");
const HIGH: Field<field::Hz> = Field::new("high_hz", "High edge", "Hz");
const BIN_F: Field<field::Hz> = Field::new("bin_hz", "Resolution", "Hz");
const RATE_F: Field<field::Hz> = Field::new("rate_hz", "Sample rate", "Hz");
const FFT: Field<field::I64> = Field::new("fft", "FFT size", "");
const SETTLE_F: Field<field::Duration> = Field::new("settle", "Settle time", "s");
const SETTLE_MEASURED: Field<field::Bool> =
    Field::new("settle_measured", "Settle time measured", "");
const RADIO_SWEEP_F: Field<field::Choice<RadioSweepUse>> =
    Field::new("radio_sweep", "Sweep of the radio", "");
const TOTAL: Field<field::I64> = Field::new("total", "Confirmed channels", "");
const HIDDEN: Field<field::I64> = Field::new("hidden", "Hidden channels", "");
const SURVEY_SWEEPS: Field<field::I64> = Field::new("survey_sweeps", "Survey sweeps", "");
const LEARNT: Field<field::Bool> = Field::new("learnt", "Band learnt", "");
const METER_HZ: Field<field::Hz> = Field::new("meter_hz", "Meter frequency", "Hz");
const METER_LEVEL: Field<field::F64> = Field::new("meter_level_dbfs", "Meter level", "dBFS");
const METER_RISE: Field<field::F64> = Field::new("meter_rise_db", "Meter rise", "dB");
const METER_SMOOTH: Field<field::F64> =
    Field::new("meter_smoothed_dbfs", "Meter smoothed level", "dBFS");
const METER_TREND: Field<field::Choice<Trend>> = Field::new("meter_trend", "Meter trend", "");
const METER_MAX: Field<field::F64> = Field::new("meter_max_dbfs", "Meter highest level", "dBFS");
const METER_NEEDLE: Field<field::F64> = Field::new("meter_needle", "Meter needle", "");
const METER_TONE: Field<field::Hz> = Field::new("meter_tone_hz", "Meter tone", "Hz");
const HEALTH: Field<field::Choice<Level>> = Field::new("health", "Station health", "");
const DIFF_LEVEL: Field<field::F64> = Field::new("diff_level_db", "Level change", "dB");

const POSITION: Field<field::I64> = Field::new("position", "Position", "");
const WIDTH: Field<field::Hz> = Field::new("width_hz", "Width", "Hz");
const PEAK: Field<field::F64> = Field::new("peak_dbfs", "Peak", "dBFS");
const TYPE: Field<field::Choice<ChannelType>> = Field::new("type", "Type", "");
const SEEN: Field<field::I64> = Field::new("seen", "Sweeps seen", "");
const PRESENT: Field<field::F64> = Field::new("present", "Share present", "");
const COUNT: Field<field::I64> = Field::new("count", "Count", "");
const FIRST_SEEN: Field<field::Time> = Field::new("first_seen", "First seen", "s");
const LAST_SEEN: Field<field::Time> = Field::new("last_seen", "Last seen", "s");
const ACTIVE: Field<field::Bool> = Field::new("active", "Active", "");
const IMAGE: Field<field::Bool> = Field::new("image", "Image", "");
const SPUR: Field<field::Bool> = Field::new("spur", "Spur", "");
const DC: Field<field::Bool> = Field::new("dc", "At DC", "");
const PRIORITY: Field<field::Bool> = Field::new("priority", "Priority", "");
const START: Field<field::Time> = Field::new("start", "Start", "s");
const DURATION: Field<field::Duration> = Field::new("duration", "Duration", "s");
const BASE: Field<field::F64> = Field::new("base_dbfs", "Quiet level", "dBFS");
const SAMPLES: Field<field::I64> = Field::new("samples", "Levels", "");
const LOW_C: Field<field::Hz> = Field::new("low_hz", "Low edge", "Hz");
const HIGH_C: Field<field::Hz> = Field::new("high_hz", "High edge", "Hz");
const OCCUPANCY: Field<field::F64> = Field::new("occupancy", "Occupancy", "");
const OCC_CHANGE: Field<field::F64> = Field::new("occupancy_change", "Occupancy change", "");
const CHANGE: Field<field::Choice<ChangeKind>> = Field::new("change", "Change", "");
const MEAN: Field<field::F64> = Field::new("mean_dbfs", "Mean", "dBFS");
const FIRST_HZ: Field<field::Hz> = Field::new("first_hz", "First frequency", "Hz");
const LAST_HZ: Field<field::Hz> = Field::new("last_hz", "Last frequency", "Hz");
const FIRST_T: Field<field::Time> = Field::new("first", "First seen", "s");
const LAST_T: Field<field::Time> = Field::new("last", "Last seen", "s");
const FRAMES: Field<field::I64> = Field::new("frames", "Frames", "");
const PEAK_HZ: Field<field::Hz> = Field::new("peak_hz", "Strongest bin", "Hz");
const RISE: Field<field::F64> = Field::new("rise_db", "Rise", "dB");
const SOURCE_CLASS: Field<field::Choice<SourceClass>> = Field::new("class", "Likely source", "");
const SWEEPS_TOTAL: Field<field::I64> = Field::new("sweeps_total", "Sweeps since first seen", "");
const NAME: Field<field::Text> = Field::new("name", "Name", "");
const LABEL: Field<field::Text> = Field::new("label", "Band", "");
const OF_PRESET: Field<field::Text> =
    Field::new("preset", "Preset", "").refers_to(OWNER, "presets");
const OF_FINDING: Field<field::Text> =
    Field::new("finding", "Channel", "").refers_to(OWNER, "findings");
const DEMOD: Field<field::Text> = Field::new("demod", "Demodulator", "");
const RECORD_AS: Field<field::Choice<RecordKind>> = Field::new("kind", "Record as", "");
const LEVEL: Field<field::Choice<Level>> = Field::new("level", "Level", "");
const FLOOR_C: Field<field::F64> = Field::new("floor_dbfs", "Noise floor", "dBFS");
const SIGNALS: Field<field::I64> = Field::new("signals", "Signals", "");
const HIT_STATE: Field<field::Choice<HitState>> = Field::new("hit", "Hit", "");

/// The confirmed channels: one page, in frequency order. The row key is the channel id that the command hit_action takes.
pub const FINDINGS: TableDef = TableDef {
    key: "findings",
    label: "Channels",
    columns: &[
        &POSITION,
        &keys::FREQ_HZ,
        &WIDTH,
        &keys::SNR_DB,
        &PEAK,
        &TYPE,
        &SEEN,
        &PRESENT,
        &COUNT,
        &FIRST_SEEN,
        &LAST_SEEN,
        &ACTIVE,
        &IMAGE,
        &SPUR,
        &DC,
        &PRIORITY,
    ],
    cap: MAX_PAGE,
};
/// The bursts seen at the frequency `burst_hz`, oldest first.
pub const BURSTS: TableDef = TableDef {
    key: "bursts",
    label: "Bursts",
    columns: &[&START, &DURATION, &PEAK, &BASE, &SAMPLES],
    cap: MAX_BURSTS,
};
/// The ranges whose occupancy changed against an older survey (the command diff).
pub const DIFFS: TableDef = TableDef {
    key: "diffs",
    label: "Survey changes",
    columns: &[&LOW_C, &HIGH_C, &OCC_CHANGE, &CHANGE],
    cap: MAX_DIFFS,
};
/// The drifting carriers found.
pub const DRIFT: TableDef = TableDef {
    key: "drift",
    label: "Drifting carriers",
    columns: &[
        &FIRST_HZ,
        &LAST_HZ,
        &keys::DRIFT_HZ_S,
        &FIRST_T,
        &LAST_T,
        &PEAK,
        &FRAMES,
    ],
    cap: MAX_DRIFT,
};
/// The interference that the hunter found. The row key is its id.
pub const INTERFERENCE: TableDef = TableDef {
    key: "interference",
    label: "Interference",
    columns: &[
        &LOW_C,
        &HIGH_C,
        &PEAK_HZ,
        &RISE,
        &SOURCE_CLASS,
        &keys::TEXT,
        &FIRST_SEEN,
        &LAST_SEEN,
        &SEEN,
        &SWEEPS_TOTAL,
    ],
    cap: MAX_INTERFERENCE,
};
/// The signals of the survey present at least `persistent_share` of the time.
pub const PERSISTENT: TableDef = TableDef {
    key: "persistent",
    label: "Persistent signals",
    columns: &[&LOW_C, &HIGH_C, &OCCUPANCY, &MEAN, &PEAK],
    cap: MAX_PERSISTENT,
};
/// The quietest slots of the survey that the command quiet requested, quietest first.
pub const QUIET: TableDef = TableDef {
    key: "quiet",
    label: "Quiet channels",
    columns: &[&POSITION, &LOW_C, &HIGH_C, &OCCUPANCY, &MEAN],
    cap: MAX_QUIET,
};
/// The range presets. The row key is the name that the command start takes.
pub const PRESETS: TableDef = TableDef {
    key: "presets",
    label: "Presets",
    columns: &[&LOW_C, &HIGH_C, &BIN_F],
    cap: MAX_PRESETS,
};
/// The bands of the presets.
pub const PRESET_BANDS: TableDef = TableDef {
    key: "preset_bands",
    label: "Preset bands",
    columns: &[&OF_PRESET, &LABEL, &LOW_C, &HIGH_C],
    cap: MAX_PRESETS * 4,
};
/// The channels that the engine must record now.
pub const RECORDINGS: TableDef = TableDef {
    key: "recordings",
    label: "Recordings",
    columns: &[&OF_FINDING, &keys::FREQ_HZ, &WIDTH, &DEMOD, &RECORD_AS],
    cap: MAX_RECORDINGS,
};
/// The advice of the station health check, worst first.
pub const ADVICE: TableDef = TableDef {
    key: "advice",
    label: "Advice",
    columns: &[&POSITION, &LEVEL, &keys::TEXT],
    cap: MAX_ADVICE,
};
/// The bands of the station health check.
pub const HEALTH_BANDS: TableDef = TableDef {
    key: "health_bands",
    label: "Health bands",
    columns: &[&FLOOR_C, &PEAK, &SIGNALS],
    cap: MAX_HEALTH_BANDS,
};

// Events.
/// A hit started, returned or ended. Its frequency is the centre of the channel.
pub static HIT: EventDef = EventDef::new(
    "sweep.hit",
    "Sweep hit",
    &[
        &HIT_STATE,
        &OF_FINDING,
        &keys::SNR_DB,
        &WIDTH,
        &COUNT,
        &DURATION,
        &keys::FAMILY,
    ],
);
/// A burst ended at the frequency `burst_hz`.
pub static BURST: EventDef = EventDef::new(
    "sweep.burst",
    "Burst",
    &[&START, &DURATION, &PEAK, &BASE, &keys::FAMILY],
);
/// The hunter found new interference. Its frequency is the strongest bin.
pub static NEW_INTERFERENCE: EventDef = EventDef::new(
    "sweep.interference",
    "New interference",
    &[
        &SOURCE_CLASS,
        &RISE,
        &LOW_C,
        &HIGH_C,
        &keys::TEXT,
        &keys::FAMILY,
    ],
);
/// The command hit_action requested saving a channel as a memory. Its frequency is the centre of the channel.
pub static SAVE: EventDef = EventDef::new(
    "sweep.save",
    "Save as memory",
    &[&NAME, &DEMOD, &WIDTH, &keys::FAMILY],
);

// Arguments.
/// The row key of the channel that a command acts on: the channel id of the findings table.
pub const ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Row", "")).help("The row key, as the table shows it.");
/// The session that a sweep tunes: its row key in the engine's sessions table.
pub const SESSION: Argument<field::Text> = Argument::new(SESSION_F);
const A_PURPOSE: Setting<field::Choice<Purpose>> =
    Setting::choice("purpose", "Purpose", Purpose::Scan);
const A_PRESET: Setting<field::Text> = Setting::new("preset", "Preset", "", "")
    .help("A row key of the presets table. Empty for none.");
const A_WHOLE: Setting<field::Bool> = Setting::new("whole_range", "Whole range", "", false)
    .help("Sweeps everything the radio tunes.");
const A_LOW: Setting<field::Hz> = Setting::new("low_hz", "From", "Hz", 0.0).range(0.0, 1e12, 0.0);
const A_HIGH: Setting<field::Hz> = Setting::new("high_hz", "To", "Hz", 0.0).range(0.0, 1e12, 0.0);
const A_LOW_REQ: Argument<field::Hz> = Argument::new(LOW_C);
const A_HIGH_REQ: Argument<field::Hz> = Argument::new(HIGH_C);
const A_ACTION: Argument<field::Choice<HitAction>> =
    Argument::new(Field::new("action", "Action", ""));
const A_DEMOD: Setting<field::Text> = Setting::new("demod", "Demodulator", "", "")
    .help("The demodulator to tune or save with. Empty for the guess of the channel type.");
const A_TUNE_SESSION: Setting<field::Text> = Setting::new("session", "Session", "", "")
    .help("The session to tune. Empty for the session of the sweep.");
const A_TARGET: Setting<field::Hz> = Setting::new("hz", "Frequency", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .help("The frequency that the meter follows. 0 follows the strongest new peak.");
const A_LIMIT: Setting<field::I64> =
    Setting::new("limit", "Rows", "", 100).range(1.0, MAX_PAGE as f64, 1.0);
const A_OFFSET: Setting<field::I64> =
    Setting::new("offset", "First row", "", 0).range(0.0, 1e7, 1.0);
const A_WIDTH: Argument<field::Hz> = Argument::new(WIDTH);
const A_COUNT: Setting<field::I64> =
    Setting::new("count", "Slots", "", 5).range(1.0, MAX_QUIET as f64, 1.0);
const A_PATH: Argument<field::Text> = Argument::new(Field::new("path", "File", ""))
    .help("A file in the storage of the host, as a relative path.");
const A_FORMAT: Setting<field::Choice<ExportFormat>> =
    Setting::choice("format", "Format", ExportFormat::RtlPower);

// Replies.
const R_FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
const R_BANDWIDTH: Field<field::Hz> = Field::new("bandwidth_hz", "Bandwidth", "Hz");
const R_WRITTEN: Field<field::I64> = Field::new("written", "Bytes written", "");
const R_BINS: Field<field::I64> = Field::new("bins", "Bins", "");
const R_CHANGES: Field<field::I64> = Field::new("changes", "Changes", "");

const C_START: Command = Command::new("start", "Start sweep")
    .help("Starts a sweep of a preset, of the whole range of the radio, or of the range from low_hz to high_hz. A health check sweeps its own bands. A sweep that runs ends first.")
    .args(&[&SESSION, &A_PURPOSE, &A_PRESET, &A_WHOLE, &A_LOW, &A_HIGH]);
const C_STOP: Command = Command::new("stop", "Stop sweep").help("Stops the sweep at once.");
const C_FINISH: Command = Command::new("finish", "Finish sweep")
    .help("Ends the sweep after the analysis of the hops captured so far.");
const C_HIT_ACTION: Command = Command::new("hit_action", "Act on a channel")
    .help("Tunes a session to a channel, saves it as a memory, locks it out, makes its hop a priority hop, or removes it.")
    .args(&[&ROW, &A_ACTION, &A_DEMOD, &A_TUNE_SESSION])
    .reply(&[&R_FREQ, &DEMOD, &R_BANDWIDTH, &LOW_C, &HIGH_C]);
const C_EXCLUDE: Command = Command::new("exclude", "Exclude range")
    .help("Never visits the range again in this sweep.")
    .args(&[&A_LOW_REQ, &A_HIGH_REQ]);
const C_RESET_MAX: Command = Command::new("reset_max", "Reset max hold");
const C_HUNT_TARGET: Command = Command::new("hunt_target", "Follow a frequency")
    .help("Sets the frequency that the meter of the hunter follows.")
    .args(&[&A_TARGET]);
const C_RELEARN: Command = Command::new("relearn", "Learn the band again");
const C_BURST_CLEAR: Command = Command::new("burst_clear", "Clear bursts");
const C_FINDINGS: Command = Command::new("findings", "Show channels")
    .help("Sets the page of the findings table.")
    .args(&[&A_LIMIT, &A_OFFSET]);
const C_QUIET: Command = Command::new("quiet", "Find quiet channels")
    .help("Fills the quiet table with the quietest slots of the survey.")
    .args(&[&A_WIDTH, &A_COUNT]);
const C_DIFF: Command = Command::new("diff", "Compare surveys")
    .help("Compares the survey with an older survey file and fills the diffs table.")
    .args(&[&A_PATH])
    .reply(&[&R_CHANGES]);
const C_EXPORT: Command = Command::new("export", "Export")
    .help("Writes what the sweep found to the file `path`.")
    .args(&[&A_PATH, &A_FORMAT])
    .reply(&[&R_WRITTEN]);
const C_FLOOR_SAVE: Command = Command::new("floor.save", "Save noise floor")
    .help("Writes the noise floor of the sweep to the file `path`.")
    .args(&[&A_PATH])
    .reply(&[&R_BINS]);
const C_FLOOR_RESTORE: Command = Command::new("floor.restore", "Restore noise floor")
    .help("Loads a noise floor saved with floor.save into the sweep. On another grid each bin takes the saved bin at its centre.")
    .args(&[&A_PATH])
    .reply(&[&R_BINS]);

sdrkit_core::commands! {
    /// The commands of the sweep service.
    pub enum SweepCommand {
        /// Starts a sweep.
        Start = C_START,
        /// Stops the sweep.
        Stop = C_STOP,
        /// Finishes the sweep.
        Finish = C_FINISH,
        /// Acts on a channel.
        HitAction = C_HIT_ACTION,
        /// Excludes a range.
        Exclude = C_EXCLUDE,
        /// Resets the max hold trace.
        ResetMax = C_RESET_MAX,
        /// Sets what the meter follows.
        HuntTarget = C_HUNT_TARGET,
        /// Learns the band again.
        Relearn = C_RELEARN,
        /// Clears the bursts.
        BurstClear = C_BURST_CLEAR,
        /// Sets the page of the findings.
        Findings = C_FINDINGS,
        /// Finds quiet channels.
        Quiet = C_QUIET,
        /// Compares surveys.
        Diff = C_DIFF,
        /// Exports what the sweep found.
        Export = C_EXPORT,
        /// Saves the noise floor.
        FloorSave = C_FLOOR_SAVE,
        /// Restores a noise floor.
        FloorRestore = C_FLOOR_RESTORE,
    }
}

/// Where a start command takes its bands from.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Source {
    Range { low_hz: f64, high_hz: f64 },
    Preset(usize),
    WholeRange,
    Health,
}

/// A sweep that a command set up and the engine runs.
struct Run {
    session: String,
    purpose: Purpose,
    source: Source,
    /// Where to tune back to, if the sweep replaced a sweep that ran.
    return_hz: Option<f64>,
    machine: Option<Machine>,
    pending: Option<Step>,
    finished: bool,
}

/// The ids that the service writes by id, found once.
#[derive(Clone, Copy, Debug)]
struct Ids {
    state: sdrkit_core::FieldId,
    purpose: sdrkit_core::FieldId,
    findings: TableId,
    presets: TableId,
    series: [SeriesId; 5],
    survey: RasterId,
}

/// What was published last, to write only what changed.
#[derive(Debug, Default)]
struct Shown {
    sweeps: u64,
    findings: Vec<u64>,
    page: Vec<u64>,
    persistent: usize,
    drift: usize,
    interference: Vec<u64>,
    recording: Vec<RecordRequest>,
}

/// The sweep service.
pub struct SweepService {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    exclusions: Vec<(f64, f64)>,
    run: Option<Run>,
    now_s: f64,
    limit: usize,
    offset: usize,
    shown: Shown,
    key: String,
    signals: Vec<SurveySignal>,
    requests: Vec<HostRequest>,
    events: Vec<Event>,
    p: Published,
    ids: Ids,
}

impl std::fmt::Debug for SweepService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SweepService")
            .field("running", &self.session().is_some())
            .finish()
    }
}

fn decl() -> Arc<ResultsDecl> {
    let mut d = ResultsDecl::new().with_fields(&[
        &STATE,
        &PURPOSE,
        &PROFILE,
        &END,
        &SESSION_F,
        &SWEEPS_F,
        &HOP,
        &HOPS,
        &HOPS_DONE,
        &LOW,
        &HIGH,
        &BIN_F,
        &RATE_F,
        &FFT,
        &SETTLE_F,
        &SETTLE_MEASURED,
        &RADIO_SWEEP_F,
        &TOTAL,
        &HIDDEN,
        &SURVEY_SWEEPS,
        &LEARNT,
        &METER_HZ,
        &METER_LEVEL,
        &METER_RISE,
        &METER_SMOOTH,
        &METER_TREND,
        &METER_MAX,
        &METER_NEEDLE,
        &METER_TONE,
        &HEALTH,
        &DIFF_LEVEL,
    ]);
    for t in [
        &FINDINGS,
        &BURSTS,
        &DIFFS,
        &DRIFT,
        &INTERFERENCE,
        &PERSISTENT,
        &QUIET,
        &PRESETS,
        &PRESET_BANDS,
        &RECORDINGS,
        &ADVICE,
        &HEALTH_BANDS,
    ] {
        d = d.with_table(t);
    }
    for (key, label) in SERIES {
        d = d.series(key, label, "MHz", "dBFS");
    }
    d = d
        .raster("survey", "Survey", "MHz", "s", "dBFS")
        .event(&HIT)
        .event(&BURST)
        .event(&NEW_INTERFERENCE)
        .event(&SAVE);
    Arc::new(d)
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn count64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn failed(e: SweepError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// The error of an argument that the service refuses: [`CommandError::BadArgument`] with `error` made from its id.
fn refused(
    args: &Values,
    key: &'static str,
    error: impl Fn(SettingId) -> SettingError,
) -> CommandError {
    match args.schema().id(key) {
        Ok(arg) => CommandError::BadArgument {
            arg,
            error: error(arg),
        },
        Err(_) => CommandError::NotAnArgument { key },
    }
}

fn file_arg(args: &Values) -> Result<StorePath, CommandError> {
    StorePath::new(args.arg(&A_PATH)?).map_err(|_| {
        refused(args, A_PATH.field.key, |id| SettingError::Malformed {
            id,
            at: None,
        })
    })
}

impl SweepService {
    /// Makes the service on the host's `storage`, with the host's wall `clock` for the times in exported files.
    pub fn new(storage: Arc<dyn Storage>, clock: Clock) -> Self {
        let p = Published::from_decl(decl());
        let d = p.results().decl();
        let ids = Ids {
            state: d
                .field_id(STATE.key)
                .expect("the service declares its fields"),
            purpose: d
                .field_id(PURPOSE.key)
                .expect("the service declares its fields"),
            findings: d
                .table_id(FINDINGS.key)
                .expect("the service declares its tables"),
            presets: d
                .table_id(PRESETS.key)
                .expect("the service declares its tables"),
            series: SERIES
                .map(|(key, _)| d.series_id(key).expect("the service declares its series")),
            survey: d
                .raster_id("survey")
                .expect("the service declares its raster"),
        };
        let mut s = SweepService {
            storage,
            clock,
            values: Values::defaults(&SweepSetting::schema()),
            exclusions: Vec::new(),
            run: None,
            now_s: 0.0,
            limit: 100,
            offset: 0,
            shown: Shown::default(),
            key: String::with_capacity(24),
            signals: Vec::new(),
            requests: Vec::new(),
            events: Vec::new(),
            p,
            ids,
        };
        s.p.set(&STATE, Phase::Idle);
        s.p.set(&END, EndReason::None);
        s.publish_presets();
        s
    }

    /// Steps the sweep at `now_s`, signal time. The engine calls it at its ticks with the radio information of the
    /// session. The first call after a start command makes the sweep. Returns what the engine must do.
    ///
    /// # Errors
    ///
    /// [`SweepError`] once, if the sweep cannot start on this radio. The state is then failed.
    pub fn step(&mut self, now_s: f64, radio: &RadioInfo) -> Result<Option<Step>, SweepError> {
        self.now_s = now_s;
        let Some(run) = self.run.as_mut().filter(|r| !r.finished) else {
            return Ok(None);
        };
        let step = if let Some(s) = run.pending.take() {
            Some(s)
        } else if let Some(m) = run.machine.as_mut() {
            m.tick(now_s)
        } else {
            let mut radio = *radio;
            if let Some(hz) = run.return_hz {
                radio.tuned_hz = hz;
            }
            match make(
                &self.values,
                &self.exclusions,
                run.purpose,
                run.source,
                &radio,
            ) {
                Ok(mut m) => {
                    let s = m.start(now_s);
                    run.machine = Some(m);
                    self.publish_start();
                    Some(s)
                }
                Err(e) => {
                    run.finished = true;
                    self.p.set(&STATE, Phase::Failed);
                    self.p.set(&END, EndReason::Failed);
                    self.p
                        .results_mut()
                        .set_status(&Status::new(LockState::Off));
                    self.p.results_mut().log(e.to_string());
                    return Err(e);
                }
            }
        };
        Ok(self.after(step))
    }

    /// A spectrum frame of the session, as [`SweepService::spectrum`] declares it. Returns what the engine must do.
    /// Once warm, a frame allocates nothing unless a row of a table changes.
    ///
    /// # Errors
    ///
    /// [`SweepError::FrameShape`] if the frame does not have the declared bins and span. The frame is not used.
    pub fn feed(&mut self, frame: &SpectrumFrame<'_>) -> Result<Option<Step>, SweepError> {
        self.now_s = frame.time_s;
        let Some(m) = self.machine_mut() else {
            return Ok(None);
        };
        let step = m.feed(frame)?;
        Ok(self.after(step))
    }

    /// IQ samples captured at `centre_hz`, after a [`Step::Tune`] that requests [`Input::Samples`](crate::Input::Samples): they
    /// measure the settle time of the radio. Returns what the engine must do.
    pub fn feed_samples(&mut self, now_s: f64, centre_hz: f64, iq: &[Complex32]) -> Option<Step> {
        self.now_s = now_s;
        let step = self.machine_mut()?.feed_samples(now_s, centre_hz, iq);
        self.after(step)
    }

    /// The radio cannot sweep by itself after a [`Step::RadioSweep`]. The sweep retunes from now on.
    pub fn radio_sweep_failed(&mut self) -> Option<Step> {
        let step = self.machine_mut()?.radio_sweep_failed();
        self.after(step)
    }

    /// The spectrum that the sweep that runs needs. `None` if no sweep runs.
    pub fn spectrum(&self) -> Option<SpectrumNeed> {
        self.running().map(Machine::spectrum)
    }

    /// The channels that the engine must record now, as the recordings table shows them.
    pub fn recordings(&self) -> &[RecordRequest] {
        self.running().map_or(&[], |m| &m.recording)
    }

    /// The session that the sweep tunes, while a sweep runs.
    pub fn session(&self) -> Option<&str> {
        self.run
            .as_ref()
            .filter(|r| !r.finished)
            .map(|r| r.session.as_str())
    }

    /// The requests for the engine that commands made, oldest first.
    pub fn take_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.requests)
    }

    /// Moves the events of the service to the end of `out`, oldest first.
    pub fn take_events(&mut self, out: &mut Vec<Event>) {
        out.append(&mut self.events);
    }

    fn running(&self) -> Option<&Machine> {
        self.run
            .as_ref()
            .filter(|r| !r.finished)
            .and_then(|r| r.machine.as_ref())
    }

    fn machine_mut(&mut self) -> Option<&mut Machine> {
        self.run
            .as_mut()
            .filter(|r| !r.finished)
            .and_then(|r| r.machine.as_mut())
    }

    fn machine(&self) -> Option<&Machine> {
        self.run.as_ref().and_then(|r| r.machine.as_ref())
    }

    /// Publishes what a step of the machine changed, and ends the run on [`Step::Done`].
    fn after(&mut self, step: Option<Step>) -> Option<Step> {
        if let Some(Step::Done { .. }) = step {
            if let Some(r) = self.run.as_mut() {
                r.finished = true;
            }
            if self.p.results().fields().get(&END) == Some(EndReason::None) {
                self.p.set(&END, EndReason::Finished);
            }
            self.p
                .results_mut()
                .set_status(&Status::new(LockState::Off));
            self.shown.sweeps = u64::MAX;
        }
        self.publish_progress();
        step
    }
}

/// Makes the sweep of a start command on `radio`.
fn make(
    v: &Values,
    exclusions: &[(f64, f64)],
    purpose: Purpose,
    source: Source,
    radio: &RadioInfo,
) -> Result<Machine, SweepError> {
    let mut p = ScanProfile::from_settings(v, purpose, "Range", Vec::new());
    match source {
        Source::Range { low_hz, high_hz } => {
            p.bands.push(Band {
                label: "Range",
                low_hz,
                high_hz,
            });
        }
        Source::Preset(i) => {
            let (name, lo, hi) = PRESET_RANGES[i];
            p.name = name;
            p.bin_hz = preset_bin_hz(lo, hi);
            p.bands.push(Band {
                label: name,
                low_hz: lo * 1e6,
                high_hz: hi * 1e6,
            });
        }
        Source::WholeRange => {
            p.name = "Whole range";
            p.whole_range(radio.min_hz, radio.max_hz, radio.rate_hz);
        }
        Source::Health => {
            crate::health::profile(&mut p, &default_bands(radio.min_hz, radio.max_hz));
        }
    }
    p.exclusions.extend(
        exclusions
            .iter()
            .map(|&(lo, hi)| crate::plan::Range::new(lo, hi)),
    );
    let burst_hz = v.get(&profile::BURST_HZ);
    let watches = Watches {
        burst: (burst_hz > 0.0).then(|| {
            (
                burst_hz,
                v.get(&profile::BURST_RISE) as f32,
                v.get(&profile::BURST_MIN),
            )
        }),
        drift: v.get(&profile::DRIFT),
        survey_rows: usize::try_from(v.get(&profile::SURVEY_ROWS)).unwrap_or(2),
    };
    let mut m = Machine::new(p, *radio, watches)?;
    m.cpu_load = v.get(&profile::CPU_LOAD);
    Ok(m)
}

impl SweepService {
    fn publish_presets(&mut self) {
        let mut key = String::new();
        for &(name, lo, hi) in PRESET_RANGES.iter().rev() {
            if let Some(mut r) = self.p.row_mut(&PRESETS, name) {
                r.set(&LOW_C, lo * 1e6);
                r.set(&HIGH_C, hi * 1e6);
                r.set(&BIN_F, preset_bin_hz(lo, hi));
            }
            key.clear();
            let _ = write!(key, "{name}/0");
            if let Some(mut r) = self.p.row_mut(&PRESET_BANDS, &key) {
                r.set(&OF_PRESET, name);
                r.set(&LABEL, name);
                r.set(&LOW_C, lo * 1e6);
                r.set(&HIGH_C, hi * 1e6);
            }
        }
    }

    /// Writes the fields that a sweep sets when it starts.
    fn publish_start(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let p = &mut self.p;
        p.set(&PROFILE, m.profile.name);
        p.set(&LOW, m.grid.low_hz);
        p.set(&HIGH, m.grid.high_hz());
        p.set(&BIN_F, m.grid.bin_hz);
        p.set(&RATE_F, m.rate_hz());
        p.set(&FFT, count(m.fft()));
        p.set(&HOPS, count(m.hops()));
        p.results_mut()
            .set_status(&Status::new(LockState::Searching));
    }

    /// Writes the progress fields, and what a sweep found when one ended.
    fn publish_progress(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let sweeps = m.sweeps;
        let p = &mut self.p;
        p.set(&STATE, m.phase);
        p.set(&SWEEPS_F, count64(sweeps));
        p.set(&HOP, count(m.pos));
        p.set(&HOPS, count(m.hops()));
        p.set(&HOPS_DONE, count64(m.hops_done));
        p.set(&SETTLE_F, m.settle_s);
        p.set(&SETTLE_MEASURED, m.settle_measured);
        p.set(&RADIO_SWEEP_F, m.radio_sweep);
        if m.recording != self.shown.recording {
            self.publish_recordings();
        }
        if sweeps != self.shown.sweeps {
            self.shown.sweeps = sweeps;
            self.publish_sweep();
        }
    }

    fn publish_recordings(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let p = &mut self.p;
        for old in &self.shown.recording {
            if !m.recording.iter().any(|r| r.id == old.id) {
                self.key.clear();
                let _ = write!(self.key, "{}", old.id);
                p.remove_row(&RECORDINGS, &self.key);
            }
        }
        for r in m.recording.iter().rev() {
            self.key.clear();
            let _ = write!(self.key, "{}", r.id);
            if let Some(mut row) = p.row_mut(&RECORDINGS, &self.key) {
                row.set(&OF_FINDING, &self.key);
                row.set(&keys::FREQ_HZ, r.hz);
                row.set(&WIDTH, r.width_hz);
                row.set(&DEMOD, r.demod);
                row.set(&RECORD_AS, r.kind);
            }
        }
        self.shown.recording.clear();
        self.shown.recording.extend_from_slice(&m.recording);
    }

    /// Writes what the last sweep found.
    fn publish_sweep(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let p = &mut self.p;
        let r = p.results_mut();
        m.traces.publish(
            r,
            &self.ids.series,
            m.floor.floor_db(),
            m.profile.threshold_db,
        );
        let hidden = m
            .channels
            .all()
            .iter()
            .filter(|c| c.confirmed && c.flags.any())
            .count();
        p.set(&HIDDEN, count(hidden));
        for e in &m.hit_events {
            let (id, state, length) = match *e {
                HitEvent::Started { id } => (id, HitState::Started, None),
                HitEvent::Returned { id, .. } => (id, HitState::Returned, None),
                HitEvent::Ended { id, length_s } => (id, HitState::Ended, Some(length_s)),
            };
            let Some(t) = m.track(id) else {
                continue;
            };
            self.events.push(
                HIT.event(self.now_s, t.centre_hz)
                    .set(&HIT_STATE, state)
                    .set(&OF_FINDING, &id.to_string())
                    .set(&keys::SNR_DB, f64::from(t.snr_db))
                    .set(&WIDTH, t.width_hz)
                    .set(&COUNT, i64::from(t.count))
                    .opt(&DURATION, length)
                    .set(&keys::FAMILY, Family::Receiver),
            );
        }
        if let (Some(b), Some((hz, _))) = (m.new_burst, m.burst.as_ref()) {
            self.events.push(
                BURST
                    .event(self.now_s, *hz)
                    .set(&START, b.start_s)
                    .set(&DURATION, b.duration_s)
                    .set(&PEAK, f64::from(b.peak_db))
                    .set(&BASE, f64::from(b.base_db))
                    .set(&keys::FAMILY, Family::Receiver),
            );
            self.key.clear();
            let _ = write!(self.key, "{}", b.start_s);
            if let Some(mut r) = p.row_mut(&BURSTS, &self.key) {
                r.set(&START, b.start_s);
                r.set(&DURATION, b.duration_s);
                r.set(&PEAK, f64::from(b.peak_db));
                r.set(&BASE, f64::from(b.base_db));
                r.set(&SAMPLES, i64::from(b.samples));
            }
        }
        if let Some(d) = &m.drift {
            let found = d.found();
            if found.len() != self.shown.drift {
                self.shown.drift = found.len();
                p.clear_rows(&DRIFT);
                for (i, t) in found.iter().enumerate() {
                    self.key.clear();
                    let _ = write!(self.key, "{i}");
                    if let Some(mut r) = p.row_mut(&DRIFT, &self.key) {
                        r.set(&FIRST_HZ, t.first_hz);
                        r.set(&LAST_HZ, t.last_hz);
                        r.set(&keys::DRIFT_HZ_S, t.drift_hz_s);
                        r.set(&FIRST_T, t.first_s);
                        r.set(&LAST_T, t.last_s);
                        r.set(&PEAK, f64::from(t.peak_db));
                        r.set(&FRAMES, i64::from(t.hits));
                    }
                }
            }
        }
        self.publish_findings();
        self.publish_survey();
        self.publish_hunter();
        self.publish_health();
        if let Some(m) = self.run.as_mut().and_then(|r| r.machine.as_mut()) {
            m.hit_events.clear();
            m.new_burst = None;
        }
    }

    /// Writes the page of the confirmed channels. Rows that left the page go.
    fn publish_findings(&mut self) {
        let Some(m) = self.run.as_ref().and_then(|r| r.machine.as_ref()) else {
            return;
        };
        let p = &mut self.p;
        let confirmed = || m.channels.all().iter().filter(|c| c.confirmed);
        p.set(&TOTAL, count(confirmed().count()));
        let page = &mut self.shown.page;
        page.clear();
        page.extend(confirmed().skip(self.offset).take(self.limit).map(|c| c.id));
        page.sort_unstable();
        for &old in &self.shown.findings {
            if page.binary_search(&old).is_err() {
                self.key.clear();
                let _ = write!(self.key, "{old}");
                p.remove_row(&FINDINGS, &self.key);
            }
        }
        for (i, c) in confirmed().skip(self.offset).take(self.limit).enumerate() {
            self.key.clear();
            let _ = write!(self.key, "{}", c.id);
            if let Some(mut r) = p.row_mut(&FINDINGS, &self.key) {
                r.set(&POSITION, count(i));
                write_channel(&mut r, c);
            }
        }
        std::mem::swap(&mut self.shown.findings, &mut self.shown.page);
    }

    fn publish_survey(&mut self) {
        let Some(s) = self
            .run
            .as_ref()
            .and_then(|r| r.machine.as_ref())
            .and_then(|m| m.survey.as_ref())
        else {
            return;
        };
        let p = &mut self.p;
        p.set(&SURVEY_SWEEPS, count64(s.sweeps()));
        s.persistent(self.values.get(&profile::PERSISTENT), &mut self.signals);
        for i in self.signals.len()..self.shown.persistent {
            self.key.clear();
            let _ = write!(self.key, "{i}");
            p.remove_row(&PERSISTENT, &self.key);
        }
        self.shown.persistent = self.signals.len().min(MAX_PERSISTENT);
        for (i, g) in self.signals.iter().enumerate().take(MAX_PERSISTENT).rev() {
            self.key.clear();
            let _ = write!(self.key, "{i}");
            if let Some(mut r) = p.row_mut(&PERSISTENT, &self.key) {
                r.set(&LOW_C, g.low_hz);
                r.set(&HIGH_C, g.high_hz);
                r.set(&OCCUPANCY, g.occupancy);
                r.set(&MEAN, f64::from(g.mean_db));
                r.set(&PEAK, f64::from(g.peak_db));
            }
        }
        p.results_mut().update_raster(self.ids.survey, |r| {
            s.fill_raster(r, RASTER_WIDTH);
            true
        });
    }

    fn publish_hunter(&mut self) {
        let Some(h) = self
            .run
            .as_ref()
            .and_then(|r| r.machine.as_ref())
            .and_then(|m| m.hunter.as_ref())
        else {
            return;
        };
        let p = &mut self.p;
        p.set(&LEARNT, h.learnt());
        let meter = h.meter();
        p.opt(&METER_HZ, meter.map(|m| m.hz));
        p.opt(&METER_LEVEL, meter.map(|m| f64::from(m.level_db)));
        p.opt(&METER_RISE, meter.map(|m| f64::from(m.rise_db)));
        p.opt(&METER_SMOOTH, meter.map(|m| f64::from(m.smoothed_db)));
        p.opt(&METER_TREND, meter.map(|m| m.trend));
        p.opt(&METER_MAX, meter.map(|m| f64::from(m.max_db)));
        p.opt(&METER_NEEDLE, meter.map(|m| f64::from(m.needle)));
        p.opt(&METER_TONE, meter.map(|m| f64::from(m.tone_hz)));
        for f in h.found() {
            let new = !self.shown.interference.contains(&f.id);
            if new {
                self.shown.interference.push(f.id);
                if self.shown.interference.len() > MAX_INTERFERENCE {
                    self.shown.interference.remove(0);
                }
                self.events.push(
                    NEW_INTERFERENCE
                        .event(self.now_s, f.peak_hz)
                        .set(&SOURCE_CLASS, f.class)
                        .set(&RISE, f64::from(f.rise_db))
                        .set(&LOW_C, f.low_hz)
                        .set(&HIGH_C, f.high_hz)
                        .set(&keys::TEXT, f.class.describe())
                        .set(&keys::FAMILY, Family::Receiver),
                );
            }
            self.key.clear();
            let _ = write!(self.key, "{}", f.id);
            if let Some(mut r) = p.row_mut(&INTERFERENCE, &self.key) {
                r.set(&LOW_C, f.low_hz);
                r.set(&HIGH_C, f.high_hz);
                r.set(&PEAK_HZ, f.peak_hz);
                r.set(&RISE, f64::from(f.rise_db));
                r.set(&SOURCE_CLASS, f.class);
                r.set(&keys::TEXT, f.class.describe());
                r.set(&FIRST_SEEN, f.first_seen_s);
                r.set(&LAST_SEEN, f.last_seen_s);
                r.set(&SEEN, count64(f.sweeps_seen));
                r.set(&SWEEPS_TOTAL, count64(f.sweeps_total));
            }
        }
    }

    fn publish_health(&mut self) {
        let Some(h) = self
            .run
            .as_ref()
            .and_then(|r| r.machine.as_ref())
            .and_then(|m| m.health.as_ref())
        else {
            return;
        };
        let p = &mut self.p;
        p.set(&HEALTH, h.overall);
        p.clear_rows(&HEALTH_BANDS);
        for b in h.bands.iter().rev() {
            if let Some(mut r) = p.row_mut(&HEALTH_BANDS, b.name) {
                r.set(&FLOOR_C, f64::from(b.floor_db));
                r.set(&PEAK, f64::from(b.peak_db));
                r.set(&SIGNALS, count(b.signals));
            }
        }
        p.clear_rows(&ADVICE);
        for (i, a) in h.advice.iter().enumerate().rev() {
            if let Some(mut r) = p.row_mut(&ADVICE, &i.to_string()) {
                r.set(&POSITION, count(i));
                r.set(&LEVEL, a.level);
                r.set(&keys::TEXT, &a.text);
            }
        }
    }

    fn not_now(&self) -> CommandError {
        self.not_now_on(self.ids.state)
    }

    fn not_now_on(&self, field: sdrkit_core::FieldId) -> CommandError {
        let state = self
            .p
            .results()
            .fields()
            .value(field)
            .cloned()
            .unwrap_or_default();
        CommandError::NotNow { field, state }
    }

    fn write_file(
        &self,
        path: &StorePath,
        f: impl FnOnce(&mut dyn std::io::Write) -> std::io::Result<()>,
    ) -> Result<u64, SweepError> {
        let storage = |source| SweepError::Storage {
            path: path.clone(),
            source,
        };
        let mut w = self
            .storage
            .open_writer(path, WriteMode::Truncate)
            .map_err(storage)?;
        let mut counted = Counted {
            inner: &mut *w,
            n: 0,
        };
        f(&mut counted).map_err(|e| storage(e.into()))?;
        let n = counted.n;
        w.flush().map_err(|e| storage(e.into()))?;
        w.sync().map_err(storage)?;
        Ok(n)
    }

    fn run_command(
        &mut self,
        cmd: SweepCommand,
        args: &Values,
    ) -> Result<Option<Record>, CommandError> {
        let mut reply = SweepCommand::commands()
            .def(cmd.id())
            .map(|d| d.new_reply())
            .filter(|r| !r.decl().is_empty());
        match cmd {
            SweepCommand::Start => self.start(args)?,
            SweepCommand::Stop | SweepCommand::Finish => {
                let now = self.now_s;
                let Some(run) = self.run.as_mut().filter(|r| !r.finished) else {
                    return Err(self.not_now());
                };
                match run.machine.as_mut() {
                    Some(m) if cmd == SweepCommand::Finish => run.pending = Some(m.finish_now(now)),
                    Some(m) => run.pending = Some(m.finish()),
                    None => {
                        run.finished = true;
                        self.p.set(&STATE, Phase::Done);
                    }
                }
                let end = if cmd == SweepCommand::Stop {
                    EndReason::Stopped
                } else {
                    EndReason::Finished
                };
                self.p.set(&END, end);
                self.publish_progress();
            }
            SweepCommand::HitAction => self.hit_action(args, &mut reply)?,
            SweepCommand::Exclude => {
                let (lo, hi) = (args.arg(&A_LOW_REQ)?, args.arg(&A_HIGH_REQ)?);
                if hi <= lo {
                    return Err(refused(args, A_HIGH_REQ.field.key, |id| {
                        SettingError::OutOfRange {
                            id,
                            min: lo,
                            max: f64::INFINITY,
                            got: hi,
                        }
                    }));
                }
                let not_now = self.not_now();
                let run = self.run.as_mut().filter(|r| !r.finished).ok_or(not_now)?;
                if let Some(m) = run.machine.as_mut() {
                    m.exclude(lo, hi);
                }
                self.exclusions.push((lo, hi));
            }
            SweepCommand::ResetMax => {
                let not_now = self.not_now();
                let m = self.machine_mut().ok_or(not_now)?;
                m.traces.reset_max();
            }
            SweepCommand::HuntTarget | SweepCommand::Relearn => {
                let hz = args.get(&A_TARGET);
                let not_now = self.not_now_on(self.ids.purpose);
                let h = self
                    .run
                    .as_mut()
                    .and_then(|r| r.machine.as_mut())
                    .and_then(|m| m.hunter.as_mut())
                    .ok_or(not_now)?;
                if cmd == SweepCommand::Relearn {
                    h.relearn();
                } else {
                    h.set_target((hz > 0.0).then_some(hz));
                }
                self.publish_hunter();
            }
            SweepCommand::BurstClear => {
                if let Some((_, b)) = self.machine_mut().and_then(|m| m.burst.as_mut()) {
                    b.clear();
                }
                self.p.clear_rows(&BURSTS);
            }
            SweepCommand::Findings => {
                self.limit = usize::try_from(args.get(&A_LIMIT)).unwrap_or(1);
                self.offset = usize::try_from(args.get(&A_OFFSET)).unwrap_or(0);
                self.publish_findings();
            }
            SweepCommand::Quiet => {
                let w = args.arg(&A_WIDTH)?;
                let n = usize::try_from(args.get(&A_COUNT)).unwrap_or(1);
                let s = self
                    .survey()
                    .ok_or_else(|| self.not_now_on(self.ids.purpose))?;
                let slots = s.quietest(w, n);
                let p = &mut self.p;
                p.clear_rows(&QUIET);
                for (i, sl) in slots.iter().enumerate().rev() {
                    if let Some(mut r) = p.row_mut(&QUIET, &i.to_string()) {
                        r.set(&POSITION, count(i));
                        r.set(&LOW_C, sl.low_hz);
                        r.set(&HIGH_C, sl.high_hz);
                        r.set(&OCCUPANCY, sl.occupancy);
                        r.set(&MEAN, f64::from(sl.mean_db));
                    }
                }
            }
            SweepCommand::Diff => {
                let path = file_arg(args)?;
                let s = self
                    .survey()
                    .ok_or_else(|| self.not_now_on(self.ids.purpose))?;
                let mut reader = self.storage.open_reader(&path).map_err(|source| {
                    failed(SweepError::Storage {
                        path: path.clone(),
                        source,
                    })
                })?;
                let older = BandSurvey::read_from(&mut reader, &path).map_err(failed)?;
                let d = s.diff(&older);
                let p = &mut self.p;
                p.set(&DIFF_LEVEL, f64::from(d.level_change_db));
                p.clear_rows(&DIFFS);
                for (i, c) in d.changes.iter().enumerate().take(MAX_DIFFS).rev() {
                    if let Some(mut r) = p.row_mut(&DIFFS, &i.to_string()) {
                        r.set(&LOW_C, c.low_hz);
                        r.set(&HIGH_C, c.high_hz);
                        r.set(&OCC_CHANGE, c.occupancy_change);
                        r.set(&CHANGE, c.kind);
                    }
                }
                if let Some(r) = reply.as_mut() {
                    r.set(&R_CHANGES, count(d.changes.len()));
                }
            }
            SweepCommand::Export => {
                let path = file_arg(args)?;
                let format = args.get(&A_FORMAT);
                let not_now = self.not_now_on(self.ids.purpose);
                let m = self.machine().ok_or_else(|| self.not_now())?;
                if m.sweeps == 0 {
                    return Err(failed(SweepError::NoSweep));
                }
                let unix_s = self.clock.now();
                let n = match format {
                    ExportFormat::RtlPower => self.write_file(&path, |mut w| {
                        files::write_rtl_power(
                            &mut w,
                            unix_s,
                            &m.grid,
                            &m.sweep_db,
                            m.samples_per_value(),
                            m.bins_per_hop(),
                        )
                    }),
                    ExportFormat::Traces => self.write_file(&path, |mut w| {
                        files::write_traces(
                            &mut w,
                            &m.grid,
                            Some(m.traces.current()),
                            Some(m.traces.max()),
                            Some(m.traces.average()),
                        )
                    }),
                    ExportFormat::Signals => self.write_file(&path, |mut w| {
                        files::write_signal_log(
                            &mut w,
                            m.channels.all().iter().filter(|c| c.confirmed),
                        )
                    }),
                    ExportFormat::SurveyCsv | ExportFormat::Survey => {
                        let s = m.survey.as_ref().ok_or(not_now)?;
                        if format == ExportFormat::Survey {
                            self.write_file(&path, |mut w| s.write_to(&mut w))
                        } else {
                            self.write_file(&path, |mut w| s.write_csv(&mut w))
                        }
                    }
                }
                .map_err(failed)?;
                if let Some(r) = reply.as_mut() {
                    r.set(&R_WRITTEN, count64(n));
                }
            }
            SweepCommand::FloorSave => {
                let path = file_arg(args)?;
                let m = self.machine().ok_or_else(|| self.not_now())?;
                if m.sweeps == 0 {
                    return Err(failed(SweepError::NoSweep));
                }
                let snap = m.floor.snapshot();
                let blob = StateBlob::new(FLOOR_STATE, FLOOR_VERSION, snap.to_bytes());
                let bytes = serde_json::to_vec(&blob).map_err(|_| failed(SweepError::Encode))?;
                self.storage.write(&path, &bytes).map_err(|source| {
                    failed(SweepError::Storage {
                        path: path.clone(),
                        source,
                    })
                })?;
                if let Some(r) = reply.as_mut() {
                    r.set(&R_BINS, count(snap.floor_db.len()));
                }
            }
            SweepCommand::FloorRestore => {
                let path = file_arg(args)?;
                if self.running().is_none() {
                    return Err(self.not_now());
                }
                let bytes = self.storage.read(&path).map_err(|source| {
                    failed(SweepError::Storage {
                        path: path.clone(),
                        source,
                    })
                })?;
                let not_a_floor = |source| {
                    failed(SweepError::NotAFloor {
                        path: path.clone(),
                        source,
                    })
                };
                let blob: StateBlob = serde_json::from_slice(&bytes)
                    .map_err(|_| not_a_floor(sdrkit_core::StateError::Damaged))?;
                let snap = blob
                    .open(FLOOR_STATE, FLOOR_VERSION)
                    .map_err(not_a_floor)
                    .and_then(|b| {
                        FloorSnapshot::from_bytes(b)
                            .ok_or_else(|| not_a_floor(sdrkit_core::StateError::Damaged))
                    })?;
                if let Some(m) = self.machine_mut() {
                    m.load_floor(&snap);
                }
                if let Some(r) = reply.as_mut() {
                    r.set(&R_BINS, count(snap.floor_db.len()));
                }
            }
        }
        Ok(reply)
    }

    fn survey(&self) -> Option<&BandSurvey> {
        self.machine()?.survey.as_ref()
    }

    fn start(&mut self, args: &Values) -> Result<(), CommandError> {
        let purpose = args.get(&A_PURPOSE);
        let preset = args.get(&A_PRESET);
        let source = if purpose == Purpose::Health {
            Source::Health
        } else if !preset.is_empty() {
            let i = PRESET_RANGES
                .iter()
                .position(|p| p.0 == preset)
                .ok_or_else(|| CommandError::NoSuchRow {
                    table: self.ids.presets,
                    key: preset.to_owned(),
                })?;
            Source::Preset(i)
        } else if args.get(&A_WHOLE) {
            Source::WholeRange
        } else {
            let (low_hz, high_hz) = (args.get(&A_LOW), args.get(&A_HIGH));
            if !(high_hz > low_hz && low_hz > 0.0) {
                return Err(refused(args, A_HIGH.field.key, |id| {
                    SettingError::OutOfRange {
                        id,
                        min: low_hz,
                        max: 1e12,
                        got: high_hz,
                    }
                }));
            }
            Source::Range { low_hz, high_hz }
        };
        let return_hz = self.end_run();
        self.exclusions.clear();
        self.run = Some(Run {
            session: args.arg(&SESSION)?.to_owned(),
            purpose,
            source,
            return_hz,
            machine: None,
            pending: None,
            finished: false,
        });
        self.shown = Shown::default();
        let p = &mut self.p;
        for t in [
            &FINDINGS,
            &BURSTS,
            &DIFFS,
            &DRIFT,
            &INTERFERENCE,
            &PERSISTENT,
            &QUIET,
            &RECORDINGS,
            &ADVICE,
            &HEALTH_BANDS,
        ] {
            p.clear_rows(t);
        }
        p.set(&STATE, Phase::Settle);
        p.set(&PURPOSE, purpose);
        p.set(&END, EndReason::None);
        p.set(&SESSION_F, args.arg(&SESSION)?);
        Ok(())
    }

    /// Ends a sweep that runs: the engine tunes back at the next step.
    /// Ends a sweep that runs. Returns where it tunes back to.
    fn end_run(&mut self) -> Option<f64> {
        let run = self.run.as_mut().filter(|r| !r.finished)?;
        run.finished = true;
        match run.machine.as_mut() {
            Some(m) => match m.finish() {
                Step::Done { return_hz } => Some(return_hz),
                _ => run.return_hz,
            },
            None => run.return_hz,
        }
    }

    /// Runs the command hit_action and writes its reply.
    fn hit_action(
        &mut self,
        args: &Values,
        reply: &mut Option<Record>,
    ) -> Result<(), CommandError> {
        let row = args.arg(&ROW)?;
        let action = args.arg(&A_ACTION)?;
        let no_row = || CommandError::NoSuchRow {
            table: self.ids.findings,
            key: row.to_owned(),
        };
        let id: u64 = row.parse().map_err(|_| no_row())?;
        let not_now = self.not_now();
        let (session, m) = match self.run.as_mut() {
            Some(Run {
                session,
                machine: Some(m),
                ..
            }) => (session.as_str(), m),
            _ => return Err(not_now),
        };
        let c: Channel = *m
            .channels
            .get(id)
            .filter(|c| c.confirmed)
            .ok_or_else(no_row)?;
        let out = m.act(id, action).ok_or_else(no_row)?;
        let demod = match args.get(&A_DEMOD) {
            "" => c.kind.demod(),
            given => given,
        };
        match out {
            ActionOutcome::Tune => {
                let to = match args.get(&A_TUNE_SESSION) {
                    "" => session,
                    s => s,
                };
                self.requests.push(HostRequest::Tune {
                    session: to.to_owned(),
                    freq_hz: Some(c.centre_hz),
                    target: TuneTarget::Running,
                    submode: demod.to_owned(),
                    rate_hz: None,
                    bandwidth_hz: Some(c.width_hz),
                    settings: Vec::new(),
                });
            }
            ActionOutcome::Save => {
                let mut name = String::new();
                let _ = write!(
                    name,
                    "{} {:.4} MHz",
                    ChannelType::CHOICES[usize::from(c.kind.index())].label,
                    c.centre_hz / 1e6
                );
                self.events.push(
                    SAVE.event(self.now_s, c.centre_hz)
                        .set(&NAME, &name)
                        .set(&DEMOD, demod)
                        .set(&WIDTH, c.width_hz)
                        .set(&keys::FAMILY, Family::Receiver),
                );
            }
            ActionOutcome::Lockout(_) | ActionOutcome::Priority(_) | ActionOutcome::Removed => {
                // The findings change at once.
                self.shown.sweeps = u64::MAX;
            }
        }
        if let Some(r) = reply.as_mut() {
            r.set(&R_FREQ, c.centre_hz);
            r.set(&DEMOD, demod);
            r.set(&R_BANDWIDTH, c.width_hz);
            if let ActionOutcome::Lockout(range) = out {
                r.set(&LOW_C, range.low_hz);
                r.set(&HIGH_C, range.high_hz);
            }
        }
        self.publish_progress();
        Ok(())
    }
}

/// Writes the columns of a confirmed channel.
fn write_channel(r: &mut RowMut<'_>, c: &Channel) {
    r.set(&keys::FREQ_HZ, c.centre_hz);
    r.set(&WIDTH, c.width_hz);
    r.set(&keys::SNR_DB, f64::from(c.snr_db));
    r.set(&PEAK, f64::from(c.peak_db));
    r.set(&TYPE, c.kind);
    r.set(&SEEN, count64(c.sweeps_seen));
    r.set(&PRESENT, c.share_present);
    r.set(&COUNT, i64::from(c.count));
    r.set(&FIRST_SEEN, c.first_seen_s);
    r.set(&LAST_SEEN, c.last_seen_s);
    r.set(&ACTIVE, c.active);
    r.set(&IMAGE, c.flags.image);
    r.set(&SPUR, c.flags.spur);
    r.set(&DC, c.flags.dc);
    r.set(&PRIORITY, c.priority);
}

/// A writer that counts the bytes it passes on.
struct Counted<'a> {
    inner: &'a mut dyn std::io::Write,
    n: u64,
}

impl std::io::Write for Counted<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.n += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Owner for SweepService {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        use SweepSetting as S;
        match S::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            S::CpuLoad => {
                self.values.set(id, value)?;
                let load = self.values.get(&profile::CPU_LOAD);
                if let Some(m) = self.machine_mut() {
                    m.cpu_load = load;
                }
                Ok(())
            }
            S::Bin
            | S::Rate
            | S::Averages
            | S::SettleAuto
            | S::Settle
            | S::Flush
            | S::Threshold
            | S::Hysteresis
            | S::MinDuration
            | S::Hang
            | S::MinSweeps
            | S::RasterStep
            | S::RasterOrigin
            | S::Usable
            | S::Overlap
            | S::PriorityEvery
            | S::DcAuto
            | S::DcBins
            | S::InnerKeep
            | S::Flatten
            | S::RemoveSpurs
            | S::Record
            | S::RecordKind
            | S::RecordMax
            | S::RecordPark
            | S::Every
            | S::Sweeps
            | S::RadioSweep
            | S::Averaging
            | S::BurstHz
            | S::BurstRise
            | S::BurstMin
            | S::Drift
            | S::Persistent
            | S::SurveyRows => self.values.set(id, value),
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
        SweepCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = SweepCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.run_command(cmd, args)
    }
}

//! The identify service as an owner: it arms a capture, analyses it as a job off the signal path, and publishes the signals, the
//! candidates and the verdict as tables and fields. It runs the automatic TV and digital voice choices, and requests that the
//! engine open a candidate with a [`HostRequest`].

use crate::error::IdentifyError;
use crate::identify::{Identification, Identifier, Look, Prior, Sureness, Verdict};
use crate::regions::Region;
use crate::signature::Signature;
use crate::tvauto::{tv_verdict, TvAction, TvAuto};
use crate::verify::{Check, Detector};
use crate::voiceauto::{voice_identifier, voice_systems, VoiceAction, VoiceAuto, VoiceSystem};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::{field, keys, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, SettingError, SettingSet};
use sdrkit_core::stack::StageError;
use sdrkit_core::TuneTarget;
use sdrkit_core::{
    Complex32, Event, EventDef, HostRequest, Job, JobControl, Owner, OwnerKey, OwnerSetting,
    Record, Results, ResultsDecl, Schema, Setting, SettingId, TableId, Value, Values,
};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

/// The owner key of the identify service.
pub const KEY: OwnerKey = OwnerKey::of("service.identify");

/// How a stack opens a signal type: the stack id, its submode and the settings to apply first.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenWith {
    /// The stack id.
    pub stack: String,
    /// The submode. Empty for the stack's default.
    pub submode: String,
    /// The settings to apply first, each with its owner.
    pub settings: Vec<OwnerSetting>,
}

/// Finds how a stack opens the signal type with this key. The engine supplies it when it makes the service, from what the stack
/// descriptors declare.
pub type OpenLookup = sdrkit_core::Lookup<OpenWith>;

const MAX_SIGNALS: usize = crate::regions::MAX_REGIONS;
const MAX_CANDIDATES: usize = MAX_SIGNALS * 16;
const MAX_LINES: usize = MAX_CANDIDATES * 32;
const MAX_SYSTEMS: usize = 64;

sdrkit_core::choice_enum! {
    /// Where the service is.
    pub enum IdentifyState {
        /// Nothing armed.
        Idle = ("idle", "Idle"),
        /// A capture is armed and the engine collects its samples.
        Armed = ("armed", "Armed"),
        /// A job analyses the capture.
        Working = ("working", "Working"),
        /// The last analysis is in the tables.
        Done = ("done", "Done"),
        /// The last analysis was cancelled.
        Cancelled = ("cancelled", "Cancelled"),
    }
}

sdrkit_core::choice_enum! {
    /// What to identify in a capture, or which automatic choice to run.
    pub enum Target {
        /// The strongest signal.
        Strongest = ("strongest", "Strongest signal"),
        /// The signal at the offset `offset_hz` from the centre.
        Click = ("click", "Signal at a click"),
        /// The signal between `lo_hz` and `hi_hz` from the centre.
        Region = ("region", "Signal in a region"),
        /// Every signal.
        All = ("all", "Every signal"),
        /// The automatic TV choice: looks at the channel and opens the TV standard found.
        Tv = ("tv", "TV (automatic)"),
        /// The automatic digital voice choice: looks every 20 s and opens the strongest sure voice system.
        Voice = ("voice", "Digital voice (automatic)"),
    }
}

// Settings.
const KEEP: Setting<field::I64> = Setting::new("keep", "Candidates kept", "", 6)
    .range(1.0, 16.0, 1.0)
    .help("How many candidates each signal keeps.");
const VERIFY_TOP: Setting<field::I64> = Setting::new("verify_top", "Candidates verified", "", 3)
    .range(0.0, 8.0, 1.0)
    .help("How many of the best candidates their own detectors verify.");

sdrkit_core::settings! {
    /// The settings of the identify service. A job takes them when it starts.
    pub enum IdentifySetting {
        /// The candidates kept for each signal.
        Keep = KEEP,
        /// The candidates verified.
        VerifyTop = VERIFY_TOP,
    }
}

// Results.
const STATE: Field<field::Choice<IdentifyState>> = Field::new("state", "State", "");
const PROGRESS: Field<field::F64> = Field::new("progress", "Progress", "");
const TARGET: Field<field::Choice<Target>> = Field::new("target", "Target", "");
const VERDICT_F: Field<field::Choice<Verdict>> = Field::new("verdict", "Verdict", "");
const SURENESS: Field<field::Choice<Sureness>> = Field::new("sureness", "Sureness", "");
const LINE: Field<field::Text> = Field::new("line", "In short", "");
const SUMMARY: Field<field::Text> = Field::new("summary", "Measurement", "");
const RATE: Field<field::Hz> = Field::new("rate_hz", "Capture rate", "Hz");
const CENTRE: Field<field::Hz> = Field::new("centre_hz", "Capture centre", "Hz");
const AUTO_TEXT: Field<field::Text> = Field::new("auto_text", "Automatic choice", "");
const OFFSET: Field<field::Hz> = Field::new("offset_hz", "Offset", "Hz");
const WIDTH: Field<field::Hz> = Field::new("width_hz", "Width (99 %)", "Hz");
const WIDTH_3DB: Field<field::Hz> = Field::new("width_3db_hz", "Width (3 dB)", "Hz");
const CLASS: Field<field::Text> = Field::new("class", "Modulation", "");
const SYMBOL_RATE: Field<field::F64> = Field::new("symbol_rate", "Symbol rate", "Bd");
const DUTY: Field<field::F64> = Field::new("duty", "Duty", "");
const OF_SIGNAL: Field<field::Text> = Field::new("signal", "Signal", "").refers_to_table("signals");
const RANK: Field<field::I64> = Field::new("rank", "Rank", "");
const NAME: Field<field::Text> = Field::new("name", "Name", "");
const SIGNAL_TYPE: Field<field::Text> = Field::new("signal_type", "Signal type", "");
const FAMILY: Field<field::Text> = Field::new("family", "Family", "");
const SCORE: Field<field::F64> = Field::new("score", "Score", "");
const RAW_SCORE: Field<field::F64> = Field::new("raw_score", "Score before the limit", "");
const PROBABILITY: Field<field::F64> = Field::new("probability", "Probability", "");
const FITS: Field<field::Bool> = Field::new("fits", "Fits", "");
const CHECK: Field<field::Choice<Check>> = Field::new("check", "Own detector", "");
const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
const SUBMODE: Field<field::Text> = Field::new("submode", "Submode", "");
const OF_CANDIDATE: Field<field::Text> =
    Field::new("candidate", "Candidate", "").refers_to_table("candidates");
const FEATURE: Field<field::Text> = Field::new("feature", "Feature", "");
const VALUE_TEXT: Field<field::Text> = Field::new("value", "Value", "");
const ITEM_OWNER: Field<field::Text> = Field::new("owner", "Owner", "");
const ITEM_KEY: Field<field::Text> = Field::new("key", "Key", "");
const ITEM_VALUE: Field<field::Any> = Field::new("value", "Value", "");
const ANALOGUE: Field<field::Bool> = Field::new("analogue", "Analogue", "");

/// One row for each signal of the last analysis, strongest first. The row key is `s<n>`.
pub const SIGNALS: TableDef = TableDef {
    key: "signals",
    label: "Signals",
    columns: &[
        &LINE,
        &SUMMARY,
        &keys::TEXT,
        &VERDICT_F,
        &SURENESS,
        &keys::FREQ_HZ,
        &OFFSET,
        &WIDTH,
        &WIDTH_3DB,
        &keys::SNR_DB,
        &keys::LEVEL_DBFS,
        &CLASS,
        &SYMBOL_RATE,
        &DUTY,
    ],
    cap: MAX_SIGNALS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The candidates of each signal, best first, and the stack that opens each. The row key is `<signal>/c<n>`.
pub const CANDIDATES: TableDef = TableDef {
    key: "candidates",
    label: "Candidates",
    columns: &[
        &OF_SIGNAL,
        &RANK,
        &NAME,
        &SIGNAL_TYPE,
        &FAMILY,
        &SCORE,
        &RAW_SCORE,
        &PROBABILITY,
        &FITS,
        &CHECK,
        &keys::FREQ_HZ,
        &WIDTH,
        &STACK,
        &SUBMODE,
    ],
    cap: MAX_CANDIDATES,
    marks: sdrkit_core::RowMarks::NONE,
};
/// One line for each rule that scored a candidate.
pub const EVIDENCE: TableDef = TableDef {
    key: "evidence",
    label: "Evidence",
    columns: &[&OF_CANDIDATE, &keys::TEXT],
    cap: MAX_LINES,
    marks: sdrkit_core::RowMarks::NONE,
};
/// What the detectors of a candidate found.
pub const NOTES: TableDef = TableDef {
    key: "notes",
    label: "Detector notes",
    columns: &[&OF_CANDIDATE, &keys::TEXT],
    cap: MAX_LINES,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The priors from band plans and the frequency database that moved a candidate.
pub const PRIORS: TableDef = TableDef {
    key: "priors",
    label: "Priors",
    columns: &[&OF_CANDIDATE, &keys::TEXT],
    cap: MAX_LINES,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The settings that the stack of a candidate applies to open it: owner key, setting key and typed value.
pub const OPEN_SETTINGS: TableDef = TableDef {
    key: "open_settings",
    label: "Settings to open",
    columns: &[&OF_CANDIDATE, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
    cap: MAX_LINES,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The measurements of each signal with the reason for each, in plain words.
pub const WHY: TableDef = TableDef {
    key: "why",
    label: "Why",
    columns: &[&OF_SIGNAL, &FEATURE, &VALUE_TEXT, &keys::TEXT],
    cap: MAX_LINES,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The voice systems that the automatic digital voice choice saw at its last look, loudest first.
pub const SYSTEMS: TableDef = TableDef {
    key: "systems",
    label: "Voice systems",
    columns: &[
        &NAME,
        &SIGNAL_TYPE,
        &keys::FREQ_HZ,
        &WIDTH,
        &keys::LEVEL_DBFS,
        &keys::SNR_DB,
        &SURENESS,
        &ANALOGUE,
    ],
    cap: MAX_SYSTEMS,
    marks: sdrkit_core::RowMarks::NONE,
};

// Events.
/// An analysis ended: done or cancelled. Its frequency is that of the strongest signal, or the centre of the capture.
pub static VERDICT: EventDef = EventDef::new(
    "identify.verdict",
    "Identification",
    &[&STATE, &VERDICT_F, &SURENESS, &keys::TEXT],
);

// Commands.
const A_TARGET: Setting<field::Choice<Target>> =
    Setting::choice("target", "Target", Target::Strongest);
const A_OFFSET: Setting<field::Hz> = Setting::new("offset_hz", "Offset", "Hz", 0.0)
    .range(-1e10, 1e10, 0.0)
    .help("The offset from the centre of a click.");
const A_LO: Setting<field::Hz> = Setting::new("lo_hz", "Low edge", "Hz", 0.0)
    .range(-1e10, 1e10, 0.0)
    .help("The low edge of a region, from the centre.");
const A_HI: Setting<field::Hz> = Setting::new("hi_hz", "High edge", "Hz", 0.0)
    .range(-1e10, 1e10, 0.0)
    .help("The high edge of a region, from the centre.");
const A_SESSION: Setting<field::Text> = Setting::new("session", "Session", "", "")
    .help("The session whose radio the capture reads. An automatic choice tunes it.");
const SESSION_F: Field<field::Text> =
    Field::new("session", "Session", "").refers_to("engine", "sessions");
const A_SESSION_REQ: Argument<field::Text> = Argument::new(SESSION_F);
const A_ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Candidate", "").refers_to_table("candidates"));

const C_ARM: Command = Command::new("arm", "Identify")
    .help("Arms a capture of the radio's samples, and identifies the target in it once it is full. The targets tv and voice start an automatic choice.")
    .args(&[&A_TARGET, &A_OFFSET, &A_LO, &A_HI, &A_SESSION]);
const C_CAPTURE: Command = Command::new("capture", "Identify again")
    .help("Identifies another target in the capture that the service holds, with no new samples.")
    .args(&[&A_TARGET, &A_OFFSET, &A_LO, &A_HI]);
const C_OPEN: Command = Command::new("open", "Open candidate")
    .help("Asks the engine to tune the session to the candidate and start the stack that opens it.")
    .args(&[&A_ROW, &A_SESSION_REQ]);
const C_CANCEL: Command = Command::new("cancel", "Cancel")
    .help("Cancels the analysis that runs and the automatic choice.");

sdrkit_core::commands! {
    /// The commands of the identify service.
    pub enum IdentifyCommand {
        /// Arms a capture, or starts an automatic choice.
        Arm = C_ARM,
        /// Identifies another target in the held capture.
        Capture = C_CAPTURE,
        /// Opens a candidate.
        Open = C_OPEN,
        /// Cancels the analysis and the automatic choice.
        Cancel = C_CANCEL,
    }
}

/// An analysis for the engine to run on a worker thread: call `job(&control)`. The service reads its progress through `control`
/// and cancels it through the same control. The job returns empty results: the service takes the outcome at
/// [`IdentifyService::tick`].
pub struct IdentifyJob {
    /// The work.
    pub job: Job,
    /// The control of the run.
    pub control: JobControl,
}

impl std::fmt::Debug for IdentifyJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentifyJob")
            .field("control", &self.control)
            .finish()
    }
}

/// A capture that the service holds: its samples, rate and centre.
struct Held {
    iq: Arc<[Complex32]>,
    rate_hz: f64,
    centre_hz: f64,
}

/// The end of a job: its number and the identifications, or `None` if it was cancelled.
type Outcome = (u64, Option<Vec<Identification>>);

/// An automatic choice that runs.
enum Auto {
    Tv(TvAuto),
    Voice(VoiceAuto),
}

struct AutoRun {
    auto: Auto,
    session: String,
    /// When to arm the next capture, on the clock of [`IdentifyService::tick`].
    next_s: Option<f64>,
    /// The delay before the next capture, set by the controller.
    after_s: f64,
}

/// The identify service.
pub struct IdentifyService {
    open: OpenLookup,
    identifier: Identifier,
    voice: Identifier,
    values: Values,
    state: IdentifyState,
    /// What the armed capture or the running job looks for.
    look: Look,
    held: Option<Held>,
    queued: Option<IdentifyJob>,
    running: Option<(u64, JobControl)>,
    jobs: u64,
    done_tx: Sender<Outcome>,
    done_rx: Receiver<Outcome>,
    auto: Option<AutoRun>,
    /// The session that the last `arm` command named, or empty.
    armed_session: String,
    now_s: f64,
    requests: Vec<HostRequest>,
    events: Vec<Event>,
    p: Published,
    ids: Ids,
}

#[derive(Clone, Copy, Debug)]
struct Ids {
    state: sdrkit_core::FieldId,
    candidates: TableId,
}

impl std::fmt::Debug for IdentifyService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentifyService")
            .field("state", &self.state)
            .field("held", &self.held.is_some())
            .finish()
    }
}

fn decl() -> Arc<ResultsDecl> {
    let mut d = ResultsDecl::new()
        .with_fields(&[
            &STATE,
            &PROGRESS,
            &TARGET,
            &VERDICT_F,
            &SURENESS,
            &keys::TEXT,
            &LINE,
            &SUMMARY,
            &RATE,
            &CENTRE,
            &AUTO_TEXT,
        ])
        .event(&VERDICT);
    for t in [
        &SIGNALS,
        &CANDIDATES,
        &EVIDENCE,
        &NOTES,
        &PRIORS,
        &OPEN_SETTINGS,
        &WHY,
        &SYSTEMS,
    ] {
        d = d.with_table(t);
    }
    Arc::new(d)
}

fn failed(e: IdentifyError) -> CommandError {
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

impl IdentifyService {
    /// Makes the service with the built-in signatures and detectors. `open` finds how a stack opens a signal type.
    pub fn new(open: OpenLookup) -> Self {
        let p = Published::from_decl(decl());
        let d = p.results().decl();
        let ids = Ids {
            state: d
                .field_id(STATE.key)
                .expect("the service declares its fields"),
            candidates: d
                .table_id(CANDIDATES.key)
                .expect("the service declares its tables"),
        };
        let (done_tx, done_rx) = channel();
        let mut s = IdentifyService {
            open,
            identifier: Identifier::new(),
            voice: voice_identifier(),
            values: Values::defaults(&IdentifySetting::schema()),
            state: IdentifyState::Idle,
            look: Look::Strongest,
            held: None,
            queued: None,
            running: None,
            jobs: 0,
            done_tx,
            done_rx,
            auto: None,
            armed_session: String::new(),
            now_s: 0.0,
            requests: Vec::new(),
            events: Vec::new(),
            p,
            ids,
        };
        s.set_state(IdentifyState::Idle);
        s.p.set(&TARGET, Target::Strongest);
        s.p.set(&VERDICT_F, Verdict::Unknown);
        s.p.set(&SURENESS, Sureness::None);
        s
    }

    /// Adds a signature that a stack declares.
    pub fn add_signature(&mut self, s: Signature) {
        self.identifier.add_signature(s);
    }

    /// Adds a detector that a stack declares.
    pub fn add_detector(&mut self, d: Arc<dyn Detector>) {
        self.identifier.add_detector(d.clone());
        self.voice.add_detector(d);
    }

    /// Sets the priors from the band plans and the frequency database. The next analysis uses them.
    pub fn set_priors(&mut self, priors: Vec<Prior>) {
        self.identifier.priors.clone_from(&priors);
        self.voice.priors = priors;
    }

    /// The number of samples that the armed capture requires at `rate_hz`, or `None` if no capture is armed.
    pub fn wanted(&self, rate_hz: f64) -> Option<usize> {
        (self.state == IdentifyState::Armed).then(|| Identifier::capture_len(rate_hz))
    }

    /// The session that the command `arm` named, while a capture is armed. `None` if no capture is armed or the command named no session.
    pub fn armed_session(&self) -> Option<&str> {
        (self.state == IdentifyState::Armed && !self.armed_session.is_empty())
            .then_some(self.armed_session.as_str())
    }

    /// Hands the service the samples of the armed capture, by reference, at `rate_hz` and centred on `centre_hz`. The service
    /// copies them once, to hold them for the job and for the command `capture`, and queues the job ([`IdentifyService::take_job`]).
    ///
    /// # Errors
    ///
    /// [`IdentifyError::NotArmed`] if no capture is armed. [`IdentifyError::EmptyCapture`] if `iq` is empty or the rate is not
    /// above 0.
    pub fn deliver(
        &mut self,
        iq: &[Complex32],
        rate_hz: f64,
        centre_hz: f64,
    ) -> Result<(), IdentifyError> {
        if self.state != IdentifyState::Armed {
            return Err(IdentifyError::NotArmed);
        }
        if iq.is_empty() || !(rate_hz > 0.0 && rate_hz.is_finite()) {
            return Err(IdentifyError::EmptyCapture);
        }
        self.held = Some(Held {
            iq: Arc::from(iq),
            rate_hz,
            centre_hz,
        });
        self.p.set(&RATE, rate_hz);
        self.p.set(&CENTRE, centre_hz);
        self.queue_job();
        Ok(())
    }

    /// The job that the last delivery or command queued, for the engine to run off the signal path.
    pub fn take_job(&mut self) -> Option<IdentifyJob> {
        self.queued.take()
    }

    /// Moves the service on, at `now_s` seconds on the engine's clock: it takes the outcome of a job that ended, publishes its
    /// progress, and arms the next capture of an automatic choice when it is due. The engine calls it at its ticks.
    pub fn tick(&mut self, now_s: f64) {
        self.now_s = now_s;
        while let Ok((n, outcome)) = self.done_rx.try_recv() {
            if self.running.as_ref().is_some_and(|(r, _)| *r == n) {
                self.running = None;
                self.end(outcome);
            }
        }
        if let Some((_, ctl)) = &self.running {
            self.p.set(&PROGRESS, f64::from(ctl.progress()));
        }
        let due = self
            .auto
            .as_ref()
            .and_then(|a| a.next_s)
            .is_some_and(|t| now_s >= t);
        let busy = matches!(self.state, IdentifyState::Armed | IdentifyState::Working);
        if due && !busy {
            if let Some(a) = self.auto.as_mut() {
                a.next_s = None;
                self.look = match a.auto {
                    Auto::Tv(_) => Look::Strongest,
                    Auto::Voice(_) => Look::All,
                };
            }
            self.set_state(IdentifyState::Armed);
        }
    }

    /// Takes the requests for the engine that commands and automatic choices made, oldest first.
    pub fn take_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.requests)
    }

    /// Moves the events of the service to the end of `out`, oldest first.
    pub fn take_events(&mut self, out: &mut Vec<Event>) {
        out.append(&mut self.events);
    }

    fn set_state(&mut self, state: IdentifyState) {
        self.state = state;
        self.p.set(&STATE, state);
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

    /// Makes the job for the held capture and `self.look`.
    fn queue_job(&mut self) {
        let Some(h) = &self.held else { return };
        if let Some((_, ctl)) = self.running.take() {
            ctl.cancel();
        }
        let mut id = if matches!(
            self.auto,
            Some(AutoRun {
                auto: Auto::Voice(_),
                ..
            })
        ) {
            self.voice.clone()
        } else {
            self.identifier.clone()
        };
        id.keep = usize::try_from(self.values.get(&KEEP)).unwrap_or(1);
        id.verify_top = usize::try_from(self.values.get(&VERIFY_TOP)).unwrap_or(0);
        self.jobs += 1;
        let n = self.jobs;
        let (iq, rate, rf, look) = (Arc::clone(&h.iq), h.rate_hz, h.centre_hz, self.look);
        let tx = self.done_tx.clone();
        let control = JobControl::new(|_| {});
        let job: Job = Box::new(move |ctl: &JobControl| {
            let out = id.look(&iq, rate, rf, look, |done| {
                ctl.set_progress(done);
                !ctl.cancelled()
            });
            // The service can be gone when the job ends: the outcome then has no reader.
            let _ = tx.send((n, out));
            Ok(Results::default())
        });
        self.running = Some((n, control.clone()));
        self.queued = Some(IdentifyJob { job, control });
        self.p.set(&PROGRESS, 0.0);
        self.set_state(IdentifyState::Working);
    }

    /// Publishes the outcome of the job that ended, emits the verdict event, and moves an automatic choice on.
    fn end(&mut self, outcome: Option<Vec<Identification>>) {
        let centre = self.held.as_ref().map_or(0.0, |h| h.centre_hz);
        let Some(ids) = outcome else {
            self.set_state(IdentifyState::Cancelled);
            self.events.push(
                VERDICT
                    .event(self.now_s, centre)
                    .set(&STATE, IdentifyState::Cancelled)
                    .set(&VERDICT_F, Verdict::Unknown)
                    .set(&SURENESS, Sureness::None)
                    .set(&keys::TEXT, "Cancelled"),
            );
            return;
        };
        self.p.set(&PROGRESS, 1.0);
        self.publish(&ids);
        self.set_state(IdentifyState::Done);
        let first = ids.first();
        self.events.push(
            VERDICT
                .event(self.now_s, first.map_or(centre, |i| i.features.rf_hz))
                .set(&STATE, IdentifyState::Done)
                .set(&VERDICT_F, first.map_or(Verdict::Unknown, |i| i.verdict))
                .set(&SURENESS, first.map_or(Sureness::None, |i| i.sureness))
                .set(
                    &keys::TEXT,
                    first.map_or("No signal", |i| i.headline.as_str()),
                ),
        );
        self.step_auto(&ids, centre);
    }

    fn publish(&mut self, ids: &[Identification]) {
        for t in [
            &SIGNALS,
            &CANDIDATES,
            &EVIDENCE,
            &NOTES,
            &PRIORS,
            &OPEN_SETTINGS,
            &WHY,
        ] {
            self.p.clear_rows(t);
        }
        let first = ids.first();
        self.p
            .set(&VERDICT_F, first.map_or(Verdict::Unknown, |i| i.verdict));
        self.p
            .set(&SURENESS, first.map_or(Sureness::None, |i| i.sureness));
        self.p
            .set(&keys::TEXT, first.map_or("", |i| i.headline.as_str()));
        self.p.set(&LINE, first.map_or("", |i| i.line.as_str()));
        self.p
            .set(&SUMMARY, first.map_or("", |i| i.summary.as_str()));
        for (i, id) in ids.iter().enumerate() {
            let sk = format!("s{i}");
            let f = &id.features;
            if let Some(mut r) = self.p.row_mut(&SIGNALS, &sk) {
                r.set(&LINE, &id.line);
                r.set(&SUMMARY, &id.summary);
                r.set(&keys::TEXT, &id.headline);
                r.set(&VERDICT_F, id.verdict);
                r.set(&SURENESS, id.sureness);
                r.set(&keys::FREQ_HZ, f.rf_hz);
                r.set(&OFFSET, f.offset_hz);
                r.set(&WIDTH, f.width_99_hz);
                r.set(&WIDTH_3DB, f.width_3db_hz);
                r.set(&keys::SNR_DB, f.snr_db);
                r.set(&keys::LEVEL_DBFS, f.power_dbfs);
                r.set(&CLASS, f.class.word());
                r.opt(&SYMBOL_RATE, f.symbol_rate);
                r.set(&DUTY, f.duty);
            }
            for (j, w) in id.why.iter().enumerate() {
                if let Some(mut r) = self.p.row_mut(&WHY, &format!("{sk}/w{j}")) {
                    r.set(&OF_SIGNAL, &sk);
                    r.set(&FEATURE, &w.feature);
                    r.set(&VALUE_TEXT, &w.value);
                    r.set(&keys::TEXT, &w.why);
                }
            }
            for (j, c) in id.candidates.iter().enumerate() {
                let ck = format!("{sk}/c{j}");
                let open = (self.open)(&c.signal);
                if let Some(mut r) = self.p.row_mut(&CANDIDATES, &ck) {
                    r.set(&OF_SIGNAL, &sk);
                    r.set(&RANK, i64::try_from(j).unwrap_or(i64::MAX));
                    r.set(&NAME, &c.name);
                    r.set(&SIGNAL_TYPE, &c.signal);
                    r.set(&FAMILY, &c.family);
                    r.set(&SCORE, c.score);
                    r.set(&RAW_SCORE, c.raw_score);
                    r.set(&PROBABILITY, c.probability);
                    r.set(&FITS, c.fits);
                    r.opt(&CHECK, c.verification.as_ref().map(|v| v.check));
                    r.set(&keys::FREQ_HZ, c.freq_hz);
                    r.set(&WIDTH, c.width_hz);
                    r.set(&STACK, open.as_ref().map_or("", |o| o.stack.as_str()));
                    r.set(&SUBMODE, open.as_ref().map_or("", |o| o.submode.as_str()));
                }
                lines(&mut self.p, &EVIDENCE, &ck, "e", &c.evidence);
                lines(&mut self.p, &PRIORS, &ck, "p", &c.priors);
                if let Some(v) = &c.verification {
                    lines(&mut self.p, &NOTES, &ck, "n", &v.notes);
                }
                for s in open.iter().flat_map(|o| &o.settings) {
                    let key = format!("{ck}/{}/{}", s.owner.as_str(), s.key);
                    if let Some(mut r) = self.p.row_mut(&OPEN_SETTINGS, &key) {
                        r.set(&OF_CANDIDATE, &ck);
                        r.set(&ITEM_OWNER, s.owner.as_str());
                        r.set(&ITEM_KEY, &s.key);
                        r.set(&ITEM_VALUE, &s.value);
                    }
                }
            }
        }
    }

    fn publish_systems(&mut self, systems: &[VoiceSystem]) {
        self.p.clear_rows(&SYSTEMS);
        for (i, s) in systems.iter().enumerate() {
            if let Some(mut r) = self.p.row_mut(&SYSTEMS, &format!("v{i}")) {
                r.set(&NAME, &s.name);
                r.set(&SIGNAL_TYPE, &s.signal);
                r.set(&keys::FREQ_HZ, s.freq_hz);
                r.set(&WIDTH, s.width_hz);
                r.set(&keys::LEVEL_DBFS, s.level_dbfs);
                r.set(&keys::SNR_DB, s.snr_db);
                r.set(&SURENESS, s.sureness);
                r.set(&ANALOGUE, s.analogue);
            }
        }
    }

    /// The request that tunes `session` to `freq_hz` and opens `signal`, or the error if no stack opens it.
    fn tune(
        &self,
        session: &str,
        signal: &str,
        freq_hz: Option<f64>,
        bandwidth_hz: Option<f64>,
    ) -> Result<HostRequest, IdentifyError> {
        let o = (self.open)(signal).ok_or_else(|| IdentifyError::NoStack {
            signal: signal.to_owned(),
        })?;
        Ok(HostRequest::Tune {
            session: session.to_owned(),
            freq_hz,
            target: TuneTarget::Stack(o.stack),
            submode: o.submode,
            rate_hz: None,
            bandwidth_hz,
            settings: o.settings,
        })
    }

    /// Moves the automatic choice on with the identifications of its capture, made at the centre `rf_hz`.
    fn step_auto(&mut self, ids: &[Identification], rf_hz: f64) {
        let Some(mut a) = self.auto.take() else {
            return;
        };
        let mut text = None;
        let mut log = None;
        let mut request = None;
        let mut keep = true;
        match &mut a.auto {
            Auto::Tv(tv) => {
                let v = ids.first().map(tv_verdict);
                let action = match &v {
                    Some(v) => tv.on_verdict(v, rf_hz),
                    None => TvAction::GiveUp {
                        message: "No TV signal found here.".into(),
                    },
                };
                if let Some(v) = &v {
                    log = Some(v.headline.clone());
                }
                match action {
                    TvAction::Capture {
                        after_s, message, ..
                    } => {
                        a.after_s = after_s;
                        text = Some(message);
                    }
                    TvAction::GiveUp { message } | TvAction::Keep { message } => {
                        text = Some(message);
                        keep = false;
                    }
                    TvAction::Switch {
                        signal,
                        bandwidth_hz,
                        tune_hz,
                        message,
                        log: line,
                    } => {
                        request = Some(self.tune(&a.session, signal, tune_hz, bandwidth_hz));
                        text = Some(message);
                        log = Some(line);
                        keep = false;
                    }
                }
            }
            Auto::Voice(voice) => {
                let found = voice_systems(ids);
                let action = voice.on_scan(self.now_s, found);
                let systems = voice.systems().to_vec();
                self.publish_systems(&systems);
                a.after_s = crate::voiceauto::VOICE_RESCAN_S;
                match action {
                    VoiceAction::Play { system } => {
                        request = Some(self.tune(
                            &a.session,
                            &system.signal,
                            Some(system.freq_hz),
                            Some(system.width_hz),
                        ));
                        text = Some(format!("Playing {}", system.name));
                        log = Some(format!(
                            "switched to {} at {:.4} MHz",
                            system.name,
                            system.freq_hz / 1e6
                        ));
                    }
                    VoiceAction::Keep => {}
                    VoiceAction::Nothing { message } => text = Some(message),
                }
            }
        }
        match request {
            Some(Ok(r)) => self.requests.push(r),
            Some(Err(e)) => {
                text = Some(format!("{e}. Pick a stack by hand."));
            }
            None => {}
        }
        if let Some(t) = text {
            self.p.set(&AUTO_TEXT, &t);
        }
        if let Some(l) = log {
            self.p.results_mut().log(l);
        }
        if keep {
            a.next_s = Some(self.now_s + a.after_s);
            self.auto = Some(a);
        }
    }

    /// The look of the target arguments, or the refusal of a region whose edges do not fit.
    fn look_of(args: &Values) -> Result<(Target, Look), CommandError> {
        let target = args.get(&A_TARGET);
        let look = match target {
            Target::Strongest | Target::Tv => Look::Strongest,
            Target::Click => Look::Click(args.get(&A_OFFSET)),
            Target::Region => {
                let (lo, hi) = (args.get(&A_LO), args.get(&A_HI));
                if hi <= lo {
                    let with = args
                        .schema()
                        .id(A_LO.key())
                        .map_err(|_| CommandError::NotAnArgument { key: A_LO.key() })?;
                    return Err(refused(args, A_HI.key(), |id| SettingError::Conflict {
                        id,
                        with,
                    }));
                }
                Look::Region(Region {
                    lo_hz: lo,
                    hi_hz: hi,
                    peak_db: 0.0,
                })
            }
            Target::All | Target::Voice => Look::All,
        };
        Ok((target, look))
    }

    fn run_command(
        &mut self,
        cmd: IdentifyCommand,
        args: &Values,
    ) -> Result<Option<Record>, CommandError> {
        match cmd {
            IdentifyCommand::Arm => {
                let (target, look) = Self::look_of(args)?;
                let session = args.get(&A_SESSION);
                session.clone_into(&mut self.armed_session);
                self.auto = match target {
                    Target::Tv | Target::Voice if session.is_empty() => {
                        return Err(refused(args, A_SESSION.key(), |id| {
                            SettingError::Required { id }
                        }));
                    }
                    Target::Tv => {
                        let mut tv = TvAuto::default();
                        let after_s = match tv.start() {
                            TvAction::Capture {
                                after_s, message, ..
                            } => {
                                self.p.set(&AUTO_TEXT, &message);
                                after_s
                            }
                            _ => 0.0,
                        };
                        Some(AutoRun {
                            auto: Auto::Tv(tv),
                            session: session.to_owned(),
                            next_s: Some(self.now_s + after_s),
                            after_s,
                        })
                    }
                    Target::Voice => {
                        self.p.set(&AUTO_TEXT, "Looking for digital voice...");
                        Some(AutoRun {
                            auto: Auto::Voice(VoiceAuto::default()),
                            session: session.to_owned(),
                            next_s: Some(self.now_s),
                            after_s: crate::voiceauto::VOICE_RESCAN_S,
                        })
                    }
                    _ => None,
                };
                if let Some((_, ctl)) = self.running.take() {
                    ctl.cancel();
                }
                self.queued = None;
                self.look = look;
                self.p.set(&TARGET, target);
                if self.auto.is_some() {
                    self.set_state(IdentifyState::Idle);
                    self.tick(self.now_s);
                } else {
                    self.set_state(IdentifyState::Armed);
                }
                Ok(None)
            }
            IdentifyCommand::Capture => {
                if self.held.is_none() || matches!(self.state, IdentifyState::Armed) {
                    return Err(self.not_now());
                }
                let (target, look) = Self::look_of(args)?;
                if matches!(target, Target::Tv | Target::Voice) {
                    return Err(refused(args, A_TARGET.key(), |id| {
                        SettingError::NotPermitted { id }
                    }));
                }
                self.auto = None;
                self.look = look;
                self.p.set(&TARGET, target);
                self.queue_job();
                Ok(None)
            }
            IdentifyCommand::Open => {
                let row = args.arg(&A_ROW)?;
                let session = args.arg(&A_SESSION_REQ)?;
                let (signal, freq, width) = {
                    let rec = self
                        .p
                        .results()
                        .table(self.ids.candidates)
                        .and_then(|t| t.row(row))
                        .ok_or_else(|| CommandError::NoSuchRow {
                            table: self.ids.candidates,
                            key: row.to_owned(),
                        })?;
                    (
                        rec.get(&SIGNAL_TYPE).unwrap_or_default().to_owned(),
                        rec.get(&keys::FREQ_HZ),
                        rec.get(&WIDTH),
                    )
                };
                let r = self.tune(session, &signal, freq, width).map_err(failed)?;
                self.requests.push(r);
                Ok(None)
            }
            IdentifyCommand::Cancel => {
                self.auto = None;
                self.queued = None;
                match self.running.take() {
                    Some((_, ctl)) => {
                        ctl.cancel();
                        self.set_state(IdentifyState::Cancelled);
                    }
                    None => self.set_state(IdentifyState::Idle),
                }
                Ok(None)
            }
        }
    }
}

/// Writes one row of `def` for each line, keyed `<of>/<prefix><n>`.
fn lines(p: &mut Published, def: &TableDef, of: &str, prefix: &str, lines: &[String]) {
    for (i, l) in lines.iter().enumerate() {
        if let Some(mut r) = p.row_mut(def, &format!("{of}/{prefix}{i}")) {
            r.set(&OF_CANDIDATE, of);
            r.set(&keys::TEXT, l);
        }
    }
}

impl Owner for IdentifyService {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match IdentifySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IdentifySetting::Keep | IdentifySetting::VerifyTop => self.values.set(id, value),
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
        IdentifyCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = IdentifyCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.run_command(cmd, args)
    }
}

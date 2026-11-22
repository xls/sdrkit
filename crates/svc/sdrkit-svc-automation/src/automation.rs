//! The automation: the owner `service.automation`. Rules, their actions, named areas and schedules are tables that commands
//! set. On each event the engine calls [`Automation::handle`], and on each tick [`Automation::tick`]. Both give events: the
//! crossings of areas, the due schedules, and the requests of the rules that the engine and the host carry out.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::owner::{Owner, OwnerKey};
use sdrkit_core::publish::keys::{self, Urgency};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, Setting, SettingError, SettingId, SettingSet};
use sdrkit_core::storage::Storage;
use sdrkit_core::{
    Event, EventDef, EventKindId, Record, Results, ResultsDecl, Schema, StorageError, StorePath,
    Value, Values,
};
use sdrkit_svc_events::service::{failed, no_such_row, reply, table_id};
use sdrkit_svc_events::{choice_key, choice_of_key, Context, Kinds, NameError, Observation};
use serde::{Deserialize, Serialize};

use crate::error::{AutomationError, TextError};
use crate::events::{
    RecordWhat, ARGS, AUTOMATION_EVENTS, CPA_KM, FENCE, GEO_CPA, GEO_ENTER, GEO_EXIT, MAX_BYTES,
    NAME, NOTIFY, OUTPUT, RECORD, REWIND, RULE, RUN_SCRIPT, SCHEDULE, SCHEDULE_DUE, SCRIPT,
    SECONDS, SEND, STACK, TCPA, TITLE, TRIGGER, TUNE, WHAT,
};
use crate::fence::{Cpa, FenceText, GeoChange, GeoWatch, NamedFence, Shape};
use crate::rule::{Action, ActionKind, ActionText, Rule, RuleText};
use crate::schedule::{Schedule, ScheduleText};

/// The owner key of the automation.
pub const KEY: OwnerKey = OwnerKey::of("service.automation");
const OWNER: &str = "service.automation";

/// The file in the host's storage that keeps the rules, actions, areas and schedules.
pub const FILE: &str = "automation/rules.json";

/// The most named areas.
pub const MAX_FENCES: usize = 64;

/// Closest approach warnings are on.
pub const CPA_ON: Setting<field::Bool> = Setting::new("cpa_on", "Closest approach", "", false);
/// A warning comes when the closest approach is at most this far.
pub const CPA_MAX_KM: Setting<field::F64> =
    Setting::new("cpa_max_km", "Closest approach within", "km", 1.852).range(0.0, 1000.0, 0.001);
/// A warning comes when the closest approach is at most this far ahead.
pub const CPA_MAX_TCPA: Setting<field::Duration> =
    Setting::new("cpa_max_tcpa", "Closest approach within", "s", 1800.0).range(0.0, 86_400.0, 1.0);
/// The families that the warning watches, separated by commas. Empty watches every family.
pub const CPA_FAMILIES: Setting<field::Text> = Setting::new(
    "cpa_families",
    "Closest approach families",
    "",
    "vessel, aircraft",
);
/// No second warning for the same object within this time.
pub const CPA_REPEAT: Setting<field::Duration> =
    Setting::new("cpa_repeat", "Warn again after", "s", 600.0).range(0.0, 86_400.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the automation.
    pub enum AutomationSetting {
        /// [`CPA_ON`].
        CpaOn = CPA_ON,
        /// [`CPA_MAX_KM`].
        CpaMaxKm = CPA_MAX_KM,
        /// [`CPA_MAX_TCPA`].
        CpaMaxTcpa = CPA_MAX_TCPA,
        /// [`CPA_FAMILIES`].
        CpaFamilies = CPA_FAMILIES,
        /// [`CPA_REPEAT`].
        CpaRepeat = CPA_REPEAT,
    }
}

/// The row key that a command acts on.
pub const ROW: Argument<field::Text> = Argument::new(Field::new("row", "Row", ""));

/// A name for people.
pub const R_NAME: Setting<field::Text> = Setting::new("name", "Name", "", "");
/// Off rows never run.
pub const R_ENABLED: Setting<field::Bool> = Setting::new("enabled", "On", "", true);
/// The event kinds that start the rule, separated by commas (`"adsb.*, geo.enter"`). Empty takes every kind.
pub const R_KINDS: Setting<field::Text> = Setting::new("kinds", "Kinds", "", "");
/// The families that start the rule, separated by commas. Empty takes every family.
pub const R_FAMILIES: Setting<field::Text> = Setting::new("families", "Families", "", "");
/// Only urgent events start the rule.
pub const R_URGENT_ONLY: Setting<field::Bool> =
    Setting::new("urgent_only", "Urgent only", "", false);
/// The condition. Empty is always true.
pub const R_CONDITION: Setting<field::Text> = Setting::new("condition", "Condition", "", "");
/// Days of the week, Monday 0 to Sunday 6, separated by commas. Empty is every day.
pub const R_DAYS: Setting<field::Text> = Setting::new("days", "Days", "", "");
/// Start of the window, `HH:MM` local time. Empty with `to` empty is the whole day.
pub const R_FROM: Setting<field::Text> = Setting::new("from", "From", "", "");
/// End of the window, `HH:MM` local time, exclusive.
pub const R_TO: Setting<field::Text> = Setting::new("to", "To", "", "");
/// The named area that the object must be inside or outside. Empty for none.
pub const R_FENCE: Setting<field::Text> = Setting {
    field: Field::new("fence", "Area", "").refers_to(OWNER, "rule_fences"),
    ..Setting::new("fence", "Area", "", "")
};
/// True: the object must be inside the area. False: outside.
pub const R_INSIDE: Setting<field::Bool> = Setting::new("inside", "Inside", "", true);
/// The object must be at least this far from the receiver. 0 for no limit.
pub const R_MIN_KM: Setting<field::F64> =
    Setting::new("min_km", "Nearest", "km", 0.0).range(0.0, 40_000.0, 0.1);
/// The object must be at most this far from the receiver. 0 for no limit.
pub const R_MAX_KM: Setting<field::F64> =
    Setting::new("max_km", "Farthest", "km", 0.0).range(0.0, 40_000.0, 0.1);
/// The rule does not run again within this time.
pub const R_COOLDOWN: Setting<field::Duration> =
    Setting::new("cooldown", "Cooldown", "s", 0.0).range(0.0, 1e7, 1.0);
/// The cooldown counts for each object on its own.
pub const R_PER_IDENTITY: Setting<field::Bool> =
    Setting::new("per_identity", "Per object", "", true);
/// The most runs in any hour. 0 for no limit.
pub const R_MAX_PER_HOUR: Setting<field::I64> =
    Setting::new("max_per_hour", "Most per hour", "", 0).range(0.0, 1e6, 1.0);

/// Adds a rule, or changes the rule with this row key.
pub const RULE_SET: Command = Command::new("rule.set", "Set a rule")
    .help("Adds a rule or changes it. Names in the text must resolve.")
    .args(&[
        &ROW,
        &R_NAME,
        &R_ENABLED,
        &R_KINDS,
        &R_FAMILIES,
        &R_URGENT_ONLY,
        &R_CONDITION,
        &R_DAYS,
        &R_FROM,
        &R_TO,
        &R_FENCE,
        &R_INSIDE,
        &R_MIN_KM,
        &R_MAX_KM,
        &R_COOLDOWN,
        &R_PER_IDENTITY,
        &R_MAX_PER_HOUR,
    ]);
/// Removes a rule and its actions.
pub const RULE_REMOVE: Command = Command::new("rule.remove", "Remove a rule").args(&[&ROW]);

/// The rule of an action.
pub const A_RULE: Argument<field::Text> =
    Argument::new(Field::new("rule", "Rule", "").refers_to(OWNER, "rules"));
/// What the action does.
pub const A_ACTION: Setting<field::Choice<ActionKind>> =
    Setting::choice("action", "Action", ActionKind::Notify);
/// The title of a notification, a template.
pub const A_TITLE: Setting<field::Text> = Setting::new("title", "Title", "", "");
/// The text of a notification, a log row or an output, a template.
pub const A_TEXT: Setting<field::Text> = Setting::new("text", "Text", "", "");
/// The notification is urgent.
pub const A_URGENT: Setting<field::Bool> = Setting::new("urgent", "Urgent", "", false);
/// The frequency to tune to, Hz, a template (`"{freq_hz}"`).
pub const A_FREQ: Setting<field::Text> = Setting::new("freq", "Frequency", "", "");
/// The stack to tune to. Empty keeps the stack.
pub const A_STACK: Setting<field::Text> = Setting::new("stack", "Stack", "", "");
/// What to record.
pub const A_WHAT: Setting<field::Choice<RecordWhat>> =
    Setting::choice("what", "Record", RecordWhat::Audio);
/// The length of a recording or of the saved rewind.
pub const A_SECONDS: Setting<field::Duration> =
    Setting::new("seconds", "Length", "s", 30.0).range(0.0, 86_400.0, 1.0);
/// The size cap of a recording, bytes. 0 is the recorder's own cap.
pub const A_MAX_BYTES: Setting<field::I64> =
    Setting::new("max_bytes", "Most bytes", "", 0).range(0.0, 1e13, 1.0);
/// The name of a host script.
pub const A_SCRIPT: Setting<field::Text> = Setting::new("script", "Script", "", "");
/// The arguments of the script, one per line, a template.
pub const A_ARGS: Setting<field::Text> = Setting::new("args", "Arguments", "", "");
/// The output that the trigger event goes to (a row key of the outputs service).
pub const A_OUTPUT: Setting<field::Text> = Setting::new("output", "Output", "", "");
/// The row key of the new action.
pub const NEW_ROW: Field<field::Text> = Field::new("row", "Row", "");

/// Adds an action at the end of a rule.
pub const ACTION_ADD: Command = Command::new("action.add", "Add an action")
    .help("Adds an action at the end of a rule. The reply gives its row key.")
    .args(&[
        &A_RULE,
        &A_ACTION,
        &A_TITLE,
        &A_TEXT,
        &A_URGENT,
        &A_FREQ,
        &A_STACK,
        &A_WHAT,
        &A_SECONDS,
        &A_MAX_BYTES,
        &A_SCRIPT,
        &A_ARGS,
        &A_OUTPUT,
    ])
    .reply(&[&NEW_ROW]);
/// Removes an action.
pub const ACTION_REMOVE: Command = Command::new("action.remove", "Remove an action").args(&[&ROW]);

/// The shape of an area.
pub const F_SHAPE: Setting<field::Choice<Shape>> = Setting::choice("shape", "Shape", Shape::Circle);
/// The latitude of the centre of a circle, degrees.
pub const F_LAT: Setting<field::F64> =
    Setting::new("lat", "Latitude", "°", 0.0).range(-90.0, 90.0, 1e-6);
/// The longitude of the centre of a circle, degrees.
pub const F_LON: Setting<field::F64> =
    Setting::new("lon", "Longitude", "°", 0.0).range(-180.0, 180.0, 1e-6);
/// The radius of a circle.
pub const F_RADIUS_KM: Setting<field::F64> =
    Setting::new("radius_km", "Radius", "km", 1.0).range(0.0, 20_000.0, 0.001);
/// The corners of a polygon, `lat lon` pairs separated by `;`.
pub const F_CORNERS: Setting<field::Text> = Setting::new("corners", "Corners", "", "");
/// The families that the area watches, separated by commas. Empty watches every family.
pub const F_FAMILIES: Setting<field::Text> = Setting::new("families", "Families", "", "");

/// Adds a named area, or changes the area with this row key.
pub const FENCE_SET: Command = Command::new("fence.set", "Set an area").args(&[
    &ROW,
    &F_SHAPE,
    &F_LAT,
    &F_LON,
    &F_RADIUS_KM,
    &F_CORNERS,
    &F_FAMILIES,
]);
/// Removes a named area.
pub const FENCE_REMOVE: Command = Command::new("fence.remove", "Remove an area").args(&[&ROW]);

/// The period of an interval schedule. 0 makes a daily schedule.
pub const S_EVERY: Setting<field::Duration> =
    Setting::new("every", "Every", "s", 0.0).range(0.0, 1e7, 1.0);
/// The times of a daily schedule, `HH:MM` local time, separated by commas.
pub const S_TIMES: Setting<field::Text> = Setting::new("times", "Times", "", "");

/// Adds a schedule, or changes the schedule with this row key.
pub const SCHEDULE_SET: Command = Command::new("schedule.set", "Set a schedule").args(&[
    &ROW, &R_NAME, &R_ENABLED, &S_EVERY, &S_TIMES, &R_DAYS, &R_FROM, &R_TO,
]);
/// Removes a schedule.
pub const SCHEDULE_REMOVE: Command =
    Command::new("schedule.remove", "Remove a schedule").args(&[&ROW]);
/// Clears the log.
pub const LOG_CLEAR: Command = Command::new("log.clear", "Clear the log");

sdrkit_core::commands! {
    /// The commands of the automation.
    pub enum AutomationCommand {
        /// [`RULE_SET`].
        RuleSet = RULE_SET,
        /// [`RULE_REMOVE`].
        RuleRemove = RULE_REMOVE,
        /// [`ACTION_ADD`].
        ActionAdd = ACTION_ADD,
        /// [`ACTION_REMOVE`].
        ActionRemove = ACTION_REMOVE,
        /// [`FENCE_SET`].
        FenceSet = FENCE_SET,
        /// [`FENCE_REMOVE`].
        FenceRemove = FENCE_REMOVE,
        /// [`SCHEDULE_SET`].
        ScheduleSet = SCHEDULE_SET,
        /// [`SCHEDULE_REMOVE`].
        ScheduleRemove = SCHEDULE_REMOVE,
        /// [`LOG_CLEAR`].
        LogClear = LOG_CLEAR,
    }
}

/// The position of an action in its rule, from 1.
pub const POSITION: Field<field::I64> = Field::new("position", "Position", "");

/// The rules, in row key order.
pub const RULES: TableDef = TableDef {
    key: "rules",
    label: "Rules",
    columns: &[
        &R_NAME.field,
        &R_ENABLED.field,
        &R_KINDS.field,
        &R_FAMILIES.field,
        &R_URGENT_ONLY.field,
        &R_CONDITION.field,
        &R_DAYS.field,
        &R_FROM.field,
        &R_TO.field,
        &R_FENCE.field,
        &R_INSIDE.field,
        &R_MIN_KM.field,
        &R_MAX_KM.field,
        &R_COOLDOWN.field,
        &R_PER_IDENTITY.field,
        &R_MAX_PER_HOUR.field,
    ],
    cap: 10_000,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The actions of the rules. The row key is `<rule>/<n>`.
pub const RULE_ACTIONS: TableDef = TableDef {
    key: "rule_actions",
    label: "Actions",
    columns: &[
        &A_RULE.field,
        &POSITION,
        &A_ACTION.field,
        &A_TITLE.field,
        &A_TEXT.field,
        &A_URGENT.field,
        &A_FREQ.field,
        &A_STACK.field,
        &A_WHAT.field,
        &A_SECONDS.field,
        &A_MAX_BYTES.field,
        &A_SCRIPT.field,
        &A_ARGS.field,
        &A_OUTPUT.field,
    ],
    cap: 100_000,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The named areas.
pub const RULE_FENCES: TableDef = TableDef {
    key: "rule_fences",
    label: "Areas",
    columns: &[
        &F_SHAPE.field,
        &F_LAT.field,
        &F_LON.field,
        &F_RADIUS_KM.field,
        &F_CORNERS.field,
        &F_FAMILIES.field,
    ],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The schedules.
pub const RULE_SCHEDULES: TableDef = TableDef {
    key: "rule_schedules",
    label: "Schedules",
    columns: &[
        &R_NAME.field,
        &R_ENABLED.field,
        &S_EVERY.field,
        &S_TIMES.field,
        &R_DAYS.field,
        &R_FROM.field,
        &R_TO.field,
    ],
    cap: 10_000,
    marks: sdrkit_core::RowMarks::NONE,
};

sdrkit_core::choice_enum! {
    /// What is wrong with a row.
    pub enum Problem {
        /// The rule has no action.
        NoAction = ("no_action", "No action"),
        /// The rule names an area that does not exist.
        NoFence = ("no_fence", "No such area"),
        /// A name in the text does not resolve.
        UnknownName = ("unknown_name", "Unknown name"),
        /// The condition does not parse.
        Syntax = ("syntax", "Not a condition"),
        /// A time, a day or a corner does not parse.
        BadValue = ("bad_value", "Bad value"),
    }
}

/// The row with the problem (a rule, an action or a schedule).
pub const OF: Field<field::Text> = Field::new("of", "Row", "");
/// What is wrong.
pub const PROBLEM: Field<field::Choice<Problem>> = Field::new("problem", "Problem", "");
/// The argument whose text has the problem.
pub const ARG: Field<field::Text> = Field::new("arg", "Argument", "");
/// The byte position of the problem in that text.
pub const AT: Field<field::I64> = Field::new("at", "Position", "");

/// What is wrong with the rows. A row with a problem does not run. The row key is `<row>/<problem>`.
pub const RULE_PROBLEMS: TableDef = TableDef {
    key: "rule_problems",
    label: "Problems",
    columns: &[&OF, &PROBLEM, &ARG, &AT],
    cap: 10_000,
    marks: sdrkit_core::RowMarks::NONE,
};

/// When the row was logged.
pub const TIME: Field<field::Time> = Field::new("time", "Time", "");
/// The log of the `log` actions, newest last.
pub const LOG: TableDef = TableDef {
    key: "log",
    label: "Log",
    columns: &[&TIME, &RULE, &keys::TEXT],
    cap: 1000,
    marks: sdrkit_core::RowMarks::NONE,
};

#[derive(Debug, Default)]
struct Limits {
    last_any: Option<f64>,
    last_by_id: HashMap<String, f64>,
    hour: VecDeque<f64>,
}

/// The rows as the user set them, as kept in [`FILE`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Saved {
    rules: BTreeMap<String, RuleText>,
    actions: BTreeMap<String, ActionText>,
    fences: BTreeMap<String, FenceText>,
    schedules: BTreeMap<String, ScheduleText>,
}

/// One rule ready to run.
struct Ready {
    key: String,
    rule: Rule,
    fence: Option<usize>,
    actions: Vec<Action>,
}

/// The ids of the automation's own kinds in the engine's catalog.
#[derive(Clone, Copy, Debug, Default)]
struct Ids {
    notify: Option<EventKindId>,
    tune: Option<EventKindId>,
    record: Option<EventKindId>,
    rewind: Option<EventKindId>,
    script: Option<EventKindId>,
    send: Option<EventKindId>,
    enter: Option<EventKindId>,
    exit: Option<EventKindId>,
    cpa: Option<EventKindId>,
    due: Option<EventKindId>,
}

/// The automation: the owner `service.automation`.
pub struct Automation {
    kinds: Arc<Kinds>,
    storage: Arc<dyn Storage>,
    path: StorePath,
    values: Values,
    saved: Saved,
    ready: Vec<Ready>,
    fence_keys: Vec<String>,
    fences: Vec<NamedFence>,
    schedules: Vec<(String, Schedule)>,
    cpa: Option<Cpa>,
    limits: HashMap<String, Limits>,
    geo: GeoWatch,
    ids: Ids,
    last_tick: Option<f64>,
    log_seq: u64,
    scratch: Vec<GeoChange>,
    due: Vec<f64>,
    p: Published,
}

fn ids(kinds: &Kinds) -> Ids {
    let c = kinds.catalog();
    let id = |d: &EventDef| c.id_of(d);
    Ids {
        notify: id(&NOTIFY),
        tune: id(&TUNE),
        record: id(&RECORD),
        rewind: id(&REWIND),
        script: id(&RUN_SCRIPT),
        send: id(&SEND),
        enter: id(&GEO_ENTER),
        exit: id(&GEO_EXIT),
        cpa: id(&GEO_CPA),
        due: id(&SCHEDULE_DUE),
    }
}

fn problem_of(e: &AutomationError) -> Option<(Problem, &'static str, usize)> {
    let AutomationError::Text { arg, error } = e else {
        return None;
    };
    let p = match error {
        TextError::Name(_) | TextError::Expr(crate::expr::ExprError::Name(_)) => {
            Problem::UnknownName
        }
        TextError::Expr(_) => Problem::Syntax,
        TextError::Time { .. } | TextError::Day { .. } | TextError::Corner { .. } => {
            Problem::BadValue
        }
    };
    Some((p, arg, error.at()))
}

impl Automation {
    /// Opens the automation with the rows kept at [`FILE`] in `storage`. No file produces no rows. Rows whose text does not
    /// resolve (the event kinds changed) do not run and are listed in the table `rule_problems`.
    ///
    /// # Errors
    ///
    /// [`AutomationError::Storage`] if the file cannot be read. [`AutomationError::Json`] if it does not parse.
    pub fn open(kinds: Arc<Kinds>, storage: Arc<dyn Storage>) -> Result<Self, AutomationError> {
        let path = StorePath::new(FILE).map_err(|e| AutomationError::Storage {
            path: StorePath::root(),
            source: StorageError::InvalidPath(e),
        })?;
        let saved = match storage.read(&path) {
            Ok(data) => serde_json::from_slice(&data)?,
            Err(StorageError::NotFound) => Saved::default(),
            Err(source) => return Err(AutomationError::Storage { path, source }),
        };
        let decl = AUTOMATION_EVENTS
            .iter()
            .fold(ResultsDecl::new(), |d, e| d.event(e));
        let mut a = Automation {
            ids: ids(&kinds),
            kinds,
            storage,
            path,
            values: Values::defaults(&AutomationSetting::schema()),
            saved,
            ready: Vec::new(),
            fence_keys: Vec::new(),
            fences: Vec::new(),
            schedules: Vec::new(),
            cpa: None,
            limits: HashMap::new(),
            geo: GeoWatch::default(),
            last_tick: None,
            log_seq: 0,
            scratch: Vec::new(),
            due: Vec::new(),
            p: Published::from_decl(Arc::new(decl)).with_tables(&[
                &RULES,
                &RULE_ACTIONS,
                &RULE_FENCES,
                &RULE_SCHEDULES,
                &RULE_PROBLEMS,
                &LOG,
            ]),
        };
        a.rebuild();
        a.resolve_cpa();
        Ok(a)
    }

    fn save(&self) -> Result<(), AutomationError> {
        let data = serde_json::to_vec_pretty(&self.saved)?;
        self.storage
            .write(&self.path, &data)
            .map_err(|source| AutomationError::Storage {
                path: self.path.clone(),
                source,
            })
    }

    fn resolve_cpa(&mut self) {
        let v = &self.values;
        self.cpa = v.get(&CPA_ON).then(|| Cpa {
            max_km: v.get(&CPA_MAX_KM),
            max_tcpa_s: v.get(&CPA_MAX_TCPA),
            families: Kinds::family_list(v.get(&CPA_FAMILIES), 0).unwrap_or_default(),
            repeat_s: v.get(&CPA_REPEAT),
        });
    }

    /// Builds the forms that run from the saved text and writes every table and the problems.
    fn rebuild(&mut self) {
        let mut problems: Vec<(String, Problem, &'static str, usize)> = Vec::new();
        let note = |problems: &mut Vec<_>, row: &str, e: &AutomationError| {
            if let Some((p, arg, at)) = problem_of(e) {
                problems.push((row.to_owned(), p, arg, at));
            }
        };
        self.fences.clear();
        self.fence_keys.clear();
        for (key, t) in &self.saved.fences {
            let shape = choice_of_key::<Shape>(&t.shape).unwrap_or(Shape::Circle);
            match NamedFence::resolve(t.clone(), shape) {
                Ok(f) => {
                    self.fence_keys.push(key.clone());
                    self.fences.push(f);
                }
                Err(e) => note(&mut problems, key, &e),
            }
        }
        self.geo.reset();
        self.schedules.clear();
        for (key, t) in &self.saved.schedules {
            match Schedule::resolve(t.clone()) {
                Ok(s) => self.schedules.push((key.clone(), s)),
                Err(e) => note(&mut problems, key, &e),
            }
        }
        self.ready.clear();
        for (key, t) in &self.saved.rules {
            let rule = match Rule::resolve(t.clone(), &self.kinds) {
                Ok(r) => r,
                Err(e) => {
                    note(&mut problems, key, &e);
                    continue;
                }
            };
            let fence = if t.fence.trim().is_empty() {
                None
            } else {
                match self.fence_keys.iter().position(|k| k == t.fence.trim()) {
                    Some(i) => Some(i),
                    None => {
                        problems.push((key.clone(), Problem::NoFence, "fence", 0));
                        continue;
                    }
                }
            };
            let prefix = format!("{key}/");
            let mut actions = Vec::new();
            for (akey, at) in self.saved.actions.range(prefix.clone()..) {
                if !akey.starts_with(&prefix) {
                    break;
                }
                let kind = choice_of_key(&at.action).unwrap_or(ActionKind::Log);
                let what = choice_of_key(&at.what).unwrap_or(RecordWhat::Audio);
                match Action::resolve(at.clone(), kind, what, &self.kinds, rule.kinds.as_ref()) {
                    Ok(a) => actions.push(a),
                    Err(e) => note(&mut problems, akey, &e),
                }
            }
            if actions.is_empty() {
                problems.push((key.clone(), Problem::NoAction, "", 0));
                continue;
            }
            self.ready.push(Ready {
                key: key.clone(),
                rule,
                fence,
                actions,
            });
        }
        self.limits.retain(|k, _| self.saved.rules.contains_key(k));
        self.write_tables(&problems);
    }

    fn write_tables(&mut self, problems: &[(String, Problem, &'static str, usize)]) {
        for t in [
            &RULES,
            &RULE_ACTIONS,
            &RULE_FENCES,
            &RULE_SCHEDULES,
            &RULE_PROBLEMS,
        ] {
            let id = table_id(&self.p, t);
            let keys: Vec<String> = self
                .p
                .results()
                .table(id)
                .map(|tb| tb.rows_newest_first().map(|(k, _)| k.to_owned()).collect())
                .unwrap_or_default();
            let keep = |k: &str| match t.key {
                "rules" => self.saved.rules.contains_key(k),
                "rule_actions" => self.saved.actions.contains_key(k),
                "rule_fences" => self.saved.fences.contains_key(k),
                "rule_schedules" => self.saved.schedules.contains_key(k),
                _ => problems.iter().any(|p| problem_key(p) == k),
            };
            let gone: Vec<String> = keys.into_iter().filter(|k| !keep(k)).collect();
            for k in gone {
                self.p.remove_row(t, &k);
            }
        }
        for (key, t) in &self.saved.rules {
            let Some(mut r) = self.p.row_mut(&RULES, key) else {
                continue;
            };
            r.set(&R_NAME.field, &t.name);
            r.set(&R_ENABLED.field, t.enabled);
            r.set(&R_KINDS.field, &t.kinds);
            r.set(&R_FAMILIES.field, &t.families);
            r.set(&R_URGENT_ONLY.field, t.urgent_only);
            r.set(&R_CONDITION.field, &t.condition);
            r.set(&R_DAYS.field, &t.days);
            r.set(&R_FROM.field, &t.from);
            r.set(&R_TO.field, &t.to);
            r.set(&R_FENCE.field, &t.fence);
            r.set(&R_INSIDE.field, t.inside);
            r.set(&R_MIN_KM.field, t.min_km);
            r.set(&R_MAX_KM.field, t.max_km);
            r.set(&R_COOLDOWN.field, t.cooldown_s);
            r.set(&R_PER_IDENTITY.field, t.per_identity);
            r.set(&R_MAX_PER_HOUR.field, t.max_per_hour);
        }
        let mut n = 0;
        let mut last_rule = "";
        for (key, t) in &self.saved.actions {
            if t.rule != last_rule {
                n = 0;
                last_rule = &t.rule;
            }
            n += 1;
            let Some(mut r) = self.p.row_mut(&RULE_ACTIONS, key) else {
                continue;
            };
            r.set(&A_RULE.field, &t.rule);
            r.set(&POSITION, n);
            let kind = choice_of_key(&t.action).unwrap_or(ActionKind::Log);
            r.set(&A_ACTION.field, kind);
            r.set(&A_TITLE.field, &t.title);
            r.set(&A_TEXT.field, &t.text);
            r.set(&A_URGENT.field, t.urgent);
            r.set(&A_FREQ.field, &t.freq);
            r.set(&A_STACK.field, &t.stack);
            let what = choice_of_key(&t.what).unwrap_or(RecordWhat::Audio);
            r.set(&A_WHAT.field, what);
            r.set(&A_SECONDS.field, t.seconds);
            r.set(&A_MAX_BYTES.field, t.max_bytes);
            r.set(&A_SCRIPT.field, &t.script);
            r.set(&A_ARGS.field, &t.args);
            r.set(&A_OUTPUT.field, &t.output);
        }
        for (key, t) in &self.saved.fences {
            let Some(mut r) = self.p.row_mut(&RULE_FENCES, key) else {
                continue;
            };
            r.set(
                &F_SHAPE.field,
                choice_of_key(&t.shape).unwrap_or(Shape::Circle),
            );
            r.set(&F_LAT.field, t.lat);
            r.set(&F_LON.field, t.lon);
            r.set(&F_RADIUS_KM.field, t.radius_km);
            r.set(&F_CORNERS.field, &t.corners);
            r.set(&F_FAMILIES.field, &t.families);
        }
        for (key, t) in &self.saved.schedules {
            let Some(mut r) = self.p.row_mut(&RULE_SCHEDULES, key) else {
                continue;
            };
            r.set(&R_NAME.field, &t.name);
            r.set(&R_ENABLED.field, t.enabled);
            r.set(&S_EVERY.field, t.every_s);
            r.set(&S_TIMES.field, &t.times);
            r.set(&R_DAYS.field, &t.days);
            r.set(&R_FROM.field, &t.from);
            r.set(&R_TO.field, &t.to);
        }
        for p in problems {
            let Some(mut r) = self.p.row_mut(&RULE_PROBLEMS, &problem_key(p)) else {
                continue;
            };
            r.set(&OF, &p.0);
            r.set(&PROBLEM, p.1);
            r.set(&ARG, p.2);
            r.set(&AT, i64::try_from(p.3).unwrap_or(i64::MAX));
        }
    }

    /// Takes an observation: the rules on it, then the crossings of the areas and the closest approach. `ctx` is the context
    /// that the engine supplied the observation, and `offset_min` the host's offset from UTC, minutes, for the windows. The
    /// events go to the end of `out`. The engine brings the crossings back like any other event, and the rules take them
    /// then.
    ///
    /// If no area, warning or rule takes the observation, the call allocates nothing.
    pub fn handle(
        &mut self,
        o: &Observation<'_>,
        ctx: &Context,
        offset_min: i32,
        out: &mut Vec<Event>,
    ) {
        let i = self.ids;
        let requests = [i.notify, i.tune, i.record, i.rewind, i.script, i.send];
        if requests.contains(&Some(o.kind)) {
            return;
        }
        let own_geo = [self.ids.enter, self.ids.exit, self.ids.cpa].contains(&Some(o.kind));
        let mut changes = std::mem::take(&mut self.scratch);
        if !own_geo {
            self.geo.observe(
                o,
                &self.fences,
                self.cpa.as_ref(),
                ctx.receiver.as_ref(),
                &mut changes,
            );
        }
        self.run_rules(o, offset_min, out);
        for c in changes.drain(..) {
            if let Some(ev) = self.geo_event(o, c) {
                out.push(ev);
            }
        }
        self.scratch = changes;
    }

    fn event(&self, id: Option<EventKindId>, def: &'static EventDef, t: f64, f: f64) -> Event {
        id.and_then(|id| self.kinds.catalog().event(id, t, f))
            .unwrap_or_else(|| def.event(t, f))
    }

    fn geo_event(&self, o: &Observation<'_>, c: GeoChange) -> Option<Event> {
        let (id, def, fence) = match c {
            GeoChange::Enter(i) => (self.ids.enter, &GEO_ENTER, Some(i)),
            GeoChange::Exit(i) => (self.ids.exit, &GEO_EXIT, Some(i)),
            GeoChange::Approach { .. } => (self.ids.cpa, &GEO_CPA, None),
        };
        let p = o.position?;
        let mut e = self
            .event(id, def, o.event.time, o.event.freq_hz)
            .opt(&keys::SOURCE, o.identity)
            .set(&TRIGGER, o.event.kind())
            .set(&keys::POSITION, (p.lat_deg, p.lon_deg));
        if let Some(i) = fence {
            e = e.set(&FENCE, self.fence_keys.get(i).map_or("", String::as_str));
        }
        if let GeoChange::Approach { cpa_km, tcpa_s } = c {
            e = e.set(&CPA_KM, cpa_km).set(&TCPA, tcpa_s);
        }
        Some(e)
    }

    /// Advances the clock to `now`, Unix seconds: every schedule due since the last tick returns a `schedule.due` event. The
    /// engine brings them back like any other event, and the rules take them then. `offset_min` is the host's offset from UTC,
    /// minutes. The first tick only starts the clock.
    pub fn tick(&mut self, now: f64, offset_min: i32, out: &mut Vec<Event>) {
        let Some(last) = self.last_tick.replace(now) else {
            return;
        };
        let mut due = std::mem::take(&mut self.due);
        let mut events = Vec::new();
        for (key, s) in &self.schedules {
            due.clear();
            s.due(last, now, offset_min, &mut due);
            for t in &due {
                events.push((
                    *t,
                    self.event(self.ids.due, &SCHEDULE_DUE, *t, 0.0)
                        .set(&SCHEDULE, key)
                        .set(&NAME, &s.text.name),
                ));
            }
        }
        self.due = due;
        events.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.extend(events.into_iter().map(|(_, e)| e));
    }

    fn run_rules(&mut self, o: &Observation<'_>, offset_min: i32, out: &mut Vec<Event>) {
        for i in 0..self.ready.len() {
            let r = &self.ready[i];
            if !r.rule.triggers(o) || !r.rule.admits(o, offset_min) {
                continue;
            }
            if let Some(f) = r.fence {
                let inside = o
                    .position
                    .zip(self.fences.get(f))
                    .map(|(p, nf)| nf.fence.contains(&p));
                if inside != Some(r.rule.text.inside) {
                    continue;
                }
            }
            if !r.rule.condition.holds(o) {
                continue;
            }
            if !self.allow(i, o) {
                continue;
            }
            self.fire(i, o, out);
        }
    }

    /// True if the limits of rule `i` let it run now. Counts the run.
    fn allow(&mut self, i: usize, o: &Observation<'_>) -> bool {
        let r = &self.ready[i];
        let t = &r.rule.text;
        let st = self.limits.entry(r.key.clone()).or_default();
        let now = o.unix_time;
        let last = match (t.per_identity, o.identity) {
            (true, Some(id)) => st.last_by_id.get(id).copied(),
            _ => st.last_any,
        };
        if t.cooldown_s > 0.0 && last.is_some_and(|l| now - l < t.cooldown_s) {
            return false;
        }
        while st.hour.front().is_some_and(|h| now - h >= 3600.0) {
            st.hour.pop_front();
        }
        if t.max_per_hour > 0 && st.hour.len() as i64 >= t.max_per_hour {
            return false;
        }
        st.hour.push_back(now);
        st.last_any = Some(now);
        if let Some(id) = o.identity {
            st.last_by_id.insert(id.to_owned(), now);
            if st.last_by_id.len() > 50_000 {
                let keep = t.cooldown_s.max(3600.0);
                st.last_by_id.retain(|_, l| now - *l < keep);
            }
        }
        true
    }

    fn fire(&mut self, i: usize, o: &Observation<'_>, out: &mut Vec<Event>) {
        let r = &self.ready[i];
        let key = r.key.as_str();
        let t = o.event.time;
        let mut logs = Vec::new();
        for a in &r.actions {
            let base = |id, def: &'static EventDef, f: f64| {
                self.event(id, def, t, f)
                    .set(&RULE, key)
                    .set(&TRIGGER, o.event.kind())
            };
            let ev = match a.kind {
                ActionKind::Notify => {
                    let urgency = if a.text.urgent && o.urgency == Urgency::None {
                        Urgency::Urgent
                    } else {
                        o.urgency
                    };
                    base(self.ids.notify, &NOTIFY, o.event.freq_hz)
                        .opt(&keys::SOURCE, o.identity)
                        .set(&TITLE, &a.title.fill(o))
                        .set(&keys::TEXT, &a.body.fill(o))
                        .set(&keys::URGENCY, urgency)
                }
                ActionKind::Log => {
                    logs.push(a.body.fill(o));
                    continue;
                }
                ActionKind::Tune => {
                    let Some(f) = a
                        .freq
                        .fill(o)
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|f| f.is_finite() && *f > 0.0)
                    else {
                        continue;
                    };
                    base(self.ids.tune, &TUNE, f)
                        .opt(&keys::SOURCE, o.identity)
                        .set(&STACK, &a.text.stack)
                }
                ActionKind::Record => base(self.ids.record, &RECORD, o.event.freq_hz)
                    .opt(&keys::SOURCE, o.identity)
                    .set(&WHAT, a.what)
                    .set(&SECONDS, a.text.seconds)
                    .set(&MAX_BYTES, a.text.max_bytes),
                ActionKind::Rewind => {
                    base(self.ids.rewind, &REWIND, o.event.freq_hz).set(&SECONDS, a.text.seconds)
                }
                ActionKind::Script => base(self.ids.script, &RUN_SCRIPT, o.event.freq_hz)
                    .opt(&keys::SOURCE, o.identity)
                    .set(&SCRIPT, &a.text.script)
                    .set(&ARGS, &a.args.fill(o)),
                ActionKind::Output => base(self.ids.send, &SEND, o.event.freq_hz)
                    .opt(&keys::SOURCE, o.identity)
                    .set(&OUTPUT, &a.text.output)
                    .set(&keys::TEXT, &a.body.fill(o)),
            };
            out.push(ev);
        }
        let rule = r.key.clone();
        for text in logs {
            self.log_seq += 1;
            if let Some(mut row) = self.p.row_mut(&LOG, &self.log_seq.to_string()) {
                row.set(&TIME, o.unix_time);
                row.set(&RULE, &rule);
                row.set(&keys::TEXT, &text);
            }
        }
    }

    fn rule_text(args: &Values) -> RuleText {
        RuleText {
            name: args.get(&R_NAME).trim().to_owned(),
            enabled: args.get(&R_ENABLED),
            kinds: args.get(&R_KINDS).to_owned(),
            families: args.get(&R_FAMILIES).to_owned(),
            urgent_only: args.get(&R_URGENT_ONLY),
            condition: args.get(&R_CONDITION).to_owned(),
            days: args.get(&R_DAYS).to_owned(),
            from: args.get(&R_FROM).to_owned(),
            to: args.get(&R_TO).to_owned(),
            fence: args.get(&R_FENCE).trim().to_owned(),
            inside: args.get(&R_INSIDE),
            min_km: args.get(&R_MIN_KM),
            max_km: args.get(&R_MAX_KM),
            cooldown_s: args.get(&R_COOLDOWN),
            per_identity: args.get(&R_PER_IDENTITY),
            max_per_hour: args.get(&R_MAX_PER_HOUR),
        }
    }

    fn commit(&mut self) -> Result<(), CommandError> {
        self.rebuild();
        self.save().map_err(failed)
    }

    fn row_key(args: &Values) -> Result<String, CommandError> {
        let row = args.arg(&ROW)?.trim();
        if row.is_empty() || row.contains('/') {
            let arg = args
                .schema()
                .id(ROW.field.key)
                .map_err(|_| CommandError::NotAnArgument { key: ROW.field.key })?;
            return Err(CommandError::BadArgument {
                arg,
                error: SettingError::Malformed { id: arg, at: None },
            });
        }
        Ok(row.to_owned())
    }
}

/// The key of a problem row.
fn problem_key(p: &(String, Problem, &'static str, usize)) -> String {
    format!("{}/{}", p.0, choice_key(p.1))
}

impl Owner for Automation {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = AutomationSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        if let (AutomationSetting::CpaFamilies, Value::Text(t)) = (s, value) {
            Kinds::family_list(t, 0).map_err(|e| SettingError::Malformed {
                id,
                at: Some(match e {
                    NameError::UnknownFamily { at, .. }
                    | NameError::UnknownField { at, .. }
                    | NameError::UnknownKind { at, .. } => at,
                }),
            })?;
        }
        self.values.set(s.id(), value)?;
        self.resolve_cpa();
        Ok(())
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
        AutomationCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match AutomationCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AutomationCommand::RuleSet => {
                let key = Self::row_key(args)?;
                let text = Self::rule_text(args);
                Rule::resolve(text.clone(), &self.kinds).map_err(failed)?;
                self.saved.rules.insert(key, text);
            }
            AutomationCommand::RuleRemove => {
                let key = args.arg(&ROW)?.trim();
                if self.saved.rules.remove(key).is_none() {
                    return Err(no_such_row(&self.p, &RULES, key));
                }
                let prefix = format!("{key}/");
                self.saved.actions.retain(|k, _| !k.starts_with(&prefix));
            }
            AutomationCommand::ActionAdd => {
                let rule_key = args.arg(&A_RULE)?.trim();
                let Some(rule) = self.saved.rules.get(rule_key) else {
                    return Err(no_such_row(&self.p, &RULES, rule_key));
                };
                let within = Rule::resolve(rule.clone(), &self.kinds)
                    .map_err(failed)?
                    .kinds;
                let kind = args.get(&A_ACTION);
                let what = args.get(&A_WHAT);
                let text = ActionText {
                    rule: rule_key.to_owned(),
                    action: choice_key(kind).to_owned(),
                    title: args.get(&A_TITLE).to_owned(),
                    text: args.get(&A_TEXT).to_owned(),
                    urgent: args.get(&A_URGENT),
                    freq: args.get(&A_FREQ).to_owned(),
                    stack: args.get(&A_STACK).trim().to_owned(),
                    what: choice_key(what).to_owned(),
                    seconds: args.get(&A_SECONDS),
                    max_bytes: args.get(&A_MAX_BYTES),
                    script: args.get(&A_SCRIPT).trim().to_owned(),
                    args: args.get(&A_ARGS).to_owned(),
                    output: args.get(&A_OUTPUT).trim().to_owned(),
                };
                Action::resolve(text.clone(), kind, what, &self.kinds, within.as_ref())
                    .map_err(failed)?;
                let prefix = format!("{rule_key}/");
                let next = self
                    .saved
                    .actions
                    .range(prefix.clone()..)
                    .take_while(|(k, _)| k.starts_with(&prefix))
                    .filter_map(|(k, _)| k[prefix.len()..].parse::<u32>().ok())
                    .max()
                    .map_or(1, |n| n + 1);
                let row = format!("{prefix}{next:04}");
                self.saved.actions.insert(row.clone(), text);
                self.commit()?;
                let mut reply = reply(AutomationCommand::commands(), id)?;
                reply.set(&NEW_ROW, &row);
                return Ok(Some(reply));
            }
            AutomationCommand::ActionRemove => {
                let key = args.arg(&ROW)?.trim();
                if self.saved.actions.remove(key).is_none() {
                    return Err(no_such_row(&self.p, &RULE_ACTIONS, key));
                }
            }
            AutomationCommand::FenceSet => {
                let key = Self::row_key(args)?;
                let shape = args.get(&F_SHAPE);
                let text = FenceText {
                    shape: choice_key(shape).to_owned(),
                    lat: args.get(&F_LAT),
                    lon: args.get(&F_LON),
                    radius_km: args.get(&F_RADIUS_KM),
                    corners: args.get(&F_CORNERS).to_owned(),
                    families: args.get(&F_FAMILIES).to_owned(),
                };
                NamedFence::resolve(text.clone(), shape).map_err(failed)?;
                if !self.saved.fences.contains_key(&key) && self.saved.fences.len() >= MAX_FENCES {
                    return Err(failed(AutomationError::TooManyFences { cap: MAX_FENCES }));
                }
                self.saved.fences.insert(key, text);
            }
            AutomationCommand::FenceRemove => {
                let key = args.arg(&ROW)?.trim();
                if self.saved.fences.remove(key).is_none() {
                    return Err(no_such_row(&self.p, &RULE_FENCES, key));
                }
            }
            AutomationCommand::ScheduleSet => {
                let key = Self::row_key(args)?;
                let text = ScheduleText {
                    name: args.get(&R_NAME).trim().to_owned(),
                    enabled: args.get(&R_ENABLED),
                    every_s: args.get(&S_EVERY),
                    times: args.get(&S_TIMES).to_owned(),
                    days: args.get(&R_DAYS).to_owned(),
                    from: args.get(&R_FROM).to_owned(),
                    to: args.get(&R_TO).to_owned(),
                };
                Schedule::resolve(text.clone()).map_err(failed)?;
                self.saved.schedules.insert(key, text);
            }
            AutomationCommand::ScheduleRemove => {
                let key = args.arg(&ROW)?.trim();
                if self.saved.schedules.remove(key).is_none() {
                    return Err(no_such_row(&self.p, &RULE_SCHEDULES, key));
                }
            }
            AutomationCommand::LogClear => {
                self.p.clear_rows(&LOG);
                return Ok(None);
            }
        }
        self.commit()?;
        Ok(None)
    }
}

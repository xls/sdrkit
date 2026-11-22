//! The engine as an owner: the station as its settings, the catalogs, the requests and the
//! benchmarks as its tables, and the commands that save and restore the state of every owner.

use crate::backends;
pub use crate::backends::{
    BACKEND_FFT, BACKEND_FIR, BACKEND_LDPC, BACKEND_RESAMPLE, BACKEND_RS, BACKEND_VITERBI,
};
use crate::catalog;
use crate::feed::lock;
use crate::request::{RequestKind, REQUEST, REQUESTS};
use crate::session as s;
use crate::Shared;
use sdrkit_core::command::CommandSet;
use sdrkit_core::publish::{field, keys};
use sdrkit_core::settings::Argument;
use sdrkit_core::{
    Command, CommandError, CommandId, Commands, Event, Field, Owner, Published, Record, Results,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, Status, TableDef, Value,
    Values,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

/// The callsign of the station.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Callsign", "", "");
/// The Maidenhead locator of the station.
pub const LOCATOR: Setting<field::Text> = Setting::new("locator", "Locator", "", "");
/// The position of the station. Latitude and longitude 0 mean that the position is not known.
pub const POSITION: Setting<field::Position> = Setting::new("position", "Position", "", (0.0, 0.0));
/// The height of the antenna above sea level.
pub const HEIGHT_M: Setting<field::F64> =
    Setting::new("height_m", "Height", "m", 0.0).range(-500.0, 10_000.0, 0.0);
/// The offset of local time from UTC, minutes east.
pub const TIME_ZONE: Setting<field::I64> =
    Setting::new("time_zone", "Time zone", "min", 0).range(-720.0, 840.0, 1.0);
/// The engine carries out the requests of receivers and services. Off refuses each one.
pub const REQUESTS_ALLOWED: Setting<field::Bool> =
    Setting::new("requests_allowed", "Requests", "", true)
        .help("If off, the engine refuses every request of a receiver or a service.");
/// The transmit switch, the only one. On: each session of a stack that transmits, on a radio that can transmit, sends,
/// and the engine puts its bursts on air. Off: no stack sends and the engine refuses each burst.
pub const TRANSMIT_ALLOWED: Setting<field::Bool> =
    Setting::new("transmit_allowed", "Transmit", "", false).help(
        "If on, a stack that transmits sends through the radio of its session, if that radio can transmit. \
         If off, nothing is sent and each own message fails. No stack and no radio has a switch of its own.",
    );

/// The processor cores that the sessions may use. The engine gives each stage of a heavy stack a thread of its own as far
/// as cores are free, and sizes the pool for loops inside a component by it.
pub const CORES_MAX: Setting<field::I64> = Setting::new("cores_max", "Cores", "", 0)
    .range(0.0, 256.0, 1.0)
    .zero_means("The performance cores of the machine")
    .help("The most processor cores that the sessions use. A session whose stack needs more than is free still starts, and its results say so.");

sdrkit_core::settings! {
    /// The settings of the engine owner.
    pub enum EngineSetting {
        /// [`CALLSIGN`].
        Callsign = CALLSIGN,
        /// [`LOCATOR`].
        Locator = LOCATOR,
        /// [`POSITION`].
        Position = POSITION,
        /// [`HEIGHT_M`].
        HeightM = HEIGHT_M,
        /// [`TIME_ZONE`].
        TimeZone = TIME_ZONE,
        /// [`REQUESTS_ALLOWED`].
        RequestsAllowed = REQUESTS_ALLOWED,
        /// [`TRANSMIT_ALLOWED`].
        TransmitAllowed = TRANSMIT_ALLOWED,
        /// [`BACKEND_FFT`].
        BackendFft = BACKEND_FFT,
        /// [`BACKEND_FIR`].
        BackendFir = BACKEND_FIR,
        /// [`BACKEND_RESAMPLE`].
        BackendResample = BACKEND_RESAMPLE,
        /// [`BACKEND_VITERBI`].
        BackendViterbi = BACKEND_VITERBI,
        /// [`BACKEND_RS`].
        BackendRs = BACKEND_RS,
        /// [`BACKEND_LDPC`].
        BackendLdpc = BACKEND_LDPC,
        /// [`CORES_MAX`].
        CoresMax = CORES_MAX,
    }
}

/// Items a benchmark handled per second of processor time.
pub const ITEMS_PER_S: Field<field::F64> = Field::new("items_per_s", "Items per second", "1/s");
/// The share of one processor core that a benchmark needs per second of signal.
pub const CPU_SHARE: Field<field::F64> = Field::new("cpu_share", "Processor share", "");

/// The calibration units per second that one fast core of this device runs. Absent until the calibration has finished.
pub const DEVICE_UNITS: Field<field::F64> =
    Field::new("engine.core_units", "Units per fast core", "");
/// The same of one slow core: equal to the fast core where the platform has one class of cores.
pub const DEVICE_SLOW_UNITS: Field<field::F64> =
    Field::new("engine.core_units_slow", "Units per slow core", "");
/// The fast cores and the slow cores of this device. The slow cores are 0 where the platform has one class of cores.
pub const DEVICE_FAST_CORES: Field<field::I64> = Field::new("engine.cores_fast", "Fast cores", "");
pub const DEVICE_SLOW_CORES: Field<field::I64> = Field::new("engine.cores_slow", "Slow cores", "");
/// The cores that the sessions may use: the setting `cores_max`, or the fast cores when it is 0.
pub const CORES_USABLE: Field<field::I64> = Field::new("engine.cores_usable", "Cores usable", "");

/// The result fields of the engine.
pub(crate) const ENGINE_FIELDS: [&dyn sdrkit_core::Declare; 5] = [
    &DEVICE_UNITS,
    &DEVICE_SLOW_UNITS,
    &DEVICE_FAST_CORES,
    &DEVICE_SLOW_CORES,
    &CORES_USABLE,
];

/// The threads that a session would give the stack of a benchmark by its measured load.
pub const BENCH_THREADS: Field<field::I64> = Field::new("threads", "Threads", "");
/// The cores that the fullest of those threads needs at the rate of the stack.
pub const BENCH_THREAD_CORES: Field<field::F64> = Field::new("thread_cores", "Fullest thread", "");
/// `ok`, `FAIL` with the thread that needs more than one core, or `no graph` for a case that ran no graph.
pub const BENCH_STATUS: Field<field::Text> = Field::new("status", "Status", "");
/// The name of the benchmark of a row of a load table.
pub const BENCH_NAME: Field<field::Text> = Field::new("benchmark", "Benchmark", "");
/// The component of a row of the load table of a benchmark.
pub const BENCH_COMPONENT: Field<field::Text> = Field::new("component", "Component", "");

/// The results of the last benchmark run. The row key is the benchmark name.
pub static BENCHMARKS: TableDef = TableDef {
    key: "benchmarks",
    label: "Benchmarks",
    columns: &[
        &catalog::STACK,
        &ITEMS_PER_S,
        &CPU_SHARE,
        &BENCH_THREADS,
        &BENCH_THREAD_CORES,
        &BENCH_STATUS,
    ],
    cap: 4096,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The components of the stack of each benchmark, as `session.components` shows them for a session. The row key is
/// `<benchmark>/<component>` or `<benchmark>/<component>/<section>`. `cost` is the cores at the rate of the stack.
pub static BENCH_COMPONENTS: TableDef = TableDef {
    key: "benchmark.components",
    label: "Benchmark components",
    columns: &[
        &BENCH_NAME,
        &BENCH_COMPONENT,
        &s::LOAD_SECTION,
        &s::LOAD_STAGE,
        &s::LOAD_THREAD,
        &s::LOAD_COST,
        &s::LOAD_CALLS,
        &s::LOAD_ITEMS_IN,
        &s::LOAD_ITEMS_OUT,
        &s::LOAD_MEAN_US,
        &s::LOAD_WORST_US,
        &s::LOAD_UNITS,
    ],
    cap: 16384,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The stages of the stack of each benchmark. The row key is `<benchmark>/<stage>`.
pub static BENCH_STAGES: TableDef = TableDef {
    key: "benchmark.stages",
    label: "Benchmark stages",
    columns: &[
        &BENCH_NAME,
        &s::LOAD_STAGE,
        &s::LOAD_THREAD,
        &s::LOAD_COST,
        &s::LOAD_UNITS,
        &s::LOAD_DECLARED_UNITS,
    ],
    cap: 4096,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The connections of the stack of each benchmark. The row key is `<benchmark>/<from>.<output>><to>.<input>`. The
/// rates are per second of signal.
pub static BENCH_CONNECTIONS: TableDef = TableDef {
    key: "benchmark.connections",
    label: "Benchmark connections",
    columns: &[
        &BENCH_NAME,
        &s::CONN_FROM,
        &s::CONN_OUTPUT,
        &s::CONN_TO,
        &s::CONN_INPUT,
        &s::CONN_FORMAT,
        &s::LOAD_STAGE,
        &s::CONN_ITEMS,
        &s::CONN_BYTES,
        &s::CONN_HELD_MEAN,
        &s::CONN_HELD_MAX,
        &s::CONN_CAPACITY,
        &s::CONN_WAIT_MEAN,
        &s::CONN_WAIT_WORST,
        &s::CONN_FULL,
        &s::CONN_EMPTY,
        &s::CONN_BOUNDARY,
    ],
    cap: 16384,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The state of every owner, as the bytes of a state file.
const STATE: Field<field::Bytes> = Field::new("state", "State", "");
const A_STATE: Argument<field::Bytes> =
    Argument::new(STATE).help("The bytes that `state.save` returned.");
const SECONDS: Setting<field::F64> =
    Setting::new("seconds", "Seconds", "s", 0.2).range(0.01, 10.0, 0.0);

const ONLY: Setting<field::Text> = Setting::new("only", "Only", "", "")
    .help("Runs only the benchmarks whose name or stack id contains this text. Empty runs all.");

pub(crate) const C_SAVE: Command = Command::new("state.save", "Save state")
    .help("Returns the saved state of every radio, session and service that has one.")
    .reply(&[&STATE]);
pub(crate) const C_RESTORE: Command = Command::new("state.restore", "Restore state")
    .help("Restores the saved state of each owner named in the bytes of `state.save`. Every owner must be there.")
    .args(&[&A_STATE]);
pub(crate) const C_BENCH: Command = Command::new("benchmark.run", "Run benchmarks")
    .help("Runs the benchmarks of every stack, or those that `only` selects, on a thread of its own, each for about `seconds`. The table `benchmarks` shows the results.")
    .args(&[&SECONDS, &ONLY]);
sdrkit_core::commands! {
    /// The commands of the engine owner.
    pub enum EngineCommand {
        /// Returns the saved state of every owner.
        StateSave = C_SAVE,
        /// Restores the saved state of every owner.
        StateRestore = C_RESTORE,
        /// Runs the benchmarks.
        BenchmarkRun = C_BENCH,
    }
}

/// The events the engine owner keeps for the host. The oldest go first.
const EVENT_CAP: usize = 256;

/// The engine as an owner.
pub(crate) struct EngineOwner {
    pub(crate) results: Published,
    values: Values,
    commands: Commands,
    /// Shared with the engine, so that the host takes the events without the lock of the owner.
    events: Arc<std::sync::Mutex<VecDeque<Event>>>,
    shared: Weak<Shared>,
    requests: u64,
    /// The value of [`TRANSMIT_ALLOWED`], read by the session threads.
    transmit_allowed: Arc<AtomicBool>,
    /// Follows [`CORES_MAX`].
    cores: Arc<crate::stages::Cores>,
}

impl EngineOwner {
    /// The engine owner with the catalogs of `sources`. `shared` reaches the other owners for the state commands.
    /// `transmit_allowed` follows the setting [`TRANSMIT_ALLOWED`].
    pub(crate) fn new(
        sources: &catalog::Sources<'_>,
        shared: Weak<Shared>,
        transmit_allowed: Arc<AtomicBool>,
        cores: Arc<crate::stages::Cores>,
        events: Arc<std::sync::Mutex<VecDeque<Event>>>,
    ) -> Self {
        let decl = catalog::declare(ResultsDecl::new())
            .with_fields(&ENGINE_FIELDS)
            .with_table(&REQUESTS)
            .with_table(&BENCHMARKS)
            .with_table(&BENCH_COMPONENTS)
            .with_table(&BENCH_STAGES)
            .with_table(&BENCH_CONNECTIONS)
            .with_table(&catalog::STACK_TRANSMITS);
        let mut p = Published::from_decl(Arc::new(decl.event(&REQUEST)));
        catalog::fill(&mut p, sources);
        let values = Values::defaults(&EngineSetting::schema());
        let mut e = EngineOwner {
            results: p,
            values,
            commands: EngineCommand::commands().clone(),
            events,
            shared,
            requests: 0,
            transmit_allowed,
            cores,
        };
        e.show_backends();
        e
    }

    /// Writes the backend that runs in each kernel family into the status.
    fn show_backends(&mut self) {
        let status = Status {
            summary: backends::status(),
            ..Status::default()
        };
        self.results.results_mut().set_status(&status);
    }

    /// True if the engine carries out requests.
    pub(crate) fn requests_allowed(&self) -> bool {
        self.values.get(&REQUESTS_ALLOWED)
    }

    /// The offset of local time from UTC, minutes.
    pub(crate) fn offset_min(&self) -> i32 {
        i32::try_from(self.values.get(&TIME_ZONE)).unwrap_or(0)
    }

    /// The position of the station, if it is known.
    pub(crate) fn position(&self) -> Option<sdrkit_svc_events::Position> {
        let (lat, lon) = self.values.get(&POSITION);
        (lat != 0.0 || lon != 0.0)
            .then(|| sdrkit_svc_events::Position::with_alt(lat, lon, self.values.get(&HEIGHT_M)))
    }

    /// The station that every receiver gets: the position, height, callsign, locator and offset from UTC.
    pub(crate) fn station(&self) -> sdrkit_core::Station {
        sdrkit_core::Station {
            position: self.position(),
            height_m: Some(self.values.get(&HEIGHT_M)),
            callsign: self.values.get(&CALLSIGN).to_owned(),
            locator: self.values.get(&LOCATOR).to_owned(),
            utc_offset_s: self.offset_min() * 60,
            ..sdrkit_core::Station::default()
        }
    }

    /// Writes a request of `from` as a row and an event, at wall time `now`.
    pub(crate) fn note_request(
        &mut self,
        now: f64,
        from: &str,
        kind: RequestKind,
        freq_hz: Option<f64>,
        refused: Option<&str>,
        session: Option<&str>,
    ) {
        use crate::request::{DONE, FREQ_HZ, FROM, KIND, REFUSED, SESSION};
        self.requests += 1;
        if let Some(mut r) = self.results.row_mut(&REQUESTS, &self.requests.to_string()) {
            r.set(&FROM, from);
            r.set(&KIND, kind);
            r.opt(&FREQ_HZ, freq_hz);
            r.set(&DONE, refused.is_none());
            r.opt(&REFUSED, refused);
            r.opt(&SESSION, session);
        }
        let mut e = REQUEST
            .event(now, freq_hz.unwrap_or(0.0))
            .set(&FROM, from)
            .set(&KIND, kind)
            .set(&DONE, refused.is_none())
            .set(&keys::FAMILY, keys::Family::Receiver);
        if let Some(f) = freq_hz {
            e = e.set(&FREQ_HZ, f);
        }
        if let Some(w) = refused {
            e = e.set(&REFUSED, w);
        }
        if let Some(s) = session {
            e = e.set(&SESSION, s);
        }
        if let Some(sh) = self.shared.upgrade() {
            sh.events.stamp(&mut e);
        }
        let mut kept = lock(&self.events);
        if kept.len() == EVENT_CAP {
            kept.pop_front();
        }
        kept.push_back(e);
    }

    /// Writes the calibration of the device, once it has finished, and the cores that the sessions may use.
    pub(crate) fn note_device(&mut self) {
        let d = crate::stages::device_if_ready();
        let int = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
        let mut f = self.results.results_mut().fields_mut();
        f.opt(&DEVICE_UNITS, d.map(|d| d.fast_units));
        f.opt(&DEVICE_SLOW_UNITS, d.map(|d| d.slow_units));
        f.opt(&DEVICE_FAST_CORES, d.map(|d| int(d.fast_cores)));
        f.opt(&DEVICE_SLOW_CORES, d.map(|d| int(d.slow_cores)));
        f.set(&CORES_USABLE, int(self.cores.max()));
    }

    /// Writes the results of one benchmark of `stack`, and the load of its graph if it ran one. The status fails
    /// when a thread that a session would give the stack needs more than one core at the rate of the stack, by the
    /// stages of the graph on at most `cores` threads. A case without a graph is not judged.
    pub(crate) fn note_benchmark(
        &mut self,
        stack: &str,
        name: &str,
        r: sdrkit_core::BenchResult,
        load: Option<&sdrkit_graph::meter::GraphLoad>,
        cores: usize,
        core_units: f64,
    ) {
        let plan = load.map(|l| crate::stages::bench_plan(&l.stages, cores));
        let (threads, fullest, thread) = plan
            .as_ref()
            .map_or((1, r.cpu_fraction_of_realtime, 0), |p| {
                (p.threads, p.fullest, p.fullest_thread)
            });
        // A case without a graph (a generator, a single kernel) has no stack threads to judge.
        let status = match &plan {
            None => "no graph".to_owned(),
            Some(_) if fullest > 1.0 => format!("FAIL: thread {thread} needs {fullest:.2} cores"),
            Some(_) => "ok".to_owned(),
        };
        if let Some(mut row) = self.results.row_mut(&BENCHMARKS, name) {
            row.set(&catalog::STACK, stack);
            row.set(&ITEMS_PER_S, r.items_per_second);
            row.set(&CPU_SHARE, r.cpu_fraction_of_realtime);
            row.set(&BENCH_THREADS, i64::from(threads));
            row.set(&BENCH_THREAD_CORES, fullest);
            row.set(&BENCH_STATUS, status.as_str());
        }
        if let (Some(l), Some(p)) = (load, plan) {
            self.note_bench_load(name, l, &p, core_units);
        }
    }

    /// Writes the rows of the load tables of the benchmark `name`.
    fn note_bench_load(
        &mut self,
        name: &str,
        l: &sdrkit_graph::meter::GraphLoad,
        p: &crate::stages::BenchPlan,
        core_units: f64,
    ) {
        let int = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
        let us = |ns: u64| ns as f64 / 1e3;
        let per_s = |n: u64| {
            if p.signal_s > 0.0 {
                n as f64 / p.signal_s
            } else {
                0.0
            }
        };
        let stage_name = |i: usize| l.stages.get(i).map_or("", |s| s.name.as_str());
        for c in &l.components {
            let key = if c.section.is_empty() {
                format!("{name}/{}", c.component)
            } else {
                format!("{name}/{}/{}", c.component, c.section)
            };
            let Some(mut row) = self.results.row_mut(&BENCH_COMPONENTS, &key) else {
                continue;
            };
            let cost = per_s(c.busy_ns) / 1e9;
            row.set(&BENCH_NAME, name);
            row.set(&BENCH_COMPONENT, c.component.as_str());
            row.set(&s::LOAD_SECTION, c.section);
            row.set(&s::LOAD_STAGE, stage_name(c.stage));
            row.set(
                &s::LOAD_THREAD,
                i64::from(p.stage_thread.get(c.stage).copied().unwrap_or(0)),
            );
            row.set(&s::LOAD_COST, cost);
            row.set(&s::LOAD_CALLS, int(c.calls));
            row.set(&s::LOAD_ITEMS_IN, int(c.items_in));
            row.set(&s::LOAD_ITEMS_OUT, int(c.items_out));
            row.set(&s::LOAD_MEAN_US, us(c.busy_ns) / c.calls.max(1) as f64);
            row.set(&s::LOAD_WORST_US, us(c.worst_ns));
            row.set(&s::LOAD_UNITS, cost * core_units);
        }
        for (i, st) in l.stages.iter().enumerate() {
            let Some(mut row) = self
                .results
                .row_mut(&BENCH_STAGES, &format!("{name}/{}", st.name))
            else {
                continue;
            };
            let cost = p.stage_cores.get(i).copied().unwrap_or(0.0);
            row.set(&BENCH_NAME, name);
            row.set(&s::LOAD_STAGE, st.name.as_str());
            row.set(
                &s::LOAD_THREAD,
                i64::from(p.stage_thread.get(i).copied().unwrap_or(0)),
            );
            row.set(&s::LOAD_COST, cost);
            row.set(&s::LOAD_UNITS, cost * core_units);
            row.opt(&s::LOAD_DECLARED_UNITS, l.decls.get(i).and_then(|d| d.cost));
        }
        for c in &l.connections {
            let key = format!("{name}/{}.{}>{}.{}", c.from, c.output, c.to, c.input);
            let Some(mut row) = self.results.row_mut(&BENCH_CONNECTIONS, &key) else {
                continue;
            };
            row.set(&BENCH_NAME, name);
            row.set(&s::CONN_FROM, c.from.as_str());
            row.set(&s::CONN_OUTPUT, c.output.as_str());
            row.set(&s::CONN_TO, c.to.as_str());
            row.set(&s::CONN_INPUT, c.input.as_str());
            row.set(&s::CONN_FORMAT, c.format.as_str());
            row.set(&s::LOAD_STAGE, stage_name(c.stage));
            row.set(&s::CONN_ITEMS, per_s(c.items));
            row.set(&s::CONN_BYTES, per_s(c.bytes));
            row.set(
                &s::CONN_HELD_MEAN,
                c.held_sum as f64 / c.held_samples.max(1) as f64,
            );
            row.set(&s::CONN_HELD_MAX, int(c.held_max));
            row.opt(
                &s::CONN_CAPACITY,
                (c.capacity_bytes > 0).then(|| int(c.capacity_bytes)),
            );
            let waited = (c.wait_samples > 0).then_some(c.wait_samples);
            row.opt(
                &s::CONN_WAIT_MEAN,
                waited.map(|n| us(c.wait_ns_sum) / n as f64),
            );
            row.opt(&s::CONN_WAIT_WORST, waited.map(|_| us(c.wait_worst_ns)));
            row.set(&s::CONN_FULL, int(c.full));
            row.set(&s::CONN_EMPTY, int(c.empty));
            row.set(&s::CONN_BOUNDARY, c.boundary);
        }
    }

    fn shared(&self) -> Result<Arc<Shared>, CommandError> {
        self.shared
            .upgrade()
            .ok_or_else(|| failed(crate::EngineError::Stopped))
    }
}

impl Owner for EngineOwner {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let which = EngineSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        self.values.set(id, value)?;
        match which {
            EngineSetting::TransmitAllowed => self
                .transmit_allowed
                .store(self.values.get(&TRANSMIT_ALLOWED), Ordering::Release),
            EngineSetting::CoresMax => {
                self.cores
                    .set_max(usize::try_from(self.values.get(&CORES_MAX)).unwrap_or(0));
                self.note_device();
            }
            EngineSetting::BackendFft
            | EngineSetting::BackendFir
            | EngineSetting::BackendResample
            | EngineSetting::BackendViterbi
            | EngineSetting::BackendRs
            | EngineSetting::BackendLdpc => {
                let decl = &self.values.schema().settings()[id.index()];
                let choice = match self.values.value(id) {
                    Some(Value::Choice(i)) => decl.field.choices.get(usize::from(*i)),
                    _ => None,
                };
                let choice = choice.ok_or(SettingError::NoSuchId { id })?;
                backends::select(&decl.field.key, &choice.key)
                    .map_err(|_| SettingError::NoSuchId { id })?;
                self.show_backends();
                return Ok(());
            }
            _ => {}
        }
        if !matches!(
            which,
            EngineSetting::RequestsAllowed | EngineSetting::TransmitAllowed
        ) {
            if let Some(sh) = self.shared.upgrade() {
                sh.give_station(&self.station());
            }
        }
        Ok(())
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
    fn commands(&self) -> &Commands {
        &self.commands
    }
    /// The state commands lock every other owner in turn. The engine owner's lock comes before every other lock.
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = EngineCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        match cmd {
            EngineCommand::StateSave => {
                let bytes = crate::saved::save_state(&*self.shared()?).map_err(failed)?;
                let mut reply = self
                    .commands
                    .def(id)
                    .map(|d| d.new_reply())
                    .ok_or(CommandError::NoSuchId { id })?;
                reply.set(&STATE, Arc::from(bytes));
                Ok(Some(reply))
            }
            EngineCommand::StateRestore => {
                let bytes = args.arg(&A_STATE)?;
                let sh = self.shared()?;
                crate::saved::restore_state(&sh, self, bytes).map_err(failed)?;
                Ok(None)
            }
            EngineCommand::BenchmarkRun => {
                let seconds = args.get(&SECONDS);
                let only = args.get(&ONLY).to_string();
                let shared = Weak::clone(&self.shared);
                std::thread::Builder::new()
                    .name("sdrkit-bench".into())
                    .spawn(move || run_benchmarks(&shared, seconds, &only))
                    .map_err(|e| failed(crate::EngineError::thread(&e)))?;
                Ok(None)
            }
        }
    }
}

fn failed(e: crate::EngineError) -> CommandError {
    CommandError::Failed {
        source: sdrkit_core::StageError::new(e),
    }
}

/// Runs the benchmarks of the stacks of the engine whose name or stack id contains `only`, each for about `seconds`, and
/// writes each result when it is known.
fn run_benchmarks(shared: &Weak<Shared>, seconds: f64, only: &str) {
    let Some(sh) = shared.upgrade() else {
        return;
    };
    let stacks = sh.keys.stacks.clone();
    drop(sh);
    for d in stacks {
        let Some(list) = d.benchmarks else {
            continue;
        };
        for b in list() {
            if !(b.name.contains(only) || d.id.contains(only)) {
                continue;
            }
            // The graph that the benchmark ran shows its load; of several, the one that worked most.
            let (r, loads) = sdrkit_graph::meter::record(|| (b.run)(seconds));
            let load = loads.into_iter().max_by_key(|l| {
                l.components
                    .iter()
                    .filter(|c| c.section.is_empty())
                    .map(|c| c.busy_ns)
                    .sum::<u64>()
            });
            let Some(sh) = shared.upgrade() else {
                return;
            };
            let core_units = crate::stages::device().fast_units;
            let cores = sh.cores.max();
            sh.engine
                .lock()
                .note_benchmark(d.id, b.name, r, load.as_ref(), cores, core_units);
        }
    }
}

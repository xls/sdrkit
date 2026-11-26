//! SuperSID (`supersid`): the level of each monitored VLF transmitter in every sample interval (5 s by default), sudden
//! changes of a level (a sudden ionospheric disturbance, as from a solar flare) as `sid.event`, the terminator marks of each
//! path, and the SuperSID data file through the host's storage.
//! Standard: SuperSID data file of the Stanford Solar Center's SuperSID program (header keys and data lines, see [`log`]).

pub mod gen;
pub mod judge;
pub mod list;
pub mod log;
pub mod meter;
pub mod terminator;

use crate::family::{self, REPORT_S};
use judge::{Judge, SidEvent};
use list::Transmitter;
use log::{LogFormat, Logged, SidLog, Site};
use meter::{Level, Meter, Spectrum};
use crate::typed::{
    SupersidEventRow, SupersidFields, SupersidMarkRow, SupersidSidEvent, SupersidTransmitterRow,
    SID_EVENT, SUPERSID_EVENTS, SUPERSID_MARKS, SUPERSID_MINUTES, SUPERSID_TRANSMITTERS,
};
use sdrkit_core::interface::{Hertz, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Choice, Clock, Command, CommandError, CommandId, Complex32, ConfigError,
    GenConfig, Generator, LockProfile, LockState, Receiver, Record, Results, ResultsDecl, Schema,
    Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Station, Storage,
    StorePath, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, StackGraph,
};
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "supersid";
/// The dial of the test signal, Hz (the usual dials are data: `data/vlf/vlf_dials.csv`).
pub const TEST_DIAL_HZ: f64 = 22e3;
/// The least rate of live input, Hz.
pub const MIN_RATE_HZ: f64 = 48e3;
/// The least rate of a recording, Hz.
pub const MIN_RECORDING_RATE_HZ: f64 = 8e3;
/// The rows of the event log.
pub const EVENT_LOG_CAP: usize = 200;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: the 5 s interval, the level rule (signal band of ±60 Hz, noise \
     bands 150 to 400 Hz from the carrier, the quieter side), the 6 dB presence limit, the event rule (the last 2 minutes \
     against the 2 minutes 10 minutes earlier, the 1.5 dB step, the end at half the step from the hourly median, nothing \
     judged before 15 minutes) and the D-region height of 85 km. In the data file, the unit of the values, the file names, \
     the form of UTC_Offset, the empty Longitude and Latitude of a station without a position and the rows of 0 before the \
     first reading are sdrkit's choices.";

/// The monitored transmitters.
pub const TRANSMITTERS: Setting<field::Text> =
    Setting::new("transmitters", "Monitored transmitters", "", "").help(
        "One record per line or between semicolons: label,frequency_hz or label,frequency_hz,lat,lon. At most 32. \
         The host fills it from its transmitter file.",
    );
/// The step threshold.
pub const STEP_DB: Setting<field::F64> = Setting::new("step_db", "Step threshold", "dB", 1.5)
    .range(0.3, 12.0, 0.1)
    .help("The change of the 2-minute mean against 10 minutes earlier that starts an event.");
/// The sample interval.
pub const AVERAGE_S: Setting<field::F64> = Setting::new("average_s", "Sample interval", "s", 5.0)
    .range(1.0, 60.0, 0.0)
    .help("The time over which the spectra are summed for one level; also the log interval of the file.");
/// The UTC of sample 0.
pub const START_UTC: Setting<field::F64> = Setting::new(
    "start_utc",
    "UTC of sample 0 (files; 0 = host clock)",
    "s",
    0.0,
)
.range(0.0, 1e11, 1.0);
/// The data file form.
pub const LOG_FORMAT: Setting<field::Choice<LogFormat>> =
    Setting::choice("log_format", "Data file", LogFormat::Off)
        .help("Writes the SuperSID text data file, one per UTC day, into the data file folder.");
/// The site name of the data file.
pub const SITE: Setting<field::Text> = Setting::new("site", "Site name", "", "")
    .help("The Site line of the data file and the start of its name.");
/// The monitor id of the data file.
pub const MONITOR_ID: Setting<field::Text> = Setting::new("monitor_id", "Monitor id", "", "")
    .help("The MonitorID line of the data file.");
/// The folder of the data files.
pub const LOG_FOLDER: Setting<field::Text> =
    Setting::new("log_folder", "Data file folder", "", "supersid")
        .help("A folder of the host's storage.");
/// The flare of the test signal.
pub const GEN_FLARE: Setting<field::Bool> =
    Setting::new("gen_flare", "Test signal flare", "", false)
        .help("The first carrier 3 dB stronger from 17 minutes on.")
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`SidDecoder`].
    pub enum SidSetting {
        /// The monitored transmitters.
        Transmitters = TRANSMITTERS,
        /// The step threshold.
        StepDb = STEP_DB,
        /// The sample interval.
        AverageS = AVERAGE_S,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The data file form.
        LogFormat = LOG_FORMAT,
        /// The site name.
        Site = SITE,
        /// The monitor id.
        MonitorId = MONITOR_ID,
        /// The data file folder.
        LogFolder = LOG_FOLDER,
    }
}

sdrkit_core::settings! {
    /// The settings of the `supersid` stack.
    pub enum SupersidSetting {
        /// The monitored transmitters.
        Transmitters = TRANSMITTERS,
        /// The step threshold.
        StepDb = STEP_DB,
        /// The sample interval.
        AverageS = AVERAGE_S,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The data file form.
        LogFormat = LOG_FORMAT,
        /// The site name.
        Site = SITE,
        /// The monitor id.
        MonitorId = MONITOR_ID,
        /// The data file folder.
        LogFolder = LOG_FOLDER,
        /// The flare of the test signal.
        GenFlare = GEN_FLARE,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// A row of the event log.
#[derive(Clone, Debug, PartialEq)]
pub struct EventRow {
    /// The transmitter.
    pub label: String,
    /// Its frequency, Hz.
    pub frequency_hz: f64,
    /// The index of the transmitter in the list.
    pub index: usize,
    /// The UTC of the start of the capture, s.
    pub start_utc: f64,
    /// The event.
    pub event: SidEvent,
}

impl Shaped for EventRow {
    type Row = SupersidEventRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&SupersidEventRow {
            label: &self.label,
            freq_hz: Hertz(self.frequency_hz),
            start_utc: Time(self.start_utc + self.event.start_s),
            signal_time_s: self.event.start_s,
            rise_db: self.event.rise_db,
            peak_db: self.event.peak_db,
            end_utc: self.event.end_s.map(|e| Time(self.start_utc + e)),
        });
    }
}

/// One monitored transmitter and its state.
struct Monitored {
    tx: Transmitter,
    level: Level,
    judge: Judge,
    sunlit: Option<f64>,
    minutes: u64,
}

/// Clears, and starts the judging again.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Clears, and starts the levels of the hour and the judging again (15 minutes); keeps the minute means.");
static COMMANDS: [&Command; 1] = [&RESET];

/// The `science.supersid` decoder: the front end, the meter, a judge per transmitter, the marks and the data file.
pub struct SidDecoder {
    list_text: String,
    step_db: f64,
    average_s: f64,
    start_setting: f64,
    log_format: LogFormat,
    site: String,
    monitor_id: String,
    log_folder: String,
    clock: Clock,
    station: Arc<Station>,
    storage: Option<Arc<dyn Storage>>,
    rate: f64,
    dial: f64,
    offset: f64,
    meter: Option<Meter>,
    t0: Option<f64>,
    start_utc: f64,
    input: u64,
    txs: Vec<Monitored>,
    intervals: u64,
    events_total: u64,
    last: Option<(String, f64, f64)>,
    events: LogRing<EventRow>,
    log: Option<SidLog>,
    log_error: String,
    marks: Vec<(usize, terminator::Mark)>,
    marks_day: Option<i64>,
    marks_dirty: bool,
    series_reset: bool,
    started: bool,
    dirty: bool,
    cleared: bool,
    summary: String,
    done: Vec<(u64, Vec<Level>)>,
}

impl Default for SidDecoder {
    fn default() -> Self {
        SidDecoder {
            list_text: String::new(),
            step_db: STEP_DB.default,
            average_s: AVERAGE_S.default,
            start_setting: START_UTC.default,
            log_format: LogFormat::Off,
            site: String::new(),
            monitor_id: String::new(),
            log_folder: LOG_FOLDER.default.into(),
            clock: Clock::fixed(0.0),
            station: Arc::new(Station::default()),
            storage: None,
            rate: 0.0,
            dial: 0.0,
            offset: 0.0,
            meter: None,
            t0: None,
            start_utc: 0.0,
            input: 0,
            txs: Vec::new(),
            intervals: 0,
            events_total: 0,
            last: None,
            events: LogRing::new(EVENT_LOG_CAP),
            log: None,
            log_error: String::new(),
            marks: Vec::new(),
            marks_day: None,
            marks_dirty: true,
            series_reset: false,
            started: false,
            dirty: true,
            cleared: false,
            summary: String::new(),
            done: Vec::new(),
        }
    }
}

/// The UTC of sample 0: the setting, else the host clock now.
fn start_utc(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

impl SidDecoder {
    /// A decoder with the monitored transmitters `text` (the form of [`list::parse`]).
    ///
    /// # Errors
    ///
    /// [`list::ListError`] if the text does not parse.
    pub fn with_transmitters(text: &str) -> Result<Self, list::ListError> {
        let mut d = SidDecoder::default();
        d.set_list(text)?;
        Ok(d)
    }

    fn set_list(&mut self, text: &str) -> Result<(), list::ListError> {
        let l = list::parse(text)?;
        self.list_text = text.into();
        self.txs = l
            .into_iter()
            .map(|tx| Monitored {
                tx,
                level: Level::default(),
                judge: Judge::new(self.average_s, self.step_db),
                sunlit: None,
                minutes: 0,
            })
            .collect();
        self.marks_day = None;
        self.marks.clear();
        self.marks_dirty = true;
        self.series_reset = true;
        self.rebuild_log();
        self.dirty = true;
        Ok(())
    }

    fn rebuild_log(&mut self) {
        self.log = None;
        self.log_error.clear();
        if self.log_format == LogFormat::Off || self.txs.is_empty() {
            return;
        }
        let folder = if self.log_folder.trim().is_empty() {
            Ok(StorePath::root())
        } else {
            StorePath::new(self.log_folder.trim())
        };
        let Ok(folder) = folder else {
            self.log_error = "the data file folder is not a valid path".into();
            return;
        };
        let pos = self.station.position;
        let site = Site {
            site: self.site.clone(),
            monitor_id: self.monitor_id.clone(),
            longitude: pos.map(|p| p.lon_deg),
            latitude: pos.map(|p| p.lat_deg),
            utc_offset_s: self.station.utc_offset_s,
            time_zone: self.station.time_zone.clone(),
            interval_s: self.average_s,
        };
        let stations = self
            .txs
            .iter()
            .map(|m| Logged {
                label: m.tx.label.clone(),
                frequency_hz: m.tx.frequency_hz,
            })
            .collect();
        self.log = Some(SidLog::new(self.log_format, folder, site, stations));
    }

    fn build_meter(&mut self) {
        self.meter = Meter::new(self.rate, self.offset, self.average_s);
        self.t0 = None;
    }

    /// The judges and the interval sums start again; the minute means stay.
    fn restart(&mut self) {
        for m in &mut self.txs {
            m.judge.restart();
            m.level = Level::default();
        }
        if let Some(meter) = self.meter.as_mut() {
            meter.restart();
        }
        self.t0 = None;
        self.dirty = true;
    }

    fn update_marks(&mut self, utc: f64) {
        let Some(rx) = self.station.position else {
            if !self.marks.is_empty() {
                self.marks.clear();
                self.marks_dirty = true;
            }
            return;
        };
        let day = (utc.floor() as i64).div_euclid(86_400) * 86_400;
        if self.marks_day != Some(day) {
            self.marks_day = Some(day);
            self.marks.clear();
            for (i, m) in self.txs.iter().enumerate() {
                if let Some(tx) = m.tx.position {
                    for mark in terminator::path_marks(&tx, &rx, day as f64) {
                        self.marks.push((i, mark));
                    }
                }
            }
            self.marks.sort_by(|a, b| a.1.unix.total_cmp(&b.1.unix));
            self.marks_dirty = true;
        }
        for m in &mut self.txs {
            m.sunlit = m
                .tx
                .position
                .map(|tx| terminator::sunlit_share(&tx, &rx, utc));
        }
    }

    fn interval(&mut self, k: u64, levels: &[Level], out: &mut Outbox) {
        let t0 = self.t0.unwrap_or(0.0);
        let t_s = t0 + k as f64 * self.average_s;
        let utc = self.start_utc + t_s;
        self.update_marks(utc);
        self.intervals += 1;
        for (i, m) in self.txs.iter_mut().enumerate() {
            m.level = levels.get(i).copied().unwrap_or_default();
            let Some(db) = m.level.level_db else {
                continue;
            };
            let v = m.judge.push(t_s, db);
            if let Some(e) = v.started {
                self.events_total += 1;
                self.last = Some((m.tx.label.clone(), e.rise_db, e.start_s));
                self.events.push(EventRow {
                    label: m.tx.label.clone(),
                    frequency_hz: m.tx.frequency_hz,
                    index: i,
                    start_utc: self.start_utc,
                    event: e,
                });
                let text = format!(
                    "{}: {:+.1} dB in 10 minutes, {:+.1} dB over the hourly median",
                    m.tx.label, e.peak_db, e.rise_db
                );
                out.messages(0).push(SID_EVENT.event(
                    t_s,
                    m.tx.frequency_hz,
                    &SupersidSidEvent {
                        label: &m.tx.label,
                        start_utc: Time(utc),
                        rise_db: e.rise_db,
                        peak_db: e.peak_db,
                        text: &text,
                    },
                ));
            }
            let active = v.ended.or_else(|| m.judge.active().copied());
            if let Some(a) = active {
                if let Some(row) = self
                    .events
                    .find_recent_mut(EVENT_LOG_CAP, |r| r.index == i && r.event.start_s == a.start_s)
                {
                    row.event = a;
                }
            }
        }
        if let (Some(log), Some(storage)) = (self.log.as_mut(), self.storage.as_ref()) {
            let powers: Vec<Option<f64>> = self.txs.iter().map(|m| m.level.power).collect();
            match log.write(storage.as_ref(), utc, &powers) {
                Ok(()) => self.log_error.clear(),
                Err(e) => self.log_error = format!("the data file cannot be written: {e}"),
            }
        }
        self.dirty = true;
    }

    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    /// The monitored transmitters with their last levels and judges.
    pub fn levels(&self) -> impl Iterator<Item = (&Transmitter, &Level, &Judge)> {
        self.txs.iter().map(|m| (&m.tx, &m.level, &m.judge))
    }
}

impl Decoder for SidDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.supersid"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        SidSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (SidSetting::Transmitters, Value::Text(self.list_text.clone())),
            (SidSetting::StepDb, Value::F64(self.step_db)),
            (SidSetting::AverageS, Value::F64(self.average_s)),
            (SidSetting::StartUtc, Value::F64(self.start_setting)),
            (SidSetting::LogFormat, Value::Choice(self.log_format.index())),
            (SidSetting::Site, Value::Text(self.site.clone())),
            (SidSetting::MonitorId, Value::Text(self.monitor_id.clone())),
            (SidSetting::LogFolder, Value::Text(self.log_folder.clone())),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match SidSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SidSetting::Transmitters => {
                let t = TRANSMITTERS.read(id, value)?;
                self.set_list(t)
                    .map_err(|e| SettingError::Malformed { id, at: Some(e.at) })?;
            }
            SidSetting::StepDb => {
                self.step_db = STEP_DB.read(id, value)?;
                for m in &mut self.txs {
                    m.judge.set_step(self.step_db);
                }
            }
            SidSetting::AverageS => {
                let a = AVERAGE_S.read(id, value)?;
                if a != self.average_s {
                    self.average_s = a;
                    for m in &mut self.txs {
                        m.judge = Judge::new(a, self.step_db);
                    }
                    self.series_reset = true;
                    if self.rate > 0.0 {
                        self.build_meter();
                    }
                    self.rebuild_log();
                }
            }
            SidSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = start_utc(self.start_setting, &self.clock);
            }
            SidSetting::LogFormat => {
                self.log_format = LOG_FORMAT.read(id, value)?;
                self.rebuild_log();
            }
            SidSetting::Site => {
                self.site = SITE.read(id, value)?.to_string();
                self.rebuild_log();
            }
            SidSetting::MonitorId => {
                self.monitor_id = MONITOR_ID.read(id, value)?.to_string();
                self.rebuild_log();
            }
            SidSetting::LogFolder => {
                let f = LOG_FOLDER.read(id, value)?;
                if !f.trim().is_empty() && StorePath::new(f.trim()).is_err() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.log_folder = f.to_string();
                self.rebuild_log();
            }
        }
        self.dirty = true;
        Ok(())
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() != 0 {
            return Err(CommandError::NoSuchId { id });
        }
        self.clear();
        self.restart();
        Ok(None)
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &sdrkit_graph::RunContext) {
        self.clock = ctx.clock.clone();
        self.station = ctx.station.clone();
        self.storage = Some(ctx.storage.clone());
        self.start_utc = start_utc(self.start_setting, &self.clock);
        self.marks_day = None;
        self.rebuild_log();
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = f.rate;
        self.dial = f.tuned_hz;
        self.offset = f.signal_offset_hz();
        self.input = 0;
        self.start_utc = start_utc(self.start_setting, &self.clock);
        self.build_meter();
        if self.meter.is_none() {
            return Err(ConfigError::RateTooLow {
                need: MIN_RECORDING_RATE_HZ,
                got: f.rate,
            });
        }
        self.restart();
        self.rebuild_log();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.dial = f.tuned_hz;
        self.offset = f.signal_offset_hz();
        self.build_meter();
        self.restart();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        if self.t0.is_none() {
            self.t0 = Some(first as f64 / self.rate.max(1.0));
        }
        let dial = self.dial;
        let mut done = std::mem::take(&mut self.done);
        done.clear();
        if let Some(meter) = self.meter.as_mut() {
            let txs = &self.txs;
            meter.process(x, |k, s: &Spectrum| {
                done.push((
                    k,
                    txs.iter()
                        .map(|m| s.level(m.tx.frequency_hz - dial))
                        .collect(),
                ));
            });
        }
        for (k, levels) in &done {
            self.interval(*k, levels, out);
        }
        self.done = done;
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now();
        let in_span = self.txs.iter().filter(|m| m.level.in_span).count();
        let present = self.txs.iter().filter(|m| m.level.present).count();
        let judging = self.txs.iter().any(|m| m.level.present && m.judge.judging());
        if self.cleared {
            self.cleared = false;
            p.clear_rows(&SUPERSID_EVENTS);
        }
        if std::mem::take(&mut self.dirty) {
            p.clear_rows(&SUPERSID_TRANSMITTERS);
            for (i, m) in self.txs.iter().enumerate() {
                let key = format!("{:02}", i + 1);
                let l = &m.level;
                SUPERSID_TRANSMITTERS.put(
                    p,
                    &key,
                    &SupersidTransmitterRow {
                        label: &m.tx.label,
                        freq_hz: Hertz(m.tx.frequency_hz),
                        in_span: l.in_span,
                        present: l.present,
                        level_db: l.level_db,
                        absolute_dbfs: l.absolute_dbfs,
                        baseline_db: m.judge.baseline(),
                        rise_db: m.judge.rise(),
                        event_on: m.judge.active().is_some(),
                        event_peak_db: m.judge.active().map(|a| a.peak_db),
                        sunlit: m.sunlit,
                        series: SUPERSID_MINUTES[i].key,
                    },
                );
            }
            log_rows(&self.events, &SUPERSID_EVENTS, now, p);
        }
        if std::mem::take(&mut self.marks_dirty) {
            p.clear_rows(&SUPERSID_MARKS);
            for (n, (i, mark)) in self.marks.iter().enumerate() {
                let key = format!("{n:03}");
                SUPERSID_MARKS.put(
                    p,
                    &key,
                    &SupersidMarkRow {
                        label: &self.txs[*i].tx.label,
                        point: mark.point.text(),
                        mark: mark.kind.text(),
                        utc: Time(mark.unix),
                    },
                );
            }
        }
        let r = p.results_mut();
        if std::mem::take(&mut self.series_reset) {
            for d in SUPERSID_MINUTES.iter().skip(self.txs.len()) {
                if let Some(id) = r.decl().series_id(d.key) {
                    family::write_series(r, id, true, std::iter::empty());
                }
            }
            for m in &mut self.txs {
                m.minutes = u64::MAX;
            }
        }
        for (i, m) in self.txs.iter_mut().enumerate() {
            let n = m.judge.minutes_made();
            if n == m.minutes {
                continue;
            }
            m.minutes = n;
            if let Some(id) = r.decl().series_id(SUPERSID_MINUTES[i].key) {
                family::write_series(
                    r,
                    id,
                    true,
                    m.judge.minutes().iter().map(|p| [p[0] as f32, p[1] as f32]),
                );
            }
        }
        self.summary.clear();
        if !self.started {
            self.summary.push_str("SuperSID: waiting for samples");
        } else if self.txs.is_empty() {
            self.summary
                .push_str("SuperSID: no transmitters listed (setting transmitters)");
        } else {
            self.summary.push_str(&format!(
                "{present} of {} transmitters present, {in_span} in span; {} events",
                self.txs.len(),
                self.events_total
            ));
            if let Some((tx, rise, _)) = &self.last {
                self.summary.push_str(&format!("; last on {tx}, {rise:+.1} dB"));
            }
        }
        let file = self
            .log
            .as_ref()
            .and_then(|l| l.paths().last())
            .map(|p| p.as_str().to_string());
        p.put(&SupersidFields {
            listed_n: self.txs.len() as i64,
            in_span_n: in_span as i64,
            present_n: present as i64,
            intervals: i64::try_from(self.intervals).unwrap_or(i64::MAX),
            bin_hz: self
                .meter
                .as_ref()
                .and_then(|m| Some(Hertz(m.rate() / m.fft_size()? as f64))),
            events_total: i64::try_from(self.events_total).unwrap_or(i64::MAX),
            last_event_tx: self.last.as_ref().map(|l| l.0.as_str()),
            last_event_rise_db: self.last.as_ref().map(|l| l.1),
            last_event_age_s: self.last.as_ref().map(|l| Seconds((now - l.2).max(0.0))),
            log_file: file.as_deref(),
            log_error: (!self.log_error.is_empty()).then_some(self.log_error.as_str()),
            text: &self.summary,
            note: NOTE,
        });
        let lock = match (self.started, present > 0, judging) {
            (false, ..) => LockState::Off,
            (_, true, true) => LockState::Locked,
            (_, true, false) => LockState::Tracking,
            _ => LockState::Searching,
        };
        let summary = &self.summary;
        p.results_mut().update_status(|st| {
            let changed = st.lock_state != lock || st.summary != *summary;
            if changed {
                st.lock_state = lock;
                st.summary.clone_from(summary);
            }
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.events_total = 0;
        self.last = None;
        self.events.clear();
        self.cleared = true;
        self.dirty = true;
    }
}

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("sid", DecoderComponent::new(SidDecoder::default()))
        .input("sid")
        .output("sid.events")
        .status_from("sid")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut b = StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RECORDING_RATE_HZ,
            "SuperSID needs a sample rate of at least 8 ksps",
        )
        .min_rate_live(
            MIN_RATE_HZ,
            "SuperSID needs a sample rate of at least 48 ksps from a radio",
        )
;
    for s in SidSetting::schema().settings() {
        let k = s.field.key.to_string();
        b = b.route(&k, &[&format!("sid.{k}")])?;
    }
    b.command("clear", CommandTarget::of("sid", "clear"))
        .command("reset", CommandTarget::of("sid", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("science.supersid", || {
        Box::new(DecoderComponent::new(SidDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::SidGenerator::from_config(cfg)?))
}

/// The `transmitters` text of the test signal's four carriers at the dial `dial_hz`.
pub fn test_transmitters(dial_hz: f64) -> String {
    let labels = ["A", "B", "C", "D"];
    let l: Vec<Transmitter> = gen::TEST_CARRIERS
        .iter()
        .zip(labels)
        .map(|(&(off, _), label)| Transmitter {
            label: label.into(),
            frequency_hz: dial_hz + off,
            position: None,
        })
        .collect();
    list::text_of(&l)
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, four carriers", o, 0.0);
    c.seconds = o.seconds.max(12.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.settings
        .push(CaseSetting::new(&TRANSMITTERS, test_transmitters(TEST_DIAL_HZ).as_str()));
    c.check = Some(|r: &Results| {
        let n = family::field(r, SupersidFields::PRESENT_N.key);
        vec![CheckLine::new(
            SupersidFields::PRESENT_N.key,
            n >= 4.0,
            format!("{n} of 4 transmitters present"),
        )]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// SuperSID.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "SuperSID (VLF)",
    group: "Science, time and utility",
    blurb: "The level of each monitored VLF transmitter every 5 s, sudden ionospheric disturbances (as from a solar \
            flare), terminator marks and the SuperSID data file.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 3e3,
        max_hz: 0.1e6,
        default_hz: TEST_DIAL_HZ,
        rate_hz: 250e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: meter::CHANNEL_HZ,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(200e3),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: SupersidSetting::schema,
    make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(make_generator),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_list_parses() {
        let l = list::parse(&test_transmitters(TEST_DIAL_HZ)).unwrap();
        assert_eq!(l.len(), 4);
        assert_eq!(l[0].frequency_hz, 24_000.0);
        assert_eq!(l[2].frequency_hz, 21_400.0);
    }
}

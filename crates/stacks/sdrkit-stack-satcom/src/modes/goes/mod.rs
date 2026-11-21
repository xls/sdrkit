//! GOES DCS (`goes_dcs`): data collection platforms on 401.7 to 402.1 MHz at 100, 300 and 1200 bit/s, with their address,
//! sensor data and the platform table.
//!
//! Standard: NESDIS DCPRS certification standard (100, 300 and 1200 bit/s).

pub mod body;
pub mod gen;
pub mod rx;

use crate::display::{set_series, set_status, Clock};
use crate::typed::{
    GoesDcsCarrierRow, GoesDcsFields, GoesDcsPlatformRow, GoesDcsReportEvent, GoesDcsReportRow,
    GOES_DCS_CARRIERS, GOES_DCS_PLATFORMS, GOES_DCS_PLATFORM_ROW, GOES_DCS_REMOVE, GOES_DCS_REPORT,
    GOES_DCS_REPORTS, GOES_DCS_SPECTRUM,
};
use body::DataFormat;
use rx::{Report, Scanner, Slot};
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_core::{Command, Record};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "goes_dcs";
/// The stack name.
pub const NAME: &str = "GOES DCS";
/// The least input rate.
pub const MIN_RATE: f64 = 48e3;
/// Seconds between two reports.
pub const REPORT_S: f64 = 0.25;
/// The most carriers followed at once.
pub const MAX_SLOTS: usize = 8;
/// Input kept to replay into a new slot (the line is confirmed half a second after it starts), seconds.
pub const REPLAY_S: f64 = 0.6;
/// The most reports the report log keeps.
pub const REPORT_CAP: usize = 300;
/// The most platforms the platform table keeps.
pub const PLATFORM_CAP: usize = 300;
/// Points of the spectrum line.
pub const LINE_POINTS: usize = 256;
/// The carrier line over the local noise floor that a carrier needs.
pub const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Carrier threshold", "dB", 11.0)
        .range(6.0, 20.0, 0.5)
        .help("A carrier line must stand this far over the local noise floor.");

sdrkit_core::settings! {
    /// The settings of [`GoesRx`] and of the stack. Noise and the receiver's impairments of the test signal come from the
    /// standard `gen_*` settings.
    pub enum GoesSetting {
        /// Carrier threshold.
        ThresholdDb = THRESHOLD_DB,
        /// Scenario of the test signal.
        GenScenario = gen::GEN_SCENARIO,
    }
}

sdrkit_core::settings! {
    /// The settings of [`GoesRx`].
    pub enum GoesRxSetting {
        /// Carrier threshold.
        ThresholdDb = THRESHOLD_DB,
    }
}

/// A carrier followed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CarrierRow {
    /// Hz from the input's 0 Hz.
    pub offset_hz: f64,
    /// Its channel (0 when unknown).
    pub channel: usize,
    /// The line over the floor when found, dB.
    pub snr_db: f64,
    /// 0 carrier, 1 locked on the preamble, 2 receiving a report.
    pub state: u8,
    /// The bit rate, once known.
    pub rate: u32,
}

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum GoesItem {
    /// A report.
    Report(Box<Report>),
    /// A carrier whose address did not check.
    Bad,
    /// At each tick: the carriers followed and the spectrum line.
    Status {
        /// The carriers followed.
        carriers: Vec<CarrierRow>,
        /// The spectrum in dB over the floor, `LINE_POINTS` points across the input.
        line: Vec<f32>,
        /// The width of the input the line spans, Hz.
        span_hz: f64,
    },
}
impl Item for GoesItem {
    const KIND: &'static str = "satcom.goes_item";
}

/// The receiver: the scanner and a slot per carrier followed.
pub struct GoesRx {
    rate: f64,
    centre: f64,
    thr_db: f64,
    scanner: Option<Scanner>,
    slots: Vec<Slot>,
    ring: VecDeque<Complex32>,
    ring_cap: usize,
    next_eval: u64,
    next_index: u64,
    /// Lines that produced no report for a long time (steady carriers), ignored.
    ignored: Vec<f64>,
    line_seen: BTreeMap<i64, (f64, f64)>,
    line: Vec<f32>,
}

impl Default for GoesRx {
    fn default() -> Self {
        GoesRx {
            rate: 0.0,
            centre: 0.0,
            thr_db: THRESHOLD_DB.default,
            scanner: None,
            slots: Vec::new(),
            ring: VecDeque::new(),
            ring_cap: 0,
            next_eval: 0,
            next_index: 0,
            ignored: Vec::new(),
            line_seen: BTreeMap::new(),
            line: Vec::new(),
        }
    }
}

impl GoesRx {
    fn eval(&mut self, now: f64, out: &mut Vec<Timed<GoesItem>>) {
        let Some(sc) = self.scanner.as_mut() else {
            return;
        };
        let (lines, spec) = sc.evaluate(self.thr_db);
        let n = spec.len();
        self.line.clear();
        self.line.extend(
            (0..LINE_POINTS)
                .map(|c| spec[(c * n / LINE_POINTS + n / (2 * LINE_POINTS)).min(n - 1)]),
        );
        // Lines that last 30 s and never produced a report are steady carriers.
        for &(off, _) in &lines {
            let key = (off / 200.0).round() as i64;
            let e = self.line_seen.entry(key).or_insert((now, now));
            e.1 = now;
        }
        self.line_seen.retain(|_, v| now - v.1 < 2.0);
        for (k, v) in &self.line_seen {
            let off = *k as f64 * 200.0;
            if now - v.0 > 30.0 && !self.ignored.iter().any(|i| (i - off).abs() < 300.0) {
                self.ignored.push(off);
                self.slots
                    .retain(|s| (s.offset - off).abs() > 300.0 || !s.reports.is_empty());
            }
        }
        for (off, snr) in lines {
            if self.slots.len() >= MAX_SLOTS {
                break;
            }
            if self.ignored.iter().any(|i| (i - off).abs() < 300.0)
                || self.slots.iter().any(|s| (s.offset - off).abs() < 300.0)
            {
                continue;
            }
            let Ok(mut s) = Slot::new(
                self.rate,
                off,
                now - self.ring.len() as f64 / self.rate,
                snr,
            ) else {
                continue;
            };
            // Replay what came before the line was confirmed.
            let (a, b) = self.ring.as_slices();
            s.process(a);
            s.process(b);
            self.slots.push(s);
        }
        // Slots whose carrier is gone: report or count, and free.
        let lines_now: Vec<f64> = sc.last_lines().to_vec();
        let mut keep = Vec::new();
        for mut s in self.slots.drain(..) {
            let present = lines_now.iter().any(|l| (l - s.offset).abs() < 300.0);
            for r in s.reports.drain(..) {
                out.push(Timed {
                    time: r.start,
                    value: GoesItem::Report(Box::new(r)),
                });
            }
            for _ in 0..s.bad {
                out.push(Timed {
                    time: now,
                    value: GoesItem::Bad,
                });
            }
            s.bad = 0;
            if present {
                s.last_present = now;
            }
            // A preamble's sidebands can hide the line for a while: a slot waits 4 s without it before it is let go.
            if (s.done && !present) || (!s.done && now - s.last_present > 4.0) {
                continue;
            }
            if s.done && present && now - s.created > 60.0 {
                continue;
            }
            keep.push(s);
        }
        self.slots = keep;
    }
}

impl Decoder for GoesRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.goes_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<GoesItem>("out").rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        GoesRxSetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match GoesRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            // The threshold changes without losing the slots.
            GoesRxSetting::ThresholdDb => self.thr_db = THRESHOLD_DB.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(GoesRxSetting::ThresholdDb, Value::F64(self.thr_db))])
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.scanner = Some(Scanner::new(f.rate));
        self.slots.clear();
        self.ring.clear();
        self.ring_cap = (REPLAY_S * f.rate) as usize;
        self.ring.reserve(self.ring_cap + 1);
        self.next_eval = (0.25 * f.rate) as u64;
        self.next_index = 0;
        self.line_seen.clear();
        let mut o = Format::new(Kind::typed::<GoesItem>(), 0.0);
        o.centre_hz = f.centre_hz;
        o.tuned_hz = f.tuned_hz;
        Ok(vec![o])
    }
    fn retune(&mut self, f: &Format) {
        // The carriers belong to the old centre: start the search again.
        self.centre = f.centre_hz;
        self.scanner = Some(Scanner::new(self.rate));
        self.slots.clear();
        self.ring.clear();
        self.line_seen.clear();
        self.ignored.clear();
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        let out = out.items::<GoesItem>(0);
        self.next_index = first + iq.len() as u64;
        let step = ((0.25 * self.rate) as u64).max(1);
        let mut done = 0usize;
        while done < iq.len() {
            let at = first + done as u64;
            let n = (self.next_eval.saturating_sub(at) as usize).clamp(1, iq.len() - done);
            let piece = &iq[done..done + n];
            if let Some(sc) = self.scanner.as_mut() {
                sc.push(piece, at);
            }
            for s in self.slots.iter_mut() {
                s.process(piece);
            }
            self.ring.extend(piece.iter().copied());
            while self.ring.len() > self.ring_cap {
                self.ring.pop_front();
            }
            done += n;
            if first + done as u64 >= self.next_eval {
                self.next_eval += step;
                self.eval((first + done as u64) as f64 / self.rate, out);
            }
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        let carriers = self
            .slots
            .iter()
            .map(|s| CarrierRow {
                offset_hz: s.offset,
                channel: if self.centre > 0.0 {
                    body::channel_of(self.centre + s.offset)
                } else {
                    0
                },
                snr_db: s.snr_db,
                state: s.state,
                rate: s.rate,
            })
            .collect();
        out.items::<GoesItem>(0).push(Timed {
            time: self.next_index as f64 / self.rate.max(1.0),
            value: GoesItem::Status {
                carriers,
                line: self.line.clone(),
                span_hz: self.rate,
            },
        });
    }
}

/// One report of the report log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportRow {
    /// When it started.
    pub at: Stamp,
    /// The address word, 8 hex digits.
    pub address: String,
    /// Address bits corrected.
    pub corrected: usize,
    /// The bit rate.
    pub rate: u32,
    /// Manchester coded.
    pub manchester: bool,
    /// The carrier frequency, Hz.
    pub frequency_hz: f64,
    /// Its channel (0 when unknown).
    pub channel: usize,
    /// The carrier line over the floor, dB.
    pub snr_db: f64,
    /// The flag byte, or -1.
    pub flags: i32,
    /// The data format.
    pub format: DataFormat,
    /// The report ended with an EOT.
    pub eot: bool,
    /// Characters whose parity failed.
    pub parity_errors: usize,
    /// The data as text.
    pub text: String,
}

impl Shift for ReportRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

/// One platform of the platform table: its last report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlatformRow {
    /// Its last report.
    pub last: ReportRow,
}

impl Shift for PlatformRow {
    fn shift_times(&mut self, by_s: f64) {
        self.last.shift_times(by_s);
    }
}

/// What the display publishes.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(GoesDcsFields::SHAPE.fields)
                .with_table(&GOES_DCS_REPORTS)
                .with_table(&GOES_DCS_PLATFORMS)
                .with_table(&GOES_DCS_CARRIERS)
                .series(
                    GOES_DCS_SPECTRUM.key,
                    GOES_DCS_SPECTRUM.label,
                    GOES_DCS_SPECTRUM.x_unit,
                    GOES_DCS_SPECTRUM.y_unit,
                ),
        )
    })
    .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for ReportRow {
    type Row = GoesDcsReportRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&GoesDcsReportRow {
            at: Seconds(self.at.t),
            heard_at: self.at.wall.map(Time),
            address: &self.address,
            bit_rate_hz: Hertz(f64::from(self.rate)),
            manchester: self.manchester,
            channel: self.channel as i64,
            snr_db: self.snr_db,
            flags: i64::from(self.flags),
            format: self.format,
            eot: self.eot,
            parity_errors: self.parity_errors as i64,
            text: &self.text,
        });
    }
}

impl Shaped for PlatformRow {
    type Row = GoesDcsPlatformRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let r = &self.last;
        let end = r
            .text
            .char_indices()
            .nth(64)
            .map_or(r.text.len(), |(i, _)| i);
        out.put(&GoesDcsPlatformRow {
            address: &r.address,
            channel: r.channel as i64,
            bit_rate_hz: Hertz(f64::from(r.rate)),
            snr_db: r.snr_db,
            format: r.format,
            text: &r.text[..end],
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }
}

/// What the display keeps across sessions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// The reports, newest last.
    pub reports: LogRing<ReportRow>,
    /// The platforms, by address.
    pub platforms: ObjectTable<String, PlatformRow>,
    /// Reports received whole.
    pub good: u64,
    /// Carriers whose address did not check, and reports cut off.
    pub bad: u64,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            reports: LogRing::new(REPORT_CAP),
            platforms: ObjectTable::new(PLATFORM_CAP),
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.reports.shift_times(by_s);
        self.platforms.shift_times(by_s);
    }
}

impl Tables for Heard {
    const FORMAT: StateFormat = StateFormat::new("satcom.goes_dcs", 1);
    fn clear(&mut self) {
        self.reports.clear();
        self.platforms.clear();
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.reports.adopt(saved.reports);
        self.platforms.adopt(saved.platforms);
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

/// The display: the report log, the platforms, the carriers, the band line and the events.
pub struct GoesDisplay {
    clock: Clock,
    heard: Heard,
    centre_hz: f64,
    carriers: Vec<CarrierRow>,
    line: Vec<f32>,
    rate: f64,
    live: bool,
    host_clock: sdrkit_core::Clock,
    /// The tables changed since the last report.
    dirty: bool,
    /// The summary of the status, kept to be written again.
    summary: String,
}

impl Default for GoesDisplay {
    fn default() -> Self {
        GoesDisplay {
            clock: Clock::default(),
            heard: Heard::default(),
            centre_hz: 0.0,
            carriers: Vec::new(),
            line: Vec::new(),
            rate: 0.0,
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            dirty: true,
            summary: String::new(),
        }
    }
}

impl GoesDisplay {
    fn report(&mut self, r: &Report, t: f64, out: &mut Outbox) {
        let freq = self.centre_hz + r.offset_hz;
        let row = ReportRow {
            at: self.clock.stamp(t),
            address: r.address.clone(),
            corrected: r.corrected,
            rate: r.rate,
            manchester: r.manchester,
            frequency_hz: freq,
            channel: body::channel_of(freq),
            snr_db: r.snr_db,
            flags: r.body.flags,
            format: r.body.format,
            eot: r.body.eot,
            parity_errors: r.body.parity_errors,
            text: r.body.text.clone(),
        };
        self.dirty = true;
        if r.cut_off {
            self.heard.bad += 1;
        } else {
            self.heard.good += 1;
        }
        out.messages(0).push(GOES_DCS_REPORT.event(
            t,
            freq,
            &GoesDcsReportEvent {
                source: &row.address,
                text: &row.text,
                snr_db: Some(row.snr_db).filter(|s| s.is_finite()),
                family: Family::Sensor,
                address: &row.address,
                channel: row.channel as i64,
                bit_rate_hz: Hertz(f64::from(row.rate)),
                manchester: row.manchester,
                flags: i64::from(row.flags),
                format: row.format,
                eot: row.eot,
                parity_errors: row.parity_errors as i64,
                bits_corrected: row.corrected as i64,
            },
        ));
        // A table that refuses the time (not finite) keeps the report in the log only.
        let _ = self
            .heard
            .platforms
            .upsert(&row.address, row.at, PlatformRow::default);
        if let Some(p) = self.heard.platforms.get_mut(&row.address) {
            p.last = row.clone();
        }
        self.heard.reports.push(row);
    }
}

sdrkit_core::commands! {
    /// The commands of [`GoesDisplay`].
    pub enum GoesDisplayCommand {
        /// Forgets one object.
        Remove = GOES_DCS_REMOVE.command,
    }
}

impl Decoder for GoesDisplay {
    type Input = Timed<GoesItem>;
    fn name(&self) -> &'static str {
        "satcom.goes_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.centre_hz = f.centre_hz;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_hz = f.centre_hz;
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, x: &[Timed<GoesItem>], _first: u64, out: &mut Outbox) {
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                GoesItem::Bad => self.heard.bad += 1,
                GoesItem::Report(r) => self.report(r, t, out),
                GoesItem::Status {
                    carriers,
                    line,
                    span_hz,
                } => {
                    self.carriers.clone_from(carriers);
                    self.line.clone_from(line);
                    self.rate = *span_hz;
                }
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let h = &self.heard;
        let state = if !h.reports.is_empty() {
            2
        } else if !self.carriers.is_empty() {
            1
        } else {
            0
        };
        p.put(&GoesDcsFields {
            frames_ok: h.good as i64,
            frames_bad: h.bad as i64,
            carriers: self.carriers.len() as i64,
        });
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&h.reports, &GOES_DCS_REPORTS, now, p);
            shaped::rows(&h.platforms, &GOES_DCS_PLATFORMS, now, p);
        }
        shaped::indexed_rows(p, &GOES_DCS_CARRIERS, &self.carriers, |c, out| {
            out.put(&GoesDcsCarrierRow {
                    offset_hz: Hertz(c.offset_hz),
                    channel: c.channel as i64,
                    snr_db: c.snr_db,
                    bit_rate_hz: (c.rate > 0).then_some(Hertz(f64::from(c.rate))),
                },
            );
        });
        let r = p.results_mut();
        if !self.line.is_empty() {
            // The line spans the input: point k at its centre frequency from the input's 0 Hz.
            let n = self.line.len() as f64;
            let rate = self.rate;
            let points = || {
                self.line
                    .iter()
                    .enumerate()
                    .map(move |(k, &v)| [((k as f64 + 0.5) / n * rate - rate / 2.0) as f32, v])
            };
            set_series(r, GOES_DCS_SPECTRUM.key, true, points);
        }
        let h = &self.heard;
        let lock_state = LockState::from_level(state);
        let offset = self.carriers.first().map(|c| c.offset_hz);
        let snr = h.reports.newest_first().next().map(|r| r.snr_db as f32);
        let (reports, platforms) = (h.reports.len(), h.platforms.len());
        let summary = &mut self.summary;
        summary.clear();
        let _ = if reports == 0 {
            write!(summary, "GOES DCS: no platform heard")
        } else {
            write!(
                summary,
                "GOES DCS: {reports} reports from {platforms} platforms"
            )
        };
        set_status(r, lock_state, offset, snr, None, summary);
    }
    fn commands(&self) -> &'static [&'static Command] {
        GoesDisplayCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match GoesDisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            GoesDisplayCommand::Remove => {
                let p = self.results();
                crate::display::remove(
                    &mut self.heard.platforms,
                    &GOES_DCS_PLATFORMS,
                    &p,
                    args,
                    &GOES_DCS_PLATFORM_ROW,
                )?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn clear(&mut self) {
        self.heard.clear();
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.heard.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

/// Adds the components of this stack to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.goes_receiver", || {
        Box::new(DecoderComponent::new(GoesRx::default()))
    });
    r.add("satcom.goes_display", || {
        Box::new(DecoderComponent::new(GoesDisplay::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("rx", DecoderComponent::new(GoesRx::default()))
        .add("display", DecoderComponent::new(GoesDisplay::default()))
        .input("rx")
        .connect("rx.out", "display")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "GOES DCS needs a sample rate of at least 48 ksps")
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .route("threshold_db", &["rx.threshold_db"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::GoesGenerator::from_config(cfg)))
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "GOES / Meteosat data collection platforms at 100, 300 and 1200 bit/s: address, sensor data, platform table. Receive only.",
    tuning: Tuning {
        min_hz: 401.0e6,
        max_hz: 403.0e6,
        default_hz: 401.85e6,
        rate_hz: 1e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.5e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: GoesSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dcp],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

/// The verification of the stack: one platform of each rate.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three platforms", o, 0.0);
    c.seconds = c.seconds.max(16.0);
    c.expect_audio = Some(false);
    c.check = Some(check_reports);
    vec![c]
}

/// At least one report received whole.
fn check_reports(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let ok = crate::field(r, GoesDcsFields::FRAMES_OK.key) as i64;
    vec![CheckLine::new(
        "reports",
        ok >= 1,
        format!("{ok} reports received"),
    )]
}

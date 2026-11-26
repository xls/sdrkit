//! Riometer (`riometer`): the power of the cosmic radio noise with interference rejected ([`meter`]), against a quiet-day curve
//! per sidereal time ([`qdc`]); absorption in dB and `riometer.absorption` events while it is over a threshold. Extra
//! ionisation of the D region (polar cap absorption, flares, aurora) absorbs part of the cosmic noise, which is otherwise steady
//! for a given sidereal time. The radio gain must stay fixed: an automatic gain change looks like absorption.
//!
//! Document: G. C. Little and H. Leinbach, "The Riometer - A Device for the Continuous Measurement of Ionospheric Absorption",
//! Proc. IRE 47 (2), 1959, pp. 315-320.

pub mod gen;
pub mod meter;
pub mod qdc;

use crate::family::{write_series, REPORT_S};
use crate::mscatter::START_UTC;
use meter::{Minute, PowerMeter};
use qdc::Qdc;
use sdrkit_core::command::{Command, CommandError, CommandId};
use crate::typed::{
    RiometerAbsorptionEvent, RiometerFields, RiometerQdcArgs, RiometerQdcReply, RIOMETER_ABSORPTION,
    RIOMETER_EXPORT_QDC, RIOMETER_IMPORT_QDC,
};
use sdrkit_core::interface::{Fields, Seconds};
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::record::RecordDecl;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile,
    LockState, Receiver, Record, ResultsDecl, Schema, SeriesId, Setting, SettingError, SettingId,
    SettingSet, StackConfig, StackDescriptor, Station, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, RunContext, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "riometer";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 48e3;
/// Minutes kept in the series: 24 hours.
pub const SERIES_MINUTES: usize = 1440;
/// The weight of a new minute in the interfered share.
const SHARE_WEIGHT: f64 = 0.02;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: the power estimate (chunks of 20 ms, the median of 1024 \
bins, the lower quartile of each second, a second interfered when its median is over 1.35 times that, the element 20 of 60 \
for a minute, a minute interfered when more than 20 of its seconds were), the quiet-day curve (288 bins of 5 sidereal \
minutes, raised by 0.2 of a quiet minute up to 1.15 times the bin, lowered by 0.001, ready with 36 bins) and the 0.3 dB \
event threshold. The radio gain must stay fixed; the stack cannot see an automatic gain change.";

/// The absorption event threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Absorption event threshold", "dB", 0.3)
        .range(0.05, 6.0, 0.0)
        .help("An event lasts while the absorption is at this or more.");
/// The quiet-day curve.
pub const QDC_SETTING: Setting<field::Text> =
    Setting::new("qdc", "Quiet-day curve (saved state)", "", "").help(
        "The curve as text: qdc1 and 288 values. Empty keeps the curve the stack holds; a bad text is refused.",
    );

sdrkit_core::settings! {
    /// The settings of [`RiometerDecoder`].
    pub enum DecoderSetting {
        /// The event threshold.
        Threshold = THRESHOLD,
        /// The quiet-day curve.
        Qdc = QDC_SETTING,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the `riometer` stack.
    pub enum RiometerSetting {
        /// The event threshold.
        Threshold = THRESHOLD,
        /// The quiet-day curve.
        Qdc = QDC_SETTING,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The burst share of the test signal.
        GenBurstShare = gen::GEN_BURST_SHARE,
    }
}

/// The start or the end of an absorption event.
#[derive(Clone, Debug, PartialEq)]
pub struct Absorption {
    /// `true` at the start.
    pub start: bool,
    /// Signal time of the minute, s.
    pub time_s: f64,
    /// The absorption at the start, the largest at the end, dB.
    pub depth_db: f64,
    /// The time from the start, s (0 at the start).
    pub duration_s: f64,
}

/// What a minute gave.
#[derive(Clone, Debug, PartialEq)]
pub struct MinuteOut {
    /// The minute.
    pub minute: Minute,
    /// The local sidereal time, h; `None` without a position.
    pub lst_h: Option<f64>,
    /// The curve at that time (linear); `None` for an empty bin or without a position.
    pub curve: Option<f64>,
    /// The absorption, dB (0 while the curve is not ready).
    pub absorption_db: f64,
    /// An event starts or ends.
    pub event: Option<Absorption>,
}

/// The minute logic: the quiet-day curve, the absorption, the events and the interfered share.
#[derive(Clone, Debug)]
pub struct Riometer {
    qdc: Qdc,
    threshold_db: f64,
    open: Option<(f64, f64)>,
    share: f64,
}

impl Riometer {
    /// A riometer with an empty curve and the event threshold `threshold_db`.
    pub fn new(threshold_db: f64) -> Self {
        Riometer {
            qdc: Qdc::default(),
            threshold_db,
            open: None,
            share: 0.0,
        }
    }

    /// The curve.
    pub fn qdc(&self) -> &Qdc {
        &self.qdc
    }

    /// Replaces the curve.
    pub fn set_qdc(&mut self, q: Qdc) {
        self.qdc = q;
    }

    /// Sets the event threshold, dB.
    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = db;
    }

    /// The interfered share: 0.98 × the last + 0.02 for an interfered minute.
    pub fn share(&self) -> f64 {
        self.share
    }

    /// Whether an event is open.
    pub fn event_on(&self) -> bool {
        self.open.is_some()
    }

    /// Forgets the open event and the share; keeps the curve.
    pub fn reset(&mut self) {
        self.open = None;
        self.share = 0.0;
    }

    /// Takes minute `m`, whose middle is at signal time `time_s` and Unix time `utc`, at east longitude `lon_deg` (`None`
    /// without a position: the power is measured, the curve is not updated).
    pub fn push(&mut self, m: Minute, time_s: f64, utc: f64, lon_deg: Option<f64>) -> MinuteOut {
        self.share = (1.0 - SHARE_WEIGHT) * self.share + if m.interfered { SHARE_WEIGHT } else { 0.0 };
        let lst = lon_deg.map(|lon| qdc::lst_hours(utc, lon));
        let mut absorption = 0.0;
        if let Some(l) = lst {
            absorption = if m.interfered {
                0.0
            } else {
                self.qdc.absorption(l, m.power)
            };
            self.qdc.update(l, m.power, m.interfered);
        }
        let curve = lst.and_then(|l| self.qdc.at(l));
        let over = absorption >= self.threshold_db;
        let event = match (self.open.as_mut(), over) {
            (None, true) => {
                self.open = Some((time_s, absorption));
                Some(Absorption {
                    start: true,
                    time_s,
                    depth_db: absorption,
                    duration_s: 0.0,
                })
            }
            (Some(o), true) => {
                o.1 = o.1.max(absorption);
                None
            }
            (Some(_), false) if !m.interfered => {
                let (t0, depth) = self.open.take().expect("open");
                Some(Absorption {
                    start: false,
                    time_s,
                    depth_db: depth,
                    duration_s: time_s - t0,
                })
            }
            _ => None,
        };
        MinuteOut {
            minute: m,
            lst_h: lst,
            curve,
            absorption_db: absorption,
            event,
        }
    }
}

/// The absorption per minute: x minutes from the start, y dB.
pub const ABSORPTION_SERIES: SeriesDef = SeriesDef {
    key: "absorption",
    label: "Absorption per minute",
    x_unit: "min",
    y_unit: "dB",
};
/// The power per minute: x minutes from the start, y dB of full scale.
pub const POWER_SERIES: SeriesDef = SeriesDef {
    key: "power",
    label: "Power per minute",
    x_unit: "min",
    y_unit: "dBFS",
};

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The `reset` command: the power estimate starts again; the curve stays.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Starts the power estimate again and empties the series; the quiet-day curve stays.");
static COMMANDS: [&Command; 3] = [&RESET, &RIOMETER_EXPORT_QDC.command, &RIOMETER_IMPORT_QDC.command];

/// The UTC of sample 0: the setting, else the host clock now.
fn start_utc_of(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

/// The `riometer` decoder.
pub struct RiometerDecoder {
    meter: Option<PowerMeter>,
    rio: Riometer,
    rate: f64,
    dial_hz: f64,
    threshold: f64,
    qdc_text: String,
    shown_qdc: Option<String>,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    station: Arc<Station>,
    input: u64,
    origin_s: f64,
    minutes: u64,
    last: Option<MinuteOut>,
    series: VecDeque<(u64, f64, f64)>,
    series_changed: bool,
    series_ids: Option<(SeriesId, SeriesId)>,
    events: Vec<Event>,
    started: bool,
    status: Status,
}

impl Default for RiometerDecoder {
    fn default() -> Self {
        RiometerDecoder {
            meter: None,
            rio: Riometer::new(THRESHOLD.default),
            rate: 0.0,
            dial_hz: 0.0,
            threshold: THRESHOLD.default,
            qdc_text: String::new(),
            shown_qdc: None,
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            station: Arc::new(Station::default()),
            input: 0,
            origin_s: 0.0,
            minutes: 0,
            last: None,
            series: VecDeque::new(),
            series_changed: false,
            series_ids: None,
            events: Vec::new(),
            started: false,
            status: Status::default(),
        }
    }
}

impl RiometerDecoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn forget(&mut self) {
        self.minutes = 0;
        self.last = None;
        self.series.clear();
        self.series_changed = true;
        self.rio.reset();
    }

    fn take(&mut self, out: &mut Outbox) {
        let Some(m) = self.meter.as_mut() else {
            return;
        };
        let lon = self.station.position.map(|p| p.lon_deg);
        for minute in m.take_minutes() {
            let t = self.origin_s + minute.index as f64 * 60.0 + 30.0;
            let o = self.rio.push(minute, t, self.start_utc + t, lon);
            self.minutes += 1;
            if self.series.len() >= SERIES_MINUTES {
                self.series.pop_front();
            }
            let idx = ((self.origin_s / 60.0).round() as u64) + minute.index;
            self.series.push_back((
                idx,
                o.absorption_db,
                10.0 * minute.power.max(1e-30).log10(),
            ));
            self.series_changed = true;
            if let Some(e) = &o.event {
                let text = if e.start {
                    format!("absorption from {:.2} dB", e.depth_db)
                } else {
                    format!("absorption ended, {:.2} dB at most, {:.0} min", e.depth_db, e.duration_s / 60.0)
                };
                self.events.push(RIOMETER_ABSORPTION.event(
                    e.time_s,
                    self.dial_hz,
                    &RiometerAbsorptionEvent {
                        text: &text,
                        phase: if e.start { "start" } else { "end" },
                        depth_db: e.depth_db,
                        duration_s: Seconds(e.duration_s),
                    },
                ));
            }
            self.last = Some(o);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }

    fn import(&mut self, text: &str) -> Result<(), String> {
        let q = Qdc::import(text)?;
        self.rio.set_qdc(q);
        Ok(())
    }
}

impl Decoder for RiometerDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.riometer"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::Threshold => {
                self.threshold = THRESHOLD.read(id, value)?;
                self.rio.set_threshold(self.threshold);
            }
            DecoderSetting::Qdc => {
                let t = QDC_SETTING.read(id, value)?;
                if !t.trim().is_empty() {
                    self.import(t)
                        .map_err(|_| SettingError::Malformed { id, at: None })?;
                }
                self.qdc_text = t.to_string();
            }
            DecoderSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = start_utc_of(self.start_setting, &self.clock);
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::Threshold, Value::F64(self.threshold)),
            (DecoderSetting::Qdc, Value::Text(self.qdc_text.clone())),
            (DecoderSetting::StartUtc, Value::F64(self.start_setting)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match id.index() {
            0 => {
                self.forget();
                self.origin_s = self.now();
                self.meter = (self.rate > 0.0).then(|| PowerMeter::new(self.rate));
                Ok(None)
            }
            1 => {
                let mut r = Record::new(Arc::new(RecordDecl::of(RIOMETER_EXPORT_QDC.command.reply)));
                RiometerQdcReply {
                    qdc: &self.rio.qdc().export(),
                }
                .write(&mut r);
                Ok(Some(r))
            }
            2 => {
                let RiometerQdcArgs { text: t } = RIOMETER_IMPORT_QDC.args(args)?;
                // The only argument: slot 0.
                let arg = SettingId(0);
                self.import(t).map_err(|_| CommandError::BadArgument {
                    arg,
                    error: SettingError::Malformed { id: arg, at: None },
                })?;
                Ok(None)
            }
            _ => Err(CommandError::NoSuchId { id }),
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
        self.station = ctx.station.clone();
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.dial_hz = input.tuned_hz;
        self.input = 0;
        self.origin_s = 0.0;
        self.start_utc = start_utc_of(self.start_setting, &self.clock);
        self.meter = Some(PowerMeter::new(input.rate));
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        if let Some(m) = self.meter.as_mut() {
            m.process(x);
        }
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        let rio = &self.rio;
        let last = self.last.as_ref();
        let power = last.map(|o| 10.0 * o.minute.power.max(1e-30).log10());
        if last.is_some() || self.series_changed {
            self.shown_qdc = Some(rio.qdc().export());
        }
        if self.series_ids.is_none() {
            let d = p.results().decl();
            if let (Some(a), Some(b)) = (
                d.series_id(ABSORPTION_SERIES.key),
                d.series_id(POWER_SERIES.key),
            ) {
                self.series_ids = Some((a, b));
            }
        }
        if let (Some((a, b)), true) = (self.series_ids, std::mem::take(&mut self.series_changed)) {
            write_series(
                p.results_mut(),
                a,
                true,
                self.series.iter().map(|s| [s.0 as f32, s.1 as f32]),
            );
            write_series(
                p.results_mut(),
                b,
                true,
                self.series.iter().map(|s| [s.0 as f32, s.2 as f32]),
            );
        }
        let has_position = self.station.position.is_some();
        let st = &mut self.status;
        st.lock_state = match (self.started, self.minutes > 0, rio.qdc().ready()) {
            (false, ..) => LockState::Off,
            (_, false, _) => LockState::Searching,
            (_, true, false) => LockState::Tracking,
            (_, true, true) => LockState::Locked,
        };
        st.level_dbfs = power.map(|v| v as f32);
        st.summary.clear();
        st.summary.push_str(&match (self.started, last) {
            (false, _) => "Riometer: waiting for samples".to_string(),
            (true, None) => format!(
                "Riometer: measuring the first minute{}",
                if has_position {
                    ""
                } else {
                    "; the station position is missing, so no quiet-day curve"
                }
            ),
            (true, Some(o)) if !has_position => format!(
                "Riometer: {:.2} dBFS; the station position is missing, so no quiet-day curve",
                10.0 * o.minute.power.max(1e-30).log10()
            ),
            (true, Some(o)) if !rio.qdc().ready() => format!(
                "Riometer: {:.2} dBFS, curve {} of {} bins (ready at {})",
                10.0 * o.minute.power.max(1e-30).log10(),
                rio.qdc().filled(),
                qdc::BINS,
                qdc::READY_BINS
            ),
            (true, Some(o)) => format!(
                "Riometer: absorption {:.2} dB{}",
                o.absorption_db,
                if rio.event_on() { ", event" } else { "" }
            ),
        });
        p.put(&RiometerFields {
            text: &st.summary,
            power_db: power,
            lst_h: last.and_then(|o| o.lst_h),
            curve_db: last.and_then(|o| o.curve).map(|q| 10.0 * q.max(1e-30).log10()),
            absorption_db: last.map(|o| o.absorption_db),
            curve_bins: rio.qdc().filled() as i64,
            curve_ready: rio.qdc().ready(),
            interfered_share: rio.share(),
            event_on: rio.event_on(),
            minutes: i64::try_from(self.minutes).unwrap_or(i64::MAX),
            qdc: self.shown_qdc.as_deref(),
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.forget();
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("dec", DecoderComponent::new(RiometerDecoder::default()))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "The riometer needs a sample rate of at least 48 ksps",
        )
        .route(THRESHOLD.key(), &["dec.threshold_db"])?
        .route(QDC_SETTING.key(), &["dec.qdc"])?
        .route(START_UTC.key(), &["dec.start_utc"])?
        .command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("dec", "reset"))
        .command("export_qdc", CommandTarget::of("dec", "export_qdc"))
        .command("import_qdc", CommandTarget::of("dec", "import_qdc"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("science.riometer", || {
        Box::new(DecoderComponent::new(RiometerDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = gen::GenParams::standard(cfg.settings.get(&gen::GEN_BURST_SHARE));
    Ok(Box::new(gen::RiometerGenerator::new(cfg.rate_hz, p, cfg.seed)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, one minute of noise with bursts", o, 0.0);
    c.seconds = o.seconds.max(62.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let m = crate::family::field(r, RiometerFields::MINUTES.key);
        let p = crate::family::field(r, RiometerFields::POWER_DB.key);
        // The level of the capture is the source's; the check is that a minute was measured and read as a power.
        vec![
            CheckLine::new(RiometerFields::MINUTES.key, m >= 1.0, format!("{m}, at least 1 expected")),
            CheckLine::new(
                RiometerFields::POWER_DB.key,
                p.is_finite() && p < 10.0 && p > -100.0,
                format!("{p:.2} dBFS, a power expected"),
            ),
        ]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The riometer.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Riometer",
    group: "Science, time and utility",
    blurb: "Cosmic radio noise near 30 MHz with interference rejected, against a quiet-day curve per sidereal time: the \
            absorption of the ionosphere in dB and its events. Needs a fixed radio gain.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 20e6,
        max_hz: 60e6,
        default_hz: 30e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: RiometerSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: None,
};

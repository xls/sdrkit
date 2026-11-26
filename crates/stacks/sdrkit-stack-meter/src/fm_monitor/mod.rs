//! FM modulation monitor (`fm_monitor`): the readings a broadcast engineer takes from a modulation monitor, from the
//! FM-demodulated multiplex (MPX) of one station, once a second: peak and RMS deviation, MPX power, carrier offset, the
//! audio, pilot, stereo and RDS levels, the subcarriers at 67 and 92 kHz, and the seconds over a threshold. It reads what
//! arrives after the radio and the path and is not a type-approval instrument. A pilot counts only when it stands 3 times
//! over the bands either side of it.
//!
//! Standards: ITU-R BS.450 (deviation limit 75 or 50 kHz, pilot at 8 to 10 % of it, 38 kHz subcarrier, supplementary
//! signals from 53 to 76 kHz), ITU-R BS.412 (0 dBr is a sine of 19 kHz deviation, power over a floating 60 s), IEC 62106 and
//! ITU-R BS.643 (RDS at 57 kHz).

pub mod gen;
pub mod measure;

use crate::instrument::Front;
use crate::typed::{
    FmMonitorFields, FmMonitorLogRow, FmMonitorOverDeviationEvent, FmMonitorPilotEvent, FmMonitorRdsEvent,
    FmMonitorScaEvent, FmPilotChange, FmPilotRange, FmRdsChange, FM_MONITOR_LOG, FM_MONITOR_OVER_DEVIATION,
    FM_MONITOR_PILOT, FM_MONITOR_RDS, FM_MONITOR_SCA,
};
use gen::{FmGenerator, Mpx};
use measure::{FmMeter, FmState, PilotChange, Second};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor,
    Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kind, Outbox,
    Port, RateFrom, StackGraph,
};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "fm_monitor";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 200e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 250e3;
/// The band the front end keeps, Hz.
pub const PASS_HZ: f64 = 100e3;
/// The stretch dropped at the start while the filters fill, s.
pub const SKIP_S: f64 = 0.1;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the monitor measures.
pub const NOTE_TEXT: &str = "Readings of the multiplex as it arrives after the radio and the path, once a second; not a \
type-approval instrument. Levels are peak equivalents of the band power of a Welch spectrum. The pilot counts when it is \
over 2 kHz and 3 times over the bands either side; RDS when over 0.3 kHz and its power per Hz twice that of 60 to 62.5 kHz. The 67 kHz band \
lies inside the range of supplementary signals of ITU-R BS.450 (53 to 76 kHz) and overlaps the RDS2 subcarrier at 66.5 kHz \
(ITU-R BS.643); 92 kHz lies outside it (North American practice). Not confirmed by a public document: the subcarrier rules, \
the block peaks and the 80 kHz default threshold.";

sdrkit_core::choice_enum! {
    /// The maximum peak deviation of ITU-R BS.450.
    pub enum DeviationLimit {
        /// 75 kHz (Western Europe, the Americas, most countries).
        Khz75 = ("75", "75 kHz"),
        /// 50 kHz (some countries of Eastern Europe).
        Khz50 = ("50", "50 kHz"),
    }
}

impl DeviationLimit {
    /// The limit, Hz.
    pub fn hz(self) -> f64 {
        match self {
            DeviationLimit::Khz75 => 75e3,
            DeviationLimit::Khz50 => 50e3,
        }
    }
}

/// The peak above which a second counts as over-deviated.
pub const OVER_DEVIATION: Setting<field::F64> =
    Setting::new("over_deviation_khz", "Over-deviation threshold", "kHz", 80.0)
        .range(50.0, 150.0, 0.0)
        .help("A second whose peak deviation is over this is counted and logged.");
/// The deviation limit.
pub const LIMIT: Setting<field::Choice<DeviationLimit>> =
    Setting::choice("deviation_limit", "Peak deviation limit", DeviationLimit::Khz75)
        .help("The maximum peak deviation of ITU-R BS.450; the pilot is in its usual range at 8 to 10 % of it.");
/// The margin of the pilot range.
pub const PILOT_MARGIN: Setting<field::F64> = Setting::new("pilot_margin_khz", "Pilot range margin", "kHz", 0.5)
    .range(0.0, 2.0, 0.0)
    .help("The reading may lie this far outside 8 to 10 % of the limit and still count as in the usual range.");

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The over-deviation threshold.
        OverDeviation = OVER_DEVIATION,
        /// The deviation limit.
        Limit = LIMIT,
        /// The pilot range margin.
        PilotMargin = PILOT_MARGIN,
    }
}

sdrkit_core::settings! {
    /// The settings of the `fm_monitor` stack.
    pub enum FmMonitorSetting {
        /// The over-deviation threshold.
        OverDeviation = OVER_DEVIATION,
        /// The deviation limit.
        Limit = LIMIT,
        /// The pilot range margin.
        PilotMargin = PILOT_MARGIN,
        /// The audio tone of the test signal.
        GenAudio = gen::GEN_AUDIO_KHZ,
        /// The second tone of the test signal.
        GenTone2 = gen::GEN_TONE2_KHZ,
        /// The pilot of the test signal.
        GenPilot = gen::GEN_PILOT_KHZ,
        /// The L-R tone of the test signal.
        GenStereo = gen::GEN_STEREO_KHZ,
        /// RDS of the test signal.
        GenRds = gen::GEN_RDS_KHZ,
        /// The 67 kHz subcarrier of the test signal.
        GenSca67 = gen::GEN_SCA67_KHZ,
        /// The 92 kHz subcarrier of the test signal.
        GenSca92 = gen::GEN_SCA92_KHZ,
        /// The scale of the test multiplex.
        GenScale = gen::GEN_SCALE,
        /// The pilot from.
        GenPilotFrom = gen::GEN_PILOT_FROM_S,
        /// The pilot until.
        GenPilotUntil = gen::GEN_PILOT_UNTIL_S,
        /// RDS from.
        GenRdsFrom = gen::GEN_RDS_FROM_S,
        /// RDS until.
        GenRdsUntil = gen::GEN_RDS_UNTIL_S,
        /// The subcarriers from.
        GenScaFrom = gen::GEN_SCA_FROM_S,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// A row of the log waiting to be published.
#[derive(Clone, Debug)]
struct Row {
    serial: u64,
    time_s: f64,
    kind: &'static str,
    identity: String,
    text: String,
}

/// The `fm_monitor` decoder: the front end, the meter of each second and the state rules.
pub struct FmMonitorDecoder {
    front: Option<Front>,
    meter: Option<FmMeter>,
    state: FmState,
    threshold_khz: f64,
    limit: DeviationLimit,
    margin_khz: f64,
    dial_hz: f64,
    seconds: u64,
    last: Option<Second>,
    rows: Vec<Row>,
    serial: u64,
    cleared: bool,
    events: Vec<Event>,
    started: bool,
    status: Status,
}

impl Default for FmMonitorDecoder {
    fn default() -> Self {
        FmMonitorDecoder {
            front: None,
            meter: None,
            state: FmState::default(),
            threshold_khz: OVER_DEVIATION.default,
            limit: DeviationLimit::Khz75,
            margin_khz: PILOT_MARGIN.default,
            dial_hz: 0.0,
            seconds: 0,
            last: None,
            rows: Vec::new(),
            serial: 0,
            cleared: false,
            events: Vec::new(),
            started: false,
            status: Status::default(),
        }
    }
}

/// The range of a pilot of `pilot_hz` under the limit `limit_hz` with a margin of `margin_hz`.
pub fn pilot_range(present: bool, pilot_hz: f64, limit_hz: f64, margin_hz: f64) -> FmPilotRange {
    if !present {
        FmPilotRange::Absent
    } else if pilot_hz >= 0.08 * limit_hz - margin_hz && pilot_hz <= 0.10 * limit_hz + margin_hz {
        FmPilotRange::InRange
    } else {
        FmPilotRange::Outside
    }
}

impl FmMonitorDecoder {
    fn row(&mut self, time_s: f64, kind: &'static str, identity: String, text: String) {
        self.serial += 1;
        self.rows.push(Row {
            serial: self.serial,
            time_s,
            kind,
            identity,
            text,
        });
    }

    fn second(&mut self, s: Second) {
        self.seconds += 1;
        let t = SKIP_S + self.seconds as f64;
        let c = self.state.push(&s, 1e3 * self.threshold_khz);
        let f = self.dial_hz;
        if let Some(p) = c.pilot {
            let (word, change) = match p {
                PilotChange::Present => ("present", FmPilotChange::Present),
                PilotChange::Absent => ("absent", FmPilotChange::Absent),
                PilotChange::Lost => ("lost", FmPilotChange::Lost),
            };
            let text = format!("pilot {word}, {:.2} kHz", s.pilot_hz / 1e3);
            let e = FmMonitorPilotEvent {
                text: &text,
                change,
                pilot_khz: s.pilot_hz / 1e3,
            };
            self.events.push(FM_MONITOR_PILOT.event(t, f, &e));
            self.row(t, "Pilot", word.to_string(), text);
        }
        if let Some(on) = c.rds {
            let (word, change) = if on { ("present", FmRdsChange::Present) } else { ("lost", FmRdsChange::Lost) };
            let text = format!("RDS {word}, {:.2} kHz", s.rds_hz / 1e3);
            let e = FmMonitorRdsEvent {
                text: &text,
                change,
                rds_khz: s.rds_hz / 1e3,
            };
            self.events.push(FM_MONITOR_RDS.event(t, f, &e));
            self.row(t, "RDS", word.to_string(), text);
        }
        for (khz, change, db) in [(67.0, c.sca67, Some(s.sca67_db)), (92.0, c.sca92, s.sca92_db)] {
            let Some(on) = change else {
                continue;
            };
            let word = format!("{khz:.0} kHz {}", if on { "on" } else { "off" });
            let text = format!("subcarrier {word}, {:.1} dB over the floor", db.unwrap_or(0.0));
            let e = FmMonitorScaEvent {
                text: &text,
                subcarrier_khz: khz,
                on,
                level_db: db,
            };
            self.events.push(FM_MONITOR_SCA.event(t, f, &e));
            self.row(t, "SCA", word, text);
        }
        if c.over_deviation {
            let text = format!("peak {:.1} kHz, MPX power {:+.1} dBr", s.peak_hz / 1e3, s.mpx_dbr());
            let e = FmMonitorOverDeviationEvent {
                text: &text,
                peak_deviation_khz: s.peak_hz / 1e3,
                mpx_power_dbr: s.mpx_dbr(),
            };
            self.events.push(FM_MONITOR_OVER_DEVIATION.event(t, f, &e));
            self.row(t, "Over-deviation", format!("{:.1} kHz", s.peak_hz / 1e3), text);
        }
        self.last = Some(s);
    }

    fn headline(&self) -> String {
        let Some(s) = self.last.as_ref() else {
            return String::new();
        };
        let stereo = self.state.pilot() && s.stereo();
        let mut h = format!(
            "{}, peak {:.1} kHz",
            if stereo { "stereo" } else if self.state.pilot() { "mono with pilot" } else { "mono (no pilot)" },
            s.peak_hz / 1e3
        );
        if self.state.pilot() {
            h.push_str(&format!(", pilot {:.2} kHz", s.pilot_hz / 1e3));
        }
        if self.state.rds() {
            h.push_str(&format!(", RDS {:.2} kHz", s.rds_hz / 1e3));
        }
        h
    }

    fn forget(&mut self) {
        self.state = FmState::default();
        self.seconds = 0;
        self.last = None;
        self.rows.clear();
        self.cleared = true;
        if let Some(m) = self.meter.as_mut() {
            m.reset();
        }
    }
}

impl Decoder for FmMonitorDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "fm_monitor.monitor"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::OverDeviation => self.threshold_khz = OVER_DEVIATION.read(id, value)?,
            DecoderSetting::Limit => self.limit = LIMIT.read(id, value)?,
            DecoderSetting::PilotMargin => self.margin_khz = PILOT_MARGIN.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::OverDeviation, Value::F64(self.threshold_khz)),
            (DecoderSetting::Limit, Value::Choice(self.limit.index())),
            (DecoderSetting::PilotMargin, Value::F64(self.margin_khz)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &[]
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        Err(CommandError::NoSuchId { id })
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        let front = Front::new(input.rate, WORK_HZ, PASS_HZ, SKIP_S);
        self.meter = Some(FmMeter::new(front.rate()));
        self.front = Some(front);
        self.dial_hz = input.tuned_hz;
        self.forget();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        let (Some(front), Some(meter)) = (self.front.as_mut(), self.meter.as_mut()) else {
            return;
        };
        let mut done = Vec::new();
        meter.push(front.process(x), |s| done.push(s));
        for s in done {
            self.second(s);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&FM_MONITOR_LOG);
        }
        for r in self.rows.drain(..) {
            FM_MONITOR_LOG.put(
                p,
                &format!("{:06}", r.serial),
                &FmMonitorLogRow {
                    signal_time_s: r.time_s,
                    kind: r.kind,
                    identity: &r.identity,
                    text: &r.text,
                },
            );
        }
        let headline = self.headline();
        let pilot = self.state.pilot();
        let (sca67, sca92) = self.state.sca();
        let limit = self.limit.hz();
        let s = self.last.as_ref();
        let st = &mut self.status;
        st.lock_state = match (self.started, pilot) {
            (false, _) => LockState::Off,
            (true, false) => LockState::Searching,
            (true, true) => LockState::Locked,
        };
        st.level_dbfs = s.map(|s| s.level_db as f32);
        st.offset_hz = s.map(|s| s.offset_hz);
        st.summary.clear();
        st.summary.push_str(match (self.started, s.is_some()) {
            (false, _) => "FM monitor: waiting for samples",
            (true, false) => "FM monitor: measuring the first second",
            _ => &headline,
        });
        let khz = |v: f64| v / 1e3;
        p.put(&FmMonitorFields {
            peak_deviation_khz: s.map(|s| khz(s.peak_hz)),
            rms_deviation_khz: s.map(|s| khz(s.rms_hz)),
            mpx_power_dbr: s.map(Second::mpx_dbr),
            mpx_power_60s_dbr: self.state.mpx_60s_dbr(),
            carrier_offset_khz: s.map(|s| khz(s.offset_hz)),
            audio_khz: s.map(|s| khz(s.audio_hz)),
            pilot_khz: s.map(|s| khz(s.pilot_hz)),
            pilot_pct: s.map(|s| 100.0 * s.pilot_hz / limit),
            pilot: pilot_range(pilot, s.map_or(0.0, |s| s.pilot_hz), limit, 1e3 * self.margin_khz),
            stereo_band_khz: s.map(|s| khz(s.stereo_hz)),
            stereo: pilot && s.is_some_and(Second::stereo),
            rds_khz: s.map(|s| khz(s.rds_hz)),
            rds: self.state.rds(),
            sca67_db: s.filter(|_| sca67).map(|s| s.sca67_db),
            sca92_db: s.filter(|_| sca92).and_then(|s| s.sca92_db),
            over_deviation_s: i64::try_from(self.state.over_deviation_s).unwrap_or(i64::MAX),
            level_db: s.map(|s| s.level_db),
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        0.25
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
        .add("monitor", DecoderComponent::new(FmMonitorDecoder::default()))
        .input("monitor")
        .output("monitor.events")
        .status_from("monitor")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "The FM monitor needs a sample rate of at least 200 ksps")
        .route(OVER_DEVIATION.key(), &["monitor.over_deviation_khz"])?
        .route(LIMIT.key(), &["monitor.deviation_limit"])?
        .route(PILOT_MARGIN.key(), &["monitor.pilot_margin_khz"])?
        .command("clear", CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("fm_monitor.monitor", || Box::new(DecoderComponent::new(FmMonitorDecoder::default())));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(FmGenerator::new(cfg.rate_hz, Mpx::from_values(&cfg.settings), cfg.seed)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, stereo with RDS", o, 0.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let p = crate::instrument::number(r, FmMonitorFields::PILOT_KHZ.key);
        let rds = crate::instrument::number(r, FmMonitorFields::RDS.key);
        vec![
            CheckLine::new(FmMonitorFields::PILOT_KHZ.key, (p - 7.5).abs() <= 0.6, format!("{p:.2} kHz, 7.5 expected")),
            CheckLine::new(FmMonitorFields::RDS.key, rds == 1.0, format!("{rds}, RDS expected")),
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

/// The FM modulation monitor.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "FM modulation monitor",
    group: "Measurement",
    blurb: "One FM broadcast station once a second: peak and RMS deviation, MPX power, carrier offset, pilot, stereo, RDS \
            and subcarrier levels, and the seconds over a threshold.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Wfm, 200e3)),
        min_hz: 64e6,
        max_hz: 110e6,
        default_hz: 100e6,
        rate_hz: 250e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 200e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: FmMonitorSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

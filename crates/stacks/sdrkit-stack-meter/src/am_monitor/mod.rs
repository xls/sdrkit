//! AM modulation monitor (`am_monitor`): reads one AM broadcast station (LW, MW, SW) once a second as a modulation monitor
//! does: the carrier offset and level, the modulation depth (RMS, as a sine), the positive and negative peaks against the
//! mean carrier, the asymmetry, mains hum, the audio bandwidth and over-modulation. It reads what arrives after the radio
//! and the path, fading and noise included, and judges no rule. A carrier counts only when the carrier test passes: 5
//! adjacent bins of one FFT hold more than 1.2 % of the channel power.
//!
//! Document for the quantities: ITU-R Report BS.2433, an AM signal is `carrier x (1 + Z(t))` with `Z` limited to -1.

pub mod gen;
pub mod measure;

use crate::instrument::Front;
use crate::typed::{
    AmCarrierChange, AmMonitorCarrierEvent, AmMonitorFields, AmMonitorLogRow, AmMonitorOverModulationEvent,
    AM_MONITOR_CARRIER, AM_MONITOR_LOG, AM_MONITOR_OVER_MODULATION,
};
use gen::{AmGenerator, AmSignal};
use measure::{AmMeter, AmState, CarrierChange, Second};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Status, Tuning,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kind, Outbox,
    Port, RateFrom, StackGraph,
};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "am_monitor";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 48e3;
/// The band the front end keeps, Hz.
pub const PASS_HZ: f64 = 12e3;
/// The stretch dropped at the start while the filters fill, s.
pub const SKIP_S: f64 = 0.1;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the monitor measures.
pub const NOTE_TEXT: &str = "Readings of the signal as it arrives after the radio and the path, once a second: no rule is \
judged. A carrier counts when 5 adjacent bins of one FFT hold more than 1.2 % of the channel power. Depth is the depth of a \
sine of the same RMS; peaks are the third largest and third smallest envelope values of 20 ms blocks against the mean \
carrier. Not confirmed by a public document: the 3 % zero level, the hum and bandwidth rules.";

/// The share of a second at zero carrier above which it counts as over-modulated.
pub const OVER_MOD_SHARE: Setting<field::F64> =
    Setting::new("over_mod_share_pct", "Over-modulation: share of a second at zero carrier", "%", 0.5)
        .range(0.1, 5.0, 0.0)
        .help("A second with more of its blocks at zero carrier than this is counted and logged.");
/// The drop below the best recent level at which the carrier counts as lost.
pub const CARRIER_LOSS: Setting<field::F64> =
    Setting::new("carrier_loss_db", "Carrier lost below the best level by", "dB", 12.0)
        .range(6.0, 30.0, 0.0)
        .help("The carrier counts as lost when its level falls this far below the best level of about the last minute.");

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The over-modulation share.
        OverModShare = OVER_MOD_SHARE,
        /// The carrier loss.
        CarrierLoss = CARRIER_LOSS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `am_monitor` stack.
    pub enum AmMonitorSetting {
        /// The over-modulation share.
        OverModShare = OVER_MOD_SHARE,
        /// The carrier loss.
        CarrierLoss = CARRIER_LOSS,
        /// The first tone depth of the test signal.
        GenDepth1 = gen::GEN_DEPTH1,
        /// The second tone depth of the test signal.
        GenDepth2 = gen::GEN_DEPTH2,
        /// The first tone of the test signal.
        GenTone1 = gen::GEN_TONE1_HZ,
        /// The second tone of the test signal.
        GenTone2 = gen::GEN_TONE2_HZ,
        /// The asymmetry of the test signal.
        GenAsymmetry = gen::GEN_ASYMMETRY,
        /// The start of the cut.
        GenCutFrom = gen::GEN_CUT_FROM_S,
        /// The end of the cut.
        GenCutUntil = gen::GEN_CUT_UNTIL_S,
        /// Clipping at zero carrier.
        GenClip = gen::GEN_CLIP,
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

/// The `am_monitor` decoder: the front end, the meter of each second and the carrier rules.
pub struct AmMonitorDecoder {
    front: Option<Front>,
    meter: Option<AmMeter>,
    state: AmState,
    over_mod_share: f64,
    carrier_loss: f64,
    dial_hz: f64,
    seconds: u64,
    last: Option<Second>,
    rows: Vec<Row>,
    serial: u64,
    cleared: bool,
    events: Vec<Event>,
    started: bool,
    headline: String,
    status: Status,
}

impl Default for AmMonitorDecoder {
    fn default() -> Self {
        AmMonitorDecoder {
            front: None,
            meter: None,
            state: AmState::default(),
            over_mod_share: OVER_MOD_SHARE.default,
            carrier_loss: CARRIER_LOSS.default,
            dial_hz: 0.0,
            seconds: 0,
            last: None,
            rows: Vec::new(),
            serial: 0,
            cleared: false,
            events: Vec::new(),
            started: false,
            headline: String::new(),
            status: Status::default(),
        }
    }
}

/// The headline of a second.
pub fn headline(s: Option<&Second>, on: bool) -> String {
    match s {
        Some(s) if on && s.present => format!(
            "carrier {:+.1} Hz off, modulation {:.0} % (+{:.0} / -{:.0} %), audio to {}",
            s.offset_hz,
            s.depth_pct,
            s.peak_pos_pct,
            s.peak_neg_pct,
            s.bandwidth_hz
                .map_or_else(|| "-".to_string(), |b| format!("{:.1} kHz", b / 1000.0))
        ),
        _ => "no carrier".to_string(),
    }
}

impl AmMonitorDecoder {
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
        let c = self.state.push(&s, self.carrier_loss, self.over_mod_share);
        if let Some(change) = c.carrier {
            let (word, choice) = match change {
                CarrierChange::Present => ("present", AmCarrierChange::Present),
                CarrierChange::Lost => ("lost", AmCarrierChange::Lost),
                CarrierChange::Returned => ("returned", AmCarrierChange::Returned),
            };
            let text = format!("carrier {word}, {:+.1} Hz off, level {:.1} dB", s.offset_hz, s.carrier_db);
            self.events.push(AM_MONITOR_CARRIER.event(
                t,
                self.dial_hz,
                &AmMonitorCarrierEvent {
                    text: &text,
                    change: choice,
                    carrier_offset_hz: s.offset_hz,
                    carrier_db: s.carrier_db,
                },
            ));
            self.row(t, "Carrier", word.to_string(), text);
        }
        if c.over_modulated {
            let text = format!(
                "{:.2} % of the second at zero carrier, positive peak {:.0} %",
                s.over_mod_pct, s.peak_pos_pct
            );
            self.events.push(AM_MONITOR_OVER_MODULATION.event(
                t,
                self.dial_hz,
                &AmMonitorOverModulationEvent {
                    text: &text,
                    over_mod_pct: s.over_mod_pct,
                    peak_pos_pct: s.peak_pos_pct,
                },
            ));
            self.row(t, "Over-modulation", format!("{:.2} %", s.over_mod_pct), text);
        }
        self.headline = headline(Some(&s), self.state.on());
        self.last = Some(s);
    }

    fn forget(&mut self) {
        self.state = AmState::default();
        self.seconds = 0;
        self.last = None;
        self.rows.clear();
        self.cleared = true;
        self.headline.clear();
        if let Some(m) = self.meter.as_mut() {
            m.reset();
        }
    }
}

impl Decoder for AmMonitorDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "am_monitor.monitor"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::OverModShare => self.over_mod_share = OVER_MOD_SHARE.read(id, value)?,
            DecoderSetting::CarrierLoss => self.carrier_loss = CARRIER_LOSS.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::OverModShare, Value::F64(self.over_mod_share)),
            (DecoderSetting::CarrierLoss, Value::F64(self.carrier_loss)),
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
        self.meter = Some(AmMeter::new(front.rate()));
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
            p.clear_rows(&AM_MONITOR_LOG);
        }
        for r in self.rows.drain(..) {
            AM_MONITOR_LOG.put(
                p,
                &format!("{:06}", r.serial),
                &AmMonitorLogRow {
                    signal_time_s: r.time_s,
                    kind: r.kind,
                    identity: &r.identity,
                    text: &r.text,
                },
            );
        }
        let on = self.state.on();
        let s = self.last.as_ref().filter(|s| s.present);
        let st = &mut self.status;
        st.lock_state = match (self.started, on) {
            (false, _) => LockState::Off,
            (true, false) => LockState::Searching,
            (true, true) => LockState::Locked,
        };
        st.level_dbfs = self.last.as_ref().map(|s| s.carrier_db as f32);
        st.offset_hz = s.map(|s| s.offset_hz);
        st.snr_db = None;
        st.summary.clear();
        st.summary.push_str(match (self.started, self.last.is_some()) {
            (false, _) => "AM monitor: waiting for samples",
            (true, false) => "AM monitor: measuring the first second",
            _ => &self.headline,
        });
        p.put(&AmMonitorFields {
            carrier_offset_hz: s.map(|s| s.offset_hz),
            carrier_db: self.last.as_ref().map(|s| s.carrier_db),
            depth_pct: s.map(|s| s.depth_pct),
            peak_pos_pct: s.map(|s| s.peak_pos_pct),
            peak_neg_pct: s.map(|s| s.peak_neg_pct),
            asymmetry_pct: s.map(|s| s.asymmetry_pct),
            hum_db: s.and_then(|s| s.hum.map(|h| h.1)),
            hum_hz: s.and_then(|s| s.hum.map(|h| h.0)),
            bandwidth_khz: s.and_then(|s| s.bandwidth_hz.map(|b| b / 1000.0)),
            over_mod_pct: s.map(|s| s.over_mod_pct),
            over_mod_s: i64::try_from(self.state.over_mod_s).unwrap_or(i64::MAX),
            carrier: on,
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
        .add("monitor", DecoderComponent::new(AmMonitorDecoder::default()))
        .input("monitor")
        .output("monitor.events")
        .status_from("monitor")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "The AM monitor needs a sample rate of at least 24 ksps")
        .route(OVER_MOD_SHARE.key(), &["monitor.over_mod_share_pct"])?
        .route(CARRIER_LOSS.key(), &["monitor.carrier_loss_db"])?
        .command("clear", CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("am_monitor.monitor", || Box::new(DecoderComponent::new(AmMonitorDecoder::default())));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(AmGenerator::new(cfg.rate_hz, AmSignal::from_values(&cfg.settings))))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, tones of 50 % and 20 %", o, 0.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let d = crate::instrument::number(r, AmMonitorFields::DEPTH_PCT.key);
        let on = crate::instrument::number(r, AmMonitorFields::CARRIER.key);
        vec![
            CheckLine::new(AmMonitorFields::CARRIER.key, on == 1.0, format!("{on}, a carrier expected")),
            CheckLine::new(
                AmMonitorFields::DEPTH_PCT.key,
                (d - 53.9).abs() <= 6.0,
                format!("{d:.1} %, 53.9 expected"),
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

/// The AM modulation monitor.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "AM modulation monitor",
    group: "Measurement",
    blurb: "One AM broadcast station once a second: carrier offset and level, modulation depth, peaks, asymmetry, mains \
            hum, audio bandwidth and over-modulation.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Am, 10e3)),
        min_hz: 0.148e6,
        max_hz: 30e6,
        default_hz: 1e6,
        rate_hz: 250e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 12e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: AmMonitorSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

//! Power-line and impulse noise locator (`noise_locator`): the impulses of the analysis band (about 20 kHz at the centre),
//! their rate and level, and the repetition rate when they have one, with a class: twice a mains frequency (an arc at each
//! half-cycle), a mains frequency (a fault on one half-cycle), another steady rate (a switching supply or a clock), or none
//! (static or lightning). It measures and decodes nothing and does not locate the source: the level can guide a search
//! with a portable antenna. Impulses wider than a few ms, and repetitions below about 17 Hz, are not seen.
//!
//! A change of frequency keeps the log of patterns: the noise of a source is wideband and the log belongs to the source,
//! not to the tuned frequency.

pub mod gen;

use crate::instrument::{Front, InputLevel, Log, Tally};
use crate::typed::{
    NoiseLocatorChange, NoiseLocatorClass, NoiseLocatorFields, NoiseLocatorPatternEvent,
    NoiseLocatorPatternRow, WatchFields, NOISE_LOCATOR_PATTERN, NOISE_LOCATOR_PATTERNS,
};
use gen::ImpulseGenerator;
use sdrkit_algo_dsp::repeat::{PairSpacing, SpacingConfig};
use sdrkit_algo_dsp::slicer::{EnvelopeSlicer, Slice, SlicerConfig};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::interface::Seconds;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, Receiver, Record,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kept, Kind, Outbox, Port, RateFrom, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "noise_locator";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 48e3;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.2;
/// The time that learns the noise, s.
pub const LEARN_S: f64 = 0.3;
/// The time constant of the noise once learned, s.
pub const NOISE_TAU_S: f64 = 0.5;
/// The time between two reports, s.
pub const REPORT_EVERY_S: f64 = 2.0;
/// The fewest impulses for a search of the repetition.
pub const MIN_IMPULSES: usize = 30;
/// Reports in a row without the rate after which a pattern is gone.
pub const GONE_AFTER: u32 = 3;
/// The weight of a new rate in the rate of a pattern.
pub const RATE_WEIGHT: f64 = 0.2;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the impulses of the analysis band and their repetition rate; decodes nothing and \
does not locate the source. An impulse is over the mean noise envelope plus 6.5 standard deviations; the rate is the \
strongest pair spacing. Levels are over the noise of the analysis band, about 9 dB over a wideband figure. No public \
document confirms these rules.";

/// The mains frequencies that name a class.
pub const MAINS_HZ: Setting<field::Text> = Setting::new("mains_hz", "Mains frequencies", "Hz", "50, 60")
    .help("The mains frequencies tested, separated by commas; a rate near one or twice one names a power-line class.");
/// The impulse threshold.
pub const SIGMAS: Setting<field::F64> =
    Setting::new("sigmas", "Impulse threshold over the noise", "sigma", 6.5).range(3.0, 20.0, 0.0);
/// The impulse window.
pub const WINDOW_S: Setting<field::F64> =
    Setting::new("window_s", "Impulse window", "s", 10.0).range(2.0, 60.0, 0.0);
/// The analysis band.
pub const ANALYSIS_KHZ: Setting<field::F64> =
    Setting::new("analysis_khz", "Analysis band", "kHz", 20.0)
        .range(2.0, 100.0, 0.0)
        .help("Twice the pass edge of the filter before the impulse detector.");

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The mains frequencies.
        MainsHz = MAINS_HZ,
        /// The threshold.
        Sigmas = SIGMAS,
        /// The window.
        WindowS = WINDOW_S,
        /// The analysis band.
        AnalysisKhz = ANALYSIS_KHZ,
    }
}

sdrkit_core::settings! {
    /// The settings of the `noise_locator` stack.
    pub enum NoiseLocatorSetting {
        /// The mains frequencies.
        MainsHz = MAINS_HZ,
        /// The threshold.
        Sigmas = SIGMAS,
        /// The window.
        WindowS = WINDOW_S,
        /// The analysis band.
        AnalysisKhz = ANALYSIS_KHZ,
        /// The impulse rate of the test signal.
        GenRateHz = gen::GEN_RATE_HZ,
        /// The jitter.
        GenJitterUs = gen::GEN_JITTER_US,
        /// The width.
        GenWidthUs = gen::GEN_WIDTH_US,
        /// The peak.
        GenPeakDb = gen::GEN_PEAK_DB,
        /// The spread.
        GenSpreadDb = gen::GEN_SPREAD_DB,
        /// The missing share.
        GenDrop = gen::GEN_DROP,
        /// The random rate.
        GenRandomRate = gen::GEN_RANDOM_RATE,
        /// The random peak.
        GenRandomDb = gen::GEN_RANDOM_DB,
        /// The rate drift.
        GenRateDriftHzS = gen::GEN_RATE_DRIFT_HZ_S,
        /// The reference noise.
        GenNoiseSigma = gen::GEN_NOISE_SIGMA,
    }
}

/// The `reset` command: the noise is learned again and the pattern dropped; the table stays.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Learns the noise again and drops the impulses and the pattern; the table stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The class of the rate `f` against the mains frequencies `mains`, with the mains frequency that it names.
pub fn classify(f: f64, mains: &[f64]) -> (NoiseLocatorClass, Option<f64>) {
    let near = |a: f64, b: f64| (a - b).abs() <= 0.02 * b + 0.3;
    if let Some(&m) = mains.iter().find(|&&m| near(f, 2.0 * m)) {
        return (NoiseLocatorClass::TwiceMains, Some(m));
    }
    if let Some(&m) = mains.iter().find(|&&m| near(f, m)) {
        return (NoiseLocatorClass::Mains, Some(m));
    }
    (NoiseLocatorClass::Steady, None)
}

fn same_rate(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.02 * b + 0.3
}

/// The present pattern.
#[derive(Clone, Copy, Debug)]
struct Pattern {
    hz: f64,
    strength: f64,
    missing: u32,
}

/// The readings of the last report.
#[derive(Clone, Copy, Debug, Default)]
struct Readings {
    rate: Option<f64>,
    level: Option<f64>,
    spread: Option<f64>,
}

/// The `noise_locator` decoder.
pub struct NoiseLocatorDecoder {
    mains: Vec<f64>,
    mains_text: String,
    sigmas: f64,
    window_s: f64,
    analysis_khz: f64,
    rate: f64,
    front: Option<Front>,
    slicer: Option<EnvelopeSlicer>,
    work: f64,
    slices: Vec<Slice>,
    impulses: VecDeque<(f64, f64)>,
    total: u64,
    spacing: PairSpacing,
    pattern: Option<Pattern>,
    pending: Option<f64>,
    readings: Readings,
    next_report: f64,
    t: f64,
    started: bool,
    tally: Tally,
    level: InputLevel,
    log: Log<NoiseLocatorPatternRow>,
    events: Vec<Event>,
    status: Status,
    dial_hz: f64,
}

impl Default for NoiseLocatorDecoder {
    fn default() -> Self {
        NoiseLocatorDecoder {
            mains: parse_list(MAINS_HZ.default),
            mains_text: MAINS_HZ.default.to_string(),
            sigmas: SIGMAS.default,
            window_s: WINDOW_S.default,
            analysis_khz: ANALYSIS_KHZ.default,
            rate: 0.0,
            front: None,
            slicer: None,
            work: WORK_HZ,
            slices: Vec::new(),
            impulses: VecDeque::new(),
            total: 0,
            spacing: PairSpacing::new(SpacingConfig::impulses()),
            pattern: None,
            pending: None,
            readings: Readings::default(),
            next_report: SKIP_S + REPORT_EVERY_S,
            t: SKIP_S,
            started: false,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
            dial_hz: 0.0,
        }
    }
}

/// The numbers of a comma-separated list; the ones that do not read are left out.
fn parse_list(text: &str) -> Vec<f64> {
    text.split(',')
        .filter_map(|x| x.trim().parse::<f64>().ok())
        .filter(|&x| x > 0.0)
        .collect()
}

impl NoiseLocatorDecoder {
    fn build(&mut self) {
        let front = Front::new(self.rate, WORK_HZ, self.analysis_khz * 500.0, SKIP_S);
        self.work = front.rate();
        self.front = Some(front);
        self.t = SKIP_S;
        self.reset();
    }

    fn reset(&mut self) {
        let pos = ((self.t - SKIP_S) * self.work).round() as u64;
        let mut s = EnvelopeSlicer::new(SlicerConfig {
            learn: (LEARN_S * self.work).round() as u64,
            sigmas: self.sigmas,
            tau: NOISE_TAU_S * self.work,
        });
        s.reset(pos);
        self.slicer = Some(s);
        self.impulses.clear();
        self.pattern = None;
        self.pending = None;
        self.readings = Readings::default();
        self.next_report = self.t + REPORT_EVERY_S;
    }

    fn clear(&mut self) {
        self.reset();
        self.total = 0;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn row(&mut self, kind: NoiseLocatorChange, p: &Pattern) {
        let (class, _) = classify(p.hz, &self.mains);
        let row = NoiseLocatorPatternRow {
            time_s: Seconds(self.t),
            kind,
            repetition_hz: p.hz,
            class,
            impulse_rate: self.readings.rate.unwrap_or(0.0),
            level_db: self.readings.level.unwrap_or(0.0),
            strength_pct: 100.0 * p.strength,
        };
        self.events.push(NOISE_LOCATOR_PATTERN.event(
            self.t,
            self.dial_hz,
            &NoiseLocatorPatternEvent {
                time_s: row.time_s,
                kind: row.kind,
                repetition_hz: row.repetition_hz,
                class: row.class,
                impulse_rate: row.impulse_rate,
                level_db: row.level_db,
                strength_pct: row.strength_pct,
            },
        ));
        self.log.push(row);
    }

    /// The report of every 2 s: the rate and levels of the window and the repetition rule.
    fn judge(&mut self) {
        let t = self.t;
        let span = self.window_s.min(t - SKIP_S).max(1e-9);
        let n = self.impulses.len();
        let mut levels: Vec<f64> = self.impulses.iter().map(|&(_, l)| l).collect();
        levels.sort_by(f64::total_cmp);
        let at = |f: f64| levels[((f * levels.len() as f64) as usize).min(levels.len() - 1)];
        self.readings = Readings {
            rate: Some(n as f64 / span),
            level: (n > 0).then(|| at(0.5)),
            spread: (n > 0).then(|| at(0.75) - at(0.25)),
        };
        if n > 0 {
            self.tally.energy(t);
        }
        let times: Vec<f64> = self.impulses.iter().map(|&(t, _)| t).collect();
        let found = if n >= MIN_IMPULSES {
            self.spacing.find(&times)
        } else {
            None
        };
        match (found, self.pattern) {
            (Some(r), Some(mut p)) if same_rate(r.rate_hz, p.hz) => {
                p.hz += RATE_WEIGHT * (r.rate_hz - p.hz);
                p.strength = r.strength;
                p.missing = 0;
                self.pattern = Some(p);
                self.pending = None;
            }
            (Some(r), current) => {
                if self.pending.is_some_and(|q| same_rate(r.rate_hz, q)) {
                    if let Some(old) = current {
                        self.row(NoiseLocatorChange::Gone, &old);
                    }
                    let p = Pattern {
                        hz: r.rate_hz,
                        strength: r.strength,
                        missing: 0,
                    };
                    self.pattern = Some(p);
                    self.pending = None;
                    self.row(NoiseLocatorChange::Found, &p);
                } else {
                    self.pending = Some(r.rate_hz);
                    self.miss();
                }
            }
            (None, _) => {
                self.pending = None;
                self.miss();
            }
        }
        if self.pattern.is_some() {
            self.tally.good(t);
        }
    }

    fn miss(&mut self) {
        let Some(mut p) = self.pattern else {
            return;
        };
        p.missing += 1;
        if p.missing >= GONE_AFTER {
            self.pattern = None;
            self.row(NoiseLocatorChange::Gone, &p);
        } else {
            self.pattern = Some(p);
        }
    }
}

impl Decoder for NoiseLocatorDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "noise_locator.locator"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::MainsHz => {
                self.mains_text = MAINS_HZ.read(id, value)?.to_string();
                self.mains = parse_list(&self.mains_text);
            }
            DecoderSetting::Sigmas => {
                self.sigmas = SIGMAS.read(id, value)?;
                if let Some(s) = self.slicer.as_mut() {
                    s.set_sigmas(self.sigmas);
                }
            }
            DecoderSetting::WindowS => self.window_s = WINDOW_S.read(id, value)?,
            DecoderSetting::AnalysisKhz => {
                self.analysis_khz = ANALYSIS_KHZ.read(id, value)?;
                if self.rate > 0.0 {
                    self.build();
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (
                DecoderSetting::MainsHz,
                Value::Text(self.mains_text.clone()),
            ),
            (DecoderSetting::Sigmas, Value::F64(self.sigmas)),
            (DecoderSetting::WindowS, Value::F64(self.window_s)),
            (DecoderSetting::AnalysisKhz, Value::F64(self.analysis_khz)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.reset();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
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
        self.rate = input.rate;
        self.dial_hz = input.tuned_hz;
        self.started = false;
        self.build();
        self.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.level.push(x);
        let (Some(mut front), Some(mut slicer)) = (self.front.take(), self.slicer.take()) else {
            return;
        };
        let step = (self.work * REPORT_EVERY_S).max(1.0) as usize;
        for piece in front.process(x).chunks(step) {
            self.slices.clear();
            slicer.process(piece, &mut self.slices);
            self.t += piece.len() as f64 / self.work;
            for s in &self.slices {
                self.impulses
                    .push_back((SKIP_S + s.peak_index as f64 / self.work, s.level_db()));
                self.total += 1;
            }
            let from = self.t - self.window_s;
            while self.impulses.front().is_some_and(|&(a, _)| a < from) {
                self.impulses.pop_front();
            }
            if self.t >= self.next_report {
                self.next_report += REPORT_EVERY_S;
                self.judge();
            }
        }
        self.front = Some(front);
        if self.slicer.is_none() {
            self.slicer = Some(slicer);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(&NOISE_LOCATOR_PATTERNS, p);
        let r = self.readings;
        let pat = self.pattern;
        let (class, mains) = pat.map_or((NoiseLocatorClass::NoRate, None), |p| {
            classify(p.hz, &self.mains)
        });
        let class_text = match class {
            NoiseLocatorClass::TwiceMains => "power-line noise, twice the mains,",
            NoiseLocatorClass::Mains => "half-wave power-line noise,",
            _ => "steady impulses",
        };
        let headline = match (pat, r.rate) {
            _ if !self.started || r.rate.is_none() => "waiting for data".to_string(),
            (Some(p), _) => format!(
                "{class_text} at {:.2} Hz, {:.1} impulses/s, {:.1} dB over the noise",
                p.hz,
                r.rate.unwrap_or(0.0),
                r.level.unwrap_or(0.0)
            ),
            (None, Some(rate)) if rate > 0.0 => format!(
                "impulses without a repetition ({rate:.1} /s, {:.1} dB): static or lightning",
                r.level.unwrap_or(0.0)
            ),
            _ => "no impulses".to_string(),
        };
        let level = self.level.read();
        p.put(&NoiseLocatorFields {
            impulse_rate: r.rate,
            impulse_level_db: r.level,
            impulse_spread_db: r.spread,
            repetition_hz: pat.map(|p| p.hz),
            class,
            mains_found_hz: mains,
            strength_pct: pat.map(|p| 100.0 * p.strength),
            impulses: self.total as i64,
        });
        let st = &mut self.status;
        st.lock_state = self.tally.lock(self.t);
        st.level_dbfs = level.map(|l| l as f32);
        st.offset_hz = None;
        st.snr_db = None;
        st.summary = headline;
        p.put(&WatchFields {
            state: self.tally.state(self.t),
            good: i64::try_from(self.tally.good).unwrap_or(i64::MAX),
            bad: i64::try_from(self.tally.bad).unwrap_or(i64::MAX),
            level_dbfs: level,
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        NoiseLocatorDecoder::clear(self);
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        self.total = 0;
        self.tally = Tally::default();
        self.events.clear();
        if !kept.table(NOISE_LOCATOR_PATTERNS.key) {
            self.log.clear();
        }
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add(
            "locator",
            DecoderComponent::new(NoiseLocatorDecoder::default()),
        )
        .input("locator")
        .output("locator.events")
        .status_from("locator")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The noise locator needs a sample rate of at least 24 ksps",
    );
    for key in [
        MAINS_HZ.key(),
        SIGMAS.key(),
        WINDOW_S.key(),
        ANALYSIS_KHZ.key(),
    ] {
        g = g.route(key, &[&format!("locator.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("locator", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("noise_locator.locator", || {
        Box::new(DecoderComponent::new(NoiseLocatorDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(ImpulseGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
        cfg.seed,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, impulses at 100 per second", o, 0.0);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::instrument::number(r, NoiseLocatorFields::REPETITION_HZ.key);
        vec![CheckLine::new(
            NoiseLocatorFields::REPETITION_HZ.key,
            (f - 100.0).abs() <= 0.65,
            format!("{f:.2} Hz, 100 sent"),
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

/// The power-line and impulse noise locator.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Power-line and impulse noise locator",
    group: "Measurement",
    blurb: "Impulse rate, level and repetition rate of the noise in a band, with a class: twice the mains, the mains, \
            another steady rate, or none.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 0.1e6,
        max_hz: 1e9,
        ..Tuning::anywhere(0.0, 250e3, MIN_RATE_HZ, 20e3)
    },
    submodes: &[],
    settings: NoiseLocatorSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

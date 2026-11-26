//! Sferics (`sferics`): counts the radio impulses of lightning over the receiver's own noise floor. Impulses of a mains train
//! (lamp dimmers, chargers, power lines) are recognised by their grid and left out. The stack gives the strikes and flashes,
//! the strike rates, the strikes of each of the last 60 minutes and a storm trend. It says when and how strong, not where.

pub mod count;
pub mod gen;

use crate::family::{self, REPORT_S};
use count::{Counter, Phase, StormChange, Strike, MINUTES};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use crate::typed::{
    SfericsFields, SfericsStormEvent, SfericsStrikeRow, SFERICS_PER_MINUTE, SFERICS_STORM,
    SFERICS_STRIKES,
};
use sdrkit_core::interface::Time;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::Setting;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, Event, LockProfile, LockState, Record,
    ResultsDecl, Schema, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Status,
    Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, RunContext, StackGraph,
};
use sdrkit_util_objects::shaped::{rows_of, RowOut, Shaped};
use sdrkit_util_objects::Seen;
use std::sync::Arc;

pub use gen::{make_generator, Scene, SfericsGenerator};

/// The stack id.
pub const ID: &str = "sferics";
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 20e3;
/// The rate the stack asks for, Hz.
pub const ASK_RATE: f64 = 2e6;
/// The dial of the test signal, Hz. Any frequency below 30 MHz hears sferics.
pub const TEST_DIAL_HZ: f64 = 3e6;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: every level, time and rule of the detector (the 25 µs power \
     average, the floor as the median of 1 ms means over 4 s, the end 3 dB under the threshold after 100 µs, 5 ms as too long \
     for lightning, the 0.5 s commit delay and flash gap), the recognition of mains trains (10 ms and 1/120 s grids, 24 of \
     32 impulses) and the storm trend (its halves of 5 minutes, its rates and its 2.5 dB and 6 dB limits). The strength is \
     relative to this receiver's floor and antenna; it is not calibrated, and irregular impulsive noise counts as lightning.";

/// The start level over the floor.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Threshold over the floor", "dB", 10.0)
        .range(6.0, 30.0, 0.5)
        .help("An impulse starts when the 25 µs power average rises this far over the floor.");
/// Leave out the impulses of mains trains.
pub const MAINS_FILTER: Setting<field::Bool> =
    Setting::new("mains_filter", "Leave out mains trains", "", true)
        .help("Impulses on a 10 ms or 1/120 s grid are counted as mains, not as strikes.");
/// The joining time.
pub const MERGE_MS: Setting<field::F64> = Setting::new("merge_ms", "Merge time", "ms", 0.5)
    .range(0.1, 5.0, 0.1)
    .help("An impulse that starts this soon after the end of the one before joins it.");
/// The UTC of sample 0 of a file, 0 for the host clock.
pub const START_UTC: Setting<field::F64> = Setting::new(
    "start_utc",
    "UTC of sample 0 (files; 0 = host clock)",
    "s",
    0.0,
)
.range(0.0, 1e11, 1.0);
/// The mean strike rate of the test signal.
pub const GEN_STRIKE_RATE: Setting<field::F64> = Setting::new(
    "gen_strike_rate",
    "Test signal: strokes a second",
    "1/s",
    1.5,
)
.range(0.0, 50.0, 0.1)
.section("Test signal")
.help("Flashes of 1 to 3 strokes come at random gaps of 1.7 / rate seconds on average.")
.advanced();
/// What the test signal holds.
pub const GEN_SCENE: Setting<field::Choice<Scene>> =
    Setting::choice("gen_scene", "Test signal: scene", Scene::Storm)
        .section("Test signal")
        .advanced();
/// The mains frequency of the test signal's train.
pub const GEN_MAINS_HZ: Setting<field::Hz> =
    Setting::new("gen_mains_hz", "Test signal: mains frequency", "Hz", 50.0)
        .range(45.0, 65.0, 1.0)
        .section("Test signal")
        .help("The train has two impulses a period: 100 a second at 50 Hz, 120 at 60 Hz.")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`SfericsDecoder`].
    pub enum DecoderSetting {
        /// The start level.
        Threshold = THRESHOLD,
        /// The mains filter.
        MainsFilter = MAINS_FILTER,
        /// The joining time.
        Merge = MERGE_MS,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the sferics stack.
    pub enum SfericsSetting {
        /// The start level.
        Threshold = THRESHOLD,
        /// The mains filter.
        MainsFilter = MAINS_FILTER,
        /// The joining time.
        Merge = MERGE_MS,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The strike rate of the test signal.
        GenStrikeRate = GEN_STRIKE_RATE,
        /// The scene of the test signal.
        GenScene = GEN_SCENE,
        /// The mains frequency of the test signal.
        GenMainsHz = GEN_MAINS_HZ,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Restarts the detectors.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Empties the tables and counters, and learns the floor and the mains train again.");

sdrkit_core::commands! {
    /// The commands of [`SfericsDecoder`].
    pub enum SfericsCommand {
        /// Restarts the detectors.
        Reset = RESET,
    }
}

/// The activity word of a rate of strikes per minute.
pub fn activity(per_min: f64) -> &'static str {
    if per_min < 0.2 {
        "none"
    } else if per_min < 3.0 {
        "low"
    } else if per_min < 15.0 {
        "moderate"
    } else {
        "high"
    }
}

/// The storm event of a change.
pub fn storm_event(c: &StormChange) -> Event {
    SFERICS_STORM.event(
        c.time_s,
        0.0,
        &SfericsStormEvent {
            storm_phase: c.trend.phase.key(),
            storm_db_change: c.trend.change_db,
            per_minute: c.trend.rate_b,
        },
    )
}

/// The sferics decoder: complex samples in, the counts and the storm events out.
pub struct SfericsDecoder {
    counter: Option<Counter>,
    threshold_db: f64,
    mains_filter: bool,
    merge_ms: f64,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    shown: u64,
    status: Status,
    cleared: bool,
}

impl Default for SfericsDecoder {
    fn default() -> Self {
        SfericsDecoder {
            counter: None,
            threshold_db: THRESHOLD.default,
            mains_filter: MAINS_FILTER.default,
            merge_ms: MERGE_MS.default,
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            shown: u64::MAX,
            status: Status::default(),
            cleared: false,
        }
    }
}

impl SfericsDecoder {
    fn start_utc(&self) -> f64 {
        if self.start_setting > 0.0 {
            self.start_setting
        } else {
            self.clock.now().floor()
        }
    }

    fn emit(&mut self, out: &mut Outbox) {
        if let Some(c) = &mut self.counter {
            for ch in c.take_changes() {
                out.messages(0).push(storm_event(&ch));
            }
        }
    }
}

impl Shaped for Strike {
    type Row = SfericsStrikeRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&SfericsStrikeRow {
            time_s: (self.time_s * 1e6).round() / 1e6,
            utc: Time(self.utc),
            peak_db: (self.peak_db * 10.0).round() / 10.0,
            width_us: self.width_us.round(),
            flash: i64::try_from(self.flash).unwrap_or(i64::MAX),
        });
    }
}

impl Decoder for SfericsDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "sferics.counter"
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
                self.threshold_db = THRESHOLD.read(id, value)?;
                if let Some(c) = &mut self.counter {
                    c.set_threshold(self.threshold_db);
                }
            }
            DecoderSetting::MainsFilter => {
                self.mains_filter = MAINS_FILTER.read(id, value)?;
                if let Some(c) = &mut self.counter {
                    c.set_mains_filter(self.mains_filter);
                }
            }
            DecoderSetting::Merge => {
                self.merge_ms = MERGE_MS.read(id, value)?;
                if let Some(c) = &mut self.counter {
                    c.set_merge_ms(self.merge_ms);
                }
            }
            DecoderSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = self.start_utc();
                if let Some(c) = &mut self.counter {
                    c.set_start_utc(self.start_utc);
                }
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::Threshold, Value::F64(self.threshold_db)),
            (DecoderSetting::MainsFilter, Value::Bool(self.mains_filter)),
            (DecoderSetting::Merge, Value::F64(self.merge_ms)),
            (DecoderSetting::StartUtc, Value::F64(self.start_setting)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        SfericsCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match SfericsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SfericsCommand::Reset => {
                if let Some(c) = &mut self.counter {
                    c.reset();
                }
                self.cleared = true;
            }
        }
        Ok(None)
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        let mut c = Counter::new(f.rate, self.threshold_db, self.merge_ms, self.mains_filter)
            .map_err(|_| ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            })?;
        self.start_utc = self.start_utc();
        c.set_start_utc(self.start_utc);
        self.counter = Some(c);
        self.shown = u64::MAX;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        if let Some(c) = &mut self.counter {
            c.process(x, first);
        }
        self.emit(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        if let Some(c) = &mut self.counter {
            c.finish();
        }
        self.emit(out);
    }
    fn report(&mut self, p: &mut Published) {
        let Some(c) = &self.counter else {
            return;
        };
        let now = c.now();
        let trend = c.trend();
        let per10 = c.per_min_10();
        let word = activity(per10);
        p.put(&SfericsFields {
            strikes: i64::try_from(c.strikes).unwrap_or(i64::MAX),
            flashes: i64::try_from(c.flashes).unwrap_or(i64::MAX),
            mains: i64::try_from(c.mains).unwrap_or(i64::MAX),
            too_long: i64::try_from(c.too_long).unwrap_or(i64::MAX),
            per_min_1: c.per_min_1(),
            per_min_10: (per10 * 100.0).round() / 100.0,
            floor_db: c.floor().map(|f| 10.0 * f.log10()),
            threshold_db: self.threshold_db,
            last_peak_db: c.last().map(|s| (s.peak_db * 10.0).round() / 10.0),
            last_ago_s: c.last().map(|s| now - s.time_s),
            mains_active: c.mains_active(),
            storm_phase: trend.phase.key(),
            storm_db_change: trend.change_db,
            activity: word,
            note: NOTE,
        });
        let written = c.strikes;
        if written != self.shown || std::mem::take(&mut self.cleared) {
            self.shown = written;
            rows_of(
                c.table().iter(),
                |s: &Strike, w| write!(w, "{:.6}", s.time_s),
                &SFERICS_STRIKES,
                now,
                p,
            );
        }
        if let Some(id) = p.results().decl().series_id(SFERICS_PER_MINUTE.key) {
            let h = c.per_minute();
            family::write_series(
                p.results_mut(),
                id,
                false,
                h.iter()
                    .enumerate()
                    .map(|(i, &n)| [i as f32 - (MINUTES - 1) as f32, n as f32]),
            );
        }
        let locked = c.last().is_some_and(|s| now - s.time_s <= count::TREND_S);
        let lock = if !c.floor_valid() {
            LockState::Searching
        } else if locked {
            LockState::Locked
        } else {
            LockState::Tracking
        };
        let (strikes, flashes, mains_on) = (c.strikes, c.flashes, c.mains_active());
        family::status(&mut self.status, lock, None, None, None, |s| {
            use std::fmt::Write;
            if lock == LockState::Searching {
                let _ = write!(s, "Sferics: learning the noise floor");
            } else {
                let _ = write!(
                    s,
                    "Sferics: {strikes} strikes in {flashes} flashes, {per10:.1} /min ({word})"
                );
                if trend.phase != Phase::None {
                    let _ = write!(s, ", storm {}", trend.phase.key());
                }
                if mains_on {
                    let _ = write!(s, ", mains train left out");
                }
            }
        });
        p.results_mut().set_status(&self.status);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        if let Some(c) = &mut self.counter {
            c.clear();
        }
        self.cleared = true;
    }
}

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("sferics", DecoderComponent::new(SfericsDecoder::default()))
        .input("sferics")
        .output("sferics.events")
        .status_from("sferics")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE, "Sferics need a sample rate of at least 20 ksps");
    for s in DecoderSetting::schema().settings() {
        let k = &s.field.key;
        g = g.route(k, &[&format!("sferics.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command(RESET.key, CommandTarget::of("sferics", RESET.key))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("sferics.counter", || {
        Box::new(DecoderComponent::new(SfericsDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, a storm", o, 0.0);
    c.seconds = o.seconds.max(8.0);
    c.expect_audio = Some(false);
    if !o.clean {
        c.snr_db = Some(25.0);
    }
    c.check = Some(|r| {
        let n = family::field(r, SfericsFields::STRIKES.key);
        vec![CheckLine::new(
            SfericsFields::STRIKES.key,
            n >= 2.0,
            format!("{n}, at least 2 expected"),
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

/// Sferics.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Sferics (lightning)",
    group: "Science, time and utility",
    blurb: "Counts the radio impulses of lightning as strikes and flashes: strike rates, a 60-minute histogram and a storm \
            trend, with mains impulse trains left out. Any band below 30 MHz; VLF and LF hear them best.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 3e3,
        max_hz: 30e6,
        default_hz: TEST_DIAL_HZ,
        rate_hz: ASK_RATE,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: true,
        radio_filter_hz: Some(1e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: SfericsSetting::schema,
    make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(make_generator),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

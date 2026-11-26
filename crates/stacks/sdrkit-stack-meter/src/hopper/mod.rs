//! Frequency-hopper analyser (`hopper`): follows the strongest signal of each short frame through its dwells and reads one
//! hopper: hop rate, dwell, guard, channel count, spacing and span. It measures and decodes nothing; the hop sequence is
//! neither kept nor predicted. Channels less than about 1.5 bins apart merge; a dwell of fewer than about ten frames reads
//! long; with two or more hoppers in the span the readings are not valid.

pub mod gen;

use crate::instrument::{InputLevel, Log, Tally};
use crate::typed::{
    HopperChangedEvent, HopperFields, HopperRow, WatchFields, HOPPER_CHANGED, HOPPER_HOPPERS,
};
use gen::HopperGenerator;
use sdrkit_algo_dsp::peak::{line_fit, log_parabolic};
use sdrkit_algo_dsp::shortframe::{Frame, ShortFrames};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, Receiver, Record,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "hopper";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The points of a frame.
pub const FFT_N: usize = 256;
/// A present frame within this many bins of the mean of the open dwell extends it.
pub const SAME_DWELL_BINS: f64 = 1.6;
/// The fewest frames of a dwell that is kept.
pub const MIN_FRAMES: usize = 3;
/// Dwells kept.
pub const DWELLS_KEPT: usize = 400;
/// The hopper is evaluated after every this many dwells.
pub const EVALUATE_EVERY: u64 = 20;
/// Start-to-start times up to this count as periods, s.
pub const MAX_PERIOD_S: f64 = 0.5;
/// Sorted dwell frequencies further apart than this many bins are different channels.
pub const SPLIT_BINS: f64 = 1.2;
/// The fewest periods of a hopper.
pub const MIN_PERIODS: usize = 20;
/// A change of the rate by more than this share makes a row.
pub const RATE_CHANGE: f64 = 0.05;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures one frequency hopper: hop rate, dwell, guard, channels, spacing and span; decodes \
nothing and does not record the hop sequence. The strongest bin of each 256-point frame is present 13 dB over the frame \
median. No public document confirms these rules.";

/// The level of the strongest bin over the frame median at which a frame is present.
pub const PRESENT_DB: Setting<field::F64> = Setting::new(
    "present_db",
    "Strongest bin over the frame median",
    "dB",
    13.0,
)
.range(6.0, 30.0, 0.0);
/// The dwells before a report.
pub const MIN_DWELLS: Setting<field::I64> =
    Setting::new("min_dwells", "Dwells before a report", "", 40).range(20.0, 400.0, 1.0);
/// The fewest channels of a hopper.
pub const MIN_CHANNELS: Setting<field::I64> =
    Setting::new("min_channels", "Fewest channels", "", 3).range(3.0, 16.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The present level.
        PresentDb = PRESENT_DB,
        /// The dwells before a report.
        MinDwells = MIN_DWELLS,
        /// The fewest channels.
        MinChannels = MIN_CHANNELS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `hopper` stack.
    pub enum HopperSetting {
        /// The present level.
        PresentDb = PRESENT_DB,
        /// The dwells before a report.
        MinDwells = MIN_DWELLS,
        /// The fewest channels.
        MinChannels = MIN_CHANNELS,
        /// The channels of the test signal.
        GenChannels = gen::GEN_CHANNELS,
        /// The channel spacing.
        GenSpacingHz = gen::GEN_SPACING_HZ,
        /// The dwell.
        GenDwellS = gen::GEN_DWELL_S,
        /// The guard.
        GenGuardS = gen::GEN_GUARD_S,
        /// The hop clock jitter.
        GenJitter = gen::GEN_JITTER,
        /// The FSK deviation.
        GenFskHz = gen::GEN_FSK_HZ,
        /// The FSK bit rate.
        GenFskBps = gen::GEN_FSK_BPS,
    }
}

/// The `reset` command: the dwells start again; the table stays.
pub const RESET: Command =
    Command::new("reset", "Reset").help("Drops the dwells and the readings; the table stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// A dwell: start time, frequency (signed bins), level over the median (dB), length (s).
#[derive(Clone, Copy, Debug)]
struct Dwell {
    start: f64,
    bin: f64,
    level: f64,
    length: f64,
}

/// The dwell being followed.
#[derive(Clone, Copy, Debug)]
struct Open {
    first: u64,
    last: u64,
    sum_bin: f64,
    sum_level: f64,
    n: usize,
    misses: u32,
}

/// One evaluation that found a hopper.
#[derive(Clone, Copy, Debug)]
struct Reading {
    rate: f64,
    period: f64,
    dwell: f64,
    guard: f64,
    channels: usize,
    spacing: f64,
    grid: bool,
    lowest: f64,
    highest: f64,
    level: f64,
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// The hop period of `d` from the rough period `rough` (a whole number of frame steps): each dwell start gets the count of
/// hops since the first, the start-to-start time over `rough` rounded and summed; the period is the slope of the
/// least-squares line of start against count over the longest run of dwells without a gap over [`MAX_PERIOD_S`]. `rough`
/// when the line is not defined.
fn hop_period(d: &[Dwell], rough: f64) -> f64 {
    let mut runs: Vec<(Vec<f64>, Vec<f64>)> = vec![(vec![0.0], vec![d[0].start])];
    let mut k = 0.0;
    for w in d.windows(2) {
        let p = w[1].start - w[0].start;
        if !(0.0..=MAX_PERIOD_S).contains(&p) {
            runs.push((Vec::new(), Vec::new()));
        } else {
            k += (p / rough).round();
        }
        if let Some(r) = runs.last_mut() {
            r.0.push(k);
            r.1.push(w[1].start);
        }
    }
    runs.iter()
        .max_by_key(|r| r.0.len())
        .and_then(|r| line_fit(&r.0, &r.1, None).ok())
        .map(|l| l.slope)
        .filter(|&s| s > 0.0)
        .unwrap_or(rough)
}

/// The hopper of `dwells`, or `None` when they do not make one.
fn evaluate(dwells: &VecDeque<Dwell>, min_channels: usize) -> Option<Reading> {
    let d: Vec<Dwell> = dwells.iter().copied().collect();
    let mut periods = Vec::new();
    let mut guards = Vec::new();
    for w in d.windows(2) {
        let p = w[1].start - w[0].start;
        if p > 0.0 && p <= MAX_PERIOD_S {
            periods.push(p);
            guards.push(p - w[0].length);
        }
    }
    let mut bins: Vec<f64> = d.iter().map(|w| w.bin).collect();
    bins.sort_by(f64::total_cmp);
    let mut channels = Vec::new();
    let mut run = vec![bins[0]];
    for &b in &bins[1..] {
        if b - run[run.len() - 1] > SPLIT_BINS {
            if run.len() >= 2 {
                channels.push(run.iter().sum::<f64>() / run.len() as f64);
            }
            run.clear();
        }
        run.push(b);
    }
    if run.len() >= 2 {
        channels.push(run.iter().sum::<f64>() / run.len() as f64);
    }
    if channels.len() < min_channels || periods.len() < MIN_PERIODS {
        return None;
    }
    let period = hop_period(&d, median(&mut periods)?);
    let mut lengths: Vec<f64> = d.iter().map(|w| w.length).collect();
    let mut gaps: Vec<f64> = channels.windows(2).map(|w| w[1] - w[0]).collect();
    let spacing = median(&mut gaps)?;
    let lowest = channels[0];
    let highest = channels[channels.len() - 1];
    let on_grid = channels
        .iter()
        .filter(|&&c| {
            let k = (c - lowest) / spacing;
            (k - k.round()).abs() <= 0.15
        })
        .count();
    Some(Reading {
        rate: 1.0 / period,
        period,
        dwell: median(&mut lengths)?,
        guard: median(&mut guards)?.max(0.0),
        channels: channels.len(),
        spacing,
        grid: on_grid as f64 >= 0.9 * channels.len() as f64,
        lowest,
        highest,
        level: d.iter().map(|w| w.level).sum::<f64>() / d.len() as f64,
    })
}

/// The `hopper` decoder.
pub struct HopperDecoder {
    present_db: f64,
    min_dwells: usize,
    min_channels: usize,
    rate: f64,
    dial_hz: f64,
    frames: Option<ShortFrames>,
    open: Option<Open>,
    dwells: VecDeque<Dwell>,
    total: u64,
    reading: Option<Reading>,
    samples: u64,
    tally: Tally,
    level: InputLevel,
    log: Log<HopperRow>,
    events: Vec<Event>,
    status: Status,
}

impl Default for HopperDecoder {
    fn default() -> Self {
        HopperDecoder {
            present_db: PRESENT_DB.default,
            min_dwells: MIN_DWELLS.default as usize,
            min_channels: MIN_CHANNELS.default as usize,
            rate: 0.0,
            dial_hz: 0.0,
            frames: None,
            open: None,
            dwells: VecDeque::new(),
            total: 0,
            reading: None,
            samples: 0,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
        }
    }
}

impl HopperDecoder {
    fn dt(&self) -> f64 {
        (FFT_N / 2) as f64 / self.rate
    }

    fn bin_hz(&self) -> f64 {
        self.rate / FFT_N as f64
    }

    fn reset(&mut self) {
        self.open = None;
        self.dwells.clear();
        self.reading = None;
        if let Some(f) = self.frames.as_mut() {
            f.reset();
        }
    }

    fn clear(&mut self) {
        self.reset();
        self.total = 0;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn close(&mut self, t: f64) {
        let Some(o) = self.open.take() else {
            return;
        };
        if o.n < MIN_FRAMES {
            return;
        }
        let dt = self.dt();
        self.dwells.push_back(Dwell {
            start: o.first as f64 * dt,
            bin: o.sum_bin / o.n as f64,
            level: o.sum_level / o.n as f64,
            length: (o.last - o.first + 1) as f64 * dt,
        });
        if self.dwells.len() > DWELLS_KEPT {
            self.dwells.pop_front();
        }
        self.total += 1;
        if self.total.is_multiple_of(EVALUATE_EVERY) && self.dwells.len() >= self.min_dwells {
            self.judge(t);
        }
    }

    fn judge(&mut self, t: f64) {
        let Some(r) = evaluate(&self.dwells, self.min_channels) else {
            self.reading = None;
            return;
        };
        let changed = self.reading.is_none_or(|o| {
            (r.rate - o.rate).abs() > RATE_CHANGE * o.rate || r.channels != o.channels
        });
        self.reading = Some(r);
        self.tally.good(t);
        if !changed {
            return;
        }
        let bin = self.bin_hz();
        let row = HopperRow {
            time_s: Seconds(t),
            hop_rate: r.rate,
            channels: r.channels as i64,
            spacing_khz: r.spacing * bin / 1e3,
            span_khz: (r.highest - r.lowest + r.spacing) * bin / 1e3,
            lowest: Hertz(self.dial_hz + r.lowest * bin),
            highest: Hertz(self.dial_hz + r.highest * bin),
            dwell_ms: 1e3 * r.dwell,
            guard_ms: 1e3 * r.guard,
            level_db: r.level,
        };
        self.events.push(HOPPER_CHANGED.event(
            t,
            self.dial_hz,
            &HopperChangedEvent {
                time_s: row.time_s,
                hop_rate: row.hop_rate,
                channels: row.channels,
                spacing_khz: row.spacing_khz,
                span_khz: row.span_khz,
                lowest: row.lowest,
                highest: row.highest,
                dwell_ms: row.dwell_ms,
                guard_ms: row.guard_ms,
                level_db: row.level_db,
            },
        ));
        self.log.push(row);
    }

    fn frame(&mut self, f: &Frame) {
        let half = f.power.len() as i64 / 2;
        let (i, &p) = f
            .power
            .iter()
            .enumerate()
            .filter(|(i, _)| (*i as i64 - half).abs() > 1)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .expect("a frame has bins");
        let t = (f.index + 1) as f64 * self.dt();
        let median = f64::from(f.median.max(1e-30));
        let level = 10.0 * (f64::from(p) / median).log10();
        if level <= self.present_db {
            if let Some(o) = self.open.as_mut() {
                o.misses += 1;
                if o.misses >= 2 {
                    self.close(t);
                }
            }
            return;
        }
        self.tally.energy(t);
        let n = f.power.len();
        let at = |k: usize| f64::from(f.power[k.min(n - 1)]);
        let bin = (i as i64 - half) as f64
            + log_parabolic(at(i.saturating_sub(1)), at(i), at(i + 1)).offset;
        let same = self
            .open
            .is_some_and(|o| (bin - o.sum_bin / o.n as f64).abs() <= SAME_DWELL_BINS);
        if !same {
            self.close(t);
            self.open = Some(Open {
                first: f.index,
                last: f.index,
                sum_bin: 0.0,
                sum_level: 0.0,
                n: 0,
                misses: 0,
            });
        }
        if let Some(o) = self.open.as_mut() {
            o.last = f.index;
            o.sum_bin += bin;
            o.sum_level += level;
            o.n += 1;
            o.misses = 0;
        }
    }
}

impl Decoder for HopperDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hopper.analyser"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::PresentDb => self.present_db = PRESENT_DB.read(id, value)?,
            DecoderSetting::MinDwells => {
                self.min_dwells = MIN_DWELLS.read(id, value)?.max(1) as usize
            }
            DecoderSetting::MinChannels => {
                self.min_channels = MIN_CHANNELS.read(id, value)?.max(1) as usize
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::PresentDb, Value::F64(self.present_db)),
            (
                DecoderSetting::MinDwells,
                Value::I64(self.min_dwells as i64),
            ),
            (
                DecoderSetting::MinChannels,
                Value::I64(self.min_channels as i64),
            ),
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
        self.samples = 0;
        self.frames = ShortFrames::new(FFT_N).ok();
        self.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.level.push(x);
        let Some(mut frames) = self.frames.take() else {
            return;
        };
        frames.push(x, |f| self.frame(f));
        self.frames = Some(frames);
        self.samples += x.len() as u64;
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(&HOPPER_HOPPERS, p);
        let now = self.samples as f64 / self.rate.max(1.0);
        let bin = self.bin_hz();
        let r = self.reading;
        let headline = match r {
            _ if self.samples == 0 => "waiting for samples".to_string(),
            Some(r) => format!(
                "hopping {:.1} hops/s over {} channels, {:.1} kHz apart",
                r.rate,
                r.channels,
                r.spacing * bin / 1e3
            ),
            None if self.total == 0 => "listening".to_string(),
            None => format!(
                "{} dwells so far, not a hopper yet (needs {} dwells on at least {} channels)",
                self.total, self.min_dwells, self.min_channels
            ),
        };
        let level = self.level.read();
        p.put(&HopperFields {
            hop_rate: r.map(|r| r.rate),
            hop_period_ms: r.map(|r| 1e3 * r.period),
            dwell_ms: r.map(|r| 1e3 * r.dwell),
            guard_ms: r.map(|r| 1e3 * r.guard),
            channels: r.map(|r| r.channels as i64),
            spacing_khz: r.map(|r| r.spacing * bin / 1e3),
            grid: r.map(|r| r.grid),
            span_khz: r.map(|r| (r.highest - r.lowest + r.spacing) * bin / 1e3),
            lowest: r.map(|r| Hertz(self.dial_hz + r.lowest * bin)),
            highest: r.map(|r| Hertz(self.dial_hz + r.highest * bin)),
            level_db: r.map(|r| r.level),
            dwells: self.total as i64,
            dwells_window: self.dwells.len() as i64,
            bin_khz: bin / 1e3,
            frame_us: 1e6 * self.dt(),
        });
        let st = &mut self.status;
        st.lock_state = self.tally.lock(now);
        st.level_dbfs = level.map(|l| l as f32);
        st.offset_hz = None;
        st.snr_db = None;
        st.summary = headline;
        p.put(&WatchFields {
            state: self.tally.state(now),
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
        HopperDecoder::clear(self);
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("analyser", DecoderComponent::new(HopperDecoder::default()))
        .input("analyser")
        .output("analyser.events")
        .status_from("analyser")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The hopper analyser needs a sample rate of at least 1 Msps",
    );
    for key in [PRESENT_DB.key(), MIN_DWELLS.key(), MIN_CHANNELS.key()] {
        g = g.route(key, &[&format!("analyser.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("analyser", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("hopper.analyser", || {
        Box::new(DecoderComponent::new(HopperDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(HopperGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
        cfg.seed,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 16 channels 50 kHz apart, 500 hops/s", o, 0.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, HopperFields::CHANNELS.key);
        let rate = crate::instrument::number(r, HopperFields::HOP_RATE.key);
        vec![
            CheckLine::new(
                HopperFields::CHANNELS.key,
                n == 16.0,
                format!("{n}, 16 sent"),
            ),
            CheckLine::new(
                HopperFields::HOP_RATE.key,
                (rate - 500.0).abs() <= 15.0,
                format!("{rate:.1}, 500 sent"),
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

/// The frequency-hopper analyser.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Frequency-hopper analyser",
    group: "Measurement",
    blurb:
        "One frequency hopper in the span: hop rate, dwell, guard, channel count, spacing and span.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 0.1e6,
        max_hz: 6e9,
        ..Tuning::anywhere(0.0, 2e6, MIN_RATE_HZ, 2e6)
    },
    submodes: &[],
    settings: HopperSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

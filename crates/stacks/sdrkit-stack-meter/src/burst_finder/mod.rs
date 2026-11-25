//! Burst finder (`burst_finder`): a row for each transmission shorter than 3 s anywhere in the span, with its frequency,
//! length, bandwidth class and level, and a list of the frequencies whose bursts come at a steady interval. It measures
//! and decodes nothing. Resolution: one bin (rate / 256) in frequency and width, one frame step (128 samples) in length; a
//! burst shorter than one frame step is not found. A steady signal is left to `occupancy` and `birdie`.

pub mod gen;

use crate::instrument::{publish_list, InputLevel, Log, Tally};
use crate::typed::{
    BurstClass, BurstFinderBurstRow, BurstFinderFields, BurstFinderRepeatingRow, BurstFoundEvent,
    BurstRepeatingEvent, WatchFields, BURST_FINDER_BURSTS, BURST_FINDER_REPEATING, BURST_FOUND,
    BURST_REPEATING,
};
use gen::BurstGenerator;
use sdrkit_algo_dsp::repeat::repeat_period;
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
pub const ID: &str = "burst_finder";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The most bursts followed at once.
pub const MAX_ACTIVE: usize = 16;
/// Frames without a group that end a burst (more than this).
pub const MAX_MISSES: u32 = 2;
/// The width of a group: its bins at least this share of its peak (12 dB under it).
pub const WIDTH_SHARE: f32 = 0.063;
/// A burst at most this many bins wide is narrow.
pub const NARROW_BINS: f64 = 3.5;
/// Bursts kept for the search for repeating sources.
pub const HISTORY: usize = 600;
/// The time between two searches for repeating sources, s.
pub const REPEAT_EVERY_S: f64 = 0.5;
/// Bursts of one frequency within this many bins of each other.
pub const SAME_SOURCE_BINS: f64 = 1.5;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Finds bursts shorter than the longest burst setting and measures their frequency, length, \
width and level; decodes nothing. A bin is hot 14 dB over the median of its frame; a burst needs 2 frames. A source repeats \
when at least 70 % of the intervals of 4 or more bursts agree within 5 % of their median. No public document confirms \
these rules.";

/// The level over the frame median at which a bin is hot.
pub const HOT_DB: Setting<field::F64> =
    Setting::new("hot_db", "Hot bin over the frame median", "dB", 14.0).range(8.0, 30.0, 0.0);
/// The longest burst; a longer signal is dropped without a row.
pub const MAX_BURST_S: Setting<field::F64> =
    Setting::new("max_burst_s", "Longest burst", "s", 3.0).range(0.1, 30.0, 0.0);
/// The agreement of the intervals of a repeating source.
pub const REPEAT_TOLERANCE_PCT: Setting<field::F64> =
    Setting::new("repeat_tolerance_pct", "Interval agreement", "%", 5.0).range(1.0, 20.0, 0.0);
/// The frame length.
pub const FFT_SIZE: Setting<field::I64> = Setting::new("fft_size", "Frame length", "", 256)
    .range(128.0, 1024.0, 1.0)
    .help("Points of a frame: 128, 256, 512 or 1024 (rounded to a power of two). Frames step by half of it.");

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The hot level.
        HotDb = HOT_DB,
        /// The longest burst.
        MaxBurstS = MAX_BURST_S,
        /// The interval agreement.
        RepeatTolerancePct = REPEAT_TOLERANCE_PCT,
        /// The frame length.
        FftSize = FFT_SIZE,
    }
}

sdrkit_core::settings! {
    /// The settings of the `burst_finder` stack.
    pub enum BurstFinderSetting {
        /// The hot level.
        HotDb = HOT_DB,
        /// The longest burst.
        MaxBurstS = MAX_BURST_S,
        /// The interval agreement.
        RepeatTolerancePct = REPEAT_TOLERANCE_PCT,
        /// The frame length.
        FftSize = FFT_SIZE,
        /// The bursts of the test signal.
        GenBursts = gen::GEN_BURSTS,
    }
}

/// The `reset` command: the bursts in progress are dropped; the tables stay.
pub const RESET: Command =
    Command::new("reset", "Reset").help("Drops the bursts in progress; the tables stay.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// A run of hot bins of one frame.
#[derive(Clone, Copy, Debug)]
struct Group {
    lo: i64,
    hi: i64,
    peak: f32,
    width: u32,
    used: bool,
}

/// A burst in progress.
#[derive(Clone, Debug)]
struct Active {
    lo: i64,
    hi: i64,
    first: u64,
    misses: u32,
    centres: Vec<f64>,
    widths: Vec<f64>,
    peaks_db: Vec<f64>,
}

/// A source found repeating.
#[derive(Clone, Copy, Debug)]
struct Source {
    hz: f64,
    period_s: f64,
    bursts: usize,
    spread: f64,
}

fn median_of(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// The `burst_finder` decoder.
pub struct BurstFinderDecoder {
    hot_db: f64,
    max_burst_s: f64,
    tolerance_pct: f64,
    fft_size: usize,
    rate: f64,
    dial_hz: f64,
    frames: Option<ShortFrames>,
    groups: Vec<Group>,
    active: Vec<Active>,
    history: VecDeque<(f64, f64)>,
    sources: Vec<Source>,
    next_search: f64,
    found: u64,
    samples: u64,
    tally: Tally,
    level: InputLevel,
    log: Log<BurstFinderBurstRow>,
    events: Vec<Event>,
    status: Status,
}

impl Default for BurstFinderDecoder {
    fn default() -> Self {
        BurstFinderDecoder {
            hot_db: HOT_DB.default,
            max_burst_s: MAX_BURST_S.default,
            tolerance_pct: REPEAT_TOLERANCE_PCT.default,
            fft_size: FFT_SIZE.default as usize,
            rate: 0.0,
            dial_hz: 0.0,
            frames: None,
            groups: Vec::new(),
            active: Vec::new(),
            history: VecDeque::new(),
            sources: Vec::new(),
            next_search: REPEAT_EVERY_S,
            found: 0,
            samples: 0,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
        }
    }
}

impl BurstFinderDecoder {
    fn bin_hz(&self) -> f64 {
        self.rate / self.fft_size as f64
    }

    fn dt(&self) -> f64 {
        (self.fft_size / 2) as f64 / self.rate
    }

    fn build(&mut self) {
        let n = self.fft_size.clamp(128, 1024).next_power_of_two();
        self.fft_size = n;
        self.frames = ShortFrames::new(n).ok();
        self.reset();
    }

    fn reset(&mut self) {
        self.active.clear();
        if let Some(f) = self.frames.as_mut() {
            f.reset();
        }
    }

    fn clear(&mut self) {
        self.reset();
        self.history.clear();
        self.sources.clear();
        self.found = 0;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn frame(&mut self, f: &Frame) {
        let bin = self.bin_hz();
        let dt = self.dt();
        let hot = f.median * 10f64.powf(self.hot_db / 10.0) as f32;
        let half = f.power.len() as i64 / 2;
        // Groups of hot bins: one cold bin may lie between two hot bins of a group.
        self.groups.clear();
        for i in 0..f.power.len() {
            let j = i as i64 - half;
            if j.abs() <= 1 || f.power[i] < hot {
                continue;
            }
            match self.groups.last_mut() {
                Some(g) if j - g.hi <= 2 => {
                    g.hi = j;
                    g.peak = g.peak.max(f.power[i]);
                }
                _ => self.groups.push(Group {
                    lo: j,
                    hi: j,
                    peak: f.power[i],
                    width: 0,
                    used: false,
                }),
            }
        }
        for g in &mut self.groups {
            let min = WIDTH_SHARE * g.peak;
            g.width = (g.lo..=g.hi).filter(|&j| f.bin(j) >= min).count() as u32;
        }
        let median = f64::from(f.median.max(1e-30));
        for a in &mut self.active {
            let pick = self.groups.iter().position(|g| {
                let c = 0.5 * (g.lo + g.hi) as f64;
                !g.used && c >= (a.lo - 2) as f64 && c <= (a.hi + 2) as f64
            });
            match pick {
                Some(k) => {
                    let g = &mut self.groups[k];
                    g.used = true;
                    a.lo = a.lo.min(g.lo);
                    a.hi = a.hi.max(g.hi);
                    a.centres.push(0.5 * (g.lo + g.hi) as f64 * bin);
                    a.widths.push(f64::from(g.width));
                    a.peaks_db.push(10.0 * (f64::from(g.peak) / median).log10());
                    a.misses = 0;
                }
                None => a.misses += 1,
            }
        }
        for g in self.groups.iter().filter(|g| !g.used) {
            if self.active.len() >= MAX_ACTIVE {
                break;
            }
            self.active.push(Active {
                lo: g.lo,
                hi: g.hi,
                first: f.index,
                misses: 0,
                centres: vec![0.5 * (g.lo + g.hi) as f64 * bin],
                widths: vec![f64::from(g.width)],
                peaks_db: vec![10.0 * (f64::from(g.peak) / median).log10()],
            });
        }
        let now = (f.index as f64 + 1.0) * dt;
        let longest = self.max_burst_s;
        let mut done = Vec::new();
        self.active.retain(|a| {
            if (f.index - a.first) as f64 * dt > longest {
                return false;
            }
            if a.misses > MAX_MISSES {
                done.push(a.clone());
                return false;
            }
            true
        });
        for mut a in done {
            let frames = a.centres.len();
            if frames < 2 {
                continue;
            }
            let hz = median_of(&mut a.centres);
            let width = median_of(&mut a.widths);
            let length = dt.max((frames as f64 - 0.5) * dt);
            let start = a.first as f64 * dt;
            let level = a.peaks_db.iter().sum::<f64>() / frames as f64;
            let row = BurstFinderBurstRow {
                time_s: Seconds(start),
                frequency: Hertz(self.dial_hz + hz),
                length_ms: 1e3 * length,
                class: if width <= NARROW_BINS {
                    BurstClass::Narrow
                } else {
                    BurstClass::Wide
                },
                width_khz: width.max(1.0) * bin / 1e3,
                level_db: level,
            };
            self.events.push(BURST_FOUND.event(
                start,
                row.frequency.0,
                &BurstFoundEvent {
                    time_s: row.time_s,
                    frequency: row.frequency,
                    length_ms: row.length_ms,
                    class: row.class,
                    width_khz: row.width_khz,
                    level_db: row.level_db,
                },
            ));
            self.log.push(row);
            self.found += 1;
            self.tally.good(now);
            self.history.push_back((start, hz));
            if self.history.len() > HISTORY {
                self.history.pop_front();
            }
        }
        if !self.groups.is_empty() {
            self.tally.energy(now);
        }
        if now >= self.next_search {
            self.next_search = now + REPEAT_EVERY_S;
            self.search(now);
        }
    }

    /// Groups the bursts of the history by frequency and lists the groups whose intervals agree.
    fn search(&mut self, now: f64) {
        if self.history.len() < 4 {
            return;
        }
        let near = SAME_SOURCE_BINS * self.bin_hz();
        let mut taken = vec![false; self.history.len()];
        let mut found = Vec::new();
        for i in 0..self.history.len() {
            if taken[i] {
                continue;
            }
            taken[i] = true;
            let hz = self.history[i].1;
            let mut starts = vec![self.history[i].0];
            for (t, &(start, f)) in taken.iter_mut().zip(&self.history).skip(i + 1) {
                if !*t && (f - hz).abs() <= near {
                    *t = true;
                    starts.push(start);
                }
            }
            if starts.len() < 4 {
                continue;
            }
            if let Some(p) = repeat_period(&starts, self.tolerance_pct / 100.0, 3, 0.7, 0.002) {
                found.push(Source {
                    hz,
                    period_s: p.period_s,
                    bursts: starts.len(),
                    spread: p.spread,
                });
            }
        }
        for s in &found {
            if !self.sources.iter().any(|o| (o.hz - s.hz).abs() <= near) {
                self.events.push(BURST_REPEATING.event(
                    now,
                    self.dial_hz + s.hz,
                    &BurstRepeatingEvent {
                        frequency: Hertz(self.dial_hz + s.hz),
                        period_s: Seconds(s.period_s),
                        bursts: s.bursts as i64,
                        spread_pct: 100.0 * s.spread,
                    },
                ));
            }
        }
        self.sources = found;
    }
}

impl Decoder for BurstFinderDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "burst_finder.finder"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::HotDb => self.hot_db = HOT_DB.read(id, value)?,
            DecoderSetting::MaxBurstS => self.max_burst_s = MAX_BURST_S.read(id, value)?,
            DecoderSetting::RepeatTolerancePct => {
                self.tolerance_pct = REPEAT_TOLERANCE_PCT.read(id, value)?
            }
            DecoderSetting::FftSize => {
                self.fft_size = FFT_SIZE.read(id, value)?.clamp(128, 1024) as usize;
                self.build();
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::HotDb, Value::F64(self.hot_db)),
            (DecoderSetting::MaxBurstS, Value::F64(self.max_burst_s)),
            (
                DecoderSetting::RepeatTolerancePct,
                Value::F64(self.tolerance_pct),
            ),
            (DecoderSetting::FftSize, Value::I64(self.fft_size as i64)),
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
        self.next_search = REPEAT_EVERY_S;
        self.build();
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
        self.log.publish(&BURST_FINDER_BURSTS, p);
        let rows: Vec<BurstFinderRepeatingRow> = self
            .sources
            .iter()
            .map(|s| BurstFinderRepeatingRow {
                frequency: Hertz(self.dial_hz + s.hz),
                period_s: Seconds(s.period_s),
                bursts: s.bursts as i64,
                spread_pct: 100.0 * s.spread,
            })
            .collect();
        publish_list(&BURST_FINDER_REPEATING, &rows, p);
        let now = self.samples as f64 / self.rate.max(1.0);
        let headline = if self.samples == 0 {
            "waiting for samples".to_string()
        } else if self.found == 0 {
            "listening".to_string()
        } else {
            format!(
                "{} bursts in {now:.0} s, {} repeating source(s)",
                self.found,
                self.sources.len()
            )
        };
        let level = self.level.read();
        p.put(&BurstFinderFields {
            bursts: self.found as i64,
            elapsed_s: Seconds(now),
            bursts_per_s: if now > 0.0 {
                self.found as f64 / now
            } else {
                0.0
            },
            repeating: self.sources.len() as i64,
            bin_khz: self.bin_hz() / 1e3,
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
        BurstFinderDecoder::clear(self);
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
            "finder",
            DecoderComponent::new(BurstFinderDecoder::default()),
        )
        .input("finder")
        .output("finder.events")
        .status_from("finder")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The burst finder needs a sample rate of at least 1 Msps",
    );
    for key in [
        HOT_DB.key(),
        MAX_BURST_S.key(),
        REPEAT_TOLERANCE_PCT.key(),
        FFT_SIZE.key(),
    ] {
        g = g.route(key, &[&format!("finder.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("finder", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("burst_finder.finder", || {
        Box::new(DecoderComponent::new(BurstFinderDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(BurstGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
        cfg.seed,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new(
        "test signal, a key fob, a sensor, a wide and a fast source",
        o,
        0.0,
    );
    c.seconds = o.seconds.max(6.5);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, BurstFinderFields::BURSTS.key);
        let rep = crate::instrument::number(r, BurstFinderFields::REPEATING.key);
        vec![
            CheckLine::new(
                BurstFinderFields::BURSTS.key,
                n >= 40.0,
                format!("{n}, 44 sent"),
            ),
            CheckLine::new(
                BurstFinderFields::REPEATING.key,
                rep >= 1.0,
                format!("{rep}, the key fob expected"),
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

/// The burst finder.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Burst finder",
    group: "Measurement",
    blurb: "Every transmission shorter than 3 s anywhere in the span: frequency, length, bandwidth and level; sources \
            that repeat at a steady interval.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6e9,
        ..Tuning::anywhere(0.0, 2e6, MIN_RATE_HZ, 2e6)
    },
    submodes: &[],
    settings: BurstFinderSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

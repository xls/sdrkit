//! Noise floor logger (`noise_floor`): once a second the noise floor of each sub-band of the span (the median of its bins of
//! a 4096-point spectrum, so narrow signals do not move it), kept for up to an hour. A sub-band that stands over its
//! baseline (the 20th percentile of the last 10 minutes) for 3 s makes a rise row, and one when it falls back; most
//! sub-bands over their baselines at once is a change of the whole band, such as a change of the radio's gain. It measures
//! and decodes nothing. The levels are relative to the radio's full scale and not calibrated.

pub mod gen;

use crate::instrument::{publish_list, InputLevel, Log, Tally};
use crate::typed::{
    NoiseFloorChange, NoiseFloorEvent, NoiseFloorEventRow, NoiseFloorFields, NoiseFloorSubBandRow,
    WatchFields, NOISE_FLOOR_EVENT, NOISE_FLOOR_EVENTS, NOISE_FLOOR_SUB_BANDS,
};
use gen::NoiseFloorGenerator;
use sdrkit_algo_dsp::channelgrid::ChannelGrid;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::snapshot::Snapshots;
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
pub const ID: &str = "noise_floor";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 500e3;
/// The points of the FFT.
pub const FFT_N: usize = 4096;
/// The share of the span that the sub-bands cover.
pub const SPAN_SHARE: f64 = 0.9;
/// The signed bins `-DC..=DC` belong to no sub-band.
pub const DC_BINS: i64 = 2;
/// Seconds kept per sub-band.
pub const HISTORY_S: usize = 3600;
/// Seconds at the start in which nothing is judged; also the values a sub-band needs before its rises are judged.
pub const SETTLE_S: usize = 30;
/// Values a sub-band needs to count in a change of the whole band.
pub const WHOLE_BAND_VALUES: usize = 20;
/// The least time between two rows of a change of the whole band, s.
pub const WHOLE_BAND_GAP_S: f64 = 120.0;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the noise floor of each sub-band once a second; decodes nothing. The numbers are \
relative to the radio's full scale per bin and not calibrated. A rise holds 3 dB over the 20th percentile of the last 10 \
minutes for 3 s; a rise that lasts more than about 8 minutes becomes the baseline. No public document confirms these rules.";

/// The number of sub-bands.
pub const SUB_BANDS: Setting<field::I64> =
    Setting::new("sub_bands", "Sub-bands", "", 16).range(4.0, 64.0, 1.0);
/// The rise over the baseline that counts.
pub const RISE_DB: Setting<field::F64> =
    Setting::new("rise_db", "Rise over the baseline", "dB", 3.0).range(1.0, 20.0, 0.0);
/// How long a rise must hold.
pub const HOLD_S: Setting<field::F64> =
    Setting::new("hold_s", "Rise must hold for", "s", 3.0).range(1.0, 60.0, 1.0);
/// The baseline window.
pub const BASELINE_MIN: Setting<field::F64> =
    Setting::new("baseline_min", "Baseline window", "min", 10.0).range(1.0, 60.0, 0.0);
/// The share of the sub-bands that makes a change of the whole band.
pub const WHOLE_BAND_SHARE: Setting<field::F64> = Setting::new(
    "whole_band_share",
    "Sub-bands that make a whole-band change",
    "",
    0.75,
)
.range(0.5, 1.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The sub-bands.
        SubBands = SUB_BANDS,
        /// The rise.
        RiseDb = RISE_DB,
        /// The hold.
        HoldS = HOLD_S,
        /// The baseline window.
        BaselineMin = BASELINE_MIN,
        /// The whole-band share.
        WholeBandShare = WHOLE_BAND_SHARE,
    }
}

sdrkit_core::settings! {
    /// The settings of the `noise_floor` stack.
    pub enum NoiseFloorSetting {
        /// The sub-bands.
        SubBands = SUB_BANDS,
        /// The rise.
        RiseDb = RISE_DB,
        /// The hold.
        HoldS = HOLD_S,
        /// The baseline window.
        BaselineMin = BASELINE_MIN,
        /// The whole-band share.
        WholeBandShare = WHOLE_BAND_SHARE,
        /// The centre of the added noise.
        GenExtraHz = gen::GEN_EXTRA_HZ,
        /// The width of the added noise.
        GenExtraBwHz = gen::GEN_EXTRA_BW_HZ,
        /// The power of the added noise.
        GenExtraPower = gen::GEN_EXTRA_POWER,
        /// The start of the added noise.
        GenExtraFromS = gen::GEN_EXTRA_FROM_S,
        /// The end of the added noise.
        GenExtraToS = gen::GEN_EXTRA_TO_S,
        /// The carrier offset.
        GenToneHz = gen::GEN_TONE_HZ,
        /// The carrier amplitude.
        GenToneAmp = gen::GEN_TONE_AMP,
    }
}

/// The `reset` command: the histories and the rise states start again; the tables stay.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Drops the histories of the sub-bands and their rise states; the tables stay.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// One sub-band: its bins, its block values of the second, its history of seconds and its rise state.
#[derive(Clone, Debug, Default)]
struct SubBand {
    bins: Vec<usize>,
    centre_hz: f64,
    acc: f64,
    history: VecDeque<f64>,
    baseline: Option<f64>,
    over_s: u32,
    over_from: f64,
    raised: Option<(f64, f64)>,
    percentiles: Option<[f64; 3]>,
}

/// The `noise_floor` decoder.
pub struct NoiseFloorDecoder {
    sub_bands: usize,
    rise_db: f64,
    hold_s: f64,
    baseline_min: f64,
    whole_band_share: f64,
    rate: f64,
    dial_hz: f64,
    spec: Option<Snapshots>,
    floor: BinMedian,
    bands: Vec<SubBand>,
    scale: f64,
    blocks_per_s: usize,
    blocks: usize,
    seconds: usize,
    band_floor: VecDeque<f64>,
    last_whole_band: Option<f64>,
    samples: u64,
    scratch: Vec<f64>,
    tally: Tally,
    level: InputLevel,
    log: Log<NoiseFloorEventRow>,
    events: Vec<Event>,
    status: Status,
}

impl Default for NoiseFloorDecoder {
    fn default() -> Self {
        NoiseFloorDecoder {
            sub_bands: SUB_BANDS.default as usize,
            rise_db: RISE_DB.default,
            hold_s: HOLD_S.default,
            baseline_min: BASELINE_MIN.default,
            whole_band_share: WHOLE_BAND_SHARE.default,
            rate: 0.0,
            dial_hz: 0.0,
            spec: None,
            floor: BinMedian::new(1).expect("one frame"),
            bands: Vec::new(),
            scale: 1.0,
            blocks_per_s: 1,
            blocks: 0,
            seconds: 0,
            band_floor: VecDeque::new(),
            last_whole_band: None,
            samples: 0,
            scratch: Vec::new(),
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
        }
    }
}

/// Element `min(n - 1, floor(f n))` of the sorted `v`.
fn percentile(v: &mut [f64], f: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let k = ((f * v.len() as f64).floor() as usize).min(v.len() - 1);
    let (_, x, _) = v.select_nth_unstable_by(k, f64::total_cmp);
    Some(*x)
}

impl NoiseFloorDecoder {
    fn build(&mut self) -> Result<(), ConfigError> {
        let rate_hz = self.rate;
        let refuse = |_| ConfigError::SettingAtRate {
            id: DecoderSetting::SubBands.id(),
            rate_hz,
        };
        let bin = self.rate / FFT_N as f64;
        let half = (SPAN_SHARE / 2.0 * FFT_N as f64).floor();
        let per = (2.0 * half / self.sub_bands as f64).floor();
        let grid = ChannelGrid::new(FFT_N, self.rate, per * bin, SPAN_SHARE)
            .map_err(refuse)?
            .without_dc(DC_BINS);
        self.bands = (0..grid.channels())
            .map(|k| SubBand {
                bins: grid.members(k),
                centre_hz: grid.centre_hz(k),
                ..SubBand::default()
            })
            .collect();
        self.spec = Some(Snapshots::new(FFT_N, 1).map_err(refuse)?);
        let win: f64 = sdrkit_algo_dsp::window::Window::Hann
            .periodic(FFT_N)
            .iter()
            .map(|w| w * w)
            .sum();
        self.scale = 1.0 / (FFT_N as f64 * win);
        self.blocks_per_s = ((self.rate / FFT_N as f64).round() as usize).max(1);
        self.clear();
        Ok(())
    }

    fn reset(&mut self) {
        for b in &mut self.bands {
            b.acc = 0.0;
            b.history.clear();
            b.baseline = None;
            b.over_s = 0;
            b.raised = None;
            b.percentiles = None;
        }
        self.blocks = 0;
        self.seconds = 0;
        self.band_floor.clear();
        self.last_whole_band = None;
        if let Some(s) = self.spec.as_mut() {
            s.reset();
        }
    }

    fn clear(&mut self) {
        self.reset();
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn block(&mut self, power: &[f32], t: f64) {
        for b in &mut self.bands {
            let m = self
                .floor
                .estimate(b.bins.iter().map(|&i| power[i]))
                .unwrap_or(0.0);
            b.acc += f64::from(m) * self.scale;
        }
        self.blocks += 1;
        if self.blocks >= self.blocks_per_s {
            self.second(t);
        }
    }

    fn row(&mut self, t: f64, row: NoiseFloorEventRow) {
        let freq = row.frequency.map_or(self.dial_hz, |f| f.0);
        self.events.push(NOISE_FLOOR_EVENT.event(
            t,
            freq,
            &NoiseFloorEvent {
                time_s: row.time_s,
                kind: row.kind,
                frequency: row.frequency,
                sub_band: row.sub_band,
                rise_db: row.rise_db,
                baseline_db: row.baseline_db,
                duration_s: row.duration_s,
                level_db: row.level_db,
            },
        ));
        self.log.push(row);
    }

    fn second(&mut self, t: f64) {
        let k = self.blocks as f64;
        self.blocks = 0;
        self.seconds += 1;
        let window = ((self.baseline_min * 60.0).round() as usize).max(1);
        for b in &mut self.bands {
            let v = power_to_db(b.acc / k);
            b.acc = 0.0;
            b.history.push_back(v);
            if b.history.len() > HISTORY_S {
                b.history.pop_front();
            }
            let n = b.history.len();
            self.scratch.clear();
            self.scratch
                .extend(b.history.iter().skip(n - n.min(window)));
            let m = self.scratch.len();
            let (_, x, _) = self.scratch.select_nth_unstable_by(m / 5, f64::total_cmp);
            b.baseline = Some(*x);
            self.scratch.clear();
            self.scratch.extend(b.history.iter());
            b.percentiles = Some([
                percentile(&mut self.scratch, 0.1).unwrap_or(v),
                percentile(&mut self.scratch, 0.5).unwrap_or(v),
                percentile(&mut self.scratch, 0.9).unwrap_or(v),
            ]);
        }
        self.scratch.clear();
        self.scratch
            .extend(self.bands.iter().filter_map(|b| b.history.back().copied()));
        let mid = self.scratch.len() / 2;
        let (_, x, _) = self.scratch.select_nth_unstable_by(mid, f64::total_cmp);
        let band = *x;
        self.band_floor.push_back(band);
        if self.band_floor.len() > window {
            self.band_floor.pop_front();
        }
        if self.seconds <= SETTLE_S {
            return;
        }
        let over =
            |b: &SubBand| b.history.back().copied().unwrap_or(0.0) - b.baseline.unwrap_or(0.0);
        let many = self
            .bands
            .iter()
            .filter(|b| b.history.len() > WHOLE_BAND_VALUES && over(b) >= self.rise_db)
            .count();
        let need = (self.whole_band_share * self.bands.len() as f64).ceil() as usize;
        if many >= need {
            for b in &mut self.bands {
                b.over_s = 0;
                b.raised = None;
            }
            if self
                .last_whole_band
                .is_none_or(|w| t - w >= WHOLE_BAND_GAP_S)
            {
                self.last_whole_band = Some(t);
                let change = band - self.band_floor.front().copied().unwrap_or(band);
                self.row(
                    t,
                    NoiseFloorEventRow {
                        time_s: Seconds(t),
                        kind: NoiseFloorChange::WholeBand,
                        frequency: None,
                        sub_band: None,
                        rise_db: change,
                        baseline_db: None,
                        duration_s: None,
                        level_db: Some(band),
                    },
                );
                self.tally.bad(t);
            }
            return;
        }
        let hold = self.hold_s.round().max(1.0) as u32;
        let mut rows = Vec::new();
        let dial = self.dial_hz;
        for (i, b) in self.bands.iter_mut().enumerate() {
            if b.history.len() < SETTLE_S {
                continue;
            }
            let rise = over(b);
            let now = b.history.back().copied().unwrap_or(0.0);
            let base = b.baseline.unwrap_or(now);
            let centre = dial + b.centre_hz;
            let row = |kind, rise_db, duration_s| NoiseFloorEventRow {
                time_s: Seconds(t),
                kind,
                frequency: Some(Hertz(centre)),
                sub_band: Some(i as i64 + 1),
                rise_db,
                baseline_db: Some(base),
                duration_s,
                level_db: Some(now),
            };
            match b.raised {
                Some((from, top)) => {
                    let top = top.max(rise);
                    if rise < 0.5 * self.rise_db {
                        rows.push(row(NoiseFloorChange::Back, top, Some(Seconds(t - from))));
                        b.raised = None;
                        b.over_s = 0;
                    } else {
                        b.raised = Some((from, top));
                    }
                }
                None if rise >= self.rise_db => {
                    if b.over_s == 0 {
                        b.over_from = t - 1.0;
                    }
                    b.over_s += 1;
                    if b.over_s >= hold {
                        b.raised = Some((b.over_from, rise));
                        rows.push(row(NoiseFloorChange::Rise, rise, None));
                    }
                }
                None => b.over_s = 0,
            }
        }
        let raised = self.bands.iter().any(|b| b.raised.is_some());
        for r in rows {
            if r.kind == NoiseFloorChange::Rise {
                self.tally.bad(t);
            }
            self.row(t, r);
        }
        if !raised {
            self.tally.good(t);
        }
    }

    fn band_floor(&self) -> Option<f64> {
        self.band_floor.back().copied()
    }

    fn headline(&self) -> String {
        if self.samples == 0 {
            return "waiting for samples".into();
        }
        match self.band_floor() {
            Some(f) if self.seconds >= 10 => {
                let change = f - self.band_floor.front().copied().unwrap_or(f);
                let raised = self.bands.iter().filter(|b| b.raised.is_some()).count();
                format!(
                    "floor {f:.1} dB re full scale per bin ({change:+.1} dB over the last {:.0} min), {raised} of {} \
                     sub-bands raised",
                    self.baseline_min,
                    self.bands.len()
                )
            }
            _ => "measuring the noise floor".into(),
        }
    }
}

impl Decoder for NoiseFloorDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "noise_floor.monitor"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::SubBands => {
                self.sub_bands = SUB_BANDS.read(id, value)?.clamp(4, 64) as usize;
                if self.rate > 0.0 {
                    self.build().map_err(|_| SettingError::NoSuchId { id })?;
                }
            }
            DecoderSetting::RiseDb => self.rise_db = RISE_DB.read(id, value)?,
            DecoderSetting::HoldS => self.hold_s = HOLD_S.read(id, value)?,
            DecoderSetting::BaselineMin => self.baseline_min = BASELINE_MIN.read(id, value)?,
            DecoderSetting::WholeBandShare => {
                self.whole_band_share = WHOLE_BAND_SHARE.read(id, value)?
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::SubBands, Value::I64(self.sub_bands as i64)),
            (DecoderSetting::RiseDb, Value::F64(self.rise_db)),
            (DecoderSetting::HoldS, Value::F64(self.hold_s)),
            (DecoderSetting::BaselineMin, Value::F64(self.baseline_min)),
            (
                DecoderSetting::WholeBandShare,
                Value::F64(self.whole_band_share),
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
        self.build()?;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.level.push(x);
        let Some(mut spec) = self.spec.take() else {
            return;
        };
        spec.push(x, |s| {
            self.block(s.power, (s.index + 1) as f64 * FFT_N as f64 / self.rate)
        });
        self.spec = Some(spec);
        self.samples += x.len() as u64;
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(&NOISE_FLOOR_EVENTS, p);
        let rows: Vec<NoiseFloorSubBandRow> = self
            .bands
            .iter()
            .map(|b| NoiseFloorSubBandRow {
                frequency: Hertz(self.dial_hz + b.centre_hz),
                now_db: b.history.back().copied(),
                baseline_db: b.baseline,
                p10_db: b.percentiles.map(|p| p[0]),
                p50_db: b.percentiles.map(|p| p[1]),
                p90_db: b.percentiles.map(|p| p[2]),
                history_min: b.history.len() as f64 / 60.0,
                raised: b.raised.is_some(),
            })
            .collect();
        publish_list(&NOISE_FLOOR_SUB_BANDS, &rows, p);
        let now = self.samples as f64 / self.rate.max(1.0);
        let headline = self.headline();
        let level = self.level.read();
        let floor = self.band_floor();
        let change = floor.map(|f| f - self.band_floor.front().copied().unwrap_or(f));
        p.put(&NoiseFloorFields {
            floor_db: floor,
            floor_change_db: change,
            raised: self.bands.iter().filter(|b| b.raised.is_some()).count() as i64,
            elapsed_s: Seconds(self.seconds as f64),
        });
        let st = &mut self.status;
        st.lock_state = self.tally.lock(now);
        st.level_dbfs = level.map(|l| l as f32);
        st.offset_hz = None;
        st.snr_db = None;
        st.summary = headline;
        p.put(&WatchFields {
            text: &st.summary,
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
        NoiseFloorDecoder::clear(self);
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
            "monitor",
            DecoderComponent::new(NoiseFloorDecoder::default()),
        )
        .input("monitor")
        .output("monitor.events")
        .status_from("monitor")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The noise floor logger needs a sample rate of at least 500 ksps",
    );
    for key in [
        SUB_BANDS.key(),
        RISE_DB.key(),
        HOLD_S.key(),
        BASELINE_MIN.key(),
        WHOLE_BAND_SHARE.key(),
    ] {
        g = g.route(key, &[&format!("monitor.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("monitor", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("noise_floor.monitor", || {
        Box::new(DecoderComponent::new(NoiseFloorDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(NoiseFloorGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
        cfg.seed,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, a carrier", o, 0.0);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.expect_lock = false;
    c.check = Some(|r| {
        let f = crate::instrument::number(r, NoiseFloorFields::FLOOR_DB.key);
        vec![CheckLine::new(
            NoiseFloorFields::FLOOR_DB.key,
            f < 0.0,
            format!("{f:.1} dB, a floor expected"),
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

/// The noise floor logger.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Noise floor logger",
    group: "Measurement",
    blurb: "The noise floor of each sub-band of the span once a second over hours: rises over a baseline and changes of \
            the whole band.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6e9,
        ..Tuning::anywhere(0.0, 2e6, MIN_RATE_HZ, 2e6)
    },
    submodes: &[],
    settings: NoiseFloorSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

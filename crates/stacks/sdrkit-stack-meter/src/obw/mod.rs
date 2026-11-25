//! Occupied bandwidth meter (`obw`): for the strongest signal of the capture, the 99 % occupied bandwidth, the bandwidths 3,
//! 6, 20 and 26 dB under the spectral peak, the centre of the occupied band against the tuned frequency, the signal power
//! and SNR, and the adjacent and alternate power ratios, from about 1 s of averaged 16384-point Blackman-Harris spectra. It
//! measures and decodes nothing. The spectrum holds noise, neighbours and any products made inside the radio by a strong
//! signal.
//!
//! Documents for the quantities: ITU-R SM.328 (spectra and bandwidth of emissions) and SM.443 (bandwidth measurement at
//! monitoring stations); which of their clauses the rules follow is not confirmed.

pub mod gen;

use crate::instrument::{InputLevel, Log, Tally};
use crate::typed::{
    ObwBandwidthRow, ObwFields, ObwReadingEvent, WatchFields, OBW_BANDWIDTHS, OBW_READING,
};
use gen::ObwGenerator;
use sdrkit_algo_dsp::bandwidth::{bridged_region, power_share_edges, x_db_edges};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::snapshot::Snapshots;
use sdrkit_algo_dsp::stats::centred_mean;
use sdrkit_algo_dsp::window::Window;
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
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "obw";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 100e3;
/// The points of the FFT.
pub const FFT_N: usize = 16384;
/// The floor is the median bin over `ln 2`: 1.443 times the mean noise of an averaged spectrum.
pub const FLOOR_OVER_NOISE: f64 = 1.0 / std::f64::consts::LN_2;
/// A bin is hot when the mean of the 5 bins around it is over this many floors.
pub const HOT_FLOORS: f64 = 2.0;
/// Bins of the smoothing of the x dB bandwidths: those in this many Hz.
pub const SMOOTH_HZ: f64 = 1000.0;
/// The time of input between two rows, s.
pub const ROW_EVERY_S: f64 = 20.0;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the occupied bandwidth of the strongest signal and its neighbours' power; decodes \
nothing. The levels are relative to the radio's full scale and not calibrated. A bin is hot 4.6 dB over the mean noise; the \
signal is the run of hot bins with the most power. Which clauses of ITU-R SM.328 and SM.443 the rules follow is not \
confirmed.";

/// The power share of the occupied band.
pub const POWER_PCT: Setting<field::F64> =
    Setting::new("power_pct", "Occupied power share", "%", 99.0).range(90.0, 99.9, 0.0);
/// The x of the x dB bandwidth of the headline.
pub const X_DB: Setting<field::F64> = Setting::new(
    "x_db",
    "The x of the x dB bandwidth in the headline",
    "dB",
    26.0,
)
.range(3.0, 60.0, 0.0);
/// The averaging time.
pub const AVERAGE_S: Setting<field::F64> =
    Setting::new("average_s", "Averaging time", "s", 1.0).range(0.2, 10.0, 0.0);
/// The centre of the adjacent band, in bandwidths.
pub const ADJACENT_OFFSET: Setting<field::F64> = Setting::new(
    "adjacent_offset",
    "Adjacent band centre, in bandwidths",
    "",
    1.25,
)
.range(1.0, 3.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The power share.
        PowerPct = POWER_PCT,
        /// The headline x.
        XDb = X_DB,
        /// The averaging time.
        AverageS = AVERAGE_S,
        /// The adjacent offset.
        AdjacentOffset = ADJACENT_OFFSET,
    }
}

sdrkit_core::settings! {
    /// The settings of the `obw` stack.
    pub enum ObwSetting {
        /// The power share.
        PowerPct = POWER_PCT,
        /// The headline x.
        XDb = X_DB,
        /// The averaging time.
        AverageS = AVERAGE_S,
        /// The adjacent offset.
        AdjacentOffset = ADJACENT_OFFSET,
        /// The symbol rate of the test signal.
        GenSymbolRate = gen::GEN_SYMBOL_RATE,
        /// The roll-off.
        GenRolloff = gen::GEN_ROLLOFF,
        /// The PSK points.
        GenPsk = gen::GEN_PSK,
        /// The neighbour.
        GenNeighbour = gen::GEN_NEIGHBOUR,
        /// The neighbour level.
        GenNeighbourDb = gen::GEN_NEIGHBOUR_DB,
        /// The neighbour offset.
        GenNeighbourOffset = gen::GEN_NEIGHBOUR_OFFSET,
    }
}

/// The `reset` command: the average starts again; the table stays.
pub const RESET: Command =
    Command::new("reset", "Reset").help("Starts the average again; the table stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The readings of one evaluation. Frequencies are offsets from the centre of the capture, Hz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    /// The occupied band: lower and upper edge.
    pub edges: (f64, f64),
    /// The widths 3, 6, 20 and 26 dB under the peak and at the x of the headline, Hz.
    pub widths: [f64; 5],
    /// The signal power, dB.
    pub power_db: f64,
    /// The SNR, dB.
    pub snr_db: f64,
    /// Adjacent below and above, alternate below and above, dB.
    pub neighbours: [Option<f64>; 4],
}

/// The rules of an evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    /// The power share of the occupied band.
    pub share: f64,
    /// The x of the headline.
    pub x_db: f64,
    /// The adjacent offset in bandwidths.
    pub adjacent: f64,
}

/// Evaluates the spectrum `s` (signed order, power per bin) with the floor `floor` (1.443 times the mean noise) and bins of
/// `bin_hz`. `None` when no run of hot bins spans more than 3 bins.
pub fn evaluate(s: &[f64], floor: f64, bin_hz: f64, rules: Rules) -> Option<Reading> {
    let n = s.len();
    let half = (n / 2) as f64;
    let clean: Vec<f64> = s.iter().map(|&v| (v - floor).max(0.0)).collect();
    let mut five = Vec::new();
    centred_mean(s, 2, &mut five);
    let hot: Vec<bool> = five.iter().map(|&v| v > HOT_FLOORS * floor).collect();
    let region = bridged_region(&hot, &clean, 4.max(n / 100))?;
    let width = region.hi - region.lo + 1;
    if width <= 3 {
        return None;
    }
    let pad = 8.max(width / 4);
    let lo = region.lo.saturating_sub(pad);
    let hi = (region.hi + pad).min(n - 1);
    let (a, b) = power_share_edges(&clean, lo, hi, rules.share)?;
    let edges = ((a as f64 - half) * bin_hz, (b as f64 + 1.0 - half) * bin_hz);
    let mut w = ((SMOOTH_HZ / bin_hz).floor() as usize).max(1);
    if w.is_multiple_of(2) {
        w += 1;
    }
    let mut smooth = Vec::new();
    centred_mean(&clean, w / 2, &mut smooth);
    let mut widths = [0.0; 5];
    for (k, x) in [3.0, 6.0, 20.0, 26.0, rules.x_db].into_iter().enumerate() {
        let (l, _, h) = x_db_edges(&smooth, lo, hi, x)?;
        widths[k] = (h - l + 1) as f64 * bin_hz;
    }
    let power: f64 = clean[a..=b].iter().sum();
    let bins = b - a + 1;
    let raw: f64 = s[a..=b].iter().sum();
    let mid = 0.5 * (a + b) as f64;
    let off = (rules.adjacent * bins as f64).round();
    let band = |centre: f64| -> Option<f64> {
        let first = (centre - 0.5 * bins as f64).round();
        if first < 0.0 || first as usize + bins > n {
            return None;
        }
        let p: f64 = s[first as usize..first as usize + bins].iter().sum();
        (p > 0.0).then(|| 10.0 * (raw / p).log10())
    };
    Some(Reading {
        edges,
        widths,
        power_db: power_to_db(power),
        snr_db: power_to_db(power / (floor * bins as f64)),
        neighbours: [
            band(mid - off),
            band(mid + off),
            band(mid - 2.0 * off),
            band(mid + 2.0 * off),
        ],
    })
}

/// The `obw` decoder.
pub struct ObwDecoder {
    power_pct: f64,
    x_db: f64,
    average_s: f64,
    adjacent: f64,
    rate: f64,
    dial_hz: f64,
    spec: Option<Snapshots>,
    scale: f64,
    spectrum: Vec<f64>,
    reading: Option<Reading>,
    evaluations: u64,
    next_row: f64,
    samples: u64,
    tally: Tally,
    level: InputLevel,
    log: Log<ObwBandwidthRow>,
    events: Vec<Event>,
    status: Status,
}

impl Default for ObwDecoder {
    fn default() -> Self {
        ObwDecoder {
            power_pct: POWER_PCT.default,
            x_db: X_DB.default,
            average_s: AVERAGE_S.default,
            adjacent: ADJACENT_OFFSET.default,
            rate: 0.0,
            dial_hz: 0.0,
            spec: None,
            scale: 1.0,
            spectrum: Vec::new(),
            reading: None,
            evaluations: 0,
            next_row: 0.0,
            samples: 0,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
        }
    }
}

impl ObwDecoder {
    fn bin_hz(&self) -> f64 {
        self.rate / FFT_N as f64
    }

    fn build(&mut self) {
        let blocks = ((self.average_s * self.rate / FFT_N as f64).round() as usize).max(1);
        self.spec = Snapshots::with_window(FFT_N, blocks, Window::BlackmanHarris4).ok();
        let win: f64 = Window::BlackmanHarris4
            .periodic(FFT_N)
            .iter()
            .map(|w| w * w)
            .sum();
        self.scale = 1.0 / (FFT_N as f64 * win);
        self.reset();
    }

    fn reset(&mut self) {
        self.reading = None;
        if let Some(s) = self.spec.as_mut() {
            s.reset();
        }
    }

    fn clear(&mut self) {
        self.reset();
        self.evaluations = 0;
        self.next_row = 0.0;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn rules(&self) -> Rules {
        Rules {
            share: self.power_pct / 100.0,
            x_db: self.x_db,
            adjacent: self.adjacent,
        }
    }

    fn snapshot(&mut self, power: &[f32], noise: f32, t: f64) {
        self.spectrum.clear();
        self.spectrum
            .extend(power.iter().map(|&p| f64::from(p) * self.scale));
        let floor = f64::from(noise) * self.scale * FLOOR_OVER_NOISE;
        self.evaluations += 1;
        self.reading = evaluate(&self.spectrum, floor, self.bin_hz(), self.rules());
        let Some(r) = self.reading else {
            if self.spectrum.iter().any(|&v| v > HOT_FLOORS * floor) {
                self.tally.energy(t);
            }
            return;
        };
        self.tally.good(t);
        if t + 1e-9 < self.next_row {
            return;
        }
        self.next_row = t + ROW_EVERY_S;
        let row = self.row(t, &r);
        self.events.push(OBW_READING.event(
            t,
            self.dial_hz + 0.5 * (r.edges.0 + r.edges.1),
            &ObwReadingEvent {
                time_s: row.time_s,
                obw_khz: row.obw_khz,
                obw_low: row.obw_low,
                obw_high: row.obw_high,
                bw3_khz: row.bw3_khz,
                bw6_khz: row.bw6_khz,
                bw20_khz: row.bw20_khz,
                bw26_khz: row.bw26_khz,
                centre_offset_hz: row.centre_offset_hz,
                power_db: row.power_db,
                snr_db: row.snr_db,
                adjacent_low_db: row.adjacent_low_db,
                adjacent_high_db: row.adjacent_high_db,
                alternate_low_db: row.alternate_low_db,
                alternate_high_db: row.alternate_high_db,
            },
        ));
        self.log.push(row);
    }

    fn row(&self, t: f64, r: &Reading) -> ObwBandwidthRow {
        ObwBandwidthRow {
            time_s: Seconds(t),
            obw_khz: (r.edges.1 - r.edges.0) / 1e3,
            obw_low: Hertz(self.dial_hz + r.edges.0),
            obw_high: Hertz(self.dial_hz + r.edges.1),
            bw3_khz: r.widths[0] / 1e3,
            bw6_khz: r.widths[1] / 1e3,
            bw20_khz: r.widths[2] / 1e3,
            bw26_khz: r.widths[3] / 1e3,
            centre_offset_hz: 0.5 * (r.edges.0 + r.edges.1),
            power_db: r.power_db,
            snr_db: r.snr_db,
            adjacent_low_db: r.neighbours[0],
            adjacent_high_db: r.neighbours[1],
            alternate_low_db: r.neighbours[2],
            alternate_high_db: r.neighbours[3],
        }
    }
}

impl Decoder for ObwDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "obw.meter"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::PowerPct => self.power_pct = POWER_PCT.read(id, value)?,
            DecoderSetting::XDb => self.x_db = X_DB.read(id, value)?,
            DecoderSetting::AverageS => {
                self.average_s = AVERAGE_S.read(id, value)?;
                if self.rate > 0.0 {
                    self.build();
                }
            }
            DecoderSetting::AdjacentOffset => self.adjacent = ADJACENT_OFFSET.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::PowerPct, Value::F64(self.power_pct)),
            (DecoderSetting::XDb, Value::F64(self.x_db)),
            (DecoderSetting::AverageS, Value::F64(self.average_s)),
            (DecoderSetting::AdjacentOffset, Value::F64(self.adjacent)),
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
        self.build();
        self.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.level.push(x);
        let Some(mut spec) = self.spec.take() else {
            return;
        };
        let per = spec.samples() as f64;
        spec.push(x, |s| {
            self.snapshot(s.power, s.noise, (s.index + 1) as f64 * per / self.rate)
        });
        self.spec = Some(spec);
        self.samples += x.len() as u64;
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(&OBW_BANDWIDTHS, p);
        let now = self.samples as f64 / self.rate.max(1.0);
        let r = self.reading.map(|r| self.row(now, &r));
        let headline = match (&r, self.reading) {
            _ if self.samples == 0 => "waiting for samples".to_string(),
            (Some(row), Some(rd)) => format!(
                "{:.0} % bandwidth {:.2} kHz, -{:.0} dB bandwidth {:.2} kHz, centre {:.0} Hz, {:.1} dB over the noise",
                self.power_pct,
                row.obw_khz,
                self.x_db,
                rd.widths[4] / 1e3,
                self.dial_hz + row.centre_offset_hz,
                row.snr_db
            ),
            _ if self.evaluations == 0 => "averaging".to_string(),
            _ => "no signal 3 dB over the noise floor".to_string(),
        };
        let level = self.level.read();
        p.put(&ObwFields {
            obw_khz: r.as_ref().map(|r| r.obw_khz),
            obw_low: r.as_ref().map(|r| r.obw_low),
            obw_high: r.as_ref().map(|r| r.obw_high),
            bw3_khz: r.as_ref().map(|r| r.bw3_khz),
            bw6_khz: r.as_ref().map(|r| r.bw6_khz),
            bw20_khz: r.as_ref().map(|r| r.bw20_khz),
            bw26_khz: r.as_ref().map(|r| r.bw26_khz),
            centre_offset_hz: r.as_ref().map(|r| r.centre_offset_hz),
            power_db: r.as_ref().map(|r| r.power_db),
            snr_db: r.as_ref().map(|r| r.snr_db),
            adjacent_low_db: r.as_ref().and_then(|r| r.adjacent_low_db),
            adjacent_high_db: r.as_ref().and_then(|r| r.adjacent_high_db),
            alternate_low_db: r.as_ref().and_then(|r| r.alternate_low_db),
            alternate_high_db: r.as_ref().and_then(|r| r.alternate_high_db),
            signal: r.is_some(),
        });
        let st = &mut self.status;
        st.lock_state = self.tally.lock(now);
        st.level_dbfs = level.map(|l| l as f32);
        st.offset_hz = r.as_ref().map(|r| r.centre_offset_hz);
        st.snr_db = r.as_ref().map(|r| r.snr_db as f32);
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
        ObwDecoder::clear(self);
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("meter", DecoderComponent::new(ObwDecoder::default()))
        .input("meter")
        .output("meter.events")
        .status_from("meter")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The occupied bandwidth meter needs a sample rate of at least 100 ksps",
    );
    for key in [
        POWER_PCT.key(),
        X_DB.key(),
        AVERAGE_S.key(),
        ADJACENT_OFFSET.key(),
    ] {
        g = g.route(key, &[&format!("meter.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("meter", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("obw.meter", || {
        Box::new(DecoderComponent::new(ObwDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(ObwGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
        cfg.seed,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new(
        "test signal, QPSK at 25 kBd with a roll-off of 0.35",
        o,
        0.0,
    );
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let b = crate::instrument::number(r, ObwFields::OBW_KHZ.key);
        vec![CheckLine::new(
            ObwFields::OBW_KHZ.key,
            (b - 29.2).abs() <= 1.2,
            format!("{b:.2} kHz, 29.2 expected"),
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

/// The occupied bandwidth meter.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Occupied bandwidth meter",
    group: "Measurement",
    blurb: "The 99 % and x dB bandwidths, the centre, power and SNR, and the adjacent and alternate power of the \
            strongest signal in the span.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6e9,
        ..Tuning::anywhere(0.0, 1e6, MIN_RATE_HZ, 500e3)
    },
    submodes: &[],
    settings: ObwSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

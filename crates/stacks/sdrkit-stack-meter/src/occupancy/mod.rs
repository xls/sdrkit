//! Spectrum occupancy monitor (`occupancy`): every 10 ms each channel of a grid over the middle 90 % of the span is marked
//! busy or idle; per channel the duty cycle, the transmissions and the longest one; for the band the busy share of the
//! channel-time. It measures and decodes nothing. It reads the signals at this antenna only; one wide signal marks every
//! channel under it busy; the floor is the quietest fifth of the channels, so with more than 80 % of the channels busy it
//! reads high.
//!
//! The quantity is that of ITU-R SM.1880 (spectrum occupancy); the rules here are not taken from it.

pub mod gen;

use crate::instrument::{publish_list, InputLevel, Log, Tally};
use crate::typed::{
    OccupancyActivityEvent, OccupancyActivityRow, OccupancyChannelRow, OccupancyFields,
    WatchFields, OCCUPANCY_ACTIVITY, OCCUPANCY_ACTIVITY_EVENT, OCCUPANCY_CHANNELS,
};
use gen::OccupancyGenerator;
use sdrkit_algo_dsp::channelgrid::ChannelGrid;
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
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "occupancy";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 500e3;
/// The points of the FFT.
pub const FFT_N: usize = 2048;
/// The share of the span that the grid covers.
pub const SPAN_SHARE: f64 = 0.9;
/// The time of one decision, s.
pub const DECISION_S: f64 = 0.01;
/// The decisions that only build the floor.
pub const WARM_UP: u64 = 30;
/// Decisions above in a row that make a channel busy.
pub const START_RUN: u32 = 3;
/// Decisions not above in a row that make a channel idle.
pub const END_RUN: u32 = 5;
/// The weight of a new floor reading in the log domain.
pub const FLOOR_WEIGHT: f64 = 0.03;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the busy and idle time of the channels of a grid; decodes nothing. The levels are \
relative to the radio's full scale and not calibrated. A channel is busy 6 dB over the floor of the quietest fifth of the \
channels after 3 decisions of 10 ms and idle under 4 dB after 5. No public document confirms these rules.";

/// The width of a channel of the grid.
pub const GRID_KHZ: Setting<field::F64> = Setting::new("grid_khz", "Channel width", "kHz", 25.0)
    .range(1.0, 1000.0, 0.0)
    .help("The width of each channel of the grid, rounded to whole FFT bins (at least 2).");
/// The level over the floor at which an idle channel counts as above.
pub const ON_DB: Setting<field::F64> =
    Setting::new("on_db", "Busy above (over the floor)", "dB", 6.0).range(2.0, 30.0, 0.0);
/// The level over the floor under which a busy channel counts as not above.
pub const OFF_DB: Setting<field::F64> =
    Setting::new("off_db", "Idle below (over the floor)", "dB", 4.0).range(2.0, 30.0, 0.0);
/// The quantile of the channel powers that is the floor.
pub const FLOOR_SHARE: Setting<field::F64> =
    Setting::new("floor_share", "Floor quantile of the channels", "", 0.2)
        .range(0.05, 0.5, 0.0)
        .help("The floor is the channel power at this quantile of the sorted channels.");
/// The shortest transmission that makes a row.
pub const MIN_LOG_S: Setting<field::F64> =
    Setting::new("min_log_s", "Shortest transmission listed", "s", 0.05).range(0.01, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The channel width.
        GridKhz = GRID_KHZ,
        /// The busy level.
        OnDb = ON_DB,
        /// The idle level.
        OffDb = OFF_DB,
        /// The floor quantile.
        FloorShare = FLOOR_SHARE,
        /// The shortest row.
        MinLogS = MIN_LOG_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `occupancy` stack.
    pub enum OccupancySetting {
        /// The channel width.
        GridKhz = GRID_KHZ,
        /// The busy level.
        OnDb = ON_DB,
        /// The idle level.
        OffDb = OFF_DB,
        /// The floor quantile.
        FloorShare = FLOOR_SHARE,
        /// The shortest row.
        MinLogS = MIN_LOG_S,
        /// The carriers of the test signal.
        GenTones = gen::GEN_TONES,
    }
}

/// The `reset` command: the floor and the channel states start again; the tables stay.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Learns the floor again and drops the channel states; the tables stay.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The state and the counts of one channel.
#[derive(Clone, Copy, Debug, Default)]
struct Channel {
    busy: bool,
    above: u32,
    below: u32,
    start: f64,
    peak: f64,
    busy_s: f64,
    transmissions: u64,
    longest_s: f64,
}

/// The `occupancy` decoder.
pub struct OccupancyDecoder {
    grid_khz: f64,
    on_db: f64,
    off_db: f64,
    floor_share: f64,
    min_log_s: f64,
    rate: f64,
    dial_hz: f64,
    grid: Option<ChannelGrid>,
    spec: Option<Snapshots>,
    bins: Vec<usize>,
    sums: Vec<f64>,
    power: Vec<f64>,
    sorted: Vec<f64>,
    channels: Vec<Channel>,
    floor: Option<f64>,
    decisions: u64,
    decision_s: f64,
    elapsed_s: f64,
    samples: u64,
    tally: Tally,
    level: InputLevel,
    log: Log<OccupancyActivityRow>,
    events: Vec<Event>,
    status: Status,
}

impl Default for OccupancyDecoder {
    fn default() -> Self {
        OccupancyDecoder {
            grid_khz: GRID_KHZ.default,
            on_db: ON_DB.default,
            off_db: OFF_DB.default,
            floor_share: FLOOR_SHARE.default,
            min_log_s: MIN_LOG_S.default,
            rate: 0.0,
            dial_hz: 0.0,
            grid: None,
            spec: None,
            bins: Vec::new(),
            sums: Vec::new(),
            power: Vec::new(),
            sorted: Vec::new(),
            channels: Vec::new(),
            floor: None,
            decisions: 0,
            decision_s: DECISION_S,
            elapsed_s: 0.0,
            samples: 0,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
        }
    }
}

impl OccupancyDecoder {
    /// Builds the grid for the present rate and width; empties every count.
    fn build(&mut self) -> Result<(), ConfigError> {
        let rate_hz = self.rate;
        let refuse = |_| ConfigError::SettingAtRate {
            id: DecoderSetting::GridKhz.id(),
            rate_hz,
        };
        let grid =
            ChannelGrid::new(FFT_N, self.rate, self.grid_khz * 1e3, SPAN_SHARE).map_err(refuse)?;
        if grid.channels() < 3 {
            return Err(refuse(sdrkit_algo_dsp::DspError::NotFinite {
                param: sdrkit_algo_dsp::Param::Channels,
            }));
        }
        let blocks = ((DECISION_S * self.rate / FFT_N as f64).round() as usize).max(1);
        self.decision_s = (blocks * FFT_N) as f64 / self.rate;
        self.spec = Some(Snapshots::new(FFT_N, blocks).map_err(refuse)?);
        let n = grid.channels();
        self.bins = grid.counted_bins();
        self.sums = vec![0.0; n];
        self.power = vec![0.0; n];
        self.channels = vec![Channel::default(); n];
        self.grid = Some(grid);
        self.clear();
        Ok(())
    }

    /// Drops the floor and the channel states; keeps the counts.
    fn reset(&mut self) {
        self.floor = None;
        self.decisions = 0;
        for c in &mut self.channels {
            c.busy = false;
            c.above = 0;
            c.below = 0;
        }
        if let Some(s) = self.spec.as_mut() {
            s.reset();
        }
    }

    fn decide(&mut self, t: f64) {
        let Some(grid) = self.grid.as_ref() else {
            return;
        };
        // The sums are of the mean block spectrum, so this is the sum over the blocks over (blocks x bins x N).
        for ((p, s), &b) in self.power.iter_mut().zip(&self.sums).zip(&self.bins) {
            *p = s / (b.max(1) * FFT_N) as f64;
        }
        self.sorted.clear();
        self.sorted.extend_from_slice(&self.power);
        self.sorted.sort_by(f64::total_cmp);
        let q = self.sorted[(self.floor_share * (self.sorted.len() - 1) as f64).floor() as usize]
            .max(1e-30);
        let floor = match self.floor {
            None => q,
            Some(f) => ((1.0 - FLOOR_WEIGHT) * f.ln() + FLOOR_WEIGHT * q.ln()).exp(),
        };
        self.floor = Some(floor);
        self.decisions += 1;
        if self.decisions <= WARM_UP {
            return;
        }
        let dt = self.decision_s;
        self.elapsed_s += dt;
        let on = floor * 10f64.powf(self.on_db / 10.0);
        let off = floor * 10f64.powf(self.off_db / 10.0);
        let mut any_busy = false;
        let mut any_above = false;
        for (k, (c, &p)) in self.channels.iter_mut().zip(&self.power).enumerate() {
            if c.busy {
                c.busy_s += dt;
                if p > off {
                    c.below = 0;
                    c.peak = c.peak.max(p);
                } else {
                    c.below += 1;
                    if c.below >= END_RUN {
                        c.busy = false;
                        c.busy_s -= f64::from(END_RUN - 1) * dt;
                        let length = t - c.start - f64::from(END_RUN) * dt;
                        c.transmissions += 1;
                        c.longest_s = c.longest_s.max(length);
                        if length >= self.min_log_s {
                            let row = OccupancyActivityRow {
                                time_s: Seconds(t),
                                frequency: Hertz(self.dial_hz + grid.centre_hz(k)),
                                length_s: Seconds(length),
                                peak_over_floor_db: power_to_db(c.peak / floor),
                                level_db: power_to_db(c.peak),
                            };
                            self.events.push(OCCUPANCY_ACTIVITY_EVENT.event(
                                t,
                                row.frequency.0,
                                &OccupancyActivityEvent {
                                    time_s: row.time_s,
                                    frequency: row.frequency,
                                    length_s: row.length_s,
                                    peak_over_floor_db: row.peak_over_floor_db,
                                    level_db: row.level_db,
                                },
                            ));
                            self.log.push(row);
                        }
                        c.below = 0;
                        c.above = 0;
                        c.peak = 0.0;
                    }
                }
            } else if p > on {
                any_above = true;
                c.above += 1;
                c.peak = c.peak.max(p);
                if c.above >= START_RUN {
                    c.busy = true;
                    c.start = t - f64::from(START_RUN) * dt;
                    c.busy_s += f64::from(START_RUN) * dt;
                }
            } else {
                c.above = 0;
                c.peak = 0.0;
            }
            any_busy |= c.busy;
        }
        if any_busy {
            self.tally.good(t);
        } else if any_above {
            self.tally.energy(t);
        }
    }

    fn headline(&self) -> String {
        if self.samples == 0 {
            return "waiting for samples".into();
        }
        if self.decisions <= WARM_UP {
            return "measuring the noise floor".into();
        }
        let busy = self.channels.iter().filter(|c| c.busy).count();
        format!(
            "{busy} of {} channels busy now, band occupancy {:.1} % over {:.0} s",
            self.channels.len(),
            self.band_occupancy().unwrap_or(0.0),
            self.elapsed_s
        )
    }

    fn band_occupancy(&self) -> Option<f64> {
        let total = self.elapsed_s * self.channels.len() as f64;
        (total > 0.0).then(|| 100.0 * self.channels.iter().map(|c| c.busy_s).sum::<f64>() / total)
    }

    fn clear(&mut self) {
        self.reset();
        for c in &mut self.channels {
            *c = Channel::default();
        }
        self.elapsed_s = 0.0;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }
}

impl Decoder for OccupancyDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "occupancy.monitor"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::GridKhz => {
                self.grid_khz = GRID_KHZ.read(id, value)?;
                if self.rate > 0.0 {
                    self.build().map_err(|_| SettingError::NoSuchId { id })?;
                }
            }
            DecoderSetting::OnDb => self.on_db = ON_DB.read(id, value)?,
            DecoderSetting::OffDb => self.off_db = OFF_DB.read(id, value)?,
            DecoderSetting::FloorShare => self.floor_share = FLOOR_SHARE.read(id, value)?,
            DecoderSetting::MinLogS => self.min_log_s = MIN_LOG_S.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::GridKhz, Value::F64(self.grid_khz)),
            (DecoderSetting::OnDb, Value::F64(self.on_db)),
            (DecoderSetting::OffDb, Value::F64(self.off_db)),
            (DecoderSetting::FloorShare, Value::F64(self.floor_share)),
            (DecoderSetting::MinLogS, Value::F64(self.min_log_s)),
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
        let per = spec.samples() as f64;
        spec.push(x, |s| {
            if let Some(g) = self.grid.as_ref() {
                self.sums.fill(0.0);
                g.accumulate(s.power, &mut self.sums);
            }
            self.decide((s.index + 1) as f64 * per / self.rate);
        });
        self.spec = Some(spec);
        self.samples += x.len() as u64;
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(&OCCUPANCY_ACTIVITY, p);
        let now = self.samples as f64 / self.rate.max(1.0);
        let mut list: Vec<(usize, &Channel)> = self
            .channels
            .iter()
            .enumerate()
            .filter(|(_, c)| c.busy_s > 0.0)
            .collect();
        list.sort_by(|a, b| b.1.busy_s.total_cmp(&a.1.busy_s));
        let rows: Vec<OccupancyChannelRow> = list
            .iter()
            .map(|&(k, c)| OccupancyChannelRow {
                frequency: Hertz(self.dial_hz + self.grid.as_ref().map_or(0.0, |g| g.centre_hz(k))),
                duty_pct: 100.0 * c.busy_s / self.elapsed_s.max(1e-9),
                transmissions: i64::try_from(c.transmissions).unwrap_or(i64::MAX),
                longest_s: Seconds(c.longest_s),
                busy: c.busy,
            })
            .collect();
        publish_list(&OCCUPANCY_CHANNELS, &rows, p);
        let headline = self.headline();
        let level = self.level.read();
        let occupancy = self.band_occupancy();
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
        let g = self.grid.as_ref();
        p.put(&OccupancyFields {
            channels: g.map_or(0, |g| g.channels() as i64),
            channel_khz: g.map_or(0.0, |g| g.channel_hz() / 1e3),
            span_mhz: g.map_or(0.0, |g| g.span_hz() / 1e6),
            floor_db: self.floor.map(power_to_db),
            busy_now: self.channels.iter().filter(|c| c.busy).count() as i64,
            band_occupancy_pct: occupancy,
            elapsed_s: Seconds(self.elapsed_s),
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        OccupancyDecoder::clear(self);
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
            DecoderComponent::new(OccupancyDecoder::default()),
        )
        .input("monitor")
        .output("monitor.events")
        .status_from("monitor")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The occupancy monitor needs a sample rate of at least 500 ksps",
    );
    for key in [
        GRID_KHZ.key(),
        ON_DB.key(),
        OFF_DB.key(),
        FLOOR_SHARE.key(),
        MIN_LOG_S.key(),
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
    r.add("occupancy.monitor", || {
        Box::new(DecoderComponent::new(OccupancyDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(OccupancyGenerator::new(
        cfg.rate_hz,
        &cfg.settings,
    )))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, four keyed carriers", o, 0.0);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, OccupancyFields::CHANNELS.key);
        let occ = crate::instrument::number(r, OccupancyFields::BAND_OCCUPANCY_PCT.key);
        vec![
            CheckLine::new(
                OccupancyFields::CHANNELS.key,
                n >= 3.0,
                format!("{n} channels"),
            ),
            CheckLine::new(
                OccupancyFields::BAND_OCCUPANCY_PCT.key,
                occ > 0.0,
                format!("{occ:.2} %, more than 0 expected"),
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

/// The spectrum occupancy monitor.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Spectrum occupancy monitor",
    group: "Measurement",
    blurb: "Busy and idle time of every channel of a grid over the captured span: duty cycle, transmissions, the longest \
            one, and the band occupancy.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6e9,
        ..Tuning::anywhere(0.0, 2e6, MIN_RATE_HZ, 2e6)
    },
    submodes: &[],
    settings: OccupancySetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

//! PTT and duty-cycle logger (`ptt_log`): the key-ups of one channel at the centre, with start, length, level and the gap
//! from the one before; a transmission under 0.5 s is a kerchunk, one over 3 minutes a stuck key; the duty cycle over the
//! run and over the last 10 minutes. It measures and decodes nothing. Only signals heard at this antenna count: of a
//! repeater whose input is not heard, only the output is measured.

pub mod gen;

use crate::instrument::{Front, InputLevel, Log, Tally, SILENCE_DB};
use crate::typed::{
    PttKind, PttLogFields, PttLogTransmissionRow, PttTransmissionEvent, WatchFields,
    PTT_LOG_TRANSMISSIONS, PTT_TRANSMISSION,
};
use gen::PttGenerator;
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
    Kind, Outbox, Port, RateFrom, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "ptt_log";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 16e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 16e3;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.1;
/// The length of a block, s.
pub const BLOCK_S: f64 = 0.005;
/// The block powers that the floor reads, s.
pub const FLOOR_WINDOW_S: f64 = 60.0;
/// Nothing is judged before this stream time, s.
pub const WARM_UP_S: f64 = 5.0;
/// The window of the second duty cycle, s.
pub const RECENT_S: f64 = 600.0;
/// Rows kept in the log.
pub const ROWS_KEPT: usize = 500;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the key-ups of one channel: start, length, level and gap, and the duty cycle; \
decodes nothing. Keyed 6 dB over the 10th percentile of the last minute of 5 ms blocks, released after 0.3 s under 3 dB. \
The channel must be idle a tenth of the time for the floor to hold. No public document confirms these rules.";

/// The width of the channel.
pub const CHANNEL_KHZ: Setting<field::F64> =
    Setting::new("channel_khz", "Channel width", "kHz", 12.0)
        .range(3.0, 30.0, 0.0)
        .help("Twice the pass edge of the channel filter.");
/// The level over the floor that keys the channel.
pub const ON_DB: Setting<field::F64> =
    Setting::new("on_db", "Keyed above (over the floor)", "dB", 6.0).range(2.0, 30.0, 0.0);
/// The level over the floor under which the hang starts.
pub const OFF_DB: Setting<field::F64> =
    Setting::new("off_db", "Released below (over the floor)", "dB", 3.0).range(2.0, 30.0, 0.0);
/// The hang before a release.
pub const HANG_S: Setting<field::F64> =
    Setting::new("hang_s", "Release after", "s", 0.3).range(0.05, 5.0, 0.0);
/// The kerchunk length.
pub const KERCHUNK_S: Setting<field::F64> =
    Setting::new("kerchunk_s", "Kerchunk shorter than", "s", 0.5).range(0.1, 5.0, 0.0);
/// The stuck-key length.
pub const STUCK_S: Setting<field::F64> =
    Setting::new("stuck_s", "Stuck key after", "s", 180.0).range(30.0, 3600.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The channel width.
        ChannelKhz = CHANNEL_KHZ,
        /// The keying level.
        OnDb = ON_DB,
        /// The release level.
        OffDb = OFF_DB,
        /// The hang.
        HangS = HANG_S,
        /// The kerchunk length.
        KerchunkS = KERCHUNK_S,
        /// The stuck-key length.
        StuckS = STUCK_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `ptt_log` stack.
    pub enum PttLogSetting {
        /// The channel width.
        ChannelKhz = CHANNEL_KHZ,
        /// The keying level.
        OnDb = ON_DB,
        /// The release level.
        OffDb = OFF_DB,
        /// The hang.
        HangS = HANG_S,
        /// The kerchunk length.
        KerchunkS = KERCHUNK_S,
        /// The stuck-key length.
        StuckS = STUCK_S,
        /// The transmissions of the test signal.
        GenKeyed = gen::GEN_KEYED,
        /// The fades.
        GenFades = gen::GEN_FADES,
        /// The carrier offset.
        GenToneHz = gen::GEN_TONE_HZ,
    }
}

/// The `reset` command: the floor and the keying start again; the table and the counts stay.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Learns the floor again and drops a transmission in progress; the table stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The transmission in progress.
#[derive(Clone, Copy, Debug)]
struct Over {
    start: f64,
    hang_from: Option<f64>,
    sum_over: f64,
    sum_level: f64,
    blocks: u32,
    stuck: bool,
}

/// The `ptt_log` decoder.
pub struct PttLogDecoder {
    channel_khz: f64,
    on_db: f64,
    off_db: f64,
    hang_s: f64,
    kerchunk_s: f64,
    stuck_s: f64,
    rate: f64,
    front: Option<Front>,
    block: usize,
    acc: f64,
    fill: usize,
    t: f64,
    powers: VecDeque<(f64, f64)>,
    floor: Option<f64>,
    next_floor: f64,
    over: Option<Over>,
    ended: Vec<(f64, f64)>,
    keyed_total: f64,
    last_end: Option<f64>,
    number: i64,
    kerchunks: i64,
    tally: Tally,
    level: InputLevel,
    log: Log<PttLogTransmissionRow>,
    events: Vec<Event>,
    status: Status,
    dial_hz: f64,
    started: bool,
}

impl Default for PttLogDecoder {
    fn default() -> Self {
        PttLogDecoder {
            channel_khz: CHANNEL_KHZ.default,
            on_db: ON_DB.default,
            off_db: OFF_DB.default,
            hang_s: HANG_S.default,
            kerchunk_s: KERCHUNK_S.default,
            stuck_s: STUCK_S.default,
            rate: 0.0,
            front: None,
            block: 8,
            acc: 0.0,
            fill: 0,
            t: SKIP_S,
            powers: VecDeque::new(),
            floor: None,
            next_floor: 1.0,
            over: None,
            ended: Vec::new(),
            keyed_total: 0.0,
            last_end: None,
            number: 0,
            kerchunks: 0,
            tally: Tally::default(),
            level: InputLevel::default(),
            log: Log::default(),
            events: Vec::new(),
            status: Status::default(),
            dial_hz: 0.0,
            started: false,
        }
    }
}

impl PttLogDecoder {
    fn build(&mut self) {
        let front = Front::new(self.rate, WORK_HZ, self.channel_khz * 500.0, SKIP_S);
        self.block = ((BLOCK_S * front.rate()).round() as usize).max(8);
        self.front = Some(front);
        self.acc = 0.0;
        self.fill = 0;
        self.t = SKIP_S;
        self.reset();
    }

    fn reset(&mut self) {
        self.powers.clear();
        self.floor = None;
        self.next_floor = self.t.max(1.0);
        self.over = None;
    }

    fn clear(&mut self) {
        self.reset();
        self.ended.clear();
        self.keyed_total = 0.0;
        self.last_end = None;
        self.number = 0;
        self.kerchunks = 0;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    fn emit(&mut self, kind: PttKind, o: &Over, length: f64, floor: f64) {
        let n = f64::from(o.blocks.max(1));
        let row = PttLogTransmissionRow {
            number: self.number + 1,
            kind,
            start_s: Seconds(o.start),
            length_s: Seconds(length),
            over_floor_db: o.sum_over / n,
            gap_s: self.last_end.map(|e| Seconds(o.start - e)),
            level_db: if o.blocks > 0 { o.sum_level / n } else { floor },
        };
        self.events.push(PTT_TRANSMISSION.event(
            o.start,
            self.dial_hz,
            &PttTransmissionEvent {
                number: row.number,
                kind: row.kind,
                start_s: row.start_s,
                length_s: row.length_s,
                over_floor_db: row.over_floor_db,
                gap_s: row.gap_s,
                level_db: row.level_db,
            },
        ));
        self.log.push(row);
    }

    fn block_power(&mut self, p_db: f64) {
        let block_s = self.block as f64 / self.front.as_ref().map_or(WORK_HZ, Front::rate);
        self.t += block_s;
        let t = self.t;
        if p_db > SILENCE_DB {
            self.powers.push_back((t, p_db));
        }
        while self
            .powers
            .front()
            .is_some_and(|&(a, _)| t - a > FLOOR_WINDOW_S)
        {
            self.powers.pop_front();
        }
        if t >= self.next_floor && self.over.is_none() && !self.powers.is_empty() {
            self.next_floor = t + 1.0;
            let mut v: Vec<f64> = self.powers.iter().map(|&(_, p)| p).collect();
            let k = v.len() / 10;
            let (_, q, _) = v.select_nth_unstable_by(k, f64::total_cmp);
            let q = *q;
            self.floor = Some(self.floor.map_or(q, |f| 0.9 * f + 0.1 * q));
        }
        let Some(floor) = self.floor else {
            return;
        };
        if t < WARM_UP_S {
            return;
        }
        let over = p_db - floor;
        match self.over.as_mut() {
            None => {
                if over > self.on_db {
                    self.over = Some(Over {
                        start: t - block_s,
                        hang_from: None,
                        sum_over: over,
                        sum_level: p_db,
                        blocks: 1,
                        stuck: false,
                    });
                    self.tally.good(t);
                } else if over > self.off_db {
                    self.tally.energy(t);
                }
            }
            Some(o) => {
                self.tally.good(t);
                if over < self.off_db {
                    let from = *o.hang_from.get_or_insert(t - block_s);
                    if t - from >= self.hang_s {
                        let o = *o;
                        self.over = None;
                        self.finish(&o, from, floor);
                    }
                } else {
                    o.hang_from = None;
                    o.sum_over += over;
                    o.sum_level += p_db;
                    o.blocks += 1;
                    if !o.stuck && t - o.start > self.stuck_s {
                        o.stuck = true;
                        let o = *o;
                        self.emit(PttKind::StuckKey, &o, t - o.start, floor);
                    }
                }
            }
        }
    }

    fn finish(&mut self, o: &Over, end: f64, floor: f64) {
        let length = end - o.start;
        let kind = if length < self.kerchunk_s {
            self.kerchunks += 1;
            PttKind::Kerchunk
        } else {
            PttKind::Transmission
        };
        self.emit(kind, o, length, floor);
        self.number += 1;
        self.keyed_total += length;
        self.ended.push((o.start, end));
        self.last_end = Some(end);
    }

    /// The keyed time in `[from, to]`, the present transmission included.
    fn keyed_in(&self, from: f64, to: f64) -> f64 {
        let ov = |a: f64, b: f64| (b.min(to) - a.max(from)).max(0.0);
        let done: f64 = self
            .ended
            .iter()
            .rev()
            .take_while(|&&(_, b)| b > from)
            .map(|&(a, b)| ov(a, b))
            .sum();
        done + self.over.map_or(0.0, |o| ov(o.start, to))
    }

    fn lengths(&self) -> Vec<f64> {
        self.ended.iter().map(|&(a, b)| b - a).collect()
    }
}

impl Decoder for PttLogDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "ptt_log.logger"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::ChannelKhz => {
                self.channel_khz = CHANNEL_KHZ.read(id, value)?;
                if self.rate > 0.0 {
                    self.build();
                }
            }
            DecoderSetting::OnDb => self.on_db = ON_DB.read(id, value)?,
            DecoderSetting::OffDb => self.off_db = OFF_DB.read(id, value)?,
            DecoderSetting::HangS => self.hang_s = HANG_S.read(id, value)?,
            DecoderSetting::KerchunkS => self.kerchunk_s = KERCHUNK_S.read(id, value)?,
            DecoderSetting::StuckS => self.stuck_s = STUCK_S.read(id, value)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::ChannelKhz, Value::F64(self.channel_khz)),
            (DecoderSetting::OnDb, Value::F64(self.on_db)),
            (DecoderSetting::OffDb, Value::F64(self.off_db)),
            (DecoderSetting::HangS, Value::F64(self.hang_s)),
            (DecoderSetting::KerchunkS, Value::F64(self.kerchunk_s)),
            (DecoderSetting::StuckS, Value::F64(self.stuck_s)),
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
        let Some(mut front) = self.front.take() else {
            return;
        };
        for z in front.process(x) {
            self.acc += f64::from(z.norm_sqr());
            self.fill += 1;
            if self.fill == self.block {
                let p = 10.0 * (self.acc / self.block as f64).max(1e-30).log10();
                self.acc = 0.0;
                self.fill = 0;
                self.block_power(p);
            }
        }
        self.front = Some(front);
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(&PTT_LOG_TRANSMISSIONS, p);
        let t = self.t;
        let elapsed = (t - WARM_UP_S).max(0.0);
        let measured = elapsed > 0.0 && self.floor.is_some();
        let duty = measured.then(|| 100.0 * self.keyed_in(WARM_UP_S, t) / elapsed);
        let recent = elapsed.min(RECENT_S);
        let duty10 = measured.then(|| 100.0 * self.keyed_in(t - recent, t) / recent.max(1e-9));
        let mut lengths = self.lengths();
        let n = lengths.len();
        lengths.sort_by(f64::total_cmp);
        let keyed_s = self.over.map(|o| t - o.start);
        let headline = if !self.started {
            "waiting for samples".to_string()
        } else if !measured {
            "measuring the noise floor".to_string()
        } else if let Some(k) = keyed_s {
            format!(
                "keyed for {k:.1} s, {n} transmissions, duty {:.1} %",
                duty.unwrap_or(0.0)
            )
        } else {
            format!(
                "idle, {n} transmissions ({} under {:.1} s), duty {:.1} %",
                self.kerchunks,
                self.kerchunk_s,
                duty.unwrap_or(0.0)
            )
        };
        let level = self.level.read();
        p.put(&PttLogFields {
            keyed: self.over.is_some(),
            keyed_s: keyed_s.map(Seconds),
            floor_db: self.floor,
            transmissions: n as i64,
            kerchunks: self.kerchunks,
            duty_pct: duty,
            duty_10min_pct: duty10,
            mean_s: (n > 0).then(|| Seconds(lengths.iter().sum::<f64>() / n as f64)),
            median_s: (n > 0).then(|| Seconds(lengths[n / 2])),
            longest_s: lengths.last().copied().map(Seconds),
            per_hour: (elapsed > 0.0).then(|| 3600.0 * n as f64 / elapsed),
            elapsed_s: Seconds(elapsed),
        });
        let st = &mut self.status;
        st.lock_state = self.tally.lock(t);
        st.level_dbfs = level.map(|l| l as f32);
        st.offset_hz = None;
        st.snr_db = None;
        st.summary = headline;
        p.put(&WatchFields {
            state: self.tally.state(t),
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
        PttLogDecoder::clear(self);
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("logger", DecoderComponent::new(PttLogDecoder::default()))
        .input("logger")
        .output("logger.events")
        .status_from("logger")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The PTT logger needs a sample rate of at least 16 ksps",
    );
    for key in [
        CHANNEL_KHZ.key(),
        ON_DB.key(),
        OFF_DB.key(),
        HANG_S.key(),
        KERCHUNK_S.key(),
        STUCK_S.key(),
    ] {
        g = g.route(key, &[&format!("logger.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("logger", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("ptt_log.logger", || {
        Box::new(DecoderComponent::new(PttLogDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(PttGenerator::new(cfg.rate_hz, &cfg.settings)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, the first 25 s of the session", o, 0.0);
    c.seconds = o.seconds.max(25.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, PttLogFields::TRANSMISSIONS.key);
        vec![CheckLine::new(
            PttLogFields::TRANSMISSIONS.key,
            n == 2.0,
            format!("{n}, 2 sent"),
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

/// The PTT and duty-cycle logger.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "PTT and duty-cycle logger",
    group: "Measurement",
    blurb: "The key-ups of one channel: start, length, level and gap; kerchunks, a stuck key and the duty cycle.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6e9,
        carrier_at_centre: true,
        ..Tuning::anywhere(0.0, 48e3, MIN_RATE_HZ, 12e3)
    },
    submodes: &[],
    settings: PttLogSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::bench::benchmarks),
};

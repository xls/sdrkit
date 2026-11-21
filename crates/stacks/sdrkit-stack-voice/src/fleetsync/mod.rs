//! FleetSync (`fleetsync`): the multi-carrier receiver of [`crate::channels`] with no plan, every carrier a narrow FM carrier
//! labelled by its frequency, and a FleetSync burst decoder on each one. The decoder takes the flat voice band of the blocks
//! that pass the squelch gate. The burst coder and decoder are in [`code`].
//!
//! It reads the identity only: the fleet and unit numbers, the frame type and the block check. The other bits of the two
//! words are shown raw. No public document gives the frame; the stack follows the specification and its status says so.

pub mod code;

use crate::burstfeed::{BlockDecoder, CarrierDecoders};
use crate::channels::{
    self, LogStyle, SummaryNote, VoiceBlocks, AUDIO_RATE, CARRIERS_SETTING, CHANNEL_WIDTH_HZ,
    DEEMPHASIS, GEN_CNR_DB, RASTER_ORIGIN_HZ, RASTER_STEP_HZ, SQUELCH, TIME_BASE,
};
use crate::plans::FM_CARRIERS;
use crate::typed::{
    FleetsyncBurstRow, FleetsyncFields, FleetsyncUnitRow, VoiceChannelsCallEvent, FLEETSYNC_BURST,
    FLEETSYNC_BURSTS, FLEETSYNC_CALL, FLEETSYNC_UNITS,
};
use code::{BitRate, BurstDecoder, Decoded, Kind, Message};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, ConfigError, Event, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, StackConfig, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Kind as PortKind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_channels::{Call, SignalClock};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "fleetsync";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The most bursts kept.
pub const BURST_CAP: usize = 400;
/// The most fleet and unit pairs kept.
pub const UNIT_CAP: usize = 400;
/// The name of the stack in messages.
const TITLE: &str = "FleetSync";
/// What the status says the stack reads and what no public document confirms.
pub const COVERAGE: &str = "fleet and unit only, other bits raw; no public document confirms the frame, the check, the FleetSync II code or the 2400 bit/s tones";

sdrkit_core::choice_enum! {
    /// The signalling rate the decoder reads.
    pub enum RateChoice {
        /// Both rates at once.
        Auto = ("auto", "Automatic"),
        /// 1200 bit/s.
        B1200 = ("1200", "1200 bit/s"),
        /// 2400 bit/s.
        B2400 = ("2400", "2400 bit/s"),
    }
}

impl RateChoice {
    /// The rates read.
    fn rates(self) -> &'static [BitRate] {
        match self {
            RateChoice::Auto => &[BitRate::B1200, BitRate::B2400],
            RateChoice::B1200 => &[BitRate::B1200],
            RateChoice::B2400 => &[BitRate::B2400],
        }
    }
}

sdrkit_core::choice_enum! {
    /// The signalling rate of the test signal.
    pub enum GenRate {
        /// 1200 bit/s.
        B1200 = ("1200", "1200 bit/s"),
        /// 2400 bit/s.
        B2400 = ("2400", "2400 bit/s"),
    }
}

sdrkit_core::choice_enum! {
    /// The frame types of the test signal.
    pub enum GenFrame {
        /// A plain frame on the first carrier and a FleetSync II frame on the second.
        Both = ("both", "Both"),
        /// Plain frames.
        Plain = ("plain", "FleetSync"),
        /// FleetSync II frames.
        Fs2 = ("fs2", "FleetSync II"),
    }
}

/// The signalling rate read.
pub const BIT_RATE: Setting<field::Choice<RateChoice>> =
    Setting::choice("bit_rate", "Signalling rate", RateChoice::Auto)
        .help("Automatic reads 1200 and 2400 bit/s at once. The tones at 2400 bit/s (1200 and 2400 Hz) are not confirmed by a public document.");
/// The fleet of the test signal.
pub const GEN_FLEET: Setting<field::I64> = Setting::new("gen_fleet", "Test signal fleet", "", 107)
    .range(99.0, 354.0, 1.0)
    .help("The maker's manuals allow 100 to 349; the frame carries 99 to 354.");
/// The unit of the test signal.
pub const GEN_UNIT: Setting<field::I64> = Setting::new("gen_unit", "Test signal unit", "", 1772)
    .range(999.0, 5094.0, 1.0)
    .help("The unit of the first carrier; the second carrier sends the next unit. The maker's manuals allow 1000 to 4999; the frame carries 999 to 5094.");
/// The signalling rate of the test signal.
pub const GEN_BIT_RATE: Setting<field::Choice<GenRate>> =
    Setting::choice("gen_bit_rate", "Test signal rate", GenRate::B1200);
/// The frame types of the test signal.
pub const GEN_FRAME: Setting<field::Choice<GenFrame>> =
    Setting::choice("gen_frame", "Test signal frames", GenFrame::Both);

sdrkit_core::settings! {
    /// The settings of the FleetSync stack.
    pub enum FleetsyncSetting {
        /// The raster step.
        RasterStep = RASTER_STEP_HZ,
        /// The raster origin.
        RasterOrigin = RASTER_ORIGIN_HZ,
        /// The width of a channel.
        ChannelWidth = CHANNEL_WIDTH_HZ,
        /// The squelch.
        Squelch = SQUELCH,
        /// De-emphasis.
        Deemphasis = DEEMPHASIS,
        /// The most carriers followed.
        Carriers = CARRIERS_SETTING,
        /// The signalling rate read.
        BitRate = BIT_RATE,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
        /// The carrier-to-noise ratio of the test signal.
        GenCnr = GEN_CNR_DB,
        /// The fleet of the test signal.
        GenFleet = GEN_FLEET,
        /// The unit of the test signal.
        GenUnit = GEN_UNIT,
        /// The rate of the test signal.
        GenBitRate = GEN_BIT_RATE,
        /// The frame types of the test signal.
        GenFrame = GEN_FRAME,
    }
}

sdrkit_core::settings! {
    /// The settings of [`BurstLog`].
    pub enum BurstSetting {
        /// The signalling rate read.
        BitRate = BIT_RATE,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`BurstLog`].
    pub enum BurstCommand {
        /// Forget every burst and unit.
        Clear = objcmd::CLEAR,
    }
}

/// What the FleetSync stack publishes: the fields and tables of the multi-carrier receiver, the bursts and the units.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The event of the ended call `c`.
fn call_event(c: &Call) -> Event {
    FLEETSYNC_CALL.event(
        c.at.t,
        c.freq_hz,
        &VoiceChannelsCallEvent {
            serial: c.serial as i64,
            key: c.key,
            number: i64::from(c.number),
            label: &c.label,
            start_s: Seconds(c.at.t),
            duration_s: Seconds(c.duration_s),
            peak_dbfs: c.peak_dbfs,
            mean_dbfs: c.mean_dbfs,
            demod: &c.demod,
            tone: &c.tone,
        },
    )
}

/// One burst in the log: the row of the `bursts` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoggedBurst {
    /// Counts from 1.
    pub serial: u64,
    /// When it was decoded.
    pub at: Stamp,
    /// The frequency of the carrier, Hz.
    pub hz: f64,
    /// Word 1.
    pub word1: u32,
    /// Word 2.
    pub word2: u32,
    /// A FleetSync II frame.
    pub fs2: bool,
    /// The signalling rate, bit/s.
    pub bit_rate: u32,
    /// Bits repaired by the FleetSync II code.
    pub repaired: u32,
}

impl LoggedBurst {
    /// The two words.
    pub fn message(&self) -> Message {
        Message {
            word1: self.word1,
            word2: self.word2,
        }
    }
    /// The frame type.
    pub fn kind(&self) -> Kind {
        if self.fs2 {
            Kind::Fs2
        } else {
            Kind::Plain
        }
    }
    /// The burst in words: "FleetSync fleet 107 unit 1772".
    pub fn text(&self) -> String {
        let m = self.message();
        format!(
            "{} fleet {} unit {}",
            self.kind().name(),
            m.fleet(),
            m.unit()
        )
    }
    /// Writes the row of the burst to `put`.
    fn with_row<T>(&self, put: impl FnOnce(&FleetsyncBurstRow<'_>) -> T) -> T {
        let m = self.message();
        let label = format!("{:.4}", self.hz / 1e6);
        let word1 = format!("{:08X}", m.word1);
        let word2 = format!("{:08X}", m.word2);
        let raw = format!("{:04X} {:03X}", m.raw1(), m.raw2());
        let check = match self.repaired {
            0 => "good".to_owned(),
            1 => "good, 1 bit repaired".to_owned(),
            n => format!("good, {n} bits repaired"),
        };
        let text = self.text();
        put(&FleetsyncBurstRow {
            serial: self.serial as i64,
            time_utc: self.at.wall.map(Time),
            hz: Hertz(self.hz),
            label: &label,
            fleet: i64::from(m.fleet()),
            unit: i64::from(m.unit()),
            in_range: m.in_maker_range(),
            kind: self.kind().name(),
            bit_rate: i64::from(self.bit_rate),
            word1: &word1,
            word2: &word2,
            raw: &raw,
            check: &check,
            text: &text,
        })
    }
}

impl Shift for LoggedBurst {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Shaped for LoggedBurst {
    type Row = FleetsyncBurstRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        self.with_row(|r| out.put(r));
    }
}

/// A fleet and unit heard: the row of the `units` table, with the carrier and type of its last burst.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitHeard {
    /// The fleet number.
    pub fleet: u16,
    /// The unit number.
    pub unit: u16,
    /// The carrier of the last burst, Hz.
    pub hz: f64,
    /// The last burst was FleetSync II.
    pub fs2: bool,
}

impl Shift for UnitHeard {
    fn shift_times(&mut self, _: f64) {}
}

impl Shaped for UnitHeard {
    type Row = FleetsyncUnitRow<'static>;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let in_range = (code::FLEET_MAKER.0..=code::FLEET_MAKER.1).contains(&self.fleet)
            && (code::UNIT_MAKER.0..=code::UNIT_MAKER.1).contains(&self.unit);
        out.put(&FleetsyncUnitRow {
            fleet: i64::from(self.fleet),
            unit: i64::from(self.unit),
            in_range,
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
            hz: Hertz(self.hz),
            kind: if self.fs2 { Kind::Fs2 } else { Kind::Plain }.name(),
        });
    }
}

/// What the burst log keeps across sessions: the bursts, the units and the serial of the last burst.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BurstLists {
    /// The bursts, oldest first.
    pub bursts: LogRing<LoggedBurst>,
    /// The units, keyed "fleet-unit".
    pub units: ObjectTable<String, UnitHeard>,
    /// The serial of the last burst.
    pub serial: u64,
}

impl Default for BurstLists {
    fn default() -> Self {
        BurstLists {
            bursts: LogRing::new(BURST_CAP),
            units: ObjectTable::new(UNIT_CAP),
            serial: 0,
        }
    }
}

impl Shift for BurstLists {
    fn shift_times(&mut self, by_s: f64) {
        self.bursts.shift_times(by_s);
        self.units.shift_times(by_s);
    }
}

impl Tables for BurstLists {
    const FORMAT: StateFormat = StateFormat::new("voice.fleetsync_bursts", 1);
    fn clear(&mut self) {
        self.bursts.clear();
        self.units.clear();
        self.serial = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.bursts.adopt(saved.bursts);
        self.units.adopt(saved.units);
        self.serial = self.serial.max(saved.serial);
    }
}

/// The decoders of one carrier: one for each signalling rate read.
pub struct RateDecoders(Vec<BurstDecoder>);

impl RateDecoders {
    /// The decoders of `rates` for audio at `rate_hz`. `None` when the audio rate is too low.
    pub fn new(rate_hz: f64, rates: &[BitRate]) -> Option<RateDecoders> {
        rates
            .iter()
            .map(|&r| BurstDecoder::new(rate_hz, r))
            .collect::<Option<Vec<_>>>()
            .map(RateDecoders)
    }
    /// Frames whose check failed, at every rate.
    pub fn check_failures(&self) -> u64 {
        self.0.iter().map(BurstDecoder::check_failures).sum()
    }
    /// Decodes `audio` at every rate and calls `on` for each burst. Each decoder learns whether a decoder of another rate
    /// is active, so a burst of one rate is not counted as a failed frame of the other.
    pub fn process(&mut self, audio: &[f32], mut on: impl FnMut(Decoded)) {
        if let [a, b] = self.0.as_mut_slice() {
            let (on_a, on_b) = (a.active(), b.active());
            a.set_foreign(on_b);
            b.set_foreign(on_a);
        }
        for d in &mut self.0 {
            d.process(audio, &mut on);
        }
    }
}

impl BlockDecoder for RateDecoders {
    fn reset(&mut self) {
        self.0.iter_mut().for_each(BurstDecoder::reset);
    }
}

/// The burst log (`fleetsync.bursts`): a burst decoder for each channel of the bank, the `bursts` and `units` tables and
/// the burst events.
pub struct BurstLog {
    p: Published,
    rate: RateChoice,
    time_base: f64,
    decoders: CarrierDecoders<RateDecoders>,
    lists: BurstLists,
    failures: u64,
    clock: SignalClock,
    host_clock: sdrkit_core::Clock,
    is_live: bool,
    events: Vec<Event>,
    found: Vec<(Decoded, f64, f64)>,
    /// The burst count that the call log has for its summary.
    noted: Option<u64>,
}

impl Default for BurstLog {
    fn default() -> Self {
        BurstLog {
            p: Published::new(FleetsyncFields::SHAPE.fields)
                .with_tables(&[FLEETSYNC_BURSTS.def(), FLEETSYNC_UNITS.def()]),
            rate: RateChoice::Auto,
            time_base: 0.0,
            decoders: CarrierDecoders::default(),
            lists: BurstLists::default(),
            failures: 0,
            clock: SignalClock::default(),
            host_clock: sdrkit_core::Clock::fixed(0.0),
            is_live: false,
            events: Vec::new(),
            found: Vec::new(),
            noted: None,
        }
    }
}

impl BurstLog {
    /// The bursts, oldest first.
    pub fn bursts(&self) -> impl Iterator<Item = &LoggedBurst> {
        self.lists.bursts.iter()
    }

    /// Takes the burst `d` heard on the carrier at `hz` at signal time `t`: its row, its unit, its event and its log line.
    fn heard(&mut self, d: &Decoded, hz: f64, t: f64) {
        self.lists.serial += 1;
        let at = self.clock.stamp(t);
        let b = LoggedBurst {
            serial: self.lists.serial,
            at,
            hz,
            word1: d.message.word1,
            word2: d.message.word2,
            fs2: d.kind == Kind::Fs2,
            bit_rate: d.bit_rate.bits_per_s() as u32,
            repaired: d.repaired,
        };
        let event = b.with_row(|r| FLEETSYNC_BURST.event(t, hz, r));
        self.events.push(event);
        let m = d.message;
        let unit = UnitHeard {
            fleet: m.fleet(),
            unit: m.unit(),
            hz,
            fs2: b.fs2,
        };
        let key = format!("{}-{}", unit.fleet, unit.unit);
        if let Ok(u) = self.lists.units.upsert(&key, at, || unit.clone()) {
            *u.row = unit;
        }
        self.p.results_mut().log(format!(
            "{TITLE} {:.4} MHz: fleet {} unit {} ({})",
            hz / 1e6,
            m.fleet(),
            m.unit(),
            d.kind.name()
        ));
        self.lists.bursts.push(b);
    }

    fn report(&mut self) {
        let now = self.clock.now();
        self.p.put(&FleetsyncFields {
            bursts: self.lists.serial as i64,
            check_failures: self.failures as i64,
            units: self.lists.units.len() as i64,
        });
        shaped::log_rows(&self.lists.bursts, &FLEETSYNC_BURSTS, now, &mut self.p);
        shaped::rows(&self.lists.units, &FLEETSYNC_UNITS, now, &mut self.p);
    }
}

impl Component for BurstLog {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "fleetsync.bursts",
            vec![Port::typed::<VoiceBlocks>("voice")],
            vec![
                Port::new("events", PortKind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<SummaryNote>("note").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(BurstSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The block times start again with the input. The bursts stay, and the clock runs on.
        self.decoders.clear();
        self.clock.restart(self.is_live, self.host_clock.now());
        self.noted = None;
        Ok(vec![
            Format::new(PortKind::Messages, 0.0),
            Format::new(PortKind::typed::<SummaryNote>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.is_live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, io: &mut Io) {
        let mut found = std::mem::take(&mut self.found);
        let rates = self.rate.rates();
        for item in io.items::<VoiceBlocks>(0) {
            for b in &item.value.blocks {
                self.clock.at(b.time);
            }
            let mut failures = 0;
            found.clear();
            self.decoders.feed(
                &item.value.blocks,
                || RateDecoders::new(AUDIO_RATE, rates),
                |d, b| {
                    let before = d.check_failures();
                    d.process(&b.flat, |x| found.push((x, b.freq_hz, b.time)));
                    failures += d.check_failures() - before;
                },
            );
            self.failures += failures;
            for (d, hz, time) in &found {
                let t = self.clock.at(*time);
                self.heard(d, *hz, t);
            }
        }
        self.found = found;
        io.messages_out(0).append(&mut self.events);
        if self.noted != Some(self.lists.serial) {
            self.noted = Some(self.lists.serial);
            let words = match self.lists.serial {
                1 => "1 burst".to_owned(),
                n => format!("{n} bursts"),
            };
            io.items_out::<SummaryNote>(1).push(Timed {
                time: self.clock.now(),
                value: SummaryNote(format!("{words} ({COVERAGE})")),
            });
        }
        if io.tick() {
            self.report();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BurstSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BurstSetting::BitRate => {
                let r = BIT_RATE.read(id, v)?;
                if r != self.rate {
                    self.rate = r;
                    self.decoders.clear();
                }
            }
            BurstSetting::TimeBase => {
                let t = TIME_BASE.read(id, v)?;
                self.time_base = t;
                self.clock.set_time_base((t > 0.0).then_some(t));
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BurstSetting::BitRate, Value::Choice(self.rate.index())),
            (BurstSetting::TimeBase, Value::Time(self.time_base)),
        ])
    }
    fn commands(&self) -> &Commands {
        BurstCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match BurstCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BurstCommand::Clear => {
                self.lists.clear();
                self.failures = 0;
                self.report();
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.clock.now())?;
        self.report();
        Ok(())
    }
}

/// The burst log component.
pub fn register(r: &mut Registry) {
    r.add("fleetsync.bursts", || Box::new(BurstLog::default()));
}

/// The graph: the multi-carrier receiver for narrow FM carriers, and the burst log on the blocks of the bank.
pub fn builder() -> GraphBuilder {
    let style = LogStyle {
        title: Cow::Borrowed(TITLE),
        service: None,
        call: call_event,
        plan: false,
    };
    channels::receiver_builder(FM_CARRIERS, style)
        .add("bursts", BurstLog::default())
        .connect("bank.voice", "bursts.voice")
        .connect("bursts.note", "log.note")
        .output("bursts.events")
}

/// The graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder().catalog(catalog).build()
}

/// The stack graph: the settings routed to the components that own them, and `clear` to the two logs.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "FleetSync: the sample rate is too low (250 kHz at least)",
        )
        .route(
            RASTER_STEP_HZ.key(),
            &[
                "finder.raster_step_hz",
                "bank.raster_step_hz",
                "player.raster_step_hz",
                "log.raster_step_hz",
            ],
        )?
        .route(
            RASTER_ORIGIN_HZ.key(),
            &[
                "finder.raster_origin_hz",
                "bank.raster_origin_hz",
                "player.raster_origin_hz",
                "log.raster_origin_hz",
            ],
        )?
        .route(CHANNEL_WIDTH_HZ.key(), &["bank.channel_width_hz"])?
        .route(SQUELCH.key(), &["finder.squelch", "bank.squelch"])?
        .route(DEEMPHASIS.key(), &["bank.deemphasis"])?
        .route(CARRIERS_SETTING.key(), &["finder.carriers"])?
        .route(BIT_RATE.key(), &["bursts.bit_rate"])?
        .route(TIME_BASE.key(), &["log.time_base", "bursts.time_base"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// The test signal of the generator settings in `s` at `rate_hz`.
pub fn demo(rate_hz: f64, s: &Values, seed: u64) -> crate::scene::SceneConfig {
    let bit_rate = match s.get(&GEN_BIT_RATE) {
        GenRate::B1200 => BitRate::B1200,
        GenRate::B2400 => BitRate::B2400,
    };
    let kind = match s.get(&GEN_FRAME) {
        GenFrame::Both => None,
        GenFrame::Plain => Some(Kind::Plain),
        GenFrame::Fs2 => Some(Kind::Fs2),
    };
    crate::scene::fleetsync_demo(
        rate_hz,
        s.get(&GEN_CNR_DB),
        seed,
        s.get(&GEN_FLEET).clamp(99, 354) as u16,
        s.get(&GEN_UNIT).clamp(999, 5094) as u16,
        bit_rate,
        kind,
    )
}

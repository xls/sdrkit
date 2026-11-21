//! MDC-1200 (`mdc1200`): the multi-carrier receiver of [`crate::channels`] with no plan, every carrier a narrow FM carrier
//! labelled by its frequency, and a burst decoder on each one. The decoder takes the flat voice band of the blocks that pass
//! the squelch gate. The burst coder and decoder are in `sdrkit_proto_tones::mdc`.

use crate::burstfeed::{BlockDecoder, CarrierDecoders};
use crate::channels::{
    self, LogStyle, SummaryNote, VoiceBlocks, AUDIO_RATE, CARRIERS_SETTING, CHANNEL_WIDTH_HZ,
    DEEMPHASIS, GEN_CNR_DB, RASTER_ORIGIN_HZ, RASTER_STEP_HZ, SQUELCH, TIME_BASE,
};
use crate::plans::FM_CARRIERS;
use crate::typed::{
    Mdc1200BurstEvent, Mdc1200BurstRow, Mdc1200CallEvent, Mdc1200Fields, MDC1200_BURST,
    MDC1200_BURSTS, MDC1200_CALL,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, Event, Record, Results, ResultsDecl, SettingError, SettingId, SettingSet,
    StackConfig, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Kind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_channels::{Call, SignalClock};
use sdrkit_proto_tones::mdc::{BurstDecoder, Packet};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "mdc1200";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The most bursts kept.
pub const BURST_CAP: usize = 400;
/// The name of the stack in messages.
const TITLE: &str = "MDC-1200";

sdrkit_core::settings! {
    /// The settings of the MDC-1200 stack.
    pub enum MdcSetting {
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
        /// The time base of a recording.
        TimeBase = TIME_BASE,
        /// The carrier-to-noise ratio of the test signal.
        GenCnr = GEN_CNR_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of [`BurstLog`].
    pub enum BurstSetting {
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`BurstLog`].
    pub enum BurstCommand {
        /// Forget every burst.
        Clear = objcmd::CLEAR,
    }
}

/// What the MDC-1200 stack publishes: the fields and tables of the multi-carrier receiver, and the bursts.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The event of the ended call `c`.
fn call_event(c: &Call) -> Event {
    MDC1200_CALL.event(
        c.at.t,
        c.freq_hz,
        &Mdc1200CallEvent {
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
    /// The packet.
    pub op: u8,
    /// Its argument.
    pub arg: u8,
    /// The unit id.
    pub unit: u16,
    /// A double packet.
    pub double: bool,
    /// The bytes of its second block.
    pub extra: [u8; 4],
}

impl LoggedBurst {
    /// The packet.
    pub fn packet(&self) -> Packet {
        Packet {
            op: self.op,
            arg: self.arg,
            unit: self.unit,
            double: self.double,
            extra: self.extra,
        }
    }

    /// The label of the carrier: its frequency in MHz.
    pub fn label(&self) -> String {
        format!("{:.4}", self.hz / 1e6)
    }
}

impl Shift for LoggedBurst {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl LoggedBurst {
    /// The texts of the burst: the label, the unit, the operation, the argument, the extra bytes and the meaning.
    fn texts(&self) -> [String; 6] {
        let e = self.extra;
        [
            self.label(),
            format!("{:04X}", self.unit),
            format!("{:02X}", self.op),
            format!("{:02X}", self.arg),
            format!("{:02X}{:02X}{:02X}{:02X}", e[0], e[1], e[2], e[3]),
            self.packet().text(),
        ]
    }
}

impl Shaped for LoggedBurst {
    type Row = Mdc1200BurstRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let [label, unit_id, op, arg, extra, text] = self.texts();
        out.put(&Mdc1200BurstRow {
            serial: self.serial as i64,
            time_utc: self.at.wall.map(Time),
            hz: Hertz(self.hz),
            label: &label,
            unit_id: &unit_id,
            op: &op,
            arg: &arg,
            double: self.double,
            extra: &extra,
            text: &text,
        });
    }
}

/// What the burst log keeps across sessions: the bursts and the serial of the last one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurstLists {
    /// The bursts, oldest first.
    pub bursts: LogRing<LoggedBurst>,
    /// The serial of the last burst.
    pub serial: u64,
}

impl Default for BurstLists {
    fn default() -> Self {
        BurstLists {
            bursts: LogRing::new(BURST_CAP),
            serial: 0,
        }
    }
}

impl Shift for BurstLists {
    fn shift_times(&mut self, by_s: f64) {
        self.bursts.shift_times(by_s);
    }
}

impl Tables for BurstLists {
    const FORMAT: StateFormat = StateFormat::new("voice.mdc_bursts", 1);
    fn clear(&mut self) {
        self.bursts.clear();
        self.serial = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.bursts.adopt(saved.bursts);
        self.serial = self.serial.max(saved.serial);
    }
}

impl BlockDecoder for BurstDecoder {
    fn reset(&mut self) {
        BurstDecoder::reset(self);
    }
}

/// The burst log (`mdc1200.bursts`): a burst decoder for each channel of the bank, the `bursts` table and the burst
/// events.
pub struct BurstLog {
    p: Published,
    time_base: f64,
    decoders: CarrierDecoders<BurstDecoder>,
    lists: BurstLists,
    failures: u64,
    clock: SignalClock,
    host_clock: sdrkit_core::Clock,
    is_live: bool,
    events: Vec<Event>,
    found: Vec<(Packet, f64, f64)>,
    /// The burst count that the call log has for its summary.
    noted: Option<u64>,
}

impl Default for BurstLog {
    fn default() -> Self {
        BurstLog {
            p: Published::new(Mdc1200Fields::SHAPE.fields).with_tables(&[MDC1200_BURSTS.def()]),
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

    /// Takes the packet `p` heard on the carrier at `hz` at signal time `t`: its row, its event and its log line.
    fn heard(&mut self, p: &Packet, hz: f64, t: f64) {
        self.lists.serial += 1;
        let b = LoggedBurst {
            serial: self.lists.serial,
            at: self.clock.stamp(t),
            hz,
            op: p.op,
            arg: p.arg,
            unit: p.unit,
            double: p.double,
            extra: p.extra,
        };
        let [label, unit_id, op, arg, extra, text] = b.texts();
        let event = MDC1200_BURST.event(
            t,
            hz,
            &Mdc1200BurstEvent {
                serial: b.serial as i64,
                time_utc: b.at.wall.map(Time),
                hz: Hertz(hz),
                label: &label,
                unit_id: &unit_id,
                op: &op,
                arg: &arg,
                double: b.double,
                extra: &extra,
                text: &text,
            },
        );
        self.events.push(event);
        self.p.results_mut().log(format!(
            "{TITLE} {} MHz: unit {:04X}, {}",
            b.label(),
            b.unit,
            p.text()
        ));
        self.lists.bursts.push(b);
    }

    fn report(&mut self) {
        let now = self.clock.now();
        self.p.put(&Mdc1200Fields {
            bursts: self.lists.serial as i64,
            check_failures: self.failures as i64,
        });
        shaped::log_rows(&self.lists.bursts, &MDC1200_BURSTS, now, &mut self.p);
    }
}

impl Component for BurstLog {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "mdc1200.bursts",
            vec![Port::typed::<VoiceBlocks>("voice")],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
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
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<SummaryNote>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.is_live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, io: &mut Io) {
        let mut found = std::mem::take(&mut self.found);
        for item in io.items::<VoiceBlocks>(0) {
            for b in &item.value.blocks {
                self.clock.at(b.time);
            }
            let mut failures = 0;
            found.clear();
            self.decoders.feed(
                &item.value.blocks,
                || BurstDecoder::new(AUDIO_RATE).ok(),
                |d, b| {
                    let before = d.check_failures();
                    d.process(&b.flat, |p| found.push((p.packet, b.freq_hz, b.time)));
                    failures += d.check_failures() - before;
                },
            );
            self.failures += failures;
            for (p, hz, time) in &found {
                let t = self.clock.at(*time);
                self.heard(p, *hz, t);
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
                value: SummaryNote(words),
            });
        }
        if io.tick() {
            self.report();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BurstSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BurstSetting::TimeBase => {
                let t = TIME_BASE.read(id, v)?;
                self.time_base = t;
                self.clock.set_time_base((t > 0.0).then_some(t));
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(BurstSetting::TimeBase, Value::Time(self.time_base))])
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
    r.add("mdc1200.bursts", || Box::new(BurstLog::default()));
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
            "MDC-1200: the sample rate is too low (250 kHz at least)",
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
        .route(TIME_BASE.key(), &["log.time_base", "bursts.time_base"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

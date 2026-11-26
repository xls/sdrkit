//! Qi wireless charging (`qi`): the packets that a phone or another power receiver sends to its charger by load modulation of
//! the power signal (Wireless Power Consortium, The Qi Wireless Power Transfer System, Parts 1 and 2, Power Class 0, version
//! 1.2.3, section 5.2). A packet is shown only after the odd parity of every byte, every start and stop bit and the checksum
//! hold; a packet that fails is counted. The answers of the charger (frequency shifts of the power signal) are not read.
//!
//! The graph: one decoder component (`reader`). It mixes out the dial offset, decimates to about 16 kHz, slices the envelope
//! with the shared tracking slicer, reads the differential bi-phase bits with the shared interval reader and the bytes and
//! packets of section 5.2.2.

pub mod gen;
pub mod packet;

use crate::family::{db20, hex_bytes, Front, InputLevel, Log, Tally, REPORT_S};
use crate::typed::{
    QiFields, QiMessage, QiPacketEvent, QiPacketRow, TagsStatusFields, QI_PACKET, QI_PACKETS,
};
use gen::QiGenerator;
use packet::{Packet, PacketReader, Parsed, Step};
use sdrkit_algo_modem::envtrack::{EdgeTime, Gate, TrackConfig, TrackingSlicer};
use sdrkit_algo_modem::halfbit::{BiphaseMarkEdges, BiphaseOut, IntervalWindows};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::interface::{ReadFields as _, Seconds};
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
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "qi";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 16e3;
/// The pass edge of the channel before the envelope, Hz.
pub const PASS_HZ: f64 = 5e3;
/// The nominal bit time, s (section 5.2.2.2: 2 kHz).
pub const BIT_S: f64 = 1.0 / 2000.0;
/// Rows kept in the packet table.
pub const ROWS_KEPT: usize = 500;

/// What the stack reads and what no public document confirms.
pub const NOTE_TEXT: &str = "Reads the packets of a Qi power receiver to its power transmitter (Qi version 1.2.3, section \
5.2): a packet is shown only after the odd parity of every byte, the start and stop bits and the checksum hold. The answers \
of the power transmitter are not read, and a power signal more than about 4 kHz from the dial is lost. Not confirmed by a \
public document: the depth of the load modulation, the bit clock followed within 12 % (the document gives 4 %), and the \
formats of versions 1.3 and 2.0. The tracking slicer and the receive windows are choices of this stack. Not tried on a real \
signal.";

/// The swing gate.
pub const GATE_SIGMAS: Setting<field::F64> = Setting::new("gate_sigmas", "Swing gate", "sigma", 5.0)
    .range(3.0, 10.0, 0.0)
    .help("Level changes are decided only while the swing of the envelope is this many deviations of its noise.");
/// The tolerance of the bit clock.
pub const CLOCK_TOLERANCE_PCT: Setting<field::F64> = Setting::new(
    "clock_tolerance_pct",
    "Bit clock followed within",
    "%",
    12.0,
)
.range(5.0, 20.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the reader component.
    pub enum ReaderSetting {
        /// The gate.
        GateSigmas = GATE_SIGMAS,
        /// The clock tolerance.
        ClockTolerancePct = CLOCK_TOLERANCE_PCT,
    }
}

sdrkit_core::settings! {
    /// The settings of the `qi` stack.
    pub enum QiSetting {
        /// The gate.
        GateSigmas = GATE_SIGMAS,
        /// The clock tolerance.
        ClockTolerancePct = CLOCK_TOLERANCE_PCT,
        /// The depth of the test signal.
        GenDepth = gen::GEN_DEPTH,
        /// The bit clock.
        GenBitClockPpm = gen::GEN_BIT_CLOCK_PPM,
        /// The wander frequency.
        GenWanderHz = gen::GEN_WANDER_HZ,
        /// The wander depth.
        GenWanderDepth = gen::GEN_WANDER_DEPTH,
        /// The gap.
        GenGapS = gen::GEN_GAP_S,
        /// The preamble.
        GenPreamble = gen::GEN_PREAMBLE,
        /// The packets.
        GenPackets = gen::GEN_PACKETS,
        /// The change after each packet.
        GenEndEdgeBits = gen::GEN_END_EDGE_BITS,
    }
}

/// The `reset` command: the trackers, the bit clock and any packet in progress start again; the table and the counts stay.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Drops the trackers, the bit clock and the packet in progress; the table stays.");
static COMMANDS: [&Command; 1] = [&RESET];

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

fn slicer_config(gate_sigmas: f64) -> TrackConfig {
    TrackConfig {
        smoothing: 0.5,
        attack: 1.0,
        peak_decay_s: 0.15,
        floor_decay_s: 0.15,
        gate: Gate::Sigma(gate_sigmas),
        shut_level: Some(0),
        hysteresis: 0.22,
        edge: EdgeTime::Interpolated { within: 6.0 },
        start_s: 0.02,
    }
}

/// A packet row as the reader holds it.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    time_s: f64,
    packet: Packet,
    parsed: Parsed,
    level_db: f64,
}

impl Row {
    fn header(&self) -> String {
        format!("{:02X}", self.packet.header)
    }

    fn shape<'a>(&'a self, header: &'a str, bytes: &'a str) -> QiPacketRow<'a> {
        let p = &self.parsed;
        QiPacketRow {
            time_s: Seconds(self.time_s),
            header,
            message: self.packet.kind(),
            bytes,
            value: p.value,
            end_reason: p.end_reason,
            charge: p.charge.as_deref(),
            power_class: p.power_class,
            max_power: p.max_power,
            version: p.version.as_deref(),
            manufacturer: p.manufacturer.as_deref(),
            device_id: p.device_id.as_deref(),
            extended: p.extended,
            level_db: self.level_db,
        }
    }
}

/// The `qi` reader component.
pub struct QiDecoder {
    gate_sigmas: f64,
    tolerance_pct: f64,
    rate: f64,
    offset_hz: f64,
    front: Option<Front>,
    slicer: Option<TrackingSlicer>,
    bits: BiphaseMarkEdges,
    packets: PacketReader,
    last_change: Option<f64>,
    t: f64,
    /// The stack time at the last build of the front end: times keep running across rebuilds.
    origin: f64,
    ok: i64,
    failed: i64,
    last_message: Option<QiMessage>,
    tally: Tally,
    level: InputLevel,
    log: Log<Row>,
    events: Vec<Event>,
    status: Status,
    dial_hz: f64,
    started: bool,
}

impl Default for QiDecoder {
    fn default() -> Self {
        QiDecoder {
            gate_sigmas: GATE_SIGMAS.default,
            tolerance_pct: CLOCK_TOLERANCE_PCT.default,
            rate: 0.0,
            offset_hz: 0.0,
            front: None,
            slicer: None,
            bits: BiphaseMarkEdges::new(IntervalWindows::biphase(BIT_S, 0.12)),
            packets: PacketReader::default(),
            last_change: None,
            t: 0.0,
            origin: 0.0,
            ok: 0,
            failed: 0,
            last_message: None,
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

impl QiDecoder {
    fn build(&mut self) {
        self.origin = self.t;
        let front = Front::new(self.rate, self.offset_hz, WORK_HZ, PASS_HZ);
        self.slicer = Some(TrackingSlicer::new(
            slicer_config(self.gate_sigmas),
            front.rate(),
        ));
        self.front = Some(front);
        self.reset();
    }

    fn reset(&mut self) {
        if let Some(s) = self.slicer.as_mut() {
            s.reset();
        }
        self.bits =
            BiphaseMarkEdges::new(IntervalWindows::biphase(BIT_S, self.tolerance_pct / 100.0));
        self.packets.reset();
        self.last_change = None;
    }

    fn clear(&mut self) {
        self.reset();
        self.ok = 0;
        self.failed = 0;
        self.last_message = None;
        self.tally = Tally::default();
        self.log.clear();
        self.events.clear();
    }

    /// The packet in progress is dropped; one that had its header counts as failed.
    fn drop_packet(&mut self, t: f64) {
        if self.packets.has_header() {
            self.failed += 1;
            self.tally.bad(t);
        }
        self.packets.reset();
    }

    fn bit(&mut self, b: u8, t: f64) {
        match self.packets.bit(b) {
            Step::Nothing => {}
            Step::Failed => {
                self.failed += 1;
                self.tally.bad(t);
            }
            Step::Packet(p) => self.packet(p, t),
        }
    }

    fn packet(&mut self, p: Packet, t: f64) {
        self.ok += 1;
        self.tally.good(t);
        self.last_message = Some(p.kind());
        let level_db = db20(self.slicer.as_ref().map_or(0.0, TrackingSlicer::carrier));
        let row = Row {
            time_s: t,
            parsed: p.parse(),
            packet: p,
            level_db,
        };
        let (h, b) = (row.header(), hex_bytes(&row.packet.message));
        let r = row.shape(&h, &b);
        self.events.push(QI_PACKET.event(
            t,
            self.dial_hz,
            &QiPacketEvent {
                time_s: r.time_s,
                header: r.header,
                message: r.message,
                bytes: r.bytes,
                value: r.value,
                end_reason: r.end_reason,
                charge: r.charge,
                power_class: r.power_class,
                max_power: r.max_power,
                version: r.version,
                manufacturer: r.manufacturer,
                device_id: r.device_id,
                extended: r.extended,
                level_db: r.level_db,
            },
        ));
        self.log.push(row);
    }

    fn change(&mut self, at: f64) {
        self.tally.energy(at);
        let Some(prev) = self.last_change.replace(at) else {
            return;
        };
        let last_bit = self.packets.wants_last_bit();
        match self.bits.edge(at - prev) {
            BiphaseOut::Bit(b) => self.bit(b, at),
            BiphaseOut::Half => {}
            // A full interval after a held half. When the half was the first of the last stop bit, a change after the packet
            // (the modulator turning off, section 5.2.2.4) completes that 1; else the half is dropped and the interval is a 0.
            BiphaseOut::Framing if last_bit => self.bit(1, at),
            BiphaseOut::Framing => self.bit(0, at),
            BiphaseOut::Lost => self.drop_packet(at),
        }
    }
}

impl Decoder for QiDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "qi.reader"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        ReaderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match ReaderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ReaderSetting::GateSigmas => {
                self.gate_sigmas = GATE_SIGMAS.read(id, value)?;
                if let Some(s) = self.slicer.as_mut() {
                    s.set_gate(Gate::Sigma(self.gate_sigmas));
                }
            }
            ReaderSetting::ClockTolerancePct => {
                self.tolerance_pct = CLOCK_TOLERANCE_PCT.read(id, value)?;
                self.bits = BiphaseMarkEdges::new(IntervalWindows::biphase(
                    BIT_S,
                    self.tolerance_pct / 100.0,
                ));
                self.packets.reset();
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (ReaderSetting::GateSigmas, Value::F64(self.gate_sigmas)),
            (
                ReaderSetting::ClockTolerancePct,
                Value::F64(self.tolerance_pct),
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
        self.offset_hz = input.tuned_hz - input.centre_hz;
        self.started = false;
        self.build();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.dial_hz = input.tuned_hz;
        self.offset_hz = input.tuned_hz - input.centre_hz;
        if self.rate > 0.0 {
            self.build();
        }
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.level.push(x);
        let (Some(mut front), Some(mut slicer)) = (self.front.take(), self.slicer.take()) else {
            return;
        };
        let r = front.rate();
        let y = front.process(x);
        for &z in y {
            let c = slicer.push_iq(z);
            let now = self.origin + slicer.position() as f64 / r;
            if let Some(c) = c {
                self.change(self.origin + c.at / r);
            } else if let Some(last) = self.last_change {
                // The last 1 of a packet has no change after its second half.
                if self.bits.is_holding() && self.bits.idle(now - last) == Some(1) {
                    self.bit(1, now);
                }
            }
            self.t = now;
        }
        self.front = Some(front);
        self.slicer = Some(slicer);
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.log.publish(
            p,
            |p| p.clear_rows(&QI_PACKETS),
            |p, key, r| {
                let (h, b) = (r.header(), hex_bytes(&r.packet.message));
                QI_PACKETS.put(p, key, &r.shape(&h, &b));
            },
        );
        let t = self.t;
        let s = self.slicer.as_ref();
        let carrier = s
            .map(TrackingSlicer::carrier)
            .filter(|&m| self.started && m > 0.0);
        let modulated = self.tally.recent_energy(t);
        p.put(&QiFields {
            power_signal_db: carrier.map(db20),
            load_modulation_pct: s.filter(|_| modulated).map(|s| 100.0 * s.depth()),
            packets_ok: self.ok,
            packets_failed: self.failed,
            bit_rate: modulated.then(|| 1.0 / self.bits.clock().unit()),
            last_message: self.last_message,
        });
        let headline = if !self.started {
            "waiting for data".to_string()
        } else if let Some(m) = self.last_message {
            format!(
                "{} packets, last: {}",
                self.ok,
                sdrkit_core::value::Choice::label(m)
            )
        } else if modulated {
            "power signal found, no packet yet".to_string()
        } else {
            "no load modulation seen".to_string()
        };
        let level = self.level.read();
        let st = &mut self.status;
        st.lock_state = self.tally.lock(t);
        st.level_dbfs = level.map(|l| l as f32);
        st.snr_db = None;
        st.offset_hz = None;
        st.summary = headline;
        p.put(&TagsStatusFields {
            text: &st.summary,
            state: self.tally.state(t),
            good: i64::try_from(self.tally.good).unwrap_or(i64::MAX),
            bad: i64::try_from(self.tally.bad).unwrap_or(i64::MAX),
            level_dbfs: level,
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        QiDecoder::clear(self);
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        self.tally = Tally::default();
        if !kept.table(QI_PACKETS.key) {
            self.log.clear();
        }
        if !kept.field(QiFields::PACKETS_OK.key) {
            self.ok = 0;
        }
        if !kept.field(QiFields::PACKETS_FAILED.key) {
            self.failed = 0;
        }
        if !kept.field(QiFields::LAST_MESSAGE.key) {
            self.last_message = None;
        }
    }
}

/// The receiver: one reader component on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("reader", DecoderComponent::new(QiDecoder::default()))
        .input("reader")
        .output("reader.events")
        .status_from("reader")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(ID), graph).min_rate(
        MIN_RATE_HZ,
        "The Qi reader needs a sample rate of at least 24 ksps",
    );
    for key in [GATE_SIGMAS.key(), CLOCK_TOLERANCE_PCT.key()] {
        g = g.route(key, &[&format!("reader.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("reader", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("qi.reader", || {
        Box::new(DecoderComponent::new(QiDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(QiGenerator::new(cfg.rate_hz, &cfg.settings)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, the nine packets of the list", o, 0.0);
    c.seconds = o.seconds.max(2.0);
    c.snr_db = (!o.clean).then_some(60.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let ok = crate::number(r, QiFields::PACKETS_OK.key);
        let failed = crate::number(r, QiFields::PACKETS_FAILED.key);
        let last = QiFields::read(r.fields()).and_then(|f| f.last_message);
        vec![
            CheckLine::new(
                QiFields::PACKETS_OK.key,
                ok >= 9.0,
                format!("{ok}, 9 sent in the first pass"),
            ),
            CheckLine::new(
                QiFields::PACKETS_FAILED.key,
                failed == 0.0,
                format!("{failed}"),
            ),
            CheckLine::new(
                QiFields::LAST_MESSAGE.key,
                last.is_some(),
                format!("{last:?}"),
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

/// The Qi packet reader.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Qi wireless charging",
    group: crate::GROUP,
    blurb: "The packets a phone sends to its Qi charger by load modulation of the power signal: signal strength, control \
error, received power, charge status, configuration and identification.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        carrier_at_centre: true,
        not_circular: true,
        ..Tuning::anywhere(0.0, 250e3, MIN_RATE_HZ, 12e3)
    },
    submodes: &[],
    settings: QiSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: None,
};

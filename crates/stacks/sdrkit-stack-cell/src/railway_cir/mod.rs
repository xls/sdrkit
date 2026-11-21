//! Railway CIR data (`railway_cir`): the data channel of the train radio (CIR) of Chinese railways, FFSK at 1200 bit/s on an
//! FM carrier, a 32-bit sync, (26,16) words and an application frame with a CRC-16/XMODEM. The submode `klw` shows the
//! end-of-train device frames (brake pipe pressure at the tail, commands between the locomotive and the tail device), the
//! submode `tdcs` the dispatch frames (train number, locomotive, kilometre post, speed). No public standard of the format
//! is known: the layout is a community description, and the header bytes 2 to 9 are checked as constants.
//!
//! [`modem`] is the chain from IQ to frames, [`air`] the sync and the words, [`frame`] the application frame, [`gen`] the
//! test signal.

pub mod air;
pub mod frame;
pub mod gen;
pub mod modem;

use crate::typed::{
    RailwayCirFields, RailwayCirKlwEvent, RailwayCirMessageRow, RailwayCirState, RailwayCirTdcsEvent,
    RAILWAY_CIR_KLW, RAILWAY_CIR_MESSAGES, RAILWAY_CIR_TDCS,
};
use crate::lbj::message::{Locomotives, LOCOMOTIVES};
use frame::{Payload, Protocol};
use modem::{AudioDecoder, Front};
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::interface::Time;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState, Receiver, Record,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, StateBlob,
    StateError, Status, SubMode, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kept, Kind, Outbox,
    Port, RateFrom, RunContext, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "railway_cir";
/// The frequency of the test signal, Hz. The host gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 450e6;
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 192e3;
/// The largest message history.
pub const HISTORY_MAX: usize = 2000;
/// The receiver is decoding while a message came within this time, s.
pub const DECODING_S: f64 = 30.0;
/// The format of the saved state.
pub const STATE_VERSION: u32 = 1;

/// The messages kept.
pub const HISTORY: Setting<field::I64> = Setting::new("history", "Messages kept", "", 400)
    .range(50.0, HISTORY_MAX as f64, 1.0)
    .help("The oldest message is dropped beyond this count.");

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum DecoderSetting {
        /// The history.
        History = HISTORY,
        /// The class names of the locomotive type codes.
        Locomotives = LOCOMOTIVES,
    }
}

sdrkit_core::settings! {
    /// The settings of the `railway_cir` stack.
    pub enum RailwayCirSetting {
        /// The history.
        History = HISTORY,
        /// The inverted air bits of the test signal.
        GenInvertBits = gen::GEN_INVERT_BITS,
        /// The tone deviation of the test signal.
        GenDeviation = gen::GEN_DEVIATION_HZ,
        /// The class names of the locomotive type codes.
        Locomotives = LOCOMOTIVES,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The protocol of a submode id (`klw` by default).
pub fn protocol_of(submode: Option<&str>) -> Protocol {
    match submode {
        Some("tdcs") => Protocol::Tdcs,
        _ => Protocol::Klw,
    }
}

/// A message received.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// Its sequence number.
    pub serial: u64,
    /// The wall time of the reception, s since 1970.
    pub wall: f64,
    /// The bits corrected.
    pub corrections: u32,
    /// The frame bytes.
    pub bytes: Vec<u8>,
    /// The decoded payload.
    pub payload: Payload,
}

/// The saved state of `messages`, oldest first: per message the wall time, the correction count and the raw frame.
pub fn save(messages: &VecDeque<Message>) -> StateBlob {
    let mut b = Vec::new();
    for m in messages.iter().rev() {
        b.extend_from_slice(&m.wall.to_le_bytes());
        b.extend_from_slice(&m.corrections.to_le_bytes());
        b.extend_from_slice(&(m.bytes.len() as u16).to_le_bytes());
        b.extend_from_slice(&m.bytes);
    }
    StateBlob::new(ID, STATE_VERSION, b)
}

/// The messages of a saved state, newest first, every frame checked again for `protocol` and its locomotive named from
/// `locos`.
///
/// # Errors
///
/// [`StateError`] if the blob is of another owner or version, or any frame fails or is of the other protocol.
pub fn load(state: &StateBlob, protocol: Protocol, locos: &Locomotives) -> Result<VecDeque<Message>, StateError> {
    if state.owner() != ID {
        return Err(StateError::WrongOwner {
            found: state.owner().to_string(),
        });
    }
    if state.version() != STATE_VERSION {
        return Err(StateError::WrongVersion {
            expected: STATE_VERSION,
            found: state.version(),
        });
    }
    let mut out = VecDeque::new();
    let mut b = state.bytes();
    let mut serial = 0;
    while !b.is_empty() {
        if b.len() < 14 {
            return Err(StateError::Damaged);
        }
        let wall = f64::from_le_bytes(b[0..8].try_into().map_err(|_| StateError::Damaged)?);
        let corrections = u32::from_le_bytes(b[8..12].try_into().map_err(|_| StateError::Damaged)?);
        let n = usize::from(u16::from_le_bytes([b[12], b[13]]));
        let bytes = b.get(14..14 + n).ok_or(StateError::Damaged)?.to_vec();
        b = &b[14 + n..];
        let payload = frame::parse(&bytes, locos)
            .filter(|p| p.protocol() == protocol)
            .ok_or(StateError::Damaged)?;
        serial += 1;
        out.push_front(Message {
            serial,
            wall,
            corrections,
            bytes,
            payload,
        });
    }
    Ok(out)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

/// The `railway_cir` decoder: the front end, the audio decoder and the message list.
pub struct RailwayCirDecoder {
    protocol: Protocol,
    locomotives: (String, Locomotives),
    front: Option<Front>,
    audio_dec: AudioDecoder,
    audio: Vec<f32>,
    history: usize,
    messages: VecDeque<Message>,
    serial: u64,
    total: u64,
    fresh: Vec<u64>,
    removed: Vec<u64>,
    cleared: bool,
    events: Vec<Event>,
    clock: Clock,
    dial_hz: f64,
    rate: f64,
    input: u64,
    last_message_s: Option<f64>,
    started: bool,
    status: Status,
}

impl RailwayCirDecoder {
    /// A decoder that shows the frames of `protocol`.
    pub fn new(protocol: Protocol) -> RailwayCirDecoder {
        RailwayCirDecoder {
            protocol,
            locomotives: (LOCOMOTIVES.default.to_owned(), Locomotives::parse(LOCOMOTIVES.default)),
            front: None,
            audio_dec: AudioDecoder::default(),
            audio: Vec::new(),
            history: HISTORY.default as usize,
            messages: VecDeque::new(),
            serial: 0,
            total: 0,
            fresh: Vec::new(),
            removed: Vec::new(),
            cleared: false,
            events: Vec::new(),
            clock: Clock::fixed(0.0),
            dial_hz: 0.0,
            rate: 0.0,
            input: 0,
            last_message_s: None,
            started: false,
            status: Status::default(),
        }
    }

    fn now_s(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn trim(&mut self) {
        while self.messages.len() > self.history {
            if let Some(m) = self.messages.pop_back() {
                self.removed.push(m.serial);
            }
        }
    }

    fn take(&mut self, bytes: Vec<u8>, corrections: u32) {
        let Some(payload) = frame::parse(&bytes, &self.locomotives.1).filter(|p| p.protocol() == self.protocol) else {
            return;
        };
        self.serial += 1;
        self.total += 1;
        let t = self.now_s();
        self.last_message_s = Some(t);
        let raw = hex(&bytes);
        let text = payload.summary();
        let fixed = i64::from(corrections);
        self.events.push(match &payload {
            Payload::Klw(k) => RAILWAY_CIR_KLW.event(
                t,
                self.dial_hz,
                &RailwayCirKlwEvent {
                    text: &text,
                    locomotive: &k.locomotive,
                    reserved: i64::from(k.reserved),
                    action_code: i64::from(k.action),
                    action: &k.action_name,
                    parameter: k.parameter.as_deref(),
                    pressure_kpa: k.pressure_kpa.map(f64::from),
                    tail_id: k.tail_id.as_deref(),
                    extra: k.extra.as_deref(),
                    clock: k.clock.as_deref(),
                    corrections: fixed,
                    raw: &raw,
                },
            ),
            Payload::Tdcs(d) => RAILWAY_CIR_TDCS.event(
                t,
                self.dial_hz,
                &RailwayCirTdcsEvent {
                    text: &text,
                    train: &d.train,
                    locomotive: &d.locomotive,
                    loco_code: i64::from(d.loco_code),
                    loco_serial: i64::from(d.loco_serial),
                    km_post_km: d.km_post,
                    increasing: d.increasing,
                    test_frame: d.test_frame,
                    speed_kmh: f64::from(d.speed_kmh),
                    weight_t: f64::from(d.weight_t),
                    length: d.length,
                    cars: i64::from(d.cars),
                    station: i64::from(d.station),
                    track: i64::from(d.track),
                    actual_track: i64::from(d.actual_track),
                    driver: i64::from(d.driver),
                    signal_type: i64::from(d.signal_type),
                    semaphore_type: i64::from(d.semaphore_type),
                    role: i64::from(d.role),
                    corrections: fixed,
                    raw: &raw,
                },
            ),
        });
        self.messages.push_front(Message {
            serial: self.serial,
            wall: self.clock.now(),
            corrections,
            bytes,
            payload,
        });
        self.fresh.push(self.serial);
        self.trim();
    }

    fn forget_all(&mut self) {
        self.messages.clear();
        self.fresh.clear();
        self.removed.clear();
        self.cleared = true;
        self.total = 0;
        self.last_message_s = None;
        self.audio_dec = AudioDecoder::default();
    }
}

fn row_of<'a>(m: &'a Message, raw: &'a str) -> RailwayCirMessageRow<'a> {
    match &m.payload {
        Payload::Klw(k) => RailwayCirMessageRow {
            received: Time(m.wall),
            protocol: "KLW",
            train: None,
            locomotive: &k.locomotive,
            action: Some(&k.action_name),
            tail_id: k.tail_id.as_deref(),
            pressure_kpa: k.pressure_kpa.map(f64::from),
            speed_kmh: None,
            km_post_km: None,
            test_frame: false,
            cars: None,
            corrections: i64::from(m.corrections),
            raw,
        },
        Payload::Tdcs(d) => RailwayCirMessageRow {
            received: Time(m.wall),
            protocol: "TDCS",
            train: Some(&d.train),
            locomotive: &d.locomotive,
            action: None,
            tail_id: None,
            pressure_kpa: None,
            speed_kmh: Some(f64::from(d.speed_kmh)),
            km_post_km: d.km_post,
            test_frame: d.test_frame,
            cars: Some(i64::from(d.cars)),
            corrections: i64::from(m.corrections),
            raw,
        },
    }
}

impl Decoder for RailwayCirDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "railway_cir.receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::History => {
                self.history = HISTORY.read(id, value)?.clamp(1, HISTORY_MAX as i64) as usize;
                self.trim();
            }
            DecoderSetting::Locomotives => {
                let text = LOCOMOTIVES.read(id, value)?;
                self.locomotives = (text.to_owned(), Locomotives::parse(text));
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::History, Value::I64(self.history as i64)),
            (DecoderSetting::Locomotives, Value::Text(self.locomotives.0.clone())),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &[]
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        Err(CommandError::NoSuchId { id })
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.input = 0;
        self.dial_hz = input.tuned_hz;
        self.front = Some(Front::new(input.rate, input.tuned_hz - input.centre_hz));
        self.audio_dec = AudioDecoder::default();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.dial_hz = f.tuned_hz;
        self.front = Some(Front::new(self.rate, f.tuned_hz - f.centre_hz));
        self.audio_dec.reset();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.input += x.len() as u64;
        let Some(front) = self.front.as_mut() else {
            return;
        };
        self.audio.clear();
        front.process(x, &mut self.audio);
        let mut heard = Vec::new();
        self.audio_dec.push(&self.audio, |h| heard.push(h));
        for h in heard {
            self.take(h.bytes, h.corrections);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&RAILWAY_CIR_MESSAGES);
        }
        for s in self.removed.drain(..) {
            p.remove_row(&RAILWAY_CIR_MESSAGES, &format!("{s:08}"));
        }
        for s in std::mem::take(&mut self.fresh) {
            if let Some(m) = self.messages.iter().find(|m| m.serial == s) {
                let raw = hex(&m.bytes);
                RAILWAY_CIR_MESSAGES.put(p, &format!("{s:08}"), &row_of(m, &raw));
            }
        }
        let decoding = self.last_message_s.is_some_and(|t| self.now_s() - t <= DECODING_S);
        let state = match (self.started, decoding) {
            (false, _) => RailwayCirState::Waiting,
            (true, false) => RailwayCirState::Listening,
            (true, true) => RailwayCirState::Decoding,
        };
        let c = self.audio_dec.counts();
        let front = self.front.as_ref().filter(|_| self.started);
        let st = &mut self.status;
        st.lock_state = match state {
            RailwayCirState::Waiting => LockState::Off,
            RailwayCirState::Listening => LockState::Searching,
            RailwayCirState::Decoding => LockState::Locked,
        };
        st.offset_hz = front.map(Front::offset_hz);
        st.level_dbfs = front.map(|f| f.level_db() as f32);
        st.summary.clear();
        match self.messages.front() {
            Some(m) => st.summary.push_str(&m.payload.summary()),
            None if self.started => st.summary.push_str("Railway CIR: listening"),
            None => st.summary.push_str("Railway CIR: waiting for samples"),
        }
        p.put(&RailwayCirFields {
            text: &st.summary,
            messages: i64::try_from(self.total).unwrap_or(i64::MAX),
            syncs: i64::try_from(c.syncs).unwrap_or(i64::MAX),
            failed_checks: i64::try_from(c.crc_failures + c.rejections + c.ambiguous).unwrap_or(i64::MAX),
            carrier_offset_hz: front.map(Front::offset_hz),
            level_db: front.map(Front::level_db),
            state,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        self.forget_all();
    }
    /// Keeps the messages and their count while the stack keeps the message table; forgets the frame being read and
    /// the time of the newest message.
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(RAILWAY_CIR_MESSAGES.key) {
            self.forget_all();
        }
        self.audio_dec = AudioDecoder::default();
        self.last_message_s = None;
    }
    fn save_state(&self) -> Option<StateBlob> {
        Some(save(&self.messages))
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let list = load(state, self.protocol, &self.locomotives.1)?;
        self.forget_all();
        self.serial = list.front().map_or(0, |m| m.serial);
        self.total = list.len() as u64;
        self.fresh = list.iter().rev().map(|m| m.serial).collect();
        self.messages = list;
        self.trim();
        Ok(())
    }
}

/// The receiver: one decoder on the input, for the submode of `cfg`.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let protocol = protocol_of(cfg.submode.as_deref());
    let graph = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(RailwayCirDecoder::new(protocol)))
        .input("receiver")
        .output("receiver.events")
        .status_from("receiver")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "Railway CIR needs a sample rate of at least 192 ksps")
        .route(HISTORY.key(), &["receiver.history"])?
        .route(LOCOMOTIVES.key(), &["receiver.locomotives"])?
        .command("clear", CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("railway_cir.receiver", || {
        Box::new(DecoderComponent::new(RailwayCirDecoder::new(Protocol::Klw)))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = protocol_of(cfg.submode.as_deref());
    Ok(Box::new(gen::CirGenerator::new(cfg.rate_hz, p, &cfg.settings)))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, the worked KLW frame", o, 0.0);
    c.seconds = o.seconds.max(2.2);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = r
            .decl()
            .field_id(RailwayCirFields::MESSAGES.key)
            .and_then(|id| r.fields().value(id))
            .and_then(|v| if let Value::I64(n) = v { Some(*n) } else { None })
            .unwrap_or(0);
        vec![CheckLine::new(RailwayCirFields::MESSAGES.key, n >= 1, format!("{n}, at least 1 expected"))]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// Railway CIR data.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Railway CIR data",
    group: "Cellular and railway",
    blurb: "The data channel of the Chinese railway train radio: end-of-train device frames (KLW) or dispatch frames (TDCS) \
            with the train, the locomotive, the brake pipe pressure, the kilometre post and the speed.",
    tuning: Tuning {
        min_hz: 400e6,
        max_hz: 900e6,
        default_hz: TEST_SIGNAL_HZ,
        rate_hz: 960e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 35e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(150e3),
        no_offset_tuning: false,
    },
    submodes: &[
        SubMode {
            id: "klw",
            name: "End-of-train device (KLW)",
            default_hz: TEST_SIGNAL_HZ,
        },
        SubMode {
            id: "tdcs",
            name: "Dispatch (TDCS)",
            default_hz: TEST_SIGNAL_HZ,
        },
    ],
    settings: RailwayCirSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::railway_cir_benchmarks),
};

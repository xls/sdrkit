//! The `arq_packet` stack: its settings, results, receiver component (bursts, the one-way listener and the acknowledgement it
//! would send), sender component (one way with repeated passes, or two way with selective repeat), test signal and descriptor.
//! The receiver passes each acknowledgement it hears and each one it would send to the sender; the sender answers a burst that
//! asks for an acknowledgement whenever the engine's transmit switch is on.

use super::arq::{Listener, Sender, Stepper};
use super::rx::{Receiver, RxBurst};
use super::{
    ack_header, ack_payload, burst_at, burst_symbols, max_blocks, one_way, parse_ack, tx_callsign, Header, PROFILES,
    SYMBOL_RATE,
};
use crate::typed::{
    ArqPacketAckInEvent, ArqPacketAckOutEvent, ArqPacketFields, ArqPacketMessageEvent, ArqPacketMessageRow, TxEvent,
    ARQ_PACKET_ACK_IN, ARQ_PACKET_ACK_OUT, ARQ_PACKET_MESSAGE, ARQ_PACKET_MESSAGES, ARQ_PACKET_TX,
};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Packet, Record, Schema,
    Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor, StageError, Tuning, Value,
    Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format, Io, Kept, Kind, Outbox, Port,
    RateFrom, RunContext, TagKind, Timed,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "arq_packet";
/// The lowest input rate.
pub const MIN_RATE: f64 = 125_000.0;
/// What the stack says about its source.
pub const NOTE: &str = crate::slotted_voice::stack::NOTE;
/// The text that `send` transmits when the message and file settings are empty.
pub const DEMO_TEXT: &str = "Test message of the packet link.";

sdrkit_core::choice_enum! {
    /// The transmit profile.
    #[derive(Default)]
    pub enum TxProfile {
        /// BPSK 1/4.
        P0 = ("0", "0: BPSK 1/4"),
        /// QPSK 1/4.
        P1 = ("1", "1: QPSK 1/4"),
        /// QPSK 1/2.
        #[default]
        P2 = ("2", "2: QPSK 1/2"),
        /// QPSK 3/4.
        P3 = ("3", "3: QPSK 3/4"),
        /// 16-QAM 1/2.
        P4 = ("4", "4: 16-QAM 1/2"),
        /// 16-QAM 3/4.
        P5 = ("5", "5: 16-QAM 3/4"),
        /// 64-QAM 2/3.
        P6 = ("6", "6: 64-QAM 2/3"),
        /// 256-QAM 3/4.
        P7 = ("7", "7: 256-QAM 3/4"),
        /// 1024-QAM 5/6.
        P8 = ("8", "8: 1024-QAM 5/6"),
    }
}

sdrkit_core::choice_enum! {
    /// How the sender moves a message.
    #[derive(Default)]
    pub enum LinkKind {
        /// Every burst of the message, repeated in passes, without acknowledgements.
        #[default]
        OneWay = ("one_way", "One way, repeated passes"),
        /// Selective repeat: each burst asks for an acknowledgement, and the bytes not acknowledged go again.
        TwoWay = ("two_way", "Two way, selective repeat"),
    }
}

const TX: &str = "Transmit";

/// The station callsign.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Station callsign", "", "")
    .section(TX)
    .help("1 to 7 characters of A-Z, 0-9 and /. Empty sends the demonstration callsign TEST1.");
/// The transmit profile.
pub const PROFILE: Setting<field::Choice<TxProfile>> = Setting::choice("profile", "Profile", TxProfile::P2).section(TX);
/// Passes of a one-way transmission.
pub const PASSES: Setting<field::I64> = Setting::new("passes", "Passes", "", 2).range(1.0, 9.0, 1.0).section(TX);
/// One way or two way.
pub const LINK: Setting<field::Choice<LinkKind>> = Setting::choice("link", "Link", LinkKind::OneWay).section(TX);
/// The text that `send` transmits.
pub const MESSAGE: Setting<field::Text> = Setting::new("message", "Message", "", "")
    .section(TX)
    .help("The text to send. Empty sends a test text, unless a file is set.");
/// A file that `send` transmits.
pub const FILE: Setting<field::Text> = Setting::new("file", "File", "", "")
    .section(TX)
    .help("A file of 1 byte to 8 MiB to send in place of the text.");

sdrkit_core::settings! {
    /// The transmitter settings.
    pub enum TxSetting {
        /// Callsign.
        Callsign = CALLSIGN,
        /// Profile.
        Profile = PROFILE,
        /// Passes.
        Passes = PASSES,
        /// Link.
        Link = LINK,
        /// Message.
        Message = MESSAGE,
        /// File.
        File = FILE,
    }
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Schema::of(&[TxSetting::SETTINGS].concat()))).clone()
}

/// What the receiver tells the sender.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkNote {
    /// An acknowledgement burst was heard.
    Heard {
        /// The acknowledging station.
        callsign: String,
        /// The sequence number it acknowledges.
        seq: u8,
        /// The Es/N0 it reports, dB.
        snr_db: i32,
        /// Its bitmap.
        bitmap: Vec<bool>,
    },
    /// A data burst asks for an acknowledgement.
    Answer {
        /// The sequence number of the burst.
        seq: u8,
        /// The Es/N0 measured, dB.
        esn0_db: f64,
        /// The bitmap of the blocks taken.
        bitmap: Vec<bool>,
    },
}

impl sdrkit_graph::Item for LinkNote {
    const KIND: &'static str = "arq_packet.link";
}

/// The text of message bytes: printable ASCII as it is, other bytes as `\xNN`, at most 400 bytes.
pub fn printable(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b.iter().take(400) {
        if (0x20..0x7F).contains(&c) {
            s.push(char::from(c));
        } else {
            s += &format!("\\x{c:02X}");
        }
    }
    s
}

/// The `arq_packet.rx` decoder.
#[derive(Default)]
pub struct ApDecoder {
    rate: f64,
    rx: Option<Receiver>,
    listener: Listener,
    found: Vec<RxBurst>,
    samples: u64,
    good: u64,
    bad: u64,
    bursts: u64,
    messages: u64,
    repeats: u64,
    acks: u64,
    last: Option<(u8, f64, f64, String, u8)>,
    last_header: Option<f64>,
    rows: Vec<Listed>,
    row_id: u64,
    cleared: bool,
    started: bool,
}

/// A completed message for the message table.
struct Listed {
    callsign: String,
    sequence: i64,
    length: i64,
    text: String,
    time: f64,
}

impl ApDecoder {
    fn now(&self) -> f64 {
        self.samples as f64 / self.rate.max(1.0)
    }

    fn take(&mut self, out: &mut Outbox) {
        let now = self.now();
        for b in std::mem::take(&mut self.found) {
            self.bursts += 1;
            self.last_header = Some(now);
            let h = &b.header;
            self.last = Some((h.profile, b.esn0_db, b.offset_hz, h.callsign.clone(), h.seq));
            if h.is_ack() {
                self.acks += 1;
                let Some(Some(p)) = b.blocks.first() else {
                    continue;
                };
                let (snr, bitmap) = parse_ack(p);
                out.messages(1).push(ARQ_PACKET_ACK_IN.event(
                    now,
                    0.0,
                    &ArqPacketAckInEvent {
                        callsign: &h.callsign,
                        sequence: i64::from(h.seq),
                        blocks_acked: bitmap.iter().filter(|&&v| v).count() as i64,
                        snr_db: f64::from(snr),
                    },
                ));
                out.items::<LinkNote>(2).push(Timed {
                    time: now,
                    value: LinkNote::Heard { callsign: h.callsign.clone(), seq: h.seq, snr_db: snr, bitmap },
                });
                continue;
            }
            let good = b.blocks.iter().filter(|v| v.is_some()).count() as u64;
            self.good += good;
            self.bad += b.blocks.len() as u64 - good;
            let (bitmap, done) = self.listener.take(&h.callsign, &b.blocks, now);
            out.messages(1).push(ARQ_PACKET_ACK_OUT.event(
                now,
                0.0,
                &ArqPacketAckOutEvent {
                    callsign: &h.callsign,
                    sequence: i64::from(h.seq),
                    blocks: b.blocks.len() as i64,
                    blocks_acked: bitmap.iter().filter(|&&v| v).count() as i64,
                    snr_db: b.esn0_db,
                },
            ));
            if h.flags & super::FLAG_ACK_REQUEST != 0 {
                out.items::<LinkNote>(2)
                    .push(Timed { time: now, value: LinkNote::Answer { seq: h.seq, esn0_db: b.esn0_db, bitmap } });
            }
            let Some(d) = done else {
                continue;
            };
            if d.repeat {
                self.repeats += 1;
                continue;
            }
            self.messages += 1;
            let text = printable(&d.bytes);
            out.messages(1).push(ARQ_PACKET_MESSAGE.event(
                now,
                0.0,
                &ArqPacketMessageEvent {
                    callsign: &d.callsign,
                    sequence: i64::from(h.seq),
                    length: d.bytes.len() as i64,
                    text: &text,
                },
            ));
            out.bytes(0).push(Packet::new(Arc::from(&d.bytes[..]), now));
            self.rows.push(Listed {
                callsign: d.callsign.clone(),
                sequence: i64::from(h.seq),
                length: d.bytes.len() as i64,
                text,
                time: now,
            });
        }
    }
}

impl Decoder for ApDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "arq_packet.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::bytes("messages"),
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<LinkNote>("link").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Published {
        Published::new(ArqPacketFields::SHAPE.fields).with_tables(&[ARQ_PACKET_MESSAGES.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow { need: MIN_RATE, got: input.rate });
        }
        self.rate = input.rate;
        self.rx = Some(Receiver::new(input.rate));
        Ok(vec![
            Format::new(Kind::Bytes, 0.0),
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<LinkNote>(), 0.0),
        ])
    }
    fn retune(&mut self, _input: &Format) {
        if let Some(r) = self.rx.as_mut() {
            r.reset();
        }
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.samples += x.len() as u64;
        if let Some(r) = self.rx.as_mut() {
            r.process(x, &mut self.found);
        }
        self.take(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        if let Some(r) = self.rx.as_mut() {
            r.finish(&mut self.found);
        }
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&ARQ_PACKET_MESSAGES);
        }
        let receiving = self.last_header.is_some_and(|t| self.now() - t <= 5.0);
        let last = self.last.as_ref();
        p.put(&ArqPacketFields {
            blocks_good: self.good as i64,
            blocks_bad: self.bad as i64,
            profile: last.map(|l| i64::from(l.0)),
            snr_db: last.map(|l| l.1),
            offset_hz: last.map(|l| Hertz(l.2)),
            callsign: last.map(|l| l.3.as_str()),
            bursts: self.bursts as i64,
            messages: self.messages as i64,
            repeats: self.repeats as i64,
            acks_heard: self.acks as i64,
            locked: receiving,
            note: NOTE,
            sequence: last.map(|l| i64::from(l.4)),
        });
        for r in std::mem::take(&mut self.rows) {
            self.row_id += 1;
            ARQ_PACKET_MESSAGES.put(
                p,
                &format!("{}", self.row_id),
                &ArqPacketMessageRow {
                    callsign: &r.callsign,
                    sequence: r.sequence,
                    length: r.length,
                    text: &r.text,
                    time: Time(r.time),
                },
            );
        }
        let (lock, summary) = if receiving {
            let call = self.last.as_ref().map_or("", |l| l.3.as_str());
            (LockState::Locked, format!("Receiving bursts from {call}"))
        } else if self.started {
            (LockState::Searching, format!("Listening; {} messages", self.messages))
        } else {
            (LockState::Off, String::new())
        };
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            s.lock_state = lock;
            s.summary.clone_from(&summary);
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.5
    }
    fn clear(&mut self) {
        self.good = 0;
        self.bad = 0;
        self.bursts = 0;
        self.messages = 0;
        self.repeats = 0;
        self.acks = 0;
        self.last = None;
        self.listener.clear();
        self.cleared = true;
    }
    fn forget(&mut self, kept: &Kept) {
        if let Some(r) = self.rx.as_mut() {
            r.reset();
        }
        self.found.clear();
        self.good = 0;
        self.bad = 0;
        self.bursts = 0;
        self.repeats = 0;
        self.acks = 0;
        self.last = None;
        self.last_header = None;
        self.listener.clear();
        if !kept.field("messages") {
            self.messages = 0;
        }
        if !kept.table(ARQ_PACKET_MESSAGES.key) {
            self.rows.clear();
            self.cleared = true;
        }
    }
}

/// Why `send` was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// A transmission is under way.
    Busy,
    /// The output rate is below 125 ksps.
    Rate,
    /// The callsign has more than 7 characters or another character than A-Z, 0-9 and /.
    Callsign,
    /// The file cannot be read, is empty or is larger than 8 MiB.
    File,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TxError::Off => "the transmitter is off",
            TxError::Busy => "a transmission is under way",
            TxError::Rate => "the transmit rate must be at least 125 ksps",
            TxError::Callsign => "the callsign must be 1 to 7 characters of A-Z, 0-9 and /",
            TxError::File => "the file cannot be read, is empty or is larger than 8 MiB",
        })
    }
}

impl std::error::Error for TxError {}

/// Starts a transmission.
pub const SEND: Command = Command::new("send", "Send")
    .help("Sends the message (the text, the file or a test text) once. Transmit must be on.")
    .transmits(sdrkit_core::TxContent::Text)
    .stopped_by("stop");
/// Ends the transmission.
pub const STOP: Command = Command::new("stop", "Stop").help("Ends the transmission after the burst on the air.");

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Send.
        Send = SEND,
        /// Stop.
        Stop = STOP,
    }
}

/// The two-way session of a sender.
struct Session {
    sender: Sender,
    stepper: Stepper,
    /// The time (sender seconds) after which the burst in flight counts as lost.
    deadline: Option<f64>,
    /// Blocks of the burst in flight.
    blocks: usize,
    /// Time-outs in a row.
    misses: u32,
}

/// The sender (`arq_packet.tx`).
#[derive(Default)]
pub struct ApSender {
    callsign: String,
    profile: TxProfile,
    passes: i64,
    link: LinkKind,
    message: String,
    file: String,
    rate: f64,
    transmit: bool,
    pending: Vec<Complex32>,
    at: usize,
    answers: Vec<Vec<Complex32>>,
    session: Option<Session>,
    seq: u8,
    stopping: bool,
    samples: u64,
    events: Vec<sdrkit_core::Event>,
}

impl ApSender {
    fn event(&mut self, state: &str, reason: &str) {
        self.events.push(ARQ_PACKET_TX.event(0.0, 0.0, &TxEvent { state, reason }));
    }

    fn now(&self) -> f64 {
        self.samples as f64 / self.rate.max(1.0)
    }

    fn busy(&self) -> bool {
        self.at < self.pending.len() || self.session.is_some()
    }

    /// The bytes that `send` transmits.
    fn bytes(&self) -> Result<Vec<u8>, TxError> {
        if !self.file.is_empty() {
            let b = std::fs::read(&self.file).map_err(|_| TxError::File)?;
            if b.is_empty() || b.len() > super::arq::MAX_MESSAGE {
                return Err(TxError::File);
            }
            return Ok(b);
        }
        Ok(if self.message.is_empty() { DEMO_TEXT.as_bytes().to_vec() } else { self.message.as_bytes().to_vec() })
    }

    /// Starts a transmission.
    pub fn send(&mut self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.busy() {
            return Err(TxError::Busy);
        }
        if self.rate < MIN_RATE {
            return Err(TxError::Rate);
        }
        let call = tx_callsign(&self.callsign).ok_or(TxError::Callsign)?;
        let bytes = self.bytes()?;
        let profile = usize::from(self.profile.index());
        self.stopping = false;
        match self.link {
            LinkKind::OneWay => {
                let (x, seq) = one_way(&bytes, &call, profile, self.passes.clamp(1, 9) as usize, self.rate, self.seq);
                self.seq = seq;
                self.pending = x;
                self.at = 0;
            }
            LinkKind::TwoWay => {
                self.session =
                    Some(Session { sender: Sender::new(&bytes, self.seq), stepper: Stepper::new(profile), deadline: None, blocks: 0, misses: 0 });
                self.next_burst();
            }
        }
        Ok(())
    }

    /// Puts the next burst of the two-way session on the air, or ends the session.
    fn next_burst(&mut self) {
        let call = tx_callsign(&self.callsign).unwrap_or_default();
        let rate = self.rate;
        let Some(s) = self.session.as_mut() else {
            return;
        };
        let p = s.stepper.profile;
        let limit = max_blocks(&PROFILES[p], 1000.0).max(1);
        let Some(b) = (!self.stopping).then(|| s.sender.next_burst(p, limit)).flatten() else {
            let done = s.sender.done();
            self.session = None;
            self.event("finished", if done { "" } else { "stopped" });
            return;
        };
        self.seq = b.seq;
        let h = Header { callsign: call, profile: p as u8, blocks: b.blocks.len() as u16, seq: b.seq, flags: b.flags };
        let gap = vec![Complex32::new(0.0, 0.0); (0.03 * rate) as usize];
        let mut x = gap.clone();
        x.extend(burst_at(&h, &b.payloads(), rate));
        x.extend(gap);
        let air = x.len() as f64 / rate;
        s.deadline = Some(self.samples as f64 / rate + air + ack_wait_s());
        s.blocks = b.blocks.len();
        self.pending = x;
        self.at = 0;
    }

    fn take_notes(&mut self, notes: &[Timed<LinkNote>]) {
        for n in notes {
            match &n.value {
                LinkNote::Heard { seq, snr_db, bitmap, .. } => {
                    let Some(s) = self.session.as_mut() else {
                        continue;
                    };
                    if !s.sender.on_ack(*seq, bitmap) {
                        continue;
                    }
                    let sent = s.blocks;
                    let good = bitmap.iter().take(sent).filter(|&&v| v).count();
                    s.stepper.on_ack(f64::from(*snr_db), sent - good, sent);
                    s.deadline = None;
                    s.misses = 0;
                    if self.at >= self.pending.len() {
                        self.next_burst();
                    }
                }
                LinkNote::Answer { seq, esn0_db, bitmap } => {
                    if !self.transmit {
                        continue;
                    }
                    let Some(call) = tx_callsign(&self.callsign) else {
                        continue;
                    };
                    let mut x = vec![Complex32::new(0.0, 0.0); (0.03 * self.rate) as usize];
                    x.extend(burst_at(&ack_header(&call, *seq), &[ack_payload(*esn0_db, bitmap)], self.rate));
                    self.answers.push(x);
                }
            }
        }
    }
}

/// Time-outs in a row after which a two-way transmission ends.
pub const MAX_MISSES: u32 = 6;

/// The time a sender waits after a burst before it counts the burst as lost: 60 ms of silence, 150 ms of turnaround and the
/// air time of an acknowledgement.
pub fn ack_wait_s() -> f64 {
    0.06 + 0.15 + burst_symbols(&PROFILES[0], 1) as f64 / SYMBOL_RATE
}

impl Component for ApSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "arq_packet.tx",
            vec![Port::iq("in"), Port::typed::<LinkNote>("link")],
            vec![Port::iq("out"), Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let dial = if f.tuned_hz > 0.0 { f.tuned_hz } else { f.centre_hz };
        Ok(vec![Format::iq(self.rate, dial, dial), Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        if !ctx.transmit && self.busy() {
            self.pending.clear();
            self.at = 0;
            self.session = None;
            self.answers.clear();
            self.event("finished", "the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        self.samples += room as u64;
        let notes = io.items::<LinkNote>(1).to_vec();
        self.take_notes(&notes);
        if self.at >= self.pending.len() {
            if let Some(a) = (!self.answers.is_empty()).then(|| self.answers.remove(0)) {
                self.pending = a;
                self.at = 0;
            } else if let Some(s) = self.session.as_ref() {
                match s.deadline {
                    // An acknowledgement came while another burst was on the air: the next burst goes now.
                    None => self.next_burst(),
                    Some(d) if self.now() > d => {
                        let mut give_up = false;
                        if let Some(s) = self.session.as_mut() {
                            s.sender.on_timeout();
                            s.stepper.on_timeout();
                            s.misses += 1;
                            give_up = s.misses >= MAX_MISSES;
                        }
                        if give_up {
                            self.session = None;
                            self.event("finished", "no acknowledgement after 6 bursts in a row");
                        } else {
                            self.next_burst();
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        if self.at < self.pending.len() {
            let n = room.min(self.pending.len() - self.at);
            let chunk = self.pending[self.at..self.at + n].to_vec();
            io.iq_out(0).extend(chunk);
            self.at += n;
            if self.at >= self.pending.len() {
                if let Some(last) = io.iq_out(0).len().checked_sub(1) {
                    io.output(0).tag_here(last, TagKind::BurstEnd);
                }
                self.pending.clear();
                self.at = 0;
                if self.session.is_none() && self.answers.is_empty() {
                    self.event("finished", "");
                }
            }
        }
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => self.callsign = CALLSIGN.read(id, v)?.to_owned(),
            TxSetting::Profile => self.profile = PROFILE.read(id, v)?,
            TxSetting::Passes => self.passes = PASSES.read(id, v)?,
            TxSetting::Link => self.link = LINK.read(id, v)?,
            TxSetting::Message => self.message = MESSAGE.read(id, v)?.to_owned(),
            TxSetting::File => self.file = FILE.read(id, v)?.to_owned(),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (TxSetting::Callsign, Value::Text(self.callsign.clone())),
            (TxSetting::Profile, Value::Choice(self.profile.index())),
            (TxSetting::Passes, Value::I64(self.passes.clamp(1, 9))),
            (TxSetting::Link, Value::Choice(self.link.index())),
            (TxSetting::Message, Value::Text(self.message.clone())),
            (TxSetting::File, Value::Text(self.file.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Send => match self.send() {
                Ok(()) => self.event("started", ""),
                Err(e) => {
                    self.event("refused", &e.to_string());
                    return Err(CommandError::Failed { source: StageError::new(e) });
                }
            },
            TxCommand::Stop => {
                self.stopping = true;
                if self.session.is_none() && self.at < self.pending.len() {
                    // A one-way transmission ends after the burst on the air: drop what follows its end.
                    let rest = &self.pending[self.at..];
                    let quiet = rest.iter().position(|v| v.norm_sqr() == 0.0).unwrap_or(rest.len());
                    self.pending.truncate(self.at + quiet);
                }
            }
        }
        Ok(None)
    }
}

/// The texts of the test signal.
pub const TEST_TEXTS: [&str; 3] = [
    "First test message of the packet link.",
    "Second test message, with the digits 0123456789.",
    "Third test message: the quick brown fox jumps over the lazy dog.",
];

/// The test signal: the three test texts from TEST1, each sent one way in the passes and at the profile of the settings, one
/// after the other with 1 s between them, at unit power, again and again.
pub struct ApGenerator {
    rate: f64,
    signal: Vec<Complex32>,
    at: usize,
}

impl Generator for ApGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(100_000.0).with_symbol_rate(SYMBOL_RATE))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            *o = self.signal[self.at];
            self.at = (self.at + 1) % self.signal.len();
        }
        out.len()
    }
}

/// The generator of the stack.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let profile = usize::from(s.get(&PROFILE).index());
    let passes = s.get(&PASSES).clamp(1, 9) as usize;
    let rate = cfg.rate_hz;
    let mut signal = Vec::new();
    let mut seq = 0;
    for t in TEST_TEXTS {
        let (x, q) = one_way(t.as_bytes(), crate::family::DEMO_CALLSIGN, profile, passes, rate, seq);
        seq = q;
        signal.extend(x);
        signal.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), rate as usize));
    }
    let g = 1.0 / super::TX_LEVEL;
    for v in &mut signal {
        *v *= g;
    }
    Ok(Box::new(ApGenerator { rate, signal, at: 0 }))
}

/// The receiver and transmitter graph.
///
/// # Errors
///
/// The errors of the graph and of a refused setting.
pub fn graph(cfg: &StackConfig) -> Result<sdrkit_graph::StackGraph, sdrkit_graph::GraphError> {
    use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, StackGraph};
    let b = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("rx", DecoderComponent::new(ApDecoder::default()))
        .add("sender", ApSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("rx")
        .input("sender.in")
        .connect("rx.link", "sender.link")
        .connect("sender.out", "tx.in")
        .output("rx.messages")
        .output("rx.events")
        .output("sender.events")
        .status_from("rx")
        .clock(0.5);
    let mut g = StackGraph::of(crate::typed::shape(ID), b.build()?)
        .min_rate(MIN_RATE, "arq_packet needs a sample rate of at least 125 ksps");
    for s in TxSetting::SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("sender.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("send", CommandTarget::of("sender", "send"))
        .command("stop", CommandTarget::of("sender", "stop"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    Ok(Box::new(graph(cfg)?))
}

/// The stack.
pub static ARQ_PACKET: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Packet link with selective repeat",
    group: crate::GROUP,
    blurb: "Burst packets 100 kHz wide at 80 ksymbols a second, in nine profiles from BPSK rate 1/4 to 1024-QAM rate 5/6, \
            with LDPC blocks; a message goes one way in repeated passes or two way with acknowledgement bursts and selective \
            repeat. Receive and transmit.",
    tuning: Tuning { min_hz: 30e6, max_hz: 6000e6, ..Tuning::anywhere(433.0e6, 250e3, MIN_RATE, 100e3) },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ARQ_PACKET_VERIFY),
    benchmarks: Some(crate::verify::arq_packet_benchmarks),
};

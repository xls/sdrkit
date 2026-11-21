//! The `file_transfer` stack: its settings, results, receiver component (frames, bursts, the receiver side of the session,
//! files saved through the host's storage), sender component (the one-way carousel, or the two-way session), test signal
//! and descriptor. The receiver passes the replies it would send and the messages for a sender to the sender component; the
//! sender puts replies on the air whenever the engine's transmit switch is on.

use super::rx::{Receiver, RxFrame};
use super::session::{
    join_message, safe_name, unique_name, Carousel, CarouselItem, Collector, Message, RxBurst, RxData, RxEvent, Sender,
    SenderConfig, SenderOut, SenderState, SessionReceiver,
};
use super::{callsign_chars_ok, frames_at, FrameKind, Width, TX_LEVEL, WIDTHS};
use crate::typed::{
    FileTransferFields, FileTransferFileRow, FileTransferOpenEvent, FileTransferReceivedEvent, TxEvent,
    FILE_TRANSFER_FILES, FILE_TRANSFER_OPEN, FILE_TRANSFER_RECEIVED, FILE_TRANSFER_TX,
};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Packet, Record, Schema,
    Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor, StageError, Storage,
    StorePath, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format, Io, Kept, Kind, Outbox, Port,
    RateFrom, RunContext, TagKind, Timed,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "file_transfer";
/// The lowest input rate.
pub const MIN_RATE: f64 = 1_250_000.0;
/// The largest file the receiver takes, bytes.
pub const RX_LIMIT: u64 = 256 << 20;
/// The largest file the sender reads, bytes.
pub const TX_LIMIT: usize = 256 << 20;
/// What the stack says about its source.
pub const NOTE: &str = crate::slotted_voice::stack::NOTE;
/// The text that `send` transmits when no file is set, and the file of the test signal.
pub const DEMO_TEXT: &str = "Test file of the file transfer link. It goes over the air in LDPC blocks, each with its own \
                             CRC-32, and the receiver checks the hash of the whole file before it saves it.\n";
/// The name of the demonstration file.
pub const DEMO_NAME: &str = "demo.txt";
/// The session id of the test signal.
pub const TEST_SESSION: u16 = 4242;
/// Silence before and after each control frame and burst, seconds.
pub const GAP_S: f64 = 0.02;
/// The time a two-way sender waits for a reply after its transmission ends, seconds.
pub const REPLY_WAIT_S: f64 = 0.5;
/// The turnaround of a reply, seconds.
pub const TURNAROUND_S: f64 = 0.025;

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
        /// 16-QAM 3/4.
        P4 = ("4", "4: 16-QAM 3/4"),
        /// 64-QAM 3/4.
        P5 = ("5", "5: 64-QAM 3/4"),
        /// 256-QAM 3/4.
        P6 = ("6", "6: 256-QAM 3/4"),
        /// 256-QAM 5/6.
        P7 = ("7", "7: 256-QAM 5/6"),
        /// 1024-QAM 5/6.
        P8 = ("8", "8: 1024-QAM 5/6"),
        /// 1024-QAM 9/10.
        P9 = ("9", "9: 1024-QAM 9/10"),
    }
}

sdrkit_core::choice_enum! {
    /// The transmit width.
    #[derive(Default)]
    pub enum TxWidth {
        /// 1 MHz.
        W1M = ("1m", "1 MHz"),
        /// 500 kHz.
        W500K = ("500k", "500 kHz"),
        /// 200 kHz.
        #[default]
        W200K = ("200k", "200 kHz"),
    }
}

impl TxWidth {
    /// The width.
    pub fn width(self) -> Width {
        Width::of(self.index() as u8).unwrap_or(Width::W200K)
    }
}

sdrkit_core::choice_enum! {
    /// The widths the receiver searches.
    #[derive(Default)]
    pub enum RxWidths {
        /// All three at once.
        #[default]
        All = ("all", "All widths"),
        /// 1 MHz only.
        W1M = ("1m", "1 MHz only"),
        /// 500 kHz only.
        W500K = ("500k", "500 kHz only"),
        /// 200 kHz only.
        W200K = ("200k", "200 kHz only"),
    }
}

impl RxWidths {
    /// The widths.
    pub fn widths(self) -> Vec<Width> {
        match self {
            RxWidths::All => WIDTHS.to_vec(),
            RxWidths::W1M => vec![Width::W1M],
            RxWidths::W500K => vec![Width::W500K],
            RxWidths::W200K => vec![Width::W200K],
        }
    }
}

sdrkit_core::choice_enum! {
    /// How the sender moves a file.
    #[derive(Default)]
    pub enum LinkKind {
        /// The file in passes, without replies.
        #[default]
        OneWay = ("one_way", "One way, repeated passes"),
        /// A session with replies: bursts are acknowledged and missing bytes go again.
        TwoWay = ("two_way", "Two way, acknowledged"),
    }
}

const RX: &str = "Receive";
const TX: &str = "Transmit";

/// The station callsign.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Station callsign", "", "").help(
    "1 to 6 characters of A-Z, 0-9, /, - and . that the sender sends; empty sends TEST1. When set, the stack takes \
     only a file sent to this callsign or to CQ.",
);
/// The widths the receiver searches.
pub const WIDTHS_SETTING: Setting<field::Choice<RxWidths>> =
    Setting::choice("widths", "Widths searched", RxWidths::All).section(RX);
/// The storage folder of received files.
pub const FOLDER: Setting<field::Text> = Setting::new("folder", "Folder", "", "file_transfer")
    .section(RX)
    .help("The folder in the host's storage where received files are saved. Empty saves at the top.");
/// The transmit width.
pub const WIDTH: Setting<field::Choice<TxWidth>> = Setting::choice("width", "Width", TxWidth::W200K).section(TX);
/// The transmit profile.
pub const PROFILE: Setting<field::Choice<TxProfile>> = Setting::choice("profile", "Profile", TxProfile::P2).section(TX);
/// Passes of a one-way transmission.
pub const PASSES: Setting<field::I64> = Setting::new("passes", "Passes", "", 2).range(1.0, 9.0, 1.0).section(TX);
/// One way or two way.
pub const LINK: Setting<field::Choice<LinkKind>> = Setting::choice("link", "Link", LinkKind::OneWay).section(TX);
/// The file that `send` transmits.
pub const FILE: Setting<field::Text> = Setting::new("file", "File", "", "")
    .section(TX)
    .help("A file in the host's storage, 1 byte to 256 MiB. Empty sends a short test text.");
/// The receiving station of a two-way session.
pub const RECIPIENT: Setting<field::Text> = Setting::new("recipient", "Recipient", "", "CQ")
    .section(TX)
    .help("The callsign of the receiving station of a two-way session; CQ for anyone.");
/// The session id.
pub const SESSION: Setting<field::I64> = Setting::new("session", "Session id", "", 0)
    .range(0.0, 65_000.0, 1.0)
    .section(TX)
    .help("1 to 65000; 0 takes a number from the clock.");
/// Keep the profile of the setting in a two-way session.
pub const FIXED_PROFILE: Setting<field::Bool> = Setting::new("fixed_profile", "Fixed profile", "", false)
    .section(TX)
    .help("Off: a two-way sender takes the profile that the receiver asks for.");

sdrkit_core::settings! {
    /// The receiver settings.
    pub enum RxSetting {
        /// Callsign.
        Callsign = CALLSIGN,
        /// Widths searched.
        Widths = WIDTHS_SETTING,
        /// Folder.
        Folder = FOLDER,
    }
}

sdrkit_core::settings! {
    /// The transmitter settings.
    pub enum TxSetting {
        /// Callsign.
        Callsign = CALLSIGN,
        /// Width.
        Width = WIDTH,
        /// Profile.
        Profile = PROFILE,
        /// Passes.
        Passes = PASSES,
        /// Link.
        Link = LINK,
        /// File.
        File = FILE,
        /// Recipient.
        Recipient = RECIPIENT,
        /// Session.
        Session = SESSION,
        /// Fixed profile.
        FixedProfile = FIXED_PROFILE,
    }
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    S.get_or_init(|| Arc::new(crate::typed::shape(ID).schema())).clone()
}

/// What the receiver tells the sender.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkNote {
    /// A message for a sender (ACCEPT, ACK, VERIFIED, REJECT, ABORT) was heard.
    Heard(Message),
    /// The receiver side answers with this message, in a frame of this width.
    Reply(Message, Width),
}

impl sdrkit_graph::Item for LinkNote {
    const KIND: &'static str = "file_transfer.link";
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}

/// The `file_transfer.rx` decoder.
pub struct FtDecoder {
    rate: f64,
    widths: RxWidths,
    folder: String,
    callsign: String,
    rx: Option<Receiver>,
    collector: Collector,
    session: SessionReceiver,
    storage: Arc<dyn Storage>,
    frames: Vec<RxFrame>,
    bursts: Vec<RxBurst>,
    replies: Vec<Message>,
    events: Vec<RxEvent>,
    samples: u64,
    good: u64,
    bad: u64,
    frame_count: u64,
    acks: u64,
    files: u64,
    held: u64,
    hash_ok: Option<bool>,
    last: Option<RxFrame>,
    last_time: Option<f64>,
    rows: Vec<(String, FileState)>,
    cleared: bool,
    started: bool,
}

/// The state of a file for the file table.
struct FileState {
    name: String,
    size: i64,
    hash: String,
    state: String,
    callsign: String,
    time: f64,
}

impl Default for FtDecoder {
    fn default() -> Self {
        FtDecoder {
            rate: 0.0,
            widths: RxWidths::All,
            folder: FOLDER.default.to_string(),
            callsign: String::new(),
            rx: None,
            collector: Collector::default(),
            session: SessionReceiver::with_limit(RX_LIMIT),
            storage: Arc::new(sdrkit_core::MemStorage::new()),
            frames: Vec::new(),
            bursts: Vec::new(),
            replies: Vec::new(),
            events: Vec::new(),
            samples: 0,
            good: 0,
            bad: 0,
            frame_count: 0,
            acks: 0,
            files: 0,
            held: 0,
            hash_ok: None,
            last: None,
            last_time: None,
            rows: Vec::new(),
            cleared: false,
            started: false,
        }
    }
}

impl FtDecoder {
    fn now(&self) -> f64 {
        self.samples as f64 / self.rate.max(1.0)
    }

    fn rebuild(&mut self) {
        if self.rate > 0.0 {
            self.rx = Some(Receiver::new(self.rate, &self.widths.widths()));
        }
    }

    /// Saves a delivered file under its safe name in the folder; returns the path or why it failed.
    fn save(&self, name: &str, bytes: &[u8]) -> Result<String, String> {
        let folder = self.folder.trim().trim_matches('/');
        let base = if folder.is_empty() { Ok(StorePath::root()) } else { StorePath::new(folder) };
        let base = base.map_err(|e| format!("bad folder: {e}"))?;
        let at = |n: &str| base.join(n);
        let safe = safe_name(name);
        let free = unique_name(&safe, |n| at(n).map_or(true, |p| self.storage.stat(&p).is_ok()));
        let path = at(&free).map_err(|e| format!("bad name: {e}"))?;
        self.storage.write(&path, bytes).map_err(|e| format!("cannot save {free}: {e}"))?;
        Ok(path.to_string())
    }

    fn row(&mut self, session: u16, name: &str, size: u64, hash: &[u8; 32], state: &str, call: &str, now: f64) {
        self.rows.push((
            format!("{session}:{}", hex(&hash[..8])),
            FileState {
                name: name.to_string(),
                size: size as i64,
                hash: hex(hash),
                state: state.to_string(),
                callsign: call.to_string(),
                time: now,
            },
        ));
    }

    fn take(&mut self, out: &mut Outbox) {
        let now = self.now();
        let mut frames = std::mem::take(&mut self.frames);
        frames.sort_by(|a, b| a.time.total_cmp(&b.time));
        for f in frames {
            self.frame_count += 1;
            self.last_time = Some(now);
            let good = f.blocks.iter().filter(|b| b.is_some()).count() as u64;
            self.good += good;
            self.bad += f.blocks.len() as u64 - good;
            let h = &f.header;
            match h.kind {
                FrameKind::Data => {
                    let d = RxData {
                        index: h.index,
                        last: h.last,
                        profile: h.profile,
                        blocks: f.blocks.clone(),
                        esn0_db: f.esn0_db,
                    };
                    self.collector.take(h.session, h.frame, d, now, &mut self.bursts);
                }
                FrameKind::Control | FrameKind::Ack => {
                    if h.kind == FrameKind::Ack {
                        self.acks += 1;
                    }
                    if let Some(m) = join_message(&f.blocks) {
                        self.message(&m, &f, out, now);
                    }
                }
            }
            self.last = Some(f);
        }
        self.collector.tick(now, &mut self.bursts);
        self.bursts_out(out, now);
    }

    fn bursts_out(&mut self, out: &mut Outbox, now: f64) {
        let width = self.last.as_ref().map_or(Width::W200K, |f| f.width);
        for b in std::mem::take(&mut self.bursts) {
            self.session.on_burst(&b, &mut self.replies);
        }
        self.replies_out(out, now, width);
    }

    fn replies_out(&mut self, out: &mut Outbox, now: f64, width: Width) {
        for m in std::mem::take(&mut self.replies) {
            out.items::<LinkNote>(2).push(Timed { time: now, value: LinkNote::Reply(m, width) });
        }
    }

    fn message(&mut self, m: &Message, f: &RxFrame, out: &mut Outbox, now: f64) {
        match m {
            Message::Accept { .. } | Message::Ack(_) | Message::Verified { .. } | Message::Reject { .. } => {
                out.items::<LinkNote>(2).push(Timed { time: now, value: LinkNote::Heard(m.clone()) });
                return;
            }
            Message::Abort { .. } => {
                out.items::<LinkNote>(2).push(Timed { time: now, value: LinkNote::Heard(m.clone()) });
            }
            Message::Finish { .. } => {
                self.collector.flush(&mut self.bursts);
                self.bursts_out(out, now);
            }
            _ => {}
        }
        self.session.on_message(m, Some(f.esn0_db), &mut self.replies, &mut self.events);
        self.replies_out(out, now, f.width);
        for e in std::mem::take(&mut self.events) {
            match e {
                RxEvent::Opened { session, sender, name, size, held } => {
                    self.held = held;
                    out.messages(1).push(FILE_TRANSFER_OPEN.event(
                        now,
                        0.0,
                        &FileTransferOpenEvent {
                            session: i64::from(session),
                            name: &name,
                            size: size as i64,
                            held_bytes: held as i64,
                            callsign: &sender,
                        },
                    ));
                    let hash = self.session.active_info().map_or([0; 32], |a| a.2);
                    self.row(session, &name, size, &hash, "receiving", &sender, now);
                }
                RxEvent::Delivered { session, sender, name, bytes, hash } => {
                    self.files += 1;
                    self.hash_ok = Some(true);
                    let (state, path) = match self.save(&name, &bytes) {
                        Ok(p) => ("verified".to_string(), p),
                        Err(e) => (format!("verified; {e}"), String::new()),
                    };
                    self.row(session, &name, bytes.len() as u64, &hash, &state, &sender, now);
                    out.messages(1).push(FILE_TRANSFER_RECEIVED.event(
                        now,
                        0.0,
                        &FileTransferReceivedEvent {
                            name: &name,
                            size: bytes.len() as i64,
                            hash: &hex(&hash),
                            path: &path,
                            callsign: &sender,
                        },
                    ));
                    out.bytes(0).push(Packet::new(Arc::from(&bytes[..]), now));
                }
                RxEvent::HashMismatch { session } => {
                    self.hash_ok = Some(false);
                    if let Some((name, call, hash)) =
                        self.session.active_info().map(|(n, c, h)| (n.to_string(), c.to_string(), h))
                    {
                        let size = self.session.active().map_or(0, |a| a.1);
                        self.row(session, &name, size, &hash, "hash mismatch; bytes dropped", &call, now);
                    }
                }
                RxEvent::Missing { .. } | RxEvent::Refused { .. } => {}
            }
        }
    }
}

impl Decoder for FtDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "file_transfer.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::bytes("files"),
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<LinkNote>("link").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (RxSetting::Callsign, Value::Text(self.callsign.clone())),
            (RxSetting::Widths, Value::Choice(self.widths.index())),
            (RxSetting::Folder, Value::Text(self.folder.clone())),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RxSetting::Callsign => {
                self.callsign = CALLSIGN.read(id, v)?.trim().to_ascii_uppercase();
                self.session.callsign.clone_from(&self.callsign);
            }
            RxSetting::Widths => {
                let w = WIDTHS_SETTING.read(id, v)?;
                if w != self.widths {
                    self.widths = w;
                    self.rebuild();
                }
            }
            RxSetting::Folder => self.folder = FOLDER.read(id, v)?.to_owned(),
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(FileTransferFields::SHAPE.fields).with_tables(&[FILE_TRANSFER_FILES.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow { need: MIN_RATE, got: input.rate });
        }
        self.rate = input.rate;
        self.rebuild();
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
    fn attach(&mut self, ctx: &RunContext) {
        self.storage = Arc::clone(&ctx.storage);
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.samples += x.len() as u64;
        if let Some(r) = self.rx.as_mut() {
            r.process(x, &mut self.frames);
        }
        self.take(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        if let Some(r) = self.rx.as_mut() {
            r.finish(&mut self.frames);
        }
        self.take(out);
        self.collector.flush(&mut self.bursts);
        let now = self.now();
        self.bursts_out(out, now);
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&FILE_TRANSFER_FILES);
        }
        let active = self.session.active();
        let receiving = self.last_time.is_some_and(|t| self.now() - t <= 5.0);
        let last = self.last.as_ref();
        p.put(&FileTransferFields {
            session: active.map(|a| i64::from(a.0)),
            blocks_good: self.good as i64,
            blocks_bad: self.bad as i64,
            frames: self.frame_count as i64,
            progress: active.map(|(_, size, have, done)| {
                if done || size == 0 {
                    100.0
                } else {
                    100.0 * have as f64 / size as f64
                }
            }),
            hash_ok: self.hash_ok,
            profile: last.map(|f| i64::from(f.header.profile)),
            width: last.map(|f| f.width.name()),
            snr_db: last.map(|f| f.esn0_db),
            offset_hz: last.map(|f| Hertz(f.offset_hz)),
            callsign: last.map(|f| f.header.callsign.as_str()),
            files: self.files as i64,
            held_bytes: self.held as i64,
            acks_heard: self.acks as i64,
            locked: receiving,
            note: NOTE,
        });
        for (k, r) in std::mem::take(&mut self.rows) {
            FILE_TRANSFER_FILES.put(
                p,
                &k,
                &FileTransferFileRow {
                    name: &r.name,
                    size: r.size,
                    hash: &r.hash,
                    state: &r.state,
                    callsign: &r.callsign,
                    time: Time(r.time),
                },
            );
        }
        let (lock, summary) = if receiving {
            let call = self.last.as_ref().map_or("", |f| f.header.callsign.as_str());
            (LockState::Locked, format!("Receiving frames from {call}"))
        } else if self.started {
            (LockState::Searching, format!("Listening; {} files", self.files))
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
        self.frame_count = 0;
        self.acks = 0;
        self.files = 0;
        self.held = 0;
        self.hash_ok = None;
        self.last = None;
        self.session.clear();
        self.collector = Collector::default();
        self.cleared = true;
    }
    fn forget(&mut self, kept: &Kept) {
        if let Some(r) = self.rx.as_mut() {
            r.reset();
        }
        self.frames.clear();
        self.bursts.clear();
        self.replies.clear();
        self.events.clear();
        self.good = 0;
        self.bad = 0;
        self.frame_count = 0;
        self.acks = 0;
        self.held = 0;
        self.hash_ok = None;
        self.last = None;
        self.last_time = None;
        self.session.clear();
        self.collector = Collector::default();
        if !kept.field("files") {
            self.files = 0;
        }
        if !kept.table(FILE_TRANSFER_FILES.key) {
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
    /// The output rate is below the least rate of the width.
    Rate,
    /// The callsign has more than 6 characters or a character outside the alphabet.
    Callsign,
    /// The file cannot be read, is empty or is larger than 256 MiB.
    File,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TxError::Off => "the transmitter is off",
            TxError::Busy => "a transmission is under way",
            TxError::Rate => "the transmit rate is below 1.25 times the width",
            TxError::Callsign => "the callsign must be 1 to 6 characters of A-Z, 0-9, /, - and .",
            TxError::File => "the file cannot be read, is empty or is larger than 256 MiB",
        })
    }
}

impl std::error::Error for TxError {}

/// Starts a transmission.
pub const SEND: Command =
    Command::new("send", "Send")
        .help("Sends the file (or a test text) one way or in a two-way session. Transmit must be on.")
        .transmits(sdrkit_core::TxContent::File)
        .stopped_by("stop");
/// Ends the transmission.
pub const STOP: Command = Command::new("stop", "Stop").help("Ends the transmission after the frame or burst on the air.");

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Send.
        Send = SEND,
        /// Stop.
        Stop = STOP,
    }
}

/// The callsign the sender transmits: the setting in upper case without trailing spaces, or TEST1 when it is empty; `None`
/// when it has more than 6 characters or one outside the alphabet.
pub fn tx_callsign(setting: &str) -> Option<String> {
    let s = setting.trim_end().to_ascii_uppercase();
    if s.is_empty() {
        return Some(crate::family::DEMO_CALLSIGN.to_string());
    }
    (s.len() <= 6 && callsign_chars_ok(&s)).then_some(s)
}

/// The name a sender announces: the text after the last "/" or "\", at most its last 90 characters.
pub fn tx_name(path: &str) -> String {
    let base = path.rsplit(['/', '\\']).next().unwrap_or("");
    let chars: Vec<char> = base.chars().collect();
    chars[chars.len().saturating_sub(90)..].iter().collect()
}

/// The samples of one carousel item at `rate`: 20 ms of silence, the frames, 20 ms of silence (or the silence alone).
pub fn item_samples(item: &CarouselItem, session: u16, width: Width, call: &str, frame_no: &mut u16, rate: f64) -> Vec<Complex32> {
    let gap = (GAP_S * rate).round() as usize;
    let frames = match item {
        CarouselItem::Silence(s) => return vec![Complex32::new(0.0, 0.0); (s * rate).round() as usize],
        CarouselItem::Control(m) => {
            let f = super::session::control_frame(m, width, call, *frame_no);
            *frame_no = frame_no.wrapping_add(1);
            f.into_iter().collect::<Vec<_>>()
        }
        CarouselItem::Burst(b) => b.tx_frames(session, width, call),
    };
    let mut x = vec![Complex32::new(0.0, 0.0); gap];
    x.extend(frames_at(&frames, rate));
    x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), gap));
    x
}

/// The whole one-way transmission of `payload`, at `rate`.
pub fn carousel_samples(
    payload: &[u8],
    name: &str,
    call: &str,
    session: u16,
    profile: usize,
    width: Width,
    passes: usize,
    rate: f64,
) -> Vec<Complex32> {
    let mut c = Carousel::new(payload.to_vec(), name, call, session, profile, width, passes, 0);
    let mut frame_no = 0u16;
    let mut out = Vec::new();
    while let Some(i) = c.next_item() {
        out.extend(item_samples(&i, session, width, call, &mut frame_no, rate));
    }
    out
}

/// The two-way session of the sender component.
struct TwoWay {
    sender: Sender,
    deadline: Option<f64>,
}

/// The sender (`file_transfer.tx`).
#[derive(Default)]
pub struct FtSender {
    callsign: String,
    width: TxWidth,
    profile: TxProfile,
    passes: i64,
    link: LinkKind,
    file: String,
    recipient: String,
    session_id: i64,
    fixed: bool,
    rate: f64,
    transmit: bool,
    storage: Option<Arc<dyn Storage>>,
    clock: Option<sdrkit_core::Clock>,
    pending: Vec<Complex32>,
    at: usize,
    carousel: Option<(Carousel, String)>,
    two_way: Option<TwoWay>,
    queue: Vec<Vec<Complex32>>,
    frame_no: u16,
    stopping: bool,
    samples: u64,
    events: Vec<sdrkit_core::Event>,
}

impl FtSender {
    fn event(&mut self, state: &str, reason: &str) {
        self.events.push(FILE_TRANSFER_TX.event(0.0, 0.0, &TxEvent { state, reason }));
    }

    fn busy(&self) -> bool {
        self.at < self.pending.len() || self.carousel.is_some() || self.two_way.is_some()
    }

    fn call(&self) -> String {
        tx_callsign(&self.callsign).unwrap_or_else(|| crate::family::DEMO_CALLSIGN.to_string())
    }

    fn bytes(&self) -> Result<(Vec<u8>, String), TxError> {
        if self.file.trim().is_empty() {
            return Ok((DEMO_TEXT.as_bytes().to_vec(), DEMO_NAME.to_string()));
        }
        let storage = self.storage.as_ref().ok_or(TxError::File)?;
        let path = StorePath::new(self.file.trim()).map_err(|_| TxError::File)?;
        let b = storage.read(&path).map_err(|_| TxError::File)?;
        if b.is_empty() || b.len() > TX_LIMIT {
            return Err(TxError::File);
        }
        Ok((b, tx_name(path.as_str())))
    }

    fn session_id(&self) -> u16 {
        if (1..=65_000).contains(&self.session_id) {
            return self.session_id as u16;
        }
        let t = self.clock.as_ref().map_or(0.0, sdrkit_core::Clock::now);
        (t.max(0.0) as u64 % 65_000 + 1) as u16
    }

    /// Starts a transmission.
    pub fn send(&mut self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.busy() {
            return Err(TxError::Busy);
        }
        let width = self.width.width();
        if self.rate < width.bandwidth() * 1.25 {
            return Err(TxError::Rate);
        }
        let call = tx_callsign(&self.callsign).ok_or(TxError::Callsign)?;
        let (bytes, name) = self.bytes()?;
        let profile = usize::from(self.profile.index());
        let session = self.session_id();
        self.stopping = false;
        match self.link {
            LinkKind::OneWay => {
                let c = Carousel::new(bytes, &name, &call, session, profile, width, self.passes.clamp(1, 9) as usize, 0);
                self.carousel = Some((c, call));
                self.next_item();
            }
            LinkKind::TwoWay => {
                let recipient = if self.recipient.trim().is_empty() { "CQ".to_string() } else { self.recipient.trim().to_ascii_uppercase() };
                let cfg = SenderConfig {
                    session,
                    callsign: call,
                    recipient,
                    width,
                    name,
                    profile,
                    fixed_profile: self.fixed,
                    profiles: (0, 9),
                };
                let mut s = Sender::new(cfg, bytes);
                let first = s.start();
                self.two_way = Some(TwoWay { sender: s, deadline: None });
                self.put(vec![first]);
            }
        }
        Ok(())
    }

    /// Puts the next carousel item on the air, or ends the carousel.
    fn next_item(&mut self) {
        let rate = self.rate;
        let Some((c, call)) = self.carousel.as_mut() else {
            return;
        };
        let item = if self.stopping { None } else { c.next_item() };
        let Some(item) = item else {
            self.carousel = None;
            self.event("finished", if self.stopping { "stopped" } else { "" });
            return;
        };
        let (session, width, call) = (c.session(), c.width(), call.clone());
        self.pending = item_samples(&item, session, width, &call, &mut self.frame_no, rate);
        self.at = 0;
    }

    /// Puts what a two-way sender sends on the air and starts the reply timer.
    fn put(&mut self, outs: Vec<SenderOut>) {
        let rate = self.rate;
        let Some(t) = self.two_way.as_ref() else {
            return;
        };
        let cfg = t.sender.config().clone();
        let mut x = Vec::new();
        for o in outs {
            let item = match o {
                SenderOut::Control(m) => CarouselItem::Control(m),
                SenderOut::Burst(b) => CarouselItem::Burst(b),
            };
            x.extend(item_samples(&item, cfg.session, cfg.width, &cfg.callsign, &mut self.frame_no, rate));
        }
        if x.is_empty() {
            return;
        }
        let air = x.len() as f64 / rate.max(1.0);
        if let Some(t) = self.two_way.as_mut() {
            t.deadline = Some(self.samples as f64 / rate.max(1.0) + air + TURNAROUND_S + REPLY_WAIT_S);
        }
        self.queue.push(x);
    }

    fn after_state(&mut self) {
        let Some(t) = self.two_way.as_ref() else {
            return;
        };
        match t.sender.state().clone() {
            SenderState::Done => {
                self.two_way = None;
                self.event("finished", "");
            }
            SenderState::Failed(why) => {
                self.two_way = None;
                self.event("failed", &why);
            }
            _ => {}
        }
    }

    fn take_notes(&mut self, notes: &[Timed<LinkNote>]) {
        for n in notes {
            match &n.value {
                LinkNote::Heard(m) => {
                    if self.stopping && self.two_way.is_some() {
                        self.two_way = None;
                        self.event("finished", "stopped");
                        continue;
                    }
                    let Some(t) = self.two_way.as_mut() else {
                        continue;
                    };
                    let outs = t.sender.on_message(m);
                    if !outs.is_empty() {
                        t.deadline = None;
                    }
                    self.put(outs);
                    self.after_state();
                }
                LinkNote::Reply(m, width) => {
                    if !self.transmit {
                        continue;
                    }
                    let call = self.call();
                    let mut x = vec![Complex32::new(0.0, 0.0); (TURNAROUND_S * self.rate) as usize];
                    let session = m.session();
                    x.extend(item_samples(&CarouselItem::Control(m.clone()), session, *width, &call, &mut self.frame_no, self.rate));
                    self.queue.push(x);
                }
            }
        }
    }
}

impl Component for FtSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "file_transfer.tx",
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
        self.storage = Some(Arc::clone(&ctx.storage));
        self.clock = Some(ctx.clock.clone());
        if !ctx.transmit && (self.busy() || !self.queue.is_empty()) {
            self.pending.clear();
            self.at = 0;
            self.carousel = None;
            self.two_way = None;
            self.queue.clear();
            self.event("finished", "the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        self.samples += room as u64;
        let notes = io.items::<LinkNote>(1).to_vec();
        self.take_notes(&notes);
        if self.at >= self.pending.len() {
            if !self.queue.is_empty() {
                self.pending = self.queue.remove(0);
                self.at = 0;
            } else if self.carousel.is_some() {
                self.next_item();
            } else if let Some(t) = self.two_way.as_mut() {
                if t.deadline.is_some_and(|d| self.samples as f64 / self.rate.max(1.0) > d) {
                    t.deadline = None;
                    let out = if self.stopping { None } else { t.sender.on_timeout() };
                    match out {
                        Some(o) => self.put(vec![o]),
                        None => {
                            self.two_way = None;
                            let why = if self.stopping { "stopped".to_string() } else { "no reply after 6 retries".to_string() };
                            self.event("failed", &why);
                        }
                    }
                }
            }
            if self.at >= self.pending.len() && !self.queue.is_empty() {
                self.pending = self.queue.remove(0);
                self.at = 0;
            }
        }
        if self.at < self.pending.len() {
            let n = room.min(self.pending.len() - self.at);
            io.iq_out(0).extend_from_slice(&self.pending[self.at..self.at + n]);
            self.at += n;
            if self.at >= self.pending.len() {
                if let Some(last) = io.iq_out(0).len().checked_sub(1) {
                    io.output(0).tag_here(last, TagKind::BurstEnd);
                }
                self.pending.clear();
                self.at = 0;
            }
        }
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => self.callsign = CALLSIGN.read(id, v)?.to_owned(),
            TxSetting::Width => self.width = WIDTH.read(id, v)?,
            TxSetting::Profile => self.profile = PROFILE.read(id, v)?,
            TxSetting::Passes => self.passes = PASSES.read(id, v)?,
            TxSetting::Link => self.link = LINK.read(id, v)?,
            TxSetting::File => self.file = FILE.read(id, v)?.to_owned(),
            TxSetting::Recipient => self.recipient = RECIPIENT.read(id, v)?.to_owned(),
            TxSetting::Session => self.session_id = SESSION.read(id, v)?,
            TxSetting::FixedProfile => self.fixed = FIXED_PROFILE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (TxSetting::Callsign, Value::Text(self.callsign.clone())),
            (TxSetting::Width, Value::Choice(self.width.index())),
            (TxSetting::Profile, Value::Choice(self.profile.index())),
            (TxSetting::Passes, Value::I64(self.passes.clamp(1, 9))),
            (TxSetting::Link, Value::Choice(self.link.index())),
            (TxSetting::File, Value::Text(self.file.clone())),
            (TxSetting::Recipient, Value::Text(self.recipient.clone())),
            (TxSetting::Session, Value::I64(self.session_id)),
            (TxSetting::FixedProfile, Value::Bool(self.fixed)),
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
            TxCommand::Stop => self.stopping = true,
        }
        Ok(None)
    }
}

/// The test signal: the demonstration file from TEST1 in session 4242, one way at the width, profile and passes of the
/// settings, then 1 s of silence, at unit power, again and again.
pub struct FtGenerator {
    rate: f64,
    width: Width,
    signal: Vec<Complex32>,
    at: usize,
}

impl Generator for FtGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.width.bandwidth()).with_symbol_rate(self.width.symbol_rate()))
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
    let width = s.get(&WIDTH).width();
    let profile = usize::from(s.get(&PROFILE).index());
    let passes = s.get(&PASSES).clamp(1, 9) as usize;
    let rate = cfg.rate_hz;
    let mut signal = carousel_samples(
        DEMO_TEXT.as_bytes(),
        DEMO_NAME,
        crate::family::DEMO_CALLSIGN,
        TEST_SESSION,
        profile,
        width,
        passes,
        rate,
    );
    signal.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), rate as usize));
    let g = 1.0 / TX_LEVEL;
    for v in &mut signal {
        *v *= g;
    }
    Ok(Box::new(FtGenerator { rate, width, signal, at: 0 }))
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
        .add("rx", DecoderComponent::new(FtDecoder::default()))
        .add("sender", FtSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("rx")
        .input("sender.in")
        .connect("rx.link", "sender.link")
        .connect("sender.out", "tx.in")
        .output("rx.files")
        .output("rx.events")
        .output("sender.events")
        .status_from("rx")
        .clock(0.5);
    let mut g = StackGraph::of(crate::typed::shape(ID), b.build()?)
        .min_rate(MIN_RATE, "file_transfer needs a sample rate of at least 1.25 Msps");
    g = g.route(CALLSIGN.key(), &["rx.callsign", "sender.callsign"])?;
    for s in RxSetting::SETTINGS.iter().filter(|s| s.key() != CALLSIGN.key()) {
        let k = s.key();
        g = g.route(k, &[&format!("rx.{k}")])?;
    }
    for s in TxSetting::SETTINGS.iter().filter(|s| s.key() != CALLSIGN.key()) {
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

/// The input rate of the default tuning, samples a second.
pub const DEFAULT_RATE: f64 = 2_400_000.0;

/// The stack.
pub static FILE_TRANSFER: StackDescriptor = StackDescriptor {
    id: ID,
    name: "File transfer in LDPC frames",
    group: crate::GROUP,
    blurb: "One file over the air in frames of LDPC blocks, each with a CRC-32, and a hash of the whole file: 1 MHz, 500 kHz \
            or 200 kHz wide, ten profiles from BPSK rate 1/4 to 1024-QAM rate 9/10; one way in repeated passes or a two-way \
            session with acknowledged bursts and resume. Receive and transmit.",
    tuning: Tuning { min_hz: 100e6, max_hz: 6000e6, ..Tuning::anywhere(1290e6, DEFAULT_RATE, MIN_RATE, 1.0e6) },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::FILE_TRANSFER_VERIFY),
    benchmarks: Some(crate::verify::file_transfer_benchmarks),
};

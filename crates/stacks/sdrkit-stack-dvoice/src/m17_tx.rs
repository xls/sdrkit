//! The sender of the M17 stack (`m17.transmit`): a voice stream from the microphone input, coded with Codec 2 at 3200 bit/s,
//! and packets of SMS text or data. Standard: the public M17 specification (the frame layout that [`crate::m17`] decodes).
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `mic` is mono audio of any rate. Output `out` is
//! complex baseband at the session's rate for `tx.sink`: the signal at 0 Hz, centred on the transmit frequency, which is the
//! dial plus the transmit offset. Output `keyed` mutes the receive audio of the stack from key down to a short hold after
//! the end mark. Each pass writes as many samples as the pass brings in.
//!
//! A transmission is the preamble, the link setup frame, then stream frames of 40 ms (two Codec 2 frames, a LICH chunk and
//! the frame number) or packet frames, then the end of transmission. Push-to-talk off, `halt` and the time-out end a voice
//! stream after the frame on air: the next frame carries the end-of-stream bit. The carrier rises and falls with a 5 ms
//! raised cosine; its magnitude is 0.9 of full scale. Every transmission ends with an end mark, also when the transmitter
//! goes off or the format changes. On a live radio whose host times bursts (`HostStreams::transmit_lead`), the first
//! sample of each transmission states its time on air (`TagKind::BurstStart`): the time of the pass plus the lead of the
//! host and two passes.
//!
//! The Codec 2 tables come from [`M17Transmit::set_codebooks`] or from the host's storage, folder
//! [`crate::sender::TABLE_FOLDER`], one file for each table of the 3200 bit/s rate under its file name, checked by its SHA-256.

use crate::front::RATE;
use crate::gen::bits_to_levels;
use crate::sender::{
    failed, Codec2Tables, FskBurst, TableError, MIC_KEEP_S, PEAK, RISE_S, SPEECH_RATE, TABLE_FOLDER,
    TX, TX_SPLIT, TX_TIMEOUT,
};
use crate::m17::{
    encode_callsign, eot, lsf_bytes, lsf_frame, modulator, packet_frames, packet_payload, preamble,
    sms_payload, stream_frame, text_metas, Codecs, TypeField, PACKET_FRAMES_MAX,
};
use crate::typed::{
    M17SendArgs, M17SendDataArgs, M17TxFields, M17TxKind, PttArgs, HALT, PTT, SEND, SEND_DATA,
};
use sdrkit_codec_codec2::{Codebooks, Encoder, Mode};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError,
    Published, Record, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, KeyedOut, MicInput};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use std::collections::VecDeque;
use std::sync::Arc;

/// The name of the component.
pub const ID: &str = "m17.transmit";
/// The length of one stream frame, seconds.
pub const FRAME_S: f64 = 0.04;
/// The speech samples of one Codec 2 frame (20 ms).
const SPEECH_FRAME: usize = 160;
/// The most bytes of SMS text in one packet.
pub const TEXT_MAX: usize = PACKET_FRAMES_MAX * crate::m17::PACKET_CHUNK - 4;
/// The most bytes of data in one packet.
pub const DATA_MAX: usize = PACKET_FRAMES_MAX * crate::m17::PACKET_CHUNK - 3;

sdrkit_core::choice_enum! {
    /// The voice form of a stream.
    #[derive(Default)]
    pub enum M17Voice {
        /// Codec 2 at 3200 bit/s: two 20 ms frames in each stream frame.
        #[default]
        V3200 = ("3200", "Voice, 3200 bit/s"),
    }
}

/// The station's own call sign: the source of every transmission.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Own call sign", "", "")
    .help("The source address of every transmission: up to 9 characters of A to Z, 0 to 9, -, / and the full stop. Nothing is sent while it is empty.")
    .section(TX);
/// The destination of every transmission.
pub const DESTINATION: Setting<field::Text> =
    Setting::new("destination", "Destination", "", "@ALL")
        .help("A call sign, or @ALL for every station (broadcast).")
        .section(TX);
/// The channel access number.
pub const CAN: Setting<field::I64> = Setting::new("can", "Channel access number", "", 0)
    .range(0.0, 15.0, 1.0)
    .help("Sent in the TYPE field of the link setup frame.")
    .section(TX);
/// The voice form.
pub const VOICE: Setting<field::Choice<M17Voice>> =
    Setting::choice("voice_mode", "Voice", M17Voice::V3200)
        .help("Codec 2 at 3200 bit/s. The form of 1600 bit/s with data is not offered: the library has no encoder for it.")
        .section(TX);
/// The META text of a stream.
pub const META_TEXT: Setting<field::Text> = Setting::new("meta_text", "Text", "", "")
    .help("Up to 52 characters, sent in the META field of the link setup frame of a voice stream, 13 characters a block.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The own call sign.
        Callsign = CALLSIGN,
        /// The destination.
        Destination = DESTINATION,
        /// The channel access number.
        Can = CAN,
        /// The voice form.
        Voice = VOICE,
        /// The META text.
        MetaText = META_TEXT,
        /// The transmit offset.
        Split = TX_SPLIT,
        /// The time-out.
        Timeout = TX_TIMEOUT,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Push-to-talk.
        Ptt = PTT.command,
        /// An SMS packet.
        Send = SEND.command,
        /// A data packet.
        SendData = SEND_DATA.command,
        /// Ends every transmission.
        Halt = HALT,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 4] = [
    PTT.command.key,
    SEND.command.key,
    SEND_DATA.command.key,
    HALT.key,
];

/// Why a transmit command was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum M17TxError {
    /// The setting `callsign` is empty.
    NoSource,
    /// Another transmission is under way.
    Busy,
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// The Codec 2 table with this file name is not in the storage folder [`TABLE_FOLDER`].
    NoTable(&'static str),
    /// The Codec 2 table with this file name does not have its length or its SHA-256.
    BadTable(&'static str),
    /// The packet needs more than [`PACKET_FRAMES_MAX`] frames: the bytes given and the most that fit.
    TooLong {
        /// The bytes given.
        bytes: usize,
        /// The most bytes that fit.
        max: usize,
    },
    /// The protocol byte is outside 0 to 255.
    Protocol(i64),
}

impl M17TxError {
    /// The key of the setting the error is about, if any.
    pub fn setting(&self) -> Option<&'static str> {
        match self {
            M17TxError::NoSource => Some(CALLSIGN.key()),
            _ => None,
        }
    }
}

impl std::fmt::Display for M17TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            M17TxError::NoSource => write!(
                f,
                "the setting {} (own call sign) is empty: enter the call sign of the station",
                CALLSIGN.key()
            ),
            M17TxError::Busy => f.write_str("another transmission is under way"),
            M17TxError::NoInput => f.write_str("the stack has no input yet"),
            M17TxError::NoTable(t) => write!(
                f,
                "the Codec 2 table {t} is not in the folder {TABLE_FOLDER} of the storage"
            ),
            M17TxError::BadTable(t) => write!(f, "the Codec 2 table {t} is not the right file"),
            M17TxError::TooLong { bytes, max } => {
                write!(
                    f,
                    "{bytes} bytes do not fit in one packet; the most is {max}"
                )
            }
            M17TxError::Protocol(p) => write!(f, "the protocol byte {p} is outside 0 to 255"),
        }
    }
}

impl std::error::Error for M17TxError {}

impl From<TableError> for M17TxError {
    fn from(e: TableError) -> Self {
        match e {
            TableError::Missing(t) => M17TxError::NoTable(t),
            TableError::Wrong(t) => M17TxError::BadTable(t),
        }
    }
}

/// The addresses and the META blocks of a transmission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The source address (base 40).
    pub src: u64,
    /// The destination address (base 40; broadcast is all ones).
    pub dst: u64,
    /// The channel access number, 0 to 15.
    pub can: u8,
    /// The META blocks of a voice stream in turn, one per cycle of six LICH chunks; a packet sends no META.
    pub metas: Vec<[u8; 14]>,
}

impl Header {
    /// The link setup frames of a voice stream (Codec 2 3200), one per META block.
    pub fn voice_lsfs(&self) -> Vec<[u8; 30]> {
        let typ = TypeField {
            stream: true,
            data_type: 2,
            can: self.can,
            ..Default::default()
        };
        let none = [[0u8; 14]];
        let metas: &[[u8; 14]] = if self.metas.is_empty() {
            &none
        } else {
            &self.metas
        };
        metas
            .iter()
            .map(|m| lsf_bytes(self.dst, self.src, typ.encode(), m))
            .collect()
    }

    /// The link setup frame of a packet.
    pub fn packet_lsf(&self) -> [u8; 30] {
        let typ = TypeField {
            stream: false,
            data_type: 1,
            can: self.can,
            ..Default::default()
        };
        lsf_bytes(self.dst, self.src, typ.encode(), &[0; 14])
    }
}

/// One frame of bits waiting to be modulated, and whether it counts as a frame sent.
type Unit = (Vec<u8>, bool);

/// One transmission as 48 kHz samples: the frames in order, the rise, the level, the end of transmission and the fall.
/// It knows nothing of the graph, so a test can drive it with frames of its own.
pub struct Sender {
    kind: M17TxKind,
    codecs: Codecs,
    lsfs: Vec<[u8; 30]>,
    queue: VecDeque<Unit>,
    /// Stream frames built.
    built: u64,
    /// Stream or packet frames modulated.
    sent: u64,
    /// The most stream frames.
    limit: u64,
    stopping: bool,
    closing: bool,
    eot_sent: bool,
    burst: FskBurst,
    payload: [u8; 16],
}

impl Sender {
    fn new(kind: M17TxKind, lsfs: Vec<[u8; 30]>, first: [u8; 30], limit: u64) -> Sender {
        let mut queue = VecDeque::with_capacity(4);
        queue.push_back((preamble(true), false));
        queue.push_back((lsf_frame(&first), false));
        Sender {
            kind,
            codecs: Codecs::default(),
            lsfs,
            queue,
            built: 0,
            sent: 0,
            limit: limit.max(1),
            stopping: false,
            closing: false,
            eot_sent: false,
            burst: FskBurst::new(modulator(), RATE, RISE_S),
            payload: [0; 16],
        }
    }

    /// A voice stream of at most `limit` stream frames.
    pub fn voice(h: &Header, limit: u64) -> Sender {
        let lsfs = h.voice_lsfs();
        let first = lsfs[0];
        Sender::new(M17TxKind::Voice, lsfs, first, limit)
    }

    /// A packet carrying `payload` (the protocol byte, the body and the CRC).
    ///
    /// # Errors
    ///
    /// [`M17TxError::TooLong`] when the payload needs more than [`PACKET_FRAMES_MAX`] frames.
    pub fn packet(h: &Header, payload: &[u8]) -> Result<Sender, M17TxError> {
        let frames = payload.len().div_ceil(crate::m17::PACKET_CHUNK);
        if frames > PACKET_FRAMES_MAX {
            return Err(M17TxError::TooLong {
                bytes: payload.len(),
                max: PACKET_FRAMES_MAX * crate::m17::PACKET_CHUNK,
            });
        }
        let lsf = h.packet_lsf();
        let mut s = Sender::new(M17TxKind::Packet, vec![lsf], lsf, 1);
        for f in packet_frames(&s.codecs, payload) {
            s.queue.push_back((f, true));
        }
        s.closing = true;
        Ok(s)
    }

    /// What the transmission sends.
    pub fn kind(&self) -> M17TxKind {
        self.kind
    }

    /// The stream or packet frames modulated so far.
    pub fn frames_sent(&self) -> u64 {
        self.sent
    }

    /// Whether the transmission ends: a voice stream sends one more frame with the end-of-stream bit, a packet drops the
    /// frames not yet modulated; then the end of transmission follows.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// Ends the transmission after the frame on air (see [`Sender::stopping`]).
    pub fn stop(&mut self) {
        if self.stopping {
            return;
        }
        self.stopping = true;
        if self.kind == M17TxKind::Packet {
            self.queue.retain(|(_, counts)| !counts);
        }
    }

    /// The bits of the next frame in order: the preamble, the link setup frame, the stream or packet frames, the end of
    /// transmission; `None` after that. A voice stream asks `speech` for the 16 payload bytes of each stream frame.
    /// [`Sender::next`] modulates them.
    pub fn next_bits(&mut self, speech: impl FnOnce(&mut [u8; 16])) -> Option<Vec<u8>> {
        let (bits, counts) = if let Some(u) = self.queue.pop_front() {
            u
        } else if !self.closing {
            speech(&mut self.payload);
            let last = self.stopping || self.built + 1 >= self.limit;
            let lsf = &self.lsfs[(self.built as usize / 6) % self.lsfs.len()];
            let f = stream_frame(&self.codecs, lsf, self.built as usize, last, &self.payload);
            self.built += 1;
            self.closing = last;
            (f, true)
        } else if !self.eot_sent {
            self.eot_sent = true;
            (eot(), false)
        } else {
            return None;
        };
        if counts {
            self.sent += 1;
        }
        Some(bits)
    }

    /// Appends the 48 kHz samples of the next frame of [`Sender::next_bits`] to `out`. After the end of transmission comes
    /// the fall; then it returns false and appends nothing more.
    pub fn next(&mut self, speech: impl FnOnce(&mut [u8; 16]), out: &mut Vec<Complex32>) -> bool {
        if self.burst.done() {
            return false;
        }
        match self.next_bits(speech) {
            Some(bits) => self.burst.levels(&bits_to_levels(&bits), out),
            None => self.burst.end(out),
        }
        true
    }
}

/// One transmission on the graph: the sender, the speech encoder of a voice stream, and the rate change.
struct Burst {
    sender: Sender,
    encoder: Option<Box<Encoder>>,
    out: BurstOut,
}

impl Burst {
    /// Makes samples until at least `room` output samples wait, or until the transmission is made with the tail of the
    /// rate change.
    fn fill(&mut self, room: usize, mic: &mut MicInput, base: &mut Vec<Complex32>) {
        let mut speech = [0f32; SPEECH_FRAME];
        let mut pcm = [0i16; SPEECH_FRAME];
        while self.out.wants(room) {
            base.clear();
            let encoder = &mut self.encoder;
            let more = self.sender.next(
                |payload| {
                    for half in payload.chunks_mut(8) {
                        mic.read(&mut speech);
                        for (p, s) in pcm.iter_mut().zip(speech) {
                            *p = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                        }
                        if let Some(e) = encoder.as_mut() {
                            if e.encode(&pcm, half).is_err() {
                                half.fill(0);
                            }
                        }
                    }
                },
                base,
            );
            self.out.push(base);
            if !more {
                self.out.finish(base);
            }
        }
    }
}

/// The values of the settings.
#[derive(Clone, Debug, PartialEq)]
struct TxConfig {
    callsign: String,
    destination: String,
    can: u8,
    voice: M17Voice,
    meta_text: String,
    split: f64,
    timeout: f64,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            callsign: String::new(),
            destination: DESTINATION.default.to_string(),
            can: 0,
            voice: M17Voice::V3200,
            meta_text: String::new(),
            split: TX_SPLIT.default,
            timeout: TX_TIMEOUT.default,
        }
    }
}

/// Whether `text` is a call sign that encodes to an address: up to 9 characters of the M17 alphabet, or a special
/// address such as @ALL.
fn valid_address(text: &str) -> bool {
    let a = encode_callsign(text);
    a != 0 && text.len() <= 9
}

/// The sender (`m17.transmit`). See the module.
pub struct M17Transmit {
    cfg: TxConfig,
    rate: f64,
    dial: f64,
    transmit: bool,
    mic: MicInput,
    burst: Option<Burst>,
    end_owed: bool,
    keyed: KeyedOut,
    tables: Codec2Tables,
    last_kind: Option<M17TxKind>,
    last_frames: u64,
    results: Published,
    base: Vec<Complex32>,
    timing: BurstTiming,
}

impl Default for M17Transmit {
    fn default() -> Self {
        let mut t = M17Transmit {
            cfg: TxConfig::default(),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            mic: MicInput::new(SPEECH_RATE, MIC_KEEP_S),
            burst: None,
            end_owed: false,
            keyed: KeyedOut::default(),
            tables: Codec2Tables::default(),
            last_kind: None,
            last_frames: 0,
            results: Published::new(M17TxFields::SHAPE.fields),
            base: Vec::new(),
            timing: BurstTiming::default(),
        };
        t.publish();
        t
    }
}

impl M17Transmit {
    /// The transmit frequency: the dial plus the transmit offset.
    pub fn frequency(&self) -> f64 {
        self.dial + self.cfg.split
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.burst.is_some()
    }

    /// Uses `books` for the speech encoder instead of the tables of the storage.
    pub fn set_codebooks(&mut self, books: Arc<Codebooks>) {
        self.tables.set(books);
    }

    /// The header of the settings.
    fn header(&self) -> Result<Header, M17TxError> {
        if self.cfg.callsign.trim().is_empty() {
            return Err(M17TxError::NoSource);
        }
        let metas = if self.cfg.meta_text.is_empty() {
            Vec::new()
        } else {
            text_metas(&self.cfg.meta_text)
        };
        Ok(Header {
            src: encode_callsign(self.cfg.callsign.trim()),
            dst: encode_callsign(self.cfg.destination.trim()),
            can: self.cfg.can,
            metas,
        })
    }

    /// Whether the receive audio is muted: a transmission, its end mark or the hold after it.
    pub fn keyed(&self) -> bool {
        self.keyed.keyed(self.burst.is_some() || self.end_owed)
    }

    /// Drops the transmission on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if let Some(b) = self.burst.take() {
            self.last_frames = b.sender.frames_sent();
            self.end_owed = true;
        }
    }

    /// Checks that a transmission can start now.
    fn ready(&self) -> Result<(), CommandError> {
        if !self.transmit {
            return Err(CommandError::TransmitOff);
        }
        if self.burst.is_some() {
            return Err(failed(M17TxError::Busy));
        }
        if self.rate <= 0.0 {
            return Err(failed(M17TxError::NoInput));
        }
        Ok(())
    }

    /// Starts the transmission of `sender`, with `encoder` for a voice stream.
    fn begin(&mut self, sender: Sender, encoder: Option<Box<Encoder>>) -> Result<(), CommandError> {
        let out = BurstOut::new(RATE, self.rate, PEAK).map_err(|_| failed(M17TxError::NoInput))?;
        self.mic.restart();
        self.last_kind = Some(sender.kind());
        self.last_frames = 0;
        self.burst = Some(Burst {
            sender,
            encoder,
            out,
        });
        Ok(())
    }

    /// Push-to-talk on: starts a voice stream; a voice stream already under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`M17TxError`] when another transmission runs, the stack has no input, the
    /// setting `callsign` is empty, or the Codec 2 tables are missing.
    pub fn key_down(&mut self) -> Result<(), CommandError> {
        if let Some(b) = &self.burst {
            if self.transmit && b.sender.kind() == M17TxKind::Voice && !b.sender.stopping() {
                return Ok(());
            }
        }
        self.ready()?;
        let header = self.header().map_err(failed)?;
        let encoder = self
            .tables
            .encoder(Mode::M3200)
            .map_err(|e| failed(M17TxError::from(e)))?;
        let limit = (self.cfg.timeout / FRAME_S).ceil() as u64;
        self.begin(Sender::voice(&header, limit), Some(Box::new(encoder)))
    }

    /// Push-to-talk off: the voice stream ends after the frame on air.
    pub fn key_up(&mut self) {
        if let Some(b) = &mut self.burst {
            if b.sender.kind() == M17TxKind::Voice {
                b.sender.stop();
            }
        }
    }

    /// Sends a packet with `protocol` and `body` (the CRC is added).
    ///
    /// # Errors
    ///
    /// As [`M17Transmit::key_down`], and [`M17TxError::TooLong`] for a body that does not fit.
    pub fn send_packet(&mut self, payload: &[u8]) -> Result<(), CommandError> {
        self.ready()?;
        let header = self.header().map_err(failed)?;
        let sender = Sender::packet(&header, payload).map_err(failed)?;
        self.begin(sender, None)
    }

    /// Ends every transmission after the frame on air.
    pub fn halt(&mut self) {
        if let Some(b) = &mut self.burst {
            b.sender.stop();
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let f = M17TxFields {
            transmitting: self.burst.is_some(),
            tx_freq_hz: self.burst.as_ref().map(|_| Hertz(self.frequency())),
            tx_kind: self
                .burst
                .as_ref()
                .map(|b| b.sender.kind())
                .or(self.last_kind),
            tx_frames: self
                .burst
                .as_ref()
                .map_or(self.last_frames, |b| b.sender.frames_sent()) as i64,
        };
        self.results.put(&f);
    }

    /// One pass: the samples of the transmission, the end mark, and the mute state.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        let voice = self
            .burst
            .as_ref()
            .is_some_and(|b| b.sender.kind() == M17TxKind::Voice);
        self.mic.take(io, 1, voice);
        let mut ended = std::mem::take(&mut self.end_owed);
        if ended {
            end_mark(io, 0);
        }
        if let Some(b) = &mut self.burst {
            b.fill(room, &mut self.mic, &mut self.base);
            if b.out.write(io, 0, room, &self.timing) {
                self.last_frames = b.sender.frames_sent();
                self.burst = None;
                ended = true;
            }
        }
        let busy = self.burst.is_some();
        self.keyed.pass(io, 1, busy, ended, room, self.rate);
        if io.tick() || ended {
            self.publish();
        }
    }
}

impl Component for M17Transmit {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            ID,
            vec![
                Port::iq("in"),
                Port::audio("mic").optional("silence: the voice frames carry coded silence"),
            ],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.mic.configure(&inputs[1]);
        self.cut();
        self.keyed.reset();
        self.publish();
        let c = self.frequency();
        Ok(vec![Format::iq(self.rate, c, c), KeyedOut::format()])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        self.mic.attach(ctx);
        self.tables.attach(Arc::clone(&ctx.storage));
        if !ctx.transmit {
            self.cut();
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        self.cut();
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let malformed = SettingError::Malformed { id, at: None };
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => {
                let t = CALLSIGN.read(id, v)?.trim().to_ascii_uppercase();
                if !t.is_empty() && (!valid_address(&t) || t.starts_with('@')) {
                    return Err(malformed);
                }
                self.cfg.callsign = t;
            }
            TxSetting::Destination => {
                let t = DESTINATION.read(id, v)?.trim().to_ascii_uppercase();
                if !valid_address(&t) {
                    return Err(malformed);
                }
                self.cfg.destination = t;
            }
            TxSetting::Can => {
                self.cfg.can = u8::try_from(CAN.read(id, v)?)
                    .ok()
                    .filter(|c| *c <= 15)
                    .ok_or(malformed)?;
            }
            TxSetting::Voice => self.cfg.voice = VOICE.read(id, v)?,
            TxSetting::MetaText => {
                let t = META_TEXT.read(id, v)?;
                if t.len() > 52 {
                    return Err(malformed);
                }
                self.cfg.meta_text = t.to_string();
            }
            TxSetting::Split => {
                let s = TX_SPLIT.read(id, v)?;
                let moved = s != self.cfg.split;
                self.cfg.split = s;
                return Ok(if moved {
                    Applied::Reformat
                } else {
                    Applied::Live
                });
            }
            TxSetting::Timeout => self.cfg.timeout = TX_TIMEOUT.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        sdrkit_graph::values_of([
            (TxSetting::Callsign, Value::Text(self.cfg.callsign.clone())),
            (
                TxSetting::Destination,
                Value::Text(self.cfg.destination.clone()),
            ),
            (TxSetting::Can, Value::I64(i64::from(self.cfg.can))),
            (TxSetting::Voice, Value::Choice(self.cfg.voice.index())),
            (TxSetting::MetaText, Value::Text(self.cfg.meta_text.clone())),
            (TxSetting::Split, Value::Hz(self.cfg.split)),
            (TxSetting::Timeout, Value::Duration(self.cfg.timeout)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Ptt => {
                let PttArgs { on } = PTT.args(args)?;
                if on {
                    self.key_down()?;
                } else {
                    self.key_up();
                }
            }
            TxCommand::Send => {
                let M17SendArgs { text } = SEND.args(args)?;
                if text.len() > TEXT_MAX {
                    return Err(failed(M17TxError::TooLong {
                        bytes: text.len(),
                        max: TEXT_MAX,
                    }));
                }
                self.send_packet(&sms_payload(text))?;
            }
            TxCommand::SendData => {
                let M17SendDataArgs { protocol, data } = SEND_DATA.args(args)?;
                let p =
                    u8::try_from(protocol).map_err(|_| failed(M17TxError::Protocol(protocol)))?;
                if data.len() > DATA_MAX {
                    return Err(failed(M17TxError::TooLong {
                        bytes: data.len(),
                        max: DATA_MAX,
                    }));
                }
                self.send_packet(&packet_payload(p, &data))?;
            }
            TxCommand::Halt => self.halt(),
        }
        self.publish();
        Ok(None)
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

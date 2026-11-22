//! The sender of the D-STAR stack (`dstar.transmit`): a DV transmission with the radio header of the settings, as a voice
//! transmission from the microphone input (push-to-talk) or as a text in the slow data. Standard: JARL D-STAR
//! specification (the frame layout that [`crate::dstar`] decodes).
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `mic` is mono audio of any rate: the speech of a
//! voice transmission, coded with the D-STAR voice coder ([`sdrkit_codec_mbe`]) in frames of 20 ms. Output `out` is
//! complex baseband at the session's rate for `tx.sink`: the signal at 0 Hz, centred on the transmit frequency, which is
//! the dial plus the transmit offset. Output `keyed` mutes the receive audio of the stack from key down to a short hold
//! after the end mark.
//!
//! A transmission is the bit sync, the frame sync, the radio header, voice frames of 20 ms (the data sync in frame 0 of
//! every 21, the slow-data blocks in the others), then the end pattern. A voice transmission carries the text message of
//! the setting `message` (four blocks of five characters, none when it is empty) and the header copy in the slow data,
//! in turn for as long as it lasts; push-to-talk off, `halt` and the time-out end it after the frame on air. A text
//! carries the text and the header copy in the slow data of voice frames with the silence pattern of the specification
//! ([`sdrkit_codec_mbe::DSTAR_SILENCE`]); it lasts the groups of 21 frames that send every slow-data block once; `halt`
//! ends it after the frame on air. GMSK at 4800 bit/s, BT 0.5, deviation ±1.2 kHz. The carrier rises and falls with a 5 ms raised cosine; its magnitude is 0.9 of full scale. Every
//! transmission ends with an end mark; on a live radio whose host times bursts, the first sample states its time on air.

use crate::dstar::{end_bits, slow_blocks, start_bits, Header, VoiceFrames};
use crate::front::RATE;
use crate::gen::{FskModulator, TxPulse};
use crate::sender::{failed, FskBurst, Maker, TxCore, RISE_S, TX, TX_SPLIT, TX_TIMEOUT};
use crate::typed::{
    DstarSendArgs, DstarTxFields, DstarTxKind, PttArgs, DSTAR_PTT, DSTAR_SEND, HALT,
};
use sdrkit_codec_mbe::{Coder, Encoder, FRAME};
use sdrkit_components::tx::{KeyedOut, MicInput};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published, Record,
    Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};

/// The name of the component.
pub const ID: &str = "dstar.transmit";
/// The most characters of a text.
pub const TEXT_MAX: usize = 20;
/// The duration of a voice frame, seconds.
pub const FRAME_S: f64 = 0.02;
/// What the sender sends.
pub const NOTE: &str =
    "voice from the microphone with the D-STAR voice coder and the text message in its slow data, \
     and texts in the slow data of silence frames";

/// The own call sign: MY of the radio header.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Own call sign", "", "")
    .help("MY of the radio header: up to 8 characters of A to Z, 0 to 9, / and space. Nothing is sent while it is empty.")
    .section(TX);
/// The suffix of the own call sign.
pub const SUFFIX: Setting<field::Text> = Setting::new("suffix", "Suffix", "", "")
    .help("Up to 4 characters after MY, for example the radio type.")
    .section(TX);
/// The station called.
pub const YOUR_CALL: Setting<field::Text> = Setting::new("your_call", "Your call sign", "", "CQCQCQ")
    .help("YOUR of the radio header: a call sign, CQCQCQ for everyone, or a command of a gateway; up to 8 characters.")
    .section(TX);
/// The first repeater.
pub const RPT1: Setting<field::Text> = Setting::new("rpt1", "Repeater 1", "", "DIRECT")
    .help("RPT1 of the radio header: DIRECT for simplex, or the repeater with its module letter in position 8.")
    .section(TX);
/// The second repeater.
pub const RPT2: Setting<field::Text> = Setting::new("rpt2", "Repeater 2", "", "DIRECT")
    .help("RPT2 of the radio header: DIRECT for simplex, or the repeater with G in position 8 for the gateway.")
    .section(TX);
/// Sent through a repeater.
pub const VIA_REPEATER: Setting<field::Bool> =
    Setting::new("via_repeater", "Via repeater", "", false)
        .help("Sets bit 6 (0x40) of flag byte 1: the transmission goes through a repeater.")
        .section(TX);
/// The text message of a voice transmission.
pub const MESSAGE: Setting<field::Text> = Setting::new("message", "Text message", "", "")
    .help("Up to 20 printable ASCII characters, sent in the slow data of every voice transmission with the copy of the radio header. Empty: no text message.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The own call sign.
        Callsign = CALLSIGN,
        /// The suffix.
        Suffix = SUFFIX,
        /// The station called.
        YourCall = YOUR_CALL,
        /// The first repeater.
        Rpt1 = RPT1,
        /// The second repeater.
        Rpt2 = RPT2,
        /// Through a repeater.
        ViaRepeater = VIA_REPEATER,
        /// The text message of a voice transmission.
        Message = MESSAGE,
        /// The transmit offset.
        Split = TX_SPLIT,
        /// The time-out of a voice transmission.
        Timeout = TX_TIMEOUT,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Push-to-talk.
        Ptt = DSTAR_PTT.command,
        /// A text.
        Send = DSTAR_SEND.command,
        /// Ends the transmission.
        Halt = HALT,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 3] = [DSTAR_PTT.command.key, DSTAR_SEND.command.key, HALT.key];

/// Why a transmit command was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DstarTxError {
    /// The setting `callsign` is empty.
    NoSource,
    /// Another transmission is under way.
    Busy,
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// The text is longer than [`TEXT_MAX`] or holds a character outside printable ASCII.
    BadText,
}

impl DstarTxError {
    /// The key of the setting the error is about, if any.
    pub fn setting(&self) -> Option<&'static str> {
        match self {
            DstarTxError::NoSource => Some(CALLSIGN.key()),
            _ => None,
        }
    }
}

impl std::fmt::Display for DstarTxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DstarTxError::NoSource => write!(
                f,
                "the setting {} (own call sign) is empty: enter the call sign of the station",
                CALLSIGN.key()
            ),
            DstarTxError::Busy => f.write_str("another transmission is under way"),
            DstarTxError::NoInput => f.write_str("the stack has no input yet"),
            DstarTxError::BadText => write!(
                f,
                "the text must be at most {TEXT_MAX} printable ASCII characters"
            ),
        }
    }
}

impl std::error::Error for DstarTxError {}

/// The 72 channel bits, as sent, of the silence pattern of the specification (JARL D-STAR section 6.6).
pub fn silence_frame() -> [u8; 72] {
    sdrkit_codec_mbe::dstar_silence_bits()
}

/// One transmission as 48 kHz samples: the start, the voice frames, the end pattern, the rise, the level and the fall. It
/// knows nothing of the graph, so a test can drive it.
pub struct Sender {
    kind: DstarTxKind,
    start: Option<Vec<u8>>,
    frames: VoiceFrames,
    limit: usize,
    stopping: bool,
    end_sent: bool,
    encoder: Option<Box<Encoder>>,
    bits: Vec<u8>,
    burst: FskBurst,
}

impl Sender {
    /// A transmission with the header `h` and the text `text` in the slow data, of the groups of 21 frames that send
    /// every slow-data block once. Its voice frames carry the silence pattern.
    pub fn text(h: &Header, text: &str) -> Sender {
        let frames = VoiceFrames::new(slow_blocks(h, text, &[], None));
        let limit = frames.cycle();
        Sender::new(DstarTxKind::Text, h, frames, limit)
    }

    /// A voice transmission with the header `h`, of at most `limit` voice frames. The slow data carries the text
    /// `message` (at most [`TEXT_MAX`] characters; none when it is empty) and the header copy, in turn.
    pub fn voice(h: &Header, message: &str, limit: usize) -> Sender {
        let mut blocks = slow_blocks(h, message, &[], None);
        // The first four slow-data blocks are the text.
        if message.is_empty() {
            blocks.drain(..4);
        }
        Sender::new(
            DstarTxKind::Voice,
            h,
            VoiceFrames::new(blocks),
            limit.max(1),
        )
    }

    fn new(kind: DstarTxKind, h: &Header, frames: VoiceFrames, limit: usize) -> Sender {
        Sender {
            kind,
            start: Some(start_bits(h)),
            frames,
            limit,
            stopping: false,
            end_sent: false,
            encoder: (kind == DstarTxKind::Voice).then(|| Box::new(Encoder::new(Coder::Dstar))),
            bits: Vec::with_capacity(96),
            burst: FskBurst::new(
                FskModulator::new(4800.0, TxPulse::Gaussian(0.5), 1200.0),
                RATE,
                RISE_S,
            ),
        }
    }

    /// What the transmission sends.
    pub fn kind(&self) -> DstarTxKind {
        self.kind
    }

    /// Whether the transmission is a voice transmission that has not been told to end.
    pub fn speaking(&self) -> bool {
        self.kind == DstarTxKind::Voice && !self.stopping && !self.end_sent
    }

    /// The voice frames modulated so far.
    pub fn frames_sent(&self) -> u64 {
        self.frames.frames() as u64
    }

    /// The voice frames of the whole transmission.
    pub fn frames_planned(&self) -> usize {
        self.limit
    }

    /// Ends the transmission after the frame on air: the end pattern follows.
    pub fn stop(&mut self) {
        self.stopping = true;
    }

    /// Appends the 48 kHz samples of the next part to `out`: the start, one voice frame whose 72 channel bits (as sent)
    /// `speech` fills, the end pattern, or the fall; then it returns false and appends nothing more.
    pub fn next_with(
        &mut self,
        mut speech: impl FnMut(&mut [u8; 72]),
        out: &mut Vec<Complex32>,
    ) -> bool {
        if self.burst.done() {
            return false;
        }
        self.bits.clear();
        if let Some(s) = self.start.take() {
            self.burst.bits(&s, out);
        } else if !self.stopping && self.frames.frames() < self.limit {
            let mut voice = [0u8; 72];
            speech(&mut voice);
            self.frames.push(&voice, &mut self.bits);
            self.burst.bits(&self.bits, out);
        } else if !self.end_sent {
            self.end_sent = true;
            self.burst.bits(&end_bits(), out);
        } else {
            self.burst.end(out);
        }
        true
    }

    /// As [`Sender::next_with`], with the silence pattern in the voice frames.
    pub fn next(&mut self, out: &mut Vec<Complex32>) -> bool {
        let silence = silence_frame();
        self.next_with(|f| *f = silence, out)
    }
}

impl Maker for Sender {
    /// Codes the speech of each voice frame of a voice transmission from 20 ms of `mic`; a text reads no audio.
    fn next(&mut self, mic: &mut MicInput, out: &mut Vec<Complex32>) -> bool {
        let mut encoder = self.encoder.take();
        let more = self.next_with(
            |frame| match encoder.as_mut() {
                Some(e) => {
                    let mut speech = [0f32; FRAME];
                    mic.read(&mut speech);
                    let pcm: [i16; FRAME] = std::array::from_fn(|i| {
                        (speech[i].clamp(-1.0, 1.0) * 32767.0).round() as i16
                    });
                    e.encode(&pcm, frame);
                }
                None => *frame = silence_frame(),
            },
            out,
        );
        self.encoder = encoder;
        more
    }
    fn speaks(&self) -> bool {
        self.speaking()
    }
    fn stop(&mut self) {
        Sender::stop(self);
    }
    fn frames_sent(&self) -> u64 {
        Sender::frames_sent(self)
    }
}

/// The values of the settings.
#[derive(Clone, Debug, PartialEq)]
struct TxConfig {
    callsign: String,
    suffix: String,
    your_call: String,
    rpt1: String,
    rpt2: String,
    via_repeater: bool,
    message: String,
    split: f64,
    timeout: f64,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            callsign: String::new(),
            suffix: String::new(),
            your_call: YOUR_CALL.default.to_string(),
            rpt1: RPT1.default.to_string(),
            rpt2: RPT2.default.to_string(),
            via_repeater: false,
            message: String::new(),
            split: TX_SPLIT.default,
            timeout: TX_TIMEOUT.default,
        }
    }
}

/// Whether `text` fits a header field of `n` characters: printable ASCII.
fn valid_field(text: &str, n: usize) -> bool {
    text.len() <= n && text.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
}

/// Whether `text` is a call sign for MY: up to 8 characters of A to Z, 0 to 9, / and space.
fn valid_callsign(text: &str) -> bool {
    text.len() <= 8
        && text
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'/' || b == b' ')
}

/// The sender (`dstar.transmit`). See the module.
pub struct DstarTransmit {
    cfg: TxConfig,
    core: TxCore,
    last_kind: Option<DstarTxKind>,
    results: Published,
}

impl Default for DstarTransmit {
    fn default() -> Self {
        let mut t = DstarTransmit {
            cfg: TxConfig::default(),
            core: TxCore::default(),
            last_kind: None,
            results: Published::new(DstarTxFields::SHAPE.fields),
        };
        t.publish();
        t
    }
}

impl DstarTransmit {
    /// The transmit frequency: the dial plus the transmit offset.
    pub fn frequency(&self) -> f64 {
        self.core.dial() + self.cfg.split
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.core.transmitting()
    }

    /// The radio header of the settings.
    pub fn header(&self) -> Header {
        Header {
            flags: [if self.cfg.via_repeater { 0x40 } else { 0 }, 0, 0],
            rpt2: self.cfg.rpt2.clone(),
            rpt1: self.cfg.rpt1.clone(),
            ur: self.cfg.your_call.clone(),
            my: self.cfg.callsign.clone(),
            suffix: self.cfg.suffix.clone(),
        }
    }

    /// Push-to-talk on: starts a voice transmission with the radio header and the text message of the settings; one
    /// already under way goes on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`DstarTxError`] when another transmission runs, the stack has no input, or the
    /// setting `callsign` is empty.
    pub fn key_down(&mut self) -> Result<(), CommandError> {
        if self.core.speaking() {
            return Ok(());
        }
        self.core.ready(DstarTxError::Busy, DstarTxError::NoInput)?;
        if self.cfg.callsign.is_empty() {
            return Err(failed(DstarTxError::NoSource));
        }
        let limit = (self.cfg.timeout / FRAME_S).ceil() as usize;
        let sender = Sender::voice(&self.header(), &self.cfg.message, limit);
        self.core
            .begin(Box::new(sender), RATE, DstarTxError::NoInput)?;
        self.last_kind = Some(DstarTxKind::Voice);
        Ok(())
    }

    /// Push-to-talk off: the voice transmission ends after the frame on air, with the end pattern.
    pub fn key_up(&mut self) {
        if self.core.speaking() {
            self.core.halt();
        }
    }

    /// Sends `text` in the slow data.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`DstarTxError`] when another transmission runs, the stack has no input, the
    /// setting `callsign` is empty, or the text does not fit.
    pub fn send(&mut self, text: &str) -> Result<(), CommandError> {
        self.core.ready(DstarTxError::Busy, DstarTxError::NoInput)?;
        if self.cfg.callsign.is_empty() {
            return Err(failed(DstarTxError::NoSource));
        }
        if !valid_field(text, TEXT_MAX) {
            return Err(failed(DstarTxError::BadText));
        }
        let sender = Sender::text(&self.header(), text);
        self.core
            .begin(Box::new(sender), RATE, DstarTxError::NoInput)?;
        self.last_kind = Some(DstarTxKind::Text);
        Ok(())
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let f = DstarTxFields {
            transmitting: self.core.transmitting(),
            tx_freq_hz: self.core.transmitting().then(|| Hertz(self.frequency())),
            tx_kind: self.last_kind,
            tx_frames: self.core.frames() as i64,
            tx_note: NOTE,
        };
        self.results.put(&f);
    }
}

impl Component for DstarTransmit {
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
        self.core.configure(inputs);
        self.publish();
        let c = self.frequency();
        Ok(vec![Format::iq(inputs[0].rate, c, c), KeyedOut::format()])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.core.attach(ctx);
    }
    fn process(&mut self, io: &mut Io) {
        if self.core.run(io) {
            self.publish();
        }
    }
    fn finish(&mut self, io: &mut Io) {
        if self.core.finish(io) {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let malformed = SettingError::Malformed { id, at: None };
        let field = |t: &str, n: usize| -> Result<String, SettingError> {
            let t = t.trim_end().to_ascii_uppercase();
            if valid_field(&t, n) {
                Ok(t)
            } else {
                Err(SettingError::Malformed { id, at: None })
            }
        };
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => {
                let t = CALLSIGN.read(id, v)?.trim().to_ascii_uppercase();
                if !valid_callsign(&t) {
                    return Err(malformed);
                }
                self.cfg.callsign = t;
            }
            TxSetting::Suffix => self.cfg.suffix = field(SUFFIX.read(id, v)?, 4)?,
            TxSetting::YourCall => self.cfg.your_call = field(YOUR_CALL.read(id, v)?, 8)?,
            TxSetting::Rpt1 => self.cfg.rpt1 = field(RPT1.read(id, v)?, 8)?,
            TxSetting::Rpt2 => self.cfg.rpt2 = field(RPT2.read(id, v)?, 8)?,
            TxSetting::ViaRepeater => self.cfg.via_repeater = VIA_REPEATER.read(id, v)?,
            TxSetting::Message => {
                let t = MESSAGE.read(id, v)?;
                if !valid_field(t, TEXT_MAX) {
                    return Err(malformed);
                }
                self.cfg.message = t.to_string();
            }
            TxSetting::Timeout => self.cfg.timeout = TX_TIMEOUT.read(id, v)?,
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
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        sdrkit_graph::values_of([
            (TxSetting::Callsign, Value::Text(self.cfg.callsign.clone())),
            (TxSetting::Suffix, Value::Text(self.cfg.suffix.clone())),
            (TxSetting::YourCall, Value::Text(self.cfg.your_call.clone())),
            (TxSetting::Rpt1, Value::Text(self.cfg.rpt1.clone())),
            (TxSetting::Rpt2, Value::Text(self.cfg.rpt2.clone())),
            (TxSetting::ViaRepeater, Value::Bool(self.cfg.via_repeater)),
            (TxSetting::Message, Value::Text(self.cfg.message.clone())),
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
                let PttArgs { on } = DSTAR_PTT.args(args)?;
                if on {
                    self.key_down()?;
                } else {
                    self.key_up();
                }
            }
            TxCommand::Send => {
                let DstarSendArgs { text } = DSTAR_SEND.args(args)?;
                self.send(text)?;
            }
            TxCommand::Halt => self.core.halt(),
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

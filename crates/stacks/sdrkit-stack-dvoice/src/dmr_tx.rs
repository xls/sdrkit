//! The sender of the DMR stack (`dmr.transmit`): a voice call from the microphone input (push-to-talk) and a text message
//! as defined short data, in TDMA direct mode, one slot, simplex. Standard: ETSI TS 102 361-1 to -3 (the burst layout
//! that [`crate::dmr`] decodes).
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `mic` is mono audio of any rate: the speech of a
//! voice call, coded with the half-rate coder ([`sdrkit_codec_mbe`]) in frames of 20 ms. Output `out` is complex
//! baseband at the session's rate for `tx.sink`: the signal at 0 Hz, centred on the transmit frequency, which is the dial
//! plus the transmit offset. Output `keyed` mutes the receive audio of the stack from key down to a short hold after the
//! end mark.
//!
//! A voice call is the voice LC header, then voice superframes of six bursts A to F, each with three speech frames (burst
//! A with the voice sync of the slot, bursts B to E with the call's LC as embedded LC, burst F with no embedded message),
//! then the terminator with LC. Push-to-talk off, `halt` and the time-out end a call after the voice burst on air. A text
//! is two preamble CSBKs, the data header and the rate 1/2 data blocks of the message; the last data block ends it;
//! `halt` ends it after the burst on air. Each burst is 27.5 ms with the direct-mode sync of the slot, one burst every
//! 60 ms; the carrier is off between the bursts. 4FSK at 4800 symbols/s, root raised cosine roll-off 0.2, 648 Hz a level
//! step. Each burst rises and falls with a 1.5 ms raised cosine; its magnitude is 0.9 of full scale. Every transmission
//! ends with an end mark; on a live radio whose host times bursts, the first sample states its time on air.

use crate::dmr::{
    dt, embedded_bits, lc_burst, text_bursts, voice_burst, Lc, ShortData, SHORT_DATA_BLOCKS_MAX,
};
use crate::front::RATE;
use crate::gen::{bits_to_levels, FskModulator, TxPulse};
use crate::sender::{failed, FskBurst, Maker, TxCore, TX, TX_SPLIT, TX_TIMEOUT};
use crate::typed::{DataSendArgs, DataTxFields, DataTxKind, PttArgs, DMR_PTT, DMR_SEND, HALT};
use sdrkit_algo_fec::voice::NXDN_EHR_SILENCE;
use sdrkit_codec_mbe::{Coder, Encoder, FRAME};
use sdrkit_components::tx::{KeyedOut, MicInput};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published,
    Record, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use std::collections::VecDeque;

/// The name of the component.
pub const ID: &str = "dmr.transmit";
/// The rise and the fall of each burst, seconds.
pub const RAMP_S: f64 = 0.0015;
/// The time from the start of one burst to the start of the next in the same slot, seconds.
pub const BURST_PERIOD_S: f64 = 0.06;
/// The most octets of a message.
pub const DATA_MAX: usize = SHORT_DATA_BLOCKS_MAX * 12 - 4;
/// What the sender sends.
pub const NOTE: &str =
    "voice from the microphone with the half-rate coder, and text messages as short data, in direct mode";
/// The largest radio ID.
const ID_MAX: i64 = 16_776_415;

sdrkit_core::choice_enum! {
    /// The time slot of direct mode.
    #[derive(Default)]
    pub enum DmrSlot {
        /// Slot 1.
        #[default]
        S1 = ("1", "Slot 1"),
        /// Slot 2.
        S2 = ("2", "Slot 2"),
    }
}

impl DmrSlot {
    /// The voice sync and the data sync of direct mode in the slot: indexes of [`crate::dmr::SYNCS`].
    pub fn syncs(self) -> (usize, usize) {
        match self {
            DmrSlot::S1 => (5, 6),
            DmrSlot::S2 => (7, 8),
        }
    }
}

/// The radio ID of the station: the source of every burst.
pub const RADIO_ID: Setting<field::I64> = Setting::new("radio_id", "Radio ID", "", 0)
    .range(0.0, ID_MAX as f64, 1.0)
    .help("The source of every transmission: the ID registered to the call sign of the operator, 1 to 16776415. Nothing is sent while it is 0.")
    .section(TX);
/// The radio called.
pub const CALLED_ID: Setting<field::I64> = Setting::new("called_id", "Radio ID called", "", 0)
    .range(0.0, ID_MAX as f64, 1.0)
    .help("The radio a call or a text goes to. With 0 it goes to the setting talkgroup.")
    .section(TX);
/// The talkgroup called.
pub const TALKGROUP: Setting<field::I64> = Setting::new("talkgroup", "Talkgroup", "", 0)
    .range(0.0, ID_MAX as f64, 1.0)
    .help("The talkgroup a call or a text goes to when the setting called_id is 0.")
    .section(TX);
/// The colour code.
pub const COLOUR_CODE: Setting<field::I64> = Setting::new("colour_code", "Colour code", "", 1)
    .range(0.0, 15.0, 1.0)
    .help("The colour code in the slot type and the EMB of every burst, 0 to 15.")
    .section(TX);
/// The time slot.
pub const SLOT: Setting<field::Choice<DmrSlot>> = Setting::choice("slot", "Time slot", DmrSlot::S1)
    .help("The slot of direct mode: its sync patterns tell a receiver the slot.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum TxSetting {
        /// The own radio ID.
        RadioId = RADIO_ID,
        /// The radio called.
        CalledId = CALLED_ID,
        /// The talkgroup called.
        Talkgroup = TALKGROUP,
        /// The colour code.
        ColourCode = COLOUR_CODE,
        /// The slot.
        Slot = SLOT,
        /// The transmit offset.
        Split = TX_SPLIT,
        /// The time-out of a voice call.
        Timeout = TX_TIMEOUT,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Push-to-talk.
        Ptt = DMR_PTT.command,
        /// A text.
        Send = DMR_SEND.command,
        /// Ends the transmission.
        Halt = HALT,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 3] = [DMR_PTT.command.key, DMR_SEND.command.key, HALT.key];

/// Why a transmit command was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DmrTxError {
    /// The setting `radio_id` is 0.
    NoSource,
    /// The settings `called_id` and `talkgroup` are both 0.
    NoTarget,
    /// Another transmission is under way.
    Busy,
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// The text is empty or needs more than [`DATA_MAX`] octets: the octets it needs.
    TooLong(usize),
}

impl DmrTxError {
    /// The key of the setting the error is about, if any.
    pub fn setting(&self) -> Option<&'static str> {
        match self {
            DmrTxError::NoSource => Some(RADIO_ID.key()),
            DmrTxError::NoTarget => Some(CALLED_ID.key()),
            _ => None,
        }
    }
}

impl std::fmt::Display for DmrTxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DmrTxError::NoSource => write!(
                f,
                "the setting {} is 0: enter the radio ID of the station",
                RADIO_ID.key()
            ),
            DmrTxError::NoTarget => write!(
                f,
                "the settings {} and {} are 0: enter the radio or the talkgroup called",
                CALLED_ID.key(),
                TALKGROUP.key()
            ),
            DmrTxError::Busy => f.write_str("another transmission is under way"),
            DmrTxError::NoInput => f.write_str("the stack has no input yet"),
            DmrTxError::TooLong(n) => write!(
                f,
                "the text needs {n} octets; a message holds 1 to {DATA_MAX}"
            ),
        }
    }
}

impl std::error::Error for DmrTxError {}

/// The octets and the defined data format of `text`: ISO 8859-1 (format 3) when every character is in it, else UTF-16LE
/// (format 19).
pub fn text_octets(text: &str) -> (Vec<u8>, u8) {
    if text.chars().all(|c| (c as u32) < 256) {
        (text.chars().map(|c| c as u32 as u8).collect(), 3)
    } else {
        (text.encode_utf16().flat_map(u16::to_le_bytes).collect(), 19)
    }
}

/// The 72 channel bits, as sent, of a half-rate silence frame (NXDN TS 1-A clause 7.2.1.2).
pub fn silence_frame() -> [u8; 72] {
    std::array::from_fn(|i| NXDN_EHR_SILENCE.coded_bit(i))
}

/// The voice part of a transmission: the call's LC, the place in the superframe, and the end.
struct Voice {
    lc: [u8; 9],
    cc: u8,
    data_sync: usize,
    voice_sync: usize,
    embedded: [u8; 128],
    pos: u8,
    made: u64,
    limit: u64,
    stopping: bool,
    ended: bool,
}

/// One transmission as 48 kHz samples: the bursts in order, one every 60 ms, each with its rise and fall and the carrier
/// off between them. It knows nothing of the graph, so a test can drive it.
pub struct Sender {
    kind: DataTxKind,
    bursts: VecDeque<[u8; 264]>,
    voice: Option<Voice>,
    encoder: Option<Box<Encoder>>,
    sent: u64,
    period: usize,
}

impl Sender {
    fn new(kind: DataTxKind, bursts: VecDeque<[u8; 264]>, voice: Option<Voice>) -> Sender {
        Sender {
            kind,
            bursts,
            encoder: voice
                .is_some()
                .then(|| Box::new(Encoder::new(Coder::HalfRate))),
            voice,
            sent: 0,
            period: (BURST_PERIOD_S * RATE).round() as usize,
        }
    }

    /// The text message `m` with colour code `cc` in `slot`.
    ///
    /// # Errors
    ///
    /// [`DmrTxError::TooLong`] when the octets of `m` are empty or more than [`DATA_MAX`].
    pub fn text(m: &ShortData, cc: u8, slot: DmrSlot) -> Result<Sender, DmrTxError> {
        if m.data.is_empty() || m.data.len() > DATA_MAX {
            return Err(DmrTxError::TooLong(m.data.len()));
        }
        let bursts = text_bursts(m, cc & 15, slot.syncs().1).into();
        Ok(Sender::new(DataTxKind::Text, bursts, None))
    }

    /// A voice call with the full LC `lc`, colour code `cc`, in `slot`, of at most `limit` voice bursts.
    pub fn voice(lc: &Lc, cc: u8, slot: DmrSlot, limit: u64) -> Sender {
        let lc = lc.to_bytes();
        let cc = cc & 15;
        let (voice_sync, data_sync) = slot.syncs();
        let header = lc_burst(&lc, dt::VOICE_LC_HEADER, cc, data_sync);
        let voice = Voice {
            lc,
            cc,
            data_sync,
            voice_sync,
            embedded: embedded_bits(&lc),
            pos: 0,
            made: 0,
            limit: limit.max(1),
            stopping: false,
            ended: false,
        };
        Sender::new(DataTxKind::Voice, VecDeque::from([header]), Some(voice))
    }

    /// What the transmission sends.
    pub fn kind(&self) -> DataTxKind {
        self.kind
    }

    /// The bursts made so far.
    pub fn bursts_sent(&self) -> u64 {
        self.sent
    }

    /// The bursts of a text not yet made.
    pub fn bursts_left(&self) -> usize {
        self.bursts.len()
    }

    /// Whether the transmission is a voice call that has not been told to end.
    pub fn speaking(&self) -> bool {
        self.voice.as_ref().is_some_and(|v| !v.stopping && !v.ended)
    }

    /// Ends the transmission after the burst on air: a text drops the bursts not yet made, a voice call sends the
    /// terminator next.
    pub fn stop(&mut self) {
        match &mut self.voice {
            Some(v) => v.stopping = true,
            None => self.bursts.clear(),
        }
    }

    /// The bits of the next burst: a queued burst, a voice burst whose three frames `speech` fills (72 channel bits each,
    /// as sent), or the terminator of a call; `None` after the last.
    pub fn next_burst(&mut self, mut speech: impl FnMut(&mut [u8; 72])) -> Option<[u8; 264]> {
        if let Some(b) = self.bursts.pop_front() {
            return Some(b);
        }
        let v = self.voice.as_mut()?;
        if v.ended {
            return None;
        }
        if v.stopping || v.made >= v.limit {
            v.ended = true;
            return Some(lc_burst(&v.lc, dt::TERMINATOR, v.cc, v.data_sync));
        }
        let mut frames = [[0u8; 72]; 3];
        for f in &mut frames {
            speech(f);
        }
        let b = voice_burst(&frames, v.pos, v.cc, v.voice_sync, &v.embedded);
        v.pos = (v.pos + 1) % 6;
        v.made += 1;
        Some(b)
    }

    /// Appends the 60 ms of the next burst of [`Sender::next_burst`] to `out`: its samples with the rise, the fall and
    /// silence to the next burst. False once the bursts are made.
    pub fn next_with(
        &mut self,
        speech: impl FnMut(&mut [u8; 72]),
        out: &mut Vec<Complex32>,
    ) -> bool {
        let Some(b) = self.next_burst(speech) else {
            return false;
        };
        let start = out.len();
        let mut burst = FskBurst::new(
            FskModulator::new(4800.0, TxPulse::RootRaisedCosine(0.2), 648.0),
            RATE,
            RAMP_S,
        );
        burst.levels(&bits_to_levels(&b), out);
        burst.end(out);
        out.resize(
            start + self.period.max(out.len() - start),
            Complex32::default(),
        );
        self.sent += 1;
        true
    }

    /// As [`Sender::next_with`], with silence frames in the voice bursts.
    pub fn next(&mut self, out: &mut Vec<Complex32>) -> bool {
        let silence = silence_frame();
        self.next_with(|f| *f = silence, out)
    }
}

impl Maker for Sender {
    /// Codes the speech of each voice frame from 20 ms of `mic`.
    fn next(&mut self, mic: &mut MicInput, out: &mut Vec<Complex32>) -> bool {
        let mut encoder = self.encoder.take();
        let mut speech = [0f32; FRAME];
        let mut pcm = [0i16; FRAME];
        let more = self.next_with(
            |frame| {
                mic.read(&mut speech);
                for (p, s) in pcm.iter_mut().zip(speech) {
                    *p = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                }
                match encoder.as_mut() {
                    Some(e) => {
                        e.encode(&pcm, frame);
                    }
                    None => *frame = silence_frame(),
                }
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
        self.sent
    }
}

/// The values of the settings.
#[derive(Clone, Debug, PartialEq)]
struct TxConfig {
    radio_id: u32,
    called_id: u32,
    talkgroup: u32,
    colour_code: u8,
    slot: DmrSlot,
    split: f64,
    timeout: f64,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            radio_id: 0,
            called_id: 0,
            talkgroup: 0,
            colour_code: COLOUR_CODE.default as u8,
            slot: DmrSlot::S1,
            split: TX_SPLIT.default,
            timeout: TX_TIMEOUT.default,
        }
    }
}

/// The sender (`dmr.transmit`). See the module.
pub struct DmrTransmit {
    cfg: TxConfig,
    core: TxCore,
    last_kind: Option<DataTxKind>,
    results: Published,
}

impl Default for DmrTransmit {
    fn default() -> Self {
        let mut t = DmrTransmit {
            cfg: TxConfig::default(),
            core: TxCore::default(),
            last_kind: None,
            results: Published::new(DataTxFields::SHAPE.fields),
        };
        t.publish();
        t
    }
}

impl DmrTransmit {
    /// The transmit frequency: the dial plus the transmit offset.
    pub fn frequency(&self) -> f64 {
        self.core.dial() + self.cfg.split
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.core.transmitting()
    }

    /// The target of the settings: the radio called, else the talkgroup, and whether it is a talkgroup.
    ///
    /// # Errors
    ///
    /// [`DmrTxError::NoSource`] while the setting `radio_id` is 0, [`DmrTxError::NoTarget`] while no target is set.
    fn target(&self) -> Result<(u32, bool), CommandError> {
        if self.cfg.radio_id == 0 {
            return Err(failed(DmrTxError::NoSource));
        }
        match (self.cfg.called_id, self.cfg.talkgroup) {
            (0, 0) => Err(failed(DmrTxError::NoTarget)),
            (0, tg) => Ok((tg, true)),
            (id, _) => Ok((id, false)),
        }
    }

    /// Push-to-talk on: starts a voice call to the radio or the talkgroup of the settings; a call already under way goes
    /// on.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`DmrTxError`] when another transmission runs, the stack has no input, the
    /// setting `radio_id` is 0 or no target is set.
    pub fn key_down(&mut self) -> Result<(), CommandError> {
        if self.core.speaking() {
            return Ok(());
        }
        self.core.ready(DmrTxError::Busy, DmrTxError::NoInput)?;
        let (dst, group) = self.target()?;
        let lc = Lc {
            pf: false,
            flco: if group { 0 } else { 3 },
            fid: 0,
            opts: 0,
            dst,
            src: self.cfg.radio_id,
        };
        let limit = (self.cfg.timeout / BURST_PERIOD_S).ceil() as u64;
        let sender = Sender::voice(&lc, self.cfg.colour_code, self.cfg.slot, limit);
        self.core
            .begin(Box::new(sender), RATE, DmrTxError::NoInput)?;
        self.last_kind = Some(DataTxKind::Voice);
        Ok(())
    }

    /// Push-to-talk off: the voice call ends after the burst on air, with the terminator.
    pub fn key_up(&mut self) {
        if self.core.speaking() {
            self.core.halt();
        }
    }

    /// Sends `text` to the radio or the talkgroup of the settings.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`DmrTxError`] when another transmission runs, the stack has no input, the
    /// setting `radio_id` is 0, no target is set, or the text does not fit.
    pub fn send(&mut self, text: &str) -> Result<(), CommandError> {
        self.core.ready(DmrTxError::Busy, DmrTxError::NoInput)?;
        let (dst, group) = self.target()?;
        let (data, format) = text_octets(text);
        let m = ShortData {
            src: self.cfg.radio_id,
            dst,
            group,
            format,
            data: &data,
        };
        let sender = Sender::text(&m, self.cfg.colour_code, self.cfg.slot).map_err(failed)?;
        self.core
            .begin(Box::new(sender), RATE, DmrTxError::NoInput)?;
        self.last_kind = Some(DataTxKind::Text);
        Ok(())
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let f = DataTxFields {
            transmitting: self.core.transmitting(),
            tx_freq_hz: self.core.transmitting().then(|| Hertz(self.frequency())),
            tx_kind: self.last_kind,
            tx_frames: self.core.frames() as i64,
            tx_note: NOTE,
        };
        self.results.put(&f);
    }
}

/// A radio ID or a talkgroup of a setting value.
fn id_of(v: i64, id: SettingId) -> Result<u32, SettingError> {
    u32::try_from(v)
        .ok()
        .filter(|&x| i64::from(x) <= ID_MAX)
        .ok_or(SettingError::Malformed { id, at: None })
}

impl Component for DmrTransmit {
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
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::RadioId => self.cfg.radio_id = id_of(RADIO_ID.read(id, v)?, id)?,
            TxSetting::CalledId => self.cfg.called_id = id_of(CALLED_ID.read(id, v)?, id)?,
            TxSetting::Talkgroup => self.cfg.talkgroup = id_of(TALKGROUP.read(id, v)?, id)?,
            TxSetting::ColourCode => {
                self.cfg.colour_code = u8::try_from(COLOUR_CODE.read(id, v)?)
                    .ok()
                    .filter(|c| *c <= 15)
                    .ok_or(SettingError::Malformed { id, at: None })?;
            }
            TxSetting::Slot => self.cfg.slot = SLOT.read(id, v)?,
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
            (TxSetting::RadioId, Value::I64(i64::from(self.cfg.radio_id))),
            (
                TxSetting::CalledId,
                Value::I64(i64::from(self.cfg.called_id)),
            ),
            (
                TxSetting::Talkgroup,
                Value::I64(i64::from(self.cfg.talkgroup)),
            ),
            (
                TxSetting::ColourCode,
                Value::I64(i64::from(self.cfg.colour_code)),
            ),
            (TxSetting::Slot, Value::Choice(self.cfg.slot.index())),
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
                let PttArgs { on } = DMR_PTT.args(args)?;
                if on {
                    self.key_down()?;
                } else {
                    self.key_up();
                }
            }
            TxCommand::Send => {
                let DataSendArgs { text } = DMR_SEND.args(args)?;
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

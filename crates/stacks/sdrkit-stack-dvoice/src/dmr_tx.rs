//! The sender of the DMR stack (`dmr.transmit`): a text message as defined short data in TDMA direct mode, one slot,
//! simplex. Standard: ETSI TS 102 361-1 to -3 (the burst layout that [`crate::dmr`] decodes).
//!
//! The library has no AMBE+2 encoder, so the sender sends no voice: a transmission is a text.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Output `out` is complex baseband at the session's rate
//! for `tx.sink`: the signal at 0 Hz, centred on the transmit frequency, which is the dial plus the transmit offset.
//! Output `keyed` mutes the receive audio of the stack from key down to a short hold after the end mark.
//!
//! A transmission is two preamble CSBKs, the data header and the rate 1/2 data blocks of the message, each a burst of
//! 27.5 ms with the direct-mode data sync of the slot, one burst every 60 ms; the carrier is off between the bursts. The
//! last data block ends it; `halt` ends it after the burst on air. 4FSK at 4800 symbols/s, root raised cosine roll-off
//! 0.2, 648 Hz a level step. Each burst rises and falls with a 1.5 ms raised cosine; its magnitude is 0.9 of full scale.
//! Every transmission ends with an end mark; on a live radio whose host times bursts, the first sample states its time on
//! air.

use crate::dmr::{text_bursts, ShortData, SHORT_DATA_BLOCKS_MAX};
use crate::front::RATE;
use crate::gen::{bits_to_levels, FskModulator, TxPulse};
use crate::sender::{failed, FskBurst, Maker, TxCore, TX, TX_SPLIT};
use crate::typed::{DataSendArgs, DataTxFields, DataTxKind, DMR_SEND, HALT};
use sdrkit_components::tx::KeyedOut;
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
    "AMBE+2 encoder not installed: the stack sends text messages as short data, no voice";
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

/// The radio ID of the station: the source of every burst.
pub const RADIO_ID: Setting<field::I64> = Setting::new("radio_id", "Radio ID", "", 0)
    .range(0.0, ID_MAX as f64, 1.0)
    .help("The source of every transmission: the ID registered to the call sign of the operator, 1 to 16776415. Nothing is sent while it is 0.")
    .section(TX);
/// The radio called.
pub const CALLED_ID: Setting<field::I64> = Setting::new("called_id", "Radio ID called", "", 0)
    .range(0.0, ID_MAX as f64, 1.0)
    .help("The radio a text goes to. With 0 the text goes to the setting talkgroup.")
    .section(TX);
/// The talkgroup called.
pub const TALKGROUP: Setting<field::I64> = Setting::new("talkgroup", "Talkgroup", "", 0)
    .range(0.0, ID_MAX as f64, 1.0)
    .help("The talkgroup a text goes to when the setting called_id is 0.")
    .section(TX);
/// The colour code.
pub const COLOUR_CODE: Setting<field::I64> = Setting::new("colour_code", "Colour code", "", 1)
    .range(0.0, 15.0, 1.0)
    .help("The colour code in the slot type of every burst, 0 to 15.")
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
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// A text.
        Send = DMR_SEND.command,
        /// Ends the transmission.
        Halt = HALT,
    }
}

/// The keys of the commands of the sender, which the stack routes to it.
pub const COMMANDS: [&str; 2] = [DMR_SEND.command.key, HALT.key];

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

/// One transmission as 48 kHz samples: the bursts in order, one every 60 ms, each with its rise and fall and the carrier
/// off between them. It knows nothing of the graph, so a test can drive it.
pub struct Sender {
    bursts: VecDeque<[u8; 264]>,
    sent: u64,
    period: usize,
}

impl Sender {
    /// The text message `m` with colour code `cc` in `slot`.
    ///
    /// # Errors
    ///
    /// [`DmrTxError::TooLong`] when the octets of `m` are empty or more than [`DATA_MAX`].
    pub fn text(m: &ShortData, cc: u8, slot: DmrSlot) -> Result<Sender, DmrTxError> {
        if m.data.is_empty() || m.data.len() > DATA_MAX {
            return Err(DmrTxError::TooLong(m.data.len()));
        }
        let sync = match slot {
            DmrSlot::S1 => 6,
            DmrSlot::S2 => 8,
        };
        Ok(Sender {
            bursts: text_bursts(m, cc & 15, sync).into(),
            sent: 0,
            period: (BURST_PERIOD_S * RATE).round() as usize,
        })
    }

    /// The bursts made so far.
    pub fn bursts_sent(&self) -> u64 {
        self.sent
    }

    /// The bursts not yet made.
    pub fn bursts_left(&self) -> usize {
        self.bursts.len()
    }

    /// Appends the 60 ms of the next burst to `out`: its samples with the rise, the fall and silence to the next burst.
    /// False once the bursts are made.
    pub fn next(&mut self, out: &mut Vec<Complex32>) -> bool {
        let Some(b) = self.bursts.pop_front() else {
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
}

impl Maker for Sender {
    fn next(&mut self, out: &mut Vec<Complex32>) -> bool {
        Sender::next(self, out)
    }
    fn stop(&mut self) {
        self.bursts.clear();
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

    /// Sends `text` to the radio or the talkgroup of the settings.
    ///
    /// # Errors
    ///
    /// [`CommandError::TransmitOff`], or [`DmrTxError`] when another transmission runs, the stack has no input, the
    /// setting `radio_id` is 0, no target is set, or the text does not fit.
    pub fn send(&mut self, text: &str) -> Result<(), CommandError> {
        self.core.ready(DmrTxError::Busy, DmrTxError::NoInput)?;
        if self.cfg.radio_id == 0 {
            return Err(failed(DmrTxError::NoSource));
        }
        let (dst, group) = match (self.cfg.called_id, self.cfg.talkgroup) {
            (0, 0) => return Err(failed(DmrTxError::NoTarget)),
            (0, tg) => (tg, true),
            (id, _) => (id, false),
        };
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
                Port::audio("mic").optional("not used: the library has no AMBE+2 encoder"),
            ],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.core.configure(&inputs[0]);
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

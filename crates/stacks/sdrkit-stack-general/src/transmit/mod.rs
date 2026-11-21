//! The transmit branch of the general stack (`general.transmit`): push-to-talk speech in NFM, AM, USB and LSB from the
//! microphone input, Morse code from text in CW, and a tune carrier.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `mic` is mono audio of any rate (48 kHz is used as
//! it is). Output `out` is complex baseband at the session's rate for `tx.sink`: the signal at 0 Hz, the format centred on
//! the transmit frequency, which is the dial plus the channel offset plus the transmit offset. For CW and the tune carrier
//! the carrier is at that frequency; the receive side hears a carrier at the channel centre at its pitch. Output `keyed`
//! tells the squelch gate to mute the receive audio. Each pass writes as many samples as the pass brings in, and each
//! transmission ends with an end mark: after ptt off, `halt`, the time-out, the tune time, the end of the text, the
//! transmitter going off, a new format and the end of the input. A setting change applies to the next transmission.
//! On a live radio whose host times bursts (`HostStreams::transmit_lead`), the first sample of each transmission states
//! its time on air (`TagKind::BurstStart`): the time of the pass that writes it plus the lead of the host and two
//! passes. The host puts its zeros in front of that sample, so the start of the transmission is not cut.

pub mod settings;
pub mod voice;

use crate::transmit::settings::{Channel, TxSettings, TX_KEYS};
use crate::transmit::voice::VoiceChain;
use crate::typed::{SendArgs, TransmitFields, TransmitKind, HALT, PTT, SEND, TUNE};
use crate::types::Demod;
use sdrkit_components::tx::KeyedOut;
use sdrkit_components::voice::{Content, KeyedSender, PttArgs, SendError};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, ConfigError, Published, Record, Results,
    Setting, SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};
use sdrkit_util_text::morse::Keyer;

const DEMOD: Setting<field::Choice<Demod>> = Setting::choice("demod", "Mode", Demod::Nfm);
const WIDTH: Setting<field::Hz> =
    Setting::new("width", "Filter width", "Hz", 12_500.0).range(0.0, 220_000.0, 0.0);
const OFFSET: Setting<field::Hz> =
    Setting::new("channel_offset", "Channel offset", "Hz", 0.0).range(-1e8, 1e8, 0.0);

sdrkit_core::settings! {
    /// The settings of the transmitter: the receive channel, then the transmit settings of the stack.
    pub enum TransmitSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// The filter width.
        Width = WIDTH,
        /// The channel offset.
        Offset = OFFSET,
        /// The transmit offset.
        Split = settings::TX_SPLIT,
        /// The NFM deviation.
        Deviation = settings::TX_DEVIATION,
        /// The NFM pre-emphasis.
        Preemphasis = settings::TX_PREEMPHASIS,
        /// The CTCSS tone.
        Ctcss = settings::TX_CTCSS,
        /// The AM depth.
        AmDepth = settings::TX_AM_DEPTH,
        /// The SSB method.
        SsbMethod = settings::TX_SSB_METHOD,
        /// The SSB band.
        SsbBand = settings::TX_SSB_BAND,
        /// The SSB compression.
        Compression = settings::TX_COMPRESSION,
        /// The time-out.
        Timeout = settings::TX_TIMEOUT,
        /// The CW speed.
        CwWpm = settings::TX_CW_WPM,
        /// The CW overall speed.
        CwFarnsworth = settings::TX_CW_FARNSWORTH,
        /// The tune time.
        TuneLimit = settings::TX_TUNE_LIMIT,
    }
}

sdrkit_core::commands! {
    /// The commands of the transmitter.
    pub enum TransmitCommand {
        /// Push-to-talk.
        Ptt = PTT.command,
        /// Morse code.
        Send = SEND.command,
        /// The tune carrier.
        Tune = TUNE,
        /// Ends every transmission.
        Halt = HALT,
    }
}

/// Why a transmit command was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// Another transmission is under way.
    Busy,
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
    /// WFM is not offered for transmission.
    Wfm,
    /// Push-to-talk needs a voice mode.
    NotVoice,
    /// Morse code needs CW.
    NotCw,
    /// No character of the text has a Morse code.
    NoMorse,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TxError::Off => "the transmitter is off",
            TxError::Busy => "another transmission is under way",
            TxError::NoInput => "the stack has no input yet",
            TxError::Wfm => "WFM is not offered for transmission",
            TxError::NotVoice => "push-to-talk sends speech in NFM, AM, USB or LSB",
            TxError::NotCw => "send keys Morse code with the demodulator on CW",
            TxError::NoMorse => "no character of the text has a Morse code",
        })
    }
}

impl std::error::Error for TxError {}

impl From<SendError> for TxError {
    fn from(e: SendError) -> Self {
        match e {
            SendError::Off => TxError::Off,
            SendError::Busy => TxError::Busy,
            SendError::NoInput => TxError::NoInput,
        }
    }
}

/// The transmitter (`general.transmit`). See the module.
pub struct Transmitter {
    channel: Channel,
    tx: TxSettings,
    dial: f64,
    sender: KeyedSender,
    kind: TransmitKind,
    results: Published,
}

impl Default for Transmitter {
    fn default() -> Self {
        let mut t = Transmitter {
            channel: Channel::default(),
            tx: TxSettings::default(),
            dial: 0.0,
            sender: KeyedSender::default(),
            kind: TransmitKind::Voice,
            results: Published::new(TransmitFields::SHAPE.fields),
        };
        t.publish();
        t
    }
}

impl Transmitter {
    /// The transmit frequency: the dial plus the channel offset plus the transmit offset.
    pub fn frequency(&self) -> f64 {
        self.dial + self.channel.offset + self.tx.split
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.sender.transmitting()
    }

    /// Whether the receive audio is muted: a transmission, its end mark or the hold after it.
    pub fn keyed(&self) -> bool {
        self.sender.keyed()
    }

    /// Starts a transmission of `kind`; `text` is the text of Morse code.
    ///
    /// # Errors
    ///
    /// [`TxError`] when the transmitter is off, another transmission runs, the stack has no input, or the mode does not fit.
    pub fn start(&mut self, kind: TransmitKind, text: &str) -> Result<(), TxError> {
        if !self.sender.allowed() {
            return Err(TxError::Off);
        }
        let demod = self.channel.demod;
        if demod == Demod::Wfm {
            return Err(TxError::Wfm);
        }
        if kind == TransmitKind::Voice && self.sender.talking() {
            return Ok(());
        }
        if self.sender.transmitting() {
            return Err(TxError::Busy);
        }
        if self.sender.rate() <= 0.0 {
            return Err(TxError::NoInput);
        }
        let (content, limit) = match kind {
            TransmitKind::Voice => (
                Content::Voice(Box::new(
                    VoiceChain::new(&self.channel, &self.tx)
                        .ok_or(TxError::NotVoice)?
                        .into_inner(),
                )),
                Some(self.tx.timeout),
            ),
            TransmitKind::Text => {
                if demod != Demod::Cw {
                    return Err(TxError::NotCw);
                }
                let wpm = self.tx.cw_wpm;
                let overall = if self.tx.cw_farnsworth > 0.0 {
                    self.tx.cw_farnsworth.min(wpm)
                } else {
                    wpm
                };
                let keyer = Keyer::farnsworth(wpm, overall).map_err(|_| TxError::NoMorse)?;
                let keys: Vec<(bool, f64)> = keyer.key(text).map(|k| (k.on, k.seconds)).collect();
                if keys.is_empty() {
                    return Err(TxError::NoMorse);
                }
                (Content::Keys(keys), None)
            }
            TransmitKind::Carrier => (Content::Carrier, Some(self.tx.tune_limit)),
        };
        self.sender.start(content, limit, None)?;
        self.kind = kind;
        Ok(())
    }

    /// Ends the transmission of `kind` (any kind for `None`), with its fall and end mark.
    pub fn stop(&mut self, kind: Option<TransmitKind>) {
        if kind.is_none_or(|k| k == self.kind) {
            self.sender.stop();
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let f = if self.sender.transmitting() {
            let m = self.sender.take_meters();
            TransmitFields {
                transmitting: true,
                tx_freq_hz: Some(Hertz(self.frequency())),
                tx_kind: Some(self.kind),
                tx_time_s: Some(Seconds(self.sender.time_s())),
                tx_mic_dbfs: m.map(|m| m.mic_dbfs()),
                tx_peak_db: m.map(|m| m.peak_db()),
                tx_level_db: m.map(|m| f64::from(m.level_db)),
                tx_limit_db: m.map(|m| f64::from(m.limit_db)),
            }
        } else {
            TransmitFields {
                transmitting: false,
                tx_freq_hz: None,
                tx_kind: None,
                tx_time_s: None,
                tx_mic_dbfs: None,
                tx_peak_db: None,
                tx_level_db: None,
                tx_limit_db: None,
            }
        };
        self.results.put(&f);
    }
}

impl Component for Transmitter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.transmit",
            vec![
                Port::iq("in"),
                Port::audio("mic").optional("silence: push-to-talk sends no speech"),
            ],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(TransmitSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.sender.configure(f, &inputs[1]);
        self.publish();
        let c = self.frequency();
        Ok(vec![Format::iq(f.rate, c, c), KeyedOut::format()])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.sender.attach(ctx);
    }
    fn process(&mut self, io: &mut Io) {
        if self.sender.run(io) {
            self.publish();
        }
    }
    fn finish(&mut self, io: &mut Io) {
        if self.sender.run(io) {
            self.publish();
        }
        if self.sender.end(io) {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let which = TransmitSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        match which {
            TransmitSetting::Demod => {
                let d = DEMOD.read(id, v)?;
                if d != self.channel.demod {
                    self.channel.demod = d;
                    self.sender.cut();
                }
                return Ok(Applied::Live);
            }
            TransmitSetting::Width => {
                self.channel.width = WIDTH.read(id, v)?;
                return Ok(Applied::Live);
            }
            TransmitSetting::Offset => {
                let o = OFFSET.read(id, v)?;
                let moved = o != self.channel.offset;
                self.channel.offset = o;
                return Ok(if moved {
                    Applied::Reformat
                } else {
                    Applied::Live
                });
            }
            _ => {}
        }
        let key = TransmitSetting::schema().settings()[id.index()]
            .field
            .key
            .clone();
        let split = self.tx.split;
        self.tx.apply(&key, id, v)?;
        Ok(if self.tx.split != split {
            Applied::Reformat
        } else {
            Applied::Live
        })
    }
    fn values(&self) -> Values {
        let mut v = sdrkit_graph::values_of([
            (
                TransmitSetting::Demod,
                Value::Choice(self.channel.demod.index()),
            ),
            (TransmitSetting::Width, Value::Hz(self.channel.width)),
            (TransmitSetting::Offset, Value::Hz(self.channel.offset)),
        ]);
        let schema = TransmitSetting::schema();
        for (key, value) in TX_KEYS.iter().zip(self.tx.values()) {
            if let Ok(id) = schema.id(key) {
                let _ = v.set(id, &value);
            }
        }
        v
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        TransmitCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let failed = |e: TxError| CommandError::Failed {
            source: StageError::new(e),
        };
        match TransmitCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TransmitCommand::Ptt => {
                let PttArgs { on } = PTT.args(args)?;
                if on {
                    self.start(TransmitKind::Voice, "").map_err(failed)?;
                } else {
                    self.stop(Some(TransmitKind::Voice));
                }
            }
            TransmitCommand::Send => {
                let SendArgs { text } = SEND.args(args)?;
                self.start(TransmitKind::Text, text).map_err(failed)?;
            }
            TransmitCommand::Tune => self.start(TransmitKind::Carrier, "").map_err(failed)?,
            TransmitCommand::Halt => self.stop(None),
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

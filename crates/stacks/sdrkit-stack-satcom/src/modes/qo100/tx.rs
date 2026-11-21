//! The voice sender of the qo100 stack (`qo100.tx`): push-to-talk speech in upper sideband on the uplink of the narrowband
//! transponder. The speech processor, the sideband filter and the transmissions are the shared ones
//! (`sdrkit_components::voice`); the audio band is 300 to 2700 Hz above the uplink dial, an occupied bandwidth under 2.7 kHz.
//!
//! The transponder moves the 2.4 GHz uplink to the 10 GHz downlink by a fixed translation ([`TRANSLATION_HZ`]): the sender
//! transmits at the dial of the stack (the setting `dial_hz`, a downlink frequency) less the translation. The stack does
//! not send the callsign: the operator speaks it.
//!
//! Input `in` is the session's samples: their rate and pace. Input `mic` is mono audio of any rate. Output `out` is complex
//! baseband at the session's rate for `tx.sink`, the suppressed carrier at 0 Hz, the format centred on the uplink
//! frequency. Output `keyed` is the keyed state. Each transmission rises and falls in 5 ms and ends with an end mark:
//! after `ptt` off, the time-out, the transmit setting going off, a new format and the end of the input. The engine's
//! transmit setting is the only switch. The receive side keeps running while the sender is keyed.

use super::display::DIAL;
use crate::typed::{Qo100TxFields, QO100_PTT};
use sdrkit_components::tx::KeyedOut;
use sdrkit_components::voice::{
    Content, KeyedSender, PttArgs, SsbMethod, VoiceChain, VoiceMode, SSB_BAND_HZ,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    ConfigError, Record, Results, Setting, SettingError, SettingId, SettingSet, StageError, Value,
    Values,
};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Port, RunContext};

/// The section of the transmit settings.
pub const TX: &str = "Transmit";
/// The translation of the narrowband transponder: the downlink frequency less the uplink frequency, Hz.
pub const TRANSLATION_HZ: f64 = 8089.5e6;
/// The audio band sent, Hz.
pub const AUDIO_HZ: (f64, f64) = SSB_BAND_HZ;

/// The peak of the signal sent.
pub const TX_LEVEL: Setting<field::F64> =
    Setting::new("tx_level", "Peak level", "of full scale", 0.4)
        .range(0.05, 0.9, 0.0)
        .help("The peak magnitude of the signal sent, of full scale. Set it so that your downlink stays below the beacon.")
        .section(TX);
/// The time-out of push-to-talk.
pub const TX_TIMEOUT: Setting<field::Duration> = Setting::new("tx_timeout", "Time-out", "s", 180.0)
    .range(1.0, 3600.0, 1.0)
    .help("Push-to-talk ends after this time on air.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum Qo100TxSetting {
        /// The dial of the stack on the downlink.
        Dial = DIAL,
        /// The peak level.
        Level = TX_LEVEL,
        /// The time-out.
        Timeout = TX_TIMEOUT,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum Qo100TxCommand {
        /// Push-to-talk.
        Ptt = QO100_PTT.command,
    }
}

/// The uplink frequency of the downlink frequency `downlink_hz`, Hz.
pub fn uplink_hz(downlink_hz: f64) -> f64 {
    downlink_hz - TRANSLATION_HZ
}

/// The voice mode sent: upper sideband by the filter method, no compression.
pub fn voice_mode() -> VoiceMode {
    VoiceMode::Ssb {
        upper: true,
        method: SsbMethod::Filter,
        band_hz: AUDIO_HZ,
        compression_db: 0.0,
    }
}

/// The sender (`qo100.tx`). See the module.
pub struct QoSender {
    dial_hz: f64,
    level: f64,
    timeout: f64,
    sender: KeyedSender,
    results: Published,
}

impl Default for QoSender {
    fn default() -> Self {
        let mut s = QoSender {
            dial_hz: DIAL.default,
            level: TX_LEVEL.default,
            timeout: TX_TIMEOUT.default,
            sender: KeyedSender::default(),
            results: Published::new(Qo100TxFields::SHAPE.fields),
        };
        s.publish();
        s
    }
}

impl QoSender {
    /// The uplink frequency of the dial, Hz.
    pub fn frequency(&self) -> f64 {
        uplink_hz(self.dial_hz)
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let on = self.sender.transmitting();
        let m = self.sender.take_meters();
        self.results.put(&Qo100TxFields {
            transmitting: on,
            tx_freq_hz: Hertz(self.frequency()),
            tx_time_s: on.then(|| Seconds(self.sender.time_s())),
            tx_mic_dbfs: m.map(|m| m.mic_dbfs()),
            tx_peak_db: m.map(|m| m.peak_db()),
            tx_level_db: m.map(|m| f64::from(m.level_db)),
            tx_limit_db: m.map(|m| f64::from(m.limit_db)),
        });
    }
}

impl Component for QoSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "qo100.tx",
            vec![
                Port::iq("in"),
                Port::audio("mic").optional("silence: push-to-talk sends no speech"),
            ],
            vec![Port::iq("out"), KeyedOut::port()],
        )
        .with_settings(Qo100TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.sender.configure(&inputs[0], &inputs[1]);
        self.publish();
        let c = self.frequency();
        Ok(vec![Format::iq(inputs[0].rate, c, c), KeyedOut::format()])
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
        match Qo100TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Qo100TxSetting::Dial => {
                let d = DIAL.read(id, v)?;
                let moved = d != self.dial_hz;
                self.dial_hz = d;
                if moved {
                    return Ok(Applied::Reformat);
                }
            }
            Qo100TxSetting::Level => self.level = TX_LEVEL.read(id, v)?,
            Qo100TxSetting::Timeout => self.timeout = TX_TIMEOUT.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (Qo100TxSetting::Dial, Value::Hz(self.dial_hz)),
            (Qo100TxSetting::Level, Value::F64(self.level)),
            (Qo100TxSetting::Timeout, Value::Duration(self.timeout)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        Qo100TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match Qo100TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            Qo100TxCommand::Ptt => {
                let PttArgs { on } = QO100_PTT.args(args)?;
                if !on {
                    self.sender.stop();
                } else if !self.sender.talking() {
                    let chain = VoiceChain::new(&voice_mode())
                        .expect("the fixed voice mode of the sender is valid");
                    self.sender
                        .start(
                            Content::Voice(Box::new(chain)),
                            Some(self.timeout),
                            Some(self.level as f32),
                        )
                        .map_err(|e| CommandError::Failed {
                            source: StageError::new(e),
                        })?;
                }
            }
        }
        self.publish();
        Ok(None)
    }
}

//! Listen: the sideband audio as mono audio for the host's mixer, only while the setting is on. Decoding never depends on it.

use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port};

/// The `listen` setting.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false)
    .help("Play the sideband (dial + 200 Hz to 3.8 kHz). Decoding is the same either way.");

sdrkit_core::settings! {
    /// The settings of [`Listen`].
    pub enum ListenSetting {
        /// Listen.
        Listen = LISTEN,
    }
}

/// `hfdata.listen`: real sideband audio in, the same audio as mono `Audio` out while `listen` is on.
#[derive(Debug, Default)]
pub struct Listen {
    on: bool,
}

impl Component for Listen {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.listen",
            vec![Port::real("in")],
            vec![Port::audio("out").same_format_as(0)],
        )
        .with_settings(ListenSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::Audio)])
    }
    fn process(&mut self, io: &mut Io) {
        if !self.on {
            return;
        }
        let x = io.real(0);
        io.audio_out(0).samples.extend_from_slice(x);
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match ListenSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ListenSetting::Listen => self.on = LISTEN.read(id, value)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(ListenSetting::Listen, Value::Bool(self.on))])
    }
}

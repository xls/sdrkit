//! The listening chain: what the speaker hears of any audio stack, [`cleanup::Cleanup`] then [`control::AudioControl`].
//! [`ListeningChain`] is the plain object for the audio path of a host, and [`ListeningComponent`] (`general.listening`) is the
//! same chain as a component. While off, the chain passes its input through unchanged.

pub mod cleanup;
pub mod control;
pub mod settings;

use crate::typed::ListeningFields;
use cleanup::Cleanup;
use control::AudioControl;
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::{
    ConfigError, Published, Results, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{Applied, ChannelRule, Component, ComponentInfo, Format, Io, Port};
pub use settings::{
    AgcSpeed, ChainDeemph, ChainSettings, ChainSquelch, EqPreset, ListenSetting, ModeKind,
    StereoMode,
};

/// The listening chain: cleanup, then audio control.
pub struct ListeningChain {
    rate: f64,
    settings: ChainSettings,
    cleanup: Cleanup,
    control: AudioControl,
}

impl ListeningChain {
    /// At `rate` (at least 8 kHz) with `settings`.
    pub fn new(rate: f64, settings: ChainSettings) -> Self {
        let rate = rate.max(8000.0);
        ListeningChain {
            rate,
            cleanup: Cleanup::new(rate, settings.cleanup.clone()),
            control: AudioControl::new(rate, settings.control.clone()),
            settings,
        }
    }
    /// The rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }
    /// The settings.
    pub fn settings(&self) -> &ChainSettings {
        &self.settings
    }
    /// New settings (from the next sample, with fades).
    pub fn set(&mut self, s: ChainSettings) {
        self.cleanup.set(s.cleanup.clone());
        self.control.set(s.control.clone());
        self.settings = s;
    }
    /// Processes the stereo frames `buf` in place. Returns false, leaving `buf` untouched, while both parts are off.
    pub fn process(&mut self, buf: &mut [[f32; 2]]) -> bool {
        let a = self.cleanup.process(buf);
        let b = self.control.process(buf);
        a || b
    }
    /// The cleanup part.
    pub fn cleanup(&self) -> &Cleanup {
        &self.cleanup
    }
    /// The audio control part.
    pub fn control(&self) -> &AudioControl {
        &self.control
    }
    /// The audio control part, mutable (meters, AF spectrum).
    pub fn control_mut(&mut self) -> &mut AudioControl {
        &mut self.control
    }
    /// Whether the recorder records the sound now: the squelch is open (when the settings require it) and the cleanup's gate is
    /// not shut (when "skip silence" is on).
    pub fn recorder_takes(&self) -> bool {
        self.control.recorder_open()
            && !(self.settings.cleanup.record_cleaned
                && self.settings.cleanup.record_skip_silence
                && self.cleanup.gate_shut())
    }
}

/// The listening chain as a component (`general.listening`): stereo `Audio` in and out. Its settings are those of
/// [`ListenSetting`]. It publishes [`ListeningFields`] every 0.1 s.
pub struct ListeningComponent {
    chain: Option<ListeningChain>,
    settings: ChainSettings,
    results: Published,
    count: u64,
    report: u64,
}

impl ListeningComponent {
    /// With `settings`.
    pub fn new(settings: ChainSettings) -> Self {
        ListeningComponent {
            chain: None,
            settings,
            results: Published::new(ListeningFields::SHAPE.fields),
            count: 0,
            report: 4800,
        }
    }
}

impl Default for ListeningComponent {
    fn default() -> Self {
        Self::new(ChainSettings::default())
    }
}

/// The lowest rate of the listening chain.
pub const MIN_RATE_HZ: f64 = 8000.0;

impl Component for ListeningComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.listening",
            vec![Port::audio("in").channels(ChannelRule::Exact(2))],
            vec![Port::audio("out").same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(ListenSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        if rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: rate,
            });
        }
        self.chain = Some(ListeningChain::new(rate, self.settings.clone()));
        self.report = (rate / 10.0) as u64;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(c) = self.chain.as_mut() else { return };
        let buf = io.audio_pass_through(0, 0);
        let (frames, _) = buf.as_chunks_mut::<2>();
        let n = frames.len() as u64;
        let active = c.process(frames);
        let before = self.count;
        self.count += n;
        if self.count / self.report.max(1) != before / self.report.max(1) {
            let (peak, rms, clip) = c.control_mut().meters.take();
            self.results.put(&ListeningFields {
                active,
                squelch: c.control().squelch_open(),
                peak: f64::from(peak),
                rms: f64::from(rms),
                clip,
                agc_db: f64::from(c.control().agc_gain_db()),
                auto_notch: c.control().auto_notch_hz().map(Hertz),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.settings.apply(id, v)?;
        if let Some(c) = &mut self.chain {
            c.set(self.settings.clone());
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.settings.values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

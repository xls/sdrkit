//! The transmit branch of Airband (`avdata.airband_transmit`): push-to-talk AM voice from the microphone input.
//!
//! The signal is double sideband AM with full carrier, the carrier at the tuned frequency. The audio passes the microphone
//! gain, a 4th-order high-pass and an 8th-order low-pass at the edges of the band of the channel spacing, a leveller that
//! brings the RMS towards [`LEVEL_RMS`] within +-12 dB, and a look-ahead limiter at full scale whose gain has no steps. So
//! the depth of the 48 kHz samples never passes the set depth, and the depth on air never passes 100 % ([`MAX_DEPTH`]). The
//! carrier rises and falls with a raised cosine of [`RISE_S`]; the microphone audio waits until the carrier is up, so no
//! speech is lost at key down.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Input `mic` is audio of any rate and channel count.
//! Output `out` is complex baseband at the session rate for `tx.sink`: the carrier at 0 Hz, the format centred on the tuned
//! frequency, the magnitude at most [`PEAK`] before the rate change and never above full scale after it. Output `keyed`
//! mutes the receive audio of the stack from key down to a short hold after the end mark. Each transmission ends with an
//! end mark: after ptt off, `halt`, the time-out, the transmitter going off, a new input format and the end of the input.
//! A setting change applies to the next transmission. On a live radio whose host gives a transmit lead
//! (`HostStreams::transmit_lead`), the first sample of each transmission states its time on air (`TagKind::BurstStart`).
//! Standard: ICAO Annex 10 Volume III Part II.

use crate::typed::{AirbandTxFields, HALT, PTT};
use sdrkit_algo_dsp::dynamics::{Leveller, SmoothLimiter};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Butterworth4, BUTTERWORTH4_Q};
use sdrkit_algo_sim::modulate::Am;
use sdrkit_components::tx::KeyedOut;
pub use sdrkit_components::voice::{PEAK, RATE, RISE_S};
use sdrkit_components::voice::{Content, KeyedSender, PttArgs, SendError, Speech, VoiceMeters};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published,
    Record, Results, Setting, SettingError, SettingId, SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Port, RunContext};

/// The RMS that the leveller aims at: that of a full-scale sine, so a steady tone has its peaks at the set depth, and the
/// peaks of speech, whose crest factor is higher, reach the limiter.
pub const LEVEL_RMS: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// The range of the leveller (dB either way).
const LEVEL_RANGE_DB: f32 = 12.0;
/// The look-ahead of the limiter.
const AHEAD_S: f64 = 0.002;
/// The release of the limiter.
const RELEASE_S: f64 = 0.05;
/// The largest depth applied, under 100 % by the rise of the peaks between the 48 kHz samples that the rate change brings
/// out (up to about 4 % for the highest audio that the level control can bring to full scale), so the carrier never pinches
/// off.
pub const MAX_DEPTH: f64 = 0.96;

/// The section of the transmit settings.
pub const TX: &str = "Transmit";

sdrkit_core::choice_enum! {
    /// The channel spacing, which sets the audio band.
    #[derive(Default)]
    pub enum Spacing {
        /// 8.33 kHz channels: audio 350 to 2500 Hz.
        #[default]
        Khz8_33 = ("8.33", "8.33 kHz"),
        /// 25 kHz channels: audio 300 to 3000 Hz.
        Khz25 = ("25", "25 kHz"),
    }
}

impl Spacing {
    /// The audio band (low edge, high edge, Hz) of the channel spacing.
    pub fn band(self) -> (f64, f64) {
        match self {
            Spacing::Khz8_33 => (350.0, 2500.0),
            Spacing::Khz25 => (300.0, 3000.0),
        }
    }
    /// The channel width (Hz).
    pub fn width(self) -> f64 {
        match self {
            Spacing::Khz8_33 => 25e3 / 3.0,
            Spacing::Khz25 => 25e3,
        }
    }
}

/// The channel spacing.
pub const TX_SPACING: Setting<field::Choice<Spacing>> =
    Setting::choice("tx_spacing", "Channel spacing", Spacing::Khz8_33)
        .help("The spacing of the channel sent on. It sets the audio band: 350 to 2500 Hz for 8.33 kHz, 300 to 3000 Hz for 25 kHz. A signal for 8.33 kHz also fits a 25 kHz channel.")
        .section(TX);
/// The peak modulation depth.
pub const TX_AM_DEPTH: Setting<field::F64> =
    Setting::new("tx_am_depth", "Modulation depth", "%", 85.0)
        .range(10.0, 100.0, 1.0)
        .help("The peak modulation depth. The level control holds the audio peaks at this depth; between the samples a peak can rise by up to 4 % of it. 100 % gives 96 %, so that the carrier never pinches off.")
        .section(TX);
/// The microphone gain.
pub const TX_MIC_GAIN: Setting<field::F64> =
    Setting::new("tx_mic_gain", "Microphone gain", "dB", 0.0)
        .range(-20.0, 20.0, 0.5)
        .help("The gain of the microphone audio before the level control. The level control adds up to 12 dB to quiet speech and takes loud speech down to the depth.")
        .section(TX);
/// The time-out of push-to-talk.
pub const TX_TIMEOUT: Setting<field::Duration> = Setting::new("tx_timeout", "Time-out", "s", 180.0)
    .range(1.0, 3600.0, 1.0)
    .help("Push-to-talk ends after this time on air.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the transmitter.
    pub enum TransmitSetting {
        /// The channel spacing.
        Spacing = TX_SPACING,
        /// The peak modulation depth.
        AmDepth = TX_AM_DEPTH,
        /// The microphone gain.
        MicGain = TX_MIC_GAIN,
        /// The time-out.
        Timeout = TX_TIMEOUT,
    }
}

sdrkit_core::commands! {
    /// The commands of the transmitter.
    pub enum TransmitCommand {
        /// Push-to-talk.
        Ptt = PTT.command,
        /// Ends the transmission.
        Halt = HALT,
    }
}

/// Why push-to-talk was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// The carrier of the last transmission is still falling.
    Busy,
    /// The stack has no input yet, so it has no rate and no frequency.
    NoInput,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TxError::Off => "the transmitter is off",
            TxError::Busy => "the last transmission is still ending",
            TxError::NoInput => "the stack has no input yet",
        })
    }
}

impl std::error::Error for TxError {}

/// The transmit settings as the transmitter reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxConfig {
    /// The channel spacing.
    pub spacing: Spacing,
    /// The peak modulation depth (%).
    pub depth_pct: f64,
    /// The microphone gain (dB).
    pub mic_gain_db: f64,
    /// The time-out (s).
    pub timeout_s: f64,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            spacing: TX_SPACING.default,
            depth_pct: TX_AM_DEPTH.default,
            mic_gain_db: TX_MIC_GAIN.default,
            timeout_s: TX_TIMEOUT.default,
        }
    }
}

/// The audio chain and the modulator at 48 kHz.
pub struct AudioChain {
    gain: f32,
    highpass: [Biquad; 2],
    lowpass: [Butterworth4; 2],
    level: Leveller,
    limit: SmoothLimiter<f32>,
    am: Am,
    carrier: f32,
    depth: f32,
    audio: Vec<f32>,
    mic_peak: f32,
    peak: f32,
}

impl AudioChain {
    /// The chain for the settings `c`.
    pub fn new(c: &TxConfig) -> Self {
        let (lo, hi) = c.spacing.band();
        let depth = (c.depth_pct / 100.0).clamp(0.0, MAX_DEPTH);
        AudioChain {
            gain: 10f32.powf(c.mic_gain_db as f32 / 20.0),
            highpass: BUTTERWORTH4_Q.map(|q| Biquad::new(BiquadCoeffs::highpass(RATE, lo, q))),
            lowpass: [
                Butterworth4::lowpass(RATE, hi),
                Butterworth4::lowpass(RATE, hi),
            ],
            level: Leveller::with_range(RATE, LEVEL_RMS, LEVEL_RANGE_DB),
            limit: SmoothLimiter::new(RATE, AHEAD_S, 1.0, RELEASE_S),
            am: Am::new(depth).unwrap_or_else(|_| Am::dsb_sc()),
            carrier: PEAK / (1.0 + depth as f32),
            depth: depth as f32,
            audio: Vec::new(),
            mic_peak: 0.0,
            peak: 0.0,
        }
    }
}

impl Speech for AudioChain {
    /// Modulates the microphone audio `mic` (48 kHz, full scale 1) into `out` (the same length), the audio delayed by the
    /// look-ahead of the limiter. The audio after the limiter stays within full scale, so each output sample has a magnitude
    /// of at most [`PEAK`].
    fn process(&mut self, mic: &[f32], out: &mut [Complex32]) {
        let n = mic.len().min(out.len());
        self.audio.clear();
        for &m in &mic[..n] {
            let m = if m.is_finite() { m } else { 0.0 };
            self.mic_peak = self.mic_peak.max(m.abs());
            let x = self.highpass[0].process_sample(m * self.gain);
            let x = self.highpass[1].process_sample(x);
            let x = self.lowpass[0].process_sample(x);
            let x = self.level.push(self.lowpass[1].process_sample(x));
            // The clamp holds the limit against a rounding of the last bit.
            let a = self.limit.push(x, x.abs()).clamp(-1.0, 1.0);
            self.peak = self.peak.max(a.abs());
            self.audio.push(a);
        }
        let out = &mut out[..n];
        let _ = self.am.process(&self.audio, out);
        out.iter_mut().for_each(|z| *z *= self.carrier);
    }

    /// The meters since the last call; `peak` is the peak modulation depth (0 to 1).
    fn take_meters(&mut self) -> VoiceMeters {
        VoiceMeters {
            mic_peak: std::mem::take(&mut self.mic_peak),
            peak: std::mem::take(&mut self.peak) * self.depth,
            level_db: self.level.gain_db(),
            limit_db: self.limit.take_reduction_db(),
        }
    }
}

/// The transmitter (`avdata.airband_transmit`). See the module.
pub struct AirbandTransmit {
    cfg: TxConfig,
    dial: f64,
    sender: KeyedSender,
    results: Published,
}

impl Default for AirbandTransmit {
    fn default() -> Self {
        let mut t = AirbandTransmit {
            cfg: TxConfig::default(),
            dial: 0.0,
            sender: KeyedSender::default().speech_after_rise(),
            results: Published::new(AirbandTxFields::SHAPE.fields),
        };
        t.publish();
        t
    }
}

impl AirbandTransmit {
    /// The transmit frequency: the tuned frequency.
    pub fn frequency(&self) -> f64 {
        self.dial
    }

    /// Whether a transmission is under way.
    pub fn transmitting(&self) -> bool {
        self.sender.transmitting()
    }

    /// Whether the receive audio is muted: a transmission is under way or its end mark is owed, or the hold after it runs.
    pub fn keyed(&self) -> bool {
        self.sender.keyed()
    }

    /// Starts push-to-talk; a transmission under way goes on.
    ///
    /// # Errors
    ///
    /// [`TxError`] when the transmitter is off or the stack has no input.
    pub fn start(&mut self) -> Result<(), TxError> {
        if self.sender.talking() {
            return Ok(());
        }
        let chain = Box::new(AudioChain::new(&self.cfg));
        self.sender
            .start(Content::Voice(chain), Some(self.cfg.timeout_s), None)
            .map_err(|e| match e {
                SendError::Off => TxError::Off,
                SendError::Busy => TxError::Busy,
                SendError::NoInput => TxError::NoInput,
            })
    }

    /// Ends the transmission with the fall of the carrier and the end mark.
    pub fn stop(&mut self) {
        self.sender.stop();
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let f = match self.sender.take_meters() {
            None => AirbandTxFields {
                transmitting: false,
                tx_freq_hz: None,
                tx_time_s: None,
                tx_mic_dbfs: None,
                tx_depth: None,
            },
            Some(m) => AirbandTxFields {
                transmitting: true,
                tx_freq_hz: Some(Hertz(self.dial)),
                tx_time_s: Some(Seconds(self.sender.time_s())),
                tx_mic_dbfs: Some(m.mic_dbfs()),
                tx_depth: Some(f64::from(100.0 * m.peak)),
            },
        };
        self.results.put(&f);
    }

    /// One pass: the samples of the transmission, the end mark and the keyed state.
    fn run(&mut self, io: &mut Io) {
        if self.sender.run(io) {
            self.publish();
        }
    }
}

impl Component for AirbandTransmit {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.airband_transmit",
            vec![
                Port::iq("in"),
                Port::audio("mic").optional("silence: push-to-talk sends the carrier alone"),
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
        Ok(vec![
            Format::iq(f.rate, self.dial, self.dial),
            KeyedOut::format(),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.sender.attach(ctx);
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        if self.sender.end(io) {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TransmitSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TransmitSetting::Spacing => self.cfg.spacing = TX_SPACING.read(id, v)?,
            TransmitSetting::AmDepth => self.cfg.depth_pct = TX_AM_DEPTH.read(id, v)?,
            TransmitSetting::MicGain => self.cfg.mic_gain_db = TX_MIC_GAIN.read(id, v)?,
            TransmitSetting::Timeout => self.cfg.timeout_s = TX_TIMEOUT.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        sdrkit_graph::values_of([
            (
                TransmitSetting::Spacing,
                Value::Choice(self.cfg.spacing.index()),
            ),
            (TransmitSetting::AmDepth, Value::F64(self.cfg.depth_pct)),
            (TransmitSetting::MicGain, Value::F64(self.cfg.mic_gain_db)),
            (
                TransmitSetting::Timeout,
                Value::Duration(self.cfg.timeout_s),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        TransmitCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TransmitCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TransmitCommand::Ptt => {
                let PttArgs { on } = PTT.args(args)?;
                if on {
                    self.start().map_err(|e| CommandError::Failed {
                        source: StageError::new(e),
                    })?;
                } else {
                    self.stop();
                }
            }
            TransmitCommand::Halt => self.stop(),
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

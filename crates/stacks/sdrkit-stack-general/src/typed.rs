//! The interface of the general family: the result sets of the report and of the AM processor, the rows of the presets
//! and of the extra channels, the whistle series, the command that applies a preset, and what the stack declares.

use crate::amproc::InUse;
use crate::settings::GeneralSetting;
use crate::squelch::SquelchKind;
use crate::types::Demod;
use sdrkit_components::voice::PttArgs;
use sdrkit_core::interface::{
    Arguments, CommandKind, Family, Fields, Hertz, Seconds, StackShape, Table,
};
use sdrkit_core::publish::{keys, SeriesDef};
use sdrkit_core::{Command, SettingSet, TxContent};
use sdrkit_proto_tones::components as tones;
use std::sync::OnceLock;

/// The id of the stack.
pub const ID: &str = "general";

/// The result fields of the AM processor. The measurements have no value while the processor does not run.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AmFields<'a> {
    /// Whether the AM processor runs.
    #[field(label = "AM processor")]
    pub am_active: bool,
    /// Whether it found a carrier.
    #[field(label = "AM carrier found")]
    pub am_carrier_found: bool,
    /// Whether its loop is locked.
    #[field(label = "AM locked")]
    pub am_locked: bool,
    /// The carrier relative to the channel. No value without a carrier.
    #[field(label = "AM carrier offset", unit = "Hz")]
    pub am_carrier_offset_hz: Option<Hertz>,
    /// The carrier level.
    #[field(label = "AM carrier", unit = "dBFS")]
    pub am_carrier_dbfs: Option<f64>,
    /// The programme SNR.
    #[field(label = "AM programme SNR", unit = "dB")]
    pub am_programme_snr_db: Option<f64>,
    /// The detector in use.
    #[field(label = "AM detector")]
    pub am_detector: Option<InUse>,
    /// Whether one sideband was chosen because of interference.
    #[field(label = "AM one sideband")]
    pub am_one_side: bool,
    /// The interference measure above the carrier.
    #[field(label = "Interference above")]
    pub am_interference_upper: Option<f64>,
    /// The interference measure below the carrier.
    #[field(label = "Interference below")]
    pub am_interference_lower: Option<f64>,
    /// The audio bandwidth in use.
    #[field(label = "AM audio bandwidth", unit = "Hz")]
    pub am_bandwidth_hz: Option<Hertz>,
    /// The noise reduction floor. No value with the noise reduction off.
    #[field(label = "AM noise floor", unit = "dB")]
    pub am_nr_floor_db: Option<f64>,
    /// The share of samples the AM blanker removed.
    #[field(label = "AM blanked", unit = "%")]
    pub am_blanked_pct: Option<f64>,
    /// The threshold of the AM blanker.
    #[field(label = "AM blanker threshold", unit = "dB")]
    pub am_blanker_db: Option<f64>,
    /// The hum notched (50 or 60 Hz). No value for none.
    #[field(label = "Hum notched", unit = "Hz")]
    pub am_hum_hz: Option<Hertz>,
    /// Whether the loop holds through a fade.
    #[field(label = "Fading")]
    pub am_fading: bool,
    /// Whether the synthetic treble is on.
    #[field(label = "Synthetic treble")]
    pub am_enhanced: bool,
    /// The gain of the leveller.
    #[field(label = "Leveller gain", unit = "dB")]
    pub am_leveller_db: Option<f64>,
    /// The state of the AM processor in words.
    #[field(label = "AM status")]
    pub am_status: Option<&'a str>,
}

/// The result fields of the report.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GeneralFields<'a> {
    /// The demodulator.
    #[field(label = "Demodulator")]
    pub demod: Demod,
    /// The filter width.
    #[field(label = "Filter width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Hertz,
    /// The demodulation rate (48 kHz, WFM 240 kHz).
    #[field(label = "Demodulation rate", unit = "Hz")]
    pub demod_rate_hz: Hertz,
    /// The channel relative to the dial.
    #[field(label = "Channel offset", unit = "Hz")]
    pub channel_offset_hz: Hertz,
    /// The dial frequency.
    #[field(label = "Dial", unit = "Hz")]
    pub dial_hz: Hertz,
    /// The listening frequency: the dial plus the channel offset.
    #[field(label = "Listening frequency", unit = "Hz")]
    pub listening_hz: Hertz,
    /// The low edge of the passband relative to the channel.
    #[field(label = "Filter low edge", unit = "Hz")]
    pub filter_lo_hz: Hertz,
    /// The high edge of the passband relative to the channel.
    #[field(label = "Filter high edge", unit = "Hz")]
    pub filter_hi_hz: Hertz,
    /// The low edge of the passband.
    #[field(label = "Passband low", unit = "Hz")]
    pub passband_lo_hz: Hertz,
    /// The high edge of the passband.
    #[field(label = "Passband high", unit = "Hz")]
    pub passband_hi_hz: Hertz,
    /// The level in the channel. No value before the first squelch block.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The noise in the channel. No value while the noise is not measured.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
    /// The SNR. No value while the noise is not measured.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The squelch threshold. No value with the squelch off or the noise not measured.
    #[field(label = "Squelch threshold", unit = "dBFS")]
    pub squelch_threshold_dbfs: Option<f64>,
    /// Whether the squelch is open.
    #[field(label = "Squelch open")]
    pub squelch_open: bool,
    /// The frequency offset of an FM signal. No value with the squelch closed or another demodulator.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The power of the audio played over the last report. No value without audio.
    #[field(label = "Audio", unit = "dBFS")]
    pub audio_dbfs: Option<f64>,
    /// 10 ms blocks with the squelch open. No value before the first squelch block.
    #[field(label = "Blocks open")]
    pub blocks_open: Option<i64>,
    /// 10 ms blocks with the squelch closed. No value before the first squelch block.
    #[field(label = "Blocks closed")]
    pub blocks_closed: Option<i64>,
    /// The share of samples the blanker removed.
    #[field(label = "Blanked", unit = "%")]
    pub blanked_pct: f64,
    /// The speech probability of the voice noise remover for the last 10 ms (0 to 1).
    #[field(label = "Speech probability")]
    pub speech_probability: f64,
    /// The position of the needle of the S-meter (0 to 1). No value without a level.
    #[field(label = "S-meter needle")]
    pub needle: Option<f64>,
    /// The CTCSS tone found.
    #[field(label = "CTCSS tone", unit = "Hz")]
    pub ctcss_hz: Option<Hertz>,
    /// The measured frequency of the CTCSS tone.
    #[field(label = "CTCSS measured", unit = "Hz")]
    pub ctcss_measured_hz: Option<Hertz>,
    /// The level of the CTCSS tone against 5 kHz deviation.
    #[field(label = "CTCSS level", unit = "dB")]
    pub ctcss_db: Option<f64>,
    /// The DCS code found ("D023N").
    #[field(label = "DCS code")]
    pub dcs: Option<&'a str>,
    /// Another standard code that the same waveform reads as.
    #[field(label = "DCS alias")]
    pub dcs_alias: Option<&'a str>,
    /// The CTCSS tone or DCS code heard, as a label for lists of things heard ("CTCSS 88.5 Hz", "DCS D023N").
    #[field(label = "Tone")]
    pub tone: Option<&'a str>,
    /// Whether the tone gate lets the audio through.
    #[field(label = "Tone gate open")]
    pub tone_gate_open: bool,
}

/// A row of the width presets. The row key is the place of the preset ("00", "01" ...).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WidthPresetRow {
    /// The width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
}

/// A row of the built-in presets. The row key is the key of the preset.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PresetRow<'a> {
    /// The name of the preset.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The frequency of the preset.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Hertz,
    /// The demodulator of the preset.
    #[field(label = "Demodulator")]
    pub demod: Demod,
    /// The filter width of the preset.
    #[field(label = "Filter", unit = "Hz")]
    pub width: Hertz,
    /// The squelch of the preset.
    #[field(label = "Squelch")]
    pub squelch: SquelchKind,
}

/// A row of the extra channels. The row key is the number of the channel ("1" to "3").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ChannelRow<'a> {
    /// The name of the channel.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The listening frequency of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// The demodulator of the channel.
    #[field(label = "Demodulator")]
    pub demod: Option<Demod>,
    /// Whether the squelch of the channel is open.
    #[field(label = "Squelch open")]
    pub squelch_open: Option<bool>,
    /// The summary line of the channel.
    #[field(label = "State")]
    pub summary: &'a str,
}

/// The result fields of the listening chain (`general.listening`). No stack of the family shows them.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ListeningFields {
    /// Whether the chain changes the sound.
    #[field(label = "Active")]
    pub active: bool,
    /// Whether the squelch of the chain is open.
    #[field(label = "Squelch open")]
    pub squelch: bool,
    /// The peak level of the last 0.1 s.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak: f64,
    /// The RMS level of the last 0.1 s.
    #[field(label = "RMS", unit = "dBFS")]
    pub rms: f64,
    /// Whether the sound clipped in the last 0.1 s.
    #[field(label = "Clip")]
    pub clip: bool,
    /// The gain of the AGC.
    #[field(label = "AGC gain", unit = "dB")]
    pub agc_db: f64,
    /// The frequency of the automatic notch. No value while it holds none.
    #[field(label = "Automatic notch", unit = "Hz")]
    pub auto_notch: Option<Hertz>,
}

sdrkit_core::choice_enum! {
    /// What a transmission sends.
    pub enum TransmitKind {
        /// Speech from the microphone input (push-to-talk).
        Voice = ("voice", "Voice"),
        /// Morse code of a text.
        Text = ("text", "Text"),
        /// An unmodulated carrier (tune).
        Carrier = ("carrier", "Carrier"),
    }
}

/// The result fields of the transmitter. The measurements have no value while the stack does not transmit.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TransmitFields {
    /// Whether the stack transmits now. The receive audio is muted meanwhile.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the listening frequency plus the transmit offset.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// What the transmission sends.
    #[field(label = "Sending")]
    pub tx_kind: Option<TransmitKind>,
    /// The time on air of the transmission.
    #[field(label = "Time on air", unit = "s")]
    pub tx_time_s: Option<Seconds>,
    /// The peak of the microphone audio over the last report. No value without voice.
    #[field(label = "Microphone peak", unit = "dBFS")]
    pub tx_mic_dbfs: Option<f64>,
    /// The peak of the modulation over the last report, against full modulation (deviation, depth or full scale).
    #[field(label = "Modulation peak", unit = "dB")]
    pub tx_peak_db: Option<f64>,
    /// The gain of the level control of the speech processor.
    #[field(label = "Level gain", unit = "dB")]
    pub tx_level_db: Option<f64>,
    /// The most gain reduction of the limiter and the compressor over the last report: 0 when they did not act.
    #[field(label = "Limiter", unit = "dB")]
    pub tx_limit_db: Option<f64>,
}

/// The arguments of `send`.
#[derive(Arguments)]
pub struct SendArgs<'a> {
    /// The text to key.
    #[arg(
        label = "Text",
        help = "Letters, digits, punctuation and <XX> procedure signals."
    )]
    pub text: &'a str,
}

/// Push-to-talk: speech from the microphone input in NFM, AM, USB or LSB at the listening frequency plus the transmit offset.
pub const PTT: CommandKind<PttArgs> = sdrkit_components::voice::ptt(
    "With on true, sends the speech of the microphone input in the mode of the demodulator (NFM, AM, USB or LSB) until ptt with on false, halt or the time-out.",
);
/// Keys a text in Morse code (CW).
pub const SEND: CommandKind<SendArgs<'static>> = CommandKind::new("send", "Send Morse")
    .help("Keys the text in Morse code at the CW speed, with the demodulator on CW, the carrier at the listening frequency plus the transmit offset.")
    .transmits(TxContent::Text)
    .stopped_by("halt");
/// Sends an unmodulated carrier for the tune time.
pub const TUNE: Command = Command::new("tune", "Tune")
    .help("Sends an unmodulated carrier at the listening frequency plus the transmit offset for the tune time.")
    .transmits(TxContent::Carrier)
    .stopped_by("halt");
/// Ends every transmission at once.
pub const HALT: Command =
    Command::new("halt", "Halt").help("Ends speech, Morse code or the tune carrier at once.");

/// The width presets of the demodulator in use.
pub static WIDTH_PRESETS: Table<WidthPresetRow> = Table::new("width_presets", "Width presets", 32);
/// The built-in presets. The command `preset.apply` applies one.
pub static PRESETS: Table<PresetRow<'static>> =
    Table::new("presets", "Presets", 64).label(&PresetRow::NAME);
/// The extra channels, while any is on.
pub static CHANNELS: Table<ChannelRow<'static>> = Table::new("channels", "Extra channels", 4);

/// The whistles that the AM processor notched: (number, frequency).
pub const WHISTLES: SeriesDef = SeriesDef {
    key: "whistles",
    label: "Whistles notched (number, frequency)",
    x_unit: "",
    y_unit: "Hz",
};

/// The arguments of `preset.apply`.
#[derive(Arguments)]
pub struct PresetApplyArgs<'a> {
    /// The row key of a preset.
    #[arg(
        label = "Preset",
        help = "The row key of a preset in the presets table."
    )]
    pub row: &'a str,
}

/// Applies a built-in preset: its demodulator, width and squelch, then a retune to its frequency.
pub const PRESET_APPLY: CommandKind<PresetApplyArgs<'static>> =
    CommandKind::new("preset.apply", "Apply preset").help(
        "Sets the demodulator, the filter width and the squelch of a preset, and tunes to its frequency.",
    );

/// What version 1.1 added: the transmit branch.
const ADDED: &[&str] = &[
    "shape.TransmitFields",
    "shape.TransmitFields.transmitting",
    "shape.TransmitFields.tx_freq_hz",
    "shape.TransmitFields.tx_kind",
    "shape.TransmitFields.tx_time_s",
    "shape.TransmitFields.tx_mic_dbfs",
    "shape.TransmitFields.tx_peak_db",
    "shape.TransmitFields.tx_level_db",
    "shape.TransmitFields.tx_limit_db",
    "general.fields.TransmitFields",
    "general.setting.tx_split",
    "general.setting.tx_deviation",
    "general.setting.tx_preemphasis",
    "general.setting.tx_ctcss",
    "general.setting.tx_am_depth",
    "general.setting.tx_ssb_method",
    "general.setting.tx_ssb_band",
    "general.setting.tx_compression",
    "general.setting.tx_timeout",
    "general.setting.tx_cw_wpm",
    "general.setting.tx_cw_farnsworth",
    "general.setting.tx_tune_limit",
    "general.command.ptt",
    "general.command.send",
    "general.command.tune",
    "general.command.halt",
];

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // The presets and the width presets hold no measurement: they stay over a change of frequency.
        Family::new("general", 1, 3)
            .added(1, 3, &["general.content"])
            .content(sdrkit_core::ContentKind::General, &["general"])
            .audio(sdrkit_core::AudioKind::Monitor, &["general"])
            .uses(tones::interface())
            .stack(
                StackShape::new(ID)
                    .fields::<AmFields>()
                    .fields::<GeneralFields>()
                    .fields::<TransmitFields>()
                    .table(&WIDTH_PRESETS)
                    .table(&PRESETS)
                    .table(&CHANNELS)
                    .series(WHISTLES)
                    .event(tones::subaudio())
                    .event(tones::dtmf())
                    .settings(GeneralSetting::SETTINGS)
                    .command(&PRESET_APPLY)
                    .command(&PTT)
                    .command(&SEND)
                    .command(&TUNE)
                    .command(&HALT)
                    .kept(&["width_presets", "presets"]),
            )
            .added(1, 1, ADDED)
            .added(1, 2, &["general.setting.gen_audio"])
    })
}

/// The description of the general receiver.
pub fn shape() -> &'static StackShape {
    family()
        .stack_shape(ID)
        .expect("the general family describes the general stack")
}

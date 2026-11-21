//! The settings of the general receiver, the memory of each demodulator, and the options of the test signal.

use crate::amproc::{AmDetector, AmNoise};
use crate::channel::parse_taps;
use crate::presets::Preset;
use crate::squelch::SquelchKind;
use crate::toneid::{parse_gate, ToneGate};
use crate::transmit::settings::{self as tx, TxSettings};
use crate::types::{Demod, NfmLowPass};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, Setting, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::values_of;

const RX: &str = "Receive";
const SQ: &str = "Squelch";
const GAIN_SECTION: &str = "Gain";
const AM: &str = "AM";
const AUDIO: &str = "Audio";
const CH: &str = "Extra channels";
const GEN: &str = "Test signal";

sdrkit_core::choice_enum! {
    /// The de-emphasis of FM audio.
    pub enum Deemphasis {
        /// None.
        Off = ("off", "Off"),
        /// 50 µs.
        Us50 = ("50", "50 µs"),
        /// 75 µs.
        Us75 = ("75", "75 µs"),
        /// 750 µs.
        Us750 = ("750", "750 µs"),
    }
}

impl Deemphasis {
    /// The time constant (µs, 0 for none).
    pub fn micros(self) -> f64 {
        match self {
            Deemphasis::Off => 0.0,
            Deemphasis::Us50 => 50.0,
            Deemphasis::Us75 => 75.0,
            Deemphasis::Us750 => 750.0,
        }
    }
}

/// The demodulator.
pub const DEMOD: Setting<field::Choice<Demod>> =
    Setting::choice("demod", "Demodulator", Demod::Nfm)
        .help("NFM, WFM (mono), AM, USB, LSB or CW.")
        .section(RX);
/// The filter widths that people use with each demodulator.
const WIDTHS: sdrkit_core::CommonSets = &[
    ("nfm", &[8000.0, 12_500.0, 25_000.0]),
    ("wfm", &[180_000.0, 200_000.0]),
    ("am", &[6000.0, 8000.0, 9000.0, 10_000.0]),
    ("usb", &[1800.0, 2400.0, 2700.0, 3000.0]),
    ("lsb", &[1800.0, 2400.0, 2700.0, 3000.0]),
    ("cw", &[50.0, 100.0, 250.0, 500.0]),
];
/// The width of the channel filter.
pub const WIDTH: Setting<field::Hz> = Setting::new("width", "Filter", "Hz", 0.0)
    .range(0.0, 220_000.0, 10.0)
    .zero_means("The stored width of the demodulator")
    .common_for("demod", WIDTHS)
    .help("The width of the channel filter. Each demodulator keeps its own. A width outside the range of the demodulator is brought into that range.")
    .section(RX);
/// The offset of the listening channel from the dial.
pub const CHANNEL_OFFSET: Setting<field::Hz> =
    Setting::new("channel_offset", "Channel offset", "Hz", 0.0)
        .range(-1e8, 1e8, 1.0)
        .help("Where the listening channel sits relative to the dial, inside the capture.")
        .section(RX);
/// The tuning step.
pub const STEP: Setting<field::Hz> = Setting::new("step", "Step", "Hz", 12_500.0)
    .range(1.0, 1e8, 0.0)
    .common_for(
        "demod",
        &[
            (
                "nfm",
                &[1000.0, 2500.0, 5000.0, 6250.0, 10_000.0, 12_500.0, 20_000.0, 25_000.0],
            ),
            ("wfm", &[10_000.0, 50_000.0, 100_000.0]),
            ("am", &[100.0, 500.0, 1000.0, 5000.0, 9000.0, 10_000.0]),
            ("usb", &[10.0, 50.0, 100.0, 500.0, 1000.0]),
            ("lsb", &[10.0, 50.0, 100.0, 500.0, 1000.0]),
            ("cw", &[10.0, 50.0, 100.0, 500.0, 1000.0]),
        ],
    )
    .help("The tuning step. Each demodulator keeps its own.")
    .section(RX);
/// Custom prototype taps of the channel filter.
pub const CUSTOM_FILTER: Setting<field::Text> = Setting::new("custom_filter", "Custom baseband filter", "", "")
    .help("Low-pass prototype taps, separated by commas, in place of the built-in channel filter. Empty for the built-in one.")
    .section(RX);
/// The squelch.
pub const SQUELCH: Setting<field::Choice<SquelchKind>> =
    Setting::choice("squelch", "Squelch", SquelchKind::Auto)
        .help("Auto opens 6 dB above the measured noise. Level opens at a level in dBFS.")
        .section(SQ);
/// The threshold of the level squelch.
pub const SQUELCH_LEVEL: Setting<field::F64> =
    Setting::new("squelch_level", "Squelch level", "dBFS", -60.0)
        .range(-140.0, 20.0, 1.0)
        .section(SQ);
/// The AGC.
pub const AGC: Setting<field::Bool> = Setting::new("agc", "AGC", "", true)
    .help("AM follows the carrier. SSB and CW follow the envelope.")
    .section(GAIN_SECTION);
/// The gain when the AGC is off.
pub const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "dB", 20.0)
    .range(-20.0, 80.0, 1.0)
    .help("Used when the AGC is off.")
    .section(GAIN_SECTION);
/// The de-emphasis of NFM.
pub const DEEMPH_NFM: Setting<field::Choice<Deemphasis>> =
    Setting::choice("deemph_nfm", "De-emphasis (NFM)", Deemphasis::Off).section(AUDIO);
/// The de-emphasis of WFM.
pub const DEEMPH_WFM: Setting<field::Choice<Deemphasis>> =
    Setting::choice("deemph_wfm", "De-emphasis (WFM)", Deemphasis::Us50).section(AUDIO);
/// The audio low-pass of NFM.
pub const NFM_LOWPASS: Setting<field::Choice<NfmLowPass>> = Setting::choice(
    "nfm_lowpass",
    "Audio low-pass (NFM)",
    NfmLowPass::Khz3,
)
.help(
    "3 kHz has about 3.7 dB less noise and keeps speech. 4 kHz passes more treble and more noise.",
)
.section(AUDIO);
/// The CW pitch.
pub const CW_PITCH: Setting<field::Hz> = Setting::new("cw_pitch", "Pitch", "Hz", 700.0)
    .range(200.0, 1500.0, 10.0)
    .common(&[400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0])
    .section(AUDIO);
/// Auto quality of AM.
pub const AM_AUTO: Setting<field::Bool> = Setting::new("am_auto", "Auto quality", "", true)
    .help("Locks to the carrier, picks the cleaner sideband, removes whistles, hum and clicks, and levels the sound.")
    .section(AM);
/// The AM detector.
pub const AM_DETECTOR: Setting<field::Choice<AmDetector>> =
    Setting::choice("am_detector", "Detector", AmDetector::Auto).section(AM);
/// The AM noise reduction.
pub const AM_NR: Setting<field::Choice<AmNoise>> =
    Setting::choice("am_nr", "Noise", AmNoise::Auto).section(AM);
/// The AM audio bandwidth.
pub const AM_BANDWIDTH: Setting<field::Hz> =
    Setting::new("am_bandwidth", "Audio bandwidth", "Hz", 0.0)
        .range(0.0, 10_000.0, 500.0)
        .zero_means("Automatic")
        .help("A width is 2500 to 10 000 Hz.")
        .section(AM);
/// Synthetic treble of AM.
pub const AM_ENHANCE: Setting<field::Bool> =
    Setting::new("am_enhance", "Enhance (synthetic treble)", "", false).section(AM);
/// The noise blanker.
pub const NB: Setting<field::Bool> = Setting::new("nb", "Blanker", "", false)
    .help("Removes clicks and ignition noise. Not offered for WFM.")
    .section(GAIN_SECTION);
/// The threshold of the noise blanker.
pub const NB_THRESHOLD: Setting<field::F64> =
    Setting::new("nb_threshold", "Blanker threshold", "dB", 12.0)
        .range(6.0, 30.0, 1.0)
        .section(GAIN_SECTION);
/// The audio high-pass.
pub const AF_HIGHPASS: Setting<field::Hz> = Setting::new("af_highpass", "High-pass", "Hz", 0.0)
    .range(0.0, 23_900.0, 10.0)
    .zero_means("Off")
    .section(AUDIO);
/// The audio low-pass.
pub const AF_LOWPASS: Setting<field::Hz> = Setting::new("af_lowpass", "Low-pass", "Hz", 0.0)
    .range(0.0, 23_900.0, 10.0)
    .zero_means("Off")
    .section(AUDIO);
/// The audio notch.
pub const AF_NOTCH: Setting<field::Hz> = Setting::new("af_notch", "Notch", "Hz", 0.0)
    .range(0.0, 23_900.0, 10.0)
    .zero_means("Off")
    .section(AUDIO);
/// The neural voice noise remover.
pub const VOICE_NR: Setting<field::Bool> =
    Setting::new("voice_nr", "Noise remover (neural)", "", false)
        .help("For NFM, WFM and AM speech. Each demodulator keeps its own.")
        .section(AUDIO);
/// The strength of the voice noise remover.
pub const VOICE_NR_STRENGTH: Setting<field::F64> =
    Setting::new("voice_nr_strength", "Strength", "%", 80.0)
        .range(0.0, 100.0, 1.0)
        .section(AUDIO);
/// The presets that the `presets` table lists and `preset.apply` applies.
pub const PRESETS: Setting<field::Text> = Setting::new("presets", "Presets", "", "")
    .help("Entries separated by \";\", each a key, \"=\", then the frequency in Hz, the demodulator, the filter width in Hz and the name separated by \",\", for example \"cw=7030000,cw,500,CW\". At most 64 entries. The host fills it from a preset file. Empty: no presets.")
    .section(RX);
/// The tone gate.
pub const TONE_GATE: Setting<field::Text> = Setting::new("tone_gate", "Tone gate", "", "open")
    .help("open, any, tone:<Hz> or code:<023N>.")
    .section(SQ);
/// The channel heard.
pub const LISTEN_CHANNEL: Setting<field::I64> =
    Setting::new("listen_channel", "Heard channel", "", 0)
        .range(0.0, 3.0, 1.0)
        .help("0 for the main channel, 1 to 3 for an extra channel.")
        .section(CH);

macro_rules! extra_settings {
    ($on:ident, $offset:ident, $demod:ident, $width:ident, $n:literal, $default_offset:expr) => {
        /// An extra channel is on.
        pub const $on: Setting<field::Bool> = Setting::new(
            concat!("extra", $n),
            concat!("Extra channel ", $n),
            "",
            false,
        )
        .section(CH);
        /// The offset of an extra channel from the dial.
        pub const $offset: Setting<field::Hz> = Setting::new(
            concat!("extra", $n, "_offset"),
            concat!("Extra ", $n, " offset"),
            "Hz",
            $default_offset,
        )
        .range(-1e8, 1e8, 1.0)
        .section(CH);
        /// The demodulator of an extra channel.
        pub const $demod: Setting<field::Choice<Demod>> = Setting::choice(
            concat!("extra", $n, "_demod"),
            concat!("Extra ", $n, " demodulator"),
            Demod::Nfm,
        )
        .section(CH);
        /// The filter width of an extra channel (0 for the default of its demodulator).
        pub const $width: Setting<field::Hz> = Setting::new(
            concat!("extra", $n, "_width"),
            concat!("Extra ", $n, " filter"),
            "Hz",
            0.0,
        )
        .range(0.0, 220_000.0, 10.0)
        .common_for(concat!("extra", $n, "_demod"), WIDTHS)
        .section(CH);
    };
}

extra_settings!(
    EXTRA1,
    EXTRA1_OFFSET,
    EXTRA1_DEMOD,
    EXTRA1_WIDTH,
    "1",
    100e3
);
extra_settings!(
    EXTRA2,
    EXTRA2_OFFSET,
    EXTRA2_DEMOD,
    EXTRA2_WIDTH,
    "2",
    -100e3
);
extra_settings!(
    EXTRA3,
    EXTRA3_OFFSET,
    EXTRA3_DEMOD,
    EXTRA3_WIDTH,
    "3",
    200e3
);

/// No station in the test signal.
pub const GEN_NOISE_ONLY: Setting<field::Bool> =
    Setting::new("gen_noise_only", "Noise only", "", false)
        .help("No station: only the noise of the test signal settings.")
        .section(GEN)
        .advanced();
/// The tune offset of the test signal.
pub const GEN_TUNE_OFFSET: Setting<field::Hz> =
    Setting::new("gen_tune_offset", "Tune offset", "Hz", 250e3)
        .range(-1e7, 1e7, 1.0)
        .section(GEN)
        .advanced();
/// The modulating tone of the test signal.
pub const GEN_TONE: Setting<field::Hz> = Setting::new("gen_tone", "Tone", "Hz", 1000.0)
    .range(1.0, 20_000.0, 1.0)
    .section(GEN)
    .advanced();
/// The modulation depth of the test signal.
pub const GEN_MOD: Setting<field::F64> = Setting::new("gen_mod", "Modulation", "", 0.5)
    .range(0.0, 1.0, 0.01)
    .section(GEN)
    .advanced();
/// The CTCSS tone of the test signal (0 for none).
pub const GEN_CTCSS: Setting<field::Hz> = Setting::new("gen_ctcss", "CTCSS tone", "Hz", 0.0)
    .range(0.0, 300.0, 0.1)
    .section(GEN)
    .advanced();
/// The deviation of the CTCSS tone of the test signal.
pub const GEN_CTCSS_DEV: Setting<field::Hz> =
    Setting::new("gen_ctcss_dev", "CTCSS deviation", "Hz", 600.0)
        .range(0.0, 2000.0, 1.0)
        .section(GEN)
        .advanced();
/// The DCS code of the test signal.
pub const GEN_DCS: Setting<field::Text> = Setting::new("gen_dcs", "DCS code", "", "")
    .help("Three octal digits, with N or I for the polarity (023N). Empty for none.")
    .section(GEN)
    .advanced();

sdrkit_core::choice_enum! {
    /// What modulates the test signal.
    pub enum GenAudio {
        /// The steady tone of the setting Tone.
        Tone = ("tone", "Tone"),
        /// Speech-like noise of the voice band in syllables with pauses.
        Voice = ("voice", "Voice"),
    }
}

/// What modulates the test signal.
pub const GEN_AUDIO: Setting<field::Choice<GenAudio>> =
    Setting::choice("gen_audio", "Sound", GenAudio::Tone)
        .help("Tone: the steady tone of the setting Tone. Voice: noise of the voice band in syllables of 4 Hz with pauses as long, at the modulation of the setting Modulation, which fills the channel as speech does. Voice applies to FM and AM; SSB and CW keep the tone.")
        .section(GEN)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the general receiver.
    pub enum GeneralSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// The filter width.
        Width = WIDTH,
        /// The channel offset.
        ChannelOffset = CHANNEL_OFFSET,
        /// The tuning step.
        Step = STEP,
        /// Custom filter taps.
        CustomFilter = CUSTOM_FILTER,
        /// The squelch.
        Squelch = SQUELCH,
        /// The squelch level.
        SquelchLevel = SQUELCH_LEVEL,
        /// The AGC.
        Agc = AGC,
        /// The gain.
        Gain = GAIN,
        /// The NFM de-emphasis.
        DeemphNfm = DEEMPH_NFM,
        /// The WFM de-emphasis.
        DeemphWfm = DEEMPH_WFM,
        /// The audio low-pass of NFM.
        NfmLowpass = NFM_LOWPASS,
        /// The CW pitch.
        CwPitch = CW_PITCH,
        /// Auto quality of AM.
        AmAuto = AM_AUTO,
        /// The AM detector.
        AmDetector = AM_DETECTOR,
        /// The AM noise reduction.
        AmNr = AM_NR,
        /// The AM audio bandwidth.
        AmBandwidth = AM_BANDWIDTH,
        /// Synthetic treble of AM.
        AmEnhance = AM_ENHANCE,
        /// The noise blanker.
        Nb = NB,
        /// The blanker threshold.
        NbThreshold = NB_THRESHOLD,
        /// The audio high-pass.
        AfHighpass = AF_HIGHPASS,
        /// The audio low-pass.
        AfLowpass = AF_LOWPASS,
        /// The audio notch.
        AfNotch = AF_NOTCH,
        /// The voice noise remover.
        VoiceNr = VOICE_NR,
        /// Its strength.
        VoiceNrStrength = VOICE_NR_STRENGTH,
        /// The presets.
        Presets = PRESETS,
        /// The tone gate.
        ToneGate = TONE_GATE,
        /// The channel heard.
        ListenChannel = LISTEN_CHANNEL,
        /// Extra channel 1 on.
        Extra1 = EXTRA1,
        /// Its offset.
        Extra1Offset = EXTRA1_OFFSET,
        /// Its demodulator.
        Extra1Demod = EXTRA1_DEMOD,
        /// Its width.
        Extra1Width = EXTRA1_WIDTH,
        /// Extra channel 2 on.
        Extra2 = EXTRA2,
        /// Its offset.
        Extra2Offset = EXTRA2_OFFSET,
        /// Its demodulator.
        Extra2Demod = EXTRA2_DEMOD,
        /// Its width.
        Extra2Width = EXTRA2_WIDTH,
        /// Extra channel 3 on.
        Extra3 = EXTRA3,
        /// Its offset.
        Extra3Offset = EXTRA3_OFFSET,
        /// Its demodulator.
        Extra3Demod = EXTRA3_DEMOD,
        /// Its width.
        Extra3Width = EXTRA3_WIDTH,
        /// The transmit offset.
        TxSplit = tx::TX_SPLIT,
        /// The NFM deviation.
        TxDeviation = tx::TX_DEVIATION,
        /// The NFM pre-emphasis.
        TxPreemphasis = tx::TX_PREEMPHASIS,
        /// The CTCSS tone.
        TxCtcss = tx::TX_CTCSS,
        /// The AM depth limit.
        TxAmDepth = tx::TX_AM_DEPTH,
        /// The SSB method.
        TxSsbMethod = tx::TX_SSB_METHOD,
        /// The SSB audio band.
        TxSsbBand = tx::TX_SSB_BAND,
        /// The SSB compression.
        TxCompression = tx::TX_COMPRESSION,
        /// The push-to-talk time-out.
        TxTimeout = tx::TX_TIMEOUT,
        /// The CW speed.
        TxCwWpm = tx::TX_CW_WPM,
        /// The CW overall speed.
        TxCwFarnsworth = tx::TX_CW_FARNSWORTH,
        /// The tune time.
        TxTuneLimit = tx::TX_TUNE_LIMIT,
        /// No station in the test signal.
        GenNoiseOnly = GEN_NOISE_ONLY,
        /// The tune offset of the test signal.
        GenTuneOffset = GEN_TUNE_OFFSET,
        /// The tone of the test signal.
        GenTone = GEN_TONE,
        /// The modulation of the test signal.
        GenMod = GEN_MOD,
        /// The CTCSS tone of the test signal.
        GenCtcss = GEN_CTCSS,
        /// The CTCSS deviation of the test signal.
        GenCtcssDev = GEN_CTCSS_DEV,
        /// The DCS code of the test signal.
        GenDcs = GEN_DCS,
        /// What modulates the test signal.
        GenAudio = GEN_AUDIO,
    }
}

/// The memory of each demodulator, by [`Demod::slot`].
#[derive(Clone, Debug, PartialEq)]
pub struct Memory {
    /// The width (Hz).
    pub width: [f64; 6],
    /// The step (Hz).
    pub step: [f64; 6],
    /// The voice noise remover is on.
    pub voice_nr: [bool; 6],
    /// Its strength (%).
    pub voice_nr_strength: [f64; 6],
}

impl Default for Memory {
    fn default() -> Self {
        Memory {
            width: Demod::ALL.map(|d| d.default_width()),
            step: Demod::ALL.map(|d| d.default_step()),
            voice_nr: [false; 6],
            voice_nr_strength: [80.0; 6],
        }
    }
}

/// One extra channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extra {
    /// The channel runs.
    pub on: bool,
    /// Its offset from the dial (Hz).
    pub offset_hz: f64,
    /// Its demodulator.
    pub demod: Demod,
    /// Its width (Hz, 0 for the default of its demodulator).
    pub width_hz: f64,
}

/// The settings of the general receiver, read.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralSettings {
    /// The demodulator.
    pub demod: Demod,
    /// The memory of each demodulator.
    pub mem: Memory,
    /// The channel offset (Hz).
    pub channel_offset: f64,
    /// Custom prototype taps, as typed.
    pub custom_filter: String,
    /// Custom prototype taps (empty for the built-in filter).
    pub custom_taps: Vec<f32>,
    /// The squelch kind.
    pub squelch: SquelchKind,
    /// The level threshold (dBFS).
    pub squelch_level: f64,
    /// The AGC.
    pub agc: bool,
    /// The manual gain (dB).
    pub gain: f64,
    /// The NFM de-emphasis.
    pub deemph_nfm: Deemphasis,
    /// The WFM de-emphasis.
    pub deemph_wfm: Deemphasis,
    /// The audio low-pass of NFM.
    pub nfm_lowpass: NfmLowPass,
    /// The CW pitch (Hz).
    pub cw_pitch: f64,
    /// Auto quality.
    pub am_auto: bool,
    /// The AM detector.
    pub am_detector: AmDetector,
    /// The AM noise reduction.
    pub am_nr: AmNoise,
    /// The AM audio bandwidth (Hz, 0 for automatic).
    pub am_bandwidth: f64,
    /// Synthetic treble.
    pub am_enhance: bool,
    /// The blanker.
    pub nb: bool,
    /// The blanker threshold (dB).
    pub nb_threshold: f64,
    /// The audio filters: high-pass, low-pass, notch (Hz, 0 for off).
    pub af: [f64; 3],
    /// The presets, as typed.
    pub presets: String,
    /// The presets.
    pub preset_list: Vec<Preset>,
    /// The tone gate, as typed.
    pub tone_gate: String,
    /// The tone gate.
    pub gate: ToneGate,
    /// The channel heard (0 main, 1 to 3 extra).
    pub listen_channel: usize,
    /// The extra channels.
    pub extra_channels: [Extra; 3],
    /// The transmit settings.
    pub tx: TxSettings,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        GeneralSettings {
            demod: Demod::Nfm,
            mem: Memory::default(),
            channel_offset: 0.0,
            custom_filter: String::new(),
            custom_taps: Vec::new(),
            squelch: SquelchKind::Auto,
            squelch_level: -60.0,
            agc: true,
            gain: 20.0,
            deemph_nfm: Deemphasis::Off,
            deemph_wfm: Deemphasis::Us50,
            nfm_lowpass: NfmLowPass::Khz3,
            cw_pitch: 700.0,
            am_auto: true,
            am_detector: AmDetector::Auto,
            am_nr: AmNoise::Auto,
            am_bandwidth: 0.0,
            am_enhance: false,
            nb: false,
            nb_threshold: 12.0,
            af: [0.0; 3],
            presets: String::new(),
            preset_list: Vec::new(),
            tone_gate: "open".into(),
            gate: ToneGate::Open,
            listen_channel: 0,
            extra_channels: [
                extra(EXTRA1_OFFSET.default),
                extra(EXTRA2_OFFSET.default),
                extra(EXTRA3_OFFSET.default),
            ],
            tx: TxSettings::default(),
        }
    }
}

fn extra(offset_hz: f64) -> Extra {
    Extra {
        on: false,
        offset_hz,
        demod: Demod::Nfm,
        width_hz: 0.0,
    }
}

impl GeneralSettings {
    /// The settings in `v`, read in the order of [`GeneralSetting`] (the demodulator first, so the width and the step land in its
    /// memory). A setting that `v` does not hold keeps its default.
    ///
    /// # Errors
    ///
    /// The first value that [`GeneralSettings::apply`] refuses.
    pub fn from_values(v: &Values) -> Result<Self, SettingError> {
        let mut s = GeneralSettings::default();
        for (i, decl) in GeneralSetting::schema().settings().iter().enumerate() {
            let Ok(at) = v.schema().id(&decl.field.key) else {
                continue;
            };
            if let Some(value) = v.value(at) {
                s.apply(SettingId(i as u16), value)?;
            }
        }
        Ok(s)
    }

    /// The values of these settings: the width, the step and the voice noise remover of the demodulator in use.
    pub fn values(&self) -> Values {
        use GeneralSetting as S;
        let d = self.demod.slot();
        let e = &self.extra_channels;
        let mut v = values_of([
            (S::Demod, Value::Choice(self.demod.index())),
            (S::Width, Value::Hz(self.mem.width[d])),
            (S::ChannelOffset, Value::Hz(self.channel_offset)),
            (S::Step, Value::Hz(self.mem.step[d])),
            (S::CustomFilter, Value::Text(self.custom_filter.clone())),
            (S::Squelch, Value::Choice(self.squelch.index())),
            (S::SquelchLevel, Value::F64(self.squelch_level)),
            (S::Agc, Value::Bool(self.agc)),
            (S::Gain, Value::F64(self.gain)),
            (S::DeemphNfm, Value::Choice(self.deemph_nfm.index())),
            (S::DeemphWfm, Value::Choice(self.deemph_wfm.index())),
            (S::NfmLowpass, Value::Choice(self.nfm_lowpass.index())),
            (S::CwPitch, Value::Hz(self.cw_pitch)),
            (S::AmAuto, Value::Bool(self.am_auto)),
            (S::AmDetector, Value::Choice(self.am_detector.index())),
            (S::AmNr, Value::Choice(self.am_nr.index())),
            (S::AmBandwidth, Value::Hz(self.am_bandwidth)),
            (S::AmEnhance, Value::Bool(self.am_enhance)),
            (S::Nb, Value::Bool(self.nb)),
            (S::NbThreshold, Value::F64(self.nb_threshold)),
            (S::AfHighpass, Value::Hz(self.af[0])),
            (S::AfLowpass, Value::Hz(self.af[1])),
            (S::AfNotch, Value::Hz(self.af[2])),
            (S::VoiceNr, Value::Bool(self.mem.voice_nr[d])),
            (
                S::VoiceNrStrength,
                Value::F64(self.mem.voice_nr_strength[d]),
            ),
            (S::Presets, Value::Text(self.presets.clone())),
            (S::ToneGate, Value::Text(self.tone_gate.clone())),
            (S::ListenChannel, Value::I64(self.listen_channel as i64)),
            (S::Extra1, Value::Bool(e[0].on)),
            (S::Extra1Offset, Value::Hz(e[0].offset_hz)),
            (S::Extra1Demod, Value::Choice(e[0].demod.index())),
            (S::Extra1Width, Value::Hz(e[0].width_hz)),
            (S::Extra2, Value::Bool(e[1].on)),
            (S::Extra2Offset, Value::Hz(e[1].offset_hz)),
            (S::Extra2Demod, Value::Choice(e[1].demod.index())),
            (S::Extra2Width, Value::Hz(e[1].width_hz)),
            (S::Extra3, Value::Bool(e[2].on)),
            (S::Extra3Offset, Value::Hz(e[2].offset_hz)),
            (S::Extra3Demod, Value::Choice(e[2].demod.index())),
            (S::Extra3Width, Value::Hz(e[2].width_hz)),
        ]);
        let schema = GeneralSetting::schema();
        for (key, x) in tx::TX_KEYS.iter().zip(self.tx.values()) {
            if let Ok(id) = schema.id(key) {
                let _ = v.set(id, &x);
            }
        }
        v
    }

    /// The width of the demodulator in use.
    pub fn width(&self) -> f64 {
        self.mem.width[self.demod.slot()]
    }
    /// The step of the demodulator in use.
    pub fn step(&self) -> f64 {
        self.mem.step[self.demod.slot()]
    }
    /// The voice noise remover is on and offered for the demodulator in use.
    pub fn voice_nr(&self) -> bool {
        self.demod.offers_voice_nr() && self.mem.voice_nr[self.demod.slot()]
    }
    /// The de-emphasis of the demodulator in use (µs).
    pub fn deemph_us(&self) -> f64 {
        match self.demod {
            Demod::Nfm => self.deemph_nfm.micros(),
            Demod::Wfm => self.deemph_wfm.micros(),
            _ => 0.0,
        }
    }
    /// The AM processor runs: AM, with Auto quality or a synchronous detector.
    pub fn am_processor(&self) -> bool {
        self.demod == Demod::Am && (self.am_auto || self.am_detector.is_synchronous())
    }

    /// Applies one value of the setting `id` of [`GeneralSetting`]. A width outside the range of its demodulator is brought into
    /// that range. The generator settings change nothing here.
    ///
    /// # Errors
    ///
    /// [`SettingError::NoSuchId`] for an id outside [`GeneralSetting`], the kind errors of [`Setting::read`],
    /// [`SettingError::NotPermitted`] for an AM bandwidth between 0 and 2500 Hz, and [`SettingError::Malformed`] for custom taps
    /// or a tone gate that do not parse. A refused value changes nothing.
    pub fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        use GeneralSetting as S;
        let d = self.demod.slot();
        let malformed = |at: usize| SettingError::Malformed { id, at: Some(at) };
        match S::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            S::Demod => self.demod = DEMOD.read(id, v)?,
            S::Width => {
                let w = WIDTH.read(id, v)?;
                self.mem.width[d] = self.demod.clamp_width(w, self.mem.width[d]);
            }
            S::ChannelOffset => self.channel_offset = CHANNEL_OFFSET.read(id, v)?,
            S::Step => self.mem.step[d] = STEP.read(id, v)?,
            S::CustomFilter => {
                let t = CUSTOM_FILTER.read(id, v)?;
                self.custom_taps = parse_taps(t).map_err(malformed)?;
                t.clone_into(&mut self.custom_filter);
            }
            S::Squelch => self.squelch = SQUELCH.read(id, v)?,
            S::SquelchLevel => self.squelch_level = SQUELCH_LEVEL.read(id, v)?,
            S::Agc => self.agc = AGC.read(id, v)?,
            S::Gain => self.gain = GAIN.read(id, v)?,
            S::DeemphNfm => self.deemph_nfm = DEEMPH_NFM.read(id, v)?,
            S::DeemphWfm => self.deemph_wfm = DEEMPH_WFM.read(id, v)?,
            S::NfmLowpass => self.nfm_lowpass = NFM_LOWPASS.read(id, v)?,
            S::CwPitch => self.cw_pitch = CW_PITCH.read(id, v)?,
            S::AmAuto => self.am_auto = AM_AUTO.read(id, v)?,
            S::AmDetector => self.am_detector = AM_DETECTOR.read(id, v)?,
            S::AmNr => self.am_nr = AM_NR.read(id, v)?,
            S::AmBandwidth => {
                let b = AM_BANDWIDTH.read(id, v)?;
                if b > 0.0 && b < 2500.0 {
                    return Err(SettingError::NotPermitted { id });
                }
                self.am_bandwidth = b;
            }
            S::AmEnhance => self.am_enhance = AM_ENHANCE.read(id, v)?,
            S::Nb => self.nb = NB.read(id, v)?,
            S::NbThreshold => self.nb_threshold = NB_THRESHOLD.read(id, v)?,
            S::AfHighpass => self.af[0] = AF_HIGHPASS.read(id, v)?,
            S::AfLowpass => self.af[1] = AF_LOWPASS.read(id, v)?,
            S::AfNotch => self.af[2] = AF_NOTCH.read(id, v)?,
            S::VoiceNr => self.mem.voice_nr[d] = VOICE_NR.read(id, v)?,
            S::VoiceNrStrength => self.mem.voice_nr_strength[d] = VOICE_NR_STRENGTH.read(id, v)?,
            S::Presets => {
                let t = PRESETS.read(id, v)?;
                self.preset_list =
                    crate::presets::parse(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.presets);
            }
            S::ToneGate => {
                let t = TONE_GATE.read(id, v)?;
                self.gate = parse_gate(t).map_err(malformed)?;
                t.clone_into(&mut self.tone_gate);
            }
            S::ListenChannel => self.listen_channel = LISTEN_CHANNEL.read(id, v)? as usize,
            S::Extra1 => self.extra_channels[0].on = EXTRA1.read(id, v)?,
            S::Extra1Offset => self.extra_channels[0].offset_hz = EXTRA1_OFFSET.read(id, v)?,
            S::Extra1Demod => self.extra_channels[0].demod = EXTRA1_DEMOD.read(id, v)?,
            S::Extra1Width => self.extra_channels[0].width_hz = EXTRA1_WIDTH.read(id, v)?,
            S::Extra2 => self.extra_channels[1].on = EXTRA2.read(id, v)?,
            S::Extra2Offset => self.extra_channels[1].offset_hz = EXTRA2_OFFSET.read(id, v)?,
            S::Extra2Demod => self.extra_channels[1].demod = EXTRA2_DEMOD.read(id, v)?,
            S::Extra2Width => self.extra_channels[1].width_hz = EXTRA2_WIDTH.read(id, v)?,
            S::Extra3 => self.extra_channels[2].on = EXTRA3.read(id, v)?,
            S::Extra3Offset => self.extra_channels[2].offset_hz = EXTRA3_OFFSET.read(id, v)?,
            S::Extra3Demod => self.extra_channels[2].demod = EXTRA3_DEMOD.read(id, v)?,
            S::Extra3Width => self.extra_channels[2].width_hz = EXTRA3_WIDTH.read(id, v)?,
            S::TxSplit
            | S::TxDeviation
            | S::TxPreemphasis
            | S::TxCtcss
            | S::TxAmDepth
            | S::TxSsbMethod
            | S::TxSsbBand
            | S::TxCompression
            | S::TxTimeout
            | S::TxCwWpm
            | S::TxCwFarnsworth
            | S::TxTuneLimit => {
                let key = GeneralSetting::SETTINGS[id.index()].key();
                self.tx.apply(key, id, v)?;
            }
            S::GenNoiseOnly
            | S::GenTuneOffset
            | S::GenTone
            | S::GenMod
            | S::GenCtcss
            | S::GenCtcssDev
            | S::GenAudio => {}
            S::GenDcs => {
                let t = GEN_DCS.read(id, v)?;
                crate::generator::parse_gen_dcs(t).map_err(malformed)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(s: &mut GeneralSettings, setting: GeneralSetting, v: Value) {
        s.apply(setting.id(), &v).unwrap();
    }

    #[test]
    fn widths_are_kept_per_demodulator() {
        let mut s = GeneralSettings::default();
        assert_eq!(s.width(), 12_500.0);
        set(&mut s, GeneralSetting::Width, Value::Hz(25_000.0));
        set(
            &mut s,
            GeneralSetting::Demod,
            Value::Choice(Demod::Usb.index()),
        );
        assert_eq!(s.width(), 2700.0);
        set(&mut s, GeneralSetting::Width, Value::Hz(0.0));
        assert_eq!(s.width(), 2700.0, "0 keeps the stored width");
        set(&mut s, GeneralSetting::Width, Value::Hz(200_000.0));
        assert_eq!(s.width(), 6000.0);
        set(
            &mut s,
            GeneralSetting::Demod,
            Value::Choice(Demod::Nfm.index()),
        );
        assert_eq!(s.width(), 25_000.0);
        assert_eq!(s.step(), 12_500.0);
        let d = GeneralSettings::default();
        assert!(
            d.am_auto
                && d.squelch == SquelchKind::Auto
                && d.agc
                && d.deemph_wfm == Deemphasis::Us50
                && d.deemph_nfm == Deemphasis::Off
        );
    }

    #[test]
    fn values_read_back_into_the_same_settings() {
        let mut s = GeneralSettings::default();
        set(
            &mut s,
            GeneralSetting::Demod,
            Value::Choice(Demod::Am.index()),
        );
        set(&mut s, GeneralSetting::Width, Value::Hz(6000.0));
        set(
            &mut s,
            GeneralSetting::ToneGate,
            Value::Text("tone:88.5".into()),
        );
        let back = GeneralSettings::from_values(&s.values()).unwrap();
        assert_eq!(back.demod, Demod::Am);
        assert_eq!(back.width(), 6000.0);
        assert_eq!(back.gate, ToneGate::Tone(88.5));
    }

    #[test]
    fn values_the_receiver_cannot_use_are_refused() {
        let mut s = GeneralSettings::default();
        let id = GeneralSetting::AmBandwidth.id();
        assert_eq!(
            s.apply(id, &Value::Hz(1000.0)),
            Err(SettingError::NotPermitted { id })
        );
        let id = GeneralSetting::ToneGate.id();
        assert_eq!(
            s.apply(id, &Value::Text("tone:x".into())),
            Err(SettingError::Malformed { id, at: Some(5) })
        );
        assert_eq!(s.tone_gate, "open", "a refused value changes nothing");
    }
}

//! The settings of the listening chain: the values, the presets of each kind of stack, and the declared settings.

use sdrkit_core::publish::field;
use sdrkit_core::{Choice, Setting, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_util_ids::tones::dcs::Dcs;

/// The DCS code 023, the default of the tone squelch.
pub const DCS_023: Dcs = match Dcs::new(0o23) {
    Some(d) => d,
    None => panic!("023 is a DCS code"),
};

/// A DCS code as written: one to three octal digits, with or without a leading `D` ("023", "D023", "23"). The value is the octal
/// number that the digits write. Standard: TIA-603 (the nine code bits are three octal digits).
pub fn parse_dcs(text: &str) -> Option<Dcs> {
    let t = text.trim();
    let t = t.strip_prefix(['D', 'd']).unwrap_or(t);
    if t.is_empty() || t.len() > 3 || !t.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return None;
    }
    let mut digits = [b'0'; 3];
    digits[3 - t.len()..].copy_from_slice(t.as_bytes());
    Dcs::from_octal(std::str::from_utf8(&digits).ok()?)
}

/// The kind of stack, for the presets and defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModeKind {
    /// Data stacks: nothing (bypass).
    Data,
    /// TV: nothing.
    Tv,
    /// Spectrum: nothing.
    Spectrum,
    /// The general receiver.
    #[default]
    Generic,
    /// AM and airband.
    AmAir,
    /// Narrow FM.
    Nfm,
    /// FM broadcast.
    WfmBroadcast,
    /// SSB and HF.
    Ssb,
    /// CW.
    Cw,
    /// Digital voice.
    DigitalVoice,
}

sdrkit_core::choice_enum! {
    /// The EQ presets.
    #[derive(Default)]
    pub enum EqPreset {
        /// 0/0/0/0/0.
        #[default]
        Flat = ("0", "Flat"),
        /// -9/-2/+2/+4/-3.
        Voice = ("1", "Voice"),
        /// +4/+1/-1/+1/+4.
        Music = ("2", "Music"),
        /// -12/-6/+6/0/-9.
        Cw = ("3", "CW"),
        /// The gains of the user.
        Custom = ("4", "Custom"),
    }
}

impl EqPreset {
    /// The gains (dB) of a preset. `None` for Custom.
    pub fn gains(self) -> Option<[f32; 5]> {
        Some(match self {
            EqPreset::Flat => [0.0; 5],
            EqPreset::Voice => [-9.0, -2.0, 2.0, 4.0, -3.0],
            EqPreset::Music => [4.0, 1.0, -1.0, 1.0, 4.0],
            EqPreset::Cw => [-12.0, -6.0, 6.0, 0.0, -9.0],
            EqPreset::Custom => return None,
        })
    }
}

sdrkit_core::choice_enum! {
    /// The stereo modes.
    #[derive(Default)]
    pub enum StereoMode {
        /// Both channels as received.
        #[default]
        AsIs = ("0", "As received"),
        /// The mean of both.
        Mono = ("1", "Mono"),
        /// Left and right swapped.
        Swap = ("2", "Swapped"),
        /// The left channel on both.
        Left = ("3", "Left"),
        /// The right channel on both.
        Right = ("4", "Right"),
    }
}

sdrkit_core::choice_enum! {
    /// The squelch kinds of the chain.
    #[derive(Default)]
    pub enum ChainSquelch {
        /// Off.
        #[default]
        Off = ("0", "Off"),
        /// The level of the mid signal.
        Level = ("1", "Level"),
        /// The speech band against the noise band.
        Snr = ("2", "SNR"),
        /// A CTCSS tone.
        Ctcss = ("3", "CTCSS"),
        /// A DCS code.
        Dcs = ("4", "DCS"),
    }
}

impl ChainSquelch {
    /// A tone squelch (CTCSS or DCS).
    pub fn is_tone(self) -> bool {
        matches!(self, ChainSquelch::Ctcss | ChainSquelch::Dcs)
    }
}

sdrkit_core::choice_enum! {
    /// The AGC speeds.
    #[derive(Default)]
    pub enum AgcSpeed {
        /// Off.
        #[default]
        Off = ("0", "Off"),
        /// 2 ms attack, 120 ms release.
        Fast = ("1", "Fast"),
        /// 20 ms attack, 1.5 s release.
        Slow = ("2", "Slow"),
    }
}

sdrkit_core::choice_enum! {
    /// The de-emphasis of the chain.
    #[derive(Default)]
    pub enum ChainDeemph {
        /// None.
        #[default]
        Off = ("0", "Off"),
        /// 50 µs.
        Us50 = ("50", "50 µs"),
        /// 75 µs.
        Us75 = ("75", "75 µs"),
    }
}

impl ChainDeemph {
    /// The time constant (µs, 0 for none).
    pub fn micros(self) -> f32 {
        match self {
            ChainDeemph::Off => 0.0,
            ChainDeemph::Us50 => 50.0,
            ChainDeemph::Us75 => 75.0,
        }
    }
}

/// The settings of the cleanup part.
#[derive(Clone, Debug, PartialEq)]
pub struct CleanupSettings {
    /// The cleanup is switched on.
    pub on: bool,
    /// Automatic notch (up to 4 steady tones).
    pub auto_notch: bool,
    /// Manual notch on.
    pub notch: bool,
    /// Manual notch frequency (30 to 20 000 Hz).
    pub notch_hz: f32,
    /// Manual notch width (0 narrowest to 1 widest).
    pub notch_width: f32,
    /// Noise reduction on.
    pub nr: bool,
    /// NR strength (0 to 1).
    pub nr_strength: f32,
    /// Voice emphasis on.
    pub voice: bool,
    /// Voice emphasis amount (0 to 1).
    pub voice_amount: f32,
    /// De-esser on.
    pub deesser: bool,
    /// De-esser amount (0 to 1).
    pub deesser_amount: f32,
    /// Silence gate on.
    pub gate: bool,
    /// Gate offset above the noise floor (dB).
    pub gate_offset_db: f32,
    /// Leveller on.
    pub leveller: bool,
    /// Leveller target (dBFS).
    pub leveller_target_db: f32,
    /// Leveller amount (0 to 1).
    pub leveller_amount: f32,
    /// "Do not boost the noise".
    pub leveller_hold: bool,
    /// The neural voice denoiser.
    pub denoiser: bool,
    /// Its strength (0 to 1).
    pub denoiser_strength: f32,
    /// Record the cleaned sound instead of the raw.
    pub record_cleaned: bool,
    /// With it, record nothing while the gate is shut.
    pub record_skip_silence: bool,
}

impl Default for CleanupSettings {
    fn default() -> Self {
        CleanupSettings {
            on: false,
            auto_notch: false,
            notch: false,
            notch_hz: 1000.0,
            notch_width: 0.5,
            nr: false,
            nr_strength: 0.5,
            voice: false,
            voice_amount: 0.6,
            deesser: false,
            deesser_amount: 0.5,
            gate: false,
            gate_offset_db: 9.0,
            leveller: false,
            leveller_target_db: -20.0,
            leveller_amount: 0.7,
            leveller_hold: true,
            denoiser: false,
            denoiser_strength: 1.0,
            record_cleaned: false,
            record_skip_silence: false,
        }
    }
}

/// The settings of the audio control part.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlSettings {
    /// The chain is switched on.
    pub on: bool,
    /// The stereo mode.
    pub stereo: StereoMode,
    /// High-pass on.
    pub highpass: bool,
    /// High-pass corner (20 to 8000 Hz).
    pub highpass_hz: f32,
    /// Low-pass on.
    pub lowpass: bool,
    /// Low-pass corner (200 to 20 000 Hz).
    pub lowpass_hz: f32,
    /// 24 dB/octave (two biquads) instead of 12.
    pub steep: bool,
    /// Notch on.
    pub notch: bool,
    /// Notch frequency (50 to 12 000 Hz).
    pub notch_hz: f32,
    /// Notch Q (1 to 60).
    pub notch_q: f32,
    /// Automatic notch (adaptive line enhancer).
    pub auto_notch: bool,
    /// Clarifier shift (Hz, -1500 to 1500).
    pub clarifier_hz: f32,
    /// Wanted CW pitch (Hz, 300 to 1500, 0 = leave).
    pub cw_pitch_hz: f32,
    /// EQ preset.
    pub eq: EqPreset,
    /// EQ gains (dB) at 120, 400, 1000, 2500, 6000 Hz.
    pub eq_gains: [f32; 5],
    /// The de-emphasis.
    pub deemph: ChainDeemph,
    /// The AGC speed.
    pub agc: AgcSpeed,
    /// AGC target (dBFS, -40 to -2).
    pub agc_target_db: f32,
    /// The squelch kind.
    pub squelch: ChainSquelch,
    /// Squelch open threshold (Level: dBFS -100 to 40, SNR: dB).
    pub squelch_open: f32,
    /// Hysteresis (0 to 30 dB).
    pub squelch_hysteresis: f32,
    /// Hang (0 to 3000 ms).
    pub squelch_hang_ms: f32,
    /// The CTCSS tone of the tone squelch (Hz).
    pub ctcss_hz: f32,
    /// The DCS code of the tone squelch.
    pub dcs_code: Dcs,
    /// DCS inverted polarity.
    pub dcs_inverted: bool,
    /// Tail elimination (0 to 400 ms).
    pub tail_ms: f32,
    /// Volume (0 to 2).
    pub volume: f32,
    /// Mute.
    pub mute: bool,
    /// The output device by name ("" the system default).
    pub device: String,
    /// Start latency (20 to 2000 ms).
    pub start_latency_ms: f32,
    /// Buffer (0 to 2000 ms, 0 automatic).
    pub buffer_ms: f32,
    /// The recorder gets nothing while the squelch is closed.
    pub record_only_open: bool,
    /// Compute the AF spectrum (a screen shows it).
    pub spectrum: bool,
}

impl Default for ControlSettings {
    fn default() -> Self {
        ControlSettings {
            on: false,
            stereo: StereoMode::AsIs,
            highpass: false,
            highpass_hz: 300.0,
            lowpass: false,
            lowpass_hz: 3000.0,
            steep: false,
            notch: false,
            notch_hz: 1000.0,
            notch_q: 10.0,
            auto_notch: false,
            clarifier_hz: 0.0,
            cw_pitch_hz: 0.0,
            eq: EqPreset::Flat,
            eq_gains: [0.0; 5],
            deemph: ChainDeemph::Off,
            agc: AgcSpeed::Off,
            agc_target_db: -18.0,
            squelch: ChainSquelch::Off,
            squelch_open: -50.0,
            squelch_hysteresis: 4.0,
            squelch_hang_ms: 250.0,
            ctcss_hz: 88.5,
            dcs_code: DCS_023,
            dcs_inverted: false,
            tail_ms: 0.0,
            volume: 1.0,
            mute: false,
            device: String::new(),
            start_latency_ms: 300.0,
            buffer_ms: 0.0,
            record_only_open: false,
            spectrum: false,
        }
    }
}

/// Everything the chain is set with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChainSettings {
    /// The cleanup part.
    pub cleanup: CleanupSettings,
    /// The audio control part.
    pub control: ControlSettings,
}

const CLEANUP: &str = "Cleanup";
const CHAIN: &str = "Chain";
const SQUELCH_SECTION: &str = "Squelch";
const OUTPUT: &str = "Output";

const CLEAN: Setting<field::Bool> = Setting::new("clean", "Cleanup", "", false).section(CLEANUP);
const CLEAN_AUTO_NOTCH: Setting<field::Bool> =
    Setting::new("clean.auto_notch", "Automatic notch", "", false).section(CLEANUP);
const CLEAN_NOTCH: Setting<field::Bool> =
    Setting::new("clean.notch", "Notch", "", false).section(CLEANUP);
const CLEAN_NOTCH_HZ: Setting<field::Hz> =
    Setting::new("clean.notch_hz", "Notch frequency", "Hz", 1000.0)
        .range(30.0, 20000.0, 0.0)
        .section(CLEANUP);
const CLEAN_NOTCH_WIDTH: Setting<field::F64> =
    Setting::new("clean.notch_width", "Notch width", "", 0.5)
        .range(0.0, 1.0, 0.0)
        .section(CLEANUP);
const CLEAN_NR: Setting<field::Bool> =
    Setting::new("clean.nr", "Noise reduction", "", false).section(CLEANUP);
const CLEAN_NR_STRENGTH: Setting<field::F64> =
    Setting::new("clean.nr_strength", "Noise reduction strength", "", 0.5)
        .range(0.0, 1.0, 0.0)
        .section(CLEANUP);
const CLEAN_VOICE: Setting<field::Bool> =
    Setting::new("clean.voice", "Voice emphasis", "", false).section(CLEANUP);
const CLEAN_VOICE_AMOUNT: Setting<field::F64> =
    Setting::new("clean.voice_amount", "Voice emphasis amount", "", 0.6)
        .range(0.0, 1.0, 0.0)
        .section(CLEANUP);
const CLEAN_DEESSER: Setting<field::Bool> =
    Setting::new("clean.deesser", "De-esser", "", false).section(CLEANUP);
const CLEAN_DEESSER_AMOUNT: Setting<field::F64> =
    Setting::new("clean.deesser_amount", "De-esser amount", "", 0.5)
        .range(0.0, 1.0, 0.0)
        .section(CLEANUP);
const CLEAN_GATE: Setting<field::Bool> =
    Setting::new("clean.gate", "Noise gate", "", false).section(CLEANUP);
const CLEAN_GATE_OFFSET: Setting<field::F64> =
    Setting::new("clean.gate_offset", "Gate offset", "dB", 9.0)
        .range(0.0, 40.0, 0.0)
        .section(CLEANUP);
const CLEAN_LEVELLER: Setting<field::Bool> =
    Setting::new("clean.leveller", "Leveller", "", false).section(CLEANUP);
const CLEAN_LEVELLER_TARGET: Setting<field::F64> =
    Setting::new("clean.leveller_target", "Leveller target", "dBFS", -20.0)
        .range(-40.0, -2.0, 0.0)
        .section(CLEANUP);
const CLEAN_LEVELLER_AMOUNT: Setting<field::F64> =
    Setting::new("clean.leveller_amount", "Leveller amount", "", 0.7)
        .range(0.0, 1.0, 0.0)
        .section(CLEANUP);
const CLEAN_LEVELLER_HOLD: Setting<field::Bool> =
    Setting::new("clean.leveller_hold", "Leveller hold", "", true).section(CLEANUP);
const CLEAN_DENOISER: Setting<field::Bool> =
    Setting::new("clean.denoiser", "Denoiser", "", false).section(CLEANUP);
const CLEAN_DENOISER_STRENGTH: Setting<field::F64> =
    Setting::new("clean.denoiser_strength", "Denoiser strength", "", 1.0)
        .range(0.0, 1.0, 0.0)
        .section(CLEANUP);
const CLEAN_RECORD_CLEANED: Setting<field::Bool> = Setting::new(
    "clean.record_cleaned",
    "Record the cleaned sound",
    "",
    false,
)
.section(CLEANUP);
const CLEAN_RECORD_SKIP_SILENCE: Setting<field::Bool> = Setting::new(
    "clean.record_skip_silence",
    "Skip silence in recordings",
    "",
    false,
)
.section(CLEANUP);
const CHAIN_ON: Setting<field::Bool> =
    Setting::new("chain", "Listening chain", "", false).section(CHAIN);
const STEREO: Setting<field::Choice<StereoMode>> =
    Setting::choice("stereo", "Channels", StereoMode::AsIs).section(CHAIN);
const HIGHPASS: Setting<field::Bool> =
    Setting::new("highpass", "High-pass", "", false).section(CHAIN);
const HIGHPASS_HZ: Setting<field::Hz> =
    Setting::new("highpass_hz", "High-pass frequency", "Hz", 300.0)
        .range(20.0, 8000.0, 0.0)
        .section(CHAIN);
const LOWPASS: Setting<field::Bool> = Setting::new("lowpass", "Low-pass", "", false).section(CHAIN);
const LOWPASS_HZ: Setting<field::Hz> =
    Setting::new("lowpass_hz", "Low-pass frequency", "Hz", 3000.0)
        .range(200.0, 20000.0, 0.0)
        .section(CHAIN);
const STEEP: Setting<field::Bool> =
    Setting::new("steep", "Steep filters", "", false).section(CHAIN);
const NOTCH: Setting<field::Bool> = Setting::new("notch", "Notch", "", false).section(CHAIN);
const NOTCH_HZ: Setting<field::Hz> = Setting::new("notch_hz", "Notch frequency", "Hz", 1000.0)
    .range(50.0, 12000.0, 0.0)
    .section(CHAIN);
const NOTCH_Q: Setting<field::F64> = Setting::new("notch_q", "Notch Q", "", 10.0)
    .range(1.0, 60.0, 0.0)
    .section(CHAIN);
const AUTO_NOTCH: Setting<field::Bool> =
    Setting::new("auto_notch", "Automatic notch", "", false).section(CHAIN);
const CLARIFIER: Setting<field::Hz> = Setting::new("clarifier", "Clarifier", "Hz", 0.0)
    .range(-1500.0, 1500.0, 0.0)
    .section(CHAIN);
const CW_PITCH: Setting<field::Hz> = Setting::new("cw_pitch", "CW pitch", "Hz", 0.0)
    .range(0.0, 1500.0, 0.0)
    .section(CHAIN);
const EQ: Setting<field::Choice<EqPreset>> =
    Setting::choice("eq", "EQ preset", EqPreset::Flat).section(CHAIN);
const EQ1: Setting<field::F64> = Setting::new("eq1", "EQ 120 Hz", "dB", 0.0)
    .range(-18.0, 18.0, 0.0)
    .section(CHAIN);
const EQ2: Setting<field::F64> = Setting::new("eq2", "EQ 400 Hz", "dB", 0.0)
    .range(-18.0, 18.0, 0.0)
    .section(CHAIN);
const EQ3: Setting<field::F64> = Setting::new("eq3", "EQ 1 kHz", "dB", 0.0)
    .range(-18.0, 18.0, 0.0)
    .section(CHAIN);
const EQ4: Setting<field::F64> = Setting::new("eq4", "EQ 2.5 kHz", "dB", 0.0)
    .range(-18.0, 18.0, 0.0)
    .section(CHAIN);
const EQ5: Setting<field::F64> = Setting::new("eq5", "EQ 6 kHz", "dB", 0.0)
    .range(-18.0, 18.0, 0.0)
    .section(CHAIN);
const DEEMPH: Setting<field::Choice<ChainDeemph>> =
    Setting::choice("deemph", "De-emphasis", ChainDeemph::Off).section(CHAIN);
const AGC: Setting<field::Choice<AgcSpeed>> =
    Setting::choice("agc", "AGC", AgcSpeed::Off).section(CHAIN);
const AGC_TARGET: Setting<field::F64> = Setting::new("agc_target", "AGC target", "dBFS", -18.0)
    .range(-40.0, -2.0, 0.0)
    .section(CHAIN);
const SQUELCH: Setting<field::Choice<ChainSquelch>> =
    Setting::choice("squelch", "Squelch", ChainSquelch::Off).section(SQUELCH_SECTION);
const SQUELCH_OPEN: Setting<field::F64> =
    Setting::new("squelch_open", "Squelch opens at", "dB", -50.0)
        .range(-100.0, 40.0, 0.0)
        .section(SQUELCH_SECTION);
const SQUELCH_HYSTERESIS: Setting<field::F64> =
    Setting::new("squelch_hysteresis", "Squelch hysteresis", "dB", 4.0)
        .range(0.0, 30.0, 0.0)
        .section(SQUELCH_SECTION);
const SQUELCH_HANG: Setting<field::F64> = Setting::new("squelch_hang", "Squelch hang", "ms", 250.0)
    .range(0.0, 3000.0, 0.0)
    .section(SQUELCH_SECTION);
const CTCSS: Setting<field::Hz> = Setting::new("ctcss", "CTCSS tone", "Hz", 88.5)
    .range(60.0, 260.0, 0.0)
    .section(SQUELCH_SECTION);
const DCS: Setting<field::Text> = Setting::new("dcs", "DCS code", "", "023")
    .help("Three octal digits, as radios write them (023 to 754).")
    .section(SQUELCH_SECTION);
const DCS_INVERTED: Setting<field::Bool> =
    Setting::new("dcs_inverted", "DCS inverted", "", false).section(SQUELCH_SECTION);
const TAIL: Setting<field::F64> = Setting::new("tail", "Squelch tail", "ms", 0.0)
    .range(0.0, 400.0, 0.0)
    .section(SQUELCH_SECTION);
const VOLUME: Setting<field::F64> = Setting::new("volume", "Volume", "", 1.0)
    .range(0.0, 2.0, 0.0)
    .section(OUTPUT);
const MUTE: Setting<field::Bool> = Setting::new("mute", "Mute", "", false).section(OUTPUT);
const DEVICE: Setting<field::Text> = Setting::new("device", "Output device", "", "")
    .help("The output device by name. Empty for the system default.")
    .section(OUTPUT);
const START_LATENCY: Setting<field::F64> =
    Setting::new("start_latency", "Start latency", "ms", 300.0)
        .range(20.0, 2000.0, 0.0)
        .section(OUTPUT);
const BUFFER: Setting<field::F64> = Setting::new("buffer", "Buffer", "ms", 0.0)
    .range(0.0, 2000.0, 0.0)
    .section(OUTPUT);
const RECORD_ONLY_OPEN: Setting<field::Bool> =
    Setting::new("record_only_open", "Record only when open", "", false).section(OUTPUT);
const SPECTRUM: Setting<field::Bool> =
    Setting::new("spectrum", "Audio spectrum", "", false).section(OUTPUT);

sdrkit_core::settings! {
    /// The settings of the listening chain.
    pub enum ListenSetting {
        /// The `clean` setting.
        Clean = CLEAN,
        /// The `clean.auto_notch` setting.
        CleanAutoNotch = CLEAN_AUTO_NOTCH,
        /// The `clean.notch` setting.
        CleanNotch = CLEAN_NOTCH,
        /// The `clean.notch_hz` setting.
        CleanNotchHz = CLEAN_NOTCH_HZ,
        /// The `clean.notch_width` setting.
        CleanNotchWidth = CLEAN_NOTCH_WIDTH,
        /// The `clean.nr` setting.
        CleanNr = CLEAN_NR,
        /// The `clean.nr_strength` setting.
        CleanNrStrength = CLEAN_NR_STRENGTH,
        /// The `clean.voice` setting.
        CleanVoice = CLEAN_VOICE,
        /// The `clean.voice_amount` setting.
        CleanVoiceAmount = CLEAN_VOICE_AMOUNT,
        /// The `clean.deesser` setting.
        CleanDeesser = CLEAN_DEESSER,
        /// The `clean.deesser_amount` setting.
        CleanDeesserAmount = CLEAN_DEESSER_AMOUNT,
        /// The `clean.gate` setting.
        CleanGate = CLEAN_GATE,
        /// The `clean.gate_offset` setting.
        CleanGateOffset = CLEAN_GATE_OFFSET,
        /// The `clean.leveller` setting.
        CleanLeveller = CLEAN_LEVELLER,
        /// The `clean.leveller_target` setting.
        CleanLevellerTarget = CLEAN_LEVELLER_TARGET,
        /// The `clean.leveller_amount` setting.
        CleanLevellerAmount = CLEAN_LEVELLER_AMOUNT,
        /// The `clean.leveller_hold` setting.
        CleanLevellerHold = CLEAN_LEVELLER_HOLD,
        /// The `clean.denoiser` setting.
        CleanDenoiser = CLEAN_DENOISER,
        /// The `clean.denoiser_strength` setting.
        CleanDenoiserStrength = CLEAN_DENOISER_STRENGTH,
        /// The `clean.record_cleaned` setting.
        CleanRecordCleaned = CLEAN_RECORD_CLEANED,
        /// The `clean.record_skip_silence` setting.
        CleanRecordSkipSilence = CLEAN_RECORD_SKIP_SILENCE,
        /// The `chain` setting.
        ChainOn = CHAIN_ON,
        /// The `stereo` setting.
        StereoMode = STEREO,
        /// The `highpass` setting.
        Highpass = HIGHPASS,
        /// The `highpass_hz` setting.
        HighpassHz = HIGHPASS_HZ,
        /// The `lowpass` setting.
        Lowpass = LOWPASS,
        /// The `lowpass_hz` setting.
        LowpassHz = LOWPASS_HZ,
        /// The `steep` setting.
        Steep = STEEP,
        /// The `notch` setting.
        Notch = NOTCH,
        /// The `notch_hz` setting.
        NotchHz = NOTCH_HZ,
        /// The `notch_q` setting.
        NotchQ = NOTCH_Q,
        /// The `auto_notch` setting.
        AutoNotch = AUTO_NOTCH,
        /// The `clarifier` setting.
        Clarifier = CLARIFIER,
        /// The `cw_pitch` setting.
        CwPitch = CW_PITCH,
        /// The `eq` setting.
        Eq = EQ,
        /// The `eq1` setting.
        Eq1 = EQ1,
        /// The `eq2` setting.
        Eq2 = EQ2,
        /// The `eq3` setting.
        Eq3 = EQ3,
        /// The `eq4` setting.
        Eq4 = EQ4,
        /// The `eq5` setting.
        Eq5 = EQ5,
        /// The `deemph` setting.
        Deemph = DEEMPH,
        /// The `agc` setting.
        Agc = AGC,
        /// The `agc_target` setting.
        AgcTarget = AGC_TARGET,
        /// The `squelch` setting.
        Squelch = SQUELCH,
        /// The `squelch_open` setting.
        SquelchOpen = SQUELCH_OPEN,
        /// The `squelch_hysteresis` setting.
        SquelchHysteresis = SQUELCH_HYSTERESIS,
        /// The `squelch_hang` setting.
        SquelchHang = SQUELCH_HANG,
        /// The `ctcss` setting.
        Ctcss = CTCSS,
        /// The `dcs` setting.
        Dcs = DCS,
        /// The `dcs_inverted` setting.
        DcsInverted = DCS_INVERTED,
        /// The `tail` setting.
        Tail = TAIL,
        /// The `volume` setting.
        Volume = VOLUME,
        /// The `mute` setting.
        Mute = MUTE,
        /// The `device` setting.
        Device = DEVICE,
        /// The `start_latency` setting.
        StartLatency = START_LATENCY,
        /// The `buffer` setting.
        Buffer = BUFFER,
        /// The `record_only_open` setting.
        RecordOnlyOpen = RECORD_ONLY_OPEN,
        /// The `spectrum` setting.
        Spectrum = SPECTRUM,
    }
}

impl ChainSettings {
    /// The settings in `v`, by key. A setting that `v` does not hold keeps its default.
    ///
    /// # Errors
    ///
    /// The first value that [`ChainSettings::apply`] refuses.
    pub fn from_values(v: &Values) -> Result<Self, SettingError> {
        let mut s = ChainSettings::default();
        for (i, decl) in ListenSetting::schema().settings().iter().enumerate() {
            let Ok(at) = v.schema().id(&decl.field.key) else {
                continue;
            };
            if let Some(value) = v.value(at) {
                s.apply(SettingId(i as u16), value)?;
            }
        }
        Ok(s)
    }

    /// The values of these settings.
    pub fn values(&self) -> Values {
        use ListenSetting as S;
        let c = &self.cleanup;
        let a = &self.control;
        values_of([
            (S::Clean, Value::Bool(c.on)),
            (S::CleanAutoNotch, Value::Bool(c.auto_notch)),
            (S::CleanNotch, Value::Bool(c.notch)),
            (S::CleanNotchHz, Value::Hz(f64::from(c.notch_hz))),
            (S::CleanNotchWidth, Value::F64(f64::from(c.notch_width))),
            (S::CleanNr, Value::Bool(c.nr)),
            (S::CleanNrStrength, Value::F64(f64::from(c.nr_strength))),
            (S::CleanVoice, Value::Bool(c.voice)),
            (S::CleanVoiceAmount, Value::F64(f64::from(c.voice_amount))),
            (S::CleanDeesser, Value::Bool(c.deesser)),
            (
                S::CleanDeesserAmount,
                Value::F64(f64::from(c.deesser_amount)),
            ),
            (S::CleanGate, Value::Bool(c.gate)),
            (S::CleanGateOffset, Value::F64(f64::from(c.gate_offset_db))),
            (S::CleanLeveller, Value::Bool(c.leveller)),
            (
                S::CleanLevellerTarget,
                Value::F64(f64::from(c.leveller_target_db)),
            ),
            (
                S::CleanLevellerAmount,
                Value::F64(f64::from(c.leveller_amount)),
            ),
            (S::CleanLevellerHold, Value::Bool(c.leveller_hold)),
            (S::CleanDenoiser, Value::Bool(c.denoiser)),
            (
                S::CleanDenoiserStrength,
                Value::F64(f64::from(c.denoiser_strength)),
            ),
            (S::CleanRecordCleaned, Value::Bool(c.record_cleaned)),
            (
                S::CleanRecordSkipSilence,
                Value::Bool(c.record_skip_silence),
            ),
            (S::ChainOn, Value::Bool(a.on)),
            (S::StereoMode, Value::Choice(a.stereo.index())),
            (S::Highpass, Value::Bool(a.highpass)),
            (S::HighpassHz, Value::Hz(f64::from(a.highpass_hz))),
            (S::Lowpass, Value::Bool(a.lowpass)),
            (S::LowpassHz, Value::Hz(f64::from(a.lowpass_hz))),
            (S::Steep, Value::Bool(a.steep)),
            (S::Notch, Value::Bool(a.notch)),
            (S::NotchHz, Value::Hz(f64::from(a.notch_hz))),
            (S::NotchQ, Value::F64(f64::from(a.notch_q))),
            (S::AutoNotch, Value::Bool(a.auto_notch)),
            (S::Clarifier, Value::Hz(f64::from(a.clarifier_hz))),
            (S::CwPitch, Value::Hz(f64::from(a.cw_pitch_hz))),
            (S::Eq, Value::Choice(a.eq.index())),
            (S::Eq1, Value::F64(f64::from(a.eq_gains[0]))),
            (S::Eq2, Value::F64(f64::from(a.eq_gains[1]))),
            (S::Eq3, Value::F64(f64::from(a.eq_gains[2]))),
            (S::Eq4, Value::F64(f64::from(a.eq_gains[3]))),
            (S::Eq5, Value::F64(f64::from(a.eq_gains[4]))),
            (S::Deemph, Value::Choice(a.deemph.index())),
            (S::Agc, Value::Choice(a.agc.index())),
            (S::AgcTarget, Value::F64(f64::from(a.agc_target_db))),
            (S::Squelch, Value::Choice(a.squelch.index())),
            (S::SquelchOpen, Value::F64(f64::from(a.squelch_open))),
            (
                S::SquelchHysteresis,
                Value::F64(f64::from(a.squelch_hysteresis)),
            ),
            (S::SquelchHang, Value::F64(f64::from(a.squelch_hang_ms))),
            (S::Ctcss, Value::Hz(f64::from(a.ctcss_hz))),
            (S::Dcs, Value::Text(format!("{:03o}", a.dcs_code.value()))),
            (S::DcsInverted, Value::Bool(a.dcs_inverted)),
            (S::Tail, Value::F64(f64::from(a.tail_ms))),
            (S::Volume, Value::F64(f64::from(a.volume))),
            (S::Mute, Value::Bool(a.mute)),
            (S::Device, Value::Text(a.device.clone())),
            (S::StartLatency, Value::F64(f64::from(a.start_latency_ms))),
            (S::Buffer, Value::F64(f64::from(a.buffer_ms))),
            (S::RecordOnlyOpen, Value::Bool(a.record_only_open)),
            (S::Spectrum, Value::Bool(a.spectrum)),
        ])
    }

    /// Applies one value of the setting `id` of [`ListenSetting`].
    ///
    /// # Errors
    ///
    /// [`SettingError::NoSuchId`] for an id outside [`ListenSetting`], the kind errors of [`Setting::read`],
    /// [`SettingError::NotPermitted`] for a CW pitch between 0 and 300 Hz, and [`SettingError::Malformed`] for a DCS code that does
    /// not parse. A refused value changes nothing.
    pub fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        use ListenSetting as S;
        let c = &mut self.cleanup;
        let a = &mut self.control;
        match S::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            S::Clean => c.on = CLEAN.read(id, v)?,
            S::CleanAutoNotch => c.auto_notch = CLEAN_AUTO_NOTCH.read(id, v)?,
            S::CleanNotch => c.notch = CLEAN_NOTCH.read(id, v)?,
            S::CleanNotchHz => c.notch_hz = CLEAN_NOTCH_HZ.read(id, v)? as f32,
            S::CleanNotchWidth => c.notch_width = CLEAN_NOTCH_WIDTH.read(id, v)? as f32,
            S::CleanNr => c.nr = CLEAN_NR.read(id, v)?,
            S::CleanNrStrength => c.nr_strength = CLEAN_NR_STRENGTH.read(id, v)? as f32,
            S::CleanVoice => c.voice = CLEAN_VOICE.read(id, v)?,
            S::CleanVoiceAmount => c.voice_amount = CLEAN_VOICE_AMOUNT.read(id, v)? as f32,
            S::CleanDeesser => c.deesser = CLEAN_DEESSER.read(id, v)?,
            S::CleanDeesserAmount => c.deesser_amount = CLEAN_DEESSER_AMOUNT.read(id, v)? as f32,
            S::CleanGate => c.gate = CLEAN_GATE.read(id, v)?,
            S::CleanGateOffset => c.gate_offset_db = CLEAN_GATE_OFFSET.read(id, v)? as f32,
            S::CleanLeveller => c.leveller = CLEAN_LEVELLER.read(id, v)?,
            S::CleanLevellerTarget => {
                c.leveller_target_db = CLEAN_LEVELLER_TARGET.read(id, v)? as f32
            }
            S::CleanLevellerAmount => c.leveller_amount = CLEAN_LEVELLER_AMOUNT.read(id, v)? as f32,
            S::CleanLevellerHold => c.leveller_hold = CLEAN_LEVELLER_HOLD.read(id, v)?,
            S::CleanDenoiser => c.denoiser = CLEAN_DENOISER.read(id, v)?,
            S::CleanDenoiserStrength => {
                c.denoiser_strength = CLEAN_DENOISER_STRENGTH.read(id, v)? as f32
            }
            S::CleanRecordCleaned => c.record_cleaned = CLEAN_RECORD_CLEANED.read(id, v)?,
            S::CleanRecordSkipSilence => {
                c.record_skip_silence = CLEAN_RECORD_SKIP_SILENCE.read(id, v)?
            }
            S::ChainOn => a.on = CHAIN_ON.read(id, v)?,
            S::StereoMode => a.stereo = STEREO.read(id, v)?,
            S::Highpass => a.highpass = HIGHPASS.read(id, v)?,
            S::HighpassHz => a.highpass_hz = HIGHPASS_HZ.read(id, v)? as f32,
            S::Lowpass => a.lowpass = LOWPASS.read(id, v)?,
            S::LowpassHz => a.lowpass_hz = LOWPASS_HZ.read(id, v)? as f32,
            S::Steep => a.steep = STEEP.read(id, v)?,
            S::Notch => a.notch = NOTCH.read(id, v)?,
            S::NotchHz => a.notch_hz = NOTCH_HZ.read(id, v)? as f32,
            S::NotchQ => a.notch_q = NOTCH_Q.read(id, v)? as f32,
            S::AutoNotch => a.auto_notch = AUTO_NOTCH.read(id, v)?,
            S::Clarifier => a.clarifier_hz = CLARIFIER.read(id, v)? as f32,
            S::CwPitch => {
                let p = CW_PITCH.read(id, v)?;
                if p > 0.0 && p < 300.0 {
                    return Err(SettingError::NotPermitted { id });
                }
                a.cw_pitch_hz = p as f32;
            }
            S::Eq => a.eq = EQ.read(id, v)?,
            S::Eq1 => a.eq_gains[0] = EQ1.read(id, v)? as f32,
            S::Eq2 => a.eq_gains[1] = EQ2.read(id, v)? as f32,
            S::Eq3 => a.eq_gains[2] = EQ3.read(id, v)? as f32,
            S::Eq4 => a.eq_gains[3] = EQ4.read(id, v)? as f32,
            S::Eq5 => a.eq_gains[4] = EQ5.read(id, v)? as f32,
            S::Deemph => a.deemph = DEEMPH.read(id, v)?,
            S::Agc => a.agc = AGC.read(id, v)?,
            S::AgcTarget => a.agc_target_db = AGC_TARGET.read(id, v)? as f32,
            S::Squelch => a.squelch = SQUELCH.read(id, v)?,
            S::SquelchOpen => a.squelch_open = SQUELCH_OPEN.read(id, v)? as f32,
            S::SquelchHysteresis => a.squelch_hysteresis = SQUELCH_HYSTERESIS.read(id, v)? as f32,
            S::SquelchHang => a.squelch_hang_ms = SQUELCH_HANG.read(id, v)? as f32,
            S::Ctcss => a.ctcss_hz = CTCSS.read(id, v)? as f32,
            S::Dcs => {
                a.dcs_code =
                    parse_dcs(DCS.read(id, v)?).ok_or(SettingError::Malformed { id, at: None })?;
            }
            S::DcsInverted => a.dcs_inverted = DCS_INVERTED.read(id, v)?,
            S::Tail => a.tail_ms = TAIL.read(id, v)? as f32,
            S::Volume => a.volume = VOLUME.read(id, v)? as f32,
            S::Mute => a.mute = MUTE.read(id, v)?,
            S::Device => DEVICE.read(id, v)?.clone_into(&mut a.device),
            S::StartLatency => a.start_latency_ms = START_LATENCY.read(id, v)? as f32,
            S::Buffer => a.buffer_ms = BUFFER.read(id, v)? as f32,
            S::RecordOnlyOpen => a.record_only_open = RECORD_ONLY_OPEN.read(id, v)?,
            S::Spectrum => a.spectrum = SPECTRUM.read(id, v)?,
        }
        Ok(())
    }

    /// The EQ gains in use (the preset's, or the user's for Custom).
    pub fn eq_gains(&self) -> [f32; 5] {
        self.control.eq.gains().unwrap_or(self.control.eq_gains)
    }

    /// The defaults for a kind of stack (the chain itself stays off until switched on).
    pub fn defaults_for(kind: ModeKind) -> Self {
        let mut s = ChainSettings::default();
        let a = &mut s.control;
        let mut band = |hp: f32, lp: f32, steep: bool| {
            a.highpass = true;
            a.highpass_hz = hp;
            a.lowpass = true;
            a.lowpass_hz = lp;
            a.steep = steep;
        };
        match kind {
            ModeKind::AmAir => band(100.0, 5000.0, false),
            ModeKind::Nfm => band(300.0, 3000.0, false),
            ModeKind::Ssb => band(300.0, 2700.0, true),
            ModeKind::Cw => {
                band(400.0, 900.0, true);
                a.eq = EqPreset::Cw;
            }
            ModeKind::DigitalVoice => band(300.0, 3400.0, false),
            _ => {}
        }
        s.cleanup = Self::cleanup_preset(kind);
        s.cleanup.on = false;
        s
    }

    /// The cleanup preset used when the user switches the cleanup on for a kind of stack.
    pub fn cleanup_preset(kind: ModeKind) -> CleanupSettings {
        let mut c = CleanupSettings {
            on: true,
            ..Default::default()
        };
        let nr = |c: &mut CleanupSettings, s: f32| {
            c.nr = true;
            c.nr_strength = s;
        };
        match kind {
            ModeKind::Data | ModeKind::Tv | ModeKind::Spectrum => {
                c.on = false;
            }
            ModeKind::Generic => {
                nr(&mut c, 0.5);
                c.auto_notch = true;
                c.leveller = true;
                c.leveller_amount = 0.6;
            }
            ModeKind::AmAir => {
                nr(&mut c, 0.5);
                c.auto_notch = true;
                c.leveller = true;
                c.leveller_amount = 0.8;
                c.gate = true;
                c.gate_offset_db = 10.0;
                c.voice = true;
                c.voice_amount = 0.3;
            }
            ModeKind::Nfm => {
                nr(&mut c, 0.6);
                c.auto_notch = true;
                c.leveller = true;
                c.leveller_amount = 0.7;
                c.gate = true;
                c.gate_offset_db = 9.0;
                c.voice = true;
                c.voice_amount = 0.5;
                c.deesser = true;
                c.deesser_amount = 0.4;
            }
            ModeKind::WfmBroadcast => {
                c.leveller = true;
                c.leveller_amount = 0.4;
                c.leveller_target_db = -18.0;
                c.leveller_hold = false;
            }
            ModeKind::Ssb => {
                nr(&mut c, 0.7);
                c.auto_notch = true;
                c.leveller = true;
                c.leveller_amount = 0.8;
                c.voice = true;
                c.voice_amount = 0.4;
            }
            ModeKind::Cw => {
                nr(&mut c, 0.3);
                c.leveller = true;
                c.leveller_amount = 0.5;
            }
            ModeKind::DigitalVoice => {
                c.leveller = true;
                c.leveller_amount = 0.5;
            }
        }
        c
    }
}

/// The output device to use: `wanted` when it is among `available`, else the system default ("").
pub fn resolve_device<'a>(wanted: &'a str, available: &[&str]) -> &'a str {
    if wanted.is_empty() || available.contains(&wanted) {
        wanted
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_back_into_the_same_settings() {
        let mut s = ChainSettings::defaults_for(ModeKind::Ssb);
        s.control.on = true;
        s.control.eq = EqPreset::Custom;
        s.control.eq_gains = [1.5, -2.0, 3.0, 0.0, 6.0];
        s.control.device = "USB Audio".into();
        s.control.squelch = ChainSquelch::Snr;
        s.cleanup.nr_strength = 0.25;
        assert_eq!(ChainSettings::from_values(&s.values()), Ok(s));
        assert_eq!(resolve_device("Gone", &["Speaker"]), "");
        assert_eq!(resolve_device("Speaker", &["Speaker"]), "Speaker");
    }

    #[test]
    fn the_defaults_of_the_schema_are_the_defaults_of_the_chain() {
        let v = Values::defaults(&ListenSetting::schema());
        assert_eq!(ChainSettings::from_values(&v), Ok(ChainSettings::default()));
    }

    #[test]
    fn values_the_chain_cannot_use_are_refused() {
        let mut s = ChainSettings::default();
        let id = ListenSetting::CwPitch.id();
        assert_eq!(
            s.apply(id, &Value::Hz(100.0)),
            Err(SettingError::NotPermitted { id })
        );
        let id = ListenSetting::Dcs.id();
        assert_eq!(
            s.apply(id, &Value::Text("9".into())),
            Err(SettingError::Malformed { id, at: None })
        );
        assert_eq!(parse_dcs("D23"), parse_dcs("023"));
    }
}

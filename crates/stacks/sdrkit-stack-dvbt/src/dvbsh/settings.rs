//! The settings of the DVB-SH stack, each once as a typed constant: the receiver's and those of the test signal (`gen_*`).

use super::ofdm::Width;
use super::tdm::{Modulation, RollOff, GOLD_MAX};
use crate::params::{Guard, Hierarchy, Mode};
use sdrkit_core::publish::field;
use sdrkit_core::Setting;
use sdrkit_proto_mpegts::layer::DECODE_VIDEO;

sdrkit_core::choice_enum! {
    /// The channel width to receive.
    #[derive(Default)]
    pub enum WidthChoice {
        /// Every width the sample rate allows, in turn.
        #[default]
        Auto = ("auto", "Automatic"),
        /// 5 MHz.
        W5 = ("5", "5 MHz"),
        /// 8 MHz.
        W8 = ("8", "8 MHz"),
        /// 7 MHz.
        W7 = ("7", "7 MHz"),
        /// 6 MHz.
        W6 = ("6", "6 MHz"),
        /// 1.7 MHz.
        W1_7 = ("1.7", "1.7 MHz"),
    }
}

impl WidthChoice {
    /// The width this choice fixes; `None` for automatic.
    pub fn width(self) -> Option<Width> {
        match self {
            WidthChoice::Auto => None,
            WidthChoice::W5 => Some(Width::W5),
            WidthChoice::W8 => Some(Width::W8),
            WidthChoice::W7 => Some(Width::W7),
            WidthChoice::W6 => Some(Width::W6),
            WidthChoice::W1_7 => Some(Width::W1_7),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The FFT size of the test signal.
    pub enum GenMode {
        /// 1K.
        K1 = ("1k", "1K"),
        /// 2K.
        K2 = ("2k", "2K"),
        /// 4K.
        K4 = ("4k", "4K"),
        /// 8K.
        K8 = ("8k", "8K"),
    }
}

impl GenMode {
    /// The mode.
    pub fn mode(self) -> Mode {
        match self {
            GenMode::K1 => Mode::K1,
            GenMode::K2 => Mode::K2,
            GenMode::K4 => Mode::K4,
            GenMode::K8 => Mode::K8,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The constellation of the OFDM test signal.
    pub enum GenConstellation {
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// 16-QAM.
        Qam16 = ("16qam", "16-QAM"),
    }
}

sdrkit_core::choice_enum! {
    /// A puncturing pattern (Punct_Pat_ID, in the order of the ids).
    pub enum Pattern {
        /// 1/5.
        P0 = ("1/5", "1/5"),
        /// 2/9.
        P1 = ("2/9", "2/9"),
        /// 1/4.
        P2 = ("1/4", "1/4"),
        /// 2/7.
        P3 = ("2/7", "2/7"),
        /// 1/3.
        P4 = ("1/3", "1/3"),
        /// 1/3 complementary.
        P5 = ("1/3c", "1/3 complementary"),
        /// 2/5.
        P6 = ("2/5", "2/5"),
        /// 2/5 complementary.
        P7 = ("2/5c", "2/5 complementary"),
        /// 1/2.
        P8 = ("1/2", "1/2"),
        /// 1/2 complementary.
        P9 = ("1/2c", "1/2 complementary"),
        /// 2/3.
        P10 = ("2/3", "2/3"),
        /// 2/3 complementary.
        P11 = ("2/3c", "2/3 complementary"),
    }
}

sdrkit_core::choice_enum! {
    /// The form of DVB-SH to receive.
    #[derive(Default)]
    pub enum WaveformChoice {
        /// The OFDM form on every usable width, then the TDM form, in turn.
        #[default]
        Auto = ("auto", "Automatic"),
        /// The OFDM form, SH-A.
        Ofdm = ("ofdm", "OFDM"),
        /// The TDM form, SH-B.
        Tdm = ("tdm", "TDM"),
    }
}

sdrkit_core::choice_enum! {
    /// The form of the test signal.
    #[derive(Default)]
    pub enum GenWaveform {
        /// The OFDM form.
        #[default]
        Ofdm = ("ofdm", "OFDM"),
        /// The TDM form in QPSK.
        TdmQpsk = ("tdm_qpsk", "TDM QPSK"),
        /// The TDM form in 8PSK.
        Tdm8psk = ("tdm_8psk", "TDM 8PSK"),
        /// The TDM form in 16APSK.
        Tdm16apsk = ("tdm_16apsk", "TDM 16APSK"),
    }
}

impl GenWaveform {
    /// The modulation of the TDM form; `None` for the OFDM form.
    pub fn modulation(self) -> Option<Modulation> {
        match self {
            GenWaveform::Ofdm => None,
            GenWaveform::TdmQpsk => Some(Modulation::Qpsk),
            GenWaveform::Tdm8psk => Some(Modulation::Psk8),
            GenWaveform::Tdm16apsk => Some(Modulation::Apsk16),
        }
    }
}

const TEST_SIGNAL: &str = "Test signal";

/// The channel width to receive.
pub const WIDTH: Setting<field::Choice<WidthChoice>> =
    Setting::choice("width", "Channel width", WidthChoice::Auto).advanced();
/// The form to receive.
pub const WAVEFORM: Setting<field::Choice<WaveformChoice>> =
    Setting::choice("waveform", "Waveform", WaveformChoice::Auto).advanced();
/// The code number of the slot scrambling of the TDM form.
pub const TDM_GOLD: Setting<field::I64> =
    Setting::new("tdm_gold", "TDM scrambling code number", "", 0)
        .range(0.0, GOLD_MAX as f64, 1.0)
        .advanced();
/// The symbol rate of the TDM form; 0 tries the rates of table 5.13.
pub const TDM_SYMBOL_RATE: Setting<field::F64> =
    Setting::new("tdm_symbol_rate", "TDM symbol rate", "sym/s", 0.0)
        .range(0.0, 8e6, 1.0)
        .advanced();
/// Decode the low-priority stream of hierarchical 16-QAM.
pub const LOW_PRIORITY: Setting<field::Bool> =
    Setting::new("low_priority", "Low-priority stream", "", false);
/// The memory limit of the time de-interleaver.
pub const MAX_INTERLEAVER_MB: Setting<field::F64> = Setting::new(
    "max_interleaver_mb",
    "Time de-interleaver memory limit",
    "MB",
    768.0,
)
.range(1.0, 65_536.0, 1.0)
.advanced();
/// The most turbo iterations per code word.
pub const ITERATIONS: Setting<field::I64> =
    Setting::new("turbo_iterations", "Turbo iterations", "", 8)
        .range(1.0, 32.0, 1.0)
        .advanced();

/// The channel width of the test signal.
pub const GEN_WIDTH: Setting<field::Choice<WidthChoice>> =
    Setting::choice("gen_width", "Test signal width", WidthChoice::W5)
        .section(TEST_SIGNAL)
        .advanced();
/// The form of the test signal.
pub const GEN_WAVEFORM: Setting<field::Choice<GenWaveform>> =
    Setting::choice("gen_waveform", "Test signal waveform", GenWaveform::Ofdm)
        .section(TEST_SIGNAL)
        .advanced();
/// The roll-off of the TDM test signal.
pub const GEN_ROLLOFF: Setting<field::Choice<RollOff>> =
    Setting::choice("gen_rolloff", "Test signal TDM roll-off", RollOff::R25)
        .section(TEST_SIGNAL)
        .advanced();
/// The FFT size of the test signal.
pub const GEN_MODE: Setting<field::Choice<GenMode>> =
    Setting::choice("gen_mode", "Test signal FFT size", GenMode::K2)
        .section(TEST_SIGNAL)
        .advanced();
/// The constellation of the test signal.
pub const GEN_CONSTELLATION: Setting<field::Choice<GenConstellation>> = Setting::choice(
    "gen_constellation",
    "Test signal constellation",
    GenConstellation::Qpsk,
)
.section(TEST_SIGNAL)
.advanced();
/// The hierarchy of the test signal (16-QAM only).
pub const GEN_HIERARCHY: Setting<field::Choice<Hierarchy>> =
    Setting::choice("gen_hierarchy", "Test signal hierarchy", Hierarchy::None)
        .section(TEST_SIGNAL)
        .advanced();
/// The guard interval of the test signal.
pub const GEN_GUARD: Setting<field::Choice<Guard>> =
    Setting::choice("gen_guard", "Test signal guard interval", Guard::G8)
        .section(TEST_SIGNAL)
        .advanced();
/// The puncturing pattern of the test signal's high-priority stream.
pub const GEN_RATE: Setting<field::Choice<Pattern>> =
    Setting::choice("gen_rate", "Test signal code rate", Pattern::P4)
        .section(TEST_SIGNAL)
        .advanced();
/// The puncturing pattern of the test signal's low-priority stream.
pub const GEN_RATE_LP: Setting<field::Choice<Pattern>> =
    Setting::choice("gen_rate_lp", "Test signal LP code rate", Pattern::P9)
        .section(TEST_SIGNAL)
        .advanced();
/// The interleaver of the test signal: common_multiplier.
pub const GEN_TI_CM: Setting<field::I64> =
    Setting::new("gen_ti_cm", "Test signal common_multiplier", "", 4)
        .range(1.0, 63.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The interleaver of the test signal: nof_late_taps.
pub const GEN_TI_LT: Setting<field::I64> =
    Setting::new("gen_ti_lt", "Test signal nof_late_taps", "", 48)
        .range(0.0, 48.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The interleaver of the test signal: nof_slices.
pub const GEN_TI_NS: Setting<field::I64> =
    Setting::new("gen_ti_ns", "Test signal nof_slices", "", 1)
        .range(1.0, 63.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The interleaver of the test signal: slice_distance.
pub const GEN_TI_SD: Setting<field::I64> =
    Setting::new("gen_ti_sd", "Test signal slice_distance", "", 0)
        .range(0.0, 255.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The interleaver of the test signal: non_late_increment.
pub const GEN_TI_NLI: Setting<field::I64> =
    Setting::new("gen_ti_nli", "Test signal non_late_increment", "", 0)
        .range(0.0, 63.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the DVB-SH stack.
    pub enum DvbshSetting {
        /// The channel width.
        Width = WIDTH,
        /// The low-priority stream.
        LowPriority = LOW_PRIORITY,
        /// The de-interleaver memory limit.
        MaxInterleaverMb = MAX_INTERLEAVER_MB,
        /// The turbo iterations.
        Iterations = ITERATIONS,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The test signal's width.
        GenWidth = GEN_WIDTH,
        /// The test signal's FFT size.
        GenMode = GEN_MODE,
        /// The test signal's constellation.
        GenConstellation = GEN_CONSTELLATION,
        /// The test signal's hierarchy.
        GenHierarchy = GEN_HIERARCHY,
        /// The test signal's guard.
        GenGuard = GEN_GUARD,
        /// The test signal's HP pattern.
        GenRate = GEN_RATE,
        /// The test signal's LP pattern.
        GenRateLp = GEN_RATE_LP,
        /// The test signal's common_multiplier.
        GenTiCm = GEN_TI_CM,
        /// The test signal's nof_late_taps.
        GenTiLt = GEN_TI_LT,
        /// The test signal's nof_slices.
        GenTiNs = GEN_TI_NS,
        /// The test signal's slice_distance.
        GenTiSd = GEN_TI_SD,
        /// The test signal's non_late_increment.
        GenTiNli = GEN_TI_NLI,
        /// The form to receive.
        Waveform = WAVEFORM,
        /// The scrambling code of the TDM form.
        TdmGold = TDM_GOLD,
        /// The symbol rate of the TDM form.
        TdmSymbolRate = TDM_SYMBOL_RATE,
        /// The test signal's form.
        GenWaveform = GEN_WAVEFORM,
        /// The test signal's TDM roll-off.
        GenRolloff = GEN_ROLLOFF,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbsh.input`.
    pub enum InputSetting {
        /// The channel width.
        Width = WIDTH,
        /// The form to receive.
        Waveform = WAVEFORM,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbsh.tdm_front`.
    pub enum TdmFrontSetting {
        /// The channel width.
        Width = WIDTH,
        /// The symbol rate.
        SymbolRate = TDM_SYMBOL_RATE,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbsh.tdm`.
    pub enum TdmSetting {
        /// The scrambling code.
        Gold = TDM_GOLD,
        /// The de-interleaver memory limit.
        MaxInterleaverMb = MAX_INTERLEAVER_MB,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbsh.data`.
    pub enum DataSetting {
        /// The low-priority stream.
        LowPriority = LOW_PRIORITY,
        /// The de-interleaver memory limit.
        MaxInterleaverMb = MAX_INTERLEAVER_MB,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbsh.decode`.
    pub enum DecodeSetting {
        /// The turbo iterations.
        Iterations = ITERATIONS,
    }
}

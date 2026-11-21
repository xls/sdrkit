//! The settings of the two stacks, each once as a typed constant: the receiver settings, the test settings, the service
//! choices, the settings of the test signal (`gen_*`) and those of the sender of `dvbt` (`tx_*`). The framework adds the
//! standard impairments.

use crate::inner::ViterbiChoice;
use crate::params::{Bandwidth, CodeRate, Constellation, Guard, Hierarchy, Mode};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, Setting, Value};

sdrkit_core::choice_enum! {
    /// The channel bandwidth choice.
    #[derive(Default)]
    pub enum BandwidthChoice {
        /// Measured from the spectrum.
        #[default]
        Auto = ("auto", "Auto"),
        /// 8 MHz.
        B8 = ("8", "8 MHz"),
        /// 7 MHz.
        B7 = ("7", "7 MHz"),
        /// 6 MHz.
        B6 = ("6", "6 MHz"),
        /// 5 MHz.
        B5 = ("5", "5 MHz"),
    }
}

impl BandwidthChoice {
    /// The width this choice fixes. `None` for [`BandwidthChoice::Auto`].
    pub fn bandwidth(self) -> Option<Bandwidth> {
        match self {
            BandwidthChoice::Auto => None,
            BandwidthChoice::B8 => Some(Bandwidth::B8),
            BandwidthChoice::B7 => Some(Bandwidth::B7),
            BandwidthChoice::B6 => Some(Bandwidth::B6),
            BandwidthChoice::B5 => Some(Bandwidth::B5),
        }
    }

    /// The choice that fixes `b`.
    pub fn of(b: Bandwidth) -> BandwidthChoice {
        match b {
            Bandwidth::B8 => BandwidthChoice::B8,
            Bandwidth::B7 => BandwidthChoice::B7,
            Bandwidth::B6 => BandwidthChoice::B6,
            Bandwidth::B5 => BandwidthChoice::B5,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The stream of a hierarchical multiplex that is decoded.
    #[derive(Default)]
    pub enum StreamChoice {
        /// The high-priority stream.
        #[default]
        Hp = ("hp", "High priority"),
        /// The low-priority stream.
        Lp = ("lp", "Low priority"),
    }
}

sdrkit_core::choice_enum! {
    /// Which standards the host tries on a frequency.
    pub enum Standard {
        /// DVB-T2 and DVB-T in turn.
        Auto = ("auto", "Auto (DVB-T2 / DVB-T)"),
        /// DVB-T only.
        Dvbt = ("dvbt", "DVB-T only"),
    }
}

sdrkit_core::choice_enum! {
    /// The TPS length indicator of the test signal.
    pub enum TpsLength {
        /// 23 (no cell id).
        L23 = ("23", "23"),
        /// 31 (with cell id).
        L31 = ("31", "31"),
    }
}

impl TpsLength {
    /// The indicator value.
    pub fn value(self) -> u8 {
        match self {
            TpsLength::L23 => 23,
            TpsLength::L31 => 31,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The transport packets of the test signal.
    pub enum GenSource {
        /// A small multiplex with one service.
        Builtin = ("builtin", "Built-in multiplex"),
        /// Counted test packets.
        Pattern = ("pattern", "Counted test pattern"),
        /// Null packets.
        Null = ("null", "Null packets"),
    }
}

const SERVICES: &str = "Services";
const TESTS: &str = "Tests";
const TEST_SIGNAL: &str = "Test signal";

/// The channel bandwidth.
pub const BANDWIDTH: Setting<field::Choice<BandwidthChoice>> =
    Setting::choice("bandwidth", "Channel bandwidth", BandwidthChoice::Auto)
        .help("The width of the channel. Auto measures the width from the spectrum, then tries the other widths when nothing locks.")
        .restart()
        .sets_rate(width_rate);

/// The sample rate for a value of the bandwidth setting: the OFDM rate of the width, at which the input stage does not
/// resample. 0 for Auto: the rate of the table then counts, which is that of the widest channel.
fn width_rate(width: &Value) -> f64 {
    let Value::Choice(i) = width else { return 0.0 };
    BandwidthChoice::from_index(*i)
        .and_then(BandwidthChoice::bandwidth)
        .map_or(0.0, Bandwidth::natural_rate)
}
/// The stream of a hierarchical multiplex.
pub const STREAM: Setting<field::Choice<StreamChoice>> =
    Setting::choice("stream", "Hierarchical stream", StreamChoice::Hp)
        .help("Which stream of a hierarchical multiplex is decoded. A multiplex that is not hierarchical has only one.");
/// The Viterbi decoder.
pub const VITERBI: Setting<field::Choice<ViterbiChoice>> =
    Setting::choice("viterbi", "Viterbi decoder", ViterbiChoice::Auto);
/// Time interpolation of the pilots.
pub const TIME_INTERP: Setting<field::Bool> =
    Setting::new("time_interp", "Time interpolation of pilots", "", true)
        .help("Off uses the newest pilot: lower latency, worse with Doppler.");
/// Narrowband interferer weighting.
pub const INTERFERER_ERASE: Setting<field::Bool> = Setting::new(
    "interferer_erase",
    "Narrowband interferer weighting",
    "",
    true,
);
/// The standards the host tries (DVB-T stack).
pub const STANDARD: Setting<field::Choice<Standard>> =
    Setting::choice("standard", "Standard", Standard::Dvbt).restart();
/// The standards the host tries (DVB-T2 / DVB-T stack).
pub const STANDARD_AUTO: Setting<field::Choice<Standard>> =
    Setting::choice("standard", "Standard", Standard::Auto).restart();
/// The most entries of [`CHANNELS`].
pub const CHANNELS_MAX: usize = 256;
/// The channel list.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "").help(
    "The centre frequencies in Hz of the channels in use, separated by \";\". At most 256 entries. A radio tuned to \
     none of them asks the host to tune to the lowest. Empty: no list.",
);
/// The preferred audio language.
pub const AUDIO_LANG: Setting<field::Text> =
    Setting::new("audio_lang", "Audio language", "", "").section(SERVICES);
/// Audio description first.
pub const AUDIO_DESCRIPTION: Setting<field::Bool> =
    Setting::new("audio_description", "Audio description", "", false).section(SERVICES);
/// Sound of more than two channels mixed down to stereo.
pub const DOWNMIX: Setting<field::Bool> = Setting::new("downmix", "Stereo downmix", "", true)
    .section(SERVICES)
    .help("Off: 5.1 sound leaves with all its channels, in the order that the audio information states.");
/// MPEG-2 video decoded to frames of raw pixels.
pub const DECODE_VIDEO: Setting<field::Bool> =
    sdrkit_proto_mpegts::layer::DECODE_VIDEO.section(SERVICES);
/// The preferred subtitle language.
pub const SUBTITLE_LANG: Setting<field::Text> =
    Setting::new("subtitle_lang", "Subtitle language", "", "")
        .help("Empty: subtitles off.")
        .section(SERVICES);
/// Subtitles for the hard of hearing first.
pub const HARD_OF_HEARING: Setting<field::Bool> = Setting::new(
    "hard_of_hearing",
    "Subtitles for the hard of hearing",
    "",
    false,
)
.section(SERVICES);
/// The country for channel numbers and the local time.
pub const COUNTRY: Setting<field::Text> = Setting::new("country", "Country", "", "")
    .help("ISO 3166 alpha-3 code (NOR, AUS), for channel numbers and the local time.")
    .section(SERVICES);
/// The guide language.
pub const GUIDE_LANG: Setting<field::Text> =
    Setting::new("guide_lang", "Guide language", "", "").section(SERVICES);
/// Test: flat noise weighting.
pub const TEST_FLAT_CSI: Setting<field::Bool> =
    Setting::new("test_flat_csi", "Flat noise weighting", "", false).section(TESTS);
/// Test: the full delay window always.
pub const TEST_FULL_WINDOW: Setting<field::Bool> =
    Setting::new("test_full_window", "Full delay window", "", false).section(TESTS);
/// Test: the window from the correlation peak only.
pub const TEST_PEAK_ONLY: Setting<field::Bool> = Setting::new(
    "test_peak_only",
    "Window from the correlation peak",
    "",
    false,
)
.section(TESTS);
/// The FFT mode of the test signal.
pub const GEN_MODE: Setting<field::Choice<Mode>> = Setting::choice("gen_mode", "Mode", Mode::K2)
    .section(TEST_SIGNAL)
    .advanced();
/// The guard interval of the test signal.
pub const GEN_GUARD: Setting<field::Choice<Guard>> =
    Setting::choice("gen_guard", "Guard interval", Guard::G8)
        .section(TEST_SIGNAL)
        .advanced();
/// The constellation of the test signal.
pub const GEN_CONSTELLATION: Setting<field::Choice<Constellation>> =
    Setting::choice("gen_constellation", "Constellation", Constellation::Qam16)
        .section(TEST_SIGNAL)
        .advanced();
/// The hierarchy of the test signal.
pub const GEN_HIERARCHY: Setting<field::Choice<Hierarchy>> =
    Setting::choice("gen_hierarchy", "Hierarchy", Hierarchy::None)
        .section(TEST_SIGNAL)
        .advanced();
/// The HP code rate of the test signal.
pub const GEN_RATE_HP: Setting<field::Choice<CodeRate>> =
    Setting::choice("gen_rate_hp", "HP code rate", CodeRate::R23)
        .section(TEST_SIGNAL)
        .advanced();
/// The LP code rate of the test signal.
pub const GEN_RATE_LP: Setting<field::Choice<CodeRate>> =
    Setting::choice("gen_rate_lp", "LP code rate", CodeRate::R23)
        .section(TEST_SIGNAL)
        .advanced();
/// The bandwidth of the test signal. Auto is the channel bandwidth of the stack, and 8 MHz if that is Auto too.
pub const GEN_BANDWIDTH: Setting<field::Choice<BandwidthChoice>> =
    Setting::choice("gen_bandwidth", "Bandwidth", BandwidthChoice::Auto)
        .section(TEST_SIGNAL)
        .advanced();
/// The cell id of the test signal.
pub const GEN_CELL_ID: Setting<field::I64> = Setting::new("gen_cell_id", "Cell id", "", 0)
    .range(0.0, 65535.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// The TPS length indicator of the test signal.
pub const GEN_LENGTH: Setting<field::Choice<TpsLength>> =
    Setting::choice("gen_length", "TPS length indicator", TpsLength::L23)
        .section(TEST_SIGNAL)
        .advanced();
/// The puncturing start phase of the test signal.
pub const GEN_PHASE: Setting<field::I64> =
    Setting::new("gen_phase", "Puncturing start phase", "", 0)
        .range(0.0, 7.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The service id of the test multiplex.
pub const GEN_SERVICE: Setting<field::I64> = Setting::new("gen_service", "Service id", "", 1)
    .range(1.0, 65535.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// The service name of the test multiplex.
pub const GEN_NAME: Setting<field::Text> =
    Setting::new("gen_name", "Service name", "", "Test service")
        .section(TEST_SIGNAL)
        .advanced();
/// The transport packets of the test signal.
pub const GEN_SOURCE: Setting<field::Choice<GenSource>> =
    Setting::choice("gen_source", "Packets", GenSource::Builtin)
        .section(TEST_SIGNAL)
        .advanced();

const TRANSMIT: &str = "Transmit";

/// The channel width of the sender.
pub const TX_BANDWIDTH: Setting<field::Choice<Bandwidth>> =
    Setting::choice("tx_bandwidth", "Channel width", Bandwidth::B8)
        .help("The width of the channel sent: 8, 7, 6 or 5 MHz. It sets the elementary rate of the signal.")
        .section(TRANSMIT);
/// The FFT mode of the sender.
pub const TX_MODE: Setting<field::Choice<Mode>> = Setting::choice("tx_mode", "FFT mode", Mode::K8)
    .help("2K or 8K carriers.")
    .section(TRANSMIT);
/// The guard interval of the sender.
pub const TX_GUARD: Setting<field::Choice<Guard>> =
    Setting::choice("tx_guard", "Guard interval", Guard::G8).section(TRANSMIT);
/// The constellation of the sender.
pub const TX_CONSTELLATION: Setting<field::Choice<Constellation>> =
    Setting::choice("tx_constellation", "Constellation", Constellation::Qam64).section(TRANSMIT);
/// The hierarchy of the sender.
pub const TX_HIERARCHY: Setting<field::Choice<Hierarchy>> =
    Setting::choice("tx_hierarchy", "Hierarchy", Hierarchy::None)
        .help("None, or a hierarchical multiplex with the factor α: the low-priority stream then takes the packets of append_lp.")
        .section(TRANSMIT);
/// The code rate of the high-priority stream of the sender.
pub const TX_RATE_HP: Setting<field::Choice<CodeRate>> =
    Setting::choice("tx_rate_hp", "Code rate", CodeRate::R23)
        .help("The code rate of the stream, or of the high-priority stream of a hierarchical multiplex.")
        .section(TRANSMIT);
/// The code rate of the low-priority stream of the sender.
pub const TX_RATE_LP: Setting<field::Choice<CodeRate>> =
    Setting::choice("tx_rate_lp", "Code rate (LP)", CodeRate::R23)
        .help("The code rate of the low-priority stream of a hierarchical multiplex. TPS carries it in every multiplex.")
        .section(TRANSMIT);
/// The cell id of the sender.
pub const TX_CELL_ID: Setting<field::I64> = Setting::new("tx_cell_id", "Cell id", "", 0)
    .range(0.0, 65535.0, 1.0)
    .help("Sent in TPS with the length indicator 31. 0: no cell id, length indicator 23.")
    .section(TRANSMIT);

sdrkit_core::settings! {
    /// The settings of the DVB-T stack.
    pub enum DvbtSetting {
        /// The channel bandwidth.
        Bandwidth = BANDWIDTH,
        /// The stream of a hierarchical multiplex.
        Stream = STREAM,
        /// The Viterbi decoder.
        Viterbi = VITERBI,
        /// Time interpolation of the pilots.
        TimeInterp = TIME_INTERP,
        /// Narrowband interferer weighting.
        InterfererErase = INTERFERER_ERASE,
        /// The standards the host tries.
        Standard = STANDARD,
        /// The channel list.
        Channels = CHANNELS,
        /// The preferred audio language.
        AudioLang = AUDIO_LANG,
        /// Audio description first.
        AudioDescription = AUDIO_DESCRIPTION,
        /// Sound of more than two channels mixed down to stereo.
        Downmix = DOWNMIX,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The preferred subtitle language.
        SubtitleLang = SUBTITLE_LANG,
        /// Subtitles for the hard of hearing first.
        HardOfHearing = HARD_OF_HEARING,
        /// The country of the service information.
        Country = COUNTRY,
        /// The guide language.
        GuideLang = GUIDE_LANG,
        /// Test: flat noise weighting.
        TestFlatCsi = TEST_FLAT_CSI,
        /// Test: the full delay window.
        TestFullWindow = TEST_FULL_WINDOW,
        /// Test: the window from the correlation peak.
        TestPeakOnly = TEST_PEAK_ONLY,
        /// The FFT mode of the test signal.
        GenMode = GEN_MODE,
        /// The guard interval of the test signal.
        GenGuard = GEN_GUARD,
        /// The constellation of the test signal.
        GenConstellation = GEN_CONSTELLATION,
        /// The hierarchy of the test signal.
        GenHierarchy = GEN_HIERARCHY,
        /// The HP code rate of the test signal.
        GenRateHp = GEN_RATE_HP,
        /// The LP code rate of the test signal.
        GenRateLp = GEN_RATE_LP,
        /// The bandwidth of the test signal.
        GenBandwidth = GEN_BANDWIDTH,
        /// The cell id of the test signal.
        GenCellId = GEN_CELL_ID,
        /// The TPS length indicator of the test signal.
        GenLength = GEN_LENGTH,
        /// The puncturing phase of the test signal.
        GenPhase = GEN_PHASE,
        /// The service id of the test multiplex.
        GenService = GEN_SERVICE,
        /// The service name of the test multiplex.
        GenName = GEN_NAME,
        /// The packets of the test signal.
        GenSource = GEN_SOURCE,
        /// The channel width of the sender.
        TxBandwidth = TX_BANDWIDTH,
        /// The FFT mode of the sender.
        TxMode = TX_MODE,
        /// The guard interval of the sender.
        TxGuard = TX_GUARD,
        /// The constellation of the sender.
        TxConstellation = TX_CONSTELLATION,
        /// The hierarchy of the sender.
        TxHierarchy = TX_HIERARCHY,
        /// The code rate of the sender.
        TxRateHp = TX_RATE_HP,
        /// The LP code rate of the sender.
        TxRateLp = TX_RATE_LP,
        /// The cell id of the sender.
        TxCellId = TX_CELL_ID,
    }
}

sdrkit_core::settings! {
    /// The settings of the DVB-T2 / DVB-T stack: those of DVB-T, with both standards tried.
    pub enum TerrestrialSetting {
        /// The channel bandwidth.
        Bandwidth = BANDWIDTH,
        /// The stream of a hierarchical multiplex.
        Stream = STREAM,
        /// The Viterbi decoder.
        Viterbi = VITERBI,
        /// Time interpolation of the pilots.
        TimeInterp = TIME_INTERP,
        /// Narrowband interferer weighting.
        InterfererErase = INTERFERER_ERASE,
        /// The standards the host tries.
        Standard = STANDARD_AUTO,
        /// The channel list.
        Channels = CHANNELS,
        /// The preferred audio language.
        AudioLang = AUDIO_LANG,
        /// Audio description first.
        AudioDescription = AUDIO_DESCRIPTION,
        /// Sound of more than two channels mixed down to stereo.
        Downmix = DOWNMIX,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The preferred subtitle language.
        SubtitleLang = SUBTITLE_LANG,
        /// Subtitles for the hard of hearing first.
        HardOfHearing = HARD_OF_HEARING,
        /// The country of the service information.
        Country = COUNTRY,
        /// The guide language.
        GuideLang = GUIDE_LANG,
        /// Test: flat noise weighting.
        TestFlatCsi = TEST_FLAT_CSI,
        /// Test: the full delay window.
        TestFullWindow = TEST_FULL_WINDOW,
        /// Test: the window from the correlation peak.
        TestPeakOnly = TEST_PEAK_ONLY,
        /// The FFT mode of the test signal.
        GenMode = GEN_MODE,
        /// The guard interval of the test signal.
        GenGuard = GEN_GUARD,
        /// The constellation of the test signal.
        GenConstellation = GEN_CONSTELLATION,
        /// The hierarchy of the test signal.
        GenHierarchy = GEN_HIERARCHY,
        /// The HP code rate of the test signal.
        GenRateHp = GEN_RATE_HP,
        /// The LP code rate of the test signal.
        GenRateLp = GEN_RATE_LP,
        /// The bandwidth of the test signal.
        GenBandwidth = GEN_BANDWIDTH,
        /// The cell id of the test signal.
        GenCellId = GEN_CELL_ID,
        /// The TPS length indicator of the test signal.
        GenLength = GEN_LENGTH,
        /// The puncturing phase of the test signal.
        GenPhase = GEN_PHASE,
        /// The service id of the test multiplex.
        GenService = GEN_SERVICE,
        /// The service name of the test multiplex.
        GenName = GEN_NAME,
        /// The packets of the test signal.
        GenSource = GEN_SOURCE,
    }
}

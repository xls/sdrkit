//! The settings of the DAB stack: each setting once as a typed constant, the choices, and [`DabSetting`], the stack's schema.
//!
//! The settings of the "Test signal" section (`gen_*`) configure the generator only. The framework adds the standard
//! impairments.

use sdrkit_core::publish::field;
use sdrkit_core::Setting;

sdrkit_core::choice_enum! {
    /// How the receiver detects the carriers.
    #[derive(Default)]
    pub enum DetectionChoice {
        /// The channel estimate when it is reliable, else differential detection.
        #[default]
        Auto = ("auto", "Automatic"),
        /// Plain differential detection.
        Differential = ("differential", "Differential"),
    }
}

sdrkit_core::choice_enum! {
    /// How DAB+ audio leaves the graph for the host's decoder.
    #[derive(Default)]
    pub enum AacPackaging {
        /// Raw access units with an AudioSpecificConfig.
        #[default]
        Raw = ("raw", "Raw access units"),
        /// LATM (ISO/IEC 14496-3 clause 1.7).
        Latm = ("latm", "LATM"),
    }
}

/// A further sub-channel to decode for data.
pub const DATA_SUBCHANNEL: Setting<field::I64> =
    Setting::new("data_subchannel", "Data sub-channel", "", -1)
        .range(-1.0, 63.0, 1.0)
        .help("A further sub-channel to decode for data (packet mode SPI or Journaline, DMB video). −1 decodes none.")
        .section("Data");
/// The detection.
pub const DETECTION: Setting<field::Choice<DetectionChoice>> =
    Setting::choice("detection", "Detection", DetectionChoice::Auto)
        .help("Automatic uses the channel estimate when it is reliable. Differential forces plain differential detection.");
/// Centring of an ensemble found off centre.
pub const CENTRING: Setting<field::Bool> =
    Setting::new("centring", "Centre the ensemble", "", true)
        .help("Moves an ensemble found off centre to the middle before the stack locks.");
/// The impulse blanker.
pub const IMPULSE_BLANKER: Setting<field::Bool> =
    Setting::new("impulse_blanker", "Impulse blanker", "", true)
        .help("Sets samples far above the running mean power to zero.");
/// The early start of DAB+.
pub const EARLY_START: Setting<field::Bool> = Setting::new("early_start", "Early start", "", true)
    .help("DAB+ decodes its first frames from 10 CIFs instead of 16.");
/// The packaging of DAB+ audio.
pub const AAC_PACKAGING: Setting<field::Choice<AacPackaging>> =
    Setting::choice("aac_packaging", "AAC packaging", AacPackaging::Raw)
        .help("How DAB+ audio is given to the host's decoder.")
        .restart();
/// The channels that service following may tune to.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "").help(
    "The centre frequencies in Hz of the channels that service following may tune to, separated by \";\". At most 128 \
     entries. A frequency within 50 kHz of a block of EN 300 401 takes the name of the block. Empty: every block of \
     Band III and L-band.",
);
/// Transmitter identification.
pub const TII: Setting<field::Bool> = Setting::new("tii", "Transmitter identification", "", true)
    .help("Analyses the null symbols for transmitter identifiers.")
    .section("Data");
/// The announcement types that apply.
pub const ANNOUNCEMENT_TYPES: Setting<field::I64> = Setting::new(
    "announcement_types",
    "Announcement types",
    "",
    63,
)
.range(0.0, 65535.0, 1.0)
.help("The announcement types (a bit mask, TS 101 756 table 14) that apply to the service playing.")
.section("Announcements");
/// Test signal: three transmitters with identification.
pub const GEN_TII: Setting<field::Bool> =
    Setting::new("gen_tii", "Transmitter identification", "", false)
        .help("Three transmitters (main 1, sub 1, 3 and 7 at 0, −5 and −11 dB).")
        .section("Test signal")
        .advanced();
/// Test signal: labels as FIG 2.
pub const GEN_FIG2: Setting<field::Bool> = Setting::new("gen_fig2", "Extended labels", "", false)
    .help("Labels as FIG 2 (UTF-8).")
    .section("Test signal")
    .advanced();
/// Test signal: the programme type.
pub const GEN_PTY: Setting<field::I64> = Setting::new("gen_pty", "Programme type", "", 0)
    .range(0.0, 31.0, 1.0)
    .help("0 for none.")
    .section("Test signal")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the DAB stack.
    pub enum DabSetting {
        /// The detection.
        Detection = DETECTION,
        /// Centring.
        Centring = CENTRING,
        /// The impulse blanker.
        ImpulseBlanker = IMPULSE_BLANKER,
        /// The early start of DAB+.
        EarlyStart = EARLY_START,
        /// The packaging of DAB+ audio.
        AacPackaging = AAC_PACKAGING,
        /// The channels of service following.
        Channels = CHANNELS,
        /// The data sub-channel.
        DataSubchannel = DATA_SUBCHANNEL,
        /// Transmitter identification.
        Tii = TII,
        /// The announcement types.
        AnnouncementTypes = ANNOUNCEMENT_TYPES,
        /// Test signal: transmitter identification.
        GenTii = GEN_TII,
        /// Test signal: labels as FIG 2.
        GenFig2 = GEN_FIG2,
        /// Test signal: the programme type.
        GenPty = GEN_PTY,
    }
}

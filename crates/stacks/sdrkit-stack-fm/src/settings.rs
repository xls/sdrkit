//! The settings of the FM stack: each setting once as a typed constant, the choices, and [`FmSetting`], the stack's schema.
//!
//! The settings of the "Test signal" section (`gen_*`) configure the generator only. The framework adds the standard
//! impairments. The settings of the "Transmit" section (`tx_*`) configure the sender ([`crate::transmit`]).

use crate::pty::Variant;
use crate::rds::Correction;
use sdrkit_core::publish::field;
use sdrkit_core::Setting;
use sdrkit_util_text::ebu::RdsCharset;

sdrkit_core::choice_enum! {
    /// The stereo choice.
    #[derive(Default)]
    pub enum StereoChoice {
        /// Stereo while the pilot is locked, with the SNR blend.
        #[default]
        Auto = ("auto", "Auto"),
        /// Mono sound.
        Mono = ("mono", "Mono"),
        /// Full stereo whenever any pilot is there, without the blend.
        Stereo = ("stereo", "Stereo"),
    }
}

sdrkit_core::choice_enum! {
    /// The de-emphasis time constant.
    /// Standard: ITU-R BS.450 (pre-emphasis of 50 or 75 µs).
    #[derive(Default)]
    pub enum Deemphasis {
        /// No de-emphasis.
        Off = ("off", "Off"),
        /// 50 µs: Europe, Asia, Africa and Australia.
        #[default]
        Us50 = ("50", "50 µs"),
        /// 75 µs: the Americas and South Korea.
        Us75 = ("75", "75 µs"),
    }
}

impl Deemphasis {
    /// The time constant in seconds (0 for off).
    pub fn tau_s(self) -> f64 {
        match self {
            Deemphasis::Off => 0.0,
            Deemphasis::Us50 => 50e-6,
            Deemphasis::Us75 => 75e-6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The pre-emphasis of the test signal.
    #[derive(Default)]
    pub enum Preemphasis {
        /// No pre-emphasis.
        None = ("0", "None"),
        /// 50 µs.
        #[default]
        Us50 = ("50", "50 µs"),
        /// 75 µs.
        Us75 = ("75", "75 µs"),
    }
}

impl Preemphasis {
    /// The time constant in seconds (0 for none).
    pub fn tau_s(self) -> f64 {
        match self {
            Preemphasis::None => 0.0,
            Preemphasis::Us50 => 50e-6,
            Preemphasis::Us75 => 75e-6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The channel filter choice.
    #[derive(Default)]
    pub enum ChannelFilterChoice {
        /// The standard 220 kHz, or the channel width setting when it is not 0.
        #[default]
        Standard = ("standard", "Standard"),
        /// 300 kHz, for strong stations with wide deviation.
        Wide = ("wide", "Wide"),
        /// 150 kHz, to keep out a strong neighbour.
        Narrow = ("narrow", "Narrow"),
        /// The channel width setting.
        Custom = ("custom", "Custom"),
    }
}

impl ChannelFilterChoice {
    /// The channel width to use with the channel width setting `setting_hz` (0 is the standard 220 kHz).
    pub fn width_hz(self, setting_hz: f64) -> f64 {
        match self {
            ChannelFilterChoice::Standard | ChannelFilterChoice::Custom => setting_hz,
            ChannelFilterChoice::Wide => WIDE_WIDTH_HZ,
            ChannelFilterChoice::Narrow => NARROW_WIDTH_HZ,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The audio bandwidth.
    #[derive(Default)]
    pub enum AudioBandwidth {
        /// The full 15 kHz.
        #[default]
        Full = ("0", "Full"),
        /// 12 kHz.
        Khz12 = ("12000", "12 kHz"),
        /// 8 kHz.
        Khz8 = ("8000", "8 kHz"),
        /// 5 kHz.
        Khz5 = ("5000", "5 kHz"),
    }
}

impl AudioBandwidth {
    /// The low-pass corner in Hz (0 for full).
    pub fn hz(self) -> f64 {
        match self {
            AudioBandwidth::Full => 0.0,
            AudioBandwidth::Khz12 => 12e3,
            AudioBandwidth::Khz8 => 8e3,
            AudioBandwidth::Khz5 => 5e3,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How much the RDS block layer repairs.
    #[derive(Default)]
    pub enum RdsCorrection {
        /// No repair.
        Off = ("0", "Off"),
        /// Bursts of 1 or 2 bits.
        Bursts = ("1", "Short bursts"),
        /// Bursts and two weak symbols.
        #[default]
        Full = ("2", "Full"),
    }
}

impl RdsCorrection {
    /// The correction with voting `voting`.
    pub fn correction(self, voting: bool) -> Correction {
        Correction {
            level: self as u8,
            voting,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How RDS text is read.
    #[derive(Default)]
    pub enum RdsCharsetChoice {
        /// The RDS table.
        EbuLatin = ("0", "EBU Latin"),
        /// GB 2312 when the bytes look like it.
        #[default]
        Auto = ("1", "Auto GB2312"),
        /// GB 2312.
        Gb2312 = ("2", "GB2312"),
    }
}

impl RdsCharsetChoice {
    /// The character set.
    pub fn charset(self) -> RdsCharset {
        match self {
            RdsCharsetChoice::EbuLatin => RdsCharset::EbuLatin,
            RdsCharsetChoice::Auto => RdsCharset::Auto,
            RdsCharsetChoice::Gb2312 => RdsCharset::Gb2312,
        }
    }
}

/// Width of the "wide" channel filter.
pub const WIDE_WIDTH_HZ: f64 = 300e3;
/// Width of the "narrow" channel filter.
pub const NARROW_WIDTH_HZ: f64 = 150e3;

const SOUND: &str = "Sound";
const RDS: &str = "RDS";
const RECEPTION: &str = "Reception";
const TEST: &str = "Test signal";
const TX: &str = "Transmit";

/// Stereo or mono.
pub const STEREO: Setting<field::Choice<StereoChoice>> =
    Setting::choice("stereo", "Stereo", StereoChoice::Auto)
        .help("Auto plays stereo while the pilot tone is locked and moves towards mono as the signal gets weak. Mono always plays mono. Stereo plays full stereo whenever any pilot is there.")
        .section(SOUND);
/// Mono whatever the stereo choice.
pub const FORCE_MONO: Setting<field::Bool> = Setting::new("force_mono", "Force mono", "", false)
    .help("Plays mono whatever the stereo choice.")
    .section(SOUND)
    .advanced();
/// The de-emphasis.
pub const DEEMPHASIS: Setting<field::Choice<Deemphasis>> =
    Setting::choice("deemphasis", "De-emphasis", Deemphasis::Us50)
        .help("Undoes the treble boost stations add. 50 µs in Europe, Asia, Africa and Australia. 75 µs in the Americas and South Korea.")
        .section(SOUND)
    .advanced();
/// The squelch threshold on the SNR measure.
pub const SQUELCH: Setting<field::F64> = Setting::new("squelch", "Squelch", "dB", 0.0)
    .range(0.0, 60.0, 1.0)
    .zero_means("Off")
    .help("Mutes the sound while the signal-to-noise measure is below this.")
    .section(SOUND);
/// The audio bandwidth.
pub const AUDIO_BANDWIDTH: Setting<field::Choice<AudioBandwidth>> =
    Setting::choice("audio_bandwidth", "Audio bandwidth", AudioBandwidth::Full)
        .help("Cuts the treble above this frequency. A lower value hides hiss on a weak station.")
        .section(SOUND)
        .advanced();
/// The bass shelf.
pub const BASS: Setting<field::F64> = Setting::new("bass", "Bass", "dB", 0.0)
    .range(-6.0, 6.0, 0.5)
    .help("Boost or cut below 200 Hz.")
    .section(SOUND);
/// The treble shelf.
pub const TREBLE: Setting<field::F64> = Setting::new("treble", "Treble", "dB", 0.0)
    .range(-6.0, 6.0, 0.5)
    .help("Boost or cut above 6 kHz.")
    .section(SOUND);
/// 3 dB more off the treble.
pub const WARM: Setting<field::Bool> = Setting::new("warm", "Warm", "", false)
    .help("Takes 3 dB more off the treble, for a softer sound.")
    .section(SOUND)
    .advanced();
/// The channel width.
pub const CHANNEL_WIDTH: Setting<field::Hz> = Setting::new("channel_width", "Channel width", "Hz", 0.0)
    .range(0.0, 1_000_000.0, 1000.0)
    .zero_means("The standard 220 kHz")
    .help("The width of the channel filter. A narrower width keeps out a strong neighbour, at the cost of distortion on loud passages and a weaker RDS signal. At least 10 kHz.")
    .section(RECEPTION)
    .advanced();
/// The channel filter.
pub const CHANNEL_FILTER: Setting<field::Choice<ChannelFilterChoice>> =
    Setting::choice("channel_filter", "Channel filter", ChannelFilterChoice::Standard)
        .help("Standard is 220 kHz (or the channel width below when it is not 0). Wide is 300 kHz, for strong stations. Narrow is 150 kHz, to keep out a strong neighbour. Custom uses the channel width below.")
        .section(RECEPTION)
    .advanced();
/// The lowest frequency of the band.
pub const BAND_LOW_HZ: Setting<field::Hz> = Setting::new("band_low_hz", "Band start", "Hz", 87.5e6)
    .range(60e6, 110e6, 0.0)
    .help("The lowest frequency of the FM band.")
    .advanced();
/// The highest frequency of the band.
pub const BAND_HIGH_HZ: Setting<field::Hz> = Setting::new("band_high_hz", "Band end", "Hz", 108e6)
    .range(60e6, 110e6, 0.0)
    .help("The highest frequency of the FM band.")
    .advanced();
/// The first channel of the raster.
pub const FIRST_HZ: Setting<field::Hz> = Setting::new("first_hz", "First channel", "Hz", 87.5e6)
    .range(60e6, 110e6, 0.0)
    .help("The first channel of the band. The channels are this frequency plus a whole number of steps.")
    .advanced();
/// The channel step.
pub const RASTER_HZ: Setting<field::Hz> = Setting::new("raster_hz", "Channel step", "Hz", 100e3)
    .range(10e3, 1e6, 0.0)
    .help("The distance between two channels of the band.")
    .advanced();
/// Where the band scan starts.
pub const SCAN_LOW: Setting<field::F64> = Setting::new("scan_low", "Scan from", "MHz", 87.0)
    .range(76.0, 108.0, 0.1)
    .help("Where the band scan starts. The scan covers at least 0.5 MHz, within 76 to 108 MHz.")
    .section(RECEPTION)
    .advanced();
/// Where the band scan ends.
pub const SCAN_HIGH: Setting<field::F64> = Setting::new("scan_high", "Scan to", "MHz", 108.0)
    .range(76.0, 108.0, 0.1)
    .help("Where the band scan ends.")
    .section(RECEPTION)
    .advanced();
/// The band scan waits for the whole RDS.
pub const SCAN_FULL_RDS: Setting<field::Bool> = Setting::new("scan_full_rds", "Scan for full RDS", "", false)
    .help("The band scan stays on each station until its whole name, its radio text and its code have come in. Slower, but complete.")
    .section(RECEPTION)
    .advanced();
/// The station finder.
pub const STATION_FINDER: Setting<field::Bool> = Setting::new("station_finder", "Station finder", "", false)
    .help("For recordings and the test signal only: finds a station that is off the centre and tunes to it.")
    .section(RECEPTION)
    .advanced();
/// RDS or RBDS meanings.
pub const RDS_VARIANT: Setting<field::Choice<Variant>> =
    Setting::choice("rds_variant", "RDS meanings", Variant::Auto)
        .help("How programme types and station codes are read. Auto reads RDS if the station sends a country code outside the Americas, else RBDS on the channel raster of RBDS (odd tenths of a MHz from 87.9 to 107.9 MHz) and RDS elsewhere.")
        .section(RDS)
    .advanced();
/// The RDS correction.
pub const RDS_CORRECTION: Setting<field::Choice<RdsCorrection>> =
    Setting::choice("rds_correction", "RDS correction", RdsCorrection::Full)
        .help("How much the stack repairs damaged RDS data. Full gets the most from a weak station. A wrong character is still never shown.")
        .section(RDS)
    .advanced();
/// RDS voting.
pub const RDS_VOTING: Setting<field::Bool> = Setting::new("rds_voting", "RDS voting", "", true)
    .help("With full correction, also repairs data by comparing repeated copies.")
    .section(RDS)
    .advanced();
/// The RDS character set.
pub const RDS_CHARSET: Setting<field::Choice<RdsCharsetChoice>> = Setting::choice(
    "rds_charset",
    "RDS characters",
    RdsCharsetChoice::Auto,
)
.help("How station names and texts are read. Auto reads Chinese (GB2312) text when it sees it.")
.section(RDS)
.advanced();
/// The RDS group monitor (a component setting, not a stack setting).
pub const RDS_MONITOR: Setting<field::Bool> = Setting::new("monitor", "Group monitor", "", false)
    .help("Logs every RDS group.")
    .section(RDS);

/// The test signal in stereo.
pub const GEN_STEREO: Setting<field::Bool> = Setting::new("gen_stereo", "Stereo", "", true)
    .help("Sends a stereo signal with its pilot tone.")
    .section(TEST)
    .advanced();
/// The left tone.
pub const GEN_LEFT_HZ: Setting<field::Hz> = Setting::new("gen_left_hz", "Left tone", "Hz", 1000.0)
    .range(0.0, 15_000.0, 10.0)
    .section(TEST)
    .advanced();
/// The left level.
pub const GEN_LEFT_AMP: Setting<field::F64> = Setting::new("gen_left_amp", "Left level", "", 0.5)
    .range(0.0, 1.0, 0.01)
    .help("0 is silent.")
    .section(TEST)
    .advanced();
/// The right tone.
pub const GEN_RIGHT_HZ: Setting<field::Hz> =
    Setting::new("gen_right_hz", "Right tone", "Hz", 3000.0)
        .range(0.0, 15_000.0, 10.0)
        .section(TEST)
        .advanced();
/// The right level.
pub const GEN_RIGHT_AMP: Setting<field::F64> =
    Setting::new("gen_right_amp", "Right level", "", 0.5)
        .range(0.0, 1.0, 0.01)
        .help("0 is silent.")
        .section(TEST)
        .advanced();
/// The pilot injection.
pub const GEN_PILOT: Setting<field::F64> = Setting::new("gen_pilot", "Pilot", "%", 9.0)
    .range(0.0, 15.0, 0.1)
    .help("Pilot injection, as a percentage of 75 kHz.")
    .section(TEST)
    .advanced();
/// RDS in the test signal.
pub const GEN_RDS: Setting<field::Bool> = Setting::new("gen_rds", "RDS", "", true)
    .section(TEST)
    .advanced();
/// The RDS injection.
pub const GEN_RDS_KHZ: Setting<field::F64> =
    Setting::new("gen_rds_khz", "RDS injection", "kHz", 2.0)
        .range(0.0, 10.0, 0.1)
        .section(TEST)
        .advanced();
/// The station name sent.
pub const GEN_PS: Setting<field::Text> = Setting::new("gen_ps", "Station name", "", "SDRKIT  ")
    .help("Up to 8 characters.")
    .section(TEST)
    .advanced();
/// The RadioText sent.
pub const GEN_RT: Setting<field::Text> =
    Setting::new("gen_rt", "Radio text", "", "sdrkit FM test signal")
        .help("Up to 64 characters.")
        .section(TEST)
        .advanced();
/// The PI sent, four hexadecimal digits.
pub const GEN_PI: Setting<field::Text> = Setting::new("gen_pi", "PI code", "", "4A21")
    .help("Four hexadecimal digits.")
    .section(TEST)
    .advanced();
/// The programme type sent.
pub const GEN_PTY: Setting<field::I64> = Setting::new("gen_pty", "Programme type", "", 10)
    .range(0.0, 31.0, 1.0)
    .section(TEST)
    .advanced();
/// The traffic programme flag sent.
pub const GEN_TP: Setting<field::Bool> = Setting::new("gen_tp", "Traffic programme", "", true)
    .section(TEST)
    .advanced();
/// The traffic announcement flag sent.
pub const GEN_TA: Setting<field::Bool> = Setting::new("gen_ta", "Traffic announcement", "", false)
    .section(TEST)
    .advanced();
/// The music flag sent.
pub const GEN_MUSIC: Setting<field::Bool> = Setting::new("gen_music", "Music", "", true)
    .section(TEST)
    .advanced();
/// The pre-emphasis of the test signal.
pub const GEN_PREEMPHASIS: Setting<field::Choice<Preemphasis>> =
    Setting::choice("gen_preemphasis", "Pre-emphasis", Preemphasis::Us50)
        .section(TEST)
        .advanced();
/// More RDS groups to send.
pub const GEN_EXTRA_GROUPS: Setting<field::Text> = Setting::new(
    "gen_extra_groups",
    "Extra RDS groups",
    "",
    "",
)
.help(
    "Blocks B, C and D of groups to send as well, in hexadecimal, for example \"1000 00E2 B945; A400 524F 434B\".",
)
.section(TEST)
.advanced();

/// The programme identification sent.
pub const TX_PI: Setting<field::Text> = Setting::new("tx_pi", "PI code", "", "")
    .help("Four hexadecimal digits, 0001 to FFFF, from the licence of the station. No default: the transmission does not start while it is empty.")
    .section(TX);
/// The station name sent.
pub const TX_PS: Setting<field::Text> = Setting::new("tx_ps", "Station name (PS)", "", "")
    .help("Up to 8 characters, padded with spaces. A change goes out from the next name cycle.")
    .section(TX);
/// The radio text sent.
pub const TX_RT: Setting<field::Text> = Setting::new("tx_rt", "Radio text", "", "")
    .help("Up to 64 characters. Empty sends no radio text. A change goes out from the next text cycle with the A/B flag turned over, so that receivers clear the old text.")
    .section(TX);
/// The programme type sent.
pub const TX_PTY: Setting<field::I64> = Setting::new("tx_pty", "Programme type", "", 0)
    .range(0.0, 31.0, 1.0)
    .zero_means("None")
    .help("The code of the programme type. The names of the codes differ between RDS and RBDS.")
    .section(TX);
/// The traffic programme flag sent.
pub const TX_TP: Setting<field::Bool> = Setting::new("tx_tp", "Traffic programme", "", false)
    .help("The station carries traffic announcements.")
    .section(TX);
/// The traffic announcement flag sent.
pub const TX_TA: Setting<field::Bool> = Setting::new("tx_ta", "Traffic announcement now", "", false)
    .help("A traffic announcement is on air now.")
    .section(TX);
/// The music flag sent.
pub const TX_MUSIC: Setting<field::Bool> = Setting::new("tx_music", "Music", "", true)
    .help("On for music, off for speech.")
    .section(TX);
/// The alternative frequencies sent.
pub const TX_AF: Setting<field::Text> = Setting::new("tx_af", "Alternative frequencies", "", "")
    .help("Up to 25 frequencies of the same programme, 87.6 to 107.9 MHz in steps of 0.1 MHz, with spaces between them. Empty sends no list.")
    .section(TX)
    .advanced();
/// The clock group sent.
pub const TX_CT: Setting<field::Bool> = Setting::new("tx_ct", "Send the clock time", "", false)
    .help("Sends the time of the clock (UTC) in group 4A as each minute starts.")
    .section(TX)
    .advanced();
/// The local time offset sent with the clock.
pub const TX_CT_OFFSET: Setting<field::I64> = Setting::new("tx_ct_offset", "Local time offset", "half hours", 0)
    .range(-24.0, 24.0, 1.0)
    .help("The local time less UTC, in half hours, sent with the clock time.")
    .section(TX)
    .advanced();
/// Stereo transmission.
pub const TX_STEREO: Setting<field::Bool> = Setting::new("tx_stereo", "Stereo", "", true)
    .help("Sends the pilot and the difference signal; off sends mono. Applies at the next start.")
    .section(TX);
/// The pre-emphasis of the transmission.
pub const TX_PREEMPHASIS: Setting<field::Choice<Preemphasis>> =
    Setting::choice("tx_preemphasis", "Pre-emphasis", Preemphasis::Us50)
        .help("50 µs in Europe, Asia, Africa and Oceania; 75 µs in the Americas and South Korea. Applies at the next start.")
        .section(TX);
/// The peak deviation of the transmission.
pub const TX_DEVIATION: Setting<field::Hz> = Setting::new("tx_deviation", "Peak deviation", "Hz", 75_000.0)
    .range(5_000.0, 100_000.0, 100.0)
    .help("The deviation that the whole multiplex never passes: audio, pilot and RDS together. 75 kHz is the broadcast value. Applies at the next start.")
    .section(TX);
/// The pilot level of the transmission.
pub const TX_PILOT: Setting<field::F64> = Setting::new("tx_pilot", "Pilot level", "%", 9.0)
    .range(0.0, 20.0, 0.1)
    .help("The pilot deviation as a share of the peak deviation. 8 to 10 % is the broadcast range. Applies at the next start.")
    .section(TX)
    .advanced();
/// The RDS level of the transmission.
pub const TX_RDS_LEVEL: Setting<field::F64> = Setting::new("tx_rds_level", "RDS level", "kHz", 2.0)
    .range(0.2, 7.5, 0.1)
    .help("The peak deviation by the RDS subcarrier; 2 to 3 kHz is usual. Applies at the next start.")
    .section(TX)
    .advanced();
/// The audio limiter of the transmission.
pub const TX_LIMITER: Setting<field::Bool> = Setting::new("tx_limiter", "Audio limiter", "", true)
    .help("Holds the audio peaks under the deviation with a gain that has no steps. Off clips each channel at full scale. Applies at the next start.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the FM stack.
    pub enum FmSetting {
        /// Stereo or mono.
        Stereo = STEREO,
        /// Mono whatever the stereo choice.
        ForceMono = FORCE_MONO,
        /// The de-emphasis.
        Deemphasis = DEEMPHASIS,
        /// The squelch threshold.
        Squelch = SQUELCH,
        /// The audio bandwidth.
        AudioBandwidth = AUDIO_BANDWIDTH,
        /// The bass shelf.
        Bass = BASS,
        /// The treble shelf.
        Treble = TREBLE,
        /// 3 dB more off the treble.
        Warm = WARM,
        /// The channel width.
        ChannelWidth = CHANNEL_WIDTH,
        /// The channel filter.
        ChannelFilter = CHANNEL_FILTER,
        /// The lowest frequency of the band.
        BandLowHz = BAND_LOW_HZ,
        /// The highest frequency of the band.
        BandHighHz = BAND_HIGH_HZ,
        /// The first channel of the raster.
        FirstHz = FIRST_HZ,
        /// The channel step.
        RasterHz = RASTER_HZ,
        /// Where the band scan starts.
        ScanLow = SCAN_LOW,
        /// Where the band scan ends.
        ScanHigh = SCAN_HIGH,
        /// The band scan waits for the whole RDS.
        ScanFullRds = SCAN_FULL_RDS,
        /// The station finder.
        StationFinder = STATION_FINDER,
        /// RDS or RBDS meanings.
        RdsVariant = RDS_VARIANT,
        /// The RDS correction.
        RdsCorrection = RDS_CORRECTION,
        /// RDS voting.
        RdsVoting = RDS_VOTING,
        /// The RDS character set.
        RdsCharset = RDS_CHARSET,
        /// The test signal in stereo.
        GenStereo = GEN_STEREO,
        /// The left tone.
        GenLeftHz = GEN_LEFT_HZ,
        /// The left level.
        GenLeftAmp = GEN_LEFT_AMP,
        /// The right tone.
        GenRightHz = GEN_RIGHT_HZ,
        /// The right level.
        GenRightAmp = GEN_RIGHT_AMP,
        /// The pilot injection.
        GenPilot = GEN_PILOT,
        /// RDS in the test signal.
        GenRds = GEN_RDS,
        /// The RDS injection.
        GenRdsKhz = GEN_RDS_KHZ,
        /// The station name sent.
        GenPs = GEN_PS,
        /// The RadioText sent.
        GenRt = GEN_RT,
        /// The PI sent.
        GenPi = GEN_PI,
        /// The programme type sent.
        GenPty = GEN_PTY,
        /// The traffic programme flag sent.
        GenTp = GEN_TP,
        /// The traffic announcement flag sent.
        GenTa = GEN_TA,
        /// The music flag sent.
        GenMusic = GEN_MUSIC,
        /// The pre-emphasis of the test signal.
        GenPreemphasis = GEN_PREEMPHASIS,
        /// More RDS groups to send.
        GenExtraGroups = GEN_EXTRA_GROUPS,
        /// The programme identification sent.
        TxPi = TX_PI,
        /// The station name sent.
        TxPs = TX_PS,
        /// The radio text sent.
        TxRt = TX_RT,
        /// The programme type sent.
        TxPty = TX_PTY,
        /// The traffic programme flag sent.
        TxTp = TX_TP,
        /// The traffic announcement flag sent.
        TxTa = TX_TA,
        /// The music flag sent.
        TxMusic = TX_MUSIC,
        /// The alternative frequencies sent.
        TxAf = TX_AF,
        /// The clock group sent.
        TxCt = TX_CT,
        /// The local time offset sent with the clock.
        TxCtOffset = TX_CT_OFFSET,
        /// Stereo transmission.
        TxStereo = TX_STEREO,
        /// The pre-emphasis of the transmission.
        TxPreemphasis = TX_PREEMPHASIS,
        /// The peak deviation of the transmission.
        TxDeviation = TX_DEVIATION,
        /// The pilot level of the transmission.
        TxPilot = TX_PILOT,
        /// The RDS level of the transmission.
        TxRdsLevel = TX_RDS_LEVEL,
        /// The audio limiter of the transmission.
        TxLimiter = TX_LIMITER,
    }
}

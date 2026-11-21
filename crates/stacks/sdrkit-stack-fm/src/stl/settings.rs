//! The settings of the STL stack: its own (deviation, channel width, the test link) and those it shares with the FM stack
//! (stereo, de-emphasis, RDS, the test tones and RDS contents), in [`StlSetting`].

use crate::settings::{
    DEEMPHASIS, GEN_LEFT_AMP, GEN_LEFT_HZ, GEN_PI, GEN_PREEMPHASIS, GEN_PS, GEN_PTY, GEN_RDS,
    GEN_RIGHT_AMP, GEN_RIGHT_HZ, GEN_RT, RDS_CHARSET, RDS_CORRECTION, RDS_VARIANT, RDS_VOTING,
    STEREO,
};
use sdrkit_core::publish::field;
use sdrkit_core::Setting;

const SOUND: &str = "Sound";
const RECEPTION: &str = "Reception";
const TEST: &str = "Test signal";

/// The widest channel: the channel rate of the FM components is near 500 kHz, and a channel filter must fit under it.
pub const MAX_WIDTH_HZ: f64 = 400e3;
/// The narrowest channel.
pub const MIN_WIDTH_HZ: f64 = 10e3;

sdrkit_core::choice_enum! {
    /// What the test link sends.
    #[derive(Default)]
    pub enum GenLink {
        /// The stereo multiplex of a broadcast transmitter, with pilot and RDS (ITU-R BS.450, IEC 62106).
        #[default]
        Composite = ("composite", "Composite stereo"),
        /// Programme audio without a pilot.
        Mono = ("mono", "Mono"),
        /// Band-limited noise, the spectrum of a digital link.
        Digital = ("digital", "Digital-like"),
    }
}

/// The deviation of full modulation.
pub const DEVIATION: Setting<field::Hz> = Setting::new("deviation", "Full deviation", "Hz", 75e3)
    .range(10e3, 200e3, 1e3)
    .help("The deviation of full modulation of the link: 75 kHz, or 50 kHz where a region uses that (ITU-R BS.450). It sets the sound level only; the measurements stay in Hz.")
    .section(SOUND);
/// The channel width follows the link.
pub const AUTO_WIDTH: Setting<field::Bool> = Setting::new("auto_width", "Automatic width", "", true)
    .help("The channel width follows the link: twice the peak deviation plus the highest multiplex frequency (60 kHz, 99 kHz with an SCA subcarrier), or the occupied width if that is more, with 35 % and 10 kHz of margin. Each change restarts the channel filter.")
    .section(RECEPTION);
/// The channel width when it does not follow the link.
pub const WIDTH: Setting<field::Hz> = Setting::new("width", "Channel width", "Hz", 300e3)
    .range(MIN_WIDTH_HZ, MAX_WIDTH_HZ, 1e3)
    .help("The channel width when the automatic width is off. At most 400 kHz and 0.9 times the sample rate.")
    .section(RECEPTION);

/// What the test link sends.
pub const GEN_LINK: Setting<field::Choice<GenLink>> =
    Setting::choice("gen_link", "Link", GenLink::Composite)
        .help("Composite sends the stereo multiplex with pilot and RDS, mono sends programme audio without a pilot, digital-like sends band-limited noise.")
        .section(TEST)
        .advanced();
/// The deviation of the test link.
pub const GEN_DEVIATION: Setting<field::Hz> =
    Setting::new("gen_deviation", "Full deviation", "Hz", 75e3)
        .range(10e3, 200e3, 1e3)
        .help("The deviation of a full-scale multiplex.")
        .section(TEST)
        .advanced();
/// The SCA subcarriers of the test link.
pub const GEN_SCA: Setting<field::Bool> = Setting::new("gen_sca", "SCA subcarriers", "", false)
    .help("Adds subcarriers at 67 and 92 kHz, each at 10 % of full deviation and frequency modulated by a 1 kHz tone.")
    .section(TEST)
    .advanced();
/// The occupied width of the digital-like test link.
pub const GEN_WIDTH: Setting<field::Hz> = Setting::new("gen_width", "Digital width", "Hz", 220e3)
    .range(20e3, 400e3, 1e3)
    .help("The occupied width of the digital-like link.")
    .section(TEST)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the STL stack.
    pub enum StlSetting {
        /// Stereo or mono.
        Stereo = STEREO,
        /// The de-emphasis.
        Deemphasis = DEEMPHASIS,
        /// The deviation of full modulation.
        Deviation = DEVIATION,
        /// The channel width follows the link.
        AutoWidth = AUTO_WIDTH,
        /// The channel width when it does not follow the link.
        Width = WIDTH,
        /// RDS or RBDS meanings.
        RdsVariant = RDS_VARIANT,
        /// The RDS correction.
        RdsCorrection = RDS_CORRECTION,
        /// RDS voting.
        RdsVoting = RDS_VOTING,
        /// The RDS character set.
        RdsCharset = RDS_CHARSET,
        /// What the test link sends.
        GenLink = GEN_LINK,
        /// The deviation of the test link.
        GenDeviation = GEN_DEVIATION,
        /// The SCA subcarriers of the test link.
        GenSca = GEN_SCA,
        /// The occupied width of the digital-like test link.
        GenWidth = GEN_WIDTH,
        /// The left tone.
        GenLeftHz = GEN_LEFT_HZ,
        /// The left level.
        GenLeftAmp = GEN_LEFT_AMP,
        /// The right tone.
        GenRightHz = GEN_RIGHT_HZ,
        /// The right level.
        GenRightAmp = GEN_RIGHT_AMP,
        /// RDS in the test signal.
        GenRds = GEN_RDS,
        /// The station name sent.
        GenPs = GEN_PS,
        /// The RadioText sent.
        GenRt = GEN_RT,
        /// The PI sent.
        GenPi = GEN_PI,
        /// The programme type sent.
        GenPty = GEN_PTY,
        /// The pre-emphasis of the test signal.
        GenPreemphasis = GEN_PREEMPHASIS,
    }
}

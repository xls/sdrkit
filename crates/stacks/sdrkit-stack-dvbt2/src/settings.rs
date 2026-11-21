//! The settings of the DVB-T2 stack: the channel width and the PLP of the receiver, the test signal options of the
//! standard (frame shape, PLP coding, profile, MISO) and the `tx_*` settings of the sender. The framework adds the standard
//! impairments.

use crate::bicm::Coding;
use crate::params::{
    allowed, ldpc_table, Bandwidth, CodeRate, FecFrame, Fft, Guard, Modulation, PilotPattern,
    Profile,
};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, Setting, SettingError, SettingSet, Value, Values};
use sdrkit_proto_mpegts::layer::DECODE_VIDEO;

sdrkit_core::choice_enum! {
    /// The constellation of the test signal's data PLP.
    pub enum PlpModulation {
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// 16-QAM.
        Qam16 = ("16qam", "16-QAM"),
        /// 64-QAM.
        Qam64 = ("64qam", "64-QAM"),
        /// 256-QAM.
        Qam256 = ("256qam", "256-QAM"),
    }
}

impl PlpModulation {
    /// The constellation.
    pub fn modulation(self) -> Modulation {
        match self {
            PlpModulation::Qpsk => Modulation::Qpsk,
            PlpModulation::Qam16 => Modulation::Qam16,
            PlpModulation::Qam64 => Modulation::Qam64,
            PlpModulation::Qam256 => Modulation::Qam256,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The code rate of the test signal's data PLP.
    pub enum PlpRate {
        /// 1/2.
        R1_2 = ("1/2", "1/2"),
        /// 3/5.
        R3_5 = ("3/5", "3/5"),
        /// 2/3.
        R2_3 = ("2/3", "2/3"),
        /// 3/4.
        R3_4 = ("3/4", "3/4"),
        /// 4/5 (T2-base).
        R4_5 = ("4/5", "4/5 (T2-base)"),
        /// 5/6 (T2-base).
        R5_6 = ("5/6", "5/6 (T2-base)"),
        /// 1/3 (T2-Lite).
        R1_3 = ("1/3", "1/3 (T2-Lite)"),
        /// 2/5 (T2-Lite).
        R2_5 = ("2/5", "2/5 (T2-Lite)"),
    }
}

impl PlpRate {
    /// The code rate.
    pub fn rate(self) -> CodeRate {
        match self {
            PlpRate::R1_2 => CodeRate::R1_2,
            PlpRate::R3_5 => CodeRate::R3_5,
            PlpRate::R2_3 => CodeRate::R2_3,
            PlpRate::R3_4 => CodeRate::R3_4,
            PlpRate::R4_5 => CodeRate::R4_5,
            PlpRate::R5_6 => CodeRate::R5_6,
            PlpRate::R1_3 => CodeRate::R1_3,
            PlpRate::R2_5 => CodeRate::R2_5,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The transport packets of the test signal.
    pub enum GenSource {
        /// A small multiplex with one service.
        Mux = ("mux", "Test multiplex"),
        /// Numbered test packets.
        Pattern = ("pattern", "Numbered packets"),
    }
}

sdrkit_core::choice_enum! {
    /// The code rate of the sender's data PLP (T2-base).
    pub enum TxRate {
        /// 1/2.
        R1_2 = ("1/2", "1/2"),
        /// 3/5.
        R3_5 = ("3/5", "3/5"),
        /// 2/3.
        R2_3 = ("2/3", "2/3"),
        /// 3/4.
        R3_4 = ("3/4", "3/4"),
        /// 4/5.
        R4_5 = ("4/5", "4/5"),
        /// 5/6.
        R5_6 = ("5/6", "5/6"),
    }
}

impl TxRate {
    /// The code rate.
    pub fn rate(self) -> CodeRate {
        match self {
            TxRate::R1_2 => CodeRate::R1_2,
            TxRate::R3_5 => CodeRate::R3_5,
            TxRate::R2_3 => CodeRate::R2_3,
            TxRate::R3_4 => CodeRate::R3_4,
            TxRate::R4_5 => CodeRate::R4_5,
            TxRate::R5_6 => CodeRate::R5_6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The constellation of the sender's L1-post signalling.
    pub enum L1Modulation {
        /// BPSK.
        Bpsk = ("bpsk", "BPSK"),
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// 16-QAM.
        Qam16 = ("16qam", "16-QAM"),
        /// 64-QAM.
        Qam64 = ("64qam", "64-QAM"),
    }
}

impl L1Modulation {
    /// The constellation.
    pub fn modulation(self) -> Modulation {
        match self {
            L1Modulation::Bpsk => Modulation::Bpsk,
            L1Modulation::Qpsk => Modulation::Qpsk,
            L1Modulation::Qam16 => Modulation::Qam16,
            L1Modulation::Qam64 => Modulation::Qam64,
        }
    }
}

const TEST_SIGNAL: &str = "Test signal";
const TRANSMIT: &str = "Transmit";

/// The channel width of the sender.
pub const TX_BANDWIDTH: Setting<field::Choice<Bandwidth>> =
    Setting::choice("tx_bandwidth", "Channel width", Bandwidth::B8)
        .help("The width of the channel sent. It sets the elementary rate of the signal.")
        .section(TRANSMIT);
/// The FFT size of the sender.
pub const TX_FFT: Setting<field::Choice<Fft>> =
    Setting::choice("tx_fft", "FFT size", Fft::K8).section(TRANSMIT);
/// The guard interval of the sender.
pub const TX_GUARD: Setting<field::Choice<Guard>> =
    Setting::choice("tx_guard", "Guard interval", Guard::G1_128)
        .help("A guard interval that the FFT size does not take is refused at start.")
        .section(TRANSMIT);
/// The pilot pattern of the sender.
pub const TX_PILOTS: Setting<field::Choice<PilotPattern>> =
    Setting::choice("tx_pilots", "Pilot pattern", PilotPattern::Pp7)
        .help("A pattern that EN 302 755 tables 58 and 59 do not allow with the FFT size and the guard interval is refused at start.")
        .section(TRANSMIT);
/// Extended carriers in the signal of the sender.
pub const TX_EXTENDED: Setting<field::Bool> =
    Setting::new("tx_extended", "Extended carriers", "", false)
        .help("8K, 16K and 32K only.")
        .section(TRANSMIT);
/// The constellation of the sender.
pub const TX_MODULATION: Setting<field::Choice<PlpModulation>> =
    Setting::choice("tx_modulation", "Constellation", PlpModulation::Qam64).section(TRANSMIT);
/// The code rate of the sender.
pub const TX_RATE: Setting<field::Choice<TxRate>> =
    Setting::choice("tx_rate", "Code rate", TxRate::R2_3).section(TRANSMIT);
/// A rotated constellation in the signal of the sender.
pub const TX_ROTATION: Setting<field::Bool> =
    Setting::new("tx_rotation", "Rotated constellation", "", true).section(TRANSMIT);
/// The FEC frame of the sender.
pub const TX_FEC_FRAME: Setting<field::Choice<FecFrame>> =
    Setting::choice("tx_fec_frame", "FEC frame", FecFrame::Normal).section(TRANSMIT);
/// The TI-blocks of each T2-frame of the sender.
pub const TX_TI_BLOCKS: Setting<field::I64> =
    Setting::new("tx_ti_blocks", "Time interleaver", "TI-blocks", 3)
        .range(0.0, 8.0, 1.0)
        .help("The TI-blocks of a T2-frame. 0: no time interleaving. Fewer TI-blocks interleave deeper and hold fewer FEC blocks a frame.")
        .section(TRANSMIT);
/// The longest T2-frame of the sender.
pub const TX_FRAME_MS: Setting<field::I64> =
    Setting::new("tx_frame_ms", "Longest frame", "ms", 100)
        .range(10.0, 250.0, 1.0)
        .help("The T2-frame holds whole symbols and whole FEC blocks and lasts this long at most, and never less than the shortest frame of the FFT size.")
        .section(TRANSMIT);
/// The constellation of the L1-post signalling of the sender.
pub const TX_L1_MOD: Setting<field::Choice<L1Modulation>> =
    Setting::choice("tx_l1_mod", "L1-post constellation", L1Modulation::Qpsk).section(TRANSMIT);
/// CELL_ID of the sender.
pub const TX_CELL_ID: Setting<field::I64> = Setting::new("tx_cell_id", "Cell id", "", 0)
    .range(0.0, 65535.0, 1.0)
    .section(TRANSMIT);
/// NETWORK_ID of the sender.
pub const TX_NETWORK_ID: Setting<field::I64> =
    Setting::new("tx_network_id", "Network id", "", 0)
        .range(0.0, 65535.0, 1.0)
        .section(TRANSMIT);
/// T2_SYSTEM_ID of the sender.
pub const TX_SYSTEM_ID: Setting<field::I64> =
    Setting::new("tx_system_id", "T2 system id", "", 0)
        .range(0.0, 65535.0, 1.0)
        .section(TRANSMIT);

/// The channel width.
pub const BANDWIDTH: Setting<field::Choice<Bandwidth>> =
    Setting::choice("bandwidth", "Channel bandwidth", Bandwidth::B8)
        .restart()
        .sets_rate(width_rate);

/// The sample rate for a value of the bandwidth setting: the elementary rate of the width, which the receive chain
/// works at.
fn width_rate(width: &Value) -> f64 {
    let Value::Choice(i) = width else { return 0.0 };
    Bandwidth::from_index(*i).map_or(0.0, Bandwidth::rate)
}
/// The data PLP to decode. -1 decodes the first data PLP.
pub const PLP: Setting<field::I64> = Setting::new("plp", "PLP", "", -1)
    .range(-1.0, 255.0, 1.0)
    .help(
        "The data PLP to decode. -1 decodes the first data PLP. Its common PLP is decoded with it.",
    );
/// The FFT size of the test signal.
pub const GEN_FFT: Setting<field::Choice<Fft>> = Setting::choice("gen_fft", "FFT size", Fft::K8)
    .section(TEST_SIGNAL)
    .advanced();
/// The guard interval of the test signal.
pub const GEN_GUARD: Setting<field::Choice<Guard>> =
    Setting::choice("gen_guard", "Guard interval", Guard::G1_8)
        .section(TEST_SIGNAL)
        .advanced();
/// The pilot pattern of the test signal.
pub const GEN_PILOTS: Setting<field::Choice<PilotPattern>> =
    Setting::choice("gen_pilots", "Pilot pattern", PilotPattern::Pp3)
        .section(TEST_SIGNAL)
        .advanced();
/// Extended carriers in the test signal (8K, 16K and 32K only).
pub const GEN_EXTENDED: Setting<field::Bool> =
    Setting::new("gen_extended", "Extended carriers", "", false)
        .section(TEST_SIGNAL)
        .advanced();
/// The constellation of the test signal.
pub const GEN_MODULATION: Setting<field::Choice<PlpModulation>> =
    Setting::choice("gen_modulation", "Constellation", PlpModulation::Qam64)
        .section(TEST_SIGNAL)
        .advanced();
/// The code rate of the test signal.
pub const GEN_RATE: Setting<field::Choice<PlpRate>> =
    Setting::choice("gen_rate", "Code rate", PlpRate::R2_3)
        .section(TEST_SIGNAL)
        .advanced();
/// The FEC frame of the test signal. T2-Lite always uses 16 200 bits.
pub const GEN_FEC_FRAME: Setting<field::Choice<FecFrame>> =
    Setting::choice("gen_fec_frame", "FEC frame", FecFrame::Normal)
        .section(TEST_SIGNAL)
        .advanced();
/// A rotated constellation in the test signal (not with 256-QAM in T2-Lite).
pub const GEN_ROTATION: Setting<field::Bool> =
    Setting::new("gen_rotation", "Rotated constellation", "", true)
        .section(TEST_SIGNAL)
        .advanced();
/// The profile of the test signal.
pub const GEN_PROFILE: Setting<field::Choice<Profile>> =
    Setting::choice("gen_profile", "Profile", Profile::Base)
        .section(TEST_SIGNAL)
        .advanced();
/// MISO in the test signal.
pub const GEN_MISO: Setting<field::Bool> = Setting::new("gen_miso", "MISO", "", false)
    .section(TEST_SIGNAL)
    .advanced();
/// The FEC blocks of each T2-frame of the test signal.
pub const GEN_BLOCKS: Setting<field::I64> =
    Setting::new("gen_blocks", "FEC blocks per frame", "", 4)
        .range(1.0, 100.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// The transport packets of the test signal.
pub const GEN_SOURCE: Setting<field::Choice<GenSource>> =
    Setting::choice("gen_source", "Content", GenSource::Mux)
        .section(TEST_SIGNAL)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the DVB-T2 stack.
    pub enum Dvbt2Setting {
        /// The channel width.
        Bandwidth = BANDWIDTH,
        /// The data PLP.
        Plp = PLP,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The test signal FFT size.
        GenFft = GEN_FFT,
        /// The test signal guard interval.
        GenGuard = GEN_GUARD,
        /// The test signal pilot pattern.
        GenPilots = GEN_PILOTS,
        /// Extended carriers in the test signal.
        GenExtended = GEN_EXTENDED,
        /// The test signal constellation.
        GenModulation = GEN_MODULATION,
        /// The test signal code rate.
        GenRate = GEN_RATE,
        /// The test signal FEC frame.
        GenFecFrame = GEN_FEC_FRAME,
        /// A rotated constellation in the test signal.
        GenRotation = GEN_ROTATION,
        /// The test signal profile.
        GenProfile = GEN_PROFILE,
        /// MISO in the test signal.
        GenMiso = GEN_MISO,
        /// FEC blocks per T2-frame of the test signal.
        GenBlocks = GEN_BLOCKS,
        /// The test signal content.
        GenSource = GEN_SOURCE,
        /// The channel width of the sender.
        TxBandwidth = TX_BANDWIDTH,
        /// The FFT size of the sender.
        TxFft = TX_FFT,
        /// The guard interval of the sender.
        TxGuard = TX_GUARD,
        /// The pilot pattern of the sender.
        TxPilots = TX_PILOTS,
        /// Extended carriers of the sender.
        TxExtended = TX_EXTENDED,
        /// The constellation of the sender.
        TxModulation = TX_MODULATION,
        /// The code rate of the sender.
        TxRate = TX_RATE,
        /// A rotated constellation of the sender.
        TxRotation = TX_ROTATION,
        /// The FEC frame of the sender.
        TxFecFrame = TX_FEC_FRAME,
        /// The TI-blocks of the sender.
        TxTiBlocks = TX_TI_BLOCKS,
        /// The longest T2-frame of the sender.
        TxFrameMs = TX_FRAME_MS,
        /// The L1-post constellation of the sender.
        TxL1Mod = TX_L1_MOD,
        /// CELL_ID of the sender.
        TxCellId = TX_CELL_ID,
        /// NETWORK_ID of the sender.
        TxNetworkId = TX_NETWORK_ID,
        /// T2_SYSTEM_ID of the sender.
        TxSystemId = TX_SYSTEM_ID,
    }
}

/// What the generator settings ask for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenSettings {
    /// Channel width.
    pub bandwidth: Bandwidth,
    /// FFT size.
    pub fft: Fft,
    /// Guard interval.
    pub guard: Guard,
    /// Pilot pattern.
    pub pp: PilotPattern,
    /// Extended carriers.
    pub extended: bool,
    /// PLP coding.
    pub coding: Coding,
    /// Profile.
    pub profile: Profile,
    /// MISO.
    pub miso: bool,
    /// FEC blocks per frame.
    pub blocks: u16,
    /// Numbered packets instead of the test multiplex.
    pub pattern: bool,
}

/// Reads the generator settings of `v`, whose schema holds the settings of [`Dvbt2Setting`].
///
/// # Errors
///
/// [`SettingError::Conflict`] if the standard does not allow the combination: a guard interval and pilot pattern that the FFT
/// size does not take (EN 302 755 tables 58, 59, I.5 and I.6), or a code rate that the profile and FEC frame do not use.
pub fn gen_settings(v: &Values) -> Result<GenSettings, SettingError> {
    let fft = v.get(&GEN_FFT);
    let guard = v.get(&GEN_GUARD);
    let pp = v.get(&GEN_PILOTS);
    let modulation = v.get(&GEN_MODULATION).modulation();
    let rate = v.get(&GEN_RATE).rate();
    let profile = v.get(&GEN_PROFILE);
    let miso = v.get(&GEN_MISO);
    let lite = profile == Profile::Lite;
    if !allowed(fft, guard, pp, miso, lite) {
        return Err(SettingError::Conflict {
            id: Dvbt2Setting::GenPilots.id(),
            with: Dvbt2Setting::GenGuard.id(),
        });
    }
    let frame = if lite {
        FecFrame::Short
    } else {
        v.get(&GEN_FEC_FRAME)
    };
    let used = rate.code().and_then(|c| CodeRate::from_code(c, lite)) == Some(rate);
    if ldpc_table(frame, rate).is_none() || !used {
        return Err(SettingError::Conflict {
            id: Dvbt2Setting::GenRate.id(),
            with: Dvbt2Setting::GenProfile.id(),
        });
    }
    Ok(GenSettings {
        bandwidth: v.get(&BANDWIDTH),
        fft,
        guard,
        pp,
        extended: v.get(&GEN_EXTENDED) && fft.has_extended(),
        coding: Coding {
            frame,
            rate,
            modulation,
            rotated: v.get(&GEN_ROTATION) && !(lite && modulation == Modulation::Qam256),
        },
        profile,
        miso,
        blocks: v.get(&GEN_BLOCKS) as u16,
        pattern: v.get(&GEN_SOURCE) == GenSource::Pattern,
    })
}

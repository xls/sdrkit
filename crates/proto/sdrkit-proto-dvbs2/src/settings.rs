//! The settings of the DVB-S/S2 stack and of its components, and the options of its test signal.

use super::modcod::{CodeRate, ModCod, Modulation};
use crate::inner::PunctureRate;
use crate::transmit::{GEN_PAYLOAD, TEST_SIGNAL};
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, SettingError, SettingId};
use sdrkit_proto_mpegts::layer::DECODE_VIDEO;

sdrkit_core::choice_enum! {
    /// The standards that the receiver looks for.
    pub enum StandardChoice {
        /// DVB-S2 and DVB-S.
        Auto = ("auto", "Auto"),
        /// DVB-S only.
        Dvbs = ("dvbs", "DVB-S only"),
        /// DVB-S2 only.
        Dvbs2 = ("dvbs2", "DVB-S2 only"),
    }
}

sdrkit_core::choice_enum! {
    /// The roll-off of the carrier.
    #[derive(Default)]
    pub enum RollOffChoice {
        /// Measured from the spectrum, or signalled.
        #[default]
        Auto = ("auto", "Auto"),
        /// 0.35.
        R35 = ("0.35", "0.35"),
        /// 0.25.
        R25 = ("0.25", "0.25"),
        /// 0.20.
        R20 = ("0.20", "0.20"),
        /// 0.15.
        R15 = ("0.15", "0.15"),
        /// 0.10.
        R10 = ("0.10", "0.10"),
        /// 0.05.
        R05 = ("0.05", "0.05"),
    }
}

impl RollOffChoice {
    /// The roll-off, 0 for automatic.
    pub fn value(self) -> f64 {
        match self {
            RollOffChoice::Auto => 0.0,
            RollOffChoice::R35 => 0.35,
            RollOffChoice::R25 => 0.25,
            RollOffChoice::R20 => 0.20,
            RollOffChoice::R15 => 0.15,
            RollOffChoice::R10 => 0.10,
            RollOffChoice::R05 => 0.05,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The polarisation of the transponder.
    pub enum Polarisation {
        /// Not given.
        Unset = ("unset", "Not set"),
        /// Vertical.
        Vertical = ("v", "Vertical"),
        /// Horizontal.
        Horizontal = ("h", "Horizontal"),
    }
}

sdrkit_core::choice_enum! {
    /// The polarisation that the LNB supply selects.
    pub enum LnbSupply {
        /// Not given.
        Unset = ("unset", "Not set"),
        /// Vertical, about 13 V.
        Vertical = ("v", "Vertical (about 13 V)"),
        /// Horizontal, about 18 V.
        Horizontal = ("h", "Horizontal (about 18 V)"),
    }
}

sdrkit_core::choice_enum! {
    /// The standard of the test signal, and of the sender.
    pub enum GenStandardChoice {
        /// DVB-S2.
        Dvbs2 = ("dvbs2", "DVB-S2"),
        /// DVB-S.
        Dvbs = ("dvbs", "DVB-S"),
    }
}

sdrkit_core::choice_enum! {
    /// The roll-off of the test signal, and of the DVB-S2 sender.
    pub enum GenRollOff {
        /// 0.35.
        R35 = ("0.35", "0.35"),
        /// 0.25.
        R25 = ("0.25", "0.25"),
        /// 0.20.
        R20 = ("0.20", "0.20"),
    }
}

impl GenRollOff {
    /// The roll-off.
    pub fn value(self) -> f64 {
        match self {
            GenRollOff::R35 => 0.35,
            GenRollOff::R25 => 0.25,
            GenRollOff::R20 => 0.20,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The FEC frame of the DVB-S2 sender.
    pub enum FecFrame {
        /// 64 800 bits.
        Normal = ("normal", "Normal"),
        /// 16 200 bits.
        Short = ("short", "Short"),
    }
}

sdrkit_core::choice_enum! {
    /// The MODCODs of DVB-S2 in the order of their numbers, 1 to 28 (EN 302 307-1 table 12).
    pub enum TxModcod {
        /// QPSK 1/4.
        Qpsk14 = ("QPSK-1/4", "QPSK 1/4"),
        /// QPSK 1/3.
        Qpsk13 = ("QPSK-1/3", "QPSK 1/3"),
        /// QPSK 2/5.
        Qpsk25 = ("QPSK-2/5", "QPSK 2/5"),
        /// QPSK 1/2.
        Qpsk12 = ("QPSK-1/2", "QPSK 1/2"),
        /// QPSK 3/5.
        Qpsk35 = ("QPSK-3/5", "QPSK 3/5"),
        /// QPSK 2/3.
        Qpsk23 = ("QPSK-2/3", "QPSK 2/3"),
        /// QPSK 3/4.
        Qpsk34 = ("QPSK-3/4", "QPSK 3/4"),
        /// QPSK 4/5.
        Qpsk45 = ("QPSK-4/5", "QPSK 4/5"),
        /// QPSK 5/6.
        Qpsk56 = ("QPSK-5/6", "QPSK 5/6"),
        /// QPSK 8/9.
        Qpsk89 = ("QPSK-8/9", "QPSK 8/9"),
        /// QPSK 9/10.
        Qpsk910 = ("QPSK-9/10", "QPSK 9/10"),
        /// 8PSK 3/5.
        Psk835 = ("8PSK-3/5", "8PSK 3/5"),
        /// 8PSK 2/3.
        Psk823 = ("8PSK-2/3", "8PSK 2/3"),
        /// 8PSK 3/4.
        Psk834 = ("8PSK-3/4", "8PSK 3/4"),
        /// 8PSK 5/6.
        Psk856 = ("8PSK-5/6", "8PSK 5/6"),
        /// 8PSK 8/9.
        Psk889 = ("8PSK-8/9", "8PSK 8/9"),
        /// 8PSK 9/10.
        Psk8910 = ("8PSK-9/10", "8PSK 9/10"),
        /// 16APSK 2/3.
        Apsk1623 = ("16APSK-2/3", "16APSK 2/3"),
        /// 16APSK 3/4.
        Apsk1634 = ("16APSK-3/4", "16APSK 3/4"),
        /// 16APSK 4/5.
        Apsk1645 = ("16APSK-4/5", "16APSK 4/5"),
        /// 16APSK 5/6.
        Apsk1656 = ("16APSK-5/6", "16APSK 5/6"),
        /// 16APSK 8/9.
        Apsk1689 = ("16APSK-8/9", "16APSK 8/9"),
        /// 16APSK 9/10.
        Apsk16910 = ("16APSK-9/10", "16APSK 9/10"),
        /// 32APSK 3/4.
        Apsk3234 = ("32APSK-3/4", "32APSK 3/4"),
        /// 32APSK 4/5.
        Apsk3245 = ("32APSK-4/5", "32APSK 4/5"),
        /// 32APSK 5/6.
        Apsk3256 = ("32APSK-5/6", "32APSK 5/6"),
        /// 32APSK 8/9.
        Apsk3289 = ("32APSK-8/9", "32APSK 8/9"),
        /// 32APSK 9/10.
        Apsk32910 = ("32APSK-9/10", "32APSK 9/10"),
    }
}

impl TxModcod {
    /// The MODCOD.
    pub fn modcod(self) -> ModCod {
        use sdrkit_core::Choice;
        ModCod::new(self.index() as u8 + 1).unwrap_or_else(|| unreachable!("28 MODCODs"))
    }
}

/// The symbol rate. 0 finds it from the signal.
pub const SYMBOL_RATE: Setting<field::Hz> = Setting::new("symbol_rate", "Symbol rate", "Hz", 0.0)
    .range(0.0, 30e6, 0.0)
    .help("0: automatic.");
/// The standards the receiver looks for.
pub const STANDARD: Setting<field::Choice<StandardChoice>> =
    Setting::choice("standard", "Standard", StandardChoice::Auto)
        .help("Auto tries DVB-S2 and DVB-S. Choose one to skip the other search on a weak signal.");
/// The roll-off.
pub const ROLL_OFF: Setting<field::Choice<RollOffChoice>> =
    Setting::choice("roll_off", "Roll-off", RollOffChoice::Auto);
/// The input stream of a DVB-S2 multistream carrier.
pub const ISI: Setting<field::I64> = Setting::new("isi", "Input stream", "", -1)
    .range(-1.0, 255.0, 1.0)
    .help("-1: the first stream seen.");
/// The PL scrambling code of DVB-S2.
pub const PL_SCRAMBLING: Setting<field::I64> =
    Setting::new("pl_scrambling", "PL scrambling code", "", 0).range(0.0, 262_141.0, 1.0);
/// The carrier to decode, as an offset in Hz. Empty picks the carrier by itself.
pub const CARRIER_PICK: Setting<field::Text> = Setting::new("carrier_pick", "Carrier", "", "")
    .help("Offset in Hz, empty for automatic (set by clicking a carrier in the survey).");
/// Every IP packet as PCAP file bytes.
pub const IP_PCAP: Setting<field::Bool> = Setting::new("ip_pcap", "PCAP output", "", false)
    .help("Send every IP packet as PCAP file bytes. The host writes the file.");
/// The polarisation of the transponder.
pub const TX_POLARISATION: Setting<field::Choice<Polarisation>> =
    Setting::choice("tx_polarisation", "Transponder", Polarisation::Unset);
/// The polarisation the LNB supply selects.
pub const LNB_SUPPLY: Setting<field::Choice<LnbSupply>> =
    Setting::choice("lnb_supply", "LNB supply", LnbSupply::Unset);
/// The section of the sender settings.
pub const TRANSMIT: &str = "Transmit";
/// The symbol rate of the sender.
pub const TX_SYMBOL_RATE: Setting<field::Hz> =
    Setting::new("tx_symbol_rate", "Symbol rate", "Hz", 5e6)
        .range(25e3, 60e6, 0.0)
        .help("The symbol rate sent. The session's rate must be at least 1.3 × (1 + roll-off) times it: 1.755 for roll-off 0.35.")
        .section(TRANSMIT);
/// The code rate of the DVB-S sender.
pub const TX_RATE: Setting<field::Choice<PunctureRate>> =
    Setting::choice("tx_rate", "Code rate", PunctureRate::R23)
        .help("DVB-S: the code rate of the convolutional code, 1/2, 2/3, 3/4, 5/6 or 7/8. DVB-S2 takes its code rate from tx_modcod.")
        .section(TRANSMIT);
/// The standard of the sender.
pub const TX_STANDARD: Setting<field::Choice<GenStandardChoice>> =
    Setting::choice("tx_standard", "Standard", GenStandardChoice::Dvbs)
        .help("The standard sent: DVB-S (EN 300 421) or DVB-S2 (EN 302 307-1).")
        .section(TRANSMIT);
/// The MODCOD of the DVB-S2 sender.
pub const TX_MODCOD: Setting<field::Choice<TxModcod>> =
    Setting::choice("tx_modcod", "MODCOD", TxModcod::Qpsk23)
        .help("DVB-S2: the modulation and the code rate of every frame.")
        .section(TRANSMIT);
/// The FEC frame of the DVB-S2 sender.
pub const TX_FRAME: Setting<field::Choice<FecFrame>> =
    Setting::choice("tx_frame", "FEC frame", FecFrame::Normal)
        .help("DVB-S2: normal frames (64 800 bits) or short frames (16 200 bits). Code rate 9/10 has no short frame.")
        .section(TRANSMIT);
/// Pilots of the DVB-S2 sender.
pub const TX_PILOTS: Setting<field::Bool> = Setting::new("tx_pilots", "Pilots", "", false)
    .help("DVB-S2: a block of 36 pilot symbols after every 16 slots.")
    .section(TRANSMIT);
/// The roll-off of the DVB-S2 sender.
pub const TX_ROLLOFF: Setting<field::Choice<GenRollOff>> =
    Setting::choice("tx_rolloff", "Roll-off", GenRollOff::R35)
        .help("DVB-S2: the roll-off of the pulse. DVB-S has 0.35.")
        .section(TRANSMIT);
/// The PL scrambling code of the DVB-S2 sender.
pub const TX_PL_SCRAMBLING: Setting<field::I64> =
    Setting::new("tx_pl_scrambling", "PL scrambling code", "", 0)
        .range(0.0, 262_141.0, 1.0)
        .help("DVB-S2: the number of the Gold code of the PL scrambling. 0 for broadcast.")
        .section(TRANSMIT);
/// Spectral inversion of the sender.
pub const TX_INVERSION: Setting<field::Bool> =
    Setting::new("tx_inversion", "Spectral inversion", "", false)
        .help("Sends the spectrum mirrored (Q negated).")
        .section(TRANSMIT);
/// The standard of the test signal.
pub const GEN_STANDARD: Setting<field::Choice<GenStandardChoice>> =
    Setting::choice("gen_standard", "Standard", GenStandardChoice::Dvbs2)
        .section(TEST_SIGNAL)
        .advanced();
/// The modulation of the DVB-S2 test signal.
pub const GEN_MODULATION: Setting<field::Choice<Modulation>> =
    Setting::choice("gen_modulation", "Modulation", Modulation::Qpsk)
        .section(TEST_SIGNAL)
        .advanced();
/// The code rate of the test signal. A DVB-S test signal with a rate that DVB-S lacks uses 2/3.
pub const GEN_RATE: Setting<field::Choice<CodeRate>> =
    Setting::choice("gen_rate", "Code rate", CodeRate::R23)
        .section(TEST_SIGNAL)
        .advanced();
/// Short FEC frames in the test signal.
pub const GEN_SHORT: Setting<field::Bool> = Setting::new("gen_short", "Short frames", "", false)
    .section(TEST_SIGNAL)
    .advanced();
/// Pilots in the test signal.
pub const GEN_PILOTS: Setting<field::Bool> = Setting::new("gen_pilots", "Pilots", "", false)
    .section(TEST_SIGNAL)
    .advanced();
/// The roll-off of the test signal.
pub const GEN_ROLLOFF: Setting<field::Choice<GenRollOff>> =
    Setting::choice("gen_rolloff", "Roll-off", GenRollOff::R35)
        .section(TEST_SIGNAL)
        .advanced();
/// The symbol rate of the test signal.
pub const GEN_SYMBOL_RATE: Setting<field::Hz> =
    Setting::new("gen_symbol_rate", "Symbol rate", "Hz", 0.0)
        .range(0.0, 30e6, 0.0)
        .section(TEST_SIGNAL)
        .help("0: 5 Msym/s or what the sample rate allows.")
        .advanced();
/// The PL scrambling code of the test signal.
pub const GEN_PL_SCRAMBLING: Setting<field::I64> =
    Setting::new("gen_pl_scrambling", "PL scrambling code", "", 0)
        .range(0.0, 262_141.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// A dummy frame every this many frames, 0 for none.
pub const GEN_DUMMY_EVERY: Setting<field::I64> =
    Setting::new("gen_dummy_every", "A dummy frame every", "frames", 0)
        .range(0.0, 1000.0, 1.0)
        .section(TEST_SIGNAL)
        .advanced();
/// GSE (IP) instead of a transport stream.
pub const GEN_GSE: Setting<field::Bool> = Setting::new(
    "gen_gse",
    "GSE (IP) instead of a transport stream",
    "",
    false,
)
.section(TEST_SIGNAL)
.advanced();
/// Two input streams, a transport stream and GSE.
pub const GEN_MULTISTREAM: Setting<field::Bool> = Setting::new(
    "gen_multistream",
    "Two input streams (TS and GSE)",
    "",
    false,
)
.section(TEST_SIGNAL)
.advanced();

sdrkit_core::choice_enum! {
    /// Which MODCODs the DVB-S2 FEC decodes.
    #[derive(Default)]
    pub enum ModcodRule {
        /// Every MODCOD.
        #[default]
        Any = ("any", "Any"),
        /// Only the MODCOD of the setting `modcod`.
        Fixed = ("fixed", "Fixed"),
        /// Only the MODCODs of the setting `modcods`.
        Set = ("set", "Set"),
    }
}

sdrkit_core::choice_enum! {
    /// Which FEC frame length the DVB-S2 FEC decodes.
    #[derive(Default)]
    pub enum FrameLengthRule {
        /// Both.
        #[default]
        Any = ("any", "Any"),
        /// Normal frames (64 800 bits) only.
        Normal = ("normal", "Normal"),
        /// Short frames (16 200 bits) only.
        Short = ("short", "Short"),
    }
}

sdrkit_core::choice_enum! {
    /// The pilots that the DVB-S2 frames are expected to have.
    #[derive(Default)]
    pub enum PilotsRule {
        /// No check.
        #[default]
        Any = ("any", "Any"),
        /// Pilots on.
        On = ("on", "On"),
        /// Pilots off.
        Off = ("off", "Off"),
    }
}

/// Which MODCODs the DVB-S2 FEC decodes. Frames of other MODCODs are counted and not decoded; the header search is the same.
pub const MODCOD_RULE: Setting<field::Choice<ModcodRule>> =
    Setting::choice("modcod_rule", "MODCODs taken", ModcodRule::Any)
        .help("Fixed: only the MODCOD of `modcod`. Set: only the MODCODs of `modcods`. Other frames are counted, not decoded.")
        .advanced();
/// The MODCOD of the rule "fixed".
pub const MODCOD: Setting<field::I64> = Setting::new("modcod", "MODCOD", "", 0)
    .range(0.0, 255.0, 1.0)
    .zero_means("None")
    .advanced();
/// The MODCODs of the rule "set": numbers with commas or spaces between them.
pub const MODCODS: Setting<field::Text> = Setting::new("modcods", "MODCOD set", "", "")
    .help("MODCOD numbers with commas between them, for the rule \"set\".")
    .advanced();
/// Which FEC frame length the DVB-S2 FEC decodes.
pub const FRAME_LENGTH: Setting<field::Choice<FrameLengthRule>> =
    Setting::choice("frame_length", "Frame length", FrameLengthRule::Any)
        .help("Frames of the other length are counted and skipped.")
        .advanced();
/// The pilots the frames are expected to have. A mismatch is counted; the frame is decoded.
pub const PILOTS_RULE: Setting<field::Choice<PilotsRule>> =
    Setting::choice("pilots", "Pilots", PilotsRule::Any)
        .help("A check only: a frame with other pilots is counted and decoded.")
        .advanced();

/// The MODCOD numbers of the text of setting `id` (`modcods`).
///
/// # Errors
///
/// [`SettingError::Malformed`] if a part is not a number from 0 to 255.
pub fn parse_modcods(id: SettingId, text: &str) -> Result<Vec<u8>, SettingError> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(|p| {
            p.parse::<u8>()
                .map_err(|_| SettingError::Malformed { id, at: Some(0) })
        })
        .collect()
}

sdrkit_core::settings! {
    /// The settings of the DVB-S/S2 stack.
    pub enum DvbsSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The standards the receiver looks for.
        Standard = STANDARD,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The input stream.
        Isi = ISI,
        /// The PL scrambling code.
        PlScrambling = PL_SCRAMBLING,
        /// The carrier to decode.
        CarrierPick = CARRIER_PICK,
        /// Which MODCODs are decoded.
        ModcodRule = MODCOD_RULE,
        /// The MODCOD of the rule "fixed".
        Modcod = MODCOD,
        /// The MODCODs of the rule "set".
        Modcods = MODCODS,
        /// Which frame length is decoded.
        FrameLength = FRAME_LENGTH,
        /// The expected pilots.
        Pilots = PILOTS_RULE,
        /// PCAP output.
        IpPcap = IP_PCAP,
        /// The polarisation of the transponder.
        TxPolarisation = TX_POLARISATION,
        /// The LNB supply.
        LnbSupply = LNB_SUPPLY,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The standard of the test signal.
        GenStandard = GEN_STANDARD,
        /// The modulation of the test signal.
        GenModulation = GEN_MODULATION,
        /// The code rate of the test signal.
        GenRate = GEN_RATE,
        /// Short frames in the test signal.
        GenShort = GEN_SHORT,
        /// Pilots in the test signal.
        GenPilots = GEN_PILOTS,
        /// The roll-off of the test signal.
        GenRollOff = GEN_ROLLOFF,
        /// The symbol rate of the test signal.
        GenSymbolRate = GEN_SYMBOL_RATE,
        /// The PL scrambling code of the test signal.
        GenPlScrambling = GEN_PL_SCRAMBLING,
        /// Dummy frames in the test signal.
        GenDummyEvery = GEN_DUMMY_EVERY,
        /// GSE in the test signal.
        GenGse = GEN_GSE,
        /// Two input streams in the test signal.
        GenMultistream = GEN_MULTISTREAM,
        /// The payload of the test signal.
        GenPayload = GEN_PAYLOAD,
        /// The symbol rate of the sender.
        TxSymbolRate = TX_SYMBOL_RATE,
        /// The code rate of the sender.
        TxRate = TX_RATE,
        /// Spectral inversion of the sender.
        TxInversion = TX_INVERSION,
        /// The standard of the sender.
        TxStandard = TX_STANDARD,
        /// The MODCOD of the DVB-S2 sender.
        TxModcod = TX_MODCOD,
        /// The FEC frame of the DVB-S2 sender.
        TxFrame = TX_FRAME,
        /// Pilots of the DVB-S2 sender.
        TxPilots = TX_PILOTS,
        /// The roll-off of the DVB-S2 sender.
        TxRollOff = TX_ROLLOFF,
        /// The PL scrambling code of the DVB-S2 sender.
        TxPlScrambling = TX_PL_SCRAMBLING,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbs.front`.
    pub enum FrontSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The carrier to decode.
        CarrierPick = CARRIER_PICK,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbs.sync`.
    pub enum SyncSetting {
        /// The standards it looks for.
        Standard = STANDARD,
        /// The PL scrambling code.
        PlScrambling = PL_SCRAMBLING,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbs.fec2`.
    pub enum Fec2Setting {
        /// Which MODCODs are decoded.
        ModcodRule = MODCOD_RULE,
        /// The MODCOD of the rule "fixed".
        Modcod = MODCOD,
        /// The MODCODs of the rule "set".
        Modcods = MODCODS,
        /// Which frame length is decoded.
        FrameLength = FRAME_LENGTH,
        /// The expected pilots.
        Pilots = PILOTS_RULE,
    }
}

sdrkit_core::settings! {
    /// The settings of `dvbs.report`.
    pub enum ReportSetting {
        /// The polarisation of the transponder.
        TxPolarisation = TX_POLARISATION,
        /// The LNB supply.
        LnbSupply = LNB_SUPPLY,
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The PL scrambling code.
        PlScrambling = PL_SCRAMBLING,
    }
}

/// The carrier offset of the text of setting `id` (`carrier_pick`): `None` for empty text.
///
/// # Errors
///
/// [`SettingError::Malformed`] if the text is not empty and not a finite number.
pub fn parse_pick(id: SettingId, text: &str) -> Result<Option<f64>, SettingError> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(Some)
        .ok_or(SettingError::Malformed { id, at: Some(0) })
}

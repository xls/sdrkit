//! The settings of the three stacks. The settings in the "Test signal" section (`gen_*`) configure the generator only, those in
//! the "Transmit" section (`tx_*`) the sender only, at its next `start`. The framework adds its own `gen_*` settings for noise,
//! offsets, clock errors, echoes and hum.

use crate::atv::nbtv::NbtvFormat;
use crate::atv::Modulation;
use crate::card::Pattern;
pub use crate::channels::CHANNELS;
use crate::gen_atv::AtvGenParams;
use crate::gen_video::SecamIdent;
use crate::mac::frame::Variant;
use crate::sound_gen::{Programme, SecondSound};
use crate::systems::{Colour, SystemId};
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, Values};

const TEST: &str = "Test signal";
const TX: &str = "Transmit";
const NBTV: &str = "NBTV";

sdrkit_core::choice_enum! {
    /// The modulation the `atv` receiver decodes: found from the signal, or a fixed one.
    pub enum ModulationChoice {
        /// AM broadcast (vestigial sideband).
        Am = ("am", "AM (broadcast)"),
        /// FM video (FPV and video links).
        FmVideo = ("fm", "FM video (FPV)"),
        /// NBTV or mechanical television on AM.
        NbtvAm = ("nbtv_am", "NBTV / mechanical on AM"),
        /// NBTV on narrow FM.
        NbtvFm = ("nbtv_fm", "NBTV / mechanical on FM"),
        /// AM broadcast or FM video, found from the signal.
        Auto = ("auto", "Automatic (AM or FM video)"),
    }
}

impl ModulationChoice {
    /// The modulation of the graph that the receiver starts with: AM for [`ModulationChoice::Auto`].
    pub fn first(self) -> Modulation {
        match self {
            ModulationChoice::Auto | ModulationChoice::Am => Modulation::Am,
            ModulationChoice::FmVideo => Modulation::FmVideo,
            ModulationChoice::NbtvAm => Modulation::NbtvAm,
            ModulationChoice::NbtvFm => Modulation::NbtvFm,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The system the `atv` receiver decodes: found from the signal, or one of ITU-R BT.470.
    pub enum SystemChoice {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// B.
        B = ("b", "B (7 MHz)"),
        /// G.
        G = ("g", "G (8 MHz)"),
        /// I.
        I = ("i", "I"),
        /// D/K.
        Dk = ("dk", "D/K"),
        /// M.
        M = ("m", "M"),
        /// N.
        N = ("n", "N"),
        /// A.
        A = ("a", "A (405 lines)"),
        /// C.
        C = ("c", "C"),
        /// E.
        E = ("e", "E (819 lines)"),
        /// H.
        H = ("h", "H"),
        /// K'.
        Kp = ("kp", "K'"),
        /// L.
        L = ("l", "L"),
        /// L'.
        Lp = ("lp", "L' (band I)"),
    }
}

impl SystemChoice {
    /// The system chosen, or `None` for automatic.
    pub fn system(self) -> Option<SystemId> {
        Some(match self {
            SystemChoice::Auto => return None,
            SystemChoice::B => SystemId::B,
            SystemChoice::G => SystemId::G,
            SystemChoice::I => SystemId::I,
            SystemChoice::Dk => SystemId::DK,
            SystemChoice::M => SystemId::M,
            SystemChoice::N => SystemId::N,
            SystemChoice::A => SystemId::A,
            SystemChoice::C => SystemId::C,
            SystemChoice::E => SystemId::E,
            SystemChoice::H => SystemId::H,
            SystemChoice::Kp => SystemId::Kp,
            SystemChoice::L => SystemId::L,
            SystemChoice::Lp => SystemId::Lp,
        })
    }
}

sdrkit_core::choice_enum! {
    /// The colour system the picture decoder uses.
    pub enum ColourChoice {
        /// Found from the burst.
        Auto = ("auto", "Automatic"),
        /// No colour.
        Mono = ("mono", "Mono"),
        /// PAL.
        Pal = ("pal", "PAL"),
        /// NTSC.
        Ntsc = ("ntsc", "NTSC"),
        /// SECAM.
        Secam = ("secam", "SECAM"),
    }
}

sdrkit_core::choice_enum! {
    /// The vision detector.
    pub enum DetectorChoice {
        /// Synchronous when the carrier is locked, else the envelope.
        Auto = ("auto", "Automatic (synchronous when the carrier is locked)"),
        /// The envelope only.
        Envelope = ("envelope", "Envelope only"),
    }
}

sdrkit_core::choice_enum! {
    /// How the two fields make a picture.
    pub enum Deinterlace {
        /// Both fields in one frame.
        Weave = ("weave", "Weave"),
        /// Each field doubled.
        Bob = ("bob", "Bob"),
    }
}

sdrkit_core::choice_enum! {
    /// The noise reduction of the picture.
    pub enum NoiseReduction {
        /// By the measured noise.
        Auto = ("auto", "Automatic"),
        /// Off.
        Off = ("off", "Off"),
        /// Light.
        Light = ("light", "Light"),
        /// Medium.
        Medium = ("medium", "Medium"),
        /// Strong.
        Strong = ("strong", "Strong"),
    }
}

sdrkit_core::choice_enum! {
    /// The black level above blanking.
    pub enum BlackSetup {
        /// As the system says.
        System = ("system", "By system"),
        /// None.
        None = ("none", "None"),
        /// 7.5 IRE.
        Ire75 = ("75", "7.5 IRE"),
    }
}

sdrkit_core::choice_enum! {
    /// The channel width, for the carrier offset with 5.5 MHz sound.
    pub enum ChannelWidth {
        /// 6 MHz.
        W6 = ("6", "6 MHz"),
        /// 7 MHz.
        W7 = ("7", "7 MHz"),
        /// 8 MHz.
        W8 = ("8", "8 MHz"),
    }
}

impl ChannelWidth {
    /// The width in MHz.
    pub fn mhz(self) -> f64 {
        match self {
            ChannelWidth::W6 => 6.0,
            ChannelWidth::W7 => 7.0,
            ChannelWidth::W8 => 8.0,
        }
    }
}

sdrkit_core::choice_enum! {
    /// Which sound the selector plays.
    pub enum SoundChoice {
        /// NICAM or A2 when there, else the carrier.
        Auto = ("auto", "Automatic (NICAM or A2 when there)"),
        /// The FM or AM carrier only.
        Carrier = ("carrier", "FM / AM carrier only"),
    }
}

sdrkit_core::choice_enum! {
    /// The language played when the second sound carries two.
    pub enum DualSound {
        /// The first.
        First = ("first", "First"),
        /// The second.
        Second = ("second", "Second"),
    }
}

sdrkit_core::settings! {
    /// The settings of the second-sound decoders.
    pub enum DualSoundSetting {
        /// The language played.
        DualSound = DUAL_SOUND,
    }
}

sdrkit_core::choice_enum! {
    /// The system of the `atv` test signal.
    pub enum GenSystem {
        /// B/G.
        G = ("g", "B/G (8 MHz)"),
        /// B.
        B = ("b", "B (7 MHz)"),
        /// I.
        I = ("i", "I"),
        /// D/K.
        Dk = ("dk", "D/K"),
        /// M.
        M = ("m", "M"),
        /// N.
        N = ("n", "N"),
        /// A.
        A = ("a", "A (405)"),
        /// C.
        C = ("c", "C"),
        /// E.
        E = ("e", "E (819)"),
        /// H.
        H = ("h", "H"),
        /// K'.
        Kp = ("kp", "K'"),
        /// L.
        L = ("l", "L"),
        /// L'.
        Lp = ("lp", "L'"),
        /// NBTV on AM.
        NbtvAm = ("nbtv_am", "NBTV on AM"),
        /// NBTV on narrow FM.
        NbtvFm = ("nbtv_fm", "NBTV on narrow FM"),
    }
}

impl GenSystem {
    /// The television system, or `None` for NBTV.
    pub fn system(self) -> Option<SystemId> {
        Some(match self {
            GenSystem::G => SystemId::G,
            GenSystem::B => SystemId::B,
            GenSystem::I => SystemId::I,
            GenSystem::Dk => SystemId::DK,
            GenSystem::M => SystemId::M,
            GenSystem::N => SystemId::N,
            GenSystem::A => SystemId::A,
            GenSystem::C => SystemId::C,
            GenSystem::E => SystemId::E,
            GenSystem::H => SystemId::H,
            GenSystem::Kp => SystemId::Kp,
            GenSystem::L => SystemId::L,
            GenSystem::Lp => SystemId::Lp,
            GenSystem::NbtvAm | GenSystem::NbtvFm => return None,
        })
    }
}

sdrkit_core::choice_enum! {
    /// The system that the `atv` sender sends, by the names of the receiver.
    pub enum TxSystem {
        /// G.
        G = ("g", "G (8 MHz)"),
        /// B.
        B = ("b", "B (7 MHz)"),
        /// I.
        I = ("i", "I"),
        /// D/K.
        Dk = ("dk", "D/K"),
        /// M.
        M = ("m", "M"),
        /// N.
        N = ("n", "N"),
        /// A.
        A = ("a", "A (405 lines)"),
        /// C.
        C = ("c", "C"),
        /// E.
        E = ("e", "E (819 lines)"),
        /// H.
        H = ("h", "H"),
        /// K'.
        Kp = ("kp", "K'"),
        /// L.
        L = ("l", "L"),
        /// L'.
        Lp = ("lp", "L' (band I)"),
    }
}

impl TxSystem {
    /// The television system.
    pub fn system(self) -> SystemId {
        match self {
            TxSystem::G => SystemId::G,
            TxSystem::B => SystemId::B,
            TxSystem::I => SystemId::I,
            TxSystem::Dk => SystemId::DK,
            TxSystem::M => SystemId::M,
            TxSystem::N => SystemId::N,
            TxSystem::A => SystemId::A,
            TxSystem::C => SystemId::C,
            TxSystem::E => SystemId::E,
            TxSystem::H => SystemId::H,
            TxSystem::Kp => SystemId::Kp,
            TxSystem::L => SystemId::L,
            TxSystem::Lp => SystemId::Lp,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The colour of the `atv` test signal.
    pub enum GenColour {
        /// As the system says.
        System = ("system", "By system"),
        /// PAL.
        Pal = ("pal", "PAL"),
        /// NTSC.
        Ntsc = ("ntsc", "NTSC"),
        /// SECAM.
        Secam = ("secam", "SECAM"),
        /// No colour.
        None = ("none", "None"),
    }
}

sdrkit_core::choice_enum! {
    /// The black setup of the 525-line test signal.
    pub enum GenSetup {
        /// 7.5 IRE.
        Ire75 = ("75", "7.5 IRE"),
        /// None.
        None = ("none", "None (Japan)"),
    }
}

sdrkit_core::choice_enum! {
    /// The test picture of the FPV test signal.
    pub enum FpvPattern {
        /// The test card.
        Card = ("card", "Test card"),
        /// Colour bars.
        Bars = ("bars", "Colour bars"),
    }
}

impl GenColour {
    /// The colour on `system`: `None` for the system's own; PAL and NTSC take the variant of the system (PAL-M and PAL-N,
    /// NTSC-N and NTSC 4.43).
    pub fn colour(self, system: SystemId) -> Option<Colour> {
        match self {
            GenColour::Pal => Some(match system {
                SystemId::M => Colour::PalM,
                SystemId::N => Colour::PalN,
                _ => Colour::Pal,
            }),
            GenColour::Ntsc => Some(match system {
                SystemId::M => Colour::Ntsc,
                SystemId::N => Colour::NtscN,
                _ => Colour::Ntsc443,
            }),
            GenColour::Secam => Some(Colour::Secam),
            GenColour::None => Some(Colour::Mono),
            GenColour::System => None,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The sound subcarrier of the `fpv` sender.
    pub enum TxSubcarrier {
        /// 6.5 MHz.
        S65 = ("6.5", "6.5 MHz"),
        /// 6.0 MHz.
        S60 = ("6.0", "6.0 MHz"),
    }
}

sdrkit_core::choice_enum! {
    /// The system of the FPV receiver.
    pub enum FpvSystem {
        /// Found from the signal.
        Auto = ("auto", "Automatic"),
        /// PAL, 625 lines.
        Pal = ("pal", "PAL (625)"),
        /// NTSC, 525 lines.
        Ntsc = ("ntsc", "NTSC (525)"),
    }
}

sdrkit_core::choice_enum! {
    /// The system of the FPV test signal.
    pub enum FpvGenSystem {
        /// PAL, 625 lines.
        Pal = ("pal", "PAL (625)"),
        /// NTSC, 525 lines.
        Ntsc = ("ntsc", "NTSC (525)"),
    }
}

sdrkit_core::choice_enum! {
    /// The FPV sound subcarrier the receiver plays.
    pub enum SoundSubcarrier {
        /// The strongest one.
        Auto = ("auto", "Automatic"),
        /// 6.0 MHz.
        S60 = ("6.0", "6.0 MHz"),
        /// 6.5 MHz.
        S65 = ("6.5", "6.5 MHz"),
    }
}

sdrkit_core::choice_enum! {
    /// The sound of the FPV test signal.
    pub enum FpvGenSound {
        /// A subcarrier at 6.5 MHz.
        S65 = ("6.5", "Subcarrier 6.5 MHz"),
        /// A subcarrier at 6.0 MHz.
        S60 = ("6.0", "6.0 MHz"),
        /// No sound.
        None = ("none", "None"),
    }
}

// atv

/// The modulation, which chooses the graph.
pub const MODULATION: Setting<field::Choice<ModulationChoice>> =
    Setting::choice("modulation", "Modulation", ModulationChoice::Auto)
        .help("Automatic finds AM broadcast or FM video from the signal. Broadcast television is AM. FM video needs 20 Msps.")
        .restart();
/// The system.
pub const SYSTEM: Setting<field::Choice<SystemChoice>> =
    Setting::choice("system", "System", SystemChoice::Auto).help(
        "Automatic finds the system from the signal. E needs 20 Msps for its 14 MHz channel.",
    );
/// The colour system.
pub const COLOUR: Setting<field::Choice<ColourChoice>> =
    Setting::choice("colour", "Colour system", ColourChoice::Auto);
/// Colour on or off.
pub const COLOUR_ON: Setting<field::Bool> = Setting::new("colour_on", "Colour", "", true);
/// The saturation.
pub const SATURATION: Setting<field::F64> =
    Setting::new("saturation", "Saturation", "", 1.0).range(0.0, 2.0, 0.01);
/// The NTSC tint.
pub const TINT: Setting<field::F64> = Setting::new("tint", "Tint", "°", 0.0)
    .range(-30.0, 30.0, 1.0)
    .help("NTSC only. PAL and SECAM correct themselves.");
/// The vision detector.
pub const DETECTOR: Setting<field::Choice<DetectorChoice>> =
    Setting::choice("detector", "Envelope detector", DetectorChoice::Auto);
/// The deinterlace.
pub const DEINTERLACE: Setting<field::Choice<Deinterlace>> =
    Setting::choice("deinterlace", "Fields", Deinterlace::Weave);
/// The noise reduction.
pub const NOISE_REDUCTION: Setting<field::Choice<NoiseReduction>> =
    Setting::choice("noise_reduction", "Noise reduction", NoiseReduction::Auto);
/// The black setup.
pub const BLACK_SETUP: Setting<field::Choice<BlackSetup>> =
    Setting::choice("black_setup", "Black setup", BlackSetup::System);
/// The chroma delay.
pub const CHROMA_DELAY: Setting<field::F64> =
    Setting::new("chroma_delay", "Chroma delay", "ns", -1.0)
        .range(-1.0, 500.0, 1.0)
        .help("-1 is the standard's 170 ns.");
/// The channel width.
pub const CHANNEL_WIDTH: Setting<field::Choice<ChannelWidth>> =
    Setting::choice("channel_width", "Channel width", ChannelWidth::W8)
        .help("Only matters for the carrier offset with 5.5 MHz sound.");
/// Snow while there is no picture.
pub const SNOW: Setting<field::Bool> =
    Setting::new("snow", "Show snow when there is no picture", "", false);
/// The sound played.
pub const SOUND_SYSTEM: Setting<field::Choice<SoundChoice>> =
    Setting::choice("sound_system", "Sound", SoundChoice::Auto);
/// The language of a two-language second sound.
pub const DUAL_SOUND: Setting<field::Choice<DualSound>> =
    Setting::choice("dual_sound", "Two languages", DualSound::First);
/// NBTV lines.
pub const NBTV_LINES: Setting<field::I64> = Setting::new("nbtv_lines", "Lines", "", 32)
    .range(24.0, 240.0, 1.0)
    .section(NBTV)
    .restart();
/// NBTV pictures a second.
pub const NBTV_RATE: Setting<field::F64> = Setting::new("nbtv_rate", "Pictures/s", "", 12.5)
    .range(1.0, 60.0, 0.01)
    .section(NBTV)
    .restart();
/// NBTV FM deviation.
pub const NBTV_DEVIATION: Setting<field::F64> =
    Setting::new("nbtv_deviation", "FM deviation", "kHz", 5.0)
        .range(0.5, 100.0, 0.5)
        .section(NBTV)
        .restart();
/// NBTV scan direction.
pub const NBTV_VERTICAL: Setting<field::Bool> =
    Setting::new("nbtv_vertical", "Vertical scan", "", true)
        .section(NBTV)
        .restart();

/// The `atv` test signal system.
pub const GEN_SYSTEM: Setting<field::Choice<GenSystem>> =
    Setting::choice("gen_system", "System", GenSystem::G)
        .section(TEST)
        .advanced();
/// The `atv` test signal colour.
pub const GEN_COLOUR: Setting<field::Choice<GenColour>> =
    Setting::choice("gen_colour", "Colour", GenColour::System)
        .section(TEST)
        .advanced();
/// The test picture.
pub const GEN_PICTURE: Setting<field::Choice<Pattern>> =
    Setting::choice("gen_picture", "Picture", Pattern::Card)
        .section(TEST)
        .advanced();
/// The test sound.
pub const GEN_SOUND: Setting<field::Choice<Programme>> =
    Setting::choice("gen_sound", "Sound", Programme::ToneGaps)
        .section(TEST)
        .advanced();
/// The second sound.
pub const GEN_SECOND: Setting<field::Choice<SecondSound>> =
    Setting::choice("gen_second", "Second sound", SecondSound::None)
        .section(TEST)
        .advanced();
/// The sync compression.
pub const GEN_COMPRESSION: Setting<field::F64> =
    Setting::new("gen_compression", "Sync compression", "%", 0.0)
        .range(0.0, 80.0, 1.0)
        .section(TEST)
        .advanced();
/// Group-delay pre-correction.
pub const GEN_GROUP_DELAY: Setting<field::Bool> =
    Setting::new("gen_group_delay", "Group-delay pre-correction", "", true)
        .section(TEST)
        .advanced();
/// A Nyquist-slope transmitter.
pub const GEN_NYQUIST: Setting<field::Bool> =
    Setting::new("gen_nyquist", "Nyquist-slope transmitter", "", false)
        .section(TEST)
        .advanced();
/// The black setup of 525 lines.
pub const GEN_SETUP: Setting<field::Choice<GenSetup>> =
    Setting::choice("gen_setup", "Black setup in 525 lines", GenSetup::Ire75)
        .section(TEST)
        .advanced();
/// The SECAM identification.
pub const GEN_SECAM_ID: Setting<field::Choice<SecamIdent>> =
    Setting::choice("gen_secam_id", "SECAM identification", SecamIdent::Both)
        .section(TEST)
        .advanced();
/// The ident text.
pub const GEN_IDENT: Setting<field::Text> =
    Setting::new("gen_ident", "Ident text", "", "TEST CARD")
        .section(TEST)
        .advanced();
/// Teletext in the test signal.
pub const GEN_TELETEXT: Setting<field::Bool> = Setting::new("gen_teletext", "Teletext", "", false)
    .help("Three pages of teletext in the field blanking (625 lines).")
    .section(TEST)
    .advanced();
/// The time the test card clock starts at.
pub const GEN_START: Setting<field::F64> = Setting::new("gen_start", "Clock start time", "s", 0.0)
    .range(0.0, 86_400.0, 0.01)
    .section(TEST)
    .advanced();
/// NBTV lines of the test signal.
pub const GEN_NBTV_LINES: Setting<field::I64> = Setting::new("gen_nbtv_lines", "NBTV lines", "", 0)
    .range(0.0, 240.0, 1.0)
    .zero_means("32")
    .section(TEST)
    .advanced();
/// NBTV pictures a second of the test signal.
pub const GEN_NBTV_RATE: Setting<field::F64> =
    Setting::new("gen_nbtv_rate", "NBTV pictures/s", "", 0.0)
        .range(0.0, 60.0, 0.01)
        .zero_means("12.5")
        .section(TEST)
        .advanced();
/// The NBTV test picture.
pub const GEN_NBTV_PATTERN: Setting<field::I64> =
    Setting::new("gen_nbtv_pattern", "NBTV picture", "", 0)
        .range(0.0, 2.0, 1.0)
        .section(TEST)
        .advanced();

/// The system sent.
pub const TX_SYSTEM: Setting<field::Choice<TxSystem>> =
    Setting::choice("tx_system", "System", TxSystem::G).section(TX);
/// The colour sent.
pub const TX_COLOUR: Setting<field::Choice<GenColour>> =
    Setting::choice("tx_colour", "Colour", GenColour::System)
        .help("By system: NTSC on M, SECAM on L, L' and K', none on A, C and E, PAL on the others.")
        .section(TX);
/// What is sent while the host gives no picture.
pub const TX_PICTURE: Setting<field::Choice<Pattern>> =
    Setting::choice("tx_picture", "Test picture", Pattern::Card)
        .help("What is sent until the host gives a picture with the command frame.")
        .section(TX);
/// Sound on or off.
pub const TX_SOUND: Setting<field::Bool> = Setting::new("tx_sound", "Sound", "", true)
    .help("On: the host's audio (mono) on the sound carrier, an unmodulated carrier while it gives none. Off: no sound carrier.")
    .section(TX);
/// Sync compression of the transmitter.
pub const TX_COMPRESSION: Setting<field::F64> =
    Setting::new("tx_compression", "Sync compression", "%", 0.0)
        .range(0.0, 80.0, 1.0)
        .section(TX)
        .advanced();
/// Chroma group-delay pre-correction.
pub const TX_GROUP_DELAY: Setting<field::Bool> =
    Setting::new("tx_group_delay", "Group-delay pre-correction", "", true)
        .help("The chrominance 170 ns early, as BT.470 asks of a PAL or NTSC transmitter.")
        .section(TX)
        .advanced();
/// The modulator has the Nyquist slope.
pub const TX_NYQUIST: Setting<field::Bool> =
    Setting::new("tx_nyquist", "Nyquist slope", "", false)
        .section(TX)
        .advanced();
/// The black setup of 525 lines.
pub const TX_SETUP: Setting<field::Choice<GenSetup>> =
    Setting::choice("tx_setup", "Black setup in 525 lines", GenSetup::Ire75)
        .section(TX)
        .advanced();
/// The SECAM identification.
pub const TX_SECAM_ID: Setting<field::Choice<SecamIdent>> =
    Setting::choice("tx_secam_id", "SECAM identification", SecamIdent::Both)
        .section(TX)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `atv` sender.
    pub enum AtvTxSetting {
        /// System.
        System = TX_SYSTEM,
        /// Colour.
        Colour = TX_COLOUR,
        /// Test picture.
        Picture = TX_PICTURE,
        /// Sound.
        Sound = TX_SOUND,
        /// Sync compression.
        Compression = TX_COMPRESSION,
        /// Group-delay pre-correction.
        GroupDelay = TX_GROUP_DELAY,
        /// Nyquist slope.
        Nyquist = TX_NYQUIST,
        /// Black setup.
        Setup = TX_SETUP,
        /// SECAM identification.
        SecamId = TX_SECAM_ID,
    }
}

sdrkit_core::settings! {
    /// The settings of the `atv` stack.
    pub enum AtvSetting {
        /// Modulation.
        Modulation = MODULATION,
        /// Channels.
        Channels = CHANNELS,
        /// System.
        System = SYSTEM,
        /// Colour system.
        Colour = COLOUR,
        /// Colour on.
        ColourOn = COLOUR_ON,
        /// Saturation.
        Saturation = SATURATION,
        /// Tint.
        Tint = TINT,
        /// Detector.
        Detector = DETECTOR,
        /// Fields.
        Deinterlace = DEINTERLACE,
        /// Noise reduction.
        NoiseReduction = NOISE_REDUCTION,
        /// Black setup.
        BlackSetup = BLACK_SETUP,
        /// Chroma delay.
        ChromaDelay = CHROMA_DELAY,
        /// Channel width.
        ChannelWidth = CHANNEL_WIDTH,
        /// Snow.
        Snow = SNOW,
        /// Sound.
        SoundSystem = SOUND_SYSTEM,
        /// Two languages.
        DualSound = DUAL_SOUND,
        /// NBTV lines.
        NbtvLines = NBTV_LINES,
        /// NBTV pictures a second.
        NbtvRate = NBTV_RATE,
        /// NBTV deviation.
        NbtvDeviation = NBTV_DEVIATION,
        /// NBTV scan direction.
        NbtvVertical = NBTV_VERTICAL,
        /// Test signal system.
        GenSystem = GEN_SYSTEM,
        /// Test signal colour.
        GenColour = GEN_COLOUR,
        /// Test picture.
        GenPicture = GEN_PICTURE,
        /// Test sound.
        GenSound = GEN_SOUND,
        /// Second sound.
        GenSecond = GEN_SECOND,
        /// Sync compression.
        GenCompression = GEN_COMPRESSION,
        /// Group-delay pre-correction.
        GenGroupDelay = GEN_GROUP_DELAY,
        /// Nyquist slope.
        GenNyquist = GEN_NYQUIST,
        /// Black setup in 525 lines.
        GenSetup = GEN_SETUP,
        /// SECAM identification.
        GenSecamId = GEN_SECAM_ID,
        /// Ident text.
        GenIdent = GEN_IDENT,
        /// Teletext.
        GenTeletext = GEN_TELETEXT,
        /// Clock start.
        GenStart = GEN_START,
        /// NBTV lines.
        GenNbtvLines = GEN_NBTV_LINES,
        /// NBTV pictures a second.
        GenNbtvRate = GEN_NBTV_RATE,
        /// NBTV picture.
        GenNbtvPattern = GEN_NBTV_PATTERN,
        /// Transmit system.
        TxSystem = TX_SYSTEM,
        /// Transmit colour.
        TxColour = TX_COLOUR,
        /// Transmit test picture.
        TxPicture = TX_PICTURE,
        /// Transmit sound.
        TxSound = TX_SOUND,
        /// Transmit sync compression.
        TxCompression = TX_COMPRESSION,
        /// Transmit group-delay pre-correction.
        TxGroupDelay = TX_GROUP_DELAY,
        /// Transmit Nyquist slope.
        TxNyquist = TX_NYQUIST,
        /// Transmit black setup.
        TxSetup = TX_SETUP,
        /// Transmit SECAM identification.
        TxSecamId = TX_SECAM_ID,
    }
}

/// The generator parameters from the `atv` settings values.
pub fn atv_gen_params(rate: f64, v: &Values) -> AtvGenParams {
    let system = v.get(&GEN_SYSTEM).system().unwrap_or(SystemId::G);
    AtvGenParams {
        rate_hz: rate,
        system,
        colour: v.get(&GEN_COLOUR).colour(system),
        pattern: v.get(&GEN_PICTURE),
        sound: v.get(&GEN_SOUND),
        host_sound: false,
        second: v.get(&GEN_SECOND),
        sync_compression: v.get(&GEN_COMPRESSION) / 100.0,
        group_delay: v.get(&GEN_GROUP_DELAY),
        nyquist_slope: v.get(&GEN_NYQUIST),
        setup: v.get(&GEN_SETUP) == GenSetup::Ire75,
        secam_ident: v.get(&GEN_SECAM_ID),
        ident: v.get(&GEN_IDENT).to_owned(),
        level: 0.4,
        start_s: v.get(&GEN_START),
        centre_offset_hz: 0.0,
        teletext: v.get(&GEN_TELETEXT),
    }
}

/// The NBTV format from the `atv` settings values.
pub fn nbtv_format(v: &Values) -> NbtvFormat {
    NbtvFormat {
        lines: v.get(&NBTV_LINES) as u32,
        rate: v.get(&NBTV_RATE),
        vertical: v.get(&NBTV_VERTICAL),
    }
}

// mac

/// The MAC variant of the test signal.
pub const GEN_VARIANT: Setting<field::Choice<Variant>> =
    Setting::choice("gen_variant", "Variant", Variant::D2)
        .section(TEST)
        .advanced();
/// EuroCrypt scrambling in the test signal.
pub const GEN_SCRAMBLED: Setting<field::Bool> =
    Setting::new("gen_scrambled", "Scrambled (EuroCrypt)", "", false)
        .section(TEST)
        .advanced();
/// A 16:9 test signal.
pub const GEN_WIDE: Setting<field::Bool> = Setting::new("gen_wide", "16:9", "", false)
    .section(TEST)
    .advanced();
/// The test tone.
pub const GEN_TONE: Setting<field::Hz> = Setting::new("gen_tone", "Tone", "Hz", 0.0)
    .range(0.0, 15_000.0, 1.0)
    .zero_means("1000 Hz")
    .section(TEST)
    .advanced();
/// The channel name of the test signal.
pub const GEN_CHANNEL: Setting<field::Text> =
    Setting::new("gen_channel", "Channel name", "", "TEST TV")
        .section(TEST)
        .advanced();
/// The network name of the test signal.
pub const GEN_NETWORK: Setting<field::Text> =
    Setting::new("gen_network", "Network name", "", "TEST SATELLITE")
        .section(TEST)
        .advanced();
/// The country of the test signal.
pub const GEN_COUNTRY: Setting<field::Text> =
    Setting::new("gen_country", "Country", "", "TEST COUNTRY")
        .section(TEST)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `mac` stack: the test signal's only.
    pub enum MacSetting {
        /// Variant.
        GenVariant = GEN_VARIANT,
        /// Scrambled.
        GenScrambled = GEN_SCRAMBLED,
        /// 16:9.
        GenWide = GEN_WIDE,
        /// Test picture.
        GenPicture = GEN_PICTURE,
        /// Tone.
        GenTone = GEN_TONE,
        /// Channel name.
        GenChannel = GEN_CHANNEL,
        /// Network name.
        GenNetwork = GEN_NETWORK,
        /// Country.
        GenCountry = GEN_COUNTRY,
    }
}

/// The MAC generator's parameters from the `mac` settings values.
pub fn mac_gen_params(rate: f64, v: &Values) -> crate::mac::gen::MacGenParams {
    let d = crate::mac::gen::MacGenParams::default();
    let text = |s: &Setting<field::Text>, x: &str| {
        let t = v.get(s);
        if t.is_empty() {
            x.to_owned()
        } else {
            t.to_owned()
        }
    };
    let tone = v.get(&GEN_TONE);
    crate::mac::gen::MacGenParams {
        rate_hz: rate,
        variant: v.get(&GEN_VARIANT),
        scrambled: v.get(&GEN_SCRAMBLED),
        wide: v.get(&GEN_WIDE),
        pattern: v.get(&GEN_PICTURE),
        tone_hz: if tone > 0.0 { tone } else { 1000.0 },
        channel: text(&GEN_CHANNEL, &d.channel),
        network: text(&GEN_NETWORK, &d.network),
        country: text(&GEN_COUNTRY, &d.country),
        ..d
    }
}

// fpv

/// The FPV system.
pub const FPV_SYSTEM: Setting<field::Choice<FpvSystem>> =
    Setting::choice("system", "System", FpvSystem::Auto).restart();
/// The FPV sound subcarrier.
pub const SOUND_SUBCARRIER: Setting<field::Choice<SoundSubcarrier>> = Setting::choice(
    "sound_subcarrier",
    "Sound subcarrier",
    SoundSubcarrier::Auto,
);
/// FPV sound on or off.
pub const AUDIO: Setting<field::Bool> = Setting::new("audio", "Audio", "", true);
/// The FPV test signal system.
pub const FPV_GEN_SYSTEM: Setting<field::Choice<FpvGenSystem>> =
    Setting::choice("gen_system", "System", FpvGenSystem::Pal)
        .section(TEST)
        .advanced();
/// The FPV test picture.
pub const FPV_GEN_PICTURE: Setting<field::Choice<FpvPattern>> =
    Setting::choice("gen_picture", "Picture", FpvPattern::Card)
        .section(TEST)
        .advanced();
/// The FPV test sound.
pub const FPV_GEN_SOUND: Setting<field::Choice<FpvGenSound>> =
    Setting::choice("gen_sound", "Sound", FpvGenSound::S65)
        .section(TEST)
        .advanced();
/// No transmitter in the test signal.
pub const GEN_NONE: Setting<field::Bool> = Setting::new("gen_none", "No transmitter", "", false)
    .section(TEST)
    .advanced();
/// The capture centre from the transmitter.
pub const GEN_CENTRE_MHZ: Setting<field::F64> = Setting::new(
    "gen_centre_mhz",
    "Capture centre from the transmitter",
    "MHz",
    0.0,
)
.range(-10.0, 10.0, 0.1)
.section(TEST)
.advanced();
/// The FM deviation of the test signal.
pub const GEN_DEVIATION_MHZ: Setting<field::F64> =
    Setting::new("gen_deviation_mhz", "Deviation", "MHz", 8.0)
        .range(1.0, 12.0, 0.1)
        .section(TEST)
        .advanced();
/// The sync tip at the highest frequency.
pub const GEN_SYNC_HIGH: Setting<field::Bool> = Setting::new(
    "gen_sync_high",
    "Sync tip at the highest frequency",
    "",
    false,
)
.section(TEST)
.advanced();
/// The AC coupling time constant of the test signal.
pub const GEN_AC_MS: Setting<field::F64> =
    Setting::new("gen_ac_ms", "AC coupling time constant", "ms", 0.0)
        .range(0.0, 100.0, 0.1)
        .section(TEST)
        .advanced();

/// The system that the `fpv` sender sends.
pub const TX_FPV_SYSTEM: Setting<field::Choice<FpvGenSystem>> =
    Setting::choice("tx_system", "System", FpvGenSystem::Pal).section(TX);
/// What the `fpv` sender sends while the host gives no picture.
pub const TX_FPV_PICTURE: Setting<field::Choice<FpvPattern>> =
    Setting::choice("tx_picture", "Test picture", FpvPattern::Card)
        .help("What is sent until the host gives a picture with the command frame.")
        .section(TX);
/// The sound subcarrier of the `fpv` sender.
pub const TX_SUBCARRIER: Setting<field::Choice<TxSubcarrier>> =
    Setting::choice("tx_subcarrier", "Sound subcarrier", TxSubcarrier::S65)
        .help("The subcarrier of the sound in the composite: FM, 25 kHz deviation at full scale.")
        .section(TX);
/// The FM deviation of the `fpv` sender.
pub const TX_DEVIATION_MHZ: Setting<field::F64> =
    Setting::new("tx_deviation_mhz", "Deviation", "MHz", 8.0)
        .range(1.0, 12.0, 0.1)
        .help("The swing from the middle to the sync tip and to peak white.")
        .section(TX);
/// The sync tip at the highest frequency.
pub const TX_SYNC_HIGH: Setting<field::Bool> = Setting::new(
    "tx_sync_high",
    "Sync tip at the highest frequency",
    "",
    false,
)
.section(TX)
.advanced();
/// The AC coupling time constant of the `fpv` sender.
pub const TX_AC_MS: Setting<field::F64> =
    Setting::new("tx_ac_ms", "AC coupling time constant", "ms", 0.0)
        .range(0.0, 100.0, 0.1)
        .zero_means("DC coupled")
        .section(TX)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `fpv` sender.
    pub enum FpvTxSetting {
        /// System.
        System = TX_FPV_SYSTEM,
        /// Test picture.
        Picture = TX_FPV_PICTURE,
        /// Sound.
        Sound = TX_SOUND,
        /// Sound subcarrier.
        Subcarrier = TX_SUBCARRIER,
        /// Deviation.
        DeviationMhz = TX_DEVIATION_MHZ,
        /// Sync tip high.
        SyncHigh = TX_SYNC_HIGH,
        /// AC coupling.
        AcMs = TX_AC_MS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `fpv` stack.
    pub enum FpvSetting {
        /// System.
        System = FPV_SYSTEM,
        /// Channels.
        Channels = CHANNELS,
        /// Sound subcarrier.
        SoundSubcarrier = SOUND_SUBCARRIER,
        /// Colour on.
        ColourOn = COLOUR_ON,
        /// Saturation.
        Saturation = SATURATION,
        /// Fields.
        Deinterlace = DEINTERLACE,
        /// Audio.
        Audio = AUDIO,
        /// Test signal system.
        GenSystem = FPV_GEN_SYSTEM,
        /// Test picture.
        GenPicture = FPV_GEN_PICTURE,
        /// Test sound.
        GenSound = FPV_GEN_SOUND,
        /// No transmitter.
        GenNone = GEN_NONE,
        /// Capture centre.
        GenCentreMhz = GEN_CENTRE_MHZ,
        /// Deviation.
        GenDeviationMhz = GEN_DEVIATION_MHZ,
        /// Sync tip high.
        GenSyncHigh = GEN_SYNC_HIGH,
        /// AC coupling.
        GenAcMs = GEN_AC_MS,
        /// Transmit system.
        TxSystem = TX_FPV_SYSTEM,
        /// Transmit test picture.
        TxPicture = TX_FPV_PICTURE,
        /// Transmit sound.
        TxSound = TX_SOUND,
        /// Transmit sound subcarrier.
        TxSubcarrier = TX_SUBCARRIER,
        /// Transmit deviation.
        TxDeviationMhz = TX_DEVIATION_MHZ,
        /// Transmit sync tip high.
        TxSyncHigh = TX_SYNC_HIGH,
        /// Transmit AC coupling.
        TxAcMs = TX_AC_MS,
    }
}

/// The FPV generator's parameters from the `fpv` settings values.
pub fn fpv_gen_params(rate: f64, v: &Values) -> crate::fpv::FpvGenParams {
    crate::fpv::FpvGenParams {
        rate_hz: rate,
        ntsc: v.get(&FPV_GEN_SYSTEM) == FpvGenSystem::Ntsc,
        bars: v.get(&FPV_GEN_PICTURE) == FpvPattern::Bars,
        sound_hz: match v.get(&FPV_GEN_SOUND) {
            FpvGenSound::S60 => 6.0e6,
            FpvGenSound::None => 0.0,
            FpvGenSound::S65 => 6.5e6,
        },
        none: v.get(&GEN_NONE),
        centre_hz: -v.get(&GEN_CENTRE_MHZ) * 1e6,
        deviation_hz: v.get(&GEN_DEVIATION_MHZ) * 1e6,
        sync_high: v.get(&GEN_SYNC_HIGH),
        ac_s: v.get(&GEN_AC_MS) / 1e3,
        ..Default::default()
    }
}

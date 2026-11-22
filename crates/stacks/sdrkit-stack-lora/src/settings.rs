//! The settings of every stack of the family, each once as a typed constant, the settings of each component, and the schema of
//! each stack. The settings of the "Test signal" section (`gen_*`) configure the generator only. The station (position,
//! callsign) and the impairments of the test signal come from the framework.

use crate::mesh::mt_client::Role;
use crate::proto::meshtastic;
use sdrkit_core::publish::field;
use sdrkit_core::settings::{DeclareSetting, Schema, SettingDecl};
use sdrkit_core::value::ChoiceDecl;
use sdrkit_core::{CatalogueSetting, Setting, SettingSet};
use std::sync::Arc;

/// A catalogue setting in a section of the setup.
pub struct SectionCatalogue {
    /// The setting.
    pub setting: CatalogueSetting,
    /// The section.
    pub section: &'static str,
}

impl std::ops::Deref for SectionCatalogue {
    type Target = CatalogueSetting;
    fn deref(&self) -> &CatalogueSetting {
        &self.setting
    }
}

impl DeclareSetting for SectionCatalogue {
    fn key(&self) -> &'static str {
        self.setting.key
    }
    fn decl(&self) -> SettingDecl {
        self.setting.decl().with_section(self.section)
    }
}

sdrkit_core::choice_enum! {
    /// A LoRa bandwidth. The keys are the bandwidths in Hz.
    pub enum LoraBandwidth {
        /// 7.8 kHz.
        Bw7_8 = ("7812.5", "7.8 kHz"),
        /// 10.4 kHz.
        Bw10_4 = ("10417", "10.4 kHz"),
        /// 15.6 kHz.
        Bw15_6 = ("15625", "15.6 kHz"),
        /// 20.8 kHz.
        Bw20_8 = ("20833", "20.8 kHz"),
        /// 31.25 kHz.
        Bw31_25 = ("31250", "31.25 kHz"),
        /// 41.7 kHz.
        Bw41_7 = ("41667", "41.7 kHz"),
        /// 62.5 kHz.
        Bw62_5 = ("62500", "62.5 kHz"),
        /// 125 kHz.
        Bw125 = ("125000", "125 kHz"),
        /// 250 kHz.
        Bw250 = ("250000", "250 kHz"),
        /// 500 kHz.
        Bw500 = ("500000", "500 kHz"),
        /// 812.5 kHz (2.4 GHz).
        Bw812_5 = ("812500", "812.5 kHz (2.4 GHz)"),
        /// 1625 kHz (2.4 GHz).
        Bw1625 = ("1625000", "1625 kHz (2.4 GHz)"),
    }
}

impl LoraBandwidth {
    /// The bandwidth, Hz.
    pub fn hz(self) -> f64 {
        match self {
            LoraBandwidth::Bw7_8 => 7812.5,
            LoraBandwidth::Bw10_4 => 10417.0,
            LoraBandwidth::Bw15_6 => 15625.0,
            LoraBandwidth::Bw20_8 => 20833.0,
            LoraBandwidth::Bw31_25 => 31250.0,
            LoraBandwidth::Bw41_7 => 41667.0,
            LoraBandwidth::Bw62_5 => 62500.0,
            LoraBandwidth::Bw125 => 125e3,
            LoraBandwidth::Bw250 => 250e3,
            LoraBandwidth::Bw500 => 500e3,
            LoraBandwidth::Bw812_5 => 812.5e3,
            LoraBandwidth::Bw1625 => 1625e3,
        }
    }
}

sdrkit_core::choice_enum! {
    /// A LoRa coding rate. The keys are the denominators.
    pub enum CodingRate {
        /// 4/5.
        Cr5 = ("5", "4/5"),
        /// 4/6.
        Cr6 = ("6", "4/6"),
        /// 4/7.
        Cr7 = ("7", "4/7"),
        /// 4/8.
        Cr8 = ("8", "4/8"),
    }
}

impl CodingRate {
    /// The denominator, 5 to 8.
    pub fn denominator(self) -> u8 {
        match self {
            CodingRate::Cr5 => 5,
            CodingRate::Cr6 => 6,
            CodingRate::Cr7 => 7,
            CodingRate::Cr8 => 8,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The sync words that `loraraw` searches.
    pub enum RawSync {
        /// 0x12, private networks.
        Private = ("0x12", "0x12 private"),
        /// 0x34, LoRaWAN.
        Lorawan = ("0x34", "0x34 LoRaWAN"),
        /// 0x2B, Meshtastic.
        Meshtastic = ("0x2B", "0x2B Meshtastic"),
        /// Each of these three.
        Any = ("any", "Any of these"),
        /// The word of `sync_custom`.
        Custom = ("custom", "Custom"),
    }
}

sdrkit_core::choice_enum! {
    /// Low data rate optimisation.
    pub enum LdroChoice {
        /// On when the symbol is longer than 16 ms.
        Auto = ("auto", "Automatic"),
        /// Off.
        Off = ("off", "Off"),
        /// On.
        On = ("on", "On"),
    }
}

sdrkit_core::choice_enum! {
    /// The emphasis filter in front of the 1200 bit/s modem.
    pub enum Emphasis {
        /// None.
        None = ("auto", "None"),
        /// De-emphasis.
        De = ("de", "De-emphasis"),
        /// Pre-emphasis.
        Pre = ("pre", "Pre-emphasis"),
    }
}

sdrkit_core::choice_enum! {
    /// A packet speed.
    pub enum PacketSpeed {
        /// 1200 bit/s AFSK.
        Afsk1200 = ("1200", "1200 bit/s"),
        /// 9600 bit/s G3RUH.
        G3ruh9600 = ("9600", "9600 bit/s"),
    }
}

impl PacketSpeed {
    /// The speed, bit/s.
    pub fn baud(self) -> u32 {
        match self {
            PacketSpeed::Afsk1200 => 1200,
            PacketSpeed::G3ruh9600 => 9600,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The packet speeds of a stack or a test signal.
    pub enum Speeds {
        /// 1200 and 9600 bit/s.
        Both = ("both", "1200 and 9600 bit/s"),
        /// Only 1200 bit/s.
        Only1200 = ("1200", "Only 1200 bit/s"),
        /// Only 9600 bit/s.
        Only9600 = ("9600", "Only 9600 bit/s"),
    }
}

sdrkit_core::choice_enum! {
    /// Where the own Meshtastic position comes from.
    pub enum PositionMode {
        /// No position is sent.
        Off = ("off", "Off"),
        /// The station's position.
        Station = ("station", "The station's"),
    }
}

sdrkit_core::choice_enum! {
    /// The scene of the mesh test signal.
    pub enum GenScene {
        /// The scene of the stack.
        Auto = ("auto", "The stack's"),
        /// Meshtastic and MeshCore nodes with texts.
        Texting = ("texting", "Meshtastic and MeshCore"),
        /// LoRa APRS and MeshCom stations.
        Amateur = ("amateur", "LoRa APRS and MeshCom"),
        /// LoRaWAN devices.
        Lorawan = ("lorawan", "LoRaWAN"),
    }
}

// ---- The mesh planner (`mesh.channels`) ----

/// The frequency of the primary channel.
pub const DIAL_FREQ: Setting<field::Hz> = Setting::new("dial_freq", "Dial", "Hz", 0.0)
    .range(0.0, 6e9, 0.0)
    .section("Channels")
    .zero_means("The tuned frequency")
    .help("The frequency of the stack's primary channel. A change after the start asks the host to tune there.")
    .advanced();
/// The protocols that `mesh` searches.
pub const PROTOCOLS: Setting<field::I64> = Setting::new("protocols", "Protocols", "", 15)
    .range(0.0, 31.0, 1.0)
    .section("Channels")
    .help("Bits: 1 Meshtastic, 2 MeshCore, 4 LoRa APRS, 8 MeshCom, 16 LoRaWAN. Only the mesh stack reads it.")
    .advanced();
/// Every other Meshtastic preset of the band too.
pub const PRESET_SEARCH: Setting<field::Bool> =
    Setting::new("preset_search", "Every Meshtastic preset", "", false)
        .section("Meshtastic")
        .help("Also every other preset of the band (more work).")
        .advanced();
/// The lower edge of the band of the Meshtastic slots.
pub const MT_SLOT_START: Setting<field::Hz> =
    Setting::new("mt_slot_start", "Slot band start", "Hz", 0.0)
        .range(0.0, 6e9, 0.0)
        .section("Meshtastic")
        .help(
            "The lower edge of the band in which the name of the primary channel chooses the frequency slot. With both \
             edges 0 each preset lies at the dial. From 2.4 GHz the presets take the wide bandwidths.",
        )
        .advanced();
/// The upper edge of the band of the Meshtastic slots.
pub const MT_SLOT_END: Setting<field::Hz> = Setting::new("mt_slot_end", "Slot band end", "Hz", 0.0)
    .range(0.0, 6e9, 0.0)
    .section("Meshtastic")
    .help("The upper edge of the band of the frequency slots.")
    .advanced();
/// The MeshCore, LoRa APRS and MeshCom channels.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "")
    .section("Channels")
    .help(
        "MeshCore, LoRa APRS and MeshCom networks: entries separated by \";\", each a label, \"=\", the protocol (meshcore, \
         loraaprs or meshcom) and, each after \"/\", the frequency in Hz, the bandwidth in Hz, the spreading factor, the \
         coding rate and, when they differ from the protocol's, the preamble and the sync word. For example \
         \"Net=meshcore/100000000/62500/8/8\". At most 256 entries. A protocol without a channel listens at the dial.",
    )
    .advanced();
/// The LoRaWAN channels.
pub const LW_CHANNELS: Setting<field::Text> = Setting::new("lw_channels", "LoRaWAN channels", "", "")
    .section("LoRaWAN")
    .help(
        "Entries separated by \";\", each the frequency in Hz, the bandwidth in Hz, the spreading factors and the flags, \
         separated by \"/\": u devices send there, d gateways answer there, j a default channel. For example \
         \"100100000/125000/7-12/udj\". At most 256 entries. Empty: one channel at the dial.",
    )
    .advanced();

fn preset_choices() -> Vec<ChoiceDecl> {
    meshtastic::PRESETS
        .iter()
        .map(|p| ChoiceDecl::new(p.name, p.name))
        .collect()
}

/// The primary Meshtastic preset.
pub const MT_PRESET: SectionCatalogue = SectionCatalogue {
    setting: CatalogueSetting {
        key: "mt_preset",
        label: "Meshtastic preset",
        help: "",
        choices: preset_choices,
        default: "LongFast",
        advanced: false,
        role: None,
    },
    section: "Meshtastic",
};
/// The primary channel's own name. Empty takes the preset's name.
pub const MT_CHANNEL_NAME: Setting<field::Text> =
    Setting::new("mt_channel_name", "Primary channel name", "", "")
        .section("Meshtastic")
        .help("The name decides the frequency slot. Empty takes the name of the preset.");
/// The frequency slot.
pub const MT_CHANNEL_NUM: Setting<field::I64> =
    Setting::new("mt_channel_num", "Channel number", "", 0)
        .range(0.0, crate::mesh::plan::SLOTS_CAP as f64, 1.0)
        .section("Meshtastic")
        .help(
            "The frequency slot, from 1. 0 takes it from the primary channel's name. With a slot band, a number above \
             the slots of the preset in the band is refused when it is set; after a later change of the band or the \
             preset it counts on from slot 1 again (number minus 1, modulo the slots), as Meshtastic devices count.",
        )
        .advanced();
/// A frequency in place of the slot's.
pub const MT_OVERRIDE_FREQ: Setting<field::Hz> =
    Setting::new("mt_override_freq", "Frequency override", "Hz", 0.0)
        .range(0.0, 6e9, 0.0)
        .section("Meshtastic")
        .zero_means("The frequency of the slot")
        .advanced();
/// An offset added to the Meshtastic frequency.
pub const MT_FREQ_OFFSET: Setting<field::Hz> =
    Setting::new("mt_freq_offset", "Frequency offset", "Hz", 0.0)
        .range(-1e6, 1e6, 1.0)
        .section("Meshtastic")
        .advanced();
/// The custom LoRa setting replaces the plan.
pub const CUSTOM: Setting<field::Bool> = Setting::new("custom", "Custom LoRa setting", "", false)
    .section("Custom")
    .advanced();
/// The frequency of the custom setting.
pub const CUSTOM_FREQ: Setting<field::Hz> = Setting::new("custom_freq", "Frequency", "Hz", 0.0)
    .range(0.0, 6e9, 0.0)
    .section("Custom")
    .zero_means("The frequency of the session")
    .advanced();
/// The bandwidth of the custom setting.
pub const CUSTOM_BW: Setting<field::Choice<LoraBandwidth>> =
    Setting::choice("custom_bw", "Bandwidth", LoraBandwidth::Bw250)
        .section("Custom")
        .advanced();
/// The spreading factor of the custom setting.
pub const CUSTOM_SF: Setting<field::I64> = Setting::new("custom_sf", "Spreading factor", "", 11)
    .range(5.0, 12.0, 1.0)
    .section("Custom")
    .advanced();
/// The coding rate of the custom setting.
pub const CUSTOM_CR: Setting<field::Choice<CodingRate>> =
    Setting::choice("custom_cr", "Coding rate", CodingRate::Cr5)
        .section("Custom")
        .advanced();
/// The sync word of the custom setting.
pub const CUSTOM_SYNC: Setting<field::I64> = Setting::new("custom_sync", "Sync word", "", 0x2B)
    .range(0.0, 255.0, 1.0)
    .section("Custom")
    .help("0x2B (43) Meshtastic and MeshCom, 0x12 (18) MeshCore and LoRa APRS, 0x34 (52) LoRaWAN.")
    .advanced();
/// The preamble length of the custom setting.
pub const CUSTOM_PREAMBLE: Setting<field::I64> =
    Setting::new("custom_preamble", "Preamble", "", 16)
        .range(6.0, 65535.0, 1.0)
        .section("Custom")
        .advanced();

/// The lost-sample search.
pub const JUMP_SEARCH: Setting<field::Bool> =
    Setting::new("jump_search", "Lost-sample search", "", true)
        .section("Receive")
        .advanced();
/// The expected jump of the lost-sample search.
pub const JUMP_CHIPS: Setting<field::F64> =
    Setting::new("jump_chips", "Expected jump", "chips", 11.7)
        .range(0.0, 64.0, 0.1)
        .section("Receive")
        .advanced();

sdrkit_core::settings! {
    /// The settings of `mesh.channels`.
    pub enum MeshPlanSetting {
        /// Dial.
        DialFreq = DIAL_FREQ,
        /// Protocols.
        Protocols = PROTOCOLS,
        /// Every Meshtastic preset.
        PresetSearch = PRESET_SEARCH,
        /// Slot band start.
        MtSlotStart = MT_SLOT_START,
        /// Slot band end.
        MtSlotEnd = MT_SLOT_END,
        /// Channels.
        Channels = CHANNELS,
        /// LoRaWAN channels.
        LwChannels = LW_CHANNELS,
        /// Meshtastic preset.
        MtPreset = MT_PRESET,
        /// Primary channel name.
        MtChannelName = MT_CHANNEL_NAME,
        /// Channel number.
        MtChannelNum = MT_CHANNEL_NUM,
        /// Frequency override.
        MtOverrideFreq = MT_OVERRIDE_FREQ,
        /// Frequency offset.
        MtFreqOffset = MT_FREQ_OFFSET,
        /// Custom LoRa setting.
        Custom = CUSTOM,
        /// Frequency.
        CustomFreq = CUSTOM_FREQ,
        /// Bandwidth.
        CustomBw = CUSTOM_BW,
        /// Spreading factor.
        CustomSf = CUSTOM_SF,
        /// Coding rate.
        CustomCr = CUSTOM_CR,
        /// Sync word.
        CustomSync = CUSTOM_SYNC,
        /// Preamble.
        CustomPreamble = CUSTOM_PREAMBLE,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Expected jump.
        JumpChips = JUMP_CHIPS,
    }
}

// ---- The mesh decoder (`mesh.decode`) ----

/// Raw payloads in the packet list.
pub const RAW: Setting<field::Bool> = Setting::new("raw", "Raw payload", "", false)
    .section("Receive")
    .advanced();
/// The computer clock at the first sample.
pub const CLOCK0: Setting<field::F64> =
    Setting::new("clock0", "Computer clock at signal 0", "s", 0.0)
        .range(0.0, 1e12, 0.0)
        .section("Receive")
        .help("Seconds since 1970 at the first sample. 0 when the times follow the signal.")
        .advanced();

sdrkit_core::settings! {
    /// The settings of `mesh.decode`.
    pub enum MeshDecodeSetting {
        /// Raw payload.
        Raw = RAW,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
    }
}

// ---- The Meshtastic client (`mt.client`) ----

/// The own node id.
pub const MT_NODE_ID: Setting<field::Text> = Setting::new("mt_node_id", "Node ID", "", "")
    .section("Meshtastic")
    .help("\"!xxxxxxxx\". Empty takes one from the callsign of the station, or a random one.")
    .advanced();
/// The own long name.
pub const MT_LONG_NAME: Setting<field::Text> =
    Setting::new("mt_long_name", "Long name", "", "sdrkit").section("Meshtastic");
/// The own short name.
pub const MT_SHORT_NAME: Setting<field::Text> =
    Setting::new("mt_short_name", "Short name", "", "SDRK").section("Meshtastic");
/// The role of the own node.
pub const MT_ROLE: Setting<field::Choice<Role>> =
    Setting::choice("mt_role", "Role", Role::ClientMute).section("Meshtastic");
/// Licensed (ham) mode.
pub const MT_LICENSED: Setting<field::Bool> =
    Setting::new("mt_licensed", "Licensed (ham) mode", "", false)
        .section("Meshtastic")
        .help("No encryption, and the callsign of the station is the long name.")
        .advanced();
/// Put the callsign in front of own texts.
pub const MT_CALLSIGN_PREFIX: Setting<field::Bool> =
    Setting::new("mt_callsign_prefix", "Callsign prefix", "", false)
        .section("Meshtastic")
        .advanced();
/// The hop limit of own packets.
pub const MT_HOP_LIMIT: Setting<field::I64> = Setting::new("mt_hop_limit", "Hop limit", "", 3)
    .range(0.0, 7.0, 1.0)
    .section("Meshtastic")
    .advanced();
/// The NodeInfo interval.
pub const MT_NODEINFO_INTERVAL: Setting<field::F64> =
    Setting::new("mt_nodeinfo_interval", "NodeInfo interval", "s", 10800.0)
        .range(0.0, 1e6, 1.0)
        .section("Meshtastic")
        .help("0 sends it only when asked.")
        .advanced();
/// Where the own position comes from.
pub const MT_POSITION_MODE: Setting<field::Choice<PositionMode>> =
    Setting::choice("mt_position_mode", "Position", PositionMode::Off).section("Meshtastic");
/// The position interval.
pub const MT_POSITION_INTERVAL: Setting<field::F64> =
    Setting::new("mt_position_interval", "Position interval", "s", 900.0)
        .range(30.0, 1e6, 1.0)
        .section("Meshtastic")
        .advanced();
/// Want an acknowledgement for own channel texts.
pub const MT_ACK_BROADCAST: Setting<field::Bool> =
    Setting::new("mt_ack_broadcast", "Want-ack on channel texts", "", true)
        .section("Meshtastic")
        .advanced();

sdrkit_core::settings! {
    /// The settings of `mt.client`.
    pub enum MtClientSetting {
        /// Node ID.
        NodeId = MT_NODE_ID,
        /// Long name.
        LongName = MT_LONG_NAME,
        /// Short name.
        ShortName = MT_SHORT_NAME,
        /// Role.
        Role = MT_ROLE,
        /// Licensed (ham) mode.
        Licensed = MT_LICENSED,
        /// Callsign prefix.
        CallsignPrefix = MT_CALLSIGN_PREFIX,
        /// Primary channel name.
        ChannelName = MT_CHANNEL_NAME,
        /// Hop limit.
        HopLimit = MT_HOP_LIMIT,
        /// NodeInfo interval.
        NodeinfoInterval = MT_NODEINFO_INTERVAL,
        /// Position.
        PositionMode = MT_POSITION_MODE,
        /// Position interval.
        PositionInterval = MT_POSITION_INTERVAL,
        /// Want-ack on channel texts.
        AckBroadcast = MT_ACK_BROADCAST,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
    }
}

// ---- The MeshCore client (`mc.client`) ----

/// The own MeshCore name.
pub const MC_NAME: Setting<field::Text> =
    Setting::new("mc_name", "Name", "", "sdrkit").section("MeshCore");
/// Send the station's position in adverts.
pub const MC_SHARE_LOCATION: Setting<field::Bool> = Setting::new(
    "mc_share_location",
    "Send the station's position",
    "",
    false,
)
.section("MeshCore");
/// The advert interval.
pub const MC_ADVERT_INTERVAL: Setting<field::F64> =
    Setting::new("mc_advert_interval", "Advert interval", "s", 0.0)
        .range(0.0, 1e6, 1.0)
        .section("MeshCore")
        .help("0 sends an advert only when asked.")
        .advanced();

sdrkit_core::settings! {
    /// The settings of `mc.client`.
    pub enum McClientSetting {
        /// Name.
        Name = MC_NAME,
        /// Send the station's position.
        ShareLocation = MC_SHARE_LOCATION,
        /// Advert interval.
        AdvertInterval = MC_ADVERT_INTERVAL,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
    }
}

// ---- The client of the amateur protocols (`ham.client`) ----

/// The SSID of own APRS frames.
pub const APRS_SSID: Setting<field::I64> = Setting::new("aprs_ssid", "SSID", "", 0)
    .range(0.0, 15.0, 1.0)
    .zero_means("The callsign alone")
    .section("APRS")
    .help("The SSID after the callsign of the station in own frames. On AX.25 the callsign must then carry no SSID of its own.");
/// The destination of own APRS frames.
pub const APRS_TOCALL: Setting<field::Text> =
    Setting::new("aprs_tocall", "Destination", "", "APRS")
        .section("APRS")
        .advanced();
/// The digipeater path of own APRS frames.
pub const APRS_PATH: Setting<field::Text> = Setting::new("aprs_path", "Path", "", "WIDE1-1")
    .section("APRS")
    .help("Calls separated by commas, at most eight. Empty sends no path.")
    .advanced();
/// The map symbol of the own APRS beacon: the table character, then the code.
pub const APRS_SYMBOL: Setting<field::Text> = Setting::new("aprs_symbol", "Symbol", "", "/-")
    .section("APRS")
    .help("Two characters: the symbol table, then the symbol code.");
/// The comment of the own APRS beacon.
pub const APRS_COMMENT: Setting<field::Text> = Setting::new("aprs_comment", "Comment", "", "")
    .section("APRS")
    .help("The comment of the beacon. Without a position of the station the beacon sends this text as its status.");
/// The speed of own AX.25 frames.
pub const TX_BAUD: Setting<field::Choice<PacketSpeed>> =
    Setting::choice("tx_baud", "Speed", PacketSpeed::Afsk1200).section("APRS");
/// The flags before an own AX.25 frame.
pub const TX_DELAY: Setting<field::F64> = Setting::new("tx_delay", "TX delay", "s", 0.3)
    .range(0.01, 2.0, 0.01)
    .section("APRS")
    .help(
        "Flags before each frame, so that the receivers open their squelch and find the bit clock.",
    )
    .advanced();
/// Pre-emphasis of the own 1200 bit/s audio.
pub const TX_PRE_EMPHASIS: Setting<field::Bool> =
    Setting::new("tx_pre_emphasis", "Pre-emphasis", "", false)
        .section("APRS")
        .help(
            "Lifts the 2200 Hz tone over the 1200 Hz tone (6 dB an octave above 300 Hz), as the microphone path of an FM \
             radio does, for receivers with de-emphasis. Off sends both tones at the same level.",
        )
        .advanced();
/// The beacon interval.
pub const BEACON_INTERVAL: Setting<field::F64> =
    Setting::new("beacon_interval", "Beacon interval", "s", 0.0)
        .range(0.0, 1e6, 1.0)
        .zero_means("A beacon only when asked")
        .section("APRS")
        .help("At least 60 s.");

sdrkit_core::settings! {
    /// The settings of `ham.client`.
    pub enum HamClientSetting {
        /// SSID.
        AprsSsid = APRS_SSID,
        /// Destination.
        AprsTocall = APRS_TOCALL,
        /// Path.
        AprsPath = APRS_PATH,
        /// Symbol.
        AprsSymbol = APRS_SYMBOL,
        /// Comment.
        AprsComment = APRS_COMMENT,
        /// Speed.
        TxBaud = TX_BAUD,
        /// TX delay.
        TxDelay = TX_DELAY,
        /// Pre-emphasis.
        TxPreEmphasis = TX_PRE_EMPHASIS,
        /// Beacon interval.
        BeaconInterval = BEACON_INTERVAL,
    }
}

// ---- The transmit scheduler (`tx.sched`) ----

/// The duty-cycle limit.
pub const DUTY_LIMIT: Setting<field::Bool> =
    Setting::new("duty_limit", "Duty-cycle limit", "", true).section("Transmit");
/// The bands of the duty-cycle limit.
pub const DUTY_BANDS: Setting<field::Text> = Setting::new("duty_bands", "Duty-cycle bands", "", "")
    .section("Transmit")
    .help(
        "Entries separated by \";\", each the lower and the upper edge in Hz separated by \"-\", \"=\" and the limit in \
         percent of the air time of the last hour, for example \"100000000-100250000=10\". Empty: no band has a limit.",
    )
    .advanced();

sdrkit_core::settings! {
    /// The settings of `tx.sched`.
    pub enum SchedSetting {
        /// Duty-cycle limit.
        DutyLimit = DUTY_LIMIT,
        /// Duty-cycle bands.
        DutyBands = DUTY_BANDS,
    }
}

// ---- The mesh stacks ----

/// The scene of the mesh test signal.
pub const GEN_SCENE: Setting<field::Choice<GenScene>> =
    Setting::choice("gen_scene", "Test signal scene", GenScene::Auto)
        .section("Test signal")
        .advanced();
/// A Meshtastic text of the test signal comes first as a damaged copy and later as a good copy.
pub const GEN_DAMAGED_COPY: Setting<field::Bool> =
    Setting::new("gen_damaged_copy", "Damaged copy first", "", false)
        .section("Test signal")
        .help(
            "The texting scene sends one more Meshtastic text: first with one bit of its payload flipped, so that its \
             checksum fails, and 6 s later as a good copy. The text shows with crc_ok false, and the good copy \
             corrects it.",
        )
        .advanced();

sdrkit_core::settings! {
    /// The settings of `mesh`: every protocol, each in its section.
    pub enum MeshSetting {
        /// Dial.
        DialFreq = DIAL_FREQ,
        /// Protocols.
        Protocols = PROTOCOLS,
        /// Every Meshtastic preset.
        PresetSearch = PRESET_SEARCH,
        /// Slot band start.
        MtSlotStart = MT_SLOT_START,
        /// Slot band end.
        MtSlotEnd = MT_SLOT_END,
        /// Channels.
        Channels = CHANNELS,
        /// LoRaWAN channels.
        LwChannels = LW_CHANNELS,
        /// Meshtastic preset.
        MtPreset = MT_PRESET,
        /// Primary channel name.
        MtChannelName = MT_CHANNEL_NAME,
        /// Channel number.
        MtChannelNum = MT_CHANNEL_NUM,
        /// Frequency override.
        MtOverrideFreq = MT_OVERRIDE_FREQ,
        /// Frequency offset.
        MtFreqOffset = MT_FREQ_OFFSET,
        /// Custom LoRa setting.
        Custom = CUSTOM,
        /// Frequency.
        CustomFreq = CUSTOM_FREQ,
        /// Bandwidth.
        CustomBw = CUSTOM_BW,
        /// Spreading factor.
        CustomSf = CUSTOM_SF,
        /// Coding rate.
        CustomCr = CUSTOM_CR,
        /// Sync word.
        CustomSync = CUSTOM_SYNC,
        /// Preamble.
        CustomPreamble = CUSTOM_PREAMBLE,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Expected jump.
        JumpChips = JUMP_CHIPS,
        /// Raw payload.
        Raw = RAW,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
        /// Node ID.
        MtNodeId = MT_NODE_ID,
        /// Long name.
        MtLongName = MT_LONG_NAME,
        /// Short name.
        MtShortName = MT_SHORT_NAME,
        /// Role.
        MtRole = MT_ROLE,
        /// Licensed (ham) mode.
        MtLicensed = MT_LICENSED,
        /// Callsign prefix.
        MtCallsignPrefix = MT_CALLSIGN_PREFIX,
        /// Hop limit.
        MtHopLimit = MT_HOP_LIMIT,
        /// NodeInfo interval.
        MtNodeinfoInterval = MT_NODEINFO_INTERVAL,
        /// Position.
        MtPositionMode = MT_POSITION_MODE,
        /// Position interval.
        MtPositionInterval = MT_POSITION_INTERVAL,
        /// Want-ack on channel texts.
        MtAckBroadcast = MT_ACK_BROADCAST,
        /// Name.
        McName = MC_NAME,
        /// Send the station's position.
        McShareLocation = MC_SHARE_LOCATION,
        /// Advert interval.
        McAdvertInterval = MC_ADVERT_INTERVAL,
        /// SSID.
        AprsSsid = APRS_SSID,
        /// Destination.
        AprsTocall = APRS_TOCALL,
        /// Path.
        AprsPath = APRS_PATH,
        /// Symbol.
        AprsSymbol = APRS_SYMBOL,
        /// Comment.
        AprsComment = APRS_COMMENT,
        /// Duty-cycle limit.
        DutyLimit = DUTY_LIMIT,
        /// Duty-cycle bands.
        DutyBands = DUTY_BANDS,
        /// Test signal scene.
        GenScene = GEN_SCENE,
        /// Damaged copy first.
        GenDamagedCopy = GEN_DAMAGED_COPY,
    }
}

sdrkit_core::settings! {
    /// The settings of `meshtastic`.
    pub enum MeshtasticSetting {
        /// Dial.
        DialFreq = DIAL_FREQ,
        /// Every Meshtastic preset.
        PresetSearch = PRESET_SEARCH,
        /// Slot band start.
        MtSlotStart = MT_SLOT_START,
        /// Slot band end.
        MtSlotEnd = MT_SLOT_END,
        /// Meshtastic preset.
        MtPreset = MT_PRESET,
        /// Primary channel name.
        MtChannelName = MT_CHANNEL_NAME,
        /// Channel number.
        MtChannelNum = MT_CHANNEL_NUM,
        /// Frequency override.
        MtOverrideFreq = MT_OVERRIDE_FREQ,
        /// Frequency offset.
        MtFreqOffset = MT_FREQ_OFFSET,
        /// Custom LoRa setting.
        Custom = CUSTOM,
        /// Frequency.
        CustomFreq = CUSTOM_FREQ,
        /// Bandwidth.
        CustomBw = CUSTOM_BW,
        /// Spreading factor.
        CustomSf = CUSTOM_SF,
        /// Coding rate.
        CustomCr = CUSTOM_CR,
        /// Sync word.
        CustomSync = CUSTOM_SYNC,
        /// Preamble.
        CustomPreamble = CUSTOM_PREAMBLE,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Expected jump.
        JumpChips = JUMP_CHIPS,
        /// Raw payload.
        Raw = RAW,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
        /// Node ID.
        MtNodeId = MT_NODE_ID,
        /// Long name.
        MtLongName = MT_LONG_NAME,
        /// Short name.
        MtShortName = MT_SHORT_NAME,
        /// Role.
        MtRole = MT_ROLE,
        /// Licensed (ham) mode.
        MtLicensed = MT_LICENSED,
        /// Callsign prefix.
        MtCallsignPrefix = MT_CALLSIGN_PREFIX,
        /// Hop limit.
        MtHopLimit = MT_HOP_LIMIT,
        /// NodeInfo interval.
        MtNodeinfoInterval = MT_NODEINFO_INTERVAL,
        /// Position.
        MtPositionMode = MT_POSITION_MODE,
        /// Position interval.
        MtPositionInterval = MT_POSITION_INTERVAL,
        /// Want-ack on channel texts.
        MtAckBroadcast = MT_ACK_BROADCAST,
        /// Duty-cycle limit.
        DutyLimit = DUTY_LIMIT,
        /// Duty-cycle bands.
        DutyBands = DUTY_BANDS,
        /// Test signal scene.
        GenScene = GEN_SCENE,
        /// Damaged copy first.
        GenDamagedCopy = GEN_DAMAGED_COPY,
    }
}

sdrkit_core::settings! {
    /// The settings of `meshcore`. The Meshtastic slot band places the Meshtastic networks of its test signal.
    pub enum MeshcoreSetting {
        /// Dial.
        DialFreq = DIAL_FREQ,
        /// Slot band start.
        MtSlotStart = MT_SLOT_START,
        /// Slot band end.
        MtSlotEnd = MT_SLOT_END,
        /// Channels.
        Channels = CHANNELS,
        /// Custom LoRa setting.
        Custom = CUSTOM,
        /// Frequency.
        CustomFreq = CUSTOM_FREQ,
        /// Bandwidth.
        CustomBw = CUSTOM_BW,
        /// Spreading factor.
        CustomSf = CUSTOM_SF,
        /// Coding rate.
        CustomCr = CUSTOM_CR,
        /// Sync word.
        CustomSync = CUSTOM_SYNC,
        /// Preamble.
        CustomPreamble = CUSTOM_PREAMBLE,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Expected jump.
        JumpChips = JUMP_CHIPS,
        /// Raw payload.
        Raw = RAW,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
        /// Name.
        McName = MC_NAME,
        /// Send the station's position.
        McShareLocation = MC_SHARE_LOCATION,
        /// Advert interval.
        McAdvertInterval = MC_ADVERT_INTERVAL,
        /// Duty-cycle limit.
        DutyLimit = DUTY_LIMIT,
        /// Duty-cycle bands.
        DutyBands = DUTY_BANDS,
        /// Test signal scene.
        GenScene = GEN_SCENE,
    }
}

sdrkit_core::settings! {
    /// The settings of `lorawan`.
    pub enum LorawanSetting {
        /// Dial.
        DialFreq = DIAL_FREQ,
        /// LoRaWAN channels.
        LwChannels = LW_CHANNELS,
        /// Custom LoRa setting.
        Custom = CUSTOM,
        /// Frequency.
        CustomFreq = CUSTOM_FREQ,
        /// Bandwidth.
        CustomBw = CUSTOM_BW,
        /// Spreading factor.
        CustomSf = CUSTOM_SF,
        /// Coding rate.
        CustomCr = CUSTOM_CR,
        /// Sync word.
        CustomSync = CUSTOM_SYNC,
        /// Preamble.
        CustomPreamble = CUSTOM_PREAMBLE,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Expected jump.
        JumpChips = JUMP_CHIPS,
        /// Raw payload.
        Raw = RAW,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
        /// Test signal scene.
        GenScene = GEN_SCENE,
    }
}

sdrkit_core::settings! {
    /// The settings of `loraaprs`.
    pub enum LoraAprsSetting {
        /// Dial.
        DialFreq = DIAL_FREQ,
        /// Channels.
        Channels = CHANNELS,
        /// Custom LoRa setting.
        Custom = CUSTOM,
        /// Frequency.
        CustomFreq = CUSTOM_FREQ,
        /// Bandwidth.
        CustomBw = CUSTOM_BW,
        /// Spreading factor.
        CustomSf = CUSTOM_SF,
        /// Coding rate.
        CustomCr = CUSTOM_CR,
        /// Sync word.
        CustomSync = CUSTOM_SYNC,
        /// Preamble.
        CustomPreamble = CUSTOM_PREAMBLE,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Expected jump.
        JumpChips = JUMP_CHIPS,
        /// Raw payload.
        Raw = RAW,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
        /// SSID.
        AprsSsid = APRS_SSID,
        /// Destination.
        AprsTocall = APRS_TOCALL,
        /// Path.
        AprsPath = APRS_PATH,
        /// Symbol.
        AprsSymbol = APRS_SYMBOL,
        /// Comment.
        AprsComment = APRS_COMMENT,
        /// Duty-cycle limit.
        DutyLimit = DUTY_LIMIT,
        /// Duty-cycle bands.
        DutyBands = DUTY_BANDS,
        /// Test signal scene.
        GenScene = GEN_SCENE,
    }
}

sdrkit_core::settings! {
    /// The settings of `meshcom`.
    pub enum MeshcomSetting {
        /// Dial.
        DialFreq = DIAL_FREQ,
        /// Channels.
        Channels = CHANNELS,
        /// Custom LoRa setting.
        Custom = CUSTOM,
        /// Frequency.
        CustomFreq = CUSTOM_FREQ,
        /// Bandwidth.
        CustomBw = CUSTOM_BW,
        /// Spreading factor.
        CustomSf = CUSTOM_SF,
        /// Coding rate.
        CustomCr = CUSTOM_CR,
        /// Sync word.
        CustomSync = CUSTOM_SYNC,
        /// Preamble.
        CustomPreamble = CUSTOM_PREAMBLE,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Expected jump.
        JumpChips = JUMP_CHIPS,
        /// Raw payload.
        Raw = RAW,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
        /// Test signal scene.
        GenScene = GEN_SCENE,
    }
}

/// The settings of the mesh stack `stack` (`mesh` for another id), as its shape lists them.
pub fn mesh_settings(stack: &str) -> &'static [&'static dyn DeclareSetting] {
    match stack {
        "meshtastic" => MeshtasticSetting::SETTINGS,
        "meshcore" => MeshcoreSetting::SETTINGS,
        "lorawan" => LorawanSetting::SETTINGS,
        "loraaprs" => LoraAprsSetting::SETTINGS,
        "meshcom" => MeshcomSetting::SETTINGS,
        _ => MeshSetting::SETTINGS,
    }
}

/// The schema of the mesh stack `stack` (`mesh` for another id).
pub fn mesh_schema(stack: &str) -> Arc<Schema> {
    match stack {
        "meshtastic" => MeshtasticSetting::schema(),
        "meshcore" => MeshcoreSetting::schema(),
        "lorawan" => LorawanSetting::schema(),
        "loraaprs" => LoraAprsSetting::schema(),
        "meshcom" => MeshcomSetting::schema(),
        _ => MeshSetting::schema(),
    }
}

// ---- `lorawanwide` ----

/// The middle of the channels of `lorawanwide`.
pub const WIDE_DIAL_FREQ: Setting<field::Hz> = Setting::new("dial_freq", "Dial", "Hz", 0.0)
        .section("Channels")
    .range(0.0, 6e9, 0.0)
    .zero_means("The tuned frequency")
    .help("The middle of the channels: the centre of the capture when the input does not state it, and the dial of the test signal.")
    .advanced();
/// The FSK channel of the plan.
pub const FSK_FREQ: Setting<field::Hz> = Setting::new("fsk_freq", "FSK frequency", "Hz", 0.0)
    .section("Channels")
    .range(0.0, 6e9, 0.0)
    .zero_means("The plan has no FSK channel")
    .advanced();
/// Downlinks too.
pub const DOWNLINKS: Setting<field::Bool> =
    Setting::new("downlinks", "Downlinks", "", true).section("Channels");
/// The default channels only.
pub const DEFAULTS_ONLY: Setting<field::Bool> =
    Setting::new("defaults_only", "Default channels only", "", false).section("Channels");
/// The private sync word 0x12 too.
pub const PRIVATE_SYNC: Setting<field::Bool> =
    Setting::new("private_sync", "Private networks (0x12)", "", false).section("Receive");
/// Channels at most.
pub const MAX_CHANNELS: Setting<field::I64> =
    Setting::new("max_channels", "Channels at most", "", 24)
        .section("Channels")
        .range(1.0, 72.0, 1.0);
/// The 250 and 500 kHz channels.
pub const WIDE_CHANNELS: Setting<field::Bool> =
    Setting::new("wide_channels", "250 and 500 kHz channels", "", true).section("Channels");
/// The FSK channel.
pub const FSK: Setting<field::Bool> =
    Setting::new("fsk", "FSK channel", "", true).section("Channels");

sdrkit_core::settings! {
    /// The settings of `lorawan.channels`.
    pub enum WidePlanSetting {
        /// Dial.
        DialFreq = WIDE_DIAL_FREQ,
        /// LoRaWAN channels.
        LwChannels = LW_CHANNELS,
        /// Downlinks.
        Downlinks = DOWNLINKS,
        /// Default channels only.
        DefaultsOnly = DEFAULTS_ONLY,
        /// Private networks (0x12).
        PrivateSync = PRIVATE_SYNC,
        /// Channels at most.
        MaxChannels = MAX_CHANNELS,
        /// 250 and 500 kHz channels.
        WideChannels = WIDE_CHANNELS,
        /// FSK channel.
        Fsk = FSK,
        /// FSK frequency.
        FskFreq = FSK_FREQ,
    }
}

sdrkit_core::settings! {
    /// The settings of `lorawanwide`.
    pub enum WideSetting {
        /// Dial.
        DialFreq = WIDE_DIAL_FREQ,
        /// LoRaWAN channels.
        LwChannels = LW_CHANNELS,
        /// Downlinks.
        Downlinks = DOWNLINKS,
        /// Default channels only.
        DefaultsOnly = DEFAULTS_ONLY,
        /// Private networks (0x12).
        PrivateSync = PRIVATE_SYNC,
        /// Channels at most.
        MaxChannels = MAX_CHANNELS,
        /// 250 and 500 kHz channels.
        WideChannels = WIDE_CHANNELS,
        /// FSK channel.
        Fsk = FSK,
        /// FSK frequency.
        FskFreq = FSK_FREQ,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
    }
}

// ---- `loraraw` ----

/// Search SF7 to SF12.
pub const AUTO_SF: Setting<field::Bool> =
    Setting::new("auto_sf", "Search SF7 to SF12", "", true).section("LoRa");
/// The spreading factor.
pub const SF: Setting<field::I64> = Setting::new("sf", "Spreading factor", "", 7)
    .section("LoRa")
    .range(5.0, 12.0, 1.0);
/// Search 125, 250 and 500 kHz.
pub const AUTO_BW: Setting<field::Bool> =
    Setting::new("auto_bw", "Search 125, 250 and 500 kHz", "", true).section("LoRa");
/// The bandwidth.
pub const BW: Setting<field::Choice<LoraBandwidth>> =
    Setting::choice("bw", "Bandwidth", LoraBandwidth::Bw125).section("LoRa");
/// Implicit header.
pub const IMPLICIT: Setting<field::Bool> =
    Setting::new("implicit", "Implicit header", "", false).section("LoRa");
/// The coding rate denominator.
pub const CR: Setting<field::I64> = Setting::new("cr", "Coding rate", "", 5)
    .section("LoRa")
    .range(5.0, 8.0, 1.0)
    .help("5 is 4/5, 8 is 4/8.");
/// The frames have a CRC.
pub const CRC: Setting<field::Bool> = Setting::new("crc", "CRC", "", true).section("LoRa");
/// The length of an implicit frame.
pub const LENGTH: Setting<field::I64> = Setting::new("length", "Length", "bytes", 16)
    .section("LoRa")
    .range(1.0, 255.0, 1.0);
/// The sync words searched.
pub const SYNC: Setting<field::Choice<RawSync>> =
    Setting::choice("sync", "Sync word", RawSync::Private).section("LoRa");
/// The custom sync word.
pub const SYNC_CUSTOM: Setting<field::I64> =
    Setting::new("sync_custom", "Custom sync word", "", 0x12)
        .section("LoRa")
        .range(0.0, 255.0, 1.0)
        .help("Used when the sync word is custom.");
/// The preamble length.
pub const PREAMBLE: Setting<field::I64> = Setting::new("preamble", "Preamble", "", 8)
    .section("LoRa")
    .range(6.0, 65.0, 1.0);
/// Low data rate optimisation.
pub const LDRO: Setting<field::Choice<LdroChoice>> =
    Setting::choice("ldro", "LDRO", LdroChoice::Auto).section("LoRa");
/// The channel's offset from the dial.
pub const OFFSET: Setting<field::Hz> = Setting::new("offset", "Offset from the dial", "Hz", 0.0)
    .section("LoRa")
    .range(-1e6, 1e6, 100.0);

sdrkit_core::settings! {
    /// The settings of `loraraw.channels`.
    pub enum RawPlanSetting {
        /// Search SF7 to SF12.
        AutoSf = AUTO_SF,
        /// Spreading factor.
        Sf = SF,
        /// Search 125, 250 and 500 kHz.
        AutoBw = AUTO_BW,
        /// Bandwidth.
        Bw = BW,
        /// Implicit header.
        Implicit = IMPLICIT,
        /// Coding rate.
        Cr = CR,
        /// CRC.
        Crc = CRC,
        /// Length.
        Length = LENGTH,
        /// Sync word.
        Sync = SYNC,
        /// Custom sync word.
        SyncCustom = SYNC_CUSTOM,
        /// Preamble.
        Preamble = PREAMBLE,
        /// LDRO.
        Ldro = LDRO,
        /// Offset from the dial.
        Offset = OFFSET,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
    }
}

/// Send the queued frame again after this time.
pub const TX_REPEAT: Setting<field::Duration> =
    Setting::new("tx_repeat_s", "Repeat every", "s", 0.0)
        .range(0.0, 3600.0, 0.1)
        .section("Transmit")
        .help("0 sends each frame once.");

sdrkit_core::settings! {
    /// The settings of `loraraw.log`.
    pub enum RawLogSetting {
        /// Repeat every.
        TxRepeat = TX_REPEAT,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
    }
}

/// The implicit-header scene of the test signal.
pub const GEN_IMPLICIT: Setting<field::Bool> =
    Setting::new("gen_implicit", "Implicit scene", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of `loraraw`.
    pub enum RawSetting {
        /// Search SF7 to SF12.
        AutoSf = AUTO_SF,
        /// Spreading factor.
        Sf = SF,
        /// Search 125, 250 and 500 kHz.
        AutoBw = AUTO_BW,
        /// Bandwidth.
        Bw = BW,
        /// Implicit header.
        Implicit = IMPLICIT,
        /// Coding rate.
        Cr = CR,
        /// CRC.
        Crc = CRC,
        /// Length.
        Length = LENGTH,
        /// Sync word.
        Sync = SYNC,
        /// Custom sync word.
        SyncCustom = SYNC_CUSTOM,
        /// Preamble.
        Preamble = PREAMBLE,
        /// LDRO.
        Ldro = LDRO,
        /// Offset from the dial.
        Offset = OFFSET,
        /// Lost-sample search.
        JumpSearch = JUMP_SEARCH,
        /// Repeat every.
        TxRepeat = TX_REPEAT,
        /// Computer clock at signal 0.
        Clock0 = CLOCK0,
        /// Implicit scene.
        GenImplicit = GEN_IMPLICIT,
    }
}

// ---- `aprs` ----

/// The 12.5 kHz channel filter.
pub const NARROW: Setting<field::Bool> =
    Setting::new("narrow", "12.5 kHz channel", "", false).section("Receive");
/// The emphasis filter.
pub const EMPHASIS: Setting<field::Choice<Emphasis>> =
    Setting::choice("emphasis", "Emphasis", Emphasis::None).section("Receive");
/// The input is FM-demodulated audio.
pub const AUDIO_INPUT: Setting<field::Bool> = Setting::new("audio_input", "Audio input", "", false)
    .section("Receive")
    .help("The input is FM-demodulated audio: no minimum rate.");
/// The discriminator audio goes to the listen output.
pub const LISTEN: Setting<field::Bool> =
    Setting::new("listen", "Listen", "", false).section("Receive");

sdrkit_core::settings! {
    /// The settings of `aprs.demod`.
    pub enum AprsDemodSetting {
        /// 12.5 kHz channel.
        Narrow = NARROW,
        /// Emphasis.
        Emphasis = EMPHASIS,
        /// Audio input.
        AudioInput = AUDIO_INPUT,
        /// Listen.
        Listen = LISTEN,
    }
}

/// Messages to the station must match the SSID of its call.
pub const MATCH_SSID: Setting<field::Bool> =
    Setting::new("match_ssid", "Match the SSID", "", false)
        .section("APRS")
        .help("Off: a message to the callsign with any SSID is for the station.");

sdrkit_core::settings! {
    /// The settings of `aprs.tables`.
    pub enum AprsTablesSetting {
        /// Match the SSID.
        MatchSsid = MATCH_SSID,
    }
}

/// One carrier from a channel bank: no minimum rate.
pub const WIDE_INPUT: Setting<field::Bool> = Setting::new("wide_input", "Wide input", "", false)
    .section("Receive")
    .help("One carrier at 192 ksps from a channel bank: no minimum rate.");
/// The packet speeds of the test signal.
pub const GEN_SPEEDS: Setting<field::Choice<Speeds>> =
    Setting::choice("gen_option", "Test speeds", Speeds::Both)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of `aprs`.
    pub enum AprsSetting {
        /// 12.5 kHz channel.
        Narrow = NARROW,
        /// Emphasis.
        Emphasis = EMPHASIS,
        /// Audio input.
        AudioInput = AUDIO_INPUT,
        /// Listen.
        Listen = LISTEN,
        /// SSID.
        AprsSsid = APRS_SSID,
        /// Destination.
        AprsTocall = APRS_TOCALL,
        /// Path.
        AprsPath = APRS_PATH,
        /// Symbol.
        AprsSymbol = APRS_SYMBOL,
        /// Comment.
        AprsComment = APRS_COMMENT,
        /// Speed.
        TxBaud = TX_BAUD,
        /// TX delay.
        TxDelay = TX_DELAY,
        /// Pre-emphasis.
        TxPreEmphasis = TX_PRE_EMPHASIS,
        /// Beacon interval.
        BeaconInterval = BEACON_INTERVAL,
        /// Match the SSID.
        MatchSsid = MATCH_SSID,
        /// Wide input.
        WideInput = WIDE_INPUT,
        /// Test speeds.
        GenSpeeds = GEN_SPEEDS,
    }
}

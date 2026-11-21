//! The settings of the four stacks, each once as a typed constant, and the schema of each stack. The settings of the "Test
//! signal" section (`gen_*`) configure the generator only. The framework adds the standard impairments.

use crate::checks::Correction;
use crate::demod::EffortChoice;
use crate::feeds::AsterixCategories;
use sdrkit_core::publish::field;
use sdrkit_core::Setting;

/// How long an address stays known after its last trusted message.
pub const EXPIRY_S: Setting<field::F64> = Setting::new("expiry_s", "Expiry time", "s", 60.0)
    .range(5.0, 3600.0, 1.0)
    .help("How long an aircraft's address stays known after its last trusted message.")
    .section("Decoding");

/// The error correction of extended squitters and all-call replies.
pub const CORRECTION: Setting<field::Choice<Correction>> =
    Setting::choice("correction", "Error correction", Correction::Three)
        .help("Repairs extended squitters and all-call replies. Up to 3 bits finds more messages but lets more false ones through.")
        .section("Decoding");
/// Repair frames of addresses not heard yet.
pub const REPAIR_UNKNOWN: Setting<field::Bool> = Setting::new(
    "repair_unknown",
    "Repair unknown addresses",
    "",
    false,
)
.help("Also repair frames from aircraft not heard yet. Raises the chance of a false aircraft.")
.section("Decoding");
/// The extra decoding passes for weak signals.
pub const EFFORT: Setting<field::Choice<EffortChoice>> = Setting::choice(
    "effort",
    "Effort",
    EffortChoice::Auto,
)
.help("Extra decoding passes for weak signals. Automatic steps down when the computer is busy.")
.section("Decoding");
/// Decode Mode A/C replies beside Mode S.
pub const MODEAC: Setting<field::Bool> = Setting::new("modeac", "Mode A/C replies", "", false)
    .help("Also decode Mode A/C replies (squawk and Mode C altitude) on 1090 MHz.")
    .section("Decoding");

/// The feeds from other receivers.
pub const FEEDS_IN: Setting<field::Text> =
    Setting::new("feeds_in", "Feeds from other receivers", "", "")
        .help("host:port of Beast, SBS or ASTERIX sources, separated by commas.")
        .section("Feeds");
/// Feeds in may be outside the local network.
pub const ALLOW_REMOTE: Setting<field::Bool> = Setting::new(
    "allow_remote",
    "Allow hosts outside the local network",
    "",
    false,
)
.section("Feeds");
/// The Beast server.
pub const BEAST_SERVER: Setting<field::Bool> =
    Setting::new("beast_server", "Beast server", "", false)
        .help("Beast binary on TCP port 30005.")
        .section("Feeds");
/// The port of the Beast server. If 0, the system chooses a free port.
pub const BEAST_PORT: Setting<field::I64> = Setting::new("beast_port", "Beast port", "", 30005)
    .range(0.0, 65535.0, 1.0)
    .help("0 asks the system for a free port.")
    .section("Feeds");
/// The SBS server.
pub const SBS_SERVER: Setting<field::Bool> = Setting::new("sbs_server", "SBS server", "", false)
    .help("BaseStation text lines on TCP port 30003.")
    .section("Feeds");
/// The port of the SBS server. If 0, the system chooses a free port.
pub const SBS_PORT: Setting<field::I64> = Setting::new("sbs_port", "SBS port", "", 30003)
    .range(0.0, 65535.0, 1.0)
    .help("0 asks the system for a free port.")
    .section("Feeds");
/// The ASTERIX server.
pub const ASTERIX_SERVER: Setting<field::Bool> =
    Setting::new("asterix_server", "ASTERIX server", "", false)
        .help("EUROCONTROL ASTERIX categories 021, 048 and 062 on TCP.")
        .section("Feeds");
/// The port of the ASTERIX server. If 0, the system chooses a free port.
pub const ASTERIX_PORT: Setting<field::I64> =
    Setting::new("asterix_port", "ASTERIX port", "", 30010)
        .range(0.0, 65535.0, 1.0)
        .section("Feeds");
/// The ASTERIX categories the server sends.
pub const ASTERIX_CAT: Setting<field::Choice<AsterixCategories>> =
    Setting::choice("asterix_cat", "ASTERIX categories", AsterixCategories::All).section("Feeds");
/// The system area code of the ASTERIX data source.
pub const SAC: Setting<field::I64> = Setting::new("sac", "ASTERIX SAC", "", 0)
    .range(0.0, 255.0, 1.0)
    .section("Feeds");
/// The system identification code of the ASTERIX data source.
pub const SIC: Setting<field::I64> = Setting::new("sic", "ASTERIX SIC", "", 1)
    .range(0.0, 255.0, 1.0)
    .section("Feeds");
/// The servers take clients of the local network.
pub const ALLOW_LAN: Setting<field::Bool> =
    Setting::new("allow_lan", "Servers accept the local network", "", false)
        .help("Otherwise only programs on this computer can connect.")
        .section("Feeds");
/// The outbound targets.
pub const TARGETS: Setting<field::Text> = Setting::new("targets", "Outbound targets", "", "")
    .help("format@host:port, separated by ';': beast, sbs, asterix21, asterix48 or asterix62. Add /udp for UDP and #text for a first line.")
    .section("Feeds");

/// The silence after which an aircraft leaves the table.
pub const DROP_S: Setting<field::F64> = Setting::new("drop_s", "Drop time", "s", 60.0)
    .range(0.0, 3600.0, 1.0)
    .zero_means("Never")
    .help("Aircraft leave the table after this silence.")
    .section("Display");
/// The GPS interference layer.
pub const GPS_LAYER: Setting<field::Bool> =
    Setting::new("gps_layer", "GPS interference layer", "", false).section("Display");

/// The number of aircraft of the ADS-B test signal.
pub const GEN_AIRCRAFT: Setting<field::I64> = Setting::new("gen_aircraft", "Aircraft", "", 12)
    .range(0.0, 300.0, 1.0)
    .restart()
    .section("Test signal")
    .advanced();
/// The test signal holds radar replies.
pub const GEN_REPLIES: Setting<field::Bool> =
    Setting::new("gen_replies", "Radar replies", "", true)
        .restart()
        .section("Test signal")
        .advanced();
/// One aircraft of the test signal declares an emergency.
pub const GEN_EMERGENCY: Setting<field::Bool> =
    Setting::new("gen_emergency", "Emergency demonstration", "", false)
        .restart()
        .section("Test signal")
        .advanced();
/// A factor on the message rate of the test signal.
pub const GEN_RATE: Setting<field::F64> =
    Setting::new("gen_rate", "Message rate multiplier", "", 1.0)
        .range(0.1, 20.0, 0.0)
        .section("Test signal")
        .advanced();
/// The width of the filter that shapes the pulse edges. 0 is automatic.
pub const GEN_FILTER_MHZ: Setting<field::F64> =
    Setting::new("gen_filter_mhz", "Pulse filter width", "MHz", 0.0)
        .range(0.0, 5.0, 0.0)
        .help("The filter that shapes the pulse edges. 0 for automatic (about seven eighths of the sample rate).")
        .restart()
        .section("Test signal")
    .advanced();
/// The latitude of the centre of the test airspace.
pub const GEN_CENTRE_LAT: Setting<field::F64> =
    Setting::new("gen_centre_lat", "Airspace centre latitude", "°", 25.25)
        .range(-90.0, 90.0, 0.0)
        .restart()
        .section("Test signal")
        .advanced();
/// The longitude of the centre of the test airspace.
pub const GEN_CENTRE_LON: Setting<field::F64> =
    Setting::new("gen_centre_lon", "Airspace centre longitude", "°", 55.36)
        .range(-180.0, 180.0, 0.0)
        .restart()
        .section("Test signal")
        .advanced();

/// The number of aircraft of the UAT test signal.
pub const UAT_GEN_AIRCRAFT: Setting<field::I64> = Setting::new("gen_aircraft", "Aircraft", "", 6)
    .range(0.0, 100.0, 1.0)
    .restart()
    .section("Test signal")
    .advanced();
/// The UAT test signal holds ground uplink frames.
pub const GEN_UPLINK: Setting<field::Bool> = Setting::new("gen_uplink", "Uplink frames", "", true)
    .restart()
    .section("Test signal")
    .advanced();
/// The UAT test signal sends the example frame of the standard in bursts.
pub const GEN_EXAMPLE: Setting<field::Bool> =
    Setting::new("gen_example", "Example frame only", "", false)
        .help("Sends the example frame of the standard in bursts instead of an airspace.")
        .restart()
        .section("Test signal")
        .advanced();
/// The number of bursts of the example frame.
pub const GEN_BURSTS: Setting<field::I64> = Setting::new("gen_bursts", "Example bursts", "", 4)
    .range(1.0, 20.0, 1.0)
    .restart()
    .section("Test signal")
    .advanced();

/// The replies before a Mode A/C code is shown.
pub const MIN_REPLIES: Setting<field::I64> =
    Setting::new("min_replies", "Replies before a code is shown", "", 2)
        .range(1.0, 20.0, 1.0)
        .section("Decoding");
/// The number of transponders of the Mode A/C test signal.
pub const GEN_TARGETS: Setting<field::I64> = Setting::new("gen_targets", "Transponders", "", 6)
    .range(1.0, 50.0, 1.0)
    .section("Test signal")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the `adsb` stack.
    pub enum AdsbSetting {
        /// Error correction.
        Correction = CORRECTION,
        /// Repair unknown addresses.
        RepairUnknown = REPAIR_UNKNOWN,
        /// Expiry time.
        ExpiryS = EXPIRY_S,
        /// Effort.
        Effort = EFFORT,
        /// Mode A/C replies.
        Modeac = MODEAC,
        /// Feeds from other receivers.
        FeedsIn = FEEDS_IN,
        /// Hosts outside the local network.
        AllowRemote = ALLOW_REMOTE,
        /// Beast server.
        BeastServer = BEAST_SERVER,
        /// Beast port.
        BeastPort = BEAST_PORT,
        /// SBS server.
        SbsServer = SBS_SERVER,
        /// SBS port.
        SbsPort = SBS_PORT,
        /// ASTERIX server.
        AsterixServer = ASTERIX_SERVER,
        /// ASTERIX port.
        AsterixPort = ASTERIX_PORT,
        /// ASTERIX categories.
        AsterixCat = ASTERIX_CAT,
        /// ASTERIX SAC.
        Sac = SAC,
        /// ASTERIX SIC.
        Sic = SIC,
        /// Servers accept the local network.
        AllowLan = ALLOW_LAN,
        /// Outbound targets.
        Targets = TARGETS,
        /// Drop time.
        DropS = DROP_S,
        /// GPS interference layer.
        GpsLayer = GPS_LAYER,
        /// Test signal: aircraft.
        GenAircraft = GEN_AIRCRAFT,
        /// Test signal: radar replies.
        GenReplies = GEN_REPLIES,
        /// Test signal: emergency.
        GenEmergency = GEN_EMERGENCY,
        /// Test signal: message rate.
        GenRate = GEN_RATE,
        /// Test signal: pulse filter.
        GenFilterMhz = GEN_FILTER_MHZ,
        /// Test signal: centre latitude.
        GenCentreLat = GEN_CENTRE_LAT,
        /// Test signal: centre longitude.
        GenCentreLon = GEN_CENTRE_LON,
    }
}

sdrkit_core::settings! {
    /// The settings of the `uat` stack.
    pub enum UatSetting {
        /// Expiry time.
        ExpiryS = EXPIRY_S,
        /// Test signal: aircraft.
        GenAircraft = UAT_GEN_AIRCRAFT,
        /// Test signal: uplink frames.
        GenUplink = GEN_UPLINK,
        /// Test signal: the example frame only.
        GenExample = GEN_EXAMPLE,
        /// Test signal: example bursts.
        GenBursts = GEN_BURSTS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `modeac` stack.
    pub enum ModeAcSetting {
        /// Expiry time.
        ExpiryS = EXPIRY_S,
        /// Replies before a code is shown.
        MinReplies = MIN_REPLIES,
        /// Test signal: transponders.
        GenTargets = GEN_TARGETS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `uplink1030` stack.
    pub enum UplinkSetting {
        /// Expiry time.
        ExpiryS = EXPIRY_S,
    }
}

//! Drone Remote ID (`remote_id`): Broadcast Remote ID on Bluetooth LE advertising channels and in Wi-Fi beacons (DSSS and OFDM),
//! from one 20 MHz capture of the 2.4 GHz band.
//!
//! Standard: ASTM F3411-22a, ASD-STAN prEN 4709-002.

pub mod generator;
pub mod messages;
pub mod phy;
pub mod table;
pub mod trust;

use crate::family::{min_rate_message, REPORT_S};
use messages::{Link, Payload, Transport, Versioned};
use phy::{BleRid, PhyStats, WifiChannel, WifiRid};
use sdrkit_core::publish::field;
use sdrkit_core::{Setting, StackConfig};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, Item, StackGraph};
use table::DroneTable;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 20e6;

sdrkit_core::choice_enum! {
    /// The 20 MHz bands of 2.4 GHz that the stack offers. The key is the centre in MHz.
    pub enum Band {
        /// BLE channel 38 and Wi-Fi channel 6.
        B2432 = ("2432", "BLE 38 + Wi-Fi 6 (2432 MHz)"),
        /// Wi-Fi channel 6.
        B2437 = ("2437", "Wi-Fi 6 (2437 MHz)"),
        /// Wi-Fi channel 1.
        B2412 = ("2412", "Wi-Fi 1 (2412 MHz)"),
        /// Wi-Fi channel 11.
        B2462 = ("2462", "Wi-Fi 11 (2462 MHz)"),
        /// BLE channel 37 and Wi-Fi channel 1.
        B2403 = ("2403", "BLE 37 + Wi-Fi 1 (2403 MHz)"),
        /// BLE channel 38.
        B2426 = ("2426", "BLE 38 (2426 MHz)"),
        /// BLE channel 39.
        B2478 = ("2478", "BLE 39 (2478 MHz)"),
    }
}

impl Band {
    /// Every band, in choice order.
    pub const ALL: [Band; 7] = [
        Band::B2432,
        Band::B2437,
        Band::B2412,
        Band::B2462,
        Band::B2403,
        Band::B2426,
        Band::B2478,
    ];
    /// The centre, MHz.
    pub fn centre_mhz(self) -> f64 {
        match self {
            Band::B2432 => 2432.0,
            Band::B2437 => 2437.0,
            Band::B2412 => 2412.0,
            Band::B2462 => 2462.0,
            Band::B2403 => 2403.0,
            Band::B2426 => 2426.0,
            Band::B2478 => 2478.0,
        }
    }
    /// The band whose centre is `centre_mhz`, or `None`.
    pub fn at(centre_mhz: f64) -> Option<Band> {
        Band::ALL
            .into_iter()
            .find(|b| (b.centre_mhz() - centre_mhz).abs() < 0.01)
    }
}

sdrkit_core::choice_enum! {
    /// The carriers of the test signal.
    pub enum GenCarriers {
        /// Bluetooth LE and Wi-Fi.
        Both = ("both", "both"),
        /// Bluetooth LE only.
        Ble = ("ble", "BLE only"),
        /// Wi-Fi only.
        Wifi = ("wifi", "Wi-Fi only"),
    }
}

sdrkit_core::choice_enum! {
    /// The Wi-Fi physical layer of the test signal.
    pub enum GenWifiPhy {
        /// 802.11b DSSS at 1 Mbit/s.
        Dsss = ("dsss", "DSSS 1 Mbit/s"),
        /// 802.11a/g OFDM at 6 Mbit/s.
        Ofdm = ("ofdm", "OFDM 6 Mbit/s"),
    }
}

/// The band. A change requests the host to retune.
pub const BAND: Setting<field::Choice<Band>> = Setting::choice("band", "Band", Band::B2432).help(
    "One 20 MHz band of 2.4 GHz is heard at a time. Drones advertise on all three BLE channels, and most send Wi-Fi beacons \
     on channel 6, so 2432 MHz hears both.",
);
/// Run the Bluetooth LE receiver.
pub const BLE: Setting<field::Bool> = Setting::new("ble", "Bluetooth LE", "", true)
    .help("Run the Bluetooth LE decoder when an advertising channel is in the band.");
/// Run the OFDM beacon receiver.
pub const WIFI_OFDM: Setting<field::Bool> =
    Setting::new("wifi_ofdm", "Wi-Fi OFDM beacons", "", true)
        .help("Run the OFDM beacon path beside the DSSS one.");
/// Flag broadcast addresses that share one radio.
pub const SHARED_RADIO: Setting<field::Bool> =
    Setting::new("shared_radio", "Shared-radio check", "", false)
        .help(
            "Flags drones as suspicious when several broadcast addresses keep one carrier offset (within 1 kHz, on the same \
             carrier) while they are heard: one radio behind several drones. Two real drones can share an offset by chance, \
             so the check is off by default.",
        )
        .advanced();
/// The carriers of the test signal.
pub const GEN_CARRIERS: Setting<field::Choice<GenCarriers>> =
    Setting::choice("gen_carriers", "Test signal carriers", GenCarriers::Both)
        .section("Test signal")
        .advanced();
/// The radio centre of the test signal.
pub const GEN_CENTRE_MHZ: Setting<field::F64> =
    Setting::new("gen_centre_mhz", "Test signal radio centre", "MHz", 2432.0)
        .range(0.0, 2500.0, 1.0)
        .help("0 places both carriers at 0 Hz (as a file without a frequency).")
        .section("Test signal")
        .advanced();
/// A spoofer in the test signal.
pub const GEN_SPOOFER: Setting<field::Bool> =
    Setting::new("gen_spoofer", "Test signal spoofer", "", false)
        .help(
            "Adds a drone on Bluetooth LE channel 38 whose position jumps 2 km at each Location message, so that the trust \
             check flags it as spoofed.",
        )
        .section("Test signal")
        .advanced();
/// The Wi-Fi physical layer of the test signal.
pub const GEN_WIFI_PHY: Setting<field::Choice<GenWifiPhy>> =
    Setting::choice("gen_wifi_phy", "Test signal Wi-Fi PHY", GenWifiPhy::Dsss)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `remote_id` stack.
    pub enum RidSetting {
        /// The band.
        Band = BAND,
        /// Run the Bluetooth LE receiver.
        Ble = BLE,
        /// Run the OFDM beacon receiver.
        WifiOfdm = WIFI_OFDM,
        /// Flag broadcast addresses that share one radio.
        SharedRadio = SHARED_RADIO,
        /// The carriers of the test signal.
        GenCarriers = GEN_CARRIERS,
        /// The radio centre of the test signal.
        GenCentreMhz = GEN_CENTRE_MHZ,
        /// The Wi-Fi physical layer of the test signal.
        GenWifiPhy = GEN_WIFI_PHY,
        /// A spoofer in the test signal.
        GenSpoofer = GEN_SPOOFER,
    }
}

/// The channel plan for a centre frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// The centre, MHz (0 when unknown).
    pub centre_mhz: f64,
    /// The BLE advertising channel (0 when the centre is unknown) and its offset from the centre in Hz; `None` when no advertising
    /// channel is within 8 MHz.
    pub ble: Option<(u8, f64)>,
    /// The Wi-Fi channel (0 when the centre is unknown) and its offset, Hz.
    pub wifi: (u8, f64),
}

/// The plan for a centre in Hz (0: unknown, a file).
pub fn plan(centre_hz: f64) -> Plan {
    if centre_hz == 0.0 {
        return Plan {
            centre_mhz: 0.0,
            ble: Some((0, 0.0)),
            wifi: (0, 0.0),
        };
    }
    let c = centre_hz / 1e6;
    let f = |n: u8| 2407.0 + 5.0 * n as f64;
    let near = |set: &mut dyn Iterator<Item = u8>| -> u8 {
        set.min_by(|&a, &b| (f(a) - c).abs().total_cmp(&(f(b) - c).abs()))
            .unwrap_or(1)
    };
    let mut w = near(&mut [1u8, 6, 11].into_iter());
    if (f(w) - c).abs() > 6.0 {
        w = near(&mut (1u8..=13));
    }
    let ble = [(37u8, 2402.0), (38, 2426.0), (39, 2480.0)]
        .iter()
        .filter(|(_, fc)| (fc - c).abs() <= 8.0)
        .min_by(|a, b| (a.1 - c).abs().total_cmp(&(b.1 - c).abs()))
        .map(|&(ch, fc)| (ch, (fc - c) * 1e6));
    Plan {
        centre_mhz: c,
        ble,
        wifi: (w, (f(w) - c) * 1e6),
    }
}

/// The Remote ID receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among
/// them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("ble", BleRid::default())
        .add("wifi", WifiChannel::default())
        .add("dsss", WifiRid::dsss())
        .add("ofdm", WifiRid::ofdm())
        .add("table", DroneTable::default())
        .input("ble")
        .input("wifi")
        .connect("wifi", "dsss")
        .connect("wifi", "ofdm")
        .input("table.clock")
        .connect("ble", "table.ble")
        .connect("dsss", "table.dsss")
        .connect("ofdm", "table.ofdm")
        .output("table.events")
        .output("table.requests")
        .clock(REPORT_S)
        .status_from("table")
        .reconfigure_on_retune()
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("remote_id"), graph)
        .min_rate(
            MIN_RATE_HZ,
            &min_rate_message("Drone Remote ID", MIN_RATE_HZ),
        )
        .route(BAND.key(), &["table.band"])?
        .route(SHARED_RADIO.key(), &["table.shared_radio"])?
        .route(BLE.key(), &["ble.enabled"])?
        .route(WIFI_OFDM.key(), &["ofdm.enabled"])?
        .command("clear", sdrkit_graph::CommandTarget::of("table", "clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("table", "remove"))
        .with_settings(&cfg.settings)
        .build()
}

/// A packet that carried Remote ID.
#[derive(Clone, Debug, PartialEq)]
pub struct RidPacket {
    /// BLE or Wi-Fi.
    pub transport: Transport,
    /// BLE 37, 38 or 39 (Wi-Fi: 0, the table takes the channel from the plan).
    pub channel: u8,
    /// The broadcast address (MAC text).
    pub address: String,
    /// The message counter.
    pub counter: u8,
    /// The messages, in order.
    pub messages: Vec<Versioned>,
    /// The air interface.
    pub link: Link,
    /// The first message or pack of the packet, which the trust check judges.
    pub payload: Payload,
    /// The header fields of the Wi-Fi beacon (`None` on Bluetooth LE).
    pub beacon: Option<remoteid::wifi::BeaconInfo>,
    /// Members of the packs that were refused.
    pub refused: u32,
    /// Messages with a value outside its range.
    pub out_of_range: u32,
    /// Level, dBFS (`None` when it was not measured).
    pub level_dbfs: Option<f64>,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// SNR, dB.
    pub snr_db: f64,
}

/// What a receive path passes to the table.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "a packet holds its payload inline, so that it needs no allocation"
)]
pub enum RidItem {
    /// A packet with Remote ID.
    Packet(RidPacket),
    /// A good packet without Remote ID.
    Plain(Transport),
    /// A receiver's counters.
    Stats(PhyStats),
}

impl Item for RidItem {
    const KIND: &'static str = "rid.message";
}

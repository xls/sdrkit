//! The Bluetooth LE advertising monitor (`ble`): the packets devices broadcast on the primary advertising channels (LE 1M and
//! LE Coded) and on the secondary channels and periodic trains inside the capture (all three PHYs, [`bank`]), extended
//! advertising followed through its pointers ([`follow`]), a device table and a packet log. The stack sends advertising
//! packets ([`tx`]) into a transmit sink.
//! Standard: Bluetooth Core Specification v5.4 Vol 6 Part B; Core Specification Supplement v11 Part A.

pub mod adv;
pub mod bank;
pub mod bench;
pub mod follow;
pub mod gen;
pub mod table;
pub mod tx;

use crate::testgen::{route_to, GEN_SECTION};
use adv::AdvInfo;
pub use bank::AdvDecoder;
use gen::GenWhitening;
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{
    Catalog, DecoderComponent, Graph, GraphBuilder, GraphError, Item, Registry, StackGraph,
    StackGraphBuilder,
};
use sdrkit_proto_wlan_phy::ble::{AdvPdu, Coding, LinkKind, Phy};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "ble";
/// The lowest input rate, Hz: one channel.
pub const MIN_RATE_HZ: f64 = 4e6;
/// The rate each channel's receiver runs at, Hz (4 samples per microsecond).
pub const RATE_HZ: f64 = 4e6;
/// The rate the stack prefers, Hz: with the radio 1 MHz above channel 37, the capture holds channel 37 and the secondary
/// channel 0. A wider rate takes more channels at once (all 40 from about 89 Msps).
pub const PREFERRED_RATE_HZ: f64 = 8e6;
/// The report period, s.
pub const REPORT_S: f64 = 0.25;
/// How near an advertising channel's centre the tuned frequency must be to give the channel hint, Hz.
pub(crate) const HINT_TOLERANCE_HZ: f64 = 0.1e6;

sdrkit_core::choice_enum! {
    /// A primary advertising channel. Standard: Bluetooth Core Specification v5.4 Vol 6 Part B clause 1.4.1.
    pub enum AdvChannel {
        /// Channel 37.
        C37 = ("37", "37  2402 MHz"),
        /// Channel 38.
        C38 = ("38", "38  2426 MHz"),
        /// Channel 39.
        C39 = ("39", "39  2480 MHz"),
    }
}

impl AdvChannel {
    /// The channel index (37, 38 or 39).
    pub fn number(self) -> u8 {
        match self {
            AdvChannel::C37 => 37,
            AdvChannel::C38 => 38,
            AdvChannel::C39 => 39,
        }
    }
}

/// A good advertising packet (the `ble.adv` stream): the PDU, what its advertising data says, and how it was received.
#[derive(Clone, Debug, PartialEq)]
pub struct Advert {
    /// Signal time of the end of the access address, s.
    pub time_s: f64,
    /// The channel whose whitening produced the good CRC (0 to 39).
    pub channel: u8,
    /// The level of the preamble and access address, dBFS.
    pub level_dbfs: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Fitted deviation, Hz.
    pub deviation_hz: f64,
    /// Eye signal-to-noise ratio, dB.
    pub snr_db: f64,
    /// The PDU (header and payload), dewhitened, without the CRC.
    pub pdu: Vec<u8>,
    /// What the advertising data says.
    pub info: AdvInfo,
    /// The one-line summary.
    pub summary: String,
    /// The PHY.
    pub phy: Phy,
    /// The coding on the LE Coded PHY.
    pub coding: Option<Coding>,
    /// The kind of the link the packet came on (advertising or a periodic train).
    pub kind: LinkKind,
    /// The access address.
    pub access_address: u32,
}

impl Advert {
    /// The parsed PDU (the receiver passes only PDUs that parse).
    pub fn parsed(&self) -> Option<AdvPdu<'_>> {
        AdvPdu::parse(&self.pdu).ok()
    }
}

impl Item for Advert {
    const KIND: &'static str = "ble.adv";
}

/// The receiver's counters and the channel level (the `ble.adv_stats` stream), sent after each piece of input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AdvStats {
    /// Signal time at the end of the piece, s.
    pub time_s: f64,
    /// Access-address matches with at most 2 bit errors.
    pub aa_hits: u64,
    /// Exact access-address matches whose CRC failed on every channel.
    pub crc_failures: u64,
    /// Good packets.
    pub packets: u64,
    /// The tuned advertising channel (0: not known, as for a recording).
    pub channel: u8,
    /// The level of the channel over the piece, dBFS.
    pub level_dbfs: Option<f64>,
    /// The BLE channels inside the capture, bit `c` for channel `c` (0 when the centre is not known).
    pub capture: u64,
    /// The periodic trains whose packets the receivers take.
    pub trains: u32,
}

impl Item for AdvStats {
    const KIND: &'static str = "ble.adv_stats";
}

/// The front of a BLE graph: the receivers of every channel inside the capture, `adv`.
///
/// # Errors
///
/// None at present: the form matches the other fronts of this crate.
pub fn front() -> Result<GraphBuilder, GraphError> {
    Ok(GraphBuilder::new()
        .add("adv", DecoderComponent::new(AdvDecoder::default()))
        .input("adv"))
}

/// Adds the advertising sender `sender` and the transmit sink `tx` to a BLE graph: the sender takes the graph's input and
/// writes into the sink.
pub fn with_sender(b: GraphBuilder, sender: tx::BleSender) -> GraphBuilder {
    b.add("sender", sender)
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("sender.in")
        .connect("sender.out", "tx.in")
        .output("sender.events")
}

/// Routes the transmit settings and the commands `send` and `halt` of a BLE stack to its sender.
///
/// # Errors
///
/// [`GraphError::Setting`] if the sender has no setting `tx_level`.
pub fn route_sender(b: StackGraphBuilder) -> Result<StackGraphBuilder, GraphError> {
    Ok(b.route(
        tx::TX_LEVEL.key(),
        &[&format!("sender.{}", tx::TX_LEVEL.key())],
    )?
    .command("send", sdrkit_graph::CommandTarget::of("sender", "send"))
    .command(
        crate::typed::BLE_HALT.key,
        sdrkit_graph::CommandTarget::of("sender", crate::typed::BLE_HALT.key),
    ))
}

/// The `ble` graph: the front, the device table, and the advertising sender.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    with_sender(front()?, tx::BleSender::ble())
        .add("table", table::DeviceTable::default())
        .connect("adv.adverts", "table.adverts")
        .connect("adv.stats", "table.stats")
        .output("table.events")
        .output("table.requests")
        .clock(REPORT_S)
        .status_from("table")
        .catalog(catalog)
        .build()
}

/// The test-signal setting `gen_channel`: the whitening of the packets.
pub const GEN_CHANNEL: Setting<field::Choice<GenWhitening>> =
    Setting::choice("gen_channel", "Whitening", GenWhitening::Round)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `ble` stack: the device table's and the test signal's.
    pub enum BleSetting {
        /// The advertising channel.
        Channel = table::CHANNEL_SETTING,
        /// Hop through the advertising channels.
        Hop = table::HOP,
        /// The dwell while hopping.
        HopSeconds = table::HOP_SECONDS,
        /// The device filter text.
        FilterText = table::FILTER_TEXT,
        /// The least level shown.
        MinLevel = table::MIN_LEVEL,
        /// Only named devices.
        NamedOnly = table::NAMED_ONLY,
        /// The level sent.
        TxLevel = tx::TX_LEVEL,
        /// The whitening of the test signal.
        GenChannel = GEN_CHANNEL,
    }
}

/// The `ble` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let b = StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?).min_rate(
        MIN_RATE_HZ,
        "Bluetooth LE needs a sample rate of at least 4 Msps",
    );
    let b = route_to(b, "table", &BleSetting::schema())?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("table", "remove"));
    route_sender(b)?.with_settings(&cfg.settings).build()
}

/// Adds the BLE components of this crate to a registry.
pub fn register(r: &mut Registry) {
    r.add("wlan.ble_adv", || {
        Box::new(DecoderComponent::new(AdvDecoder::default()))
    });
    r.add("wlan.ble_devices", || {
        Box::new(table::DeviceTable::default())
    });
    r.add("wlan.ble_tx", || Box::new(tx::BleSender::ble()));
    r.add("wlan.ble_tracker_tx", || Box::new(tx::BleSender::tracker()));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::BleGenerator::from_config(cfg)))
}

/// The `ble` stack.
pub static BLE: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Bluetooth LE",
    group: "Data and IoT",
    blurb: "Bluetooth LE advertising on LE 1M, LE 2M and LE Coded, legacy and extended, on every channel inside the capture: the devices around, their names, vendors and beacons, as a scanner app shows them.",
    tuning: Tuning {
        min_hz: 2400e6,
        max_hz: 2484e6,
        default_hz: 2402e6,
        rate_hz: PREFERRED_RATE_HZ,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 2e6,
        tune_offset_hz: 1e6,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: BleSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::BluetoothLe],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::BLE),
    benchmarks: Some(bench::benchmarks),
};

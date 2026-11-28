//! The Bluetooth LE monitor (`ble`): the packets devices broadcast on the primary advertising channels (LE 1M and
//! LE Coded) and on the secondary channels and periodic trains inside the capture (all three PHYs, [`bank`]), extended
//! advertising followed through its pointers ([`follow`]), a device table and a packet log. Every data channel inside the
//! capture is received as well: connections are followed from their start ([`conn`]) or found by their packets
//! ([`blind`], LE 1M and LE 2M; a connection on LE Coded is found only from its start). The stack sends advertising
//! packets ([`tx`]) into a transmit sink.
//! Standard: Bluetooth Core Specification v5.4 Vol 6 Part B; Core Specification Supplement v11 Part A.

pub mod adv;
pub mod bank;
pub mod bench;
pub mod blind;
pub mod broadcast;
pub mod conn;
pub mod follow;
pub mod gen;
pub mod scene;
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
use sdrkit_proto_wlan_phy::ble::{AdvPdu, BlindPacket, Coding, LinkKind, Phy};
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

/// A good packet (the `ble.adv` stream): the PDU, what its advertising data says, and how it was received. A packet of a
/// connection has an empty `info`.
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
    /// The kind of the link the packet came on (advertising, a periodic train or a connection).
    pub kind: LinkKind,
    /// The access address.
    pub access_address: u32,
    /// The connection event of a packet of a followed connection, and whether the central sent it.
    pub conn: Option<conn::Placed>,
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
    /// The centre of the input is known (not a recording without one).
    pub centre_known: bool,
    /// The periodic trains whose packets the receivers take.
    pub trains: u32,
    /// The connections followed.
    pub connections: u32,
    /// The rate of the input, Hz.
    pub rate_hz: f64,
    /// Every data channel inside the capture is received all the time, and links not held are looked for on them.
    pub wide: bool,
    /// The data channels are left out but for followed links: live input came faster than the work.
    pub shed: bool,
    /// The counters of each channel.
    pub per: PerChannel,
}

impl Item for AdvStats {
    const KIND: &'static str = "ble.adv_stats";
}

/// The counters of each channel's receiver since the capture was set, by channel index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerChannel {
    /// Good packets.
    pub good: [u64; 40],
    /// Exact access-address matches of a held link whose CRC failed.
    pub bad: [u64; 40],
    /// Packets of links not held.
    pub unknown: [u64; 40],
}

impl Default for PerChannel {
    fn default() -> Self {
        PerChannel {
            good: [0; 40],
            bad: [0; 40],
            unknown: [0; 40],
        }
    }
}

/// What the receivers say about a link (the `ble.link_note` stream).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkNote {
    /// A packet of a link the receivers do not hold.
    Unknown {
        /// Signal time of the end of the access address, s.
        time_s: f64,
        /// The packet.
        packet: BlindPacket,
    },
    /// The packets of a followed connection whose access address matched and whose CRC failed, in total.
    Failures {
        /// The access address.
        access_address: u32,
        /// The count.
        total: u64,
    },
}

impl Item for LinkNote {
    const KIND: &'static str = "ble.link_note";
}

/// The text of the channels inside a capture: "9 of 40 LE channels inside the capture at 20 Msps: 0 advertising, 9 data".
/// `capture` has bit `c` for channel `c`; `rate_hz` is the rate the samples arrive at.
pub fn capture_text(capture: u64, rate_hz: f64) -> String {
    let adv = (capture & PRIMARY_CHANNELS).count_ones();
    let data = (capture & !PRIMARY_CHANNELS).count_ones();
    let msps = rate_hz / 1e6;
    let rate = if (msps - msps.round()).abs() < 0.005 {
        format!("{msps:.0}")
    } else {
        format!("{msps:.1}")
    };
    format!("{} of 40 LE channels inside the capture at {rate} Msps: {adv} advertising, {data} data", adv + data)
}

/// The text of the data channels left out by a live receiver that is behind its input.
pub const DATA_CHANNELS_SHED: &str =
    "The processor is behind: the data channels are received only for followed connections";
/// The text of a wide capture that holds data channels and no primary advertising channel.
pub const NO_ADVERTISING_CHANNEL_WIDE: &str =
    "No advertising channel is inside the capture: connections are listed, devices are not; tune near 2402, 2426 or 2480 MHz for devices";

/// The text of a capture that holds no primary advertising channel.
pub const NO_ADVERTISING_CHANNEL: &str =
    "No advertising channel is inside the capture: tune to 2402, 2426 or 2480 MHz";

/// The bits of channels 37, 38 and 39 in [`AdvStats::capture`].
const PRIMARY_CHANNELS: u64 = 0b111 << 37;

impl AdvStats {
    /// Why devices do not show or what is left out, for a receiver that takes every data channel: no primary advertising
    /// channel inside the capture ([`NO_ADVERTISING_CHANNEL`] when no data channel is inside either), and the data
    /// channels left out by a receiver that is behind ([`DATA_CHANNELS_SHED`]). `None` otherwise.
    pub fn wide_reason(&self) -> Option<String> {
        let none = self.centre_known && self.capture & PRIMARY_CHANNELS == 0;
        let text = match (none, self.capture & !PRIMARY_CHANNELS != 0) {
            (true, true) => Some(NO_ADVERTISING_CHANNEL_WIDE),
            (true, false) => Some(NO_ADVERTISING_CHANNEL),
            _ => None,
        };
        match (text, self.shed) {
            (Some(t), true) => Some(format!("{t}. {DATA_CHANNELS_SHED}")),
            (Some(t), false) => Some(t.to_owned()),
            (None, true) => Some(DATA_CHANNELS_SHED.to_owned()),
            (None, false) => None,
        }
    }

    /// Why nothing can be heard: [`NO_ADVERTISING_CHANNEL`] when the centre is known and the capture holds none of the
    /// channels 37, 38 and 39. `None` otherwise.
    pub fn quiet_reason(&self) -> Option<&'static str> {
        (self.centre_known && self.capture & PRIMARY_CHANNELS == 0).then_some(NO_ADVERTISING_CHANNEL)
    }
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
    let front = GraphBuilder::new()
        .add("adv", DecoderComponent::new(AdvDecoder::wide()))
        .input("adv");
    with_sender(front, tx::BleSender::ble())
        .add("table", table::DeviceTable::default())
        .connect("adv.adverts", "table.adverts")
        .connect("adv.stats", "table.stats")
        .connect("adv.links", "table.links")
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
        /// Keep the ATT values.
        AttValues = table::ATT_VALUES,
        /// The level sent.
        TxLevel = tx::TX_LEVEL,
        /// The whitening of the test signal.
        GenChannel = GEN_CHANNEL,
        /// Show the access address of a connection in full.
        AccessAddressFull = table::ACCESS_ADDRESS_FULL,
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
    r.add("wlan.ble_wide", || {
        Box::new(DecoderComponent::new(AdvDecoder::wide()))
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
    blurb: "Bluetooth LE on every channel inside the capture: advertising on LE 1M, LE 2M and LE Coded, legacy and extended, as a scanner app shows it, and a link-activity view of the connections on the data channels, each followed from its start or found by its packets. The data-channel payloads are not decoded beyond the link-layer header.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
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
        ..Tuning::DEFAULT
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

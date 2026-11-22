//! The Bluetooth LE advertising monitor (`ble`): the packets devices broadcast on the three primary advertising channels with
//! the LE 1M PHY, a device table and a packet log. The stack sends advertising packets ([`tx`]) into a transmit sink.
//! Standard: Bluetooth Core Specification v5.4 Vol 6 Part B; Core Specification Supplement v11 Part A.

pub mod adv;
pub mod gen;
pub mod table;
pub mod tx;

use sdrkit_algo_dsp::meter::IntervalLevel;

use crate::testgen::{route_to, GEN_SECTION};
use adv::{parse_ad, summary, AdvInfo};
use gen::GenWhitening;
use sdrkit_components::dsp::Channel;
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting,
    SettingSet, StackConfig, StackDescriptor, StageError, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Decoder, DecoderComponent, Format, Graph, GraphBuilder, GraphError, Item,
    Kind, Outbox, Port, Registry, RunContext, StackGraph, StackGraphBuilder, Timed,
};
use sdrkit_proto_wlan_phy::ble::{advertising_channel_at, AdvPdu, AdvReceiver};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "ble";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = 4e6;
/// The rate the advertising receiver runs at, Hz (4 samples per bit).
pub const RATE_HZ: f64 = 4e6;
/// The width the channel filter keeps, Hz (650 kHz each side of the carrier).
pub const CHANNEL_WIDTH_HZ: f64 = 1.3e6;
/// The report period, s.
pub const REPORT_S: f64 = 0.25;
/// How near an advertising channel's centre the tuned frequency must be to give the channel hint, Hz.
const HINT_TOLERANCE_HZ: f64 = 0.1e6;

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
    /// The advertising channel whose whitening produced the good CRC (37, 38 or 39).
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
}

impl Item for AdvStats {
    const KIND: &'static str = "ble.adv_stats";
}

/// The advertising receiver as a decoder (`wlan.ble_adv`): channel samples at a whole number of MHz in, good packets and the
/// counters out.
pub struct AdvDecoder {
    rx: Option<AdvReceiver>,
    rate: f64,
    tuned_hz: f64,
    live: bool,
    base: Option<u64>,
    level: IntervalLevel,
}

impl Default for AdvDecoder {
    fn default() -> Self {
        AdvDecoder {
            rx: None,
            rate: RATE_HZ,
            tuned_hz: 0.0,
            live: false,
            base: None,
            level: IntervalLevel::default(),
        }
    }
}

impl AdvDecoder {
    /// The channel hint: the advertising channel the tuned frequency is on, for a live source only.
    fn hint(&self) -> u8 {
        if self.live {
            advertising_channel_at(self.tuned_hz, HINT_TOLERANCE_HZ).unwrap_or(0)
        } else {
            0
        }
    }

    fn apply_hint(&mut self) {
        let h = self.hint();
        if let Some(rx) = &mut self.rx {
            // The hint comes from `advertising_channel_at`, so it is always 37, 38 or 39, which the receiver takes.
            let _ = rx.set_channel_hint((h != 0).then_some(h));
        }
    }
}

impl Decoder for AdvDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "wlan.ble_adv"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::typed::<Advert>("adverts"),
            Port::typed::<AdvStats>("stats"),
        ]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        let sps = (input.rate / 1e6).round();
        if !(2.0..=16.0).contains(&sps) || (input.rate - sps * 1e6).abs() > 1.0 {
            return Err(ConfigError::WrongRate {
                need: RATE_HZ,
                got: input.rate,
            });
        }
        self.rx = Some(
            AdvReceiver::new(sps as usize).map_err(|e| ConfigError::Stage(StageError::new(e)))?,
        );
        self.rate = input.rate;
        self.tuned_hz = input.tuned_hz;
        self.base = None;
        self.level = IntervalLevel::default();
        self.apply_hint();
        Ok(vec![
            Format::new(Kind::typed::<Advert>(), 0.0),
            Format::new(Kind::typed::<AdvStats>(), 0.0),
        ])
    }
    fn retune(&mut self, input: &Format) {
        self.tuned_hz = input.tuned_hz;
        if let Some(rx) = &mut self.rx {
            rx.reset();
        }
        self.apply_hint();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.apply_hint();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        let Some(rx) = self.rx.as_mut() else {
            return;
        };
        let base = *self.base.get_or_insert(first.saturating_sub(rx.samples()));
        let rate = self.rate;
        let adverts = out.items::<Advert>(0);
        self.level.push(x);
        rx.process(x, |p| {
            let Ok(pdu) = p.parse() else {
                return;
            };
            let data = pdu.data().unwrap_or(&[]);
            let info = parse_ad(data);
            let summary = summary(&pdu, &info);
            let time_s = (base + p.sample) as f64 / rate;
            adverts.push(Timed {
                time: time_s,
                value: Advert {
                    time_s,
                    channel: p.channel,
                    level_dbfs: p.level_dbfs.max(-200.0),
                    offset_hz: p.offset_hz,
                    deviation_hz: p.deviation_hz,
                    snr_db: p.snr_db,
                    pdu: p.pdu.to_vec(),
                    info,
                    summary,
                },
            });
        });
        let s = rx.stats();
        let time_s = (first + x.len() as u64) as f64 / rate;
        let stats = AdvStats {
            time_s,
            aa_hits: s.aa_hits,
            crc_failures: s.crc_failures,
            packets: s.packets,
            channel: self.hint(),
            level_dbfs: self.level.take_dbfs(),
        };
        out.items::<AdvStats>(1).push(Timed {
            time: time_s,
            value: stats,
        });
    }
}

/// The channel filter in front of the advertising receiver: the signal moved to 0 Hz, 650 kHz kept each side, at exactly
/// 4 Msps (the graph sets its `exact` setting).
///
/// # Errors
///
/// [`sdrkit_core::SettingError`] if the channel filter refuses the width or the rate.
pub fn channel() -> Result<Channel, sdrkit_core::SettingError> {
    Channel::new(CHANNEL_WIDTH_HZ, RATE_HZ)
}

/// The front of a BLE graph: the channel filter `channel` at exactly 4 Msps and the advertising receiver `adv`.
///
/// # Errors
///
/// [`GraphError`] if the channel filter refuses its settings.
pub fn front() -> Result<GraphBuilder, GraphError> {
    let chan = channel().map_err(|error| GraphError::Setting {
        key: "channel".into(),
        error,
    })?;
    Ok(GraphBuilder::new()
        .add("channel", chan)
        .set("channel.exact", Value::Bool(true))
        .add("adv", DecoderComponent::new(AdvDecoder::default()))
        .input("channel")
        .connect("channel", "adv"))
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
    blurb: "Bluetooth LE advertising: the devices around, their names, vendors and beacons, as a scanner app shows them.",
    tuning: Tuning {
        min_hz: 2400e6,
        max_hz: 2484e6,
        default_hz: 2402e6,
        rate_hz: 4e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 2e6,
        tune_offset_hz: 1e6,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(2e6),
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
    benchmarks: Some(sdrkit_proto_wlan_phy::benchmarks),
};

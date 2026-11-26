//! ESP-NOW (`espnow`): the connectionless vendor-specific action frames of Espressif devices on one 2.4 GHz Wi-Fi channel,
//! heard with the DSSS and non-HT OFDM receivers of the `wifi` stack, with a device table and a frame log. Receive only.
//! Standard: IEEE Std 802.11-2020 clauses 9, 12.5.3, 16 and 17; the maker's public ESP-NOW guide.

pub mod ccmp;
pub mod frame;
pub mod gen;
pub mod table;

use crate::testgen::{gen_setting, route_to, GEN_SECTION};
use crate::wifi::gen::{WifiGenParams, WifiGenerator};
use crate::wifi::WifiDecoder;
use gen::{espnow_frame, parse_mac, EspNowSpec, GenPhy};
use sdrkit_core::bench::{BenchResult, Benchmark};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, InputInfo, LockProfile, Output,
    Receiver, Setting, SettingSet, StackConfig, StackDescriptor, Tuning, Values,
};
use sdrkit_graph::{
    Catalog, DecoderComponent, Graph, GraphBuilder, GraphError, Registry, StackGraph,
};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "espnow";
/// The lowest input rate, Hz.
pub const MIN_RATE_HZ: f64 = crate::wifi::OFDM_RATE_HZ;
/// The report interval, s.
pub const REPORT_S: f64 = 0.25;

/// The `espnow` graph: the Wi-Fi receiver at the channel nearest the radio centre, and the device table.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("rx", DecoderComponent::new(WifiDecoder::nearest_channel()))
        .add("table", table::EspnowTable::default())
        .input("rx")
        .connect("rx.mpdus", "table.mpdus")
        .connect("rx.stats", "table.stats")
        .output("table.events")
        .output("table.requests")
        .clock(REPORT_S)
        .status_from("table")
        .catalog(catalog)
        .build()
}

/// The test-signal setting `gen_scene`.
pub const GEN_SCENE: Setting<field::Choice<GenScene>> =
    Setting::choice("gen_scene", "Scene", GenScene::Mixed)
        .section(GEN_SECTION)
        .advanced();
/// The test-signal setting `gen_phy`: the PHY of the first frame.
pub const GEN_PHY: Setting<field::Choice<GenPhy>> = Setting::choice("gen_phy", "PHY", GenPhy::Dsss1)
    .section(GEN_SECTION)
    .advanced();
/// The test-signal setting `gen_sender`: the sender address of the first frame.
pub const GEN_SENDER: Setting<field::Text> =
    Setting::new("gen_sender", "Sender", "", "24:6F:28:11:22:33")
        .help("A MAC address, AA:BB:CC:DD:EE:FF.")
        .section(GEN_SECTION)
        .advanced();
/// The test-signal setting `gen_peer`: the destination of the first frame.
pub const GEN_PEER: Setting<field::Text> = Setting::new("gen_peer", "Peer", "", "broadcast")
    .help("A MAC address, AA:BB:CC:DD:EE:FF, or broadcast.")
    .section(GEN_SECTION)
    .advanced();
/// The test-signal setting `gen_payload`: the payload text of the first frame.
pub const GEN_PAYLOAD: Setting<field::Text> =
    Setting::new("gen_payload", "Payload", "", "temp=21.5")
        .help("Up to 250 bytes in version 1, 1470 in version 2.")
        .section(GEN_SECTION)
        .advanced();
/// The test-signal setting `gen_version`: the ESP-NOW version of the first frame.
pub const GEN_VERSION: Setting<field::I64> = Setting::new("gen_version", "Version", "", 1)
    .range(1.0, 2.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::choice_enum! {
    /// The choices of the test-signal setting `gen_scene`.
    pub enum GenScene {
        /// The first frame and seven more of five senders, an encrypted one among them, and an access point beacon.
        Mixed = ("mixed", "Mixed scene"),
        /// The first frame alone.
        Single = ("single", "One frame"),
    }
}

sdrkit_core::settings! {
    /// The settings of the `espnow` stack: the device table's and the test signal's.
    pub enum EspnowSetting {
        /// The channel.
        Channel = table::CHANNEL,
        /// Hop through the hop list.
        Hop = table::HOP,
        /// The dwell while hopping.
        DwellMs = table::DWELL_MS,
        /// The channels of the hop list.
        Channels24 = crate::wifi::table::CHANNELS_24,
        /// The keys.
        Keys = table::KEYS,
        /// The scene of the test signal.
        GenScene = GEN_SCENE,
        /// The PHY of the first test frame.
        GenPhy = GEN_PHY,
        /// The sender of the first test frame.
        GenSender = GEN_SENDER,
        /// The destination of the first test frame.
        GenPeer = GEN_PEER,
        /// The payload of the first test frame.
        GenPayload = GEN_PAYLOAD,
        /// The version of the first test frame.
        GenVersion = GEN_VERSION,
    }
}

/// The `espnow` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let b = StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "ESP-NOW needs a sample rate of at least 20 Msps");
    route_to(b, "table", &EspnowSetting::schema())?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the ESP-NOW component of this crate to a registry.
pub fn register(r: &mut Registry) {
    r.add("wlan.espnow_table", || {
        Box::new(table::EspnowTable::default())
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The first test frame as the options give it; an option that does not parse takes its default.
fn first_frame(cfg: &GenConfig) -> EspNowSpec {
    let from = gen_setting(cfg, &GEN_SENDER)
        .and_then(parse_mac)
        .unwrap_or(gen::SENDER);
    let to = gen_setting(cfg, &GEN_PEER)
        .and_then(parse_mac)
        .unwrap_or([0xFF; 6]);
    let payload = gen_setting(cfg, &GEN_PAYLOAD).unwrap_or(GEN_PAYLOAD.default);
    let version = gen_setting(cfg, &GEN_VERSION).unwrap_or(1).clamp(1, 2) as u8;
    EspNowSpec {
        seq: 10,
        version,
        ..EspNowSpec::new(from, to, payload.as_bytes())
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let first = first_frame(cfg);
    let phy = gen_setting(cfg, &GEN_PHY).unwrap_or(GenPhy::Dsss1);
    let frames = match gen_setting(cfg, &GEN_SCENE).unwrap_or(GenScene::Mixed) {
        GenScene::Mixed => gen::scene(&first, phy),
        GenScene::Single => vec![gen::on_phy(espnow_frame(&first), phy)],
    };
    Ok(Box::new(WifiGenerator::new(WifiGenParams {
        rate_hz: cfg.rate_hz,
        frames,
        ..Default::default()
    })))
}

/// Runs the `espnow` receiver on 0.5 s of its test signal at 20 Msps, for `seconds` of processor time.
fn bench_receiver(seconds: f64) -> BenchResult {
    let Ok(schema) = sdrkit_components::sim::with_generator_settings(&EspnowSetting::schema())
    else {
        return BenchResult::default();
    };
    let settings = Values::defaults(&schema);
    let rate = ESPNOW.tuning.rate_hz;
    let signal_s = 0.5;
    let Ok(mut g) = make_generator(&GenConfig::new(rate, settings.clone()).with_seed(1)) else {
        return BenchResult::default();
    };
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::default(); n];
    g.generate(&mut x);
    let Ok(mut rx) = receiver(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    let f = ESPNOW.tuning.default_hz;
    if rx.configure(&InputInfo::new(rate, f, f).recording()).is_err() {
        return BenchResult::default();
    }
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The benchmarks of this crate.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "wlan.espnow_20m",
        run: bench_receiver,
    }]
}

/// The `espnow` stack.
pub static ESPNOW: StackDescriptor = StackDescriptor {
    id: ID,
    name: "ESP-NOW",
    group: "Data and IoT",
    blurb: "ESP-NOW frames of Espressif devices on a 2.4 GHz Wi-Fi channel: senders, peers, payloads and encrypted frames.",
    tuning: Tuning {
        min_hz: 2400e6,
        max_hz: 2500e6,
        default_hz: 2412e6,
        rate_hz: 20e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 20e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(20e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: EspnowSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ESPNOW),
    benchmarks: Some(benchmarks),
};

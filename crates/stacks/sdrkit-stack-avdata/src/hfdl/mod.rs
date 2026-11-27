//! HFDL: every channel of the list (by default every table frequency) inside the capture, bursts found by their preamble,
//! decoded and read, and ground stations, aircraft and channels tabled. A band hopper can ask the host to follow propagation.
//! Standard: ARINC 635-4, ICAO Doc 9741.

pub mod bank;
pub mod front;
pub mod generator;
pub mod pdu;
pub mod phy;
pub mod stations;
pub mod tables;

use crate::verify;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Results, Setting,
    SettingSet, StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, Item, Registry, StackGraph};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use generator::{GenParams, HfdlGenerator};

/// The stack id.
pub const ID: &str = "hfdl";
/// Below this the receiver refuses.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The radio centre of a recording without one.
pub const DEFAULT_CENTRE_HZ: f64 = 8.9e6;

/// A decoded burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BurstOut {
    /// Signal time of the start of the burst (its prekey).
    pub time_s: f64,
    /// Dial, Hz.
    pub dial_hz: f64,
    /// Level, dBFS.
    pub level_db: f32,
    /// SNR on the known symbols.
    pub snr_db: f32,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// bit/s.
    pub bps: u32,
    /// Slots.
    pub slots: u8,
    /// The messages.
    pub messages: Vec<pdu::Message>,
}

impl Item for BurstOut {
    const KIND: &'static str = "hfdl.message";
}

/// A channel.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelState {
    /// Dial, Hz.
    pub dial_hz: f64,
    /// The table's station on it.
    pub station: Option<u8>,
    /// In band.
    pub in_band: bool,
    /// Level of the last burst.
    pub level_db: f32,
    /// SNR of the last burst.
    pub snr_db: f32,
    /// Preambles found.
    pub preambles: u64,
    /// Good bursts.
    pub good: u64,
    /// Bad bursts.
    pub bad: u64,
    /// Last heard.
    pub last_heard_s: f64,
    /// Last offset.
    pub last_offset_hz: f32,
    /// Last bit/s.
    pub last_bps: u32,
}

/// The front end at a report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrontState {
    /// Signal time.
    pub time_s: f64,
    /// Rate.
    pub rate_hz: f64,
    /// Centre.
    pub centre_hz: f64,
    /// Channels.
    pub channels: Vec<ChannelState>,
    /// Why counters: preamble rejected, short, squitter FCS, header FCS, LPDU FCS, LPDU type.
    pub why: [u64; 6],
}

impl Item for FrontState {
    const KIND: &'static str = "hfdl.front_state";
}

/// The channel list.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "").help(
    "Dial frequencies in kHz separated by spaces. Empty watches every frequency of the ground station table inside the capture.",
);
/// Every table frequency inside the capture.
pub const WHOLE_SPAN: Setting<field::Bool> = Setting::new("whole_span", "Whole span", "", false)
    .help("Every table frequency inside the capture, and the list.");
/// One channel bank for every channel.
pub const SHARED_CHANNELISER: Setting<field::Bool> =
    Setting::new("shared_channeliser", "Shared channeliser", "", true)
        .help("One polyphase bank feeds every channel (else one channeliser each).");
/// The width of each channel's filter.
pub const CHANNEL_FILTER_HZ: Setting<field::Hz> =
    Setting::new("channel_filter_hz", "Channel filter", "Hz", 3400.0)
        .range(2000.0, 5000.0, 100.0)
        .help("The width of each channel's filter.")
        .restart();
/// Follow the band with the most traffic.
pub const FOLLOW_BEST_BAND: Setting<field::Bool> =
    Setting::new("follow_best_band", "Follow the best band", "", false)
        .help("Asks the host to retune to the HFDL band with the most traffic.");

sdrkit_core::settings! {
    /// The settings of the HFDL stack.
    pub enum HfdlSetting {
        /// The channel list.
        Channels = CHANNELS,
        /// Every table frequency inside the capture.
        WholeSpan = WHOLE_SPAN,
        /// One channel bank for every channel.
        SharedChanneliser = SHARED_CHANNELISER,
        /// The width of each channel's filter.
        ChannelFilterHz = CHANNEL_FILTER_HZ,
        /// Follow the band with the most traffic.
        FollowBestBand = FOLLOW_BEST_BAND,
    }
}

/// The HFDL receiver graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", front::Front::default())
        .add("tables", tables::Tables::default())
        .input("front")
        .connect("front.bursts", "tables.bursts")
        .connect("front.state", "tables.state")
        .output("tables.out")
        .output("tables.requests")
        .clock(0.25)
        .reconfigure_on_retune()
        .status_from("tables")
        .catalog(catalog)
        .build()
}

/// The HFDL receiver with these settings.
///
/// # Errors
///
/// The errors of [`graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "HFDL needs a sample rate of at least 24 kHz")
        .default_centre(DEFAULT_CENTRE_HZ)
        .route(CHANNELS.key(), &["front.channels"])?
        .route(WHOLE_SPAN.key(), &["front.whole_span"])?
        .route(SHARED_CHANNELISER.key(), &["front.shared_channeliser"])?
        .route(CHANNEL_FILTER_HZ.key(), &["front.channel_filter_hz"])?
        .route(FOLLOW_BEST_BAND.key(), &["tables.follow_best_band"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tables", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut Registry) {
    r.add("avdata.hfdl_front", || Box::new(front::Front::default()));
    r.add("avdata.hfdl_tables", || Box::new(tables::Tables::default()));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(HfdlGenerator::from_config(cfg)))
}

/// The engine's checks of the test signal.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three ground stations", o, 0.0);
    c.seconds = o.seconds.max(40.0);
    c.expect_audio = Some(false);
    c.check = Some(check_pdus);
    vec![c]
}

fn check_pdus(r: &Results) -> Vec<CheckLine> {
    vec![verify::at_least(r, "pdus", 3.0, "PDUs")]
}

/// The HFDL stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "HFDL",
    group: "Aviation",
    blurb: "The HF data link of aircraft over oceans and remote areas: squitters, logons, positions and ACARS from ground stations worldwide.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        min_hz: 2e6,
        max_hz: 30e6,
        default_hz: 8.9e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 0.4e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: HfdlSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hfdl],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

//! ACARS on VHF: every carrier in the capture found and decoded at once, blocks checked and repaired, joined into messages,
//! read for positions and tabled.
//! Standard: ARINC 618 (air-ground character-oriented protocol), ARINC 620 (labels).

pub mod demod;
pub mod front;
pub mod generator;
pub mod stages;
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

pub use generator::{AcarsGenerator, GenParams};

/// The stack id.
pub const ID: &str = "acars";
/// The radio centre a recording without one is taken to have.
pub const DEFAULT_CENTRE_HZ: f64 = 131.5e6;
/// Below this the receiver refuses.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The report period.
pub const REPORT_S: f64 = 0.25;

/// A block as the framer found it: the bytes after SOH up to and including the suffix (parity still on), the check bytes, the
/// carrier level during it and the signal time of its end.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawBlock {
    /// Channel frequency, Hz.
    pub freq_hz: f64,
    /// Mode .. suffix.
    pub bytes: Vec<u8>,
    /// The two check bytes, low first.
    pub bcs: [u8; 2],
    /// Carrier level during the block, dB below full scale.
    pub level_db: f32,
    /// Signal time of the end.
    pub end_s: f64,
}

impl Item for RawBlock {
    const KIND: &'static str = "acars.raw_block";
}

/// A checked block: its body with the parity bits cleared.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    /// Channel frequency, Hz.
    pub freq_hz: f64,
    /// Carrier level, dB.
    pub level_db: f32,
    /// Signal time of the end.
    pub end_s: f64,
    /// It passed the block check (after the repair) and its fields parse.
    pub good: bool,
    /// Bits the repair changed.
    pub bits_fixed: u32,
    /// Mode .. ETX or ETB, 7-bit characters.
    pub body: Vec<u8>,
}

impl Item for Block {
    const KIND: &'static str = "acars.block";
}

/// A finished message (one block or several joined).
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// Channel frequency of the last block, Hz.
    pub freq_hz: f64,
    /// Carrier level of the last block.
    pub level_db: f32,
    /// Signal time of the end of the last block.
    pub end_s: f64,
    /// The message.
    pub msg: sdrkit_proto_aero::acars::Message,
}

impl Item for Message {
    const KIND: &'static str = "acars.message";
}

/// Why a channel stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// The carrier went.
    Gone,
    /// No frame start in 8 s.
    NotAcars,
    /// A stronger carrier captured the decoder.
    Preempted,
}

/// A channel's decoder started or stopped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelEvent {
    /// Channel frequency, Hz.
    pub freq_hz: f64,
    /// Offset in the input, Hz.
    pub offset_hz: f64,
    /// Started (or stopped).
    pub start: bool,
    /// Carrier-to-noise, dB.
    pub cn_db: f32,
    /// Carrier offset, Hz.
    pub cfo_hz: f32,
    /// Why it stopped.
    pub reason: Option<StopReason>,
}

impl Item for ChannelEvent {
    const KIND: &'static str = "acars.channel_event";
}

/// One channel as the finder sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelLook {
    /// Channel frequency, Hz.
    pub freq_hz: f64,
    /// Level now, dB (-120 when nothing).
    pub level_db: f32,
    /// SNR in the 7 kHz band, dB.
    pub snr_db: f32,
    /// Carrier offset, Hz.
    pub cfo_hz: f32,
    /// A decoder is on it.
    pub active: bool,
    /// It has had a carrier.
    pub heard: bool,
}

/// The front end's state at a report. The lists are shared slices that the front end reuses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrontState {
    /// Signal time.
    pub time_s: f64,
    /// Radio centre, Hz.
    pub centre_hz: f64,
    /// Input rate.
    pub rate_hz: f64,
    /// Frames started (SYN SYN SOH, first demodulator).
    pub frames_started: u64,
    /// Candidates.
    pub channels: Arc<[ChannelLook]>,
    /// Watching the list (else the grid).
    pub list: Arc<[f64]>,
    /// Refusal text when the rate is too low.
    pub refused: String,
}

impl Item for FrontState {
    const KIND: &'static str = "acars.front_state";
}

/// The channel list.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "")
    .help("Frequencies in MHz separated by spaces. Empty watches every carrier on the 25 kHz grid. An 8.33 kHz channel such as 131.5167 needs the list. The host fills it from a channel file (data/avdata/datalink_channels.csv).");
/// The carrier threshold.
pub const THRESHOLD_DB: Setting<field::F64> = Setting::new("threshold_db", "Threshold", "dB", 8.0)
    .range(3.0, 30.0, 0.5)
    .help("Carrier over the noise needed to start decoding a channel.");
/// Join the blocks of a long message.
pub const JOIN_BLOCKS: Setting<field::Bool> = Setting::new("join_blocks", "Join blocks", "", true)
    .help("Joins the blocks of a long message into one message.");
/// Aircraft in the test signal.
pub const GEN_AIRCRAFT: Setting<field::I64> = Setting::new("gen_aircraft", "Test aircraft", "", 6)
    .range(1.0, 24.0, 1.0)
    .help("Aircraft in the test signal.")
    .section("Test signal")
    .advanced();
/// How often the test aircraft send.
pub const GEN_RATE_FACTOR: Setting<field::F64> =
    Setting::new("gen_rate_factor", "Test traffic", "", 1.0)
        .range(0.5, 4.0, 0.1)
        .help("Scales how often the test aircraft send.")
        .section("Test signal")
        .advanced();
/// Position reports in the test signal.
pub const GEN_POSITIONS: Setting<field::Bool> =
    Setting::new("gen_positions", "Test positions", "", false)
        .help("The first six test aircraft also send position reports.")
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the ACARS stack.
    pub enum AcarsSetting {
        /// The channel list.
        Channels = CHANNELS,
        /// The carrier threshold.
        ThresholdDb = THRESHOLD_DB,
        /// Join the blocks of a long message.
        JoinBlocks = JOIN_BLOCKS,
        /// Aircraft in the test signal.
        GenAircraft = GEN_AIRCRAFT,
        /// How often the test aircraft send.
        GenRateFactor = GEN_RATE_FACTOR,
        /// Position reports in the test signal.
        GenPositions = GEN_POSITIONS,
    }
}

/// The ACARS receiver graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", front::Front::default())
        .add("check", stages::Checker)
        .add("join", stages::JoinStage::default())
        .add("tables", tables::Tables::default())
        .input("front")
        .connect("front.blocks", "check")
        .connect("check", "join.in")
        .connect("check", "tables.blocks")
        .connect("front.state", "join.state")
        .connect("join", "tables.messages")
        .connect("front.events", "tables.events")
        .connect("front.state", "tables.state")
        .output("tables.out")
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .status_from("tables")
        .catalog(catalog)
        .build()
}

/// The ACARS receiver with these settings.
///
/// # Errors
///
/// The errors of [`graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "ACARS needs a sample rate of at least 1 MHz")
        .default_centre(DEFAULT_CENTRE_HZ)
        .route(CHANNELS.key(), &["front.channels"])?
        .route(THRESHOLD_DB.key(), &["front.threshold_db"])?
        .route(JOIN_BLOCKS.key(), &["join.join_blocks"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut Registry) {
    r.add("avdata.acars_front", || Box::new(front::Front::default()));
    r.add("avdata.acars_check", || Box::new(stages::Checker));
    r.add("avdata.acars_join", || {
        Box::new(stages::JoinStage::default())
    });
    r.add(
        "avdata.acars_tables",
        || Box::new(tables::Tables::default()),
    );
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(AcarsGenerator::from_config(cfg)))
}

/// The engine's checks of the test signal.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    // The generator puts its channel one grid step above the tuned frequency. One case with no frequency, one tuned to
    // the grid point nearest the middle of the VHF band.
    let mut plain = VerifyCase::new("test signal, no frequency", o, 0.0);
    let mut band = VerifyCase::new("test signal, one channel", o, 0.0);
    let (lo, hi) = front::BAND_HZ;
    band.freq_hz = Some(((lo + hi) / 2.0 / 25e3).round() * 25e3);
    for c in [&mut plain, &mut band] {
        c.seconds = o.seconds.max(20.0);
        c.expect_audio = Some(false);
        c.check = Some(check_messages);
    }
    vec![plain, band]
}

fn check_messages(r: &Results) -> Vec<CheckLine> {
    vec![
        verify::at_least(r, "frames_ok", 3.0, "good blocks"),
        verify::at_least(r, "messages", 3.0, "messages"),
    ]
}

/// The ACARS stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "ACARS",
    group: "Aviation",
    blurb: "Aircraft text messages on VHF: every ACARS channel in the capture at once, with positions and an aircraft list.",
    tuning: Tuning {
        min_hz: 118e6,
        max_hz: 137e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 2e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: AcarsSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Acars],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: Some(crate::benchmarks),
};

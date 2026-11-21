//! VDL Mode 4, receive only: GFSK bursts found by their training sequence, NRZI decoded and HDLC framed, the burst header and
//! the synchronisation (position) and data link service messages read, and aircraft with positions tabled.
//! Standard: ICAO Annex 10 Volume III clause 6.9, ICAO Doc 9816, ETSI EN 302 842.

pub mod front;
pub mod generator;
pub mod reader;
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

pub use generator::{GenParams, Vdl4Generator};

/// The stack id.
pub const ID: &str = "vdl4";
/// Below this the receiver refuses.
pub const MIN_RATE_HZ: f64 = 400e3;
/// Bit rate.
pub const BAUD: f64 = 19_200.0;
/// The radio centre of a recording without one.
pub const DEFAULT_CENTRE_HZ: f64 = 136.85e6;

/// A frame with a good FCS from one burst.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Burst {
    /// Signal time of the training sequence.
    pub time_s: f64,
    /// Channel, Hz.
    pub freq_hz: f64,
    /// Power, dBFS.
    pub level_db: f32,
    /// Power over the floor, dB.
    pub snr_db: f32,
    /// Carrier offset, Hz.
    pub offset_hz: f32,
    /// Line bits collected.
    pub line_bits: usize,
    /// The frame (FCS removed).
    pub frame: Vec<u8>,
}

impl Item for Burst {
    const KIND: &'static str = "vdl4.burst";
}

/// A channel's state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelState {
    /// Hz.
    pub freq_hz: f64,
    /// In band.
    pub in_band: bool,
    /// Training sequences found.
    pub found: u64,
    /// Good frames.
    pub good: u64,
    /// Failed frames.
    pub bad: u64,
    /// Last level.
    pub level_db: f32,
    /// Last SNR.
    pub snr_db: f32,
    /// Noise floor, dBFS.
    pub floor_db: f32,
    /// Last heard.
    pub last_heard_s: f64,
    /// Last offset.
    pub last_offset_hz: f32,
}

/// The front end at a report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrontState {
    /// Signal time.
    pub time_s: f64,
    /// Rate.
    pub rate_hz: f64,
    /// Channels.
    pub channels: Vec<ChannelState>,
    /// Refusal ("sample rate N kHz is too low ...").
    pub refused: String,
}

impl Item for FrontState {
    const KIND: &'static str = "vdl4.front_state";
}

/// The channel list.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "").help(
    "Frequencies in MHz separated by spaces. Empty watches the tuned frequency. The host fills it from a channel file (data/avdata/datalink_channels.csv).",
);

sdrkit_core::settings! {
    /// The settings of the VDL Mode 4 stack.
    pub enum Vdl4Setting {
        /// The channel list.
        Channels = CHANNELS,
    }
}

/// The VDL4 receiver graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", front::Front::default())
        .add("reader", reader::Reader::default())
        .add("tables", tables::Tables::default())
        .input("front")
        .connect("front.bursts", "reader")
        .connect("reader", "tables.messages")
        .connect("front.state", "tables.state")
        .output("tables.out")
        .clock(0.25)
        .reconfigure_on_retune()
        .status_from("tables")
        .catalog(catalog)
        .build()
}

/// The VDL4 receiver with these settings.
///
/// # Errors
///
/// The errors of [`graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "VDL Mode 4 needs a sample rate of at least 400 kHz",
        )
        .default_centre(DEFAULT_CENTRE_HZ)
        .route(CHANNELS.key(), &["front.channels"])?
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
    r.add("avdata.vdl4_front", || Box::new(front::Front::default()));
    r.add("avdata.vdl4_reader", || Box::new(reader::Reader::default()));
    r.add("avdata.vdl4_tables", || Box::new(tables::Tables::default()));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(Vdl4Generator::from_config(cfg)))
}

/// The engine's checks of the test signal.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, tuned channel", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(check_frames);
    vec![c]
}

fn check_frames(r: &Results) -> Vec<CheckLine> {
    vec![verify::at_least(r, "frames", 4.0, "frames")]
}

/// The VDL Mode 4 stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "VDL Mode 4",
    group: "Aviation",
    blurb: "The VHF data link used for surveillance and position reports, on the VDL4 channels in the capture.",
    tuning: Tuning {
        min_hz: 118e6,
        max_hz: 140e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 0.4e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: Vdl4Setting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Vdl4],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

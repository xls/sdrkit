//! VDL Mode 2: every channel of the list inside the capture, bursts found by their training sequence, D8PSK demodulated,
//! descrambled, Reed-Solomon corrected, framed, and read through AVLC, ACARS, X.25, CLNP, COTP and XID.
//! Standard: ICAO Annex 10 Volume III chapter 6, ICAO Doc 9776, ARINC 631.

pub mod avlc;
pub mod front;
pub mod generator;
pub mod phy;
pub mod tables;

use crate::verify;
use avlc::Frame;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Results, Setting,
    SettingSet, StackConfig, StackDescriptor, Tuning, Values,
};
use sdrkit_graph::{
    no_values, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError, Io,
    Item, Kind, Port, RateFrom, Registry, StackGraph, Timed,
};
use sdrkit_proto_aero::acars::assemble::{Assembler, Joined, Quality};
use sdrkit_proto_aero::acars::{self, Message};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use generator::{GenParams, Vdl2Generator};

/// The stack id.
pub const ID: &str = "vdl2";
/// The radio centre of a recording without one: 125 kHz below the common signalling channel [`front::CSC_MHZ`] (ICAO
/// Annex 10 Volume III).
pub const DEFAULT_CENTRE_HZ: f64 = 136.85e6;
/// Below this the receiver refuses.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The report period.
pub const REPORT_S: f64 = 0.25;

/// A good burst: its frames and measurements.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Burst {
    /// Signal time of the start of the training sequence.
    pub time_s: f64,
    /// Channel, Hz.
    pub freq_hz: f64,
    /// Amplitude squared, dBFS.
    pub level_db: f32,
    /// Signal over decision error, dB.
    pub snr_db: f32,
    /// Carrier offset, the tuning error included.
    pub offset_hz: f64,
    /// Octets corrected by Reed-Solomon.
    pub corrected: u32,
    /// The header needed its FEC.
    pub header_fixed: bool,
    /// Frames whose FCS failed.
    pub fcs_bad: u32,
    /// The frames (FCS removed).
    pub frames: Vec<Vec<u8>>,
}

impl Item for Burst {
    const KIND: &'static str = "vdl2.burst";
}

/// A channel as the bank sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelState {
    /// Hz.
    pub freq_hz: f64,
    /// Inside the capture.
    pub in_band: bool,
    /// Level of the last burst.
    pub level_db: f32,
    /// SNR of the last burst.
    pub snr_db: f32,
    /// Bursts found.
    pub bursts_found: u64,
    /// Good bursts.
    pub good: u64,
    /// Bad bursts.
    pub bad: u64,
    /// Frames.
    pub frames: u64,
    /// Last heard (signal time).
    pub last_heard_s: f64,
    /// Last offset.
    pub last_offset_hz: f32,
    /// Octets corrected by Reed-Solomon.
    pub corrected: u64,
    /// Headers that needed their FEC.
    pub headers_fixed: u64,
    /// Frames whose FCS failed.
    pub fcs_bad: u64,
    /// Training sequences whose header was refused.
    pub headers_refused: u64,
}

/// The bank at a report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BankState {
    /// Signal time.
    pub time_s: f64,
    /// Input rate.
    pub rate_hz: f64,
    /// Radio centre.
    pub centre_hz: f64,
    /// The common tuning error.
    pub tune_error_hz: f64,
    /// Channels.
    pub channels: Arc<[ChannelState]>,
    /// A training sequence was found.
    pub burst_found: bool,
    /// Refusal text.
    pub refused: String,
}

impl Item for BankState {
    const KIND: &'static str = "vdl2.bank_state";
}

/// A frame with where and how it was received.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RxFrame {
    /// Signal time of the burst.
    pub time_s: f64,
    /// Channel.
    pub freq_hz: f64,
    /// Burst level, dBFS.
    pub level_db: f32,
    /// Burst SNR.
    pub snr_db: f32,
    /// Carrier offset.
    pub offset_hz: f64,
    /// Octets corrected in the burst.
    pub corrected: u32,
    /// One block of a longer ACARS message, held (hidden from the lists).
    pub held: bool,
    /// The frame.
    #[serde(flatten)]
    pub frame: Frame,
}

impl Item for RxFrame {
    const KIND: &'static str = "vdl2.frame";
}

/// Reads the frames of each burst and joins multi-block ACARS messages (ARINC 631: the blocks of one message share the
/// aircraft's 24-bit address, the label and the message sequence number).
#[derive(Default)]
pub struct Reader {
    joiner: Assembler<RxFrame>,
    out: Vec<Joined<RxFrame>>,
    frames: Vec<RxFrame>,
}

impl Reader {
    fn joined(out: &mut Vec<Joined<RxFrame>>, frames: &mut Vec<RxFrame>) {
        for j in out.drain(..) {
            let m = Message::from_joined(&j);
            let mut rx = j.body.extra;
            avlc::describe_acars(&mut rx.frame, &m);
            rx.frame.acars = Some(m);
            rx.held = false;
            frames.push(rx);
        }
    }

    /// Reads the frames of one burst, appending them (and the messages they complete) to `frames`.
    pub fn burst(&mut self, b: &Burst, frames: &mut Vec<RxFrame>) {
        for raw in &b.frames {
            let Some(f) = avlc::read(raw) else { continue };
            let rx = RxFrame {
                time_s: b.time_s,
                freq_hz: b.freq_hz,
                level_db: b.level_db,
                snr_db: b.snr_db,
                offset_hz: b.offset_hz,
                corrected: b.corrected,
                held: false,
                frame: f,
            };
            let block = rx
                .frame
                .acars_crc_ok
                .then(|| acars::Block::parse(&rx.frame.acars_body).ok())
                .flatten();
            match block {
                Some(block) => {
                    if block.end == acars::End::Etb {
                        let mut h = rx.clone();
                        h.held = true;
                        h.frame
                            .detail
                            .push("one block of a longer ACARS message".into());
                        frames.push(h);
                    }
                    let key = rx.frame.src;
                    self.joiner.push(
                        rx.time_s,
                        &block,
                        Quality::GOOD,
                        key,
                        rx.clone(),
                        &mut self.out,
                    );
                    // The joiner's output goes out in its place, so the order never depends on the cuts.
                    Self::joined(&mut self.out, frames);
                }
                None => frames.push(rx),
            }
        }
    }
}

impl Component for Reader {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.vdl2_reader",
            vec![
                Port::typed::<Burst>("in"),
                Port::typed::<BankState>("state"),
            ],
            vec![Port::typed::<RxFrame>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.joiner.clear();
        Ok(vec![Format::new(Kind::typed::<RxFrame>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut frames = std::mem::take(&mut self.frames);
        for b in io.items::<Burst>(0) {
            self.burst(&b.value, &mut frames);
        }
        for s in io.items::<BankState>(1) {
            self.joiner.expire(s.value.time_s, &mut self.out);
        }
        Self::joined(&mut self.out, &mut frames);
        if !frames.is_empty() {
            let o = io.items_out::<RxFrame>(0);
            for f in frames.drain(..) {
                o.push(Timed {
                    time: f.time_s,
                    value: f,
                });
            }
        }
        self.frames = frames;
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// The channel list.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "").help(
    "Frequencies in MHz separated by spaces. Empty watches the common signalling channel 136.975 MHz and the tuned frequency. The host fills it from a channel file (data/avdata/datalink_channels.csv). Tune between the channels.",
);
/// Follow the radio's tuning error.
pub const TRACK_TUNING: Setting<field::Bool> =
    Setting::new("track_tuning", "Track tuning", "", true)
        .help("Measures the radio's tuning error from the bursts and takes it out.");
/// The tuning error to start from.
pub const TUNE_ERROR_HZ: Setting<field::Hz> =
    Setting::new("tune_error_hz", "Tuning error guess", "Hz", 0.0)
        .range(-6000.0, 6000.0, 1.0)
        .help("The radio's tuning error to start from.");
/// Aircraft in the test signal.
pub const GEN_AIRCRAFT: Setting<field::I64> = Setting::new("gen_aircraft", "Test aircraft", "", 6)
    .range(1.0, 24.0, 1.0)
    .help("Aircraft in the test signal.")
    .section("Test signal")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the VDL Mode 2 stack.
    pub enum Vdl2Setting {
        /// The channel list.
        Channels = CHANNELS,
        /// Follow the radio's tuning error.
        TrackTuning = TRACK_TUNING,
        /// The tuning error to start from.
        TuneErrorHz = TUNE_ERROR_HZ,
        /// Aircraft in the test signal.
        GenAircraft = GEN_AIRCRAFT,
    }
}

/// The VDL2 receiver graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", front::Front::default())
        .add("reader", Reader::default())
        .add("tables", tables::Tables::default())
        .input("front")
        .connect("front.bursts", "reader.in")
        .connect("front.state", "reader.state")
        .connect("reader", "tables.frames")
        .connect("front.state", "tables.state")
        .output("tables.out")
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .status_from("tables")
        .catalog(catalog)
        .build()
}

/// The VDL2 receiver with these settings.
///
/// # Errors
///
/// The errors of [`graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "VDL Mode 2 needs a sample rate of at least 250 kHz",
        )
        .default_centre(DEFAULT_CENTRE_HZ)
        .route(CHANNELS.key(), &["front.channels"])?
        .route(TRACK_TUNING.key(), &["front.track_tuning"])?
        .route(TUNE_ERROR_HZ.key(), &["front.tune_error_hz"])?
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
    r.add("avdata.vdl2_front", || Box::new(front::Front::default()));
    r.add("avdata.vdl2_reader", || Box::new(Reader::default()));
    r.add("avdata.vdl2_tables", || Box::new(tables::Tables::default()));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(Vdl2Generator::from_config(cfg)))
}

/// The engine's checks of the test signal.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    // With an empty list the receiver watches the common signalling channel and the tuned frequency: one case with no
    // frequency, one tuned to the common signalling channel.
    let mut plain = VerifyCase::new("test signal, no frequency", o, 0.0);
    let mut csc = VerifyCase::new("test signal, common signalling channel", o, 0.0);
    csc.freq_hz = Some((front::CSC_MHZ * 1e6).round());
    for c in [&mut plain, &mut csc] {
        c.seconds = o.seconds.max(10.0);
        c.expect_audio = Some(false);
        c.check = Some(check_frames);
    }
    vec![plain, csc]
}

fn check_frames(r: &Results) -> Vec<CheckLine> {
    vec![verify::at_least(r, "frames_ok", 5.0, "frames")]
}

/// The VDL Mode 2 stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "VDL Mode 2",
    group: "Aviation",
    blurb: "The VHF digital data link of airliners: ACARS, ATN and link frames on every VDL2 channel in the capture.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
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
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: Vdl2Setting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Vdl2],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

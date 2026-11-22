//! ATSC 3.0: the receiver graph and the stack. `atsc3.sync` finds the bootstraps, `atsc3.frame` demodulates each frame and
//! reads its signalling, `atsc3.fec` de-interleaves and decodes the FEC blocks of one PLP to baseband packets, the shared `ipcast.alp`,
//! `ipcast.ip` and `ipcast.route` carry the link and delivery layers, `atsc3.lls` reads the service list,
//! `atsc3.media` turns ROUTE objects into media samples, and `atsc3.report` publishes the results.
//!
//! Standard: ATSC A/321, A/322, A/330, A/331.

pub mod bicm;
pub mod bootstrap;
pub mod components;
pub mod frame;
pub mod gen;
pub mod l1;
pub mod layout;
pub mod ldm;
pub mod nuc;
pub mod results;
pub mod rx;
pub mod svc;
pub mod ti;
#[rustfmt::skip]
pub mod tables;

pub use gen::{Atsc3GenParams, Atsc3Generator};

use sdrkit_core::SignalType;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, Receiver, Setting, SettingSet, StackConfig,
    StackDescriptor, StageError, Tuning,
};
use sdrkit_core::CostTable;
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph, Stage};
use sdrkit_proto_ipcast::components::{AlpComponent, IpComponent, RouteComponent};
use std::sync::Arc;

/// The occupied bandwidth of a 6 MHz channel (0.8438 of 6.912 MHz is about 5.83 MHz. The test signal fills 5.83 MHz too).
pub const OCCUPIED: f64 = 5.83e6;

/// The stack id.
pub const ID: &str = "atsc3";

/// Adds the delivery chain after `from`, an output of descrambled baseband packets (one per `Bytes` packet): ALP, IP and UDP,
/// the service list, ROUTE and the media samples, all reported to `report`.
pub fn delivery(b: GraphBuilder, from: &str) -> GraphBuilder {
    b.add("alp", AlpComponent::default())
        .add("ip", IpComponent::default())
        .add("lls", components::Lls::default())
        .add("route", RouteComponent::default())
        .add("media", components::Media::default())
        .connect(from, "alp.in")
        .connect("alp.ip", "ip.in")
        .connect("ip.udp", "lls.in")
        .connect("ip.udp", "route.in")
        .connect("route.out", "media.in")
        .connect("lls.state", "report.lls")
        .connect("media.state", "report.media")
        .output("media.out")
}

/// The PLP that the stack decodes.
pub const PLP: Setting<field::I64> = Setting::new("plp", "PLP", "", -1)
    .range(-1.0, 63.0, 1.0)
    .help("The id of the PLP to decode. -1 takes the first core PLP with low level signalling that the stack decodes, or without one the first core PLP that it decodes.");

sdrkit_core::settings! {
    /// The settings of the ATSC 3.0 stack and of `atsc3.frame`.
    pub enum Atsc3Setting {
        /// The PLP to decode.
        Plp = PLP,
    }
}

/// The stage of the FEC blocks: time de-interleaving, demapping, LDPC and the outer code.
pub const FEC_STAGE: &str = "fec";

/// The recorded cost of each stage on the test signal (`cost.txt` of the crate).
const COSTS: &str = include_str!("../../cost.txt");

/// What the stack decodes and what it does not.
pub const NOTE: &str = "Decodes frames at 6.144 Msps (sample-rate coefficient 0): the bootstrap, L1-Basic and L1-Detail, and \
    one PLP in every subframe to baseband packets, with the ALP packets, the IP and UDP layers, the service list and the \
    ROUTE objects that they carry. The setting plp chooses the PLP by its id; without it the first core PLP with low \
    level signalling that the stack decodes is taken, or the first core PLP that it decodes. Tested on the stack's own test signal: \
    L1-Basic modes 1 to 5, L1-Detail modes 1, 2, 5 and \
    7, one and several coded blocks and Preamble symbols, symbol-aligned and time-aligned frames, 8K, 16K and 32K FFT, \
    subframe boundary symbols with null cells, carrier reduction, the frequency interleaver on and off, a PLP of 4, 16, 64 \
    or 256 points at 16200 or 64800 bits and of 1024 or 4096 points at 64800 bits, with BCH, CRC or no outer code, FEC blocks that cross from frame to frame, a PLP \
    that begins after other cells, two PLPs in one subframe, non-dispersed and dispersed in subslices, the convolutional \
    time interleaver at its six depths, the hybrid time interleaver within a subframe (one and several TI blocks) and \
    across 1 to 16 subframes, with and without the cell interleaver, frames of three subframes with the three FFT \
    sizes, a PLP that continues from subframe to subframe with each time interleaver, a PLP that only a later \
    subframe carries, and layered division multiplexing: a core PLP with an enhanced PLP on its cells at injection \
    levels from 3 to 15 dB, without a time interleaver, with the hybrid one within a subframe and dummy enhanced \
    cells, with the hybrid one across subframes and with the convolutional one, and an enhanced PLP across two core \
    PLPs without a time interleaver and with the hybrid one. The setting chooses either layer; the enhanced PLP is \
    decoded after the core PLPs under it are decoded, encoded and mapped again and taken off its cells, and its \
    modulation error ratio is measured on what is left. A PLP bonded with another RF channel without SNR averaging is \
    decoded as a PLP of its own: only the baseband packets of this channel, each after a gap; the table of PLPs shows \
    the bonding and the BSID of the other channel. A subframe without boundary symbols was not exercised. Open in the hybrid time interleaver across \
    subframes: the standard names the unit that moves between the branches of the delay line a row, and the stack takes \
    it as as many consecutive cells of the twisted block read-out as the TI block has FEC blocks; this was not checked \
    against a signal. The test signal starts the rows of the convolutional time interleaver with zeros and not with the \
    cells of the standard. Built from the \
    standard and not exercised by a signal: the other guard intervals, pilot \
    patterns, code rates and L1-Detail modes, a subframe without boundary symbols. Not decoded: frames at another sample \
    rate (6.912 Msps and the others), MIMO, MISO, channel bonding with SNR averaging (half the cells of each FEC block \
    are on the other RF channel) and the merge of two bonded channels (the stack takes one), tone reservation, L1-Detail \
    additional parity with several coded blocks, an enhanced PLP on part of a core PLP under the convolutional time \
    interleaver, across more than four core PLPs or across core PLPs under the convolutional time interleaver or the \
    hybrid one across subframes, a subframe with an enhanced layer whose core PLPs are dispersed or do not follow each \
    other from cell 0 in the order of L1-Detail or where a core PLP before the last has other cell counts before and \
    after time interleaving, a core PLP with two injection levels on it or, without the hybrid time interleaver, \
    enhanced PLPs on part of it only, a subframe that ends more than 1 s after the bootstrap and the \
    subframes after it, a later subframe with a waveform that the stack does not decode and those after it, more than \
    one PLP at a time, a first \
    subframe that ends more than 1 s after the bootstrap. MMTP services are listed and not received. The media of a ROUTE service is given as \
    samples and not played. The channel estimate is a straight line between pilots in frequency and in time: echoes longer \
    than about a quarter of the pilot spacing and fast fading are not followed. The carrier offset is taken once per frame \
    and the sample clock is not corrected. No signal of a transmitter was decoded.";

/// The ATSC 3.0 receiver graph. The FEC blocks (`fec`) and all after them are the second stage.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<sdrkit_graph::Graph, GraphError> {
    let b = GraphBuilder::new()
        .add("sync", components::Sync::default())
        .add("frame", components::Frame::default())
        .add("fec", components::Fec::default())
        .add("report", results::Report::default())
        .input("sync.in")
        .connect("sync.iq", "frame.iq")
        .connect("sync.out", "frame.in")
        .connect("frame.out", "fec.in")
        .first_stage("front")
        .stage("frame", "fec", Stage::new(FEC_STAGE))
        .stage("sync", "report", Stage::new(FEC_STAGE))
        .costs(&CostTable::parse(COSTS), ID, "-")
        .connect("sync.state", "report.sync")
        .connect("fec.state", "report.frame")
        .clock(results::REPORT_S)
        .status_from("report");
    delivery(b, "fec.out").catalog(catalog).build()
}

/// The ATSC 3.0 receiver: the graph in a [`StackGraph`] that refuses a rate below the bootstrap rate, 6.144 Msps (the stack
/// requests 6.5 Msps or more from the radio). Command `service.select` chooses the service.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that the stack refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            bootstrap::RATE,
            "the sample rate is too low for ATSC 3.0 (6.144 Msps or more is needed)",
        )
        .route(PLP.key(), &["frame.plp"])?
        .command(
            "service.select",
            sdrkit_graph::CommandTarget::of("report", "service.select"),
        )
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let g = Atsc3Generator::from_config(cfg).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(g))
}

/// The ATSC 3.0 stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "ATSC 3.0",
    group: "TV",
    blurb: "ATSC 3.0 (NextGen TV), 6 MHz channel, ROUTE services",
    tuning: Tuning {
        min_hz: 174e6,
        max_hz: 806e6,
        ..Tuning::anywhere(539e6, 8e6, 6.5e6, 6e6)
    },
    submodes: &[],
    settings: Atsc3Setting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::Atsc3],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ATSC3),
    benchmarks: None,
};

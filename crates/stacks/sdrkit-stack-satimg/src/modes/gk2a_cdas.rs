//! GK-2A CDAS (`gk2a_cdas`): a QPSK data link of GEO-KOMPSAT-2A at 3 096 000 symbols/s, uncoded QPSK with frames of 1264
//! bytes behind five shortened Reed-Solomon code words. The stack lists the frames; it does not decode their content. The
//! frequency of the link is not known: the session gives it.
//!
//! Standard: CCSDS 131.0-B-5 (sync marker, randomiser, Reed-Solomon, interleave, virtual fill).

use super::tables::{no_settings, test_data, TableDisplay, TableStack};
use crate::components::link::{DemodComponent, FrontEndComponent};
use crate::components::table::{FrameTableComponent, Line, TableLink};
use crate::gen::{BitSource, GenError, ModParams, Modulator};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, SignalInfo,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph};
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig};
use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
use sdrkit_util_bits::xorshift::Xorshift64;

/// The stack id.
pub const ID: &str = "gk2a_cdas";
/// The least input rate.
pub const MIN_RATE: f64 = 6.5e6;
/// Symbols a second.
pub const SYMBOL_RATE: f64 = 3_096_000.0;
/// Bytes of a frame with its sync marker.
pub const FRAME: usize = 1264;
/// Data bytes of a frame.
pub const DATA: usize = 1100;
/// The coding of a frame: five Reed-Solomon (255,223) code words, each shortened by 3 bytes to (252,220), interleaved,
/// dual basis, behind the randomiser.
pub const CODE: ChannelConfig = ChannelConfig::rs_223(5).with_virtual_fill(3);

/// The link: QPSK with a roll-off of 0.5.
pub fn link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Qpsk,
        symbol_rate: SYMBOL_RATE,
        roll_off: 0.5,
        class: LinkClass::Geo,
        search: Search::PowerLine {
            power: 4,
            min_ratio: 0.0,
        },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: false,
        constellation: 256,
        interleaved: false,
    }
}

/// The frame rules: the four bit streams of uncoded QPSK, the CCSDS marker with 3 wrong bits to acquire and 9 to track,
/// either polarity, then the randomiser and the code.
pub fn table() -> TableLink {
    TableLink {
        sync: WordSyncConfig {
            word: 0x1ACF_FC1D,
            word_bits: 32,
            frame_bits: 8 * FRAME,
            acquire_errors: 3,
            track_errors: 9,
            complement: true,
            max_misses: 3,
        },
        line: Line::Quad,
        code: Some(CODE),
        invert: false,
        retry_plain: false,
        data_randomised: false,
        llr_rate: 2.0 * SYMBOL_RATE,
    }
}

/// What the display says.
pub static STACK: TableStack = TableStack {
    name: "GK-2A CDAS",
    frame_period_s: 8.0 * FRAME as f64 / (2.0 * SYMBOL_RATE),
    pm: false,
    min_rate: MIN_RATE,
    note: "Experimental: not confirmed on a real signal. No public document describes this link. Not confirmed by a public \
        document: the frequency (the frequency of this link is not known: tune by hand), QPSK at 3 096 000 symbols/s, the \
        frame of 1264 bytes, the five Reed-Solomon code words shortened to (252,220) and the randomiser. Frame contents \
        are not decoded; whether the headers are TM or AOS headers is not known.",
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The CDAS graph.
///
/// # Errors
///
/// None at present; the type leaves room for the errors of [`GraphBuilder`].
pub fn builder() -> Result<GraphBuilder, GraphError> {
    Ok(GraphBuilder::new()
        .add("frontend", FrontEndComponent::new(link(), MIN_RATE, false))
        .add("demod", DemodComponent::new(link()))
        .add("frames", FrameTableComponent::new(table()))
        .add("display", TableDisplay::new(&STACK))
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "frames.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("frames.report", "display.table")
        .feedback("display.control", "demod.control")
        .output("frames.frames")
        .status_from("display")
        .clock(REPORT_S))
}

/// The CDAS receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "GK-2A CDAS needs at least 6.5 MHz of samples")
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        CdasGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "GK-2A CDAS",
    group: crate::GROUP,
    blurb: "A QPSK data link of GEO-KOMPSAT-2A (3.096 Msym/s): the frame list. Its frequency is not known: tune by hand.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 1e6,
        max_hz: 12e9,
        // The frequency of this link is not known. The default is the one of the test signal; the host gives the frequency
        // of a session.
        ..Tuning::anywhere(1690e6, 8e6, MIN_RATE, 4.66e6)
    },
    submodes: &[],
    settings: no_settings,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The bytes of test frame `counter`: the sync marker, then 1100 data bytes coded and randomised.
pub fn test_frame(counter: u32, rng: &mut Xorshift64) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME);
    let data = test_data(counter, DATA, rng);
    let coded = ChannelCoder::new(CODE).is_ok_and(|mut c| c.encode(&data, &mut out).is_ok());
    if !coded {
        out.clear();
        out.resize(FRAME, 0);
    }
    out
}

/// The channel bits of the CDAS test signal.
pub struct CdasSource {
    rng: Xorshift64,
    counter: u32,
}

impl BitSource for CdasSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let f = test_frame(self.counter, &mut self.rng);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        for b in f {
            out.extend((0..8).rev().map(|k| (b >> k) & 1));
        }
    }
}

/// The CDAS test signal: the clean signal at unit mean power.
pub struct CdasGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<CdasSource>,
}

impl CdasGenerator {
    /// From a configuration.
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let l = link();
        let modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
        let src = CdasSource {
            rng: Xorshift64::new(cfg.seed ^ Xorshift64::GOLDEN),
            counter: 0,
        };
        Ok(CdasGenerator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for CdasGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.m.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
}

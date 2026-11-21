//! GOES MDL (`goes_mdl`): the multi-use data link of GOES-13, 14 and 15, uncoded QPSK at 200 000 symbols/s with frames of
//! 464 bytes. The stack lists the frames; it does not decode their content.
//!
//! Standard: GOES N Series Data Book section 19 (frequency, bandwidth).

use super::tables::{no_settings, TableDisplay, TableStack};
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
use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
use sdrkit_util_bits::xorshift::Xorshift64;

/// The stack id.
pub const ID: &str = "goes_mdl";
/// The least input rate.
pub const MIN_RATE: f64 = 500e3;
/// Symbols a second.
pub const SYMBOL_RATE: f64 = 200_000.0;
/// The sync word as the receiver searches it.
pub const SYNC: u32 = 0x0BEB_CC80;
/// Bytes of a frame with its sync word.
pub const FRAME: usize = 464;

/// The link: QPSK with a roll-off of 0.5. The front end decimates to 4 samples a symbol or more and has no band locator: a
/// capture many times wider than this narrow signal does not need one.
pub fn link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Qpsk,
        symbol_rate: SYMBOL_RATE,
        roll_off: 0.5,
        class: LinkClass::Leo,
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

/// The frame rules: the four bit streams of uncoded QPSK, the sync word with 3 wrong bits to acquire and 9 to track, either
/// polarity, and every byte inverted for the table.
pub fn table() -> TableLink {
    TableLink {
        sync: WordSyncConfig {
            word: u64::from(SYNC),
            word_bits: 32,
            frame_bits: 8 * FRAME,
            acquire_errors: 3,
            track_errors: 9,
            complement: true,
            max_misses: 3,
        },
        line: Line::Quad,
        code: None,
        invert: true,
        retry_plain: false,
        data_randomised: false,
        llr_rate: 2.0 * SYMBOL_RATE,
    }
}

/// What the display says.
pub static STACK: TableStack = TableStack {
    name: "GOES MDL",
    frame_period_s: 8.0 * FRAME as f64 / (2.0 * SYMBOL_RATE),
    pm: false,
    min_rate: MIN_RATE,
    note: "Experimental: not confirmed on a real signal. A public document gives the frequency and the bandwidth. Not confirmed by a public document: QPSK at 200 000 symbols/s, the sync word and its \
        polarity, the frame of 464 bytes. The table shows every byte of a frame inverted, the sync word too. Frame \
        contents are not decoded.",
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The MDL graph.
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

/// The MDL receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "GOES MDL needs at least 500 kHz of samples")
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        MdlGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "GOES MDL",
    group: crate::GROUP,
    blurb: "The multi-use data link of GOES-13, 14 and 15 (QPSK, 200 ksym/s): the frame list.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(1681.478e6, 3e6, MIN_RATE, 320e3)
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

/// The bytes of test frame `counter` as sent: the sync word upright, then the 16-bit counter low byte first and random
/// bytes, all inverted.
pub fn test_frame(counter: u16, rng: &mut Xorshift64) -> Vec<u8> {
    let mut f = vec![0u8; FRAME];
    f[..4].copy_from_slice(&SYNC.to_be_bytes());
    f[4..6].copy_from_slice(&counter.to_le_bytes());
    for b in &mut f[6..] {
        *b = rng.step() as u8;
    }
    for b in &mut f[4..] {
        *b = !*b;
    }
    f
}

/// The channel bits of the MDL test signal.
pub struct MdlSource {
    rng: Xorshift64,
    counter: u16,
}

impl BitSource for MdlSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let f = test_frame(self.counter, &mut self.rng);
        self.counter = self.counter.wrapping_add(1);
        for b in f {
            out.extend((0..8).rev().map(|k| (b >> k) & 1));
        }
    }
}

/// The MDL test signal: the clean signal at unit mean power.
pub struct MdlGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<MdlSource>,
}

impl MdlGenerator {
    /// From a configuration.
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let l = link();
        let modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
        let src = MdlSource {
            rng: Xorshift64::new(cfg.seed ^ Xorshift64::GOLDEN),
            counter: 0,
        };
        Ok(MdlGenerator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for MdlGenerator {
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

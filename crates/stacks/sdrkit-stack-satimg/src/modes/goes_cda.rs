//! GOES CDA (`goes_cda`): the telemetry downlinks of the GOES satellites to their command and data acquisition station. The
//! submodes `r40` and `r4` are GOES-R direct BPSK at 40 000 and 4000 symbols/s with frames of 256 bytes behind a
//! shortened Reed-Solomon code; `n` is GOES-N phase modulation with a residual carrier at 4000 symbols/s with frames of 512
//! bytes. The stack lists the frames; it does not decode their content.
//!
//! Standard: GOES-R Series Data Book section 19 (link, rates); GOES N Series Data Book section 19 (link, rate); CCSDS
//! 131.0-B-5 (randomiser, Reed-Solomon, virtual fill).

use super::tables::{test_data, TableDisplay, TableStack};
use crate::components::link::DemodComponent;
use crate::components::link::FrontEndComponent;
use crate::components::pm::PmDemodComponent;
use crate::components::table::{FrameTableComponent, Line, TableLink};
use crate::gen::{BitSource, CcsdsCoder, CodeOptions, GenError, ModParams, Modulator};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_algo_sim::modulate::PhaseMod;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting,
    SettingSet, SignalInfo, StackConfig, StackDescriptor, SubMode, Tuning,
};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph};
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig};
use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
use sdrkit_util_bits::xorshift::Xorshift64;

/// The stack id.
pub const ID: &str = "goes_cda";
/// The least input rate.
pub const MIN_RATE: f64 = 250e3;
/// The sync marker of the GOES-R frames (CCSDS 131.0-B-5 clause 9).
pub const ASM: u32 = 0x1ACF_FC1D;
/// The sync word of the GOES-N frames.
pub const N_SYNC: u32 = 0xFAF3_201C;
/// Bytes of a GOES-R frame with its sync word.
pub const R_FRAME: usize = 256;
/// Bytes of a GOES-N frame with its sync word.
pub const N_FRAME: usize = 512;
/// The radio's centre lies this many Hz above the signal, clear of its DC spike.
pub const TUNE_OFFSET_HZ: f64 = 100e3;
/// The phase index of the GOES-N test signal, degrees.
pub const INDEX_DEG: f64 = 60.0;

/// The coding of a GOES-R frame: one Reed-Solomon (255,223) code word shortened by 3 bytes to (252,220), dual basis, behind
/// the randomiser.
pub const R_CODE: ChannelConfig = ChannelConfig::rs_223(1).with_virtual_fill(3);

/// A link of the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CdaLink {
    /// GOES-R, 40 000 symbols/s.
    R40,
    /// GOES-R, 4000 symbols/s.
    R4,
    /// GOES-N, 4000 symbols/s.
    N,
}

impl CdaLink {
    /// The link of a submode id; the default for `None`.
    pub fn of(submode: Option<&str>) -> CdaLink {
        match submode {
            Some("r4") => CdaLink::R4,
            Some("n") => CdaLink::N,
            _ => CdaLink::R40,
        }
    }
    /// Symbols a second.
    pub fn symbol_rate(self) -> f64 {
        match self {
            CdaLink::R40 => 40_000.0,
            CdaLink::R4 | CdaLink::N => 4000.0,
        }
    }
    /// The demodulator's link. GOES-N is phase modulated: the front end needs only its rate and filter. The front end
    /// decimates to 4 samples a symbol or more and has no band locator: a capture many times wider than these narrow
    /// signals does not need one.
    pub fn link(self) -> LinkParams {
        let n = self == CdaLink::N;
        LinkParams {
            modulation: Modulation::Bpsk,
            symbol_rate: self.symbol_rate(),
            roll_off: if n { 0.6 } else { 0.5 },
            class: LinkClass::Leo,
            search: Search::PowerLine {
                power: 2,
                min_ratio: 0.0,
            },
            snr: SnrEstimator::Decisions,
            bpsk_plus_is_zero: false,
            constellation: 256,
            interleaved: false,
        }
    }
    /// The frame rules.
    pub fn table(self) -> TableLink {
        let r = self != CdaLink::N;
        TableLink {
            sync: WordSyncConfig {
                word: u64::from(if r { ASM } else { N_SYNC }),
                word_bits: 32,
                frame_bits: 8 * if r { R_FRAME } else { N_FRAME },
                acquire_errors: 3,
                track_errors: 9,
                complement: true,
                max_misses: 3,
            },
            line: if r { Line::NrzM } else { Line::Nrz },
            code: r.then_some(R_CODE),
            invert: false,
            retry_plain: false,
            data_randomised: false,
            llr_rate: self.symbol_rate(),
        }
    }
    /// What the display says.
    pub fn stack(self) -> &'static TableStack {
        match self {
            CdaLink::R40 => &R40,
            CdaLink::R4 => &R4,
            CdaLink::N => &N,
        }
    }
}

const NOTE_R: &str = "Experimental: not confirmed on a real signal. A public document gives the frequency, direct BPSK and \
    the coded and uncoded rates. Not confirmed by a public document: the sync word, the frame of 256 bytes, NRZ-M, the \
    Reed-Solomon code shortened to (252,220) with its dual basis, the randomiser. Frame contents are not decoded; whether \
    the headers are TM or AOS headers is not known.";
const NOTE_N: &str = "Experimental: not confirmed on a real signal. A public document gives the frequency and the rate of \
    4 kbit/s. Not confirmed by a public document: phase modulation with a residual carrier, NRZ data, the sync word FAF3201C, \
    the frame of 512 bytes, the matched filter. Frame contents are not decoded; whether the headers are TM or AOS headers \
    is not known.";

/// GOES-R at 40 000 symbols/s.
pub static R40: TableStack = TableStack {
    name: "GOES CDA (GOES-R 40 ksps)",
    frame_period_s: 2048.0 / 40_000.0,
    pm: false,
    min_rate: MIN_RATE,
    note: NOTE_R,
};
/// GOES-R at 4000 symbols/s.
pub static R4: TableStack = TableStack {
    name: "GOES CDA (GOES-R 4 ksps)",
    frame_period_s: 2048.0 / 4000.0,
    pm: false,
    min_rate: MIN_RATE,
    note: NOTE_R,
};
/// GOES-N at 4000 symbols/s.
pub static N: TableStack = TableStack {
    name: "GOES CDA (GOES-N)",
    frame_period_s: 4096.0 / 4000.0,
    pm: true,
    min_rate: MIN_RATE,
    note: NOTE_N,
};

const TEST: &str = "Test signal";

/// The phase index of the GOES-N test signal.
pub const GEN_PM_INDEX: Setting<field::F64> =
    Setting::new("gen_pm_index", "Test phase index (GOES-N)", "°", INDEX_DEG)
        .range(10.0, 85.0, 0.5)
        .section(TEST)
        .advanced();

sdrkit_core::settings! {
    /// The settings of `goes_cda`: the options of its test signal.
    pub enum CdaSetting {
        /// Test: the phase index of the GOES-N signal.
        GenPmIndex = GEN_PM_INDEX,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The graph of `link`.
///
/// # Errors
///
/// None at present; the type leaves room for the errors of [`GraphBuilder`].
pub fn builder(link: CdaLink) -> Result<GraphBuilder, GraphError> {
    let l = link.link();
    let g = GraphBuilder::new().add(
        "frontend",
        FrontEndComponent::new(l.clone(), MIN_RATE, false),
    );
    let g = if link == CdaLink::N {
        g.add(
            "demod",
            PmDemodComponent::new(l.symbol_rate, l.roll_off, 0.45 * l.symbol_rate),
        )
    } else {
        g.add("demod", DemodComponent::new(l))
    };
    Ok(g.add("frames", FrameTableComponent::new(link.table()))
        .add("display", TableDisplay::new(link.stack()))
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

/// The receiver of the submode of `cfg`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let link = CdaLink::of(cfg.submode.as_deref());
    let g = builder(link)?.catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "GOES CDA needs at least 250 kHz of samples")
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        CdaGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "GOES CDA",
    group: crate::GROUP,
    blurb: "The telemetry downlinks of GOES-R (BPSK, 40 or 4 ksps) and GOES-N (PM, 4 kbit/s) to their ground station: the frame list. It needs a fixed dish.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 1670e6,
        max_hz: 1710e6,
        tune_offset_hz: TUNE_OFFSET_HZ,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(1693e6, 1e6, MIN_RATE, 80e3)
    },
    // The default frequencies of the submodes are those of their test signals.
    submodes: &[
        SubMode {
            id: "r40",
            name: "GOES-R 40 ksps",
            default_hz: 1693e6,
            ..SubMode::DEFAULT
        },
        SubMode {
            id: "r4",
            name: "GOES-R 4 ksps",
            default_hz: 1693e6,
            ..SubMode::DEFAULT
        },
        SubMode {
            id: "n",
            name: "GOES-N",
            default_hz: 1694e6,
            ..SubMode::DEFAULT
        },
    ],
    settings: CdaSetting::schema,
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

/// The bytes of GOES-R test frame `counter`: the sync marker, then 220 data bytes coded, randomised.
pub fn test_r_frame(counter: u32, rng: &mut Xorshift64) -> Vec<u8> {
    let mut out = Vec::with_capacity(R_FRAME);
    let data = test_data(counter, 220, rng);
    let coded = ChannelCoder::new(R_CODE).is_ok_and(|mut c| c.encode(&data, &mut out).is_ok());
    if !coded {
        out.clear();
        out.resize(R_FRAME, 0);
    }
    out
}

/// The bytes of GOES-N test frame `counter`: the sync word, then 508 data bytes.
pub fn test_n_frame(counter: u32, rng: &mut Xorshift64) -> Vec<u8> {
    let mut out = N_SYNC.to_be_bytes().to_vec();
    out.extend(test_data(counter, N_FRAME - 4, rng));
    out
}

/// The channel bits of the GOES-R test signal: the frames, NRZ-M encoded.
pub struct RSource {
    coder: CcsdsCoder,
    rng: Xorshift64,
    counter: u32,
}

impl BitSource for RSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let f = test_r_frame(self.counter, &mut self.rng);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        self.coder.push(&f, out);
    }
}

/// The samples of the GOES-N test signal: frames, NRZ, phase modulated with a residual carrier.
pub struct NSource {
    modulator: PhaseMod,
    rng: Xorshift64,
    counter: u32,
    bits: Vec<u8>,
    samples: Vec<Complex32>,
    next: usize,
}

impl NSource {
    /// Fills `out` with samples.
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut filled = 0;
        while filled < out.len() {
            if self.next >= self.samples.len() {
                let f = test_n_frame(self.counter, &mut self.rng);
                self.counter = (self.counter + 1) & 0xFF_FFFF;
                self.bits.clear();
                for b in f {
                    self.bits.extend((0..8).rev().map(|k| (b >> k) & 1));
                }
                self.samples.clear();
                self.next = 0;
                self.modulator.push(&self.bits, &mut self.samples);
                continue;
            }
            let n = (self.samples.len() - self.next).min(out.len() - filled);
            out[filled..filled + n].copy_from_slice(&self.samples[self.next..self.next + n]);
            self.next += n;
            filled += n;
        }
        out.len()
    }
}

enum Kind {
    R(Box<Modulator<RSource>>),
    N(Box<NSource>),
}

/// The GOES CDA test signal of the submode: the clean signal at unit power, [`TUNE_OFFSET_HZ`] below the centre of the
/// samples, as a radio tuned with the stack's offset sees it.
pub struct CdaGenerator {
    rate: f64,
    info: SignalInfo,
    kind: Kind,
    phase: f64,
}

impl CdaGenerator {
    /// From a configuration: the submode and `gen_pm_index`. The schema of the settings must be [`CdaSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let link = CdaLink::of(cfg.submode.as_deref());
        let l = link.link();
        let rng = Xorshift64::new(cfg.seed ^ Xorshift64::GOLDEN);
        let (info, kind) = if link == CdaLink::N {
            let index = cfg.settings.get(&GEN_PM_INDEX).to_radians();
            (
                SignalInfo::new(l.symbol_rate * (1.0 + l.roll_off))
                    .with_symbol_rate(l.symbol_rate)
                    .with_bit_rate(l.symbol_rate),
                Kind::N(Box::new(NSource {
                    modulator: PhaseMod::new(l.roll_off, 12.0, cfg.rate_hz / l.symbol_rate, index)?,
                    rng,
                    counter: 0,
                    bits: Vec::new(),
                    samples: Vec::new(),
                    next: 0,
                })),
            )
        } else {
            let modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
            let src = RSource {
                coder: CcsdsCoder::new(CodeOptions {
                    nrzm: true,
                    invert_g2: false,
                    conv: false,
                }),
                rng,
                counter: 0,
            };
            (
                modem.signal_info(),
                Kind::R(Box::new(Modulator::new(modem, src)?)),
            )
        };
        Ok(CdaGenerator {
            rate: cfg.rate_hz,
            info,
            kind,
            phase: 0.0,
        })
    }
}

impl Generator for CdaGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = match &mut self.kind {
            Kind::R(m) => m.generate(out),
            Kind::N(n) => n.generate(out),
        };
        // Move the signal to -TUNE_OFFSET_HZ.
        let step = -std::f64::consts::TAU * TUNE_OFFSET_HZ / self.rate;
        for x in &mut out[..n] {
            *x *= Complex32::from_polar(1.0, self.phase as f32);
            self.phase = (self.phase + step).rem_euclid(std::f64::consts::TAU);
        }
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
}

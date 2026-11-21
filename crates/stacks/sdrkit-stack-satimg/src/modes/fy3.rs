//! FY-3 AHRPT and MPT (`fy3_ahrpt`, `fy3_mpt`): QPSK with a K = 7 code on each rail, a differential decoder, CADUs in
//! the dual basis, and the instruments by virtual channel (VIRR, MERSI of every generation, MWRI and MWRI-2).

use super::display::{
    instrument_choices, link_fields, status, view_choices, with_series, Histories,
};
use crate::components::instruments::instrument_owner;
use crate::components::link::{
    control_port, item_format, item_port, DemodComponent, DemodReport, FrameDecoder, FrameLink,
    FrameReport, FrameStats, FrontEndComponent, FrontReport, PacketReport, VcduComponent,
};
use crate::components::{controls, last, Control};
use crate::gen::{aos_frame, cadu_bytes, test_pixel, BitSource, GenError, ModParams, Modulator};
use crate::instruments::fy3::{
    build_mersi_scan, build_mwri, build_virr, channels, satellite, thermal, width, Fy3Instruments,
    Instrument, InstrumentStats, MersiLayout,
};
use crate::items::{Cadu, Vcdu};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_fec::conv::{codes, Encoder, Viterbi};
use sdrkit_algo_fec::rs::Basis;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::{Strip, View};
use crate::typed::{
    Fy3Fields, Fy3InstrumentRow, ImageRow, LinkFields, FY3_INSTRUMENTS, IMAGES,
};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ChoiceDecl, Complex32, ConfigError, GenConfig, Generator, HostRequest, LockProfile,
    Receiver, Results, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Item, Port, StackGraph, Timed,
};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::codesync::transform;
use sdrkit_proto_ccsds::tm::FirstHeader;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// A preset: (value, label, frequency, symbol rate, roll-off, MPT).
pub type Preset = (&'static str, &'static str, f64, f64, f64, bool);

/// The presets of FY-3.
pub const PRESETS: [Preset; 8] = [
    (
        "fy3c",
        "1701.400 FY-3C AHRPT (2.6 Msym/s)",
        1701.4e6,
        2.6e6,
        0.5,
        false,
    ),
    (
        "fy3ab",
        "1704.500 FY-3A / FY-3B AHRPT (2.8 Msym/s)",
        1704.5e6,
        2.8e6,
        0.5,
        false,
    ),
    (
        "fy3g",
        "7790 FY-3G (6 Msym/s, X-band, needs a down-converter)",
        7790e6,
        6e6,
        0.35,
        false,
    ),
    (
        "fy3d",
        "7820 FY-3D (30 Msym/s, X-band, needs a down-converter and 40 Msps)",
        7820e6,
        30e6,
        0.35,
        false,
    ),
    (
        "fy3f",
        "7790 FY-3F (38.4 Msym/s, X-band)",
        7790e6,
        38.4e6,
        0.35,
        false,
    ),
    (
        "fy3eh",
        "7860 FY-3E / FY-3H (38.4 Msym/s, X-band)",
        7860e6,
        38.4e6,
        0.35,
        false,
    ),
    (
        "mpt_ab",
        "7750 FY-3A / FY-3B MPT (18.7 Msym/s, X-band, needs a down-converter and 28 Msps)",
        7750e6,
        18.7e6,
        0.5,
        true,
    ),
    (
        "mpt_c",
        "7780 FY-3C MPT (18.7 Msym/s, X-band)",
        7780e6,
        18.7e6,
        0.5,
        true,
    ),
];

/// The candidate symbol rates of a product.
pub fn candidates(mpt: bool) -> &'static [f64] {
    if mpt {
        &[18.7e6]
    } else {
        &[2.6e6, 2.8e6, 6e6, 30e6, 38.4e6]
    }
}

/// The link at a symbol rate and roll-off.
pub fn link(rs: f64, roll_off: f64) -> LinkParams {
    LinkParams {
        modulation: Modulation::Qpsk,
        symbol_rate: rs,
        roll_off,
        class: LinkClass::Leo,
        search: Search::Template { range_hz: 75e3 },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: false,
        constellation: 256,
        interleaved: false,
    }
}

/// The roll-off of a symbol rate: the preset's when one matches within 1 %, else 0.5 (L-band) or 0.35.
pub fn roll_off_for(rs: f64) -> f64 {
    PRESETS
        .iter()
        .find(|p| (p.3 - rs).abs() <= 0.01 * rs)
        .map_or(if rs < 5e6 { 0.5 } else { 0.35 }, |p| p.4)
}

/// The preset choices of a product: automatic, then the presets of the product.
fn preset_choices(mpt: bool) -> Vec<ChoiceDecl> {
    let mut v = vec![ChoiceDecl::new("auto", "Automatic")];
    v.extend(
        PRESETS
            .iter()
            .filter(|p| p.5 == mpt)
            .map(|p| ChoiceDecl::new(p.0, p.1)),
    );
    v
}

/// The preset of choice index `i` of the preset setting of a product, `None` for automatic.
pub fn preset_at(mpt: bool, i: usize) -> Option<&'static Preset> {
    i.checked_sub(1)
        .and_then(|k| PRESETS.iter().filter(|p| p.5 == mpt).nth(k))
}

const TEST: &str = "Test signal";

/// The AHRPT satellite.
pub const AHRPT_PRESET: CatalogueSetting = CatalogueSetting {
    key: "preset",
    label: "Satellite",
    help: "Retunes to the preset's frequency; restarts.",
    choices: || preset_choices(false),
    default: "auto",
    advanced: false,
};
/// The MPT satellite.
pub const MPT_PRESET: CatalogueSetting = CatalogueSetting {
    key: "preset",
    label: "Satellite",
    help: "Retunes to the preset's frequency; restarts.",
    choices: || preset_choices(true),
    default: "auto",
    advanced: false,
};
/// The symbol rate in Msym/s, 0 for automatic.
pub const SYMBOL_RATE: Setting<field::F64> =
    Setting::new("symbol_rate", "Symbol rate", "Msym/s", 0.0)
        .range(0.0, 500.0, 0.001)
        .help("0 = automatic. Restarts.")
        .restart();
/// The roll-off, 0 for automatic.
pub const ROLL_OFF: Setting<field::F64> = Setting::new("roll_off", "Roll-off", "", 0.0)
    .range(0.0, 1.0, 0.01)
    .help("0 = automatic. Restarts.")
    .restart();
/// The AHRPT instrument shown.
pub const AHRPT_INSTRUMENT: CatalogueSetting = CatalogueSetting {
    key: "instrument",
    label: "Instrument",
    help: "The instrument whose picture is shown.",
    choices: || instrument_choices(layouts(false)),
    default: "VIRR",
    advanced: false,
};
/// The MPT instrument shown.
pub const MPT_INSTRUMENT: CatalogueSetting = CatalogueSetting {
    key: "instrument",
    label: "Instrument",
    help: "The instrument whose picture is shown.",
    choices: || instrument_choices(layouts(true)),
    default: "MERSI",
    advanced: false,
};
/// The AHRPT view.
pub const AHRPT_VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "The composite, else a channel. A view that the instrument shown lacks is refused.",
    choices: || view_choices(layouts(false)),
    default: "true",
    advanced: false,
};
/// The MPT view.
pub const MPT_VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "The composite, else a channel. A view that the instrument shown lacks is refused.",
    choices: || view_choices(layouts(true)),
    default: "true",
    advanced: false,
};
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow", "", true);
/// The spacecraft id of the test signal (51 is FY-3C).
pub const GEN_SATELLITE: Setting<field::I64> =
    Setting::new("gen_satellite", "Test spacecraft id", "", 51)
        .range(0.0, 255.0, 1.0)
        .section(TEST)
        .advanced();
/// The symbol rate of the AHRPT test signal.
pub const GEN_SYMBOL_RATE_AHRPT: Setting<field::F64> =
    Setting::new("gen_symbol_rate", "Test symbol rate", "Msym/s", 2.6)
        .range(0.1, 100.0, 0.001)
        .section(TEST)
        .advanced();
/// The symbol rate of the MPT test signal.
pub const GEN_SYMBOL_RATE_MPT: Setting<field::F64> =
    Setting::new("gen_symbol_rate", "Test symbol rate", "Msym/s", 18.7)
        .range(0.1, 100.0, 0.001)
        .section(TEST)
        .advanced();
/// The test signal negates Q.
pub const GEN_QNEG: Setting<field::Bool> = Setting::new("gen_qneg", "Test Q negated", "", false)
    .section(TEST)
    .advanced();
/// The test signal exchanges I and Q.
pub const GEN_SWAP: Setting<field::Bool> =
    Setting::new("gen_swap", "Test I and Q exchanged", "", false)
        .section(TEST)
        .advanced();
/// A fill frame every this many frames of the test signal, 0 for the instrument's own share.
pub const GEN_FILL_EVERY: Setting<field::I64> =
    Setting::new("gen_fill_every", "Test fill frame every", "", 0)
        .range(0.0, 1000.0, 1.0)
        .help("0: the instrument's own share of fill frames.")
        .section(TEST)
        .advanced();

sdrkit_core::settings! {
    /// The settings of `fy3_ahrpt`, with the options of its test signal.
    pub enum AhrptSetting {
        /// The satellite.
        Preset = AHRPT_PRESET,
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The instrument shown.
        Instrument = AHRPT_INSTRUMENT,
        /// The view.
        View = AHRPT_VIEW,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// Test: the spacecraft id.
        GenSatellite = GEN_SATELLITE,
        /// Test: the symbol rate.
        GenSymbolRate = GEN_SYMBOL_RATE_AHRPT,
        /// Test: Q negated.
        GenQneg = GEN_QNEG,
        /// Test: I and Q exchanged.
        GenSwap = GEN_SWAP,
        /// Test: fill frames.
        GenFillEvery = GEN_FILL_EVERY,
    }
}

sdrkit_core::settings! {
    /// The settings of `fy3_mpt`, with the options of its test signal.
    pub enum MptSetting {
        /// The satellite.
        Preset = MPT_PRESET,
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The instrument shown.
        Instrument = MPT_INSTRUMENT,
        /// The view.
        View = MPT_VIEW,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// Test: the spacecraft id.
        GenSatellite = GEN_SATELLITE,
        /// Test: the symbol rate.
        GenSymbolRate = GEN_SYMBOL_RATE_MPT,
        /// Test: Q negated.
        GenQneg = GEN_QNEG,
        /// Test: I and Q exchanged.
        GenSwap = GEN_SWAP,
        /// Test: fill frames.
        GenFillEvery = GEN_FILL_EVERY,
    }
}

sdrkit_core::settings! {
    /// The settings of the AHRPT display.
    pub enum AhrptDisplaySetting {
        /// The satellite.
        Preset = AHRPT_PRESET,
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
    }
}

sdrkit_core::settings! {
    /// The settings of the MPT display: those of [`AhrptDisplaySetting`] in the same order, with the MPT presets.
    pub enum MptDisplaySetting {
        /// The satellite.
        Preset = MPT_PRESET,
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
    }
}

/// The settings of a product, with the options of its test signal.
pub fn schema_for(mpt: bool) -> Arc<Schema> {
    if mpt {
        MptSetting::schema()
    } else {
        AhrptSetting::schema()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The rail decoder.

/// The syndrome of a rate 1/2 code pair window: zero for a valid code stream.
#[inline]
fn syndrome(r1: u8, r2: u8) -> u8 {
    const G1: u8 = 0o171;
    const G2: u8 = 0o133;
    (((r1 & G2).count_ones() ^ (r2 & G1).count_ones()) & 1) as u8
}

/// One rail's synchroniser and decoder.
struct Rail {
    hard: Vec<u8>,
    // The LLRs behind `hard`, decoded once the phase is chosen so nothing collected is lost.
    soft: Vec<i8>,
    choice: Option<(u8, u8)>,
    quality: f32,
    viterbi: Viterbi,
    pend: Option<i8>,
    n: u64,
    // The hard code symbols of each step since `hist_base`, and the encoder that re-codes the decoded bits.
    hist: Vec<u8>,
    hist_base: u64,
    enc: Encoder,
    // The code symbols of this call that the decoder has not had yet ([`rail_decode`]).
    batch: Vec<i8>,
}

impl Rail {
    fn new() -> Self {
        Rail {
            hard: Vec::with_capacity(40_000),
            soft: Vec::with_capacity(40_000),
            choice: None,
            quality: 0.0,
            viterbi: Viterbi::new(codes::K7_171_133, 256).expect("the K = 7 code is valid"),
            pend: None,
            n: 0,
            hist: Vec::with_capacity(8192),
            batch: Vec::with_capacity(32_768),
            hist_base: 0,
            enc: Encoder::new(codes::K7_171_133),
        }
    }
    fn start_decoding(&mut self) {
        self.viterbi.reset();
        self.pend = None;
        self.hist.clear();
        self.hist_base = 0;
        self.enc = Encoder::new(codes::K7_171_133);
    }
    /// The syndrome error rate of each (pairing, transform) over the collected hard symbols.
    fn rates(&self) -> [[f32; 8]; 2] {
        let mut out = [[0.5f32; 8]; 2];
        for (p, row) in out.iter_mut().enumerate() {
            for t in 0..8u8 {
                let (mut r1, mut r2) = (0u8, 0u8);
                let (mut errs, mut n) = (0u32, 0u32);
                let mut k = p;
                while k + 1 < self.hard.len() {
                    // As LLRs (positive means 0), through the transform, back to bits.
                    let a: i8 = if self.hard[k] == 1 { -1 } else { 1 };
                    let b: i8 = if self.hard[k + 1] == 1 { -1 } else { 1 };
                    let (x, y) = transform(t, a, b);
                    r1 = ((r1 >> 1) | (((x < 0) as u8) << 6)) & 0x7F;
                    r2 = ((r2 >> 1) | (((y < 0) as u8) << 6)) & 0x7F;
                    if k >= p + 14 {
                        errs += syndrome(r1, r2) as u32;
                        n += 1;
                    }
                    k += 2;
                }
                if n > 0 {
                    row[t as usize] = errs as f32 / n as f32;
                }
            }
        }
        out
    }
}

/// The differential decoder of the rail bits, under one of 8 hypotheses (I/Q exchanged, Q negated, output order).
#[derive(Clone, Copy, Debug, Default)]
struct Diff {
    hyp: u8,
    px: u8,
    py: u8,
}

impl Diff {
    #[inline]
    fn step(&mut self, x: u8, y: u8, out: &mut Vec<u8>) {
        let (mut x, mut y) = (x, y);
        if self.hyp & 1 != 0 {
            std::mem::swap(&mut x, &mut y);
        }
        let yt = y ^ ((self.hyp >> 1) & 1);
        let (hi, lo) = if x != yt {
            (y ^ self.py, x ^ self.px)
        } else {
            (x ^ self.px, y ^ self.py)
        };
        self.px = x;
        self.py = y;
        if self.hyp & 4 != 0 {
            out.push(lo);
            out.push(hi);
        } else {
            out.push(hi);
            out.push(lo);
        }
    }
}

/// The differential encoder (the generator's side): the rail bits (x, y) that give the data pair (hi, lo).
pub fn diff_encode(hi: u8, lo: u8, px: u8, py: u8) -> (u8, u8) {
    for (x, y) in [(0u8, 0u8), (0, 1), (1, 0), (1, 1)] {
        let (h, l) = if x != y {
            (y ^ py, x ^ px)
        } else {
            (x ^ px, y ^ py)
        };
        if h == hi && l == lo {
            return (x, y);
        }
    }
    (0, 0)
}

/// The rail decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RailReport {
    /// Both rails have their phase.
    pub code_locked: bool,
    /// Each rail's lock quality (0 to 1).
    pub quality: [f32; 2],
    /// Each rail's (pairing, transform).
    pub phases: [Option<(u8, u8)>; 2],
    /// The differential hypothesis locked (0 to 7).
    pub hypothesis: Option<u8>,
    /// Each rail's Viterbi metric growth per step.
    pub metrics: [f64; 2],
    /// Frame counters.
    pub frames: FrameStats,
    /// Re-encoded bit error estimate.
    pub ber: f64,
    /// Good frames since the last report.
    pub good_now: u64,
    /// Seconds since the last good frame.
    pub since_good_s: f64,
}
impl Item for RailReport {
    const KIND: &'static str = "satimg.fy3_rail_report";
}

/// Soft QPSK symbols in (I then Q), CADUs out: two rail decoders, the differential decoder and 8 frame-sync hypotheses.
pub struct RailComponent {
    rails: [Rail; 2],
    bits_rate: f64,
    hyps: Vec<(Diff, FrameDecoder)>,
    locked: Option<usize>,
    xs: Vec<u8>,
    ys: Vec<u8>,
    data: Vec<u8>,
    cadus: Vec<Cadu>,
    trial: Vec<Cadu>,
    last_good: u64,
    symbols: u64,
    last_good_sym: u64,
    rs: f64,
    ber: (u64, u64),
}

impl Default for RailComponent {
    fn default() -> Self {
        RailComponent {
            rails: [Rail::new(), Rail::new()],
            bits_rate: 1.0,
            hyps: (0..8u8)
                .map(|h| {
                    (
                        Diff {
                            hyp: h,
                            ..Default::default()
                        },
                        FrameDecoder::new(FrameLink::LEO_DUAL)
                            .expect("the low-orbit frame rules are valid"),
                    )
                })
                .collect(),
            locked: None,
            xs: Vec::new(),
            ys: Vec::new(),
            data: Vec::new(),
            cadus: Vec::new(),
            trial: Vec::new(),
            last_good: 0,
            symbols: 0,
            last_good_sym: 0,
            rs: 1.0,
            ber: (0, 0),
        }
    }
}

impl RailComponent {
    fn restart(&mut self) {
        let rs = self.rs;
        let br = self.bits_rate;
        *self = RailComponent::default();
        self.rs = rs;
        self.bits_rate = br;
        for h in &mut self.hyps {
            h.1.set_bit_rate(br);
        }
    }

    fn acquire(&mut self) {
        // Both rails share the pairing: choose it by the sum of their best rates, then each rail's transform.
        let rates = [self.rails[0].rates(), self.rails[1].rates()];
        let best = |r: &[f32; 8]| r.iter().copied().fold(1.0f32, f32::min);
        let p =
            if best(&rates[0][0]) + best(&rates[1][0]) <= best(&rates[0][1]) + best(&rates[1][1]) {
                0
            } else {
                1
            };
        for (k, rail) in self.rails.iter_mut().enumerate() {
            let (t, r) = rates[k][p]
                .iter()
                .enumerate()
                .fold(
                    (0usize, 1.0f32),
                    |a, (t, &r)| if r < a.1 { (t, r) } else { a },
                );
            rail.quality = (1.0 - 2.0 * r).clamp(0.0, 1.0);
            if rail.quality > 0.3 {
                rail.choice = Some((p as u8, t as u8));
                rail.start_decoding();
            }
        }
        if self.rails.iter().any(|r| r.choice.is_none()) {
            for r in self.rails.iter_mut() {
                r.choice = None;
                r.hard.clear();
                r.soft.clear();
                r.n = 0;
            }
            return;
        }
        // Decode what was collected, so nothing is lost.
        for k in 0..2 {
            let buf = std::mem::take(&mut self.rails[k].soft);
            self.rails[k].n = 0;
            self.rails[k].hard.clear();
            for &s in &buf {
                rail_symbol(&mut self.rails[k], s);
            }
            self.rails[k].soft = buf;
            self.rails[k].soft.clear();
        }
    }
}

/// One LLR of a rail: collected while the phase is not known, else kept as a code symbol for [`rail_decode`].
fn rail_symbol(rail: &mut Rail, s: i8) {
    rail.n += 1;
    let Some((p, t)) = rail.choice else {
        rail.hard.push((s < 0) as u8);
        rail.soft.push(s);
        return;
    };
    if (rail.n - 1) % 2 == p as u64 {
        rail.pend = Some(s);
        return;
    }
    let Some(a) = rail.pend.take() else { return };
    let (x, y) = transform(t, a, s);
    rail.hist.push((((x < 0) as u8) << 1) | (y < 0) as u8);
    rail.batch.extend([x, y]);
}

/// Decodes the code symbols that [`rail_symbol`] kept and appends the bits to `out`, with the estimate of the bit error
/// rate from the decoded bits coded again. The decoder releases the same bits for every size of the batch.
fn rail_decode(rail: &mut Rail, out: &mut Vec<u8>, ber: &mut (u64, u64)) {
    if rail.batch.is_empty() {
        return;
    }
    let first = rail.viterbi.released();
    let m = out.len();
    rail.viterbi.push(&rail.batch, out);
    rail.batch.clear();
    for (j, &b) in out[m..].iter().enumerate() {
        let step = first + j as u64;
        let l = rail.enc.push_bit(b);
        if step < rail.hist_base {
            continue;
        }
        if let Some(&hh) = rail.hist.get((step - rail.hist_base) as usize) {
            ber.0 += ((l & 1) != hh >> 1) as u64 + (((l >> 1) & 1) != (hh & 1)) as u64;
            ber.1 += 2;
        }
    }
    // Keep only the steps not yet released.
    if rail.hist.len() > 4096 {
        let done = (rail.viterbi.released() - rail.hist_base) as usize;
        let d = done.min(rail.hist.len());
        rail.hist.drain(..d);
        rail.hist_base += d as u64;
    }
}

impl Component for RailComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.fy3_rails",
            vec![Port::llr("in"), control_port()],
            vec![item_port::<Cadu>("out"), item_port::<RailReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // Two code symbols a channel symbol.
        self.rs = inputs[0].rate / 2.0;
        self.bits_rate = self.rs;
        self.restart();
        Ok(vec![item_format::<Cadu>(), item_format::<RailReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        for c in controls(io, 1) {
            match c {
                Control::Link(l) => {
                    self.rs = l.symbol_rate;
                    self.bits_rate = l.symbol_rate;
                    self.restart();
                }
                Control::RestartCarrier | Control::RejectCarrier | Control::CodeSearch => {
                    self.restart()
                }
                Control::MinRate(_) | Control::Measure(_) => {}
            }
        }
        self.xs.clear();
        self.ys.clear();
        for pair in io.llrs(0).as_chunks::<2>().0 {
            self.symbols += 1;
            for (k, &s) in pair.iter().enumerate() {
                rail_symbol(&mut self.rails[k], s);
            }
            if self.rails[1].choice.is_none() && self.rails[1].hard.len() >= 40_000 {
                self.acquire();
            }
        }
        rail_decode(&mut self.rails[0], &mut self.xs, &mut self.ber);
        rail_decode(&mut self.rails[1], &mut self.ys, &mut self.ber);
        // Pair the decoded rail bits in step.
        let n = self.xs.len().min(self.ys.len());
        if n > 0 {
            self.cadus.clear();
            match self.locked {
                Some(h) => {
                    self.data.clear();
                    for i in 0..n {
                        self.hyps[h].0.step(self.xs[i], self.ys[i], &mut self.data);
                    }
                    let d = std::mem::take(&mut self.data);
                    self.hyps[h].1.process(&d, &mut self.cadus);
                    self.data = d;
                }
                None => {
                    for (k, (diff, fs)) in self.hyps.iter_mut().enumerate() {
                        self.data.clear();
                        for i in 0..n {
                            diff.step(self.xs[i], self.ys[i], &mut self.data);
                        }
                        let before = fs.stats().good;
                        self.trial.clear();
                        fs.process(&self.data, &mut self.trial);
                        if fs.stats().good > before && fs.stats().good >= 2 {
                            self.locked = Some(k);
                            self.cadus.append(&mut self.trial);
                            break;
                        }
                        if fs.stats().good > 0 && self.locked.is_none() {
                            self.cadus.extend(self.trial.drain(..).filter(|c| c.good()));
                        }
                    }
                }
            }
            if self.cadus.iter().any(|c| c.good()) {
                self.last_good_sym = self.symbols;
            }
            let out = io.items_out::<Cadu>(0);
            for c in self.cadus.drain(..) {
                out.push(Timed {
                    time: c.time,
                    value: c,
                });
            }
            self.xs.drain(..n);
            self.ys.drain(..n);
        }
        if io.tick() {
            let fstats: FrameStats = match self.locked {
                Some(h) => *self.hyps[h].1.stats(),
                None => self
                    .hyps
                    .iter()
                    .map(|h| *h.1.stats())
                    .max_by_key(|s| s.good)
                    .unwrap_or_default(),
            };
            let good_now = fstats.good - self.last_good.min(fstats.good);
            self.last_good = fstats.good;
            let r = RailReport {
                code_locked: self.rails.iter().all(|r| r.choice.is_some()),
                quality: [self.rails[0].quality, self.rails[1].quality],
                phases: [self.rails[0].choice, self.rails[1].choice],
                hypothesis: self.locked.map(|h| h as u8),
                metrics: [
                    self.rails[0].viterbi.mean_gain() as f64,
                    self.rails[1].viterbi.mean_gain() as f64,
                ],
                frames: fstats,
                ber: if self.ber.1 > 0 {
                    self.ber.0 as f64 / self.ber.1 as f64
                } else {
                    0.0
                },
                good_now,
                since_good_s: (self.symbols - self.last_good_sym) as f64 / self.rs.max(1.0),
            };
            io.items_out::<RailReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The symbol-rate measurement.

/// The rate meter's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RateReport {
    /// Measuring now.
    pub measuring: bool,
    /// The measured rate (the nearest candidate within 1 %).
    pub measured: Option<f64>,
    /// The raw line, Hz.
    pub line_hz: Option<f64>,
}
impl Item for RateReport {
    const KIND: &'static str = "satimg.rate_report";
}

/// The symbol-rate decision from the summed power spectrum of the instantaneous power (`acc`, `n` bins over the input rate `fs`).
///
/// The line at the symbol rate Rs folds into the first half of the spectrum when Rs is above fs/2 (the power is real, so its
/// spectrum is symmetric): each candidate's line is looked for where it lands, within 1 % of the candidate, and the candidate whose
/// line stands highest above the median floor wins. Returns (the line found, unfolded; the candidate or 0 when none stood out).
pub fn decide_rate(acc: &[f64], fs: f64, cands: &[f64]) -> (f64, f64) {
    let n = acc.len();
    let half = n / 2;
    let bin = fs / n as f64;
    let mut sorted: Vec<f64> = acc[8.min(half)..half].to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted
        .get(sorted.len() / 2)
        .copied()
        .unwrap_or(0.0)
        .max(1e-30);
    let mut best = (0.0, 0.0, 0.0f64);
    for &c in cands {
        if c > fs / 1.06 {
            continue;
        }
        let a = c.rem_euclid(fs);
        let (folded, mirrored) = if a > fs / 2.0 {
            (fs - a, true)
        } else {
            (a, false)
        };
        let w = (0.01 * c / bin).ceil() as isize + 1;
        let k0 = (folded / bin).round() as isize;
        let mut peak = (0usize, 0.0f64);
        for k in (k0 - w).max(8)..=(k0 + w).min(half as isize - 1) {
            let v = acc[k as usize];
            if v > peak.1 {
                peak = (k as usize, v);
            }
        }
        let ratio = peak.1 / floor;
        if ratio > best.2 {
            // Parabolic refinement, then back to the candidate's side of the fold.
            let k = peak.0;
            let mut f = k as f64;
            if k > 0 && k + 1 < n {
                f += sdrkit_algo_dsp::peak::parabolic(acc[k - 1], acc[k], acc[k + 1]).offset;
            }
            let f = f * bin;
            let line = (c - a) + if mirrored { fs - f } else { f };
            best = (line, c, ratio);
        }
    }
    if best.2 < 8.0 || (best.0 - best.1).abs() > 0.01 * best.1 {
        return (best.0, 0.0);
    }
    (best.0, best.1)
}

/// Measures the symbol rate from the line of the instantaneous power of the signal: 16384-point FFTs over 96 pieces. It measures
/// only when it is on and two or more candidates fit the input rate.
pub struct RateMeter {
    mpt: bool,
    on: bool,
    fit: usize,
    rate: f64,
    fft: Option<Fft>,
    buf: Vec<Complex32>,
    acc: Vec<f64>,
    blocks: usize,
    result: Option<(f64, f64)>,
}

impl RateMeter {
    /// For a product, measuring (`on`) or not.
    pub fn new(mpt: bool, on: bool) -> Self {
        RateMeter {
            mpt,
            on,
            fit: 0,
            rate: 0.0,
            fft: None,
            buf: Vec::new(),
            acc: Vec::new(),
            blocks: 0,
            result: None,
        }
    }
}

impl Component for RateMeter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.fy3_rate_meter",
            vec![Port::iq("in"), control_port()],
            vec![item_port::<RateReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.fit = candidates(self.mpt)
            .iter()
            .filter(|&&c| self.rate >= 1.1 * c)
            .count();
        self.result = None;
        self.blocks = 0;
        Ok(vec![item_format::<RateReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        for c in controls(io, 1) {
            if let Control::Measure(on) = c {
                self.on = *on;
                self.result = None;
                self.blocks = 0;
            }
        }
        let n = 16_384;
        let x = io.iq(0);
        let on = self.on && self.fit >= 2;
        if on && self.result.is_none() {
            let fft = self.fft.get_or_insert_with(|| Fft::forward(n));
            if self.acc.len() != n {
                self.acc = vec![0.0; n];
            }
            for &s in x {
                self.buf.push(Complex32::new(s.norm_sqr(), 0.0));
                if self.buf.len() == n {
                    let mean = self.buf.iter().map(|v| v.re).sum::<f32>() / n as f32;
                    for (k, v) in self.buf.iter_mut().enumerate() {
                        let w = sdrkit_algo_dsp::window::Window::Hann.value(k, n) as f32;
                        v.re = (v.re - mean) * w;
                    }
                    fft.process(&mut self.buf);
                    for (a, v) in self.acc.iter_mut().zip(&self.buf) {
                        *a += v.norm_sqr() as f64;
                    }
                    self.buf.clear();
                    self.blocks += 1;
                    if self.blocks == 96 {
                        self.result = Some(decide_rate(&self.acc, self.rate, candidates(self.mpt)));
                        break;
                    }
                }
            }
        }
        if io.tick() {
            let r = RateReport {
                measuring: on && self.result.is_none(),
                measured: self.result.and_then(|r| (r.1 > 0.0).then_some(r.1)),
                line_hz: self.result.map(|r| r.0),
            };
            io.items_out::<RateReport>(0).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The instruments component.

/// The instruments' report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fy3InstrumentReport {
    /// Per instrument.
    pub stats: BTreeMap<String, InstrumentStats>,
    /// The spacecraft heard.
    pub spacecraft: Option<u8>,
}
impl Item for Fy3InstrumentReport {
    const KIND: &'static str = "satimg.fy3_instrument_report";
}

/// VCDUs to strips, by instrument.
pub struct Fy3InstrumentComponent {
    dec: Fy3Instruments,
    strips: Vec<Strip>,
}

impl Fy3InstrumentComponent {
    /// The decoder of a product.
    pub fn new(mpt: bool) -> Self {
        Fy3InstrumentComponent {
            dec: Fy3Instruments::new(mpt),
            strips: Vec::new(),
        }
    }
}

impl Component for Fy3InstrumentComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.fy3_instruments",
            vec![Port::typed::<Vcdu>("in")],
            vec![
                item_port::<Strip>("out"),
                item_port::<Fy3InstrumentReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            item_format::<Strip>(),
            item_format::<Fy3InstrumentReport>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for v in io.items::<Vcdu>(0) {
            self.dec.push(&v.value, &mut self.strips);
        }
        io.items_out::<Strip>(0)
            .extend(self.strips.drain(..).map(|s| Timed {
                time: 0.0,
                value: s,
            }));
        if io.tick() {
            let r = Fy3InstrumentReport {
                stats: self.dec.stats.clone(),
                spacecraft: self.dec.spacecraft,
            };
            io.items_out::<Fy3InstrumentReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    instrument_owner!();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

sdrkit_core::choice_enum! {
    /// An FY-3 product.
    pub enum Product {
        /// AHRPT (L-band and X-band).
        Ahrpt = ("ahrpt", "AHRPT"),
        /// MPT (X-band).
        Mpt = ("mpt", "MPT"),
    }
}

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(LinkFields::SHAPE.fields)
            .with_fields(Fy3Fields::SHAPE.fields)
            .with_table(&FY3_INSTRUMENTS)
            .with_table(&IMAGES),
    )
}

/// The FY-3 display.
pub struct Fy3Display {
    mpt: bool,
    preset: usize,
    user_rs: f64,
    user_roll: f64,
    published: Published,
    hist: Histories,
    started: bool,
    running_rs: f64,
    tried: usize,
    last_change_s: f64,
    measured_done: bool,
    pending: bool,
    retune: bool,
    rate_noted_rs: f64,
    summary: String,
    hint: String,
    fitting: Vec<f64>,
}

impl Fy3Display {
    fn new(mpt: bool) -> Self {
        Fy3Display {
            mpt,
            preset: 0,
            user_rs: 0.0,
            user_roll: 0.0,
            published: super::display::published(display_declaration()),
            hist: Histories::default(),
            started: false,
            running_rs: 0.0,
            tried: 0,
            last_change_s: 0.0,
            measured_done: false,
            pending: false,
            retune: false,
            rate_noted_rs: 0.0,
            summary: String::new(),
            hint: String::new(),
            fitting: Vec::new(),
        }
    }
    fn preset(&self) -> Option<&'static Preset> {
        preset_at(self.mpt, self.preset)
    }
    fn auto(&self) -> bool {
        self.preset == 0 && self.user_rs == 0.0
    }
    fn link_for(&self, rs: f64) -> LinkParams {
        let roll = if self.user_roll > 0.0 {
            self.user_roll
        } else {
            roll_off_for(rs)
        };
        link(rs, roll)
    }
    /// Keeps the candidate symbol rates that the input rate holds in `self.fitting`.
    fn fit(&mut self, rate: f64) {
        self.fitting.clear();
        self.fitting.extend(
            candidates(self.mpt)
                .iter()
                .copied()
                .filter(|&c| rate >= 1.06 * c),
        );
    }
}

impl Component for Fy3Display {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.fy3_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<RailReport>("rails"),
                Port::typed::<PacketReport>("packets"),
                Port::typed::<Fy3InstrumentReport>("instruments"),
                Port::typed::<RateReport>("rate"),
            ],
            vec![
                item_port::<Control>("control"),
                item_port::<HostRequest>("requests"),
            ],
        )
        .with_settings(if self.mpt {
            MptDisplaySetting::schema()
        } else {
            AhrptDisplaySetting::schema()
        })
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.started = false;
        Ok(vec![item_format::<Control>(), item_format::<HostRequest>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let rl = last::<RailReport>(io, 2);
        let pk = last::<PacketReport>(io, 3);
        let ins = last::<Fy3InstrumentReport>(io, 4);
        let rate = last::<RateReport>(io, 5);
        let t = f.map_or(0.0, |f| f.time_s);
        let in_rate = f.map_or(0.0, |f| f.input_rate);
        let good_recent = rl.is_some_and(|r| r.frames.good > 0 && r.since_good_s < 2.0);
        if std::mem::take(&mut self.retune) {
            if let Some(p) = self.preset() {
                io.items_out::<HostRequest>(1).push(Timed {
                    time: t,
                    value: HostRequest::retune(p.2, "FY-3 preset"),
                });
            }
        }
        if !self.started || self.pending {
            self.started = true;
            self.pending = false;
            self.measured_done = false;
            self.tried = 0;
            self.last_change_s = t;
            self.fit(in_rate);
            let rs = if self.user_rs > 0.0 {
                self.user_rs * 1e6
            } else if let Some(p) = self.preset() {
                p.3
            } else {
                self.fitting
                    .first()
                    .copied()
                    .unwrap_or(candidates(self.mpt)[0])
            };
            self.running_rs = rs;
            let auto = self.auto();
            let least = least_rate(self.mpt, self.preset(), self.user_rs);
            let link = self.link_for(rs);
            let out = io.items_out::<Control>(0);
            for c in [
                Control::Link(link),
                Control::MinRate(least),
                Control::Measure(auto),
            ] {
                out.push(Timed { time: t, value: c });
            }
        }
        // Automatic: a measured rate, or the candidates in turn after failed searches.
        if self.auto() && !good_recent {
            if let Some(m) = rate.and_then(|r| r.measured) {
                if !self.measured_done {
                    self.measured_done = true;
                    if (m - self.running_rs).abs() > 0.01 * m {
                        self.running_rs = m;
                        self.last_change_s = t;
                        io.items_out::<Control>(0).push(Timed {
                            time: t,
                            value: Control::Link(self.link_for(m)),
                        });
                    }
                }
            } else if t - self.last_change_s > 8.0 && !rl.is_some_and(|r| r.code_locked) {
                self.fit(in_rate);
                if self.fitting.len() > 1 {
                    self.tried = (self.tried + 1) % self.fitting.len();
                    self.running_rs = self.fitting[self.tried];
                    self.last_change_s = t;
                    io.items_out::<Control>(0).push(Timed {
                        time: t,
                        value: Control::Link(self.link_for(self.running_rs)),
                    });
                }
            }
        }
        // The rate check: logged once for each running symbol rate whose whole signal the input rate does not hold.
        if in_rate > 0.0 && self.running_rs > 0.0 && self.rate_noted_rs != self.running_rs {
            if let Some(msg) = rate_message(
                self.running_rs,
                self.link_for(self.running_rs).roll_off,
                in_rate,
            ) {
                self.published.results_mut().log(msg);
            }
            self.rate_noted_rs = self.running_rs;
        }
        let locked = d.is_some_and(|d| d.state.locked && d.state.tracking);
        let code = rl.is_some_and(|r| r.code_locked);
        let fsync = rl.is_some_and(|r| r.frames.locked && r.frames.syncs > 0);
        let state = if good_recent {
            3
        } else if fsync && code {
            2
        } else if locked {
            1
        } else {
            0
        };
        self.publish(state, t, f, d, rl, pk, ins, rate);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        // The MPT display settings have the AHRPT order.
        match AhrptDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AhrptDisplaySetting::Preset => {
                let p = if self.mpt {
                    MPT_PRESET.read(id, v)?
                } else {
                    AHRPT_PRESET.read(id, v)?
                };
                if p != self.preset {
                    self.retune = true;
                }
                self.preset = p;
            }
            AhrptDisplaySetting::SymbolRate => self.user_rs = SYMBOL_RATE.read(id, v)?,
            AhrptDisplaySetting::RollOff => self.user_roll = ROLL_OFF.read(id, v)?,
        }
        self.pending = self.started;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let present = [
            (
                AhrptDisplaySetting::Preset,
                Value::Choice(self.preset as u16),
            ),
            (AhrptDisplaySetting::SymbolRate, Value::F64(self.user_rs)),
            (AhrptDisplaySetting::RollOff, Value::F64(self.user_roll)),
        ];
        if self.mpt {
            values_of(present.map(|(s, v)| {
                let m = match s {
                    AhrptDisplaySetting::Preset => MptDisplaySetting::Preset,
                    AhrptDisplaySetting::SymbolRate => MptDisplaySetting::SymbolRate,
                    AhrptDisplaySetting::RollOff => MptDisplaySetting::RollOff,
                };
                (m, v)
            }))
        } else {
            values_of(present)
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

impl Fy3Display {
    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        state: u8,
        t: f64,
        f: Option<&FrontReport>,
        d: Option<&DemodReport>,
        rl: Option<&RailReport>,
        pk: Option<&PacketReport>,
        ins: Option<&Fy3InstrumentReport>,
        rate: Option<&RateReport>,
    ) {
        let product = if self.mpt { "MPT" } else { "AHRPT" };
        let sat = ins.and_then(|i| i.spacecraft).and_then(satellite);
        let rows = ins.map_or(0, |i| i.stats.values().map(|s| s.rows).max().unwrap_or(0));
        let frames = rl.map_or(0, |r| r.frames.good);
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "{} {product}, {rows} rows, {frames} frames",
                sat.unwrap_or("FY-3")
            ),
            2 => write!(self.summary, "FY-3 {product}, frame sync, frames failing"),
            1 => write!(
                self.summary,
                "FY-3 {product}, carrier, {}",
                if rl.is_some_and(|r| r.code_locked) {
                    "code locked, no frame sync"
                } else {
                    "no code lock"
                }
            ),
            _ => write!(self.summary, "searching (FY-3 {product})"),
        };
        self.write_hint(state, t, f, d, rl, rate);
        let fr = rl.map(|rl| FrameReport {
            stats: rl.frames,
            good_now: rl.good_now,
            byte_error_pct: rl.ber * 100.0,
            ..Default::default()
        });
        let p = &mut self.published;
        p.put(&LinkFields {
            ber: rl.map(|r| r.ber),
            symbol_rate_hz: (self.running_rs > 0.0).then_some(Hertz(self.running_rs)),
            ..link_fields(t, f, d, None, fr.as_ref(), pk)
        });
        status(p.results_mut(), state, f, d, &self.summary);
        p.put(&Fy3Fields {
            product: if self.mpt {
                Product::Mpt
            } else {
                Product::Ahrpt
            },
            preset: preset_at(self.mpt, self.preset).map_or("auto", |p| p.0),
            satellite: sat,
            measured_symbol_rate_hz: rate.and_then(|r| r.measured).map(Hertz),
            rails_locked: rl.map(|r| r.code_locked),
            rail_i_quality: rl.map(|r| r.quality[0] as f64),
            rail_q_quality: rl.map(|r| r.quality[1] as f64),
            differential_hypothesis: rl.and_then(|r| r.hypothesis).map(|h| h as i64),
            picture_rows: rows as i64,
            hint: &self.hint,
        });
        if let Some(ins) = ins {
            for (name, s) in &ins.stats {
                FY3_INSTRUMENTS.put(
                    p,
                    name,
                    &Fy3InstrumentRow {
                        instrument: name,
                        frames: s.frames.max(s.vcdus) as i64,
                        rows: s.rows as i64,
                    },
                );
                let instrument = ALL_INSTRUMENTS.iter().find(|i| i.name() == name.as_str());
                if let (Some(i), true) = (instrument, s.rows > 0) {
                    IMAGES.put(
                        p,
                        name,
                        &ImageRow {
                            product: name,
                            channel: None,
                            time: None,
                            width: i64::from(width(*i)),
                            height: Some(s.rows as i64),
                        },
                    );
                }
            }
        }
        self.hist
            .publish(self.published.results_mut(), d, fr.as_ref());
    }

    #[allow(clippy::too_many_arguments)]
    fn write_hint(
        &mut self,
        state: u8,
        t: f64,
        f: Option<&FrontReport>,
        d: Option<&DemodReport>,
        rl: Option<&RailReport>,
        rate: Option<&RateReport>,
    ) {
        let rs = self.running_rs;
        let alpha = self.link_for(rs).roll_off;
        let h = &mut self.hint;
        h.clear();
        let text = if f.is_some_and(|f| f.input_rate < 1.06 * rs) {
            // A write to a String cannot fail.
            let _ = write!(
                h,
                "The sample rate is too low for this symbol rate: use {:.1} Msps or more.",
                rs * (1.0 + alpha) * 1.04 / 1e6
            );
            return;
        } else if rate.is_some_and(|r| r.measuring) {
            "Measuring the symbol rate."
        } else if d.is_some_and(|d| !d.state.tracking) && t > 2.0 {
            "Looking for the carrier."
        } else if state == 3 {
            ""
        } else if t <= 2.0 {
            "Looking for the signal."
        } else {
            let locked = d.is_some_and(|d| d.state.locked);
            let code = rl.is_some_and(|r| r.code_locked);
            let fsync = rl.is_some_and(|r| r.frames.syncs > 0);
            let (good, bad) = rl.map_or((0, 0), |r| (r.frames.good, r.frames.failed));
            if !locked {
                "No QPSK signal at this symbol rate yet. FY-3 passes are short (about 10 minutes): is the satellite above the horizon and the dish pointing at it? Check the LNA power and the product (AHRPT or MPT) and symbol rate."
            } else if !code {
                "The carrier is locked but the convolutional code is not found: the signal is too weak (needs about 3 dB Es/N0) or the symbol rate or roll-off is not right."
            } else if !fsync {
                "The code is locked but no frame sync word is found: wrong arrangement of the rails, or a very noisy signal."
            } else if bad > good {
                "Frames are found but Reed-Solomon fails: the signal is too weak. A bigger dish or a lower-noise LNA closer to the feed will help."
            } else {
                "Frames decode only now and then: the signal is marginal."
            }
        };
        h.push_str(text);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Graphs and descriptors.

/// Every FY-3 instrument.
const ALL_INSTRUMENTS: [Instrument; 8] = [
    Instrument::Virr,
    Instrument::Mersi,
    Instrument::Mersi2,
    Instrument::Mersi3,
    Instrument::MersiLl,
    Instrument::MersiRm,
    Instrument::Mwri,
    Instrument::Mwri2,
];

fn layouts(mpt: bool) -> Vec<PictureLayout> {
    let order: Vec<Instrument> = if mpt {
        vec![
            Instrument::Mersi,
            Instrument::Mersi2,
            Instrument::Mersi3,
            Instrument::MersiLl,
            Instrument::MersiRm,
            Instrument::Virr,
            Instrument::Mwri,
            Instrument::Mwri2,
        ]
    } else {
        vec![
            Instrument::Virr,
            Instrument::Mersi,
            Instrument::Mersi2,
            Instrument::Mersi3,
            Instrument::MersiLl,
            Instrument::MersiRm,
            Instrument::Mwri,
            Instrument::Mwri2,
        ]
    };
    order
        .into_iter()
        .map(|i| {
            let ch = channels(i);
            let mut views = Vec::new();
            match i {
                Instrument::Virr => {
                    views.push(NamedView::new(
                        "true",
                        "True colour (1, 9, 7)",
                        View::Rgb([(1, false), (9, false), (7, false)]),
                    ));
                    views.push(NamedView::new(
                        "false",
                        "False colour (2, 1, 4 inverted)",
                        View::Rgb([(2, false), (1, false), (4, true)]),
                    ));
                }
                Instrument::Mersi | Instrument::Mersi2 | Instrument::Mersi3 => {
                    views.push(NamedView::new(
                        "true",
                        "True colour (3, 2, 1)",
                        View::Rgb([(3, false), (2, false), (1, false)]),
                    ));
                }
                _ => {}
            }
            for (c, name) in &ch {
                views.push(NamedView::new(
                    format!("ch{c}"),
                    format!("Channel {name}"),
                    View::Grey {
                        channel: *c,
                        invert: thermal(i, *c),
                    },
                ));
            }
            PictureLayout {
                name: i.name(),
                width: width(i),
                max_rows: 4000,
                full_mask: 1,
                views,
            }
        })
        .collect()
}

/// The graph of a product that starts on link `start`. `measure` turns the symbol-rate measurement on.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts give no valid schema.
pub fn builder(
    mpt: bool,
    start: LinkParams,
    min_rate: f64,
    measure: bool,
) -> Result<GraphBuilder, GraphError> {
    let link = start;
    let pictures = Pictures::new(layouts(mpt)).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(link.clone(), min_rate, true).with_decimation_for(if mpt {
                18.7e6
            } else {
                38.4e6
            }),
        )
        .add("demod", DemodComponent::new(link))
        .add("rails", RailComponent::default())
        .add("rate", RateMeter::new(mpt, measure))
        .add("vcdu", VcduComponent::new(Layout::METEOR, false))
        .add("instruments", Fy3InstrumentComponent::new(mpt))
        .add("pictures", pictures)
        .add("display", Fy3Display::new(mpt))
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("frontend.out", "rate.in")
        .connect("demod.soft", "rails.in")
        .connect("rails.out", "vcdu.in")
        .connect("vcdu.vcdu", "instruments.in")
        .connect("instruments.out", "pictures.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("rails.report", "display.rails")
        .connect("vcdu.report", "display.packets")
        .connect("instruments.report", "display.instruments")
        .connect("rate.report", "display.rate")
        .feedback("display.control", "frontend.control")
        .feedback("display.control", "demod.control")
        .feedback("display.control", "rails.control")
        .feedback("display.control", "rate.control")
        .output("pictures.picture")
        .output("display.requests")
        .status_from("display")
        .clock(REPORT_S))
}

/// The receiver of a product: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(mpt: bool, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let id = AhrptSetting::Preset.id();
    let preset_index = cfg
        .settings
        .value(id)
        .and_then(|v| {
            if mpt {
                MPT_PRESET.read(id, v)
            } else {
                AHRPT_PRESET.read(id, v)
            }
            .ok()
        })
        .unwrap_or(0);
    let preset = preset_at(mpt, preset_index);
    let user_msym = cfg.settings.get(&SYMBOL_RATE);
    let least = least_rate(mpt, preset, user_msym);
    let auto = preset.is_none() && user_msym == 0.0;
    let rs = if user_msym > 0.0 {
        user_msym * 1e6
    } else {
        preset.map_or(candidates(mpt)[0], |p| p.3)
    };
    let roll = cfg.settings.get(&ROLL_OFF);
    let start = link(rs, if roll > 0.0 { roll } else { roll_off_for(rs) });
    let mut g = builder(mpt, start, least, auto)?
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(if mpt { MPT_ID } else { AHRPT_ID }), g)
        .min_rate(least, "")
        .route("preset", &["display.preset"])?
        .route("symbol_rate", &["display.symbol_rate"])?
        .route("roll_off", &["display.roll_off"])?
        .route("instrument", &["pictures.instrument"])?
        .route("view", &["pictures.view"])?
        .route("follow", &["pictures.follow"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// The rate check: `None` when the input rate `fs` holds the whole signal of `rs` with roll-off `alpha`, else the
/// message naming the least rate the chain runs at (1.06 Rs) and the rate of the whole signal (Rs (1 + alpha) x 1.04).
pub fn rate_message(rs: f64, alpha: f64, fs: f64) -> Option<String> {
    let whole = rs * (1.0 + alpha) * 1.04;
    (fs < whole).then(|| {
        format!(
            "the sample rate is too low for {} Msym/s: {:.2} Msps at the least, {:.2} Msps for the whole signal",
            trim_rate(rs / 1e6),
            1.06 * rs / 1e6,
            whole / 1e6
        )
    })
}

fn trim_rate(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The least input rate the chain runs at (1.06 Rs): of the user's symbol rate (Msym/s), else the preset's, else the slowest
/// candidate (automatic).
pub fn least_rate(mpt: bool, preset: Option<&Preset>, user_msym: f64) -> f64 {
    let rs = if user_msym > 0.0 {
        user_msym * 1e6
    } else if let Some(p) = preset {
        p.3
    } else {
        candidates(mpt).iter().copied().fold(f64::MAX, f64::min)
    };
    1.06 * rs
}

/// The stack id of FY-3 AHRPT.
pub const AHRPT_ID: &str = "fy3_ahrpt";
/// The stack id of FY-3 MPT.
pub const MPT_ID: &str = "fy3_mpt";

fn make_receiver(mpt: bool, cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(mpt, cfg))
}
fn ahrpt_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make_receiver(false, cfg)
}
fn mpt_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make_receiver(true, cfg)
}
fn make_generator(cfg: &GenConfig, mpt: bool) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        Fy3Generator::from_config(cfg, mpt).map_err(super::gen_error)?,
    ))
}
fn ahrpt_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    make_generator(cfg, false)
}
fn mpt_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    make_generator(cfg, true)
}

/// FY-3 AHRPT.
pub static AHRPT: StackDescriptor = StackDescriptor {
    id: AHRPT_ID,
    name: "FY-3 AHRPT",
    group: crate::GROUP,
    blurb: "Fengyun-3 AHRPT: VIRR, MERSI and MWRI pictures from the polar FY-3 satellites (L-band, and X-band with a down-converter).",
    tuning: Tuning { min_hz: 1690e6, max_hz: 1710e6, ..Tuning::anywhere(1701.4e6, 6e6, 4e6, 4.5e6) },
    submodes: &[],
    settings: AhrptSetting::schema,
    make_receiver: ahrpt_receiver,
    make_generator: Some(ahrpt_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hrpt],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::FY3_AHRPT_HOOKS),
    benchmarks: None,
};

/// FY-3 MPT.
pub static MPT: StackDescriptor = StackDescriptor {
    id: MPT_ID,
    name: "FY-3 MPT",
    group: crate::GROUP,
    blurb: "Fengyun-3 MPT (X-band, 18.7 Msym/s): MERSI pictures. It needs a dish of 1.2 m or more and a down-converter.",
    tuning: Tuning { min_hz: 7700e6, max_hz: 7850e6, ..Tuning::anywhere(7750e6, 20e6, 20e6, 28e6) },
    submodes: &[],
    settings: MptSetting::schema,
    make_receiver: mpt_receiver,
    make_generator: Some(mpt_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::FY3_MPT_HOOKS),
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The FY-3 instrument a test signal carries for a satellite and product.
pub fn test_instrument(sc: u8, mpt: bool) -> (Instrument, u8) {
    match (satellite(sc), mpt) {
        (Some("FY-3D"), _) => (Instrument::Mwri, 10),
        (Some("FY-3A" | "FY-3B" | "FY-3C"), false) => (Instrument::Virr, 5),
        (Some("FY-3F" | "FY-3H"), _) => (Instrument::Mersi3, 3),
        (Some("FY-3E"), _) => (Instrument::MersiLl, 3),
        (Some("FY-3G"), _) => (Instrument::MersiRm, 35),
        _ => (Instrument::Mersi, 3),
    }
}

/// A test pixel of instrument channel `c`, row `y`, column `x` (16 bits wide, the instrument keeps what it can).
pub fn test_value(c: usize, y: usize, x: usize, w: usize) -> u16 {
    (test_pixel(c as u16, x as u32, y as u32, w as u32) as u16) << 8
}

/// Channel bits of the FY-3 test signal: instrument frames on their VC, half the VCDUs fill, a differential coder and one code
/// per rail.
pub struct Fy3Source {
    sc: u8,
    vc: u8,
    instrument: Instrument,
    stream: std::collections::VecDeque<u8>,
    counter: u32,
    fill_counter: u32,
    k: u64,
    row: usize,
    enc: [Encoder; 2],
    px: u8,
    py: u8,
    q_neg: bool,
    swap: bool,
    fill_every: u64,
}

impl Fy3Source {
    fn refill(&mut self) {
        match self.instrument {
            Instrument::Virr => {
                let mut px = vec![0u16; 10 * 2048];
                for c in 0..10 {
                    for x in 0..2048 {
                        px[c * 2048 + x] = test_value(c + 1, self.row, x, 2048) >> 6;
                    }
                }
                let ms = 36_000_000 + (self.row as f64 * 1000.0 / 6.0).round() as u32;
                self.stream.extend(build_virr(24_000, ms, &px));
            }
            Instrument::Mwri => {
                let mut px = vec![0u16; 10 * 266];
                for c in 0..10 {
                    for x in 0..266 {
                        px[c * 266 + x] = test_value(c + 1, self.row, x, 266);
                    }
                }
                self.stream.extend(build_mwri(self.row as u32 * 1700, &px));
            }
            Instrument::Mwri2 => {
                let mut px = vec![0u16; 26 * 492];
                for c in 0..26 {
                    for x in 0..492 {
                        px[c * 492 + x] = test_value(c + 1, self.row, x, 492);
                    }
                }
                self.stream.extend(crate::instruments::fy3::build_mwri2(
                    self.row as u32 * 1800,
                    &px,
                ));
            }
            i => {
                let l = MersiLayout::of(i).expect("a MERSI");
                let row = self.row;
                let w1 = l.width1k();
                let ch250 = l.ch250;
                let frames = build_mersi_scan(&l, row as u32 * 1500, |c, line, x| {
                    if c <= ch250 {
                        // A 250 m pixel: the 1 km value repeated over its 4 x 4 block, 12 bits.
                        test_value(c, row * 10 + line / 4, x / 4, w1) >> 4
                    } else {
                        test_value(c, row * 10 + line, x, w1) >> 4
                    }
                });
                for f in frames {
                    self.stream.extend(f);
                }
            }
        }
        self.row += 1;
    }
}

impl BitSource for Fy3Source {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let layout = Layout::METEOR;
        let zl = layout.data_zone_len();
        self.k += 1;
        let v = if self.fill_every > 0 && self.k.is_multiple_of(self.fill_every) {
            let c = self.fill_counter;
            self.fill_counter = (c + 1) & 0xFF_FFFF;
            aos_frame(&layout, self.sc, 63, c, FirstHeader::None, &[])
        } else {
            while self.stream.len() < zl {
                self.refill();
            }
            let zone: Vec<u8> = self.stream.drain(..zl).collect();
            let c = self.counter;
            self.counter = (c + 1) & 0xFF_FFFF;
            aos_frame(&layout, self.sc, self.vc, c, FirstHeader::None, &zone)
        };
        let cadu = cadu_bytes(&v, Basis::Dual, true);
        for &b in &cadu {
            let bits: Vec<u8> = (0..8).rev().map(|k| (b >> k) & 1).collect();
            for pair in bits.chunks(2) {
                let (x, y) = diff_encode(pair[0], pair[1], self.px, self.py);
                self.px = x;
                self.py = y;
                let la = self.enc[0].push_bit(x);
                let lb = self.enc[1].push_bit(y);
                let (a1, a2) = (la & 1, (la >> 1) & 1);
                let (b1, b2) = (lb & 1, (lb >> 1) & 1);
                // Symbol 2k carries (a1 on I, b1 on Q), symbol 2k + 1 (a2, b2).
                for (i, q) in [(a1, b1), (a2, b2)] {
                    let (mut i, mut q) = (i, q);
                    if self.swap {
                        std::mem::swap(&mut i, &mut q);
                    }
                    if self.q_neg {
                        q ^= 1;
                    }
                    out.push(i);
                    out.push(q);
                }
            }
        }
    }
}

/// The FY-3 test signal: the clean signal at unit mean power.
pub struct Fy3Generator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<Fy3Source>,
}

impl Fy3Generator {
    /// From a configuration: `gen_satellite` (the spacecraft id, 51 is FY-3C), `gen_symbol_rate` (Msym/s), `gen_qneg`,
    /// `gen_swap` and `gen_fill_every` (2: half the frames are fill; 0: the instrument's own share). The schema of the
    /// settings must be the product's.
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the options.
    pub fn from_config(cfg: &GenConfig, mpt: bool) -> Result<Self, GenError> {
        let v = &cfg.settings;
        let sc = v.get(&GEN_SATELLITE).clamp(0, 255) as u8;
        let rs = if mpt {
            v.get(&GEN_SYMBOL_RATE_MPT)
        } else {
            v.get(&GEN_SYMBOL_RATE_AHRPT)
        } * 1e6;
        let modem = ModParams::new(cfg.rate_hz, rs, 0.5, Modulation::Qpsk);
        let (instrument, vc) = test_instrument(sc, mpt);
        let own = if matches!(instrument, Instrument::Virr | Instrument::Mwri) {
            2
        } else {
            8
        };
        let fill_every = match v.get(&GEN_FILL_EVERY).clamp(0, 1000) {
            0 => own,
            n => n as u64,
        };
        let src = Fy3Source {
            sc,
            vc,
            instrument,
            stream: Default::default(),
            counter: 0,
            fill_counter: 0,
            k: 0,
            row: 0,
            enc: [Encoder::new(codes::CCSDS_K7), Encoder::new(codes::CCSDS_K7)],
            px: 0,
            py: 0,
            q_neg: v.get(&GEN_QNEG),
            swap: v.get(&GEN_SWAP),
            fill_every,
        };
        Ok(Fy3Generator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for Fy3Generator {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_check_names_both_rates() {
        // 2.6 Msym/s at roll-off 0.5: the whole signal needs 2.6 x 1.5 x 1.04 = 4.056 Msps; the chain runs from 2.756 Msps.
        assert_eq!(rate_message(2.6e6, 0.5, 4.1e6), None);
        assert_eq!(
            rate_message(2.6e6, 0.5, 3.0e6).as_deref(),
            Some("the sample rate is too low for 2.6 Msym/s: 2.76 Msps at the least, 4.06 Msps for the whole signal")
        );
        assert!(rate_message(38.4e6, 0.35, 40e6)
            .unwrap()
            .starts_with("the sample rate is too low for 38.4 Msym/s"));
    }

    #[test]
    fn the_differential_coder_round_trips() {
        let mut d = Diff::default();
        let (mut px, mut py) = (0u8, 0u8);
        let data: Vec<u8> = (0..400).map(|i| ((i * 7 + i / 3) % 2) as u8).collect();
        let mut out = Vec::new();
        for p in data.chunks(2) {
            let (x, y) = diff_encode(p[0], p[1], px, py);
            px = x;
            py = y;
            d.step(x, y, &mut out);
        }
        assert_eq!(&out[2..], &data[2..]);
    }

    #[test]
    fn a_valid_code_stream_has_no_syndrome() {
        let mut e = Encoder::new(codes::K7_171_133);
        let (mut r1, mut r2) = (0u8, 0u8);
        let mut errs = 0;
        for k in 0..500u32 {
            let l = e.push_bit((k * 13 % 7 % 2) as u8);
            let (a, b) = (l & 1, (l >> 1) & 1);
            r1 = ((r1 >> 1) | (a << 6)) & 0x7F;
            r2 = ((r2 >> 1) | (b << 6)) & 0x7F;
            if k > 8 {
                errs += syndrome(r1, r2);
            }
        }
        assert_eq!(errs, 0);
    }
}

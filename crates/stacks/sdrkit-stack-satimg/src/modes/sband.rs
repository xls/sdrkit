//! S-band CCSDS (`sband_ccsds`): a CCSDS-style telemetry link of a science satellite, launch vehicle or crew craft on S or L
//! band, set by plain link settings (modulation, symbol rate, code, frame, randomiser, Reed-Solomon). The stack lists the
//! frames with the spacecraft and virtual channel of their headers; it does not decode their content. A host that keeps a
//! table of links copies one row into the settings.
//!
//! Standard: CCSDS 131.0-B-5 (K = 7 code, sync marker, randomiser, Reed-Solomon, interleave, virtual fill); CCSDS 132.0-B-3
//! and 732.0-B-4 (frame headers).

use super::display::{log_lines, published, status, with_series, Histories};
use super::tables::{frame_table_fields, publish_tables, test_data};
use super::xrit::RsBasis;
use crate::typed::{
    FrameTableFields, SbandCcsdsFields, FRAMES, VIRTUAL_CHANNELS,
};
use crate::components::fsk::FskDemodComponent;
use crate::components::link::{
    item_format, item_port, DemodComponent, DemodReport, FrontEndComponent, FrontReport,
};
use crate::components::pm::PmDemodComponent;
use crate::components::table::{FrameTableComponent, Line, TableCommand, TableLink, TableReport};
use crate::components::{last, Control};
use crate::gen::{BitSource, CcsdsCoder, CodeOptions, GenError, ModParams, Modulator};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_fec::rs::{codes, Basis};
use sdrkit_algo_sim::modulate::{Cpm, PhaseMod};
use sdrkit_core::command::{CommandError, CommandId, CommandSet as _, Commands};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Record, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, StageError, Tuning, Value, Values,
};
use sdrkit_graph::{
    Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io, Port,
    StackGraph, Timed,
};
use sdrkit_proto_ccsds::channel::{BasisPolicy, ChannelCoder, ChannelConfig};
use sdrkit_proto_ccsds::codesync::CodeConfig;
use sdrkit_proto_ccsds::quaddiff::QuadDiffEncoder;
use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
use sdrkit_util_bits::xorshift::Xorshift64;
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "sband_ccsds";
/// The least input rate of any link.
pub const MIN_RATE: f64 = 500e3;
/// The phase index of the PM test signal, radians (about 60 degrees).
pub const PM_INDEX_RAD: f64 = 1.05;

sdrkit_core::choice_enum! {
    /// The modulation of a link.
    pub enum SbandModulation {
        /// Binary PSK.
        Bpsk = ("bpsk", "BPSK"),
        /// QPSK, I and Q at the same instant.
        Qpsk = ("qpsk", "QPSK"),
        /// Offset QPSK: Q half a symbol after I.
        Oqpsk = ("oqpsk", "OQPSK"),
        /// Phase modulation with a residual carrier.
        Pm = ("pm", "PM (residual carrier)"),
        /// Two-level FSK.
        Fsk = ("fsk", "FSK"),
    }
}

sdrkit_core::choice_enum! {
    /// Where the CCSDS randomiser sits.
    pub enum Randomiser {
        /// No randomiser.
        Off = ("none", "None"),
        /// Over every byte after the sync word: removed first.
        Frame = ("frame", "On the coded frame"),
        /// Over the data before the check bytes are made: removed after the Reed-Solomon code.
        Inside = ("inside", "Inside the Reed-Solomon code"),
    }
}

sdrkit_core::choice_enum! {
    /// The Reed-Solomon code.
    pub enum RsChoice {
        /// No Reed-Solomon code.
        Off = ("none", "None"),
        /// (255,223): 32 check bytes a word.
        Rs223 = ("255_223", "(255,223)"),
        /// (255,239): 16 check bytes a word.
        Rs239 = ("255_239", "(255,239)"),
    }
}

const LINK_SECTION: &str = "Link";

/// The name of the link in the summary.
pub const LINK_NAME: Setting<field::Text> =
    Setting::new("link_name", "Link name", "", "S-band link").help("The name in the summary.");
/// The modulation.
pub const MODULATION: Setting<field::Choice<SbandModulation>> =
    Setting::choice("modulation", "Modulation", SbandModulation::Bpsk)
        .restart()
        .section(LINK_SECTION);
/// Symbols a second.
pub const SYMBOL_RATE: Setting<field::Hz> =
    Setting::new("symbol_rate", "Symbol rate", "Hz", 1_000_000.0)
        .range(1000.0, 20_000_000.0, 1.0)
        .restart()
        .section(LINK_SECTION);
/// The roll-off of the root raised cosine matched filter.
pub const ROLL_OFF: Setting<field::F64> =
    Setting::new("roll_off", "Matched filter roll-off", "", 0.5)
        .range(0.1, 1.0, 0.01)
        .restart()
        .section(LINK_SECTION);
/// The K = 7 code before the sync word.
pub const CONV: Setting<field::Bool> = Setting::new("conv", "K = 7 code", "", false)
    .help(
        "The rate 1/2 code of CCSDS, second output inverted, over the frames and their sync words.",
    )
    .restart()
    .section(LINK_SECTION);
/// Bits of a frame with its sync word.
pub const FRAME_BITS: Setting<field::I64> =
    Setting::new("frame_bits", "Frame length", "bits", 10_232)
        .range(64.0, 262_144.0, 1.0)
        .help("The frame with its sync word.")
        .restart()
        .section(LINK_SECTION);
/// The 32-bit sync word in hexadecimal.
pub const SYNC_WORD: Setting<field::Text> = Setting::new("sync_word", "Sync word", "", "1ACFFC1D")
    .help("32 bits, 8 hexadecimal digits.")
    .restart()
    .section(LINK_SECTION);
/// NRZ-M.
pub const NRZM: Setting<field::Bool> = Setting::new("nrzm", "NRZ-M", "", false)
    .help("On a coded link before the code; on uncoded QPSK the four-phase differential coding.")
    .restart()
    .section(LINK_SECTION);
/// Where the randomiser sits.
pub const RANDOMISER: Setting<field::Choice<Randomiser>> =
    Setting::choice("randomiser", "CCSDS randomiser", Randomiser::Frame)
        .restart()
        .section(LINK_SECTION);
/// The Reed-Solomon code.
pub const RS: Setting<field::Choice<RsChoice>> =
    Setting::choice("rs", "Reed-Solomon", RsChoice::Rs223)
        .restart()
        .section(LINK_SECTION);
/// The interleave depth of the Reed-Solomon code.
pub const RS_INTERLEAVE: Setting<field::I64> = Setting::new("rs_interleave", "Interleave", "", 5)
    .range(1.0, 8.0, 1.0)
    .restart()
    .section(LINK_SECTION);
/// The basis of the Reed-Solomon symbols.
pub const RS_BASIS: Setting<field::Choice<RsBasis>> =
    Setting::choice("rs_basis", "Basis", RsBasis::Dual)
        .restart()
        .section(LINK_SECTION);

sdrkit_core::settings! {
    /// The settings of `sband_ccsds`: the name and the parameters of the link.
    pub enum SbandSetting {
        /// The name in the summary.
        LinkName = LINK_NAME,
        /// The modulation.
        Modulation = MODULATION,
        /// Symbols a second.
        SymbolRate = SYMBOL_RATE,
        /// The matched filter roll-off.
        RollOff = ROLL_OFF,
        /// The K = 7 code.
        Conv = CONV,
        /// Bits of a frame.
        FrameBits = FRAME_BITS,
        /// The sync word.
        SyncWord = SYNC_WORD,
        /// NRZ-M.
        Nrzm = NRZM,
        /// The randomiser.
        Randomiser = RANDOMISER,
        /// The Reed-Solomon code.
        Rs = RS,
        /// The interleave depth.
        RsInterleave = RS_INTERLEAVE,
        /// The basis.
        RsBasis = RS_BASIS,
    }
}

/// Why link settings make no link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkError {
    /// The sync word is not 8 hexadecimal digits.
    SyncWord(String),
    /// The frame is too short for the sync word and one byte, or for one Reed-Solomon code word of each interleave.
    FrameTooShort {
        /// Bits of the frame.
        frame_bits: u32,
    },
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::SyncWord(w) => write!(f, "the sync word {w:?} is not 8 hexadecimal digits"),
            LinkError::FrameTooShort { frame_bits } => write!(
                f,
                "a frame of {frame_bits} bits is too short for the sync word and the Reed-Solomon code words"
            ),
        }
    }
}

impl std::error::Error for LinkError {}

/// The sync word of its hexadecimal text: exactly 8 digits.
pub fn parse_sync_word(text: &str) -> Option<u32> {
    let t = text.trim();
    (t.len() == 8 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(t, 16).ok())
        .flatten()
}

/// A link as its settings give it.
#[derive(Clone, Debug, PartialEq)]
pub struct SbandLink {
    /// The modulation.
    pub modulation: SbandModulation,
    /// Symbols a second.
    pub symbol_rate: f64,
    /// The matched filter roll-off.
    pub roll_off: f64,
    /// The K = 7 code is on.
    pub conv: bool,
    /// Bits of a frame with its sync word.
    pub frame_bits: u32,
    /// The sync word.
    pub sync_word: u32,
    /// NRZ-M.
    pub nrzm: bool,
    /// Where the randomiser sits.
    pub randomiser: Randomiser,
    /// The Reed-Solomon code.
    pub rs: RsChoice,
    /// The interleave depth, 1 to 8.
    pub interleave: usize,
    /// The basis of the Reed-Solomon symbols.
    pub basis: RsBasis,
}

impl SbandLink {
    /// The link of the settings `v` (of [`SbandSetting`]).
    ///
    /// # Errors
    ///
    /// [`LinkError`] for a sync word that does not parse or a frame too short for its code words.
    pub fn from_values(v: &Values) -> Result<Self, LinkError> {
        let word = v.get(&SYNC_WORD);
        let link = SbandLink {
            modulation: v.get(&MODULATION),
            symbol_rate: v.get(&SYMBOL_RATE),
            roll_off: v.get(&ROLL_OFF),
            conv: v.get(&CONV),
            frame_bits: v.get(&FRAME_BITS).clamp(64, 262_144) as u32,
            sync_word: parse_sync_word(word)
                .ok_or_else(|| LinkError::SyncWord(word.to_string()))?,
            nrzm: v.get(&NRZM),
            randomiser: v.get(&RANDOMISER),
            rs: v.get(&RS),
            interleave: v.get(&RS_INTERLEAVE).clamp(1, 8) as usize,
            basis: v.get(&RS_BASIS),
        };
        link.check()?;
        Ok(link)
    }

    fn check(&self) -> Result<(), LinkError> {
        let short = LinkError::FrameTooShort {
            frame_bits: self.frame_bits,
        };
        if self.frame_bytes() < 5 {
            return Err(short);
        }
        if self.rs != RsChoice::Off && self.word_len() <= self.check_bytes() {
            return Err(short);
        }
        Ok(())
    }

    /// Whole bytes of a frame with its sync word.
    pub fn frame_bytes(&self) -> usize {
        self.frame_bits as usize / 8
    }

    /// The length n of a Reed-Solomon code word: (frame bytes - 4) / interleave, at most 255.
    pub fn word_len(&self) -> usize {
        ((self.frame_bytes() - 4) / self.interleave).min(255)
    }

    /// Zero bytes in front of each code word: 255 - n.
    pub fn virtual_fill(&self) -> usize {
        255 - self.word_len()
    }

    /// Check bytes of a code word: 32, 16, or 0 without Reed-Solomon.
    pub fn check_bytes(&self) -> usize {
        match self.rs {
            RsChoice::Off => 0,
            RsChoice::Rs223 => 32,
            RsChoice::Rs239 => 16,
        }
    }

    /// Data bytes of a frame after decoding.
    pub fn data_bytes(&self) -> usize {
        match self.rs {
            RsChoice::Off => self.frame_bytes() - 4,
            _ => self.interleave * (self.word_len() - self.check_bytes()),
        }
    }

    /// The channel coding of the bytes after the sync word.
    pub fn channel(&self) -> ChannelConfig {
        let basis = match self.basis {
            RsBasis::Dual => Basis::Dual,
            RsBasis::Conventional => Basis::Conventional,
        };
        let c = match self.rs {
            RsChoice::Off => ChannelConfig::uncoded(self.frame_bytes() - 4),
            RsChoice::Rs223 => ChannelConfig::rs_223(self.interleave),
            RsChoice::Rs239 => ChannelConfig {
                rs: Some(codes::CCSDS_255_239),
                ..ChannelConfig::rs_223(self.interleave)
            },
        };
        let c = match self.rs {
            RsChoice::Off => c,
            _ => c.with_virtual_fill(self.virtual_fill()),
        }
        .with_basis(BasisPolicy::Fixed(basis));
        if self.randomiser == Randomiser::Frame {
            c
        } else {
            c.without_randomiser()
        }
    }

    /// Code symbols (soft values) of one channel symbol: 2 for QPSK and OQPSK, else 1.
    pub fn bits_per_symbol(&self) -> usize {
        match self.modulation {
            SbandModulation::Qpsk | SbandModulation::Oqpsk => 2,
            _ => 1,
        }
    }

    /// Seconds from one frame to the next.
    pub fn frame_period_s(&self) -> f64 {
        let code = if self.conv { 2.0 } else { 1.0 };
        f64::from(self.frame_bits) * code / (self.symbol_rate * self.bits_per_symbol() as f64)
    }

    /// The least input rate: 2.8 samples a symbol, and 500 ksps.
    pub fn min_rate(&self) -> f64 {
        (2.8 * self.symbol_rate).max(MIN_RATE)
    }

    /// The input rate the link asks: 4 samples a symbol, and 1 Msps.
    pub fn rate(&self) -> f64 {
        (4.0 * self.symbol_rate).max(1e6)
    }

    /// The front end's and the PSK demodulator's link: at least 4 samples a symbol, a carrier search on the line of the
    /// signal raised to the power 2 (BPSK, PM, FSK) or 4 (QPSK, OQPSK) that stands 6 times over the mean.
    pub fn psk(&self) -> LinkParams {
        let (modulation, power) = match self.modulation {
            SbandModulation::Qpsk => (Modulation::Qpsk, 4),
            SbandModulation::Oqpsk => (Modulation::Oqpsk, 4),
            _ => (Modulation::Bpsk, 2),
        };
        LinkParams {
            modulation,
            symbol_rate: self.symbol_rate,
            roll_off: self.roll_off,
            class: LinkClass::Leo,
            search: Search::PowerLine {
                power,
                min_ratio: 6.0,
            },
            snr: SnrEstimator::Decisions,
            bpsk_plus_is_zero: false,
            constellation: 256,
            interleaved: false,
        }
    }

    /// The frame rules: the sync word with 3 wrong bits to acquire and 9 to track, either polarity; the line of the link;
    /// the randomiser and the Reed-Solomon code.
    pub fn table(&self) -> TableLink {
        let quad = self.bits_per_symbol() == 2;
        let line = if self.conv {
            Line::Coded(CodeConfig {
                frame_bits: self.frame_bits,
                search_errors: 7,
                track_errors: 16,
                max_misses: 4,
                nrzm: self.nrzm,
            })
        } else {
            match (quad, self.nrzm) {
                (true, true) => Line::QuadDiff,
                (true, false) => Line::Quad,
                (false, true) => Line::NrzM,
                (false, false) => Line::Nrz,
            }
        };
        TableLink {
            sync: WordSyncConfig {
                word: u64::from(self.sync_word),
                word_bits: 32,
                frame_bits: self.frame_bits as usize,
                acquire_errors: 3,
                track_errors: 9,
                complement: true,
                max_misses: 3,
            },
            line,
            code: Some(self.channel()),
            invert: false,
            retry_plain: false,
            data_randomised: self.randomiser == Randomiser::Inside,
            llr_rate: self.symbol_rate * self.bits_per_symbol() as f64,
        }
    }
}

const NOTE_TEXT: &str = "Experimental: not confirmed on a real signal. No public document gives the link parameters. Not \
    confirmed by a public document: every link parameter of a satellite (modulation, symbol rate, roll-off, code, frame \
    length, sync word, NRZ-M, randomiser, Reed-Solomon code, interleave and basis), which links are still on the air, and \
    the four-phase differential rule of uncoded QPSK with NRZ-M. Frame contents are not decoded.";

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

/// The declaration of the display: the fields of the frame table and of the stack, the two tables and the series.
fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(FrameTableFields::SHAPE.fields)
            .with_fields(SbandCcsdsFields::SHAPE.fields)
            .with_table(&FRAMES)
            .with_table(&VIRTUAL_CHANNELS),
    )
}

/// The display: states, fields, the tables, summary, hint, histories and, for a PM link, the new carrier search. It holds the
/// settings of the stack; a change of a link setting takes effect when the stack restarts.
pub struct SbandDisplay {
    link: SbandLink,
    values: Values,
    published: Published,
    hist: Histories,
    searches: u32,
    search_end_s: f64,
    row: u64,
    summary: String,
    hint: String,
    key: String,
}

impl SbandDisplay {
    /// A display for `link`.
    pub fn new(link: SbandLink) -> Self {
        SbandDisplay {
            link,
            values: Values::defaults(&SbandSetting::schema()),
            published: published(display_declaration()),
            hist: Histories::default(),
            searches: 0,
            search_end_s: 0.0,
            row: 0,
            summary: String::new(),
            hint: String::new(),
            key: String::new(),
        }
    }
}

impl Component for SbandDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.sband_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<TableReport>("table"),
            ],
            vec![item_port::<Control>("control")],
        )
        .with_settings(SbandSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.searches = 0;
        self.search_end_s = 0.0;
        Ok(vec![item_format::<Control>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let none = TableReport::default();
        let r = last::<TableReport>(io, 2).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        let period = self.link.frame_period_s();
        log_lines(self.published.results_mut(), &r.log);
        let low = f.is_some_and(|f| f.rate_too_low);
        let hold = (3.0 * period).max(3.0);
        let carrier = d.is_some_and(|d| d.state.locked && d.state.tracking);
        let state = if low {
            0
        } else if r.stats.good > 0 && r.since_good_s < hold && r.locked {
            3
        } else if r.locked {
            2
        } else if carrier {
            1
        } else {
            0
        };
        if let (SbandModulation::Pm, Some(d)) = (self.link.modulation, d) {
            // Without a frame lock the carrier is searched again, after the longer of 2.5 s and 4 frames since the last
            // search ended and since the last good frame.
            if d.state.searches != self.searches {
                self.searches = d.state.searches;
                self.search_end_s = t;
            }
            let wait = (4.0 * period).max(2.5);
            if d.state.tracking
                && !r.locked
                && t - self.search_end_s >= wait
                && (r.stats.good == 0 || r.since_good_s >= wait)
            {
                self.search_end_s = t;
                io.items_out::<Control>(0).push(Timed {
                    time: t,
                    value: Control::RestartCarrier,
                });
            }
        }
        let name = self.values.get(&LINK_NAME);
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(self.summary, "{name}, {} frames", r.stats.good),
            2 => write!(self.summary, "{name}, frame sync, frames failing"),
            1 => write!(self.summary, "{name}, symbols, no frame sync"),
            _ => write!(self.summary, "{name}, searching"),
        };
        self.hint.clear();
        if low {
            // A write to a String cannot fail.
            let _ = write!(
                self.hint,
                "The sample rate ({:.2} Msps) is too low: this link needs at least {:.2} Msps. Raise it in the radio settings.",
                f.map_or(0.0, |f| f.input_rate) / 1e6,
                self.link.min_rate() / 1e6
            );
        }
        let p = &mut self.published;
        status(p.results_mut(), state, f, d, &self.summary);
        p.put(&FrameTableFields {
            symbol_rate_hz: d.map(|d| Hertz(d.state.symbol_rate)),
            ..frame_table_fields(t, f, d, r, NOTE_TEXT)
        });
        p.put(&SbandCcsdsFields {
            link: name,
            carrier_locked: d.map(|_| carrier || state == 3),
            code_locked: r.code.map(|c| c.locked),
            phase_ambiguity: r
                .code
                .filter(|c| c.locked)
                .map(|c| i64::from(c.ambiguity())),
            hint: &self.hint,
        });
        publish_tables(p, r, &mut self.row, &mut self.key);
        self.hist.publish(self.published.results_mut(), d, None);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = SbandSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        if s == SbandSetting::SyncWord && parse_sync_word(SYNC_WORD.read(id, v)?).is_none() {
            return Err(SettingError::Malformed { id, at: None });
        }
        self.values.set(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        TableCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match TableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TableCommand::Clear => {
                self.published.clear_rows(&FRAMES);
                self.published.clear_rows(&VIRTUAL_CHANNELS);
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The graph of `link`.
pub fn builder(link: &SbandLink) -> GraphBuilder {
    let psk = link.psk();
    let g = GraphBuilder::new().add(
        "frontend",
        FrontEndComponent::new(psk.clone(), link.min_rate(), false),
    );
    let g = match link.modulation {
        SbandModulation::Pm => g.add(
            "demod",
            PmDemodComponent::new(link.symbol_rate, link.roll_off, 0.45 * link.symbol_rate),
        ),
        SbandModulation::Fsk => g.add(
            "demod",
            FskDemodComponent::new(link.symbol_rate, link.roll_off),
        ),
        _ => g.add("demod", DemodComponent::new(psk)),
    };
    g.add("frames", FrameTableComponent::new(link.table()))
        .add("display", SbandDisplay::new(link.clone()))
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "frames.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("frames.report", "display.table")
        .feedback("display.control", "demod.control")
        .output("frames.frames")
        .status_from("display")
        .clock(REPORT_S)
}

/// The receiver of `link` with the settings of `cfg`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig, link: &SbandLink) -> Result<StackGraph, GraphError> {
    let mut g = builder(link).catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            link.min_rate(),
            "this link needs 2.8 samples a symbol and at least 500 kHz of samples",
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for s in SbandSetting::schema().settings() {
        let key = s.field.key.to_string();
        let target = format!("display.{key}");
        b = b.route(&key, &[target.as_str()])?;
    }
    b.with_settings(&cfg.settings).build()
}

fn link_error(e: LinkError) -> ConfigError {
    ConfigError::Stage(StageError::new(e))
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    let link = SbandLink::from_values(&cfg.settings).map_err(link_error)?;
    super::stack_receiver(receiver(cfg, &link))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let link = SbandLink::from_values(&cfg.settings).map_err(link_error)?;
    Ok(Box::new(
        SbandGenerator::new(cfg.rate_hz, &link, cfg.seed).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "S-band CCSDS",
    group: crate::GROUP,
    blurb: "A CCSDS telemetry link on S or L band, set by its link settings (modulation, rate, code, frame): the frame list with spacecraft and virtual channel ids. It needs a tracking dish.",
    // The default frequency is the one of the test signal, and the rate and width are those of the default link (1 Msym/s).
    // The host gives the frequency and the link of a session.
    tuning: Tuning::anywhere(2215e6, 4e6, MIN_RATE, 1.52e6),
    submodes: &[],
    settings: SbandSetting::schema,
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

/// The bytes of test frame `counter` of `link`: the sync word, then the data bytes (44 85, the 24-bit counter, then bytes of
/// `rng`) behind the randomiser and the Reed-Solomon code of the link, padded with zeros to the whole bytes of the frame.
pub fn test_frame(link: &SbandLink, counter: u32, rng: &mut Xorshift64) -> Vec<u8> {
    let n = link.data_bytes();
    let mut data = test_data(counter, n.max(5), rng);
    data.truncate(n);
    if link.randomiser == Randomiser::Inside {
        sdrkit_proto_ccsds::randomiser::tm_bytes(&mut data);
    }
    let mut out = Vec::with_capacity(link.frame_bytes() + 1);
    let coded =
        ChannelCoder::new(link.channel()).is_ok_and(|mut c| c.encode(&data, &mut out).is_ok());
    if !coded {
        out.clear();
        out.resize(4, 0);
    }
    out[..4].copy_from_slice(&link.sync_word.to_be_bytes());
    out.resize(link.frame_bits.div_ceil(8) as usize, 0);
    out
}

/// The channel bits of the test signal: frames, then NRZ-M and the K = 7 code, or the four-phase differential coding of
/// uncoded QPSK with NRZ-M.
pub struct SbandSource {
    link: SbandLink,
    coder: CcsdsCoder,
    diff: Option<QuadDiffEncoder>,
    rng: Xorshift64,
    counter: u32,
    bits: Vec<u8>,
}

impl SbandSource {
    /// The source of `link` with random bytes of seed `seed`.
    pub fn new(link: &SbandLink, seed: u64) -> Self {
        let diff = !link.conv && link.nrzm && link.bits_per_symbol() == 2;
        SbandSource {
            link: link.clone(),
            coder: CcsdsCoder::new(CodeOptions {
                nrzm: link.nrzm && !diff,
                invert_g2: true,
                conv: link.conv,
            }),
            diff: diff.then(QuadDiffEncoder::default),
            rng: Xorshift64::new(seed ^ Xorshift64::GOLDEN),
            counter: 0,
            bits: Vec::new(),
        }
    }
}

impl BitSource for SbandSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let f = test_frame(&self.link, self.counter, &mut self.rng);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        self.bits.clear();
        for b in &f {
            self.bits.extend((0..8).rev().map(|k| (b >> k) & 1));
        }
        self.bits.truncate(self.link.frame_bits as usize);
        if let Some(e) = &mut self.diff {
            for &[a, b] in self.bits.as_chunks::<2>().0 {
                let (x, y) = e.encode(a, b);
                out.extend([x, y]);
            }
            return;
        }
        // The coder takes bytes: pack the bits, then keep as many channel bits as the frame gives.
        let mut bytes = vec![0u8; self.bits.len().div_ceil(8)];
        for (i, &b) in self.bits.iter().enumerate() {
            bytes[i / 8] |= b << (7 - i % 8);
        }
        let start = out.len();
        self.coder.push(&bytes, out);
        let keep = self.bits.len() * if self.link.conv { 2 } else { 1 };
        out.truncate(start + keep);
    }
}

/// The samples of a PM or FSK test signal.
struct Shaped {
    source: SbandSource,
    pm: Option<PhaseMod>,
    fsk: Option<Cpm>,
    bits: Vec<u8>,
    levels: Vec<f32>,
    samples: Vec<Complex32>,
    next: usize,
}

impl Shaped {
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut filled = 0;
        while filled < out.len() {
            if self.next >= self.samples.len() {
                self.bits.clear();
                self.source.next_bits(&mut self.bits);
                self.samples.clear();
                self.next = 0;
                if let Some(m) = &mut self.pm {
                    m.push(&self.bits, &mut self.samples);
                } else if let Some(m) = &mut self.fsk {
                    self.levels.clear();
                    self.levels
                        .extend(self.bits.iter().map(|&b| if b == 1 { 1.0 } else { -1.0 }));
                    m.push(&self.levels, &mut self.samples);
                }
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
    Psk(Box<Modulator<SbandSource>>),
    Shaped(Box<Shaped>),
}

/// The test signal of a link: frames of the link at unit power, with root raised cosine pulses of the link's roll-off. QPSK
/// sends I then Q (OQPSK: Q half a symbol later); PM moves the phase by 1.05 radians times the shaped signal; FSK moves the
/// frequency by half the symbol rate times the shaped signal.
pub struct SbandGenerator {
    rate: f64,
    info: SignalInfo,
    kind: Kind,
}

impl SbandGenerator {
    /// The signal of `link` at `rate` samples a second, with random bytes of seed `seed`.
    ///
    /// # Errors
    ///
    /// [`GenError`] if the rate is below 2 samples a symbol or the modulator refuses its options.
    pub fn new(rate: f64, link: &SbandLink, seed: u64) -> Result<Self, GenError> {
        let rs = link.symbol_rate;
        if rate < 2.0 * rs {
            return Err(GenError::RateTooLow {
                need: 2.0 * rs,
                got: rate,
            });
        }
        let source = SbandSource::new(link, seed);
        let bit_rate = rs * link.bits_per_symbol() as f64;
        let info = SignalInfo::new(rs * (1.0 + link.roll_off))
            .with_symbol_rate(rs)
            .with_bit_rate(bit_rate);
        let shaped = |pm, fsk| {
            Kind::Shaped(Box::new(Shaped {
                source: SbandSource::new(link, seed),
                pm,
                fsk,
                bits: Vec::new(),
                levels: Vec::new(),
                samples: Vec::new(),
                next: 0,
            }))
        };
        let kind = match link.modulation {
            SbandModulation::Pm => shaped(
                Some(PhaseMod::new(link.roll_off, 12.0, rate / rs, PM_INDEX_RAD)?),
                None,
            ),
            SbandModulation::Fsk => shaped(
                None,
                Some(Cpm::with_deviation(
                    Shape::Rrc {
                        beta: link.roll_off,
                    },
                    12.0,
                    0.5 * rs,
                    rs,
                    rate,
                )?),
            ),
            m => {
                let modulation = match m {
                    SbandModulation::Qpsk => Modulation::Qpsk,
                    SbandModulation::Oqpsk => Modulation::Oqpsk,
                    _ => Modulation::Bpsk,
                };
                let p = ModParams::new(rate, rs, link.roll_off, modulation);
                Kind::Psk(Box::new(Modulator::new(p, source)?))
            }
        };
        Ok(SbandGenerator { rate, info, kind })
    }
}

impl Generator for SbandGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        match &mut self.kind {
            Kind::Psk(m) => m.generate(out),
            Kind::Shaped(s) => s.generate(out),
        }
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
}

//! GOES GVAR (`goes_gvar`): the imager broadcast of GOES I to P at 1685.7 MHz, BPSK at 2 111 360 bit/s with NRZ-S and blocks
//! of different lengths. The stack lists the blocks and draws reduced visible and infrared pictures.
//!
//! Standard: GOES Operations Ground Equipment Interface Specification DRL 504-02, section 3 (GVAR Transmission Format).

use super::display::{instrument_choices, published, view_choices, with_series, Histories};
use super::notes::set_status;
use crate::components::instruments::instrument_owner;
use crate::components::last;
use crate::components::link::{
    control_port, item_format, item_port, DemodComponent, DemodReport, FrontEndComponent,
    FrontReport,
};
use crate::gen::{test_value, BitSource, GenError, ModParams, Modulator};
use crate::instruments::gvar::{
    build_block, sync_code, test_scan, Block, Deframer, DeframerStats, Imager, ImagerStats,
    BIT_RATE, IDLE_BLOCK, INFRARED, INFRARED_WIDTH, MAX_SCAN, VISIBLE, VISIBLE_WIDTH,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::{Strip, View};
use crate::typed::{GoesGvarBlockRow, GoesGvarFields, ImageRow, GOES_GVAR_BLOCKS, IMAGES};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Port, StackGraph, Timed,
};
use sdrkit_util_bits::line::Nrzi;
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "goes_gvar";
/// The least input rate.
pub const MIN_RATE: f64 = 4.5e6;

/// The link: BPSK at the GVAR bit rate.
pub fn link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Bpsk,
        symbol_rate: BIT_RATE,
        roll_off: 0.5,
        class: LinkClass::Geo,
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

const TEST: &str = "Test signal";

/// The picture shown.
pub const PICTURE: CatalogueSetting = CatalogueSetting {
    key: "picture",
    label: "Picture",
    help: "The visible picture or the infrared picture.",
    choices: || instrument_choices(layouts()),
    default: VISIBLE,
    advanced: false,
    role: None,
};
/// The view of the picture.
pub const VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "The channel shown. The visible picture has one view.",
    choices: || view_choices(layouts()),
    default: "visible",
    advanced: false,
    role: Some(sdrkit_core::SettingRole::PictureView),
};
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow", "", true);
/// Scans of the test picture before the scan count starts again.
pub const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Test scans", "", 1300)
    .range(1.0, 1974.0, 1.0)
    .section(TEST)
    .advanced();

sdrkit_core::settings! {
    /// The settings of `goes_gvar`, with the options of its test signal.
    pub enum GvarSetting {
        /// The picture shown.
        Picture = PICTURE,
        /// The view of the picture.
        View = VIEW,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// Test: scans.
        GenLines = GEN_LINES,
    }
}

/// The picture layouts: the reduced visible picture and the reduced infrared picture with its four channels.
pub fn layouts() -> Vec<PictureLayout> {
    let grey = |channel| View::Grey {
        channel,
        invert: false,
    };
    vec![
        PictureLayout {
            name: VISIBLE,
            width: VISIBLE_WIDTH as u32,
            max_rows: 2 * MAX_SCAN,
            full_mask: 1,
            views: vec![NamedView::new("visible", "Visible (channel 1)", grey(1))],
        },
        PictureLayout {
            name: INFRARED,
            width: INFRARED_WIDTH as u32,
            max_rows: MAX_SCAN,
            full_mask: 1,
            views: vec![
                NamedView::new("ir2", "Channel 2 (3.9 um)", grey(1)),
                NamedView::new("ir3", "Channel 3 (6.5 um)", grey(2)),
                NamedView::new("ir4", "Channel 4 (10.7 um)", grey(3)),
                NamedView::new("ir6", "Channel 6 (13.3 um) or 5 (12 um)", grey(4)),
            ],
        },
    ]
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The block and picture component.

/// One row of the block table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockRow {
    /// The block count of the header.
    pub count: u16,
    /// The block id as sent.
    pub id: u8,
    /// Bits of a word.
    pub word_size: u8,
    /// The word count of the header.
    pub word_count: u16,
    /// A header copy had a good CRC.
    pub header_ok: bool,
    /// The data field had a good CRC.
    pub data_ok: bool,
}

/// The report of [`GvarComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GvarReport {
    /// The counters of the deframer.
    pub blocks: DeframerStats,
    /// The counters of the pictures.
    pub imager: ImagerStats,
    /// A block is being read.
    pub in_block: bool,
    /// The header of the last good block.
    pub last: Option<BlockRow>,
    /// The spacecraft of the last good block.
    pub spacecraft: Option<u8>,
    /// The blocks since the last report (at most 32).
    pub rows: Vec<BlockRow>,
    /// Seconds of signal since the last good block.
    pub since_good_s: f64,
}
impl Item for GvarReport {
    const KIND: &'static str = "satimg.gvar_report";
}

/// Soft BPSK symbols to GVAR blocks to picture strips: hard decisions, NRZ-S (a change of the level is a 0), the block
/// deframer and the imager pictures.
#[derive(Default)]
pub struct GvarComponent {
    line: Nrzi,
    bits: Vec<u8>,
    deframer: Deframer,
    found: Vec<Block>,
    dec: Imager,
    strips: Vec<Strip>,
    rows: Vec<BlockRow>,
    last: Option<BlockRow>,
    spacecraft: Option<u8>,
    symbols: u64,
    last_good: u64,
}

impl Component for GvarComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.gvar",
            vec![Port::llr("in"), control_port()],
            vec![item_port::<Strip>("out"), item_port::<GvarReport>("report")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.deframer.reset();
        Ok(vec![item_format::<Strip>(), item_format::<GvarReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let soft = io.llrs(0);
        self.bits.clear();
        self.bits.extend(soft.iter().map(|&l| u8::from(l < 0)));
        self.line.decode(&mut self.bits);
        self.symbols += soft.len() as u64;
        self.found.clear();
        self.deframer.push(&self.bits, &mut self.found);
        for b in &self.found {
            let row = BlockRow {
                count: b.header.block_count,
                id: b.header.id,
                word_size: b.header.word_size,
                word_count: b.header.word_count,
                header_ok: b.header.crc_ok,
                data_ok: b.data_ok,
            };
            if self.rows.len() < 32 {
                self.rows.push(row);
            }
            if b.header.crc_ok && b.header.block().is_some() && !b.data.is_empty() {
                self.last = Some(row);
                self.last_good = self.symbols;
                if b.header.id != IDLE_BLOCK {
                    self.spacecraft = Some(b.header.spacecraft());
                }
                self.dec.push(b, &mut self.strips);
            }
        }
        io.items_out::<Strip>(0)
            .extend(self.strips.drain(..).map(|s| Timed {
                time: 0.0,
                value: s,
            }));
        if io.tick() {
            let r = GvarReport {
                blocks: self.deframer.stats,
                imager: self.dec.stats,
                in_block: self.deframer.in_block(),
                last: self.last,
                spacecraft: self.spacecraft,
                rows: std::mem::take(&mut self.rows),
                since_good_s: (self.symbols - self.last_good) as f64 / BIT_RATE,
            };
            io.items_out::<GvarReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    instrument_owner!();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(GoesGvarFields::SHAPE.fields)
            .with_table(&GOES_GVAR_BLOCKS)
            .with_table(&IMAGES),
    )
}

/// The display: states, fields, the block table, summary and histories.
pub struct GvarDisplay {
    published: Published,
    hist: Histories,
    row: u64,
    first_row_s: Option<f64>,
    summary: String,
    key: String,
}

impl Default for GvarDisplay {
    fn default() -> Self {
        GvarDisplay {
            published: published(display_declaration()),
            hist: Histories::default(),
            row: 0,
            first_row_s: None,
            summary: String::new(),
            key: String::new(),
        }
    }
}

impl Component for GvarDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.gvar_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<GvarReport>("gvar"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let none = GvarReport::default();
        let g = last::<GvarReport>(io, 2).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        let low = f.is_some_and(|f| f.rate_too_low);
        let good_recent = g.blocks.good > 0 && g.since_good_s < 3.0;
        let state = if low {
            0
        } else if good_recent {
            3
        } else if g.in_block {
            2
        } else if d.is_some_and(|d| d.state.locked && d.state.tracking) {
            1
        } else {
            0
        };
        let rows = g.imager.visible_rows.max(g.imager.infrared_rows);
        if rows > 0 && self.first_row_s.is_none() {
            self.first_row_s = Some(t);
        }
        if rows == 0 {
            self.first_row_s = None;
        }
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "GOES GVAR, {} blocks, scan {}",
                g.blocks.good, g.imager.scan
            ),
            2 => write!(self.summary, "GOES GVAR, sync code found, blocks failing"),
            1 => write!(self.summary, "GOES GVAR, symbols, no block sync"),
            _ if low => write!(
                self.summary,
                "GOES GVAR needs at least {} MHz of samples",
                MIN_RATE / 1e6
            ),
            _ => write!(self.summary, "GOES GVAR, searching"),
        };
        let p = &mut self.published;
        let carrier = super::display::carrier_hz(f, d);
        set_status(
            p.results_mut(),
            state,
            d.filter(|d| d.state.tracking).map(|d| d.state.es_n0_db),
            f.map(|f| f.level_dbfs),
            Some(carrier + f.map_or(0.0, |f| f.user_offset_hz)),
            &self.summary,
        );
        p.put(&GoesGvarFields {
            snr_db: d.map(|d| f64::from(d.state.es_n0_db)),
            level_dbfs: f.map(|f| f64::from(f.level_dbfs)),
            offset_hz: d.map(|_| carrier).map(Hertz),
            clock_ppm: d.map(|d| d.state.clock_ppm),
            symbol_rate_hz: d.map(|d| d.symbol_rate_nominal).map(Hertz),
            frames_ok: g.blocks.good as i64,
            frames_bad: g.blocks.bad as i64,
            signal_time: Seconds(t),
            input_rate_hz: f.map(|f| f.input_rate).map(Hertz),
            lock_metric: d.map(|d| f64::from(d.state.lock_metric)),
            spacecraft: g.spacecraft.map(i64::from),
            sync_hits: g.blocks.sync_hits as i64,
            idle_blocks: g.blocks.idle as i64,
            data_crc_errors: g.blocks.data_crc_errors as i64,
            block_id: g.last.map(|l| i64::from(l.id)),
            block_count: g.last.map(|l| i64::from(l.count)),
            scan: i64::from(g.imager.scan),
            frame_starts: g.imager.frame_starts as i64,
            visible_rows: i64::from(g.imager.visible_rows),
            infrared_rows: i64::from(g.imager.infrared_rows),
            records_refused: g.imager.records_refused as i64,
            note: "Experimental: not confirmed on a real signal. The pictures are reduced: visible detectors 1 and 5 of 8 and every \
                 fourth pixel; the first infrared detector of each channel and every second pixel. Rows follow the scan count \
                 of the imager frame. Not decoded: block 0 beyond its frame start flag, the sounder and auxiliary blocks 11, \
                 calibration and navigation.",
        });
        for b in &g.rows {
            self.key.clear();
            // A write to a String cannot fail.
            let _ = write!(self.key, "{}", self.row);
            self.row += 1;
            GOES_GVAR_BLOCKS.put(
                p,
                &self.key,
                &GoesGvarBlockRow {
                    block_count: i64::from(b.count),
                    block_id: i64::from(b.id),
                    word_size: i64::from(b.word_size),
                    word_count: i64::from(b.word_count),
                    header_ok: b.header_ok,
                    data_ok: b.data_ok,
                },
            );
        }
        for (name, width, rows) in [
            (VISIBLE, VISIBLE_WIDTH, g.imager.visible_rows),
            (INFRARED, INFRARED_WIDTH, g.imager.infrared_rows),
        ] {
            if rows == 0 {
                continue;
            }
            IMAGES.put(
                p,
                name,
                &ImageRow {
                    product: name,
                    channel: None,
                    time: self.first_row_s.map(Seconds),
                    width: width as i64,
                    height: Some(i64::from(rows)),
                },
            );
        }
        self.hist.publish(self.published.results_mut(), d, None);
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The GVAR graph.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts produce no valid schema.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let pictures = Pictures::new(layouts()).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add("frontend", FrontEndComponent::new(link(), MIN_RATE, false))
        .add("demod", DemodComponent::new(link()))
        .add("gvar", GvarComponent::default())
        .add("pictures", pictures)
        .add("display", GvarDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "gvar.in")
        .connect("gvar.out", "pictures.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("gvar.report", "display.gvar")
        .output("pictures.picture")
        .status_from("display")
        .clock(REPORT_S))
}

/// The GVAR receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "GOES GVAR needs at least 4.5 MHz of samples")
        .route("picture", &["pictures.instrument"])?
        .route("view", &["pictures.view"])?
        .route("follow", &["pictures.follow"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        GvarGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "GOES GVAR",
    group: crate::GROUP,
    blurb: "The GVAR imager broadcast of GOES I to P (BPSK, 2.11 Mbit/s): the block list and reduced visible and infrared pictures.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(1685.7e6, 6e6, MIN_RATE, 3.2e6)
    },
    submodes: &[],
    settings: GvarSetting::schema,
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

/// The channel bits of the GVAR test signal: for each scan the twelve blocks, each behind its synchronisation code, coded
/// and NRZ-S encoded.
pub struct GvarSource {
    scans: u32,
    scan: u32,
    count: u16,
    queue: std::collections::VecDeque<Vec<u8>>,
    line: Nrzi,
    sync: Vec<u8>,
}

impl BitSource for GvarSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        if self.queue.is_empty() {
            let scan = self.scan % self.scans + 1;
            self.scan += 1;
            for (h, data) in test_scan(scan, self.count, test_value) {
                self.queue.push_back(build_block(&h, &data));
            }
            self.count = self.count.wrapping_add(12);
        }
        let start = out.len();
        out.extend_from_slice(&self.sync);
        if let Some(block) = self.queue.pop_front() {
            for b in block {
                out.extend((0..8).rev().map(|k| (b >> k) & 1));
            }
        }
        self.line.encode(&mut out[start..]);
    }
}

/// The GVAR test signal: the clean signal at unit mean power.
pub struct GvarGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<GvarSource>,
}

impl GvarGenerator {
    /// From a configuration: `gen_lines`. The schema of the settings must be [`GvarSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let l = link();
        let modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
        let src = GvarSource {
            scans: cfg.settings.get(&GEN_LINES).clamp(1, i64::from(MAX_SCAN)) as u32,
            scan: 0,
            count: 0,
            queue: Default::default(),
            line: Nrzi::new(0),
            sync: sync_code(),
        };
        Ok(GvarGenerator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for GvarGenerator {
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

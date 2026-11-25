//! NOAA GAC (`noaa_gac`): the recorder playback of the NOAA KLM satellites, BPSK at 2 661 600 bit/s with frames of 3327
//! words. The stack draws the reduced AVHRR picture and the HIRS sounder lines of the TIP frames inside.
//!
//! Standard: NOAA KLM User's Guide, sections 1.2.5, 1.2.6, 3.1.3.2 and Table 4.1.3.1-1.

use super::display::{instrument_choices, published, view_choices, with_series, Histories};
use super::notes::set_status;
use crate::components::instruments::instrument_owner;
use crate::components::last;
use crate::components::link::{
    control_port, item_format, item_port, DemodComponent, DemodReport, FrontEndComponent,
    FrontReport,
};
use crate::components::pass::{
    pass_format, pass_port, PassPictures, PassSetting, PassTimer, PASS_GAP,
};
use crate::gen::{pass_lines, test_value, BitSource, GenError, ModParams, Modulator};
use crate::instruments::gac::{
    frame_code, pack_words, scramble_mask, spacecraft, test_words, GacDecoder, GacStats, BIT_RATE,
    CHANNELS, FRAME_BITS, INSTRUMENT, MAX_ROWS, SYNC, WIDTH,
};
use crate::instruments::tip::{test_frame, HIRS_ELEMENTS, HIRS_WIDTH, MAJOR_FRAME, TIP_BYTES};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView};
use sdrkit_codec_image::strip::{Strip, View};
use crate::typed::{ImageRow, NoaaGacFields, IMAGES};
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
use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig};
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "noaa_gac";
/// The least input rate.
pub const MIN_RATE: f64 = 6e6;
/// The picture of the reduced AVHRR data.
pub const AVHRR: &str = INSTRUMENT;
/// The picture of the HIRS sounder.
pub const HIRS: &str = "HIRS";

/// The rules of the frame synchroniser: the first 32 bits of the sync words, 3 wrong bits to acquire, 10 to track, either
/// polarity (the complement settles the half-turn ambiguity of BPSK).
pub const SYNC_RULES: WordSyncConfig = WordSyncConfig {
    word: SYNC as u64,
    word_bits: 32,
    frame_bits: FRAME_BITS,
    acquire_errors: 3,
    track_errors: 10,
    complement: true,
    max_misses: 3,
};

/// The link: BPSK at the playback rate.
pub fn link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Bpsk,
        symbol_rate: BIT_RATE,
        roll_off: 0.5,
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

const TEST: &str = "Test signal";

/// The picture shown.
pub const PICTURE: CatalogueSetting = CatalogueSetting {
    key: "picture",
    label: "Picture",
    help: "The AVHRR picture or the HIRS picture.",
    choices: || instrument_choices(layouts()),
    default: AVHRR,
    advanced: false,
    role: None,
};
/// The view of the picture.
pub const VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "What the picture shows.",
    choices: || view_choices(layouts()),
    default: "rgb214",
    advanced: false,
    role: Some(sdrkit_core::SettingRole::PictureView),
};
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow", "", true);
/// The spacecraft address of the test signal.
pub const GEN_SCID: Setting<field::I64> =
    Setting::new("gen_scid", "Test spacecraft address", "", 15)
        .range(0.0, 15.0, 1.0)
        .section(TEST)
        .advanced();
/// The lines of the test signal: a pass of that many lines, then the signal ends. 0: the test picture of
/// [`TEST_PICTURE_LINES`] lines repeats without end.
pub const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Test picture lines", "", 0)
    .range(1.0, 4000.0, 1.0)
    .zero_means("Without end")
    .help("The test signal sends this many lines of the test picture and then ends. Without end: the picture repeats.")
    .section(TEST)
    .advanced();
/// The lines of the test picture before it repeats.
pub const TEST_PICTURE_LINES: u32 = 400;

sdrkit_core::settings! {
    /// The settings of `noaa_gac`, with the options of its test signal.
    pub enum GacSetting {
        /// The picture shown.
        Picture = PICTURE,
        /// The view of the picture.
        View = VIEW,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// The end of a pass.
        PassGap = PASS_GAP,
        /// Test: the spacecraft address.
        GenScid = GEN_SCID,
        /// Test: picture lines.
        GenLines = GEN_LINES,
    }
}

/// The picture layouts: AVHRR (409 pixels, 5 channels, the colour view and one grey view a channel) and HIRS.
pub fn layouts() -> Vec<PictureLayout> {
    let mut views = vec![NamedView::new(
        "rgb214",
        "Colour (2, 1, 4)",
        View::Rgb([(2, false), (1, false), (4, true)]),
    )];
    for c in 1..=CHANNELS as u16 {
        views.push(NamedView::new(
            format!("ch{c}"),
            format!("Channel {c}"),
            View::Grey {
                channel: c,
                invert: false,
            },
        ));
    }
    vec![
        PictureLayout {
            name: AVHRR,
            width: WIDTH as u32,
            max_rows: MAX_ROWS,
            full_mask: 1,
            views,
        },
        super::dsb::picture_layout(),
    ]
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The frame and instrument component.

/// The report of [`GacComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GacReport {
    /// A confirmed frame lock is held.
    pub locked: bool,
    /// The stream of the lock is inverted.
    pub inverted: bool,
    /// Sync words found.
    pub sync_hits: u64,
    /// The counters of the decoder.
    pub stats: GacStats,
    /// The identification word of the last frame.
    pub id: Option<u16>,
    /// Seconds of signal since the last frame.
    pub since_frame_s: f64,
    /// New log lines.
    pub log: Vec<String>,
}
impl Item for GacReport {
    const KIND: &'static str = "satimg.gac_report";
}

/// Soft BPSK symbols to GAC frames to AVHRR and HIRS strips: hard decisions (a positive symbol is a 1), the frame
/// synchroniser, the descrambler, the words, the AVHRR pixels and the five TIP frames of each frame.
pub struct GacComponent {
    sync: Option<WordSync>,
    bits: Vec<u8>,
    frames: Vec<Vec<u8>>,
    symbols: u64,
    last_frame: u64,
    was_locked: bool,
    dec: GacDecoder,
    strips: Vec<Strip>,
    pass: PassTimer,
    log: Vec<String>,
}

impl Default for GacComponent {
    fn default() -> Self {
        GacComponent {
            sync: WordSync::new(SYNC_RULES).ok(),
            bits: Vec::new(),
            frames: Vec::new(),
            symbols: 0,
            last_frame: 0,
            was_locked: false,
            dec: GacDecoder::default(),
            strips: Vec::new(),
            pass: PassTimer::default(),
            log: Vec::new(),
        }
    }
}

impl Component for GacComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.gac",
            vec![Port::llr("in"), control_port()],
            vec![
                item_port::<Strip>("out"),
                item_port::<GacReport>("report"),
                pass_port(),
            ],
        )
        .with_settings(PassSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(s) = &mut self.sync {
            s.drop_lock();
        }
        Ok(vec![
            item_format::<Strip>(),
            item_format::<GacReport>(),
            pass_format(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let soft = io.llrs(0);
        self.symbols += soft.len() as u64;
        self.bits.clear();
        self.bits.extend(soft.iter().map(|&l| u8::from(l < 0)));
        self.frames.clear();
        if let Some(s) = &mut self.sync {
            let frames = &mut self.frames;
            s.push_bits(&self.bits, |f| frames.push(f.bytes.to_vec()));
            let locked = s.locked();
            if locked != self.was_locked {
                self.log.push(
                    if locked {
                        "frame sync"
                    } else {
                        "frame sync lost"
                    }
                    .into(),
                );
                self.was_locked = locked;
            }
        }
        for f in &self.frames {
            self.dec.push(f, &mut self.strips);
            self.last_frame = self.symbols;
        }
        if self.pass.after(io, !self.strips.is_empty(), 2) {
            self.dec.new_picture();
        }
        io.items_out::<Strip>(0)
            .extend(self.strips.drain(..).map(|s| Timed {
                time: 0.0,
                value: s,
            }));
        if io.tick() {
            let s = self.sync.as_ref();
            let r = GacReport {
                locked: s.is_some_and(WordSync::locked),
                inverted: s.is_some_and(WordSync::inverted),
                sync_hits: s.map_or(0, |s| s.stats().sync_hits),
                stats: self.dec.stats,
                id: self.dec.id,
                since_frame_s: (self.symbols - self.last_frame) as f64 / BIT_RATE,
                log: std::mem::take(&mut self.log),
            };
            io.items_out::<GacReport>(1).push(Timed {
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
            .with_fields(NoaaGacFields::SHAPE.fields)
            .with_table(&IMAGES),
    )
}

/// The display: states, fields, summary and histories.
pub struct GacDisplay {
    published: Published,
    hist: Histories,
    first_row_s: Option<f64>,
    summary: String,
}

impl Default for GacDisplay {
    fn default() -> Self {
        GacDisplay {
            published: published(display_declaration()),
            hist: Histories::default(),
            first_row_s: None,
            summary: String::new(),
        }
    }
}

impl Component for GacDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.gac_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<GacReport>("gac"),
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
        let none = GacReport::default();
        let g = last::<GacReport>(io, 2).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        for l in &g.log {
            self.published.results_mut().log(l.as_str());
        }
        let low = f.is_some_and(|f| f.rate_too_low);
        let good_recent = g.stats.frames > 0 && g.since_frame_s < 3.0;
        let state = if low {
            0
        } else if good_recent && g.locked {
            3
        } else if g.locked {
            2
        } else if d.is_some_and(|d| d.state.locked && d.state.tracking) {
            1
        } else {
            0
        };
        let rows = g.stats.rows;
        if rows > 0 && self.first_row_s.is_none() {
            self.first_row_s = Some(t);
        }
        if rows == 0 {
            self.first_row_s = None;
        }
        let code = g.id.map(frame_code);
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "NOAA GAC, {rows} lines, {} frames",
                g.stats.frames
            ),
            2 => write!(self.summary, "NOAA GAC, frame sync, frames failing"),
            1 => write!(self.summary, "NOAA GAC, symbols, no frame sync"),
            _ if low => write!(
                self.summary,
                "this link needs {} Msps or more (a recording works)",
                MIN_RATE / 1e6
            ),
            _ => write!(self.summary, "NOAA GAC, searching"),
        };
        if state >= 2 && code.is_some_and(|c| c != 0) {
            self.summary
                .push_str("; identification word is not that of a GAC frame");
        }
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
        p.put(&NoaaGacFields {
            snr_db: d.map(|d| f64::from(d.state.es_n0_db)),
            level_dbfs: f.map(|f| f64::from(f.level_dbfs)),
            offset_hz: d.map(|_| carrier).map(Hertz),
            clock_ppm: d.map(|d| d.state.clock_ppm),
            symbol_rate_hz: d.map(|d| d.symbol_rate_nominal).map(Hertz),
            frames_ok: g.stats.frames as i64,
            frames_bad: 0,
            signal_time: Seconds(t),
            input_rate_hz: f.map(|f| f.input_rate).map(Hertz),
            lock_metric: d.map(|d| f64::from(d.state.lock_metric)),
            frame_lock: g.locked,
            spacecraft: g.id.map(|i| i64::from(spacecraft(i))),
            id_code: code.map(i64::from),
            sync_hits: g.sync_hits as i64,
            tip_frames: g.stats.tip_frames as i64,
            lines: i64::from(rows),
            hirs_lines: i64::from(g.stats.hirs.rows),
            inverted: g.inverted,
            note: "Experimental: not confirmed on a real signal. The satellite is shown by its address number only. Not confirmed \
                 by a public document: BPSK as the modulation of the playback link, that the GAC frame starts with the HRPT sync \
                 words, the places of the TIP words (103 to 622) and of the AVHRR words (1182 to 3226), the order of the pixels, \
                 that the whole frame after the sync words is scrambled, and the spacecraft address values. Not decoded: the time \
                 code, words 7 to 102, 623 to 1181 and 3227 to 3326, a playback in the opposite direction.",
        });
        for (name, width, rows) in [(AVHRR, WIDTH, rows), (HIRS, HIRS_WIDTH, g.stats.hirs.rows)] {
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

/// The GAC graph.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts produce no valid schema.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let pictures = PassPictures::new(layouts(), "NOAA").map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add("frontend", FrontEndComponent::new(link(), MIN_RATE, false))
        .add("demod", DemodComponent::new(link()))
        .add("gac", GacComponent::default())
        .add("pictures", pictures)
        .add("display", GacDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "gac.in")
        .connect("gac.out", "pictures.in")
        .connect("gac.pass", "pictures.pass")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("gac.report", "display.gac")
        .output("pictures.picture")
        .status_from("display")
        .clock(REPORT_S))
}

/// The GAC receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "NOAA GAC needs at least 6 MHz of samples")
        .route("picture", &["pictures.instrument"])?
        .route("view", &["pictures.view"])?
        .route("follow", &["pictures.follow"])?
        .route("pass_gap", &["gac.pass_gap"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        GacGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "NOAA GAC",
    group: crate::GROUP,
    blurb: "The recorder playback (GAC) of the NOAA KLM satellites at 2.66 Mbit/s: reduced AVHRR pictures and HIRS sounder lines.",
    tuning: Tuning {
        min_hz: 1690e6,
        max_hz: 2255e6,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(1702.5e6, 8e6, MIN_RATE, 3.73e6)
    },
    submodes: &[],
    settings: GacSetting::schema,
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

/// The channel bits of the GAC test signal: frames of the test picture, each with five TIP frames, scrambled from bit 60.
pub struct GacSource {
    scid: u8,
    lines: u32,
    line: u32,
    /// The lines of the pass, `None` for a signal without end.
    pass: Option<u64>,
    tip: u64,
    mask: Vec<u8>,
}

impl GacSource {
    /// The bytes of the next frame, scrambled.
    pub fn next_frame(&mut self) -> Vec<u8> {
        let mut tip = [[0u8; TIP_BYTES]; 5];
        for t in &mut tip {
            let per_line = u64::from(HIRS_ELEMENTS);
            let line = ((self.tip / per_line) % u64::from(self.lines)) as u32;
            *t = test_frame(
                self.scid,
                (self.tip % u64::from(MAJOR_FRAME)) as u16,
                (self.tip % per_line) as u8,
                line,
                test_value,
            );
            self.tip += 1;
        }
        let words = test_words(self.scid, self.line % self.lines, &tip, test_value);
        self.line += 1;
        let mut f = pack_words(&words);
        f.iter_mut().zip(&self.mask).for_each(|(b, m)| *b ^= m);
        f
    }
}

impl BitSource for GacSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let f = self.next_frame();
        out.extend((0..FRAME_BITS).map(|b| (f[b / 8] >> (7 - b % 8)) & 1));
    }
    fn ended(&self) -> bool {
        self.pass.is_some_and(|p| u64::from(self.line) >= p)
    }
}

/// The GAC test signal: the clean signal at unit mean power.
pub struct GacGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<GacSource>,
}

impl GacGenerator {
    /// From a configuration: `gen_scid` and `gen_lines`. The schema of the settings must be [`GacSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the rate.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let l = link();
        let modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
        let v = &cfg.settings;
        let src = GacSource {
            scid: v.get(&GEN_SCID).clamp(0, 15) as u8,
            lines: TEST_PICTURE_LINES,
            pass: pass_lines(v.get(&GEN_LINES)),
            line: 0,
            tip: 0,
            mask: scramble_mask(),
        };
        Ok(GacGenerator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for GacGenerator {
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

//! NOAA DSB (`noaa_dsb`): the TIP beacon of the NOAA KLM satellites on 137 MHz, 8320 bit/s split-phase data that move the
//! phase of the carrier by 67 degrees. The stack counts the TIP minor frames and draws the HIRS sounder lines.
//!
//! Standard: NOAA KLM User's Guide, sections 4.3 (DSB and TIP) and 4.3.4.1 (HIRS).

use super::display::{published, view_choices, with_series, Histories};
use super::notes::set_status;
use crate::components::instruments::instrument_owner;
use crate::components::link::{
    control_port, item_format, item_port, DemodReport, FrontEndComponent, FrontReport,
};
use crate::components::pm::PmDemodComponent;
use crate::components::{last, Control};
use crate::gen::{test_value, GenError};
use crate::instruments::tip::{
    test_frame, tip_header, HirsDecoder, HirsStats, HIRS_ELEMENTS, HIRS_MAX_ROWS, HIRS_WIDTH,
    MAJOR_FRAME, TIP_BITS, TIP_BIT_RATE, TIP_BYTES, TIP_SYNC,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_algo_sim::modulate::PhaseMod;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::{Strip, View};
use crate::typed::{ImageRow, NoaaDsbFields, IMAGES};
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
use sdrkit_proto_ccsds::multisync::{SplitPhaseSync, StreamEvent};
use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "noaa_dsb";
/// The least input rate.
pub const MIN_RATE: f64 = 40e3;
/// Chips a second: two for each bit.
pub const CHIP_RATE: f64 = 2.0 * TIP_BIT_RATE;
/// The phase index of the downlink, degrees.
pub const INDEX_DEG: f64 = 67.0;
/// The carrier search covers this many Hz to each side: more than the Doppler shift of +-3.4 kHz.
const SEARCH_HZ: f64 = 7500.0;
/// Without a frame lock the carrier is searched again after this many seconds.
const RESEARCH_S: f64 = 2.5;

/// The rules of the frame synchroniser: the 16-bit sync word, no wrong bit to acquire, 4 to track, either polarity.
pub const SYNC: WordSyncConfig = WordSyncConfig {
    word: TIP_SYNC as u64,
    word_bits: 16,
    frame_bits: TIP_BITS,
    acquire_errors: 0,
    track_errors: 4,
    complement: true,
    max_misses: 3,
};

/// What the front end needs to know of the link: the chip rate and the filter class.
fn front_link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Bpsk,
        symbol_rate: CHIP_RATE,
        roll_off: 1.0,
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

/// The view of the HIRS picture.
pub const VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "What the HIRS picture shows.",
    choices: || view_choices(vec![picture_layout()]),
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
/// The phase index of the test signal.
pub const GEN_PM_INDEX: Setting<field::F64> =
    Setting::new("gen_pm_index", "Test phase index", "deg", INDEX_DEG)
        .range(10.0, 85.0, 0.5)
        .section(TEST)
        .advanced();
/// Lines of the test picture before it repeats.
pub const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Test picture lines", "", 400)
    .range(1.0, 4000.0, 1.0)
    .section(TEST)
    .advanced();

sdrkit_core::settings! {
    /// The settings of `noaa_dsb`, with the options of its test signal.
    pub enum DsbSetting {
        /// The view of the picture.
        View = VIEW,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// Test: the spacecraft address.
        GenScid = GEN_SCID,
        /// Test: the phase index.
        GenPmIndex = GEN_PM_INDEX,
        /// Test: picture lines.
        GenLines = GEN_LINES,
    }
}

/// The picture layout of HIRS: 56 Earth views a line, 20 channels, a colour view and one grey view for each channel.
pub fn picture_layout() -> PictureLayout {
    let mut views = vec![NamedView::new(
        "rgb214",
        "Colour (2, 1, 4)",
        View::Rgb([(2, false), (1, false), (4, true)]),
    )];
    for c in 1..=20u16 {
        views.push(NamedView::new(
            format!("ch{c}"),
            format!("HIRS channel {c}"),
            View::Grey {
                channel: c,
                invert: false,
            },
        ));
    }
    PictureLayout {
        name: "HIRS",
        width: HIRS_WIDTH as u32,
        max_rows: HIRS_MAX_ROWS,
        full_mask: 1,
        views,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The frame and instrument component.

/// The report of [`TipComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TipReport {
    /// A confirmed frame lock is held.
    pub locked: bool,
    /// Which chip starts a bit in the lock: 0 or 1.
    pub pairing: Option<u8>,
    /// The stream of the lock is inverted.
    pub inverted: bool,
    /// Minor frames of confirmed locks.
    pub frames: u64,
    /// Sync words found by both synchronisers.
    pub sync_hits: u64,
    /// The spacecraft address of the last frame.
    pub spacecraft: Option<u8>,
    /// The minor frame counter of the last frame.
    pub minor_frame: Option<u16>,
    /// The HIRS counters.
    pub hirs: HirsStats,
    /// Seconds of signal since the last frame.
    pub since_frame_s: f64,
    /// New log lines.
    pub log: Vec<String>,
}
impl Item for TipReport {
    const KIND: &'static str = "satimg.tip_report";
}

/// Soft split-phase chips to TIP minor frames to HIRS strips, through the two chip pairings of [`SplitPhaseSync`].
pub struct TipComponent {
    sync: Option<SplitPhaseSync>,
    chips: u64,
    frames: Vec<[u8; TIP_BYTES]>,
    count: u64,
    last_frame_chip: u64,
    spacecraft: Option<u8>,
    minor_frame: Option<u16>,
    dec: HirsDecoder,
    strips: Vec<Strip>,
    log: Vec<String>,
}

impl Default for TipComponent {
    fn default() -> Self {
        TipComponent {
            sync: SplitPhaseSync::new(SYNC).ok(),
            chips: 0,
            frames: Vec::new(),
            count: 0,
            last_frame_chip: 0,
            spacecraft: None,
            minor_frame: None,
            dec: HirsDecoder::default(),
            strips: Vec::new(),
            log: Vec::new(),
        }
    }
}

impl Component for TipComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.tip_hirs",
            vec![Port::llr("in"), control_port()],
            vec![item_port::<Strip>("out"), item_port::<TipReport>("report")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(s) = &mut self.sync {
            s.drop_lock();
        }
        Ok(vec![item_format::<Strip>(), item_format::<TipReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let chips = io.llrs(0);
        self.chips += chips.len() as u64;
        self.frames.clear();
        if let Some(s) = &mut self.sync {
            let frames = &mut self.frames;
            s.push_llrs(chips, |_, f| {
                let mut b = [0u8; TIP_BYTES];
                b.copy_from_slice(&f.bytes[..TIP_BYTES]);
                frames.push(b);
            });
            for e in s.take_events() {
                self.log.push(match e {
                    StreamEvent::Chosen(w) => format!("frame sync (chip pairing {w})"),
                    StreamEvent::Lost => "frame sync lost".into(),
                });
            }
        }
        for f in &self.frames {
            self.count += 1;
            self.last_frame_chip = self.chips;
            let h = tip_header(f);
            self.spacecraft = Some(h.spacecraft);
            self.minor_frame = Some(h.minor_frame);
            self.dec.push_frame(f, &mut self.strips);
        }
        io.items_out::<Strip>(0)
            .extend(self.strips.drain(..).map(|s| Timed {
                time: 0.0,
                value: s,
            }));
        if io.tick() {
            let s = self.sync.as_ref();
            let r = TipReport {
                locked: s.is_some_and(|s| s.chosen().is_some()),
                pairing: s.and_then(SplitPhaseSync::chosen).map(|c| c as u8),
                inverted: s.is_some_and(SplitPhaseSync::inverted),
                frames: self.count,
                sync_hits: s.map_or(0, SplitPhaseSync::sync_hits),
                spacecraft: self.spacecraft,
                minor_frame: self.minor_frame,
                hirs: self.dec.stats,
                since_frame_s: (self.chips - self.last_frame_chip) as f64 / CHIP_RATE,
                log: std::mem::take(&mut self.log),
            };
            io.items_out::<TipReport>(1).push(Timed {
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
            .with_fields(NoaaDsbFields::SHAPE.fields)
            .with_table(&IMAGES),
    )
}

/// The display: states, fields, summary, histories and the new carrier search.
pub struct DsbDisplay {
    published: Published,
    hist: Histories,
    searches: u32,
    search_end_s: f64,
    first_line_s: Option<f64>,
    summary: String,
}

impl Default for DsbDisplay {
    fn default() -> Self {
        DsbDisplay {
            published: published(display_declaration()),
            hist: Histories::default(),
            searches: 0,
            search_end_s: 0.0,
            first_line_s: None,
            summary: String::new(),
        }
    }
}

impl Component for DsbDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.dsb_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<TipReport>("tip"),
            ],
            vec![item_port::<Control>("control")],
        )
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
        let tip = last::<TipReport>(io, 2);
        let t = f.map_or(0.0, |f| f.time_s);
        let none = TipReport::default();
        let tip = tip.unwrap_or(&none);
        for l in &tip.log {
            self.published.results_mut().log(l.as_str());
        }
        let low = f.is_some_and(|f| f.rate_too_low);
        let good_recent = tip.frames > 0 && tip.since_frame_s < 3.0;
        let carrier = d.is_some_and(|d| d.state.locked);
        let state = if low {
            0
        } else if good_recent && tip.locked {
            3
        } else if tip.locked {
            2
        } else if carrier {
            1
        } else {
            0
        };
        // Without a frame lock the carrier is searched again, 2.5 s after the last search ended and after the last frame.
        if let Some(d) = d {
            if d.state.searches != self.searches {
                self.searches = d.state.searches;
                self.search_end_s = t;
            }
            if d.state.tracking
                && !tip.locked
                && t - self.search_end_s >= RESEARCH_S
                && (tip.frames == 0 || tip.since_frame_s >= RESEARCH_S)
            {
                self.search_end_s = t;
                io.items_out::<Control>(0).push(Timed {
                    time: t,
                    value: Control::RestartCarrier,
                });
            }
        }
        if tip.hirs.rows > 0 && self.first_line_s.is_none() {
            self.first_line_s = Some(t);
        }
        if tip.hirs.rows == 0 {
            self.first_line_s = None;
        }
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "NOAA DSB, {} lines, {} frames",
                tip.hirs.rows, tip.frames
            ),
            2 => write!(self.summary, "NOAA DSB, frame sync, frames failing"),
            1 => write!(self.summary, "NOAA DSB, symbols, no frame sync"),
            _ if low => write!(
                self.summary,
                "NOAA DSB needs at least {} kHz of samples",
                MIN_RATE / 1e3
            ),
            _ => write!(self.summary, "NOAA DSB, searching"),
        };
        let p = &mut self.published;
        let carrier_hz = d.map(|d| d.state.carrier_hz);
        set_status(
            p.results_mut(),
            state,
            d.filter(|d| d.state.tracking).map(|d| d.state.es_n0_db),
            f.map(|f| f.level_dbfs),
            carrier_hz.map(|c| c + f.map_or(0.0, |f| f.user_offset_hz)),
            &self.summary,
        );
        p.put(&NoaaDsbFields {
            snr_db: d.map(|d| f64::from(d.state.es_n0_db)),
            level_dbfs: f.map(|f| f64::from(f.level_dbfs)),
            offset_hz: carrier_hz.map(Hertz),
            clock_ppm: d.map(|d| d.state.clock_ppm),
            symbol_rate_hz: d.map(|d| d.symbol_rate_nominal).map(Hertz),
            frames_ok: tip.frames as i64,
            frames_bad: 0,
            signal_time: Seconds(t),
            input_rate_hz: f.map(|f| f.input_rate).map(Hertz),
            lock_metric: d.map(|d| f64::from(d.state.lock_metric)),
            spacecraft: tip.spacecraft.map(i64::from),
            sync_hits: tip.sync_hits as i64,
            tip_frames: tip.frames as i64,
            lines: i64::from(tip.hirs.rows),
            minor_frame: tip.minor_frame.map(i64::from),
            chip_pairing: tip.pairing.map(i64::from),
            inverted: tip.inverted,
            hirs_parity_errors: tip.hirs.parity_errors as i64,
            note: "Experimental: not confirmed on a real signal. The satellite is shown by its address number only. Not decoded: \
                 every instrument but HIRS, the housekeeping words and the HIRS calibration views. HIRS columns are in the \
                 order of the scan.",
        });
        if tip.hirs.rows > 0 {
            IMAGES.put(
                p,
                "HIRS",
                &ImageRow {
                    product: "HIRS",
                    channel: None,
                    time: self.first_line_s.map(Seconds),
                    width: HIRS_WIDTH as i64,
                    height: Some(i64::from(tip.hirs.rows)),
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

/// The DSB graph.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layout produces no valid schema.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let pictures = Pictures::new(vec![picture_layout()]).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(front_link(), MIN_RATE, false),
        )
        .add("demod", PmDemodComponent::new(CHIP_RATE, 1.0, SEARCH_HZ))
        .add("tip", TipComponent::default())
        .add("pictures", pictures)
        .add("display", DsbDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "tip.in")
        .connect("tip.out", "pictures.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("tip.report", "display.tip")
        .feedback("display.control", "demod.control")
        .output("pictures.picture")
        .status_from("display")
        .clock(REPORT_S))
}

/// The DSB receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "NOAA DSB needs at least 40 kHz of samples")
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
        DsbGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "NOAA DSB",
    group: crate::GROUP,
    blurb: "The TIP beacon (DSB) of the NOAA KLM satellites on 137 MHz: minor frames at 8320 bit/s and HIRS sounder lines.",
    tuning: Tuning {
        min_hz: 136e6,
        max_hz: 138e6,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(137.77e6, 1.024e6, MIN_RATE, 50e3)
    },
    submodes: &[],
    settings: DsbSetting::schema,
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

/// The DSB test signal: TIP minor frames with HIRS elements of the test picture, split-phase, phase modulated with a residual
/// carrier, at unit power.
pub struct DsbGenerator {
    rate: f64,
    spacecraft: u8,
    lines: u32,
    counter: u64,
    modulator: PhaseMod,
    chips: Vec<u8>,
    samples: Vec<Complex32>,
    next: usize,
}

impl DsbGenerator {
    /// From a configuration: `gen_scid`, `gen_pm_index` and `gen_lines`. The schema of the settings must be [`DsbSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the rate is below 40 kHz or the modulator refuses its options.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        if cfg.rate_hz < MIN_RATE {
            return Err(GenError::RateTooLow {
                need: MIN_RATE,
                got: cfg.rate_hz,
            });
        }
        let v = &cfg.settings;
        let index = v.get(&GEN_PM_INDEX).to_radians();
        Ok(DsbGenerator {
            rate: cfg.rate_hz,
            spacecraft: v.get(&GEN_SCID).clamp(0, 15) as u8,
            lines: v.get(&GEN_LINES).clamp(1, 4000) as u32,
            counter: 0,
            modulator: PhaseMod::new(1.0, 12.0, cfg.rate_hz / CHIP_RATE, index)?,
            chips: Vec::with_capacity(2 * TIP_BITS),
            samples: Vec::new(),
            next: 0,
        })
    }

    /// The next minor frame as samples.
    fn more(&mut self) {
        let per_line = u64::from(HIRS_ELEMENTS);
        let line = ((self.counter / per_line) % u64::from(self.lines)) as u32;
        let element = (self.counter % per_line) as u8;
        let frame = test_frame(
            self.spacecraft,
            (self.counter % u64::from(MAJOR_FRAME)) as u16,
            element,
            line,
            test_value,
        );
        self.counter += 1;
        self.chips.clear();
        for byte in frame {
            for k in (0..8).rev() {
                // Split-phase: a 1 is high then low.
                let b = (byte >> k) & 1;
                self.chips.push(b);
                self.chips.push(b ^ 1);
            }
        }
        self.samples.clear();
        self.next = 0;
        self.modulator.push(&self.chips, &mut self.samples);
    }
}

impl Generator for DsbGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut filled = 0;
        while filled < out.len() {
            if self.next >= self.samples.len() {
                self.more();
                continue;
            }
            let n = (self.samples.len() - self.next).min(out.len() - filled);
            out[filled..filled + n].copy_from_slice(&self.samples[self.next..self.next + n]);
            self.next += n;
            filled += n;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(2.0 * CHIP_RATE)
                .with_symbol_rate(CHIP_RATE)
                .with_bit_rate(TIP_BIT_RATE),
        )
    }
}

//! FY-2 S-VISSR (`fy2_svissr`): uncoded differential BPSK at 660 ksym/s, frames behind a PN preamble, descrambled, and
//! five pictures for each scan (IR1 to IR4, and VIS at half resolution).

use super::display::{
    log_lines, put_link, status, with_series, Histories,
};
use crate::components::link::{
    item_format, item_port, DemodComponent, DemodReport, FrontEndComponent, FrontReport,
};
use crate::components::pass::{
    pass_format, pass_port, PassPictures, PassSetting, PassTimer, PASS_GAP,
};
use crate::components::{last, Control};
use crate::files;
use crate::gen::{pass_lines, test_pixel, BitSource, GenError, ModParams, Modulator};
use crate::instruments::svissr::{
    build, parse, pn_bits, scramble, SvissrFramer, SvissrLine, FRAME_BYTES, IR_SAMPLES, MAX_LINES,
    PREAMBLE, VIS_SAMPLES,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView};
use sdrkit_codec_image::strip::{Strip, View};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use crate::typed::{FileArgs, Fy2SvissrFields, ImageRow, LinkFields, IMAGES, OPEN_SVISSR};
use sdrkit_core::interface::{Fields as _, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Record, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, Storage, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Port, StackGraph, Timed,
};
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "fy2_svissr";

/// The nominal symbol rate.
pub const SYMBOL_RATE: f64 = 660_000.0;

/// The link at `rs`.
pub fn link(rs: f64) -> LinkParams {
    LinkParams {
        modulation: Modulation::Bpsk,
        symbol_rate: rs,
        roll_off: 0.5,
        class: LinkClass::Fy2,
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

/// The picture names.
pub const NAMES: [&str; 5] = [
    "FY-2 S-VISSR IR1 10.8 um",
    "FY-2 S-VISSR IR2 12 um",
    "FY-2 S-VISSR IR3 6.75 um",
    "FY-2 S-VISSR IR4 3.75 um",
    "FY-2 S-VISSR VIS 0.725 um (half resolution)",
];

const TEST: &str = "Test signal";

/// The symbol rate in ksym/s, 0 for 660 ksym/s.
pub const SYMBOL_RATE_SETTING: Setting<field::F64> =
    Setting::new("symbol_rate", "Symbol rate", "ksym/s", 0.0)
        .range(0.0, 2000.0, 0.1)
        .zero_means("660 ksym/s")
        .restart();
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Newest", "", true);
/// The symbol rate of the test signal.
pub const GEN_SYMBOL_RATE: Setting<field::Hz> =
    Setting::new("gen_symbol_rate", "Test symbol rate", "Hz", SYMBOL_RATE)
        .range(100e3, 2e6, 1.0)
        .section(TEST)
        .advanced();
/// The first line counter of the test signal.
pub const GEN_FIRST_LINE: Setting<field::I64> =
    Setting::new("gen_first_line", "Test first line", "", 0)
        .range(0.0, 2499.0, 1.0)
        .section(TEST)
        .advanced();
/// The lines of the test signal: a pass of that many lines, then the signal ends. 0: the test picture of
/// [`TEST_PICTURE_LINES`] lines repeats without end.
pub const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Test lines", "", 0)
    .range(1.0, 2500.0, 1.0)
    .zero_means("Without end")
    .help("The test signal sends this many lines of the test picture and then ends. Without end: the picture repeats.")
    .section(TEST)
    .advanced();
/// The lines of the test signal before they repeat.
pub const TEST_PICTURE_LINES: u32 = 8;

sdrkit_core::settings! {
    /// The settings of `fy2_svissr`, with the options of its test signal.
    pub enum Fy2Setting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE_SETTING,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// The end of a pass.
        PassGap = PASS_GAP,
        /// Test: the symbol rate.
        GenSymbolRate = GEN_SYMBOL_RATE,
        /// Test: the first line.
        GenFirstLine = GEN_FIRST_LINE,
        /// Test: the lines.
        GenLines = GEN_LINES,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Fy2Display`].
    pub enum Fy2DisplaySetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE_SETTING,
    }
}

/// The S-VISSR decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SvissrReport {
    /// Frames drawn (lines).
    pub lines: u64,
    /// Sync windows found.
    pub syncs: u64,
    /// Frames refused.
    pub rejected: u64,
    /// Frames under 2 % preamble errors.
    pub usable: u64,
    /// The last frame's preamble bit error ratio.
    pub preamble_ber: f64,
    /// The data bit error estimate (the spare area's set bits).
    pub data_ber: f64,
    /// The last line counter.
    pub last_counter: Option<u16>,
    /// Pictures started.
    pub pictures: u32,
    /// Collecting a frame now.
    pub collecting: bool,
    /// Seconds since the last frame drawn.
    pub since_frame_s: f64,
    /// New log lines.
    pub log: Vec<String>,
    /// Frames imported.
    pub imported: u64,
    /// Lines of the picture in progress.
    pub picture_lines: u64,
    /// The signal time at which the picture in progress started.
    pub picture_start_s: f64,
}
impl Item for SvissrReport {
    const KIND: &'static str = "satimg.svissr_report";
}

sdrkit_core::commands! {
    /// The commands of [`SvissrComponent`].
    pub enum SvissrCommand {
        /// Imports a file of clear frames.
        OpenSvissr = OPEN_SVISSR.command,
    }
}

/// The S-VISSR decoder: LLRs in (hard decisions), NRZ-M, the frame finder, pictures out as strips. A file of clear frames comes in
/// through the command `open_svissr`.
pub struct SvissrComponent {
    framer: SvissrFramer,
    bits: Vec<u8>,
    prev: u8,
    report: SvissrReport,
    picture: u32,
    last_counter: Option<u16>,
    samples_bits: u64,
    bit_rate: f64,
    last_frame_bit: u64,
    import: Vec<u8>,
    frames: Vec<crate::instruments::svissr::FoundFrame>,
    strips: Vec<Strip>,
    storage: Arc<dyn Storage>,
    pass: PassTimer,
}

impl Default for SvissrComponent {
    fn default() -> Self {
        SvissrComponent {
            framer: SvissrFramer::default(),
            bits: Vec::new(),
            prev: 0,
            report: SvissrReport::default(),
            picture: 0,
            last_counter: None,
            samples_bits: 0,
            bit_rate: SYMBOL_RATE,
            last_frame_bit: 0,
            import: Vec::new(),
            frames: Vec::new(),
            strips: Vec::new(),
            storage: files::in_memory(),
            pass: PassTimer::default(),
        }
    }
}

impl SvissrComponent {
    /// Returns the bytes of clear frames (a `.svissr` file the host read) to decode at the next input.
    pub fn import(&mut self, bytes: Vec<u8>) {
        self.import = bytes;
    }

    fn frame(&mut self, bytes: &[u8], ber: f64, out: &mut Vec<Strip>) {
        let l = parse(bytes);
        self.report.preamble_ber = ber;
        self.report.data_ber = l.spare_ones as f64 / 1432.0;
        if l.counter >= MAX_LINES as u16 || ber > 0.42 {
            self.report.rejected += 1;
            return;
        }
        if ber < 0.02 {
            self.report.usable += 1;
        }
        if ber >= 0.25 {
            return;
        }
        // A new picture: the first frame, the counter falling by more than 100, or 300 s without a frame and the counter back.
        let since = (self.samples_bits - self.last_frame_bit) as f64 / self.bit_rate;
        let new = match self.last_counter {
            None => true,
            Some(c) => (l.counter as i32) < c as i32 - 100 || (since > 300.0 && l.counter < c),
        };
        if new {
            self.picture += 1;
            self.report.pictures += 1;
            self.report.picture_lines = 0;
            self.report.picture_start_s = self.samples_bits as f64 / self.bit_rate;
            self.report.log.push(if self.picture == 1 {
                "S-VISSR: frame sync found".into()
            } else {
                "S-VISSR: a new picture starts".into()
            });
        }
        self.last_counter = Some(l.counter);
        self.last_frame_bit = self.samples_bits;
        self.report.last_counter = Some(l.counter);
        self.report.lines += 1;
        self.report.picture_lines += 1;
        let row = l.counter as u32;
        for k in 0..3 {
            out.push(strip(
                self.picture,
                k as u16 + 1,
                row,
                1,
                IR_SAMPLES as u32,
                l.ir[k].clone(),
                "IR",
            ));
        }
        out.push(strip(
            self.picture,
            4,
            row,
            1,
            IR_SAMPLES as u32,
            l.ir4.iter().map(|&v| (v >> 2) as u8).collect(),
            "IR",
        ));
        // VIS: two rows of 4580, each pixel the rounded mean of a 2 x 2 block of the four lines (shown as value x 4 + 2).
        let mut vis = vec![0u8; 2 * 4580];
        for r in 0..2 {
            for x in 0..4580 {
                let s: u32 = [
                    l.vis[2 * r][2 * x],
                    l.vis[2 * r][2 * x + 1],
                    l.vis[2 * r + 1][2 * x],
                    l.vis[2 * r + 1][2 * x + 1],
                ]
                .iter()
                .map(|&v| v as u32 * 4 + 2)
                .sum();
                vis[r * 4580 + x] = ((s + 2) / 4).min(255) as u8;
            }
        }
        out.push(strip(self.picture, 5, row * 2, 2, 4580, vis, "VIS"));
    }
}

fn strip(
    picture: u32,
    ch: u16,
    row: u32,
    rows: u16,
    w: u32,
    px: Vec<u8>,
    name: &'static str,
) -> Strip {
    Strip {
        picture,
        channel: ch,
        first_row: row,
        x: 0,
        rows,
        width: w,
        samples: Arc::from(px),
        mask: 1,
        onboard_s: None,
        instrument: name,
    }
}

impl Component for SvissrComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.svissr",
            vec![Port::llr("in")],
            vec![
                item_port::<Strip>("out"),
                item_port::<SvissrReport>("report"),
                pass_port(),
            ],
        )
        .with_settings(PassSetting::schema())
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PassSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PassSetting::PassGap => self.pass.apply(id, v),
        }
    }
    fn values(&self) -> Values {
        values_of([(PassSetting::PassGap, Value::F64(self.pass.gap_s()))])
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn commands(&self) -> &Commands {
        SvissrCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SvissrCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SvissrCommand::OpenSvissr => {
                let a: FileArgs = OPEN_SVISSR.args(args)?;
                let bytes = files::read(self.storage.as_ref(), args, a.path)?;
                self.import(bytes);
            }
        }
        Ok(None)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.bit_rate = inputs[0].rate.max(1.0);
        self.framer = SvissrFramer::default();
        Ok(vec![
            item_format::<Strip>(),
            item_format::<SvissrReport>(),
            pass_format(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        // NRZ-M: a 1 is a change of level, so the polarity of the carrier does not matter.
        let llrs = io.llrs(0);
        self.bits.clear();
        for &s in llrs {
            let b = (s < 0) as u8;
            self.bits.push(b ^ self.prev);
            self.prev = b;
        }
        self.samples_bits += llrs.len() as u64;
        let mut frames = std::mem::take(&mut self.frames);
        let mut strips = std::mem::take(&mut self.strips);
        self.framer.process(&self.bits, &mut frames);
        for f in frames.drain(..) {
            self.frame(&f.bytes, f.preamble_ber, &mut strips);
        }
        self.frames = frames;
        if !self.import.is_empty() {
            let data = std::mem::take(&mut self.import);
            for f in data.as_chunks::<FRAME_BYTES>().0 {
                self.report.imported += 1;
                self.frame(f, 0.0, &mut strips);
            }
        }
        // At the end of a pass the next frame starts a new picture.
        if self.pass.after(io, !strips.is_empty(), 2) {
            self.last_counter = None;
        }
        io.items_out::<Strip>(0)
            .extend(strips.drain(..).map(|s| Timed {
                time: 0.0,
                value: s,
            }));
        self.strips = strips;
        if io.tick() {
            self.report.syncs = self.framer.syncs;
            self.report.collecting = self.framer.collecting();
            self.report.since_frame_s =
                (self.samples_bits - self.last_frame_bit) as f64 / self.bit_rate;
            let r = self.report.clone();
            self.report.log.clear();
            io.items_out::<SvissrReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
}

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(LinkFields::SHAPE.fields)
            .with_fields(Fy2SvissrFields::SHAPE.fields)
            .with_table(&IMAGES),
    )
}

/// The FY-2 display.
pub struct Fy2Display {
    published: Published,
    hist: Histories,
    user_rs: f64,
    summary: String,
    hint: String,
    key: String,
}

impl Default for Fy2Display {
    fn default() -> Self {
        Fy2Display {
            published: super::display::published(display_declaration()),
            hist: Histories::default(),
            user_rs: 0.0,
            summary: String::new(),
            hint: String::new(),
            key: String::new(),
        }
    }
}

/// The channel names of the pictures of a scan.
const CHANNELS: [&str; 5] = ["IR1", "IR2", "IR3", "IR4", "VIS"];

impl Component for Fy2Display {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.fy2_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<SvissrReport>("svissr"),
            ],
            vec![item_port::<Control>("control")],
        )
        .with_settings(Fy2DisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Control>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let none = SvissrReport::default();
        let s = last::<SvissrReport>(io, 2).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        let locked = d.is_some_and(|d| d.state.tracking && d.state.locked);
        let state = if s.lines > 0 && s.since_frame_s < 1.5 {
            3
        } else if s.collecting || (s.syncs > s.rejected && s.syncs > 0) {
            2
        } else if locked {
            1
        } else {
            0
        };
        let p = &mut self.published;
        log_lines(p.results_mut(), &s.log);
        let es = d.map_or(-10.0, |d| d.state.es_n0_db);
        let products = s.pictures * 5;
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = write!(self.summary, "FY-2 S-VISSR: {} lines, line ", s.lines);
        // A write to a String cannot fail.
        let _ = match s.last_counter {
            Some(c) => write!(self.summary, "{c}"),
            None => write!(self.summary, "-"),
        };
        // A write to a String cannot fail.
        let _ = write!(
            self.summary,
            ", {products} products, BER {:.1e}, Es/N0 {es:.1} dB",
            s.preamble_ber
        );
        let rs = if self.user_rs > 0.0 {
            self.user_rs * 1e3
        } else {
            SYMBOL_RATE
        };
        let rate = f.map_or(0.0, |f| f.input_rate);
        self.hint.clear();
        // A write to a String cannot fail.
        let _ = if rate < 1.4 * rs {
            write!(
                self.hint,
                "The sample rate is too low for S-VISSR: use at least {:.1} Msps.",
                1.5 * rs / 1e6
            )
        } else if state == 0 {
            write!(self.hint, "No carrier yet. Point a dish (about 1 m or more) at the satellite, check the LNA and filter power and the 1.7 GHz band, and tune to the frequency of the link.")
        } else if state == 1 {
            write!(self.hint, "Carrier found but no frame sync: the signal is too weak (Es/N0 below about 3 dB), or this is not S-VISSR (660 ksym/s BPSK). Is the dish too small or pointed wrongly?")
        } else if s.preamble_ber > 0.12 {
            write!(self.hint, "Frames are found but the bit error ratio is high (the pictures will be noisy): more dish gain, a better LNA or a dish pointed again will help.")
        } else {
            Ok(())
        };
        put_link(p, t, f, d, None, None, None);
        status(p.results_mut(), state, f, d, &self.summary);
        p.put(&Fy2SvissrFields {
            lines: s.lines as i64,
            syncs: s.syncs as i64,
            rejected: s.rejected as i64,
            usable: s.usable as i64,
            preamble_ber: s.preamble_ber,
            data_ber: s.data_ber,
            line_counter: s.last_counter.map(i64::from),
            pictures_started: i64::from(s.pictures),
            imported: s.imported as i64,
            hint: &self.hint,
        });
        if s.pictures > 0 {
            for ch in 1..=5usize {
                let (rows, width) = if ch == 5 {
                    (2 * s.picture_lines, 4580)
                } else {
                    (s.picture_lines, IR_SAMPLES as i64)
                };
                self.key.clear();
                // A write to a String cannot fail.
                let _ = write!(self.key, "{}-{ch}", s.pictures);
                IMAGES.put(
                    p,
                    &self.key,
                    &ImageRow {
                        product: NAMES[ch - 1],
                        channel: Some(CHANNELS[ch - 1]),
                        time: Some(Seconds(s.picture_start_s)),
                        width,
                        height: Some(rows as i64),
                    },
                );
            }
        }
        self.hist.publish(self.published.results_mut(), d, None);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match Fy2DisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Fy2DisplaySetting::SymbolRate => self.user_rs = SYMBOL_RATE_SETTING.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(Fy2DisplaySetting::SymbolRate, Value::F64(self.user_rs))])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

/// The picture layouts: the four infrared channels and the visible one.
pub fn picture_layouts() -> Vec<PictureLayout> {
    let ir_views: Vec<NamedView> = (1..=4u16)
        .map(|c| {
            NamedView::new(
                format!("ch{c}"),
                NAMES[c as usize - 1],
                View::Grey {
                    channel: c,
                    invert: false,
                },
            )
        })
        .collect();
    vec![
        PictureLayout {
            name: "IR",
            width: IR_SAMPLES as u32,
            max_rows: MAX_LINES,
            full_mask: 1,
            views: ir_views,
        },
        PictureLayout {
            name: "VIS",
            width: 4580,
            max_rows: 2 * MAX_LINES,
            full_mask: 1,
            views: vec![NamedView::new(
                "ch5",
                NAMES[4],
                View::Grey {
                    channel: 5,
                    invert: false,
                },
            )],
        },
    ]
}

/// The FY-2 graph at symbol rate `rs`.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts give no valid schema.
pub fn builder(rs: f64) -> Result<GraphBuilder, GraphError> {
    let pictures = PassPictures::new(picture_layouts(), "FY-2").map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(link(rs), 1.4 * rs * 0.86, false),
        )
        .add("demod", DemodComponent::new(link(rs)))
        .add("svissr", SvissrComponent::default())
        .add("pictures", pictures)
        .add("display", Fy2Display::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "svissr.in")
        .connect("svissr.out", "pictures.in")
        .connect("svissr.pass", "pictures.pass")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("svissr.report", "display.svissr")
        .feedback("display.control", "demod.control")
        .output("pictures.picture")
        .status_from("display")
        .clock(REPORT_S))
}

/// The FY-2 receiver: the graph with the stack's settings. Below the least rate the front end measures the level only and the
/// display shows the hint.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let k = cfg.settings.get(&SYMBOL_RATE_SETTING);
    let rs = if k > 0.0 { k * 1e3 } else { SYMBOL_RATE };
    let mut g = builder(rs)?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .route("symbol_rate", &["display.symbol_rate"])?
        .route("follow", &["pictures.follow"])?
        .route("pass_gap", &["svissr.pass_gap"])?
        .command(
            "open_svissr",
            sdrkit_graph::CommandTarget::of("svissr", "open_svissr"),
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}
fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        Fy2Generator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "FY-2 S-VISSR",
    group: crate::GROUP,
    blurb: "FengYun-2 stretched VISSR on 1687.5 MHz: full-disk infrared and visible pictures, line by line.",
    tuning: Tuning { min_hz: 1670e6, max_hz: 1710e6, ..Tuning::anywhere(1687.5e6, 2.4e6, 1.0e6, 1.0e6) },
    submodes: &[],
    settings: Fy2Setting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::SVissr],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::FY2_HOOKS),
    benchmarks: None,
};

/// The test scan line `c` (counter) of the test picture.
pub fn test_line(c: u16) -> SvissrLine {
    let y = c as u32;
    SvissrLine {
        counter: c,
        ir: std::array::from_fn(|k| {
            (0..IR_SAMPLES)
                .map(|x| test_pixel(k as u16 + 1, x as u32, y * 4, IR_SAMPLES as u32))
                .collect()
        }),
        ir4: (0..IR_SAMPLES)
            .map(|x| {
                ((test_pixel(4, x as u32, y * 4, IR_SAMPLES as u32) as u16) << 2) | (x as u16 & 3)
            })
            .collect(),
        vis: std::array::from_fn(|j| {
            (0..VIS_SAMPLES)
                .map(|x| test_pixel(5, x as u32 / 2, y * 4 + j as u32, 4580) >> 2)
                .collect()
        }),
        spare_ones: 0,
    }
}

/// Channel bits of the test signal: preamble, scrambled frame, NRZ-M.
pub struct Fy2Source {
    first: u16,
    lines: u16,
    k: u16,
    level: u8,
    /// The lines sent.
    sent: u64,
    /// The lines of the pass, `None` for a signal without end.
    pass: Option<u64>,
}

impl BitSource for Fy2Source {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let c = self.first + self.k;
        self.k = (self.k + 1) % self.lines.max(1);
        self.sent += 1;
        let mut f = build(&test_line(c));
        scramble(&mut f);
        let pn = pn_bits();
        let push = |b: u8, level: &mut u8, out: &mut Vec<u8>| {
            *level ^= b;
            out.push(*level);
        };
        for &b in &pn[..PREAMBLE] {
            push(b, &mut self.level, out);
        }
        for byte in f {
            for i in (0..8).rev() {
                push((byte >> i) & 1, &mut self.level, out);
            }
        }
    }
    fn ended(&self) -> bool {
        self.pass.is_some_and(|p| self.sent >= p)
    }
}

/// The FY-2 test signal: the clean signal at unit mean power.
pub struct Fy2Generator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<Fy2Source>,
}

impl Fy2Generator {
    /// From a configuration: `gen_symbol_rate`, `gen_first_line` and `gen_lines`. The schema of the settings must be
    /// [`Fy2Setting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the options.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let v = &cfg.settings;
        let rs = v.get(&GEN_SYMBOL_RATE);
        let first = v.get(&GEN_FIRST_LINE).clamp(0, 2499) as u16;
        // A pass of a stated number of lines sends each line once.
        let pass = pass_lines(v.get(&GEN_LINES));
        let lines = pass.map_or(TEST_PICTURE_LINES, |p| p.min(2500) as u32) as u16;
        let modem = ModParams::new(cfg.rate_hz, rs, 0.5, Modulation::Bpsk);
        let src = Fy2Source {
            first,
            lines,
            k: 0,
            level: 0,
            sent: 0,
            pass,
        };
        Ok(Fy2Generator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for Fy2Generator {
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

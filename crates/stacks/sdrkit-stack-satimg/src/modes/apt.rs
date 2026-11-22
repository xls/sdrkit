//! NOAA APT (`noaa_apt`): analogue pictures on 137 MHz, an FM carrier with a 2400 Hz subcarrier whose amplitude is the
//! brightness.
//!
//! Standard: NOAA KLM User's Guide, section 4.2 (APT) and Table 1.2.5-1.

use super::display::{
    instrument_choices, published,
};
use super::notes::set_status;
use crate::components::instruments::InstrumentCommand;
use crate::components::last;
use crate::components::link::{item_format, item_port, stage, FrontEndComponent, FrontReport};
use crate::gen::{test_value, GenError};
use crate::instruments::apt::{
    amplitude, burst_amplitude, sync_a, test_word, AptDecoder, AptLine, LineFilter, DEVIATION_HZ,
    LINE_WORDS, OVERSAMPLE, PICTURE_A, PICTURE_B, PICTURE_WORDS, SUBCARRIER_HZ, SYNC_WORDS,
    WORD_RATE,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::{design_by_taps, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::loopfilter::PhaseAcc;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::modulate::Fm;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::{Strip, View};
use sdrkit_core::command::{CommandError, CommandId, CommandSet as _, Commands};
use crate::typed::{ImageRow, NoaaAptFields, IMAGES};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Record, Results,
    ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Item, Port, StackGraph, Timed,
};
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "noaa_apt";
/// The least input rate.
pub const MIN_RATE: f64 = 60e3;
/// The most rows of a picture.
pub const MAX_ROWS: u32 = 8000;
/// The subcarrier level (full deviation is 1) above which the stack reports a subcarrier.
const SUBCARRIER_FOUND: f32 = 0.02;

/// The front end keeps 112 ksps or more, flat to 28 kHz from the centre. The signal has sidebands of weight to 22 kHz from
/// its carrier (Carson's rule gives 19.4 kHz), and the carrier is up to 3 kHz from the centre.
fn front_link() -> LinkParams {
    LinkParams {
        modulation: Modulation::Bpsk,
        symbol_rate: 28_000.0,
        roll_off: 1.0,
        class: LinkClass::Leo,
        search: Search::PowerLine {
            power: 2,
            min_ratio: 0.0,
        },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: false,
        constellation: 0,
        interleaved: false,
    }
}

const TEST: &str = "Test signal";

/// The picture shown: the full line or one of its two channels.
pub const PICTURE: CatalogueSetting = CatalogueSetting {
    key: "picture",
    label: "Picture",
    help: "The full line, or one of the two channels of the line.",
    choices: || instrument_choices(layouts()),
    default: "APT",
    advanced: false,
    role: None,
};
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow", "", true);
/// The line filter of pictures A and B.
pub const LINE_FILTER: Setting<field::Bool> = Setting::new("line_filter", "Line filter", "", true)
    .help("Removes clicks and smooths noise along each line of both pictures, with the noise measured in space A.");
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
    /// The settings of [`AptComponent`].
    pub enum AptLineSetting {
        /// The line filter.
        LineFilter = LINE_FILTER,
    }
}

sdrkit_core::settings! {
    /// The settings of `noaa_apt`, with the options of its test signal.
    pub enum AptSetting {
        /// The picture shown.
        Picture = PICTURE,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// The line filter.
        LineFilter = LINE_FILTER,
        /// Test: picture lines.
        GenLines = GEN_LINES,
    }
}

/// The instrument name of the full line.
pub const FULL: &str = "APT";
/// The instrument name of channel A.
pub const CHANNEL_A: &str = "APT channel A";
/// The instrument name of channel B.
pub const CHANNEL_B: &str = "APT channel B";

/// The picture layouts: the full line of 2080 words and the two channels of 909 words.
pub fn layouts() -> Vec<PictureLayout> {
    let one = |name, width| PictureLayout {
        name,
        width,
        max_rows: MAX_ROWS,
        full_mask: 1,
        views: vec![NamedView::new(
            "grey",
            "Grey",
            View::Grey {
                channel: 1,
                invert: false,
            },
        )],
    };
    vec![
        one(FULL, LINE_WORDS as u32),
        one(CHANNEL_A, PICTURE_WORDS as u32),
        one(CHANNEL_B, PICTURE_WORDS as u32),
    ]
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The decoder component.

/// The APT decoder's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AptReport {
    /// The line lock holds.
    pub locked: bool,
    /// Lines of the picture.
    pub rows: u32,
    /// Lines decoded since the start.
    pub lines: u64,
    /// Lines taken at the expected place because their sync was not found.
    pub missed: u64,
    /// Line locks so far.
    pub locks: u32,
    /// Line locks lost so far.
    pub losses: u32,
    /// The mean of the frequency of the carrier against the tuned frequency, Hz.
    pub offset_hz: f64,
    /// The mean subcarrier amplitude (full deviation is 1).
    pub subcarrier: f32,
    /// The correlation of the last sync A with its pattern.
    pub quality: f32,
    /// The line clock of the signal against the sample clock, ppm.
    pub clock_ppm: f64,
}
impl Item for AptReport {
    const KIND: &'static str = "satimg.apt_report";
}

/// The lines of the picture and its row counter.
#[derive(Clone, Debug)]
struct Lines {
    dec: AptDecoder,
    rows: u32,
}

impl Lines {
    fn new_picture(&mut self) {
        self.rows = 0;
    }
}

/// The [`burst_amplitude`] of a sync A of height 1 behind a low-pass with `taps` at `rate`.
fn burst_share(taps: &[f32], rate: f64) -> f32 {
    let per_word = rate / WORD_RATE;
    let pattern = sync_a();
    let level = |i: usize| {
        let w = (i as f64 / per_word) as usize;
        f32::from(u8::from(w < SYNC_WORDS && pattern[w]))
    };
    let delay = (taps.len() - 1) / 2;
    // The filter output for the input sample nearest to the centre of word `w`.
    let at = |w: usize| {
        let c = ((w as f64 + 0.5) * per_word).round() as usize + delay;
        taps.iter()
            .enumerate()
            .filter(|(k, _)| c >= *k)
            .map(|(k, t)| t * level(c - k))
            .sum::<f32>()
    };
    let words: Vec<f32> = (0..SYNC_WORDS).map(at).collect();
    burst_amplitude(&words).clamp(0.05, 1.0)
}

/// The APT receiver after the front end: channel filter, FM demodulator, subcarrier envelope, resampler to the word rate and
/// the line decoder. One strip of each of the three pictures leaves for each line.
pub struct AptComponent {
    rate: f64,
    channel: FirDecimator<Complex32>,
    filtered: Vec<Complex32>,
    fm: Option<Discriminator>,
    audio: FirDecimator<f32>,
    osc: PhaseAcc,
    envelope: Fir<Complex32>,
    prev: f32,
    /// The place of the next value for the decoder between the last two envelope samples, in samples.
    pos: f64,
    step: f64,
    offset: f32,
    level: f32,
    words: Vec<f32>,
    found: Vec<AptLine>,
    dec: Lines,
    lines: u64,
    missed: u64,
    quality: f32,
    strips: Vec<Strip>,
    filter: LineFilter,
    filter_on: bool,
    /// The words of the line being cut into strips.
    line_words: Vec<u8>,
}

impl Default for AptComponent {
    fn default() -> Self {
        AptComponent {
            rate: 0.0,
            channel: FirDecimator::new(&[1.0], 1),
            filtered: Vec::new(),
            fm: None,
            audio: FirDecimator::new(&[1.0], 1),
            osc: PhaseAcc::new(0.0),
            envelope: Fir::new(&[1.0]),
            prev: 0.0,
            pos: 0.0,
            step: 1.0,
            offset: 0.0,
            level: 0.0,
            words: Vec::new(),
            found: Vec::new(),
            dec: Lines {
                dec: AptDecoder::new(1.0),
                rows: 0,
            },
            lines: 0,
            missed: 0,
            quality: 0.0,
            strips: Vec::new(),
            filter: LineFilter::new(),
            filter_on: true,
            line_words: Vec::new(),
        }
    }
}

impl AptComponent {
    fn line(&mut self, l: &AptLine) {
        self.lines += 1;
        self.missed += u64::from(l.missed);
        self.quality = l.quality;
        let row = self.dec.rows;
        self.dec.rows += 1;
        let mut words = std::mem::take(&mut self.line_words);
        words.clear();
        words.extend_from_slice(&l.words);
        if self.filter_on {
            self.filter.filter(&mut words);
        }
        let cut = |from: usize, n: usize| -> Arc<[u8]> { Arc::from(&words[from..from + n]) };
        let strip = |name, samples: Arc<[u8]>| {
            let mut s = Strip::new(1, row, 0, 1, samples.len() as u32, samples);
            s.instrument = name;
            s
        };
        self.strips.push(strip(FULL, cut(0, LINE_WORDS)));
        self.strips
            .push(strip(CHANNEL_A, cut(PICTURE_A, PICTURE_WORDS)));
        self.strips
            .push(strip(CHANNEL_B, cut(PICTURE_B, PICTURE_WORDS)));
        self.line_words = words;
    }
}

impl Component for AptComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.apt",
            vec![Port::iq("in")],
            vec![item_port::<Strip>("out"), item_port::<AptReport>("report")],
        )
        .with_settings(AptLineSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        // Channel filter: 28 kHz to each side, at 64 ksps or more.
        let factor = ((rate / 64_000.0) as usize).max(1);
        let n = ((3.3 * rate / 6000.0) as usize).clamp(9, 601) | 1;
        let taps = design_by_taps(n, (28_000.0 / rate).min(0.5), Window::Hamming).map_err(stage)?;
        self.channel = FirDecimator::new(&taps, factor);
        let rate = rate / factor as f64;
        // The audio: the subcarrier with its sidebands ends at 4480 Hz. 16 ksps or more are kept.
        let factor = ((rate / 16_000.0) as usize).max(1);
        self.audio = FirDecimator::design(rate, factor, 4600.0, 4600.0, 60.0).map_err(stage)?;
        self.fm = Some(Discriminator::new(rate, DEVIATION_HZ).map_err(stage)?);
        let rate = rate / factor as f64;
        self.rate = rate;
        self.osc = PhaseAcc::new(SUBCARRIER_HZ / rate);
        // Envelope filter: passes the picture (to half the word rate) and stops the image at twice the subcarrier.
        let n = ((3.3 * rate / 1200.0) as usize).clamp(9, 801) | 1;
        let taps = design_by_taps(n, (2200.0 / rate).min(0.5), Window::Hamming).map_err(stage)?;
        self.envelope = Fir::new(&taps);
        self.dec.dec = AptDecoder::new(burst_share(&taps, rate));
        self.step = rate / (WORD_RATE * OVERSAMPLE as f64);
        self.pos = 0.0;
        self.prev = 0.0;
        self.level = 0.0;
        self.offset = 0.0;
        self.words.clear();
        self.filter.reset();
        Ok(vec![item_format::<Strip>(), item_format::<AptReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(fm) = &mut self.fm else { return };
        self.words.clear();
        self.filtered.clear();
        self.channel.process(io.iq(0), &mut self.filtered);
        for &x in &self.filtered {
            let Some(audio) = self.audio.push(fm.process_sample(x)) else {
                continue;
            };
            // A carrier offset is a DC term of the audio. It is taken off before the subcarrier is mixed down.
            self.offset += 0.004 * (audio - self.offset);
            let mixed = self.osc.next_phasor().conj() * (audio - self.offset);
            let env = 2.0 * self.envelope.process_sample(mixed).norm();
            self.level += 0.008 * (env - self.level);
            while self.pos < 1.0 {
                self.words
                    .push(self.prev + self.pos as f32 * (env - self.prev));
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.prev = env;
        }
        self.found.clear();
        self.dec.dec.push(&self.words, &mut self.found);
        let found = std::mem::take(&mut self.found);
        for l in &found {
            self.line(l);
        }
        self.found = found;
        io.items_out::<Strip>(0)
            .extend(self.strips.drain(..).map(|s| Timed {
                time: 0.0,
                value: s,
            }));
        if io.tick() {
            let r = AptReport {
                locked: self.dec.dec.locked(),
                rows: self.dec.rows,
                lines: self.lines,
                missed: self.missed,
                locks: self.dec.dec.locks,
                losses: self.dec.dec.losses,
                offset_hz: f64::from(self.offset) * DEVIATION_HZ,
                subcarrier: self.level,
                quality: self.quality,
                clock_ppm: self.dec.dec.clock_ppm(),
            };
            io.items_out::<AptReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AptLineSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AptLineSetting::LineFilter => self.filter_on = LINE_FILTER.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(AptLineSetting::LineFilter, Value::Bool(self.filter_on))])
    }
    fn commands(&self) -> &Commands {
        InstrumentCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match InstrumentCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            InstrumentCommand::Clear => self.dec.new_picture(),
        }
        self.filter.reset();
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(NoaaAptFields::SHAPE.fields)
        .with_table(&IMAGES)
}

/// The display: state, fields and summary.
pub struct AptDisplay {
    published: Published,
    was_locked: bool,
    first_line_s: Option<f64>,
    summary: String,
}

impl Default for AptDisplay {
    fn default() -> Self {
        AptDisplay {
            published: published(display_declaration()),
            was_locked: false,
            first_line_s: None,
            summary: String::new(),
        }
    }
}

impl Component for AptDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.apt_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<AptReport>("apt"),
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
        let a = last::<AptReport>(io, 1);
        let t = f.map_or(0.0, |f| f.time_s);
        let none = AptReport::default();
        let a = a.unwrap_or(&none);
        let low = f.is_some_and(|f| f.rate_too_low);
        let state = if a.locked {
            3
        } else if a.subcarrier > SUBCARRIER_FOUND && !low {
            1
        } else {
            0
        };
        if a.locked != self.was_locked {
            self.was_locked = a.locked;
            self.summary.clear();
            // A write to a String cannot fail.
            let _ = if a.locked {
                write!(self.summary, "APT lines locked at line {}", a.lines)
            } else {
                write!(self.summary, "APT lock lost at line {}", a.lines)
            };
            self.published.results_mut().log(self.summary.as_str());
        }
        if a.rows > 0 && self.first_line_s.is_none() {
            self.first_line_s = Some(t);
        }
        if a.rows == 0 {
            self.first_line_s = None;
        }
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(self.summary, "NOAA APT, {} lines", a.rows),
            1 => write!(self.summary, "NOAA APT, subcarrier found, no line sync"),
            _ if low => write!(
                self.summary,
                "NOAA APT needs at least {} kHz of samples",
                MIN_RATE / 1e3
            ),
            _ => write!(self.summary, "NOAA APT, searching"),
        };
        let p = &mut self.published;
        let offset = a.offset_hz + f.map_or(0.0, |f| f.user_offset_hz);
        set_status(
            p.results_mut(),
            state,
            None,
            f.map(|f| f.level_dbfs),
            Some(offset),
            &self.summary,
        );
        p.put(&NoaaAptFields {
            level_dbfs: f.map(|f| f64::from(f.level_dbfs)),
            offset_hz: Hertz(a.offset_hz),
            clock_ppm: a.clock_ppm,
            signal_time: Seconds(t),
            input_rate_hz: f.map(|f| Hertz(f.input_rate)),
            locked: a.locked,
            lines: a.lines as i64,
            lines_missed: a.missed as i64,
            subcarrier_level: f64::from(a.subcarrier),
            sync_quality: f64::from(a.quality),
            word_rate_hz: Hertz(WORD_RATE),
            note: "Not decoded: the telemetry wedges and the minute markers.",
        });
        if a.rows > 0 {
            for (name, width) in [
                (FULL, LINE_WORDS),
                (CHANNEL_A, PICTURE_WORDS),
                (CHANNEL_B, PICTURE_WORDS),
            ] {
                IMAGES.put(
                    p,
                    name,
                    &ImageRow {
                        product: name,
                        channel: None,
                        time: self.first_line_s.map(Seconds),
                        width: width as i64,
                        height: Some(i64::from(a.rows)),
                    },
                );
            }
        }
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

/// The APT graph.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts produce no valid schema.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let pictures = Pictures::new(layouts()).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(front_link(), MIN_RATE, false),
        )
        .add("apt", AptComponent::default())
        .add("pictures", pictures)
        .add("display", AptDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "apt.in")
        .connect("apt.out", "pictures.in")
        .connect("frontend.report", "display.front")
        .connect("apt.report", "display.apt")
        .output("pictures.picture")
        .status_from("display")
        .clock(REPORT_S))
}

/// The APT receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "NOAA APT needs at least 60 kHz of samples")
        .route("picture", &["pictures.instrument"])?
        .route("follow", &["pictures.follow"])?
        .route("line_filter", &["apt.line_filter"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        AptGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "NOAA APT",
    group: crate::GROUP,
    blurb: "Analogue APT pictures of the NOAA KLM satellites on 137 MHz: FM with a 2400 Hz subcarrier, two picture channels a line.",
    tuning: Tuning {
        min_hz: 136e6,
        max_hz: 138e6,
        // The default frequency is the one of the test signal. The host gives the frequency of a session.
        ..Tuning::anywhere(137.1e6, 1.024e6, MIN_RATE, 50e3)
    },
    submodes: &[],
    settings: AptSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: Some(crate::bench::benchmarks),
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The APT test signal: the test picture in both channels, through the subcarrier and the FM modulator, at unit power.
pub struct AptGenerator {
    rate: f64,
    /// The samples of the pass, after which the signal ends. `None` for a signal without end.
    samples: Option<u64>,
    n: u64,
    phase: PhaseAcc,
    fm: Fm,
    audio: Vec<f32>,
}

impl AptGenerator {
    /// From a configuration: `gen_lines`, the lines of the pass (0: without end). The schema of the settings must be
    /// [`AptSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the rate is below 60 kHz or the modulator refuses it.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        if cfg.rate_hz < MIN_RATE {
            return Err(GenError::RateTooLow {
                need: MIN_RATE,
                got: cfg.rate_hz,
            });
        }
        let lines = cfg.settings.get(&GEN_LINES).clamp(0, 4000) as u64;
        let line_samples = cfg.rate_hz / WORD_RATE * LINE_WORDS as f64;
        Ok(AptGenerator {
            rate: cfg.rate_hz,
            samples: (lines > 0).then(|| (lines as f64 * line_samples).round() as u64),
            n: 0,
            phase: PhaseAcc::new(SUBCARRIER_HZ / cfg.rate_hz),
            fm: Fm::new(cfg.rate_hz, DEVIATION_HZ)?,
            audio: Vec::new(),
        })
    }
}

impl Generator for AptGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.audio.clear();
        let per_word = self.rate / WORD_RATE;
        // A pass of a stated number of lines ends: the rest of `out` stays unwritten.
        let left = self.samples.map_or(u64::MAX, |s| s.saturating_sub(self.n));
        let n = out.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        let out = &mut out[..n];
        for _ in 0..out.len() {
            let word = (self.n as f64 / per_word) as u64;
            let line = (word / LINE_WORDS as u64 % u64::from(TEST_PICTURE_LINES)) as u32;
            let level = test_word((word % LINE_WORDS as u64) as usize, line, test_value);
            self.audio
                .push(amplitude(level) * self.phase.next_phasor().im);
            self.n += 1;
        }
        if self.fm.process(&self.audio, out).is_err() {
            out.fill(Complex32::new(1.0, 0.0));
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(40e3).with_symbol_rate(WORD_RATE))
    }
}

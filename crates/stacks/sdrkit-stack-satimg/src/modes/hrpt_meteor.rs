//! Meteor HRPT, the submode `meteor` of the stack `hrpt`: the MSU-MR pictures of the Meteor-M satellites near 1.7 GHz,
//! 665 400 bit/s split-phase data that move the phase of the carrier, with a residual carrier.

use super::display::{published, with_series, Histories};
use super::hrpt::{
    HrptDisplaySetting, HrptPreset, HrptSetting, VariantChoice, FREQUENCY_PRESET, GEN_LINES,
    GEN_PM_INDEX, VARIANT_CHOICE,
};
use super::notes::set_status;
use crate::components::instruments::instrument_owner;
use crate::components::link::{
    control_port, item_format, item_port, DemodReport, FrontEndComponent, FrontReport,
};
use crate::components::pass::{pass_format, pass_port, PassPictures, PassSetting, PassTimer};
use crate::components::pm::PmDemodComponent;
use crate::components::{last, Control};
use crate::gen::{pass_lines, test_value, GenError};
use crate::instruments::msumr_hrpt::{
    payload, test_line, transport_frame, MsuHrptDecoder, MsuHrptStats, CHANNELS, FRAME_BYTES,
    INSTRUMENT, MAX_ROWS, PAYLOAD_BYTES, TRANSPORT, WIDTH,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::typed::{ImageRow, MeteorHrptFields, IMAGES};
use crate::REPORT_S;
use sdrkit_algo_sim::modulate::PhaseMod;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView};
use sdrkit_codec_image::strip::{Strip, View};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, Complex32, ConfigError, GenConfig, Generator, HostRequest, Results, ResultsDecl,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Item, Port, StackGraph, Timed,
};
use sdrkit_proto_ccsds::multisync::{SplitPhaseSync, StreamEvent};
use std::fmt::Write as _;

/// The submode id.
pub const SUBMODE: &str = "meteor";
/// The least input rate.
pub const MIN_RATE: f64 = 3e6;
/// Bits a second.
pub const BIT_RATE: f64 = 665_400.0;
/// Chips a second: two for each bit.
pub const CHIP_RATE: f64 = 2.0 * BIT_RATE;
/// The carrier search covers this many Hz to each side: more than the Doppler shift near 1.7 GHz.
const SEARCH_HZ: f64 = 150e3;
/// Without a frame lock the carrier is searched again after this many seconds.
const RESEARCH_S: f64 = 2.5;
/// The name the screens show.
const NAME: &str = "Meteor HRPT";

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

/// The picture layout of MSU-MR: 1572 pixels a line, 6 channels, the colour view and one grey view for each channel. The
/// first six views have the keys of the AVHRR views of the stack, so the `view` setting means the same view.
pub fn picture_layout() -> PictureLayout {
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
                invert: c >= 5,
            },
        ));
    }
    PictureLayout {
        name: INSTRUMENT,
        width: WIDTH as u32,
        max_rows: MAX_ROWS,
        full_mask: 1,
        views,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The frame and instrument component.

/// The report of [`MsuHrptComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MsuHrptReport {
    /// A confirmed transport frame lock is held.
    pub locked: bool,
    /// Which chip starts a bit in the lock: 0 or 1.
    pub pairing: Option<u8>,
    /// The stream of the lock is inverted.
    pub inverted: bool,
    /// Transport frames of confirmed locks.
    pub frames: u64,
    /// Sync words of transport frames found.
    pub sync_hits: u64,
    /// The MSU-MR counters.
    pub msu: MsuHrptStats,
    /// Seconds of signal since the last transport frame.
    pub since_frame_s: f64,
    /// New log lines.
    pub log: Vec<String>,
}
impl Item for MsuHrptReport {
    const KIND: &'static str = "satimg.msumr_hrpt_report";
}

/// Soft split-phase chips to transport frames to MSU-MR lines: the two chip pairings of [`SplitPhaseSync`], the payload
/// parts of each frame as one bit stream, and the MSU-MR decoder on that stream. The MSU-MR synchroniser starts again
/// when the transport lock ends.
pub struct MsuHrptComponent {
    sync: Option<SplitPhaseSync>,
    chips: u64,
    payload: Vec<u8>,
    count: u64,
    last_frame_chip: u64,
    dec: MsuHrptDecoder,
    strips: Vec<Strip>,
    pass: PassTimer,
    log: Vec<String>,
}

impl Default for MsuHrptComponent {
    fn default() -> Self {
        MsuHrptComponent {
            sync: SplitPhaseSync::new(TRANSPORT).ok(),
            chips: 0,
            payload: Vec::new(),
            count: 0,
            last_frame_chip: 0,
            dec: MsuHrptDecoder::default(),
            strips: Vec::new(),
            pass: PassTimer::default(),
            log: Vec::new(),
        }
    }
}

impl Component for MsuHrptComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.msumr_hrpt",
            vec![Port::llr("in"), control_port()],
            vec![
                item_port::<Strip>("out"),
                item_port::<MsuHrptReport>("report"),
                pass_port(),
            ],
        )
        .with_settings(PassSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(s) = &mut self.sync {
            s.drop_lock();
        }
        self.dec.restart();
        Ok(vec![
            item_format::<Strip>(),
            item_format::<MsuHrptReport>(),
            pass_format(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let chips = io.llrs(0);
        self.chips += chips.len() as u64;
        self.payload.clear();
        let mut frames = 0;
        if let Some(s) = &mut self.sync {
            let p = &mut self.payload;
            s.push_llrs(chips, |_, f| {
                payload(f.bytes, p);
                frames += 1;
            });
            for e in s.take_events() {
                match e {
                    StreamEvent::Chosen(w) => {
                        self.log.push(format!("frame sync (chip pairing {w})"))
                    }
                    StreamEvent::Lost => {
                        self.log.push("frame sync lost".into());
                        self.dec.restart();
                    }
                }
            }
        }
        if frames > 0 {
            self.count += frames;
            self.last_frame_chip = self.chips;
            self.dec.push(&self.payload, &mut self.strips);
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
            let r = MsuHrptReport {
                locked: s.is_some_and(|s| s.chosen().is_some()),
                pairing: s.and_then(SplitPhaseSync::chosen).map(|c| c as u8),
                inverted: s.is_some_and(SplitPhaseSync::inverted),
                frames: self.count,
                sync_hits: s.map_or(0, SplitPhaseSync::sync_hits),
                msu: self.dec.stats,
                since_frame_s: (self.chips - self.last_frame_chip) as f64 / CHIP_RATE,
                log: std::mem::take(&mut self.log),
            };
            io.items_out::<MsuHrptReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    instrument_owner!();
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

/// What no public document confirms of the link.
const METEOR_NOTE: &str = "Experimental: not confirmed on a real signal. Not confirmed by a public document: the transport \
    frame of 1024 bytes with its four payload parts, the MSU-MR frame of 11 850 bytes with its sync word and pixel layout, \
    the channel order, and that the link has no randomiser and no Reed-Solomon code. A public document gives only the \
    frequencies, the bit rate and the CCSDS sync marker. Not decoded: the on-board time, the other instruments on the \
    link, calibration.";

/// What the submode publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::meteor_hrpt_shape().results_decl()
}

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(MeteorHrptFields::SHAPE.fields)
            .with_table(IMAGES.def()),
    )
}

/// The display: states, fields, summary, histories and the new carrier search. It holds the stack's settings
/// `frequency_preset` (a change asks the host to retune) and `variant` (it chooses among the links of the default graph and
/// has no effect here).
pub struct MsuHrptDisplay {
    preset: HrptPreset,
    variant: VariantChoice,
    retune: bool,
    published: Published,
    hist: Histories,
    searches: u32,
    search_end_s: f64,
    first_line_s: Option<f64>,
    summary: String,
}

impl Default for MsuHrptDisplay {
    fn default() -> Self {
        MsuHrptDisplay {
            preset: FREQUENCY_PRESET.default,
            variant: VARIANT_CHOICE.default,
            retune: false,
            published: published(display_declaration()),
            hist: Histories::default(),
            searches: 0,
            search_end_s: 0.0,
            first_line_s: None,
            summary: String::new(),
        }
    }
}

impl Component for MsuHrptDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.msumr_hrpt_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<MsuHrptReport>("msu"),
            ],
            vec![
                item_port::<Control>("control"),
                item_port::<HostRequest>("requests"),
            ],
        )
        .with_settings(HrptDisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.searches = 0;
        self.search_end_s = 0.0;
        Ok(vec![item_format::<Control>(), item_format::<HostRequest>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let none = MsuHrptReport::default();
        let m = last::<MsuHrptReport>(io, 2).unwrap_or(&none);
        let t = f.map_or(0.0, |f| f.time_s);
        if std::mem::take(&mut self.retune) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: t,
                value: HostRequest::retune(self.preset.preset().0, "HRPT frequency preset"),
            });
        }
        for l in &m.log {
            self.published.results_mut().log(l.as_str());
        }
        let low = f.is_some_and(|f| f.rate_too_low);
        let good_recent = m.frames > 0 && m.since_frame_s < 3.0;
        let carrier = d.is_some_and(|d| d.state.locked);
        let state = if low {
            0
        } else if good_recent && m.locked {
            3
        } else if m.locked {
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
                && !m.locked
                && t - self.search_end_s >= RESEARCH_S
                && (m.frames == 0 || m.since_frame_s >= RESEARCH_S)
            {
                self.search_end_s = t;
                io.items_out::<Control>(0).push(Timed {
                    time: t,
                    value: Control::RestartCarrier,
                });
            }
        }
        let rows = m.msu.rows;
        if rows > 0 && self.first_line_s.is_none() {
            self.first_line_s = Some(t);
        }
        if rows == 0 {
            self.first_line_s = None;
        }
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(self.summary, "{NAME}, {rows} lines, {} frames", m.frames),
            2 => write!(self.summary, "{NAME}, frame sync, frames failing"),
            1 => write!(self.summary, "{NAME}, symbols, no frame sync"),
            _ if low => write!(
                self.summary,
                "this link needs {} Msps or more (a recording works)",
                MIN_RATE / 1e6
            ),
            _ => write!(self.summary, "{NAME}, searching"),
        };
        let p = &mut self.published;
        let carrier_hz = d.map(|d| d.state.carrier_hz + f.map_or(0.0, |f| f.located_hz));
        set_status(
            p.results_mut(),
            state,
            d.filter(|d| d.state.tracking).map(|d| d.state.es_n0_db),
            f.map(|f| f.level_dbfs),
            carrier_hz.map(|c| c + f.map_or(0.0, |f| f.user_offset_hz)),
            &self.summary,
        );
        p.put(&MeteorHrptFields {
            snr_db: d.map(|d| f64::from(d.state.es_n0_db)),
            level_dbfs: f.map(|f| f64::from(f.level_dbfs)),
            offset_hz: carrier_hz.map(Hertz),
            clock_ppm: d.map(|d| d.state.clock_ppm),
            symbol_rate_hz: d.map(|d| Hertz(d.symbol_rate_nominal)),
            frames_ok: m.frames as i64,
            frames_bad: 0,
            signal_time: Seconds(t),
            input_rate_hz: f.map(|f| Hertz(f.input_rate)),
            lock_metric: d.map(|d| f64::from(d.state.lock_metric)),
            frame_lock: m.locked,
            sync_hits: m.sync_hits as i64,
            packets: m.msu.frames as i64,
            line_sync_hits: m.msu.sync_hits as i64,
            lines: i64::from(rows),
            chip_pairing: m.pairing.map(i64::from),
            inverted: m.inverted,
            note: METEOR_NOTE,
        });
        if rows > 0 {
            IMAGES.put(
                p,
                INSTRUMENT,
                &ImageRow {
                    product: INSTRUMENT,
                    channel: None,
                    time: self.first_line_s.map(Seconds),
                    width: WIDTH as i64,
                    height: Some(i64::from(rows)),
                },
            );
        }
        self.hist.publish(self.published.results_mut(), d, None);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match HrptDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HrptDisplaySetting::Variant => self.variant = VARIANT_CHOICE.read(id, v)?,
            HrptDisplaySetting::FrequencyPreset => {
                let preset = FREQUENCY_PRESET.read(id, v)?;
                self.retune |= preset != self.preset;
                self.preset = preset;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                HrptDisplaySetting::FrequencyPreset,
                Value::Choice(self.preset.index()),
            ),
            (
                HrptDisplaySetting::Variant,
                Value::Choice(self.variant.index()),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph.

/// The Meteor HRPT graph.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layout produces no valid schema.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    let pictures =
        PassPictures::new(vec![picture_layout()], "Meteor-M").map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(front_link(), MIN_RATE, false),
        )
        .add("demod", PmDemodComponent::new(CHIP_RATE, 1.0, SEARCH_HZ))
        .add("msumr", MsuHrptComponent::default())
        .add("pictures", pictures)
        .add("display", MsuHrptDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "msumr.in")
        .connect("msumr.out", "pictures.in")
        .connect("msumr.pass", "pictures.pass")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("msumr.report", "display.msu")
        .feedback("display.control", "demod.control")
        .output("pictures.picture")
        .output("display.requests")
        .status_from("display")
        .clock(REPORT_S))
}

/// The settings of the stack `hrpt` that Meteor HRPT has: all but the HIRS view of NOAA.
pub fn settings() -> &'static [&'static dyn sdrkit_core::settings::DeclareSetting] {
    static LIST: std::sync::OnceLock<Vec<&'static dyn sdrkit_core::settings::DeclareSetting>> =
        std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        HrptSetting::SETTINGS
            .iter()
            .copied()
            .filter(|s| s.key() != super::hrpt::HIRS_VIEW.key)
            .collect()
    })
}

/// The Meteor HRPT receiver: the graph with the settings of the stack `hrpt`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::meteor_hrpt_shape(), g)
        .min_rate(MIN_RATE, "Meteor HRPT needs at least 3 MHz of samples")
        .route("frequency_preset", &["display.frequency_preset"])?
        .route("variant", &["display.variant"])?
        .route("view", &["pictures.view"])?
        .route("follow", &["pictures.follow"])?
        .route("pass_gap", &["msumr.pass_gap"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The Meteor HRPT test signal: MSU-MR lines of the test picture in transport frames, split-phase, phase modulated with a
/// residual carrier, at unit power.
pub struct MsuHrptGenerator {
    rate: f64,
    lines: u32,
    /// The lines of the pass, `None` for a signal without end.
    pass: Option<u64>,
    line: u32,
    counter: u32,
    stream: std::collections::VecDeque<u8>,
    modulator: PhaseMod,
    chips: Vec<u8>,
    samples: Vec<Complex32>,
    next: usize,
}

impl MsuHrptGenerator {
    /// From a configuration: `gen_pm_index` and `gen_lines`. The schema of the settings must be [`HrptSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the rate is below the least rate or the modulator refuses its options.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        if cfg.rate_hz < 2.0 * CHIP_RATE {
            return Err(GenError::RateTooLow {
                need: 2.0 * CHIP_RATE,
                got: cfg.rate_hz,
            });
        }
        let v = &cfg.settings;
        let index = v.get(&GEN_PM_INDEX).to_radians();
        Ok(MsuHrptGenerator {
            rate: cfg.rate_hz,
            lines: super::hrpt::TEST_PICTURE_LINES,
            pass: pass_lines(v.get(&GEN_LINES)),
            line: 0,
            counter: 0,
            stream: std::collections::VecDeque::with_capacity(2 * FRAME_BYTES),
            modulator: PhaseMod::new(1.0, 12.0, cfg.rate_hz / CHIP_RATE, index)?,
            chips: Vec::with_capacity(16 * 1024),
            samples: Vec::new(),
            next: 0,
        })
    }

    /// The next transport frame as samples.
    fn more(&mut self) {
        while self.stream.len() < PAYLOAD_BYTES {
            self.stream
                .extend(test_line(self.line % self.lines, test_value));
            self.line += 1;
        }
        let piece: Vec<u8> = self.stream.drain(..PAYLOAD_BYTES).collect();
        let frame = transport_frame(self.counter, &piece);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
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

impl Generator for MsuHrptGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut filled = 0;
        while filled < out.len() {
            if self.next >= self.samples.len() {
                // The signal ends after the frame of the last line of the pass.
                if self.pass.is_some_and(|p| u64::from(self.line) >= p) {
                    return filled;
                }
                self.more();
                continue;
            }
            let n = (self.samples.len() - self.next).min(out.len() - filled);
            out[filled..filled + n].copy_from_slice(&self.samples[self.next..self.next + n]);
            self.next += n;
            filled += n;
        }
        filled
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(2.0 * CHIP_RATE)
                .with_symbol_rate(CHIP_RATE)
                .with_bit_rate(BIT_RATE),
        )
    }
}

//! HRPT and AHRPT (`hrpt`): MetOp-B and -C AHRPT, FY-3 AHRPT frames on the same chain, and NOAA HRPT (archived
//! recordings). NOAA HRPT also gives the HIRS sounder picture from the TIP minor frames of its frames of number 1. The submode
//! `meteor` is Meteor HRPT, on its own graph (see [`super::hrpt_meteor`]).
//!
//! Standard: NOAA KLM User's Guide, sections 4.1.3 (TIP words of an HRPT frame) and 4.3.4.1 (HIRS digital data).

use super::display::{log_lines, put_link, status, view_choices, with_series, Histories};
use super::hrpt_meteor;
use crate::components::instruments::{
    AvhrrComponent, AvhrrReport, HirsComponent, HirsReport, NoaaComponent,
};
use crate::components::link::{
    item_format, item_port, CodeComponent, CodeReport, DemodComponent, DemodReport, FrameComponent,
    FrameLink, FrameReport, FrontEndComponent, FrontReport, PacketReport, VcduComponent,
};
use crate::components::{last, Control};
use crate::gen::{
    aos_frame, cadu_bytes, space_packet, test_pixel, BitSource, CcsdsCoder, CodeOptions, GenError,
    ModParams, Modulator, ZoneStream,
};
use crate::instruments::avhrr::{build_noaa, AvhrrStats, WIDTH};
use crate::instruments::tip::{test_frame, HIRS_WIDTH, HRPT_TIP_WORD, TIP_BYTES};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::typed::{HrptFields, ImageRow, LinkFields, VirtualChannelRow, IMAGES, VIRTUAL_CHANNELS};
use crate::REPORT_S;
use sdrkit_algo_fec::rs::Basis;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::View;
use sdrkit_core::interface::{Fields as _, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, HostRequest, LockProfile,
    Receiver, Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StackConfig, StackDescriptor, Tuning, Value, Values,
};
use sdrkit_core::{SignalType, SubMode};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Port, StackGraph, Timed,
};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::codesync::CodeConfig;
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "hrpt";
/// The least input rate.
pub const MIN_RATE: f64 = 4e6;

sdrkit_core::choice_enum! {
    /// A variant of the link.
    pub enum Variant {
        /// MetOp AHRPT.
        Metop = ("metop", "MetOp AHRPT (QPSK 2.33 Msym/s)"),
        /// FY-3 AHRPT (frames and virtual channels counted only).
        Fy3 = ("fy3", "FY-3 AHRPT (QPSK 2.8 Msym/s)"),
        /// NOAA HRPT.
        Noaa = ("noaa", "NOAA HRPT (retired)"),
    }
}

sdrkit_core::choice_enum! {
    /// The variant setting: automatic or one variant.
    pub enum VariantChoice {
        /// Try each variant in turn.
        Auto = ("auto", "Automatic"),
        /// MetOp AHRPT.
        Metop = ("metop", "MetOp AHRPT (QPSK 2.33 Msym/s)"),
        /// FY-3 AHRPT.
        Fy3 = ("fy3", "FY-3 AHRPT (QPSK 2.8 Msym/s)"),
        /// NOAA HRPT.
        Noaa = ("noaa", "NOAA HRPT (retired)"),
    }
}

impl VariantChoice {
    /// The fixed variant, `None` for automatic.
    pub fn fixed(self) -> Option<Variant> {
        match self {
            VariantChoice::Auto => None,
            VariantChoice::Metop => Some(Variant::Metop),
            VariantChoice::Fy3 => Some(Variant::Fy3),
            VariantChoice::Noaa => Some(Variant::Noaa),
        }
    }
    /// The choice of a fixed variant, automatic for `None`.
    pub fn of(v: Option<Variant>) -> VariantChoice {
        match v {
            None => VariantChoice::Auto,
            Some(Variant::Metop) => VariantChoice::Metop,
            Some(Variant::Fy3) => VariantChoice::Fy3,
            Some(Variant::Noaa) => VariantChoice::Noaa,
        }
    }
}

impl Variant {
    /// The name the screens show.
    pub fn name(self) -> &'static str {
        match self {
            Variant::Metop => "MetOp AHRPT",
            Variant::Fy3 => "FY-3 AHRPT",
            Variant::Noaa => "NOAA HRPT",
        }
    }
    /// The link.
    pub fn link(self) -> LinkParams {
        match self {
            Variant::Metop | Variant::Fy3 => LinkParams {
                modulation: Modulation::Qpsk,
                symbol_rate: if self == Variant::Metop {
                    2.33e6
                } else {
                    2.8e6
                },
                roll_off: if self == Variant::Metop { 0.6 } else { 0.5 },
                class: LinkClass::Leo,
                search: Search::Template { range_hz: 75e3 },
                snr: SnrEstimator::Decisions,
                bpsk_plus_is_zero: false,
                constellation: 256,
                interleaved: false,
            },
            Variant::Noaa => LinkParams {
                modulation: Modulation::Bpsk,
                symbol_rate: 1_330_800.0,
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
            },
        }
    }
    /// The automatic order: MetOp, FY-3, MetOp, FY-3, NOAA.
    pub fn auto_order() -> [Variant; 5] {
        [
            Variant::Metop,
            Variant::Fy3,
            Variant::Metop,
            Variant::Fy3,
            Variant::Noaa,
        ]
    }
}

sdrkit_core::choice_enum! {
    /// The frequencies of HRPT and AHRPT.
    pub enum HrptPreset {
        /// MetOp-B and MetOp-C AHRPT.
        Mhz1701_3 = ("1701.300", "1701.300 MetOp-B / MetOp-C AHRPT"),
        /// FY-3 AHRPT.
        Mhz1701_4 = ("1701.400", "1701.400 FY-3 AHRPT (check the satellite)"),
        /// NOAA-19 HRPT.
        Mhz1698_0 = ("1698.000", "1698.000 NOAA-19 HRPT (retired)"),
        /// NOAA-15 HRPT.
        Mhz1702_5 = ("1702.500", "1702.500 NOAA-15 HRPT (retired)"),
        /// NOAA-18 HRPT.
        Mhz1707_0 = ("1707.000", "1707.000 NOAA-18 HRPT (retired)"),
    }
}

impl HrptPreset {
    /// The frequency and the variant of the preset.
    pub fn preset(self) -> (f64, Variant) {
        match self {
            HrptPreset::Mhz1701_3 => (1701.3e6, Variant::Metop),
            HrptPreset::Mhz1701_4 => (1701.4e6, Variant::Fy3),
            HrptPreset::Mhz1698_0 => (1698.0e6, Variant::Noaa),
            HrptPreset::Mhz1702_5 => (1702.5e6, Variant::Noaa),
            HrptPreset::Mhz1707_0 => (1707.0e6, Variant::Noaa),
        }
    }
}

const TEST: &str = "Test signal";

/// The frequency. A change requests the host to retune.
pub const FREQUENCY_PRESET: Setting<field::Choice<HrptPreset>> =
    Setting::choice("frequency_preset", "Frequency", HrptPreset::Mhz1701_3).help(
        "Active now: MetOp-B and MetOp-C (AHRPT 1701.3 MHz) and the FY-3 satellites (check the frequency of each one).",
    );
/// The variant.
pub const VARIANT_CHOICE: Setting<field::Choice<VariantChoice>> =
    Setting::choice("variant", "Satellite", VariantChoice::Auto).help("Restarts the demodulator.");
/// The view of the picture.
pub const VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "What the picture shows.",
    choices: || view_choices(vec![picture_layout()]),
    default: "rgb214",
    advanced: false,
};
/// The view of the HIRS picture (NOAA).
pub const HIRS_VIEW: CatalogueSetting = CatalogueSetting {
    key: "hirs_view",
    label: "HIRS view",
    help: "What the HIRS picture of NOAA HRPT shows.",
    choices: || view_choices(vec![super::dsb::picture_layout()]),
    default: "rgb214",
    advanced: false,
};
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow", "", true);
/// The variant of the test signal.
pub const GEN_VARIANT: Setting<field::Choice<Variant>> =
    Setting::choice("gen_variant", "Test satellite", Variant::Metop)
        .section(TEST)
        .advanced();
/// The test signal sends channel 3B (APID 104).
pub const GEN_3B: Setting<field::Bool> =
    Setting::new("gen_3b", "Test channel 3B (APID 104)", "", false)
        .section(TEST)
        .advanced();
/// Lines of the test picture before it repeats.
pub const GEN_LINES: Setting<field::I64> = Setting::new("gen_lines", "Test picture lines", "", 60)
    .range(1.0, 4000.0, 1.0)
    .section(TEST)
    .advanced();
/// The phase index of the Meteor HRPT test signal (submode `meteor`).
pub const GEN_PM_INDEX: Setting<field::F64> =
    Setting::new("gen_pm_index", "Test phase index (Meteor)", "deg", 67.5)
        .range(10.0, 85.0, 0.5)
        .section(TEST)
        .advanced();
/// The NOAA test signal has the inverted polarity.
pub const GEN_POLARITY: Setting<field::Bool> =
    Setting::new("gen_polarity", "Test inverted NOAA polarity", "", false)
        .section(TEST)
        .advanced();

sdrkit_core::settings! {
    /// The settings of `hrpt`, with the options of its test signal.
    pub enum HrptSetting {
        /// The frequency.
        FrequencyPreset = FREQUENCY_PRESET,
        /// The variant.
        Variant = VARIANT_CHOICE,
        /// The view of the picture.
        View = VIEW,
        /// The view of the HIRS picture.
        HirsView = HIRS_VIEW,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// Test: the variant.
        GenVariant = GEN_VARIANT,
        /// Test: channel 3B.
        Gen3b = GEN_3B,
        /// Test: picture lines.
        GenLines = GEN_LINES,
        /// Test: inverted NOAA polarity.
        GenPolarity = GEN_POLARITY,
        /// Test: the phase index of the Meteor HRPT signal.
        GenPmIndex = GEN_PM_INDEX,
    }
}

sdrkit_core::settings! {
    /// The settings of [`HrptDisplay`].
    pub enum HrptDisplaySetting {
        /// The frequency.
        FrequencyPreset = FREQUENCY_PRESET,
        /// The variant.
        Variant = VARIANT_CHOICE,
    }
}

fn views() -> Vec<NamedView> {
    let grey = |channel, invert| View::Grey { channel, invert };
    vec![
        NamedView::new(
            "rgb214",
            "Colour (2, 1, 4)",
            View::Rgb([(2, false), (1, false), (4, true)]),
        ),
        NamedView::new("ch1", "Channel 1 (visible)", grey(1, false)),
        NamedView::new("ch2", "Channel 2 (near infrared)", grey(2, false)),
        NamedView::new("ch3", "Channel 3 (3A / 3B)", grey(3, false)),
        NamedView::new("ch4", "Channel 4 (thermal)", grey(4, true)),
        NamedView::new("ch5", "Channel 5 (thermal)", grey(5, true)),
    ]
}

/// The picture layout of AVHRR.
pub fn picture_layout() -> PictureLayout {
    PictureLayout {
        name: "AVHRR",
        width: WIDTH as u32,
        max_rows: crate::instruments::avhrr::MAX_LINES,
        full_mask: 1,
        views: views(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

/// What the HRPT stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(LinkFields::SHAPE.fields)
            .with_fields(HrptFields::SHAPE.fields)
            .with_table(VIRTUAL_CHANNELS.def())
            .with_table(IMAGES.def()),
    )
}

/// The display: states, fields, summary, histories and the automatic variant.
pub struct HrptDisplay {
    setting: Option<Variant>,
    running: Variant,
    auto_index: usize,
    preset: HrptPreset,
    retune: bool,
    published: Published,
    hist: Histories,
    started: bool,
    last_change_s: f64,
    last_lock_s: f64,
    pending: bool,
    summary: String,
    key: String,
    first_lines_s: Option<f64>,
    first_hirs_s: Option<f64>,
}

impl Default for HrptDisplay {
    fn default() -> Self {
        HrptDisplay {
            setting: None,
            running: Variant::Metop,
            auto_index: 0,
            preset: HrptPreset::Mhz1701_3,
            retune: false,
            published: super::display::published(display_declaration()),
            hist: Histories::default(),
            started: false,
            last_change_s: 0.0,
            last_lock_s: 0.0,
            pending: false,
            summary: String::new(),
            key: String::new(),
            first_lines_s: None,
            first_hirs_s: None,
        }
    }
}

impl Component for HrptDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.hrpt_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<CodeReport>("code"),
                Port::typed::<FrameReport>("frames"),
                Port::typed::<PacketReport>("packets"),
                Port::typed::<AvhrrReport>("avhrr"),
                Port::typed::<AvhrrReport>("noaa"),
                Port::typed::<HirsReport>("hirs"),
            ],
            vec![
                item_port::<Control>("control"),
                item_port::<HostRequest>("requests"),
            ],
        )
        .with_settings(HrptDisplaySetting::schema())
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
        let c = last::<CodeReport>(io, 2);
        let fr = last::<FrameReport>(io, 3);
        let pk = last::<PacketReport>(io, 4);
        let av = last::<AvhrrReport>(io, 5);
        let no = last::<AvhrrReport>(io, 6);
        let hirs = last::<HirsReport>(io, 7)
            .map(|h| h.stats)
            .unwrap_or_default();
        let t = f.map_or(0.0, |f| f.time_s);
        if std::mem::take(&mut self.retune) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: t,
                value: HostRequest::retune(self.preset.preset().0, "HRPT frequency preset"),
            });
        }
        if !self.started || self.pending {
            self.started = true;
            self.pending = false;
            self.running = self
                .setting
                .unwrap_or(Variant::auto_order()[self.auto_index]);
            self.last_change_s = t;
            self.last_lock_s = t;
            io.items_out::<Control>(0).push(Timed {
                time: t,
                value: Control::Link(self.running.link()),
            });
        }
        if let Some(fr) = fr {
            log_lines(self.published.results_mut(), &fr.log);
        }
        let code = c.is_some_and(|c| c.lock.locked);
        let noaa_lock = no.is_some_and(|n| n.noaa_locked);
        let locked = d.is_some_and(|d| d.state.locked && d.state.tracking);
        let good_recent = fr.is_some_and(|f| f.stats.good > 0 && f.since_good_s < 2.0);
        let noaa_recent = self.running == Variant::Noaa
            && no.is_some_and(|n| n.stats.noaa_frames > 0)
            && noaa_lock;
        let state = if good_recent || noaa_recent {
            3
        } else if (fr.is_some_and(|f| f.stats.locked) && code) || noaa_lock {
            2
        } else if locked {
            1
        } else {
            0
        };
        if code || noaa_lock || state == 3 {
            self.last_lock_s = t;
        }
        // Automatic: step on after 8 s (4 s for NOAA) without lock, at most every 2 s.
        if self.setting.is_none() {
            let wait = if self.running == Variant::Noaa {
                4.0
            } else {
                8.0
            };
            if t - self.last_lock_s >= wait && t - self.last_change_s >= 2.0 {
                self.auto_index = (self.auto_index + 1) % 5;
                self.running = Variant::auto_order()[self.auto_index];
                self.last_change_s = t;
                self.last_lock_s = t;
                io.items_out::<Control>(0).push(Timed {
                    time: t,
                    value: Control::Link(self.running.link()),
                });
            }
        }
        self.publish(state, t, f, d, c, fr, pk, av, no, &hirs);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match HrptDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HrptDisplaySetting::Variant => {
                self.setting = VARIANT_CHOICE.read(id, v)?.fixed();
                self.pending = self.started;
            }
            HrptDisplaySetting::FrequencyPreset => {
                let preset = FREQUENCY_PRESET.read(id, v)?;
                if preset != self.preset {
                    self.retune = true;
                }
                self.preset = preset;
                if self.setting.is_some() {
                    self.setting = Some(preset.preset().1);
                    self.pending = self.started;
                }
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
                Value::Choice(VariantChoice::of(self.setting).index()),
            ),
        ])
    }
    /// The frames counted are of the carrier of the frequency before. The images stay where `kept` holds them.
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.published.clear_rows(VIRTUAL_CHANNELS.def());
        if !kept.table(IMAGES.key) {
            self.published.clear_rows(IMAGES.def());
            self.first_lines_s = None;
            self.first_hirs_s = None;
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

impl HrptDisplay {
    /// The row of the HIRS picture once it has lines.
    fn publish_hirs(&mut self, h: &crate::instruments::tip::HirsStats, t: f64) {
        let p = &mut self.published;
        if h.rows > 0 {
            let first = *self.first_hirs_s.get_or_insert(t);
            IMAGES.put(
                p,
                "HIRS",
                &ImageRow {
                    product: "HIRS",
                    channel: None,
                    time: Some(Seconds(first)),
                    width: HIRS_WIDTH as i64,
                    height: Some(i64::from(h.rows)),
                },
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        state: u8,
        t: f64,
        f: Option<&FrontReport>,
        d: Option<&DemodReport>,
        c: Option<&CodeReport>,
        fr: Option<&FrameReport>,
        pk: Option<&PacketReport>,
        av: Option<&AvhrrReport>,
        no: Option<&AvhrrReport>,
        hirs: &crate::instruments::tip::HirsStats,
    ) {
        let v = self.running;
        let p = &mut self.published;
        put_link(p, t, f, d, c, fr, pk);
        let none = AvhrrStats::default();
        let a = av.map_or(&none, |a| &a.stats);
        let n = no.map_or(&none, |a| &a.stats);
        let lines = a.height.max(n.height);
        if lines > 0 && self.first_lines_s.is_none() {
            self.first_lines_s = Some(t);
        }
        let frames =
            fr.map_or(0, |f| f.stats.good) + if v == Variant::Noaa { n.noaa_frames } else { 0 };
        let summary = &mut self.summary;
        summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(summary, "{}, {} lines, {} frames", v.name(), lines, frames),
            2 => write!(summary, "{}, frame sync, RS failing", v.name()),
            1 => write!(summary, "{}, symbols, no frame sync", v.name()),
            _ => write!(
                summary,
                "searching ({})",
                if self.setting.is_none() {
                    "automatic"
                } else {
                    v.name()
                }
            ),
        };
        status(p.results_mut(), state, f, d, summary);
        let noaa = v == Variant::Noaa;
        p.put(&HrptFields {
            variant: v,
            variant_setting: VariantChoice::of(self.setting),
            phase_ambiguity_mirrored: c.filter(|_| !noaa).map(|c| c.lock.g2_inverted()),
            noaa_rotation: no.filter(|_| noaa).map(|n| n.noaa_rotation as i64),
            packets_apid_103: a.apid103 as i64,
            packets_apid_104: a.apid104 as i64,
            other_packets: a.other as i64,
            noaa_frames: n.noaa_frames as i64,
            avhrr_lines: (a.lines + n.lines) as i64,
            picture_lines: lines as i64,
            onboard_time: a
                .onboard_ms
                .or(n.onboard_ms)
                .map(|o| Seconds(o as f64 / 1000.0)),
            hirs_lines: hirs.lines as i64,
            hirs_valid: hirs.valid as i64,
            hirs_parity_errors: hirs.parity_errors as i64,
        });
        if lines > 0 {
            IMAGES.put(
                p,
                "AVHRR",
                &ImageRow {
                    product: v.name(),
                    channel: None,
                    time: self.first_lines_s.map(Seconds),
                    width: WIDTH as i64,
                    height: Some(i64::from(lines)),
                },
            );
        }
        if let Some(pk) = pk {
            for (vc, frames) in &pk.stats.per_vc {
                self.key.clear();
                // A write to a String cannot fail.
                let _ = write!(self.key, "{vc}");
                let row = VirtualChannelRow {
                    vcid: i64::from(*vc),
                    frames: *frames as i64,
                };
                VIRTUAL_CHANNELS.put(p, &self.key, &row);
            }
        }
        self.hist.publish(self.published.results_mut(), d, fr);
        self.publish_hirs(hirs, t);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The HRPT graph, starting with `variant`.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layout produces no valid schema.
pub fn builder(variant: Variant) -> Result<GraphBuilder, GraphError> {
    let link = variant.link();
    let pictures = Pictures::new(vec![picture_layout()])
        .map_err(GraphError::Schema)?
        .with_inputs(2);
    let hirs_pictures =
        Pictures::new(vec![super::dsb::picture_layout()]).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(link.clone(), MIN_RATE, false).with_decimation_for(2.8e6),
        )
        .add("demod", DemodComponent::new(link.clone()))
        .add("decoder", CodeComponent::new(CodeConfig::LEO))
        .add("frames", FrameComponent::new(FrameLink::LEO_DUAL))
        .add("vcdu", VcduComponent::new(Layout::METEOR, false))
        .add("avhrr", AvhrrComponent::default())
        .add(
            "noaa",
            NoaaComponent::new(link.modulation == Modulation::Bpsk),
        )
        .add("pictures", pictures)
        .add("hirs", HirsComponent::default())
        .add("hirs_pictures", hirs_pictures)
        .add("display", HrptDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "decoder.in")
        .connect("demod.soft", "noaa.in")
        .connect("decoder.out", "frames.in")
        .connect("frames.out", "vcdu.in")
        .connect("vcdu.packets", "avhrr.in")
        .connect("avhrr.out", "pictures.in")
        .connect("noaa.out", "pictures.in2")
        .connect("noaa.tip", "hirs.in")
        .connect("hirs.out", "hirs_pictures.in")
        .connect("hirs.report", "display.hirs")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("decoder.report", "display.code")
        .connect("frames.report", "display.frames")
        .connect("vcdu.report", "display.packets")
        .connect("avhrr.report", "display.avhrr")
        .connect("noaa.report", "display.noaa")
        .feedback("display.control", "demod.control")
        .feedback("display.control", "decoder.control")
        .feedback("display.control", "noaa.control")
        .output("pictures.picture")
        .output("hirs_pictures.picture")
        .output("display.requests")
        .status_from("display")
        .clock(REPORT_S))
}

/// The HRPT receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let v = cfg
        .settings
        .get(&VARIANT_CHOICE)
        .fixed()
        .unwrap_or(Variant::Metop);
    let mut g = builder(v)?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "HRPT needs at least 4 MHz of samples")
        .route("frequency_preset", &["display.frequency_preset"])?
        .route("variant", &["display.variant"])?
        .route("view", &["pictures.view"])?
        .route("hirs_view", &["hirs_pictures.view"])?
        .route("follow", &["pictures.follow", "hirs_pictures.follow"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    if cfg.submode.as_deref() == Some(hrpt_meteor::SUBMODE) {
        return super::stack_receiver(hrpt_meteor::receiver(cfg));
    }
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    if cfg.submode.as_deref() == Some(hrpt_meteor::SUBMODE) {
        return Ok(Box::new(
            hrpt_meteor::MsuHrptGenerator::from_config(cfg).map_err(super::gen_error)?,
        ));
    }
    Ok(Box::new(
        HrptGenerator::from_config(cfg).map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "HRPT / AHRPT",
    group: crate::GROUP,
    blurb: "High-resolution pictures from MetOp (AHRPT), FY-3 frames and archived NOAA HRPT near 1.7 GHz. It needs a tracking dish.",
    tuning: Tuning {
        min_hz: 1690e6,
        max_hz: 1710e6,
        ..Tuning::anywhere(1701.3e6, 6e6, MIN_RATE, 4.5e6)
    },
    // The default frequency of the submode is the one of its test signal. The host gives the frequency of a session.
    submodes: &[SubMode {
        id: hrpt_meteor::SUBMODE,
        name: "Meteor HRPT",
        default_hz: 1700e6,
    }],
    settings: HrptSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hrpt],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::HRPT_HOOKS),
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The test AVHRR line `l` (5 x 2048 samples of 10 bits).
pub fn test_line(l: u32) -> Vec<u16> {
    let mut px = vec![0u16; 5 * WIDTH];
    for c in 0..5 {
        for x in 0..WIDTH {
            px[c * WIDTH + x] = ((test_pixel(c as u16 + 1, x as u32, l, WIDTH as u32) as u16) << 2)
                | ((x as u16 + l as u16) & 3);
        }
    }
    px
}

/// The channel bits of the HRPT test signal.
pub struct HrptSource {
    variant: Variant,
    apid: u16,
    coder: CcsdsCoder,
    zones: ZoneStream,
    counter: u32,
    seq: u16,
    line: u32,
    lines: u32,
    tip_counter: u16,
    hirs_element: u8,
    hirs_line: u32,
}

/// The signed value (0 to 4095) of HIRS word `w` (in sending order) at Earth view `element` of HIRS line `line` in the NOAA
/// test signal: the test pixel of the channel times 16, with the element in the low 4 bits. Its word is 4096 plus this value.
pub fn hirs_test_value(w: u16, element: u32, line: u32) -> u16 {
    (u16::from(test_pixel(w + 1, element, line, HIRS_WIDTH as u32)) << 4) | (element as u16 & 15)
}

impl BitSource for HrptSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        if self.variant == Variant::Noaa {
            let l = self.line % self.lines;
            let ms = 36_000_000 + (self.line as f64 * 1000.0 / 6.0).round() as u32;
            self.line += 1;
            let mut words = build_noaa(300, ms, &test_line(l));
            // Frame number (line mod 3) + 1 and spacecraft address 15 (NOAA-19) in word 7; frames of number 1 carry five TIP
            // minor frames, each with the next HIRS Earth view.
            let number = (self.line - 1) % 3 + 1;
            words[6] = ((number as u16) << 7) | (15 << 3);
            if number == 1 {
                for k in 0..5 {
                    let tip = test_frame(
                        15,
                        self.tip_counter,
                        self.hirs_element,
                        self.hirs_line,
                        hirs_test_value,
                    );
                    for (j, &b) in tip.iter().enumerate() {
                        words[HRPT_TIP_WORD + TIP_BYTES * k + j] = u16::from(b) << 2;
                    }
                    self.tip_counter = self.tip_counter.wrapping_add(1);
                    self.hirs_element += 1;
                    if usize::from(self.hirs_element) == HIRS_WIDTH {
                        self.hirs_element = 0;
                        self.hirs_line += 1;
                    }
                }
            }
            for w in words {
                for i in (0..10).rev() {
                    // Split-phase: a 1 is (high, low).
                    let b = ((w >> i) & 1) as u8;
                    out.push(b);
                    out.push(b ^ 1);
                }
            }
            return;
        }
        let layout = Layout::METEOR;
        let zl = layout.data_zone_len();
        while self.zones.len() < zl {
            if self.variant == Variant::Metop {
                let l = self.line % self.lines;
                let ms = 36_000_000 + (self.line as f64 * 1000.0 / 6.0).round() as u32;
                self.line += 1;
                let user = crate::instruments::avhrr::build_packet(24_800, ms, 0, &test_line(l));
                self.zones
                    .push(&space_packet(self.apid, 3, self.seq, &user, false));
            } else {
                self.zones
                    .push(&space_packet(30, 3, self.seq, &[0x55u8; 1000], false));
            }
            self.seq = (self.seq + 1) & 0x3FFF;
        }
        let (fhp, zone) = self.zones.take(zl);
        let (sc, vc) = if self.variant == Variant::Metop {
            (11, 9)
        } else {
            (51, 5)
        };
        let v = aos_frame(&layout, sc, vc, self.counter, fhp, &zone);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        let cadu = cadu_bytes(&v, Basis::Dual, true);
        self.coder.push(&cadu, out);
    }
}

/// The HRPT test signal: the clean signal at unit mean power.
pub struct HrptGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<HrptSource>,
}

impl HrptGenerator {
    /// From a configuration: `gen_variant`, `gen_3b` (APID 104), `gen_lines` and `gen_polarity` (NOAA, inverted). The schema of
    /// the settings must be [`HrptSetting`].
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses the options.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, GenError> {
        let v: &Values = &cfg.settings;
        let variant = v.get(&GEN_VARIANT);
        let l = variant.link();
        let mut modem = ModParams::new(cfg.rate_hz, l.symbol_rate, l.roll_off, l.modulation);
        modem.zero_is_plus = v.get(&GEN_POLARITY);
        let three_b = v.get(&GEN_3B);
        let lines = v.get(&GEN_LINES).clamp(1, 4000) as u32;
        let src = HrptSource {
            variant,
            apid: if three_b { 104 } else { 103 },
            coder: CcsdsCoder::new(CodeOptions {
                nrzm: false,
                invert_g2: true,
                conv: true,
            }),
            zones: ZoneStream::default(),
            counter: 0,
            seq: 0,
            line: 0,
            lines,
            tip_counter: 0,
            hirs_element: 0,
            hirs_line: 0,
        };
        Ok(HrptGenerator {
            rate: cfg.rate_hz,
            info: modem.signal_info(),
            m: Modulator::new(modem, src)?,
        })
    }
}

impl Generator for HrptGenerator {
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

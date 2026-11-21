//! Meteor-M LRPT (`meteor_lrpt`): MSU-MR pictures on 137.1 or 137.9 MHz, OQPSK or QPSK at 72 ksym/s or the
//! interleaved OQPSK 80k link, the CCSDS code with G2 not inverted, and Reed-Solomon in the conventional basis.

use super::display::{
    log_lines, put_link, status, view_choices, with_series, Histories,
};
use crate::components::instruments::{MsuMrComponent, MsuReport};
use crate::components::link::{
    control_port, item_format, item_port, CodeComponent, CodeReport, DemodComponent, DemodReport,
    FrameComponent, FrameLink, FrameReport, FrontEndComponent, FrontReport, PacketReport,
    VcduComponent,
};
use crate::components::{controls, last, Control};
use crate::gen::{
    aos_frame, cadu_bytes, space_packet, test_pixel, BitSource, CcsdsCoder, CodeOptions, GenError,
    ModParams, Modulator, ZoneStream,
};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::REPORT_S;
use sdrkit_algo_fec::rs::Basis;
use sdrkit_codec_image::component::{Layout as PictureLayout, NamedView, Pictures};
use sdrkit_codec_image::strip::{Enhance, View};
use crate::typed::{
    ImageRow, LinkFields, MeteorLrptChannelRow, MeteorLrptFields, IMAGES, METEOR_LRPT_CHANNELS,
};
use sdrkit_core::interface::{Fields as _, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::CatalogueSetting;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, HostRequest, LockProfile,
    Receiver, Results, ResultsDecl, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StackConfig, StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Port, StackGraph, Timed,
};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::codesync::{transform, CodeConfig};
use sdrkit_util_bits::interleave::DelayLanes;
use std::fmt::Write as _;

/// The stack id.
pub const ID: &str = "meteor_lrpt";
/// The least input rate.
pub const MIN_RATE: f64 = 250e3;

sdrkit_core::choice_enum! {
    /// A modulation of Meteor-M LRPT.
    pub enum MeteorModulation {
        /// OQPSK 72 ksym/s with NRZ-M (N2-3, N2-4).
        Oqpsk72 = ("oqpsk72", "OQPSK 72k (N2-3, N2-4)"),
        /// QPSK 72 ksym/s (N2).
        Qpsk72 = ("qpsk72", "QPSK 72k (N2)"),
        /// OQPSK 80 ksym/s, NRZ-M, interleaved.
        Oqpsk80 = ("oqpsk80", "OQPSK 80k interleaved"),
    }
}

sdrkit_core::choice_enum! {
    /// The modulation setting: automatic or one modulation.
    pub enum ModulationChoice {
        /// Try each modulation in turn until frames decode.
        Auto = ("auto", "Automatic"),
        /// OQPSK 72 ksym/s.
        Oqpsk72 = ("oqpsk72", "OQPSK 72k (N2-3, N2-4)"),
        /// QPSK 72 ksym/s.
        Qpsk72 = ("qpsk72", "QPSK 72k (N2)"),
        /// OQPSK 80 ksym/s, interleaved.
        Oqpsk80 = ("oqpsk80", "OQPSK 80k interleaved"),
    }
}

impl ModulationChoice {
    /// The fixed modulation, `None` for automatic.
    pub fn fixed(self) -> Option<MeteorModulation> {
        match self {
            ModulationChoice::Auto => None,
            ModulationChoice::Oqpsk72 => Some(MeteorModulation::Oqpsk72),
            ModulationChoice::Qpsk72 => Some(MeteorModulation::Qpsk72),
            ModulationChoice::Oqpsk80 => Some(MeteorModulation::Oqpsk80),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The frequencies of Meteor-M LRPT.
    pub enum FrequencyPreset {
        /// 137.900 MHz.
        Mhz137_9 = ("137.900", "137.900 MHz (N2-4, N2-3)"),
        /// 137.100 MHz.
        Mhz137_1 = ("137.100", "137.100 MHz"),
    }
}

impl FrequencyPreset {
    /// The tuned frequency.
    pub fn hz(self) -> f64 {
        match self {
            FrequencyPreset::Mhz137_9 => 137.9e6,
            FrequencyPreset::Mhz137_1 => 137.1e6,
        }
    }
}

impl MeteorModulation {
    /// The name the screens show.
    pub fn name(self) -> &'static str {
        match self {
            MeteorModulation::Oqpsk72 => "OQPSK 72k",
            MeteorModulation::Qpsk72 => "QPSK 72k",
            MeteorModulation::Oqpsk80 => "OQPSK 80k",
        }
    }
    /// The next one in the automatic order.
    pub fn next(self) -> Self {
        match self {
            MeteorModulation::Oqpsk72 => MeteorModulation::Qpsk72,
            MeteorModulation::Qpsk72 => MeteorModulation::Oqpsk80,
            MeteorModulation::Oqpsk80 => MeteorModulation::Oqpsk72,
        }
    }
    /// The link.
    pub fn link(self) -> LinkParams {
        LinkParams {
            modulation: if self == MeteorModulation::Qpsk72 {
                Modulation::Qpsk
            } else {
                Modulation::Oqpsk
            },
            symbol_rate: if self == MeteorModulation::Oqpsk80 {
                80e3
            } else {
                72e3
            },
            roll_off: 0.6,
            class: LinkClass::Leo,
            search: Search::Template { range_hz: 15e3 },
            snr: SnrEstimator::Moments,
            bpsk_plus_is_zero: false,
            constellation: 256,
            interleaved: self == MeteorModulation::Oqpsk80,
        }
    }
}

/// The frequency. A change requests the host to retune.
pub const FREQUENCY_PRESET: Setting<field::Choice<FrequencyPreset>> =
    Setting::choice("frequency_preset", "Frequency", FrequencyPreset::Mhz137_9).help(
        "Meteor-M N2-3 and N2-4 send LRPT on 137.900 or 137.100 MHz. Which one each satellite uses changes, so use a current list.",
    );
/// The modulation.
pub const MODULATION_CHOICE: Setting<field::Choice<ModulationChoice>> =
    Setting::choice("modulation", "Modulation", ModulationChoice::Auto).help(
        "Automatic tries OQPSK 72k (N2-3, N2-4), then QPSK 72k, then the interleaved 80k link, until frames decode.",
    );
/// The view of the picture.
pub const VIEW: CatalogueSetting = CatalogueSetting {
    key: "view",
    label: "View",
    help: "What the picture shows.",
    choices: || view_choices(vec![picture_layout()]),
    default: "auto",
    advanced: false,
};
/// The enhancement of the picture.
pub const ENHANCE: Setting<field::Choice<Enhance>> =
    Setting::choice("enhance", "Enhancement", Enhance::None)
        .help("Thermal colours apply to a single channel view.");
/// Keep the newest lines in view.
pub const FOLLOW: Setting<field::Bool> =
    Setting::new("follow", "Follow", "", true).help("Keeps the newest lines in view.");
/// The modulation of the test signal.
pub const GEN_MODULATION: Setting<field::Choice<MeteorModulation>> = Setting::choice(
    "gen_modulation",
    "Test modulation",
    MeteorModulation::Oqpsk72,
)
.section("Test signal")
.advanced();
/// The test signal sends the night channels (1, 2, 5).
pub const GEN_NIGHT: Setting<field::Bool> =
    Setting::new("gen_night", "Test night channels", "", false)
        .section("Test signal")
        .advanced();
/// Rows of the test picture before it repeats.
pub const GEN_ROWS: Setting<field::I64> = Setting::new("gen_rows", "Test picture rows", "", 40)
    .range(1.0, 1500.0, 1.0)
    .section("Test signal")
    .advanced();

sdrkit_core::settings! {
    /// The settings of `meteor_lrpt`, with the options of its test signal.
    pub enum MeteorSetting {
        /// The frequency.
        FrequencyPreset = FREQUENCY_PRESET,
        /// The modulation.
        Modulation = MODULATION_CHOICE,
        /// The view of the picture.
        View = VIEW,
        /// The enhancement of the picture.
        Enhance = ENHANCE,
        /// Keep the newest lines in view.
        Follow = FOLLOW,
        /// The modulation of the test signal.
        GenModulation = GEN_MODULATION,
        /// The night channels of the test signal.
        GenNight = GEN_NIGHT,
        /// The rows of the test picture.
        GenRows = GEN_ROWS,
    }
}

sdrkit_core::settings! {
    /// The settings of [`MeteorDisplay`].
    pub enum MeteorDisplaySetting {
        /// The frequency.
        FrequencyPreset = FREQUENCY_PRESET,
        /// The modulation.
        Modulation = MODULATION_CHOICE,
    }
}

fn views() -> Vec<NamedView> {
    let rgb = |a, b, c| View::Rgb([(a, false), (b, false), (c, false)]);
    let mut v = vec![
        NamedView::new(
            "auto",
            "Colour (automatic)",
            View::FirstOf(vec![rgb(1, 2, 3), rgb(1, 2, 5), View::FirstChannel]),
        ),
        NamedView::new("rgb123", "Colour 123", rgb(1, 2, 3)),
        NamedView::new("rgb125", "Colour 125", rgb(1, 2, 5)),
    ];
    for c in 1..=6u16 {
        v.push(NamedView::new(
            format!("ch{c}"),
            format!("Channel {c}"),
            View::Grey {
                channel: c,
                invert: false,
            },
        ));
    }
    v
}

/// The picture layout of MSU-MR.
pub fn picture_layout() -> PictureLayout {
    PictureLayout {
        name: "MSU-MR",
        width: crate::instruments::msumr::LINE_WIDTH,
        max_rows: crate::instruments::msumr::MAX_ROWS * 8,
        full_mask: (1 << 14) - 1,
        views: views(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The 80k link: marker sync and the convolutional deinterleaver.

/// The marker every 80 channel bits.
pub const MARKER: u8 = 0x27;
/// Channel bits in a period.
pub const PERIOD_BITS: usize = 80;
/// Data bits in a period.
pub const DATA_BITS: usize = 72;
/// Branches of the interleaver.
pub const BRANCHES: usize = 36;
/// The delay unit of the interleaver, in bits: branch `i` of the transmitter delays by `i` units.
const UNIT: usize = 2048;
const WINDOW: usize = 48 * PERIOD_BITS;
/// Items to push through a fresh transmit interleaver to fill its memory: its longest delay.
const FILL: usize = (BRANCHES - 1) * BRANCHES * UNIT;

/// Marker sync and deinterleaving of LLRs (positive means 0).
pub struct Deinterleaver {
    hist: Vec<i8>,
    since_search: usize,
    lock: Option<(usize, u8, usize)>,
    pend: Option<i8>,
    net_bad: i32,
    inter: DelayLanes<i8>,
    marker_err: u32,
    /// Times the markers were lost.
    pub losses: u32,
    /// Locks found.
    pub locks: u32,
    scratch: Vec<u8>,
}

impl Default for Deinterleaver {
    fn default() -> Self {
        Self::new()
    }
}

impl Deinterleaver {
    /// A deinterleaver that searches for the markers.
    pub fn new() -> Self {
        Deinterleaver {
            hist: Vec::with_capacity(2 * WINDOW),
            since_search: 0,
            lock: None,
            pend: None,
            net_bad: 0,
            inter: DelayLanes::forney(BRANCHES as u32, UNIT as u32, true, 0),
            marker_err: 0,
            losses: 0,
            locks: 0,
            scratch: Vec::with_capacity(WINDOW),
        }
    }
    /// The markers are found.
    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }

    /// Searches the window. `first_abs` is the stream index of the first bit of the window.
    fn search(&mut self, first_abs: u64) -> Option<(usize, u8, usize)> {
        let h = &self.hist;
        let bits = &mut self.scratch;
        if h.len() < WINDOW + 2 {
            return None;
        }
        let base = h.len() - WINDOW - 2;
        let mut best = (0usize, 0usize, 0u8, 0usize);
        for p in 0..2usize {
            for t in 0..8u8 {
                // The hard bits of the window after transform t, at pairing p (a negative LLR is a 1).
                bits.clear();
                let mut j = base + p;
                while j + 1 < h.len() && bits.len() < WINDOW {
                    let (a, b) = transform(t, h[j], h[j + 1]);
                    bits.push((a < 0) as u8);
                    bits.push((b < 0) as u8);
                    j += 2;
                }
                if bits.len() < WINDOW {
                    continue;
                }
                for o in (0..PERIOD_BITS).step_by(2) {
                    let mut count = 0;
                    for m in 0..48 {
                        if o + m * PERIOD_BITS + 8 > bits.len() {
                            break;
                        }
                        let mut v = 0u8;
                        for b in 0..8 {
                            v = (v << 1) | bits[o + m * PERIOD_BITS + b];
                        }
                        if (v ^ MARKER).count_ones() <= 1 {
                            count += 1;
                        }
                    }
                    // Offset QPSK turned by a quarter turn moves the pairing by one bit and mirrors the pair, so a wrong pairing can
                    // show every marker too. Of two candidates with the same count, the one whose pairing agrees with its mirror
                    // (pairing 1 with a mirrored transform, pairing 0 without) is the true one.
                    let agrees = |p: usize, t: u8| (p == 1) == (t >= 4);
                    if count > best.0
                        || (count == best.0 && agrees(p, t) && !agrees(best.1, best.2))
                    {
                        best = (count, p, t, o);
                    }
                }
            }
        }
        if best.0 >= 36 {
            let start = first_abs as usize + base + best.1 + best.3;
            Some((
                start % PERIOD_BITS,
                best.2,
                (first_abs as usize + base + best.1) % 2,
            ))
        } else {
            None
        }
    }

    /// Runs LLRs, appending the deinterleaved LLRs of the data bits. `total_in` counts the input since the start.
    pub fn process(&mut self, soft: &[i8], out: &mut Vec<i8>, total_in: &mut u64) {
        for &s in soft {
            let idx = *total_in;
            *total_in += 1;
            match self.lock {
                None => {
                    self.hist.push(s);
                    // Trimmed in pieces (the search looks at the newest window only), so a bit costs no move of memory.
                    if self.hist.len() >= 2 * WINDOW {
                        self.hist
                            .drain(..self.hist.len() - (WINDOW + 2 * PERIOD_BITS));
                    }
                    self.since_search += 1;
                    if self.since_search >= 1600 {
                        self.since_search = 0;
                        let first_abs = idx + 1 - self.hist.len() as u64;
                        if let Some((phase, t, parity)) = self.search(first_abs) {
                            // Marker bits start where (stream index) % 80 == phase, pairs at the parity.
                            self.lock = Some((phase, t, parity));
                            self.locks += 1;
                            self.net_bad = 0;
                            self.pend = None;
                            self.inter.reset(0);
                        }
                    }
                }
                Some((phase, t, parity)) => {
                    let pos_abs = (idx as usize + PERIOD_BITS - phase) % PERIOD_BITS;
                    if (idx as usize) % 2 == parity {
                        self.pend = Some(s);
                        continue;
                    }
                    let Some(a) = self.pend.take() else { continue };
                    let (x, y) = transform(t, a, s);
                    let p0 = (pos_abs + PERIOD_BITS - 1) % PERIOD_BITS;
                    for (pos, v) in [(p0, x), (pos_abs, y)] {
                        self.bit(pos, v, out);
                    }
                }
            }
        }
    }

    fn bit(&mut self, pos: usize, v: i8, out: &mut Vec<i8>) {
        if pos < 8 {
            let want = (MARKER >> (7 - pos)) & 1;
            if ((v < 0) as u8) != want {
                self.marker_err += 1;
            }
            if pos == 7 {
                if self.marker_err >= 3 {
                    self.net_bad += 1;
                } else if self.net_bad > 0 {
                    self.net_bad -= 1;
                }
                self.marker_err = 0;
                if self.net_bad > 10 {
                    self.lock = None;
                    self.losses += 1;
                    self.hist.clear();
                }
            }
            return;
        }
        // Branch b of the deinterleaver is counted from the first data bit after a marker (72 data bits are two whole turns).
        if (pos - 8).is_multiple_of(BRANCHES) {
            self.inter.sync();
        }
        out.push(self.inter.push(v));
    }
}

/// Interleaves code bits and puts the markers in (the generator's side): data bits in, channel bits out.
pub struct Interleaver {
    inter: DelayLanes<u8>,
    pos: usize,
}

impl Interleaver {
    /// An interleaver whose memory holds the bits of `seed`, as a transmitter that has been running a while: the channel carries
    /// symbols that look random from the first bit. An interleaver with memory of zeros sends a carrier with no modulation for the
    /// first 18 s.
    pub fn new(seed: u64) -> Self {
        let mut inter = DelayLanes::forney(BRANCHES as u32, UNIT as u32, false, 0u8);
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        for _ in 0..FILL {
            inter.push((rng.next_u32() & 1) as u8);
        }
        inter.sync();
        Interleaver { inter, pos: 0 }
    }

    /// Appends the channel bits of `data`.
    pub fn push(&mut self, data: &[u8], out: &mut Vec<u8>) {
        for &v in data {
            if self.pos == 0 {
                for b in 0..8 {
                    out.push((MARKER >> (7 - b)) & 1);
                }
                self.inter.sync();
            }
            out.push(self.inter.push(v));
            self.pos = (self.pos + 1) % DATA_BITS;
        }
    }
}

/// The deinterleaver as a component: it passes the LLRs through when the link is not interleaved.
///
/// Control items: [`Control::Link`] switches the deinterleaver on or off.
#[derive(Default)]
pub struct DeinterleaverComponent {
    on: bool,
    d: Deinterleaver,
    out: Vec<i8>,
    total: u64,
}

impl DeinterleaverComponent {
    /// A deinterleaver that starts on (an interleaved link) or off.
    pub fn new(on: bool) -> Self {
        DeinterleaverComponent {
            on,
            ..DeinterleaverComponent::default()
        }
    }
}

/// The deinterleaver's report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeinterleaverReport {
    /// In use.
    pub on: bool,
    /// The markers are found.
    pub locked: bool,
    /// Times the markers were lost.
    pub losses: u32,
}
impl sdrkit_graph::Item for DeinterleaverReport {
    const KIND: &'static str = "satimg.lrpt_deinterleaver_report";
}

impl Component for DeinterleaverComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.lrpt_deinterleaver",
            vec![Port::llr("in"), control_port()],
            vec![Port::llr("out"), item_port::<DeinterleaverReport>("report")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.d = Deinterleaver::new();
        self.total = 0;
        let mut o = inputs[0].clone();
        if self.on {
            o.rate *= DATA_BITS as f64 / PERIOD_BITS as f64;
        }
        Ok(vec![o, item_format::<DeinterleaverReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        for c in controls(io, 1) {
            if let Control::Link(l) = c {
                if l.interleaved != self.on {
                    self.on = l.interleaved;
                    self.d = Deinterleaver::new();
                }
            }
        }
        if !self.on {
            let llrs = io.llrs(0);
            io.llrs_out(0).extend_from_slice(llrs);
        } else {
            self.out.clear();
            self.d.process(io.llrs(0), &mut self.out, &mut self.total);
            io.llrs_out(0).extend_from_slice(&self.out);
        }
        if io.tick() {
            let r = DeinterleaverReport {
                on: self.on,
                locked: self.d.locked(),
                losses: self.d.losses,
            };
            io.items_out::<DeinterleaverReport>(1).push(Timed {
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
// The display.

/// The row keys and names of the channels table: the image channels 1 to 6 (APID 64 to 69), then the telemetry (APID 70).
const CHANNEL_ROWS: [(&str, &str); 7] = [
    ("1", "Channel 1 (APID 64)"),
    ("2", "Channel 2 (APID 65)"),
    ("3", "Channel 3 (APID 66)"),
    ("4", "Channel 4 (APID 67)"),
    ("5", "Channel 5 (APID 68)"),
    ("6", "Channel 6 (APID 69)"),
    ("7", "Telemetry (APID 70)"),
];

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(LinkFields::SHAPE.fields)
            .with_fields(MeteorLrptFields::SHAPE.fields)
            .with_table(&METEOR_LRPT_CHANNELS)
            .with_table(&IMAGES),
    )
}

/// The display: states, fields, summary, histories and the automatic modulation.
pub struct MeteorDisplay {
    setting: Option<MeteorModulation>,
    running: MeteorModulation,
    preset: FrequencyPreset,
    retune: bool,
    published: Published,
    hist: Histories,
    first_decoding_s: Option<f64>,
    last_change_s: f64,
    last_good_s: f64,
    started: bool,
    pending: Option<MeteorModulation>,
    window: std::collections::VecDeque<(u64, u64)>,
    marker_losses: u32,
    summary: String,
}

impl Default for MeteorDisplay {
    fn default() -> Self {
        MeteorDisplay {
            setting: None,
            running: MeteorModulation::Oqpsk72,
            preset: FrequencyPreset::Mhz137_9,
            retune: false,
            published: super::display::published(display_declaration()),
            hist: Histories::default(),
            first_decoding_s: None,
            last_change_s: 0.0,
            last_good_s: 0.0,
            started: false,
            pending: None,
            window: Default::default(),
            marker_losses: 0,
            summary: String::new(),
        }
    }
}

impl Component for MeteorDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.meteor_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<DeinterleaverReport>("deinterleaver"),
                Port::typed::<CodeReport>("code"),
                Port::typed::<FrameReport>("frames"),
                Port::typed::<PacketReport>("packets"),
                Port::typed::<MsuReport>("msumr"),
            ],
            vec![
                item_port::<Control>("control"),
                item_port::<HostRequest>("requests"),
            ],
        )
        .with_settings(MeteorDisplaySetting::schema())
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
        let di = last::<DeinterleaverReport>(io, 2);
        let c = last::<CodeReport>(io, 3);
        let fr = last::<FrameReport>(io, 4);
        let pk = last::<PacketReport>(io, 5);
        let ms = last::<MsuReport>(io, 6);
        let t = f.map_or(0.0, |f| f.time_s);
        if std::mem::take(&mut self.retune) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: t,
                value: HostRequest::retune(self.preset.hz(), "Meteor frequency preset"),
            });
        }
        let control_out = |io: &mut Io, c: Control| {
            io.items_out::<Control>(0).push(Timed { time: t, value: c });
        };
        // The link starts as the setting (or OQPSK 72k) the first time.
        if !self.started {
            self.started = true;
            self.running = self.setting.unwrap_or(MeteorModulation::Oqpsk72);
            self.last_change_s = t;
            self.last_good_s = t;
            control_out(io, Control::Link(self.running.link()));
        }
        if let Some(m) = self.pending.take() {
            self.running = m;
            self.last_change_s = t;
            self.last_good_s = t;
            control_out(io, Control::Link(m.link()));
        }
        if let Some(fr) = fr {
            if fr.good_now > 0 {
                self.last_good_s = t;
            }
            log_lines(self.published.results_mut(), &fr.log);
        }
        let tracking = d.is_some_and(|d| d.state.tracking);
        let locked = d.is_some_and(|d| d.state.locked);
        let code = c.is_some_and(|c| c.lock.locked);
        let frame_lock = fr.is_some_and(|f| f.stats.locked);
        let decoding = fr.is_some_and(|f| f.since_good_s < 2.0 && f.stats.good > 0);
        let state = if decoding {
            3
        } else if frame_lock && code {
            2
        } else if tracking && locked {
            1
        } else {
            0
        };
        // Automatic: no code lock and no good frame for 8 s (40 s interleaved), the last change at least 2 s ago. Also when most
        // frames of that time failed Reed-Solomon (a neighbouring modulation half decodes).
        let wait = if self.running == MeteorModulation::Oqpsk80 {
            40.0
        } else {
            8.0
        };
        self.window
            .push_back(fr.map_or((0, 0), |f| (f.good_now, f.failed_now)));
        while self.window.len() as f64 * REPORT_S > wait {
            self.window.pop_front();
        }
        let (wg, wf) = self
            .window
            .iter()
            .fold((0, 0), |a, w| (a.0 + w.0, a.1 + w.1));
        let failing = wf > 5 && wf > wg && t - self.last_change_s >= wait;
        if tracking
            && ((!code && t - self.last_good_s >= wait) || failing)
            && t - self.last_change_s >= 2.0
        {
            self.window.clear();
            let next = match self.setting {
                None => self.running.next(),
                Some(m) => m,
            };
            let changed = next != self.running;
            self.running = next;
            self.last_change_s = t;
            self.last_good_s = t;
            if changed {
                control_out(io, Control::Link(next.link()));
            } else {
                control_out(io, Control::RestartCarrier);
            }
        }
        if let Some(di) = di.filter(|d| d.on) {
            if di.losses > self.marker_losses {
                self.published
                    .results_mut()
                    .log("LRPT: lost the interleaver markers");
            }
            self.marker_losses = di.losses;
        }
        if state == 3 && self.first_decoding_s.is_none() {
            self.first_decoding_s = Some(t);
            self.published.results_mut().log(format!(
                "LRPT: frames found, decoding ({})",
                self.running.name()
            ));
        }
        self.publish(state, t, f, d, di, c, fr, pk, ms);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MeteorDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MeteorDisplaySetting::Modulation => {
                self.setting = MODULATION_CHOICE.read(id, v)?.fixed();
                if let Some(m) = self.setting {
                    self.pending = Some(m);
                } else if self.started {
                    self.pending = Some(self.running);
                }
            }
            MeteorDisplaySetting::FrequencyPreset => {
                let p = FREQUENCY_PRESET.read(id, v)?;
                if p != self.preset {
                    self.retune = true;
                }
                self.preset = p;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let modulation = match self.setting {
            None => ModulationChoice::Auto,
            Some(MeteorModulation::Oqpsk72) => ModulationChoice::Oqpsk72,
            Some(MeteorModulation::Qpsk72) => ModulationChoice::Qpsk72,
            Some(MeteorModulation::Oqpsk80) => ModulationChoice::Oqpsk80,
        };
        values_of([
            (
                MeteorDisplaySetting::FrequencyPreset,
                Value::Choice(self.preset.index()),
            ),
            (
                MeteorDisplaySetting::Modulation,
                Value::Choice(modulation.index()),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

impl MeteorDisplay {
    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        state: u8,
        t: f64,
        f: Option<&FrontReport>,
        d: Option<&DemodReport>,
        di: Option<&DeinterleaverReport>,
        c: Option<&CodeReport>,
        fr: Option<&FrameReport>,
        pk: Option<&PacketReport>,
        ms: Option<&MsuReport>,
    ) {
        let m = self.running;
        let p = &mut self.published;
        put_link(p, t, f, d, c, fr, pk);
        let frames_good = fr.map_or(0, |f| f.stats.good);
        let rows = ms.map_or(0, |m| m.stats.rows);
        let lines = rows * 8;
        let summary = &mut self.summary;
        summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                summary,
                "{}, {} lines, {} frames",
                m.name(),
                lines,
                frames_good
            ),
            2 => write!(summary, "{}, frame sync, RS failing", m.name()),
            1 => write!(summary, "{}, symbols, no frame sync", m.name()),
            _ => write!(
                summary,
                "searching ({})",
                if self.setting.is_none() {
                    "automatic"
                } else {
                    m.name()
                }
            ),
        };
        status(p.results_mut(), state, f, d, summary);
        const SINGLE: [&str; 6] = ["1", "2", "3", "4", "5", "6"];
        let colour = ms.and_then(|m| {
            let has = |c: usize| m.stats.per_channel[c] > 0;
            if has(1) && has(2) && has(3) {
                Some("1,2,3")
            } else if has(1) && has(2) && has(5) {
                Some("1,2,5")
            } else {
                (1..=6).find(|&c| has(c)).map(|c| SINGLE[c - 1])
            }
        });
        p.put(&MeteorLrptFields {
            modulation: m,
            modulation_setting: match self.setting {
                None => ModulationChoice::Auto,
                Some(MeteorModulation::Oqpsk72) => ModulationChoice::Oqpsk72,
                Some(MeteorModulation::Qpsk72) => ModulationChoice::Qpsk72,
                Some(MeteorModulation::Oqpsk80) => ModulationChoice::Oqpsk80,
            },
            interleaver_locked: di.is_some_and(|d| d.locked),
            onboard_time: ms
                .and_then(|m| m.stats.onboard_ms)
                .map(|o| Seconds(o as f64 / 1000.0)),
            quality: ms.map(|m| m.stats.quality as i64),
            picture_rows: rows as i64,
            picture_lines: lines as i64,
            row_time: ms.and_then(|m| m.row_time_ms).map(|r| Seconds(r / 1000.0)),
            first_decoding: self.first_decoding_s.map(Seconds),
            colour_channels: colour,
        });
        if lines > 0 {
            IMAGES.put(
                p,
                "MSU-MR",
                &ImageRow {
                    product: "Meteor-M MSU-MR",
                    channel: colour,
                    time: self.first_decoding_s.map(Seconds),
                    width: i64::from(crate::instruments::msumr::LINE_WIDTH),
                    height: Some(lines as i64),
                },
            );
        }
        if let Some(ms) = ms {
            for (ch, (key, name)) in CHANNEL_ROWS.iter().enumerate() {
                let n = if ch < 6 {
                    ms.stats.per_channel[ch + 1]
                } else {
                    ms.stats.telemetry
                };
                METEOR_LRPT_CHANNELS.put(
                    p,
                    key,
                    &MeteorLrptChannelRow {
                        channel: name,
                        packets: n as i64,
                    },
                );
            }
        }
        self.hist.publish(self.published.results_mut(), d, fr);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The Meteor graph, starting with `start`.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layout produces no valid schema.
pub fn builder(start: MeteorModulation) -> Result<GraphBuilder, GraphError> {
    let link = start.link();
    let pictures = Pictures::new(vec![picture_layout()]).map_err(GraphError::Schema)?;
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(link.clone(), MIN_RATE, false).with_decimation_for(80e3),
        )
        .add("demod", DemodComponent::new(link.clone()))
        .add(
            "deinterleaver",
            DeinterleaverComponent::new(link.interleaved),
        )
        .add("decoder", CodeComponent::new(CodeConfig::LEO))
        .add("frames", FrameComponent::new(FrameLink::METEOR))
        .add("vcdu", VcduComponent::new(Layout::METEOR, false))
        .add("msumr", MsuMrComponent::default())
        .add("pictures", pictures)
        .add("display", MeteorDisplay::default())
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "deinterleaver.in")
        .connect("deinterleaver.out", "decoder.in")
        .connect("decoder.out", "frames.in")
        .connect("frames.out", "vcdu.in")
        .connect("vcdu.packets", "msumr.in")
        .connect("msumr.out", "pictures.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("deinterleaver.report", "display.deinterleaver")
        .connect("decoder.report", "display.code")
        .connect("frames.report", "display.frames")
        .connect("vcdu.report", "display.packets")
        .connect("msumr.report", "display.msumr")
        .feedback("display.control", "demod.control")
        .feedback("display.control", "deinterleaver.control")
        .feedback("display.control", "decoder.control")
        .output("pictures.picture")
        .output("display.requests")
        .status_from("display")
        .clock(REPORT_S))
}

/// The Meteor receiver: the graph with the stack's settings.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
///
/// [`StackGraphBuilder::build`]: sdrkit_graph::StackGraphBuilder::build
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let start = cfg
        .settings
        .get(&MODULATION_CHOICE)
        .fixed()
        .unwrap_or(MeteorModulation::Oqpsk72);
    let mut g = builder(start)?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "Meteor-M LRPT needs at least 250 kHz of samples")
        .route("frequency_preset", &["display.frequency_preset"])?
        .route("modulation", &["display.modulation"])?
        .route("view", &["pictures.view"])?
        .route("enhance", &["pictures.enhance"])?
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
        MeteorGenerator::new(MeteorGenParams::from_values(
            cfg.rate_hz,
            cfg.seed,
            &cfg.settings,
        ))
        .map_err(super::gen_error)?,
    ))
}

/// The stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Meteor-M LRPT",
    group: crate::GROUP,
    blurb: "Pictures from the Meteor-M weather satellites as they pass over, on 137 MHz.",
    tuning: Tuning {
        min_hz: 136e6,
        max_hz: 139e6,
        default_hz: 137.9e6,
        rate_hz: 1e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 150e3,
        tune_offset_hz: 200e3,
        ..Tuning::anywhere(137.9e6, 1e6, MIN_RATE, 150e3)
    },
    submodes: &[],
    settings: MeteorSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Lrpt],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::METEOR_HOOKS),
    benchmarks: None,
};

/// The idle hint before start.
pub const IDLE_HINT: &str =
    "Start the stack during a pass (the pass planner can start it at AOS and follow the Doppler).";

// ---------------------------------------------------------------------------------------------------------------------------------
// The generator.

/// The options of the generator.
#[derive(Clone, Debug, PartialEq)]
pub struct MeteorGenParams {
    /// The modulation.
    pub modulation: MeteorModulation,
    /// Night channels (1, 2, 5: APIDs 64, 65, 68) instead of day (1, 2, 3).
    pub night: bool,
    /// Rows before the picture repeats.
    pub rows: u32,
    /// The seed of the memory of the interleaver.
    pub seed: u64,
    /// The options of the modulator.
    pub modem: ModParams,
}

impl MeteorGenParams {
    /// The defaults at `rate`.
    pub fn new(rate: f64, modulation: MeteorModulation) -> Self {
        let l = modulation.link();
        MeteorGenParams {
            modulation,
            night: false,
            rows: 40,
            seed: 1,
            modem: ModParams::new(rate, l.symbol_rate, 0.6, l.modulation),
        }
    }
    /// The options of a generator configuration: `gen_modulation`, `gen_night` and `gen_rows`. The schema of `v` must be
    /// [`MeteorSetting`] (with the generator settings).
    pub fn from_values(rate: f64, seed: u64, v: &Values) -> Self {
        let mut p = MeteorGenParams::new(rate, v.get(&GEN_MODULATION));
        p.night = v.get(&GEN_NIGHT);
        p.rows = v.get(&GEN_ROWS).clamp(1, 1500) as u32;
        p.seed = seed;
        p
    }
}

/// Makes the Meteor channel bits.
pub struct MeteorSource {
    p: MeteorGenParams,
    coder: CcsdsCoder,
    inter: Option<Interleaver>,
    zones: ZoneStream,
    counter: u32,
    seq: u16,
    row: u32,
    bits: Vec<u8>,
}

impl MeteorSource {
    /// A source.
    pub fn new(p: MeteorGenParams) -> Self {
        let nrzm = p.modulation != MeteorModulation::Qpsk72;
        MeteorSource {
            coder: CcsdsCoder::new(CodeOptions {
                nrzm,
                invert_g2: false,
                conv: true,
            }),
            inter: (p.modulation == MeteorModulation::Oqpsk80).then(|| Interleaver::new(p.seed)),
            p,
            zones: ZoneStream::default(),
            counter: 0,
            seq: 0,
            row: 0,
            bits: Vec::new(),
        }
    }

    /// The pixels of the test picture for `channel` at picture row `row` (8 lines), from MCU `mcu`.
    pub fn strip_pixels(channel: u16, row: u32, mcu: u8) -> Vec<u8> {
        let mut px = vec![0u8; 8 * 112];
        for y in 0..8 {
            for x in 0..112 {
                px[y * 112 + x] =
                    test_pixel(channel, mcu as u32 * 8 + x as u32, row * 8 + y as u32, 1568);
            }
        }
        px
    }

    /// The channels sent.
    pub fn channels(night: bool) -> [u16; 3] {
        if night {
            [1, 2, 5]
        } else {
            [1, 2, 3]
        }
    }

    fn queue_row(&mut self) {
        let r = self.row % self.p.rows;
        let ms = 36_000_000 + self.row * 1230;
        for ch in Self::channels(self.p.night) {
            for k in 0..14u8 {
                let px = Self::strip_pixels(ch, r, k * 14);
                let user = crate::instruments::msumr::build_packet(20_000, ms, k * 14, 80, &px);
                self.zones
                    .push(&space_packet(63 + ch, 3, self.seq, &user, false));
                self.seq = (self.seq + 1) & 0x3FFF;
            }
        }
        // One telemetry packet in each period.
        self.zones
            .push(&space_packet(70, 3, self.seq, &[0u8; 40], false));
        self.seq = (self.seq + 1) & 0x3FFF;
        self.row += 1;
    }
}

impl BitSource for MeteorSource {
    fn next_bits(&mut self, out: &mut Vec<u8>) {
        let layout = Layout::METEOR;
        let zl = layout.data_zone_len();
        while self.zones.len() < zl {
            self.queue_row();
        }
        let (fhp, zone) = self.zones.take(zl);
        let v = aos_frame(&layout, 156, 5, self.counter, fhp, &zone);
        self.counter = (self.counter + 1) & 0xFF_FFFF;
        let cadu = cadu_bytes(&v, Basis::Conventional, true);
        self.bits.clear();
        self.coder.push(&cadu, &mut self.bits);
        match &mut self.inter {
            Some(i) => i.push(&self.bits, out),
            None => out.extend_from_slice(&self.bits),
        }
    }
}

/// The Meteor test signal: the clean signal at unit mean power.
pub struct MeteorGenerator {
    rate: f64,
    info: SignalInfo,
    m: Modulator<MeteorSource>,
}

impl MeteorGenerator {
    /// With these options.
    ///
    /// # Errors
    ///
    /// [`GenError`] if the modulator refuses them.
    pub fn new(p: MeteorGenParams) -> Result<Self, GenError> {
        Ok(MeteorGenerator {
            rate: p.modem.rate,
            info: p.modem.signal_info(),
            m: Modulator::new(p.modem.clone(), MeteorSource::new(p))?,
        })
    }
}

impl Generator for MeteorGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.m.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
    /// The carrier is at 0 Hz, on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}

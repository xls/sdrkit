//! The geostationary xRIT stacks: GOES HRIT (`goes_hrit`), GK-2A LRIT and HRIT (`gk2a_lrit`, `gk2a_hrit`) and Elektro-L HRIT,
//! LRIT and GGAK (`elektro_l`). The submode `arktika_ggak` of `elektro_l` is the GGAK chain under the name of Arktika-M.

use super::display::{log_lines, put_link, status, with_series, Histories};
use crate::components::link::{
    control_port, item_format, item_port, CodeComponent, CodeReport, DemodComponent, DemodReport,
    FrameComponent, FrameLink, FrameReport, FrontEndComponent, FrontReport, PacketReport,
    VcduComponent,
};
use crate::components::xrit::{
    FilesComponent, FilesReport, ProductComponent, StoreReport, FILTER, FOLLOW_NEWEST,
};
use crate::components::{controls, last, Control};
use crate::instruments::ggak::{magnitude, GgakFramer};
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use crate::typed::{ArktikaFields, LinkFields, VirtualChannelRow, XritFields, VIRTUAL_CHANNELS};
use crate::verify::{at_least, count_at_least};
use crate::xrit::store::StoreConfig;
use crate::REPORT_S;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, Record, Results,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, SubMode, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Item, Port, StackGraph, Timed,
};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::codesync::CodeConfig;
use sdrkit_proto_ccsds::xrit::headers::Dialect;
use std::fmt::Write as _;
use std::sync::Arc;

/// Which xRIT stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XritMode {
    /// GOES HRIT.
    Goes,
    /// GK-2A LRIT.
    Gk2aLrit,
    /// GK-2A HRIT.
    Gk2aHrit,
    /// Elektro-L.
    Elektro,
}

/// The submode of `elektro_l` for the GGAK link of Arktika-M.
pub const ARKTIKA_SUBMODE: &str = "arktika_ggak";
/// The name of Arktika-M in the summary.
const ARKTIKA: &str = "Arktika-M";
/// Seconds after the last good GGAK frame during which the submode `arktika_ggak` stays locked.
const ARKTIKA_HOLD_S: f64 = 3.0;
/// What the submode `arktika_ggak` says about itself.
const ARKTIKA_NOTE: &str = "Experimental: not confirmed on a real signal. Not confirmed by a public document: the GGAK frame \
    of 224 bytes on 1ACFFC1D with each frame sent twice, the CRC-16 over bytes 4 to 221, the meaning of the fields (marker \
    and sub-frame, time stamp, the magnetometer triples), and the modulation: whether the link has a residual carrier or \
    is suppressed-carrier BPSK as received here.";

/// A BPSK geostationary link at `rs` symbols per second.
pub fn bpsk_geo(rs: f64) -> LinkParams {
    LinkParams {
        modulation: Modulation::Bpsk,
        symbol_rate: rs,
        roll_off: 0.5,
        class: LinkClass::Geo,
        search: Search::PowerLine {
            power: 2,
            min_ratio: 0.0,
        },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: true,
        constellation: 512,
        interleaved: false,
    }
}

/// The GK-2A HRIT link: QPSK, G1 on I, G2 on Q.
pub fn qpsk_geo(rs: f64) -> LinkParams {
    LinkParams {
        modulation: Modulation::Qpsk,
        search: Search::PowerLine {
            power: 4,
            min_ratio: 0.0,
        },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: false,
        constellation: 256,
        ..bpsk_geo(rs)
    }
}

/// The GGAK link: BPSK 5 ksym/s (PCM/PM, the residual carrier taken as BPSK).
pub fn ggak_link() -> LinkParams {
    LinkParams {
        class: LinkClass::Ggak,
        search: Search::PowerLine {
            power: 2,
            min_ratio: 12.0,
        },
        bpsk_plus_is_zero: false,
        constellation: 256,
        ..bpsk_geo(5000.0)
    }
}

impl XritMode {
    /// The stack id.
    pub fn id(self) -> &'static str {
        match self {
            XritMode::Goes => "goes_hrit",
            XritMode::Gk2aLrit => "gk2a_lrit",
            XritMode::Gk2aHrit => "gk2a_hrit",
            XritMode::Elektro => "elektro_l",
        }
    }
    /// The default link.
    pub fn link(self) -> LinkParams {
        match self {
            XritMode::Goes => bpsk_geo(927_000.0),
            XritMode::Gk2aLrit => bpsk_geo(128_000.0),
            XritMode::Gk2aHrit => qpsk_geo(3_000_000.0),
            XritMode::Elektro => bpsk_geo(665_400.0),
        }
    }
    /// The least input rate for a link.
    pub fn min_rate(self, l: &LinkParams) -> f64 {
        match self {
            XritMode::Goes => (2.1 * l.symbol_rate).max(200e3),
            XritMode::Gk2aLrit | XritMode::Gk2aHrit => 2.1 * l.symbol_rate,
            XritMode::Elektro if l.class == LinkClass::Ggak => (2.1 * l.symbol_rate).max(20e3),
            XritMode::Elektro => (2.1 * l.symbol_rate).max(200e3),
        }
    }
    fn store(self) -> StoreConfig {
        match self {
            XritMode::Goes | XritMode::Elektro => StoreConfig::goes(),
            XritMode::Gk2aLrit => StoreConfig::gk2a("LRIT"),
            XritMode::Gk2aHrit => StoreConfig::gk2a("HRIT"),
        }
    }
    fn dialect(self) -> Dialect {
        match self {
            XritMode::Gk2aLrit | XritMode::Gk2aHrit => Dialect::Gk2a,
            _ => Dialect::Goes,
        }
    }
    fn gk2a(self) -> bool {
        matches!(self, XritMode::Gk2aLrit | XritMode::Gk2aHrit)
    }
    /// The frequency, Hz.
    pub fn frequency(self) -> f64 {
        match self {
            XritMode::Goes => 1694.1e6,
            XritMode::Gk2aLrit => 1692.14e6,
            XritMode::Gk2aHrit => 1695.4e6,
            XritMode::Elektro => 1691.0e6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// An Elektro-L satellite.
    pub enum ElektroCraft {
        /// Elektro-L N3 (76°E).
        N3 = ("n3", "Elektro-L N3 (76°E)"),
        /// Elektro-L N4 (165.8°E).
        N4 = ("n4", "Elektro-L N4 (165.8°E)"),
        /// Elektro-L N5 (14.5°W).
        N5 = ("n5", "Elektro-L N5 (14.5°W)"),
    }
}

impl ElektroCraft {
    /// The name in the summary and the product names.
    pub fn name(self) -> &'static str {
        match self {
            ElektroCraft::N3 => "Elektro-L N3",
            ElektroCraft::N4 => "Elektro-L N4",
            ElektroCraft::N5 => "Elektro-L N5",
        }
    }
}

sdrkit_core::choice_enum! {
    /// A link of Elektro-L.
    pub enum ElektroLink {
        /// HRIT at 665.4 ksym/s.
        Hrit665 = ("hrit665", "HRIT 665.4 ksym/s"),
        /// HRIT at 1.0 Msym/s.
        Hrit1000 = ("hrit1000", "HRIT 1.0 Msym/s"),
        /// LRIT at 128 ksym/s.
        Lrit128 = ("lrit128", "LRIT 128 ksym/s"),
        /// LRIT at 64 ksym/s.
        Lrit64 = ("lrit64", "LRIT 64 ksym/s"),
        /// GGAK telemetry at 5 ksym/s.
        Ggak = ("ggak", "GGAK 5 ksym/s"),
    }
}

impl ElektroLink {
    /// The name in the summary.
    pub fn name(self) -> &'static str {
        match ElektroLink::CHOICES[usize::from(self.index())].label {
            std::borrow::Cow::Borrowed(l) => l,
            std::borrow::Cow::Owned(_) => "",
        }
    }
    /// The link.
    pub fn link(self) -> LinkParams {
        match self {
            ElektroLink::Hrit665 => bpsk_geo(665_400.0),
            ElektroLink::Hrit1000 => bpsk_geo(1_000_000.0),
            ElektroLink::Lrit128 => bpsk_geo(128_000.0),
            ElektroLink::Lrit64 => bpsk_geo(64_000.0),
            ElektroLink::Ggak => ggak_link(),
        }
    }
    /// The frequency, Hz.
    pub fn frequency(self) -> f64 {
        match self {
            ElektroLink::Ggak => 1693.0e6,
            _ => 1691.0e6,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The basis of the Reed-Solomon symbols.
    pub enum RsBasis {
        /// The dual basis of CCSDS.
        Dual = ("dual", "Dual"),
        /// The conventional basis.
        Conventional = ("conventional", "Conventional"),
    }
}

const TEST: &str = "Test signal";

/// The Elektro-L satellite (names only: summary and product names).
pub const CRAFT: Setting<field::Choice<ElektroCraft>> =
    Setting::choice("craft", "Satellite", ElektroCraft::N3)
        .help("Names only (summary and product names).");
/// The Elektro-L link.
pub const LINK_SETTING: Setting<field::Choice<ElektroLink>> =
    Setting::choice("link", "Link", ElektroLink::Hrit665)
        .help("Restarts the demodulator (the products stay).");
/// The test signal sends the second code output inverted.
pub const GEN_INVERT_G2: Setting<field::Bool> =
    Setting::new("gen_invert_g2", "Test G2 inverted", "", true)
        .section(TEST)
        .advanced();
/// The test signal codes NRZ-M before the convolutional code.
pub const GEN_NRZM: Setting<field::Bool> = Setting::new("gen_nrzm", "Test NRZ-M", "", false)
    .section(TEST)
    .advanced();
/// The Reed-Solomon basis of the test signal.
pub const GEN_BASIS: Setting<field::Choice<RsBasis>> =
    Setting::choice("gen_basis", "Test Reed-Solomon basis", RsBasis::Dual)
        .section(TEST)
        .advanced();
/// Fill frames after each round of test files (GOES and Elektro-L).
pub const GEN_FILL: Setting<field::I64> = Setting::new("gen_fill", "Test fill frames", "", 250)
    .range(0.0, 100_000.0, 1.0)
    .section(TEST)
    .advanced();
/// Fill frames after each round of test files (GK-2A).
pub const GEN_FILL_GK2A: Setting<field::I64> = Setting::new("gen_fill", "Test fill frames", "", 20)
    .range(0.0, 100_000.0, 1.0)
    .section(TEST)
    .advanced();
/// The test signal sends its files once.
pub const GEN_ONCE: Setting<field::Bool> = Setting::new("gen_once", "Test files once", "", false)
    .section(TEST)
    .advanced();
/// The first frame count of the test signal.
pub const GEN_COUNTER: Setting<field::I64> =
    Setting::new("gen_counter", "Test frame count start", "", 0)
        .range(0.0, 16_777_215.0, 1.0)
        .section(TEST)
        .advanced();
/// The GK-2A test signal encrypts its VI006 segments.
pub const GEN_ENCRYPT: Setting<field::Bool> =
    Setting::new("gen_encrypt", "Test encrypted VI006", "", false)
        .section(TEST)
        .advanced();
/// The link of the Elektro-L test signal.
pub const GEN_LINK: Setting<field::Choice<ElektroLink>> =
    Setting::choice("gen_link", "Test link", ElektroLink::Hrit665)
        .section(TEST)
        .advanced();

sdrkit_core::settings! {
    /// The settings of `goes_hrit`, with the options of its test signal.
    pub enum GoesSetting {
        /// Which products the table lists.
        Filter = FILTER,
        /// Show the newest product.
        FollowNewest = FOLLOW_NEWEST,
        /// Test: G2 inverted.
        GenInvertG2 = GEN_INVERT_G2,
        /// Test: NRZ-M.
        GenNrzm = GEN_NRZM,
        /// Test: the Reed-Solomon basis.
        GenBasis = GEN_BASIS,
        /// Test: fill frames.
        GenFill = GEN_FILL,
        /// Test: the files once.
        GenOnce = GEN_ONCE,
        /// Test: the first frame count.
        GenCounter = GEN_COUNTER,
    }
}

sdrkit_core::settings! {
    /// The settings of `gk2a_lrit` and `gk2a_hrit`, with the options of their test signal.
    pub enum Gk2aSetting {
        /// Which products the table lists.
        Filter = FILTER,
        /// Show the newest product.
        FollowNewest = FOLLOW_NEWEST,
        /// Test: G2 inverted.
        GenInvertG2 = GEN_INVERT_G2,
        /// Test: NRZ-M.
        GenNrzm = GEN_NRZM,
        /// Test: the Reed-Solomon basis.
        GenBasis = GEN_BASIS,
        /// Test: fill frames.
        GenFill = GEN_FILL_GK2A,
        /// Test: the files once.
        GenOnce = GEN_ONCE,
        /// Test: the first frame count.
        GenCounter = GEN_COUNTER,
        /// Test: encrypted VI006.
        GenEncrypt = GEN_ENCRYPT,
    }
}

sdrkit_core::settings! {
    /// The settings of `elektro_l`, with the options of its test signal.
    pub enum ElektroSetting {
        /// The satellite.
        Craft = CRAFT,
        /// The link.
        Link = LINK_SETTING,
        /// Which products the table lists.
        Filter = FILTER,
        /// Show the newest product.
        FollowNewest = FOLLOW_NEWEST,
        /// Test: G2 inverted.
        GenInvertG2 = GEN_INVERT_G2,
        /// Test: NRZ-M.
        GenNrzm = GEN_NRZM,
        /// Test: the Reed-Solomon basis.
        GenBasis = GEN_BASIS,
        /// Test: fill frames.
        GenFill = GEN_FILL,
        /// Test: the files once.
        GenOnce = GEN_ONCE,
        /// Test: the first frame count.
        GenCounter = GEN_COUNTER,
        /// Test: the link.
        GenLink = GEN_LINK,
    }
}

sdrkit_core::settings! {
    /// The settings of the Elektro-L display.
    pub enum ElektroDisplaySetting {
        /// The satellite.
        Craft = CRAFT,
        /// The link.
        Link = LINK_SETTING,
    }
}

/// The settings of a stack, with the options of its test signal.
pub fn schema_for(m: XritMode) -> Arc<Schema> {
    match m {
        XritMode::Goes => GoesSetting::schema(),
        XritMode::Gk2aLrit | XritMode::Gk2aHrit => Gk2aSetting::schema(),
        XritMode::Elektro => ElektroSetting::schema(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The report of the GGAK decoder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GgakReport {
    /// In use (the GGAK link is selected).
    pub on: bool,
    /// Frames (not counting duplicates).
    pub frames: u64,
    /// Good CRCs.
    pub good: u64,
    /// Bad CRCs.
    pub bad: u64,
    /// Duplicates.
    pub duplicates: u64,
    /// The marker of the newest frame.
    pub marker: Option<u8>,
    /// The first field triple of the newest frame, when its CRC is good.
    pub field: Option<[i16; 3]>,
}
impl Item for GgakReport {
    const KIND: &'static str = "satimg.ggak_report";
}

/// The `clear` command of the GGAK decoder.
pub const GGAK_CLEAR: Command = Command::new("clear", "Clear").help("Forgets the newest frame.");

sdrkit_core::commands! {
    /// The commands of [`GgakComponent`].
    pub enum GgakCommand {
        /// Forgets the newest frame.
        Clear = GGAK_CLEAR,
    }
}

/// The GGAK decoder: LLRs in (hard decisions), frames out as [`crate::items::GgakFrame`] items, which leave the graph as
/// events.
///
/// Control items: [`Control::Link`] turns it on for the GGAK link only.
#[derive(Default)]
pub struct GgakComponent {
    on: bool,
    framer: GgakFramer,
    bits: Vec<u8>,
    frames: Vec<crate::items::GgakFrame>,
    marker: Option<u8>,
    field: Option<[i16; 3]>,
}

impl GgakComponent {
    /// A decoder that starts on (the GGAK link) or off.
    pub fn new(on: bool) -> Self {
        GgakComponent {
            on,
            ..GgakComponent::default()
        }
    }
}

impl Component for GgakComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.ggak",
            vec![Port::llr("in"), control_port()],
            vec![
                item_port::<crate::items::GgakFrame>("events"),
                item_port::<GgakReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            item_format::<crate::items::GgakFrame>(),
            item_format::<GgakReport>(),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        for c in controls(io, 1) {
            if let Control::Link(l) = c {
                self.on = l.class == LinkClass::Ggak;
            }
        }
        if self.on {
            self.bits.clear();
            self.bits.extend(io.llrs(0).iter().map(|&s| (s < 0) as u8));
            self.framer.process(&self.bits, 0.0, &mut self.frames);
            for f in &self.frames {
                self.marker = Some(f.marker);
                if f.crc_ok {
                    self.field = f.field.first().copied();
                }
            }
            io.items_out::<crate::items::GgakFrame>(0)
                .extend(self.frames.drain(..).map(|f| Timed {
                    time: f.time,
                    value: f,
                }));
        }
        if io.tick() {
            let r = GgakReport {
                on: self.on,
                frames: self.framer.frames,
                good: self.framer.good,
                bad: self.framer.bad,
                duplicates: self.framer.duplicates,
                marker: self.marker,
                field: self.field,
            };
            io.items_out::<GgakReport>(1).push(Timed {
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
    fn commands(&self) -> &Commands {
        GgakCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match GgakCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            GgakCommand::Clear => {
                self.marker = None;
                self.field = None;
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(LinkFields::SHAPE.fields)
            .with_fields(XritFields::SHAPE.fields)
            .with_table(&VIRTUAL_CHANNELS),
    )
}

/// What an xRIT stack publishes, with its event kinds.
pub fn declaration_for(m: XritMode) -> ResultsDecl {
    crate::typed::shape(m.id()).results_decl()
}

/// What GOES HRIT publishes.
pub fn goes_declaration() -> ResultsDecl {
    declaration_for(XritMode::Goes)
}
/// What GK-2A LRIT publishes.
pub fn gk2a_lrit_declaration() -> ResultsDecl {
    declaration_for(XritMode::Gk2aLrit)
}
/// What GK-2A HRIT publishes.
pub fn gk2a_hrit_declaration() -> ResultsDecl {
    declaration_for(XritMode::Gk2aHrit)
}
/// What Elektro-L publishes.
pub fn elektro_declaration() -> ResultsDecl {
    declaration_for(XritMode::Elektro)
}

/// The display of the xRIT stacks.
pub struct XritDisplay {
    mode: XritMode,
    craft: ElektroCraft,
    link_value: ElektroLink,
    published: Published,
    hist: Histories,
    first_decoding_s: Option<f64>,
    track_since: Option<f64>,
    locked_once: bool,
    frames_logged: bool,
    ggak_field: Option<f64>,
    link_changed: bool,
    arktika: bool,
    ggak_good: u64,
    ggak_good_at: f64,
    summary: String,
    hint: String,
    key: String,
}

impl XritDisplay {
    /// The display of a stack. `link` is the Elektro-L link the chain starts on.
    pub fn new(mode: XritMode, link: ElektroLink) -> Self {
        XritDisplay {
            mode,
            craft: ElektroCraft::N3,
            link_value: link,
            published: super::display::published(display_declaration()),
            hist: Histories::default(),
            first_decoding_s: None,
            track_since: None,
            locked_once: false,
            frames_logged: false,
            ggak_field: None,
            link_changed: false,
            arktika: false,
            ggak_good: 0,
            ggak_good_at: 0.0,
            summary: String::new(),
            hint: String::new(),
            key: String::new(),
        }
    }
    /// The display of the submode `arktika_ggak`: the GGAK link of Elektro-L under the name of Arktika-M, with the note. The
    /// settings `link` and `craft` are taken and have no effect.
    pub fn arktika() -> Self {
        XritDisplay {
            arktika: true,
            published: super::display::published(
                display_declaration().with_fields(ArktikaFields::SHAPE.fields),
            ),
            ..XritDisplay::new(XritMode::Elektro, ElektroLink::Ggak)
        }
    }
    fn link(&self) -> (&'static str, LinkParams) {
        match self.mode {
            XritMode::Elektro => (self.link_value.name(), self.link_value.link()),
            XritMode::Goes => ("HRIT", self.mode.link()),
            XritMode::Gk2aLrit => ("LRIT", self.mode.link()),
            XritMode::Gk2aHrit => ("HRIT", self.mode.link()),
        }
    }
    fn tag(&self) -> &'static str {
        match self.mode {
            XritMode::Goes => "HRIT",
            XritMode::Gk2aLrit => "GK-2A LRIT",
            XritMode::Gk2aHrit => "GK-2A HRIT",
            XritMode::Elektro if self.arktika || self.link_value == ElektroLink::Ggak => "GGAK",
            XritMode::Elektro => self.craft.name(),
        }
    }
}

/// The newest reports a display reads at a tick.
struct Reports<'a> {
    f: Option<&'a FrontReport>,
    d: Option<&'a DemodReport>,
    c: Option<&'a CodeReport>,
    fr: Option<&'a FrameReport>,
    pk: Option<&'a PacketReport>,
    fl: Option<&'a FilesReport>,
    st: Option<&'a StoreReport>,
    gg: Option<&'a GgakReport>,
}

impl Component for XritDisplay {
    fn info(&self) -> ComponentInfo {
        let info = ComponentInfo::processor(
            "satimg.xrit_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<CodeReport>("code"),
                Port::typed::<FrameReport>("frames"),
                Port::typed::<PacketReport>("packets"),
                Port::typed::<FilesReport>("files"),
                Port::typed::<StoreReport>("store"),
                Port::typed::<GgakReport>("ggak"),
            ],
            vec![item_port::<Control>("control")],
        );
        if self.mode == XritMode::Elektro {
            info.with_settings(ElektroDisplaySetting::schema())
        } else {
            info
        }
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.track_since = None;
        Ok(vec![item_format::<Control>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let r = Reports {
            f: last::<FrontReport>(io, 0),
            d: last::<DemodReport>(io, 1),
            c: last::<CodeReport>(io, 2),
            fr: last::<FrameReport>(io, 3),
            pk: last::<PacketReport>(io, 4),
            fl: last::<FilesReport>(io, 5),
            st: last::<StoreReport>(io, 6),
            gg: last::<GgakReport>(io, 7),
        };
        let t = r.f.map_or(0.0, |f| f.time_s);
        let (_, link) = self.link();
        if std::mem::take(&mut self.link_changed) {
            let out = io.items_out::<Control>(0);
            out.push(Timed {
                time: t,
                value: Control::MinRate(self.mode.min_rate(&link)),
            });
            out.push(Timed {
                time: t,
                value: Control::Link(link.clone()),
            });
        }
        let ggak = link.class == LinkClass::Ggak;
        let rs = link.symbol_rate;
        let frame_s = 16384.0 / (rs * link.modulation.bits_per_symbol() as f64) * 2.0;
        let tracking = r.d.is_some_and(|d| d.state.tracking);
        let locked = r.d.is_some_and(|d| d.state.locked);
        let imported = r.fr.is_some_and(|f| f.imported > 0);
        let decoding = r.fr.is_some_and(|f| {
            f.stats.good > 0 && f.since_good_s <= (4.0 * frame_s).max(REPORT_S + 0.05)
        }) || (imported && r.fl.is_some_and(|f| f.stats.files > 0));
        let frame_lock = r.fr.is_some_and(|f| f.stats.locked);
        let too_low = r.f.is_some_and(|f| f.rate_too_low);
        let ggak_frames = r.gg.map_or(0, |g| g.frames);
        let ggak_good = r.gg.map_or(0, |g| g.good);
        if ggak_good > self.ggak_good {
            self.ggak_good = ggak_good;
            self.ggak_good_at = t;
        }
        let mut state = if too_low {
            0
        } else if ggak {
            if self.arktika && ggak_good > 0 && t - self.ggak_good_at < ARKTIKA_HOLD_S {
                3
            } else if ggak_frames > 0 {
                2
            } else if locked {
                1
            } else {
                0
            }
        } else if decoding {
            3
        } else if frame_lock {
            2
        } else if locked {
            1
        } else {
            0
        };
        if state > 0 && !tracking && !imported {
            state = 0;
        }
        // A carrier that holds without frames for 120 000 symbols is rejected and searched again.
        if tracking {
            let since = *self.track_since.get_or_insert(t);
            let no_frames =
                r.fr.is_some_and(|f| f.stats.good == 0 || f.since_good_s > 120_000.0 / rs);
            if !ggak && no_frames && t - since > 120_000.0 / rs && t - since > 2.0 {
                io.items_out::<Control>(0).push(Timed {
                    time: t,
                    value: Control::RejectCarrier,
                });
                self.track_since = None;
            }
        } else {
            self.track_since = None;
        }
        let tag = self.tag();
        let res = self.published.results_mut();
        if locked && !self.locked_once {
            self.locked_once = true;
            res.log(format!(
                "{tag}: carrier locked, offset {:.1} kHz",
                super::display::carrier_hz(r.f, r.d) / 1e3
            ));
        }
        // The log keeps 200 lines, 80 on the GGAK link.
        res.set_log_cap(if ggak { 80 } else { 200 });
        if ggak && ggak_frames > 0 && !self.frames_logged {
            self.frames_logged = true;
            let marker = r.gg.and_then(|g| g.marker).unwrap_or(0);
            res.log(format!("GGAK: frames locked (marker {marker})"));
        }
        if state == 3 && !self.frames_logged {
            self.frames_logged = true;
            res.log(format!("{tag}: frames found, decoding"));
            self.first_decoding_s = Some(t);
        }
        if let Some(fr) = r.fr {
            log_lines(res, &fr.log);
        }
        if let Some(fl) = r.fl {
            log_lines(res, &fl.log);
        }
        if let Some(st) = r.st {
            log_lines(res, &st.log);
        }
        if let Some(fr) = r.fr.filter(|f| !f.import_note.is_empty()) {
            res.log(fr.import_note.as_str());
        }
        self.publish(state, t, &r);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ElektroDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ElektroDisplaySetting::Craft => self.craft = CRAFT.read(id, v)?,
            ElektroDisplaySetting::Link => {
                let l = LINK_SETTING.read(id, v)?;
                if l != self.link_value && !self.arktika {
                    self.link_value = l;
                    self.link_changed = true;
                    self.locked_once = false;
                    self.frames_logged = false;
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        if self.mode != XritMode::Elektro {
            return no_values();
        }
        values_of([
            (
                ElektroDisplaySetting::Craft,
                Value::Choice(self.craft.index()),
            ),
            (
                ElektroDisplaySetting::Link,
                Value::Choice(self.link_value.index()),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

impl XritDisplay {
    fn write_summary(&mut self, state: u8, link_name: &str, link: &LinkParams, r: &Reports) {
        let carrier = super::display::carrier_hz(r.f, r.d);
        let es = r.d.map_or(-10.0, |d| d.state.es_n0_db);
        let good = r.fr.map_or(0, |f| f.stats.good);
        let bad = r.fr.map_or(0, |f| f.stats.failed);
        let products = r.st.map_or(0, |s| s.products);
        let rate_ks = link.symbol_rate / 1e3;
        let craft = if self.arktika {
            ARKTIKA
        } else {
            self.craft.name()
        };
        let s = &mut self.summary;
        s.clear();
        let name = match self.mode {
            XritMode::Goes => "HRIT",
            XritMode::Gk2aLrit => "GK-2A LRIT",
            XritMode::Gk2aHrit => "GK-2A HRIT",
            XritMode::Elektro => "",
        };
        // A write to a String cannot fail.
        let _ = if link.class == LinkClass::Ggak {
            let (g_good, g_bad) = r.gg.map_or((0, 0), |g| (g.good, g.bad));
            let marker = r.gg.and_then(|g| g.marker).unwrap_or(0);
            match state {
                2 | 3 => write!(
                    s,
                    "{craft} GGAK: {g_good} frames ({g_bad} bad), marker {marker}, Es/N0 {es:.1} dB"
                ),
                1 => write!(
                    s,
                    "{craft} GGAK: carrier locked ({:.1} kHz), looking for frames",
                    carrier / 1e3
                ),
                _ if self.arktika => write!(s, "{craft} GGAK: searching for the 5 ksym/s carrier"),
                _ => write!(
                    s,
                    "{craft} GGAK: searching for the 5 ksym/s PCM/PM carrier on 1693 MHz"
                ),
            }
        } else {
            if self.mode == XritMode::Elektro {
                // A write to a String cannot fail.
                let _ = write!(s, "{craft} {link_name}");
            } else {
                s.push_str(name);
            }
            match state {
                2 | 3 => write!(
                    s,
                    ": {good} frames ({bad} bad), {products} products, Es/N0 {es:.1} dB"
                ),
                1 => write!(
                    s,
                    ": carrier locked ({:.1} kHz), looking for frames",
                    carrier / 1e3
                ),
                _ => match self.mode {
                    XritMode::Gk2aHrit => write!(s, ": searching for the 3000 ksym/s QPSK carrier"),
                    XritMode::Goes => write!(s, ": searching for the 927 ksym/s BPSK carrier"),
                    _ => write!(s, ": searching for the {rate_ks:.0} ksym/s BPSK carrier"),
                },
            }
        };
    }

    fn publish(&mut self, state: u8, t: f64, r: &Reports) {
        let (link_name, link) = self.link();
        self.write_summary(state, link_name, &link, r);
        self.write_hint(state, r, &link);
        let p = &mut self.published;
        put_link(p, t, r.f, r.d, r.c, r.fr, r.pk);
        status(p.results_mut(), state, r.f, r.d, &self.summary);
        let gk2a = self.mode.gk2a();
        let ggak = r.gg.filter(|g| g.on);
        if let Some(tr) = ggak.and_then(|g| g.field.as_ref()) {
            let (m, ok) = magnitude(tr);
            if ok {
                self.ggak_field = Some(m);
            }
        }
        p.put(&XritFields {
            link: link_name,
            carrier_locked: r.d.map(|d| d.state.locked || state == 3),
            symbol_rate_measured_hz: r.d.map(|d| Hertz(d.state.symbol_rate)),
            g2_inverted: r.c.map(|c| c.lock.g2_inverted()),
            rs_basis: r.fr.and_then(|f| f.stats.basis).map(|b| match b {
                sdrkit_algo_fec::rs::Basis::Conventional => RsBasis::Conventional,
                _ => RsBasis::Dual,
            }),
            byte_errors_pct: r.fr.map(|f| f.byte_error_pct),
            transfer_frames: r.fl.map(|f| f.stats.frames as i64),
            fill_frames: r.pk.map(|k| k.stats.fill_frames as i64),
            frame_gaps: r.fl.map(|f| f.stats.gaps as i64),
            crc_errors: r.pk.map(|k| k.stats.crc_errors as i64),
            files: r.fl.map(|f| f.stats.files as i64),
            file_errors: r.fl.map(|f| f.stats.with_errors as i64),
            length_errors: r.fl.map(|f| f.stats.length_errors as i64),
            products: r.st.map(|s| s.products as i64),
            keys: r.st.filter(|_| gk2a).map(|s| s.keys as i64),
            segments_locked: r.st.filter(|_| gk2a).map(|s| s.stats.locked_now as i64),
            hint: &self.hint,
            first_decoding: self.first_decoding_s.map(Seconds),
            cadus_imported: r.fr.map(|f| f.imported as i64),
            ggak_frames: ggak.map(|g| g.frames as i64),
            ggak_good: ggak.map(|g| g.good as i64),
            ggak_bad: ggak.map(|g| g.bad as i64),
            ggak_marker: ggak.and_then(|g| g.marker.map(i64::from)),
            ggak_field: ggak.and(self.ggak_field),
        });
        if self.arktika {
            p.put(&ArktikaFields { note: ARKTIKA_NOTE });
        }
        if let Some(fl) = r.fl {
            for (vc, n) in &fl.stats.per_vc {
                self.key.clear();
                // A write to a String cannot fail.
                let _ = write!(self.key, "{vc}");
                VIRTUAL_CHANNELS.put(
                    p,
                    &self.key,
                    &VirtualChannelRow {
                        vcid: i64::from(*vc),
                        frames: *n as i64,
                    },
                );
            }
        }
        self.hist.publish(self.published.results_mut(), r.d, r.fr);
    }

    fn write_hint(&mut self, state: u8, r: &Reports, link: &LinkParams) {
        let h = &mut self.hint;
        h.clear();
        let rate = r.f.map_or(0.0, |f| f.input_rate);
        let need = self.mode.min_rate(link);
        if r.f.is_some_and(|f| f.rate_too_low) {
            // A write to a String cannot fail.
            let _ = write!(
                h,
                "The sample rate ({:.2} Msps) is too low: this link needs at least {:.2} Msps (2.1 samples a symbol). Raise it in the radio settings.",
                rate / 1e6,
                need / 1e6
            );
            return;
        }
        if !self.mode.gk2a() {
            return;
        }
        let es = r.d.map_or(-10.0, |d| d.state.es_n0_db);
        let st = r.st;
        let fmhz = self.mode.frequency() / 1e6;
        // A write to a String cannot fail.
        let _ = if state == 0 && r.f.is_some_and(|f| f.level_dbfs > -5.0) {
            write!(h, "The input is very strong (near full scale): lower the gain, because a clipped signal does not decode.")
        } else if state == 0 {
            write!(h, "No carrier found. GK-2A is at 128.2 E: above the horizon from East Asia, Australia and the western Pacific, not from Europe. Point a dish with an LNA at it and tune {fmhz} MHz. A small antenna can not hear it.")
        } else if state == 1 && es < 2.0 {
            let n = if self.mode == XritMode::Gk2aLrit {
                -1
            } else {
                2
            };
            write!(h, "Carrier found but weak (Es/N0 {es:.1} dB): decoding needs about {n} dB. Aim the dish again, add LNA gain or use a larger dish.")
        } else if state == 1 {
            write!(h, "Carrier locked. The stack looks for the frame marker in every polarity and I/Q order. If it never finds it, the symbol rate is wrong.")
        } else if state == 2 {
            write!(
                h,
                "Frames are found but Reed-Solomon fails: the signal is too weak for clean frames."
            )
        } else if st.is_some_and(|s| s.keys == 0 && s.stats.locked_now > 0) {
            write!(h, "Pictures are DES-encrypted by KMA. Import your KMA key file (Import keys) to see them. Text files show anyway.")
        } else if st.is_some_and(|s| s.stats.locked_now > 0) {
            write!(
                h,
                "Some pictures use a key that is not in the imported key file."
            )
        } else {
            Ok(())
        };
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// The graph of an xRIT stack. `link` is the Elektro-L link the chain starts on (the others have one link).
pub fn builder(mode: XritMode, link: ElektroLink) -> GraphBuilder {
    graph(mode, link, XritDisplay::new(mode, link))
}

/// The graph of an xRIT stack with the display `display`.
fn graph(mode: XritMode, link: ElektroLink, display: XritDisplay) -> GraphBuilder {
    let start = if mode == XritMode::Elektro {
        link.link()
    } else {
        mode.link()
    };
    let ggak = start.class == LinkClass::Ggak;
    GraphBuilder::new()
        .add(
            "frontend",
            FrontEndComponent::new(start.clone(), mode.min_rate(&start), false),
        )
        .add("demod", DemodComponent::new(start))
        .add("decoder", CodeComponent::new(CodeConfig::GEO))
        .add("frames", FrameComponent::new(FrameLink::GEO))
        .add("vcdu", VcduComponent::new(Layout::RS_I4, true))
        .add("files", FilesComponent::new(mode.dialect()))
        .add("products", ProductComponent::new(mode.store()))
        .add("ggak", GgakComponent::new(ggak))
        .add("display", display)
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "decoder.in")
        .connect("demod.soft", "ggak.in")
        .connect("decoder.out", "frames.in")
        .connect("frames.out", "vcdu.in")
        .connect("vcdu.packets", "files.in")
        .connect("vcdu.vcdu", "files.vcdu")
        .connect("files.out", "products.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("decoder.report", "display.code")
        .connect("frames.report", "display.frames")
        .connect("vcdu.report", "display.packets")
        .connect("files.report", "display.files")
        .connect("products.report", "display.store")
        .connect("ggak.report", "display.ggak")
        .feedback("display.control", "frontend.control")
        .feedback("display.control", "demod.control")
        .feedback("display.control", "decoder.control")
        .feedback("display.control", "ggak.control")
        .output("products.events")
        .output("products.text")
        .output("products.pictures")
        .output("ggak.events")
        .status_from("display")
        .clock(REPORT_S)
}

/// The receiver of an xRIT stack: the graph with the settings of the stack.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(mode: XritMode, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let arktika = mode == XritMode::Elektro && cfg.submode.as_deref() == Some(ARKTIKA_SUBMODE);
    let link = if arktika {
        ElektroLink::Ggak
    } else if mode == XritMode::Elektro {
        cfg.settings.get(&LINK_SETTING)
    } else {
        ElektroLink::Hrit665
    };
    let display = if arktika {
        XritDisplay::arktika()
    } else {
        XritDisplay::new(mode, link)
    };
    let mut g = graph(mode, link, display)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    g.set_max_items(1 << 20);
    let min = match mode {
        XritMode::Elektro => mode.min_rate(&ggak_link()),
        _ => mode.min_rate(&mode.link()),
    };
    let shape = if arktika {
        crate::typed::arktika_shape()
    } else {
        crate::typed::shape(mode.id())
    };
    let r = StackGraph::of(shape, g);
    let mut r = r
        .min_rate(min, "")
        .route("filter", &["products.filter"])?
        .route("follow_newest", &["products.follow_newest"])?
        .command(
            "open_cadu",
            sdrkit_graph::CommandTarget::of("frames", "open_cadu"),
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    if mode.gk2a() {
        r = r
            .command(
                "import_keys",
                sdrkit_graph::CommandTarget::of("products", "import_keys"),
            )
            .command(
                "add_keys",
                sdrkit_graph::CommandTarget::of("products", "add_keys"),
            );
    }
    if mode == XritMode::Elektro {
        r = r
            .route("craft", &["display.craft"])?
            .route("link", &["display.link"])?;
    }
    r.with_settings(&cfg.settings).build()
}

fn make(mode: XritMode, cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(mode, cfg))
}
fn goes_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make(XritMode::Goes, cfg)
}
fn gk2a_lrit_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make(XritMode::Gk2aLrit, cfg)
}
fn gk2a_hrit_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make(XritMode::Gk2aHrit, cfg)
}
fn elektro_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make(XritMode::Elektro, cfg)
}
fn goes_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    super::xrit_gen::generator(cfg, XritMode::Goes)
}
fn gk2a_lrit_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    super::xrit_gen::generator(cfg, XritMode::Gk2aLrit)
}
fn gk2a_hrit_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    super::xrit_gen::generator(cfg, XritMode::Gk2aHrit)
}
fn elektro_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    super::xrit_gen::generator(cfg, XritMode::Elektro)
}
fn goes_schema() -> Arc<Schema> {
    GoesSetting::schema()
}
fn gk2a_schema() -> Arc<Schema> {
    Gk2aSetting::schema()
}
fn elektro_schema() -> Arc<Schema> {
    ElektroSetting::schema()
}

/// GOES HRIT.
pub static GOES: StackDescriptor = StackDescriptor {
    id: "goes_hrit",
    name: "GOES HRIT",
    group: crate::GROUP,
    blurb: "GOES-16, 18 and 19 HRIT and EMWIN on 1694.1 MHz: full-disk and mesoscale pictures, relayed pictures and text.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        ..Tuning::anywhere(1694.1e6, 2.4e6, 2.0e6, 1.4e6)
    },
    submodes: &[],
    settings: goes_schema,
    make_receiver: goes_receiver,
    make_generator: Some(goes_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hrit],
    auto_gain: AutoGain::Radio,
    verify: Some(&GOES_HOOKS),
    benchmarks: None,
};

/// GK-2A LRIT.
pub static GK2A_LRIT: StackDescriptor = StackDescriptor {
    id: "gk2a_lrit",
    name: "GK-2A LRIT",
    group: crate::GROUP,
    blurb: "GEO-KOMPSAT-2A LRIT on 1692.14 MHz: full-disk channel pictures and text.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        ..Tuning::anywhere(1692.14e6, 2.4e6, 300e3, 0.3e6)
    },
    submodes: &[],
    settings: gk2a_schema,
    make_receiver: gk2a_lrit_receiver,
    make_generator: Some(gk2a_lrit_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Lrit],
    auto_gain: AutoGain::Radio,
    verify: Some(&GK2A_LRIT_HOOKS),
    benchmarks: None,
};

/// GK-2A HRIT.
pub static GK2A_HRIT: StackDescriptor = StackDescriptor {
    id: "gk2a_hrit",
    name: "GK-2A HRIT",
    group: crate::GROUP,
    blurb: "GEO-KOMPSAT-2A HRIT on 1695.4 MHz (3 Msym/s QPSK): full-disk channel pictures.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        ..Tuning::anywhere(1695.4e6, 10e6, 6.4e6, 5.2e6)
    },
    submodes: &[],
    settings: gk2a_schema,
    make_receiver: gk2a_hrit_receiver,
    make_generator: Some(gk2a_hrit_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hrit],
    auto_gain: AutoGain::Radio,
    verify: Some(&GK2A_HRIT_HOOKS),
    benchmarks: None,
};

/// Elektro-L.
pub static ELEKTRO: StackDescriptor = StackDescriptor {
    id: "elektro_l",
    name: "Elektro-L",
    group: crate::GROUP,
    blurb: "Elektro-L N3, N4 and N5: HRIT and LRIT pictures on 1691 MHz and GGAK telemetry on 1693 MHz.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        ..Tuning::anywhere(1691.0e6, 2.4e6, 200e3, 1.0e6)
    },
    // The default frequency of the submode is the one of its test signal. The host gives the frequency of a session.
    submodes: &[SubMode {
        id: ARKTIKA_SUBMODE,
        name: "Arktika-M GGAK",
        default_hz: 1703e6,
    }],
    settings: elektro_schema,
    make_receiver: elektro_receiver,
    make_generator: Some(elektro_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hrit, SignalType::Lrit],
    auto_gain: AutoGain::Radio,
    verify: Some(&ELEKTRO_HOOKS),
    benchmarks: None,
};

/// The help before start (GOES).
pub const GOES_HELP: &str = "Start the stack. GOES HRIT needs a dish (about 1 m, or a 1.7 GHz grid antenna) pointed at the satellite, with an LNA and filter at the feed.";

// ---------------------------------------------------------------------------------------------------------------------------------
// Declarations and verification.

fn check_products(r: &Results) -> Vec<CheckLine> {
    vec![
        at_least(r, &sdrkit_core::publish::keys::FRAMES_OK, 20),
        count_at_least(r, &XritFields::PRODUCTS, 1),
    ]
}

fn check_ggak(r: &Results) -> Vec<CheckLine> {
    vec![count_at_least(r, &XritFields::GGAK_GOOD, 2)]
}

fn case(
    o: &VerifyOptions,
    name: &str,
    seconds: f64,
    check: fn(&Results) -> Vec<CheckLine>,
) -> VerifyCase {
    let mut c = VerifyCase::new(name, o, 5e3);
    c.seconds = c.seconds.max(seconds);
    c.check = Some(check);
    c
}

fn goes_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal", 1.5, check_products).with(&GEN_FILL, 12)]
}
fn gk2a_lrit_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal", 12.0, check_products)]
}
fn gk2a_hrit_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal", 1.0, check_products)]
}
fn elektro_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![
        case(o, "test signal, HRIT", 2.0, check_products).with(&GEN_FILL, 12),
        case(o, "test signal, GGAK", 4.0, check_ggak)
            .with(&LINK_SETTING, ElektroLink::Ggak)
            .with(&GEN_LINK, ElektroLink::Ggak),
    ]
}

static GOES_HOOKS: VerifyHooks = VerifyHooks {
    cases: goes_cases,
    vector_settings: None,
    check_vector: None,
};
static GK2A_LRIT_HOOKS: VerifyHooks = VerifyHooks {
    cases: gk2a_lrit_cases,
    vector_settings: None,
    check_vector: None,
};
static GK2A_HRIT_HOOKS: VerifyHooks = VerifyHooks {
    cases: gk2a_hrit_cases,
    vector_settings: None,
    check_vector: None,
};
static ELEKTRO_HOOKS: VerifyHooks = VerifyHooks {
    cases: elektro_cases,
    vector_settings: None,
    check_vector: None,
};

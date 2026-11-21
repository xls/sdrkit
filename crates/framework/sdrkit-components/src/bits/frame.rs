//! Frames: `bits.pack` and `bits.hdlc` (bits to byte packets), and `bytes.crc` (the check of a CRC trailer).

use super::{walk, FrameStartChoice, PacketPool, Step};
use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::{CatalogueSetting, ChoiceDecl};
use sdrkit_core::{Choice, EventDef, Record, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Every, Format, Io, Item, Kind, Port, RateFrom,
    TagCarry, TagKind,
};
use sdrkit_util_bits::crc::catalog::{self, Entry};
use sdrkit_util_bits::crc::Trailer;
use sdrkit_util_bits::hdlc::HdlcDeframer;
use sdrkit_util_bits::{BitOrder, Pad};

sdrkit_core::choice_enum! {
    /// Where the first bit of each byte goes.
    pub enum BitOrderChoice {
        /// The first bit is the most significant.
        Msb = ("msb", "First bit most significant"),
        /// The first bit is the least significant.
        Lsb = ("lsb", "First bit least significant"),
    }
}

impl BitOrderChoice {
    fn order(self) -> BitOrder {
        match self {
            BitOrderChoice::Msb => BitOrder::Msb,
            BitOrderChoice::Lsb => BitOrder::Lsb,
        }
    }
    /// The bit order of a stream format.
    pub(crate) fn stream(self) -> sdrkit_graph::BitOrder {
        match self {
            BitOrderChoice::Msb => sdrkit_graph::BitOrder::MsbFirst,
            BitOrderChoice::Lsb => sdrkit_graph::BitOrder::LsbFirst,
        }
    }
    /// The choice for the bit order of a stream format.
    pub(crate) fn of(order: sdrkit_graph::BitOrder) -> BitOrderChoice {
        match order {
            sdrkit_graph::BitOrder::MsbFirst => BitOrderChoice::Msb,
            sdrkit_graph::BitOrder::LsbFirst => BitOrderChoice::Lsb,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How the bytes of a CRC follow the data.
    pub enum CrcTrailer {
        /// Little-endian for a reflected CRC, big-endian for the others.
        Auto = ("auto", "By the CRC"),
        /// Most significant byte first.
        Big = ("big", "Most significant first"),
        /// Least significant byte first.
        Little = ("little", "Least significant first"),
    }
}

sdrkit_core::choice_enum! {
    /// What `bytes.crc` does with a packet whose CRC is wrong.
    pub enum CrcAction {
        /// Drop it.
        Drop = ("drop", "Drop"),
        /// Pass it on, marked bad in the status.
        Mark = ("mark", "Pass on, marked"),
    }
}

const ORDER: Setting<field::Choice<BitOrderChoice>> =
    Setting::choice("order", "Bit order", BitOrderChoice::Msb)
        .help("Where the first bit of each byte goes.");
const FRAME_BITS: Setting<field::I64> = Setting::new("frame_bits", "Frame length", "bits", 8)
    .range(0.0, 16_777_216.0, 1.0)
    .help("Bits per frame. 0 packs the bits from one frame start to the next.");
const START: Setting<field::Choice<FrameStartChoice>> =
    Setting::choice("start", "Frame start", FrameStartChoice::FrameStart)
        .help("Where frames begin.");
const MAX_BITS: Setting<field::I64> = Setting::new("max_bits", "Longest frame", "bits", 65536)
    .range(8.0, 16_777_216.0, 1.0)
    .help("The longest frame kept when the frame length is 0.");

sdrkit_core::settings! {
    /// The settings of [`Packer`].
    pub enum PackerSetting {
        /// Bit order.
        Order = ORDER,
        /// Frame length.
        FrameBits = FRAME_BITS,
        /// Frame start.
        Start = START,
        /// Longest frame.
        MaxBits = MAX_BITS,
    }
}

/// `bits.unpack`: byte packets of packed bits to hard bits, in the bit order that the input format states. The bits have no
/// rate.
#[derive(Debug, Default)]
pub struct Unpacker {
    order: Option<BitOrder>,
}

/// The input of `bits.unpack` does not state how its bits are packed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoBitOrder;

impl std::fmt::Display for NoBitOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the bytes do not state the order of their bits")
    }
}

impl std::error::Error for NoBitOrder {}

impl Component for Unpacker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bits.unpack",
            vec![Port::bytes("in")],
            vec![Port::hard_bits("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let order = inputs[0].bit_order.ok_or(crate::stage(NoBitOrder))?;
        self.order = Some(BitOrderChoice::of(order).order());
        Ok(vec![Format::hard_bits(0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(order) = self.order else { return };
        let packets = io.bytes(0);
        let out = io.hard_bits_out(0);
        for p in packets {
            sdrkit_util_bits::pack::unpack(p.bytes(), order, out);
        }
    }
}

/// The settings of `bits.pack` as an adapter: one byte per packet, back to back, in the order that the input `want` needs.
pub(crate) fn pack_step(_: &Format, want: &sdrkit_graph::FormatConstraint) -> Option<Values> {
    let order = BitOrderChoice::of(want.bit_order?);
    Some(values_of([
        (PackerSetting::Order, Value::Choice(order.index())),
        (PackerSetting::FrameBits, Value::I64(8)),
        (
            PackerSetting::Start,
            Value::Choice(FrameStartChoice::Continuous.index()),
        ),
    ]))
}

/// `bits.pack`: hard bits into byte packets ([`sdrkit_util_bits::pack::pack`]), one packet per frame.
///
/// If `frame_bits` is 0, a frame holds the bits from one `FrameStart` tag to the next, at most `max_bits` (a longer run is
/// dropped). With `start` at `frame_start`, a frame begins at each `FrameStart` tag and bits outside frames are dropped. With
/// `continuous`, frames follow each other from the first bit, and a `FrameStart` tag starts a new frame there (a partial frame
/// is dropped).
///
/// A final partial byte is padded with zero bits. A packet's time is that of its first bit. Tags other than `FrameStart` move to
/// the next packet.
#[derive(Debug)]
pub struct Packer {
    order: BitOrderChoice,
    frame_bits: usize,
    start: FrameStartChoice,
    max_bits: usize,
    bits: Vec<u8>,
    first_at: u64,
    in_frame: bool,
    rate_format: Format,
    pool: PacketPool,
    carry: TagCarry,
}

impl Default for Packer {
    fn default() -> Self {
        Packer {
            order: ORDER.default,
            frame_bits: FRAME_BITS.default as _,
            start: START.default,
            max_bits: MAX_BITS.default as _,
            bits: Vec::new(),
            first_at: 0,
            in_frame: false,
            rate_format: Format::hard_bits(0.0),
            pool: PacketPool::default(),
            carry: TagCarry::same_rate(),
        }
    }
}

impl Packer {
    fn emit(&mut self, io: &mut Io) {
        let n = self.bits.len();
        if n == 0 {
            return;
        }
        let (order, bits) = (self.order.order(), &self.bits);
        let time = self.rate_format.time_of(self.first_at);
        let p = self.pool.make(n.div_ceil(8), time, |d| {
            // The packet has exactly the bytes these bits fill, so this cannot fail.
            let _ = sdrkit_util_bits::pack::pack_into(bits, order, Pad::Zero, d);
        });
        io.bytes_out(0).push(p);
        self.bits.clear();
    }
    fn begin(&mut self, at: u64) {
        self.bits.clear();
        self.first_at = at;
        self.in_frame = true;
    }
}

impl Component for Packer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bits.pack",
            vec![Port::hard_bits("in")],
            vec![Port::bytes("out")
                .rate_from(RateFrom::Fixed(0.0))
                .bit_order(self.order.stream())],
        )
        .manual_tags()
        .with_settings(PackerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate_format = inputs[0].clone();
        let cap = if self.frame_bits > 0 {
            self.frame_bits
        } else {
            self.max_bits
        };
        self.bits = Vec::with_capacity(cap);
        self.in_frame = false;
        self.carry.reset(inputs[0].rate, 0.0);
        Ok(vec![Format {
            bit_order: Some(self.order.stream()),
            ..Format::new(Kind::Bytes, 0.0)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let x = io.hard_bits(0);
        let by_tags = self.start == FrameStartChoice::FrameStart || self.frame_bits == 0;
        walk(first, x.len(), tags, |step| match step {
            Step::Items(a, b) => {
                for (k, &v) in x[a..b].iter().enumerate() {
                    if !self.in_frame {
                        if by_tags {
                            continue;
                        }
                        self.begin(first + (a + k) as u64);
                    }
                    self.bits.push(v & 1);
                    if self.frame_bits > 0 && self.bits.len() == self.frame_bits {
                        self.emit(io);
                        self.in_frame = false;
                    } else if self.frame_bits == 0 && self.bits.len() > self.max_bits {
                        self.bits.clear();
                        self.in_frame = false;
                    }
                }
            }
            Step::Tag(at, t) => {
                if matches!(t.kind, TagKind::FrameStart { .. }) {
                    if self.frame_bits == 0 && self.in_frame {
                        self.emit(io);
                    }
                    self.begin(first + at as u64);
                } else {
                    let end = io.output(0).end();
                    self.carry.queue(end, t.kind.clone());
                }
            }
        });
        self.carry.flush(io.output(0));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PackerSetting::Order => self.order = ORDER.read(id, v)?,
            PackerSetting::FrameBits => {
                self.frame_bits = FRAME_BITS.read(id, v)? as usize;
                self.reserve();
            }
            PackerSetting::Start => self.start = START.read(id, v)?,
            PackerSetting::MaxBits => {
                self.max_bits = MAX_BITS.read(id, v)? as usize;
                self.reserve();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (PackerSetting::Order, Value::Choice(self.order.index())),
            (PackerSetting::FrameBits, Value::I64(self.frame_bits as i64)),
            (PackerSetting::Start, Value::Choice(self.start.index())),
            (PackerSetting::MaxBits, Value::I64(self.max_bits as i64)),
        ])
    }
}

impl Packer {
    fn reserve(&mut self) {
        let cap = if self.frame_bits > 0 {
            self.frame_bits
        } else {
            self.max_bits + 1
        };
        self.bits.reserve(cap.saturating_sub(self.bits.len()));
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// bits.hdlc
// ---------------------------------------------------------------------------------------------------------------------------

/// The setting key of a catalogue CRC ([`catalog::key`]): "CRC-16/IBM-SDLC" is `crc16_ibm_sdlc`.
pub use sdrkit_util_bits::crc::catalog::key as crc_key;

/// The frame check sequences that `bits.hdlc` offers: the 16-bit catalogue CRCs.
fn fcs_entries() -> impl Iterator<Item = &'static Entry> {
    catalog::ENTRIES.iter().filter(|e| e.spec().width == 16)
}

/// The CRCs that `bytes.crc` offers: the catalogue CRCs of whole bytes.
fn crc_entries() -> impl Iterator<Item = &'static Entry> {
    catalog::ENTRIES
        .iter()
        .filter(|e| e.spec().width.is_multiple_of(8))
}

/// The entry of choice `i` of `list`, for setting `id`.
fn entry_at(
    mut list: impl Iterator<Item = &'static Entry>,
    id: SettingId,
    i: usize,
) -> Result<(&'static Entry, u16), SettingError> {
    let index = u16::try_from(i).map_err(|_| SettingError::NotAChoice {
        id,
        index: u16::MAX,
    })?;
    let e = list.nth(i).ok_or(SettingError::NotAChoice { id, index })?;
    Ok((e, index))
}

/// The CRC with key `key` in `list`, and its index. The key must be in the list.
fn entry_of(list: impl Iterator<Item = &'static Entry>, key: &str) -> (&'static Entry, u16) {
    list.enumerate()
        .find(|(_, e)| e.key == key)
        .map(|(i, e)| (e, i as u16))
        .expect("the default CRC is in the catalogue")
}

/// The CRC that `bits.hdlc` and `bytes.crc` take by default: CRC-16/IBM-SDLC.
const DEFAULT_CRC: &str = "crc16_ibm_sdlc";

const FCS: CatalogueSetting = CatalogueSetting {
    key: "fcs",
    label: "Frame check",
    help: "The 16-bit CRC of the frame check sequence.",
    choices: || {
        fcs_entries()
            .map(|e| ChoiceDecl::new(e.key, e.key))
            .collect()
    },
    default: DEFAULT_CRC,
    advanced: false,
};
const MIN_LEN: Setting<field::I64> = Setting::new("min_len", "Shortest frame", "bytes", 1)
    .range(0.0, 65_535.0, 1.0)
    .help("Frames shorter than this (without the check) are dropped.");
const MAX_LEN: Setting<field::I64> = Setting::new("max_len", "Longest frame", "bytes", 400)
    .range(1.0, 65_535.0, 1.0)
    .help("Frames longer than this (without the check) are dropped.");

sdrkit_core::settings! {
    /// The settings of [`HdlcFramer`].
    pub enum HdlcFramerSetting {
        /// Frame check.
        Fcs = FCS,
        /// Shortest frame.
        MinLen = MIN_LEN,
        /// Longest frame.
        MaxLen = MAX_LEN,
    }
}

/// `bits.hdlc`: HDLC frames from hard bits ([`HdlcDeframer`], which checks the FCS): flags (01111110), bit stuffing, abort on
/// seven ones, octets least significant bit first, and the frame check sequence. Each frame with a good FCS and an allowed length
/// becomes a packet of its bytes without the FCS, timed at the bit that closed it. The bits must already be NRZI-decoded
/// (`bits.nrzi`).
///
/// The default FCS is `crc16_ibm_sdlc`, the 16-bit FCS of ISO/IEC 13239 that AX.25 and AIS use ([`crc_key`] returns the key of a
/// catalogue CRC). `min_len` and `max_len` count bytes without the FCS.
///
/// Results: `frames_ok` (good FCS) and `frames_bad` (bad FCS). A `Retuned` or `Discontinuity` tag drops a frame in progress.
#[derive(Debug)]
pub struct HdlcFramer {
    fcs: &'static Entry,
    fcs_index: u16,
    min_len: usize,
    max_len: usize,
    d: HdlcDeframer,
    rate_format: Format,
    pool: PacketPool,
    carry: TagCarry,
    every: Every,
    published: Published,
}

impl Default for HdlcFramer {
    fn default() -> Self {
        let (fcs, fcs_index) = entry_of(fcs_entries(), DEFAULT_CRC);
        HdlcFramer {
            fcs,
            fcs_index,
            min_len: MIN_LEN.default as _,
            max_len: MAX_LEN.default as _,
            d: HdlcDeframer::x25(1, 400),
            rate_format: Format::hard_bits(0.0),
            pool: PacketPool::default(),
            carry: TagCarry::same_rate(),
            every: Every::default(),
            published: Published::new(&[&keys::FRAMES_OK, &keys::FRAMES_BAD]),
        }
    }
}

impl HdlcFramer {
    fn publish(&mut self) {
        self.published
            .set(&keys::FRAMES_OK, self.d.stats.good as i64);
        self.published
            .set(&keys::FRAMES_BAD, self.d.stats.bad_fcs as i64);
    }
}

impl Component for HdlcFramer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bits.hdlc",
            vec![Port::hard_bits("in")],
            vec![Port::bytes("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .manual_tags()
        .with_settings(HdlcFramerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.d = HdlcDeframer::try_new(self.fcs.engine, self.min_len, self.max_len)
            .map_err(crate::stage)?;
        self.rate_format = inputs[0].clone();
        self.carry.reset(inputs[0].rate, 0.0);
        self.every = Every::new(inputs[0].rate, 0.25);
        self.publish();
        Ok(vec![Format::new(Kind::Bytes, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let x = io.hard_bits(0);
        walk(first, x.len(), tags, |step| match step {
            Step::Items(a, b) => {
                for (k, &v) in x[a..b].iter().enumerate() {
                    if let Some(end) = self.d.push(v) {
                        let time = self.rate_format.time_of(first + (a + k) as u64);
                        let frame = &self.d.frame()[..end.len];
                        let p = self
                            .pool
                            .make(frame.len(), time, |d| d.copy_from_slice(frame));
                        io.bytes_out(0).push(p);
                    }
                }
            }
            Step::Tag(_, t) => {
                match t.kind {
                    TagKind::Retuned { .. } | TagKind::Discontinuity { .. } => self.d.reset(),
                    TagKind::FrameStart { .. } => return,
                    _ => {}
                }
                let end = io.output(0).end();
                self.carry.queue(end, t.kind.clone());
            }
        });
        self.carry.flush(io.output(0));
        if self.every.tick(x.len()) {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match HdlcFramerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HdlcFramerSetting::Fcs => {
                let (e, i) = entry_at(fcs_entries(), id, FCS.read(id, v)?)?;
                self.d
                    .set_fcs(e.engine)
                    .map_err(|_| SettingError::NotPermitted { id })?;
                (self.fcs, self.fcs_index) = (e, i);
            }
            HdlcFramerSetting::MinLen => {
                let n = MIN_LEN.read(id, v)? as usize;
                self.d
                    .set_lengths(n, self.max_len)
                    .map_err(|_| SettingError::Conflict {
                        id,
                        with: HdlcFramerSetting::MaxLen.id(),
                    })?;
                self.min_len = n;
            }
            HdlcFramerSetting::MaxLen => {
                let n = MAX_LEN.read(id, v)? as usize;
                self.d
                    .set_lengths(self.min_len, n)
                    .map_err(|_| SettingError::Conflict {
                        id,
                        with: HdlcFramerSetting::MinLen.id(),
                    })?;
                self.max_len = n;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (HdlcFramerSetting::Fcs, Value::Choice(self.fcs_index)),
            (HdlcFramerSetting::MinLen, Value::I64(self.min_len as i64)),
            (HdlcFramerSetting::MaxLen, Value::I64(self.max_len as i64)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// bytes.crc
// ---------------------------------------------------------------------------------------------------------------------------

/// What `bytes.crc` found for one packet it passed on (its `status` output, one item per packet of `out`, in the same order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrcStatus {
    /// The CRC trailer matched.
    pub good: bool,
}

impl Item for CrcStatus {
    const KIND: &'static str = "bytes.crc_status";
    fn event_def() -> Option<&'static EventDef> {
        Some(&CRC_STATUS)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&GOOD, self.good);
    }
}

const GOOD: Field<field::Bool> = Field::new("good", "CRC good", "");

static CRC_STATUS: EventDef = EventDef::new("bytes.crc_status", "CRC status", &[&GOOD]);

const CRC: CatalogueSetting = CatalogueSetting {
    key: "crc",
    label: "CRC",
    help: "The CRC at the end of each packet.",
    choices: || {
        crc_entries()
            .map(|e| ChoiceDecl::new(e.key, e.key))
            .collect()
    },
    default: DEFAULT_CRC,
    advanced: false,
};
const TRAILER: Setting<field::Choice<CrcTrailer>> =
    Setting::choice("trailer", "Byte order", CrcTrailer::Auto)
        .help("How the CRC's bytes follow the data.");
const ACTION: Setting<field::Choice<CrcAction>> =
    Setting::choice("action", "Bad packets", CrcAction::Drop)
        .help("What to do with a packet whose CRC is wrong.");
const STRIP: Setting<field::Bool> = Setting::new("strip", "Remove the CRC", "", true)
    .help("Leave the CRC bytes out of the packets passed on.");

sdrkit_core::settings! {
    /// The settings of [`CrcCheck`].
    pub enum CrcCheckSetting {
        /// CRC.
        Crc = CRC,
        /// Byte order.
        Trailer = TRAILER,
        /// Bad packets.
        Action = ACTION,
        /// Remove the CRC.
        Strip = STRIP,
    }
}

/// `bytes.crc`: checks the CRC trailer of each byte packet ([`Engine::verify`](sdrkit_util_bits::crc::catalog::Engine::verify)) and passes the packet on (the bytes are shared,
/// not copied).
///
/// `crc` is a catalogue CRC of whole bytes, by key ([`crc_key`]). With `action` at `mark`, every packet passes on and its
/// status says whether it is good.
///
/// Outputs: `out` (the packets) and `status` ([`CrcStatus`], one per packet of `out`). Results: `frames_ok` and `frames_bad`. A
/// packet too short to hold the CRC is bad.
#[derive(Debug)]
pub struct CrcCheck {
    crc: &'static Entry,
    crc_index: u16,
    trailer: CrcTrailer,
    action: CrcAction,
    strip: bool,
    good: u64,
    bad: u64,
    published: Published,
}

impl Default for CrcCheck {
    fn default() -> Self {
        let (crc, crc_index) = entry_of(crc_entries(), DEFAULT_CRC);
        CrcCheck {
            crc,
            crc_index,
            trailer: TRAILER.default,
            action: ACTION.default,
            strip: STRIP.default,
            good: 0,
            bad: 0,
            published: Published::new(&[&keys::FRAMES_OK, &keys::FRAMES_BAD]),
        }
    }
}

impl CrcCheck {
    fn at(&self) -> Trailer {
        match self.trailer {
            CrcTrailer::Big => Trailer::Big,
            CrcTrailer::Little => Trailer::Little,
            CrcTrailer::Auto => self.crc.engine.natural_trailer(),
        }
    }
    fn publish(&mut self) {
        self.published.set(&keys::FRAMES_OK, self.good as i64);
        self.published.set(&keys::FRAMES_BAD, self.bad as i64);
    }
}

impl Component for CrcCheck {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bytes.crc",
            vec![Port::bytes("in")],
            vec![
                Port::bytes("out").same_format_as(0),
                Port::typed::<CrcStatus>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(CrcCheckSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.good = 0;
        self.bad = 0;
        self.publish();
        Ok(vec![
            inputs[0].clone(),
            Format::new(Kind::typed::<CrcStatus>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.bytes(0);
        if x.is_empty() {
            return;
        }
        let at = self.at();
        let engine = self.crc.engine;
        let n = engine.spec().width as usize / 8;
        for p in x {
            let good = engine.verify(p.bytes(), at);
            if good {
                self.good += 1;
            } else {
                self.bad += 1;
                if self.action == CrcAction::Drop {
                    continue;
                }
            }
            let mut q = p.clone();
            if self.strip {
                q.len = q.len.saturating_sub(n);
            }
            let time = q.time;
            io.bytes_out(0).push(q);
            io.items_out::<CrcStatus>(1).push(sdrkit_graph::Timed {
                time,
                value: CrcStatus { good },
            });
        }
        self.publish();
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CrcCheckSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CrcCheckSetting::Crc => {
                (self.crc, self.crc_index) = entry_at(crc_entries(), id, CRC.read(id, v)?)?;
            }
            CrcCheckSetting::Trailer => self.trailer = TRAILER.read(id, v)?,
            CrcCheckSetting::Action => self.action = ACTION.read(id, v)?,
            CrcCheckSetting::Strip => self.strip = STRIP.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (CrcCheckSetting::Crc, Value::Choice(self.crc_index)),
            (
                CrcCheckSetting::Trailer,
                Value::Choice(self.trailer.index()),
            ),
            (CrcCheckSetting::Action, Value::Choice(self.action.index())),
            (CrcCheckSetting::Strip, Value::Bool(self.strip)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(schema: &sdrkit_core::Schema, key: &str, choice: &str) -> (SettingId, Value) {
        let id = schema.id(key).unwrap();
        let i = schema
            .setting(id)
            .unwrap()
            .field
            .choice_index(choice)
            .unwrap();
        (id, Value::Choice(i))
    }

    #[test]
    fn the_choices_cover_the_catalogue() {
        assert!(fcs_entries().any(|e| e.key == DEFAULT_CRC));
        assert_eq!(
            fcs_entries().count(),
            catalog::ALL.iter().filter(|s| s.width == 16).count()
        );
        assert_eq!(
            crc_entries().count(),
            catalog::ALL.iter().filter(|s| s.width % 8 == 0).count()
        );
    }

    #[test]
    fn bad_crc_settings_are_refused() {
        let mut h = HdlcFramer::default();
        let s = HdlcFramerSetting::schema();
        assert!(s.id("fcs").is_ok());
        assert!(s
            .setting(s.id("fcs").unwrap())
            .unwrap()
            .field
            .choice_index("crc32_iso_hdlc")
            .is_none());
        let (id, v) = choice(&s, "fcs", "crc16_kermit");
        h.apply(id, &v).unwrap();
        assert_eq!(h.d.fcs().spec().name, "CRC-16/KERMIT");
        assert!(h.apply(id, &Value::Choice(u16::MAX)).is_err());
        h.apply(s.id("max_len").unwrap(), &Value::I64(10)).unwrap();
        assert!(h.apply(s.id("min_len").unwrap(), &Value::I64(11)).is_err());
        let mut c = CrcCheck::default();
        let s = CrcCheckSetting::schema();
        assert!(s
            .setting(s.id("crc").unwrap())
            .unwrap()
            .field
            .choice_index("crc12_dect")
            .is_none());
        let (id, v) = choice(&s, "crc", "crc32_mpeg_2");
        c.apply(id, &v).unwrap();
        assert_eq!(c.crc.spec().check, 0x0376_E6E7);
    }
}

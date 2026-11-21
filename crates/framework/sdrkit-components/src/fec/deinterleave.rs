//! `fec.deinterleave`: the receive side of the FEC-chain interleavers of `sdrkit-algo-fec` (Forney convolutional, block permutations,
//! delay banks) on a soft-bit stream or on byte packets.

use super::common::PacketPool;
use sdrkit_algo_fec::interleave::{bit_reverse, dvbs2_bits, DelayLanes, Permutation, ReadOrder};
use sdrkit_core::publish::field;
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Kind, Port, Tag, TagKind};

sdrkit_core::choice_enum! {
    /// The kind of [`Deinterleave`].
    pub enum DeinterleaverKind {
        /// Forney convolutional.
        Forney = ("forney", "Forney convolutional"),
        /// Block permutation.
        Permutation = ("permutation", "Block permutation"),
        /// Delay bank.
        Delay = ("delay", "Delay bank"),
    }
}

sdrkit_core::choice_enum! {
    /// A named interleaver of [`Deinterleave`]. `custom` is the Forney interleaver of `branches` and `unit`, or the delay
    /// bank of `delays`. `rows_cols` is the block of `rows` × `cols`, written by rows and read by columns.
    pub enum InterleaverName {
        /// `custom`.
        Custom = ("custom", "custom"),
        /// `dvb`.
        Dvb = ("dvb", "dvb"),
        /// `atsc`.
        Atsc = ("atsc", "atsc"),
        /// `meteor`.
        Meteor = ("meteor", "meteor"),
        /// `rows_cols`.
        RowsCols = ("rows_cols", "rows_cols"),
        /// `dvbs2`.
        Dvbs2 = ("dvbs2", "dvbs2"),
        /// `dvbs2_reverse`.
        Dvbs2Reverse = ("dvbs2_reverse", "dvbs2_reverse"),
        /// `bit_reverse`.
        BitReverse = ("bit_reverse", "bit_reverse"),
        /// `nicam`.
        Nicam = ("nicam", "nicam"),
        /// `gsm_xcch`.
        GsmXcch = ("gsm_xcch", "gsm_xcch"),
        /// `dmr_trellis`.
        DmrTrellis = ("dmr_trellis", "dmr_trellis"),
        /// `dvbt_symbol_2k`.
        DvbtSymbol2k = ("dvbt_symbol_2k", "dvbt_symbol_2k"),
        /// `dvbt_symbol_8k`.
        DvbtSymbol8k = ("dvbt_symbol_8k", "dvbt_symbol_8k"),
        /// `dvbt_bit_0`.
        DvbtBit0 = ("dvbt_bit_0", "dvbt_bit_0"),
        /// `dvbt_bit_1`.
        DvbtBit1 = ("dvbt_bit_1", "dvbt_bit_1"),
        /// `dvbt_bit_2`.
        DvbtBit2 = ("dvbt_bit_2", "dvbt_bit_2"),
        /// `dvbt_bit_3`.
        DvbtBit3 = ("dvbt_bit_3", "dvbt_bit_3"),
        /// `dvbt_bit_4`.
        DvbtBit4 = ("dvbt_bit_4", "dvbt_bit_4"),
        /// `dvbt_bit_5`.
        DvbtBit5 = ("dvbt_bit_5", "dvbt_bit_5"),
        /// `dab_time`.
        DabTime = ("dab_time", "dab_time"),
    }
}

impl InterleaverName {
    /// Whether this interleaver is of kind `kind`.
    fn is_of(self, kind: DeinterleaverKind) -> bool {
        use DeinterleaverKind as K;
        use InterleaverName as N;
        match self {
            N::Custom => kind != K::Permutation,
            N::Dvb | N::Atsc | N::Meteor => kind == K::Forney,
            N::DabTime => kind == K::Delay,
            N::RowsCols
            | N::Dvbs2
            | N::Dvbs2Reverse
            | N::BitReverse
            | N::Nicam
            | N::GsmXcch
            | N::DmrTrellis
            | N::DvbtSymbol2k
            | N::DvbtSymbol8k
            | N::DvbtBit0
            | N::DvbtBit1
            | N::DvbtBit2
            | N::DvbtBit3
            | N::DvbtBit4
            | N::DvbtBit5 => kind == K::Permutation,
        }
    }
}

/// The deinterleaver for one item type.
#[derive(Debug)]
enum Engine<T: Copy> {
    Forney(DelayLanes<T>),
    Delay(DelayLanes<T>),
    Perm {
        p: Permutation,
        block: Vec<T>,
        out: Vec<T>,
    },
}

impl<T: Copy + Default> Engine<T> {
    /// Runs `x` through, appending to `out`. A frame start before item `r` of `x` (relative positions in `starts`) brings a
    /// convolutional deinterleaver back to its first branch, and makes a block one start again (a partial block before it comes
    /// out as fill values).
    fn run(&mut self, x: &[T], starts: &[usize], out: &mut Vec<T>) {
        let mut a = 0;
        for &b in starts.iter().chain(std::iter::once(&x.len())) {
            let b = b.min(x.len());
            if b > a {
                self.feed(&x[a..b], out);
            }
            if b < x.len() {
                self.sync(out);
            }
            a = b;
        }
    }

    fn feed(&mut self, x: &[T], out: &mut Vec<T>) {
        match self {
            Engine::Forney(c) => {
                let s = out.len();
                out.extend_from_slice(x);
                c.process(&mut out[s..]);
            }
            Engine::Delay(d) => {
                let s = out.len();
                out.extend_from_slice(x);
                d.process(&mut out[s..]);
            }
            Engine::Perm { p, block, out: o } => {
                let n = p.len();
                for &v in x {
                    block.push(v);
                    if block.len() == n {
                        if p.invert(block, o).is_ok() {
                            out.extend_from_slice(o);
                        }
                        block.clear();
                    }
                }
            }
        }
    }

    fn sync(&mut self, out: &mut Vec<T>) {
        match self {
            Engine::Forney(c) => c.sync(),
            Engine::Delay(d) => d.sync(),
            Engine::Perm { block, .. } => {
                out.resize(out.len() + block.len(), T::default());
                block.clear();
            }
        }
    }

    /// Deinterleaves one whole packet into `out` (of its length; see [`Engine::fits`]).
    fn packet(&mut self, x: &[T], out: &mut [T]) {
        match self {
            Engine::Forney(c) => {
                out.copy_from_slice(x);
                c.process(out);
            }
            Engine::Delay(d) => {
                out.copy_from_slice(x);
                d.process(out);
            }
            Engine::Perm { p, .. } => {
                // `fits` let only packets of the permutation's length through, so this cannot fail.
                let _ = p.invert(x, out);
            }
        }
    }

    /// Whether a packet of `len` items fits (a permutation needs exactly its length).
    fn fits(&self, len: usize) -> bool {
        match self {
            Engine::Perm { p, .. } => p.len() == len,
            Engine::Forney(_) | Engine::Delay(_) => true,
        }
    }
}

const KIND: Setting<field::Choice<DeinterleaverKind>> =
    Setting::choice("kind", "Kind", DeinterleaverKind::Forney);
const NAME: Setting<field::Choice<InterleaverName>> =
    Setting::choice("name", "Interleaver", InterleaverName::Dvb);
const BRANCHES: Setting<field::I64> =
    Setting::new("branches", "Branches", "", 12).range(1.0, 4096.0, 1.0);
const UNIT: Setting<field::I64> =
    Setting::new("unit", "Unit delay", "", 17).range(1.0, 65_536.0, 1.0);
const ROWS: Setting<field::I64> = Setting::new("rows", "Rows", "", 1).range(1.0, 65_536.0, 1.0);
const COLS: Setting<field::I64> = Setting::new("cols", "Columns", "", 1).range(1.0, 65_536.0, 1.0);
const SIZE: Setting<field::I64> = Setting::new("size", "Size", "", 0)
    .range(0.0, 16_777_216.0, 1.0)
    .help("Items per block (dvbs2, bit_reverse) or bits per logical frame (dab_time).");
const DELAYS: Setting<field::Text> = Setting::new("delays", "Lane delays", "", "")
    .help("For a custom delay bank: the delay of each lane, such as 0, 4, 8.");

sdrkit_core::settings! {
    /// The settings of [`Deinterleave`].
    pub enum DeinterleaveSetting {
        /// Kind.
        Kind = KIND,
        /// Interleaver.
        Name = NAME,
        /// Branches.
        Branches = BRANCHES,
        /// Unit delay.
        Unit = UNIT,
        /// Rows.
        Rows = ROWS,
        /// Columns.
        Cols = COLS,
        /// Size.
        Size = SIZE,
        /// Lane delays.
        Delays = DELAYS,
    }
}

/// Undoes an interleaver: `fec.deinterleave` on `Llr` (fill values are erasures, 0), or `fec.deinterleave_bytes` on `Bytes`
/// (each packet deinterleaved, the convolutional kinds carrying their memory from packet to packet). The output has the kind
/// of the input.
///
/// - `forney`: the Forney convolutional deinterleaver ([`DelayLanes::forney`], receive side): `dvb` (12 branches, unit 17 bytes),
///   `atsc` (52, 4), `meteor` (36, 2048), or `custom` (`branches`, `unit`).
/// - `permutation`: a block permutation inverted ([`Permutation::invert`]): `rows_cols` (`rows` × `cols`), `dvbs2` and
///   `dvbs2_reverse` (`size` bits in `cols` columns), `bit_reverse` (`size` = 2^bits items), `nicam`, `gsm_xcch`, `dmr_trellis`,
///   `dvbt_symbol_2k`, `dvbt_symbol_8k` and `dvbt_bit_0` to `dvbt_bit_5`. On a stream, blocks are counted from the start; on
///   packets, each packet is a block and must have its length (others are dropped and counted in `dropped`).
/// - `delay`: a delay bank ([`DelayLanes`]), items going to lanes in turn: `dab_time` (the DAB time deinterleaver for logical
///   frames of `size` bits) or `custom` (`delays`, the delay of each lane in its own items, as a list such as `"0, 4, 8"`).
///
/// A `FrameStart` tag (on the item or packet where a frame starts) brings a convolutional deinterleaver back to its first lane
/// and starts a new block of a permutation. The output keeps the input's item count, so tags stay on their items.
#[derive(Debug)]
pub struct Deinterleave {
    on_bytes: bool,
    kind: DeinterleaverKind,
    name: InterleaverName,
    branches: u32,
    unit: u32,
    rows: usize,
    cols: usize,
    size: usize,
    delays: Vec<u32>,
    llr: Option<Engine<i8>>,
    bytes: Option<Engine<u8>>,
    starts: Vec<usize>,
    scratch: Vec<u8>,
    pool: PacketPool,
    dropped: u64,
    published: Published,
}

impl Default for Deinterleave {
    fn default() -> Self {
        Deinterleave {
            on_bytes: false,
            kind: KIND.default,
            name: NAME.default,
            branches: BRANCHES.default as _,
            unit: UNIT.default as _,
            rows: ROWS.default as _,
            cols: COLS.default as _,
            size: SIZE.default as _,
            delays: Vec::new(),
            llr: None,
            bytes: None,
            starts: Vec::new(),
            scratch: Vec::new(),
            pool: PacketPool::default(),
            dropped: 0,
            published: Published::new(&[&keys::DROPPED]),
        }
    }
}

fn perm_err(e: impl std::error::Error + Send + Sync + 'static) -> ConfigError {
    crate::stage(e)
}

impl Deinterleave {
    /// `fec.deinterleave_bytes`: the deinterleaver on byte packets.
    pub fn on_bytes() -> Self {
        Deinterleave {
            on_bytes: true,
            ..Self::default()
        }
    }

    fn engine<T: Copy + Default>(&self) -> Result<Engine<T>, ConfigError> {
        use DeinterleaverKind as K;
        use InterleaverName as N;
        let fill = T::default();
        let conflict = SettingError::Conflict {
            id: DeinterleaveSetting::Name.id(),
            with: DeinterleaveSetting::Kind.id(),
        };
        if !self.name.is_of(self.kind) {
            return Err(conflict.into());
        }
        Ok(match (self.kind, self.name) {
            (K::Forney, name) => {
                let (b, m) = match name {
                    N::Dvb => (12, 17),
                    N::Atsc => (52, 4),
                    N::Meteor => (36, 2048),
                    _ => (self.branches, self.unit),
                };
                Engine::Forney(DelayLanes::forney(b, m, true, fill))
            }
            (K::Delay, N::DabTime) => {
                Engine::Delay(DelayLanes::dab_time(self.size as u32, true, fill).map_err(perm_err)?)
            }
            (K::Delay, _) => {
                if self.delays.is_empty() {
                    return Err(SettingError::Required {
                        id: DeinterleaveSetting::Delays.id(),
                    }
                    .into());
                }
                Engine::Delay(DelayLanes::new(&self.delays, fill))
            }
            (K::Permutation, name) => {
                let p = match name {
                    N::RowsCols => {
                        Permutation::row_column(self.rows, self.cols, ReadOrder::Columns)
                            .map_err(perm_err)?
                    }
                    N::Dvbs2 => dvbs2_bits(self.size, self.cols, false).map_err(perm_err)?,
                    N::Dvbs2Reverse => dvbs2_bits(self.size, self.cols, true).map_err(perm_err)?,
                    N::BitReverse => {
                        if !self.size.is_power_of_two() {
                            return Err(SettingError::NotPermitted {
                                id: DeinterleaveSetting::Size.id(),
                            }
                            .into());
                        }
                        bit_reverse(self.size.trailing_zeros()).map_err(perm_err)?
                    }
                    N::Nicam => Permutation::nicam().map_err(perm_err)?,
                    N::GsmXcch => Permutation::gsm_xcch().map_err(perm_err)?,
                    N::DmrTrellis => Permutation::dmr_trellis().map_err(perm_err)?,
                    N::DvbtSymbol2k => Permutation::dvbt_symbol(false).map_err(perm_err)?,
                    N::DvbtSymbol8k => Permutation::dvbt_symbol(true).map_err(perm_err)?,
                    N::DvbtBit0 => Permutation::dvbt_bit(0).map_err(perm_err)?,
                    N::DvbtBit1 => Permutation::dvbt_bit(1).map_err(perm_err)?,
                    N::DvbtBit2 => Permutation::dvbt_bit(2).map_err(perm_err)?,
                    N::DvbtBit3 => Permutation::dvbt_bit(3).map_err(perm_err)?,
                    N::DvbtBit4 => Permutation::dvbt_bit(4).map_err(perm_err)?,
                    N::DvbtBit5 => Permutation::dvbt_bit(5).map_err(perm_err)?,
                    N::Custom | N::Dvb | N::Atsc | N::Meteor | N::DabTime => {
                        return Err(conflict.into())
                    }
                };
                let n = p.len();
                Engine::Perm {
                    p,
                    block: Vec::with_capacity(n),
                    out: vec![fill; n],
                }
            }
        })
    }
}

/// Positions (relative to `first`, below `len`) of the `FrameStart` tags.
fn frame_starts(tags: &[Tag], first: u64, len: usize, out: &mut Vec<usize>) {
    out.clear();
    out.extend(
        tags.iter()
            .filter(|t| matches!(t.kind, TagKind::FrameStart { .. }))
            .map(|t| t.at.saturating_sub(first) as usize)
            .filter(|&r| r < len),
    );
}

impl Component for Deinterleave {
    fn info(&self) -> ComponentInfo {
        let kind = if self.on_bytes {
            Kind::Bytes
        } else {
            Kind::Llr
        };
        let name = if self.on_bytes {
            "fec.deinterleave_bytes"
        } else {
            "fec.deinterleave"
        };
        ComponentInfo::processor(
            name,
            vec![Port::new("in", kind)],
            vec![Port::new("out", kind).same_format_as(0)],
        )
        .with_settings(DeinterleaveSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.llr = None;
        self.bytes = None;
        if self.on_bytes {
            self.bytes = Some(self.engine::<u8>()?);
        } else {
            self.llr = Some(self.engine::<i8>()?);
        }
        self.starts.clear();
        self.starts.reserve(16);
        self.dropped = 0;
        Ok(vec![inputs[0].clone()])
    }

    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        let first = input.first();
        frame_starts(input.tags(), first, io.input_len(0), &mut self.starts);
        if let Some(e) = self.llr.as_mut() {
            let x = io.llrs(0);
            e.run(x, &self.starts, io.llrs_out(0));
            return;
        }
        let Some(e) = self.bytes.as_mut() else {
            return;
        };
        let mut dropped = false;
        for (i, p) in io.bytes(0).iter().enumerate() {
            if self.starts.contains(&i) {
                e.sync(&mut self.scratch);
            }
            let x = p.bytes();
            if e.fits(x.len()) {
                let pkt = self.pool.make(x.len(), p.time, |b| e.packet(x, b));
                io.bytes_out(0).push(pkt);
            } else {
                self.dropped += 1;
                dropped = true;
            }
        }
        if dropped {
            self.published.set(&keys::DROPPED, self.dropped as i64);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DeinterleaveSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DeinterleaveSetting::Kind => self.kind = KIND.read(id, v)?,
            DeinterleaveSetting::Name => self.name = NAME.read(id, v)?,
            DeinterleaveSetting::Branches => self.branches = BRANCHES.read(id, v)? as u32,
            DeinterleaveSetting::Unit => self.unit = UNIT.read(id, v)? as u32,
            DeinterleaveSetting::Rows => self.rows = ROWS.read(id, v)? as usize,
            DeinterleaveSetting::Cols => self.cols = COLS.read(id, v)? as usize,
            DeinterleaveSetting::Size => self.size = SIZE.read(id, v)? as usize,
            DeinterleaveSetting::Delays => {
                let t = DELAYS.read(id, v)?;
                let mut d = Vec::new();
                for (i, part) in t.split(',').map(str::trim).enumerate() {
                    if part.is_empty() {
                        continue;
                    }
                    let x = part
                        .parse::<u32>()
                        .map_err(|_| SettingError::Malformed { id, at: Some(i) })?;
                    d.push(x);
                }
                self.delays = d;
            }
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        let delays = self
            .delays
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        values_of([
            (DeinterleaveSetting::Kind, Value::Choice(self.kind.index())),
            (DeinterleaveSetting::Name, Value::Choice(self.name.index())),
            (
                DeinterleaveSetting::Branches,
                Value::I64(self.branches as i64),
            ),
            (DeinterleaveSetting::Unit, Value::I64(self.unit as i64)),
            (DeinterleaveSetting::Rows, Value::I64(self.rows as i64)),
            (DeinterleaveSetting::Cols, Value::I64(self.cols as i64)),
            (DeinterleaveSetting::Size, Value::I64(self.size as i64)),
            (DeinterleaveSetting::Delays, Value::Text(delays)),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

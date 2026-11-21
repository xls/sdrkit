//! `bits.deinterleave`: block (permutation) and convolutional (delay-lane) deinterleaving of hard bits or LLRs.

use super::{bit_ports, walk, BitItem, FrameStartChoice, Step};
use sdrkit_core::publish::field;
use sdrkit_core::{Choice, ConfigError, Setting, SettingError, Value, Values};
use sdrkit_core::{SettingId, SettingSet};
use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, TagCarry, TagKind};
use sdrkit_util_bits::interleave::{DelayLanes, Permutation, ReadOrder};

sdrkit_core::choice_enum! {
    /// The interleaver that `bits.deinterleave` undoes.
    pub enum BitInterleaver {
        /// Written by rows, read by columns (`rows`, `cols`).
        RowColumn = ("row_column", "Rows and columns"),
        /// The IEEE 802.11 BCC interleaver (`n_cbps`, `n_bpsc`, `n_col`).
        Wifi = ("wifi", "IEEE 802.11"),
        /// The DRM interleaver, ETSI ES 201 980 clause 7.3.3 (`length`, `t0`).
        Drm = ("drm", "DRM"),
        /// The DVB-T bit interleaver, EN 300 744 clause 4.3.4.1 (`branch`).
        DvbtBit = ("dvbt_bit", "DVB-T bit"),
        /// The HFDL interleaver, ARINC 635 (`length`, `shift`).
        Hfdl = ("hfdl", "HFDL"),
        /// The LoRa diagonal interleaver (`rows`, `cols`).
        Lora = ("lora", "LoRa"),
        /// The Forney convolutional interleaver (`branches`, `unit`).
        Forney = ("forney", "Forney"),
        /// The DAB time interleaver, EN 300 401 clause 12 (`cif_bits`).
        DabTime = ("dab_time", "DAB time"),
    }
}

const KIND: Setting<field::Choice<BitInterleaver>> =
    Setting::choice("kind", "Interleaver", BitInterleaver::RowColumn)
        .help("Which interleaver to undo.");
const START: Setting<field::Choice<FrameStartChoice>> =
    Setting::choice("start", "Block start", FrameStartChoice::FrameStart)
        .help("Where blocks begin, for the block kinds.");
const ROWS: Setting<field::I64> = Setting::new("rows", "Rows", "", 8)
    .range(1.0, 65_536.0, 1.0)
    .help("row_column and lora: rows of the matrix (for lora, the spreading factor).");
const COLS: Setting<field::I64> = Setting::new("cols", "Columns", "", 8)
    .range(1.0, 65_536.0, 1.0)
    .help("row_column and lora: columns of the matrix (for lora, 4 + coding rate).");
const N_CBPS: Setting<field::I64> = Setting::new("n_cbps", "Coded bits per symbol", "", 48)
    .range(1.0, 65_536.0, 1.0)
    .help("wifi: coded bits per OFDM symbol.");
const N_BPSC: Setting<field::I64> = Setting::new("n_bpsc", "Bits per subcarrier", "", 1)
    .range(1.0, 16.0, 1.0)
    .help("wifi: coded bits per subcarrier.");
const N_COL: Setting<field::I64> = Setting::new("n_col", "Columns", "", 16)
    .range(1.0, 64.0, 1.0)
    .help("wifi: 16 for non-HT, 13 or 18 for HT.");
const LENGTH: Setting<field::I64> = Setting::new("length", "Length", "", 40)
    .range(1.0, 4_194_304.0, 1.0)
    .help("drm and hfdl: items per block (hfdl: a multiple of 40).");
const T0: Setting<field::I64> = Setting::new("t0", "Multiplier", "", 5)
    .range(1.0, 65_536.0, 1.0)
    .help("drm: 5 for bits, 13 or 21 for cells.");
const SHIFT: Setting<field::I64> = Setting::new("shift", "Column shift", "", 0)
    .range(0.0, 65_536.0, 1.0)
    .help("hfdl: the column step of the data rate.");
const BRANCH: Setting<field::I64> = Setting::new("branch", "Branch", "", 0)
    .range(0.0, 5.0, 1.0)
    .help("dvbt_bit: the bit interleaver branch, 0 to 5.");
const BRANCHES: Setting<field::I64> = Setting::new("branches", "Branches", "", 12)
    .range(1.0, 256.0, 1.0)
    .help("forney: branches (12 for DVB, 52 for ATSC).");
const UNIT: Setting<field::I64> = Setting::new("unit", "Unit delay", "", 17)
    .range(1.0, 4096.0, 1.0)
    .help("forney: the unit delay (17 for DVB, 4 for ATSC).");
const CIF_BITS: Setting<field::I64> = Setting::new("cif_bits", "Logical frame", "", 3072)
    .range(16.0, 4_194_304.0, 1.0)
    .help("dab_time: bits per logical frame, a multiple of 16.");

sdrkit_core::settings! {
    /// The settings of [`Deinterleaver`].
    pub enum DeinterleaverSetting {
        /// Interleaver.
        Kind = KIND,
        /// Block start.
        Start = START,
        /// Rows.
        Rows = ROWS,
        /// Columns.
        Cols = COLS,
        /// Coded bits per symbol.
        NCbps = N_CBPS,
        /// Bits per subcarrier.
        NBpsc = N_BPSC,
        /// Columns (wifi).
        NCol = N_COL,
        /// Length.
        Length = LENGTH,
        /// Multiplier.
        T0 = T0,
        /// Column shift.
        Shift = SHIFT,
        /// Branch.
        Branch = BRANCH,
        /// Branches.
        Branches = BRANCHES,
        /// Unit delay.
        Unit = UNIT,
        /// Logical frame.
        CifBits = CIF_BITS,
    }
}

/// The parameter settings, in the order of [`DeinterleaverSetting`] after `start`.
const PARAMS: [(DeinterleaverSetting, &Setting<field::I64>); 12] = [
    (DeinterleaverSetting::Rows, &ROWS),
    (DeinterleaverSetting::Cols, &COLS),
    (DeinterleaverSetting::NCbps, &N_CBPS),
    (DeinterleaverSetting::NBpsc, &N_BPSC),
    (DeinterleaverSetting::NCol, &N_COL),
    (DeinterleaverSetting::Length, &LENGTH),
    (DeinterleaverSetting::T0, &T0),
    (DeinterleaverSetting::Shift, &SHIFT),
    (DeinterleaverSetting::Branch, &BRANCH),
    (DeinterleaverSetting::Branches, &BRANCHES),
    (DeinterleaverSetting::Unit, &UNIT),
    (DeinterleaverSetting::CifBits, &CIF_BITS),
];

/// The parameter values, in the order of [`PARAMS`].
#[derive(Clone, Copy, Debug)]
struct Params([i64; 12]);

impl Params {
    fn defaults() -> Self {
        Params(PARAMS.map(|(_, s)| s.default))
    }
    /// The slot of setting `s`, if it is a parameter.
    fn slot(s: DeinterleaverSetting) -> Option<usize> {
        PARAMS.iter().position(|(p, _)| *p == s)
    }
    fn get(&self, s: DeinterleaverSetting) -> usize {
        Self::slot(s).map_or(0, |i| self.0[i] as usize)
    }
}

impl BitInterleaver {
    /// The first parameter of this kind.
    fn main_param(self) -> DeinterleaverSetting {
        use DeinterleaverSetting as S;
        match self {
            BitInterleaver::RowColumn | BitInterleaver::Lora => S::Rows,
            BitInterleaver::Wifi => S::NCbps,
            BitInterleaver::Drm | BitInterleaver::Hfdl => S::Length,
            BitInterleaver::DvbtBit => S::Branch,
            BitInterleaver::Forney => S::Branches,
            BitInterleaver::DabTime => S::CifBits,
        }
    }
}

/// The deinterleaver a kind and its parameters make.
#[derive(Clone, Debug)]
enum Plan {
    /// A block of `len()` items, undone with [`Permutation::invert`].
    Block(Permutation),
    /// Delay lanes on the receive side.
    Forney(u32, u32),
    /// The DAB time deinterleaver of this many bits per logical frame.
    DabTime(u32),
}

/// Running state for one item type.
#[derive(Clone, Debug)]
struct State<T: Copy> {
    lanes: Option<DelayLanes<T>>,
    frame: Vec<T>,
    have: usize,
    waiting: bool,
}

impl<T: Copy> Default for State<T> {
    fn default() -> Self {
        State {
            lanes: None,
            frame: Vec::new(),
            have: 0,
            waiting: true,
        }
    }
}

/// `bits.deinterleave` and `bits.deinterleave_llr`: undoes a block interleaver ([`Permutation`]) or a convolutional one ([`DelayLanes`]) on hard bits or
/// LLRs (whole values move, so soft bits keep their reliability; lanes start filled with 0, an erasure for LLRs).
///
/// [`BitInterleaver`] names the parameters of each kind. If a parameter value makes the current kind impossible, it is refused
/// with [`SettingError::Conflict`].
///
/// Block kinds gather a block, then output it deinterleaved, so the output runs one block late. With `start` =
/// `frame_start` (the default) a block begins at each `FrameStart` tag and items outside a block are dropped; with `continuous`
/// blocks follow each other from the first item, and a `FrameStart` tag starts a new block there (a partial block is dropped).
/// Tags of a block move to its first output item. Delay-lane kinds work item by item: a `FrameStart` tag puts its item into
/// lane 0 (a DVB sync byte), and tags stay at their positions.
#[derive(Clone, Debug)]
pub struct Deinterleaver<T: BitItem> {
    kind: BitInterleaver,
    params: Params,
    start: FrameStartChoice,
    plan: Plan,
    state: State<T>,
    carry: TagCarry,
    tags: Vec<TagKind>,
}

impl<T: BitItem> Default for Deinterleaver<T> {
    fn default() -> Self {
        let params = Params::defaults();
        let plan = Self::plan(KIND.default, &params).expect("the default interleaver is valid");
        Deinterleaver {
            kind: KIND.default,
            params,
            start: START.default,
            plan,
            state: State::default(),
            carry: TagCarry::same_rate(),
            tags: Vec::new(),
        }
    }
}

/// The most tags held for a block that has not come out yet.
const MAX_HELD_TAGS: usize = 64;

impl<T: BitItem> Deinterleaver<T> {
    const NAMES: [&'static str; 2] = ["bits.deinterleave", "bits.deinterleave_llr"];

    /// The plan for `kind` with `params`, or `None` when they do not make one.
    fn plan(kind: BitInterleaver, params: &Params) -> Option<Plan> {
        use DeinterleaverSetting as S;
        let p = |s| params.get(s);
        let block = match kind {
            BitInterleaver::RowColumn => {
                Permutation::row_column(p(S::Rows), p(S::Cols), ReadOrder::Columns)
            }
            BitInterleaver::Wifi => Permutation::wifi(p(S::NCbps), p(S::NBpsc), p(S::NCol)),
            BitInterleaver::Drm => Permutation::drm(p(S::Length), p(S::T0)),
            BitInterleaver::DvbtBit => Permutation::dvbt_bit(p(S::Branch)),
            BitInterleaver::Hfdl => Permutation::hfdl(p(S::Length), p(S::Shift)),
            BitInterleaver::Lora => Permutation::lora(p(S::Rows), p(S::Cols)),
            BitInterleaver::Forney => {
                let (b, m) = (p(S::Branches) as u32, p(S::Unit) as u32);
                return (b >= 1 && m >= 1).then_some(Plan::Forney(b, m));
            }
            BitInterleaver::DabTime => {
                let c = p(S::CifBits) as u32;
                return DelayLanes::<u8>::dab_time(c, true, 0)
                    .ok()
                    .map(|_| Plan::DabTime(c));
            }
        };
        match block {
            Ok(b) if b.len() <= 1 << 22 => Some(Plan::Block(b)),
            _ => None,
        }
    }

    fn lanes(plan: &Plan) -> Option<DelayLanes<T>> {
        match *plan {
            Plan::Block(_) => None,
            Plan::Forney(b, m) => Some(DelayLanes::forney(b, m, true, T::default())),
            Plan::DabTime(c) => DelayLanes::dab_time(c, true, T::default()).ok(),
        }
    }

    fn reset(plan: &Plan, st: &mut State<T>) {
        st.lanes = Self::lanes(plan);
        st.frame.clear();
        if let Plan::Block(p) = plan {
            st.frame.resize(p.len(), T::default());
        }
        st.have = 0;
        st.waiting = true;
    }

    fn restart(&mut self) {
        Self::reset(&self.plan, &mut self.state);
        self.tags.clear();
        self.tags.reserve(MAX_HELD_TAGS);
    }

    fn run(
        io: &mut Io,
        plan: &Plan,
        st: &mut State<T>,
        start_tags: bool,
        carry: &mut TagCarry,
        held: &mut Vec<TagKind>,
    ) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let x = T::read(io, 0);
        match plan {
            Plan::Block(perm) => {
                let len = perm.len();
                walk(first, x.len(), tags, |step| match step {
                    Step::Items(a, b) => {
                        for &v in &x[a..b] {
                            if st.waiting && start_tags {
                                continue;
                            }
                            st.waiting = false;
                            st.frame[st.have] = v;
                            st.have += 1;
                            if st.have == len {
                                let at = io.output(0).end();
                                for k in held.drain(..) {
                                    carry.queue(at, k);
                                }
                                let out = T::write(io, 0);
                                let s = out.len();
                                out.resize(s + len, T::ZERO);
                                // Both slices hold exactly `len` items, so this cannot fail.
                                let _ = perm.invert(&st.frame, &mut out[s..]);
                                st.have = 0;
                                st.waiting = true;
                            }
                        }
                    }
                    Step::Tag(_, t) => {
                        if matches!(t.kind, TagKind::FrameStart { .. }) {
                            st.have = 0;
                            st.waiting = false;
                        }
                        if held.len() >= MAX_HELD_TAGS {
                            held.remove(0);
                        }
                        held.push(t.kind.clone());
                    }
                });
            }
            Plan::Forney(..) | Plan::DabTime(_) => {
                let out = T::write(io, 0);
                let s = out.len();
                out.extend_from_slice(x);
                let y = &mut out[s..];
                walk(first, x.len(), tags, |step| match step {
                    Step::Items(a, b) => {
                        if let Some(l) = st.lanes.as_mut() {
                            l.process(&mut y[a..b]);
                        }
                    }
                    Step::Tag(at, t) => {
                        if matches!(t.kind, TagKind::FrameStart { .. }) {
                            if let Some(l) = st.lanes.as_mut() {
                                l.sync();
                            }
                        }
                        carry.queue(first + at as u64, t.kind.clone());
                    }
                });
            }
        }
        carry.flush(io.output(0));
    }
}

impl<T: BitItem> Component for Deinterleaver<T> {
    fn info(&self) -> ComponentInfo {
        let (i, o) = bit_ports::<T>();
        ComponentInfo::processor(Self::NAMES[T::INDEX], i, o)
            .manual_tags()
            .with_settings(DeinterleaverSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        self.carry.reset(1.0, 1.0);
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        Self::run(
            io,
            &self.plan,
            &mut self.state,
            self.start == FrameStartChoice::FrameStart,
            &mut self.carry,
            &mut self.tags,
        );
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let setting = DeinterleaverSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        let (mut kind, mut params) = (self.kind, self.params);
        let with = match setting {
            DeinterleaverSetting::Start => {
                self.start = START.read(id, v)?;
                return Ok(Applied::Live);
            }
            DeinterleaverSetting::Kind => {
                kind = KIND.read(id, v)?;
                kind.main_param()
            }
            p => {
                let i = Params::slot(p).ok_or(SettingError::NoSuchId { id })?;
                params.0[i] = PARAMS[i].1.read(id, v)?;
                DeinterleaverSetting::Kind
            }
        };
        let plan = Self::plan(kind, &params).ok_or(SettingError::Conflict {
            id,
            with: with.id(),
        })?;
        (self.kind, self.params, self.plan) = (kind, params, plan);
        self.restart();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of(
            [
                (DeinterleaverSetting::Kind, Value::Choice(self.kind.index())),
                (
                    DeinterleaverSetting::Start,
                    Value::Choice(self.start.index()),
                ),
            ]
            .into_iter()
            .chain(
                PARAMS
                    .iter()
                    .zip(self.params.0)
                    .map(|((s, _), x)| (*s, Value::I64(x))),
            ),
        )
    }
}

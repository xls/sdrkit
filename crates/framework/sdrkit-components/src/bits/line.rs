//! Line decoders: `bits.nrzi`, `bits.nrzm`, `bits.diff` and `bits.manchester`, each also on LLRs (`_llr`).

use super::{bit_ports, walk, BitItem, Step};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Every, Format, Io, Port, RateFrom, TagCarry,
    TagKind,
};
use sdrkit_util_bits::line::{ManchesterConv, NrzM, Nrzi};
use std::marker::PhantomData;
use std::num::NonZeroU32;

/// Which differential line code a [`LineDecoder`] undoes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Code {
    /// NRZI (NRZ-S): a change of level is 0.
    Nrzi,
    /// NRZ-M: a change of level is 1.
    NrzM,
    /// The differential code (data = level XOR previous level): NRZ-M under the name that RDS, GSM and DBPSK use.
    Diff,
}

impl Code {
    /// The registered names: hard bits, then LLRs.
    fn names(self) -> [&'static str; 2] {
        match self {
            Code::Nrzi => ["bits.nrzi", "bits.nrzi_llr"],
            Code::NrzM => ["bits.nrzm", "bits.nrzm_llr"],
            Code::Diff => ["bits.diff", "bits.diff_llr"],
        }
    }
}

const LEVEL: Setting<field::I64> = Setting::new("level", "Starting level", "", 0)
    .range(0.0, 1.0, 1.0)
    .help("The line level before the first bit, 0 or 1.");

sdrkit_core::settings! {
    /// The settings of [`LineDecoder`].
    pub enum LineDecoderSetting {
        /// Starting level.
        Level = LEVEL,
    }
}

/// A differential line decoder: line levels in, data bits out, one for one. On LLRs each data LLR is the XOR rule applied to
/// two level LLRs, so its reliability is that of the weaker level.
///
/// Registered as `bits.nrzi` ([`Nrzi`]: AIS, AX.25, VDL Mode 4), `bits.nrzm` ([`NrzM`]: CCSDS NRZ-M) and `bits.diff` (the
/// NRZ-M rule under the name that RDS, GSM and DBPSK use), each with an `_llr` form. Tags pass through unchanged.
///
/// ```
/// use sdrkit_components::bits::LineDecoder;
/// use sdrkit_components::testing::{run_passes, Source};
/// use sdrkit_graph::components::NullSink;
/// use sdrkit_graph::{stream::Data, Format, GraphBuilder};
///
/// // NRZI: a change of level is 0, no change is 1.
/// let mut g = GraphBuilder::new()
///     .add("src", Source::new(Format::hard_bits(9600.0), Data::HardBits(vec![1, 1, 0, 0, 0, 1]), 4))
///     .add("nrzi", LineDecoder::<u8>::nrzi())
///     .add("sink", NullSink::default())
///     .connect("src", "nrzi")
///     .connect("nrzi", "sink")
///     .build()
///     .unwrap();
/// let out = run_passes(&mut g, 2, "nrzi.out");
/// assert!(matches!(out, Data::HardBits(ref b) if b == &[0, 1, 0, 1, 1, 0]));
/// ```
#[derive(Clone, Debug)]
pub struct LineDecoder<T: BitItem> {
    code: Code,
    level: u8,
    nrzi: Nrzi,
    nrzm: NrzM,
    prev: i8,
    bits: PhantomData<T>,
}

impl<T: BitItem> LineDecoder<T> {
    fn new(code: Code) -> Self {
        LineDecoder {
            code,
            level: 0,
            nrzi: Nrzi::new(0),
            nrzm: NrzM::new(0),
            prev: 0,
            bits: PhantomData,
        }
        .restarted()
    }
    /// `bits.nrzi`.
    pub fn nrzi() -> Self {
        Self::new(Code::Nrzi)
    }
    /// `bits.nrzm`.
    pub fn nrzm() -> Self {
        Self::new(Code::NrzM)
    }
    /// `bits.diff`.
    pub fn diff() -> Self {
        Self::new(Code::Diff)
    }
    fn restarted(mut self) -> Self {
        self.restart();
        self
    }
    fn restart(&mut self) {
        use sdrkit_util_bits::Soft;
        self.nrzi = Nrzi::new(self.level);
        self.nrzm = NrzM::new(self.level);
        self.prev = i8::from_hard(self.level);
    }
}

impl<T: BitItem> Component for LineDecoder<T> {
    fn info(&self) -> ComponentInfo {
        let (i, o) = bit_ports::<T>();
        ComponentInfo::processor(self.code.names()[T::INDEX], i, o)
            .with_settings(LineDecoderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = T::read(io, 0);
        let out = T::write(io, 0);
        let s = out.len();
        out.extend_from_slice(x);
        T::line_decode(
            self.code == Code::Nrzi,
            &mut self.nrzi,
            &mut self.nrzm,
            &mut self.prev,
            &mut out[s..],
        );
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match LineDecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LineDecoderSetting::Level => {
                self.level = LEVEL.read(id, v)? as u8;
                self.restart();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(LineDecoderSetting::Level, Value::I64(i64::from(self.level)))])
    }
}

/// Chip pairs that were not a valid Manchester pair (00 or 11), for `bits.manchester`.
pub const PAIRS_BAD: Field<field::I64> = Field::new("pairs_bad", "Bad chip pairs", "");

sdrkit_core::choice_enum! {
    /// Which Manchester chip pair stands for a 1.
    pub enum ManchesterConvention {
        /// IEEE 802.3 (01 is 1).
        Ieee = ("ieee", "IEEE 802.3 (01 is 1)"),
        /// G. E. Thomas (10 is 1).
        Thomas = ("thomas", "G. E. Thomas (10 is 1)"),
    }
}

impl ManchesterConvention {
    fn conv(self) -> ManchesterConv {
        match self {
            ManchesterConvention::Ieee => ManchesterConv::Ieee,
            ManchesterConvention::Thomas => ManchesterConv::Thomas,
        }
    }
}

const CONVENTION: Setting<field::Choice<ManchesterConvention>> =
    Setting::choice("convention", "Convention", ManchesterConvention::Ieee)
        .help("Which chip pair stands for a 1.");

sdrkit_core::settings! {
    /// The settings of [`ManchesterDecoder`].
    pub enum ManchesterDecoderSetting {
        /// Convention.
        Convention = CONVENTION,
    }
}

/// `bits.manchester` and `bits.manchester_llr`: Manchester chips in, bits out at half the rate
/// ([`Manchester`](sdrkit_util_bits::line::Manchester)).
///
/// With LLRs the two chips are two observations of the bit and their LLRs add ([`Manchester::decode_soft`](sdrkit_util_bits::line::Manchester::decode_soft)). With hard bits an
/// invalid pair (00 or 11) is decided by its second chip (IEEE) or its first (Thomas) and counted in the result `pairs_bad`.
///
/// Pairs are counted from the first chip. A `FrameStart` tag starts a new pair at its chip and drops a half pair before it. Tags
/// move to the bit made from the pair that holds their chip.
#[derive(Debug)]
pub struct ManchesterDecoder<T: BitItem> {
    convention: ManchesterConvention,
    half: Option<T>,
    bad: u64,
    carry: TagCarry,
    every: Every,
    published: Published,
}

impl<T: BitItem> Default for ManchesterDecoder<T> {
    fn default() -> Self {
        ManchesterDecoder {
            convention: CONVENTION.default,
            half: None,
            bad: 0,
            carry: TagCarry::same_rate(),
            every: Every::default(),
            published: Published::new(&[&PAIRS_BAD]),
        }
    }
}

impl<T: BitItem> ManchesterDecoder<T> {
    const NAMES: [&'static str; 2] = ["bits.manchester", "bits.manchester_llr"];

    fn run(&mut self, x: &[T], out: &mut Vec<T>) {
        let conv = self.convention.conv();
        let mut x = x;
        if let Some(a) = self.half.take() {
            let Some((&b, rest)) = x.split_first() else {
                self.half = Some(a);
                return;
            };
            self.bad += T::manchester(&[a, b], conv, out);
            x = rest;
        }
        let even = x.len() & !1;
        self.bad += T::manchester(&x[..even], conv, out);
        if even < x.len() {
            self.half = Some(x[even]);
        }
    }
}

/// Manchester sends each data bit as two line chips.
const CHIPS_PER_BIT: NonZeroU32 = NonZeroU32::new(2).unwrap();

impl<T: BitItem> Component for ManchesterDecoder<T> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            Self::NAMES[T::INDEX],
            vec![Port::new("in", T::KIND)],
            vec![Port::new("out", T::KIND).rate_from(RateFrom::Divided {
                input: 0,
                by: CHIPS_PER_BIT,
            })],
        )
        .manual_tags()
        .with_settings(ManchesterDecoderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.half = None;
        self.bad = 0;
        let rate = inputs[0].rate / 2.0;
        self.carry.reset(inputs[0].rate, rate);
        self.every = Every::new(inputs[0].rate, 0.25);
        self.published.set(&PAIRS_BAD, 0);
        Ok(vec![inputs[0].with_rate(rate)])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        let n = io.input_len(0);
        let (first, tags) = (input.first(), input.tags());
        let x = T::read(io, 0);
        walk(first, n, tags, |s| match s {
            Step::Items(a, b) => {
                let out = T::write(io, 0);
                self.run(&x[a..b], out);
            }
            Step::Tag(_, t) => {
                if matches!(t.kind, TagKind::FrameStart { .. }) {
                    self.half = None;
                }
                let at = io.output(0).end();
                self.carry.queue(at, t.kind.clone());
            }
        });
        self.carry.flush(io.output(0));
        if self.every.tick(n) {
            self.published.set(&PAIRS_BAD, self.bad as i64);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ManchesterDecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ManchesterDecoderSetting::Convention => self.convention = CONVENTION.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            ManchesterDecoderSetting::Convention,
            Value::Choice(self.convention.index()),
        )])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

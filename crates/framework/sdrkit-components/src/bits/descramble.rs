//! Descramblers: `bits.descramble` (additive), `bits.self_sync` (multiplicative) and `bytes.descramble` (additive, on byte
//! packets).

use super::{bit_ports, walk, BitItem, PacketPool, Step};
use sdrkit_core::publish::field;
use sdrkit_core::{CatalogueSetting, ChoiceDecl};
use sdrkit_core::{Choice, ConfigError, Setting, SettingError, Value, Values};
use sdrkit_core::{SettingId, SettingSet};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Packet, Port, TagKind,
};
use sdrkit_util_bits::lfsr::{degree, Additive, Lfsr, Multiplicative};
use sdrkit_util_bits::seq::{self, ByteSequence};
use std::marker::PhantomData;

/// The named bit sequences of `bits.descramble`: [`seq::SCRAMBLERS`], by the lower-case name of their constant.
pub use sdrkit_util_bits::seq::SCRAMBLERS as BIT_SEQUENCES;

/// The named byte sequences of `bytes.descramble`: [`seq::BYTE_SEQUENCES`] ([`ByteSequence::named`]).
pub use sdrkit_util_bits::seq::BYTE_SEQUENCES;

/// The largest whole number that a setting carries exactly (an `f64`).
const MAX_EXACT: f64 = 9_007_199_254_740_992.0;

sdrkit_core::choice_enum! {
    /// How the register of a custom scrambler feeds back.
    pub enum RegisterForm {
        /// Fibonacci.
        Fibonacci = ("fibonacci", "Fibonacci"),
        /// Galois.
        Galois = ("galois", "Galois"),
    }
}

sdrkit_core::choice_enum! {
    /// Which bit of a custom Fibonacci register is the sequence.
    pub enum SequenceOutput {
        /// The new bit.
        Feedback = ("feedback", "The new bit"),
        /// The oldest stage, so the seed comes out first.
        Oldest = ("oldest", "The oldest stage"),
    }
}

/// The key of the custom register in the `sequence` list of `bits.descramble`.
const CUSTOM: &str = "custom";

const SEQUENCE: CatalogueSetting = CatalogueSetting {
    key: "sequence",
    label: "Sequence",
    help: "A named scrambler sequence, or custom for the register below.",
    choices: || {
        BIT_SEQUENCES
            .iter()
            .map(|(k, _)| *k)
            .chain([CUSTOM])
            .map(|k| ChoiceDecl::new(k, k))
            .collect()
    },
    default: "prbs9_dispersal",
    advanced: false,
};
const POLY: Setting<field::I64> = Setting::new("poly", "Polynomial", "", 0x221)
    .range(2.0, MAX_EXACT, 1.0)
    .help("The register's polynomial in recurrence notation: x^9 + x^5 + 1 is 545 (0x221).");
const SEED: Setting<field::I64> = Setting::new("seed", "Seed", "", 1)
    .range(0.0, MAX_EXACT, 1.0)
    .help("The register at the start, stage 1 in bit 0. It must fit in the polynomial's degree.");
const FORM: Setting<field::Choice<RegisterForm>> =
    Setting::choice("form", "Register form", RegisterForm::Fibonacci)
        .help("How the register feeds back.");
const OUTPUT: Setting<field::Choice<SequenceOutput>> =
    Setting::choice("output", "Output", SequenceOutput::Feedback)
        .help("Which bit of a custom Fibonacci register is the sequence.");
const PERIOD_BITS: Setting<field::I64> = Setting::new("period", "Period", "bits", 0)
    .range(0.0, MAX_EXACT, 1.0)
    .help("Bits after which the sequence starts again. 0 for never.");
const RESTART: Setting<field::Bool> = Setting::new("restart", "Restart at frame start", "", true)
    .help("Start the sequence again at each frame start.");

sdrkit_core::settings! {
    /// The settings of [`Descrambler`].
    pub enum DescramblerSetting {
        /// Sequence.
        Sequence = SEQUENCE,
        /// Polynomial.
        Poly = POLY,
        /// Seed.
        Seed = SEED,
        /// Register form.
        Form = FORM,
        /// Output.
        Output = OUTPUT,
        /// Period.
        Period = PERIOD_BITS,
        /// Restart at frame start.
        Restart = RESTART,
    }
}

/// `bits.descramble` and `bits.descramble_llr`: an additive descrambler ([`Additive`]). Each bit is XORed with a sequence: hard
/// bits flip, LLRs change sign and keep their magnitude.
///
/// `sequence` is a name from [`BIT_SEQUENCES`], or `custom` for the register of `poly`, `seed`, `form` and `output`. The
/// polynomial is in recurrence notation ([`sdrkit_util_bits::lfsr`]). The seed must fit in the polynomial's degree, so set
/// `poly` first. With `restart` on, a `FrameStart` tag starts the sequence again at the tagged bit.
#[derive(Clone, Debug)]
pub struct Descrambler<T: BitItem> {
    sequence: u16,
    poly: u64,
    seed: u64,
    form: RegisterForm,
    output: SequenceOutput,
    period: u64,
    restart: bool,
    s: Additive,
    count: u64,
    bits: PhantomData<T>,
}

impl<T: BitItem> Default for Descrambler<T> {
    fn default() -> Self {
        let sequence = BIT_SEQUENCES
            .iter()
            .position(|(k, _)| *k == SEQUENCE.default)
            .expect("the default sequence is in the list") as u16;
        let mut d = Descrambler {
            sequence,
            poly: POLY.default as u64,
            seed: SEED.default as u64,
            form: FORM.default,
            output: OUTPUT.default,
            period: 0,
            restart: true,
            s: Additive::new(seq::PRBS9_DISPERSAL),
            count: 0,
            bits: PhantomData,
        };
        d.rebuild();
        d
    }
}

impl<T: BitItem> Descrambler<T> {
    const NAMES: [&'static str; 2] = ["bits.descramble", "bits.descramble_llr"];

    fn register(&self) -> Lfsr {
        if let Some(&(_, l)) = BIT_SEQUENCES.get(usize::from(self.sequence)) {
            return l;
        }
        if self.form == RegisterForm::Galois {
            Lfsr::galois(self.poly, self.seed)
        } else {
            let l = Lfsr::fibonacci(self.poly, self.seed);
            if self.output == SequenceOutput::Oldest {
                l.with_output(1u64 << (degree(self.poly) - 1))
            } else {
                l
            }
        }
    }
    fn rebuild(&mut self) {
        self.s = Additive::new(self.register());
        self.count = 0;
    }
    fn reset(&mut self) {
        self.s.reset();
        self.count = 0;
    }
    /// Descrambles `y` in place, restarting every `period` bits.
    fn descramble(&mut self, mut y: &mut [T]) {
        while !y.is_empty() {
            let take = if self.period > 0 {
                ((self.period - self.count) as usize).min(y.len())
            } else {
                y.len()
            };
            let (now, rest) = y.split_at_mut(take);
            self.s.apply_soft(now);
            self.count += take as u64;
            if self.period > 0 && self.count >= self.period {
                self.reset();
            }
            y = rest;
        }
    }
    fn run(&mut self, io: &mut Io) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let x = T::read(io, 0);
        let out = T::write(io, 0);
        let s = out.len();
        out.extend_from_slice(x);
        let y = &mut out[s..];
        let restart = self.restart;
        walk(first, x.len(), tags, |step| match step {
            Step::Items(a, b) => self.descramble(&mut y[a..b]),
            Step::Tag(_, t) => {
                if restart && matches!(t.kind, TagKind::FrameStart { .. }) {
                    self.reset();
                }
            }
        });
    }
}

impl<T: BitItem> Component for Descrambler<T> {
    fn info(&self) -> ComponentInfo {
        let (i, o) = bit_ports::<T>();
        ComponentInfo::processor(Self::NAMES[T::INDEX], i, o)
            .with_settings(DescramblerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rebuild();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DescramblerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DescramblerSetting::Sequence => {
                let i = SEQUENCE.read(id, v)?;
                if i > BIT_SEQUENCES.len() {
                    return Err(SettingError::NotAChoice {
                        id,
                        index: i as u16,
                    });
                }
                self.sequence = i as u16;
            }
            DescramblerSetting::Poly => {
                let p = POLY.read(id, v)? as u64;
                if self.seed >> degree(p) != 0 {
                    return Err(SettingError::Conflict {
                        id,
                        with: DescramblerSetting::Seed.id(),
                    });
                }
                self.poly = p;
            }
            DescramblerSetting::Seed => {
                let s = SEED.read(id, v)? as u64;
                if s >> degree(self.poly) != 0 {
                    return Err(SettingError::Conflict {
                        id,
                        with: DescramblerSetting::Poly.id(),
                    });
                }
                self.seed = s;
            }
            DescramblerSetting::Form => self.form = FORM.read(id, v)?,
            DescramblerSetting::Output => self.output = OUTPUT.read(id, v)?,
            DescramblerSetting::Period => self.period = PERIOD_BITS.read(id, v)? as u64,
            DescramblerSetting::Restart => self.restart = RESTART.read(id, v)?,
        }
        self.rebuild();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DescramblerSetting::Sequence, Value::Choice(self.sequence)),
            (DescramblerSetting::Poly, Value::I64(self.poly as i64)),
            (DescramblerSetting::Seed, Value::I64(self.seed as i64)),
            (DescramblerSetting::Form, Value::Choice(self.form.index())),
            (
                DescramblerSetting::Output,
                Value::Choice(self.output.index()),
            ),
            (DescramblerSetting::Period, Value::I64(self.period as i64)),
            (DescramblerSetting::Restart, Value::Bool(self.restart)),
        ])
    }
}

sdrkit_core::choice_enum! {
    /// The scrambler that `bits.self_sync` undoes.
    pub enum SelfSyncScrambler {
        /// G3RUH, x^17 + x^12 + 1 ([`seq::g3ruh`]).
        G3ruh = ("g3ruh", "G3RUH (x^17 + x^12 + 1)"),
        /// IEEE 802.11b, 1 + z^-4 + z^-7 ([`seq::ieee80211b`]).
        Ieee80211b = ("ieee80211b", "802.11b (1 + z^-4 + z^-7)"),
        /// The polynomial `poly` in the form `form`.
        Custom = ("custom", "Custom polynomial"),
    }
}

const SCRAMBLER: Setting<field::Choice<SelfSyncScrambler>> =
    Setting::choice("scrambler", "Scrambler", SelfSyncScrambler::G3ruh)
        .help("A named self-synchronising scrambler, or custom for the polynomial below.");
const SELF_SYNC_POLY: Setting<field::I64> = Setting::new("poly", "Polynomial", "", 0x21001)
    .range(2.0, MAX_EXACT, 1.0)
    .help("The register's polynomial in recurrence notation: x^17 + x^12 + 1 is 135169 (0x21001).");

sdrkit_core::settings! {
    /// The settings of [`SelfSyncDescrambler`].
    pub enum SelfSyncDescramblerSetting {
        /// Scrambler.
        Scrambler = SCRAMBLER,
        /// Polynomial.
        Poly = SELF_SYNC_POLY,
        /// Register form.
        Form = FORM,
    }
}

/// `bits.self_sync` and `bits.self_sync_llr`: a self-synchronising (multiplicative) descrambler ([`Multiplicative`]). The
/// register holds received bits, so it locks after as many bits as the polynomial's degree, with no seed and no frame timing.
/// LLRs are flipped where the key is 1, and the register takes their hard decisions.
#[derive(Clone, Debug)]
pub struct SelfSyncDescrambler<T: BitItem> {
    scrambler: SelfSyncScrambler,
    poly: u64,
    form: RegisterForm,
    m: Multiplicative,
    bits: PhantomData<T>,
}

impl<T: BitItem> Default for SelfSyncDescrambler<T> {
    fn default() -> Self {
        let mut d = SelfSyncDescrambler {
            scrambler: SCRAMBLER.default,
            poly: SELF_SYNC_POLY.default as u64,
            form: FORM.default,
            m: seq::g3ruh(0),
            bits: PhantomData,
        };
        d.rebuild();
        d
    }
}

impl<T: BitItem> SelfSyncDescrambler<T> {
    const NAMES: [&'static str; 2] = ["bits.self_sync", "bits.self_sync_llr"];

    fn rebuild(&mut self) {
        self.m = match (self.scrambler, self.form) {
            (SelfSyncScrambler::G3ruh, _) => seq::g3ruh(0),
            (SelfSyncScrambler::Ieee80211b, _) => seq::ieee80211b(0),
            (SelfSyncScrambler::Custom, RegisterForm::Galois) => {
                Multiplicative::galois(self.poly, 0)
            }
            (SelfSyncScrambler::Custom, RegisterForm::Fibonacci) => {
                Multiplicative::fibonacci(self.poly, 0)
            }
        };
    }
    fn run(&mut self, io: &mut Io) {
        let x = T::read(io, 0);
        let out = T::write(io, 0);
        let s = out.len();
        out.extend_from_slice(x);
        self.m.descramble_soft(&mut out[s..]);
    }
}

impl<T: BitItem> Component for SelfSyncDescrambler<T> {
    fn info(&self) -> ComponentInfo {
        let (i, o) = bit_ports::<T>();
        ComponentInfo::processor(Self::NAMES[T::INDEX], i, o)
            .with_settings(SelfSyncDescramblerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rebuild();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SelfSyncDescramblerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SelfSyncDescramblerSetting::Scrambler => self.scrambler = SCRAMBLER.read(id, v)?,
            SelfSyncDescramblerSetting::Poly => self.poly = SELF_SYNC_POLY.read(id, v)? as u64,
            SelfSyncDescramblerSetting::Form => self.form = FORM.read(id, v)?,
        }
        self.rebuild();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                SelfSyncDescramblerSetting::Scrambler,
                Value::Choice(self.scrambler.index()),
            ),
            (
                SelfSyncDescramblerSetting::Poly,
                Value::I64(self.poly as i64),
            ),
            (
                SelfSyncDescramblerSetting::Form,
                Value::Choice(self.form.index()),
            ),
        ])
    }
}

const BYTE_SEQUENCE: CatalogueSetting = CatalogueSetting {
    key: "sequence",
    label: "Sequence",
    help: "The byte sequence XORed onto each packet.",
    choices: || {
        BYTE_SEQUENCES
            .iter()
            .map(|k| ChoiceDecl::new(k, k))
            .collect()
    },
    default: "ccsds",
    advanced: false,
};
const PERIOD: Setting<field::I64> = Setting::new("period", "Period", "packets", 1)
    .range(1.0, 1e6, 1.0)
    .help("Packets per run of the sequence. It starts again with each run.");
const SKIP: Setting<field::I64> = Setting::new("skip", "Skip", "bytes", 0)
    .range(0.0, 65536.0, 1.0)
    .help("Leading bytes of each packet left as they are (a sync byte).");

sdrkit_core::settings! {
    /// The settings of [`ByteDescrambler`].
    pub enum ByteDescramblerSetting {
        /// Sequence.
        Sequence = BYTE_SEQUENCE,
        /// Period.
        Period = PERIOD,
        /// Skip.
        Skip = SKIP,
    }
}

/// `bytes.descramble`: XORs each byte packet with a byte sequence ([`ByteSequence`]: a register read 8 bits per byte, or a
/// published table).
///
/// `sequence` is a name from [`BYTE_SEQUENCES`]. The sequence starts again at the first packet of each run of `period` packets,
/// and a `FrameStart` tag on a packet starts a new run there. The first `skip` bytes of each packet stay as they are: at the
/// first packet of a run the sequence starts after them, and in the other packets it runs on through them unused. This is the
/// DVB rule (EN 300 421 clause 4.4.1: `period` 8, `skip` 1).
#[derive(Debug)]
pub struct ByteDescrambler {
    sequence: u16,
    period: u64,
    skip: usize,
    seq: ByteSequence,
    nth: u64,
    pool: PacketPool,
}

impl Default for ByteDescrambler {
    fn default() -> Self {
        ByteDescrambler {
            sequence: BYTE_SEQUENCES
                .iter()
                .position(|k| *k == BYTE_SEQUENCE.default)
                .expect("the default sequence is in the list") as u16,
            period: PERIOD.default as _,
            skip: SKIP.default as _,
            seq: ByteSequence::table(seq::CCSDS.bytes()),
            nth: 0,
            pool: PacketPool::default(),
        }
    }
}

impl ByteDescrambler {
    fn descramble(&mut self, p: &Packet) -> Packet {
        if self.nth.is_multiple_of(self.period.max(1)) {
            self.seq.reset();
            self.nth = 0;
        }
        let first_of_run = self.nth == 0;
        self.nth += 1;
        let bytes = p.bytes();
        let skip = self.skip.min(bytes.len());
        let seq = &mut self.seq;
        let out = self.pool.make(bytes.len(), p.time, |d| {
            d.copy_from_slice(bytes);
            if !first_of_run {
                seq.advance(skip);
            }
            seq.apply(&mut d[skip..]);
        });
        out.stamped(p.pts, p.dts, p.keyframe)
            .clean_if(p.clean_start)
    }
}

impl Component for ByteDescrambler {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "bytes.descramble",
            vec![Port::bytes("in")],
            vec![Port::bytes("out").same_format_as(0)],
        )
        .with_settings(ByteDescramblerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.seq.reset();
        self.nth = 0;
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        let (first, tags) = (input.first(), input.tags());
        let x = io.bytes(0);
        walk(first, x.len(), tags, |step| match step {
            Step::Items(a, b) => {
                for p in &x[a..b] {
                    let q = self.descramble(p);
                    io.bytes_out(0).push(q);
                }
            }
            Step::Tag(_, t) => {
                if matches!(t.kind, TagKind::FrameStart { .. }) {
                    self.nth = 0;
                }
            }
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ByteDescramblerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ByteDescramblerSetting::Sequence => {
                let i = BYTE_SEQUENCE.read(id, v)?;
                let not_a_choice = SettingError::NotAChoice {
                    id,
                    index: i as u16,
                };
                let key = BYTE_SEQUENCES.get(i).ok_or(not_a_choice.clone())?;
                self.seq = ByteSequence::named(key).ok_or(not_a_choice)?;
                self.sequence = i as u16;
                self.nth = 0;
            }
            ByteDescramblerSetting::Period => {
                self.period = PERIOD.read(id, v)? as u64;
                self.nth = 0;
            }
            ByteDescramblerSetting::Skip => self.skip = SKIP.read(id, v)? as usize,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                ByteDescramblerSetting::Sequence,
                Value::Choice(self.sequence),
            ),
            (
                ByteDescramblerSetting::Period,
                Value::I64(self.period as i64),
            ),
            (ByteDescramblerSetting::Skip, Value::I64(self.skip as i64)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_sequence_name_resolves() {
        for k in BYTE_SEQUENCES {
            assert!(ByteSequence::named(k).is_some(), "{k}");
        }
    }

    #[test]
    fn bit_sequence_names_are_snake_case_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for (k, _) in BIT_SEQUENCES {
            assert!(
                k.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{k}"
            );
            assert!(seen.insert(*k), "{k} twice");
        }
    }
}

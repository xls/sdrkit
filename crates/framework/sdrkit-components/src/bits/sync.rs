//! `bits.sync`: sync-word search on a bit stream.

use super::{bit_ports, parse_bits_text, walk, BitItem, Step};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{ConfigError, Results, Setting, SettingError, Value, Values};
use sdrkit_core::{SettingId, SettingSet};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Every, Format, Io, TagCarry, TagKind,
};
use sdrkit_util_bits::sync::SyncWord;
use std::marker::PhantomData;

/// Sync words found, for `bits.sync`.
pub const SYNC_WORDS: Field<field::I64> = Field::new("sync_words", "Sync words found", "");

const PATTERN: Setting<field::Text> = Setting::new("pattern", "Sync word", "", "1ACFFC1D")
    .help("The word in hex (1ACFFC1D) or binary (0b01111110), first bit most significant.");
const LENGTH: Setting<field::I64> = Setting::new("length", "Length", "bits", 0)
    .range(0.0, 128.0, 1.0)
    .help("Bits in the word. 0 counts the digits given.");
const MAX_ERRORS: Setting<field::I64> = Setting::new("max_errors", "Errors allowed", "bits", 0)
    .range(0.0, 63.0, 1.0)
    .help("Bit errors allowed in a match, less than half the length.");
const INVERTED: Setting<field::Bool> = Setting::new("inverted", "Inverted too", "", false)
    .help("Also find the word with every bit inverted.");
const HOLDOFF: Setting<field::I64> = Setting::new("holdoff", "Hold-off", "bits", 0)
    .range(0.0, 4_294_967_296.0, 1.0)
    .help("Bits after a match during which no new match is taken. 0 always looks.");

sdrkit_core::settings! {
    /// The settings of [`SyncSearch`].
    pub enum SyncSearchSetting {
        /// Sync word.
        Pattern = PATTERN,
        /// Length.
        Length = LENGTH,
        /// Errors allowed.
        MaxErrors = MAX_ERRORS,
        /// Inverted too.
        Inverted = INVERTED,
        /// Hold-off.
        Holdoff = HOLDOFF,
    }
}

/// `bits.sync` and `bits.sync_llr`: looks for a sync word in a bit stream ([`SyncWord`]: one XOR and one population count per bit) and marks the
/// first bit after each one found with a [`TagKind::FrameStart`] tag, which says whether the word was inverted and how many of
/// its bits were wrong. Every bit passes through; after an inverted word, the bits that follow are inverted back (hard bits
/// flipped, LLRs negated) until the next word found. LLRs are searched by their hard decisions.
///
/// The pattern is hex (`"1ACFFC1D"` or `"0x1ACFFC1D"`) or binary (`"0b01111110"`), first bit sent most significant. If `length`
/// is 0, the length is the number of digits given (4 per hex digit, 1 per binary digit). The pattern must fit in the length.
///
/// `FrameStart` tags arriving from upstream are replaced by this component's own; other tags pass through. A `Retuned` or
/// `Discontinuity` tag clears the search and the inversion. Result: `sync_words`, the number of words found.
#[derive(Clone, Debug)]
pub struct SyncSearch<T: BitItem> {
    pattern_text: String,
    pattern: u128,
    digits: u32,
    length: u32,
    max_errors: u32,
    inverted_ok: bool,
    holdoff: u64,
    word: SyncWord,
    invert: bool,
    quiet: u64,
    found: u64,
    carry: TagCarry,
    every: Every,
    published: Published,
    bits: PhantomData<T>,
}

impl<T: BitItem> Default for SyncSearch<T> {
    fn default() -> Self {
        // The CCSDS attached sync marker.
        let pattern = 0x1ACF_FC1D;
        SyncSearch {
            pattern_text: "1ACFFC1D".into(),
            pattern,
            digits: 32,
            length: 0,
            max_errors: 0,
            inverted_ok: false,
            holdoff: 0,
            word: SyncWord::new(pattern, 32, 0)
                .unwrap_or_else(|_| unreachable!("a 32-bit word with no errors is valid")),
            invert: false,
            quiet: 0,
            found: 0,
            carry: TagCarry::same_rate(),
            every: Every::default(),
            published: Published::new(&[&SYNC_WORDS]),
            bits: PhantomData,
        }
    }
}

impl<T: BitItem> SyncSearch<T> {
    const NAMES: [&'static str; 2] = ["bits.sync", "bits.sync_llr"];

    /// The matcher for these settings, or `None` when they do not make one.
    fn matcher(
        pattern: u128,
        digits: u32,
        length: u32,
        max_errors: u32,
        inverted: bool,
    ) -> Option<SyncWord> {
        let len = if length == 0 { digits } else { length };
        if len == 0 || len > 128 || (len < 128 && pattern >> len != 0) {
            return None;
        }
        SyncWord::new(pattern, len, max_errors)
            .ok()
            .map(|w| w.with_inverted(inverted))
    }
    fn clear(&mut self) {
        self.word.reset();
        self.invert = false;
        self.quiet = 0;
    }
    /// Searches `x` (already appended to the output as `y`), flipping inverted runs; queues a tag for each word found. `base` is
    /// the absolute index of `x[0]`.
    fn search(&mut self, x: &[T], y: &mut [T], base: u64) {
        for (k, (v, o)) in x.iter().zip(y.iter_mut()).enumerate() {
            if self.invert {
                *o = v.flip();
            }
            let hit = self.word.push(v.hard());
            if self.quiet > 0 {
                self.quiet -= 1;
                continue;
            }
            if let Some(h) = hit {
                self.invert = h.inverted();
                self.quiet = self.holdoff;
                self.found += 1;
                let kind = TagKind::FrameStart {
                    inverted: h.inverted(),
                    errors: h.errors(),
                };
                self.carry.queue(base + k as u64 + 1, kind);
            }
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
        walk(first, x.len(), tags, |step| match step {
            Step::Items(a, b) => self.search(&x[a..b], &mut y[a..b], first + a as u64),
            Step::Tag(at, t) => match t.kind {
                TagKind::FrameStart { .. } => {}
                TagKind::Retuned { .. } | TagKind::Discontinuity { .. } => {
                    self.clear();
                    self.carry.queue(first + at as u64, t.kind.clone());
                }
                _ => self.carry.queue(first + at as u64, t.kind.clone()),
            },
        });
        self.carry.flush(io.output(0));
        if self.every.tick(x.len()) {
            self.published.set(&SYNC_WORDS, self.found as i64);
        }
    }
}

impl<T: BitItem> Component for SyncSearch<T> {
    fn info(&self) -> ComponentInfo {
        let (i, o) = bit_ports::<T>();
        ComponentInfo::processor(Self::NAMES[T::INDEX], i, o)
            .manual_tags()
            .with_settings(SyncSearchSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.clear();
        self.found = 0;
        self.carry.reset(1.0, 1.0);
        self.every = Every::new(inputs[0].rate, 0.25);
        self.published.set(&SYNC_WORDS, 0);
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let setting = SyncSearchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        let (mut pattern, mut digits, mut length, mut max_errors, mut inverted) = (
            self.pattern,
            self.digits,
            self.length,
            self.max_errors,
            self.inverted_ok,
        );
        let mut text = None;
        match setting {
            SyncSearchSetting::Pattern => {
                let t = PATTERN.read(id, v)?.trim();
                (pattern, digits) =
                    parse_bits_text(t).ok_or(SettingError::Malformed { id, at: None })?;
                text = Some(t);
            }
            SyncSearchSetting::Length => length = LENGTH.read(id, v)? as u32,
            SyncSearchSetting::MaxErrors => max_errors = MAX_ERRORS.read(id, v)? as u32,
            SyncSearchSetting::Inverted => inverted = INVERTED.read(id, v)?,
            SyncSearchSetting::Holdoff => {
                self.holdoff = HOLDOFF.read(id, v)? as u64;
                return Ok(Applied::Live);
            }
        }
        let with = match setting {
            SyncSearchSetting::Length => SyncSearchSetting::Pattern,
            _ => SyncSearchSetting::Length,
        };
        self.word = Self::matcher(pattern, digits, length, max_errors, inverted).ok_or(
            SettingError::Conflict {
                id,
                with: with.id(),
            },
        )?;
        (
            self.pattern,
            self.digits,
            self.length,
            self.max_errors,
            self.inverted_ok,
        ) = (pattern, digits, length, max_errors, inverted);
        if let Some(t) = text {
            t.clone_into(&mut self.pattern_text);
        }
        self.clear();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                SyncSearchSetting::Pattern,
                Value::Text(self.pattern_text.clone()),
            ),
            (
                SyncSearchSetting::Length,
                Value::I64(i64::from(self.length)),
            ),
            (
                SyncSearchSetting::MaxErrors,
                Value::I64(i64::from(self.max_errors)),
            ),
            (SyncSearchSetting::Inverted, Value::Bool(self.inverted_ok)),
            (SyncSearchSetting::Holdoff, Value::I64(self.holdoff as i64)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

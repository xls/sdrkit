//! `modem.sync_search` and `modem.sync_search_symbols`: a sync word found in soft bits or soft symbols, marked with a
//! `FrameStart` tag.

use sdrkit_algo_modem::correlate::SoftCorrelator;
use sdrkit_core::publish::{Field, Published};
use sdrkit_core::{Complex32, ConfigError, Setting, SettingError, Value, Values};

use sdrkit_core::publish::field;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Kind, Port, TagCarry, TagKind};
use sdrkit_util_bits::sync::{SoftSync, SyncWord};
use sdrkit_util_bits::Soft;
use std::collections::VecDeque;

/// Sync words found since the start.
const FOUND: Field<sdrkit_core::publish::field::I64> =
    Field::new("sync_found", "Sync words found", "");

/// A sync word as text: binary digits (`"1110 1011"`) or hexadecimal with `0x` and a length of four bits per digit
/// (`"0x1ACFFC1D"`). Returns the word (first bit most significant) and its length; `None` for anything else or more than 128
/// bits.
fn parse_word(s: &str) -> Option<(u128, u32)> {
    let t: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    let (digits, radix, bits) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => (h, 16, 4),
        None => (t.strip_prefix("0b").unwrap_or(&t), 2, 1),
    };
    let len = digits.len() as u32 * bits;
    if digits.is_empty() || len > 128 {
        return None;
    }
    let w = u128::from_str_radix(digits, radix).ok()?;
    Some((w, len))
}

sdrkit_core::choice_enum! {
    /// Which forms of the word [`SyncSearch`] counts.
    pub enum Polarity {
        /// As written.
        Normal = ("normal", "As written"),
        /// Inverted only.
        Inverted = ("inverted", "Inverted only"),
        /// Either.
        Both = ("both", "Either"),
    }
}

/// Symbols held back so a correlation peak is final before the symbols after the word leave.
const DELAY: u64 = 4;

/// The search at work.
#[derive(Debug)]
enum Engine {
    Idle,
    Bits {
        hard: SyncWord,
        soft: SoftSync,
    },
    Symbols {
        corr: Box<SoftCorrelator>,
        /// The newest symbols, the last ending at absolute input index `end`.
        hist: VecDeque<Complex32>,
        end: u64,
        /// Absolute input index of the correlator's first value.
        origin: u64,
        next_out: u64,
        /// Pending polarity changes: from this input index on, inverted or not.
        switches: VecDeque<(u64, bool)>,
    },
}

const WORD: Setting<field::Text> = Setting::new("word", "Sync word", "", "").help("Binary digits (1110 1011) or hexadecimal with 0x (0x1ACFFC1D, four bits a digit); first bit first; up to 128 bits.");
const MAX_ERRORS: Setting<field::I64> = Setting::new("max_errors", "Allowed bit errors", "", 0)
    .range(0.0, 63.0, 1.0)
    .help("Bits of the word that may differ (fewer than half the word).");
const POLARITY: Setting<field::Choice<Polarity>> =
    Setting::choice("polarity", "Polarity", Polarity::Normal);
const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "Correlation threshold", "", 0.5)
    .range(0.0, 1.0, 0.0)
    .help("The normalised soft correlation a match must reach (0 to 1).");

sdrkit_core::settings! {
    /// The settings of [`SyncSearch`].
    pub enum SyncSearchSetting {
        /// Sync word.
        Word = WORD,
        /// Allowed bit errors.
        MaxErrors = MAX_ERRORS,
        /// Polarity.
        Polarity = POLARITY,
        /// Correlation threshold.
        Threshold = THRESHOLD,
    }
}

/// `modem.sync_search` and `modem.sync_search_symbols`: finds a sync word in soft bits (`Llr`) or soft symbols (`Symbols`, the real part is the level: BPSK, or
/// 2-level FSK after a slicer) and passes the stream on unchanged except that the items after an inverted match are inverted back.
///
/// - **Soft bits**: the hard decisions must match `word` within `max_errors` bits (`sdrkit_util_bits::sync::SyncWord`), and the soft
///   correlation (the LLRs signed by the word over their magnitudes, `sdrkit_util_bits::sync::SoftSync`) must reach `threshold`.
/// - **Soft symbols**: the Pearson correlation of the levels with the word (bit 0 is +1, bit 1 is -1; gain and offset ignored,
///   `sdrkit_algo_modem::correlate::SoftCorrelator`, the largest within a symbol) must reach `threshold`, and the decisions about the
///   fitted centre must match within `max_errors`. Symbols leave 4 symbols late, so a peak is final before the symbols after it.
///
/// `polarity` says whether the word counts as written, inverted (all its bits flipped), or both. A match puts a `FrameStart` tag
/// (`inverted`, `errors`: the differing bits) on the first item after the word; after an inverted match the following items are
/// negated (LLRs and symbols), so downstream sees the true bits, until the next match. A `Retuned` or `Discontinuity` tag clears
/// the search and the inversion. Hard-bit streams use `bits.sync`. Publishes `sync_found`.
#[derive(Debug)]
pub struct SyncSearch {
    on_symbols: bool,
    word: String,
    max_errors: u32,
    polarity: Polarity,
    threshold: f64,
    pat: u128,
    len: u32,
    engine: Engine,
    invert: bool,
    carry: TagCarry,
    found: i64,
    published: i64,
    results: Published,
}

impl Default for SyncSearch {
    fn default() -> Self {
        SyncSearch {
            on_symbols: false,
            word: String::new(),
            max_errors: MAX_ERRORS.default as _,
            polarity: Polarity::Normal,
            threshold: THRESHOLD.default,
            pat: 0,
            len: 0,
            engine: Engine::Idle,
            invert: false,
            carry: TagCarry::same_rate(),
            found: 0,
            published: -1,
            results: Published::new(&[&FOUND]),
        }
    }
}

impl SyncSearch {
    fn build(&self, symbols: bool) -> Result<Engine, ConfigError> {
        let (pat, len) = (self.pat, self.len);
        let inv = self.polarity != Polarity::Normal;
        if symbols {
            let levels: Vec<f32> = (0..len)
                .map(|k| 1.0 - 2.0 * ((pat >> (len - 1 - k)) & 1) as f32)
                .collect();
            let corr = SoftCorrelator::new(&levels, 1.0, self.threshold as f32)
                .map_err(ConfigError::from)?
                .with_inverted(inv);
            let cap = len as usize + DELAY as usize + 4;
            Ok(Engine::Symbols {
                corr: Box::new(corr),
                hist: VecDeque::with_capacity(cap),
                end: 0,
                origin: 0,
                next_out: 0,
                switches: VecDeque::with_capacity(16),
            })
        } else {
            Ok(Engine::Bits {
                hard: SyncWord::new(pat, len, self.max_errors)
                    .map_err(crate::stage)?
                    .with_inverted(inv),
                soft: SoftSync::new(pat, len, self.threshold as f32)
                    .map_err(crate::stage)?
                    .with_inverted(inv),
            })
        }
    }

    /// Forgets the search at absolute input index `at`.
    fn reset(&mut self, at: u64) {
        self.invert = false;
        match &mut self.engine {
            Engine::Idle => {}
            Engine::Bits { hard, soft } => {
                hard.reset();
                soft.reset();
            }
            Engine::Symbols {
                corr,
                origin,
                switches,
                ..
            } => {
                corr.reset();
                *origin = at;
                switches.clear();
                // The symbols already held still leave, at the polarity they had.
                switches.push_back((at, false));
            }
        }
    }

    fn bits(&mut self, io: &mut Io, a: usize, b: usize) {
        let x = io.llrs(0);
        let (input, output) = io.pair(0, 0);
        let Engine::Bits { hard, soft } = &mut self.engine else {
            return;
        };
        for (i, &l) in x[a..b].iter().enumerate() {
            let h = hard.push(l.hard());
            let s = soft.push(l);
            out_push_llr(output, if self.invert { l.flip() } else { l });
            if let (Some(h), Some(s)) = (h, s) {
                let inverted = h.inverted();
                // Both tests must agree on the polarity.
                if inverted == (s.score < 0.0) && polarity_ok(self.polarity, inverted) {
                    self.invert = inverted;
                    self.found += 1;
                    let at = self
                        .carry
                        .map(input, output, input.first() + (a + i + 1) as u64);
                    self.carry.queue(
                        at,
                        TagKind::FrameStart {
                            inverted,
                            errors: h.errors(),
                        },
                    );
                }
            }
        }
    }

    fn symbols(&mut self, io: &mut Io, a: usize, b: usize) {
        let x = io.samples::<Complex32>(0);
        let (input, output) = io.pair(0, 0);
        let first = input.first();
        let (pat, len, max_errors, polarity) = (self.pat, self.len, self.max_errors, self.polarity);
        let Engine::Symbols {
            corr,
            hist,
            end,
            origin,
            next_out,
            switches,
        } = &mut self.engine
        else {
            return;
        };
        if hist.is_empty() && *end == 0 {
            (*end, *origin, *next_out) = (first + a as u64, first + a as u64, first + a as u64);
        }
        for &v in &x[a..b] {
            if hist.len() == hist.capacity() {
                hist.pop_front();
            }
            hist.push_back(v);
            *end += 1;
            if let Some(p) = corr.push(v.re) {
                let s = *origin + p.index.round().max(0.0) as u64;
                let inverted = p.score < 0.0;
                // Decisions about the fitted centre, against the word as matched.
                let base = *end - hist.len() as u64;
                if s >= base && s + len as u64 <= *end && polarity_ok(polarity, inverted) {
                    let mut errors = 0;
                    for k in 0..len as u64 {
                        let bit = (hist[(s + k - base) as usize].re < p.offset) as u32;
                        let want = ((pat >> (len as u64 - 1 - k)) & 1) as u32 ^ inverted as u32;
                        errors += bit ^ want;
                    }
                    if errors <= max_errors {
                        let frame = s + len as u64;
                        switches.push_back((frame, inverted));
                        self.found += 1;
                        let at = self.carry.map(input, output, frame);
                        self.carry
                            .queue(at, TagKind::FrameStart { inverted, errors });
                    }
                }
            }
            // Symbols leave once the peak of a word ending just before them is final.
            while *next_out + DELAY < *end {
                while switches.front().is_some_and(|w| w.0 <= *next_out) {
                    if let Some((_, inv)) = switches.pop_front() {
                        self.invert = inv;
                    }
                }
                let base = *end - hist.len() as u64;
                let z = if *next_out >= base {
                    hist[(*next_out - base) as usize]
                } else {
                    Complex32::default()
                };
                out_push_sym(output, if self.invert { -z } else { z });
                *next_out += 1;
            }
        }
    }
}

fn polarity_ok(p: Polarity, inverted: bool) -> bool {
    match p {
        Polarity::Normal => !inverted,
        Polarity::Inverted => inverted,
        Polarity::Both => true,
    }
}

fn out_push_llr(b: &mut sdrkit_graph::Buffer, l: i8) {
    if let sdrkit_graph::stream::Data::Llr(v) = &mut b.data {
        v.push(l);
    }
}

fn out_push_sym(b: &mut sdrkit_graph::Buffer, z: Complex32) {
    if let sdrkit_graph::stream::Data::Symbols(v) = &mut b.data {
        v.push(z);
    }
}

impl SyncSearch {
    /// `modem.sync_search_symbols`: the search on soft symbols.
    pub fn on_symbols() -> Self {
        SyncSearch {
            on_symbols: true,
            ..Self::default()
        }
    }
}

impl Component for SyncSearch {
    fn info(&self) -> ComponentInfo {
        let (name, kind) = if self.on_symbols {
            ("modem.sync_search_symbols", Kind::Symbols)
        } else {
            ("modem.sync_search", Kind::Llr)
        };
        ComponentInfo::processor(
            name,
            vec![Port::new("in", kind)],
            vec![Port::new("out", kind).same_format_as(0)],
        )
        .with_settings(SyncSearchSetting::schema())
        .manual_tags()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let word = SyncSearchSetting::Word.id();
        let (pat, len) = parse_word(&self.word).ok_or(SettingError::Required { id: word })?;
        if 2 * self.max_errors >= len {
            return Err(SettingError::Conflict {
                id: SyncSearchSetting::MaxErrors.id(),
                with: word,
            }
            .into());
        }
        (self.pat, self.len) = (pat, len);
        let symbols = self.on_symbols;
        if symbols && len < 2 {
            return Err(SettingError::NotPermitted { id: word }.into());
        }
        self.engine = self.build(symbols)?;
        self.invert = false;
        self.carry = TagCarry::same_rate();
        Ok(vec![inputs[0].clone()])
    }

    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        let input = io.input(0);
        let first = input.first();
        let symbols = matches!(self.engine, Engine::Symbols { .. });
        let mut at = 0usize;
        for t in input.tags() {
            if matches!(
                t.kind,
                TagKind::Retuned { .. } | TagKind::Discontinuity { .. }
            ) {
                let p = (t.at.saturating_sub(first) as usize).min(n);
                if p > at {
                    if symbols {
                        self.symbols(io, at, p);
                    } else {
                        self.bits(io, at, p);
                    }
                    at = p;
                }
                self.reset(first + p as u64);
            }
        }
        if n > at {
            if symbols {
                self.symbols(io, at, n);
            } else {
                self.bits(io, at, n);
            }
        }
        let (input, output) = io.pair(0, 0);
        self.carry.forward(input, output);
        if self.found != self.published {
            self.published = self.found;
            self.results.set(&FOUND, self.found);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SyncSearchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SyncSearchSetting::Word => {
                let w = WORD.read(id, v)?;
                if parse_word(w).is_none() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.word = w.to_owned();
            }
            SyncSearchSetting::MaxErrors => self.max_errors = MAX_ERRORS.read(id, v)? as u32,
            SyncSearchSetting::Polarity => self.polarity = POLARITY.read(id, v)?,
            SyncSearchSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (SyncSearchSetting::Word, Value::Text(self.word.clone())),
            (
                SyncSearchSetting::MaxErrors,
                Value::I64(self.max_errors as i64),
            ),
            (
                SyncSearchSetting::Polarity,
                Value::Choice(self.polarity.index()),
            ),
            (SyncSearchSetting::Threshold, Value::F64(self.threshold)),
        ])
    }

    fn results(&self) -> Option<&sdrkit_core::Results> {
        Some(self.results.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_parse_in_binary_and_hex() {
        assert_eq!(parse_word("1110 1011"), Some((0b1110_1011, 8)));
        assert_eq!(parse_word("0x1ACFFC1D"), Some((0x1ACF_FC1D, 32)));
        assert_eq!(parse_word("0b101"), Some((0b101, 3)));
        assert_eq!(parse_word("10201"), None);
        assert_eq!(parse_word(""), None);
        assert_eq!(parse_word(&"1".repeat(129)), None);
        assert_eq!(parse_word("0x"), None);
    }
}

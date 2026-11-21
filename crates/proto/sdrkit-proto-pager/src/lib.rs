//! POCSAG and FLEX paging decoders and encoders, from the code word to the page.
//!
//! The decoders take hard bits (`u8`, 0 or 1) or soft bits (`f32` LLRs, positive means 0) and return pages. Symbol timing and
//! FSK slicing are not part of this crate.
//!
//! Standards: ITU-R M.584-2, ARIB RCR STD-43A (FLEX-TD).
//!
//! # Modules
//!
//! | Module | What it holds |
//! |---|---|
//! | [`word`] | the 32-bit code word of both codes: BCH(31,21) plus even parity, hard repair, soft (Chase) repair |
//! | [`vote`] | one code word from several received copies of it (multi-copy voting) |
//! | [`charset`] | the POCSAG numeric sets, the FLEX numeric set, and 7-bit characters packed first bit least significant |
//! | [`page`] | the [`Page`] that both codes produce, the text readings, and the choice between numeric and text |
//! | [`pocsag`] | POCSAG at 512, 1200 and 2400 bit/s: words, the streaming batch reader, page assembly with voting, the encoder |
//! | [`flex`] | FLEX at 1600/2, 3200/2, 3200/4 and 6400/4: sync, frame information, block information, addresses, vectors, phases, page assembly, fragments, the encoder |
//!
//! A code word is a `u32` whose most significant bit is the first bit sent. Reliabilities are `|LLR|` per bit, with index 0 the
//! first bit sent. Each decoder keeps one [`Page`] and the caller reads it as `&Page`. Bad input returns an [`Error`].
//!
//! # Example
//!
//! ```
//! use sdrkit_proto_pager::pocsag::{self, Content, PocsagDecoder, PocsagTx};
//!
//! let tx = [PocsagTx { ric: 1_234_567, function: 3, content: Content::Alpha("Hello") }];
//! let mut words = Vec::new();
//! pocsag::encode_batches(&tx, pocsag::NumericSet::Itu, &mut words).unwrap();
//! let mut bits = Vec::new();
//! pocsag::transmission_bits(&words, pocsag::PREAMBLE_BITS, &mut bits);
//!
//! let mut rx = PocsagDecoder::new(pocsag::Config::default()).unwrap();
//! let mut texts = Vec::new();
//! for &b in &bits {
//!     rx.push_bit(b, 0.0, |p| texts.push(p.text.clone()));
//! }
//! rx.finish(0.0, |p| texts.push(p.text.clone()));
//! assert_eq!(texts, ["Hello"]);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod charset;
pub mod flex;
pub mod page;
pub mod pocsag;
pub mod vote;
pub mod word;

pub use page::{Kind, Page, Protocol, ReadMode, Refusals};

/// A setting that was out of its range. [`Error::BadSetting`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Setting {
    /// FLEX sync 1 errors must be 0 to 39.
    FlexSyncErrors,
    /// FLEX sync 1 score must be 0 to 1.
    FlexSyncScore,
    /// Phases kept and fragment slots must be 0 to 1024.
    FlexPhasesKept,
    /// Ages and soft limits must be finite numbers of 0 or more.
    AgeAndSoftLimits,
    /// POCSAG sync errors must be 0 to 15.
    PocsagSyncErrors,
    /// First batch valid words must be 0 to 16.
    FirstBatchValidWords,
    /// POCSAG soft sync score must be 0 to 1.
    PocsagSoftSyncScore,
    /// Longest message must be 1 to 4096 words.
    LongestMessage,
    /// Messages kept for voting must be 0 to 1024.
    MessagesKept,
}

impl Setting {
    /// The refusal in a plain sentence.
    pub fn describe(self) -> &'static str {
        match self {
            Setting::FlexSyncErrors => "FLEX sync 1 errors must be 0 to 39",
            Setting::FlexSyncScore => "FLEX sync 1 score must be 0 to 1",
            Setting::FlexPhasesKept => "phases kept and fragment slots must be 0 to 1024",
            Setting::AgeAndSoftLimits => "ages and soft limits must be finite numbers of 0 or more",
            Setting::PocsagSyncErrors => "POCSAG sync errors must be 0 to 15",
            Setting::FirstBatchValidWords => "first batch valid words must be 0 to 16",
            Setting::PocsagSoftSyncScore => "POCSAG soft sync score must be 0 to 1",
            Setting::LongestMessage => "longest message must be 1 to 4096 words",
            Setting::MessagesKept => "messages kept for voting must be 0 to 1024",
        }
    }
}

/// An error from this crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A POCSAG RIC is 21 bits. The value is larger.
    RicTooLarge(u32),
    /// A POCSAG function code is 0 to 3. The value is larger.
    BadFunction(u8),
    /// The character is not in the character set of the message.
    BadCharacter(char),
    /// The FLEX capcode is not a short or long address that this encoder can send.
    BadCapcode(u32),
    /// The pages do not fit in one FLEX phase (88 words).
    PhaseFull,
    /// The FLEX encoder sends tone, numeric and text pages only.
    BadKind(crate::page::Kind),
    /// A setting is out of its range.
    BadSetting(Setting),
    /// A slice has the wrong length. The value is the length that is necessary.
    BadLength(usize),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::RicTooLarge(r) => write!(f, "RIC {r} is larger than 21 bits"),
            Error::BadFunction(v) => write!(f, "function code {v} is not 0 to 3"),
            Error::BadCharacter(c) => write!(f, "character {c:?} is not in the character set"),
            Error::BadCapcode(c) => write!(f, "capcode {c} is not a short or long FLEX address"),
            Error::PhaseFull => write!(f, "the pages do not fit in one FLEX phase"),
            Error::BadKind(k) => write!(f, "the FLEX encoder does not send {} pages", k.name()),
            Error::BadSetting(s) => write!(f, "refused: {}", s.describe()),
            Error::BadLength(n) => write!(f, "the slice must hold {n} items"),
        }
    }
}

impl std::error::Error for Error {}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;

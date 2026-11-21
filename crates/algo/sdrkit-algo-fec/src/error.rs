//! The result and error types every decoder in this crate returns.

/// What a successful decode reports, enough for the health and bit-error counters every stack keeps.
///
/// - `corrected`: how many code symbols the decoder changed. For bit decoders (Viterbi, LDPC, turbo, polar, BCH, short codes) it is
///   the number of received hard decisions (non-zero LLRs only) that disagree with the decoded code word; for Reed-Solomon it is
///   the number of symbols corrected (errors and erasures).
/// - `metric`: a decoder-specific quality figure, documented on each `decode` (for Viterbi the growth of the best path metric, for
///   Reed-Solomon the number of erasures used, for the iterative decoders the final number of unsatisfied checks, normally 0).
/// - `iterations`: iterations used by an iterative decoder (LDPC, turbo); 1 for the others.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Decoded {
    /// Code symbols changed by the decoder.
    pub corrected: u32,
    /// Decoder-specific quality figure (see the type documentation).
    pub metric: i32,
    /// Iterations used (1 for non-iterative decoders).
    pub iterations: u32,
}

impl Decoded {
    /// A result with `corrected` changes, a metric of 0 and one iteration.
    pub const fn corrected(corrected: u32) -> Self {
        Decoded {
            corrected,
            metric: 0,
            iterations: 1,
        }
    }
}

/// Declares [`Param`] and its plain-word names from one list.
macro_rules! params {
    ($($v:ident = $name:literal,)+) => {
        /// A parameter of a code or a decoder. [`FecError`] names it.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Param {
            $(#[doc = concat!("The ", $name, ".")] $v,)+
        }

        impl Param {
            /// The name of the parameter in plain words.
            pub fn name(self) -> &'static str {
                match self {
                    $(Param::$v => $name,)+
                }
            }
        }
    };
}

params! {
BaseGraph = "base graph",
    Basis = "symbol basis",
    Bits = "number of bits",
    BufferLen = "circular buffer length",
    BurstLen = "burst length",
    Checks = "parity checks",
    ChunkLen = "chunk length",
    Coefficient = "check coefficient",
    Column = "column",
    Columns = "number of columns",
    Combine = "combine factor",
    ConstraintLength = "constraint length",
    Cycles = "cycles per bit",
    Decoders = "decoder list",
    Delta = "threshold step",
    Depth = "depth",
    DifferenceSet = "difference set",
    E = "number of sent bits",
    Encoder = "encoder",
    Filler = "number of filler bits",
    FrameLen = "frame length",
    Gap = "gap",
    Generator = "generator polynomial",
    Generators = "number of generators",
    Info = "information positions",
    Inputs = "number of inputs",
    K = "number of information symbols",
    Kernel = "decoder kernel",
    Labels = "number of labels",
    Lane = "masked lane",
    Layout = "voice layout",
    Limit = "quantizer limit",
    List = "list size",
    M = "symbol size in bits",
    N = "code length",
    NMax = "largest code length exponent",
    Parity = "parity size",
    Permutation = "permutation",
    Polynomial = "field polynomial",
    Prim = "root step",
    Probabilities = "symbol probabilities",
    Puncturing = "puncturing pattern",
    Q = "field size",
    Rate = "code rate",
    Repeats = "repeat counts",
    Rows = "table rows",
    Rv = "redundancy version",
    Scale = "scale",
    Shift = "shift",
    Shortening = "shortening",
    StartState = "start state",
    States = "number of states",
    StreamLen = "stream length",
    Symbol = "symbol",
    T = "number of correctable errors",
    Table = "table",
    TailBiting = "tail-biting decoder",
    Weights = "weights",
    Word = "code word",
    WordLen = "word length",
    Words = "number of code words",
    Z = "lifting size",
}

/// What is wrong in a line of a code text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextFault {
    /// A word is not a number.
    NotANumber,
    /// A number is too large for its field.
    TooLarge,
    /// The header does not hold `q`, `n` and `k`.
    ShortHeader,
    /// The header holds more values than `q n k [poly]`.
    ExtraValues,
    /// A field above 256 symbols needs a field polynomial in the header.
    NoFieldPolynomial,
    /// There is more than one info line, or the info line comes after a check.
    InfoOrder,
    /// The info line does not start with the word `info`.
    BadInfo,
    /// The info line does not list `k` positions.
    InfoCount,
    /// A check entry is not `symbol:coefficient`.
    BadCheckEntry,
    /// The text has no header.
    NoHeader,
    /// The text has no checks.
    NoChecks,
}

impl std::fmt::Display for TextFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TextFault::NotANumber => "a word is not a number",
            TextFault::TooLarge => "a number is too large",
            TextFault::ShortHeader => "the header needs q, n and k",
            TextFault::ExtraValues => "the header has extra values",
            TextFault::NoFieldPolynomial => "a field above 256 symbols needs a field polynomial",
            TextFault::InfoOrder => "there must be one info line, before the checks",
            TextFault::BadInfo => "the info line is not valid",
            TextFault::InfoCount => "the info line must list k positions",
            TextFault::BadCheckEntry => "a check entry is not symbol:coefficient",
            TextFault::NoHeader => "the text has no header",
            TextFault::NoChecks => "the text has no checks",
        })
    }
}

/// Why a call failed. No function in this crate panics on bad input. It returns one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FecError {
    /// The input does not have a length the code allows. `expected` is the length (or the multiple) that was needed.
    InputLength {
        /// The length needed (or, for streams, the multiple the length must be).
        expected: usize,
        /// The length given.
        got: usize,
    },
    /// The input is longer than the decoder was sized for at construction.
    TooLong {
        /// The most the decoder accepts.
        max: usize,
    },
    /// An output slice is too short: `need` items were needed.
    OutputTooShort {
        /// Items needed.
        need: usize,
    },
    /// An erasure position is outside the code word, or listed twice.
    BadErasure {
        /// The position given.
        at: usize,
    },
    /// The input holds a NaN or infinite soft value at this index.
    NotFinite {
        /// Index of the first bad value.
        at: usize,
    },
    /// The word could not be decoded: too many errors, no code word found, or a check (CRC, re-check) failed. The input is left
    /// as it was.
    Uncorrectable,
    /// A parameter is outside the range the code can use.
    OutOfRange {
        /// The parameter.
        param: Param,
        /// The lowest value the code can use.
        min: i64,
        /// The highest value the code can use.
        max: i64,
        /// The value given.
        got: i64,
    },
    /// A parameter has a value that the code does not have (a size with no table, a value that is not a power of two).
    NotAllowed {
        /// The parameter.
        param: Param,
        /// The value given.
        got: i64,
    },
    /// Two parameters do not fit together.
    Conflict {
        /// The parameter.
        param: Param,
        /// The parameter it does not fit.
        with: Param,
    },
    /// The values give no usable code (a polynomial that is not primitive, a singular parity part, a state that cannot be
    /// reached).
    Degenerate {
        /// The parameter.
        param: Param,
    },
    /// The code or this machine does not have the feature that the call needs (an encoder, a kernel, tail-biting).
    Unsupported {
        /// The feature.
        param: Param,
    },
    /// A line of a code text cannot be read.
    CodeText {
        /// The line, from 1.
        line: usize,
        /// What is wrong.
        fault: TextFault,
    },
    /// An alist text cannot be read.
    Alist(crate::ldpc::alist::AlistError),
    /// A worker thread of a parallel decoder stopped without a result.
    WorkerFailed,
    /// A bit operation refused its input.
    Bits(sdrkit_util_bits::Error),
}

impl std::fmt::Display for FecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            FecError::InputLength { expected, got } => {
                write!(
                    f,
                    "input length {got} does not fit the code (need {expected})"
                )
            }
            FecError::TooLong { max } => {
                write!(f, "input longer than the decoder's maximum of {max}")
            }
            FecError::OutputTooShort { need } => write!(f, "output too short: {need} items needed"),
            FecError::BadErasure { at } => write!(f, "bad erasure position {at}"),
            FecError::NotFinite { at } => write!(f, "soft value at index {at} is not finite"),
            FecError::Uncorrectable => write!(f, "uncorrectable"),
            FecError::OutOfRange {
                param,
                min,
                max,
                got,
            } => write!(f, "the {} is {got}, outside {min} to {max}", param.name()),
            FecError::NotAllowed { param, got } => {
                write!(f, "the {} cannot be {got}", param.name())
            }
            FecError::Conflict { param, with } => {
                write!(f, "the {} does not fit the {}", param.name(), with.name())
            }
            FecError::Degenerate { param } => {
                write!(f, "the {} gives no usable code", param.name())
            }
            FecError::Unsupported { param } => write!(f, "there is no {} for this", param.name()),
            FecError::CodeText { line, fault } => write!(f, "code text line {line}: {fault}"),
            FecError::Alist(e) => e.fmt(f),
            FecError::WorkerFailed => write!(f, "a decoder thread stopped without a result"),
            FecError::Bits(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for FecError {}

impl From<sdrkit_util_bits::Error> for FecError {
    fn from(e: sdrkit_util_bits::Error) -> Self {
        match e {
            sdrkit_util_bits::Error::OutputTooShort { need } => FecError::OutputTooShort { need },
            e => FecError::Bits(e),
        }
    }
}

impl Param {
    /// `Err(OutOfRange)`.
    pub(crate) fn range<T>(self, min: i64, max: i64, got: impl TryInto<i64>) -> Result<T> {
        Err(FecError::OutOfRange {
            param: self,
            min,
            max,
            got: got.try_into().unwrap_or(i64::MAX),
        })
    }

    /// `Ok(())` if `n` is from `min` to `max`.
    pub(crate) fn check(self, n: impl TryInto<i64> + Copy, min: i64, max: i64) -> Result<()> {
        match n.try_into() {
            Ok(v) if (min..=max).contains(&v) => Ok(()),
            _ => self.range(min, max, n),
        }
    }

    /// `Err(NotAllowed)`.
    pub(crate) fn not_allowed<T>(self, got: impl TryInto<i64>) -> Result<T> {
        Err(FecError::NotAllowed {
            param: self,
            got: got.try_into().unwrap_or(i64::MAX),
        })
    }

    /// `Err(Conflict)`.
    pub(crate) const fn conflict<T>(self, with: Param) -> Result<T> {
        Err(FecError::Conflict { param: self, with })
    }

    /// `Err(Degenerate)`.
    pub(crate) const fn degenerate<T>(self) -> Result<T> {
        Err(FecError::Degenerate { param: self })
    }

    /// `Err(Unsupported)`.
    pub(crate) const fn unsupported<T>(self) -> Result<T> {
        Err(FecError::Unsupported { param: self })
    }
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, FecError>;

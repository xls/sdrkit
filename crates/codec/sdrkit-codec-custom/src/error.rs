//! The error type of the crate.

/// The setting or parameter a call refused. [`Error`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// A synthetic video, frame or audio rate is not positive and finite, or the key interval is 0.
    SyntheticRates,
    /// A picture side is outside 1 to 8191.
    PictureSize,
    /// The picture level count is outside 1 to 8.
    Levels,
    /// The picture top plane is outside -1 to 14.
    TopPlane,
    /// The voice codec 4 budget per 20 ms is outside 64 to 400 bits.
    Budget,
    /// A voice codec 4 budget below 72 bits per 20 ms needs 40 ms frames.
    BudgetFrameLength,
    /// No voice codec 4 tier fits the budget.
    Tier,
    /// The voice codec 4 budget leaves too few bits for the pulses.
    PulseBits,
    /// A voice codec 4 pulse index needs more than 63 bits.
    PulseIndexBits,
}

impl Param {
    /// The refusal in a plain sentence.
    pub fn describe(self) -> &'static str {
        match self {
            Param::SyntheticRates => "rates must be positive and the key interval above 0",
            Param::PictureSize => "picture size must be 1 to 8191",
            Param::Levels => "levels must be 1 to 8",
            Param::TopPlane => "top plane must be -1 to 14",
            Param::Budget => "budget per 20 ms must be 64 to 400 bits",
            Param::BudgetFrameLength => "budgets below 72 bits per 20 ms need 40 ms frames",
            Param::Tier => "no tier fits the budget",
            Param::PulseBits => "budget leaves too few bits for the pulses",
            Param::PulseIndexBits => "budget gives a pulse index above 63 bits",
        }
    }
}

/// What went wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A configuration value is not allowed.
    BadConfig {
        /// The refused value.
        param: Param,
    },
    /// The input has the wrong length. `need` is the length the call needs.
    BadInputLength {
        /// The length the call needs.
        need: usize,
    },
    /// The output slice is too short. `need` is the length the call needs.
    OutputTooShort {
        /// The length the call needs.
        need: usize,
    },
    /// A parameter is outside its range.
    BadParameter {
        /// The refused value.
        param: Param,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadConfig { param } => write!(f, "bad configuration: {}", param.describe()),
            Error::BadInputLength { need } => {
                write!(f, "input has the wrong length: {need} needed")
            }
            Error::OutputTooShort { need } => write!(f, "output too short: {need} needed"),
            Error::BadParameter { param } => write!(f, "bad parameter: {}", param.describe()),
        }
    }
}

impl std::error::Error for Error {}

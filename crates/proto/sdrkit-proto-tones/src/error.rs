//! The error type of this crate.

use sdrkit_algo_dsp::DspError;
use std::fmt;

/// A parameter or a group of settings that a call refused. [`TonesError::BadParameter`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// The CTCSS decoder settings.
    Ctcss,
    /// The DCS decoder settings.
    Dcs,
    /// The DTMF decoder settings, with a rate of 4000 Hz or more.
    Dtmf,
    /// The selcall decoder settings.
    Selcall,
    /// The rate for the tones of the chosen selcall sets.
    SelcallRate,
    /// The SELCAL decoder settings, with a rate of 4000 Hz or more.
    Selcal,
    /// The two-tone decoder settings, with a rate of 4000 Hz or more.
    TwoTone,
    /// The audio rate for sub-audio decoding: 1000 Hz or more.
    SubAudioRate,
    /// The rate and the length of a DCS signal.
    DcsSignal,
    /// The rate and the ramp of a tone sequence.
    Sequence,
    /// The length and the tones of a step of a tone sequence.
    Step,
    /// The tones of an analysis: between 0 and half the rate.
    AnalysisTones,
    /// The analysis window: at least 4 samples, and a hop between 1 and its length.
    AnalysisWindow,
    /// The audio rate of an MDC-1200 decoder: 8000 Hz or more.
    MdcRate,
}

impl Param {
    /// The parameter and its limits in plain words.
    pub fn describe(self) -> &'static str {
        match self {
            Param::Ctcss => "a CTCSS setting is out of its range",
            Param::Dcs => "a DCS setting is out of its range",
            Param::Dtmf => "a DTMF setting is out of its range (the rate must be 4000 Hz or more)",
            Param::Selcall => "a selcall setting is out of its range",
            Param::SelcallRate => "the rate is too low for the tones of the chosen sets",
            Param::Selcal => {
                "a SELCAL setting is out of its range (the rate must be 4000 Hz or more)"
            }
            Param::TwoTone => {
                "a two-tone setting is out of its range (the rate must be 4000 Hz or more)"
            }
            Param::SubAudioRate => "sub-audio decoding needs audio at 1000 Hz or more",
            Param::DcsSignal => {
                "a DCS signal needs a rate of 1000 Hz or more and a length of 0 s or more"
            }
            Param::Sequence => "a tone sequence needs a positive rate and a ramp of 0 s or more",
            Param::Step => {
                "every step needs a length of 0 s or more and tones between 0 and half the rate"
            }
            Param::AnalysisTones => "every tone must lie between 0 and half the sample rate",
            Param::MdcRate => "an MDC-1200 decoder needs audio at 8000 Hz or more",
            Param::AnalysisWindow => {
                "the analysis window needs at least 4 samples and a hop between 1 and its length"
            }
        }
    }
}

/// An error from this crate. The decoders and encoders return it for a bad setting. They never panic on bad input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TonesError {
    /// A setting is out of its range.
    BadParameter(Param),
    /// A selcall digit has no tone in the tone set.
    NoTone {
        /// The digit.
        digit: char,
        /// The name of the tone set.
        set: &'static str,
    },
    /// A call holds more tones than a sequence can take.
    TooManyTones {
        /// The most tones a call holds.
        max: usize,
    },
    /// A character is not a DTMF key.
    NotADtmfKey {
        /// The character.
        key: char,
    },
    /// The decimating filter cannot be made.
    Dsp(DspError),
}

impl fmt::Display for TonesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TonesError::BadParameter(p) => write!(f, "invalid parameter: {}", p.describe()),
            TonesError::NoTone { digit, set } => write!(f, "{digit:?} has no tone in {set}"),
            TonesError::TooManyTones { max } => write!(f, "a call holds at most {max} tones"),
            TonesError::NotADtmfKey { key } => write!(f, "{key:?} is not a DTMF key"),
            TonesError::Dsp(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for TonesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TonesError::Dsp(e) => Some(e),
            TonesError::BadParameter(_)
            | TonesError::NoTone { .. }
            | TonesError::TooManyTones { .. }
            | TonesError::NotADtmfKey { .. } => None,
        }
    }
}

/// A component whose decoder cannot be made refuses the format with this error.
impl From<TonesError> for sdrkit_core::ConfigError {
    fn from(e: TonesError) -> Self {
        sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
    }
}

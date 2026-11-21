//! The error type of this crate, and the parameters it names.

use sdrkit_algo_dsp::DspError;

/// Declares [`Param`] and its plain-word names from one list.
macro_rules! params {
    ($($v:ident = $name:literal,)+) => {
        /// A parameter of a simulation object. [`SimError`] names it.
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
    AdcBits = "number of ADC bits",
    AmDepth = "AM depth",
    AttenuationDb = "sideband suppression",
    Audio = "audio length",
    Bandwidth = "signal bandwidth",
    Baud = "baud rate",
    BitRate = "bit rate",
    Bits = "number of bits",
    Bt = "bandwidth-time product",
    ClockPpm = "clock error",
    DcOffset = "DC offset",
    Deviation = "frequency deviation",
    Doppler = "Doppler spectrum",
    DrmChannel = "DRM channel",
    Drift = "frequency drift",
    Duration = "duration",
    EchoDelay = "echo delay",
    EchoLevel = "echo level",
    EchoPhase = "echo phase",
    Echoes = "number of echoes",
    Floor = "phase noise floor",
    FromHz = "start frequency",
    Harmonics = "harmonic amplitudes",
    HumDepth = "hum depth",
    HumFreq = "hum frequency",
    ImpulseRate = "impulse rate",
    ImpulseWidth = "impulse width",
    IqGain = "IQ gain error",
    IqPhase = "IQ phase error",
    JitterPeak = "jitter peak",
    JitterPeriod = "jitter period",
    KeyChanges = "number of key changes in one rise time",
    KeyInterval = "key interval",
    Keys = "key list",
    Latitude = "latitude",
    Lead = "lead time",
    Level = "level",
    Linewidth = "linewidth",
    LosHz = "direct path frequency",
    MarkHz = "mark tone",
    MaxDoppler = "maximum Doppler",
    ModIndex = "modulation index",
    NoiseLevel = "noise level",
    NoisePower = "noise power",
    OffsetHz = "frequency offset",
    Output = "output length",
    Pause = "pause",
    Period = "period",
    PhaseNoise = "phase noise level",
    PhaseNoiseOffset = "phase noise offset",
    Points = "number of points",
    Pulse = "pulse",
    Ramp = "ramp length",
    Rate = "sample rate",
    RiceFactor = "Rice factor",
    RiseTime = "rise time",
    SigmaV = "white phase deviation",
    SigmaW = "random-walk phase deviation",
    SignalPower = "signal power",
    SpaceHz = "space tone",
    Span = "pulse span",
    Sps = "samples per symbol",
    Start = "start",
    State = "generator state",
    StreamRate = "stream rate",
    SymbolRate = "symbol rate",
    Tap = "tap",
    Taps = "number of taps",
    ToHz = "end frequency",
    Transition = "transition width",
    UpdateRate = "update rate",
    Wpm = "speed in words a minute",
}

/// What is wrong in one entry of an echo list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EchoFault {
    /// The entry has fewer than two fields (a delay and a level).
    TooFewFields,
    /// The entry has more than three fields.
    TooManyFields,
    /// A field is not a finite number.
    NotANumber,
}

/// Why a simulation object cannot be made, or why a call is refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SimError {
    /// A number is outside the range the object can use.
    OutOfRange {
        /// The parameter.
        param: Param,
        /// The lowest value the object can use.
        min: f64,
        /// The highest value the object can use.
        max: f64,
        /// The value given.
        got: f64,
    },
    /// A number is NaN or infinite.
    NotFinite {
        /// The parameter.
        param: Param,
    },
    /// A slice or a count does not have the length the object needs.
    Length {
        /// The parameter.
        param: Param,
        /// The length needed.
        expected: usize,
        /// The length given.
        got: usize,
    },
    /// A value is not one of the values the object has.
    NotAllowed {
        /// The parameter.
        param: Param,
        /// The value given.
        got: f64,
    },
    /// Two parameters do not fit together.
    Conflict {
        /// The parameter.
        param: Param,
        /// The parameter it does not fit.
        with: Param,
    },
    /// The values give no usable result (a pulse with no energy, a text with nothing to send).
    Degenerate {
        /// The parameter.
        param: Param,
    },
    /// The signal does not state a value that the configuration needs (a symbol rate for an Es/N0, for example).
    SignalLacks {
        /// The value the signal does not state.
        param: Param,
    },
    /// An entry of an echo list cannot be read.
    EchoText {
        /// The index of the entry, from 0.
        entry: usize,
        /// What is wrong.
        fault: EchoFault,
    },
    /// A DSP object refused its parameters.
    Dsp(DspError),
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            SimError::OutOfRange {
                param,
                min,
                max,
                got,
            } => write!(f, "the {} is {got}, outside {min} to {max}", param.name()),
            SimError::NotFinite { param } => {
                write!(f, "the {} is not a finite number", param.name())
            }
            SimError::Length {
                param,
                expected,
                got,
            } => write!(f, "the {} is {got}, {expected} is needed", param.name()),
            SimError::NotAllowed { param, got } => {
                write!(f, "the {} cannot be {got}", param.name())
            }
            SimError::Conflict { param, with } => {
                write!(f, "the {} does not fit the {}", param.name(), with.name())
            }
            SimError::Degenerate { param } => {
                write!(f, "the {} gives no usable result", param.name())
            }
            SimError::SignalLacks { param } => {
                write!(f, "the signal states no {}", param.name())
            }
            SimError::EchoText { entry, fault } => {
                let what = match fault {
                    EchoFault::TooFewFields => "needs a delay and a level",
                    EchoFault::TooManyFields => "has more than a delay, a level and a phase",
                    EchoFault::NotANumber => "has a field that is not a number",
                };
                write!(f, "echo {entry} {what}")
            }
            SimError::Dsp(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SimError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SimError::Dsp(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DspError> for SimError {
    fn from(e: DspError) -> Self {
        SimError::Dsp(e)
    }
}

/// A generator that cannot build its impairments refuses the format with the simulation error.
impl From<SimError> for sdrkit_core::ConfigError {
    fn from(e: SimError) -> Self {
        sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
    }
}

impl Param {
    /// `Err(NotFinite)` if `got` is not finite, else `Err(OutOfRange)`.
    pub(crate) fn range<T>(self, min: f64, max: f64, got: f64) -> Result<T, SimError> {
        if got.is_finite() {
            Err(SimError::OutOfRange {
                param: self,
                min,
                max,
                got,
            })
        } else {
            Err(SimError::NotFinite { param: self })
        }
    }

    /// The refusal of a value that must be above 0.
    pub(crate) fn positive<T>(self, got: f64) -> Result<T, SimError> {
        self.range(0.0, f64::INFINITY, got)
    }

    /// The refusal of a value that must be at least `min`.
    pub(crate) fn at_least<T>(self, min: f64, got: f64) -> Result<T, SimError> {
        self.range(min, f64::INFINITY, got)
    }

    /// `Err(NotFinite)`.
    pub(crate) fn not_finite<T>(self) -> Result<T, SimError> {
        Err(SimError::NotFinite { param: self })
    }

    /// `Err(Length)`.
    pub(crate) fn length<T>(self, expected: usize, got: usize) -> Result<T, SimError> {
        Err(SimError::Length {
            param: self,
            expected,
            got,
        })
    }

    /// `Err(NotAllowed)`.
    pub(crate) fn not_allowed<T>(self, got: f64) -> Result<T, SimError> {
        Err(SimError::NotAllowed { param: self, got })
    }

    /// `Err(Conflict)`.
    pub(crate) fn conflict<T>(self, with: Param) -> Result<T, SimError> {
        Err(SimError::Conflict { param: self, with })
    }

    /// `Err(SignalLacks)`.
    pub(crate) fn signal_lacks<T>(self) -> Result<T, SimError> {
        Err(SimError::SignalLacks { param: self })
    }

    /// `Err(Degenerate)`.
    pub(crate) fn degenerate<T>(self) -> Result<T, SimError> {
        Err(SimError::Degenerate { param: self })
    }

    /// `x` if it is finite and above 0.
    pub(crate) fn check_positive(self, x: f64) -> Result<f64, SimError> {
        if x > 0.0 && x.is_finite() {
            Ok(x)
        } else {
            self.positive(x)
        }
    }

    /// `x` if it is finite.
    pub(crate) fn check_finite(self, x: f64) -> Result<f64, SimError> {
        if x.is_finite() {
            Ok(x)
        } else {
            self.not_finite()
        }
    }

    /// `x` if it is finite and from `min` to `max`.
    pub(crate) fn check_in(self, x: f64, min: f64, max: f64) -> Result<f64, SimError> {
        if x.is_finite() && (min..=max).contains(&x) {
            Ok(x)
        } else {
            self.range(min, max, x)
        }
    }
}

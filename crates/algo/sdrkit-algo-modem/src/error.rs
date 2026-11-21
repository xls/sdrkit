//! The error type of this crate, and the parameters it names.

use sdrkit_algo_dsp::DspError;
use sdrkit_algo_fec::FecError;

/// Declares [`Param`] and its plain-word names from one list.
macro_rules! params {
    ($($v:ident = $name:literal,)+) => {
        /// A parameter of a modem object. [`ModemError`] names it.
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
AgcGain = "AGC gain limit",
    AgcTarget = "AGC target",
    AgcTime = "AGC time",
    Amplitude = "amplitude",
    Analytic = "analytic converter",
    AudioFilter = "audio filter",
    Averaging = "averaging length",
    Backoff = "FFT window backoff",
    Baud = "baud rate",
    Beta = "Kaiser beta",
    Bin = "bin index",
    BinWidth = "bin width",
    Bins = "number of bins",
    BlockLen = "block length",
    Bt = "bandwidth-time product",
    Cap = "cap",
    Capacity = "capacity",
    CarrierTime = "AM carrier time",
    CarrierToCode = "carrier to code ratio",
    Carriers = "carriers",
    Centre = "centre",
    ChipLen = "chip length",
    Clamp = "clamp",
    CloseLevel = "close threshold",
    Code = "spreading code",
    CodeDoppler = "code Doppler",
    CodeRateOffset = "code rate offset",
    Coherent = "coherent groups",
    Context = "context length",
    Cutoff = "cut-off",
    DcCorner = "DC corner",
    Decimation = "decimation factor",
    Deemphasis = "de-emphasis time",
    Deviation = "frequency deviation",
    Estimator = "channel estimator",
    Factor = "factor",
    Feedback = "feedback taps",
    FftSize = "FFT size",
    Floor = "floor minimum",
    FloorRatio = "floor ratio",
    Frequencies = "number of frequencies",
    Frequency = "frequency",
    Gain = "detector gain",
    GridStep = "grid step",
    Guard = "guard interval",
    HighLevel = "high level",
    Hysteresis = "hysteresis",
    Inertia = "inertia",
    Input = "input",
    Kurtosis = "kurtosis",
    LabelMap = "label map",
    Labels = "labels",
    Lag = "lag",
    Lags = "lag list",
    Layout = "OFDM layout",
    Level = "level",
    Levels = "number of levels",
    Loop = "carrier loop",
    LoopBandwidth = "loop bandwidth",
    LowHz = "lower band edge",
    LowLevel = "low level",
    MaxFreq = "frequency range",
    MaxLen = "maximum length",
    MaxPpm = "clock range",
    Memory = "memory",
    MinBlocks = "number of blocks to decide",
    NoiseBand = "noise band",
    NoiseVariance = "noise variance",
    Nulls = "null carriers",
    OffsetRange = "offset range",
    OpenLevel = "open threshold",
    Order = "order",
    Oversampling = "oversampling factor",
    Period = "period",
    Periods = "number of periods",
    Phases = "number of phases",
    Pilots = "pilots",
    Points = "number of points",
    Pool = "pool size",
    Post = "post-burst length",
    Power = "power",
    Pre = "pre-burst length",
    Prefix = "cyclic prefix",
    Probability = "probability",
    Pulse = "pulse",
    Quantile = "quantile",
    Rate = "sample rate",
    Reject = "reject list",
    ReportTime = "report time",
    RollOff = "roll-off",
    SamplesPerCell = "samples per cell",
    SamplesPerChip = "samples per chip",
    Shift = "shift",
    Shifts = "number of shifts",
    SidebandFilter = "sideband filter",
    Smoothing = "smoothing factor",
    Spacing = "spacing",
    Span = "span",
    SpreadingFactor = "spreading factor",
    Sps = "samples per symbol",
    Start = "start",
    StartState = "start state",
    Step = "step",
    StepSize = "step size",
    Steps = "number of steps",
    SubBins = "number of sub-bins",
    SwitchLevel = "decision-directed switch level",
    SymbolLen = "symbol length",
    SymbolRate = "symbol rate",
    Symbols = "number of symbols",
    Taps = "number of taps",
    Template = "template",
    Terms = "number of terms",
    Threshold = "threshold",
    TimeConstant = "time constant",
    Tones = "tones",
    Training = "training sequence",
    UpdateSpacing = "update spacing",
    Width = "width",
    Window = "window",
}

/// Why a modem object cannot be made, or why a call is refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModemError {
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
    /// The values give no usable result (no energy, a singular fit, nothing inside the span).
    Degenerate {
        /// The parameter.
        param: Param,
    },
    /// A name does not name one of the objects of its kind.
    UnknownName {
        /// The kind of object.
        param: Param,
    },
    /// An output slice is too short: `need` items were needed.
    OutputTooShort {
        /// Items needed.
        need: usize,
    },
    /// The input is shorter than the operation needs: `need` items were needed.
    InputTooShort {
        /// Items needed.
        need: usize,
    },
    /// A DSP object refused its parameters.
    Dsp(DspError),
    /// A decoder of the FEC crate refused its parameters.
    Fec(FecError),
}

impl std::fmt::Display for ModemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ModemError::OutOfRange {
                param,
                min,
                max,
                got,
            } => write!(f, "the {} is {got}, outside {min} to {max}", param.name()),
            ModemError::NotFinite { param } => {
                write!(f, "the {} is not a finite number", param.name())
            }
            ModemError::Length {
                param,
                expected,
                got,
            } => write!(f, "the {} is {got}, {expected} is needed", param.name()),
            ModemError::NotAllowed { param, got } => {
                write!(f, "the {} cannot be {got}", param.name())
            }
            ModemError::Conflict { param, with } => {
                write!(f, "the {} does not fit the {}", param.name(), with.name())
            }
            ModemError::Degenerate { param } => {
                write!(f, "the {} gives no usable result", param.name())
            }
            ModemError::UnknownName { param } => {
                write!(f, "there is no {} of that name", param.name())
            }
            ModemError::OutputTooShort { need } => {
                write!(f, "output too short: {need} items needed")
            }
            ModemError::InputTooShort { need } => write!(f, "input too short: {need} items needed"),
            ModemError::Dsp(e) => e.fmt(f),
            ModemError::Fec(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ModemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ModemError::Dsp(e) => Some(e),
            ModemError::Fec(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DspError> for ModemError {
    fn from(e: DspError) -> Self {
        ModemError::Dsp(e)
    }
}

impl From<FecError> for ModemError {
    fn from(e: FecError) -> Self {
        ModemError::Fec(e)
    }
}

/// A component that cannot build its modem objects refuses the format with the modem error.
impl From<ModemError> for sdrkit_core::ConfigError {
    fn from(e: ModemError) -> Self {
        sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
    }
}

impl Param {
    /// `Err(NotFinite)` if `got` is not finite, else `Err(OutOfRange)`.
    pub(crate) fn range<T>(self, min: f64, max: f64, got: impl Into<f64>) -> Result<T, ModemError> {
        let got = got.into();
        if got.is_finite() {
            Err(ModemError::OutOfRange {
                param: self,
                min,
                max,
                got,
            })
        } else {
            Err(ModemError::NotFinite { param: self })
        }
    }

    /// The refusal of a value that must be above 0.
    pub(crate) fn positive<T>(self, got: impl Into<f64>) -> Result<T, ModemError> {
        self.range(0.0, f64::INFINITY, got)
    }

    /// The refusal of a value that must be at least `min`.
    pub(crate) fn at_least<T>(self, min: f64, got: impl Into<f64>) -> Result<T, ModemError> {
        self.range(min, f64::INFINITY, got)
    }

    /// `Err(NotFinite)`.
    pub(crate) fn not_finite<T>(self) -> Result<T, ModemError> {
        Err(ModemError::NotFinite { param: self })
    }

    /// `Err(Length)`.
    pub(crate) fn length<T>(self, expected: usize, got: usize) -> Result<T, ModemError> {
        Err(ModemError::Length {
            param: self,
            expected,
            got,
        })
    }

    /// `Err(NotAllowed)`.
    pub(crate) fn not_allowed<T>(self, got: impl Into<f64>) -> Result<T, ModemError> {
        Err(ModemError::NotAllowed {
            param: self,
            got: got.into(),
        })
    }

    /// `Err(Conflict)`.
    pub(crate) fn conflict<T>(self, with: Param) -> Result<T, ModemError> {
        Err(ModemError::Conflict { param: self, with })
    }

    /// `Err(Degenerate)`.
    pub(crate) fn degenerate<T>(self) -> Result<T, ModemError> {
        Err(ModemError::Degenerate { param: self })
    }

    /// `Err(UnknownName)`.
    pub(crate) fn unknown_name<T>(self) -> Result<T, ModemError> {
        Err(ModemError::UnknownName { param: self })
    }

    /// `Ok(())` if `x` is finite and above 0.
    pub(crate) fn check_positive(self, x: impl Into<f64>) -> Result<(), ModemError> {
        let x = x.into();
        if x > 0.0 && x.is_finite() {
            Ok(())
        } else {
            self.positive(x)
        }
    }

    /// `Ok(())` if `x` is finite.
    pub(crate) fn check_finite(self, x: impl Into<f64>) -> Result<(), ModemError> {
        let x = x.into();
        if x.is_finite() {
            Ok(())
        } else {
            self.not_finite()
        }
    }

    /// `Ok(())` if `x` is finite and from `min` to `max`.
    pub(crate) fn check_in(self, x: impl Into<f64>, min: f64, max: f64) -> Result<(), ModemError> {
        let x = x.into();
        if x.is_finite() && (min..=max).contains(&x) {
            Ok(())
        } else {
            self.range(min, max, x)
        }
    }

    /// `Ok(())` if `x` is finite and inside `(min, max)`.
    pub(crate) fn check_open(
        self,
        x: impl Into<f64>,
        min: f64,
        max: f64,
    ) -> Result<(), ModemError> {
        let x = x.into();
        if x > min && x < max && x.is_finite() {
            Ok(())
        } else {
            self.range(min, max, x)
        }
    }

    /// `Ok(())` if `x` is finite, above `min` and at most `max`.
    pub(crate) fn check_above(
        self,
        x: impl Into<f64>,
        min: f64,
        max: f64,
    ) -> Result<(), ModemError> {
        let x = x.into();
        if x > min && x <= max && x.is_finite() {
            Ok(())
        } else {
            self.range(min, max, x)
        }
    }

    /// `Ok(())` if `n` is from `min` to `max`.
    pub(crate) fn check_count(self, n: usize, min: usize, max: usize) -> Result<(), ModemError> {
        if (min..=max).contains(&n) {
            Ok(())
        } else {
            self.range(min as f64, max as f64, n as f64)
        }
    }
}

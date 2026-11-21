//! The error type of this crate, and the parameters it names.

/// Declares [`Param`] and its plain-word names from one list.
macro_rules! params {
    ($($v:ident = $name:literal,)+) => {
        /// A parameter of a DSP object. [`DspError`] names it.
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
    Alpha = "hierarchical QAM alpha",
    Attack = "attack factor",
    AttenuationDb = "stop-band attenuation",
    Band = "band index",
    Bands = "number of bands",
    Bandwidth = "bandwidth",
    Beta = "Kaiser beta",
    Bins = "number of bins",
    Bits = "number of bits",
    BlockLen = "block length",
    Bt = "bandwidth-time product",
    Capacity = "capacity",
    Channel = "channel index",
    Channels = "number of channels",
    CentreHz = "centre frequency",
    Coefficients = "number of coefficients",
    Cutoff = "cut-off",
    Damping = "damping factor",
    Decimation = "decimation factor",
    Delay = "differential delay",
    Deviation = "frequency deviation",
    FallTime = "fall time",
    FftSize = "FFT size",
    FrameRate = "frames per second",
    Frames = "number of frames",
    Frequency = "frequency",
    FullScale = "full scale",
    Gain = "loop gain",
    Gamma = "APSK ring ratio",
    Gamma1 = "inner APSK ring ratio",
    Gamma2 = "outer APSK ring ratio",
    GridStep = "grid step",
    HalfSpan = "half span",
    HighHz = "upper band edge",
    Input = "input length",
    InputBits = "number of input bits",
    InputRate = "input rate",
    Interpolation = "interpolation factor",
    Labels = "label map",
    Lag = "lag",
    Lags = "number of lags",
    LoopBandwidth = "loop bandwidth",
    LowHz = "lower band edge",
    Lsp = "number of line spectral pairs",
    Margin = "attenuation margin",
    Matrix = "matrix",
    MinRate = "minimum rate",
    OffsetHz = "frequency offset",
    Order = "order",
    Output = "output length",
    OutputRate = "output rate",
    Overlap = "overlap",
    Oversampling = "oversampling factor",
    Pass = "pass-band edge",
    Phase = "phase",
    Points = "number of points",
    Power = "power",
    Quantile = "quantile",
    Radius = "ring radius",
    Rank = "rank",
    Rate = "sample rate",
    Resolution = "table resolution",
    Rise = "rise per update",
    Rings = "rings",
    RollOff = "roll-off",
    Row = "row length",
    Shape = "pulse shape",
    Shift = "frequency shift",
    Smoothing = "smoothing factor",
    Span = "span",
    Sps = "samples per symbol",
    Start = "start offset",
    Stop = "stop-band edge",
    Tau = "time constant",
    Taps = "number of taps",
    TapsPerBand = "taps per band",
    Tones = "number of tones",
    Transition = "transition width",
    Unknowns = "number of unknowns",
    UpperHz = "upper corner",
    Values = "values",
    Weight = "weight",
    Window = "window",
    X = "x values",
    Y = "y values",
}

/// Why a DSP object cannot be made, or why a call is refused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DspError {
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
    /// A value is not one of the values the object has (a size that is not a power of two, an order without a design).
    NotAllowed {
        /// The parameter.
        param: Param,
        /// The value given.
        got: f64,
    },
    /// Two parameters do not fit together (a pass-band edge above the stop-band edge, for example).
    Conflict {
        /// The parameter.
        param: Param,
        /// The parameter it does not fit.
        with: Param,
    },
    /// The values give no usable result (a sum of zero, a singular matrix, no stable inverse).
    Degenerate {
        /// The parameter.
        param: Param,
    },
}

impl std::fmt::Display for DspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            DspError::OutOfRange {
                param,
                min,
                max,
                got,
            } => write!(f, "the {} is {got}, outside {min} to {max}", param.name()),
            DspError::NotFinite { param } => {
                write!(f, "the {} is not a finite number", param.name())
            }
            DspError::Length {
                param,
                expected,
                got,
            } => write!(f, "the {} is {got}, {expected} is needed", param.name()),
            DspError::NotAllowed { param, got } => {
                write!(f, "the {} cannot be {got}", param.name())
            }
            DspError::Conflict { param, with } => {
                write!(f, "the {} does not fit the {}", param.name(), with.name())
            }
            DspError::Degenerate { param } => {
                write!(f, "the {} gives no usable result", param.name())
            }
        }
    }
}

impl std::error::Error for DspError {}

/// A component that cannot build its DSP objects refuses the format with the DSP error.
impl From<DspError> for sdrkit_core::ConfigError {
    fn from(e: DspError) -> Self {
        sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
    }
}

impl Param {
    /// `Err(NotFinite)` if `got` is not finite, else `Err(OutOfRange)`.
    pub(crate) fn range<T>(self, min: f64, max: f64, got: f64) -> Result<T, DspError> {
        if got.is_finite() {
            Err(DspError::OutOfRange {
                param: self,
                min,
                max,
                got,
            })
        } else {
            Err(DspError::NotFinite { param: self })
        }
    }

    /// The refusal of a value that must be above 0.
    pub(crate) fn positive<T>(self, got: f64) -> Result<T, DspError> {
        self.range(0.0, f64::INFINITY, got)
    }

    /// The refusal of a value that must be at least `min`.
    pub(crate) fn at_least<T>(self, min: f64, got: f64) -> Result<T, DspError> {
        self.range(min, f64::INFINITY, got)
    }

    /// `Err(NotFinite)`.
    pub(crate) fn not_finite<T>(self) -> Result<T, DspError> {
        Err(DspError::NotFinite { param: self })
    }

    /// `Err(Length)`.
    pub(crate) fn length<T>(self, expected: usize, got: usize) -> Result<T, DspError> {
        Err(DspError::Length {
            param: self,
            expected,
            got,
        })
    }

    /// `Err(NotAllowed)`.
    pub(crate) fn not_allowed<T>(self, got: f64) -> Result<T, DspError> {
        Err(DspError::NotAllowed { param: self, got })
    }

    /// `Err(Conflict)`.
    pub(crate) fn conflict<T>(self, with: Param) -> Result<T, DspError> {
        Err(DspError::Conflict { param: self, with })
    }

    /// `Err(Degenerate)`.
    pub(crate) fn degenerate<T>(self) -> Result<T, DspError> {
        Err(DspError::Degenerate { param: self })
    }
}

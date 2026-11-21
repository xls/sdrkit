//! The error type of this crate.

use sdrkit_algo_dsp::DspError;
use sdrkit_algo_fec::FecError;
use sdrkit_algo_modem::ModemError;
use sdrkit_algo_sim::SimError;

/// A parameter that a call refused. [`LoraError::BadParameter`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// Bandwidth finite and above 0 Hz.
    Bandwidth,
    /// Channel filter does not fit the rate.
    ChannelFilter,
    /// Channel offset finite.
    ChannelOffset,
    /// Coding rate 5 to 8 (4/5 to 4/8).
    CodingRate,
    /// FSK rate 100 kHz or more.
    FskRate,
    /// Implicit header: coding rate 5 to 8, length 1 to 255.
    ImplicitHeader,
    /// Implicit length 1 to 255.
    ImplicitLength,
    /// Jump size and carrier finite.
    JumpOrCarrier,
    /// Layout spreading factor differs from the codec.
    LayoutSpreadingFactor,
    /// Preamble 6 symbols or more.
    Preamble,
    /// Sample rate 2 BW or more, bandwidth finite and above 0 Hz.
    Rate,
    /// Spreading factor 5 to 12.
    SpreadingFactor,
    /// 1 to 16 sync words.
    SyncWords,
}

impl Param {
    /// The parameter and its limits in plain words.
    pub fn describe(self) -> &'static str {
        match self {
            Param::Bandwidth => "bandwidth finite and above 0 Hz",
            Param::ChannelFilter => "channel filter does not fit the rate",
            Param::ChannelOffset => "channel offset finite",
            Param::CodingRate => "coding rate 5 to 8 (4/5 to 4/8)",
            Param::FskRate => "FSK rate 100 kHz or more",
            Param::ImplicitHeader => "implicit header: coding rate 5 to 8, length 1 to 255",
            Param::ImplicitLength => "implicit length 1 to 255",
            Param::JumpOrCarrier => "jump size and carrier finite",
            Param::LayoutSpreadingFactor => "layout spreading factor differs from the codec",
            Param::Preamble => "preamble 6 symbols or more",
            Param::Rate => "sample rate 2 BW or more, bandwidth finite and above 0 Hz",
            Param::SpreadingFactor => "spreading factor 5 to 12",
            Param::SyncWords => "1 to 16 sync words",
        }
    }
}

/// What went wrong. Every fallible function of this crate returns this.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoraError {
    /// A parameter is out of its range.
    BadParameter(Param),
    /// The payload is too long for one frame. A frame carries at most 255 bytes.
    PayloadTooLong {
        /// The length that was given.
        len: usize,
    },
    /// A filter, resampler or channel bank cannot be made.
    Dsp(DspError),
    /// The dechirp cannot be made.
    Modem(ModemError),
    /// The FSK modulator cannot be made.
    Sim(SimError),
    /// The Hamming code cannot be made.
    Fec(FecError),
    /// The interleaver cannot be made or used.
    Bits(sdrkit_util_bits::Error),
}

impl std::fmt::Display for LoraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoraError::BadParameter(p) => write!(f, "bad LoRa parameter: {}", p.describe()),
            LoraError::PayloadTooLong { len } => {
                write!(f, "a payload of {len} bytes is too long (255 at most)")
            }
            LoraError::Dsp(e) => e.fmt(f),
            LoraError::Modem(e) => e.fmt(f),
            LoraError::Sim(e) => e.fmt(f),
            LoraError::Fec(e) => e.fmt(f),
            LoraError::Bits(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for LoraError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoraError::Dsp(e) => Some(e),
            LoraError::Modem(e) => Some(e),
            LoraError::Sim(e) => Some(e),
            LoraError::Fec(e) => Some(e),
            LoraError::Bits(e) => Some(e),
            LoraError::BadParameter(_) | LoraError::PayloadTooLong { .. } => None,
        }
    }
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, LoraError>;

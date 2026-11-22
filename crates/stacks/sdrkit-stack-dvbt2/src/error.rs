//! The errors of the DVB-T2 coding, framing and transmitter.
//!
//! Standard: ETSI EN 302 755 V1.4.1.

use sdrkit_algo_fec::FecError;
use sdrkit_algo_modem::ModemError;

/// A rule of EN 302 755 that a frame shape, a coding or a transmitter configuration does not keep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// The extended carrier mode exists for 8K, 16K and 32K only (clause 9.5).
    ExtendedCarriers,
    /// The guard interval is not allowed for the FFT size (table 67).
    GuardForFft,
    /// The pilot pattern is not allowed for the FFT size and the guard interval (tables 58 and 59).
    PilotsForGuard,
    /// The frame has too few data symbols, or more than 4095 symbols (clause 8.3.1).
    SymbolCount,
    /// A 32K frame has an even number of symbols (clause 8.3.1).
    OddSymbols,
    /// The L1 cells do not divide by N_P2 (clause 8.3.5).
    L1NotDivisible,
    /// The L1 signalling does not fit the P2 symbols (clause 8.3.5).
    L1DoesNotFit,
    /// The standard has no LDPC or BCH code for this FEC frame and code rate (tables 6(a), 6(b) and I.1).
    NoCode,
    /// The standard has no cell interleaver register for this number of cells.
    NoCellInterleaver,
    /// A symbol has more data cells than FFT points.
    TooManyCells,
    /// N_post is not a multiple of the L1 bit interleaver columns.
    PostNotDivisible,
    /// The L1-post constellation is 256-QAM, or L1_POST_INFO_SIZE is 0 (clause 7.3.1.2).
    PostCoding,
    /// T2-Lite carries the L1-post in one FEC block (annex I).
    LitePostBlocks,
    /// A transmitter configuration has no PLP.
    NoPlp,
    /// The code rate is not used in this profile (annex I).
    RateForProfile,
    /// T2-Lite uses the 16 200-bit FEC frame only (annex I.1).
    LiteFecFrame,
    /// T2-Lite never uses 256-QAM with the code rates 2/3 and 3/4, and never rotates 256-QAM (table I.4).
    LiteConstellation,
    /// A data PLP of T2-Lite with its common PLP carries more than 4 Mbit/s (annex I.10).
    LiteBitRate,
    /// FEF_LENGTH does not fit its field (22 bits, 24 bits in T2-Lite), or the FEF part is longer than 250 ms (1 s in
    /// T2-Lite).
    FefLength,
    /// BPSK carries L1 only.
    BpskData,
    /// Time interleaving can be off only with one PLP.
    TimeIlOff,
    /// NUM_T2_FRAMES is not a multiple of FEF_INTERVAL.
    FefInterval,
    /// The cells of an Interleaving Frame do not divide by P_I (clause 8.3.6.1).
    InterleavingFrame,
    /// N_cells of a type 2 PLP is not a multiple of 5 N_subslices P_I (clause 8.3.6.3.3).
    SubSlices,
    /// NUM_T2_FRAMES is not a multiple of P_I.
    FramesForPi,
    /// The PLPs do not fit the frame.
    PlpsDoNotFit,
    /// The FFT size and guard interval cannot be signalled in this profile.
    NotSignalled,
}

/// The error of the DVB-T2 coding, framing and transmitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum T2Error {
    /// The parameters break a rule of the standard.
    Rule(Rule),
    /// A buffer has another length than the coding needs.
    Length {
        /// The length the coding needs.
        expected: usize,
        /// The length given.
        got: usize,
    },
    /// The error correction refused its parameters or its input.
    Fec(FecError),
    /// The constellation or the OFDM layer refused its parameters or its input.
    Modem(ModemError),
    /// A bit interleaver refused its parameters or its input.
    Bits(sdrkit_util_bits::Error),
    /// The frequency interleaver has no permutation for this number of data cells.
    NoInterleaver {
        /// The data cells of the symbol.
        cells: usize,
    },
}

impl From<Rule> for T2Error {
    fn from(r: Rule) -> Self {
        T2Error::Rule(r)
    }
}

impl From<FecError> for T2Error {
    fn from(e: FecError) -> Self {
        T2Error::Fec(e)
    }
}

impl From<ModemError> for T2Error {
    fn from(e: ModemError) -> Self {
        T2Error::Modem(e)
    }
}

impl From<sdrkit_util_bits::Error> for T2Error {
    fn from(e: sdrkit_util_bits::Error) -> Self {
        T2Error::Bits(e)
    }
}

impl std::fmt::Display for T2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            T2Error::Rule(r) => write!(f, "EN 302 755 does not allow this: {r:?}"),
            T2Error::Length { expected, got } => {
                write!(f, "{got} items where {expected} are needed")
            }
            T2Error::Fec(e) => write!(f, "error correction: {e}"),
            T2Error::Modem(e) => write!(f, "demodulator: {e}"),
            T2Error::Bits(e) => write!(f, "bit interleaver: {e}"),
            T2Error::NoInterleaver { cells } => {
                write!(f, "no frequency interleaver for {cells} cells")
            }
        }
    }
}

impl std::error::Error for T2Error {}

//! Bit-level utilities shared by every sdrkit crate.
//!
//! It holds the algorithms that work on bits and bytes: CRCs, scramblers, interleavers, line codes, bit packing and readers.
//! Hard bits are `u8`, one bit per byte (only the lowest bit is read). Soft bits are log-likelihood ratios
//! `L = ln P(b=0) / P(b=1)`, positive for 0; the [`Soft`] trait covers `i8`, `f32` and hard bits.
//!
//! # Modules
//!
//! | Module | What it holds |
//! |---|---|
//! | [`pack`] | [`BitOrder`], packing and unpacking (8 bits at a time), fields on one-bit-per-byte slices, bit reversal |
//! | [`reader`] | [`BitReader`] and [`BitWriter`] over packed bytes, MSB or LSB first; [`HardBits`] over one-bit-per-byte input |
//! | [`crc`] | the CRC engine with compile-time tables, the [`crc::catalog`] of named CRCs (looked up by name or key), and cyclic remainders on bit strings |
//! | [`checksum`] | sums, Fletcher, Adler-32, the Internet checksum, NMEA, parity and the small protocol checks |
//! | [`lfsr`] | linear feedback shift registers, additive and self-synchronising scramblers, precomputed periods |
//! | [`seq`] | every named scrambler and sequence, Gold codes and m-sequences; scramblers and byte sequences looked up by key |
//! | [`interleave`] | [`interleave::Permutation`] and [`interleave::DelayLanes`], with the named interleavers |
//! | [`line`](mod@line) | line codes: NRZI, NRZ-M, differential, Manchester, Miller, FM0, PIE, 3-of-6, 4b/6b, PPM, UART framing |
//! | [`hdlc`] | HDLC bit stuffing and framing with the FCS check (16- or 32-bit), byte stuffing (PPP, KISS, Beast) |
//! | [`sync`] | sync-word matching with allowed bit errors (hard bits) and a soft correlator (LLRs) |
//! | [`gray`] | Gray code |
//! | [`bcd`] | binary-coded decimal |
//! | [`per`] | ASN.1 unaligned PER (ITU-T X.691) reader and writer |
//! | [`per_schema`] | ASN.1 types written as tables, one walker that reads, writes and prints them in unaligned PER |
//! | [`protobuf`] | the protobuf wire format reader and writer |
//! | [`xorshift`] | the 64-bit xorshift generator (13, 7, 17) for tables and test data made from a seed |

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod bcd;
pub mod checksum;
pub mod crc;
pub mod gray;
pub mod hdlc;
pub mod interleave;
pub mod lfsr;
pub mod line;
pub mod pack;
pub mod per;
pub mod per_schema;
pub mod protobuf;
pub mod reader;
pub mod seq;
mod soft;
pub mod sync;
pub mod xorshift;

pub use pack::{BitOrder, Pad};
pub use reader::{BitReader, BitWriter, HardBits, Lsb, Msb};
pub use soft::{Llr, Soft};

/// A parameter that a call refused. [`Error::BadParameter`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// At most 19 BCD digits.
    BcdDigits,
    /// BeiDou B1I code (1 to 37).
    BeidouCode,
    /// N above 64.
    BitCount,
    /// Bit reversal over 8 bits: n at most 256.
    BitReversalLength,
    /// At most 9 bytes.
    ByteCount,
    /// N_cbps must be a multiple of n_col.
    CodedBitsPerSymbol,
    /// CRC width is not whole bytes.
    CrcWholeBytes,
    /// CRC width or parameters.
    CrcWidth,
    /// DAB logical frame: a multiple of 16 bits.
    DabFrameLength,
    /// Value has more digits than allowed.
    Digits,
    /// DVB-T bit interleaver branch (0 to 5).
    DvbtBranch,
    /// C/A G2 delay (0 to 1022 chips).
    G2Delay,
    /// DVB-S2 Gold code number.
    GoldCode,
    /// GPS PRN (1 to 32).
    GpsPrn,
    /// HDLC FCS: 8 to 32 bits, whole bytes.
    HdlcFcs,
    /// HDLC frame lengths: min above max.
    HdlcFrameLengths,
    /// HFDL length: a multiple of 40.
    HfdlLength,
    /// L1 C/A PRN (1-32, 120-158, 193-202).
    L1CaPrn,
    /// LoRa interleaver size.
    LoraInterleaver,
    /// Multiplier not prime to the length.
    MultiplierNotPrime,
    /// Multiplier does not give a permutation.
    NotAPermutation,
    /// PBCH bit kinds.
    PbchBitKinds,
    /// PIDS CRC: 68 PDU bits.
    PidsLength,
    /// PIE lengths: need 0 < pw < data0 < data1.
    PieLengths,
    /// Polynomial is not primitive or the seed is zero.
    Polynomial,
    /// PPM bits per symbol (1 to 8).
    PpmBits,
    /// Register length for a period check.
    RegisterLength,
    /// Soft sync length 1..=128, threshold 0..=1.
    SoftSync,
    /// Sync length 1..=128 and max_err below len/2.
    Sync,
    /// Zadoff-Chu root (1 to N - 1, prime to N).
    ZadoffChuRoot,
}

impl Param {
    /// The parameter and its limits in plain words.
    pub fn describe(self) -> &'static str {
        match self {
            Param::BcdDigits => "at most 19 BCD digits",
            Param::BeidouCode => "BeiDou B1I code (1 to 37)",
            Param::BitCount => "n above 64",
            Param::BitReversalLength => "bit reversal over 8 bits: n at most 256",
            Param::ByteCount => "at most 9 bytes",
            Param::CodedBitsPerSymbol => "n_cbps must be a multiple of n_col",
            Param::CrcWholeBytes => "CRC width is not whole bytes",
            Param::CrcWidth => "CRC width or parameters",
            Param::DabFrameLength => "DAB logical frame: a multiple of 16 bits",
            Param::Digits => "value has more digits than allowed",
            Param::DvbtBranch => "DVB-T bit interleaver branch (0 to 5)",
            Param::G2Delay => "C/A G2 delay (0 to 1022 chips)",
            Param::GoldCode => "DVB-S2 Gold code number",
            Param::GpsPrn => "GPS PRN (1 to 32)",
            Param::HdlcFcs => "HDLC FCS: 8 to 32 bits, whole bytes",
            Param::HdlcFrameLengths => "HDLC frame lengths: min above max",
            Param::HfdlLength => "HFDL length: a multiple of 40",
            Param::L1CaPrn => "L1 C/A PRN (1-32, 120-158, 193-202)",
            Param::LoraInterleaver => "LoRa interleaver size",
            Param::MultiplierNotPrime => "multiplier not prime to the length",
            Param::NotAPermutation => "multiplier does not give a permutation",
            Param::PbchBitKinds => "PBCH bit kinds",
            Param::PidsLength => "PIDS CRC: 68 PDU bits",
            Param::PieLengths => "PIE lengths: need 0 < pw < data0 < data1",
            Param::Polynomial => "polynomial is not primitive or the seed is zero",
            Param::PpmBits => "PPM bits per symbol (1 to 8)",
            Param::RegisterLength => "register length for a period check",
            Param::SoftSync => "soft sync length 1..=128, threshold 0..=1",
            Param::Sync => "sync length 1..=128 and max_err below len/2",
            Param::ZadoffChuRoot => "Zadoff-Chu root (1 to N - 1, prime to N)",
        }
    }
}

/// How a message is malformed. [`Error::Malformed`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Malformation {
    /// Field number.
    FieldNumber,
    /// Varint longer than 10 bytes.
    VarintLength,
    /// Varint over 64 bits.
    VarintOverflow,
    /// Wire type (groups are not supported).
    WireType,
}

impl Malformation {
    /// The fault in plain words.
    pub fn describe(self) -> &'static str {
        match self {
            Malformation::FieldNumber => "field number",
            Malformation::VarintLength => "varint longer than 10 bytes",
            Malformation::VarintOverflow => "varint over 64 bits",
            Malformation::WireType => "wire type (groups are not supported)",
        }
    }
}

/// What went wrong. Every fallible function in this crate returns this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// An output slice is too short: `need` items were needed.
    OutputTooShort {
        /// Items needed.
        need: usize,
    },
    /// The input is shorter than the operation needs.
    InputTooShort,
    /// The input does not have the length the operation needs (for example a frame of the wrong size).
    BadLength,
    /// A parameter is outside its allowed range.
    BadParameter(Param),
    /// The input holds a value the code does not allow (a bad Manchester pair, a nibble above 9, an unknown codeword ...).
    /// `at` is the index of the first bad item.
    Invalid {
        /// Index of the first bad item.
        at: usize,
    },
    /// A message is malformed (protobuf, PER).
    Malformed(Malformation),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OutputTooShort { need } => write!(f, "output too short: {need} items needed"),
            Error::InputTooShort => write!(f, "input too short"),
            Error::BadLength => write!(f, "input has the wrong length"),
            Error::BadParameter(p) => write!(f, "bad parameter: {}", p.describe()),
            Error::Invalid { at } => write!(f, "invalid value at index {at}"),
            Error::Malformed(m) => write!(f, "malformed message: {}", m.describe()),
        }
    }
}

impl std::error::Error for Error {}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;

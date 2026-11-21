//! Wi-Fi (IEEE 802.11) and Bluetooth LE physical layers and frame formats: receivers, transmitters and parsers.
//!
//! Each module names the clause of the standard that it implements.
//!
//! # Modules
//!
//! | Module | Standard | What it holds |
//! |---|---|---|
//! | [`ble`] | Bluetooth Core Specification v5.4, Vol 6 Part B | channels, access address, whitening, CRC-24, advertising PDUs (parse and build), the bits on the air, and the LE 1M advertising receiver ([`ble::AdvReceiver`]) |
//! | [`ble::ad`] | Core Specification Supplement v11, Part A | advertising data: the AD structure walk and the common structures (flags, names, UUID lists, service data, manufacturer data) |
//! | [`dsss`] | IEEE Std 802.11-2020 clause 16 | 802.11b DSSS at 1 and 2 Mbit/s: Barker spreading, DBPSK and DQPSK, the scrambler, long and short PLCP preambles and headers (all four rates' LENGTH rules), the PPDU builder and the receiver ([`dsss::DsssReceiver`]) |
//! | [`ofdm`] | IEEE Std 802.11-2020 clause 17 | 802.11a/g OFDM at all eight rates: SIGNAL, SERVICE and the scrambler seed, the coding chain (scrambler, K = 7 code, puncturing, interleaver, mapping) both ways, training fields, the PPDU builder and the burst receiver ([`ofdm::OfdmReceiver`]) |
//! | [`s1g`] | IEEE Std 802.11-2020 clause 23 | 802.11ah (S1G) numerology: widths, tone plans, MCS table, data bits per symbol, rates |
//! | [`mac`] | IEEE Std 802.11-2020 clause 9 | MAC frames: frame control, addresses, sequence control, the FCS, management frame bodies (beacon, probe request and response), the element walk and the common elements (SSID, rates, DS parameter set, RSN, vendor specific), and builders |
//!
//! The receivers have benchmarks for the device in [`benchmarks`].
//!
//! Hard bits are `u8`, one bit per byte, the first bit sent first. Soft bits are LLRs where a positive value means 0.
//! The receivers keep their state between calls, so the output does not depend on how the input is split. Parsers borrow the frame
//! and return `None` or an [`Error`] for a truncated or malformed field.
//!
//! ```
//! use sdrkit_proto_wlan_phy::{ble, mac};
//! // A BLE advertising PDU with a name, and an 802.11 FCS check.
//! let mut pdu = Vec::new();
//! let mut data = Vec::new();
//! ble::ad::push(&mut data, ble::ad::COMPLETE_NAME, b"probe").unwrap();
//! ble::build_adv(&mut pdu, ble::PduType::AdvNonconnInd, &ble::Addr::random([1, 2, 3, 4, 5, 0xC6]), &data).unwrap();
//! let p = ble::AdvPdu::parse(&pdu).unwrap();
//! assert_eq!(p.data().unwrap(), &data[..]);
//! let mut frame = b"payload".to_vec();
//! mac::push_fcs(&mut frame);
//! assert!(mac::fcs_ok(&frame));
//! ```
#![warn(missing_docs)]
#![forbid(unsafe_code)]
// Signal-processing kernels index several arrays by the same position, as the standards' equations do.
#![allow(clippy::needless_range_loop)]

mod bench;
pub mod ble;
pub mod dsss;
pub mod mac;
pub mod ofdm;
pub mod s1g;

pub use bench::benchmarks;
pub use sdrkit_core::Complex32;

/// A parameter that was refused. [`Error::BadParameter`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// The PSDU length does not match the header.
    PsduLengthMismatch,
    /// CCK modulation is not available.
    CckNotBuilt,
    /// A DSSS receiver needs 11 to 64 samples per symbol.
    DsssSamplesPerSymbol,
    /// A BLE channel index is 0 to 39.
    BleChannelIndex,
    /// Build_adv makes ADV_IND, ADV_NONCONN_IND, ADV_SCAN_IND or SCAN_RSP.
    BleAdvType,
    /// A PSDU has 1 to 4095 octets.
    PsduLength,
    /// A scrambler seed is 1 to 127.
    ScramblerSeed,
    /// The input must be one symbol of coded bits.
    CodedBitsPerSymbol,
    /// The LLRs must fill n_sym whole symbols.
    LlrCount,
    /// An S1G width is 1, 2, 4, 8 or 16 MHz.
    S1gWidth,
    /// A BLE receiver needs 2 to 16 samples per bit.
    BleSamplesPerBit,
    /// The filter cannot be designed.
    FilterDesign,
    /// An advertising channel is 37, 38 or 39.
    BleAdvChannel,
}

impl Param {
    /// The fault in a plain sentence.
    pub fn describe(self) -> &'static str {
        match self {
            Param::PsduLengthMismatch => "the PSDU length does not match the header",
            Param::CckNotBuilt => "CCK modulation is not available",
            Param::DsssSamplesPerSymbol => "a DSSS receiver needs 11 to 64 samples per symbol",
            Param::BleChannelIndex => "a BLE channel index is 0 to 39",
            Param::BleAdvType => {
                "build_adv makes ADV_IND, ADV_NONCONN_IND, ADV_SCAN_IND or SCAN_RSP"
            }
            Param::PsduLength => "a PSDU has 1 to 4095 octets",
            Param::ScramblerSeed => "a scrambler seed is 1 to 127",
            Param::CodedBitsPerSymbol => "the input must be one symbol of coded bits",
            Param::LlrCount => "the LLRs must fill n_sym whole symbols",
            Param::S1gWidth => "an S1G width is 1, 2, 4, 8 or 16 MHz",
            Param::BleSamplesPerBit => "a BLE receiver needs 2 to 16 samples per bit",
            Param::FilterDesign => "the filter cannot be designed",
            Param::BleAdvChannel => "an advertising channel is 37, 38 or 39",
        }
    }
}

/// The way a frame or field is malformed. [`Error::Malformed`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Malformation {
    /// The protocol version is not 0.
    ProtocolVersion,
    /// An RSN suite count goes past the end.
    RsnSuiteCount,
    /// The SIGNAL value is unknown.
    UnknownSignal,
    /// An extended PDU has no header byte.
    ExtendedNoHeader,
    /// The extended header is longer than the payload.
    ExtendedHeaderLong,
    /// An extended header field goes past its end.
    ExtendedFieldPastEnd,
    /// A non-extended advertising payload is 6 to 37 bytes.
    AdvPayloadLength,
    /// An ADV_DIRECT_IND payload is 12 bytes.
    AdvDirectLength,
    /// A SCAN_REQ payload is 12 bytes.
    ScanReqLength,
    /// A CONNECT_IND payload is 34 bytes.
    ConnectIndLength,
    /// The SIGNAL reserved bit is set.
    SignalReserved,
    /// The RATE value is unknown.
    UnknownRate,
    /// The LENGTH is 0.
    ZeroLength,
    /// No scrambler state produces these bits.
    NoScramblerState,
}

impl Malformation {
    /// The fault in a plain sentence.
    pub fn describe(self) -> &'static str {
        match self {
            Malformation::ProtocolVersion => "the protocol version is not 0",
            Malformation::RsnSuiteCount => "an RSN suite count goes past the end",
            Malformation::UnknownSignal => "the SIGNAL value is unknown",
            Malformation::ExtendedNoHeader => "an extended PDU has no header byte",
            Malformation::ExtendedHeaderLong => "the extended header is longer than the payload",
            Malformation::ExtendedFieldPastEnd => "an extended header field goes past its end",
            Malformation::AdvPayloadLength => "a non-extended advertising payload is 6 to 37 bytes",
            Malformation::AdvDirectLength => "an ADV_DIRECT_IND payload is 12 bytes",
            Malformation::ScanReqLength => "a SCAN_REQ payload is 12 bytes",
            Malformation::ConnectIndLength => "a CONNECT_IND payload is 34 bytes",
            Malformation::SignalReserved => "the SIGNAL reserved bit is set",
            Malformation::UnknownRate => "the RATE value is unknown",
            Malformation::ZeroLength => "the LENGTH is 0",
            Malformation::NoScramblerState => "no scrambler state gives these bits",
        }
    }
}

/// The check a frame failed. [`Error::Check`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckKind {
    /// The frame check sequence.
    Fcs,
    /// The PLCP header CRC-16.
    PlcpCrc,
    /// The SIGNAL parity bit.
    SignalParity,
}

impl CheckKind {
    /// The fault in a plain sentence.
    pub fn describe(self) -> &'static str {
        match self {
            CheckKind::Fcs => "the frame check sequence",
            CheckKind::PlcpCrc => "the PLCP header CRC-16",
            CheckKind::SignalParity => "the SIGNAL parity bit",
        }
    }
}

/// What went wrong. Every fallible function of this crate returns this.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// A parameter is outside what the standard or the receiver allows.
    BadParameter(Param),
    /// The input is shorter than the operation needs.
    InputTooShort,
    /// The input is longer than the standard allows (a PSDU over 4095 bytes, a BLE payload over 255 bytes ...).
    TooLong,
    /// A frame or field is malformed.
    Malformed(Malformation),
    /// A frame failed its CRC, parity or tail bits.
    Check(CheckKind),
    /// An error from `sdrkit-util-bits`.
    Bits(sdrkit_util_bits::Error),
    /// An error from `sdrkit-algo-fec`.
    Fec(sdrkit_algo_fec::error::FecError),
    /// An error from `sdrkit-algo-modem`.
    Modem(sdrkit_algo_modem::ModemError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadParameter(p) => write!(f, "bad parameter: {}", p.describe()),
            Error::InputTooShort => write!(f, "input too short"),
            Error::TooLong => write!(f, "input longer than the standard allows"),
            Error::Malformed(m) => write!(f, "malformed: {}", m.describe()),
            Error::Check(m) => write!(f, "{} is wrong", m.describe()),
            Error::Bits(e) => write!(f, "bits: {e}"),
            Error::Fec(e) => write!(f, "fec: {e:?}"),
            Error::Modem(e) => write!(f, "modem: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<sdrkit_util_bits::Error> for Error {
    fn from(e: sdrkit_util_bits::Error) -> Self {
        Error::Bits(e)
    }
}

impl From<sdrkit_algo_fec::error::FecError> for Error {
    fn from(e: sdrkit_algo_fec::error::FecError) -> Self {
        Error::Fec(e)
    }
}

impl From<sdrkit_algo_modem::ModemError> for Error {
    fn from(e: sdrkit_algo_modem::ModemError) -> Self {
        Error::Modem(e)
    }
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// A sample with NaN or infinite parts replaced by zero, so one bad sample cannot poison a running sum.
#[inline]
pub(crate) fn finite(x: Complex32) -> Complex32 {
    if x.re.is_finite() && x.im.is_finite() {
        x
    } else {
        Complex32::default()
    }
}

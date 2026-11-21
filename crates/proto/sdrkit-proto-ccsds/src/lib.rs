//! CCSDS space link layers: frame synchronisation, channel decoding, transfer frames, virtual channels, packets and LRIT/HRIT files.
//!
//! Standard: CCSDS Blue Books 131.0, 132.0, 133.0, 732.0 and CGMS LRIT/HRIT Global Specification.
//!
//! # Modules
//!
//! | Module | What it holds | Standard |
//! |---|---|---|
//! | [`marker`] | the attached sync markers (TM, turbo, Proximity-1) and the telecommand start and tail sequences | CCSDS 131.0-B-5 clause 9, 211.0-B-6, 231.0-B-4 |
//! | [`randomiser`] | the TM pseudo-randomiser on bytes, hard bits and LLRs at any offset, and the TC randomiser | CCSDS 131.0-B-5 clause 10, 231.0-B-4 clause 6 |
//! | [`codesync`] | the code synchroniser and decoder of the rate 1/2, K = 7 code: the coded marker under every phase, pairing, polarity and NRZ-M hypothesis, the streaming Viterbi decoder, and a blind search by re-encoding | CCSDS 131.0-B-5 clause 3 |
//! | [`cadufind`] | byte-aligned CADUs in a byte stream: the marker at a fixed spacing, no randomiser or code (CADUs in DVB-S2 data fields) | CCSDS 131.0-B-5 clause 9 |
//! | [`framesync`] | the streaming frame synchroniser: acquire, track, flywheel and 180 degree slips, on hard bits or LLRs | CCSDS 131.0-B-5 clause 9 |
//! | [`channel`] | derandomising and interleaved Reed-Solomon of a channel frame, with the basis found automatically | CCSDS 131.0-B-5 clauses 4 and 10 |
//! | [`tm`] | TM transfer frame primary header, secondary header, OCF and FECF | CCSDS 132.0-B-3 |
//! | [`aos`] | AOS transfer frame primary header, insert zone, OCF, FECF, and the M_PDU and B_PDU headers | CCSDS 732.0-B-4 |
//! | [`demux`] | virtual channel demultiplexing with frame counter continuity (TM and AOS, the AOS cycle extension) | CCSDS 132.0 and 732.0 |
//! | [`packet`] | space packet primary header, reassembly of packets from M_PDU zones per virtual channel, packing for generators | CCSDS 133.0-B-2, 732.0-B-4 clause 4.1.4 |
//! | [`xrit`] | the LRIT/HRIT file layer: transport files, header records in the CGMS, NOAA (GOES), KMA (GK-2A) and FY-4 dialects, segments and compression flags | CGMS 03 (LRIT/HRIT Global Specification 2.6), GOES-R HRIT/EMWIN, KMA LRIT/HRIT Mission Specific Implementation |
//! | [`quaddiff`] | four-phase differential coding of uncoded QPSK: two data bits a symbol from the change of the hard bits | - |
//! | [`rice`] | the adaptive entropy coder of CCSDS 121.0 (all options, the preprocessor, restricted sets for small samples), decoder and encoder | CCSDS 121.0-B-3 |
//!
//! Mission framing that is not CCSDS stays in its family: the Meteor-M LRPT interleaver and marker search, AHRPT and FY-3
//! instrument formats, GomSpace AX100 framing and CSP headers (`sdrkit-proto-link`), and the DES and ZIP envelopes of the xRIT products.
//!
//! Soft bits are `i8` LLRs where a positive value means 0. Hard bits are `u8`, one per byte. Bytes are sent most significant
//! bit first, as the CCSDS standards number them (bit 0 is the first bit sent). Bad lengths and parameters return an [`Error`].
//! Damaged frames and packets are counted and skipped.
//!
//! # Example: from a coded bit stream to packets
//!
//! ```
//! use sdrkit_proto_ccsds::{aos, channel, framesync, packet};
//!
//! // The transmit side: one packet in an AOS frame, Reed-Solomon coded (I = 4), randomised, behind the marker.
//! let mut pkt = Vec::new();
//! packet::build(&packet::PrimaryHeader::telemetry(0x123, packet::SeqFlags::Unsegmented, 7), b"hello", &mut pkt).unwrap();
//! let layout = aos::Layout::RS_I4;
//! let mut zone = vec![0u8; layout.data_zone_len()];
//! let fh = packet::fill_zone(&pkt, &mut zone).unwrap();
//! let hdr = aos::PrimaryHeader { spacecraft: 42, vcid: 3, ..aos::PrimaryHeader::default() };
//! let mut frame = vec![0u8; layout.frame_len];
//! layout.build(&hdr, &[], fh, &zone, &mut frame).unwrap();
//! let mut coder = channel::ChannelCoder::new(channel::ChannelConfig::rs_223(4)).unwrap();
//! let mut cadu = Vec::new();
//! coder.encode(&frame, &mut cadu).unwrap();
//! let bits: Vec<u8> = cadu.iter().flat_map(|b| (0..8).rev().map(move |k| (b >> k) & 1)).collect();
//!
//! // The receive side: frame sync, channel decoding, packet reassembly.
//! let mut sync = framesync::FrameSync::new(framesync::SyncConfig::cadu(1020)).unwrap();
//! let mut dec = channel::ChannelDecoder::new(channel::ChannelConfig::rs_223(4)).unwrap();
//! let mut asm = packet::Assembler::new(packet::AssemblerConfig::default());
//! let mut body = vec![0u8; 1020];
//! let mut got = Vec::new();
//! sync.push_bits(&bits, |f| {
//!     body.copy_from_slice(f.bytes);
//!     let d = dec.decode(&mut body).unwrap();
//!     assert!(d.ok());
//!     let v = layout.parse(&body[..d.frame_len]).unwrap();
//!     asm.push_mpdu(v.header.vcid, Some(v.header.counter), v.first_header, v.zone, |p| got.push(p.data.to_vec()));
//! });
//! assert_eq!(got, vec![b"hello".to_vec()]);
//! ```
#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod aos;
pub mod cadufind;
pub mod channel;
pub mod codesync;
pub mod demux;
pub mod framesync;
pub mod marker;
pub mod multisync;
pub mod packet;
pub mod quaddiff;
pub mod randomiser;
pub mod rice;
pub mod tm;
pub mod wordsync;
pub mod xrit;

/// A parameter that was outside its allowed range. [`Error::BadParameter`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// Frame bytes must be 1 to 2^20.
    FrameBytes,
    /// Track errors must be below half the marker bits.
    TrackErrors,
    /// Interleave depth must be 1 to 8.
    InterleaveDepth,
    /// Virtual fill must be below k.
    VirtualFill,
    /// Uncoded frame length must be at least 1.
    UncodedFrameLength,
    /// Blind search window must be at least 64 steps.
    BlindSearchWindow,
    /// TM secondary header must be at most 64 bytes.
    TmSecondaryHeader,
    /// Frame count bits must be 1 to 32.
    FrameCountBits,
    /// Rice bits per sample must be 1 to 32.
    RiceBitsPerSample,
    /// Rice block size must be 8, 16, 32 or 64.
    RiceBlockSize,
    /// Rice sample interval (RSI) must be 1 to 4096 blocks.
    RiceSampleInterval,
    /// Idle packet length must be 7 to 65,542.
    IdlePacketLength,
    /// Zone length must be 1 to 2046.
    ZoneLength,
    /// Code sync frame bits must be 64 to 2^20.
    CodeSyncFrameBits,
    /// Code sync errors must be below 26 of 52.
    CodeSyncErrors,
    /// Marker errors must be below half the marker bits.
    MarkerErrors,
    /// Soft marker threshold must be 0 to 1.
    SoftMarkerThreshold,
}

impl Param {
    /// The refusal in a plain sentence.
    pub fn describe(self) -> &'static str {
        match self {
            Param::FrameBytes => "frame bytes must be 1 to 2^20",
            Param::TrackErrors => "track errors must be below half the marker bits",
            Param::InterleaveDepth => "interleave depth must be 1 to 8",
            Param::VirtualFill => "virtual fill must be below k",
            Param::UncodedFrameLength => "uncoded frame length must be at least 1",
            Param::BlindSearchWindow => "blind search window must be at least 64 steps",
            Param::TmSecondaryHeader => "TM secondary header must be at most 64 bytes",
            Param::FrameCountBits => "frame count bits must be 1 to 32",
            Param::RiceBitsPerSample => "Rice bits per sample must be 1 to 32",
            Param::RiceBlockSize => "Rice block size must be 8, 16, 32 or 64",
            Param::RiceSampleInterval => "Rice sample interval (RSI) must be 1 to 4096 blocks",
            Param::IdlePacketLength => "idle packet length must be 7 to 65,542",
            Param::ZoneLength => "zone length must be 1 to 2046",
            Param::CodeSyncFrameBits => "code sync frame bits must be 64 to 2^20",
            Param::CodeSyncErrors => "code sync errors must be below 26 of 52",
            Param::MarkerErrors => "marker errors must be below half the marker bits",
            Param::SoftMarkerThreshold => "soft marker threshold must be 0 to 1",
        }
    }
}

/// A header field that holds a value the standard does not allow. [`Error::BadField`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeaderField {
    /// A TM header field is wider than its place.
    TmHeader,
    /// An AOS header field is wider than its place.
    AosHeader,
    /// A space packet header field is wider than its place.
    SpacePacketHeader,
    /// A Rice zero-block run goes past the end.
    RiceZeroRun,
    /// The xRIT primary header must have type 0 and length 16.
    XritPrimaryHeader,
    /// The xRIT total header length is wrong.
    XritTotalLength,
    /// An xRIT record goes past the header length.
    XritRecordPast,
    /// An xRIT record has a wrong length.
    XritRecordLength,
}

impl HeaderField {
    /// The refusal in a plain sentence.
    pub fn describe(self) -> &'static str {
        match self {
            HeaderField::TmHeader => "a TM header field is wider than its place",
            HeaderField::AosHeader => "an AOS header field is wider than its place",
            HeaderField::SpacePacketHeader => "a space packet header field is wider than its place",
            HeaderField::RiceZeroRun => "a Rice zero-block run goes past the end",
            HeaderField::XritPrimaryHeader => {
                "the xRIT primary header must have type 0 and length 16"
            }
            HeaderField::XritTotalLength => "the xRIT total header length is wrong",
            HeaderField::XritRecordPast => "an xRIT record goes past the header length",
            HeaderField::XritRecordLength => "an xRIT record has a wrong length",
        }
    }
}

/// What went wrong. Every fallible function in this crate returns this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// An input is shorter than the operation needs.
    InputTooShort {
        /// Bytes (or items) needed.
        need: usize,
    },
    /// An output buffer is too short.
    OutputTooShort {
        /// Bytes (or items) needed.
        need: usize,
    },
    /// An input has a length the operation does not allow (a frame of the wrong size).
    BadLength,
    /// A parameter is outside its allowed range.
    BadParameter(Param),
    /// A header field holds a value the standard does not allow.
    BadField(HeaderField),
    /// A lower layer refused (the Reed-Solomon or Viterbi decoder set up with impossible parameters).
    Fec(sdrkit_algo_fec::error::FecError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InputTooShort { need } => write!(f, "input too short: {need} needed"),
            Error::OutputTooShort { need } => write!(f, "output too short: {need} needed"),
            Error::BadLength => write!(f, "input has the wrong length"),
            Error::BadParameter(p) => write!(f, "bad parameter: {}", p.describe()),
            Error::BadField(p) => write!(f, "bad header field: {}", p.describe()),
            Error::Fec(e) => write!(f, "error correction: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<sdrkit_algo_fec::error::FecError> for Error {
    fn from(e: sdrkit_algo_fec::error::FecError) -> Self {
        Error::Fec(e)
    }
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Reads a big-endian 16-bit field (the caller made sure that the length is correct).
#[inline]
pub(crate) fn be16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

/// Reads a big-endian 32-bit field (the caller made sure that the length is correct).
#[inline]
pub(crate) fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

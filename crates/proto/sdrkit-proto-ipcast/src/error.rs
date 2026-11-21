//! The one error type of this crate.

/// What went wrong. Every fallible function of this crate returns this. No function panics on bad input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The input is shorter than the structure it must hold.
    #[error("the input is too short")]
    Short,
    /// A field holds a value that the standard does not allow.
    #[error("invalid {0:?}")]
    Invalid(Malformed),
    /// A CRC or checksum is wrong.
    #[error("bad {0:?}")]
    Check(Check),
    /// A length is larger than the limit of the standard or of this receiver.
    #[error("{0:?} is too large")]
    TooLarge(Limit),
    /// The input uses a feature that this crate does not decode.
    #[error("{0:?} is not supported")]
    Unsupported(Feature),
}

/// A field or structure that its standard does not allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// An ALP length of 0.
    AlpLength,
    /// The extension headers of an ATSC 3.0 baseband packet.
    BbpExtensionHeaders,
    /// The extension length of an ATSC 3.0 baseband packet.
    BbpExtensionLength,
    /// The component lengths of a concatenated ALP packet.
    ComponentLengths,
    /// A counter length other than 1 or 2 bytes.
    CounterLength,
    /// A counter value that does not fit its length.
    Counter,
    /// DEFLATE data (RFC 1951).
    Deflate,
    /// A DFL that is not whole bytes.
    Dfl,
    /// A DNP field.
    Dnp,
    /// An IPv4 fragment length.
    FragmentLength,
    /// An IPv4 fragment offset.
    FragmentOffset,
    /// The FLG byte of a gzip member.
    GzipFlags,
    /// The header of a gzip member.
    GzipHeader,
    /// The H-LEN of a GSE extension header.
    HLen,
    /// The HDR_LEN of an LCT header.
    HdrLen,
    /// The length of an LCT header extension.
    HeaderExtensionLength,
    /// The HEL of an ALP header extension.
    Hel,
    /// An init segment without an mdhd box.
    InitNoMdhd,
    /// An init segment without a moov box.
    InitNoMoov,
    /// An init segment without an stsd box.
    InitNoStsd,
    /// An init segment without a tkhd box.
    InitNoTkhd,
    /// An init segment without a trak box.
    InitNoTrak,
    /// The version of an IP header.
    IpVersion,
    /// The IHL and total length of an IPv4 header.
    Ipv4Lengths,
    /// The version of an LCT header.
    LctVersion,
    /// A media segment without a moof box.
    MediaNoMoof,
    /// A MIME multipart boundary.
    MimeBoundary,
    /// A MIME Content-Type.
    MimeContentType,
    /// The headers of a MIME part.
    MimeHeaders,
    /// An MTU too small for the headers.
    Mtu,
    /// The NUMTS of an ALP TS packet.
    Numts,
    /// A packet type that the writer does not write (a TS packet goes through `write_ts`).
    PacketType,
    /// A PCAP time that is negative or not finite.
    PcapTime,
    /// The pointer of a baseband packet.
    Pointer,
    /// A sample that lies outside its segment.
    SampleOutsideSegment,
    /// The sequence of segments of an ALP packet.
    SegmentSequence,
    /// A SYNCD that is not whole bytes.
    SyncdNotWholeBytes,
    /// A SYNCD past the data field.
    SyncdPastDataField,
    /// A traf box without a tfhd box.
    TrafNoTfhd,
    /// A TS/GS field that is not a packetised stream.
    TsGs,
    /// The length of a UDP header.
    UdpLength,
    /// A UPL that does not hold the ISSY field.
    UplWithIssy,
    /// A UPL.
    Upl,
}

/// A check value that did not match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// The CRC-8 of a BBHEADER (EN 302 307-1 clause 5.1.6).
    BbHeaderCrc8,
    /// The CRC-32 of a gzip member (RFC 1952 clause 2.3.1).
    GzipCrc32,
    /// The header CRC-16 of a gzip member (RFC 1952 clause 2.3.1).
    GzipHeaderCrc16,
    /// The ISIZE of a gzip member (RFC 1952 clause 2.3.1).
    GzipIsize,
    /// The IPv4 header checksum (RFC 791).
    Ipv4HeaderChecksum,
}

/// What was larger than the limit of its standard or of this receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    /// An ALP payload.
    AlpPayload,
    /// A chain of IPv6 or GSE extension headers.
    ExtensionHeaderChain,
    /// Gzip data or what it inflates to.
    GzipData,
    /// An IPv4 packet.
    Ipv4Packet,
    /// Padding.
    Padding,
    /// A GSE PDU.
    Pdu,
    /// A ROUTE object.
    RouteObject,
    /// A segmented ALP packet.
    SegmentedPacket,
    /// A datagram section (MPE).
    Section,
    /// An LCT transfer length.
    TransferLength,
    /// A UDP datagram.
    UdpDatagram,
}

/// A feature of a standard that this crate does not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// High efficiency mode for a generic packetised stream.
    HemGenericStream,
    /// A mandatory GSE extension header that this crate does not know.
    MandatoryExtensionHeader,
    /// A GSE test SNDU.
    TestSndu,
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;

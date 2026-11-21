//! The error of this crate: what is malformed in a section, an MPEG-4 systems structure or a test multiplex.

use std::fmt;

/// Why a section, a structure or a test multiplex is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A section is refused.
    Section(SectionFault),
    /// An MPEG-4 systems structure is malformed.
    Mpeg4(Mpeg4Fault),
    /// A test multiplex cannot be made.
    TestMux(MuxFault),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Section(x) => write!(f, "section: {x}"),
            Error::Mpeg4(x) => write!(f, "MPEG-4 systems: {x}"),
            Error::TestMux(x) => write!(f, "test multiplex: {x}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<SectionFault> for Error {
    fn from(x: SectionFault) -> Self {
        Error::Section(x)
    }
}

impl From<Mpeg4Fault> for Error {
    fn from(x: Mpeg4Fault) -> Self {
        Error::Mpeg4(x)
    }
}

impl From<MuxFault> for Error {
    fn from(x: MuxFault) -> Self {
        Error::TestMux(x)
    }
}

/// What is wrong with a section. Standard: ISO/IEC 13818-1 2.4.4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionFault {
    /// It is shorter than its table allows.
    Short,
    /// It is longer than allowed, or its length runs past the data.
    Length,
    /// Its CRC_32 is wrong.
    Crc,
    /// Its table_id is not one that the PID carries.
    TableId,
}

impl fmt::Display for SectionFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SectionFault::Short => "shorter than its table allows",
            SectionFault::Length => "its length is wrong",
            SectionFault::Crc => "bad CRC",
            SectionFault::TableId => "a table_id that the PID does not carry",
        })
    }
}

/// What is wrong with an MPEG-4 systems structure. Standard: ISO/IEC 14496-1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mpeg4Fault {
    /// The data ends inside a field or a descriptor.
    Short,
    /// A descriptor has another tag than the one expected.
    Tag {
        /// The tag expected.
        expected: u8,
        /// The tag found.
        found: u8,
    },
    /// A size field is longer than four bytes, or a size is too large to write (8.3.3).
    Size,
    /// A field holds a value that the standard does not allow.
    Field(Mpeg4Field),
    /// A predefined SL configuration that is not defined here: 2 is for MP4 files only, 3 to 255 are reserved.
    Predefined(u8),
}

impl fmt::Display for Mpeg4Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mpeg4Fault::Short => write!(f, "the data ends inside a field"),
            Mpeg4Fault::Tag { expected, found } => {
                write!(f, "descriptor tag 0x{found:02X}, expected 0x{expected:02X}")
            }
            Mpeg4Fault::Size => write!(f, "a size field is longer than four bytes"),
            Mpeg4Fault::Field(field) => {
                write!(
                    f,
                    "{} holds a value the standard does not allow",
                    field.name()
                )
            }
            Mpeg4Fault::Predefined(v) => {
                write!(f, "predefined SL configuration {v} is not supported")
            }
        }
    }
}

/// A field of an MPEG-4 systems structure, by its name in ISO/IEC 14496-1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mpeg4Field {
    /// `bufferSizeDB` of a DecoderConfigDescriptor.
    BufferSizeDb,
    /// The tag of an object descriptor command.
    CommandTag,
    /// The DecoderConfigDescriptor of an ES_Descriptor.
    DecoderConfigDescriptor,
    /// The InitialObjectDescriptor of an IOD descriptor.
    InitialObjectDescriptor,
    /// `instantBitrateLength` of an SLConfigDescriptor.
    InstantBitrateLength,
    /// `OCRLength` of an SLConfigDescriptor.
    OcrLength,
    /// `AU_Length` of an SLConfigDescriptor.
    AuLength,
    /// `AU_seqNumLength` of an SLConfigDescriptor.
    AuSeqNumLength,
    /// `packetSeqNumLength` of an SLConfigDescriptor.
    PacketSeqNumLength,
    /// `ObjectDescriptorID` of an object descriptor.
    ObjectDescriptorId,
    /// `section_syntax_indicator` of a 14496 section.
    SectionSyntaxIndicator,
    /// The SLConfigDescriptor of an ES_Descriptor.
    SlConfigDescriptor,
    /// `streamPriority` of an ES_Descriptor.
    StreamPriority,
    /// `streamType` of a DecoderConfigDescriptor.
    StreamType,
    /// `table_id` of a 14496 section.
    TableId,
    /// `timeStampLength` of an SLConfigDescriptor.
    TimeStampLength,
    /// `URLlength` of a descriptor.
    UrlLength,
}

impl Mpeg4Field {
    /// The name of the field in ISO/IEC 14496-1.
    pub fn name(self) -> &'static str {
        match self {
            Mpeg4Field::BufferSizeDb => "bufferSizeDB",
            Mpeg4Field::CommandTag => "command tag",
            Mpeg4Field::DecoderConfigDescriptor => "DecoderConfigDescriptor",
            Mpeg4Field::InitialObjectDescriptor => "InitialObjectDescriptor",
            Mpeg4Field::InstantBitrateLength => "instantBitrateLength",
            Mpeg4Field::OcrLength => "OCRLength",
            Mpeg4Field::AuLength => "AU_Length",
            Mpeg4Field::AuSeqNumLength => "AU_seqNumLength",
            Mpeg4Field::PacketSeqNumLength => "packetSeqNumLength",
            Mpeg4Field::ObjectDescriptorId => "ObjectDescriptorID",
            Mpeg4Field::SectionSyntaxIndicator => "section_syntax_indicator",
            Mpeg4Field::SlConfigDescriptor => "SLConfigDescriptor",
            Mpeg4Field::StreamPriority => "streamPriority",
            Mpeg4Field::StreamType => "streamType",
            Mpeg4Field::TableId => "table_id",
            Mpeg4Field::TimeStampLength => "timeStampLength",
            Mpeg4Field::UrlLength => "URLlength",
        }
    }
}

/// Why a test multiplex cannot be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuxFault {
    /// No service was given.
    NoServices,
    /// Two services have the same service_id.
    DuplicateService(u16),
    /// The data PID is reserved (0x0000 to 0x000F, 0x1FFF) or a table uses it.
    BadDataPid(u16),
    /// The tables need more packets than one repetition period holds.
    PeriodTooShort {
        /// Packets the tables need.
        need: usize,
    },
    /// `data_every` is 0.
    BadDataEvery,
    /// A text (service, provider or network name) is longer than 255 bytes.
    TextTooLong,
    /// The sound PID is reserved (0x0000 to 0x000F, 0x0011, 0x1FFF), or the data stream or a table uses it.
    BadSoundPid(u16),
    /// The bit rate of the multiplex is too low to carry the sound.
    BadSoundRate,
}

impl fmt::Display for MuxFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MuxFault::NoServices => write!(f, "a test multiplex needs at least one service"),
            MuxFault::DuplicateService(s) => write!(f, "service_id {s} is given twice"),
            MuxFault::BadDataPid(p) => write!(f, "PID 0x{p:04X} cannot carry the data stream"),
            MuxFault::PeriodTooShort { need } => {
                write!(f, "the tables need {need} packets, more than the period")
            }
            MuxFault::BadDataEvery => write!(f, "data_every must be 1 or more"),
            MuxFault::TextTooLong => write!(f, "a name is longer than 255 bytes"),
            MuxFault::BadSoundPid(p) => write!(f, "PID 0x{p:04X} cannot carry the sound"),
            MuxFault::BadSoundRate => write!(f, "the bit rate of the multiplex is too low for the sound"),
        }
    }
}

//! The error type of every fallible function in this crate.

/// What went wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The input is shorter than its own header or length fields say.
    #[error("input shorter than its header says")]
    Truncated,
    /// A CRC is not correct.
    #[error("CRC failed")]
    Crc,
    /// A field holds a value that its standard does not allow.
    #[error("invalid {0:?}")]
    Invalid(Malformed),
    /// A parameter given to a builder is out of range.
    #[error("bad parameter: {0:?}")]
    BadParameter(Parameter),
    /// The Reed-Solomon coder refused a word.
    #[error("Reed-Solomon coding: {0}")]
    Fec(#[from] sdrkit_algo_fec::FecError),
}

/// A field or structure that its standard does not allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// A user access field whose transport id flag is set but whose length is below 2 (EN 300 401 clause 5.3.3.2).
    UserAccess,
    /// A useful data length longer than the packet data field (EN 300 401 clause 5.3.2).
    UsefulDataLength,
    /// A compressed MOT directory (EN 301 234 clause 7.2.4), which this crate does not read.
    CompressedDirectory,
    /// A DAB+ superframe header with PS but without SBR (TS 102 563 clause 5.2).
    PsWithoutSbr,
    /// A MOT DirectorySize below 13 bytes.
    DirectorySize,
    /// A DL Plus command other than the tags command.
    DlPlusCommand,
    /// A dynamic label command that the standard does not define.
    LabelCommand,
    /// The extensionFlag of a GASpecificConfig.
    GaExtensionFlag,
    /// Compressed Journaline content that does not inflate.
    JournalineCompressed,
    /// A Journaline object type that TS 102 979 does not define.
    JournalineObjectType,
    /// A MOT HeaderSize below 7 bytes.
    HeaderSize,
    /// The hours or minutes of a MOT time.
    MotTime,
    /// A dynamic label command that is not DL Plus.
    NotDlPlus,
    /// A MOT directory whose NumberOfObjects differs from its entries.
    NumberOfObjects,
    /// A samplingFrequencyIndex that ISO/IEC 14496-3 does not define.
    SamplingFrequencyIndex,
    /// SPI elements nested deeper than this crate reads.
    SpiNesting,
}

/// A builder parameter outside its range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parameter {
    /// A label that does not fit 1 to 8 segments of 16 bytes.
    LabelLength,
    /// An ADTS frame above 8191 bytes.
    AdtsFrameLength,
    /// A sampling rate that ADTS cannot signal.
    AdtsRate,
    /// A channel configuration or an audio object type that the writer does not write.
    ChannelConfig,
    /// A DAB packet address outside 1 to 1021.
    PacketAddress,
    /// A data group too long for its header.
    DataGroupLength,
    /// A DL Plus tag count outside 1 to 4.
    DlPlusTagCount,
    /// A DRM packet id outside 0 to 3, or a length outside 4 to 258.
    DrmPacket,
    /// An empty data unit.
    EmptyDataUnit,
    /// A LOAS frame above 8191 bytes.
    LoasFrameLength,
    /// A MOT directory field outside its range.
    DirectoryField,
    /// A MOT header core field outside its range.
    HeaderCoreField,
    /// PS without SBR.
    PsNeedsSbr,
    /// A sampling rate that the writer does not write.
    SamplingRate,
    /// A segment number above 32767.
    SegmentNumber,
    /// A user access field too long for its header.
    UserAccess,
    /// A data group type, continuity index or repetition index above 15.
    DataGroupHeader,
    /// A MOT parameter id above 63, or parameter data above 32767 bytes.
    MotParameter,
    /// A segment above 8191 bytes, or a repetition count above 7.
    SegmentHeader,
    /// 960-sample frames in ADTS, which has no field for them (LOAS carries them).
    AdtsFrame960,
    /// An ADTS object type outside 1 to 4, or a channel configuration outside 0 to 7.
    AdtsObjectType,
    /// An application data table that is not 2256 bytes.
    ApplicationDataTable,
}

/// `Result` with this crate's [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

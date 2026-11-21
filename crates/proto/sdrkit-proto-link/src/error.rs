//! The error type of the crate.

use sdrkit_algo_dsp::DspError;
use sdrkit_algo_fec::FecError;
use sdrkit_algo_modem::ModemError;
use sdrkit_algo_sim::SimError;
use std::fmt;

/// A field or part of a frame or text that does not have the layout of its standard. [`Error::Syntax`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// Address character.
    AddressChar,
    /// Address character (extension bit set).
    AddressExtension,
    /// Callsign.
    Callsign,
    /// D-PRS CRC.
    DprsCrc,
    /// D-PRS header.
    DprsHeader,
    /// Empty callsign.
    EmptyCallsign,
    /// Item name.
    ItemName,
    /// Item position.
    ItemPosition,
    /// Locator.
    Locator,
    /// Locator beacon.
    LocatorBeacon,
    /// Message addressee.
    MessageAddressee,
    /// Mic-E ambiguity.
    MiceAmbiguity,
    /// Mic-E destination.
    MiceDestination,
    /// Mic-E latitude.
    MiceLatitude,
    /// Mic-E longitude.
    MiceLongitude,
    /// Mic-E speed and course.
    MiceSpeedCourse,
    /// Object name.
    ObjectName,
    /// Object position.
    ObjectPosition,
    /// Address field (one address).
    OneAddress,
    /// Position.
    Position,
    /// RS word length.
    RsWordLength,
    /// Modulo-128 S control field.
    SControl128,
    /// SSID.
    Ssid,
    /// Telemetry.
    Telemetry,
    /// Third-party header.
    ThirdPartyHeader,
    /// Time stamp.
    TimeStamp,
    /// TNC2 header.
    Tnc2Header,
    /// Address field (more than 10 addresses).
    TooManyAddresses,
    /// Weather time stamp.
    WeatherTimeStamp,
    /// XID format or group identifier.
    Xid,
}

impl Part {
    /// The name in plain words.
    pub fn name(self) -> &'static str {
        match self {
            Part::AddressChar => "address character",
            Part::AddressExtension => "address character (extension bit set)",
            Part::Callsign => "callsign",
            Part::DprsCrc => "D-PRS CRC",
            Part::DprsHeader => "D-PRS header",
            Part::EmptyCallsign => "empty callsign",
            Part::ItemName => "item name",
            Part::ItemPosition => "item position",
            Part::Locator => "locator",
            Part::LocatorBeacon => "locator beacon",
            Part::MessageAddressee => "message addressee",
            Part::MiceAmbiguity => "Mic-E ambiguity",
            Part::MiceDestination => "Mic-E destination",
            Part::MiceLatitude => "Mic-E latitude",
            Part::MiceLongitude => "Mic-E longitude",
            Part::MiceSpeedCourse => "Mic-E speed and course",
            Part::ObjectName => "object name",
            Part::ObjectPosition => "object position",
            Part::OneAddress => "address field (one address)",
            Part::Position => "position",
            Part::RsWordLength => "RS word length",
            Part::SControl128 => "modulo-128 S control field",
            Part::Ssid => "SSID",
            Part::Telemetry => "telemetry",
            Part::ThirdPartyHeader => "third-party header",
            Part::TimeStamp => "time stamp",
            Part::Tnc2Header => "TNC2 header",
            Part::TooManyAddresses => "address field (more than 10 addresses)",
            Part::WeatherTimeStamp => "weather time stamp",
            Part::Xid => "XID format or group identifier",
        }
    }
}

/// A parameter that a call refused. [`Error::BadParameter`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// Addressee: 1 to 9 ASCII characters.
    Addressee,
    /// Position ambiguity: 0 to 4.
    Ambiguity,
    /// Compressed position.
    CompressedPosition,
    /// Course or speed.
    CourseOrSpeed,
    /// HDLC frame lengths: min above max.
    FrameLengths,
    /// KISS port: 0 to 15.
    KissPort,
    /// Message number: 1 to 5 letters or digits.
    MessageNumber,
    /// Message text: up to 67 characters, no | ~ {.
    MessageText,
    /// Mode 5 with convolutional coding.
    Mode5Convolutional,
    /// A PID goes with I and UI frames only.
    Pid,
    /// Position.
    Position,
    /// Sequence number.
    SequenceNumber,
    /// Sync word: 8 to 128 bits, and a frame length above 0.
    SyncWord,
}

impl Param {
    /// The parameter and its limits in plain words.
    pub fn describe(self) -> &'static str {
        match self {
            Param::Addressee => "addressee: 1 to 9 ASCII characters",
            Param::Ambiguity => "position ambiguity: 0 to 4",
            Param::CompressedPosition => "compressed position",
            Param::CourseOrSpeed => "course or speed",
            Param::FrameLengths => "HDLC frame lengths: min above max",
            Param::KissPort => "KISS port: 0 to 15",
            Param::MessageNumber => "message number: 1 to 5 letters or digits",
            Param::MessageText => "message text: up to 67 characters, no | ~ {",
            Param::Mode5Convolutional => "Mode 5 with convolutional coding",
            Param::Pid => "a PID goes with I and UI frames only",
            Param::Position => "position",
            Param::SequenceNumber => "sequence number",
            Param::SyncWord => "sync word: 8 to 128 bits, and a frame length above 0",
        }
    }
}

/// What went wrong. Every function that can fail on bad input returns one of these. Nothing panics on bad input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    /// The input is shorter than the structure needs.
    Short,
    /// The input is longer than the structure allows.
    TooLong,
    /// A field does not have the layout its standard provides.
    Syntax(Part),
    /// A parameter is outside its allowed range.
    BadParameter(Param),
    /// A modem cannot work at this audio rate.
    Rate {
        /// The lowest rate in samples per second.
        min: f64,
        /// The highest rate in samples per second.
        max: f64,
        /// The rate given.
        got: f64,
    },
    /// A filter of the modem cannot be made.
    Dsp(DspError),
    /// A demodulator part of the modem cannot be made.
    Modem(ModemError),
    /// The transmit modulator cannot be made.
    Sim(SimError),
    /// A code cannot be made or used.
    Fec(FecError),
    /// Error correction cannot repair the input.
    Uncorrectable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Short => f.write_str("input too short"),
            Error::TooLong => f.write_str("input too long"),
            Error::Syntax(p) => write!(f, "bad {}", p.name()),
            Error::BadParameter(p) => write!(f, "bad parameter: {}", p.describe()),
            Error::Rate { min, max, got } => {
                write!(f, "the audio rate is {got}, outside {min} to {max}")
            }
            Error::Dsp(e) => e.fmt(f),
            Error::Modem(e) => e.fmt(f),
            Error::Sim(e) => e.fmt(f),
            Error::Fec(e) => e.fmt(f),
            Error::Uncorrectable => f.write_str("too many errors to correct"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Dsp(e) => Some(e),
            Error::Modem(e) => Some(e),
            Error::Sim(e) => Some(e),
            Error::Fec(e) => Some(e),
            _ => None,
        }
    }
}

/// A component whose modem cannot be made refuses the format with this error.
impl From<Error> for sdrkit_core::ConfigError {
    fn from(e: Error) -> Self {
        sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
    }
}

/// The crate's result type.
pub type Result<T> = std::result::Result<T, Error>;

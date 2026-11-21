//! The error type.

use std::fmt;

/// A field of a message or a value that a call refused. [`Error::BadField`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// CPDLC header or element.
    CpdlcHeaderOrElement,
    /// Ground address.
    GroundAddress,
    /// Aircraft address.
    AircraftAddress,
    /// Pressure altitude.
    PressureAltitude,
    /// True airspeed.
    TrueAirspeed,
    /// Mach number.
    MachNumber,
    /// Direction.
    Direction,
    /// Speed.
    Speed,
    /// Position.
    Position,
    /// ADS-C downlink tag.
    AdscDownlinkTag,
    /// Latitude or longitude.
    LatitudeOrLongitude,
    /// Angle.
    Angle,
    /// Negative acknowledgement extension.
    NegativeAckExtension,
    /// Noncompliance groups.
    NoncomplianceGroups,
    /// Accuracy.
    Accuracy,
    /// ICAO address.
    IcaoAddress,
    /// Reporting interval.
    ReportingInterval,
    /// Modulus tag.
    ModulusTag,
    /// ADS-C request tag.
    AdscRequestTag,
    /// Time.
    Time,
    /// Label.
    Label,
    /// Mode.
    Mode,
    /// Tak or block id.
    TakOrBlockId,
    /// Altitude.
    Altitude,
    /// Time stamp.
    TimeStamp,
    /// Time to go.
    TimeToGo,
    /// Vertical rate.
    VerticalRate,
    /// Wind speed.
    WindSpeed,
    /// Temperature.
    Temperature,
    /// Distance.
    Distance,
    /// Projected time.
    ProjectedTime,
}

impl Item {
    /// The name in plain words.
    pub fn name(self) -> &'static str {
        match self {
            Item::CpdlcHeaderOrElement => "CPDLC header or element",
            Item::GroundAddress => "ground address",
            Item::AircraftAddress => "aircraft address",
            Item::PressureAltitude => "pressure altitude",
            Item::TrueAirspeed => "true airspeed",
            Item::MachNumber => "Mach number",
            Item::Direction => "direction",
            Item::Speed => "speed",
            Item::Position => "position",
            Item::AdscDownlinkTag => "ADS-C downlink tag",
            Item::LatitudeOrLongitude => "latitude or longitude",
            Item::Angle => "angle",
            Item::NegativeAckExtension => "negative acknowledgement extension",
            Item::NoncomplianceGroups => "noncompliance groups",
            Item::Accuracy => "accuracy",
            Item::IcaoAddress => "ICAO address",
            Item::ReportingInterval => "reporting interval",
            Item::ModulusTag => "modulus tag",
            Item::AdscRequestTag => "ADS-C request tag",
            Item::Time => "time",
            Item::Label => "label",
            Item::Mode => "mode",
            Item::TakOrBlockId => "tak or block id",
            Item::Altitude => "altitude",
            Item::TimeStamp => "time stamp",
            Item::TimeToGo => "time to go",
            Item::VerticalRate => "vertical rate",
            Item::WindSpeed => "wind speed",
            Item::Temperature => "temperature",
            Item::Distance => "distance",
            Item::ProjectedTime => "projected time",
        }
    }
}

/// A framing byte of an ACARS block. [`Error::Framing`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FramingByte {
    /// SOH.
    Soh,
    /// ETX or ETB.
    EtxOrEtb,
    /// STX.
    Stx,
}

impl FramingByte {
    /// The name in plain words.
    pub fn name(self) -> &'static str {
        match self {
            FramingByte::Soh => "SOH",
            FramingByte::EtxOrEtb => "ETX or ETB",
            FramingByte::Stx => "STX",
        }
    }
}

/// What went wrong. Every parser and builder returns this instead of panicking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The input ends before the end of the item.
    Short,
    /// The input is longer than the standard allows (the limit is given).
    TooLong(usize),
    /// A framing byte is missing or wrong.
    Framing(FramingByte),
    /// A character that the field cannot hold.
    BadChar(u8),
    /// A field holds a value the standard does not define.
    BadField(Item),
    /// Hex text with an odd length or a character that is not a hex digit.
    BadHex,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Short => write!(f, "input too short"),
            Error::TooLong(n) => write!(f, "input longer than {n}"),
            Error::Framing(w) => write!(f, "framing: {}", w.name()),
            Error::BadChar(c) => write!(f, "character 0x{c:02X} not allowed here"),
            Error::BadField(w) => write!(f, "bad value in {}", w.name()),
            Error::BadHex => write!(f, "bad hex text"),
        }
    }
}

impl std::error::Error for Error {}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;

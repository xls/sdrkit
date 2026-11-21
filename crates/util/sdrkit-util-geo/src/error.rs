//! The one error type of the crate.

use std::fmt;

/// A value that a call refused because it is out of range. [`Error::OutOfRange`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Quantity {
    /// Angle.
    Angle,
    /// Bit count.
    BitCount,
    /// Bits or full scale.
    BitsOrFullScale,
    /// CPR field wider than the format.
    CprFieldWiderThanFormat,
    /// CPR field width.
    CprFieldWidth,
    /// CPR zone span.
    CprZoneSpan,
    /// Day.
    Day,
    /// Day count.
    DayCount,
    /// Day of year.
    DayOfYear,
    /// Decimals.
    Decimals,
    /// Decoded latitude.
    DecodedLatitude,
    /// Elements.
    Elements,
    /// Epoch day.
    EpochDay,
    /// Fill bits.
    FillBits,
    /// Fill bits longer than the payload.
    FillBitsPastPayload,
    /// Fragment numbers.
    FragmentNumbers,
    /// GLONASS N4 or NT.
    GlonassDayFields,
    /// GLONASS time before 1996 or too far ahead.
    GlonassTime,
    /// Inclination.
    Inclination,
    /// Latitude.
    Latitude,
    /// A leap second takes effect at a UTC midnight.
    LeapSecondNotAtMidnight,
    /// A new leap second must follow the last one and change the offset by one.
    LeapSecondOrder,
    /// Locator length.
    LocatorLength,
    /// Longitude.
    Longitude,
    /// Mean motion.
    MeanMotion,
    /// Minutes.
    Minutes,
    /// MJD.
    Mjd,
    /// Month.
    Month,
    /// Nanoseconds.
    Nanoseconds,
    /// The orbit is not closed.
    OrbitNotClosed,
    /// Radial trajectory.
    RadialTrajectory,
    /// Second.
    Second,
    /// Seconds.
    Seconds,
    /// Sequential message id.
    SequentialMessageId,
    /// Sequential message id or fill bits.
    SequentialMessageIdOrFillBits,
    /// 6-bit value.
    SixBitValue,
    /// State vector.
    StateVector,
    /// Time.
    Time,
    /// Time before the epoch of the time scale.
    TimeBeforeEpoch,
    /// Time of day.
    TimeOfDay,
    /// Truncated week.
    TruncatedWeek,
    /// UTC offset.
    UtcOffset,
    /// Week.
    Week,
    /// Year.
    Year,
}

impl Quantity {
    /// The name in plain words.
    pub fn name(self) -> &'static str {
        match self {
            Quantity::Angle => "angle",
            Quantity::BitCount => "bit count",
            Quantity::BitsOrFullScale => "bits or full scale",
            Quantity::CprFieldWiderThanFormat => "CPR field wider than the format",
            Quantity::CprFieldWidth => "CPR field width",
            Quantity::CprZoneSpan => "CPR zone span",
            Quantity::Day => "day",
            Quantity::DayCount => "day count",
            Quantity::DayOfYear => "day of year",
            Quantity::Decimals => "decimals",
            Quantity::DecodedLatitude => "decoded latitude",
            Quantity::Elements => "elements",
            Quantity::EpochDay => "epoch day",
            Quantity::FillBits => "fill bits",
            Quantity::FillBitsPastPayload => "fill bits longer than the payload",
            Quantity::FragmentNumbers => "fragment numbers",
            Quantity::GlonassDayFields => "GLONASS N4 or NT",
            Quantity::GlonassTime => "GLONASS time before 1996 or too far ahead",
            Quantity::Inclination => "inclination",
            Quantity::Latitude => "latitude",
            Quantity::LeapSecondNotAtMidnight => "a leap second takes effect at a UTC midnight",
            Quantity::LeapSecondOrder => {
                "a new leap second must follow the last one and change the offset by one"
            }
            Quantity::LocatorLength => "locator length",
            Quantity::Longitude => "longitude",
            Quantity::MeanMotion => "mean motion",
            Quantity::Minutes => "minutes",
            Quantity::Mjd => "MJD",
            Quantity::Month => "month",
            Quantity::Nanoseconds => "nanoseconds",
            Quantity::OrbitNotClosed => "the orbit is not closed",
            Quantity::RadialTrajectory => "radial trajectory",
            Quantity::Second => "second",
            Quantity::Seconds => "seconds",
            Quantity::SequentialMessageId => "sequential message id",
            Quantity::SequentialMessageIdOrFillBits => "sequential message id or fill bits",
            Quantity::SixBitValue => "6-bit value",
            Quantity::StateVector => "state vector",
            Quantity::Time => "time",
            Quantity::TimeBeforeEpoch => "time before the epoch of the time scale",
            Quantity::TimeOfDay => "time of day",
            Quantity::TruncatedWeek => "truncated week",
            Quantity::UtcOffset => "UTC offset",
            Quantity::Week => "week",
            Quantity::Year => "year",
        }
    }
}

/// What is wrong in a text that does not have the expected layout. [`Error::Syntax`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyntaxFault {
    /// Address field.
    AddressField,
    /// Character outside the AIS payload alphabet.
    AisPayloadChar,
    /// Too many parts, or a fraction before the last part.
    AngleParts,
    /// Argument of perigee.
    ArgumentOfPerigee,
    /// ASCII.
    Ascii,
    /// B*.
    Bstar,
    /// Catalogue number.
    CatalogueNumber,
    /// Channel.
    Channel,
    /// Checksum is two hex digits.
    ChecksumDigits,
    /// Date field is ddmmyy.
    DateField,
    /// Date separator.
    DateSeparator,
    /// Decimal point without digits.
    DecimalPointWithoutDigits,
    /// Degrees.
    Degrees,
    /// Eccentricity.
    Eccentricity,
    /// Element set number.
    ElementSetNumber,
    /// Empty angle.
    EmptyAngle,
    /// Empty coordinate field.
    EmptyCoordinate,
    /// Empty position.
    EmptyPosition,
    /// Empty sentence.
    EmptySentence,
    /// Ephemeris type.
    EphemerisType,
    /// Epoch day.
    EpochDay,
    /// Epoch year.
    EpochYear,
    /// Expiry line.
    ExpiryLine,
    /// Fill bits.
    FillBits,
    /// Fix quality.
    FixQuality,
    /// Fragment count.
    FragmentCount,
    /// Fragment number.
    FragmentNumber,
    /// Fragment out of order.
    FragmentOrder,
    /// Frames given in the wrong order.
    FrameOrder,
    /// Hemisphere letter.
    HemisphereLetter,
    /// Inclination.
    Inclination,
    /// Cannot tell where the latitude ends.
    LatitudeEnd,
    /// Line numbers 1 and 2.
    LineNumbers,
    /// A locator has 2, 4, 6, 8 or 10 characters.
    LocatorLength,
    /// Magnetic variation direction.
    MagneticVariationDirection,
    /// Mean anomaly.
    MeanAnomaly,
    /// Mean motion.
    MeanMotion,
    /// Second derivative of mean motion.
    MeanMotionDdot,
    /// First derivative of mean motion.
    MeanMotionDot,
    /// Minutes.
    Minutes,
    /// Too few digits before the minutes.
    MinutesDigits,
    /// No checksum.
    NoChecksum,
    /// No entries.
    NoEntries,
    /// The surface format needs a nearby known position.
    NoNearbyPosition,
    /// No number.
    NoNumber,
    /// No TLE lines.
    NoTleLines,
    /// Not a GGA sentence.
    NotGga,
    /// Not a GLL sentence.
    NotGll,
    /// Not an RMC sentence.
    NotRmc,
    /// Not a VDM or VDO sentence.
    NotVdm,
    /// NTP seconds.
    NtpSeconds,
    /// Number.
    Number,
    /// Number field.
    NumberField,
    /// Revolution number.
    RevolutionNumber,
    /// Right ascension.
    RightAscension,
    /// The two lines are of different satellites.
    SatelliteMismatch,
    /// Satellites.
    Satellites,
    /// Seconds.
    Seconds,
    /// A sentence starts with $ or !.
    SentenceStart,
    /// Sequential message id.
    SequentialMessageId,
    /// Tag block without checksum.
    TagBlockChecksum,
    /// TAI - UTC.
    TaiMinusUtc,
    /// Text ends early.
    TextEndsEarly,
    /// Time field is hhmmss.ss.
    TimeField,
    /// Time separator.
    TimeSeparator,
    /// A TLE has two lines.
    TleLineCount,
    /// A TLE line has 69 columns.
    TleLineLength,
    /// TLE lines are ASCII.
    TleNotAscii,
    /// More than nine decimals.
    TooManyDecimals,
    /// More than three lines.
    TooManyLines,
    /// Unterminated tag block.
    UnterminatedTagBlock,
    /// VDM has six fields.
    VdmFieldCount,
}

impl SyntaxFault {
    /// The fault in plain words.
    pub fn describe(self) -> &'static str {
        match self {
            SyntaxFault::AddressField => "address field",
            SyntaxFault::AisPayloadChar => "character outside the AIS payload alphabet",
            SyntaxFault::AngleParts => "too many parts, or a fraction before the last part",
            SyntaxFault::ArgumentOfPerigee => "argument of perigee",
            SyntaxFault::Ascii => "ASCII",
            SyntaxFault::Bstar => "B*",
            SyntaxFault::CatalogueNumber => "catalogue number",
            SyntaxFault::Channel => "channel",
            SyntaxFault::ChecksumDigits => "checksum is two hex digits",
            SyntaxFault::DateField => "date field is ddmmyy",
            SyntaxFault::DateSeparator => "date separator",
            SyntaxFault::DecimalPointWithoutDigits => "decimal point without digits",
            SyntaxFault::Degrees => "degrees",
            SyntaxFault::Eccentricity => "eccentricity",
            SyntaxFault::ElementSetNumber => "element set number",
            SyntaxFault::EmptyAngle => "empty angle",
            SyntaxFault::EmptyCoordinate => "empty coordinate field",
            SyntaxFault::EmptyPosition => "empty position",
            SyntaxFault::EmptySentence => "empty sentence",
            SyntaxFault::EphemerisType => "ephemeris type",
            SyntaxFault::EpochDay => "epoch day",
            SyntaxFault::EpochYear => "epoch year",
            SyntaxFault::ExpiryLine => "expiry line",
            SyntaxFault::FillBits => "fill bits",
            SyntaxFault::FixQuality => "fix quality",
            SyntaxFault::FragmentCount => "fragment count",
            SyntaxFault::FragmentNumber => "fragment number",
            SyntaxFault::FragmentOrder => "fragment out of order",
            SyntaxFault::FrameOrder => "frames given in the wrong order",
            SyntaxFault::HemisphereLetter => "hemisphere letter",
            SyntaxFault::Inclination => "inclination",
            SyntaxFault::LatitudeEnd => "cannot tell where the latitude ends",
            SyntaxFault::LineNumbers => "line numbers 1 and 2",
            SyntaxFault::LocatorLength => "a locator has 2, 4, 6, 8 or 10 characters",
            SyntaxFault::MagneticVariationDirection => "magnetic variation direction",
            SyntaxFault::MeanAnomaly => "mean anomaly",
            SyntaxFault::MeanMotion => "mean motion",
            SyntaxFault::MeanMotionDdot => "second derivative of mean motion",
            SyntaxFault::MeanMotionDot => "first derivative of mean motion",
            SyntaxFault::Minutes => "minutes",
            SyntaxFault::MinutesDigits => "too few digits before the minutes",
            SyntaxFault::NoChecksum => "no checksum",
            SyntaxFault::NoEntries => "no entries",
            SyntaxFault::NoNearbyPosition => "the surface format needs a nearby known position",
            SyntaxFault::NoNumber => "no number",
            SyntaxFault::NoTleLines => "no TLE lines",
            SyntaxFault::NotGga => "not a GGA sentence",
            SyntaxFault::NotGll => "not a GLL sentence",
            SyntaxFault::NotRmc => "not an RMC sentence",
            SyntaxFault::NotVdm => "not a VDM or VDO sentence",
            SyntaxFault::NtpSeconds => "NTP seconds",
            SyntaxFault::Number => "number",
            SyntaxFault::NumberField => "number field",
            SyntaxFault::RevolutionNumber => "revolution number",
            SyntaxFault::RightAscension => "right ascension",
            SyntaxFault::SatelliteMismatch => "the two lines are of different satellites",
            SyntaxFault::Satellites => "satellites",
            SyntaxFault::Seconds => "seconds",
            SyntaxFault::SentenceStart => "a sentence starts with $ or !",
            SyntaxFault::SequentialMessageId => "sequential message id",
            SyntaxFault::TagBlockChecksum => "tag block without checksum",
            SyntaxFault::TaiMinusUtc => "TAI - UTC",
            SyntaxFault::TextEndsEarly => "text ends early",
            SyntaxFault::TimeField => "time field is hhmmss.ss",
            SyntaxFault::TimeSeparator => "time separator",
            SyntaxFault::TleLineCount => "a TLE has two lines",
            SyntaxFault::TleLineLength => "a TLE line has 69 columns",
            SyntaxFault::TleNotAscii => "TLE lines are ASCII",
            SyntaxFault::TooManyDecimals => "more than nine decimals",
            SyntaxFault::TooManyLines => "more than three lines",
            SyntaxFault::UnterminatedTagBlock => "unterminated tag block",
            SyntaxFault::VdmFieldCount => "VDM has six fields",
        }
    }
}

/// Why a conversion, parse or calculation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A number was NaN or infinite.
    NotFinite,
    /// A value was outside its valid range.
    OutOfRange(Quantity),
    /// Text did not have the expected layout.
    Syntax(SyntaxFault),
    /// A character was not allowed at this place (byte offset into the input).
    BadChar {
        /// Byte offset of the character.
        at: usize,
    },
    /// A checksum did not match.
    Checksum {
        /// The checksum the text carried.
        found: u8,
        /// The checksum computed from the text.
        computed: u8,
    },
    /// An output did not fit its fixed-capacity buffer.
    TooLong,
    /// An iteration did not converge (for example Vincenty's formulae for nearly antipodal points).
    NoConvergence,
    /// Two CPR frames provided different longitude zone counts, so they cannot be decoded together (DO-260B A.1.7.7).
    CprZoneMismatch,
    /// SGP4 refused the elements or the propagation time; the number is the error code of Spacetrack Report #3 as revised by
    /// Vallado et al. (1 mean eccentricity out of range, 2 mean motion not positive, 3 perturbed eccentricity out of range,
    /// 4 semi-latus rectum negative, 6 the satellite has decayed).
    Sgp4(u8),
    /// The time is before the first entry of the leap-second table (1972-01-01), where UTC had no whole-second offset from TAI.
    BeforeLeapTable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFinite => write!(f, "a number is not finite"),
            Error::OutOfRange(what) => write!(f, "{} is out of range", what.name()),
            Error::Syntax(what) => write!(f, "bad syntax: {}", what.describe()),
            Error::BadChar { at } => write!(f, "character not allowed at byte {at}"),
            Error::Checksum { found, computed } => {
                write!(
                    f,
                    "checksum {found:#04x} does not match the computed {computed:#04x}"
                )
            }
            Error::TooLong => write!(f, "the result does not fit its buffer"),
            Error::NoConvergence => write!(f, "the iteration did not converge"),
            Error::CprZoneMismatch => {
                write!(f, "the CPR frames are in different longitude zone counts")
            }
            Error::Sgp4(code) => write!(f, "SGP4 error {code}"),
            Error::BeforeLeapTable => {
                write!(f, "the time is before 1972, outside the leap-second table")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Returns `Err(NotFinite)` unless every value is finite.
#[inline]
pub(crate) fn finite(values: &[f64]) -> Result<(), Error> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(Error::NotFinite)
    }
}

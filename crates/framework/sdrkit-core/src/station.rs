//! The station context: where the receiving station is, who it is, and its local time. Set once by the user, given by the host to
//! every receiver ([`Receiver::set_station`](crate::Receiver::set_station)), so no stack keeps its own position, callsign or time
//! zone settings.

use sdrkit_util_geo::time::LeapSeconds;
use sdrkit_util_geo::Position;
use serde::{Deserialize, Serialize};

/// The receiving station.
///
/// Every part is optional: a stack that needs one that is missing says so in its status, and never guesses.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// Where the antenna is. `alt_m` is the height above the WGS-84 ellipsoid, 0 when not known (see `height_m`).
    pub position: Option<Position>,
    /// The antenna's height above the WGS-84 ellipsoid in metres, when known (the position alone cannot say "unknown").
    pub height_m: Option<f64>,
    /// The station's callsign, upper case ("VK2ABC"); empty when none.
    pub callsign: String,
    /// The Maidenhead locator the user supplied ("QF56od"); empty to derive it from the position ([`Station::locator`]).
    pub locator: String,
    /// The IANA time zone name ("Australia/Sydney"); empty when not known.
    pub time_zone: String,
    /// The offset of local time from UTC now, in seconds (the host keeps it current across daylight saving changes).
    pub utc_offset_s: i32,
    /// The leap-second table the host keeps up to date; `None` uses the table built into `sdrkit-util-geo`.
    #[serde(skip)]
    pub leap_seconds: Option<LeapSeconds>,
}

impl Station {
    /// A station at `position`, with nothing else known.
    pub fn at(position: Position) -> Station {
        Station {
            position: Some(position),
            ..Station::default()
        }
    }
    /// The locator: the one the user supplied, else six characters from the position, else `None`.
    pub fn locator(&self) -> Option<String> {
        if !self.locator.is_empty() {
            return Some(self.locator.clone());
        }
        let p = self.position.as_ref()?;
        sdrkit_util_geo::maidenhead::encode(p, 6)
            .ok()
            .map(|t| t.as_str().to_string())
    }
    /// Local time for a UTC time, both in seconds since 1970.
    pub fn local_unix(&self, utc_unix: f64) -> f64 {
        utc_unix + self.utc_offset_s as f64
    }
    /// The leap-second table in use: the host's, else the built-in one.
    pub fn leap_seconds(&self) -> LeapSeconds {
        self.leap_seconds.unwrap_or_default()
    }
    /// Checks the parts: a position inside its ranges, a finite height, a callsign of upper-case letters, digits and `/`, and a valid locator.
    ///
    /// # Errors
    ///
    /// [`StationError`] for the first bad part.
    pub fn validate(&self) -> Result<(), StationError> {
        if let Some(p) = &self.position {
            p.validate().map_err(StationError::Position)?;
        }
        if self.height_m.is_some_and(|h| !h.is_finite()) {
            return Err(StationError::Height);
        }
        if let Some(at) = self
            .callsign
            .bytes()
            .position(|b| !(b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'/'))
        {
            return Err(StationError::Callsign { at });
        }
        if !self.locator.is_empty() {
            sdrkit_util_geo::maidenhead::decode(&self.locator).map_err(StationError::Locator)?;
        }
        Ok(())
    }
}

/// A part of the station context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StationItem {
    /// The position.
    Position,
    /// The height of the antenna.
    Height,
    /// The callsign.
    Callsign,
    /// The locator.
    Locator,
    /// The time zone.
    TimeZone,
}

/// Why a station context was refused.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum StationError {
    /// The position is outside its ranges or not finite.
    #[error("position: {0}")]
    Position(sdrkit_util_geo::Error),
    /// The height is not finite.
    #[error("the height is not a finite number")]
    Height,
    /// The callsign has a character other than an upper-case letter, a digit or `/`.
    #[error("the callsign has a character that is not allowed at byte {at}")]
    Callsign {
        /// The byte offset.
        at: usize,
    },
    /// The locator does not decode.
    #[error("locator: {0}")]
    Locator(sdrkit_util_geo::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_locator_comes_from_the_position_unless_given() {
        // Berlin is JO62qm (the example in sdrkit-util-geo).
        let mut s = Station::at(Position::new(52.52, 13.405));
        assert_eq!(s.locator().as_deref(), Some("JO62qm"));
        s.locator = "JO62".into();
        assert_eq!(s.locator().as_deref(), Some("JO62"));
        assert_eq!(Station::default().locator(), None);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn bad_parts_are_refused() {
        let s = Station {
            callsign: "vk2abc".into(),
            ..Station::default()
        };
        assert_eq!(s.validate(), Err(StationError::Callsign { at: 0 }));
        let s = Station {
            utc_offset_s: 36_000,
            ..Station::default()
        };
        assert_eq!(s.local_unix(0.0), 36_000.0);
    }
}

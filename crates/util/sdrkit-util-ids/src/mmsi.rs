//! MMSI forms (ITU-R M.585-9, "Assignment and use of identities in the maritime mobile service", 05/2022): what kind of
//! station or device a nine-digit identity names, and its MID. One classifier serves AIS, DSC and ATIS; it reads identities with
//! digits that could not be decoded (`~` or `?`), as DSC receivers produce.
//!
//! | Form | Kind | M.585-9 |
//! |---|---|---|
//! | `MIDXXXXXX` (first digit 2 to 7) | ship station | Annex 1 |
//! | `0MIDXXXXX` | group of ship stations | Annex 1 |
//! | `00MIDXXXX` | coast station | Annex 1 |
//! | `111MIDaXX` | SAR aircraft; a = 1 fixed wing, 5 helicopter | Annex 1 |
//! | `8MIDXXXXX` | handheld VHF with DSC and GNSS | Annex 2 |
//! | `98MIDXXXX` | craft associated with a parent ship | Annex 2 |
//! | `99MIDaXXX` | AIS aid to navigation; a = 1 physical, 6 virtual | Annex 2 |
//! | `970XXYYYY` | AIS-SART (search and rescue transmitter) | Annex 2 |
//! | `972XXYYYY` | man overboard device | Annex 2 |
//! | `974XXYYYY` | EPIRB with AIS | Annex 2 |
//! | `979XXYYYY` | autonomous maritime radio device, group B | Annex 2 |
//!
//! For the free-form devices (97x) XX is the maker's number and YYYY a sequence number; they carry no MID.
//!
//! ```
//! use sdrkit_util_ids::mmsi::{classify, Kind};
//!
//! let i = classify("002320064");
//! assert_eq!(i.kind, Kind::CoastStation);
//! assert_eq!(i.country, Some("United Kingdom"));
//! assert_eq!(classify("23212~456").mid, Some(232));
//! ```

use crate::mid;

/// What an SAR aircraft identity says about the aircraft (the digit after the MID).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Aircraft {
    /// 1: fixed wing.
    FixedWing,
    /// 5: helicopter.
    Helicopter,
    /// Any other digit, or one that could not be read.
    Other,
}

/// What an AtoN identity says (the digit after the MID).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Aton {
    /// 1: a physical aid.
    Physical,
    /// 6: a virtual aid (no structure at the position).
    Virtual,
    /// Any other digit, or one that could not be read.
    Other,
}

/// The kind of station or device an MMSI names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A ship station.
    Ship,
    /// A group of ship stations.
    GroupShip,
    /// A coast station.
    CoastStation,
    /// An aircraft taking part in search and rescue.
    SarAircraft(Aircraft),
    /// A handheld VHF transceiver with DSC and GNSS.
    Handheld,
    /// A craft associated with a parent ship (a tender, a lifeboat).
    ParentShipCraft,
    /// An AIS aid to navigation.
    AidToNavigation(Aton),
    /// An AIS search and rescue transmitter.
    AisSart,
    /// A man overboard device.
    ManOverboard,
    /// An EPIRB with an AIS locating signal.
    EpirbAis,
    /// An autonomous maritime radio device of group B.
    AmrdGroupB,
    /// Not a form of M.585.
    Unknown,
}

impl Kind {
    /// A short English name.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Ship => "ship",
            Kind::GroupShip => "group of ships",
            Kind::CoastStation => "coast station",
            Kind::SarAircraft(Aircraft::FixedWing) => "SAR aircraft (fixed wing)",
            Kind::SarAircraft(Aircraft::Helicopter) => "SAR aircraft (helicopter)",
            Kind::SarAircraft(Aircraft::Other) => "SAR aircraft",
            Kind::Handheld => "handheld VHF",
            Kind::ParentShipCraft => "craft of a parent ship",
            Kind::AidToNavigation(Aton::Physical) => "aid to navigation (physical)",
            Kind::AidToNavigation(Aton::Virtual) => "aid to navigation (virtual)",
            Kind::AidToNavigation(Aton::Other) => "aid to navigation",
            Kind::AisSart => "AIS-SART",
            Kind::ManOverboard => "man overboard device",
            Kind::EpirbAis => "EPIRB-AIS",
            Kind::AmrdGroupB => "autonomous maritime radio device",
            Kind::Unknown => "unknown",
        }
    }
}

/// What the classifier says about an identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Info {
    /// The kind.
    pub kind: Kind,
    /// The MID, when the form has one and its three digits were read.
    pub mid: Option<u16>,
    /// The country of the MID, when allocated.
    pub country: Option<&'static str>,
    /// Some digits could not be read.
    pub partial: bool,
}

const UNREAD: [u8; 2] = *b"~?";

/// Classifies nine digits, of which some may be unreadable (`~` or `?`); spaces are ignored. Anything that is not nine digits
/// returns [`Kind::Unknown`].
pub fn classify(digits: &str) -> Info {
    let mut d = [0u8; 9];
    let mut n = 0;
    let mut partial = false;
    for b in digits.bytes().filter(|b| !b.is_ascii_whitespace()) {
        if n == 9 || !(b.is_ascii_digit() || UNREAD.contains(&b)) {
            return unknown(partial || UNREAD.contains(&b));
        }
        partial |= UNREAD.contains(&b);
        d[n] = b;
        n += 1;
    }
    if n != 9 {
        return unknown(partial);
    }
    let is = |i: usize, c: u8| d[i] == c;
    let (kind, mid_at) = if is(0, b'9') && is(1, b'7') {
        let k = match d[2] {
            b'0' => Kind::AisSart,
            b'2' => Kind::ManOverboard,
            b'4' => Kind::EpirbAis,
            b'9' => Kind::AmrdGroupB,
            _ => Kind::Unknown,
        };
        (k, None)
    } else if is(0, b'9') && is(1, b'8') {
        (Kind::ParentShipCraft, Some(2))
    } else if is(0, b'9') && is(1, b'9') {
        let a = match d[5] {
            b'1' => Aton::Physical,
            b'6' => Aton::Virtual,
            _ => Aton::Other,
        };
        (Kind::AidToNavigation(a), Some(2))
    } else if is(0, b'1') && is(1, b'1') && is(2, b'1') {
        let a = match d[6] {
            b'1' => Aircraft::FixedWing,
            b'5' => Aircraft::Helicopter,
            _ => Aircraft::Other,
        };
        (Kind::SarAircraft(a), Some(3))
    } else if is(0, b'0') && is(1, b'0') {
        (Kind::CoastStation, Some(2))
    } else if is(0, b'0') {
        (Kind::GroupShip, Some(1))
    } else if is(0, b'8') {
        (Kind::Handheld, Some(1))
    } else if (b'2'..=b'7').contains(&d[0]) || UNREAD.contains(&d[0]) {
        (Kind::Ship, Some(0))
    } else {
        (Kind::Unknown, None)
    };
    let mid = mid_at.and_then(|i| {
        let m = &d[i..i + 3];
        m.iter()
            .all(u8::is_ascii_digit)
            .then(|| m.iter().fold(0u16, |a, &b| a * 10 + (b - b'0') as u16))
    });
    Info {
        kind,
        mid,
        country: mid.and_then(mid::country),
        partial,
    }
}

fn unknown(partial: bool) -> Info {
    Info {
        kind: Kind::Unknown,
        mid: None,
        country: None,
        partial,
    }
}

/// The kind of a nine-digit identity.
pub fn kind(digits: &str) -> Kind {
    classify(digits).kind
}

/// An MMSI as the number AIS carries (30 bits); it is always written with nine digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mmsi(u32);

impl Mmsi {
    /// An MMSI from its number, `None` above 999 999 999.
    pub const fn new(n: u32) -> Option<Mmsi> {
        if n <= 999_999_999 {
            Some(Mmsi(n))
        } else {
            None
        }
    }

    /// The number.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The nine digits, as ASCII.
    pub fn digits(self) -> [u8; 9] {
        let mut d = [b'0'; 9];
        let mut v = self.0;
        for i in (0..9).rev() {
            d[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
        d
    }

    /// What the identity names.
    pub fn info(self) -> Info {
        let d = self.digits();
        // Nine ASCII digits are always valid UTF-8.
        classify(std::str::from_utf8(&d).unwrap_or(""))
    }

    /// Whether an AIS identity is plausible: a form AIS uses (ship, SAR aircraft, group or coast station, aid to navigation,
    /// craft of a parent ship, or a 970 to 974 or 979 device) with a MID of valid form ([`mid::has_valid_form`]). Handheld
    /// identities (8MID) are not used on AIS. A frame whose MMSI fails this is almost certainly noise that passed the CRC.
    pub fn is_plausible_ais(self) -> bool {
        let n = self.0;
        let d = |i: u32, len: u32| (n / 10u32.pow(9 - i - len) % 10u32.pow(len)) as u16;
        match n {
            0 => false,
            200_000_000..=799_999_999 => mid::has_valid_form(d(0, 3)),
            111_000_000..=111_999_999 => mid::has_valid_form(d(3, 3)),
            970_000_000..=974_999_999 | 979_000_000..=979_999_999 => true,
            980_000_000..=999_999_999 => mid::has_valid_form(d(2, 3)),
            1..=99_999_999 => mid::has_valid_form(d(1, 3)) || mid::has_valid_form(d(2, 3)),
            _ => false,
        }
    }
}

impl std::fmt::Display for Mmsi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:09}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_form_of_m585() {
        let c = classify("232123456");
        assert_eq!(
            (c.kind, c.mid, c.country),
            (Kind::Ship, Some(232), Some("United Kingdom"))
        );
        let c = classify("002320064");
        assert_eq!((c.kind, c.mid), (Kind::CoastStation, Some(232)));
        assert_eq!(classify("023212345").kind, Kind::GroupShip);
        assert_eq!(
            classify("111232101").kind,
            Kind::SarAircraft(Aircraft::FixedWing)
        );
        assert_eq!(
            classify("111232501").kind,
            Kind::SarAircraft(Aircraft::Helicopter)
        );
        assert_eq!(
            classify("111232001").kind,
            Kind::SarAircraft(Aircraft::Other)
        );
        assert_eq!(classify("111232101").mid, Some(232));
        assert_eq!(classify("823212345").kind, Kind::Handheld);
        assert_eq!(classify("982321234").kind, Kind::ParentShipCraft);
        assert_eq!(classify("982321234").mid, Some(232));
        assert_eq!(
            classify("992321001").kind,
            Kind::AidToNavigation(Aton::Physical)
        );
        assert_eq!(
            classify("992326001").kind,
            Kind::AidToNavigation(Aton::Virtual)
        );
        assert_eq!(classify("970123456").kind, Kind::AisSart);
        assert_eq!(classify("970123456").mid, None);
        assert_eq!(classify("972123456").kind, Kind::ManOverboard);
        assert_eq!(classify("974123456").kind, Kind::EpirbAis);
        assert_eq!(classify("979123456").kind, Kind::AmrdGroupB);
        assert_eq!(classify("971123456").kind, Kind::Unknown);
        assert_eq!(classify("123456789").kind, Kind::Unknown);
    }

    #[test]
    fn unreadable_digits_and_bad_input() {
        let c = classify("23212~456");
        assert_eq!((c.mid, c.partial, c.kind), (Some(232), true, Kind::Ship));
        let c = classify("2~2123456");
        assert_eq!((c.mid, c.country, c.partial), (None, None, true));
        assert_eq!(classify("~32123456").kind, Kind::Ship);
        assert_eq!(classify("2321").kind, Kind::Unknown);
        assert_eq!(classify("2321234567").kind, Kind::Unknown);
        assert_eq!(classify("23212x456").kind, Kind::Unknown);
        assert_eq!(classify("232 123 456").mid, Some(232));
        assert_eq!(classify("").kind, Kind::Unknown);
        assert_eq!(kind("é"), Kind::Unknown);
    }

    #[test]
    fn plausibility() {
        let p = |n: u32| Mmsi::new(n).unwrap().is_plausible_ais();
        for m in [
            389716608, 386574822, 590364551, 791679468, 150000000, 800000001, 0, 123456789,
        ] {
            assert!(!p(m), "{m}");
        }
        for m in [
            244670123, 366999999, 503123456, 111244001, 994700001, 970123456, 979000001, 2440001,
            24400000,
        ] {
            assert!(p(m), "{m}");
        }
        assert_eq!(Mmsi::new(1_000_000_000), None);
        assert_eq!(Mmsi::new(2_320_064).unwrap().to_string(), "002320064");
        assert_eq!(
            Mmsi::new(2_320_064).unwrap().info().kind,
            Kind::CoastStation
        );
    }
}

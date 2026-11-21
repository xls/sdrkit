//! Mobile country codes (MCC) and mobile network codes (MNC) of ITU-T E.212: the country of a network and the public name of
//! its operator.
//!
//! Data (`data/mcc.tsv`, `data/mnc.tsv`, read at build time):
//! - MCC: ITU-T "List of Mobile Country or Geographical Area Codes", Annex to ITU Operational Bulletin No. 1117, position on
//!   1 February 2017 (238 codes).
//! - MNC: ITU TSB "Mobile Network Codes (MNC) for the international identification plan for public networks and
//!   subscriptions", Annex to ITU Operational Bulletin No. 1162, position on 15 December 2018 (about 2,000 networks, with the
//!   shared codes 901 and 902). Later amendments are not applied, so newer networks are missing and some names are the names
//!   of 2018. A host that needs newer names imports its own list with [`crate::names`].
//!
//! The MNC keeps its number of digits: E.212 allows two or three, and a two-digit `01` and a three-digit `001` are different
//! codes. [`operator_any`] is for callers that do not know the digit count.
//!
//! ```
//! use sdrkit_util_ids::mcc::{self, Plmn};
//!
//! let p = Plmn::parse("262-01").unwrap();
//! assert_eq!(mcc::country(p.mcc), Some("Germany (Federal Republic of)"));
//! assert_eq!(mcc::operator(p), Some("Telekom Deutschland GmbH"));
//! assert_eq!(p.to_string(), "262-01");
//! ```

use crate::pool::{self, find};
use std::fmt;

/// A network: MCC, MNC and the number of digits the MNC is written with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Plmn {
    /// Mobile country code, 0 to 999.
    pub mcc: u16,
    /// Mobile network code, 0 to 999.
    pub mnc: u16,
    /// 2 or 3.
    pub mnc_digits: u8,
}

impl Plmn {
    /// A network, `None` when a code is out of range or the digit count is not 2 or 3 (or 2 with an MNC above 99).
    pub const fn new(mcc: u16, mnc: u16, mnc_digits: u8) -> Option<Plmn> {
        let ok = mcc <= 999
            && match mnc_digits {
                2 => mnc <= 99,
                3 => mnc <= 999,
                _ => false,
            };
        if ok {
            Some(Plmn {
                mcc,
                mnc,
                mnc_digits,
            })
        } else {
            None
        }
    }

    /// Reads "MCC-MNC", "MCC MNC" or the five or six digits run together (the length sets the number of MNC digits).
    pub fn parse(s: &str) -> Option<Plmn> {
        let s = s.trim();
        let (a, b) = match s.find(['-', ' ', '/']) {
            Some(i) => (s[..i].trim(), s[i + 1..].trim()),
            None if s.len() == 5 || s.len() == 6 => s.split_at(3),
            None => return None,
        };
        let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
        if a.len() != 3 || !digits(a) || !digits(b) || !(2..=3).contains(&b.len()) {
            return None;
        }
        Plmn::new(a.parse().ok()?, b.parse().ok()?, b.len() as u8)
    }

    fn key(self) -> u32 {
        self.mcc as u32 * 10000 + self.mnc_digits as u32 * 1000 + self.mnc as u32
    }
}

impl fmt::Display for Plmn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mnc_digits == 3 {
            write!(f, "{:03}-{:03}", self.mcc, self.mnc)
        } else {
            write!(f, "{:03}-{:02}", self.mcc, self.mnc)
        }
    }
}

/// The test network 001-01 used by test equipment and by base stations in a lab (3GPP TS 36.508 clause 4.4.2, "test PLMN").
pub const TEST_PLMN: Plmn = Plmn {
    mcc: 1,
    mnc: 1,
    mnc_digits: 2,
};

/// The country or geographical area of an MCC, as the ITU list writes it (areas sharing a code are joined with "; ").
pub fn country(mcc: u16) -> Option<&'static str> {
    find(&pool::MCC_KEYS, &mcc).map(|i| pool::MCC_POOL.get(pool::MCC_AREA[i]))
}

/// The country without the formal part in brackets: "Germany (Federal Republic of)" is returned as "Germany".
pub fn country_short(mcc: u16) -> Option<&'static str> {
    country(mcc).map(|c| match c.find(" (") {
        Some(i) if !c.contains(';') => &c[..i],
        _ => c,
    })
}

/// Whether an MCC is shared by networks of several countries (901, 902) rather than one country.
pub fn is_shared(mcc: u16) -> bool {
    find(&pool::MCC_KEYS, &mcc).is_some_and(|i| pool::MCC_NOTE[i] == b'c') || mcc == 902
}

/// The operator or network name of a PLMN, as the ITU list writes it. [`TEST_PLMN`] returns "Test network".
pub fn operator(plmn: Plmn) -> Option<&'static str> {
    if plmn == TEST_PLMN {
        return Some("Test network");
    }
    find(&pool::MNC_KEYS, &plmn.key()).map(|i| pool::MNC_POOL.get(pool::MNC_NAME[i]))
}

/// The operator when the number of MNC digits is not known: the two-digit code first, then the three-digit one.
pub fn operator_any(mcc: u16, mnc: u16) -> Option<&'static str> {
    Plmn::new(mcc, mnc, 2)
        .and_then(operator)
        .or_else(|| Plmn::new(mcc, mnc, 3).and_then(operator))
}

/// The number of networks in the table.
pub fn network_count() -> usize {
    pool::MNC_KEYS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countries_from_the_itu_list() {
        assert_eq!(country(246), Some("Lithuania (Republic of)"));
        assert_eq!(country_short(246), Some("Lithuania"));
        assert_eq!(country(310), Some("United States of America"));
        assert_eq!(country(228), Some("Switzerland (Confederation of)"));
        assert_eq!(country(221), Some("Kosovo"));
        assert_eq!(
            country(340),
            Some("Guadeloupe (French Department of); Martinique (French Department of)")
        );
        assert_eq!(country_short(340), country(340));
        assert_eq!(country(901), Some("International Mobile, shared code"));
        assert!(is_shared(901) && is_shared(902) && !is_shared(262));
        assert_eq!(country(1), None);
        assert_eq!(country(999), None);
    }

    #[test]
    fn operators_from_the_itu_list() {
        let op = |s: &str| operator(Plmn::parse(s).unwrap());
        assert_eq!(op("262-10"), Some("DB Netz AG"));
        assert_eq!(op("310-410"), Some("AT&T Mobility"));
        assert_eq!(op("313-100"), Some("First Responder Network Authority"));
        assert_eq!(op("228-06"), Some("SBB AG"));
        assert_eq!(op("234-12"), Some("Network Rail Infrastructure Limited"));
        assert_eq!(op("412-01"), Some("AWCC"));
        assert_eq!(op("302-220"), Some("Telus Mobility"));
        assert_eq!(op("901-03"), Some("Iridium Communications Inc"));
        assert_eq!(
            op("901-88"),
            Some("UN Office for the Coordination of Humanitarian Affairs (OCHA)")
        );
        assert_eq!(op("001-01"), Some("Test network"));
        assert_eq!(op("999-99"), None);
        assert_eq!(op("310-41"), None, "two digits is another code");
        assert_eq!(operator_any(310, 410), Some("AT&T Mobility"));
        assert_eq!(operator_any(262, 1), Some("Telekom Deutschland GmbH"));
        assert!(network_count() > 1900);
    }

    #[test]
    fn plmn_text() {
        assert_eq!(Plmn::parse("310410"), Plmn::new(310, 410, 3));
        assert_eq!(Plmn::parse("26201"), Plmn::new(262, 1, 2));
        assert_eq!(Plmn::parse("001 01").unwrap().to_string(), "001-01");
        assert_eq!(Plmn::new(310, 410, 3).unwrap().to_string(), "310-410");
        for bad in [
            "", "31-410", "310-4", "310-4100", "31x-01", "1234567", "310-",
        ] {
            assert_eq!(Plmn::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Plmn::new(1000, 1, 2), None);
        assert_eq!(Plmn::new(1, 100, 2), None);
        assert_eq!(Plmn::new(1, 1, 4), None);
    }
}

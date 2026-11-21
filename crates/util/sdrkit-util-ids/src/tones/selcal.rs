//! Aviation SELCAL (selective calling, ARINC 714; ICAO Annex 10 Volume II clause 5.2.4 for the procedures): a ground station
//! calls an aircraft with two pulses of two simultaneous tones each. A code is four tone designators written in two pairs,
//! `AB-CD`.
//!
//! Tones (ASRI "SELCAL User's Guide", Rev E, 10 September 2024, Table 2-1): the 16 tones A to S (without I, N and O) of the
//! classic set, P to S having been added in 1985, and the 16 tones T to Z and 1 to 9 added on 3 November 2022 ("SELCAL 32").
//! The new tones fall between the old ones, so the 32 tones are about 0.0225 decades (5.3 %) apart. Each pulse lasts 1.0 ±
//! 0.25 s, with 0.2 ± 0.1 s between the pulses.
//!
//! ```
//! use sdrkit_util_ids::tones::selcal::{self, Code};
//!
//! assert_eq!(selcal::tone('A'), Some(312.6));
//! assert_eq!(selcal::tone('T'), Some(329.2));
//! let c = Code::parse("am cp").unwrap();
//! assert_eq!(c.to_string(), "AM-CP");
//! assert_eq!(c.tones(), [312.6, 977.2, 384.6, 1083.9]);
//! assert!(Code::parse("AA-BC").is_none()); // the two tones of a pulse differ
//! ```

use std::fmt;

/// One SELCAL tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    /// The designator.
    pub designator: char,
    /// The frequency, Hz.
    pub hz: f32,
    /// One of the 16 tones in use before 2022.
    pub classic: bool,
}

const fn t(designator: char, hz: f32, classic: bool) -> Tone {
    Tone {
        designator,
        hz,
        classic,
    }
}

/// The 32 tones in ascending frequency.
pub static TONES: [Tone; 32] = [
    t('A', 312.6, true),
    t('T', 329.2, false),
    t('B', 346.7, true),
    t('U', 365.2, false),
    t('C', 384.6, true),
    t('V', 405.0, false),
    t('D', 426.6, true),
    t('W', 449.3, false),
    t('E', 473.2, true),
    t('X', 498.3, false),
    t('F', 524.8, true),
    t('Y', 552.7, false),
    t('G', 582.1, true),
    t('Z', 613.1, false),
    t('H', 645.7, true),
    t('1', 680.0, false),
    t('J', 716.1, true),
    t('2', 754.2, false),
    t('K', 794.3, true),
    t('3', 836.6, false),
    t('L', 881.0, true),
    t('4', 927.9, false),
    t('M', 977.2, true),
    t('5', 1029.2, false),
    t('P', 1083.9, true),
    t('6', 1141.6, false),
    t('Q', 1202.3, true),
    t('7', 1266.2, false),
    t('R', 1333.5, true),
    t('8', 1404.4, false),
    t('S', 1479.1, true),
    t('9', 1557.8, false),
];

/// The length of each pulse, s.
pub const PULSE_S: f32 = 1.0;
/// The gap between the two pulses, s.
pub const GAP_S: f32 = 0.2;

fn index(designator: char) -> Option<usize> {
    let d = designator.to_ascii_uppercase();
    TONES.iter().position(|t| t.designator == d)
}

/// The frequency of a designator (either case), Hz.
pub fn tone(designator: char) -> Option<f32> {
    index(designator).map(|i| TONES[i].hz)
}

/// The tone nearest to `hz` within `tolerance_hz`.
pub fn nearest(hz: f32, tolerance_hz: f32) -> Option<Tone> {
    if !hz.is_finite() || tolerance_hz.is_nan() || tolerance_hz < 0.0 {
        return None;
    }
    TONES
        .iter()
        .filter(|t| (t.hz - hz).abs() <= tolerance_hz)
        .min_by(|a, b| (a.hz - hz).abs().total_cmp(&(b.hz - hz).abs()))
        .copied()
}

/// A SELCAL code: four tone designators in two pairs.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Code([u8; 4]);

impl Code {
    /// Reads a code: four designators, spaces and a hyphen ignored, either case ("AM-CP", "amcp", "AM CP"). The two tones of a
    /// pulse must differ (they are sent together). `None` for anything else.
    pub fn parse(s: &str) -> Option<Code> {
        let mut d = [0u8; 4];
        let mut n = 0;
        for c in s.chars().filter(|c| !c.is_whitespace() && *c != '-') {
            if n == 4 {
                return None;
            }
            index(c)?;
            d[n] = c.to_ascii_uppercase() as u8;
            n += 1;
        }
        (n == 4 && d[0] != d[1] && d[2] != d[3]).then_some(Code(d))
    }

    /// The four frequencies: first pulse, then second pulse, each pair as written.
    pub fn tones(&self) -> [f32; 4] {
        self.0.map(|b| tone(b as char).unwrap_or(0.0))
    }

    /// Whether every tone is one of the 16 classic tones (a code any SELCAL decoder made before 2022 can receive).
    pub fn is_classic(&self) -> bool {
        self.0
            .iter()
            .all(|&b| index(b as char).is_some_and(|i| TONES[i].classic))
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = |i: usize| self.0[i] as char;
        write!(f, "{}{}-{}{}", c(0), c(1), c(2), c(3))
    }
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Code({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asri_table_2_1() {
        assert!(TONES.windows(2).all(|w| w[0].hz < w[1].hz));
        assert_eq!(TONES.iter().filter(|t| t.classic).count(), 16);
        assert_eq!(tone('S'), Some(1479.1));
        assert_eq!(tone('9'), Some(1557.8));
        assert_eq!(tone('j'), Some(716.1));
        for missing in ['I', 'N', 'O', '0'] {
            assert_eq!(tone(missing), None);
        }
        // The tones follow 10^(0.0225 n) within the table's rounding (0.1 Hz).
        for (n, t) in TONES.iter().enumerate() {
            let f = 312.6 * 10f32.powf(0.0225 * n as f32);
            assert!(
                (f - t.hz).abs() / t.hz < 0.002,
                "{} {} {}",
                t.designator,
                f,
                t.hz
            );
        }
    }

    #[test]
    fn codes() {
        let c = Code::parse("AM-CP").unwrap();
        assert!(c.is_classic());
        assert!(!Code::parse("AT-B9").unwrap().is_classic());
        for bad in ["AM-C", "AI-CP", "AA-CP", "AM-CC", "AMCPQ", ""] {
            assert!(Code::parse(bad).is_none(), "{bad:?}");
        }
        assert_eq!(nearest(978.0, 5.0).map(|t| t.designator), Some('M'));
        assert_eq!(nearest(1000.0, 5.0), None);
    }
}

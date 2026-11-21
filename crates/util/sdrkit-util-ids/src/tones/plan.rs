//! Tone plans: the set of tones a detector listens for, by name, built from the tables of this module. A plan is a list of
//! frequencies with a label for each, in a fixed order a decoder can rely on:
//!
//! | Name | Tones, in order | Source |
//! |---|---|---|
//! | `ctcss` | the 50 CTCSS tones, ascending | [`super::ctcss::TONES`] |
//! | `ctcss_eia` | the 38 EIA RS-220-A tones, ascending | [`super::ctcss::TONES`] |
//! | `dtmf` | the four row tones, then the four column tones | [`super::dtmf`] (ITU-T Q.23) |
//! | `selcal` | the 32 SELCAL tones, ascending | [`super::selcal::TONES`] |
//! | `selcal_classic` | the 16 SELCAL tones in use before 2022, ascending | [`super::selcal::TONES`] |
//! | `selcall_<set>` (`selcall_zvei1`, `selcall_ccir1` ...) | digits 0 to 9, the repeat tone, then the group tone if the set has one | [`super::selcall::TONE_SETS`] |
//!
//! DCS is not a tone plan: it is a 23-bit code word sent as a 134.4 bit/s bit stream ([`super::dcs`]), decoded from bits.
//!
//! ```
//! use sdrkit_util_ids::tones::plan;
//!
//! let dtmf = plan::by_name("dtmf").unwrap();
//! assert_eq!(dtmf.len(), 8);
//! assert_eq!(dtmf.tone(5), Some((1336.0, "1336")));
//! let z = plan::by_name("selcall_zvei1").unwrap();
//! assert_eq!(z.tone(10), Some((2600.0, "E"))); // the repeat tone
//! assert!(plan::NAMES.contains(&"selcal_classic"));
//! ```

use super::{ctcss, dtmf, selcal, selcall};

/// Every plan name [`by_name`] accepts.
pub const NAMES: [&str; 19] = [
    "ctcss",
    "ctcss_eia",
    "dtmf",
    "selcal",
    "selcal_classic",
    "selcall_zvei1",
    "selcall_zvei2",
    "selcall_zvei3",
    "selcall_pzvei",
    "selcall_dzvei",
    "selcall_pdzvei",
    "selcall_ccir1",
    "selcall_ccir2",
    "selcall_pccir",
    "selcall_eea",
    "selcall_eia",
    "selcall_euro",
    "selcall_natel",
    "selcall_modat",
];

/// The labels of the DTMF tones, in plan order.
const DTMF_LABELS: [&str; 8] = ["697", "770", "852", "941", "1209", "1336", "1477", "1633"];
/// The labels of selcall tones, in plan order.
const SELCALL_LABELS: [&str; 12] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "E", "A"];
/// The SELCAL designators as text.
const SELCAL_LABELS: [&str; 32] = [
    "A", "T", "B", "U", "C", "V", "D", "W", "E", "X", "F", "Y", "G", "Z", "H", "1", "J", "2", "K",
    "3", "L", "4", "M", "5", "P", "6", "Q", "7", "R", "8", "S", "9",
];

/// A tone plan: the tones of one table, in the order of the module documentation. Small and `Copy`; reading it does not
/// allocate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    name: &'static str,
    kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Ctcss { eia_only: bool },
    Dtmf,
    Selcal { classic_only: bool },
    Selcall(&'static selcall::ToneSet),
}

/// The plan called `name` (one of [`NAMES`], either case).
pub fn by_name(name: &str) -> Option<Plan> {
    let n = name.trim();
    let name = *NAMES.iter().find(|k| k.eq_ignore_ascii_case(n))?;
    let kind = match name {
        "ctcss" => Kind::Ctcss { eia_only: false },
        "ctcss_eia" => Kind::Ctcss { eia_only: true },
        "dtmf" => Kind::Dtmf,
        "selcal" => Kind::Selcal {
            classic_only: false,
        },
        "selcal_classic" => Kind::Selcal { classic_only: true },
        _ => Kind::Selcall(selcall::by_name(name.strip_prefix("selcall_")?)?),
    };
    Some(Plan { name, kind })
}

impl Plan {
    /// The plan's name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The number of tones.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Always false (every plan has tones).
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Tone `k`: its frequency in Hz and its label (the CTCSS or DTMF frequency as text, the SELCAL designator, the selcall digit,
    /// `E` for the repeat tone and `A` for the group tone).
    pub fn tone(&self, k: usize) -> Option<(f32, &'static str)> {
        self.iter().nth(k)
    }

    /// The tones in plan order, as (Hz, label).
    pub fn iter(&self) -> impl Iterator<Item = (f32, &'static str)> + '_ {
        let ctcss = match self.kind {
            Kind::Ctcss { eia_only } => Some(
                ctcss::TONES
                    .iter()
                    .zip(CTCSS_LABELS.iter())
                    .filter(move |(t, _)| t.eia || !eia_only)
                    .map(|(t, l)| (t.hz, *l)),
            ),
            _ => None,
        };
        let dtmf = match self.kind {
            Kind::Dtmf => Some(
                dtmf::ROWS
                    .iter()
                    .chain(dtmf::COLUMNS.iter())
                    .zip(DTMF_LABELS)
                    .map(|(&f, l)| (f, l)),
            ),
            _ => None,
        };
        let selcal = match self.kind {
            Kind::Selcal { classic_only } => Some(
                selcal::TONES
                    .iter()
                    .zip(SELCAL_LABELS)
                    .filter(move |(t, _)| t.classic || !classic_only)
                    .map(|(t, l)| (t.hz, l)),
            ),
            _ => None,
        };
        let sets = match self.kind {
            Kind::Selcall(s) => Some(
                s.digits
                    .iter()
                    .copied()
                    .chain(std::iter::once(s.repeat_hz))
                    .chain(s.group_hz)
                    .zip(SELCALL_LABELS),
            ),
            _ => None,
        };
        ctcss
            .into_iter()
            .flatten()
            .chain(dtmf.into_iter().flatten())
            .chain(selcal.into_iter().flatten())
            .chain(sets.into_iter().flatten())
    }
}

/// The CTCSS frequencies as text, in table order.
const CTCSS_LABELS: [&str; 50] = [
    "67.0", "69.3", "71.9", "74.4", "77.0", "79.7", "82.5", "85.4", "88.5", "91.5", "94.8", "97.4",
    "100.0", "103.5", "107.2", "110.9", "114.8", "118.8", "123.0", "127.3", "131.8", "136.5",
    "141.3", "146.2", "151.4", "156.7", "159.8", "162.2", "165.5", "167.9", "171.3", "173.8",
    "177.3", "179.9", "183.5", "186.2", "189.9", "192.8", "196.6", "199.5", "203.5", "206.5",
    "210.7", "218.1", "225.7", "229.1", "233.6", "241.8", "250.3", "254.1",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_returns_its_plan() {
        for n in NAMES {
            let p = by_name(n).unwrap_or_else(|| panic!("{n}"));
            assert_eq!(p.name(), n);
            assert!(!p.is_empty() && p.len() == p.iter().count());
            // No tone twice: a detector would report it twice.
            let f: Vec<f32> = p.iter().map(|t| t.0).collect();
            for (i, a) in f.iter().enumerate() {
                assert!(f[i + 1..].iter().all(|b| b != a), "{n} {a}");
            }
        }
        assert_eq!(by_name(" DTMF ").unwrap().name(), "dtmf");
        assert_eq!(by_name("dcs"), None);
        assert_eq!(by_name("selcall_"), None);
        // Every selcall set has a plan.
        for s in &selcall::TONE_SETS {
            assert!(
                by_name(&format!("selcall_{}", s.name)).is_some(),
                "{}",
                s.name
            );
        }
    }

    #[test]
    fn plans_follow_their_tables() {
        let c = by_name("ctcss").unwrap();
        assert_eq!(c.len(), 50);
        for (t, (f, l)) in ctcss::TONES.iter().zip(c.iter()) {
            assert_eq!(t.hz, f);
            assert_eq!(l.parse::<f32>().unwrap(), f);
        }
        assert_eq!(by_name("ctcss_eia").unwrap().len(), 38);
        let d = by_name("dtmf").unwrap();
        let f: Vec<f32> = d.iter().map(|t| t.0).collect();
        assert_eq!(
            f,
            [697.0, 770.0, 852.0, 941.0, 1209.0, 1336.0, 1477.0, 1633.0]
        );
        assert!(d.iter().all(|(f, l)| l.parse::<f32>().unwrap() == f));
        let s = by_name("selcal").unwrap();
        assert_eq!(s.len(), 32);
        for (f, l) in s.iter() {
            assert_eq!(selcal::tone(l.chars().next().unwrap()), Some(f));
        }
        let sc = by_name("selcal_classic").unwrap();
        assert_eq!(sc.len(), 16);
        assert_eq!(sc.tone(15), Some((1479.1, "S")));
        let z = by_name("selcall_zvei1").unwrap();
        assert_eq!(z.len(), 12);
        assert_eq!(z.tone(0), Some((2400.0, "0")));
        assert_eq!(z.tone(11), Some((2800.0, "A")));
        assert_eq!(z.tone(12), None);
        // ZVEI2 has no group tone.
        assert_eq!(by_name("selcall_zvei2").unwrap().len(), 11);
        for p in NAMES.iter().filter_map(|n| n.strip_prefix("selcall_")) {
            let set = selcall::by_name(p).unwrap();
            for (f, l) in by_name(&format!("selcall_{p}")).unwrap().iter() {
                assert_eq!(set.tone(l.chars().next().unwrap()), Some(f), "{p} {l}");
            }
        }
    }
}

//! Programme type (PTY) codes 0 to 31 of RDS, RBDS, DAB and DRM, with the 8-character and 16-character display texts where the
//! standard provides them (`_` in a standard's table is a space here).
//!
//! | Table | Source |
//! |---|---|
//! | [`PtyTable::Rds`] | IEC 62106 / EN 50067 Annex F, as reproduced in NRSC-4-A (2005) Annex F Table F.2 |
//! | [`PtyTable::Rbds`] | NRSC-4-A (2005) Annex F Table F.1; codes 24 to 26 added by NRSC-4-B (April 2011) |
//! | [`PtyTable::Dab`] | ETSI TS 101 756 V2.4.1 Table 12 (International Table Identifier 0x01) |
//! | [`PtyTable::DabNorthAmerica`] | ETSI TS 101 756 V2.4.1 Table 13 (International Table Identifier 0x02) |
//! | [`PtyTable::Drm`] | ETSI ES 201 980 V4.2.1 clause 6.3.4 Table 19 |
//!
//! Codes a table leaves unassigned, reserved or "not used" give `None` (RBDS 27 and 28; DAB 30 and 31, which DAB signals
//! with announcements instead; DAB North America 24 to 28; DRM 30 and 31, which are receiver indicators, not types).
//!
//! ```
//! use sdrkit_util_ids::pty::{self, PtyTable};
//!
//! assert_eq!(pty::name(10, PtyTable::Rds), Some("Pop Music"));
//! assert_eq!(pty::name(10, PtyTable::Rbds), Some("Country"));
//! assert_eq!(pty::pty(31, PtyTable::Rds).unwrap().short8, Some("Alarm !"));
//! assert_eq!(pty::name(31, PtyTable::Dab), None);
//! ```

/// Which programme type table a code belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PtyTable {
    /// RDS (everywhere but North America).
    Rds,
    /// RBDS (North America).
    Rbds,
    /// DAB, everywhere but North America.
    Dab,
    /// DAB in North America.
    DabNorthAmerica,
    /// DRM (the FAC programme type).
    Drm,
}

impl PtyTable {
    /// The DAB table named by an International Table Identifier (TS 101 756 Table 11): 0x01 or 0x02.
    pub const fn from_dab_table_id(id: u8) -> Option<PtyTable> {
        match id {
            0x01 => Some(PtyTable::Dab),
            0x02 => Some(PtyTable::DabNorthAmerica),
            _ => None,
        }
    }
}

/// One programme type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pty {
    /// The name as the standard's table writes it.
    pub name: &'static str,
    /// The text for 8-character displays, when the standard provides one.
    pub short8: Option<&'static str>,
    /// The text for 16-character displays, when the standard provides one.
    pub short16: Option<&'static str>,
}

const fn p(name: &'static str, short8: &'static str, short16: &'static str) -> Option<Pty> {
    Some(Pty {
        name,
        short8: if short8.is_empty() {
            None
        } else {
            Some(short8)
        },
        short16: if short16.is_empty() {
            None
        } else {
            Some(short16)
        },
    })
}

static RDS: [Option<Pty>; 32] = [
    p("No program type or undefined", "None", "None"),
    p("News", "News", "News"),
    p("Current Affairs", "Affairs", "Current Affairs"),
    p("Information", "Info", "Information"),
    p("Sport", "Sport", "Sport"),
    p("Education", "Educate", "Education"),
    p("Drama", "Drama", "Drama"),
    p("Culture", "Culture", "Cultures"),
    p("Science", "Science", "Science"),
    p("Varied", "Varied", "Varied Speech"),
    p("Pop Music", "Pop M", "Pop Music"),
    p("Rock Music", "Rock M", "Rock Music"),
    p("Easy Listening Music", "Easy M", "Easy Listening"),
    p("Light classical", "Light M", "Light Classics M"),
    p("Serious classical", "Classics", "Serious Classics"),
    p("Other Music", "Other M", "Other Music"),
    p("Weather", "Weather", "Weather & Metr"),
    p("Finance", "Finance", "Finance"),
    p("Children's programs", "Children", "Children's Progs"),
    p("Social Affairs", "Social", "Social Affairs"),
    p("Religion", "Religion", "Religion"),
    p("Phone In", "Phone In", "Phone In"),
    p("Travel", "Travel", "Travel & Touring"),
    p("Leisure", "Leisure", "Leisure & Hobby"),
    p("Jazz Music", "Jazz", "Jazz Music"),
    p("Country Music", "Country", "Country Music"),
    p("National Music", "Nation M", "National Music"),
    p("Oldies Music", "Oldies", "Oldies Music"),
    p("Folk Music", "Folk M", "Folk Music"),
    p("Documentary", "Document", "Documentary"),
    p("Alarm Test", "TEST", "Alarm Test"),
    p("Alarm", "Alarm !", "Alarm - Alarm !"),
];

static RBDS: [Option<Pty>; 32] = [
    p("No program type or undefined", "None", "None"),
    p("News", "News", "News"),
    p("Information", "Inform", "Information"),
    p("Sports", "Sports", "Sports"),
    p("Talk", "Talk", "Talk"),
    p("Rock", "Rock", "Rock"),
    p("Classic Rock", "Cls Rock", "Classic Rock"),
    p("Adult Hits", "Adlt Hit", "Adult Hits"),
    p("Soft Rock", "Soft Rck", "Soft Rock"),
    p("Top 40", "Top 40", "Top 40"),
    p("Country", "Country", "Country"),
    p("Oldies", "Oldies", "Oldies"),
    p("Soft", "Soft", "Soft"),
    p("Nostalgia", "Nostalga", "Nostalgia"),
    p("Jazz", "Jazz", "Jazz"),
    p("Classical", "Classicl", "Classical"),
    p("Rhythm and Blues", "R & B", "Rhythm and Blues"),
    p("Soft Rhythm and Blues", "Soft R&B", "Soft R & B"),
    p("Foreign Language", "Language", "Foreign Language"),
    p("Religious Music", "Rel Musc", "Religious Music"),
    p("Religious Talk", "Rel Talk", "Religious Talk"),
    p("Personality", "Persnlty", "Personality"),
    p("Public", "Public", "Public"),
    p("College", "College", "College"),
    p("Spanish Talk", "", ""),
    p("Spanish Music", "", ""),
    p("Hip Hop", "", ""),
    None,
    None,
    p("Weather", "Weather", "Weather"),
    p("Emergency Test", "Test", "Emergency Test"),
    p("Emergency", "ALERT !", "ALERT! ALERT!"),
];

static DAB: [Option<Pty>; 32] = [
    p("No programme type", "None", "None"),
    p("News", "News", "News"),
    p("Current Affairs", "Affairs", "Current Affairs"),
    p("Information", "Info", "Information"),
    p("Sport", "Sport", "Sport"),
    p("Education", "Educate", "Education"),
    p("Drama", "Drama", "Drama"),
    p("Culture", "Arts", "Arts"),
    p("Science", "Science", "Science"),
    p("Varied", "Talk", "Talk"),
    p("Pop Music", "Pop", "Pop Music"),
    p("Rock Music", "Rock", "Rock Music"),
    p("Easy Listening Music", "Easy", "Easy Listening"),
    p("Light Classical", "Classics", "Light Classical"),
    p("Serious Classical", "Classics", "Classical Music"),
    p("Other Music", "Music", "Music"),
    p("Weather/meteorology", "Weather", "Weather"),
    p("Finance/Business", "Finance", "Finance"),
    p("Children's programmes", "Children", "Children's"),
    p("Social Affairs", "Factual", "Factual"),
    p("Religion", "Religion", "Religion"),
    p("Phone In", "Phone In", "Phone In"),
    p("Travel", "Travel", "Travel"),
    p("Leisure", "Leisure", "Leisure"),
    p("Jazz Music", "Jazz", "Jazz and Blues"),
    p("Country Music", "Country", "Country Music"),
    p("National Music", "Nation M", "National Music"),
    p("Oldies Music", "Oldies", "Oldies Music"),
    p("Folk Music", "Folk", "Folk Music"),
    p("Documentary", "Document", "Documentary"),
    None,
    None,
];

static DAB_NA: [Option<Pty>; 32] = [
    p("No program type", "None", "None"),
    p("News", "News", "News"),
    p("Information", "Inform", "Information"),
    p("Sports", "Sports", "Sports"),
    p("Talk", "Talk", "Talk"),
    p("Rock", "Rock", "Rock"),
    p("Classic Rock", "Cls Rock", "Classic Rock"),
    p("Adult Hits", "Adlt Hit", "Adult Hits"),
    p("Soft Rock", "Soft Rck", "Soft Rock"),
    p("Top 40", "Top 40", "Top 40"),
    p("Country", "Country", "Country"),
    p("Oldies", "Oldies", "Oldies"),
    p("Soft", "Soft", "Soft"),
    p("Nostalgia", "Nostalga", "Nostalgia"),
    p("Jazz", "Jazz", "Jazz"),
    p("Classical", "Classicl", "Classical"),
    p("Rhythm and Blues", "R & B", "Rhythm and Blues"),
    p("Soft Rhythm and Blues", "Soft R&B", "Soft R & B"),
    p("Foreign Language", "Language", "Foreign Language"),
    p("Religious Music", "Rel Musc", "Religious Music"),
    p("Religious Talk", "Rel Talk", "Religious Talk"),
    p("Personality", "Persnlty", "Personality"),
    p("Public", "Public", "Public"),
    p("College", "College", "College"),
    None,
    None,
    None,
    None,
    None,
    p("Weather", "Weather", "Weather"),
    None,
    None,
];

static DRM: [Option<Pty>; 32] = [
    p("No programme type", "", ""),
    p("News", "", ""),
    p("Current Affairs", "", ""),
    p("Information", "", ""),
    p("Sport", "", ""),
    p("Education", "", ""),
    p("Drama", "", ""),
    p("Culture", "", ""),
    p("Science", "", ""),
    p("Varied", "", ""),
    p("Pop Music", "", ""),
    p("Rock Music", "", ""),
    p("Easy Listening Music", "", ""),
    p("Light Classical", "", ""),
    p("Serious Classical", "", ""),
    p("Other Music", "", ""),
    p("Weather/meteorology", "", ""),
    p("Finance/Business", "", ""),
    p("Children's programmes", "", ""),
    p("Social Affairs", "", ""),
    p("Religion", "", ""),
    p("Phone In", "", ""),
    p("Travel", "", ""),
    p("Leisure", "", ""),
    p("Jazz Music", "", ""),
    p("Country Music", "", ""),
    p("National Music", "", ""),
    p("Oldies Music", "", ""),
    p("Folk Music", "", ""),
    p("Documentary", "", ""),
    None,
    None,
];

/// The programme type of `code` in `table`, `None` for a code above 31 or one the table does not assign.
pub fn pty(code: u8, table: PtyTable) -> Option<Pty> {
    let t = match table {
        PtyTable::Rds => &RDS,
        PtyTable::Rbds => &RBDS,
        PtyTable::Dab => &DAB,
        PtyTable::DabNorthAmerica => &DAB_NA,
        PtyTable::Drm => &DRM,
    };
    t.get(code as usize).copied().flatten()
}

/// The name of programme type `code` in `table`.
pub fn name(code: u8, table: PtyTable) -> Option<&'static str> {
    pty(code, table).map(|p| p.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rds_and_rbds_from_nrsc4_annex_f() {
        assert_eq!(name(0, PtyTable::Rds), Some("No program type or undefined"));
        assert_eq!(pty(14, PtyTable::Rds).unwrap().short8, Some("Classics"));
        assert_eq!(pty(30, PtyTable::Rds).unwrap().short8, Some("TEST"));
        assert_eq!(name(17, PtyTable::Rbds), Some("Soft Rhythm and Blues"));
        assert_eq!(pty(17, PtyTable::Rbds).unwrap().short8, Some("Soft R&B"));
        assert_eq!(name(24, PtyTable::Rbds), Some("Spanish Talk"));
        assert_eq!(name(26, PtyTable::Rbds), Some("Hip Hop"));
        assert_eq!(pty(26, PtyTable::Rbds).unwrap().short8, None);
        assert_eq!(name(27, PtyTable::Rbds), None);
        assert_eq!(name(31, PtyTable::Rbds), Some("Emergency"));
        assert_eq!(name(32, PtyTable::Rds), None);
    }

    #[test]
    fn dab_and_drm() {
        assert_eq!(name(11, PtyTable::Dab), Some("Rock Music"));
        assert_eq!(pty(7, PtyTable::Dab).unwrap().short16, Some("Arts"));
        assert_eq!(pty(9, PtyTable::Dab).unwrap().short8, Some("Talk"));
        assert_eq!(name(30, PtyTable::Dab), None);
        assert_eq!(name(23, PtyTable::DabNorthAmerica), Some("College"));
        assert_eq!(name(24, PtyTable::DabNorthAmerica), None);
        assert_eq!(name(29, PtyTable::DabNorthAmerica), Some("Weather"));
        assert_eq!(name(16, PtyTable::Drm), Some("Weather/meteorology"));
        assert_eq!(name(31, PtyTable::Drm), None);
        assert_eq!(
            PtyTable::from_dab_table_id(2),
            Some(PtyTable::DabNorthAmerica)
        );
        assert_eq!(PtyTable::from_dab_table_id(3), None);
        for t in [
            PtyTable::Rds,
            PtyTable::Rbds,
            PtyTable::Dab,
            PtyTable::DabNorthAmerica,
        ] {
            for c in 0..32 {
                if let Some(p) = pty(c, t) {
                    assert!(p.short8.is_none_or(|s| s.chars().count() <= 8), "{t:?} {c}");
                    assert!(
                        p.short16.is_none_or(|s| s.chars().count() <= 16),
                        "{t:?} {c}"
                    );
                }
            }
        }
    }
}

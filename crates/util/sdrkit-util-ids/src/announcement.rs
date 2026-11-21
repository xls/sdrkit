//! DAB announcement types: the 16 bits of the announcement support (ASu) and switching (ASw) flags, ETSI EN 300 401 clause
//! 8.1.6, registered in ETSI TS 101 756 V2.4.1 Table 14 (types), with the display texts of Table 15. Bits 11 to 15 are reserved.
//!
//! ```
//! use sdrkit_util_ids::announcement;
//!
//! assert_eq!(announcement::name(1), Some("Road Traffic flash"));
//! let on: Vec<&str> = announcement::flags(0b0000_0000_0001_0011).map(|a| a.short8).collect();
//! assert_eq!(on, ["Alarm", "Traffic", "News"]);
//! ```

/// One announcement type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Announcement {
    /// The flag bit, 0 to 10.
    pub bit: u8,
    /// The name (Table 14).
    pub name: &'static str,
    /// The text for 8-character displays (Table 15).
    pub short8: &'static str,
    /// The text for 16-character displays (Table 15).
    pub short16: &'static str,
}

const fn a(
    bit: u8,
    name: &'static str,
    short8: &'static str,
    short16: &'static str,
) -> Announcement {
    Announcement {
        bit,
        name,
        short8,
        short16,
    }
}

/// Every announcement type, by bit.
pub static ANNOUNCEMENTS: [Announcement; 11] = [
    a(0, "Alarm", "Alarm", "Alarm"),
    a(1, "Road Traffic flash", "Traffic", "Traffic News"),
    a(2, "Transport flash", "Travel", "Transport News"),
    a(3, "Warning/Service", "Warning", "Warning"),
    a(4, "News flash", "News", "News"),
    a(5, "Area weather flash", "Weather", "Weather"),
    a(6, "Event announcement", "Event", "Event"),
    a(7, "Special event", "Special", "Special event"),
    a(8, "Programme Information", "Rad info", "Radio info"),
    a(9, "Sport report", "Sports", "Sports news"),
    a(10, "Financial report", "Finance", "Financial news"),
];

/// The announcement type of a flag bit (0 to 15); `None` for the reserved bits 11 to 15.
pub fn get(bit: u8) -> Option<Announcement> {
    ANNOUNCEMENTS.get(bit as usize).copied()
}

/// The name of the announcement type of a flag bit.
pub fn name(bit: u8) -> Option<&'static str> {
    get(bit).map(|a| a.name)
}

/// The announcement types whose bits are set in a 16-bit ASu or ASw field, lowest bit first. Reserved bits are skipped.
pub fn flags(field: u16) -> impl Iterator<Item = Announcement> {
    ANNOUNCEMENTS
        .iter()
        .copied()
        .filter(move |a| field & (1 << a.bit) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_14_and_15() {
        assert_eq!(name(0), Some("Alarm"));
        assert_eq!(name(10), Some("Financial report"));
        assert_eq!(get(8).unwrap().short8, "Rad info");
        assert_eq!(get(9).unwrap().short16, "Sports news");
        assert_eq!(name(11), None);
        assert_eq!(flags(0xF800).count(), 0, "reserved bits only");
        assert_eq!(flags(0xFFFF).count(), 11);
        assert!(ANNOUNCEMENTS
            .iter()
            .enumerate()
            .all(|(i, a)| a.bit as usize == i));
    }
}

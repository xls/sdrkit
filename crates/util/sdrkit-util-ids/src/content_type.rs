//! RadioText Plus (RDS) and DL Plus (DAB) content types 0 to 63. ETSI TS 102 980 V2.1.2 Annex A Table A.1 defines them and states
//! that types 0 to 63 are the same in both; types 64 to 127 are reserved for DL Plus. The class names are the standard's; the
//! labels are short English forms of its descriptions.
//!
//! Type 23 is `INFO.TV` ("information about TV-movies") in TS 102 980; early RT+ texts called the same class
//! `INFO.STUPIDITY_MACHINE`.
//!
//! ```
//! use sdrkit_util_ids::content_type;
//!
//! let t = content_type::get(4).unwrap();
//! assert_eq!((t.class, t.label), ("ITEM.ARTIST", "Artist"));
//! assert_eq!(content_type::get(57).unwrap().class, "PRIVATE");
//! assert_eq!(content_type::get(54), None);
//! ```

/// One content type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentType {
    /// The code.
    pub code: u8,
    /// The class name of Table A.1 ("ITEM.TITLE").
    pub class: &'static str,
    /// A short English label ("Title").
    pub label: &'static str,
}

const fn c(code: u8, class: &'static str, label: &'static str) -> ContentType {
    ContentType { code, class, label }
}

/// Every assigned content type, by code (54 and 55 are reserved and left out; 56 to 58 are the private classes).
pub static CONTENT_TYPES: [ContentType; 62] = [
    c(0, "DUMMY", "Dummy"),
    c(1, "ITEM.TITLE", "Title"),
    c(2, "ITEM.ALBUM", "Album"),
    c(3, "ITEM.TRACKNUMBER", "Track number"),
    c(4, "ITEM.ARTIST", "Artist"),
    c(5, "ITEM.COMPOSITION", "Composition"),
    c(6, "ITEM.MOVEMENT", "Movement"),
    c(7, "ITEM.CONDUCTOR", "Conductor"),
    c(8, "ITEM.COMPOSER", "Composer"),
    c(9, "ITEM.BAND", "Band"),
    c(10, "ITEM.COMMENT", "Comment"),
    c(11, "ITEM.GENRE", "Genre"),
    c(12, "INFO.NEWS", "News"),
    c(13, "INFO.NEWS.LOCAL", "Local news"),
    c(14, "INFO.STOCKMARKET", "Stock market"),
    c(15, "INFO.SPORT", "Sport"),
    c(16, "INFO.LOTTERY", "Lottery"),
    c(17, "INFO.HOROSCOPE", "Horoscope"),
    c(18, "INFO.DAILY_DIVERSION", "Daily diversion"),
    c(19, "INFO.HEALTH", "Health"),
    c(20, "INFO.EVENT", "Event"),
    c(21, "INFO.SCENE", "Scene"),
    c(22, "INFO.CINEMA", "Cinema"),
    c(23, "INFO.TV", "TV"),
    c(24, "INFO.DATE_TIME", "Date and time"),
    c(25, "INFO.WEATHER", "Weather"),
    c(26, "INFO.TRAFFIC", "Traffic"),
    c(27, "INFO.ALARM", "Alarm"),
    c(28, "INFO.ADVERTISEMENT", "Advertisement"),
    c(29, "INFO.URL", "URL"),
    c(30, "INFO.OTHER", "Other information"),
    c(31, "STATIONNAME.SHORT", "Station name"),
    c(32, "STATIONNAME.LONG", "Station name (long)"),
    c(33, "PROGRAMME.NOW", "Programme now"),
    c(34, "PROGRAMME.NEXT", "Programme next"),
    c(35, "PROGRAMME.PART", "Programme part"),
    c(36, "PROGRAMME.HOST", "Host"),
    c(37, "PROGRAMME.EDITORIAL_STAFF", "Editorial staff"),
    c(38, "PROGRAMME.FREQUENCY", "Frequency"),
    c(39, "PROGRAMME.HOMEPAGE", "Homepage"),
    c(40, "PROGRAMME.SUBCHANNEL", "Sub-channel"),
    c(41, "PHONE.HOTLINE", "Hotline phone"),
    c(42, "PHONE.STUDIO", "Studio phone"),
    c(43, "PHONE.OTHER", "Other phone"),
    c(44, "SMS.STUDIO", "Studio SMS"),
    c(45, "SMS.OTHER", "Other SMS"),
    c(46, "EMAIL.HOTLINE", "Hotline email"),
    c(47, "EMAIL.STUDIO", "Studio email"),
    c(48, "EMAIL.OTHER", "Other email"),
    c(49, "MMS.OTHER", "Other MMS"),
    c(50, "CHAT", "Chat"),
    c(51, "CHAT.CENTER", "Chat centre"),
    c(52, "VOTE.QUESTION", "Vote question"),
    c(53, "VOTE.CENTRE", "Vote centre"),
    c(56, "PRIVATE", "Private class 1"),
    c(57, "PRIVATE", "Private class 2"),
    c(58, "PRIVATE", "Private class 3"),
    c(59, "DESCRIPTOR.PLACE", "Place"),
    c(60, "DESCRIPTOR.APPOINTMENT", "Appointment"),
    c(61, "DESCRIPTOR.IDENTIFIER", "Identifier"),
    c(62, "DESCRIPTOR.PURCHASE", "Purchase"),
    c(63, "DESCRIPTOR.GET_DATA", "Get data"),
];

/// The content type of a code; `None` for the reserved codes 54, 55 and 64 to 127.
pub fn get(code: u8) -> Option<ContentType> {
    CONTENT_TYPES
        .binary_search_by_key(&code, |t| t.code)
        .ok()
        .map(|i| CONTENT_TYPES[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_a1() {
        assert!(CONTENT_TYPES.windows(2).all(|w| w[0].code < w[1].code));
        assert_eq!(get(1).unwrap().class, "ITEM.TITLE");
        assert_eq!(get(23).unwrap().class, "INFO.TV");
        assert_eq!(get(31).unwrap().class, "STATIONNAME.SHORT");
        assert_eq!(get(53).unwrap().class, "VOTE.CENTRE");
        assert_eq!(get(63).unwrap().class, "DESCRIPTOR.GET_DATA");
        assert_eq!(get(55), None);
        assert_eq!(get(64), None);
    }
}

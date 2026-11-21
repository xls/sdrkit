//! Language codes: ISO 639-2 (DVB service information and DAB use the three-letter codes), and the one-byte language codes of
//! DAB and RDS (ETSI TS 101 756 V2.4.1 Tables 9 and 10; RDS uses the same codes) and of the DRM FAC (ETSI ES 201 980 V4.2.1
//! Table 18).
//!
//! **ISO 639-2 data.** `data/iso639_2.tsv` is the Library of Congress list (the ISO 639-2 Registration Authority),
//! <https://www.loc.gov/standards/iso639-2/ISO-639-2_utf-8.txt>, file of 29 September 2025: 486 codes with their bibliographic
//! (B) and terminology (T) forms, ISO 639-1 two-letter codes and English names. EN 300 468 allows either the B or the T form
//! ("ger" or "deu"); both are found.
//!
//! ```
//! use sdrkit_util_ids::language;
//!
//! assert_eq!(language::iso639("deu"), Some("German"));
//! assert_eq!(language::iso639("ger"), Some("German"));
//! assert_eq!(language::iso639("de"), Some("German"));
//! assert_eq!(language::broadcast(0x09), Some("English"));
//! assert_eq!(language::drm(3), Some("Chinese (Mandarin)"));
//! ```

use crate::pool::{self, find};

fn pack(code: &[u8]) -> Option<u32> {
    let mut v = 0u32;
    for &b in code {
        if !b.is_ascii_alphabetic() {
            return None;
        }
        v = (v << 8) | b.to_ascii_lowercase() as u32;
    }
    Some(v)
}

/// Returns the English name of an ISO 639-2 code (B or T form, either case) or ISO 639-1 code. `qaa` to `qtz` return "Reserved
/// for local use"; DVB's `und` returns "Undetermined".
pub fn iso639(code: &str) -> Option<&'static str> {
    let b = code.trim().as_bytes();
    let row = match b.len() {
        3 => {
            let k = pack(b)?;
            if (pack(b"qaa")?..=pack(b"qtz")?).contains(&k) {
                return Some("Reserved for local use");
            }
            find(&pool::LANG_B, &k).or_else(|| {
                pool::LANG_BY_T
                    .binary_search_by_key(&k, |e| e.0)
                    .ok()
                    .map(|i| pool::LANG_BY_T[i].1 as usize)
            })?
        }
        2 => {
            let k = pack(b)?;
            pool::LANG_BY_A2
                .binary_search_by_key(&k, |e| e.0)
                .ok()
                .map(|i| pool::LANG_BY_A2[i].1 as usize)?
        }
        _ => return None,
    };
    Some(pool::LANG_POOL.get(pool::LANG_NAME[row]))
}

/// One ISO 639-2 language with all its codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Language {
    /// The bibliographic code (639-2/B), lower case.
    pub bibliographic: [u8; 3],
    /// The terminology code (639-2/T); the same as the B code for most languages.
    pub terminology: [u8; 3],
    /// The ISO 639-1 code, when the language has one.
    pub alpha2: Option<[u8; 2]>,
    /// The English name.
    pub name: &'static str,
}

fn unpack3(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// Returns the language of any of its codes, with all its codes: `iso639_entry("deu")` returns B "ger", T "deu", 639-1 "de".
pub fn iso639_entry(code: &str) -> Option<Language> {
    let name = iso639(code)?;
    let b = code.trim().as_bytes();
    let k = pack(b)?;
    let row = match b.len() {
        3 => find(&pool::LANG_B, &k).or_else(|| {
            pool::LANG_BY_T
                .binary_search_by_key(&k, |e| e.0)
                .ok()
                .map(|i| pool::LANG_BY_T[i].1 as usize)
        })?,
        _ => pool::LANG_BY_A2
            .binary_search_by_key(&k, |e| e.0)
            .ok()
            .map(|i| pool::LANG_BY_A2[i].1 as usize)?,
    };
    let bib = unpack3(pool::LANG_B[row]);
    let t = pool::LANG_T[row];
    let a2 = pool::LANG_A2[row];
    Some(Language {
        bibliographic: bib,
        terminology: if t == 0 { bib } else { unpack3(t) },
        alpha2: (a2 != 0).then_some([(a2 >> 8) as u8, a2 as u8]),
        name,
    })
}

/// The ISO 639-2 code a DVB descriptor carries as three bytes (ISO 8859-1 letters).
pub fn iso639_bytes(code: [u8; 3]) -> Option<&'static str> {
    std::str::from_utf8(&code).ok().and_then(iso639)
}

/// The number of ISO 639-2 codes in the table.
pub fn iso639_count() -> usize {
    pool::LANG_B.len()
}

/// TS 101 756 Table 9: European Latin-based languages, codes 0x00 to 0x2B.
static EUROPEAN: [&str; 44] = [
    "Unknown/not applicable",
    "Albanian",
    "Breton",
    "Catalan",
    "Croatian",
    "Welsh",
    "Czech",
    "Danish",
    "German",
    "English",
    "Spanish",
    "Esperanto",
    "Estonian",
    "Basque",
    "Faroese",
    "French",
    "Frisian",
    "Irish",
    "Gaelic",
    "Galician",
    "Icelandic",
    "Italian",
    "Sami",
    "Latin",
    "Latvian",
    "Luxembourgian",
    "Lithuanian",
    "Hungarian",
    "Maltese",
    "Dutch",
    "Norwegian",
    "Occitan",
    "Polish",
    "Portuguese",
    "Romanian",
    "Romansh",
    "Serbian",
    "Slovak",
    "Slovene",
    "Finnish",
    "Swedish",
    "Turkish",
    "Flemish",
    "Walloon",
];

/// TS 101 756 Table 10: other languages, codes 0x7F down to 0x45 (0x41 to 0x44 are reserved), with the table's spellings.
static OTHER: [&str; 59] = [
    "Amharic",
    "Arabic",
    "Armenian",
    "Assamese",
    "Azerbaijani",
    "Bambora",
    "Belorussian",
    "Bengali",
    "Bulgarian",
    "Burmese",
    "Chinese",
    "Chuvash",
    "Dari",
    "Fulani",
    "Georgian",
    "Greek",
    "Gujurati",
    "Gurani",
    "Hausa",
    "Hebrew",
    "Hindi",
    "Indonesian",
    "Japanese",
    "Kannada",
    "Kazakh",
    "Khmer",
    "Korean",
    "Laotian",
    "Macedonian",
    "Malagasay",
    "Malaysian",
    "Moldavian",
    "Marathi",
    "Ndebele",
    "Nepali",
    "Oriya",
    "Papiamento",
    "Persian",
    "Punjabi",
    "Pushtu",
    "Quechua",
    "Russian",
    "Rusyn",
    "Serbo-Croat",
    "Shona",
    "Sinhalese",
    "Somali",
    "Sranan Tongo",
    "Swahili",
    "Tadzhik",
    "Tamil",
    "Tatar",
    "Telugu",
    "Thai",
    "Ukranian",
    "Urdu",
    "Uzbek",
    "Vietnamese",
    "Zulu",
];

/// The language of a DAB or RDS language code (TS 101 756 Tables 9 and 10). 0x40 is "Background sound/clean feed"; 0x30 to
/// 0x3F are "Reserved for national assignment"; the reserved codes 0x2C to 0x2F and 0x41 to 0x44 and codes above 0x7F give
/// `None`.
pub fn broadcast(code: u8) -> Option<&'static str> {
    match code {
        0x00..=0x2B => Some(EUROPEAN[code as usize]),
        0x30..=0x3F => Some("Reserved for national assignment"),
        0x40 => Some("Background sound/clean feed"),
        0x45..=0x7F => Some(OTHER[(0x7F - code) as usize]),
        _ => None,
    }
}

/// ES 201 980 Table 18: the DRM FAC language codes.
static DRM: [&str; 16] = [
    "No language specified",
    "Arabic",
    "Bengali",
    "Chinese (Mandarin)",
    "Dutch",
    "English",
    "French",
    "German",
    "Hindi",
    "Japanese",
    "Javanese",
    "Korean",
    "Portuguese",
    "Russian",
    "Spanish",
    "Other language",
];

/// The language of a DRM FAC language code (4 bits; `None` above 15).
pub fn drm(code: u8) -> Option<&'static str> {
    DRM.get(code as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso639_from_the_loc_list() {
        assert_eq!(iso639("eng"), Some("English"));
        assert_eq!(iso639("ENG"), Some("English"));
        assert_eq!(iso639("fre"), Some("French"));
        assert_eq!(iso639("fra"), Some("French"));
        assert_eq!(iso639("fr"), Some("French"));
        assert_eq!(iso639("chi"), Some("Chinese"));
        assert_eq!(iso639("zho"), Some("Chinese"));
        assert_eq!(iso639("und"), Some("Undetermined"));
        assert_eq!(iso639("mul"), Some("Multiple languages"));
        assert_eq!(iso639("qab"), Some("Reserved for local use"));
        assert_eq!(iso639("xxx"), None);
        assert_eq!(iso639("e1g"), None);
        assert_eq!(iso639("english"), None);
        assert_eq!(iso639_bytes(*b"spa"), Some("Spanish; Castilian"));
        assert_eq!(iso639_bytes([0xFF, 0x61, 0x61]), None);
        assert_eq!(iso639_count(), 486);
        let de = iso639_entry("de").unwrap();
        assert_eq!(
            (&de.bibliographic, &de.terminology, de.alpha2),
            (b"ger", b"deu", Some(*b"de"))
        );
        let en = iso639_entry("ENG").unwrap();
        assert_eq!((&en.terminology, en.alpha2), (b"eng", Some(*b"en")));
        assert_eq!(iso639_entry("ace").unwrap().alpha2, None);
        assert_eq!(iso639_entry("qaa"), None, "local codes have no entry");
    }

    #[test]
    fn ts101756_tables_9_and_10() {
        assert_eq!(broadcast(0x00), Some("Unknown/not applicable"));
        assert_eq!(broadcast(0x08), Some("German"));
        assert_eq!(broadcast(0x2B), Some("Walloon"));
        assert_eq!(broadcast(0x2C), None);
        assert_eq!(broadcast(0x35), Some("Reserved for national assignment"));
        assert_eq!(broadcast(0x7F), Some("Amharic"));
        assert_eq!(broadcast(0x75), Some("Chinese"));
        assert_eq!(broadcast(0x60), Some("Moldavian"));
        assert_eq!(broadcast(0x5F), Some("Marathi"));
        assert_eq!(broadcast(0x56), Some("Russian"));
        assert_eq!(broadcast(0x45), Some("Zulu"));
        assert_eq!(broadcast(0x44), None);
        assert_eq!(broadcast(0x80), None);
        assert_eq!(drm(0), Some("No language specified"));
        assert_eq!(drm(15), Some("Other language"));
        assert_eq!(drm(16), None);
    }
}

//! Finding amateur and other station callsigns in decoded free text.
//!
//! A callsign follows the form of the ITU Radio Regulations, Article 19 (19.68): a prefix of one character (B, F, G, I, K, M, N,
//! R, W or 2) or of two or three characters including a letter, then one digit, then up to four characters of which the last is a
//! letter. Portable and location parts separated by `/` (`DL1XYZ/P`, `OH2/LY2ABC`) are kept with the call; the longest part is the
//! one checked. Only upper case is accepted, as keyboard protocols send calls in capitals; the word must stand alone (letters, digits
//! and `/` only).
//!
//! ```
//! use sdrkit_util_text::callsign;
//!
//! let calls: Vec<&str> = callsign::find("CQ CQ DE K0TST/P K0TST 599 5NN TU").collect();
//! assert_eq!(calls, ["K0TST/P", "K0TST"]);
//! ```

/// The longest word considered.
pub const MAX_LEN: usize = 16;

fn is_call_byte(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'/'
}

/// Whether a base call (no `/`) has the Article 19 form.
fn is_base(s: &[u8]) -> bool {
    if !(3..=8).contains(&s.len())
        || !s
            .iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return false;
    }
    (1..=3).any(|p| {
        if p >= s.len() || !s[p].is_ascii_digit() {
            return false;
        }
        let prefix = &s[..p];
        let suffix = &s[p + 1..];
        let prefix_ok = match p {
            1 => b"BFGIKMNRW2".contains(&prefix[0]),
            _ => prefix.iter().any(|c| c.is_ascii_uppercase()),
        };
        prefix_ok
            && (1..=4).contains(&suffix.len())
            && suffix.last().is_some_and(|c| c.is_ascii_uppercase())
    })
}

/// Whether a word is a callsign.
pub fn is_callsign(word: &str) -> bool {
    let w = word.as_bytes();
    if w.len() < 3
        || w.len() > MAX_LEN
        || !w.iter().all(|&b| is_call_byte(b))
        || w[0] == b'/'
        || w[w.len() - 1] == b'/'
    {
        return false;
    }
    let mut parts = word.split('/');
    let longest = parts.clone().max_by_key(|p| p.len()).unwrap_or("");
    if parts.any(|p| p.is_empty() || p.len() > 8) {
        return false;
    }
    is_base(longest.as_bytes())
}

/// The callsigns in a text, in order (a call that appears twice is given twice).
pub fn find(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '/'))
        .filter(|w| is_callsign(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_19_forms() {
        for c in [
            "LY2ABC",
            "K1ABC",
            "9A1AA",
            "DL1XYZ/P",
            "2E0ABC",
            "W1AW",
            "OH2/LY2ABC",
            "VK2ABC/MM",
            "K0TST",
            "3DA0RS",
            "G4ABCD",
            "VP8/G3XYZ",
            "B2B",
            "JA1ZZZ",
            "M0ABC",
        ] {
            assert!(is_callsign(c), "{c}");
        }
        for c in [
            "5NN", "TU", "CQ", "599", "TEST", "73", "QRZ", "ABCDEFG", "K", "MP3", "R2D2", "4X4",
            "A1", "k0tst", "/K0TST", "K0TST/", "C1ABC", "Q1ABC", "K1ABCDE",
        ] {
            assert!(!is_callsign(c), "{c}");
        }
    }

    #[test]
    fn finding() {
        assert_eq!(find("CQ CQ DE K0TST K").collect::<Vec<_>>(), ["K0TST"]);
        assert_eq!(
            find("LY2ABC DE DL1XYZ K").collect::<Vec<_>>(),
            ["LY2ABC", "DL1XYZ"]
        );
        assert_eq!(find("5 R 9A1AA, TNX").collect::<Vec<_>>(), ["9A1AA"]);
        assert_eq!(find("THE QUICK BROWN FOX").count(), 0);
    }
}

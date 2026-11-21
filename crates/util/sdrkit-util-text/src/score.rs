//! Scores of decoded text: how much a stretch of decoded characters looks like plain text. Keyboard stacks compare candidate
//! decoders with them and judge whether a decode still works.
//!
//! ```
//! use sdrkit_util_text::score::{readable_share, teleprinter_score};
//! assert_eq!(readable_share("CQ DE K0TST".chars()), 1.0);
//! assert_eq!(readable_share("".chars()), 0.0);
//! assert!((teleprinter_score("RY\n=".chars()) - 2.8).abs() < 1e-9);
//! ```

/// Whether `c` reads as text: a letter, a digit, a space, a line end (CR or LF) or one of `. , - / ? ' :`.
pub fn is_readable(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            ' ' | '\r' | '\n' | '.' | ',' | '-' | '/' | '?' | '\'' | ':'
        )
}

/// The share of the characters of `text` that [`is_readable`] accepts, 0 to 1. No characters: 0.
pub fn readable_share(text: impl IntoIterator<Item = char>) -> f64 {
    let (mut good, mut all) = (0usize, 0usize);
    for c in text {
        all += 1;
        good += usize::from(is_readable(c));
    }
    if all == 0 {
        0.0
    } else {
        good as f64 / all as f64
    }
}

/// The text score of teleprinter output: 1 for each letter A to Z, digit or space, 0.3 for each CR or LF, 0.5 for each other
/// character. A stretch of real text scores near its length; the output of a wrong speed or polarity scores much less.
pub fn teleprinter_score(text: impl IntoIterator<Item = char>) -> f64 {
    text.into_iter()
        .map(|c| match c {
            'A'..='Z' | '0'..='9' | ' ' => 1.0,
            '\r' | '\n' => 0.3,
            _ => 0.5,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_of_short_texts() {
        assert_eq!(teleprinter_score("THE QUICK".chars()), 9.0);
        assert!((teleprinter_score("a\r\n".chars()) - 1.1).abs() < 1e-9);
        assert_eq!(readable_share("AB#$".chars()), 0.5);
        assert!(is_readable('\''));
        assert!(!is_readable('#'));
    }
}

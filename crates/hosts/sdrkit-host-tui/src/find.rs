//! The search of a list: the lines that hold a typed text, best match first. Upper and lower case are the same.

use ratatui::crossterm::event::KeyCode;

/// Returns how well `line` matches `text`, both in lower case: 0 if the first word of the line is `text`, 1 if the line
/// starts with `text`, 2 if a word of the line starts with `text`, 3 if the line holds `text` elsewhere. Returns `None`
/// if the line does not hold `text`.
fn rank(line: &str, text: &str) -> Option<u8> {
    let word_end = |rest: &str| !rest.chars().next().is_some_and(char::is_alphanumeric);
    if let Some(rest) = line.strip_prefix(text) {
        return Some(u8::from(!word_end(rest)));
    }
    let mut best = None;
    for (at, _) in line.match_indices(text) {
        let before = line[..at].chars().next_back();
        if !before.is_some_and(char::is_alphanumeric) {
            return Some(2);
        }
        best = Some(3);
    }
    best
}

/// Returns the indexes of the lines of `lines` that hold `text`, best match first. Lines that match equally well keep
/// the order of the list. An empty `text` matches no line.
pub fn matches<S: AsRef<str>>(lines: &[S], text: &str) -> Vec<usize> {
    let text = text.to_lowercase();
    if text.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(u8, usize)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| Some((rank(&l.as_ref().to_lowercase(), &text)?, i)))
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, i)| i).collect()
}

/// The text typed to search a list, and the match that the list shows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Find {
    /// The text so far.
    pub text: String,
    /// The index of the shown match among the matches.
    pub at: usize,
}

impl Find {
    /// Carries out the search key `code` on `lines`: a character extends the text, Backspace removes the last character,
    /// Tab shows the next match and Shift-Tab the previous one. Returns the line to select, or `None` if no line matches
    /// or `code` is not a search key. A `/` as the first character is not part of the text.
    pub fn key<S: AsRef<str>>(&mut self, code: KeyCode, lines: &[S]) -> Option<usize> {
        let step = match code {
            KeyCode::Char('/') if self.text.is_empty() => return None,
            KeyCode::Char(c) => {
                self.text.push(c);
                None
            }
            KeyCode::Backspace => {
                self.text.pop();
                None
            }
            KeyCode::Tab => Some(1),
            KeyCode::BackTab => Some(-1),
            _ => return None,
        };
        let found = matches(lines, &self.text);
        let n = found.len() as isize;
        self.at = match step {
            Some(step) if n > 0 => (self.at as isize + step).rem_euclid(n) as usize,
            _ => 0,
        };
        found.get(self.at).copied()
    }

    /// Returns true if `code` is a key that [`Find::key`] carries out.
    pub fn takes(code: KeyCode) -> bool {
        matches!(
            code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Tab | KeyCode::BackTab
        )
    }

    /// Forgets the text.
    pub fn clear(&mut self) {
        self.text.clear();
        self.at = 0;
    }

    /// Returns the number of the shown match and the count of matches in `lines` as text, "(2 of 5)", or "(no match)".
    /// Returns an empty string if no text is typed.
    pub fn count<S: AsRef<str>>(&self, lines: &[S]) -> String {
        if self.text.is_empty() {
            return String::new();
        }
        match matches(lines, &self.text).len() {
            0 => "(no match)".to_owned(),
            n => format!("({} of {n})", self.at.min(n - 1) + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINES: [&str; 5] = [
        "am AM broadcast",
        "dab DAB and DAB+ radio",
        "fm FM broadcast with RDS",
        "nfm Narrow FM voice",
        "wfm-tv Analogue TV sound",
    ];

    #[test]
    fn the_first_word_ranks_before_a_later_word_and_before_a_part_of_a_word() {
        assert_eq!(matches(&LINES, "fm"), [2, 3, 4]);
    }

    #[test]
    fn case_does_not_matter_and_an_empty_text_matches_nothing() {
        assert_eq!(matches(&LINES, "DAB"), [1]);
        assert!(matches(&LINES, "").is_empty());
        assert!(matches(&LINES, "zz").is_empty());
    }

    #[test]
    fn tab_walks_the_matches_and_wraps() {
        let mut f = Find::default();
        assert_eq!(f.key(KeyCode::Char('/'), &LINES), None);
        assert_eq!(f.key(KeyCode::Char('b'), &LINES), Some(0));
        assert_eq!(f.key(KeyCode::Tab, &LINES), Some(2));
        assert_eq!(f.key(KeyCode::Tab, &LINES), Some(1));
        assert_eq!(f.key(KeyCode::Tab, &LINES), Some(0));
        assert_eq!(f.key(KeyCode::BackTab, &LINES), Some(1));
        assert_eq!(f.count(&LINES), "(3 of 3)");
        assert_eq!(f.key(KeyCode::Backspace, &LINES), None);
        assert_eq!(f.count(&LINES), "");
    }
}

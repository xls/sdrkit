//! The line of text that the session screen takes for the first command of a stack.

use ratatui::crossterm::event::KeyCode;

/// What a key did to the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKey {
    /// The key changed the text or moved the cursor, or has no use in the line.
    Stay,
    /// Enter: the command runs with the text.
    Run,
    /// Esc: the keys leave the line. The text stays.
    Leave,
}

/// The line of text, the values chosen for the other text arguments of the command, and why the last run was refused.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Entry {
    /// The text.
    pub text: String,
    /// The place of the cursor, as a count of characters from the start of the text.
    pub cursor: usize,
    /// The chosen values, each with the key of its argument.
    pub chosen: Vec<(String, String)>,
    /// Why the command refused the text, or empty.
    pub error: String,
}

impl Entry {
    /// Returns the byte offset of character `n` of the text.
    fn offset(&self, n: usize) -> usize {
        self.text
            .char_indices()
            .nth(n)
            .map_or(self.text.len(), |(i, _)| i)
    }

    /// Carries out key `code`: a character goes into the text at the cursor; Left, Right, Home and End move the
    /// cursor; Backspace and Delete remove a character. A change of the text removes the error.
    pub fn key(&mut self, code: KeyCode) -> EntryKey {
        let len = self.text.chars().count();
        self.cursor = self.cursor.min(len);
        match code {
            KeyCode::Enter => return EntryKey::Run,
            KeyCode::Esc => return EntryKey::Leave,
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(len),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = len,
            KeyCode::Char(c) => {
                let at = self.offset(self.cursor);
                self.text.insert(at, c);
                self.cursor += 1;
                self.error.clear();
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                let at = self.offset(self.cursor);
                self.text.remove(at);
                self.error.clear();
            }
            KeyCode::Delete if self.cursor < len => {
                let at = self.offset(self.cursor);
                self.text.remove(at);
                self.error.clear();
            }
            _ => {}
        }
        EntryKey::Stay
    }

    /// Returns the value chosen for the argument with `key`.
    pub fn chosen(&self, key: &str) -> Option<&str> {
        let mut all = self.chosen.iter();
        all.find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// Makes `value` the chosen value of the argument with `key`. If `value` is the chosen value already, the argument
    /// has no chosen value after the call. Returns true if the argument has a chosen value after the call.
    pub fn choose(&mut self, key: &str, value: &str) -> bool {
        let same = self.chosen(key) == Some(value);
        self.chosen.retain(|(k, _)| k != key);
        if !same {
            self.chosen.push((key.to_owned(), value.to_owned()));
        }
        !same
    }

    /// Notes that the command ran: the line is empty again. The chosen values stay for the next text.
    pub fn done(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.error.clear();
    }

    /// Notes that the command refused the text with `message`. The text stays, to be corrected.
    pub fn refused(&mut self, message: String) {
        self.error = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Entry {
        let mut e = Entry::default();
        for c in text.chars() {
            assert_eq!(e.key(KeyCode::Char(c)), EntryKey::Stay);
        }
        e
    }

    #[test]
    fn every_character_goes_into_the_text() {
        let e = typed("q 1[]/é");
        assert_eq!(e.text, "q 1[]/é");
        assert_eq!(e.cursor, 7);
    }

    #[test]
    fn the_cursor_keys_edit_inside_the_text() {
        let mut e = typed("héllo");
        e.key(KeyCode::Home);
        e.key(KeyCode::Right);
        e.key(KeyCode::Delete);
        assert_eq!(e.text, "hllo");
        e.key(KeyCode::Char('e'));
        assert_eq!((e.text.as_str(), e.cursor), ("hello", 2));
        e.key(KeyCode::End);
        e.key(KeyCode::Backspace);
        e.key(KeyCode::Left);
        e.key(KeyCode::Backspace);
        assert_eq!((e.text.as_str(), e.cursor), ("hel", 2));
        e.key(KeyCode::Home);
        e.key(KeyCode::Backspace);
        e.key(KeyCode::End);
        e.key(KeyCode::Delete);
        e.key(KeyCode::Right);
        assert_eq!((e.text.as_str(), e.cursor), ("hel", 3));
    }

    #[test]
    fn enter_runs_and_esc_leaves_with_the_text_kept() {
        let mut e = typed("hi");
        assert_eq!(e.key(KeyCode::Esc), EntryKey::Leave);
        assert_eq!(e.key(KeyCode::Enter), EntryKey::Run);
        assert_eq!(e.text, "hi");
    }

    #[test]
    fn a_command_that_ran_clears_the_line_and_keeps_the_chosen_values() {
        let mut e = typed("hi");
        e.choose("channel", "LongFast");
        e.done();
        assert_eq!((e.text.as_str(), e.cursor), ("", 0));
        assert_eq!(e.chosen("channel"), Some("LongFast"));
    }

    #[test]
    fn a_refused_text_stays_with_the_reason_until_it_changes() {
        let mut e = typed("hi");
        e.refused("transmit is off".to_owned());
        assert_eq!(
            (e.text.as_str(), e.error.as_str()),
            ("hi", "transmit is off")
        );
        e.key(KeyCode::Left);
        assert_eq!(e.error, "transmit is off");
        e.key(KeyCode::Char('!'));
        assert_eq!(e.error, "");
    }

    #[test]
    fn the_same_value_chosen_again_clears_the_choice() {
        let mut e = Entry::default();
        assert!(e.choose("channel", "LongFast"));
        assert!(e.choose("channel", "Public"));
        assert_eq!(e.chosen("channel"), Some("Public"));
        assert!(!e.choose("channel", "Public"));
        assert_eq!(e.chosen("channel"), None);
    }
}

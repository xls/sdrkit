//! A list of the identification words heard, which confirms the word that comes back again and again.

/// The longest word kept, bytes.
const WORD_MAX: usize = 16;

#[derive(Clone, Copy, Debug)]
struct Heard {
    text: [u8; WORD_MAX],
    len: u8,
    count: u32,
    time: f64,
}

impl Heard {
    fn as_str(&self) -> &str {
        // Only whole UTF-8 words of at most WORD_MAX bytes go in.
        std::str::from_utf8(&self.text[..self.len as usize]).unwrap_or("")
    }
}

/// The identification words heard: how often each came, and which one is confirmed (heard at least `confirm` times). It holds
/// at most [`IdentTracker::CAP`] words and allocates nothing.
///
/// ```
/// use sdrkit_util_text::morse::IdentTracker;
/// let mut t = IdentTracker::new(2);
/// t.heard("SYD", 1.0);
/// assert_eq!(t.confirmed(), None);
/// t.heard("SYD", 11.0);
/// assert_eq!(t.confirmed(), Some("SYD"));
/// ```
#[derive(Clone, Debug)]
pub struct IdentTracker {
    heard: [Heard; IdentTracker::CAP],
    n: usize,
    confirm: u32,
}

impl Default for IdentTracker {
    fn default() -> Self {
        IdentTracker::new(2)
    }
}

impl IdentTracker {
    /// The most words kept. If a new word comes and the list is full, the least heard word goes.
    pub const CAP: usize = 16;

    /// A tracker that confirms a word after `confirm` hearings (at least 1).
    pub fn new(confirm: u32) -> Self {
        IdentTracker {
            heard: [Heard {
                text: [0; WORD_MAX],
                len: 0,
                count: 0,
                time: 0.0,
            }; IdentTracker::CAP],
            n: 0,
            confirm: confirm.max(1),
        }
    }

    /// The hearings a word needs to be confirmed.
    pub fn confirm(&self) -> u32 {
        self.confirm
    }

    /// Sets the hearings a word needs to be confirmed (at least 1).
    pub fn set_confirm(&mut self, confirm: u32) {
        self.confirm = confirm.max(1);
    }

    /// Counts each word of `text` heard at `time` ("ABC ABC" counts ABC twice). Words with `?` or `*` (unknown characters) and
    /// words longer than 16 bytes are left out.
    pub fn heard(&mut self, text: &str, time: f64) {
        for w in text.split_whitespace() {
            if w.contains(['?', '*']) || w.len() > WORD_MAX {
                continue;
            }
            if let Some(h) = self.heard[..self.n].iter_mut().find(|h| h.as_str() == w) {
                h.count += 1;
                h.time = time;
                continue;
            }
            if self.n == Self::CAP {
                let i = (0..self.n)
                    .min_by_key(|&i| self.heard[i].count)
                    .unwrap_or(0);
                self.heard.copy_within(i + 1..self.n, i);
                self.n -= 1;
            }
            let h = &mut self.heard[self.n];
            h.text[..w.len()].copy_from_slice(w.as_bytes());
            h.len = w.len() as u8;
            h.count = 1;
            h.time = time;
            self.n += 1;
        }
    }

    /// The confirmed identification: the most heard word with at least `confirm` hearings (the latest wins a tie).
    pub fn confirmed(&self) -> Option<&str> {
        self.heard[..self.n]
            .iter()
            .filter(|h| h.count >= self.confirm)
            .max_by(|a, b| a.count.cmp(&b.count).then(a.time.total_cmp(&b.time)))
            .map(Heard::as_str)
    }

    /// The last word heard.
    pub fn last(&self) -> Option<&str> {
        self.heard[..self.n]
            .iter()
            .max_by(|a, b| a.time.total_cmp(&b.time))
            .map(Heard::as_str)
    }

    /// The words heard with their counts, in the order first heard.
    pub fn words(&self) -> impl Iterator<Item = (&str, u32)> {
        self.heard[..self.n].iter().map(|h| (h.as_str(), h.count))
    }

    /// Forgets every word.
    pub fn clear(&mut self) {
        self.n = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirms_after_repeats() {
        let mut t = IdentTracker::new(2);
        t.heard("ABC", 1.0);
        assert_eq!(t.confirmed(), None);
        t.heard("ABC XY? DEF", 2.0);
        assert_eq!(t.confirmed(), Some("ABC"));
        assert_eq!(t.last(), Some("DEF"));
        assert_eq!(t.words().collect::<Vec<_>>(), vec![("ABC", 2), ("DEF", 1)]);
        // A full list drops the least heard word and keeps the order.
        for i in 0..20 {
            t.heard(&format!("W{i}"), 3.0 + i as f64);
        }
        assert_eq!(t.words().count(), IdentTracker::CAP);
        assert_eq!(t.words().next(), Some(("ABC", 2)));
        assert_eq!(t.last(), Some("W19"));
        t.clear();
        assert_eq!(t.confirmed(), None);
    }
}

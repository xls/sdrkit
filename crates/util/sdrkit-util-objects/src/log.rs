//! Capped logs of messages and events.

use crate::persist::Shift;
use crate::seen::Stamp;
use sdrkit_core::Results;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A log that keeps the newest `cap` entries. If the log is full, the oldest entry leaves when a new one comes.
///
/// ```
/// use sdrkit_util_objects::LogRing;
/// let mut log = LogRing::new(2);
/// log.push("a");
/// log.push("b");
/// assert_eq!(log.push("c"), Some("a"));
/// assert_eq!(log.newest_first().copied().collect::<Vec<_>>(), ["c", "b"]);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct LogRing<T> {
    entries: VecDeque<T>,
    cap: usize,
    /// Entries ever added.
    added: u64,
}

impl<T> LogRing<T> {
    /// An empty log of at most `cap` entries (at least 1).
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        LogRing {
            entries: VecDeque::with_capacity(cap),
            cap,
            added: 0,
        }
    }

    /// Adds an entry. Returns the oldest entry if it had to leave.
    pub fn push(&mut self, entry: T) -> Option<T> {
        let gone = if self.entries.len() >= self.cap {
            self.entries.pop_front()
        } else {
            None
        };
        self.entries.push_back(entry);
        self.added += 1;
        gone
    }

    /// The newest entry, to change (an own message whose state moves on).
    pub fn newest_mut(&mut self) -> Option<&mut T> {
        self.entries.back_mut()
    }

    /// The last entry, among the newest `within`, for which `matches` says yes, to change in place (a repeated position whose time
    /// moves, an own message whose delivery state changes).
    pub fn find_recent_mut(
        &mut self,
        within: usize,
        mut matches: impl FnMut(&T) -> bool,
    ) -> Option<&mut T> {
        self.entries
            .iter_mut()
            .rev()
            .take(within)
            .find(|e| matches(e))
    }

    /// The entries, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.entries.iter()
    }

    /// The entries, oldest first, each with its sequence number: the count of entries added before it. A cleared log goes on
    /// counting.
    pub fn iter_seq(&self) -> impl DoubleEndedIterator<Item = (u64, &T)> + ExactSizeIterator {
        let first = self.added - self.entries.len() as u64;
        self.entries
            .iter()
            .enumerate()
            .map(move |(i, e)| (first + i as u64, e))
    }

    /// The entries, oldest first, to change.
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> + ExactSizeIterator {
        self.entries.iter_mut()
    }

    /// The entries, newest first.
    pub fn newest_first(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().rev()
    }

    /// Entries now.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The most entries kept.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Changes the cap (at least 1). If there are more entries, the oldest leave.
    pub fn set_cap(&mut self, cap: usize) {
        self.cap = cap.max(1);
        while self.entries.len() > self.cap {
            self.entries.pop_front();
        }
        self.entries
            .reserve(self.cap.saturating_sub(self.entries.len()));
    }

    /// Forgets every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Takes the entries of `saved` (a restored state), keeping this log's cap (the newest stay).
    pub fn adopt(&mut self, saved: LogRing<T>) {
        self.entries.clear();
        self.added = saved.added;
        let skip = saved.entries.len().saturating_sub(self.cap);
        self.entries.extend(saved.entries.into_iter().skip(skip));
    }
}

impl<T: Shift> Shift for LogRing<T> {
    fn shift_times(&mut self, by_s: f64) {
        self.entries.shift_times(by_s);
    }
}

/// Written as `{"cap": n, "added": n, "entries": [...]}`, oldest first.
impl<T: Serialize> Serialize for LogRing<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Out<'a, T> {
            cap: usize,
            added: u64,
            entries: &'a VecDeque<T>,
        }
        Out {
            cap: self.cap,
            added: self.added,
            entries: &self.entries,
        }
        .serialize(s)
    }
}

/// Reads what [`Serialize`] wrote. The cap is kept with the entries, so a restored log never loses its size.
impl<'de, T: Deserialize<'de>> Deserialize<'de> for LogRing<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct In<T> {
            cap: usize,
            #[serde(default)]
            added: u64,
            entries: VecDeque<T>,
        }
        let i = In::<T>::deserialize(d)?;
        let added = i.added.max(i.entries.len() as u64);
        let mut log = LogRing {
            entries: i.entries,
            cap: 1,
            added,
        };
        log.set_cap(i.cap);
        Ok(log)
    }
}

/// One line of an event log: when, and the text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    /// When.
    pub at: Stamp,
    /// The text.
    pub text: String,
}

impl Shift for LogLine {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl LogRing<LogLine> {
    /// Adds a line to this log and the same text to the results' log (the recent lines every front end shows).
    ///
    /// ```
    /// use sdrkit_core::Results;
    /// use sdrkit_util_objects::{LogRing, Stamp};
    /// let mut log = LogRing::new(300);
    /// let mut r = Results::default();
    /// log.line(Stamp::at(3.0), "cell 505-01 found".into(), &mut r);
    /// assert_eq!(r.log_lines().back().map(String::as_str), Some("cell 505-01 found"));
    /// assert_eq!(log.len(), 1);
    /// ```
    pub fn line(&mut self, at: Stamp, text: String, results: &mut Results) {
        results.log(text.as_str());
        self.push(LogLine { at, text });
    }
}

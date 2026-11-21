//! Bookmarks on the spectrum: a point or a named range with a colour, tuneable, drawn on the spectrum, and kept as CSV.

use crate::error::{LineProblem, MemoryError, Problem};
use crate::freq::{self, UnitHint};
use sdrkit_util_text::csv;
use serde::{Deserialize, Serialize};

/// The most bookmarks a store keeps.
pub const MAX_BOOKMARKS: usize = 10_000;

/// A point or a range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bookmark {
    /// Unique in its store.
    pub id: u64,
    /// The label drawn.
    pub name: String,
    /// The point, or the range's low edge, Hz.
    pub start_hz: f64,
    /// The range's high edge, Hz; `None` for a point.
    pub end_hz: Option<f64>,
    /// `#rrggbb`, empty for the front end's default.
    pub color: String,
    /// The stack to tune in. Empty keeps the current one.
    pub stack: String,
    /// Free notes.
    pub notes: String,
}

impl Default for Bookmark {
    fn default() -> Self {
        Bookmark {
            id: 0,
            name: String::new(),
            start_hz: 0.0,
            end_hz: None,
            color: String::new(),
            stack: String::new(),
            notes: String::new(),
        }
    }
}

impl Bookmark {
    /// A point.
    pub fn point(name: &str, hz: f64) -> Bookmark {
        Bookmark {
            name: name.into(),
            start_hz: hz,
            ..Bookmark::default()
        }
    }

    /// A range; the edges are put in order.
    pub fn range(name: &str, a_hz: f64, b_hz: f64) -> Bookmark {
        Bookmark {
            name: name.into(),
            start_hz: a_hz.min(b_hz),
            end_hz: Some(a_hz.max(b_hz)),
            ..Bookmark::default()
        }
    }

    fn validate(&self) -> Result<(), MemoryError> {
        if !(self.start_hz.is_finite() && self.start_hz > 0.0) {
            return Err(MemoryError::Invalid(Problem::FrequencyOutOfRange));
        }
        if let Some(e) = self.end_hz {
            if !(e.is_finite() && e >= self.start_hz) {
                return Err(MemoryError::Invalid(Problem::EndBelowStart));
            }
        }
        if !self.color.is_empty() && !crate::memory::is_color(&self.color) {
            return Err(MemoryError::Invalid(Problem::NotAColour));
        }
        Ok(())
    }
}

/// The bookmarks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BookmarkStore {
    /// The bookmarks, in the order added.
    pub bookmarks: Vec<Bookmark>,
    next_id: u64,
}

impl BookmarkStore {
    /// Adds a bookmark and returns its id.
    pub fn add(&mut self, mut b: Bookmark) -> Result<u64, MemoryError> {
        b.validate()?;
        if self.bookmarks.len() >= MAX_BOOKMARKS {
            return Err(MemoryError::Full { cap: MAX_BOOKMARKS });
        }
        self.next_id = self
            .next_id
            .max(self.bookmarks.iter().map(|b| b.id).max().unwrap_or(0) + 1);
        b.id = self.next_id;
        self.next_id += 1;
        self.bookmarks.push(b);
        Ok(self.next_id - 1)
    }

    /// Deletes a bookmark; false when there was none with that id.
    pub fn remove(&mut self, id: u64) -> bool {
        let n = self.bookmarks.len();
        self.bookmarks.retain(|b| b.id != id);
        n != self.bookmarks.len()
    }

    /// The bookmarks as CSV: `name,start_hz,end_hz,color,mode,notes`.
    pub fn export_csv(&self) -> String {
        let mut out = String::from("name,start_hz,end_hz,color,mode,notes\n");
        for b in &self.bookmarks {
            let row = [
                b.name.clone(),
                b.start_hz.to_string(),
                b.end_hz.map(|e| e.to_string()).unwrap_or_default(),
                b.color.clone(),
                b.stack.clone(),
                b.notes.clone(),
            ];
            out.push_str(&csv::record(&row, ','));
            out.push('\n');
        }
        out
    }

    /// Imports CSV in the layout of [`BookmarkStore::export_csv`] (a header row is optional; frequencies may carry units). Points
    /// already there (same start and end) are skipped. Returns how many were added and the rows that could not be read.
    pub fn import_csv(&mut self, text: &str) -> (usize, Vec<LineProblem>) {
        let d = csv::sniff_delimiter(text);
        let mut added = 0;
        let mut errors = Vec::new();
        for (line, row) in csv::read(text, d) {
            let cell = |i: usize| row.get(i).map(|s| s.trim()).unwrap_or("");
            if line == 1 && freq::parse_hz(cell(1), UnitHint::Hz).is_none() {
                continue; // a header
            }
            let Some(start) = freq::parse_hz(cell(1), UnitHint::Hz) else {
                errors.push(LineProblem {
                    line,
                    kind: Problem::NotAFrequency,
                });
                continue;
            };
            let end = if cell(2).is_empty() {
                None
            } else {
                match freq::parse_hz(cell(2), UnitHint::Hz) {
                    Some(e) => Some(e),
                    None => {
                        errors.push(LineProblem {
                            line,
                            kind: Problem::NotAFrequency,
                        });
                        continue;
                    }
                }
            };
            if self
                .bookmarks
                .iter()
                .any(|b| (b.start_hz - start).abs() < 1.0 && b.end_hz == end)
            {
                continue;
            }
            let b = Bookmark {
                name: cell(0).into(),
                start_hz: start,
                end_hz: end,
                color: cell(3).into(),
                stack: cell(4).into(),
                notes: cell(5).into(),
                ..Bookmark::default()
            };
            match self.add(b) {
                Ok(_) => added += 1,
                Err(MemoryError::Invalid(kind)) => errors.push(LineProblem { line, kind }),
                Err(_) => errors.push(LineProblem {
                    line,
                    kind: Problem::Full,
                }),
            }
        }
        (added, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_and_ranges() {
        let mut s = BookmarkStore::default();
        let a = s.add(Bookmark::point("Beacon", 144.43e6)).unwrap();
        let b = s
            .add(Bookmark::range("Satellites", 146e6, 145.8e6))
            .unwrap();
        assert_ne!(a, b);
        assert_eq!(s.bookmarks[1].start_hz, 145.8e6);
        assert!(s.add(Bookmark::point("bad", -1.0)).is_err());
        let mut bad = Bookmark::point("bad colour", 1e6);
        bad.color = "blue".into();
        assert!(s.add(bad).is_err());
        assert!(s.remove(a));
        assert!(!s.remove(a));
    }

    #[test]
    fn csv_round_trip_and_duplicates() {
        let mut s = BookmarkStore::default();
        let mut b = Bookmark::range("Band, edge", 7e6, 7.2e6);
        b.color = "#ff8800".into();
        s.add(b).unwrap();
        s.add(Bookmark::point("Beacon", 14.1e6)).unwrap();
        let text = s.export_csv();
        let mut t = BookmarkStore::default();
        assert_eq!(t.import_csv(&text), (2, vec![]));
        assert_eq!(t.bookmarks[0].name, "Band, edge");
        assert_eq!(t.bookmarks[0].end_hz, Some(7.2e6));
        assert_eq!(t.bookmarks[0].color, "#ff8800");
        assert_eq!(t.import_csv(&text).0, 0);
        let (n, errors) = t.import_csv("X,145.5 MHz\nY,oops\n");
        assert_eq!(n, 1);
        assert_eq!(errors.len(), 1);
    }
}

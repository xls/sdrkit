//! The entries of the frequency database that the data folder supplies (`freqdb/builtin.csv`), and the scope tags of
//! a country for a query of the database.

use crate::data::{read_text, Data};
use crate::error::DataError;
use crate::region::Region;
use sdrkit_host_engine::Engine;
use sdrkit_svc_freqdb::Entry;

/// One supplied entry of the frequency database, as a row for a host.
#[derive(Clone, Debug, PartialEq)]
pub struct FrequencyRow {
    /// The stable id of the entry (`b:<slug>`).
    pub id: String,
    /// The name.
    pub name: String,
    /// The frequency to tune, Hz.
    pub frequency_hz: f64,
    /// The bandwidth, Hz, if the entry states one.
    pub bandwidth_hz: Option<f64>,
    /// The notes of the entry. Empty if it has none.
    pub note: String,
}

/// True if `entry` is for the stack `stack`: its stack column names the stack, or the entry is for the general
/// receiver or for no stack and one of its tags is the stack id (an FT8 dial is a `general` entry tagged `ft8`).
fn for_stack(entry: &Entry, stack: &str) -> bool {
    entry.stack == stack
        || ((entry.stack.is_empty() || entry.stack == "general") && entry.tags.iter().any(|t| t == stack))
}

/// The path of the file of supplied entries in a data folder.
pub const ENTRIES_FILE: &str = "freqdb/builtin.csv";

impl Data {
    /// The entries of [`ENTRIES_FILE`].
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for the first line that is not an entry.
    pub fn frequency_entries(&self) -> Result<Vec<Entry>, DataError> {
        let path = self.folder().join(ENTRIES_FILE);
        sdrkit_svc_freqdb::supplied_entries(&read_text(&path)?).map_err(|p| DataError::Format {
            path,
            reason: format!("line {}: {:?}", p.line, p.kind),
        })
    }

    /// Gives the frequency database of `engine` the entries of [`ENTRIES_FILE`].
    ///
    /// # Errors
    ///
    /// Those of [`Data::frequency_entries`].
    pub fn load_frequency_entries(&self, engine: &Engine) -> Result<usize, DataError> {
        let entries = self.frequency_entries()?;
        let n = entries.len();
        engine.set_frequency_entries(entries);
        Ok(n)
    }

    /// The supplied entries of [`ENTRIES_FILE`] for `stack` in the country of `region`, lowest frequency first: the
    /// entries whose stack column is `stack`, and the entries of the general receiver or of no stack with the tag
    /// `stack`; each applies everywhere or in the country or its ITU region.
    ///
    /// # Errors
    ///
    /// Those of [`Data::frequency_entries`].
    pub fn frequency_rows(&self, stack: &str, region: &Region) -> Result<Vec<FrequencyRow>, DataError> {
        let scope = self.frequency_scope(region.of_stack(stack).country());
        let mut out: Vec<FrequencyRow> = self
            .frequency_entries()?
            .into_iter()
            .filter(|e| for_stack(e, stack) && e.applies(&scope))
            .map(|e| FrequencyRow {
                id: e.id,
                name: e.name,
                frequency_hz: e.hz,
                bandwidth_hz: e.bandwidth_hz.filter(|b| *b > 0.0),
                note: e.notes,
            })
            .collect();
        out.sort_by(|a, b| a.frequency_hz.total_cmp(&b.frequency_hz));
        Ok(out)
    }

    /// The scope tags of the country `code` (ISO 3166 alpha-2, any case) for the `scope` argument of the database: the
    /// code, then `itu1` to `itu3` for its ITU region if the country entries name the country.
    pub fn frequency_scope(&self, code: &str) -> Vec<String> {
        let code = code.trim().to_ascii_uppercase();
        let itu = self.plans.itu(&code).map(|n| format!("itu{n}"));
        std::iter::once(code).chain(itu).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supplied_entry_loads() {
        let d = Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
        let all = d.frequency_entries().unwrap();
        assert_eq!(all.len(), 183);
        let aprs = all.iter().find(|e| e.id == "b:aprs-au").unwrap();
        assert_eq!(aprs.hz, 145.175e6);
        assert!(aprs.applies(&d.frequency_scope("au")));
        assert!(!aprs.applies(&d.frequency_scope("NZ")));
        assert_eq!(d.frequency_scope("de"), ["DE", "itu1"]);
    }
}

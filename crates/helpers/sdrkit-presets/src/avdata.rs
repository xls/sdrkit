//! Aircraft data link channels: the ACARS, VDL Mode 2 and VDL Mode 4 channel sets of `avdata/datalink_channels.csv`.
//!
//! [`settings`] gives the stack `acars`, `vdl2` or `vdl4` the setting `channels` of one set.

use crate::data::{Data, Table};
use crate::error::DataError;
use sdrkit_core::Value;
use std::path::Path;

/// The path of the channel sets in the data folder.
pub const DATALINK_CHANNELS: &str = "avdata/datalink_channels.csv";

/// One channel of a set.
#[derive(Clone, Debug, PartialEq)]
pub struct DatalinkChannel {
    /// The stack id ("acars", "vdl2", "vdl4").
    pub stack: String,
    /// The set ("Europe", "North America", "Worldwide").
    pub set: String,
    /// The frequency, Hz.
    pub frequency_hz: f64,
    /// The note of the row.
    pub note: String,
}

/// The channels of the file `path` (the format of `avdata/datalink_channels.csv`), in the order of the file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_channels(path: &Path) -> Result<Vec<DatalinkChannel>, DataError> {
    let t = Table::read(path.to_owned(), &["stack", "set", "frequency_mhz", "note"])?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(DatalinkChannel {
            stack: t.text(r, "stack").to_owned(),
            set: t.text(r, "set").to_owned(),
            frequency_hz: (t.required(*line, r, "frequency_mhz")? * 1e6).round(),
            note: t.text(r, "note").to_owned(),
        });
    }
    Ok(out)
}

/// The frequencies (Hz) of the set `set` of the stack `stack`, in the order of the file; empty for no such set.
pub fn set_hz(rows: &[DatalinkChannel], stack: &str, set: &str) -> Vec<f64> {
    rows.iter()
        .filter(|c| c.stack == stack && c.set.eq_ignore_ascii_case(set))
        .map(|c| c.frequency_hz)
        .collect()
}

/// The setting `channels` of the stack `stack` for the set `set`: the frequencies in MHz separated by spaces. `None` when
/// the file has no such set.
pub fn settings(rows: &[DatalinkChannel], stack: &str, set: &str) -> Option<Vec<(String, Value)>> {
    let hz = set_hz(rows, stack, set);
    if hz.is_empty() {
        return None;
    }
    let text: Vec<String> = hz.iter().map(|f| format!("{:.6}", f / 1e6)).collect();
    Some(vec![("channels".to_owned(), Value::Text(text.join(" ")))])
}

impl Data {
    /// The channel sets of `avdata/datalink_channels.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_channels`].
    pub fn datalink_channels(&self) -> Result<Vec<DatalinkChannel>, DataError> {
        read_channels(&self.folder().join(DATALINK_CHANNELS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn every_row_of_the_file_loads() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data")
            .join(DATALINK_CHANNELS);
        let rows = read_channels(&path).unwrap();
        assert_eq!(rows.len(), 61);
        assert_eq!(
            set_hz(&rows, "acars", "Europe"),
            vec![131.525e6, 131.725e6, 131.825e6]
        );
        assert_eq!(set_hz(&rows, "acars", "north america").len(), 4);
        assert_eq!(set_hz(&rows, "acars", "single"), vec![131.525e6]);
        assert_eq!(set_hz(&rows, "vdl2", "Europe").len(), 8);
        let eu27 = set_hz(&rows, "vdl2", "Europe 2027");
        assert_eq!((eu27.len(), eu27[0], eu27[11]), (12, 136.700e6, 136.975e6));
        assert_eq!(set_hz(&rows, "vdl2", "North America").len(), 5);
        let ww = set_hz(&rows, "vdl2", "Worldwide");
        assert_eq!((ww.len(), ww[0], ww[21]), (22, 131.250e6, 136.975e6));
        assert_eq!(
            set_hz(&rows, "vdl4", "six channels"),
            vec![136.725e6, 136.775e6, 136.825e6, 136.875e6, 136.900e6, 136.975e6]
        );
        assert_eq!(
            settings(&rows, "acars", "Europe"),
            Some(vec![(
                "channels".to_owned(),
                Value::Text("131.525000 131.725000 131.825000".into())
            )])
        );
        assert_eq!(settings(&rows, "acars", "Mars"), None);
    }
}

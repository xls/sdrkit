//! Weak-signal data files: the dials of `wsjt/dials.csv` and the band edges of `wsjt/bands.csv`.
//!
//! [`settings`] gives the weak-signal stacks the settings `dials` and `band_edges`. [`dial_of`] gives the dial of a
//! stack on a named band, the frequency a host tunes. [`Data::dials`] gives a host the dials of a stack as rows.

use crate::data::{Data, Table};
use crate::error::DataError;
use crate::preset::PlanChannel;
use crate::region::Region;
use sdrkit_core::Value;
use std::fmt::Write as _;
use std::path::Path;

/// The path of the dials in the data folder.
pub const DIALS: &str = "wsjt/dials.csv";
/// The path of the band edges in the data folder.
pub const BANDS: &str = "wsjt/bands.csv";

/// One dial: the frequency of one stack on one band.
#[derive(Clone, Debug, PartialEq)]
pub struct WsjtDial {
    /// The stack id of the mode ("ft8").
    pub mode: String,
    /// The band ("20m").
    pub band: String,
    /// The upper-sideband dial, Hz.
    pub dial_hz: f64,
}

impl WsjtDial {
    /// The tooltip of a band button: "FT8 on 20m: dial 14.0740 MHz (upper sideband)".
    pub fn tooltip(&self) -> String {
        format!(
            "{} on {}: dial {:.4} MHz (upper sideband)",
            self.mode.to_ascii_uppercase(),
            self.band,
            self.dial_hz / 1e6
        )
    }
}

/// One band of the multi-band monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct WsjtBand {
    /// The name ("20m").
    pub name: String,
    /// The lower edge, Hz.
    pub low_hz: f64,
    /// The upper edge, Hz.
    pub high_hz: f64,
}

/// The dials of the file `path` (the format of `wsjt/dials.csv`), in the order of the file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_dials(path: &Path) -> Result<Vec<WsjtDial>, DataError> {
    let t = Table::read(path.to_owned(), &["mode", "band", "dial_mhz"])?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(WsjtDial {
            mode: t.text(r, "mode").to_ascii_lowercase(),
            band: t.text(r, "band").to_owned(),
            dial_hz: (t.required(*line, r, "dial_mhz")? * 1e6).round(),
        });
    }
    Ok(out)
}

/// The bands of the file `path` (the format of `wsjt/bands.csv`), in the order of the file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_bands(path: &Path) -> Result<Vec<WsjtBand>, DataError> {
    let t = Table::read(path.to_owned(), &["band", "low_mhz", "high_mhz"])?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(WsjtBand {
            name: t.text(r, "band").to_owned(),
            low_hz: (t.required(*line, r, "low_mhz")? * 1e6).round(),
            high_hz: (t.required(*line, r, "high_mhz")? * 1e6).round(),
        });
    }
    Ok(out)
}

/// The dial of the stack `mode` ("ft8") on the band `band` ("20m").
pub fn dial_of(dials: &[WsjtDial], mode: &str, band: &str) -> Option<f64> {
    dials
        .iter()
        .find(|d| d.mode.eq_ignore_ascii_case(mode) && d.band == band)
        .map(|d| d.dial_hz)
}

/// The text of the stack setting `dials`: `mode=frequency_hz` entries separated by `;`.
pub fn dials_text(dials: &[WsjtDial]) -> String {
    let mut out = String::new();
    for d in dials {
        if !out.is_empty() {
            out.push(';');
        }
        let _ = write!(out, "{}={}", d.mode, d.dial_hz);
    }
    out
}

/// The text of the stack setting `band_edges`: `name=low_hz-high_hz` entries separated by `;`, in the order of `bands`.
pub fn bands_text(bands: &[WsjtBand]) -> String {
    let mut out = String::new();
    for b in bands {
        if !out.is_empty() {
            out.push(';');
        }
        let _ = write!(out, "{}={}-{}", b.name, b.low_hz, b.high_hz);
    }
    out
}

/// The settings `dials` and `band_edges` of a weak-signal stack. `wsjt`, `ft8`, `ft4`, `ft2` and `wspr` take both; the
/// other stacks of `wsjt/dials.csv` take `band_edges` only, and [`crate::Data::preset`] gives each stack its own.
pub fn settings(dials: &[WsjtDial], bands: &[WsjtBand]) -> Vec<(String, Value)> {
    vec![
        ("dials".to_owned(), Value::Text(dials_text(dials))),
        ("band_edges".to_owned(), Value::Text(bands_text(bands))),
    ]
}

impl Data {
    /// The dials of `wsjt/dials.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_dials`].
    pub fn wsjt_dials(&self) -> Result<Vec<WsjtDial>, DataError> {
        read_dials(&self.folder().join(DIALS))
    }

    /// The bands of `wsjt/bands.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_bands`].
    pub fn wsjt_bands(&self) -> Result<Vec<WsjtBand>, DataError> {
        read_bands(&self.folder().join(BANDS))
    }

    /// The dials that a host offers for `stack` in `region`, lowest first: the rows of `wsjt/dials.csv` of the stack
    /// `submode` or of `stack`, each labelled by its band. If neither has a row and the data gives the stack the
    /// setting `dials`, every row, labelled by its stack and band. Empty for a folder without the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], [`DataError::Format`] for a file that cannot be used.
    pub fn dials(
        &self,
        stack: &str,
        region: &Region,
        submode: Option<&str>,
    ) -> Result<Vec<PlanChannel>, DataError> {
        let rows = match self.wsjt_dials() {
            Ok(rows) => rows,
            Err(DataError::Missing { .. }) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let named = |d: &&WsjtDial| d.mode == stack || submode.is_some_and(|m| d.mode == m);
        let row = |label: String, d: &WsjtDial| PlanChannel {
            label,
            frequency_hz: d.dial_hz,
            width_hz: None,
        };
        let mut out: Vec<PlanChannel> = rows
            .iter()
            .filter(named)
            .map(|d| row(d.band.clone(), d))
            .collect();
        if out.is_empty() {
            let code = region.of_stack(stack).country();
            let family = self.family_settings(stack, code)?;
            if family.settings.iter().any(|(k, _)| k == "dials") {
                out = rows
                    .iter()
                    .map(|d| row(format!("{} {}", d.mode.to_ascii_uppercase(), d.band), d))
                    .collect();
            }
        }
        out.sort_by(|a, b| a.frequency_hz.total_cmp(&b.frequency_hz));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data")
            .join(rel)
    }

    #[test]
    fn every_row_of_the_files_loads() {
        let dials = read_dials(&file(DIALS)).unwrap();
        assert_eq!(dials.len(), 92);
        let count = |m: &str| dials.iter().filter(|d| d.mode == m).count();
        assert_eq!(
            (count("ft8"), count("ft4"), count("ft2"), count("wspr")),
            (14, 10, 1, 12)
        );
        assert_eq!((count("fst4"), count("fst4w"), count("js8")), (8, 8, 7));
        assert!(dials.iter().all(|d| d.dial_hz > 1e6 && !d.band.is_empty()));
        let bands = read_bands(&file(BANDS)).unwrap();
        assert_eq!(bands.len(), 11);
        assert!(bands.iter().all(|b| b.low_hz < b.high_hz));
        assert_eq!(dial_of(&dials, "ft8", "20m"), Some(14.074e6));
        assert_eq!(dial_of(&dials, "FT4", "40m"), Some(7.0475e6));
        assert_eq!(dial_of(&dials, "ft2", "40m"), None);
        let ft8 = dials
            .iter()
            .find(|d| d.mode == "ft8" && d.band == "20m")
            .unwrap();
        assert_eq!(
            ft8.tooltip(),
            "FT8 on 20m: dial 14.0740 MHz (upper sideband)"
        );
        // Every monitor dial of FT8, FT4, FT2 and WSPR lies in the band of its name.
        for d in dials
            .iter()
            .filter(|d| ["ft8", "ft4", "ft2", "wspr"].contains(&d.mode.as_str()))
        {
            if let Some(b) = bands.iter().find(|b| b.name == d.band) {
                assert!(d.dial_hz >= b.low_hz && d.dial_hz <= b.high_hz, "{d:?}");
            }
        }
        let s = settings(&dials, &bands);
        let Value::Text(t) = &s[0].1 else { panic!() };
        assert_eq!(t.split(';').count(), 92);
        assert!(t.starts_with("ft8=1840000;ft8=3573000;"));
        let Value::Text(t) = &s[1].1 else { panic!() };
        assert!(t.starts_with("160m=1810000-2000000;80m=3500000-3800000;"));
    }
}

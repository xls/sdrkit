//! The bands of the band scan of the broadcast stacks (`broadcast/scan_bands.csv`) and the C-QUAM presets
//! (`broadcast/cquam_presets.csv`).
//!
//! [`Data::broadcast_bands`] gives the bands of a stack for a medium wave raster, with the edges, the first channel and the
//! raster from the plan files. A host makes the band of the stack from each of them.

use crate::data::{Data, Table};
use crate::error::DataError;
use std::path::Path;

/// The path of the scan bands in the data folder.
pub const SCAN_BANDS: &str = "broadcast/scan_bands.csv";
/// The path of the C-QUAM presets in the data folder.
pub const CQUAM_PRESETS: &str = "broadcast/cquam_presets.csv";

/// One row of the scan bands: a band of a stack and the band of the plan files that it is.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanBandRow {
    /// The stack id ("drm").
    pub stack: String,
    /// The id of the band in the stack ("sw31").
    pub id: String,
    /// The name on the screen ("SW 31 m").
    pub name: String,
    /// The id of the band group of the plan files ("bc_r1").
    pub group: String,
    /// The name of the band in the group ("31 m").
    pub band: String,
    /// The medium wave raster that the row is for, Hz; `None` for every raster.
    pub mw_raster_hz: Option<f64>,
    /// Where the shortcut of the band tunes, Hz.
    pub shortcut_hz: f64,
}

/// A band of a stack as plain values.
#[derive(Clone, Debug, PartialEq)]
pub struct BroadcastBand {
    /// The stack id.
    pub stack: String,
    /// The id of the band in the stack.
    pub id: String,
    /// The name on the screen.
    pub name: String,
    /// The lowest frequency, Hz.
    pub low_hz: f64,
    /// The highest frequency, Hz.
    pub high_hz: f64,
    /// The first channel, Hz.
    pub first_hz: f64,
    /// The channel raster, Hz.
    pub raster_hz: f64,
    /// Where the shortcut of the band tunes, Hz.
    pub shortcut_hz: f64,
}

/// The rows of the file `path` (the format of `broadcast/scan_bands.csv`), in the order of the file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_scan_bands(path: &Path) -> Result<Vec<ScanBandRow>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &[
            "stack",
            "id",
            "name",
            "group",
            "band",
            "mw_raster_khz",
            "shortcut_khz",
        ],
    )?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(ScanBandRow {
            stack: t.text(r, "stack").to_owned(),
            id: t.text(r, "id").to_owned(),
            name: t.text(r, "name").to_owned(),
            group: t.text(r, "group").to_owned(),
            band: t.text(r, "band").to_owned(),
            mw_raster_hz: t
                .number(*line, r, "mw_raster_khz")?
                .map(|k| (k * 1e3).round()),
            shortcut_hz: (t.required(*line, r, "shortcut_khz")? * 1e3).round(),
        });
    }
    Ok(out)
}

/// The presets of the file `path` (the format of `broadcast/cquam_presets.csv`), Hz, in the order of the file.
///
/// # Errors
///
/// As [`read_scan_bands`].
pub fn read_presets(path: &Path) -> Result<Vec<f64>, DataError> {
    let t = Table::read(path.to_owned(), &["freq_khz"])?;
    t.rows
        .iter()
        .map(|(line, r)| Ok((t.required(*line, r, "freq_khz")? * 1e3).round()))
        .collect()
}

impl Data {
    /// The rows of `broadcast/scan_bands.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_scan_bands`].
    pub fn scan_band_rows(&self) -> Result<Vec<ScanBandRow>, DataError> {
        read_scan_bands(&self.folder().join(SCAN_BANDS))
    }

    /// The bands of the stack `stack` for a host whose medium wave raster is `mw_raster_hz` (9 or 10 kHz), in the order of
    /// the file. A row whose band the plan files lack, or whose band has no raster, is left out.
    ///
    /// # Errors
    ///
    /// As [`read_scan_bands`].
    pub fn broadcast_bands(
        &self,
        stack: &str,
        mw_raster_hz: f64,
    ) -> Result<Vec<BroadcastBand>, DataError> {
        Ok(self
            .scan_band_rows()?
            .into_iter()
            .filter(|r| {
                r.stack == stack
                    && r.mw_raster_hz
                        .is_none_or(|m| (m - mw_raster_hz).abs() < 1.0)
            })
            .filter_map(|r| {
                let b = self
                    .plans
                    .group(&r.group)?
                    .bands
                    .iter()
                    .find(|b| b.name == r.band)?;
                let raster = b.raster.as_ref()?;
                Some(BroadcastBand {
                    stack: r.stack,
                    id: r.id,
                    name: r.name,
                    low_hz: b.start_hz,
                    high_hz: b.end_hz,
                    first_hz: raster.first_hz,
                    raster_hz: raster.step_hz,
                    shortcut_hz: r.shortcut_hz,
                })
            })
            .collect())
    }

    /// The C-QUAM presets of `broadcast/cquam_presets.csv`, Hz.
    ///
    /// # Errors
    ///
    /// As [`read_presets`].
    pub fn cquam_presets(&self) -> Result<Vec<f64>, DataError> {
        read_presets(&self.folder().join(CQUAM_PRESETS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Data {
        Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
    }

    #[test]
    fn every_row_of_the_files_loads() {
        let d = data();
        let rows = d.scan_band_rows().unwrap();
        assert_eq!(rows.len(), 19);
        // Every row names a band of the plan files with a raster.
        for raster in [9e3, 10e3] {
            let mut n = 0;
            for stack in ["drm", "hdradio", "cdr", "cquam"] {
                for b in d.broadcast_bands(stack, raster).unwrap() {
                    assert!(
                        b.low_hz < b.high_hz && b.first_hz >= b.low_hz && b.raster_hz > 0.0,
                        "{b:?}"
                    );
                    n += 1;
                }
            }
            assert_eq!(n, 16);
        }
        let mw9 = &d.broadcast_bands("cquam", 9e3).unwrap()[0];
        assert_eq!(
            (mw9.low_hz, mw9.first_hz, mw9.raster_hz, mw9.shortcut_hz),
            (526.5e3, 531e3, 9e3, 999e3)
        );
        let mw10 = &d.broadcast_bands("cquam", 10e3).unwrap()[0];
        assert_eq!(
            (
                mw10.high_hz,
                mw10.first_hz,
                mw10.raster_hz,
                mw10.shortcut_hz
            ),
            (1705e3, 540e3, 10e3, 1000e3)
        );
        let drm = d.broadcast_bands("drm", 9e3).unwrap();
        assert_eq!(drm.len(), 12);
        assert_eq!(
            (drm[10].id.as_str(), drm[10].first_hz, drm[10].shortcut_hz),
            ("vhf2", 87.5e6, 87.6e6)
        );
        assert!(d.broadcast_bands("hamdrm", 9e3).unwrap().is_empty());
        assert_eq!(
            d.cquam_presets().unwrap(),
            vec![531e3, 810e3, 999e3, 1170e3, 1368e3, 1570e3, 1602e3]
        );
    }
}

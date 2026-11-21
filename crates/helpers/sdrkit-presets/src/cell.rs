//! The band lists of the cell stacks (`cell`, `nr`, `nbiot`, `gsmr`).
//!
//! `cell/band_order.csv` has the columns `stack,list,name,values`, one row for each list. `list` is `band_order` (the
//! value of the stack setting `band_order`), `scan_bands` (the bands a band scan covers), `scan_offer` (the bands a scan
//! offers next) or `scan_mhz` (the low and the high edge of a scan range, MHz). `values` holds numbers apart by spaces.

use crate::data::Table;
use crate::error::DataError;
use sdrkit_core::Value;
use std::path::Path;

/// The path of the cell band file in a data folder.
pub const CELL_BANDS_FILE: &str = "cell/band_order.csv";

/// One row of the cell band file.
#[derive(Clone, Debug, PartialEq)]
pub struct CellList {
    /// The stack id.
    pub stack: String,
    /// The kind of list: `band_order`, `scan_bands`, `scan_offer` or `scan_mhz`.
    pub list: String,
    /// The name of the row ("europe").
    pub name: String,
    /// The values: band numbers, or two edges in MHz.
    pub values: Vec<f64>,
}

impl CellList {
    /// The values as band numbers. A value that is not a band number is left out.
    pub fn bands(&self) -> Vec<u16> {
        self.values
            .iter()
            .filter(|v| v.fract() == 0.0 && (0.0..=f64::from(u16::MAX)).contains(*v))
            .map(|&v| v as u16)
            .collect()
    }

    /// The range of a `scan_mhz` row, Hz, or none for a row without two values.
    pub fn range_hz(&self) -> Option<(f64, f64)> {
        match self.values[..] {
            [lo, hi] => Some((lo * 1e6, hi * 1e6)),
            _ => None,
        }
    }
}

/// Reads a cell band file.
///
/// # Errors
///
/// [`DataError::Missing`] and [`DataError::Read`] for the file, [`DataError::Format`] for a missing column, an empty
/// `stack`, `list` or `name`, or a value that is not a number.
pub fn read_cell_lists(path: &Path) -> Result<Vec<CellList>, DataError> {
    let t = Table::read(path.to_owned(), &["stack", "list", "name", "values"])?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        let text = |name: &str| -> Result<String, DataError> {
            let v = t.text(r, name);
            if v.is_empty() {
                return Err(DataError::Format {
                    path: t.path.clone(),
                    reason: format!("line {line}: the column `{name}` is empty"),
                });
            }
            Ok(v.to_owned())
        };
        let values = t
            .text(r, "values")
            .split_whitespace()
            .map(|v| {
                v.parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| DataError::Format {
                        path: t.path.clone(),
                        reason: format!("line {line}: `{v}` in the column `values` is not a number"),
                    })
            })
            .collect::<Result<Vec<f64>, DataError>>()?;
        out.push(CellList {
            stack: text("stack")?,
            list: text("list")?,
            name: text("name")?,
            values,
        });
    }
    Ok(out)
}

/// The settings of `stack` for the rows named `name`: `band_order` from its `band_order` row. None when the file has no
/// such row.
pub fn cell_settings(rows: &[CellList], stack: &str, name: &str) -> Option<Vec<(String, Value)>> {
    cell_list(rows, stack, "band_order", name).map(|r| {
        let text = r.bands().iter().map(u16::to_string).collect::<Vec<_>>().join(" ");
        vec![("band_order".to_owned(), Value::Text(text))]
    })
}

/// The row of `stack` with this `list` and `name`.
pub fn cell_list<'a>(rows: &'a [CellList], stack: &str, list: &str, name: &str) -> Option<&'a CellList> {
    rows.iter()
        .find(|r| r.stack == stack && r.list == list && r.name == name)
}

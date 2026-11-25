//! Starting dials of the stacks: the files `<family>/default_dials.csv` (columns `stack`, `label`, `hz`).
//!
//! A stack's descriptor holds no starting frequency. [`dial_hz`] gives a host the dial of a stack or of one of its
//! submodes, to set as the tuned frequency.

use crate::data::{Data, Table};
use crate::error::DataError;
use std::path::Path;

/// The file name of the starting dials in a family folder.
pub const DEFAULT_DIALS: &str = "default_dials.csv";

/// The families whose folder holds a file [`DEFAULT_DIALS`].
pub const FAMILIES: &[&str] = &[
    "avdata",
    "broadcast",
    "drones",
    "dvbt2",
    "general",
    "hfkbd",
    "ism",
    "science",
    "wsjt",
];

/// One starting dial.
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultDial {
    /// The stack id.
    pub stack: String,
    /// The submode id; empty for the stack itself.
    pub label: String,
    /// The dial, Hz.
    pub hz: f64,
}

/// The dials of the file `path` (the format of [`DEFAULT_DIALS`]), in the order of the file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row without a number in `hz`.
pub fn read_dials(path: &Path) -> Result<Vec<DefaultDial>, DataError> {
    let t = Table::read(path.to_owned(), &["stack", "label", "hz"])?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(DefaultDial {
            stack: t.text(r, "stack").to_owned(),
            label: t.text(r, "label").to_owned(),
            hz: t.required(*line, r, "hz")?,
        });
    }
    Ok(out)
}

/// The dial of `stack`, or of its submode `submode`, Hz. A submode without a row of its own takes the row of the stack.
pub fn dial_hz(rows: &[DefaultDial], stack: &str, submode: Option<&str>) -> Option<f64> {
    let find = |label: &str| {
        rows.iter()
            .find(|d| d.stack == stack && d.label == label)
            .map(|d| d.hz)
    };
    submode.and_then(find).or_else(|| find(""))
}

impl Data {
    /// The starting dials of every family in [`FAMILIES`] whose folder holds the file.
    ///
    /// # Errors
    ///
    /// As [`read_dials`], except that a family without the file is left out.
    pub fn default_dials(&self) -> Result<Vec<DefaultDial>, DataError> {
        let mut out = Vec::new();
        for f in FAMILIES {
            let path = self.folder().join(f).join(DEFAULT_DIALS);
            if path.exists() {
                out.extend(read_dials(&path)?);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn family(f: &str) -> Vec<DefaultDial> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data")
            .join(f)
            .join(DEFAULT_DIALS);
        read_dials(&path).unwrap_or_else(|e| panic!("{f}: {e:?}"))
    }

    #[test]
    fn every_row_of_every_file_loads() {
        let counts: Vec<(&str, usize)> = FAMILIES.iter().map(|f| (*f, family(f).len())).collect();
        assert_eq!(
            counts,
            [
                ("avdata", 3),
                ("broadcast", 1),
                ("drones", 1),
                ("dvbt2", 1),
                ("general", 4),
                ("hfkbd", 22),
                ("ism", 2),
                ("science", 2),
                ("wsjt", 29),
            ]
        );
        for f in FAMILIES {
            for d in family(f) {
                assert!(d.hz > 0.0 && !d.stack.is_empty(), "{f}: {d:?}");
            }
        }
    }

    #[test]
    fn the_dial_of_a_stack_and_of_a_submode() {
        let g = family("general");
        assert_eq!(dial_hz(&g, "general", None), Some(145.5e6));
        assert_eq!(dial_hz(&g, "general", Some("am")), Some(999e3));
        assert_eq!(dial_hz(&g, "general", Some("fm")), Some(145.5e6));
        assert_eq!(dial_hz(&g, "dab", None), None);
        let w = family("wsjt");
        assert_eq!(dial_hz(&w, "jt65", Some("jt65c")), Some(432.065e6));
        assert_eq!(dial_hz(&w, "q65", Some("q65-120e")), Some(10368.2e6));
        let d = Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
        let all = d.default_dials().unwrap();
        assert_eq!(dial_hz(&all, "dvbt2", None), Some(474e6));
        assert_eq!(dial_hz(&all, "hamdrm", None), Some(14.233e6));
        assert_eq!(dial_hz(&all, "zigbee", None), Some(2405e6));
        assert_eq!(dial_hz(&all, "dsrc_toll", None), Some(5802.5e6));
    }

    #[test]
    fn the_ism_stacks_start_from_a_dial_or_a_plan() {
        let d = Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
        let start = |stack: &str, code: &str| {
            let region = crate::Region::new(code).unwrap();
            let p = d.preset(stack, &region, &crate::Options::default()).unwrap();
            (p.plan, p.frequency_hz)
        };
        assert_eq!(start("zigbee", "AU"), (String::new(), 2405e6));
        assert_eq!(start("dsrc_toll", "AU"), (String::new(), 5802.5e6));
        assert_eq!(start("dsrc_toll", "DE"), ("dsrc_toll_europe".into(), 5797.5e6));
        assert_eq!(start("ism", "AU"), ("ism_r3".into(), 433.92e6));
        assert_eq!(start("dect", "AU"), ("dect_1880".into(), 1897.344e6));
        assert_eq!(start("dect", "US"), ("dect_us".into(), 1921.536e6));
        assert_eq!(start("subghzchat", "AU"), ("subghzchat_au".into(), 433.92e6));
        assert_eq!(start("subghzchat", "US"), ("subghzchat_r2".into(), 915e6));
        assert_eq!(start("wmbus", "DE"), ("wmbus_868".into(), 868.95e6));
    }
}

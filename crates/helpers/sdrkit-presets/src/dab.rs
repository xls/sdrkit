//! The channels of the `dab` stack from the DAB groups of the plan files.
//!
//! `plans/dab.json` holds one group for each set of blocks: Band III (`dab_band3`), L-band (`dab_band3_lband`), and the
//! T-DMB blocks of Korea (`dab_tdmb_kr`). The stack setting `channels` takes the centre frequencies of the groups that a
//! host chooses for service following; the stack's own ensemble scan takes the same list.

use crate::data::{read_text, Data};
use crate::error::DataError;
use sdrkit_core::Value;
use sdrkit_data_plans::{band_channels, data};
use std::path::Path;

/// The path of the DAB plan file in a data folder.
pub const DAB_PLANS_FILE: &str = "plans/dab.json";

/// A DAB channel of a plan group.
#[derive(Clone, Debug, PartialEq)]
pub struct DabChannel {
    /// The name of the channel ("12B").
    pub name: String,
    /// The centre frequency, Hz.
    pub centre_hz: f64,
}

/// A group of a DAB plan file: its id, name and channels in the order of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct DabGroup {
    /// The group id ("dab_band3").
    pub id: String,
    /// The group name.
    pub name: String,
    /// The channels of every band of the group that is in use.
    pub channels: Vec<DabChannel>,
}

/// Reads the groups of a DAB plan file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a file that is not a plan file.
pub fn read_dab_groups(path: &Path) -> Result<Vec<DabGroup>, DataError> {
    let text = read_text(path)?;
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let file = data::parse(&name, &text).map_err(|e| DataError::Format {
        path: path.to_owned(),
        reason: e.to_string(),
    })?;
    Ok(file
        .groups
        .iter()
        .map(|g| DabGroup {
            id: g.id.clone(),
            name: g.name.clone(),
            channels: g
                .bands
                .iter()
                .filter(|b| b.in_use)
                .flat_map(band_channels)
                .filter(|c| !c.reallocated)
                .map(|c| DabChannel {
                    name: c.name,
                    centre_hz: c.centre_hz,
                })
                .collect(),
        })
        .collect())
}

/// The `dab` settings for the channels of the groups `ids`, in the order given: `channels`. `None` when a group is
/// missing.
pub fn dab_settings(groups: &[DabGroup], ids: &[&str]) -> Option<Vec<(String, Value)>> {
    let mut hz = Vec::new();
    for id in ids {
        let g = groups.iter().find(|g| g.id == *id)?;
        hz.extend(g.channels.iter().map(|c| format!("{:.0}", c.centre_hz)));
    }
    Some(vec![("channels".to_owned(), Value::Text(hz.join(";")))])
}

impl Data {
    /// The groups of [`DAB_PLANS_FILE`] of the folder.
    ///
    /// # Errors
    ///
    /// What [`read_dab_groups`] returns.
    pub fn dab_groups(&self) -> Result<Vec<DabGroup>, DataError> {
        read_dab_groups(&self.folder().join(DAB_PLANS_FILE))
    }
}

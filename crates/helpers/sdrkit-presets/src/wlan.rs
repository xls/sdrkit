//! The channel lists of the `wifi` and `halow` stacks by regulatory domain and region.
//!
//! `wlan/wifi_domains.csv` has the columns `domain,name,band,channels,dfs,not_hopped`, one row for each domain and band.
//! `wlan/halow_regions.csv` has the columns `code,name,start_mhz,bands_mhz,widths_mhz,classes,channels_1,channels_2,
//! channels_4,channels_8,channels_16`, one row for each region. A list field holds numbers apart by spaces.

use crate::data::{Data, Table};
use crate::error::DataError;
use sdrkit_core::Value;
use std::path::Path;

/// The path of the Wi-Fi domain file in a data folder.
pub const WIFI_DOMAINS_FILE: &str = "wlan/wifi_domains.csv";
/// The path of the HaLow region file in a data folder.
pub const HALOW_REGIONS_FILE: &str = "wlan/halow_regions.csv";

/// One row of the Wi-Fi domain file: the channels of one band in one regulatory domain.
#[derive(Clone, Debug, PartialEq)]
pub struct WifiDomain {
    /// The domain key ("eu").
    pub domain: String,
    /// The domain name.
    pub name: String,
    /// The band: "2.4", "5" or "6".
    pub band: String,
    /// The channels that the domain permits, ascending.
    pub channels: Vec<u32>,
    /// The channels of `channels` that need radar detection (DFS).
    pub dfs: Vec<u32>,
    /// The channels of `channels` that a hop list leaves out.
    pub not_hopped: Vec<u32>,
}

impl WifiDomain {
    /// The hop list of the row: its channels without `not_hopped`, and without `dfs` when `skip_dfs` is set.
    pub fn hop_list(&self, skip_dfs: bool) -> Vec<u32> {
        self.channels
            .iter()
            .copied()
            .filter(|c| !self.not_hopped.contains(c) && !(skip_dfs && self.dfs.contains(c)))
            .collect()
    }

    /// The key of the `wifi` setting that holds the hop list of the band of the row.
    pub fn setting_key(&self) -> Option<&'static str> {
        match self.band.as_str() {
            "2.4" => Some("channels_24"),
            "5" => Some("channels_5"),
            "6" => Some("channels_6"),
            _ => None,
        }
    }
}

/// One row of the HaLow region file: the S1G channel plan of one region.
#[derive(Clone, Debug, PartialEq)]
pub struct HalowRegion {
    /// The region code ("EU").
    pub code: String,
    /// The region name.
    pub name: String,
    /// The channel starting frequency, Hz.
    pub start_hz: f64,
    /// The bands (low, high), Hz.
    pub bands_hz: Vec<(f64, f64)>,
    /// The channel widths that the region permits, MHz.
    pub widths_mhz: Vec<u32>,
    /// The operating class of each width of `widths_mhz`.
    pub classes: Vec<u32>,
    /// The channel numbers of the widths 1, 2, 4, 8 and 16 MHz.
    pub channels: [Vec<u32>; 5],
}

/// The widths of the channel columns of the HaLow region file, MHz.
pub const HALOW_WIDTHS_MHZ: [u32; 5] = [1, 2, 4, 8, 16];

impl HalowRegion {
    /// The centre of channel `number`, Hz.
    pub fn centre_hz(&self, number: u32) -> f64 {
        self.start_hz + 0.5e6 * f64::from(number)
    }

    /// The `halow` settings of the plan: `start_hz` and `channels_1` to `channels_16`.
    pub fn settings(&self) -> Vec<(String, Value)> {
        let mut out = vec![("start_hz".to_owned(), Value::Hz(self.start_hz))];
        for (w, list) in HALOW_WIDTHS_MHZ.iter().zip(&self.channels) {
            out.push((format!("channels_{w}"), Value::Text(join(list))));
        }
        out
    }
}

/// Numbers apart by spaces.
fn join(list: &[u32]) -> String {
    list.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The numbers of the list field `name` of `row`. An empty field is an empty list.
fn numbers(t: &Table, line: usize, row: &[String], name: &str) -> Result<Vec<u32>, DataError> {
    t.text(row, name)
        .split_whitespace()
        .map(|n| {
            n.parse::<u32>().map_err(|_| DataError::Format {
                path: t.path.clone(),
                reason: format!("line {line}: `{n}` in the column `{name}` is not a whole number"),
            })
        })
        .collect()
}

/// The text field `name` of `row`, which must not be empty.
fn required_text(t: &Table, line: usize, row: &[String], name: &str) -> Result<String, DataError> {
    let v = t.text(row, name);
    if v.is_empty() {
        return Err(DataError::Format {
            path: t.path.clone(),
            reason: format!("line {line}: the column `{name}` is empty"),
        });
    }
    Ok(v.to_owned())
}

/// Reads a Wi-Fi domain file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a missing column, an empty key or a list
/// entry that is not a whole number.
pub fn read_wifi_domains(path: &Path) -> Result<Vec<WifiDomain>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &["domain", "name", "band", "channels", "dfs", "not_hopped"],
    )?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(WifiDomain {
            domain: required_text(&t, *line, r, "domain")?,
            name: t.text(r, "name").to_owned(),
            band: required_text(&t, *line, r, "band")?,
            channels: numbers(&t, *line, r, "channels")?,
            dfs: numbers(&t, *line, r, "dfs")?,
            not_hopped: numbers(&t, *line, r, "not_hopped")?,
        });
    }
    Ok(out)
}

/// The `wifi` settings of a domain: the hop list of each band that the rows give for it, without the DFS channels when
/// `skip_dfs` is set. `None` when no row names the domain.
pub fn wifi_settings(
    rows: &[WifiDomain],
    domain: &str,
    skip_dfs: bool,
) -> Option<Vec<(String, Value)>> {
    let out: Vec<(String, Value)> = rows
        .iter()
        .filter(|r| r.domain.eq_ignore_ascii_case(domain))
        .filter_map(|r| {
            r.setting_key()
                .map(|k| (k.to_owned(), Value::Text(join(&r.hop_list(skip_dfs)))))
        })
        .collect();
    (!out.is_empty()).then_some(out)
}

/// Reads a HaLow region file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a missing column, an empty code or start,
/// a band that is not `low-high` in MHz, or a list entry that is not a whole number.
pub fn read_halow_regions(path: &Path) -> Result<Vec<HalowRegion>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &[
            "code",
            "name",
            "start_mhz",
            "bands_mhz",
            "widths_mhz",
            "classes",
            "channels_1",
            "channels_2",
            "channels_4",
            "channels_8",
            "channels_16",
        ],
    )?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        let mut bands_hz = Vec::new();
        for b in t.text(r, "bands_mhz").split_whitespace() {
            let band = b
                .split_once('-')
                .and_then(|(lo, hi)| Some((lo.parse::<f64>().ok()?, hi.parse::<f64>().ok()?)))
                .ok_or_else(|| DataError::Format {
                    path: t.path.clone(),
                    reason: format!("line {line}: `{b}` in the column `bands_mhz` is not low-high"),
                })?;
            bands_hz.push((band.0 * 1e6, band.1 * 1e6));
        }
        out.push(HalowRegion {
            code: required_text(&t, *line, r, "code")?,
            name: t.text(r, "name").to_owned(),
            start_hz: t.required(*line, r, "start_mhz")? * 1e6,
            bands_hz,
            widths_mhz: numbers(&t, *line, r, "widths_mhz")?,
            classes: numbers(&t, *line, r, "classes")?,
            channels: [
                numbers(&t, *line, r, "channels_1")?,
                numbers(&t, *line, r, "channels_2")?,
                numbers(&t, *line, r, "channels_4")?,
                numbers(&t, *line, r, "channels_8")?,
                numbers(&t, *line, r, "channels_16")?,
            ],
        });
    }
    Ok(out)
}

/// The `halow` settings of the region with `code`. `None` when no row has it.
pub fn halow_settings(rows: &[HalowRegion], code: &str) -> Option<Vec<(String, Value)>> {
    rows.iter()
        .find(|r| r.code.eq_ignore_ascii_case(code))
        .map(HalowRegion::settings)
}

impl Data {
    /// The rows of [`WIFI_DOMAINS_FILE`] of the folder.
    ///
    /// # Errors
    ///
    /// What [`read_wifi_domains`] returns.
    pub fn wifi_domains(&self) -> Result<Vec<WifiDomain>, DataError> {
        read_wifi_domains(&self.folder().join(WIFI_DOMAINS_FILE))
    }

    /// The rows of [`HALOW_REGIONS_FILE`] of the folder.
    ///
    /// # Errors
    ///
    /// What [`read_halow_regions`] returns.
    pub fn halow_regions(&self) -> Result<Vec<HalowRegion>, DataError> {
        read_halow_regions(&self.folder().join(HALOW_REGIONS_FILE))
    }
}

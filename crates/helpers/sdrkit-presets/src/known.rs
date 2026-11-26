//! The known frequencies that the family files give a stack: the channels of its regional row (`halow`, `zwave`,
//! `wisun`, and the marine VHF plan of the country for `marinevhf` and `atis`), the rows of the dial tables, and the
//! bands that name it in the plan files of the family folders.
//!
//! A dial table is a CSV file in a family folder (`<family>/*.csv`, not `plans` or `freqdb`) with a frequency column
//! (`hz`, `dial_hz` or `frequency_hz` in Hz, `freq_khz` or `carrier_khz` in kHz, `frequency_mhz`, `freq_mhz` or
//! `centre_mhz` in MHz) and either a column `stack` or a line `# stacks:` that names the stacks of every row. Each row
//! is a known frequency of its stack; a frequency field may hold several numbers apart by `|` or spaces. Its label is the first field that is not empty of `label`, `entry`, `submode`,
//! `set`, `name`, `carrier`, `station` and `band`, else the frequency; its width is the field `width_hz` if the table
//! has one.

use crate::data::{read_text, Data, Table};
use crate::error::DataError;
use crate::preset::PlanChannel;
use sdrkit_data_plans::format_frequency;
use sdrkit_util_text::csv;
use std::path::PathBuf;

/// The frequency columns of a dial table and the factor to Hz, in the order they are tried.
const FREQUENCY_COLUMNS: &[(&str, f64)] = &[
    ("hz", 1.0),
    ("dial_hz", 1.0),
    ("frequency_hz", 1.0),
    ("freq_khz", 1e3),
    ("carrier_khz", 1e3),
    ("frequency_mhz", 1e6),
    ("freq_mhz", 1e6),
    ("centre_mhz", 1e6),
];

/// The label columns of a dial table, in the order they are tried.
const LABEL_COLUMNS: &[&str] = &["label", "entry", "submode", "set", "name", "carrier", "station", "band"];

/// The folders of the data folder that hold no dial tables.
const NOT_FAMILIES: &[&str] = &["plans", "freqdb", "stations", "cities", "dab-tii"];

/// A channel labelled by its frequency when `label` is empty.
fn channel(label: &str, hz: f64, width_hz: Option<f64>) -> PlanChannel {
    PlanChannel {
        label: if label.is_empty() { format_frequency(hz) } else { label.to_owned() },
        frequency_hz: hz,
        width_hz,
    }
}

/// `list` without the channels whose frequency an earlier channel has.
fn dedup(mut list: Vec<PlanChannel>) -> Vec<PlanChannel> {
    let mut seen: Vec<f64> = Vec::new();
    list.retain(|c| {
        let new = !seen.iter().any(|hz| (hz - c.frequency_hz).abs() < 0.5);
        seen.push(c.frequency_hz);
        new
    });
    list
}

/// An empty list for a file that the folder lacks; another error stays.
fn missing<T>(e: DataError) -> Result<Vec<T>, DataError> {
    match e {
        DataError::Missing { .. } => Ok(Vec::new()),
        e => Err(e),
    }
}

/// The stacks of the line `# stacks:` of the CSV text `text`.
fn stacks_line(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|l| l.starts_with('#'))
        .find_map(|l| l.strip_prefix('#').map(str::trim).and_then(|l| l.strip_prefix("stacks:")))
        .map(|s| s.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Every file with the extension `ext` in the family folders of `data`, sorted by path.
fn family_files(data: &Data, ext: &str) -> Vec<PathBuf> {
    let Ok(dirs) = std::fs::read_dir(data.folder()) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = dirs
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !NOT_FAMILIES.contains(&n))
        })
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|d| d.flatten().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(ext))
        .collect();
    out.sort();
    out
}

impl Data {
    /// The rows of the dial tables of the family folders for `stack`, in the order of the files and their rows. A row
    /// with the frequency and the label of an earlier row is left out.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], and [`DataError::Format`] for a row of a dial table whose frequency is not a number.
    pub fn table_dials(&self, stack: &str) -> Result<Vec<PlanChannel>, DataError> {
        let mut out: Vec<PlanChannel> = Vec::new();
        for path in family_files(self, "csv") {
            let named = stacks_line(&read_text(&path)?);
            let t = Table::read(path, &[])?;
            let header = t.header();
            let by_row = csv::column(header, &["stack"]).is_some();
            if !by_row && !named.iter().any(|s| s == stack) {
                continue;
            }
            let Some((column, factor)) = FREQUENCY_COLUMNS
                .iter()
                .find(|(c, _)| csv::column(header, &[c]).is_some())
            else {
                continue;
            };
            let has_width = csv::column(header, &["width_hz"]).is_some();
            for (line, r) in &t.rows {
                if by_row && t.text(r, "stack") != stack {
                    continue;
                }
                let field = t.text(r, column);
                let label = LABEL_COLUMNS
                    .iter()
                    .map(|c| t.text(r, c))
                    .find(|s| !s.is_empty())
                    .unwrap_or("");
                let width = if has_width { t.number(*line, r, "width_hz")?.filter(|w| *w > 0.0) } else { None };
                for item in field.split(|c: char| c == '|' || c.is_whitespace()).filter(|s| !s.is_empty()) {
                    let v: f64 = item.parse().map_err(|_| DataError::Format {
                        path: t.path.clone(),
                        reason: format!("line {line}: `{item}` in the column `{column}` is not a number"),
                    })?;
                    let hz = v * factor;
                    let label = if label.is_empty() { format_frequency(hz) } else { label.to_owned() };
                    if out.iter().any(|c| (c.frequency_hz - hz).abs() < 0.5 && c.label == label) {
                        continue;
                    }
                    out.push(channel(&label, hz, width));
                }
            }
        }
        Ok(out)
    }

    /// The channels of the regional row of `stack` for the country `code` in its family file: the HaLow region, the
    /// Z-Wave region, the first Wi-SUN band plan of the country, or its marine VHF plan ([`Data::marine_plan_of`]; both
    /// halves of a duplex channel). Empty for another stack, a country without a row,
    /// or a folder without the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], [`DataError::Format`] for a file that cannot be used.
    pub fn region_channels(&self, stack: &str, code: &str) -> Result<Vec<PlanChannel>, DataError> {
        let mhz = |hz: f64| format_frequency(hz);
        Ok(dedup(match stack {
            "halow" => {
                let rows = self.halow_regions().or_else(missing::<crate::wlan::HalowRegion>)?;
                crate::wlan::halow_region_of(&rows, code)
                    .map(|r| {
                        r.every_channel()
                            .into_iter()
                            .map(|(w, n)| {
                                let hz = r.centre_hz(n);
                                channel(&format!("{w} MHz ch {n} ({})", mhz(hz)), hz, Some(f64::from(w) * 1e6))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            "zwave" => {
                let rows = self.zwave_regions().or_else(missing::<crate::ism::ZwaveRegion>)?;
                crate::ism::zwave_region_of(&rows, code)
                    .map(|r| {
                        r.channels()
                            .into_iter()
                            .map(|(hz, rate)| channel(&format!("{} R{rate} ({})", r.id, mhz(hz)), hz, None))
                            .collect()
                    })
                    .unwrap_or_default()
            }
            "wisun" => {
                let plans = self.wisun_bands().or_else(missing::<crate::ism::WisunPlan>)?;
                plans
                    .iter()
                    .find(|p| p.countries.iter().any(|c| c.eq_ignore_ascii_case(code)))
                    .map(|p| {
                        p.classes
                            .iter()
                            .flat_map(|c| {
                                (0..c.channels).map(move |k| {
                                    let hz = c.first_hz + f64::from(k) * c.spacing_hz;
                                    (c.class, k, hz, c.spacing_hz)
                                })
                            })
                            .map(|(class, k, hz, w)| {
                                channel(&format!("{} class {class} ch {k} ({})", p.id, mhz(hz)), hz, Some(w))
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            "marinevhf" | "atis" => {
                let id = self.marine_plan_of(code)?;
                self.marine_plan(&id)
                    .or_else(|e| match e {
                        DataError::Missing { .. } => Ok(None),
                        e => Err(e),
                    })?
                    .map(|p| {
                        p.channels
                            .iter()
                            .flat_map(|c| {
                                let title = c.title();
                                let w = Some(25e3);
                                if c.simplex() {
                                    vec![channel(&title, c.ship_hz, w)]
                                } else {
                                    vec![
                                        channel(&format!("{title} (ship)"), c.ship_hz, w),
                                        channel(&format!("{title} (coast)"), c.coast_hz, w),
                                    ]
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        }))
    }

    /// The channels of the bands that name `stack` in the plan files of the family folders (`<family>/*.json` in the
    /// plan format). A file that assigns its groups to countries or ITU regions gives the groups of the country `code`;
    /// a file that assigns none gives every group.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], [`DataError::Format`] for a plan file that does not parse.
    pub fn family_plan_channels(&self, stack: &str, code: &str) -> Result<Vec<PlanChannel>, DataError> {
        let itu = self.plans.itu(code);
        let mut out = Vec::new();
        for path in family_files(self, "json") {
            let text = read_text(&path)?;
            if !sdrkit_data_plans::api::is_plan_file(&text) {
                continue;
            }
            let name = path.display().to_string();
            let file = sdrkit_data_plans::data::parse(&name, &text).map_err(|e| DataError::Format {
                path: path.clone(),
                reason: format!("{}: {}", e.path, e.reason),
            })?;
            let assigned = !file.countries.is_empty() || !file.regions.is_empty();
            let mine: Vec<&String> = file
                .countries
                .iter()
                .filter(|c| c.code == code)
                .flat_map(|c| &c.groups)
                .chain(file.regions.iter().filter(|r| Some(r.itu) == itu).flat_map(|r| &r.groups))
                .collect();
            for g in file.groups.iter().filter(|g| !assigned || mine.contains(&&g.id)) {
                for b in g.bands.iter().filter(|b| b.modes.iter().any(|m| m == stack)) {
                    out.extend(crate::plans::band_points(b));
                }
            }
        }
        Ok(out)
    }
}

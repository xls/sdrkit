//! The channel plans of the `zwave` and `wisun` stacks by region.
//!
//! `ism/zwave_regions.csv` has the columns `id,name,centre_mhz,r1_mhz,r2_mhz,r3_mhz`, one row for each region; a list field
//! holds frequencies apart by spaces. `ism/wisun_bands.csv` has the columns
//! `id,name,domain,ranges_mhz,class,first_mhz,spacing_khz,channels,modes,countries`, one row for each operating class of a
//! band plan; `countries` lists the ISO codes of the countries that use the plan, apart by spaces.
//! `ism/wisun_plan_of_frequency.csv` has the columns `low_mhz,high_mhz,id`: the plan that a host takes for a tuned
//! frequency.

use crate::data::{Data, Table};
use crate::error::DataError;
use sdrkit_core::Value;
use std::path::Path;

/// The path of the Z-Wave region file in a data folder.
pub const ZWAVE_REGIONS_FILE: &str = "ism/zwave_regions.csv";
/// The path of the Wi-SUN band plan file in a data folder.
pub const WISUN_BANDS_FILE: &str = "ism/wisun_bands.csv";
/// The path of the file of the Wi-SUN band plan for a tuned frequency in a data folder.
pub const WISUN_PLAN_OF_FREQUENCY_FILE: &str = "ism/wisun_plan_of_frequency.csv";

/// One row of the Z-Wave region file.
#[derive(Clone, Debug, PartialEq)]
pub struct ZwaveRegion {
    /// The region id ("EU").
    pub id: String,
    /// The region name.
    pub name: String,
    /// The frequency to tune, Hz.
    pub centre_hz: f64,
    /// The channels of the rates R1, R2 and R3, Hz.
    pub rates_hz: [Vec<f64>; 3],
}

impl ZwaveRegion {
    /// Every channel of the region as (frequency in Hz, rate), R1 first.
    pub fn channels(&self) -> Vec<(f64, u8)> {
        let mut out = Vec::new();
        for (rate, list) in (1u8..).zip(&self.rates_hz) {
            out.extend(list.iter().map(|&hz| (hz, rate)));
        }
        out
    }

    /// The value of the `zwave` setting `channels`: one entry "Hz/rate" for each channel, R1 first.
    pub fn channels_text(&self) -> String {
        self.channels()
            .iter()
            .map(|(hz, r)| format!("{hz:.0}/{r}"))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// The `zwave` settings of the region: `channels`. The host tunes to [`ZwaveRegion::centre_hz`].
    pub fn settings(&self) -> Vec<(String, Value)> {
        vec![("channels".to_owned(), Value::Text(self.channels_text()))]
    }
}

/// Reads a Z-Wave region file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a missing column, an empty id or centre, or
/// a list entry that is not a number.
pub fn read_zwave_regions(path: &Path) -> Result<Vec<ZwaveRegion>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &["id", "name", "centre_mhz", "r1_mhz", "r2_mhz", "r3_mhz"],
    )?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        out.push(ZwaveRegion {
            id: required_text(&t, *line, r, "id")?,
            name: t.text(r, "name").to_owned(),
            centre_hz: t.required(*line, r, "centre_mhz")? * 1e6,
            rates_hz: [
                mhz_list(&t, *line, r, "r1_mhz")?,
                mhz_list(&t, *line, r, "r2_mhz")?,
                mhz_list(&t, *line, r, "r3_mhz")?,
            ],
        });
    }
    Ok(out)
}

/// The `zwave` settings and the frequency to tune of the region with `id`. `None` when no row has it.
pub fn zwave_settings(rows: &[ZwaveRegion], id: &str) -> Option<(f64, Vec<(String, Value)>)> {
    rows.iter()
        .find(|r| r.id.eq_ignore_ascii_case(id))
        .map(|r| (r.centre_hz, r.settings()))
}

/// The region with the most channels within ±0.45 `rate_hz` of `centre_hz` (the first such row on a tie). `None` when no
/// row has a channel there.
pub fn zwave_region_at(rows: &[ZwaveRegion], centre_hz: f64, rate_hz: f64) -> Option<&ZwaveRegion> {
    let mut best = None;
    let mut most = 0;
    for r in rows {
        let n = r
            .channels()
            .iter()
            .filter(|c| (c.0 - centre_hz).abs() < 0.45 * rate_hz)
            .count();
        if n > most {
            most = n;
            best = Some(r);
        }
    }
    best
}

/// An operating class of a Wi-SUN band plan.
#[derive(Clone, Debug, PartialEq)]
pub struct WisunClass {
    /// The class number, from 1.
    pub class: u32,
    /// The centre of channel 0, Hz.
    pub first_hz: f64,
    /// The channel spacing, Hz.
    pub spacing_hz: f64,
    /// The number of channels.
    pub channels: u32,
    /// The FSK modes: (bit rate in kbit/s, modulation index). Empty for a class that the stack does not receive.
    pub modes: Vec<(u32, f32)>,
}

/// A Wi-SUN band plan: the rows of one id of the band plan file.
#[derive(Clone, Debug, PartialEq)]
pub struct WisunPlan {
    /// The plan id ("ANZ").
    pub id: String,
    /// The plan name.
    pub name: String,
    /// The regulatory domain value of the PHY.
    pub domain: u8,
    /// The usable ranges (low, high), Hz.
    pub ranges_hz: Vec<(f64, f64)>,
    /// The operating classes in the order of their numbers 1, 2, ...
    pub classes: Vec<WisunClass>,
    /// The ISO 3166-1 alpha-2 codes of the countries that use the plan, upper case. Empty for a plan that no country
    /// is known to use.
    pub countries: Vec<String>,
}

impl WisunPlan {
    /// The value of the `wisun` setting `ranges`.
    pub fn ranges_text(&self) -> String {
        self.ranges_hz
            .iter()
            .map(|(lo, hi)| format!("{lo:.0}-{hi:.0}"))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// The value of the `wisun` setting `classes`.
    pub fn classes_text(&self) -> String {
        self.classes
            .iter()
            .map(|c| {
                let modes: Vec<String> = c.modes.iter().map(|(k, h)| format!("{k}:{h}")).collect();
                format!(
                    "{:.0}/{:.0}/{}/{}",
                    c.first_hz,
                    c.spacing_hz,
                    c.channels,
                    modes.join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// The frequency to tune, Hz: the middle of the widest usable range. `None` for a plan without a range.
    pub fn centre_hz(&self) -> Option<f64> {
        self.ranges_hz
            .iter()
            .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
            .map(|(lo, hi)| (lo + hi) / 2.0)
    }

    /// The `wisun` settings of the plan: `ranges` and `classes`.
    pub fn settings(&self) -> Vec<(String, Value)> {
        vec![
            ("ranges".to_owned(), Value::Text(self.ranges_text())),
            ("classes".to_owned(), Value::Text(self.classes_text())),
        ]
    }
}

/// Reads a Wi-SUN band plan file. The rows of one id make one plan, in the order of the first row of each id; the classes
/// of a plan must be numbered 1, 2, ... without a gap, and its rows must agree on the name, the domain and the ranges.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a missing column, an empty id, a number
/// that does not parse, a range that is not `low-high` in MHz, a mode that is not `kbps:index`, rows of one id that
/// disagree on the name, the domain, the ranges or the countries, or classes with a gap.
pub fn read_wisun_bands(path: &Path) -> Result<Vec<WisunPlan>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &[
            "id",
            "name",
            "domain",
            "ranges_mhz",
            "class",
            "first_mhz",
            "spacing_khz",
            "channels",
            "modes",
            "countries",
        ],
    )?;
    let bad = |line: usize, reason: String| DataError::Format {
        path: t.path.clone(),
        reason: format!("line {line}: {reason}"),
    };
    let mut out: Vec<WisunPlan> = Vec::new();
    for (line, r) in &t.rows {
        let line = *line;
        let id = required_text(&t, line, r, "id")?;
        let domain = u8::try_from(t.count(line, r, "domain")?)
            .map_err(|_| bad(line, "the domain is above 255".into()))?;
        let mut ranges_hz = Vec::new();
        for b in t.text(r, "ranges_mhz").split_whitespace() {
            let range = b
                .split_once('-')
                .and_then(|(lo, hi)| Some((lo.parse::<f64>().ok()?, hi.parse::<f64>().ok()?)))
                .ok_or_else(|| {
                    bad(
                        line,
                        format!("`{b}` in the column `ranges_mhz` is not low-high"),
                    )
                })?;
            ranges_hz.push((range.0 * 1e6, range.1 * 1e6));
        }
        let mut modes = Vec::new();
        for m in t.text(r, "modes").split_whitespace() {
            let mode = m
                .split_once(':')
                .and_then(|(k, h)| Some((k.parse::<u32>().ok()?, h.parse::<f32>().ok()?)))
                .ok_or_else(|| {
                    bad(
                        line,
                        format!("`{m}` in the column `modes` is not kbps:index"),
                    )
                })?;
            modes.push(mode);
        }
        let class = WisunClass {
            class: u32::try_from(t.count(line, r, "class")?).unwrap_or(u32::MAX),
            first_hz: t.required(line, r, "first_mhz")? * 1e6,
            spacing_hz: t.required(line, r, "spacing_khz")? * 1e3,
            channels: u32::try_from(t.count(line, r, "channels")?).unwrap_or(u32::MAX),
            modes,
        };
        let name = t.text(r, "name").to_owned();
        let countries: Vec<String> = t
            .text(r, "countries")
            .split_whitespace()
            .map(str::to_ascii_uppercase)
            .collect();
        match out.iter_mut().find(|p| p.id == id) {
            Some(p) => {
                if p.name != name
                    || p.domain != domain
                    || p.ranges_hz != ranges_hz
                    || p.countries != countries
                {
                    return Err(bad(line, format!("the rows of `{id}` disagree")));
                }
                p.classes.push(class);
            }
            None => out.push(WisunPlan {
                id,
                name,
                domain,
                ranges_hz,
                classes: vec![class],
                countries,
            }),
        }
    }
    for p in &mut out {
        p.classes.sort_by_key(|c| c.class);
        if p.classes.iter().zip(1u32..).any(|(c, n)| c.class != n) {
            return Err(DataError::Format {
                path: t.path.clone(),
                reason: format!("the classes of `{}` are not numbered 1, 2, ...", p.id),
            });
        }
    }
    Ok(out)
}

/// The `wisun` settings of the plan with `id`. `None` when no plan has it.
pub fn wisun_settings(plans: &[WisunPlan], id: &str) -> Option<Vec<(String, Value)>> {
    plans
        .iter()
        .find(|p| p.id.eq_ignore_ascii_case(id))
        .map(WisunPlan::settings)
}

/// The `wisun` settings and the frequency to tune ([`WisunPlan::centre_hz`]) of the first plan in file order that lists
/// the country `code`. A country with two plans (Singapore) takes the first. `None` when no plan lists the country.
pub fn wisun_country_settings(plans: &[WisunPlan], code: &str) -> Option<(f64, Vec<(String, Value)>)> {
    let p = plans
        .iter()
        .find(|p| p.countries.iter().any(|c| c.eq_ignore_ascii_case(code)))?;
    Some((p.centre_hz()?, p.settings()))
}

/// One row of the file of the Wi-SUN band plan for a tuned frequency.
#[derive(Clone, Debug, PartialEq)]
pub struct WisunPlanRange {
    /// The lowest frequency, Hz (inclusive).
    pub low_hz: f64,
    /// The highest frequency, Hz (exclusive).
    pub high_hz: f64,
    /// The plan id.
    pub id: String,
}

/// Reads the file of the Wi-SUN band plan for a tuned frequency.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a missing column, an empty id or edge, or
/// a range whose high edge is not above its low edge.
pub fn read_wisun_plan_ranges(path: &Path) -> Result<Vec<WisunPlanRange>, DataError> {
    let t = Table::read(path.to_owned(), &["low_mhz", "high_mhz", "id"])?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        let low_hz = t.required(*line, r, "low_mhz")? * 1e6;
        let high_hz = t.required(*line, r, "high_mhz")? * 1e6;
        if high_hz <= low_hz {
            return Err(DataError::Format {
                path: t.path.clone(),
                reason: format!("line {line}: the high edge is not above the low edge"),
            });
        }
        out.push(WisunPlanRange {
            low_hz,
            high_hz,
            id: required_text(&t, *line, r, "id")?,
        });
    }
    Ok(out)
}

/// The plan of `plans` that the first range of `ranges` holding `hz` names. `None` when no range holds `hz` or the plan
/// it names is not in `plans`.
pub fn wisun_plan_at<'a>(
    ranges: &[WisunPlanRange],
    plans: &'a [WisunPlan],
    hz: f64,
) -> Option<&'a WisunPlan> {
    let r = ranges.iter().find(|r| hz >= r.low_hz && hz < r.high_hz)?;
    plans.iter().find(|p| p.id == r.id)
}

/// The frequencies in MHz of the list field `name` of `row`, in Hz. An empty field is an empty list.
fn mhz_list(t: &Table, line: usize, row: &[String], name: &str) -> Result<Vec<f64>, DataError> {
    t.text(row, name)
        .split_whitespace()
        .map(|n| match n.parse::<f64>() {
            Ok(v) if v.is_finite() => Ok(v * 1e6),
            _ => Err(DataError::Format {
                path: t.path.clone(),
                reason: format!("line {line}: `{n}` in the column `{name}` is not a number"),
            }),
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

impl Data {
    /// The rows of [`ZWAVE_REGIONS_FILE`] of the folder.
    ///
    /// # Errors
    ///
    /// What [`read_zwave_regions`] returns.
    pub fn zwave_regions(&self) -> Result<Vec<ZwaveRegion>, DataError> {
        read_zwave_regions(&self.folder().join(ZWAVE_REGIONS_FILE))
    }

    /// The plans of [`WISUN_BANDS_FILE`] of the folder.
    ///
    /// # Errors
    ///
    /// What [`read_wisun_bands`] returns.
    pub fn wisun_bands(&self) -> Result<Vec<WisunPlan>, DataError> {
        read_wisun_bands(&self.folder().join(WISUN_BANDS_FILE))
    }

    /// The rows of [`WISUN_PLAN_OF_FREQUENCY_FILE`] of the folder.
    ///
    /// # Errors
    ///
    /// What [`read_wisun_plan_ranges`] returns.
    pub fn wisun_plan_ranges(&self) -> Result<Vec<WisunPlanRange>, DataError> {
        read_wisun_plan_ranges(&self.folder().join(WISUN_PLAN_OF_FREQUENCY_FILE))
    }
}

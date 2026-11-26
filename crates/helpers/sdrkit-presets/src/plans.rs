//! The plan files of the data folder, and the plan that a country has for a stack.
//!
//! A country uses the groups that its entries list. For a kind of plan that it lists no group of, it uses the groups of
//! its ITU region, unless it lists the kind as absent. A plan in the CSV or plan JSON form applies in the countries and
//! ITU regions that it names, or everywhere if it names none.
//!
//! A band of a group lists the stacks that receive it. For a service with one channel raster (FM, DAB, television) that
//! makes the group the plan of those stacks. For another stack it says only where the stack can receive, not where it
//! starts: such a stack has a plan only in a group of its own, whose id starts with the stack id (`fanet_au`), or in a
//! plan in the CSV or plan JSON form that names it.

use crate::data::{io_error, read_text};
use crate::error::DataError;
use crate::preset::PlanChannel;
use sdrkit_data_plans::data::{self, Known};
use sdrkit_data_plans::{
    api, band_channels, format_frequency, io, Band, BandGroup, ChannelPlan, CountryEntry, PlanFile,
    PlanKind, Presets, RegionEntry,
};
use sdrkit_util_text::csv;
use std::path::Path;

/// The most channels that a raster of one band gives.
const MAX_POINTS: usize = sdrkit_data_plans::registry::MAX_CHANNELS;

/// A plan in the CSV or plan JSON form, with the `band` field of each channel (CSV only).
#[derive(Debug)]
struct ListPlan {
    plan: ChannelPlan,
    bands: Vec<String>,
}

/// Every plan file of a data folder.
#[derive(Debug, Default)]
pub(crate) struct PlanSet {
    groups: Vec<BandGroup>,
    countries: Vec<CountryEntry>,
    regions: Vec<RegionEntry>,
    lists: Vec<ListPlan>,
}

/// The plan of a stack in a country.
#[derive(Debug)]
pub(crate) struct Found {
    /// The id of the group or the plan.
    pub id: String,
    /// The channels that the plan has for the stack, in the order of the plan.
    pub channels: Vec<PlanChannel>,
}

/// The channels of one band: its numbered and listed channels in use, else the points of its raster, else its centre
/// if the band is one channel (not wider than one and a half channel widths). A wider band without channels has none.
pub(crate) fn band_points(b: &Band) -> Vec<PlanChannel> {
    if !b.in_use {
        return Vec::new();
    }
    let width = |w: f64| (w > 0.0).then_some(w);
    let listed = band_channels(b);
    if !listed.is_empty() {
        return listed
            .into_iter()
            .filter(|c| !c.reallocated)
            .map(|c| PlanChannel {
                label: c.name,
                frequency_hz: c.centre_hz,
                width_hz: width(c.width_hz),
            })
            .collect();
    }
    if let Some(r) = b.raster.as_ref().filter(|r| r.step_hz > 0.0) {
        let first = ((b.start_hz - r.first_hz) / r.step_hz).ceil().max(0.0);
        return (0..MAX_POINTS)
            .map(|i| r.first_hz + (first + i as f64) * r.step_hz)
            .take_while(|hz| *hz <= b.end_hz)
            .map(|hz| PlanChannel {
                label: format_frequency(hz),
                frequency_hz: hz,
                width_hz: width(b.channel_width_hz.unwrap_or(r.step_hz)),
            })
            .collect();
    }
    match b.channel_width_hz {
        Some(w) if w > 0.0 && b.end_hz - b.start_hz <= 1.5 * w => vec![PlanChannel {
            label: b.name.clone(),
            frequency_hz: (b.start_hz + b.end_hz) / 2.0,
            width_hz: Some(w),
        }],
        _ => Vec::new(),
    }
}

/// True if the group `id` is the stack's own: its id is the stack id, or the stack id and '_' (`fanet_au`).
fn of_stack(id: &str, stack: &str) -> bool {
    id.strip_prefix(stack)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('_'))
}

/// The `band` field of each channel row of a CSV plan, in the order of the rows. Empty if the file has no such column.
fn csv_bands(text: &str) -> Vec<String> {
    let mut rows = csv::read(text, ',').into_iter();
    let Some((_, header)) = rows.next() else {
        return Vec::new();
    };
    let Some(col) = csv::column(&header, &["band"]) else {
        return Vec::new();
    };
    rows.map(|(_, r)| r.get(col).map_or("", |s| s.trim()).to_owned())
        .collect()
}

impl PlanSet {
    /// Every band group, in the order of the files.
    pub(crate) fn groups(&self) -> impl Iterator<Item = &BandGroup> {
        self.groups.iter()
    }

    /// Reads every `.json` and `.csv` file of `dir`, in the order of their names, and checks each plan file.
    pub(crate) fn load(dir: &Path) -> Result<PlanSet, DataError> {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| io_error(dir, e))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        paths.sort();
        let mut set = PlanSet::default();
        let mut files: Vec<(String, PlanFile)> = Vec::new();
        for path in &paths {
            let format = |reason: String| DataError::Format {
                path: path.clone(),
                reason,
            };
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("plan");
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => {
                    let text = read_text(path)?;
                    if api::is_plan_file(&text) {
                        let name = path.display().to_string();
                        let file = data::parse(&name, &text)
                            .map_err(|e| format(format!("{}: {}", e.path, e.reason)))?;
                        files.push((name, file));
                    } else {
                        let plans = io::from_json(&text).map_err(|e| format(e.to_string()))?;
                        set.lists.extend(plans.into_iter().map(|plan| ListPlan {
                            plan,
                            bands: Vec::new(),
                        }));
                    }
                }
                Some("csv") => {
                    let text = read_text(path)?;
                    let plan =
                        io::from_csv(&text, stem, stem).map_err(|e| format(e.to_string()))?;
                    let mut bands = csv_bands(&text);
                    if bands.len() != plan.channels.len() {
                        bands.clear();
                    }
                    set.lists.push(ListPlan { plan, bands });
                }
                _ => {}
            }
        }
        // Each file is checked on its own, and may name the groups and the countries of the other files.
        for (i, one) in files.iter().enumerate() {
            let others = || files.iter().enumerate().filter(move |(j, _)| *j != i);
            let known = Known {
                groups: others()
                    .flat_map(|(_, (_, f))| f.groups.iter().map(|g| g.id.clone()))
                    .collect(),
                countries: others()
                    .flat_map(|(_, (_, f))| f.countries.iter().map(|c| c.code.clone()))
                    .collect(),
            };
            if let Some(e) = data::check(std::slice::from_ref(one), &known)
                .into_iter()
                .next()
            {
                return Err(DataError::Format {
                    path: e.file.into(),
                    reason: format!("{}: {}", e.path, e.reason),
                });
            }
        }
        for (_, f) in files {
            set.groups.extend(f.groups);
            set.countries.extend(f.countries);
            set.regions.extend(f.regions);
        }
        Ok(set)
    }

    pub(crate) fn group(&self, id: &str) -> Option<&BandGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// Appends the bands of `g` to `out`, those of the groups it includes first.
    fn bands<'a>(&'a self, g: &'a BandGroup, seen: &mut Vec<&'a str>, out: &mut Vec<&'a Band>) {
        for id in &g.include {
            if seen.contains(&id.as_str()) {
                continue;
            }
            seen.push(id);
            if let Some(inner) = self.group(id) {
                self.bands(inner, seen, out);
            }
        }
        out.extend(&g.bands);
    }

    /// The channels that the group `id` has for `stack`. A group whose id starts with the stack id is the plan of the
    /// stack with every band that names no stack or this one. In another group, the bands that name the stack count
    /// if `shared` is set.
    fn group_channels(&self, id: &str, stack: &str, shared: bool) -> Vec<PlanChannel> {
        let Some(g) = self.group(id) else {
            return Vec::new();
        };
        let mut bands = Vec::new();
        self.bands(g, &mut vec![g.id.as_str()], &mut bands);
        let own = of_stack(id, stack);
        bands
            .into_iter()
            .filter(|b| {
                let named = b.modes.iter().any(|m| m == stack);
                if own {
                    named || b.modes.is_empty()
                } else {
                    shared && named
                }
            })
            .flat_map(band_points)
            .collect()
    }

    /// The ITU region of the country `code`.
    pub(crate) fn itu(&self, code: &str) -> Option<u8> {
        self.countries
            .iter()
            .filter(|c| c.code == code)
            .find_map(|c| c.itu)
    }

    /// The presets of the country `code`: its own, each one that is not set from its ITU region.
    pub(crate) fn presets(&self, code: &str) -> Presets {
        let mut p = Presets::default();
        for c in self.countries.iter().filter(|c| c.code == code) {
            p = p.over(&c.presets);
        }
        let itu = self.itu(code);
        for r in self.regions.iter().filter(|r| Some(r.itu) == itu) {
            p = p.over(&r.presets);
        }
        p
    }

    /// The plan that the country `code` has for `stack`. With `shared`, a group that lists the stack on its bands is
    /// a plan of the stack; without it, only a group or a plan of the stack's own is. `None` if the country has none.
    pub(crate) fn find(&self, stack: &str, code: &str, shared: bool) -> Option<Found> {
        let found = |id: &str| {
            let channels = self.group_channels(id, stack, shared);
            (!channels.is_empty()).then(|| Found {
                id: id.to_owned(),
                channels,
            })
        };
        let own: Vec<&CountryEntry> = self.countries.iter().filter(|c| c.code == code).collect();
        let listed = || own.iter().flat_map(|c| &c.groups);
        if let Some(f) = listed().find_map(|id| found(id)) {
            return Some(f);
        }
        // The kinds that the ITU region does not decide for this country: those it lists as absent, and those it lists
        // a group of. A listed group does not close the region's group of the stack's own, as it is no plan of the
        // stack.
        let absent: Vec<PlanKind> = own.iter().flat_map(|c| c.absent.clone()).collect();
        let listed_kinds: Vec<PlanKind> = listed().filter_map(|id| self.group(id)).map(|g| g.kind).collect();
        let itu = self.itu(code);
        let regional = self
            .regions
            .iter()
            .filter(|r| Some(r.itu) == itu)
            .flat_map(|r| &r.groups)
            .filter(|id| {
                self.group(id).is_some_and(|g| {
                    !absent.contains(&g.kind)
                        && (of_stack(id, stack) || !listed_kinds.contains(&g.kind))
                })
            })
            .find_map(|id| found(id));
        if regional.is_some() {
            return regional;
        }
        self.lists.iter().find_map(|l| {
            let p = &l.plan;
            let here = (p.countries.is_empty() && p.itu_regions.is_empty())
                || p.countries.iter().any(|c| c == code)
                || itu.is_some_and(|i| p.itu_regions.contains(&i));
            if !here || !p.modes.iter().any(|m| m == stack) {
                return None;
            }
            // A row whose band is a stack of the plan belongs to that stack only.
            let of_stack = |i: usize| {
                l.bands
                    .get(i)
                    .is_none_or(|b| b == stack || !p.modes.contains(b))
            };
            let channels: Vec<PlanChannel> = p
                .channels
                .iter()
                .enumerate()
                .filter(|(i, c)| !c.reallocated && of_stack(*i))
                .map(|(_, c)| PlanChannel {
                    label: c.name.clone(),
                    frequency_hz: c.centre_hz,
                    width_hz: (c.width_hz > 0.0).then_some(c.width_hz),
                })
                .collect();
            (!channels.is_empty()).then(|| Found {
                id: p.id.clone(),
                channels,
            })
        })
    }
}

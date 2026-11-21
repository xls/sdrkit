//! What a host offers to choose from: the data folder, the countries that have plans, and the channels of a stack.

use crate::data::Data;
use crate::default_dials::{read_dials, DefaultDial, DEFAULT_DIALS};
use crate::error::DataError;
use crate::preset::{recipe, PlanChannel};
use crate::region::Region;
use std::path::{Path, PathBuf};

/// One country that a host offers as a region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionChoice {
    /// The ISO 3166 alpha-2 code, in upper case.
    pub code: String,
    /// The name of the country, from the plan files.
    pub name: String,
}

impl RegionChoice {
    /// The line that a list shows: `AU  Australia`.
    pub fn line(&self) -> String {
        format!("{}  {}", self.code, self.name)
    }
}

/// The data folder of a host: `given`, else the first of these that holds a `plans` folder: `data` in `storage`,
/// `~/.sdrkit/data`, then for each folder above the program file `data` (the repository) and `share/sdrkit/data` (an
/// installed copy), then `data` in the current folder. If none has plans, `data` in `storage`, else `~/.sdrkit/data`:
/// the folder that a message names.
pub fn data_folder(given: Option<&Path>, storage: Option<&Path>) -> PathBuf {
    if let Some(g) = given {
        return g.to_owned();
    }
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let program = std::env::current_exe().ok();
    let current = std::env::current_dir().ok();
    let list = candidates(storage, &home, program.as_deref(), current.as_deref());
    let own = list[0].clone();
    first_with_plans(list).unwrap_or(own)
}

/// The folders that [`data_folder`] tries, in order, for the storage folder `storage`, the home folder `home`, the
/// program file `program` and the current folder `current`. The first is the folder of a message.
fn candidates(storage: Option<&Path>, home: &Path, program: Option<&Path>, current: Option<&Path>) -> Vec<PathBuf> {
    let home = home.join(".sdrkit/data");
    let mut out = vec![storage.map_or_else(|| home.clone(), |s| s.join("data")), home];
    for folder in program.into_iter().flat_map(|p| p.ancestors().skip(1)) {
        out.push(folder.join("data"));
        out.push(folder.join("share/sdrkit/data"));
    }
    out.extend(current.map(|c| c.join("data")));
    out
}

/// The first folder of `candidates` that holds a folder `plans`.
fn first_with_plans(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|c| c.join("plans").is_dir())
}

/// `r`, with a missing file read as nothing.
fn or_empty<T: Default>(r: Result<T, DataError>) -> Result<T, DataError> {
    match r {
        Err(DataError::Missing { .. }) => Ok(T::default()),
        r => r,
    }
}

/// The dials of `<family>/default_dials.csv` for every folder of `data` that holds the file.
fn every_dial(data: &Data) -> Result<Vec<DefaultDial>, DataError> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir(data.folder()) else {
        return Ok(out);
    };
    let mut files: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path().join(DEFAULT_DIALS))
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for f in files {
        out.extend(read_dials(&f)?);
    }
    Ok(out)
}

impl Data {
    /// The countries that have at least one plan, by code: the regions that a host offers. The names are those of the
    /// country rows of the plan files.
    pub fn regions(&self) -> Vec<RegionChoice> {
        let mut out: Vec<RegionChoice> = Vec::new();
        for c in self.store.country_rows() {
            match out.iter_mut().find(|r| r.code == c.code) {
                Some(r) if r.name.is_empty() => r.name.clone_from(&c.name),
                Some(_) => {}
                None => out.push(RegionChoice {
                    code: c.code.clone(),
                    name: c.name.clone(),
                }),
            }
        }
        out.retain(|r| Region::new(&r.code).is_ok() && !self.plan_ids(&r.code).is_empty());
        out.sort_by(|a, b| a.code.cmp(&b.code));
        out
    }

    /// The region of the country `code` (any case), if [`Data::regions`] has it.
    ///
    /// # Errors
    ///
    /// The list of the regions, one [`RegionChoice::line`] a line, if it does not have the code.
    pub fn region(&self, code: &str) -> Result<Region, String> {
        let regions = self.regions();
        let code = code.trim().to_ascii_uppercase();
        match Region::new(&code) {
            Ok(r) if regions.iter().any(|c| c.code == code) => Ok(r),
            _ => Err(regions
                .iter()
                .map(RegionChoice::line)
                .collect::<Vec<_>>()
                .join("\n")),
        }
    }

    /// The starting dial that the data has for `stack` in `submode`, else for the stack itself.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], [`DataError::Format`] for a dial file that cannot be used.
    pub fn dial_hz(&self, stack: &str, submode: Option<&str>) -> Result<Option<f64>, DataError> {
        Ok(crate::default_dials::dial_hz(
            &every_dial(self)?,
            stack,
            submode,
        ))
    }

    /// The channels that a host offers for `stack` in `region`, lowest first: those of the plan that the country has
    /// for the stack, with their widths; else the scan channels of the plans that the country uses; else the dials of
    /// every band plan of the LoRa files that a mesh stack offers in the country; else the downlinks of the stack; else
    /// its starting dials, each with its submode as the label. The channels of the user's plans
    /// (see [`Data::open_personal_plans`]) that name the stack among their stacks, and the country or no country,
    /// come in addition; at a frequency that both have, the user's channel stays. Empty if the data has none.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], [`DataError::Format`] for a file that cannot be used.
    pub fn stack_channels(
        &self,
        stack: &str,
        region: &Region,
    ) -> Result<Vec<PlanChannel>, DataError> {
        let country = region.of_stack(stack).country();
        let mut out = self
            .plans
            .find(stack, country, recipe(stack).is_some())
            .map(|f| f.channels)
            .unwrap_or_default();
        if out.is_empty() {
            let ids = self.plan_ids(country);
            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
            let plan = |(label, hz): (String, f64)| PlanChannel {
                label,
                frequency_hz: hz,
                width_hz: None,
            };
            out = self
                .scan_channels(&ids)
                .remove(stack)
                .map(|c| c.into_iter().map(plan).collect())
                .unwrap_or_default();
        }
        if out.is_empty() {
            out = self.mesh_channels(stack, country)?;
        }
        if out.is_empty() {
            out = self
                .downlinks(stack)?
                .into_iter()
                .map(|d| PlanChannel {
                    label: format!("{} {}", d.name, d.submode).trim().to_owned(),
                    frequency_hz: d.frequency_hz,
                    width_hz: d.width_hz,
                })
                .collect();
        }
        if out.is_empty() {
            out = every_dial(self)?
                .into_iter()
                .filter(|d| d.stack == stack)
                .map(|d| PlanChannel {
                    label: d.label,
                    frequency_hz: d.hz,
                    width_hz: None,
                })
                .collect();
        }
        let own = self.user_channels(stack, country);
        out.retain(|c| !own.iter().any(|o| (o.frequency_hz - c.frequency_hz).abs() < 0.5));
        out.extend(own);
        out.sort_by(|a, b| a.frequency_hz.total_cmp(&b.frequency_hz));
        Ok(out)
    }

    /// The dials of every band plan of the LoRa files that the mesh stack `stack` offers in the Meshtastic region of
    /// `country`, labelled by the name of the plan (the MeshCore dial for `meshcore`). Empty for another stack, a
    /// country without a region, or a folder without the LoRa files.
    fn mesh_channels(&self, stack: &str, country: &str) -> Result<Vec<PlanChannel>, DataError> {
        let Some(region) = or_empty(self.meshtastic_region_of(country))? else {
            return Ok(Vec::new());
        };
        Ok(or_empty(self.mesh_band_plans_in(stack, &region))?
            .into_iter()
            .map(|p| PlanChannel {
                label: p.name,
                frequency_hz: if stack == "meshcore" {
                    p.meshcore_dial_hz
                } else {
                    p.dial_hz
                },
                width_hz: None,
            })
            .collect())
    }

    /// The channels of the user's plans that list `stack` and list `country` (upper case) or no country.
    fn user_channels(&self, stack: &str, country: &str) -> Vec<PlanChannel> {
        self.store
            .user_plans()
            .iter()
            .filter(|p| p.modes.iter().any(|m| m == stack))
            .filter(|p| p.countries.is_empty() || p.countries.iter().any(|c| c.eq_ignore_ascii_case(country)))
            .flat_map(|p| &p.channels)
            .map(|c| PlanChannel {
                label: c.name.clone(),
                frequency_hz: c.centre_hz,
                width_hz: (c.width_hz > 0.0).then_some(c.width_hz),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A new empty folder for one test.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sdrkit-presets-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a test folder");
        dir
    }

    #[test]
    fn the_storage_folder_comes_first_then_the_folders_above_the_program() {
        let root = folder("order");
        let (storage, home, current) = (root.join("storage"), root.join("home"), root.join("work"));
        let program = root.join("prefix/bin/sdrkit");
        let list = || candidates(Some(&storage), &home, Some(&program), Some(&current));
        // Nothing has plans: the folder of a message is that of the storage.
        assert_eq!(first_with_plans(list()), None);
        assert_eq!(list()[0], storage.join("data"));
        // An installed copy: `share/sdrkit/data` beside the folder of the program.
        let installed = root.join("prefix/share/sdrkit/data");
        std::fs::create_dir_all(installed.join("plans")).expect("plans");
        assert_eq!(first_with_plans(list()), Some(installed));
        // The repository: `data` in a folder above the program comes before a folder further up.
        let repo = root.join("prefix/bin/data");
        std::fs::create_dir_all(repo.join("plans")).expect("plans");
        assert_eq!(first_with_plans(list()), Some(repo));
        // The home folder comes before the program, and the storage folder before both.
        std::fs::create_dir_all(home.join(".sdrkit/data/plans")).expect("plans");
        assert_eq!(first_with_plans(list()), Some(home.join(".sdrkit/data")));
        std::fs::create_dir_all(storage.join("data/plans")).expect("plans");
        assert_eq!(first_with_plans(list()), Some(storage.join("data")));
        assert_eq!(data_folder(None, Some(&storage)), storage.join("data"));
        assert_eq!(data_folder(Some(&current), Some(&storage)), current);
        let _ = std::fs::remove_dir_all(root);
    }
}

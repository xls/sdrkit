//! The plan files that the host loads and the user's files, read by one loader. It returns the plans by id and by kind,
//! the rows of the files, the name of a frequency in the plans that the host names, typed channel names, and checks on
//! user plans. A store with no files is valid and holds no plans.

use crate::compile::{bands_of, group_of, plan_of};
use crate::data::{self, BandGroup, CountryEntry, DataError, Known, PlanFile, PlanKind, RegionEntry};
use crate::model::{ChannelPlan, Direction, Place};
use sdrkit_core::{StorageError, StorePath};
use std::sync::Arc;

/// Why a plan or a plan file was refused.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PlanError {
    /// The plan breaks one or more rules (each problem in plain words, with its JSON path for a plan file).
    #[error("the plan is not valid: {}", .0.join("; "))]
    Invalid(Vec<String>),
    /// A plan file could not be read from the host's storage.
    #[error("{file}: {error}")]
    Read {
        /// The path of the file in the storage.
        file: String,
        /// Why the storage refused it.
        error: StorageError,
    },
    /// The JSON text could not be read.
    #[error("the file could not be read: {kind} at line {line}, column {column}")]
    Parse {
        /// What kind of fault the text has.
        kind: ParseFault,
        /// The line of the fault, counted from 1.
        line: usize,
        /// The column of the fault, counted from 1.
        column: usize,
    },
}

/// The kind of fault in a JSON text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, thiserror::Error)]
pub enum ParseFault {
    /// The text is not valid JSON.
    #[error("bad JSON syntax")]
    Syntax,
    /// The JSON does not describe a plan: a wrong type or a missing field.
    #[error("the JSON is not a plan")]
    Data,
    /// The text ends too early.
    #[error("the text ends too early")]
    Eof,
    /// The text could not be read from its source.
    #[error("the text could not be read")]
    Io,
}

impl From<&serde_json::Error> for PlanError {
    fn from(e: &serde_json::Error) -> PlanError {
        use serde_json::error::Category;
        let kind = match e.classify() {
            Category::Syntax => ParseFault::Syntax,
            Category::Data => ParseFault::Data,
            Category::Eof => ParseFault::Eof,
            Category::Io => ParseFault::Io,
        };
        PlanError::Parse {
            kind,
            line: e.line(),
            column: e.column(),
        }
    }
}

impl PlanError {
    fn data(errors: Vec<DataError>) -> PlanError {
        PlanError::Invalid(errors.iter().map(DataError::to_string).collect())
    }
}

/// Most channels a user plan can have.
pub const MAX_CHANNELS: usize = data::MAX_CHANNELS;
/// Most raster points a user segment can have.
pub const MAX_RASTER_POINTS: usize = data::MAX_RASTER_POINTS;
/// Most user plans kept.
pub const MAX_USER_PLANS: usize = 200;

/// One band group with its bands (includes resolved) and its plan view.
#[derive(Clone, Debug)]
pub struct GroupEntry {
    /// The group as written, with the bands of the groups it includes put first.
    pub group: BandGroup,
    /// The plan view.
    pub plan: ChannelPlan,
}

/// The plans of the loaded files, with the bands of included groups resolved.
#[derive(Debug, Default)]
struct Loaded {
    groups: Vec<GroupEntry>,
    plans: Vec<ChannelPlan>,
    countries: Vec<CountryEntry>,
    regions: Vec<RegionEntry>,
}

impl Loaded {
    /// Builds the plan views of the groups of `file`, which has passed [`data::check`].
    fn of(file: PlanFile) -> Loaded {
        let find = |id: &str| file.groups.iter().find(|g| g.id == id);
        let mut groups: Vec<GroupEntry> = file
            .groups
            .iter()
            .map(|g| {
                let bands = bands_of(g, &find);
                let mut plan = plan_of(g, &bands, false);
                plan.countries = file
                    .countries
                    .iter()
                    .filter(|c| c.groups.contains(&g.id))
                    .map(|c| c.code.clone())
                    .collect();
                plan.itu_regions = file
                    .regions
                    .iter()
                    .filter(|r| r.groups.contains(&g.id))
                    .map(|r| r.itu)
                    .collect();
                let mut group = g.clone();
                group.bands = bands;
                GroupEntry { group, plan }
            })
            .collect();
        groups.sort_by_key(|e| e.plan.kind);
        let plans = groups.iter().map(|e| e.plan.clone()).collect();
        let mut countries = file.countries;
        countries.sort_by(|a, b| a.code.cmp(&b.code));
        let mut regions = file.regions;
        regions.sort_by_key(|r| r.itu);
        Loaded {
            groups,
            plans,
            countries,
            regions,
        }
    }
}

/// True for an entry that names its country and gives its ITU region.
fn defines(c: &CountryEntry) -> bool {
    !c.name.trim().is_empty() && c.itu.is_some()
}

/// The file of a CSV plan text: one group with the country and region entries that its header lists. The id and the name
/// default to `stem`.
fn csv_file(stem: &str, text: &str) -> Result<PlanFile, PlanError> {
    view_file(crate::io::from_csv(text, stem, stem)?)
}

/// One plan of the plan view as a plan file, after the checks of [`validate`].
fn view_file(mut plan: ChannelPlan) -> Result<PlanFile, PlanError> {
    plan.countries = plan
        .countries
        .iter()
        .map(|c| c.trim().to_ascii_uppercase())
        .collect();
    validate(&plan)?;
    Ok(user_plan_file(&plan))
}

/// One plan of the plan view as a plan file: its group, and a country or region entry for each country and ITU region it
/// lists.
fn user_plan_file(plan: &ChannelPlan) -> PlanFile {
    let mut file = PlanFile {
        format: data::FORMAT_VERSION,
        groups: vec![group_of(plan)],
        ..PlanFile::default()
    };
    for cc in &plan.countries {
        file.countries.push(CountryEntry {
            code: cc.clone(),
            groups: vec![plan.id.clone()],
            unverified_reason: "made by the user".into(),
            ..CountryEntry::default()
        });
    }
    for itu in &plan.itu_regions {
        file.regions.push(RegionEntry {
            itu: *itu,
            name: format!("ITU Region {itu}"),
            groups: vec![plan.id.clone()],
            ..RegionEntry::default()
        });
    }
    file
}

/// Joins checked plan files into one: the groups in file order; for each country, the entry that names it and lists the
/// most groups (the first such entry on a tie), with the other entries of the country added to it in file order; for
/// each ITU region, the entries added in file order.
fn join(files: Vec<(String, PlanFile)>) -> PlanFile {
    let mut out = PlanFile {
        format: data::FORMAT_VERSION,
        ..PlanFile::default()
    };
    let mut all: Vec<CountryEntry> = Vec::new();
    for (_, f) in files {
        out.groups.extend(f.groups);
        all.extend(f.countries);
        for r in f.regions {
            match out.regions.iter_mut().find(|o| o.itu == r.itu) {
                Some(o) => merge_region(o, r),
                None => out.regions.push(r),
            }
        }
    }
    // The main entry of each country: it names the country, and of those it lists the most groups.
    let mut main: Vec<usize> = Vec::new();
    for (i, c) in all.iter().enumerate().filter(|(_, c)| defines(c)) {
        match main.iter_mut().find(|m| all[**m].code == c.code) {
            Some(m) if c.groups.len() > all[*m].groups.len() => *m = i,
            Some(_) => {}
            None => main.push(i),
        }
    }
    out.countries = main.iter().map(|&i| all[i].clone()).collect();
    for (i, c) in all.into_iter().enumerate() {
        if main.contains(&i) {
            continue;
        }
        match out.countries.iter_mut().find(|o| o.code == c.code) {
            Some(o) => merge_country(o, c),
            None => out.countries.push(c),
        }
    }
    out
}

/// Reads plan files given as (file name, text), in name order, without checking them (see [`Plans::from_files`] and
/// [`Plans::from_parsed`]).
///
/// # Errors
///
/// [`PlanError::Invalid`] for a file that does not have its format, or the [`PlanError`] of a CSV file that cannot be read.
pub fn parse_files(files: &[(String, String)]) -> Result<Vec<(String, PlanFile)>, PlanError> {
    let mut sorted: Vec<&(String, String)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut parsed = Vec::with_capacity(sorted.len());
    for (name, text) in sorted {
        match name
            .strip_suffix(".csv")
            .or_else(|| name.strip_suffix(".CSV"))
        {
            Some(stem) => {
                parsed.push((
                    name.clone(),
                    csv_file(stem.rsplit('/').next().unwrap_or(stem), text)?,
                ));
            }
            None if crate::api::is_plan_file(text) => {
                let file =
                    data::parse(name, text).map_err(|e| PlanError::Invalid(vec![e.to_string()]))?;
                parsed.push((name.clone(), file));
            }
            None => {
                for plan in crate::io::from_json(text)? {
                    parsed.push((name.clone(), view_file(plan)?));
                }
            }
        }
    }
    Ok(parsed)
}

/// The plans: those of the loaded files and the user's.
///
/// A user group with the id of a loaded group replaces it. User country and region entries add groups (before the loaded
/// ones) and presets (over the loaded ones).
#[derive(Clone, Debug, Default)]
pub struct Plans {
    loaded: Arc<Loaded>,
    written: Vec<BandGroup>,
    user: Vec<GroupEntry>,
    user_plans: Vec<ChannelPlan>,
    user_countries: Vec<CountryEntry>,
    user_regions: Vec<RegionEntry>,
}

impl Plans {
    /// A store with no plans, no countries and no regions.
    pub fn new() -> Plans {
        Plans::default()
    }

    /// The plans of a set of plan files, each given as (file name, text): plan files of the data format
    /// (`docs/schema/plans.json`), JSON lists of plans in the plan view, and, for a name that ends in `.csv`, CSV plans whose
    /// id and name default to the file name without `.csv`. The files are checked as one set, in name order. A country entry that gives no name and no ITU region
    /// adds its groups and presets to the entry of another file that names the country.
    ///
    /// # Errors
    ///
    /// [`PlanError::Invalid`] with every problem of every file, or the [`PlanError`] of a CSV file that cannot be read.
    pub fn from_files(files: &[(String, String)]) -> Result<Plans, PlanError> {
        Plans::from_parsed(parse_files(files)?)
    }

    /// As [`Plans::from_files`], for files that [`parse_files`] has read.
    ///
    /// # Errors
    ///
    /// [`PlanError::Invalid`] with every problem of every file.
    pub fn from_parsed(parsed: Vec<(String, PlanFile)>) -> Result<Plans, PlanError> {
        let known = Known {
            countries: parsed
                .iter()
                .flat_map(|(_, f)| f.countries.iter())
                .filter(|c| defines(c))
                .map(|c| c.code.clone())
                .collect(),
            ..Known::default()
        };
        let errors = data::check(&parsed, &known);
        if !errors.is_empty() {
            return Err(PlanError::data(errors));
        }
        Ok(Plans {
            loaded: Arc::new(Loaded::of(join(parsed))),
            ..Plans::default()
        })
    }

    /// The plans of every `*.json` and `*.csv` file in `folder` of the host's storage, except the files named in `except`
    /// (see [`Plans::from_files`]). A folder that does not exist gives a store with no plans.
    ///
    /// # Errors
    ///
    /// [`PlanError::Read`] if the storage cannot list the folder or read a file, else what [`Plans::from_files`] returns.
    pub fn from_storage(
        storage: &dyn sdrkit_core::Storage,
        folder: &StorePath,
        except: &[&str],
    ) -> Result<Plans, PlanError> {
        let read_error = |file: &str, error| PlanError::Read {
            file: file.to_owned(),
            error,
        };
        let entries = match storage.list(folder) {
            Ok(e) => e,
            Err(StorageError::NotFound) => return Ok(Plans::new()),
            Err(e) => return Err(read_error(folder.as_str(), e)),
        };
        let mut files = Vec::new();
        for e in entries {
            let lower = e.name.to_ascii_lowercase();
            if e.is_dir
                || !(lower.ends_with(".json") || lower.ends_with(".csv"))
                || except.contains(&e.name.as_str())
            {
                continue;
            }
            let name = format!("{}/{}", folder.as_str(), e.name);
            let path =
                StorePath::new(&name).map_err(|_| read_error(&name, StorageError::NotFound))?;
            let bytes = storage.read(&path).map_err(|err| read_error(&name, err))?;
            files.push((name, String::from_utf8_lossy(&bytes).into_owned()));
        }
        Plans::from_files(&files)
    }

    /// True when the store holds no plan.
    pub fn is_empty(&self) -> bool {
        self.all().next().is_none()
    }

    /// Every plan: the user's first, then the loaded ones that no user plan replaces.
    pub fn all(&self) -> impl Iterator<Item = &ChannelPlan> {
        self.user_plans.iter().chain(
            self.loaded
                .plans
                .iter()
                .filter(|p| !self.user_plans.iter().any(|u| u.id == p.id)),
        )
    }

    /// Every band group: the user's first, then the loaded ones that no user group replaces.
    pub fn groups(&self) -> impl Iterator<Item = &GroupEntry> {
        self.user.iter().chain(
            self.loaded
                .groups
                .iter()
                .filter(|g| !self.user.iter().any(|u| u.group.id == g.group.id)),
        )
    }

    /// Every country row of the files, the user's first. A file may hold several rows for one code.
    pub fn country_rows(&self) -> impl Iterator<Item = &CountryEntry> {
        self.user_countries.iter().chain(&self.loaded.countries)
    }

    /// Every row of an ITU region of the files, the user's first.
    pub fn region_rows(&self) -> impl Iterator<Item = &RegionEntry> {
        self.user_regions.iter().chain(&self.loaded.regions)
    }

    /// The codes of the country rows that give a name and an ITU region.
    fn defined_codes(&self) -> std::collections::BTreeSet<String> {
        self.country_rows()
            .filter(|c| defines(c))
            .map(|c| c.code.clone())
            .collect()
    }

    /// The band group with this id.
    pub fn group(&self, id: &str) -> Option<&GroupEntry> {
        self.groups().find(|g| g.group.id == id)
    }

    /// The user's plans.
    pub fn user_plans(&self) -> &[ChannelPlan] {
        &self.user_plans
    }

    /// The user's part as one plan file: groups, country entries and region entries.
    pub fn user_file(&self) -> PlanFile {
        PlanFile {
            schema: String::new(),
            format: data::FORMAT_VERSION,
            groups: self.written.clone(),
            countries: self.user_countries.clone(),
            regions: self.user_regions.clone(),
        }
    }

    /// The plan with this id.
    pub fn get(&self, id: &str) -> Option<&ChannelPlan> {
        self.all().find(|p| p.id == id)
    }

    /// The plans of one kind, user plans first.
    pub fn of_kind(&self, kind: PlanKind) -> Vec<&ChannelPlan> {
        self.all().filter(|p| p.kind == kind).collect()
    }


    /// The names of `hz` in the plans `ids`, most specific first: a channel before a segment, a narrow channel before a
    /// wide one. An id that no plan has is skipped.
    pub fn places(&self, hz: f64, ids: &[&str]) -> Vec<Place> {
        let mut out: Vec<Place> = ids
            .iter()
            .filter_map(|id| self.get(id))
            .filter_map(|p| p.place(hz))
            .collect();
        // Channels before segments; among channels, the one whose centre is nearest relative to its width.
        let rank = |p: &Place| match p.width_hz {
            Some(w) if w > 0.0 => (0, p.offset_hz.abs() / (w / 2.0), w),
            _ => (1, 0.0, f64::INFINITY),
        };
        out.sort_by(|a, b| {
            let (ka, ra, wa) = rank(a);
            let (kb, rb, wb) = rank(b);
            ka.cmp(&kb).then(ra.total_cmp(&rb)).then(wa.total_cmp(&wb))
        });
        out
    }

    /// The best single name of `hz` in the plans `ids`, for a frequency readout ("21", "PMR 3", "40 m").
    pub fn name_of(&self, hz: f64, ids: &[&str]) -> Option<Place> {
        self.places(hz, ids).into_iter().next()
    }

    /// A typed channel name ("E5", "21", "S11", "9A", "12B", "DS-13", "PMR 3", "16"), looked up in the plans `ids` in
    /// their order. Returns the place of the channel in the first plan that has it.
    pub fn parse_channel(&self, typed: &str, ids: &[&str]) -> Option<Place> {
        ids.iter().filter_map(|id| self.get(id)).find_map(|p| {
            let c = p.find(typed)?;
            p.place(c.centre_hz).map(|mut place| {
                place.name = c.name.clone();
                place.centre_hz = c.centre_hz;
                place.width_hz = Some(c.width_hz);
                place.offset_hz = 0.0;
                place
            })
        })
    }

    /// The next or previous channel of plan `id` from `hz`.
    pub fn step(
        &self,
        id: &str,
        hz: f64,
        dir: Direction,
        include_reallocated: bool,
    ) -> Option<(f64, String)> {
        self.get(id)?.step(hz, dir, include_reallocated)
    }

    /// Adds or replaces a user plan given in the plan view (from a CSV file or a JSON file). A plan with the id of a
    /// loaded plan replaces it. The plan's countries and ITU regions become user country and region entries.
    pub fn add_user(&mut self, mut plan: ChannelPlan) -> Result<(), PlanError> {
        plan.countries = plan
            .countries
            .iter()
            .map(|c| c.trim().to_ascii_uppercase())
            .collect();
        validate(&plan)?;
        let file = user_plan_file(&plan);
        self.add_file("user plan", file).map(|_| ())
    }

    /// Adds a set of plan files, each given as (file name, text), to the user's plans, all or nothing. The files are read
    /// and joined as [`Plans::from_files`] reads them, then added as one file ([`Plans::add_file`]). Returns the ids of the
    /// groups added.
    ///
    /// # Errors
    ///
    /// [`PlanError::Invalid`] with every problem, or the [`PlanError`] of a file that cannot be read.
    pub fn add_files(&mut self, files: &[(String, String)]) -> Result<Vec<String>, PlanError> {
        let parsed = parse_files(files)?;
        let mut known = Known {
            groups: self
                .groups()
                .map(|g| g.group.id.clone())
                .filter(|id| {
                    !parsed
                        .iter()
                        .any(|(_, f)| f.groups.iter().any(|g| &g.id == id))
                })
                .collect(),
            countries: self.defined_codes(),
        };
        known.countries.extend(
            parsed
                .iter()
                .flat_map(|(_, f)| f.countries.iter())
                .filter(|c| defines(c))
                .map(|c| c.code.clone()),
        );
        let errors = data::check(&parsed, &known);
        if !errors.is_empty() {
            return Err(PlanError::data(errors));
        }
        self.add_file("import", join(parsed))
    }

    /// Adds a user plan file (the format of the plan data, `docs/schema/plans.json`), all or nothing. A group with the id of
    /// a loaded group or an earlier user group replaces it. Country and region entries add their groups and presets to the
    /// user's entries. Returns the ids of the groups added.
    pub fn add_file(&mut self, name: &str, file: PlanFile) -> Result<Vec<String>, PlanError> {
        let known = Known {
            groups: self
                .groups()
                .map(|g| g.group.id.clone())
                .filter(|id| !file.groups.iter().any(|g| &g.id == id))
                .collect(),
            countries: self.defined_codes(),
        };
        let errors = data::check(&[(name.to_string(), file.clone())], &known);
        if !errors.is_empty() {
            return Err(PlanError::data(errors));
        }
        let mut next = self.clone();
        let mut ids = Vec::new();
        for g in &file.groups {
            next.written.retain(|u| u.id != g.id);
            next.written.push(g.clone());
            ids.push(g.id.clone());
        }
        if next.written.len() > MAX_USER_PLANS {
            return Err(PlanError::Invalid(vec![format!(
                "at most {MAX_USER_PLANS} user plans"
            )]));
        }
        for c in file.countries {
            match next.user_countries.iter_mut().find(|u| u.code == c.code) {
                Some(u) => merge_country(u, c),
                None => next.user_countries.push(c),
            }
        }
        for r in file.regions {
            match next.user_regions.iter_mut().find(|u| u.itu == r.itu) {
                Some(u) => merge_region(u, r),
                None => next.user_regions.push(r),
            }
        }
        next.rebuild()?;
        *self = next;
        Ok(ids)
    }

    /// Builds the plan views of the user groups again (after a change), with the bands of the groups they include and the
    /// countries and regions that list them.
    fn rebuild(&mut self) -> Result<(), PlanError> {
        let written = &self.written;
        let loaded = &self.loaded.groups;
        let find = |id: &str| -> Option<&BandGroup> {
            written
                .iter()
                .find(|w| w.id == id)
                .or_else(|| loaded.iter().find(|b| b.group.id == id).map(|b| &b.group))
        };
        let mut user = Vec::with_capacity(written.len());
        for g in written {
            let bands = bands_of(g, &find);
            let mut plan = plan_of(g, &bands, true);
            if plan.channels.len() > MAX_CHANNELS {
                return Err(PlanError::Invalid(vec![format!(
                    "{}: more than {MAX_CHANNELS} channels",
                    g.id
                )]));
            }
            let mut countries: Vec<String> = self
                .user_countries
                .iter()
                .filter(|c| c.groups.contains(&g.id))
                .map(|c| c.code.clone())
                .collect();
            if let Some(b) = loaded.iter().find(|b| b.group.id == g.id) {
                for c in &b.plan.countries {
                    if !countries.contains(c) {
                        countries.push(c.clone());
                    }
                }
            }
            plan.countries = countries;
            plan.itu_regions = self
                .user_regions
                .iter()
                .filter(|r| r.groups.contains(&g.id))
                .map(|r| r.itu)
                .collect();
            let mut group = g.clone();
            group.bands = bands;
            user.push(GroupEntry { group, plan });
        }
        self.user_plans = user.iter().map(|e| e.plan.clone()).collect();
        self.user = user;
        Ok(())
    }

    /// Deletes a user plan (a loaded plan it replaced comes back). Loaded plans cannot be deleted.
    pub fn remove_user(&mut self, id: &str) -> bool {
        let n = self.written.len();
        self.written.retain(|g| g.id != id);
        if self.written.len() == n {
            return false;
        }
        for c in &mut self.user_countries {
            c.groups.retain(|g| g != id);
        }
        for r in &mut self.user_regions {
            r.groups.retain(|g| g != id);
        }
        self.user_countries
            .retain(|c| !c.groups.is_empty() || !c.absent.is_empty() || !c.presets.is_empty());
        self.user_regions
            .retain(|r| !r.groups.is_empty() || !r.presets.is_empty());
        // Removing a group cannot make the others invalid, so the rebuild cannot fail here.
        let _ = self.rebuild();
        true
    }
}

/// Adds a region entry to an earlier entry for the same ITU region.
fn merge_region(into: &mut RegionEntry, r: RegionEntry) {
    for g in r.groups {
        if !into.groups.contains(&g) {
            into.groups.push(g);
        }
    }
    into.presets = r.presets.over(&into.presets);
}

/// Adds a country entry to an earlier entry for the same country.
fn merge_country(into: &mut CountryEntry, c: CountryEntry) {
    for g in c.groups {
        if !into.groups.contains(&g) {
            into.groups.push(g);
        }
    }
    for k in c.absent {
        if !into.absent.contains(&k) {
            into.absent.push(k);
        }
    }
    into.presets = c.presets.over(&into.presets);
    if !c.note.is_empty() {
        into.note = c.note;
    }
    into.sources.extend(c.sources);
    for sv in c.services {
        into.services.retain(|x| x.mode != sv.mode);
        into.services.push(sv);
    }
}

/// Checks a plan: its id, names, frequencies, widths, rasters and sizes. Every problem is listed.
pub fn validate(plan: &ChannelPlan) -> Result<(), PlanError> {
    let mut e = Vec::new();
    let ok_hz = |hz: f64| hz.is_finite() && hz > 0.0 && hz < 300e9;
    if plan.id.is_empty()
        || plan.id.len() > 64
        || !plan
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        e.push("the id must be 1 to 64 letters, digits, '_' or '-'".to_string());
    }
    if plan.name.trim().is_empty() {
        e.push("the plan needs a name".into());
    }
    if plan.channels.is_empty() && plan.segments.is_empty() {
        e.push("the plan has no channels and no segments".into());
    }
    if plan.channels.len() > MAX_CHANNELS {
        e.push(format!("more than {MAX_CHANNELS} channels"));
    }
    for c in &plan.countries {
        if c.len() != 2 || !c.chars().all(|ch| ch.is_ascii_uppercase()) {
            e.push(format!("country code {c:?} is not two letters"));
        }
    }
    if plan.itu_regions.iter().any(|r| !(1..=3).contains(r)) {
        e.push("ITU regions are 1, 2 or 3".into());
    }
    for c in &plan.channels {
        if c.name.trim().is_empty() {
            e.push(format!("the channel at {} Hz has no name", c.centre_hz));
        }
        if !ok_hz(c.centre_hz) {
            e.push(format!("channel {}: the centre is not a frequency", c.name));
        }
        if !(c.width_hz.is_finite() && c.width_hz > 0.0 && c.width_hz <= 100e6) {
            e.push(format!(
                "channel {}: the width must be above 0 and at most 100 MHz",
                c.name
            ));
        }
        if let Some(p) = c.pair_hz {
            if !ok_hz(p) {
                e.push(format!("channel {}: the pair is not a frequency", c.name));
            }
        }
    }
    for s in &plan.segments {
        if !(ok_hz(s.lo_hz) && ok_hz(s.hi_hz) && s.lo_hz < s.hi_hz) {
            e.push(format!(
                "segment {}: the edges must be frequencies, lower first",
                s.name
            ));
            continue;
        }
        if let Some(step) = s.step_hz {
            if !(step.is_finite() && step > 0.0) {
                e.push(format!("segment {}: the step must be above 0", s.name));
            } else if (s.hi_hz - s.lo_hz) / step > MAX_RASTER_POINTS as f64 {
                e.push(format!(
                    "segment {}: more than {MAX_RASTER_POINTS} raster points",
                    s.name
                ));
            }
        }
        if let Some(f) = s.first_hz {
            if !s.contains(f) {
                e.push(format!(
                    "segment {}: the first raster point is outside it",
                    s.name
                ));
            }
        }
    }
    if let Some(d) = plan.deemphasis_s {
        if !(d.is_finite() && (0.0..=1e-3).contains(&d)) {
            e.push("the de-emphasis must be 0 to 1 ms".into());
        }
    }
    if e.is_empty() {
        Ok(())
    } else {
        Err(PlanError::Invalid(e))
    }
}

//! Lookups over the plans and the frequency entries of a data folder: the plans that a country uses, the channels of a
//! scan, a search by name and by frequency, the labels of a span, the name of a frequency, the next channel of a plan,
//! and the user's plan file. The host names the plans of each lookup by id; [`Data::plan_ids`] lists those of a country.

use crate::data::{io_error, read_text, Data};
use crate::error::DataError;
use crate::plans::PlanSet;
use sdrkit_core::Choice as _;
use sdrkit_data_plans::api::{self, PlanFormat};
use sdrkit_data_plans::{ChannelPlan, Direction, Place, PlanKind, Plans};
use sdrkit_host_engine::Engine;
use sdrkit_svc_freqdb::{auto_labels, parse_frequency, Entry, FreqDb, Labels, Query, View};
use sdrkit_svc_scan::PlanChannels;
use std::path::Path;

/// The most results of one search.
pub const MAX_HITS: usize = 500;

/// What a search result is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HitKind {
    /// A plan.
    Plan,
    /// A channel of a plan.
    Channel,
    /// A segment of a plan.
    Segment,
    /// An entry of the frequency database.
    Entry,
}

/// One search result.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// What it is.
    pub kind: HitKind,
    /// The plan id, `<plan>/<channel>` for a channel or a segment, or the id of the entry.
    pub id: String,
    /// The name to show.
    pub name: String,
    /// The name of the plan of a channel, the kind of a plan, the stack of an entry.
    pub detail: String,
    /// The frequency, Hz: the centre of a channel, the lowest frequency of a plan.
    pub frequency_hz: Option<f64>,
    /// The width, Hz.
    pub width_hz: Option<f64>,
    /// The plan id of a plan, a channel or a segment.
    pub plan: Option<String>,
    /// How well it matches, 0 to 150. The results are in the order of the score, best first.
    pub score: f64,
}

/// Text in one spelling for matching: lower-case letters and digits only ("DVB-T" returns "dvbt").
fn fold(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// How well `q` (folded) matches any of `fields`: 100 equal, 60 a prefix, 30 inside (from three characters), 0 none.
fn score(q: &str, fields: &[&str]) -> f64 {
    if q.is_empty() {
        return 0.0;
    }
    fields
        .iter()
        .map(|f| {
            let f = fold(f);
            if f == q {
                100.0
            } else if f.starts_with(q) {
                60.0
            } else if q.len() >= 3 && f.contains(q) {
                30.0
            } else {
                0.0
            }
        })
        .fold(0.0, f64::max)
}

/// The channel name that `text` names, without a leading "ch" or "channel".
fn channel_name(text: &str) -> &str {
    let t = text.trim();
    let lower = t.to_ascii_lowercase();
    for p in ["channel", "ch"] {
        if lower.starts_with(p) {
            let rest = t[p.len()..].trim_start_matches(['.', ' ']);
            if !rest.is_empty() {
                return rest;
            }
        }
    }
    t
}

/// The hit of the channel `name` of `plan`.
fn channel_hit(plan: &ChannelPlan, name: &str, hz: f64, width_hz: f64, score: f64) -> Hit {
    Hit {
        kind: HitKind::Channel,
        id: format!("{}/{name}", plan.id),
        name: name.to_owned(),
        detail: plan.name.clone(),
        frequency_hz: Some(hz),
        width_hz: Some(width_hz),
        plan: Some(plan.id.clone()),
        score,
    }
}

/// The `.json` and `.csv` files of `dir` as (file name, text), in name order.
pub(crate) fn plan_texts(dir: &Path) -> Result<Vec<(String, String)>, DataError> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|e| io_error(dir, e))?
        .flatten()
    {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str());
        if path.is_file() && matches!(ext, Some("json" | "csv")) {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            files.push((name.to_owned(), read_text(&path)?));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// The plan files of `dir`, read once: the plan set of the presets and the store, whose files are checked as one set.
pub(crate) fn load_plans(dir: &Path) -> Result<(PlanSet, Plans), DataError> {
    let texts = plan_texts(dir)?;
    let format = |e: sdrkit_data_plans::PlanError| DataError::Format {
        path: dir.to_owned(),
        reason: e.to_string(),
    };
    let parsed = sdrkit_data_plans::registry::parse_files(&texts).map_err(format)?;
    let set = PlanSet::of(dir, &texts, &parsed)?;
    let store = Plans::from_parsed(parsed).map_err(format)?;
    Ok((set, store))
}

impl Data {
    /// Every plan of the data folder and of the user's plan file, as rows that a host picks by id.
    pub fn plans(&self) -> &Plans {
        &self.store
    }

    /// The ids of the plans of `kind` that the country `code` (ISO 3166 alpha-2, any case) uses, the first one the plan
    /// to start with. A country uses the plans that its rows list. If they list none of the kind, it uses those of
    /// its ITU region, unless a row of the country says that it has none.
    pub fn plan_ids_of(&self, kind: PlanKind, code: &str) -> Vec<String> {
        let code = code.trim().to_ascii_uppercase();
        let mut own: Vec<_> = self
            .store
            .country_rows()
            .filter(|c| c.code == code)
            .collect();
        // The user's entries come first; of the files' entries, the one that names the country and its ITU region holds
        // its defaults and comes next.
        let users = (self.store.user_file().countries.iter())
            .filter(|c| c.code == code)
            .count()
            .min(own.len());
        own[users..].sort_by_key(|c| c.name.trim().is_empty() || c.itu.is_none());
        let mut out: Vec<String> = Vec::new();
        let add = |ids: &[String], out: &mut Vec<String>| {
            for id in ids {
                if !out.contains(id) && self.store.get(id).is_some_and(|p| p.kind == kind) {
                    out.push(id.clone());
                }
            }
        };
        own.iter().for_each(|c| add(&c.groups, &mut out));
        if out.is_empty() && !own.iter().any(|c| c.absent.contains(&kind)) {
            let itu = own.iter().find_map(|c| c.itu);
            self.store
                .region_rows()
                .filter(|r| Some(r.itu) == itu)
                .for_each(|r| add(&r.groups, &mut out));
        }
        out
    }

    /// The ids of every plan that the country `code` uses, in the order of the kinds (see [`Data::plan_ids_of`]).
    pub fn plan_ids(&self, code: &str) -> Vec<String> {
        PlanKind::ALL
            .into_iter()
            .flat_map(|k| self.plan_ids_of(k, code))
            .collect()
    }

    /// The channels that a scan of each stack steps through, from the plans `plans`: for each stack that a plan
    /// names, the name and the frequency of each channel and raster point, lowest first. An id that no plan has is
    /// skipped.
    pub fn scan_channels(&self, plans: &[&str]) -> PlanChannels {
        let mut out = PlanChannels::new();
        for plan in plans.iter().filter_map(|id| self.store.get(id)) {
            if plan.modes.is_empty() {
                continue;
            }
            let points: Vec<(String, f64)> = plan
                .tune_points(false)
                .into_iter()
                .map(|hz| {
                    let name = plan.channel_at(hz, Some(0.5)).map(|c| c.name.clone());
                    (name.unwrap_or_default(), hz)
                })
                .collect();
            for m in &plan.modes {
                out.entry(m.clone())
                    .or_default()
                    .extend(points.iter().cloned());
            }
        }
        for v in out.values_mut() {
            v.sort_by(|a, b| a.1.total_cmp(&b.1));
            v.dedup_by(|a, b| (a.1 - b.1).abs() < 0.5);
        }
        out
    }

    /// Gives the scan service of `engine` the channels of [`Data::scan_channels`], and returns the number of stacks
    /// that have channels.
    pub fn load_scan_channels(&self, engine: &Engine, plans: &[&str]) -> usize {
        let channels = self.scan_channels(plans);
        let n = channels.len();
        engine.set_scan_channels(channels);
        n
    }

    /// Searches every plan, the channels and segments of the plans `plans`, and `entries` for `text`, best first, at
    /// most [`MAX_HITS`]. A plan of `plans` whose name matches returns its channels too. A frequency ("474", "14074
    /// kHz") finds the channels and segments of `plans` that hold it and the entries near it. A channel name ("21",
    /// "ch 16") finds the channels of that name in `plans`.
    pub fn search(&self, text: &str, plans: &[&str], entries: &[Entry]) -> Vec<Hit> {
        let q = fold(text);
        let mut hits: Vec<Hit> = Vec::new();
        let named: Vec<&ChannelPlan> = plans.iter().filter_map(|id| self.store.get(id)).collect();
        for plan in self.store.all() {
            let s = score(&q, &[&plan.id, &plan.name]);
            let s = s + plan
                .modes
                .iter()
                .map(|m| score(&q, &[m]))
                .fold(0.0, f64::max)
                / 2.0;
            if s <= 0.0 {
                continue;
            }
            if named.iter().any(|a| a.id == plan.id) {
                hits.extend(
                    plan.channels
                        .iter()
                        .map(|c| channel_hit(plan, &c.name, c.centre_hz, c.width_hz, s / 2.0)),
                );
            }
            hits.push(Hit {
                kind: HitKind::Plan,
                id: plan.id.clone(),
                name: plan.name.clone(),
                detail: plan.kind.key().to_owned(),
                frequency_hz: plan.span().map(|s| s.0),
                width_hz: plan.span().map(|s| s.1 - s.0),
                plan: Some(plan.id.clone()),
                score: s,
            });
        }
        if let Some(hz) = parse_frequency(text) {
            for pl in self.store.places(hz, plans) {
                let kind = if pl.width_hz.is_some() {
                    HitKind::Channel
                } else {
                    HitKind::Segment
                };
                let off = pl.width_hz.map_or(0.0, |w| pl.offset_hz.abs() / w.max(1.0));
                hits.push(Hit {
                    kind,
                    id: format!("{}/{}", pl.plan, pl.name),
                    name: pl.name.clone(),
                    detail: pl.segment.clone().unwrap_or_default(),
                    frequency_hz: Some(pl.centre_hz),
                    width_hz: pl.width_hz,
                    plan: Some(pl.plan.clone()),
                    score: 100.0 - 50.0 * off.min(1.0),
                });
            }
        }
        let name = channel_name(text);
        for plan in &named {
            if let Some(c) = plan.find(name) {
                let s = score(&fold(name), &[&c.name]).max(60.0);
                hits.push(channel_hit(plan, &c.name, c.centre_hz, c.width_hz, s));
            }
        }
        if !entries.is_empty() && !text.trim().is_empty() {
            let mut db = FreqDb::new();
            db.set_supplied(entries.to_vec());
            let query = Query {
                text: text.to_owned(),
                ..Query::default()
            };
            hits.extend(db.search(&query).into_iter().map(|h| {
                let e = h.entry;
                let near = h
                    .distance_hz
                    .map_or(0.0, |d| d.abs() / e.bandwidth_hz.unwrap_or(1e3).max(1.0));
                Hit {
                    kind: HitKind::Entry,
                    id: e.id.clone(),
                    name: e.name.clone(),
                    detail: e.stack.clone(),
                    frequency_hz: Some(e.hz),
                    width_hz: e.bandwidth_hz,
                    plan: None,
                    score: score(&q, &[&e.name]).max(50.0) - 25.0 * near.min(1.0),
                }
            }));
        }
        let mut seen = std::collections::HashSet::new();
        hits.retain(|h| seen.insert((h.kind, h.id.clone())));
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(MAX_HITS);
        hits
    }

    /// The labels of the span `view`: the names of `entries` that apply in `scope` (see [`Data::frequency_scope`]; empty
    /// for all) and the channels and segments of the plans `plans`, placed without overlap.
    pub fn labels(&self, entries: &[Entry], plans: &[&str], scope: &[String], view: &View) -> Labels {
        let mut db = FreqDb::new();
        db.set_supplied(entries.to_vec());
        let named: Vec<&ChannelPlan> = plans.iter().filter_map(|id| self.store.get(id)).collect();
        auto_labels(&db, &named, scope, view, &[])
    }

    /// Every name of `hz` in the plans `plans`, most specific first.
    pub fn places(&self, hz: f64, plans: &[&str]) -> Vec<Place> {
        self.store.places(hz, plans)
    }

    /// The name of the channel or the segment at `hz` in the plans `plans`, for a frequency readout.
    pub fn name_of(&self, hz: f64, plans: &[&str]) -> Option<Place> {
        self.store.name_of(hz, plans)
    }

    /// The channel that the typed name `typed` ("21", "9A", "ch 16") has in the first of the plans `plans` that has it.
    pub fn channel_named(&self, typed: &str, plans: &[&str]) -> Option<Place> {
        self.store.parse_channel(channel_name(typed), plans)
    }

    /// The next channel of the plan `plan` above `hz` (`up`) or below it: its frequency and its name. A reallocated
    /// channel counts with `include_reallocated`. `None` if no plan has the id or the plan has no channel there.
    pub fn step(
        &self,
        plan: &str,
        hz: f64,
        up: bool,
        include_reallocated: bool,
    ) -> Option<(f64, String)> {
        let dir = if up { Direction::Up } else { Direction::Down };
        self.store.step(plan, hz, dir, include_reallocated)
    }

    /// Sets the file that holds the user's plans, and adds its plans if it exists. A later import or removal writes
    /// the file. Returns the number of plans read.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], and [`DataError::Format`] for a file that is not a plan file.
    pub fn open_personal_plans(&mut self, file: impl AsRef<Path>) -> Result<usize, DataError> {
        let file = file.as_ref().to_owned();
        let n = match read_text(&file) {
            Ok(text) => api::import_as(&mut self.store, &text, PlanFormat::Json, "", "")
                .map_err(|e| DataError::Format {
                    path: file.clone(),
                    reason: e.to_string(),
                })?
                .len(),
            Err(DataError::Missing { .. }) => 0,
            Err(e) => return Err(e),
        };
        self.personal = Some(file);
        Ok(n)
    }

    /// Writes the user's plans to the file of [`Data::open_personal_plans`], if one is set.
    fn save_personal_plans(&self) -> Result<(), DataError> {
        let Some(file) = &self.personal else {
            return Ok(());
        };
        if let Some(dir) = file.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
        }
        std::fs::write(file, api::user_plans_json(&self.store)).map_err(|e| io_error(file, e))
    }

    /// Adds the plans of the text `text` of a plan file in `format` to the user's plans, and returns their ids. A CSV
    /// text that gives no id or name takes `id` and `name`. A plan with the id of a plan of the folder replaces it.
    /// All or nothing.
    ///
    /// # Errors
    ///
    /// [`DataError::Format`] with the path `id` for a text that is not a valid plan file, and the error of the write
    /// of the user's file.
    pub fn import_plan(
        &mut self,
        text: &str,
        format: PlanFormat,
        id: &str,
        name: &str,
    ) -> Result<Vec<String>, DataError> {
        let mut next = self.store.clone();
        let ids =
            api::import_as(&mut next, text, format, id, name).map_err(|e| DataError::Format {
                path: id.into(),
                reason: e.to_string(),
            })?;
        let before = std::mem::replace(&mut self.store, next);
        if let Err(e) = self.save_personal_plans() {
            self.store = before;
            return Err(e);
        }
        Ok(ids)
    }

    /// The plan `id` as the text of a plan file in `format` ([`PlanFormat::Auto`] is CSV). `None` if no plan has the id.
    pub fn export_plan(&self, id: &str, format: PlanFormat) -> Option<String> {
        api::export_as(&self.store, id, format)
    }

    /// Removes the user's plan `id`. A plan of the folder that it replaced is back. Returns false if the user has no
    /// plan of the id.
    ///
    /// # Errors
    ///
    /// The error of the write of the user's file.
    pub fn remove_plan(&mut self, id: &str) -> Result<bool, DataError> {
        let before = self.store.clone();
        if !self.store.remove_user(id) {
            return Ok(false);
        }
        if let Err(e) = self.save_personal_plans() {
            self.store = before;
            return Err(e);
        }
        Ok(true)
    }
}

//! The database: the entries that the host supplies, channels taken from plans, the user's own entries and the starred set, with search.

use crate::entry::{Category, Entry};
use crate::error::{DbError, Problem};
use sdrkit_data_plans::{ChannelPlan, PlanKind, Plans};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Most user entries kept.
pub const MAX_USER_ENTRIES: usize = 10_000;

/// A search.
#[derive(Clone, Debug, Default)]
pub struct Query {
    /// Words that must each start a word of the name, notes, tags, category or stack, or a frequency ("145.5", "14074 kHz").
    pub text: String,
    /// Only these categories (empty for all).
    pub categories: Vec<Category>,
    /// Only entries with all these tags.
    pub tags: Vec<String>,
    /// Only entries of this stack id.
    pub stack: String,
    /// Only entries reaching above this frequency, Hz.
    pub lo_hz: Option<f64>,
    /// Only entries starting below this frequency, Hz.
    pub hi_hz: Option<f64>,
    /// Only entries that apply in one of these tags (see [`Entry::applies`]); empty for all.
    pub scope: Vec<String>,
    /// Only starred entries.
    pub starred_only: bool,
    /// Only the user's entries.
    pub user_only: bool,
}

/// One search result.
#[derive(Clone, Copy, Debug)]
pub struct Hit<'a> {
    /// The entry.
    pub entry: &'a Entry,
    /// It is starred.
    pub starred: bool,
    /// For a frequency search, how far the entry is from it, Hz.
    pub distance_hz: Option<f64>,
}

/// The part of the database that the user made, as the service saves it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserState {
    /// The user's entries.
    pub entries: Vec<Entry>,
    /// Ids of starred entries (any kind).
    pub starred: Vec<String>,
}

/// The frequency database.
#[derive(Clone, Debug)]
pub struct FreqDb {
    plan_entries: Vec<Entry>,
    supplied: Vec<Entry>,
    user: Vec<Entry>,
    starred: BTreeSet<String>,
    next_user: u64,
    index: Index,
}

/// Where an entry is stored: the user list, the built-in list or the plan channels, and its place there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    User(u32),
    Builtin(u32),
    Plan(u32),
}

/// An index of the entries by frequency. Points are sorted by frequency. Ranges are few and kept apart.
#[derive(Clone, Debug, Default)]
struct Index {
    /// The points, by frequency.
    points: Vec<(f64, At)>,
    /// The largest half bandwidth of a point.
    max_half_bw_hz: f64,
    /// The ranges.
    ranges: Vec<At>,
}

/// Reads a frequency as typed: "145.5" and "1090" are MHz, "14074" is kHz (bare numbers from 3000), "145.5 MHz", "7074 kHz",
/// "1.2 GHz" and "77500 Hz" carry their unit.
pub fn parse_frequency(text: &str) -> Option<f64> {
    let t = text.trim().to_ascii_lowercase().replace([' ', '_'], "");
    let (num, mult) = if let Some(n) = t.strip_suffix("ghz") {
        (n, Some(1e9))
    } else if let Some(n) = t.strip_suffix("mhz").or_else(|| t.strip_suffix('m')) {
        (n, Some(1e6))
    } else if let Some(n) = t.strip_suffix("khz").or_else(|| t.strip_suffix('k')) {
        (n, Some(1e3))
    } else if let Some(n) = t.strip_suffix("hz") {
        (n, Some(1.0))
    } else {
        (t.as_str(), None)
    };
    let v: f64 = num.replace(',', ".").parse().ok()?;
    if !(v.is_finite() && v > 0.0) {
        return None;
    }
    Some(match mult {
        Some(m) => v * m,
        None if v < 3000.0 => v * 1e6,
        None if v < 3_000_000.0 => v * 1e3,
        None => v,
    })
}

fn category_of(kind: PlanKind) -> Category {
    match kind {
        PlanKind::Fm
        | PlanKind::Dab
        | PlanKind::Tv
        | PlanKind::AnalogTv
        | PlanKind::Cable
        | PlanKind::Broadcast => Category::Broadcast,
        PlanKind::Amateur => Category::Amateur,
        PlanKind::Marine => Category::Marine,
        PlanKind::Aviation => Category::Aviation,
        PlanKind::Personal => Category::Personal,
        PlanKind::Ism => Category::Ism,
        PlanKind::Other => Category::Other,
    }
}

/// The channels of a plan as entries (`p:<plan>:<channel>:<hz>`).
pub fn plan_entries(plan: &ChannelPlan) -> Vec<Entry> {
    let stack = plan.modes.first().cloned().unwrap_or_default();
    plan.channels
        .iter()
        .filter(|c| !c.reallocated)
        .map(|c| {
            let name = match plan.kind {
                PlanKind::Marine if c.name.chars().next().is_some_and(|ch| ch.is_ascii_digit()) => {
                    format!("Marine {}", c.name)
                }
                PlanKind::Tv | PlanKind::AnalogTv | PlanKind::Cable => format!("TV {}", c.name),
                PlanKind::Dab => format!("DAB {}", c.name),
                _ => c.name.clone(),
            };
            let mut e = Entry::new(
                format!("p:{}:{}:{}", plan.id, c.name, c.centre_hz.round()),
                name,
                c.centre_hz,
                category_of(plan.kind),
            );
            e.bandwidth_hz = Some(c.width_hz);
            e.stack = stack.clone();
            if stack == "general" {
                e.submode = plan
                    .segment_at(c.centre_hz)
                    .map(|s| s.modulation.clone())
                    .filter(|m| !m.is_empty())
                    .unwrap_or_else(|| "nfm".into());
            }
            e.scope = plan.scope();
            e.notes = c.note.clone();
            e.tags = vec![plan.id.clone(), "channel".into()];
            e.source = plan.source.clone();
            e
        })
        .collect()
}

/// Makes sure that an entry of the user can be stored: a name, a frequency above 0 and below 300 GHz, an upper edge above
/// it, a bandwidth above 0 and at most 100 MHz.
///
/// # Errors
///
/// [`DbError::Invalid`] with the first problem.
pub fn validate(e: &Entry) -> Result<(), DbError> {
    let ok = |hz: f64| hz.is_finite() && hz > 0.0 && hz < 300e9;
    let problem = if e.name.trim().is_empty() {
        Some(Problem::NoName)
    } else if e.name.len() > 200 || e.notes.len() > 4000 {
        Some(Problem::TooLong)
    } else if !ok(e.hz) {
        Some(Problem::BadFrequency)
    } else if e.end_hz.is_some_and(|end| !ok(end) || end <= e.hz) {
        Some(Problem::BadUpperEdge)
    } else if e
        .bandwidth_hz
        .is_some_and(|bw| !(bw.is_finite() && bw > 0.0 && bw <= 100e6))
    {
        Some(Problem::BadBandwidth)
    } else {
        None
    };
    problem.map_or(Ok(()), |p| Err(DbError::Invalid(p)))
}

impl Default for FreqDb {
    fn default() -> Self {
        FreqDb::new()
    }
}

impl FreqDb {
    /// A database with no entries.
    pub fn new() -> FreqDb {
        let mut db = FreqDb {
            plan_entries: Vec::new(),
            supplied: Vec::new(),
            user: Vec::new(),
            starred: BTreeSet::new(),
            next_user: 0,
            index: Index::default(),
        };
        db.rebuild_index();
        db
    }

    /// Replaces the supplied entries (see [`crate::supplied_entries`]): those that the user cannot change.
    pub fn set_supplied(&mut self, entries: Vec<Entry>) {
        self.supplied = entries;
        self.rebuild_index();
    }

    /// Replaces the plan channels with those of every plan of `kinds`.
    pub fn set_plan_channels(&mut self, plans: &Plans, kinds: &[PlanKind]) {
        self.plan_entries = plans
            .all()
            .filter(|p| kinds.contains(&p.kind))
            .flat_map(plan_entries)
            .collect();
        self.rebuild_index();
    }

    /// Every entry: the user's, the supplied ones, then plan channels.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.user
            .iter()
            .chain(self.supplied.iter())
            .chain(self.plan_entries.iter())
    }

    /// The user's entries.
    pub fn user_entries(&self) -> &[Entry] {
        &self.user
    }

    fn at(&self, at: At) -> Option<&Entry> {
        match at {
            At::User(i) => self.user.get(i as usize),
            At::Builtin(i) => self.supplied.get(i as usize),
            At::Plan(i) => self.plan_entries.get(i as usize),
        }
    }

    fn rebuild_index(&mut self) {
        let mut ix = Index::default();
        let lists = [
            (&self.user[..], At::User as fn(u32) -> At),
            (&self.supplied[..], At::Builtin),
            (&self.plan_entries[..], At::Plan),
        ];
        for (list, make) in lists {
            for (i, e) in list.iter().enumerate() {
                let at = make(u32::try_from(i).unwrap_or(u32::MAX));
                if e.end_hz.is_some() {
                    ix.ranges.push(at);
                } else {
                    ix.points.push((e.hz, at));
                    ix.max_half_bw_hz = ix.max_half_bw_hz.max(e.bandwidth_hz.unwrap_or(0.0) / 2.0);
                }
            }
        }
        ix.points.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.index = ix;
    }

    /// The entries that cover `hz` within `tolerance_hz`, found through the index: a binary search over the points, a walk
    /// over the points near `hz`, and a test of each range.
    fn covering(&self, hz: f64, tolerance_hz: f64) -> impl Iterator<Item = &Entry> {
        let ix = &self.index;
        let reach = tolerance_hz.max(0.0) + ix.max_half_bw_hz;
        let from = ix.points.partition_point(|(f, _)| *f < hz - reach);
        ix.points[from..]
            .iter()
            .take_while(move |(f, _)| *f <= hz + reach)
            .map(|(_, at)| *at)
            .chain(ix.ranges.iter().copied())
            .filter_map(|at| self.at(at))
            .filter(move |e| e.covers(hz, tolerance_hz))
    }

    /// The entry with this id.
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries().find(|e| e.id == id)
    }

    /// Whether an entry is starred.
    pub fn is_starred(&self, id: &str) -> bool {
        self.starred.contains(id)
    }

    /// Stars or unstars an entry; false for an unknown id.
    pub fn star(&mut self, id: &str, on: bool) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        if on {
            self.starred.insert(id.to_string());
        } else {
            self.starred.remove(id);
        }
        true
    }

    /// Searches. Results: starred first, then those whose name holds the words, then (for a frequency) nearest first, then by
    /// frequency.
    pub fn search(&self, q: &Query) -> Vec<Hit<'_>> {
        let freq = parse_frequency(&q.text);
        let words: Vec<String> = if freq.is_some() {
            Vec::new()
        } else {
            q.text
                .to_lowercase()
                .split_whitespace()
                .map(String::from)
                .collect()
        };
        let tags: Vec<String> = q.tags.iter().map(|t| t.to_lowercase()).collect();
        let mut hits: Vec<Hit> = self
            .entries()
            .filter(|e| q.categories.is_empty() || q.categories.contains(&e.category))
            .filter(|e| q.stack.is_empty() || e.stack == q.stack)
            .filter(|e| !q.user_only || e.is_user())
            .filter(|e| !q.starred_only || self.is_starred(&e.id))
            .filter(|e| q.lo_hz.is_none_or(|lo| e.upper_hz() >= lo))
            .filter(|e| q.hi_hz.is_none_or(|hi| e.hz <= hi))
            .filter(|e| e.applies(&q.scope))
            .filter(|e| {
                tags.iter()
                    .all(|t| e.tags.iter().any(|et| et.to_lowercase() == *t))
            })
            .filter(|e| {
                if words.is_empty() {
                    return true;
                }
                let have = e.words();
                words
                    .iter()
                    .all(|w| have.iter().any(|h| h.starts_with(w.as_str())))
            })
            .filter_map(|e| {
                let distance = match freq {
                    Some(f) => {
                        let tol = (f * 1e-5).max(1e3);
                        if !e.covers(f, tol) {
                            return None;
                        }
                        Some(if e.end_hz.is_some() {
                            0.0
                        } else {
                            (e.hz - f).abs()
                        })
                    }
                    None => None,
                };
                Some(Hit {
                    entry: e,
                    starred: self.is_starred(&e.id),
                    distance_hz: distance,
                })
            })
            .collect();
        // Entries whose name holds every word rank above those matched only by notes, tags or source.
        let in_name = |e: &Entry| {
            let n = e.name.to_lowercase();
            !words.is_empty() && words.iter().all(|w| n.contains(w.as_str()))
        };
        hits.sort_by(|a, b| {
            b.starred
                .cmp(&a.starred)
                .then(in_name(b.entry).cmp(&in_name(a.entry)))
                .then(
                    a.distance_hz
                        .unwrap_or(0.0)
                        .total_cmp(&b.distance_hz.unwrap_or(0.0)),
                )
                // Points before ranges at the same distance.
                .then(a.entry.end_hz.is_some().cmp(&b.entry.end_hz.is_some()))
                .then(a.entry.hz.total_cmp(&b.entry.hz))
        });
        hits
    }

    /// The entries on `hz` (within `tolerance_hz`, at least half their bandwidth) that apply in `scope` (see
    /// [`Entry::applies`]), points nearest first, then ranges. For a readout ("what is this frequency") and for detection priors.
    pub fn near(&self, hz: f64, tolerance_hz: f64, scope: &[String]) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self
            .covering(hz, tolerance_hz)
            .filter(|e| e.applies(scope))
            .collect();
        v.sort_by(|a, b| {
            a.end_hz
                .is_some()
                .cmp(&b.end_hz.is_some())
                .then((a.hz - hz).abs().total_cmp(&(b.hz - hz).abs()))
        });
        v
    }

    /// The entries that overlap `lo_hz..hi_hz` and apply in `scope`, by frequency.
    pub fn in_span(&self, lo_hz: f64, hi_hz: f64, scope: &[String]) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self
            .entries()
            .filter(|e| e.upper_hz() >= lo_hz && e.hz <= hi_hz && e.applies(scope))
            .collect();
        v.sort_by(|a, b| a.hz.total_cmp(&b.hz));
        v
    }

    /// Adds an entry of the user. It gets a new `u:` id, which is returned.
    ///
    /// # Errors
    ///
    /// [`DbError::Invalid`] or [`DbError::Full`].
    pub fn add_user(&mut self, mut e: Entry) -> Result<String, DbError> {
        validate(&e)?;
        if self.user.len() >= MAX_USER_ENTRIES {
            return Err(DbError::Full);
        }
        self.next_user += 1;
        while self
            .user
            .iter()
            .any(|u| u.id == format!("u:{}", self.next_user))
        {
            self.next_user += 1;
        }
        e.id = format!("u:{}", self.next_user);
        let id = e.id.clone();
        self.user.push(e);
        self.rebuild_index();
        Ok(id)
    }

    /// Replaces the entry of the user with the same id.
    ///
    /// # Errors
    ///
    /// [`DbError::BuiltIn`], [`DbError::Invalid`] or [`DbError::NoEntry`].
    pub fn update_user(&mut self, e: Entry) -> Result<(), DbError> {
        if !e.is_user() {
            return Err(DbError::BuiltIn { id: e.id });
        }
        validate(&e)?;
        let slot = self
            .user
            .iter_mut()
            .find(|u| u.id == e.id)
            .ok_or_else(|| DbError::NoEntry { id: e.id.clone() })?;
        *slot = e;
        self.rebuild_index();
        Ok(())
    }

    /// Deletes an entry of the user and its star.
    ///
    /// # Errors
    ///
    /// [`DbError::BuiltIn`] or [`DbError::NoEntry`].
    pub fn remove_user(&mut self, id: &str) -> Result<Entry, DbError> {
        if !id.starts_with("u:") {
            return Err(DbError::BuiltIn { id: id.into() });
        }
        let i = self
            .user
            .iter()
            .position(|u| u.id == id)
            .ok_or_else(|| DbError::NoEntry { id: id.into() })?;
        self.starred.remove(id);
        let e = self.user.remove(i);
        self.rebuild_index();
        Ok(e)
    }

    /// Whether a user entry already has this frequency (within 1 Hz) and name (any case).
    pub fn has_user_duplicate(&self, e: &Entry) -> bool {
        self.user
            .iter()
            .any(|u| (u.hz - e.hz).abs() < 1.0 && u.name.eq_ignore_ascii_case(&e.name))
    }

    /// The user's entries and stars, to save.
    pub fn user_state(&self) -> UserState {
        UserState {
            entries: self.user.clone(),
            starred: self.starred.iter().cloned().collect(),
        }
    }

    /// Restores saved user entries and stars. Entries that fail the checks are skipped and counted; unknown stars are dropped.
    pub fn load_user_state(&mut self, state: UserState) -> usize {
        self.user.clear();
        self.starred.clear();
        let mut skipped = 0;
        for e in state.entries.into_iter().take(MAX_USER_ENTRIES) {
            if !e.is_user() || validate(&e).is_err() || self.user.iter().any(|u| u.id == e.id) {
                skipped += 1;
                continue;
            }
            if let Ok(n) = e.id[2..].parse::<u64>() {
                self.next_user = self.next_user.max(n);
            }
            self.user.push(e);
        }
        self.rebuild_index();
        for s in state.starred {
            if self.get(&s).is_some() {
                self.starred.insert(s);
            }
        }
        skipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> FreqDb {
        let mut db = FreqDb::new();
        db.set_supplied(crate::data::tests::supplied());
        db.set_plan_channels(
            &sdrkit_dev_test::plans(),
            &[PlanKind::Personal, PlanKind::Marine],
        );
        db
    }

    fn scope(tags: &str) -> Vec<String> {
        tags.split(' ').map(String::from).collect()
    }

    #[test]
    fn the_index_finds_what_a_full_scan_finds() {
        let mut d = db();
        let mut wide = Entry::new("", "Wide", 433.0e6, Category::Other);
        wide.bandwidth_hz = Some(2e6);
        d.add_user(wide).unwrap();
        for hz in [
            7.074e6, 121.5e6, 156.8e6, 432.5e6, 446.1e6, 1090e6, 50.0, 3e11,
        ] {
            for tol in [0.0, 1e3, 50e3] {
                let mut fast: Vec<&str> = d.covering(hz, tol).map(|e| e.id.as_str()).collect();
                let mut all: Vec<&str> = d
                    .entries()
                    .filter(|e| e.covers(hz, tol))
                    .map(|e| e.id.as_str())
                    .collect();
                fast.sort_unstable();
                all.sort_unstable();
                assert_eq!(fast, all, "{hz} Hz, {tol} Hz");
            }
        }
    }

    #[test]
    fn frequencies_as_typed() {
        assert_eq!(parse_frequency("145.5"), Some(145.5e6));
        assert_eq!(parse_frequency("1090"), Some(1090e6));
        assert_eq!(parse_frequency("14074"), Some(14.074e6));
        assert_eq!(parse_frequency("7074 kHz"), Some(7.074e6));
        assert_eq!(parse_frequency("77.5k"), Some(77.5e3));
        assert_eq!(parse_frequency("1,2 GHz"), Some(1.2e9));
        assert_eq!(parse_frequency("ft8"), None);
        assert_eq!(parse_frequency("-3"), None);
    }

    #[test]
    fn search() {
        let db = db();
        let hits = db.search(&Query {
            text: "ft8".into(),
            ..Default::default()
        });
        assert!(hits[..12].iter().all(|h| h.entry.name.starts_with("FT8")));
        assert_eq!(hits[0].entry.hz, 1.84e6);
        let hits = db.search(&Query {
            tags: vec!["ft8".into()],
            ..Default::default()
        });
        assert_eq!(hits.len(), 12);
        let hits = db.search(&Query {
            text: "2 m fm calling".into(),
            scope: scope("DE itu1"),
            ..Default::default()
        });
        assert_eq!(
            hits.len(),
            1,
            "{:?}",
            hits.iter().map(|h| &h.entry.id).collect::<Vec<_>>()
        );
        assert_eq!(hits[0].entry.hz, 145.5e6);
        let hits = db.search(&Query {
            text: "156.8".into(),
            ..Default::default()
        });
        assert!(hits.iter().any(|h| h.entry.id == "b:marine-16"));
        assert!(hits.iter().any(|h| h.entry.name == "Marine 16"));
        let hits = db.search(&Query {
            categories: vec![Category::Personal],
            text: "pmr".into(),
            ..Default::default()
        });
        assert_eq!(hits.len(), 16);
        let hits = db.search(&Query {
            tags: vec!["DSC".into()],
            lo_hz: Some(3e6),
            hi_hz: Some(30e6),
            ..Default::default()
        });
        assert_eq!(hits.len(), 5);
    }

    #[test]
    fn near_and_span() {
        let db = db();
        let us = scope("US itu2");
        let v = db.near(146.52e6, 1e3, &us);
        assert_eq!(v[0].id, "b:call-2fm-us");
        let au = scope("AU itu3");
        let v = db.near(476.425e6, 1e3, &au);
        assert_eq!(v[0].name, "UHF 1");
        assert!(db.near(476.425e6, 1e3, &us).is_empty());
        let v = db.in_span(14.0e6, 14.35e6, &us);
        assert!(v.iter().any(|e| e.id == "b:ft8-20") && v.iter().all(|e| e.upper_hz() >= 14e6));
    }

    #[test]
    fn user_entries_and_stars() {
        let mut db = db();
        let mut e = Entry::new("", "Local repeater", 145.6e6, Category::Amateur);
        e.stack = "general".into();
        e.submode = "nfm".into();
        let id = db.add_user(e.clone()).unwrap();
        assert_eq!(id, "u:1");
        assert!(db.has_user_duplicate(&e));
        assert!(db.star(&id, true));
        assert!(db.star("b:ft8-20", true));
        assert!(!db.star("b:nope", true));
        let hits = db.search(&Query {
            starred_only: true,
            ..Default::default()
        });
        assert_eq!(hits.len(), 2);
        let state = db.user_state();
        let mut again = FreqDb::new();
        again.set_supplied(crate::data::tests::supplied());
        assert_eq!(again.load_user_state(state), 0);
        assert!(again.is_starred("u:1") && again.is_starred("b:ft8-20"));
        assert_eq!(again.add_user(e.clone()).unwrap(), "u:2");
        let mut changed = again.get("u:1").unwrap().clone();
        changed.name = "Club repeater".into();
        again.update_user(changed).unwrap();
        assert_eq!(again.get("u:1").unwrap().name, "Club repeater");
        assert!(matches!(
            again.remove_user("b:ft8-20"),
            Err(DbError::BuiltIn { .. })
        ));
        again.remove_user("u:1").unwrap();
        assert!(!again.is_starred("u:1"));
        let mut bad = e;
        bad.hz = -1.0;
        assert_eq!(
            again.add_user(bad),
            Err(DbError::Invalid(Problem::BadFrequency))
        );
    }
}

//! The memory store: memories with ids, "store current", search, sort, manual order, groups, tags, and an undo of the last
//! delete. Times come in as arguments, in seconds since 1970.

use crate::error::MemoryError;
use crate::freq::{self, UnitHint};
use crate::memory::Memory;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// The most memories a store keeps.
pub const DEFAULT_CAP: usize = 10_000;

/// What "store current" does if a memory of the same frequency and stack exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnSame {
    /// Change nothing and report the existing one, so the host can ask the user.
    Ask,
    /// Update the existing one.
    Update,
    /// Add a new one beside it.
    Add,
}

/// What "store current" did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stored {
    /// A new memory with this id.
    Added(u64),
    /// The existing memory with this id was updated.
    Updated(u64),
    /// A memory of the same channel exists (with [`OnSame::Ask`]); nothing changed.
    Exists(u64),
}

sdrkit_core::choice_enum! {
    /// A column to sort by. Starred memories always come first.
    pub enum SortKey {
        /// The manual order of the user.
        Manual = ("manual", "Manual"),
        /// Name, ignoring case.
        Name = ("name", "Name"),
        /// Frequency.
        Frequency = ("frequency", "Frequency"),
        /// Stack, then frequency.
        Stack = ("stack", "Stack"),
        /// Group, then name.
        Group = ("group", "Group"),
        /// Creation time.
        Created = ("created", "Created"),
        /// Last recall.
        LastUsed = ("last_used", "Last used"),
        /// Scan hits.
        Hits = ("hits", "Hits"),
    }
}

/// Which memories a memory scan or a list takes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum MemoryFilter {
    /// Every memory.
    All,
    /// The memories marked "include in scan".
    InScan,
    /// One group.
    Group {
        /// The group's name.
        name: String,
    },
    /// A frequency range, inclusive.
    Range {
        /// Low edge, Hz.
        lo_hz: f64,
        /// High edge, Hz.
        hi_hz: f64,
    },
    /// The starred ones.
    Starred,
}

impl MemoryFilter {
    /// The memory passes the filter.
    pub fn matches(&self, m: &Memory) -> bool {
        match self {
            MemoryFilter::All => true,
            MemoryFilter::InScan => m.scan,
            MemoryFilter::Group { name } => m.group.eq_ignore_ascii_case(name),
            MemoryFilter::Range { lo_hz, hi_hz } => m.freq_hz >= *lo_hz && m.freq_hz <= *hi_hz,
            MemoryFilter::Starred => m.starred,
        }
    }
}

#[derive(Clone, Debug)]
struct Removed {
    memories: Vec<(usize, Memory)>,
    at_s: f64,
}

/// The memories, in their stored order.
#[derive(Clone, Debug)]
pub struct MemoryStore {
    items: Vec<Memory>,
    next_id: u64,
    cap: usize,
    removed: Option<Removed>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        MemoryStore::new()
    }
}

impl MemoryStore {
    /// An empty store with the default cap.
    pub fn new() -> MemoryStore {
        MemoryStore::with_cap(DEFAULT_CAP)
    }

    /// An empty store holding at most `cap` memories.
    pub fn with_cap(cap: usize) -> MemoryStore {
        MemoryStore {
            items: Vec::new(),
            next_id: 1,
            cap: cap.max(1),
            removed: None,
        }
    }

    /// A store of loaded memories. Ids that are 0 or repeated get new ones, and memories past the cap are dropped (the count dropped
    /// is returned).
    pub fn from_memories(memories: Vec<Memory>, cap: usize) -> (MemoryStore, usize) {
        let mut s = MemoryStore::with_cap(cap);
        let total = memories.len();
        let mut seen = std::collections::BTreeSet::new();
        for mut m in memories.into_iter().take(s.cap) {
            if m.id == 0 || !seen.insert(m.id) {
                m.id = 0;
            }
            s.items.push(m);
        }
        s.next_id = s.items.iter().map(|m| m.id).max().unwrap_or(0) + 1;
        for i in 0..s.items.len() {
            if s.items[i].id == 0 {
                s.items[i].id = s.next_id;
                seen.insert(s.next_id);
                s.next_id += 1;
            }
        }
        let dropped = total - s.items.len();
        (s, dropped)
    }

    /// The cap.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// How many memories there are.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// No memories.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The memories in stored order.
    pub fn memories(&self) -> &[Memory] {
        &self.items
    }

    /// The memory with an id.
    pub fn get(&self, id: u64) -> Option<&Memory> {
        self.items.iter().find(|m| m.id == id)
    }

    /// The memory with an id, to change. The caller re-validates what it changes.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Memory> {
        self.items.iter_mut().find(|m| m.id == id)
    }

    /// Adds a memory and returns its new id. Its creation time is `now_s` when it has none, and it goes to the end of the manual
    /// order.
    pub fn insert(&mut self, mut m: Memory, now_s: f64) -> Result<u64, MemoryError> {
        m.validate()?;
        if self.items.len() >= self.cap {
            return Err(MemoryError::Full { cap: self.cap });
        }
        m.id = self.next_id;
        self.next_id += 1;
        if m.created_s <= 0.0 {
            m.created_s = now_s;
        }
        m.order = self.items.iter().map(|x| x.order + 1).max().unwrap_or(0);
        let id = m.id;
        self.items.push(m);
        Ok(id)
    }

    /// The first memory of the same channel (stack and frequency within `tolerance_hz`).
    pub fn find_same(&self, freq_hz: f64, stack: &str, tolerance_hz: f64) -> Option<u64> {
        self.items
            .iter()
            .find(|m| m.same_channel(freq_hz, stack, tolerance_hz))
            .map(|m| m.id)
    }

    /// Store current: adds `m`, or when a memory of the same channel exists (the same stack, the frequency within
    /// `tolerance_hz`), does what `on_same` says. An update replaces where and
    /// how to tune (frequency, stack, demodulator, rate, bandwidth, gains, settings) and the name when `m` has one; it keeps the user's
    /// organisation (group, tags, notes, colour, star, scan flags, hits, created time).
    pub fn store_current(
        &mut self,
        m: Memory,
        on_same: OnSame,
        now_s: f64,
        tolerance_hz: f64,
    ) -> Result<Stored, MemoryError> {
        m.validate()?;
        let existing = self.find_same(m.freq_hz, &m.stack, tolerance_hz);
        match (existing, on_same) {
            (Some(id), OnSame::Ask) => Ok(Stored::Exists(id)),
            (Some(id), OnSame::Update) => {
                let e = self.get_mut(id).ok_or(MemoryError::NoItem { id })?;
                e.freq_hz = m.freq_hz;
                e.stack = m.stack;
                e.submode = m.submode;
                e.rate_hz = m.rate_hz;
                e.bandwidth_hz = m.bandwidth_hz;
                e.gains = m.gains;
                e.settings = m.settings;
                if !m.name.trim().is_empty() {
                    e.name = m.name;
                }
                e.last_used_s = Some(now_s);
                Ok(Stored::Updated(id))
            }
            _ => self.insert(m, now_s).map(Stored::Added),
        }
    }

    /// Changes a memory through `f`; the change is refused (and undone) when the result is not valid. Its id cannot change.
    pub fn update(&mut self, id: u64, f: impl FnOnce(&mut Memory)) -> Result<(), MemoryError> {
        let e = self.get_mut(id).ok_or(MemoryError::NoItem { id })?;
        let mut changed = e.clone();
        f(&mut changed);
        changed.id = id;
        changed.validate()?;
        *e = changed;
        Ok(())
    }

    /// Deletes memories at `now_s`. [`MemoryStore::undo_remove`] can put them back. Returns how many were deleted. A later
    /// delete replaces what can be undone.
    pub fn remove(&mut self, ids: &[u64], now_s: f64) -> usize {
        let mut gone = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            if ids.contains(&self.items[i].id) {
                gone.push((i + gone.len(), self.items.remove(i)));
            } else {
                i += 1;
            }
        }
        let n = gone.len();
        if n > 0 {
            self.removed = Some(Removed {
                memories: gone,
                at_s: now_s,
            });
        }
        n
    }

    /// The last delete can still be undone at `now_s`: it was at most `window_s` seconds before.
    pub fn can_undo(&self, now_s: f64, window_s: f64) -> bool {
        self.removed
            .as_ref()
            .is_some_and(|r| now_s <= r.at_s + window_s)
    }

    /// Puts back what the last delete removed, at its old places, when that was at most `window_s` seconds before `now_s`.
    /// Returns how many came back.
    pub fn undo_remove(&mut self, now_s: f64, window_s: f64) -> usize {
        let Some(r) = self.removed.take() else {
            return 0;
        };
        if now_s > r.at_s + window_s {
            return 0;
        }
        let n = r.memories.len();
        for (at, m) in r.memories {
            let at = at.min(self.items.len());
            self.items.insert(at, m);
        }
        n
    }

    /// Notes a recall.
    pub fn mark_used(&mut self, id: u64, now_s: f64) -> Result<(), MemoryError> {
        let e = self.get_mut(id).ok_or(MemoryError::NoItem { id })?;
        e.last_used_s = Some(now_s);
        Ok(())
    }

    /// Notes a scan stop: one more hit, its time, and the tone heard (kept when the new one is empty).
    pub fn record_hit(&mut self, id: u64, at_s: f64, tone: &str) -> Result<(), MemoryError> {
        let e = self.get_mut(id).ok_or(MemoryError::NoItem { id })?;
        e.hits += 1;
        e.last_hit_s = Some(at_s);
        if !tone.trim().is_empty() {
            e.tone = tone.trim().to_string();
        }
        Ok(())
    }

    /// The groups in use, sorted, without repeats.
    pub fn groups(&self) -> Vec<String> {
        let mut g: Vec<String> = self
            .items
            .iter()
            .filter(|m| !m.group.is_empty())
            .map(|m| m.group.clone())
            .collect();
        g.sort_by_key(|a| a.to_lowercase());
        g.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        g
    }

    /// The tags in use, sorted, without repeats.
    pub fn tags(&self) -> Vec<String> {
        let mut t: Vec<String> = self.items.iter().flat_map(|m| m.tags.clone()).collect();
        t.sort_by_key(|a| a.to_lowercase());
        t.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        t
    }

    /// The memories a filter takes, in stored order.
    pub fn filter(&self, filter: &MemoryFilter) -> Vec<&Memory> {
        self.items.iter().filter(|m| filter.matches(m)).collect()
    }

    /// Searches. Every word of the query must be found in the name, notes, tags, group, stack or demodulator (ignoring case); a word that
    /// reads as a frequency (`"145.5"`, `"7100k"`) matches a memory within half a step of its last digit instead. An empty query
    /// matches everything. Results come sorted by `sort` (stars first).
    pub fn search(&self, query: &str, sort: SortKey, ascending: bool) -> Vec<&Memory> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut hits: Vec<&Memory> = self
            .items
            .iter()
            .filter(|m| words.iter().all(|w| word_matches(m, w)))
            .collect();
        sort_memories(&mut hits, sort, ascending);
        hits
    }

    /// Every memory sorted by `sort` (stars first).
    pub fn sorted(&self, sort: SortKey, ascending: bool) -> Vec<&Memory> {
        self.search("", sort, ascending)
    }

    /// Moves a memory to `index` in the manual order (clamped), renumbering the order of all.
    pub fn move_to(&mut self, id: u64, index: usize) -> Result<(), MemoryError> {
        let mut order: Vec<u64> = {
            let mut v: Vec<&Memory> = self.items.iter().collect();
            v.sort_by_key(|m| m.order);
            v.into_iter().map(|m| m.id).collect()
        };
        let from = order
            .iter()
            .position(|&x| x == id)
            .ok_or(MemoryError::NoItem { id })?;
        order.remove(from);
        order.insert(index.min(order.len()), id);
        for (pos, id) in order.into_iter().enumerate() {
            if let Some(m) = self.get_mut(id) {
                m.order = pos as u64;
            }
        }
        Ok(())
    }
}

fn word_matches(m: &Memory, w: &str) -> bool {
    let text = |s: &str| s.to_lowercase().contains(w);
    if text(&m.name)
        || text(&m.notes)
        || text(&m.group)
        || text(&m.stack)
        || text(&m.submode)
        || m.tags.iter().any(|t| text(t))
    {
        return true;
    }
    match freq::parse(w, UnitHint::Guess) {
        Some(p) => (m.freq_hz - p.hz).abs() <= p.resolution_hz / 2.0,
        None => false,
    }
}

fn sort_memories(v: &mut [&Memory], key: SortKey, ascending: bool) {
    v.sort_by(|a, b| {
        let stars = b.starred.cmp(&a.starred);
        if stars != Ordering::Equal {
            return stars;
        }
        let o = match key {
            SortKey::Manual => a.order.cmp(&b.order),
            SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortKey::Frequency => a.freq_hz.total_cmp(&b.freq_hz),
            SortKey::Stack => a.stack.cmp(&b.stack).then(a.freq_hz.total_cmp(&b.freq_hz)),
            SortKey::Group => a
                .group
                .to_lowercase()
                .cmp(&b.group.to_lowercase())
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            SortKey::Created => a.created_s.total_cmp(&b.created_s),
            SortKey::LastUsed => a
                .last_used_s
                .unwrap_or(0.0)
                .total_cmp(&b.last_used_s.unwrap_or(0.0)),
            SortKey::Hits => a.hits.cmp(&b.hits),
        };
        if ascending {
            o
        } else {
            o.reverse()
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::Value;

    fn store() -> MemoryStore {
        let mut s = MemoryStore::new();
        let mut a = Memory::new("Tower", 118.1e6, "general");
        a.group = "Airport".into();
        a.tags = vec!["atc".into()];
        s.insert(a, 100.0).unwrap();
        let mut b = Memory::new("Repeater", 145.6e6, "general");
        b.notes = "club net on Tuesdays".into();
        b.starred = true;
        s.insert(b, 200.0).unwrap();
        s.insert(Memory::new("Radio One", 98.8e6, "fm"), 300.0)
            .unwrap();
        s
    }

    const GENERAL: sdrkit_core::OwnerKey = sdrkit_core::OwnerKey::of("stack.general");

    #[test]
    fn store_current_asks_updates_or_adds() {
        let mut s = store();
        let m = Memory::new("New name", 118.1e6, "general").with_setting(
            GENERAL,
            "width",
            Value::F64(8000.0),
        );
        assert_eq!(
            s.store_current(m.clone(), OnSame::Ask, 400.0, 1.0),
            Ok(Stored::Exists(1))
        );
        assert_eq!(
            s.store_current(m.clone(), OnSame::Update, 400.0, 1.0),
            Ok(Stored::Updated(1))
        );
        let e = s.get(1).unwrap();
        assert_eq!(e.name, "New name");
        assert_eq!(e.group, "Airport", "the user's organisation stays");
        assert_eq!(e.settings[0].value, Value::F64(8000.0));
        assert_eq!(
            s.store_current(m.clone(), OnSame::Add, 400.0, 1.0),
            Ok(Stored::Added(4))
        );
        // A different stack is a different channel.
        let other = Memory::new("x", 118.1e6, "airband");
        assert_eq!(
            s.store_current(other, OnSame::Ask, 400.0, 1.0),
            Ok(Stored::Added(5))
        );
    }

    #[test]
    fn cap_is_kept() {
        let mut s = MemoryStore::with_cap(2);
        s.insert(Memory::new("a", 1e6, "fm"), 0.0).unwrap();
        s.insert(Memory::new("b", 2e6, "fm"), 0.0).unwrap();
        assert_eq!(
            s.insert(Memory::new("c", 3e6, "fm"), 0.0),
            Err(MemoryError::Full { cap: 2 })
        );
        let (loaded, dropped) = MemoryStore::from_memories(vec![Memory::default(); 5], 3);
        assert_eq!((loaded.len(), dropped), (3, 2));
        let ids: Vec<u64> = loaded.memories().iter().map(|m| m.id).collect();
        assert_eq!(ids, [1, 2, 3]);
    }

    #[test]
    fn search_by_words_and_frequency() {
        let s = store();
        let names = |v: Vec<&Memory>| v.iter().map(|m| m.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(s.search("tower", SortKey::Name, true)), ["Tower"]);
        assert_eq!(names(s.search("ATC", SortKey::Name, true)), ["Tower"]);
        assert_eq!(
            names(s.search("tuesdays", SortKey::Name, true)),
            ["Repeater"]
        );
        assert_eq!(names(s.search("airport", SortKey::Name, true)), ["Tower"]);
        assert_eq!(names(s.search("fm", SortKey::Name, true)), ["Radio One"]);
        assert_eq!(names(s.search("118.1", SortKey::Name, true)), ["Tower"]);
        assert_eq!(names(s.search("145.6", SortKey::Name, true)), ["Repeater"]);
        assert_eq!(
            names(s.search("145.65", SortKey::Name, true)),
            Vec::<String>::new()
        );
        assert_eq!(s.search("", SortKey::Name, true).len(), 3);
    }

    #[test]
    fn sorting_keeps_stars_on_top() {
        let s = store();
        let f: Vec<f64> = s
            .sorted(SortKey::Frequency, true)
            .iter()
            .map(|m| m.freq_hz)
            .collect();
        assert_eq!(f, [145.6e6, 98.8e6, 118.1e6]);
        let f: Vec<f64> = s
            .sorted(SortKey::Frequency, false)
            .iter()
            .map(|m| m.freq_hz)
            .collect();
        assert_eq!(f, [145.6e6, 118.1e6, 98.8e6]);
    }

    #[test]
    fn manual_order() {
        let mut s = store();
        s.update(2, |m| m.starred = false).unwrap();
        s.move_to(3, 0).unwrap();
        let ids: Vec<u64> = s
            .sorted(SortKey::Manual, true)
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, [3, 1, 2]);
        s.move_to(3, 99).unwrap();
        let ids: Vec<u64> = s
            .sorted(SortKey::Manual, true)
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, [1, 2, 3]);
        assert_eq!(s.move_to(42, 0), Err(MemoryError::NoItem { id: 42 }));
    }

    #[test]
    fn undo_of_a_delete_within_the_window() {
        let mut s = store();
        assert_eq!(s.remove(&[1, 3], 1000.0), 2);
        assert_eq!(s.len(), 1);
        assert!(s.can_undo(1005.0, 10.0));
        assert_eq!(s.undo_remove(1005.0, 10.0), 2);
        let ids: Vec<u64> = s.memories().iter().map(|m| m.id).collect();
        assert_eq!(ids, [1, 2, 3], "back at their places");
        s.remove(&[2], 2000.0);
        assert_eq!(s.undo_remove(2011.0, 10.0), 0);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn groups_tags_filters_hits() {
        let mut s = store();
        s.update(3, |m| m.group = "airport".into()).unwrap();
        assert_eq!(s.groups(), ["Airport"]);
        assert_eq!(s.tags(), ["atc"]);
        assert_eq!(
            s.filter(&MemoryFilter::Group {
                name: "AIRPORT".into()
            })
            .len(),
            2
        );
        assert_eq!(s.filter(&MemoryFilter::Starred).len(), 1);
        assert_eq!(
            s.filter(&MemoryFilter::Range {
                lo_hz: 100e6,
                hi_hz: 150e6
            })
            .len(),
            2
        );
        s.update(1, |m| m.scan = false).unwrap();
        assert_eq!(s.filter(&MemoryFilter::InScan).len(), 2);
        s.record_hit(2, 50.0, "CTCSS 88.5 Hz").unwrap();
        s.record_hit(2, 60.0, "").unwrap();
        let m = s.get(2).unwrap();
        assert_eq!(
            (m.hits, m.last_hit_s, m.tone.as_str()),
            (2, Some(60.0), "CTCSS 88.5 Hz")
        );
        assert!(s.update(2, |m| m.freq_hz = -1.0).is_err());
        assert_eq!(
            s.get(2).unwrap().freq_hz,
            145.6e6,
            "a bad change is refused"
        );
    }
}

//! The keyed table of things heard.

use crate::error::Error;
use crate::persist::Shift;
use crate::seen::{Seen, Stamp};
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

const NONE: u32 = u32::MAX;

struct Slot<K, R> {
    key: K,
    seen: Seen,
    favourite: bool,
    record: R,
    /// Towards the least recently heard.
    prev: u32,
    /// Towards the most recently heard.
    next: u32,
}

/// A table of things heard, keyed by their id: rows with a cap, the least recently heard row leaving first when a new one needs
/// room, favourites that never leave by themselves, and ageing.
///
/// Each row holds the caller's record `R` and a [`Seen`] (first and last heard, count) that the table keeps. Rows are ordered by
/// when they were last updated with [`ObjectTable::upsert`]. Reading or changing a record with [`ObjectTable::get_mut`] does not
/// count as hearing it.
///
/// Finding a row, adding one and evicting the oldest take constant time. [`ObjectTable::expire`] looks at every row.
///
/// ```
/// use sdrkit_util_objects::{ObjectTable, Stamp};
///
/// let mut t: ObjectTable<String, u32> = ObjectTable::new(100);
/// let up = t.upsert("VH-OQA", Stamp::at(5.0), || 0).unwrap();
/// assert!(up.new);
/// *up.row += 1;
/// t.set_favourite("VH-OQA", true);
/// t.upsert("VH-EBA", Stamp::at(6.0), || 0).unwrap();
/// assert_eq!(t.expire(400.0, 300.0), 1); // VH-EBA aged out; the favourite stays
/// assert_eq!(t.len(), 1);
/// ```
pub struct ObjectTable<K, R> {
    slots: Vec<Option<Slot<K, R>>>,
    free: Vec<u32>,
    index: HashMap<K, u32>,
    /// Least recently heard.
    head: u32,
    /// Most recently heard.
    tail: u32,
    len: usize,
    cap: usize,
}

/// What [`ObjectTable::upsert`] did.
#[derive(Debug)]
pub struct Upsert<'a, K, R> {
    /// The row's record, to fill in.
    pub row: &'a mut R,
    /// Whether the row is new (made by the `make` closure).
    pub new: bool,
    /// The row that left to make room, if one had to.
    pub evicted: Option<Evicted<K, R>>,
}

/// A row that left the table: by eviction, ageing or removal.
#[derive(Clone, Debug, PartialEq)]
pub struct Evicted<K, R> {
    /// Its key.
    pub key: K,
    /// When it was heard.
    pub seen: Seen,
    /// Its record.
    pub record: R,
}

/// One row, as iteration yields it.
#[derive(Debug)]
pub struct RowRef<'a, K, R> {
    /// Its key.
    pub key: &'a K,
    /// When it was heard.
    pub seen: &'a Seen,
    /// Whether it is a favourite.
    pub favourite: bool,
    /// Its record.
    pub record: &'a R,
}

impl<K, R> Clone for RowRef<'_, K, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K, R> Copy for RowRef<'_, K, R> {}

impl<K: Hash + Eq + Clone, R> ObjectTable<K, R> {
    /// An empty table of at most `cap` rows (at least 1).
    pub fn new(cap: usize) -> Self {
        let cap = cap.clamp(1, NONE as usize - 1);
        ObjectTable {
            slots: Vec::with_capacity(cap),
            free: Vec::with_capacity(cap),
            // Twice the cap, so removals and inserts never make the map grow or rehash into new memory.
            index: HashMap::with_capacity(cap.saturating_mul(2)),
            head: NONE,
            tail: NONE,
            len: 0,
            cap,
        }
    }

    /// The most rows kept.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Rows now.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Changes the cap (at least 1). When there are more rows than the new cap, the least recently heard rows that are not
    /// favourites leave. Favourites stay even beyond the cap. Returns how many left.
    pub fn set_cap(&mut self, cap: usize) -> usize {
        let cap = cap.clamp(1, NONE as usize - 1);
        if cap > self.cap {
            self.slots.reserve(cap - self.slots.len().min(cap));
            self.free.reserve(cap.saturating_sub(self.free.len()));
            self.index
                .reserve(cap.saturating_mul(2).saturating_sub(self.index.len()));
        }
        self.cap = cap;
        let mut gone = 0;
        while self.len > self.cap {
            match self.oldest_not_favourite() {
                Some(i) => {
                    self.take(i);
                    gone += 1;
                }
                None => break,
            }
        }
        gone
    }

    /// Hears the row `key` at `at`: updates its [`Seen`] and makes it the most recently heard, or makes a new row with `make`
    /// (taking the place of the least recently heard row that is not a favourite when the table is full).
    ///
    /// Refused with [`Error::BadTime`] when a time is not finite, and with [`Error::Full`] for a new row when every row is a
    /// favourite. Nothing changes then.
    pub fn upsert<Q>(
        &mut self,
        key: &Q,
        at: Stamp,
        make: impl FnOnce() -> R,
    ) -> Result<Upsert<'_, K, R>, Error>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ToOwned<Owned = K> + ?Sized,
    {
        if !at.is_valid() {
            return Err(Error::BadTime);
        }
        if let Some(&i) = self.index.get(key) {
            self.unlink(i);
            self.push_newest(i);
            let s = self.slot_mut(i);
            s.seen.heard(at);
            return Ok(Upsert {
                row: &mut s.record,
                new: false,
                evicted: None,
            });
        }
        let mut evicted = None;
        if self.len >= self.cap {
            let i = self.oldest_not_favourite().ok_or(Error::Full)?;
            evicted = Some(self.take(i));
        }
        let i = self.insert(key.to_owned(), Seen::new(at), false, make());
        Ok(Upsert {
            row: &mut self.slot_mut(i).record,
            new: true,
            evicted,
        })
    }

    /// The record of `key`.
    pub fn get<Q>(&self, key: &Q) -> Option<&R>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.get(key).map(|&i| &self.slot(i).record)
    }

    /// The record of `key`, to change. This does not count as hearing it.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut R>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let i = *self.index.get(key)?;
        Some(&mut self.slot_mut(i).record)
    }

    /// When `key` was heard.
    pub fn seen<Q>(&self, key: &Q) -> Option<&Seen>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.get(key).map(|&i| &self.slot(i).seen)
    }

    /// The whole row of `key`.
    pub fn row<Q>(&self, key: &Q) -> Option<RowRef<'_, K, R>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.get(key).map(|&i| self.row_at(i))
    }

    /// Whether `key` has a row.
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.index.contains_key(key)
    }

    /// Marks `key` as a favourite (it never leaves by eviction, ageing or [`ObjectTable::clear`]) or not. Returns whether the row
    /// exists.
    pub fn set_favourite<Q>(&mut self, key: &Q, on: bool) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.index.get(key) {
            Some(&i) => {
                self.slot_mut(i).favourite = on;
                true
            }
            None => false,
        }
    }

    /// Removes the row of `key`, whether a favourite or not.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<Evicted<K, R>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let i = *self.index.get(key)?;
        Some(self.take(i))
    }

    /// Forgets every row heard, except favourites (the [`CLEAR`](crate::commands::CLEAR) command).
    pub fn clear(&mut self) {
        self.retain(|r| r.favourite);
    }

    /// Forgets every row, favourites too.
    pub fn clear_all(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.index.clear();
        self.head = NONE;
        self.tail = NONE;
        self.len = 0;
    }

    /// Removes every row not heard for more than `max_age_s` seconds at signal time `now_s`, except favourites. Returns how many
    /// left.
    pub fn expire(&mut self, now_s: f64, max_age_s: f64) -> usize {
        if !now_s.is_finite() || max_age_s.is_nan() {
            return 0;
        }
        let mut n = 0;
        let mut i = self.head;
        while i != NONE {
            let s = self.slot(i);
            let next = s.next;
            if !s.favourite && now_s - s.seen.last_s > max_age_s {
                self.take(i);
                n += 1;
            }
            i = next;
        }
        n
    }

    /// Keeps only the rows for which `keep` says yes.
    pub fn retain(&mut self, mut keep: impl FnMut(RowRef<'_, K, R>) -> bool) {
        let mut i = self.head;
        while i != NONE {
            let next = self.slot(i).next;
            if !keep(self.row_at(i)) {
                self.take(i);
            }
            i = next;
        }
    }

    /// The latest last-heard time of any row.
    pub fn newest_s(&self) -> Option<f64> {
        self.iter().map(|r| r.seen.last_s).reduce(f64::max)
    }

    /// The rows, least recently heard first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = RowRef<'_, K, R>> + '_ {
        Order {
            table: self,
            front: self.head,
            back: self.tail,
            left: self.len,
        }
    }

    /// The rows, most recently heard first (as front ends list them).
    pub fn iter_newest_first(&self) -> impl Iterator<Item = RowRef<'_, K, R>> + '_ {
        self.iter().rev()
    }

    /// Every record, to change (in no set order). This does not count as hearing them.
    pub fn records_mut(&mut self) -> impl Iterator<Item = (&K, &mut R)> + '_ {
        self.slots
            .iter_mut()
            .flatten()
            .map(|s| (&s.key, &mut s.record))
    }

    /// Takes the rows of `saved` (a restored state), keeping this table's cap: rows replace what this table holds, least recently
    /// heard first, so when there are more than the cap the newest stay (and every favourite).
    pub fn adopt(&mut self, mut saved: ObjectTable<K, R>) {
        self.clear_all();
        let mut i = saved.head;
        while i != NONE {
            let next = saved.slot(i).next;
            let fav = saved.slot(i).favourite;
            let e = saved.take(i);
            self.insert(e.key, e.seen, fav, e.record);
            i = next;
        }
        let cap = self.cap;
        self.set_cap(cap);
    }

    fn slot(&self, i: u32) -> &Slot<K, R> {
        self.slots[i as usize]
            .as_ref()
            .expect("linked slots are full")
    }

    fn slot_mut(&mut self, i: u32) -> &mut Slot<K, R> {
        self.slots[i as usize]
            .as_mut()
            .expect("linked slots are full")
    }

    fn row_at(&self, i: u32) -> RowRef<'_, K, R> {
        let s = self.slot(i);
        RowRef {
            key: &s.key,
            seen: &s.seen,
            favourite: s.favourite,
            record: &s.record,
        }
    }

    fn oldest_not_favourite(&self) -> Option<u32> {
        let mut i = self.head;
        while i != NONE {
            let s = self.slot(i);
            if !s.favourite {
                return Some(i);
            }
            i = s.next;
        }
        None
    }

    fn insert(&mut self, key: K, seen: Seen, favourite: bool, record: R) -> u32 {
        let slot = Slot {
            key: key.clone(),
            seen,
            favourite,
            record,
            prev: NONE,
            next: NONE,
        };
        let i = match self.free.pop() {
            Some(i) => {
                self.slots[i as usize] = Some(slot);
                i
            }
            None => {
                self.slots.push(Some(slot));
                (self.slots.len() - 1) as u32
            }
        };
        self.index.insert(key, i);
        self.push_newest(i);
        self.len += 1;
        i
    }

    fn take(&mut self, i: u32) -> Evicted<K, R> {
        self.unlink(i);
        let s = self.slots[i as usize]
            .take()
            .expect("linked slots are full");
        self.index.remove(&s.key);
        self.free.push(i);
        self.len -= 1;
        Evicted {
            key: s.key,
            seen: s.seen,
            record: s.record,
        }
    }

    fn unlink(&mut self, i: u32) {
        let (prev, next) = {
            let s = self.slot(i);
            (s.prev, s.next)
        };
        if prev == NONE {
            self.head = next;
        } else {
            self.slot_mut(prev).next = next;
        }
        if next == NONE {
            self.tail = prev;
        } else {
            self.slot_mut(next).prev = prev;
        }
    }

    fn push_newest(&mut self, i: u32) {
        let tail = self.tail;
        {
            let s = self.slot_mut(i);
            s.prev = tail;
            s.next = NONE;
        }
        if tail == NONE {
            self.head = i;
        } else {
            self.slot_mut(tail).next = i;
        }
        self.tail = i;
    }
}

struct Order<'a, K, R> {
    table: &'a ObjectTable<K, R>,
    front: u32,
    back: u32,
    left: usize,
}

impl<'a, K: Hash + Eq + Clone, R> Iterator for Order<'a, K, R> {
    type Item = RowRef<'a, K, R>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        let i = self.front;
        self.front = self.table.slot(i).next;
        self.left -= 1;
        Some(self.table.row_at(i))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl<K: Hash + Eq + Clone, R> DoubleEndedIterator for Order<'_, K, R> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        let i = self.back;
        self.back = self.table.slot(i).prev;
        self.left -= 1;
        Some(self.table.row_at(i))
    }
}

impl<K: Hash + Eq + Clone, R> Default for ObjectTable<K, R> {
    /// A table of 100 rows.
    fn default() -> Self {
        ObjectTable::new(100)
    }
}

impl<K: Hash + Eq + Clone + std::fmt::Debug, R: std::fmt::Debug> std::fmt::Debug
    for ObjectTable<K, R>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectTable")
            .field("cap", &self.cap)
            .field("rows", &self.iter().collect::<Vec<_>>())
            .finish()
    }
}

impl<K: Hash + Eq + Clone, R: Clone> Clone for ObjectTable<K, R> {
    fn clone(&self) -> Self {
        let mut t = ObjectTable::new(self.cap);
        for r in self.iter() {
            t.insert(r.key.clone(), *r.seen, r.favourite, r.record.clone());
        }
        t
    }
}

impl<K: Hash + Eq + Clone, R: PartialEq> PartialEq for ObjectTable<K, R> {
    /// Same cap, and the same rows in the same order.
    fn eq(&self, other: &Self) -> bool {
        self.cap == other.cap
            && self.len == other.len
            && self.iter().zip(other.iter()).all(|(a, b)| {
                a.key == b.key
                    && a.seen == b.seen
                    && a.favourite == b.favourite
                    && a.record == b.record
            })
    }
}

impl<K: Hash + Eq + Clone, R: Shift> Shift for ObjectTable<K, R> {
    fn shift_times(&mut self, by_s: f64) {
        for s in self.slots.iter_mut().flatten() {
            s.seen.shift_times(by_s);
            s.record.shift_times(by_s);
        }
    }
}

#[derive(serde::Serialize)]
struct RowOut<'a, K, R> {
    key: &'a K,
    seen: &'a Seen,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    favourite: bool,
    record: &'a R,
}

#[derive(serde::Deserialize)]
#[serde(bound(deserialize = "K: DeserializeOwned, R: DeserializeOwned"))]
struct RowIn<K, R> {
    key: K,
    seen: Seen,
    #[serde(default)]
    favourite: bool,
    record: R,
}

#[derive(serde::Serialize)]
struct TableOut<'a, K, R> {
    cap: usize,
    rows: Vec<RowOut<'a, K, R>>,
}

#[derive(serde::Deserialize)]
#[serde(bound(deserialize = "K: DeserializeOwned, R: DeserializeOwned"))]
struct TableIn<K, R> {
    cap: usize,
    rows: Vec<RowIn<K, R>>,
}

/// Written as `{"cap": n, "rows": [{"key", "seen", "favourite", "record"}, ...]}`, least recently heard first, so any key type
/// works (JSON map keys must be text).
impl<K: Hash + Eq + Clone + Serialize, R: Serialize> Serialize for ObjectTable<K, R> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TableOut {
            cap: self.cap,
            rows: self
                .iter()
                .map(|r| RowOut {
                    key: r.key,
                    seen: r.seen,
                    favourite: r.favourite,
                    record: r.record,
                })
                .collect(),
        }
        .serialize(s)
    }
}

/// Reads what [`Serialize`] wrote. Rows with a time that is not finite, and repeated keys, are refused.
impl<'de, K, R> Deserialize<'de> for ObjectTable<K, R>
where
    K: Hash + Eq + Clone + DeserializeOwned,
    R: DeserializeOwned,
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let t = TableIn::<K, R>::deserialize(d)?;
        let mut out = ObjectTable::new(t.cap.max(t.rows.len()));
        for r in t.rows {
            let ok = r.seen.first_s.is_finite()
                && r.seen.last_s.is_finite()
                && r.seen.first_wall.is_none_or(f64::is_finite)
                && r.seen.last_wall.is_none_or(f64::is_finite);
            if !ok {
                return Err(D::Error::custom("a row with a time that is not finite"));
            }
            if out.index.contains_key(&r.key) {
                return Err(D::Error::custom("a key given twice"));
            }
            out.insert(r.key, r.seen, r.favourite, r.record);
        }
        out.set_cap(t.cap);
        Ok(out)
    }
}

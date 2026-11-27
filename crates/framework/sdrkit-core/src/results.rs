//! What a receiver publishes: a status, a record of fields, tables of records, series, map points, rasters and a log. Each part
//! has an id from the [`ResultsDecl`].
//!
//! The receiver writes its results on the decoding thread. The engine copies them into a snapshot about 30 times a second, so
//! writing is cheap and never shared. Every table and the log have a cap, so a busy band cannot grow memory without bound.

use crate::event::{EventDecl, EventDef};
use crate::marks::TableMarks;
use crate::publish::Field;
use crate::publish::{Declare, TableDef};
use crate::record::{DeclError, FieldDecl, FieldId, Kind, Record, RecordDecl, RecordError};
use crate::value::Value;
use serde::ser::{SerializeMap, SerializeSeq, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{BuildHasher, BuildHasherDefault, Hasher};
use std::sync::Arc;

/// The receiver's lock state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LockState {
    #[default]
    Off,
    Searching,
    Tracking,
    Locked,
}

impl LockState {
    /// The lock state of a receiver level: 0 searching, 1 a signal (a carrier, or bursts with sync), 2 or more decoding.
    ///
    /// ```
    /// use sdrkit_core::LockState;
    /// assert_eq!([0, 1, 2, 3].map(LockState::from_level), [LockState::Searching, LockState::Tracking, LockState::Locked, LockState::Locked]);
    /// ```
    pub fn from_level(level: u8) -> LockState {
        match level {
            0 => LockState::Searching,
            1 => LockState::Tracking,
            _ => LockState::Locked,
        }
    }
}

/// The common header every stack keeps up to date.
///
/// Build it with [`Status::new`] and the `with_*` methods (or change the fields of the one in [`Results`]), never as a struct
/// literal: fields may be added.
///
/// ```
/// use sdrkit_core::{LockState, Status};
/// let s = Status::new(LockState::Locked).with_snr_db(12.5).with_summary("Locked, 12 services");
/// assert_eq!(s.lock_state, LockState::Locked);
/// assert_eq!(s.snr_db, Some(12.5));
/// assert_eq!(s.offset_hz, None);
/// ```
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub lock_state: LockState,
    /// Signal to noise of what is being decoded, in dB; `None` when not measured.
    pub snr_db: Option<f32>,
    /// Level of the channel in dBFS.
    pub level_dbfs: Option<f32>,
    /// The carrier's offset from where it was expected, in Hz.
    pub offset_hz: Option<f64>,
    /// A short line for the status strip ("Locked, 12 services"); empty for none.
    pub summary: String,
}

/// A point on a map.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct MapPoint {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub label: String,
    /// Degrees from north, for a moving target.
    pub heading: Option<f32>,
    /// The table row that the point shows, for a point of a row ([`Results::link_point`]).
    #[serde(default)]
    pub row: Option<RowLink>,
}

/// One row of a table of the same results: the table and the row key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowLink {
    /// The table.
    pub table: TableId,
    /// The row key.
    pub key: String,
}

/// A small plot: points, or a line when `line` is set.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub points: Vec<[f32; 2]>,
    pub line: bool,
}

impl Series {
    /// Replaces the points with `points` and sets `line`, in place. Nothing is allocated unless the series grows. Returns
    /// true if a point, the number of points or `line` changed.
    pub fn fill(&mut self, line: bool, points: impl IntoIterator<Item = [f32; 2]>) -> bool {
        let mut changed = self.line != line;
        self.line = line;
        let mut n = 0;
        for p in points {
            match self.points.get_mut(n) {
                Some(q) if *q == p => {}
                Some(q) => {
                    *q = p;
                    changed = true;
                }
                None => {
                    self.points.push(p);
                    changed = true;
                }
            }
            n += 1;
        }
        if self.points.len() != n {
            self.points.truncate(n);
            changed = true;
        }
        changed
    }
}

/// One axis of a [`Raster`]: the value at the first cell, the step from one cell to the next, and the unit.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Axis {
    /// The value at the centre of the first cell.
    pub start: f64,
    /// The change from one cell to the next. It can be negative.
    pub step: f64,
    /// The unit, for example `"MHz"` or `"km/s"`.
    pub unit: String,
}

impl Axis {
    /// An axis that starts at `start` and moves by `step` per cell, in `unit`.
    pub fn new(start: f64, step: f64, unit: &str) -> Self {
        Axis {
            start,
            step,
            unit: unit.into(),
        }
    }
    /// The value at the centre of cell `i`.
    pub fn at(&self, i: usize) -> f64 {
        self.start + self.step * i as f64
    }
}

/// A picture that is a result: a grid of numbers with two axes, for example an ionogram (frequency against delay, in dB) or a
/// longitude-velocity map. A front end draws it with a colour map. A cell holds `f32::NAN` when it has no data.
///
/// The cells are in rows: row 0 first, and in each row column 0 first. Cell `(row, col)` is `values[row * width + col]`.
///
/// ```
/// use sdrkit_core::{Axis, Raster};
/// let mut r = Raster::new(3, 2, Axis::new(0.0, 0.05, "MHz"), Axis::new(0.0, 15.0, "km"), "dB");
/// r.set(1, 2, 7.5);
/// assert_eq!(r.get(1, 2), Some(7.5));
/// assert!(r.get(0, 0).unwrap().is_nan());
/// assert_eq!(r.get(2, 0), None);
/// ```
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Raster {
    /// Cells in a row.
    pub width: usize,
    /// Rows.
    pub height: usize,
    /// The axis along a row (from column to column).
    pub x: Axis,
    /// The axis from row to row.
    pub y: Axis,
    /// The unit of the cell values, for example `"dB"`.
    pub unit: String,
    /// The cells, `width * height` of them. They are left out of the JSON form for people, which is for logs.
    #[serde(skip)]
    pub values: Vec<f32>,
}

impl Raster {
    /// A raster of `width` by `height` cells with no data.
    pub fn new(width: usize, height: usize, x: Axis, y: Axis, unit: &str) -> Self {
        Raster {
            width,
            height,
            x,
            y,
            unit: unit.into(),
            values: vec![f32::NAN; width * height],
        }
    }
    /// The value of cell `(row, col)`. `None` when the cell lies outside the raster.
    pub fn get(&self, row: usize, col: usize) -> Option<f32> {
        (row < self.height && col < self.width)
            .then(|| self.values.get(row * self.width + col).copied())
            .flatten()
    }
    /// Writes cell `(row, col)`. A cell outside the raster is ignored.
    pub fn set(&mut self, row: usize, col: usize, v: f32) {
        if row < self.height && col < self.width {
            if let Some(c) = self.values.get_mut(row * self.width + col) {
                *c = v;
            }
        }
    }
    /// Row `row`. `None` when the row lies outside the raster.
    pub fn row(&self, row: usize) -> Option<&[f32]> {
        (row < self.height)
            .then(|| self.values.get(row * self.width..(row + 1) * self.width))
            .flatten()
    }
}

impl Clone for Raster {
    fn clone(&self) -> Self {
        Raster {
            width: self.width,
            height: self.height,
            x: self.x.clone(),
            y: self.y.clone(),
            unit: self.unit.clone(),
            values: self.values.clone(),
        }
    }
    fn clone_from(&mut self, s: &Self) {
        (self.width, self.height) = (s.width, s.height);
        self.x.clone_from(&s.x);
        self.y.clone_from(&s.y);
        self.unit.clone_from(&s.unit);
        self.values.clone_from(&s.values);
    }
}

impl Status {
    /// A status with this lock state and nothing measured.
    pub fn new(lock_state: LockState) -> Self {
        Status {
            lock_state,
            ..Status::default()
        }
    }
    /// The same status with the SNR of what is being decoded, in dB.
    pub fn with_snr_db(mut self, snr_db: f32) -> Self {
        self.snr_db = Some(snr_db);
        self
    }
    /// The same status with the channel level, in dBFS.
    pub fn with_level_dbfs(mut self, level_dbfs: f32) -> Self {
        self.level_dbfs = Some(level_dbfs);
        self
    }
    /// The same status with the carrier's offset from where it was expected, in Hz.
    pub fn with_offset_hz(mut self, offset_hz: f64) -> Self {
        self.offset_hz = Some(offset_hz);
        self
    }
    /// The same status with a short line for the status strip.
    pub fn with_summary(mut self, summary: &str) -> Self {
        self.summary = summary.into();
        self
    }
}

// `clone_from` reuses the strings and vectors already there, so the engine's snapshots and a graph's merged results allocate
// nothing once they have their size.
impl Clone for Status {
    fn clone(&self) -> Self {
        Status {
            summary: self.summary.clone(),
            ..*self
        }
    }
    fn clone_from(&mut self, s: &Self) {
        (
            self.lock_state,
            self.snr_db,
            self.level_dbfs,
            self.offset_hz,
        ) = (s.lock_state, s.snr_db, s.level_dbfs, s.offset_hz);
        self.summary.clone_from(&s.summary);
    }
}

impl Clone for Series {
    fn clone(&self) -> Self {
        Series {
            points: self.points.clone(),
            line: self.line,
        }
    }
    fn clone_from(&mut self, s: &Self) {
        self.points.clone_from(&s.points);
        self.line = s.line;
    }
}

impl Clone for MapPoint {
    fn clone(&self) -> Self {
        MapPoint {
            id: self.id.clone(),
            lat: self.lat,
            lon: self.lon,
            label: self.label.clone(),
            heading: self.heading,
            row: self.row.clone(),
        }
    }
    fn clone_from(&mut self, s: &Self) {
        self.id.clone_from(&s.id);
        self.label.clone_from(&s.label);
        self.row.clone_from(&s.row);
        (self.lat, self.lon, self.heading) = (s.lat, s.lon, s.heading);
    }
}

/// Makes `dst` hold what `src` holds, reusing the entries (and their strings and vectors) of the keys both have.
pub(crate) fn map_clone_from<V: Clone>(dst: &mut BTreeMap<String, V>, src: &BTreeMap<String, V>) {
    dst.retain(|k, _| src.contains_key(k));
    for (k, v) in src {
        match dst.get_mut(k) {
            Some(d) => d.clone_from(v),
            None => {
                dst.insert(k.clone(), v.clone());
            }
        }
    }
}

pub(crate) fn write_number(f: &mut std::fmt::Formatter<'_>, v: f64) -> std::fmt::Result {
    if !v.is_finite() {
        return write!(f, "{v}");
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return write!(f, "{}", v as i64);
    }
    let mut buf = [0u8; 48];
    let mut w = std::io::Cursor::new(&mut buf[..]);
    let _ = std::io::Write::write_fmt(&mut w, format_args!("{v:.6}"));
    let n = w.position() as usize;
    let t = std::str::from_utf8(&buf[..n]).unwrap_or("");
    let t = if t.contains('.') {
        t.trim_end_matches('0').trim_end_matches('.')
    } else {
        t
    };
    f.write_str(t)
}

/// The declaration of a series: its key in `series`, a label and the units of its two axes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SeriesDecl {
    pub key: String,
    pub label: String,
    pub x_unit: String,
    pub y_unit: String,
}

/// The declaration of a raster: its key in `rasters`, a label, and the units of its axes and its cells.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RasterDecl {
    pub key: String,
    pub label: String,
    pub x_unit: String,
    pub y_unit: String,
    pub unit: String,
}

/// The index of a table in its [`ResultsDecl`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
pub struct TableId(pub u16);

/// The index of a series in its [`ResultsDecl`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
pub struct SeriesId(pub u16);

/// The index of a raster in its [`ResultsDecl`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
pub struct RasterId(pub u16);

/// The declaration of a table: its key, a label, its columns and the most rows it keeps.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TableDecl {
    /// The key.
    pub key: Cow<'static, str>,
    /// What people see.
    pub label: Cow<'static, str>,
    /// The columns.
    pub columns: Arc<RecordDecl>,
    /// The most rows kept. The oldest row goes first.
    pub cap: usize,
    /// The column that names a row, the default order of the rows and the columns of a place on the map.
    pub marks: TableMarks,
}

/// What a receiver publishes, declared before it runs: fields, tables, series, rasters, map points and event kinds.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ResultsDecl {
    fields: Arc<RecordDecl>,
    tables: Vec<TableDecl>,
    series: Vec<SeriesDecl>,
    rasters: Vec<RasterDecl>,
    points: String,
    events: Vec<EventDecl>,
}

fn panic_twice(what: &str, key: &str) -> ! {
    panic!("the {what} {key} is declared twice")
}

impl ResultsDecl {
    /// Nothing declared.
    pub fn new() -> Self {
        Self::default()
    }
    /// The same declaration with these fields added.
    ///
    /// # Panics
    ///
    /// If a key is declared twice. A declaration is a constant list, so this is a programming error.
    pub fn with_fields(mut self, fields: &[&dyn Declare]) -> Self {
        let mut all: Vec<FieldDecl> = self.fields.fields().to_vec();
        all.extend(fields.iter().map(|f| f.decl()));
        self.fields = match RecordDecl::new(all) {
            Ok(d) => Arc::new(d),
            Err(e) => panic!("{e}"),
        };
        self
    }
    /// The same declaration with table `t` added.
    ///
    /// # Panics
    ///
    /// If the table key or a column key is declared twice.
    pub fn with_table(mut self, t: &TableDef) -> Self {
        if self.table_id(t.key).is_some() {
            panic_twice("table", t.key);
        }
        self.tables.push(t.decl());
        self
    }
    /// The same declaration with a series added: its key, a label and the units of its two axes.
    ///
    /// # Panics
    ///
    /// If the series key is declared twice.
    pub fn series(mut self, key: &str, label: &str, x_unit: &str, y_unit: &str) -> Self {
        if self.series_id(key).is_some() {
            panic_twice("series", key);
        }
        self.series.push(SeriesDecl {
            key: key.into(),
            label: label.into(),
            x_unit: x_unit.into(),
            y_unit: y_unit.into(),
        });
        self
    }
    /// The same declaration with a raster added: its key, a label, the units of its two axes and of its cells.
    ///
    /// # Panics
    ///
    /// If the raster key is declared twice.
    pub fn raster(
        mut self,
        key: &str,
        label: &str,
        x_unit: &str,
        y_unit: &str,
        unit: &str,
    ) -> Self {
        if self.raster_id(key).is_some() {
            panic_twice("raster", key);
        }
        self.rasters.push(RasterDecl {
            key: key.into(),
            label: label.into(),
            x_unit: x_unit.into(),
            y_unit: y_unit.into(),
            unit: unit.into(),
        });
        self
    }
    /// The same declaration with the series `s` added.
    ///
    /// # Panics
    ///
    /// If the series key is declared twice.
    pub fn with_series(self, s: &crate::publish::SeriesDef) -> Self {
        self.series(s.key, s.label, s.x_unit, s.y_unit)
    }
    /// The same declaration with the raster `r` added.
    ///
    /// # Panics
    ///
    /// If the raster key is declared twice.
    pub fn with_raster(self, r: &crate::publish::RasterDef) -> Self {
        self.raster(r.key, r.label, r.x_unit, r.y_unit, r.unit)
    }
    /// The same declaration with the map points named (`"Aircraft"`).
    pub fn points(mut self, label: &str) -> Self {
        label.clone_into(&mut self.points);
        self
    }
    /// The same declaration with event kind `e` added.
    ///
    /// # Panics
    ///
    /// If the kind is declared twice or a field key of the kind is declared twice.
    pub fn event(mut self, e: &EventDef) -> Self {
        if self.events.iter().any(|d| d.kind == e.kind) {
            panic_twice("event kind", e.kind);
        }
        self.events.push(e.decl());
        self
    }
    /// A declaration built at run time from these parts (a graph that merges the results of its components).
    ///
    /// # Errors
    ///
    /// [`DeclError::DuplicateKey`] if two fields, two tables, two series or two rasters share a key. [`DeclError::TooMany`] if a
    /// list is longer than an id can count.
    pub fn from_parts(
        fields: Vec<FieldDecl>,
        tables: Vec<TableDecl>,
        series: Vec<SeriesDecl>,
        rasters: Vec<RasterDecl>,
        points: &str,
        events: Vec<EventDecl>,
    ) -> Result<Self, DeclError> {
        fn unique<'a>(keys: impl Iterator<Item = &'a str>) -> Result<(), DeclError> {
            let mut seen: Vec<&str> = Vec::new();
            for k in keys {
                if seen.contains(&k) {
                    return Err(DeclError::DuplicateKey { key: k.to_owned() });
                }
                seen.push(k);
            }
            if seen.len() > usize::from(u16::MAX) {
                return Err(DeclError::TooMany { count: seen.len() });
            }
            Ok(())
        }
        unique(tables.iter().map(|t| t.key.as_ref()))?;
        unique(series.iter().map(|s| s.key.as_str()))?;
        unique(rasters.iter().map(|r| r.key.as_str()))?;
        Ok(ResultsDecl {
            fields: Arc::new(RecordDecl::new(fields)?),
            tables,
            series,
            rasters,
            points: points.to_owned(),
            events,
        })
    }
    /// The fields.
    pub fn fields(&self) -> &Arc<RecordDecl> {
        &self.fields
    }
    /// The tables, in [`TableId`] order.
    pub fn tables(&self) -> &[TableDecl] {
        &self.tables
    }
    /// The series, in [`SeriesId`] order.
    pub fn series_decls(&self) -> &[SeriesDecl] {
        &self.series
    }
    /// The rasters, in [`RasterId`] order.
    pub fn rasters(&self) -> &[RasterDecl] {
        &self.rasters
    }
    /// What the map points are. Empty if the receiver shows none.
    pub fn points_label(&self) -> &str {
        &self.points
    }
    /// The event kinds.
    pub fn events(&self) -> &[EventDecl] {
        &self.events
    }
    /// True if nothing is declared.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
            && self.tables.is_empty()
            && self.series.is_empty()
            && self.rasters.is_empty()
            && self.points.is_empty()
            && self.events.is_empty()
    }
    /// The id of field `key`. Do the lookup once and keep the id.
    pub fn field_id(&self, key: &str) -> Option<FieldId> {
        self.fields.id(key)
    }
    /// The id of table `key`. Do the lookup once and keep the id.
    pub fn table_id(&self, key: &str) -> Option<TableId> {
        position(self.tables.iter().map(|t| t.key.as_ref()), key).map(TableId)
    }
    /// The id of series `key`. Do the lookup once and keep the id.
    pub fn series_id(&self, key: &str) -> Option<SeriesId> {
        position(self.series.iter().map(|s| s.key.as_str()), key).map(SeriesId)
    }
    /// The id of raster `key`. Do the lookup once and keep the id.
    pub fn raster_id(&self, key: &str) -> Option<RasterId> {
        position(self.rasters.iter().map(|r| r.key.as_str()), key).map(RasterId)
    }
    /// The declaration of table `id`.
    pub fn table(&self, id: TableId) -> Option<&TableDecl> {
        self.tables.get(usize::from(id.0))
    }
}

fn position<'a>(mut keys: impl Iterator<Item = &'a str>, key: &str) -> Option<u16> {
    keys.position(|k| k == key)
        .and_then(|i| u16::try_from(i).ok())
}

/// No row: the end of a list of rows.
const NIL: usize = usize::MAX;

/// One row of a [`Table`]: its key, the counter of its last change, its record, and its links in the table's lists.
#[derive(Debug, PartialEq)]
struct Row {
    key: String,
    changed: u64,
    record: Record,
    /// The next older row, or [`NIL`].
    older: usize,
    /// The next newer row, or [`NIL`].
    newer: usize,
    /// The next row whose key has the same hash, or [`NIL`].
    same_hash: usize,
}

impl Clone for Row {
    fn clone(&self) -> Self {
        Row {
            key: self.key.clone(),
            changed: self.changed,
            record: self.record.clone(),
            older: self.older,
            newer: self.newer,
            same_hash: self.same_hash,
        }
    }
    fn clone_from(&mut self, s: &Self) {
        self.key.clone_from(&s.key);
        self.changed = s.changed;
        self.record.clone_from(&s.record);
        self.older = s.older;
        self.newer = s.newer;
        self.same_hash = s.same_hash;
    }
}

/// A hasher for keys that are hashes already: it passes the value on.
#[derive(Clone, Copy, Debug, Default)]
struct PassHasher(u64);

impl Hasher for PassHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = self.0.rotate_left(8) ^ u64::from(*b);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }
}

/// A table: rows of records, each with a row key (an aircraft's address, a station's frequency). It keeps at most `cap` rows,
/// and the oldest row goes first. It is written through [`Results::table_mut`]. A row is found by its key in constant time.
///
/// Each row has the change counter of its last change. The table's own counter is the counter of its last change of any row,
/// and of the last row added or removed.
#[derive(Debug)]
pub struct Table {
    columns: Arc<RecordDecl>,
    cap: usize,
    /// The rows, in no order. The links of each row give the age order.
    rows: Vec<Row>,
    oldest: usize,
    newest: usize,
    /// The hash of a key, to the newest row that has a key of that hash.
    index: HashMap<u64, usize, BuildHasherDefault<PassHasher>>,
    keys: RandomState,
    changed: u64,
    /// In tests, every key hashes to one of three values, so rows share hashes.
    #[cfg(test)]
    collide: bool,
}

/// Two tables are equal if their columns, caps, row keys and records are equal, in the same age order. The change counters do
/// not count.
impl PartialEq for Table {
    fn eq(&self, o: &Self) -> bool {
        self.columns == o.columns
            && self.cap == o.cap
            && self.rows.len() == o.rows.len()
            && self
                .rows_newest_first()
                .zip(o.rows_newest_first())
                .all(|(a, b)| a == b)
    }
}

impl Table {
    fn new(columns: Arc<RecordDecl>, cap: usize) -> Self {
        Table {
            columns,
            cap: cap.max(1),
            rows: Vec::new(),
            oldest: NIL,
            newest: NIL,
            index: HashMap::default(),
            keys: RandomState::new(),
            changed: 0,
            #[cfg(test)]
            collide: false,
        }
    }
    /// The change counter: the counter of the last change of a row, or of the last row added or removed.
    pub fn change_counter(&self) -> u64 {
        self.changed
    }
    /// The rows that changed after counter `since`, newest first. A row that was added counts. A row that was removed is not
    /// in the list, but the table's own counter shows the removal.
    pub fn rows_changed_since(&self, since: u64) -> impl Iterator<Item = (&str, &Record)> {
        self.slots_newest_first()
            .filter(move |r| r.changed > since)
            .map(|r| (r.key.as_str(), &r.record))
    }
    /// The columns.
    pub fn columns(&self) -> &Arc<RecordDecl> {
        &self.columns
    }
    /// The most rows kept.
    pub fn cap(&self) -> usize {
        self.cap
    }
    /// The row `key`.
    pub fn row(&self, key: &str) -> Option<&Record> {
        self.find(key).map(|s| &self.rows[s].record)
    }
    /// The number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    /// True if the table has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    /// The rows, newest first.
    pub fn rows_newest_first(&self) -> impl Iterator<Item = (&str, &Record)> {
        self.slots_newest_first()
            .map(|r| (r.key.as_str(), &r.record))
    }
    fn slots_newest_first(&self) -> impl Iterator<Item = &Row> {
        let mut s = self.newest;
        std::iter::from_fn(move || {
            let r = self.rows.get(s)?;
            s = r.older;
            Some(r)
        })
    }
    fn hash(&self, key: &str) -> u64 {
        #[cfg(test)]
        if self.collide {
            return u64::from(key.bytes().map(u32::from).sum::<u32>() % 3);
        }
        self.keys.hash_one(key)
    }
    /// The slot of row `key`.
    fn find(&self, key: &str) -> Option<usize> {
        let mut s = *self.index.get(&self.hash(key))?;
        while let Some(r) = self.rows.get(s) {
            if r.key == key {
                return Some(s);
            }
            s = r.same_hash;
        }
        None
    }
    /// Adds slot `s` to the index under its key.
    fn index_slot(&mut self, s: usize) {
        let h = self.hash(&self.rows[s].key);
        self.rows[s].same_hash = self.index.insert(h, s).unwrap_or(NIL);
    }
    /// Takes slot `s` out of the index, or moves its index entry to slot `to`.
    fn reindex_slot(&mut self, s: usize, to: usize) {
        let h = self.hash(&self.rows[s].key);
        let next = self.rows[s].same_hash;
        let Some(head) = self.index.get_mut(&h) else {
            return;
        };
        let replace = if to == NIL { next } else { to };
        if *head == s {
            if replace == NIL {
                self.index.remove(&h);
            } else {
                *head = replace;
            }
            return;
        }
        let mut p = *head;
        while let Some(r) = self.rows.get_mut(p) {
            if r.same_hash == s {
                r.same_hash = replace;
                return;
            }
            p = r.same_hash;
        }
    }
    /// Takes slot `s` out of the age order.
    fn unlink(&mut self, s: usize) {
        let (older, newer) = (self.rows[s].older, self.rows[s].newer);
        match self.rows.get_mut(older) {
            Some(r) => r.newer = newer,
            None => self.oldest = newer,
        }
        match self.rows.get_mut(newer) {
            Some(r) => r.older = older,
            None => self.newest = older,
        }
    }
    /// Puts slot `s`, which is out of the age order, at its newest end.
    fn link_newest(&mut self, s: usize) {
        let newest = self.newest;
        self.rows[s].older = newest;
        self.rows[s].newer = NIL;
        match self.rows.get_mut(newest) {
            Some(r) => r.newer = s,
            None => self.oldest = s,
        }
        self.newest = s;
    }
    /// Removes slot `s`. The last slot moves into its place.
    fn remove_slot(&mut self, s: usize) {
        self.reindex_slot(s, NIL);
        self.unlink(s);
        let last = self.rows.len() - 1;
        if s != last {
            self.reindex_slot(last, s);
            let (older, newer) = (self.rows[last].older, self.rows[last].newer);
            match self.rows.get_mut(older) {
                Some(r) => r.newer = s,
                None => self.oldest = s,
            }
            match self.rows.get_mut(newer) {
                Some(r) => r.older = s,
                None => self.newest = s,
            }
        }
        self.rows.swap_remove(s);
    }
}

impl Clone for Table {
    fn clone(&self) -> Self {
        Table {
            columns: Arc::clone(&self.columns),
            cap: self.cap,
            rows: self.rows.clone(),
            oldest: self.oldest,
            newest: self.newest,
            index: self.index.clone(),
            keys: self.keys.clone(),
            changed: self.changed,
            #[cfg(test)]
            collide: self.collide,
        }
    }
    fn clone_from(&mut self, s: &Self) {
        if !Arc::ptr_eq(&self.columns, &s.columns) {
            self.columns = Arc::clone(&s.columns);
        }
        self.cap = s.cap;
        self.rows.clone_from(&s.rows);
        self.oldest = s.oldest;
        self.newest = s.newest;
        self.index.clone_from(&s.index);
        self.keys.clone_from(&s.keys);
        self.changed = s.changed;
        #[cfg(test)]
        {
            self.collide = s.collide;
        }
    }
}

/// A table serialises as a list of `[row key, record]` pairs, newest first.
impl Serialize for Table {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut q = s.serialize_seq(Some(self.rows.len()))?;
        for row in self.rows_newest_first() {
            q.serialize_element(&row)?;
        }
        q.end()
    }
}

/// A table of a [`Results`], to write. Every change moves the change counter of the results.
#[derive(Debug)]
pub struct TableMut<'a> {
    table: &'a mut Table,
    clock: &'a mut u64,
}

impl<'a> TableMut<'a> {
    fn tick(&mut self) -> u64 {
        *self.clock += 1;
        self.table.changed = *self.clock;
        *self.clock
    }
    /// The row `key`, made the newest, to write. If the row is new, it has no values. If the table is full, the oldest row
    /// goes, and its storage is used again.
    pub fn row_mut(&mut self, key: &str) -> RowMut<'_> {
        TableMut {
            table: &mut *self.table,
            clock: &mut *self.clock,
        }
        .into_row(key)
    }
    /// The row `key`, as [`TableMut::row_mut`] returns it, for the whole life of the table borrow.
    pub(crate) fn into_row(mut self, key: &str) -> RowMut<'a> {
        let s = if let Some(s) = self.table.find(key) {
            if s != self.table.newest {
                self.table.unlink(s);
                self.table.link_newest(s);
            }
            s
        } else {
            let now = self.tick();
            let t = &mut *self.table;
            let s = if t.rows.len() >= t.cap && t.oldest != NIL {
                let s = t.oldest;
                t.reindex_slot(s, NIL);
                t.unlink(s);
                let r = &mut t.rows[s];
                r.key.clear();
                r.key.push_str(key);
                r.record.clear();
                r.changed = now;
                s
            } else {
                t.rows.push(Row {
                    key: key.to_owned(),
                    changed: now,
                    record: Record::new(Arc::clone(&t.columns)),
                    older: NIL,
                    newer: NIL,
                    same_hash: NIL,
                });
                t.rows.len() - 1
            };
            t.index_slot(s);
            t.link_newest(s);
            s
        };
        let t = self.table;
        let r = &mut t.rows[s];
        RowMut {
            record: &mut r.record,
            row: &mut r.changed,
            table: Some(&mut t.changed),
            clock: self.clock,
        }
    }
    /// Removes row `key`.
    pub fn remove(&mut self, key: &str) {
        if let Some(s) = self.table.find(key) {
            self.table.remove_slot(s);
            self.tick();
        }
    }
    /// Removes every row and keeps the columns.
    pub fn clear(&mut self) {
        if !self.table.rows.is_empty() {
            let t = &mut *self.table;
            t.rows.clear();
            t.index.clear();
            t.oldest = NIL;
            t.newest = NIL;
            self.tick();
        }
    }
    /// Makes the table hold the rows of `src`, which has the same columns. Only the rows that differ get a new counter.
    /// Nothing is allocated once the table has its size.
    pub fn copy_from(&mut self, src: &Table) {
        let t = &mut *self.table;
        let mut changed =
            t.rows.len() != src.rows.len() || t.oldest != src.oldest || t.newest != src.newest;
        t.rows.truncate(src.rows.len());
        let mut now = None;
        let clock = &mut *self.clock;
        let mut stamp = || {
            *now.get_or_insert_with(|| {
                *clock += 1;
                *clock
            })
        };
        for (d, s) in t.rows.iter_mut().zip(&src.rows) {
            if d.older != s.older || d.newer != s.newer {
                changed = true;
            }
            d.older = s.older;
            d.newer = s.newer;
            d.same_hash = s.same_hash;
            if d.key != s.key || d.record != s.record {
                d.key.clone_from(&s.key);
                d.record.clone_from(&s.record);
                d.changed = stamp();
                changed = true;
            }
        }
        let n = t.rows.len();
        for s in src.rows.iter().skip(n) {
            let mut r = s.clone();
            r.changed = stamp();
            t.rows.push(r);
        }
        t.oldest = src.oldest;
        t.newest = src.newest;
        t.index.clone_from(&src.index);
        t.keys.clone_from(&src.keys);
        if changed {
            t.changed = stamp();
        }
    }
}

impl std::ops::Deref for TableMut<'_> {
    type Target = Table;
    fn deref(&self) -> &Table {
        self.table
    }
}

/// A record of a [`Results`] (its fields or one row of a table), to write. A write that changes a slot moves the change
/// counter of the results, and of the row and the table.
#[derive(Debug)]
pub struct RowMut<'a> {
    record: &'a mut Record,
    row: &'a mut u64,
    table: Option<&'a mut u64>,
    clock: &'a mut u64,
}

impl RowMut<'_> {
    fn touch(&mut self, changed: bool) -> bool {
        if changed {
            *self.clock += 1;
            *self.row = *self.clock;
            if let Some(t) = self.table.as_deref_mut() {
                *t = *self.clock;
            }
        }
        changed
    }
    /// Writes field `f`, as [`Record::set`] does.
    pub fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) -> bool {
        let c = self.record.set(f, v);
        self.touch(c)
    }
    /// Marks field `f` as having no value now, as [`Record::none`] does.
    pub fn none<K: Kind>(&mut self, f: &Field<K>) -> bool {
        let c = self.record.none(f);
        self.touch(c)
    }
    /// Writes the catalogue entry `index` into field `f`, as [`Record::set_entry`] does.
    pub fn set_entry(&mut self, f: &crate::publish::CatalogueField, index: u16) -> bool {
        let c = self.record.set_entry(f, index);
        self.touch(c)
    }
    /// Writes field `f` if `v` is `Some`, else marks it as having no value.
    pub fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) -> bool {
        let c = self.record.opt(f, v);
        self.touch(c)
    }
    /// Writes field `id` from a value, as [`Record::set_value`] does.
    ///
    /// # Errors
    ///
    /// What [`Record::set_value`] returns. Nothing changes.
    pub fn set_value(&mut self, id: FieldId, v: &Value) -> Result<bool, RecordError> {
        let c = self.record.set_value(id, v)?;
        Ok(self.touch(c))
    }
    /// Sets every slot to [`Value::None`].
    pub fn clear(&mut self) -> bool {
        let c = self.record.clear();
        self.touch(c)
    }
}

impl std::ops::Deref for RowMut<'_> {
    type Target = Record;
    fn deref(&self) -> &Record {
        self.record
    }
}

/// Everything a receiver publishes.
///
/// The results have a change counter: a plain integer that goes up on every change made through the methods of the results.
/// A write that changes nothing does not move it. The fields, each table, each row, each series and each raster keep the
/// counter of their last change, so a host can request the changes since the counter value it holds.
#[derive(Debug)]
pub struct Results {
    decl: Arc<ResultsDecl>,
    status: Status,
    fields: Record,
    tables: Vec<Table>,
    series: Vec<Series>,
    rasters: Vec<Raster>,
    points: BTreeMap<String, MapPoint>,
    log: VecDeque<String>,
    log_cap: usize,
    /// The summary that [`Results::lost_signal`] writes; empty for none.
    searching: String,
    /// The counter of the last change.
    clock: u64,
    fields_changed: u64,
    series_changed: Vec<u64>,
    rasters_changed: Vec<u64>,
}

/// Two results are equal if their declarations and contents are equal. The change counters do not count.
impl PartialEq for Results {
    fn eq(&self, o: &Self) -> bool {
        self.decl == o.decl
            && self.status == o.status
            && self.fields == o.fields
            && self.tables == o.tables
            && self.series == o.series
            && self.rasters == o.rasters
            && self.points == o.points
            && self.log == o.log
            && self.log_cap == o.log_cap
    }
}

impl Default for Results {
    fn default() -> Self {
        Results::new(Arc::default())
    }
}

impl Results {
    /// Empty results of `decl`: no field values, no rows, empty series and rasters.
    pub fn new(decl: Arc<ResultsDecl>) -> Self {
        Results {
            fields: Record::new(Arc::clone(&decl.fields)),
            tables: decl
                .tables
                .iter()
                .map(|t| Table::new(Arc::clone(&t.columns), t.cap))
                .collect(),
            series: decl.series.iter().map(|_| Series::default()).collect(),
            rasters: decl.rasters.iter().map(|_| Raster::default()).collect(),
            series_changed: vec![0; decl.series.len()],
            rasters_changed: vec![0; decl.rasters.len()],
            decl,
            status: Status::default(),
            points: BTreeMap::new(),
            log: VecDeque::new(),
            log_cap: 200,
            searching: String::new(),
            clock: 0,
            fields_changed: 0,
        }
    }
    fn tick(&mut self, changed: bool) -> bool {
        if changed {
            self.clock += 1;
        }
        changed
    }
    /// The change counter: the counter of the last change, 0 if nothing changed. A host stores the value it read and requests
    /// the changes since that value.
    pub fn change_counter(&self) -> u64 {
        self.clock
    }
    /// The counter of the last change of a field.
    pub fn fields_counter(&self) -> u64 {
        self.fields_changed
    }
    /// The tables that changed after counter `since`, in [`TableId`] order.
    pub fn tables_changed_since(&self, since: u64) -> impl Iterator<Item = TableId> + '_ {
        self.tables
            .iter()
            .enumerate()
            .filter(move |(_, t)| t.changed > since)
            .filter_map(|(i, _)| u16::try_from(i).ok().map(TableId))
    }
    /// The declaration.
    pub fn decl(&self) -> &Arc<ResultsDecl> {
        &self.decl
    }
    /// The common header.
    pub fn status(&self) -> &Status {
        &self.status
    }
    /// Changes the status through `f`, which returns true if it changed it.
    pub fn update_status(&mut self, f: impl FnOnce(&mut Status) -> bool) -> bool {
        let c = f(&mut self.status);
        self.tick(c)
    }
    /// Sets the status to `s`. Nothing is allocated once the summary has its size.
    pub fn set_status(&mut self, s: &Status) -> bool {
        let c = self.status != *s;
        if c {
            self.status.clone_from(s);
        }
        self.tick(c)
    }
    /// The declared fields.
    pub fn fields(&self) -> &Record {
        &self.fields
    }
    /// The declared fields, to write.
    pub fn fields_mut(&mut self) -> RowMut<'_> {
        RowMut {
            record: &mut self.fields,
            row: &mut self.fields_changed,
            table: None,
            clock: &mut self.clock,
        }
    }
    /// Table `id`.
    pub fn table(&self, id: TableId) -> Option<&Table> {
        self.tables.get(usize::from(id.0))
    }
    /// Table `id`, to write.
    pub fn table_mut(&mut self, id: TableId) -> Option<TableMut<'_>> {
        Some(TableMut {
            table: self.tables.get_mut(usize::from(id.0))?,
            clock: &mut self.clock,
        })
    }
    /// Series `id`.
    pub fn series(&self, id: SeriesId) -> Option<&Series> {
        self.series.get(usize::from(id.0))
    }
    /// The counter of the last change of series `id`.
    pub fn series_counter(&self, id: SeriesId) -> Option<u64> {
        self.series_changed.get(usize::from(id.0)).copied()
    }
    /// Changes series `id` through `f`, which returns true if it changed it. False if there is no series `id`.
    pub fn update_series(&mut self, id: SeriesId, f: impl FnOnce(&mut Series) -> bool) -> bool {
        let i = usize::from(id.0);
        let c = self.series.get_mut(i).is_some_and(f);
        if self.tick(c) {
            self.series_changed[i] = self.clock;
        }
        c
    }
    /// Raster `id`.
    pub fn raster(&self, id: RasterId) -> Option<&Raster> {
        self.rasters.get(usize::from(id.0))
    }
    /// The counter of the last change of raster `id`.
    pub fn raster_counter(&self, id: RasterId) -> Option<u64> {
        self.rasters_changed.get(usize::from(id.0)).copied()
    }
    /// Changes raster `id` through `f`, which returns true if it changed it. False if there is no raster `id`.
    pub fn update_raster(&mut self, id: RasterId, f: impl FnOnce(&mut Raster) -> bool) -> bool {
        let i = usize::from(id.0);
        let c = self.rasters.get_mut(i).is_some_and(f);
        if self.tick(c) {
            self.rasters_changed[i] = self.clock;
        }
        c
    }
    /// The map points, by point id.
    pub fn points(&self) -> &BTreeMap<String, MapPoint> {
        &self.points
    }
    /// Sets the map point `id`, as [`Results::update_point`] does. The row of `p` is not copied: its table is a table
    /// of the results that `p` is from ([`Results::link_point`] states the row here).
    pub fn set_point(&mut self, id: &str, p: &MapPoint) -> bool {
        self.update_point(id, p.lat, p.lon, &p.label, p.heading)
    }
    /// Sets the map point `id` to `lat`, `lon` (degrees), `label` and `heading`. Only the parts that differ are written.
    /// Nothing is allocated for a point that is there already. Returns true if something changed.
    pub fn update_point(
        &mut self,
        id: &str,
        lat: f64,
        lon: f64,
        label: &str,
        heading: Option<f32>,
    ) -> bool {
        let c = match self.points.get_mut(id) {
            Some(d) => {
                let c = d.lat != lat || d.lon != lon || d.label != label || d.heading != heading;
                if c {
                    d.lat = lat;
                    d.lon = lon;
                    d.heading = heading;
                    if d.label != label {
                        d.label.clear();
                        d.label.push_str(label);
                    }
                }
                c
            }
            None => {
                self.points.insert(
                    id.to_owned(),
                    MapPoint {
                        id: id.to_owned(),
                        lat,
                        lon,
                        label: label.to_owned(),
                        heading,
                        row: None,
                    },
                );
                true
            }
        };
        self.tick(c)
    }
    /// States that the map point `id` shows row `key` of table `table`, or no row for `None`. Nothing is allocated for
    /// a link that is there already. Returns true if something changed. False if there is no point `id`.
    pub fn link_point(&mut self, id: &str, row: Option<(TableId, &str)>) -> bool {
        let Some(p) = self.points.get_mut(id) else {
            return false;
        };
        let same = match (&p.row, row) {
            (None, None) => true,
            (Some(a), Some((table, key))) => a.table == table && a.key == key,
            _ => false,
        };
        if !same {
            p.row = row.map(|(table, key)| RowLink {
                table,
                key: key.to_owned(),
            });
        }
        self.tick(!same)
    }
    /// Removes the map point `id`.
    pub fn remove_point(&mut self, id: &str) -> bool {
        let c = self.points.remove(id).is_some();
        self.tick(c)
    }
    /// Removes every map point.
    pub fn clear_points(&mut self) -> bool {
        let c = !self.points.is_empty();
        self.points.clear();
        self.tick(c)
    }
    /// The log, oldest line first.
    pub fn log_lines(&self) -> &VecDeque<String> {
        &self.log
    }
    /// Adds a line to the log. The oldest line goes when the log holds [`Results::log_cap`] lines.
    pub fn log(&mut self, line: impl Into<String>) {
        self.log.push_back(line.into());
        while self.log.len() > self.log_cap {
            self.log.pop_front();
        }
        self.tick(true);
    }
    /// Writes `line` into the log and as the summary of the status.
    pub fn problem(&mut self, line: &str) {
        self.update_status(|s| {
            let c = s.summary != line;
            if c {
                line.clone_into(&mut s.summary);
            }
            c
        });
        self.log(line);
    }
    /// Removes every log line.
    pub fn clear_log(&mut self) -> bool {
        let c = !self.log.is_empty();
        self.log.clear();
        self.tick(c)
    }
    /// The most log lines kept.
    pub fn log_cap(&self) -> usize {
        self.log_cap
    }
    /// Keeps at most `cap` log lines (at least one). The oldest lines go.
    pub fn set_log_cap(&mut self, cap: usize) {
        self.log_cap = cap.max(1);
        let c = self.log.len() > self.log_cap;
        while self.log.len() > self.log_cap {
            self.log.pop_front();
        }
        self.tick(c);
    }
    /// The same results on `decl`, which extends the declaration of these results. The status, the fields, the points, the
    /// log and the counter stay. Tables, series and rasters start empty.
    pub fn with_decl(self, decl: Arc<ResultsDecl>) -> Results {
        let mut r = Results::new(decl);
        r.status = self.status;
        r.fields = self.fields;
        r.points = self.points;
        r.log = self.log;
        r.log_cap = self.log_cap;
        r.searching = self.searching;
        r.clock = self.clock + 1;
        r.fields_changed = self.fields_changed;
        r
    }
    /// Makes these results hold what `src` holds, part by part with the same ids: the status, the fields that both declare,
    /// and the tables, series, rasters, points and log. A part that is equal already keeps its counter. Nothing is allocated
    /// once the parts have their sizes.
    pub fn copy_from(&mut self, src: &Results) {
        self.copy_from_where(src, |_| true);
    }
    /// As [`Results::copy_from`], for a caller that knows which tables of `src` changed since the last copy: a table is
    /// compared and copied only if `table` returns true for its id. The other tables stay as they are.
    pub fn copy_from_where(&mut self, src: &Results, mut table: impl FnMut(TableId) -> bool) {
        self.set_status(&src.status);
        let mut fields = self.fields_mut();
        for (i, (_, v)) in src.fields.iter().enumerate() {
            let _ = fields.set_value(FieldId(i as u16), v);
        }
        for (i, t) in src.tables.iter().enumerate() {
            if !table(TableId(i as u16)) {
                continue;
            }
            if let Some(mut d) = self.table_mut(TableId(i as u16)) {
                if d.columns == t.columns || *d.columns == *t.columns {
                    d.copy_from(t);
                }
            }
        }
        for (i, x) in src.series.iter().enumerate() {
            self.update_series(SeriesId(i as u16), |d| copy_if_changed(d, x));
        }
        for (i, x) in src.rasters.iter().enumerate() {
            self.update_raster(RasterId(i as u16), |d| copy_if_changed(d, x));
        }
        let c = self.points != src.points;
        if c {
            map_clone_from(&mut self.points, &src.points);
        }
        self.tick(c);
        let c = self.log != src.log;
        if c {
            clone_lines(&mut self.log, &src.log);
        }
        self.tick(c);
    }
    /// Keeps what was decoded, and forgets that anything is being received. Use it after a retune. The summary becomes the
    /// text of [`Results::set_searching_summary`], empty when none was set. Nothing is allocated once the summary has its size.
    pub fn lost_signal(&mut self) {
        let Results { status, searching, .. } = self;
        let lost = status.lock_state == LockState::Searching
            && status.snr_db.is_none()
            && status.level_dbfs.is_none()
            && status.offset_hz.is_none()
            && status.summary == *searching;
        if !lost {
            status.lock_state = LockState::Searching;
            (status.snr_db, status.level_dbfs, status.offset_hz) = (None, None, None);
            status.summary.clone_from(searching);
        }
        self.tick(!lost);
    }
    /// Sets the summary that [`Results::lost_signal`] writes: what the decoder does from the next retune on, until its
    /// next report. It is no result: it changes no counter, and equality and serialization leave it out.
    pub fn set_searching_summary(&mut self, text: &str) {
        if self.searching != text {
            self.searching.clear();
            self.searching.push_str(text);
        }
    }
}

// `clone_from` reuses every string, record and vector already there, so a snapshot allocates nothing once it has its size.
impl Clone for Results {
    fn clone(&self) -> Self {
        Results {
            decl: Arc::clone(&self.decl),
            status: self.status.clone(),
            fields: self.fields.clone(),
            tables: self.tables.clone(),
            series: self.series.clone(),
            rasters: self.rasters.clone(),
            points: self.points.clone(),
            log: self.log.clone(),
            log_cap: self.log_cap,
            searching: self.searching.clone(),
            clock: self.clock,
            fields_changed: self.fields_changed,
            series_changed: self.series_changed.clone(),
            rasters_changed: self.rasters_changed.clone(),
        }
    }
    fn clone_from(&mut self, s: &Self) {
        if !Arc::ptr_eq(&self.decl, &s.decl) {
            self.decl = Arc::clone(&s.decl);
        }
        self.status.clone_from(&s.status);
        self.fields.clone_from(&s.fields);
        self.tables.clone_from(&s.tables);
        self.series.clone_from(&s.series);
        self.rasters.clone_from(&s.rasters);
        map_clone_from(&mut self.points, &s.points);
        clone_lines(&mut self.log, &s.log);
        self.log_cap = s.log_cap;
        self.searching.clone_from(&s.searching);
        self.clock = s.clock;
        self.fields_changed = s.fields_changed;
        self.series_changed.clone_from(&s.series_changed);
        self.rasters_changed.clone_from(&s.rasters_changed);
    }
}

/// Makes `dst` equal to `src` and returns true if it was not.
pub fn copy_if_changed<T: Clone + PartialEq>(dst: &mut T, src: &T) -> bool {
    let c = dst != src;
    if c {
        dst.clone_from(src);
    }
    c
}

/// Makes `dst` hold the lines of `src`, reusing its strings.
pub(crate) fn clone_lines(dst: &mut VecDeque<String>, src: &VecDeque<String>) {
    dst.truncate(src.len());
    for (d, s) in dst.iter_mut().zip(src) {
        d.clone_from(s);
    }
    let n = dst.len();
    dst.extend(src.iter().skip(n).cloned());
}

/// A list of parts serialised as a map from each declared key to its part.
struct Keyed<'a, T>(Vec<&'a str>, &'a [T]);

impl<T: Serialize> Serialize for Keyed<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.1.len()))?;
        for (k, v) in self.0.iter().zip(self.1) {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// Results serialise with every table, series and raster under its declared key.
impl Serialize for Results {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let d = &self.decl;
        let mut st = s.serialize_struct("Results", 7)?;
        st.serialize_field("status", &self.status)?;
        st.serialize_field("fields", &self.fields)?;
        let tables = Keyed(
            d.tables.iter().map(|t| t.key.as_ref()).collect(),
            &self.tables,
        );
        st.serialize_field("tables", &tables)?;
        let series = Keyed(
            d.series.iter().map(|t| t.key.as_str()).collect(),
            &self.series,
        );
        st.serialize_field("series", &series)?;
        let rasters = Keyed(
            d.rasters.iter().map(|t| t.key.as_str()).collect(),
            &self.rasters,
        );
        st.serialize_field("rasters", &rasters)?;
        st.serialize_field("points", &self.points)?;
        st.serialize_field("log", &self.log)?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::{field, Field};

    const N: Field<field::I64> = Field::new("n", "N", "");

    #[test]
    fn rows_that_share_a_hash_are_found_moved_and_removed() {
        let mut t = Table::new(Arc::new(RecordDecl::of(&[&N])), 12);
        t.collide = true;
        let mut clock = 0;
        let mut model: VecDeque<(String, i64)> = VecDeque::new();
        let mut seed = 3u64;
        for step in 0..5000i64 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let key = format!("k{}", (seed >> 33) % 20);
            let mut tm = TableMut {
                table: &mut t,
                clock: &mut clock,
            };
            let at = model.iter().position(|(k, _)| *k == key);
            if (seed >> 20).is_multiple_of(3) {
                tm.remove(&key);
                if let Some(i) = at {
                    model.remove(i);
                }
            } else {
                tm.row_mut(&key).set(&N, step);
                match at {
                    Some(i) => {
                        model.remove(i);
                    }
                    None if model.len() == 12 => {
                        model.pop_front();
                    }
                    None => {}
                }
                model.push_back((key, step));
            }
            let got: Vec<(&str, i64)> = t
                .rows_newest_first()
                .map(|(k, r)| (k, r.get(&N).unwrap_or(-1)))
                .collect();
            let want: Vec<(&str, i64)> =
                model.iter().rev().map(|(k, v)| (k.as_str(), *v)).collect();
            assert_eq!(got, want, "step {step}");
            for i in 0..20 {
                let k = format!("k{i}");
                let there = model.iter().find(|(m, _)| *m == k).map(|(_, v)| *v);
                assert_eq!(
                    t.row(&k).and_then(|r| r.get(&N)),
                    there,
                    "step {step} key {k}"
                );
            }
        }
    }

    #[test]
    fn a_series_is_filled_in_place_and_says_what_changed() {
        let mut s = Series::default();
        assert!(s.fill(true, [[0.0, 1.0], [1.0, 2.0]]));
        assert!(!s.fill(true, [[0.0, 1.0], [1.0, 2.0]]));
        assert!(s.fill(false, [[0.0, 1.0], [1.0, 2.0]]));
        assert!(s.fill(false, [[0.0, 1.0]]));
        assert_eq!(s.points, [[0.0, 1.0]]);
        assert!(s.fill(false, [[0.0, 3.0]]));
        assert!(!s.line);
    }
}

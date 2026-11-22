//! Publishing objects cell by cell, for a stack family without an interface description: one declaration of columns
//! defines the table row, the map point and the events of an object type. A family with a description publishes
//! through [`crate::shaped`].
//!
//! A row is written cell by cell, so a row with unchanged values does not advance a change counter of the results.

use crate::log::LogRing;
use crate::seen::Seen;
use crate::table::{ObjectTable, RowRef};
use sdrkit_core::publish::{Field, Published, TableDef};
use sdrkit_core::record::Kind;
use sdrkit_core::results::RowMut;
use sdrkit_core::{Event, EventDef, Record, Results};
use sdrkit_util_geo::Position;
use std::borrow::Cow;
use std::fmt::{self, Display, Write};
use std::hash::Hash;

/// An object type that is published. Declare its columns once as a `&[&dyn Declare]`, and use them in its `TableDef` and in
/// its `EventDef`.
///
/// ```
/// use sdrkit_core::publish::{field, Declare, Field, Published, TableDef};
/// use sdrkit_util_geo::Position;
/// use sdrkit_util_objects::publish::{self, columns, Cells, Object, Point};
/// use sdrkit_util_objects::{ObjectTable, Seen, Stamp};
///
/// struct Ship { name: String, at: Option<Position> }
///
/// const NAME: Field<field::Text> = Field::new("name", "Name", "");
/// const SHIPS: TableDef = TableDef {
///     key: "ships",
///     label: "Ships",
///     columns: &[&NAME as &dyn Declare, &columns::HEARD],
///     cap: 300,
///     marks: sdrkit_core::RowMarks::NONE,
/// };
///
/// impl Object for Ship {
///     fn write(&self, seen: Option<&Seen>, _now_s: f64, out: &mut impl Cells) {
///         out.set(&NAME, &self.name);
///         out.opt(&columns::HEARD, columns::heard(seen));
///     }
///     fn point(&self) -> Option<Point<'_>> {
///         self.at.map(|p| Point::new(p, &self.name))
///     }
/// }
///
/// let mut t: ObjectTable<u32, Ship> = ObjectTable::new(300);
/// t.upsert(&235_009_000, Stamp::at(10.0), || Ship { name: "PRIDE OF KENT".into(), at: Some(Position::new(51.1, 1.4)) })
///     .unwrap();
/// let mut p = Published::new(&[]).with_tables(&[&SHIPS]);
/// publish::rows(&t, &SHIPS, 25.0, &mut p);
/// publish::points(&t, "ships/", p.results_mut());
/// let r = p.results();
/// let table = r.table(r.decl().table_id("ships").unwrap()).unwrap();
/// assert_eq!(table.row("235009000").unwrap().get(&NAME), Some("PRIDE OF KENT"));
/// assert_eq!(r.points()["ships/235009000"].label, "PRIDE OF KENT");
/// ```
pub trait Object {
    /// Writes the cells of the object's columns to `out`. `seen` is the object's [`Seen`] when it is in an [`ObjectTable`],
    /// and `None` for a log entry. `now_s` is the signal time of the report.
    fn write(&self, seen: Option<&Seen>, now_s: f64, out: &mut impl Cells);

    /// Where to draw the object on a map, if it has a position.
    fn point(&self) -> Option<Point<'_>> {
        None
    }
}

/// The unit object has no cells and no point. It is the default link data of an object that takes one.
impl Object for () {
    fn write(&self, _seen: Option<&Seen>, _now_s: f64, _out: &mut impl Cells) {}
}

/// Where an [`Object`] writes its cells: a table row ([`RowMut`]) or the fields of an event ([`Record`]).
pub trait Cells: sealed::Sealed {
    /// Writes column `f`.
    fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>);
    /// Writes column `f` if `v` is `Some`, else marks it as having no value.
    fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>);
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for sdrkit_core::Record {}
    impl Sealed for sdrkit_core::results::RowMut<'_> {}
}

impl Cells for RowMut<'_> {
    fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) {
        RowMut::set(self, f, v);
    }
    fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) {
        RowMut::opt(self, f, v);
    }
}

impl Cells for Record {
    fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) {
        Record::set(self, f, v);
    }
    fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) {
        Record::opt(self, f, v);
    }
}

/// An object's place on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Point<'a> {
    /// Where.
    pub position: Position,
    /// The label.
    pub label: Cow<'a, str>,
    /// Degrees from true north, for a moving target.
    pub heading_deg: Option<f32>,
}

impl<'a> Point<'a> {
    /// A point at `position` labelled `label`, without heading.
    pub fn new(position: Position, label: impl Into<Cow<'a, str>>) -> Self {
        Point {
            position,
            label: label.into(),
            heading_deg: None,
        }
    }

    /// The same point with a heading in degrees from true north.
    #[must_use]
    pub fn heading(self, deg: f32) -> Self {
        Point {
            heading_deg: Some(deg),
            ..self
        }
    }
}

/// The standard columns about when an object was heard, and their values.
pub mod columns {
    use crate::seen::Seen;
    use sdrkit_core::publish::{field, Field};

    /// Times heard.
    pub const HEARD: Field<field::I64> = Field::new("heard", "Heard", "");
    /// Wall-clock time when first heard.
    pub const FIRST_HEARD: Field<field::Time> = Field::new("first_heard", "First heard", "");
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    pub const LAST_HEARD: Field<field::Time> = Field::new("last_heard", "Last heard", "");

    /// The value of [`HEARD`].
    pub fn heard(seen: Option<&Seen>) -> Option<i64> {
        seen.map(|s| i64::try_from(s.count).unwrap_or(i64::MAX))
    }

    /// The value of [`FIRST_HEARD`], if the wall time is known.
    pub fn first_heard(seen: Option<&Seen>) -> Option<f64> {
        seen.and_then(|s| s.first_wall)
    }

    /// The value of [`LAST_HEARD`], if the wall time is known: the wall time rounded down to the whole second, so an object
    /// that is heard again within the same second keeps the same value.
    pub fn last_heard(seen: Option<&Seen>) -> Option<f64> {
        seen.and_then(|s| s.last_wall).map(f64::floor)
    }
}

/// Makes the rows of the declared table `def` hold the objects of `objects`. The row key is the object key's text. Only the
/// cells that changed are written, and rows of objects that left are removed. The newest object is the newest row.
pub fn rows<K, R>(objects: &ObjectTable<K, R>, def: &TableDef, now_s: f64, p: &mut Published)
where
    K: Hash + Eq + Clone + Display,
    R: Object,
{
    rows_where(objects, def, now_s, p, |_| true);
}

/// As [`rows`], with only the objects for which `keep` says yes (the filters a stack offers: text, least level, open only).
pub fn rows_where<K, R>(
    objects: &ObjectTable<K, R>,
    def: &TableDef,
    now_s: f64,
    p: &mut Published,
    mut keep: impl FnMut(RowRef<'_, K, R>) -> bool,
) where
    K: Hash + Eq + Clone + Display,
    R: Object,
{
    rows_with(objects, def, p, &mut keep, |r, row| {
        r.record.write(Some(r.seen), now_s, row);
    });
}

/// Makes the rows of `def` hold the objects of `objects` for which `keep` says yes. `write` writes the row of one.
pub(crate) fn rows_with<K, R>(
    objects: &ObjectTable<K, R>,
    def: &TableDef,
    p: &mut Published,
    mut keep: impl FnMut(RowRef<'_, K, R>) -> bool,
    mut write: impl FnMut(RowRef<'_, K, R>, &mut RowMut<'_>),
) where
    K: Hash + Eq + Clone + Display,
{
    let mut key = KeyText::new();
    let mut written = 0;
    for r in objects.iter() {
        if !keep(r) {
            continue;
        }
        if let Some(mut row) = p.row_mut(def, key.set("", r.key)) {
            write(r, &mut row);
            written += 1;
        }
    }
    keep_newest_rows(def, written, p);
}

/// Makes the rows of the declared table `def` hold the entries of `log`. The row key is the entry's sequence number in the
/// log, so a new entry adds one row and moves no other.
pub fn log_rows<T: Object>(log: &LogRing<T>, def: &TableDef, now_s: f64, p: &mut Published) {
    log_rows_with(log, def, p, |e, row| e.write(None, now_s, row));
}

/// Makes the rows of `def` hold the entries of `log`. `write` writes the row of one.
pub(crate) fn log_rows_with<T>(
    log: &LogRing<T>,
    def: &TableDef,
    p: &mut Published,
    mut write: impl FnMut(&T, &mut RowMut<'_>),
) {
    let mut key = KeyText::new();
    let mut written = 0;
    for (seq, e) in log.iter_seq() {
        if let Some(mut row) = p.row_mut(def, key.set("", &format_args!("{seq:06}"))) {
            write(e, &mut row);
            written += 1;
        }
    }
    keep_newest_rows(def, written, p);
}

/// Makes the rows of the declared table `def` hold `items`, in order, the last the newest. `key` writes the row key of an
/// item. An item whose key does not write is left out. Rows of other keys are removed.
pub fn rows_of<'a, T>(
    items: impl IntoIterator<Item = &'a T>,
    key: impl Fn(&T, &mut dyn Write) -> fmt::Result,
    def: &TableDef,
    now_s: f64,
    p: &mut Published,
) where
    T: Object + 'a,
{
    rows_of_with(items, key, def, p, |it, row| it.write(None, now_s, row));
}

/// Makes the rows of `def` hold `items`, each under the key that `key` writes. `write` writes the row of one.
pub(crate) fn rows_of_with<'a, T: 'a>(
    items: impl IntoIterator<Item = &'a T>,
    key: impl Fn(&T, &mut dyn Write) -> fmt::Result,
    def: &TableDef,
    p: &mut Published,
    mut write: impl FnMut(&T, &mut RowMut<'_>),
) {
    let mut text = KeyText::new();
    let mut written = 0;
    for it in items {
        let Some(k) = text.write_with(|w| key(it, w)) else {
            continue;
        };
        if let Some(mut row) = p.row_mut(def, k) {
            write(it, &mut row);
            written += 1;
        }
    }
    keep_newest_rows(def, written, p);
}

/// Makes the rows of the declared table `def` whose key starts with `parent` and `/` hold `children`, in order: the row key
/// is `parent/child`. Rows of the same parent that are not written are removed. Rows of other parents stay.
pub fn child_rows<'a, T, C>(
    parent: &impl Display,
    children: impl IntoIterator<Item = (&'a C, &'a T)>,
    def: &TableDef,
    now_s: f64,
    p: &mut Published,
) where
    T: Object + 'a,
    C: Display + ?Sized + 'a,
{
    child_rows_with(parent, children, def, p, |it, row| {
        it.write(None, now_s, row)
    });
}

/// Makes the rows of `def` under `parent` hold `children`. `write` writes the row of one.
pub(crate) fn child_rows_with<'a, T, C>(
    parent: &impl Display,
    children: impl IntoIterator<Item = (&'a C, &'a T)>,
    def: &TableDef,
    p: &mut Published,
    mut write: impl FnMut(&T, &mut RowMut<'_>),
) where
    T: 'a,
    C: Display + ?Sized + 'a,
{
    let mut text = KeyText::new();
    let mut written = 0;
    for (child, it) in children {
        let Some(k) = text.write_with(|w| write!(w, "{parent}/{child}")) else {
            continue;
        };
        if let Some(mut row) = p.row_mut(def, k) {
            write(it, &mut row);
            written += 1;
        }
    }
    let Some(prefix) = text.write_with(|w| write!(w, "{parent}/")) else {
        return;
    };
    let Some(id) = p.results().decl().table_id(def.key) else {
        return;
    };
    let Some(table) = p.results().table(id) else {
        return;
    };
    let held = table
        .rows_newest_first()
        .filter(|(k, _)| k.starts_with(prefix))
        .count();
    if held == written {
        return;
    }
    // The rows written last are the newest of the table.
    let stale: Vec<String> = table
        .rows_newest_first()
        .skip(written)
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, _)| k.to_owned())
        .collect();
    if let Some(mut t) = p.results_mut().table_mut(id) {
        for k in &stale {
            t.remove(k);
        }
    }
}

/// Keeps the `n` newest rows of the declared table `def` and removes the others. A row that a writer touched last is the
/// newest, so after writing `n` rows this removes every row that was not written.
pub fn keep_newest_rows(def: &TableDef, n: usize, p: &mut Published) {
    let Some(id) = p.results().decl().table_id(def.key) else {
        return;
    };
    let Some(table) = p.results().table(id) else {
        return;
    };
    let stale = table.len().saturating_sub(n);
    if stale == 0 {
        return;
    }
    let keys: Vec<String> = table
        .rows_newest_first()
        .skip(n)
        .map(|(k, _)| k.to_owned())
        .collect();
    if let Some(mut t) = p.results_mut().table_mut(id) {
        for k in &keys {
            t.remove(k);
        }
    }
}

/// An event of kind `def` with the cells of `object`. The fields of `def` must be the object's columns.
pub fn event<R: Object>(
    object: &R,
    seen: Option<&Seen>,
    def: &'static EventDef,
    time: f64,
    freq_hz: f64,
) -> Event {
    let mut e = def.event(time, freq_hz);
    object.write(seen, time, &mut e.fields);
    e
}

/// Makes the map points whose id starts with `prefix` hold the points of the objects of `objects` (id: `prefix` and the key's
/// text). A point that did not change is not written. Points with other prefixes stay.
pub fn points<K, R>(objects: &ObjectTable<K, R>, prefix: &str, r: &mut Results)
where
    K: Hash + Eq + Clone + Display,
    R: Object,
{
    points_with(objects, None, prefix, r, R::point);
}

/// As [`points`], for objects whose rows are in table `table` under the text of their keys: each point states its row
/// ([`Results::link_point`]), so a front end finds the row of a point without reading the point id.
pub fn row_points<K, R>(objects: &ObjectTable<K, R>, table: &TableDef, prefix: &str, r: &mut Results)
where
    K: Hash + Eq + Clone + Display,
    R: Object,
{
    points_with(objects, Some(table), prefix, r, R::point);
}

/// Makes the map points under `prefix` hold the points that `point` returns for the objects of `objects`. With
/// `table`, each point states the row of its object in that table.
pub(crate) fn points_with<K, R>(
    objects: &ObjectTable<K, R>,
    table: Option<&TableDef>,
    prefix: &str,
    r: &mut Results,
    point: impl for<'x> Fn(&'x R) -> Option<Point<'x>>,
) where
    K: Hash + Eq + Clone + Display,
{
    let mut id = KeyText::new();
    let mut written = 0;
    let table = table.and_then(|t| r.decl().table_id(t.key));
    for row in objects.iter() {
        let Some(pt) = point(row.record) else {
            continue;
        };
        if pt.position.validate().is_err() {
            continue;
        }
        written += 1;
        let id = id.set(prefix, row.key);
        r.update_point(
            id,
            pt.position.lat_deg,
            pt.position.lon_deg,
            &pt.label,
            pt.heading_deg,
        );
        if let Some(table) = table {
            r.link_point(id, Some((table, id.get(prefix.len()..).unwrap_or_default())));
        }
    }
    let held = r.points().keys().filter(|k| k.starts_with(prefix)).count();
    if held == written {
        return;
    }
    let mut now: Vec<String> = Vec::with_capacity(written);
    for row in objects.iter() {
        if point(row.record).is_some_and(|p| p.position.validate().is_ok()) {
            now.push(id.set(prefix, row.key).to_owned());
        }
    }
    now.sort_unstable();
    let gone: Vec<String> = r
        .points()
        .keys()
        .filter(|k| k.starts_with(prefix) && now.binary_search(k).is_err())
        .cloned()
        .collect();
    for k in &gone {
        r.remove_point(k);
    }
}

/// The text of a row key or point id. A short text is kept in a buffer of its own.
pub(crate) struct KeyText {
    buf: [u8; KeyText::CAP],
    len: usize,
    long: String,
}

impl KeyText {
    const CAP: usize = 96;

    pub(crate) fn new() -> Self {
        KeyText {
            buf: [0; KeyText::CAP],
            len: 0,
            long: String::new(),
        }
    }

    /// `prefix` followed by the text of `key`.
    pub(crate) fn set(&mut self, prefix: &str, key: &impl Display) -> &str {
        self.write_with(|w| {
            w.write_str(prefix)?;
            write!(w, "{key}")
        })
        .unwrap_or_default()
    }

    /// The text that `f` writes. `None` if `f` fails.
    pub(crate) fn write_with(
        &mut self,
        f: impl FnOnce(&mut dyn Write) -> fmt::Result,
    ) -> Option<&str> {
        self.len = 0;
        self.long.clear();
        f(self).ok()?;
        if self.long.is_empty() {
            std::str::from_utf8(&self.buf[..self.len]).ok()
        } else {
            Some(&self.long)
        }
    }
}

impl fmt::Write for KeyText {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.long.is_empty() && self.len + s.len() <= KeyText::CAP {
            self.buf[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
            self.len += s.len();
        } else {
            if self.long.is_empty() {
                self.long
                    .push_str(std::str::from_utf8(&self.buf[..self.len]).unwrap_or_default());
            }
            self.long.push_str(s);
        }
        Ok(())
    }
}

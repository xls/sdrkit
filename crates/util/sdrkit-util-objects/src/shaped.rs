//! Publishing objects of a stack family with an interface description: an object type names the shape of its row, a
//! table names the shape of its rows, and a row is written as one struct. An object cannot be published into a table
//! of another shape, and no single cell can be written.
//!
//! Only the cells that changed are written, so a row with unchanged values does not advance a change counter.

use crate::log::LogRing;
use crate::publish::{self, Point};
use crate::seen::Seen;
use crate::table::{ObjectTable, RowRef};
use sdrkit_core::interface::{EventKind, Fields, Table};
use sdrkit_core::publish::Published;
use sdrkit_core::results::RowMut;
use sdrkit_core::{Event, Record, Results};
use std::fmt::{self, Display, Write};
use std::hash::Hash;
use std::marker::PhantomData;

/// An object type that is published as rows of the shape [`Shaped::Row`].
///
/// ```
/// use sdrkit_core::interface::{Fields, Table};
/// use sdrkit_core::publish::Published;
/// use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
/// use sdrkit_util_objects::{ObjectTable, Seen, Stamp};
///
/// /// A ship heard.
/// #[derive(Fields)]
/// struct ShipRow<'a> {
///     /// The name.
///     #[field(label = "Name")]
///     name: &'a str,
///     /// Times heard.
///     #[field(label = "Heard")]
///     heard: Option<i64>,
/// }
/// static SHIPS: Table<ShipRow<'static>> = Table::new("ships", "Ships", 300);
///
/// struct Ship { name: String }
///
/// impl Shaped for Ship {
///     type Row = ShipRow<'static>;
///     fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
///         out.put(&ShipRow { name: &self.name, heard: seen.map(|s| s.count as i64) });
///     }
/// }
///
/// let mut t: ObjectTable<u32, Ship> = ObjectTable::new(300);
/// t.upsert(&235_009_000, Stamp::at(10.0), || Ship { name: "PRIDE OF KENT".into() }).unwrap();
/// let mut p = Published::new(&[]).with_tables(&[&SHIPS]);
/// shaped::rows(&t, &SHIPS, 25.0, &mut p);
/// let (key, ship) = SHIPS.rows::<ShipRow>(p.results()).next().unwrap();
/// assert_eq!((key, ship.name, ship.heard), ("235009000", "PRIDE OF KENT", Some(1)));
/// ```
pub trait Shaped {
    /// The shape of the row, with `'static` borrows.
    type Row: Fields + 'static;

    /// Writes the row of the object to `out` with one [`RowOut::put`]. `seen` is the object's [`Seen`] when it is in
    /// an [`ObjectTable`], and `None` for a log entry. `now_s` is the signal time of the report.
    fn row(&self, seen: Option<&Seen>, now_s: f64, out: RowOut<'_, '_, Self::Row>);

    /// Where to draw the object on a map, if it has a position.
    fn point(&self) -> Option<Point<'_>> {
        None
    }
}

enum To<'a, 'r> {
    Row(&'a mut RowMut<'r>),
    Record(&'a mut Record),
}

/// Where a [`Shaped`] object writes its row: a table row or the fields of an event, of the shape `S`.
pub struct RowOut<'a, 'r, S> {
    to: To<'a, 'r>,
    shape: PhantomData<fn() -> S>,
}

impl<S: Fields + 'static> RowOut<'_, '_, S> {
    /// Writes the row.
    pub fn put<R: Fields<Static = S>>(self, row: &R) {
        match self.to {
            To::Row(out) => row.write(out),
            To::Record(out) => row.write(out),
        }
    }
}

fn to_row<'a, 'r, S>(row: &'a mut RowMut<'r>) -> RowOut<'a, 'r, S> {
    RowOut {
        to: To::Row(row),
        shape: PhantomData,
    }
}

/// Makes the rows of `table` hold the objects of `objects`. The row key is the text of the object key. Rows of objects
/// that left are removed. The newest object is the newest row.
pub fn rows<K, R>(objects: &ObjectTable<K, R>, table: &Table<R::Row>, now_s: f64, p: &mut Published)
where
    K: Hash + Eq + Clone + Display,
    R: Shaped,
{
    rows_where(objects, table, now_s, p, |_| true);
}

/// As [`rows`], with only the objects for which `keep` says yes.
pub fn rows_where<K, R>(
    objects: &ObjectTable<K, R>,
    table: &Table<R::Row>,
    now_s: f64,
    p: &mut Published,
    keep: impl FnMut(RowRef<'_, K, R>) -> bool,
) where
    K: Hash + Eq + Clone + Display,
    R: Shaped,
{
    publish::rows_with(objects, table.def(), p, keep, |r, row| {
        r.record.row(Some(r.seen), now_s, to_row(row));
    });
}

/// As [`rows_where`], for objects of a type that is not [`Shaped`]: `row` writes the row of one object with one
/// [`RowOut::put`].
pub fn rows_by<K, R, S>(
    objects: &ObjectTable<K, R>,
    table: &Table<S>,
    p: &mut Published,
    keep: impl FnMut(RowRef<'_, K, R>) -> bool,
    mut row: impl FnMut(RowRef<'_, K, R>, RowOut<'_, '_, S>),
) where
    K: Hash + Eq + Clone + Display,
    S: Fields + 'static,
{
    publish::rows_with(objects, table.def(), p, keep, |r, out| row(r, to_row(out)));
}

/// Makes the rows of `table` hold the entries of `log`. The row key is the sequence number of the entry in the log, so
/// a new entry adds one row and moves no other.
pub fn log_rows<T: Shaped>(log: &LogRing<T>, table: &Table<T::Row>, now_s: f64, p: &mut Published) {
    publish::log_rows_with(log, table.def(), p, |e, row| {
        e.row(None, now_s, to_row(row))
    });
}

/// Makes the rows of `table` hold `items`, in order, the last the newest. `key` writes the row key of an item. An item
/// whose key does not write is left out. Rows of other keys are removed.
pub fn rows_of<'a, T>(
    items: impl IntoIterator<Item = &'a T>,
    key: impl Fn(&T, &mut dyn Write) -> fmt::Result,
    table: &Table<T::Row>,
    now_s: f64,
    p: &mut Published,
) where
    T: Shaped + 'a,
{
    publish::rows_of_with(items, key, table.def(), p, |it, row| {
        it.row(None, now_s, to_row(row))
    });
}

/// Makes the rows of `table` in `p` hold `items`, in order, keyed by their place (`000`, `001`, ...). `row` writes the
/// row of one item with one [`RowOut::put`]. The rows past the last item are removed.
///
/// ```
/// use sdrkit_core::interface::{Fields, Table};
/// use sdrkit_core::publish::Published;
/// use sdrkit_util_objects::shaped;
///
/// /// A carrier found.
/// #[derive(Fields)]
/// struct CarrierRow {
///     /// The level.
///     #[field(label = "Level", unit = "dB")]
///     level_db: f64,
/// }
/// static CARRIERS: Table<CarrierRow> = Table::new("carriers", "Carriers", 8);
///
/// let mut p = Published::new(&[]).with_tables(&[&CARRIERS]);
/// shaped::indexed_rows(&mut p, &CARRIERS, &[3.0, 5.0], |level, out| out.put(&CarrierRow { level_db: *level }));
/// shaped::indexed_rows(&mut p, &CARRIERS, &[7.0], |level, out| out.put(&CarrierRow { level_db: *level }));
/// let rows: Vec<(&str, f64)> = CARRIERS.rows::<CarrierRow>(p.results()).map(|(k, r)| (k, r.level_db)).collect();
/// assert_eq!(rows, [("000", 7.0)]);
/// ```
pub fn indexed_rows<T, S: Fields + 'static>(
    p: &mut Published,
    table: &Table<S>,
    items: &[T],
    mut row: impl FnMut(&T, RowOut<'_, '_, S>),
) {
    let mut key = String::with_capacity(8);
    for (i, it) in items.iter().enumerate() {
        key.clear();
        let _ = write!(key, "{i:03}");
        if let Some(mut out) = p.row_mut(table.def(), &key) {
            row(it, to_row(&mut out));
        }
    }
    for i in items.len()..table.def().cap {
        key.clear();
        let _ = write!(key, "{i:03}");
        if !p.remove_row(table.def(), &key) {
            break;
        }
    }
}

/// Makes the rows of `table` whose key starts with `parent` and `/` hold `children`, in order: the row key is
/// `parent/child`. Rows of the same parent that are not written are removed. Rows of other parents stay.
pub fn child_rows<'a, T, C>(
    parent: &impl Display,
    children: impl IntoIterator<Item = (&'a C, &'a T)>,
    table: &Table<T::Row>,
    now_s: f64,
    p: &mut Published,
) where
    T: Shaped + 'a,
    C: Display + ?Sized + 'a,
{
    publish::child_rows_with(parent, children, table.def(), p, |it, row| {
        it.row(None, now_s, to_row(row));
    });
}

/// An event of kind `kind` with the row of `object` as its fields.
pub fn event<R: Shaped>(
    object: &R,
    seen: Option<&Seen>,
    kind: &'static EventKind<R::Row>,
    time: f64,
    freq_hz: f64,
) -> Event {
    let mut e = kind.def().event(time, freq_hz);
    object.row(
        seen,
        time,
        RowOut {
            to: To::Record(&mut e.fields),
            shape: PhantomData,
        },
    );
    e
}

/// Makes the map points whose id starts with `prefix` hold the points of the objects of `objects` (id: `prefix` and
/// the text of the key). A point that did not change is not written. Points with other prefixes stay.
pub fn points<K, R>(objects: &ObjectTable<K, R>, prefix: &str, r: &mut Results)
where
    K: Hash + Eq + Clone + Display,
    R: Shaped,
{
    publish::points_with(objects, prefix, r, R::point);
}

/// As [`points`], for objects of a type that is not [`Shaped`]: `point` returns the point of one object.
pub fn points_by<K, R>(
    objects: &ObjectTable<K, R>,
    prefix: &str,
    r: &mut Results,
    point: impl for<'x> Fn(&'x R) -> Option<Point<'x>>,
) where
    K: Hash + Eq + Clone + Display,
{
    publish::points_with(objects, prefix, r, point);
}

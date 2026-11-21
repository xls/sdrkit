//! The object commands that a host runs on the objects of a stack, and the [`Tables`] trait for clear and saved state.
//!
//! A stack lists these commands in its own `commands!` enum and runs each one with one call.

use crate::error::Error;
use crate::persist::{Format, Shift};
use crate::publish::KeyText;
use crate::table::ObjectTable;
use sdrkit_core::command::{Command, CommandError};
use sdrkit_core::interface::{Arguments, CommandKind};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::settings::{Argument, DeclareSetting};
use sdrkit_core::{StateBlob, StateError, TableId, Values};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Display;
use std::hash::Hash;

/// Returns a command argument that holds a row key of table `table` of the same owner.
pub const fn row_of(table: &'static str) -> Argument<field::Text> {
    Argument::new(Field::new("row", "Row", "").refers_to_table(table))
        .help("The row key of the object, as the table shows it.")
}

/// Forgets every object heard so far. Favourites stay. Run it with [`Tables::clear`].
pub const CLEAR: Command =
    Command::new("clear", "Clear").help("Forgets everything heard so far. Favourites stay.");

/// Returns the `remove` command, which forgets one object, a favourite too. `args` holds the row argument of the table,
/// made with [`row_of`]. Run it with [`remove`].
pub const fn remove_command(args: &'static [&'static dyn DeclareSetting]) -> Command {
    Command::new("remove", "Remove")
        .help("Forgets one object, a favourite too.")
        .args(args)
}

/// Returns the `remove` command with the arguments struct `A` of a stack family that has an interface description. `A`
/// holds the row argument of the table, made with [`row_of`]. Run it with [`remove`].
pub const fn remove_kind<A: Arguments + 'static>() -> CommandKind<A> {
    CommandKind::new("remove", "Remove").help("Forgets one object, a favourite too.")
}

/// Runs the `remove` command: removes the object whose row key is the argument `row` from `objects`, and its row from
/// table `table` of `p`.
///
/// # Errors
///
/// [`CommandError::NoSuchRow`] with `table` if no object has that row key. The errors of [`Values::arg`].
pub fn remove<K, R>(
    objects: &mut ObjectTable<K, R>,
    table: TableId,
    p: &mut Published,
    args: &Values,
    row: &Argument<field::Text>,
) -> Result<(), CommandError>
where
    K: Hash + Eq + Clone + Display,
{
    let row = args.arg(row)?;
    let mut text = KeyText::new();
    let Some(key) = objects
        .iter()
        .find(|r| text.set("", r.key) == row)
        .map(|r| r.key.clone())
    else {
        return Err(CommandError::NoSuchRow {
            table,
            key: row.to_owned(),
        });
    };
    objects.remove(&key);
    if let Some(mut t) = p.results_mut().table_mut(table) {
        t.remove(row);
    }
    Ok(())
}

/// The objects a stack keeps, with the [`CLEAR`] command and saved state.
///
/// ```
/// use sdrkit_util_objects::{Format, LogLine, LogRing, ObjectTable, Shift, Stamp, Tables};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Heard {
///     stations: ObjectTable<u32, String>,
///     log: LogRing<LogLine>,
/// }
///
/// impl Shift for Heard {
///     fn shift_times(&mut self, by_s: f64) {
///         self.stations.shift_times(by_s);
///         self.log.shift_times(by_s);
///     }
/// }
///
/// impl Tables for Heard {
///     const FORMAT: Format = Format::new("example.heard", 1);
///     fn clear(&mut self) {
///         self.stations.clear();
///         self.log.clear();
///     }
///     fn adopt(&mut self, saved: Self) {
///         self.stations.adopt(saved.stations);
///         self.log.adopt(saved.log);
///     }
/// }
///
/// let mut h = Heard { stations: ObjectTable::new(300), log: LogRing::new(100) };
/// h.stations.upsert(&1, Stamp::at(50.0), || "first".into()).unwrap();
/// let state = h.save_state(80.0).unwrap();
/// h.clear();
/// h.restore_state(&state, 0.0).unwrap();
/// assert_eq!(h.stations.seen(&1).unwrap().age(0.0), 30.0);
/// ```
pub trait Tables: Shift + Serialize + DeserializeOwned {
    /// The format of the saved state.
    const FORMAT: Format;

    /// Forgets everything heard. Favourites stay. This runs [`CLEAR`].
    fn clear(&mut self);

    /// Takes the content of a restored state. Keep this stack's own configuration (caps, spacing, windows): use
    /// [`ObjectTable::adopt`], [`LogRing::adopt`](crate::LogRing::adopt) and [`Trail::adopt`](crate::Trail::adopt).
    fn adopt(&mut self, saved: Self);

    /// The saved state, at signal time `clock_s`.
    ///
    /// # Errors
    ///
    /// The errors of [`Format::save`].
    fn save_state(&self, clock_s: f64) -> Result<StateBlob, Error> {
        Self::FORMAT.save(clock_s, self)
    }

    /// Replaces what is held with a saved state. Every signal time moves to keep its age at `now_s`. If the state is refused,
    /// nothing changes.
    ///
    /// # Errors
    ///
    /// The errors of [`Format::restore`], as a [`StateError`].
    fn restore_state(&mut self, state: &StateBlob, now_s: f64) -> Result<(), StateError> {
        let saved: Self = Self::FORMAT.restore(state, now_s)?;
        self.adopt(saved);
        Ok(())
    }
}

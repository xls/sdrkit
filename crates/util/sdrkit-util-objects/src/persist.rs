//! Saved state: the format of what a table type saves, and moving signal times on restore.
//!
//! A saved state is a [`StateBlob`] tagged with the format id and version. On restore every signal time moves by
//! `now - clock` (through [`Shift`]), so each row keeps the age it had when it was saved.

use crate::error::Error;
use sdrkit_core::{StateBlob, StateError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

/// Moves every signal time a value holds by the same number of seconds. Wall-clock times stay.
///
/// Implemented by every type of this crate. A record type kept in a table implements it for the signal times it holds itself
/// (and does nothing when it holds none):
///
/// ```
/// use sdrkit_util_objects::Shift;
/// struct Ship { position_time_s: f64, name: String }
/// impl Shift for Ship {
///     fn shift_times(&mut self, by_s: f64) {
///         self.position_time_s += by_s;
///     }
/// }
/// ```
pub trait Shift {
    /// Adds `by_s` seconds to every signal time.
    fn shift_times(&mut self, by_s: f64);
}

impl<T: Shift> Shift for Option<T> {
    fn shift_times(&mut self, by_s: f64) {
        if let Some(x) = self {
            x.shift_times(by_s);
        }
    }
}

impl<T: Shift> Shift for Vec<T> {
    fn shift_times(&mut self, by_s: f64) {
        self.iter_mut().for_each(|x| x.shift_times(by_s));
    }
}

impl<T: Shift> Shift for VecDeque<T> {
    fn shift_times(&mut self, by_s: f64) {
        self.iter_mut().for_each(|x| x.shift_times(by_s));
    }
}

impl<K, V: Shift> Shift for BTreeMap<K, V> {
    fn shift_times(&mut self, by_s: f64) {
        self.values_mut().for_each(|x| x.shift_times(by_s));
    }
}

/// A record with no signal times of its own.
impl Shift for () {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// Text holds no signal times.
impl Shift for String {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// What a saved state is: its format id and the version of its layout.
///
/// ```
/// use sdrkit_util_objects::{Format, ObjectTable, Stamp};
///
/// const HEARD: Format = Format::new("example.heard", 1);
/// let mut t: ObjectTable<u32, ()> = ObjectTable::new(10);
/// t.upsert(&7, Stamp::at(100.0), || ()).unwrap();
/// let state = HEARD.save(130.0, &t).unwrap();
///
/// // A new session whose clock reads 5 s: row 7 is still 30 s old.
/// let back: ObjectTable<u32, ()> = HEARD.restore(&state, 5.0).unwrap();
/// assert_eq!(back.seen(&7).unwrap().age(5.0), 30.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    /// The id, `<stack>.<what>`. It is the owner tag of the [`StateBlob`].
    pub id: &'static str,
    /// The version of the layout of the data.
    pub version: u32,
}

#[derive(Serialize)]
struct Out<'a, T: ?Sized> {
    clock: f64,
    data: &'a T,
}

#[derive(Deserialize)]
struct In<T> {
    clock: f64,
    data: T,
}

impl Format {
    /// The format `id` at `version`.
    pub const fn new(id: &'static str, version: u32) -> Format {
        Format { id, version }
    }

    /// The state of `data`, saved at signal time `clock_s`.
    ///
    /// # Errors
    ///
    /// [`Error::BadTime`] if the clock is not finite. [`Error::NotSaved`] if serde cannot write `data`.
    pub fn save<T: Serialize + ?Sized>(&self, clock_s: f64, data: &T) -> Result<StateBlob, Error> {
        if !clock_s.is_finite() {
            return Err(Error::BadTime);
        }
        let bytes = serde_json::to_vec(&Out {
            clock: clock_s,
            data,
        })
        .map_err(|_| Error::NotSaved)?;
        Ok(StateBlob::new(self.id, self.version, bytes))
    }

    /// The content of a state saved in this format and version, with every signal time moved so that ages are kept at signal
    /// time `now_s`.
    ///
    /// # Errors
    ///
    /// [`StateError::WrongOwner`] or [`StateError::WrongVersion`] for a state of another format, [`StateError::Damaged`] if
    /// the bytes do not read as `T`, [`Error::BadTime`] if a clock is not finite.
    pub fn restore<T: DeserializeOwned + Shift>(
        &self,
        state: &StateBlob,
        now_s: f64,
    ) -> Result<T, Error> {
        if !now_s.is_finite() {
            return Err(Error::BadTime);
        }
        let bytes = state.open(self.id, self.version)?;
        let mut saved: In<T> = serde_json::from_slice(bytes).map_err(|_| StateError::Damaged)?;
        if !saved.clock.is_finite() {
            return Err(Error::BadTime);
        }
        saved.data.shift_times(now_s - saved.clock);
        Ok(saved.data)
    }
}

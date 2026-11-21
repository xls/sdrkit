//! What every display of the family shares: the signal clock kept over a restart of the input, the drift of a carrier, the
//! points of a declared series and the `remove` command on a table of objects.

use sdrkit_core::publish::field;
use sdrkit_core::settings::Argument;
use sdrkit_core::{CommandError, LockState, Published, Results, SeriesId, TableDef, Values};
use sdrkit_util_objects::{ObjectTable, Stamp};
use std::fmt::Display;
use std::hash::Hash;

/// The signal clock of a display: seconds of signal since the stack started.
///
/// Item times restart at 0 when the input restarts (a rate change or a reset). The clock adds the time that passed before, so
/// the times kept in tables keep growing. With a live radio it also provides the wall-clock time of a moment.
///
/// ```
/// use sdrkit_stack_satcom::display::Clock;
/// let mut c = Clock::default();
/// assert_eq!(c.at(10.0), 10.0);
/// c.restart(false, 0.0);
/// assert_eq!(c.at(2.0), 12.0);
/// assert_eq!(c.stamp(12.0).wall, None);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Clock {
    base_s: f64,
    last_s: f64,
    live: bool,
    wall_at_zero: f64,
}

impl Clock {
    /// The input starts again at time 0. `live` is true when the samples come from a radio as they are received. `unix_s` is
    /// the host's time now, seconds since 1970 (UTC).
    pub fn restart(&mut self, live: bool, unix_s: f64) {
        self.base_s += self.last_s;
        self.last_s = 0.0;
        self.live = live;
        if live {
            self.wall_at_zero = unix_s - self.base_s;
        }
    }
    /// The signal time of an item time of the current input. A time that is not finite returns the current time.
    pub fn at(&mut self, item_s: f64) -> f64 {
        if item_s.is_finite() && item_s > self.last_s {
            self.last_s = item_s;
        }
        if item_s.is_finite() {
            self.base_s + item_s
        } else {
            self.now()
        }
    }
    /// The newest signal time seen.
    pub fn now(&self) -> f64 {
        self.base_s + self.last_s
    }
    /// The wall-clock time (seconds since 1970, UTC) of a signal time, when the samples are live.
    pub fn wall(&self, signal_s: f64) -> Option<f64> {
        self.live.then_some(self.wall_at_zero + signal_s)
    }
    /// The moment of a signal time, with its wall-clock time when the samples are live.
    pub fn stamp(&self, signal_s: f64) -> Stamp {
        match self.wall(signal_s) {
            Some(w) => Stamp::at(signal_s).with_wall(w),
            None => Stamp::at(signal_s),
        }
    }
}

/// The least-squares slope of `(t, f)` points (a drift in Hz/s from carrier offsets over time), or 0 when the points do not
/// define one (fewer than two distinct times, or a value that is not finite).
pub fn slope_of(points: &[(f64, f64)]) -> f64 {
    let (t, f): (Vec<f64>, Vec<f64>) = points.iter().copied().unzip();
    sdrkit_algo_dsp::peak::line_fit(&t, &f, None).map_or(0.0, |l| l.slope)
}

/// Makes series `id` of `r` hold the points that `points` returns (drawn as a line if `line`), if they differ from what it
/// holds. Allocates nothing once the series has reached its size.
pub fn set_points<I>(r: &mut Results, id: SeriesId, line: bool, points: impl Fn() -> I)
where
    I: Iterator<Item = [f32; 2]>,
{
    r.update_series(id, |s| {
        let same = s.line == line
            && s.points.len() == points().count()
            && s.points.iter().zip(points()).all(|(a, b)| *a == b);
        if !same {
            s.line = line;
            s.points.clear();
            s.points.extend(points());
        }
        !same
    });
}

/// Makes series `key` of `r` hold the points that `points` returns, as [`set_points`]. A key the results do not declare is left
/// alone.
pub fn set_series<I>(r: &mut Results, key: &str, line: bool, points: impl Fn() -> I)
where
    I: Iterator<Item = [f32; 2]>,
{
    if let Some(id) = r.decl().series_id(key) {
        set_points(r, id, line, points);
    }
}

/// Makes the status of `r` show `lock_state`, `offset_hz`, `snr_db`, `level_dbfs` and `summary`, if it shows something else. Allocates
/// nothing once the summary has reached its length.
pub fn set_status(
    r: &mut Results,
    lock_state: LockState,
    offset_hz: Option<f64>,
    snr_db: Option<f32>,
    level_dbfs: Option<f32>,
    summary: &str,
) {
    r.update_status(|st| {
        let same = st.lock_state == lock_state
            && st.offset_hz == offset_hz
            && st.snr_db == snr_db
            && st.level_dbfs == level_dbfs
            && st.summary == summary;
        if !same {
            st.lock_state = lock_state;
            st.offset_hz = offset_hz;
            st.snr_db = snr_db;
            st.level_dbfs = level_dbfs;
            st.summary.clear();
            st.summary.push_str(summary);
        }
        !same
    });
}

/// Runs the object command `remove`: forgets the object of `objects` whose row key is the argument `row` of `args`. The next
/// report removes its row of table `t` of `p`.
///
/// # Errors
///
/// [`CommandError::NoSuchRow`] if no object has that row key. The errors of [`Values::arg`].
pub(crate) fn remove<K, R>(
    objects: &mut ObjectTable<K, R>,
    t: &TableDef,
    p: &Published,
    args: &Values,
    row: &Argument<field::Text>,
) -> Result<(), CommandError>
where
    K: Hash + Eq + Clone + Display,
{
    let row = args.arg(row)?;
    let key =
        sdrkit_util_objects::rows::key_of_row(objects, row).ok_or_else(|| p.no_such_row(t, row))?;
    objects.remove(&key);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_keeps_growing_over_restarts() {
        let mut c = Clock::default();
        c.at(5.0);
        c.at(3.0);
        assert_eq!(c.now(), 5.0);
        c.restart(false, 0.0);
        assert_eq!(c.at(1.0), 6.0);
        c.restart(true, 1000.0);
        assert!(c.wall(6.0).is_some());
        assert_eq!(c.at(f64::NAN), 6.0);
    }

    #[test]
    fn slope_of_a_ramp() {
        let p: Vec<(f64, f64)> = (0..5).map(|i| (i as f64, 3.0 * i as f64 - 1.0)).collect();
        assert!((slope_of(&p) - 3.0).abs() < 1e-12);
        assert_eq!(slope_of(&p[..1]), 0.0);
    }
}

//! What the three stacks of the crate share: the report period, the minimum-rate message, the wall clock of live samples, the
//! text of addresses and of padded ASCII fields, the signal clock of a table, the status line, the track points table and a span
//! resampler.

use sdrkit_core::publish::{Published, TableDef};
use sdrkit_core::{Clock, LockState, Status};
use sdrkit_util_objects::publish::keep_newest_rows;
use sdrkit_util_objects::{ObjectTable, Trail};
use sdrkit_util_text::ia5::{printable_into, Printable};
use std::fmt::{Display, Write as _};
use std::hash::Hash;

/// The report period of every graph of the crate, seconds of signal.
pub const REPORT_S: f64 = 0.25;

/// The status line of a stack that refuses a rate below `rate_hz`: "`<name>` needs a sample rate of at least `<n>` Msps".
pub fn min_rate_message(name: &str, rate_hz: f64) -> String {
    let v = (rate_hz / 1e3).round() / 1e3;
    format!("{name} needs a sample rate of at least {v} Msps")
}

/// The wall-clock time now (seconds since 1970, UTC) when the samples are live, else `None`. A recording has no wall time.
pub fn wall(live: bool, clock: &Clock) -> Option<f64> {
    live.then(|| clock.now())
}

/// Bytes as upper-case hex pairs joined by colons, the first byte first ("C6:12:34:56:78:9A").
pub fn mac_text(b: &[u8]) -> String {
    let mut s = String::with_capacity(3 * b.len());
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            s.push(':');
        }
        // Writing to a `String` cannot fail.
        let _ = write!(s, "{x:02X}");
    }
    s
}

/// A null-padded ASCII field as text: it ends at the first zero, a byte outside 32 to 126 becomes `?`, and trailing spaces go.
pub fn ascii_text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let opt = Printable {
        replacement: Some('?'),
        line_breaks: false,
        ..Printable::default()
    };
    let mut s = String::with_capacity(end);
    printable_into(&b[..end], &opt, &mut s);
    s.truncate(s.trim_end_matches(' ').len());
    s
}

/// The signal time of a table component. It keeps running across reconfigures, so the times of rows never jump back.
#[derive(Clone, Debug, Default)]
pub struct SignalClock {
    /// Seconds before the current configure.
    pub epoch: f64,
    /// Samples since the current configure.
    pub samples: u64,
    /// The input rate.
    pub rate: f64,
}

impl SignalClock {
    /// Starts a new configure at `rate`. The time so far becomes the epoch.
    pub fn restart(&mut self, rate: f64) {
        self.epoch = self.now();
        self.samples = 0;
        self.rate = rate;
    }
    /// The signal time now, seconds.
    pub fn now(&self) -> f64 {
        if self.rate > 0.0 {
            self.epoch + self.samples as f64 / self.rate
        } else {
            self.epoch
        }
    }
    /// The signal time of an item time `t` of the current configure.
    pub fn at(&self, t: f64) -> f64 {
        self.epoch + t
    }
}

/// The parts of a status that a table component writes at each report.
#[derive(Clone, Debug, Default)]
pub struct StatusLine {
    status: Status,
}

impl StatusLine {
    /// Writes the lock state, the measures and the summary line into `p`. Only a change moves the change counter. Nothing is
    /// allocated once the summary has its size.
    pub fn write(
        &mut self,
        p: &mut Published,
        lock_state: LockState,
        snr_db: Option<f64>,
        level_dbfs: Option<f64>,
        offset_hz: Option<f64>,
        summary: std::fmt::Arguments<'_>,
    ) {
        let s = &mut self.status;
        s.lock_state = lock_state;
        s.snr_db = snr_db.map(|v| v as f32);
        s.level_dbfs = level_dbfs.map(|v| v as f32);
        s.offset_hz = offset_hz;
        s.summary.clear();
        // Writing to a `String` cannot fail.
        let _ = s.summary.write_fmt(summary);
        p.results_mut().set_status(s);
    }
}

/// Writes the trail points of every object of `objects` into a track table, and removes the rows of points that left.
/// `put` writes one row: it gets the store, the row key (the object key and the signal time of the point), the object
/// key, the position and the signal time. `key` is a scratch buffer.
pub fn publish_tracks<K, R>(
    objects: &ObjectTable<K, R>,
    trail: impl Fn(&R) -> &Trail,
    p: &mut Published,
    key: &mut String,
    mut put: impl FnMut(&mut Published, &str, &str, (f64, f64), f64),
    table: &TableDef,
) where
    K: Hash + Eq + Clone + Display,
{
    let mut written = 0;
    for r in objects.iter() {
        for pt in trail(r.record).points() {
            key.clear();
            let _ = write!(key, "{}/{:.3}", r.key, pt.t);
            let end = key.rfind('/').unwrap_or(key.len());
            put(
                p,
                key,
                &key[..end],
                (pt.position.lat_deg, pt.position.lon_deg),
                pt.t,
            );
            written += 1;
        }
    }
    keep_newest_rows(table, written, p);
}

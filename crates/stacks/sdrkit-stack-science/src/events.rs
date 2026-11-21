//! Event rows of the event monitors (`schumann`, `whistler` and others): the row, its event, the CSV line of a
//! row, the trend CSV, the counts per UTC hour, and the logs of events and trend rows that a monitor keeps and publishes.

use crate::calendar;
use crate::family::write_series;
use crate::typed::{ScienceEvent, ScienceEventRow, SCIENCE_EVENT, SCIENCE_EVENTS};
use sdrkit_core::interface::{Fields, Hertz, Seconds, Table, Time};
use sdrkit_core::publish::{Published, SeriesDef};
use sdrkit_core::{Event, SeriesId};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// One event of a monitor.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    /// Signal time of the start, s.
    pub signal_time_s: f64,
    /// The type: `transient`, `whistler`, `tweek`.
    pub kind: String,
    /// The characteristic frequency, Hz.
    pub frequency_hz: f64,
    /// The strength over the background, dB.
    pub peak_db: f64,
    /// The duration, s.
    pub duration_s: f64,
    /// Stack-specific value 1.
    pub aux1: f64,
    /// Stack-specific value 2.
    pub aux2: f64,
    /// Free text.
    pub note: String,
}

/// The header line of the event CSV.
pub const EVENT_CSV_HEADER: &str =
    "utc,signal_time_s,type,frequency_hz,peak_db,duration_s,aux1,aux2,note";

/// The UTC text of the capture start `start_utc` plus the whole seconds of `signal_time_s`: `YYYY-MM-DDTHH:MM:SSZ`.
pub fn utc_text(start_utc: f64, signal_time_s: f64) -> String {
    calendar::iso((start_utc.floor() + signal_time_s.max(0.0).floor()) as i64)
}

impl EventRow {
    /// The CSV line of the row for a capture that started at `start_utc` (Unix s). The note is in double quotes; a double
    /// quote inside it becomes a single quote.
    pub fn csv(&self, start_utc: f64) -> String {
        format!(
            "{},{:.3},{},{:.1},{:.2},{:.3},{},{},\"{}\"",
            utc_text(start_utc, self.signal_time_s),
            self.signal_time_s,
            self.kind,
            self.frequency_hz,
            self.peak_db,
            self.duration_s,
            num(self.aux1),
            num(self.aux2),
            self.note.replace('"', "'")
        )
    }

    /// The event `science.event` of the row.
    pub fn event(&self, start_utc: f64) -> Event {
        SCIENCE_EVENT.event(
            self.signal_time_s,
            self.frequency_hz,
            &ScienceEvent {
                text: &self.note,
                utc: Time(start_utc.floor() + self.signal_time_s.max(0.0).floor()),
                r#type: &self.kind,
                frequency_hz: Hertz(self.frequency_hz),
                peak_db: self.peak_db,
                duration_s: Seconds(self.duration_s),
                aux1: self.aux1,
                aux2: self.aux2,
            },
        )
    }
}

/// A number with up to 3 decimals and no trailing zeros.
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

/// The trend CSV: the header `utc,<a>,<b>` and one row per minute, `utc` = `start_utc` + (i + 1) × 60 s.
pub fn trend_csv(start_utc: f64, names: [&str; 2], rows: &[[f64; 2]]) -> String {
    let mut s = format!("utc,{},{}\n", names[0], names[1]);
    for (i, r) in rows.iter().enumerate() {
        s.push_str(&format!(
            "{},{},{}\n",
            calendar::iso((start_utc.floor() + (i as f64 + 1.0) * 60.0) as i64),
            num(r[0]),
            num(r[1])
        ));
    }
    s
}

/// Events per UTC hour of the day (0 to 23) of the events at `utcs` (Unix s).
pub fn per_utc_hour(utcs: impl IntoIterator<Item = f64>) -> [u32; 24] {
    let mut h = [0u32; 24];
    for u in utcs {
        h[(u.div_euclid(3600.0) as i64).rem_euclid(24) as usize] += 1;
    }
    h
}

/// The most event rows kept.
pub const MAX_EVENTS: usize = 500;

/// The events counted in each UTC hour of the day: x the hour (0 to 23), y the events.
pub const HOURS_SERIES: SeriesDef = SeriesDef {
    key: "events_per_hour",
    label: "Events per UTC hour",
    x_unit: "h",
    y_unit: "",
};

/// Trend rows kept: 24 hours of minutes.
pub const TREND_CAP: usize = 1440;

/// The event rows of a monitor, newest last, with the counts per UTC hour and the total.
#[derive(Clone, Debug, Default)]
pub struct EventLog {
    rows: VecDeque<(u64, EventRow)>,
    next: u64,
    fresh: usize,
    total: u64,
    hours: [u32; 24],
    cleared: bool,
    hours_changed: bool,
    hours_id: Option<SeriesId>,
}

impl EventLog {
    /// Adds `row` for a capture that started at `start_utc` (Unix s): counts it in its UTC hour and appends its
    /// `science.event` to `out`.
    pub fn push(&mut self, row: EventRow, start_utc: f64, out: &mut Vec<Event>) {
        let utc = start_utc.floor() + row.signal_time_s.max(0.0).floor();
        for (h, n) in self.hours.iter_mut().zip(per_utc_hour([utc])) {
            *h += n;
        }
        out.push(row.event(start_utc));
        self.hours_changed = true;
        self.total += 1;
        self.rows.push_back((self.next, row));
        self.next += 1;
        self.fresh = (self.fresh + 1).min(MAX_EVENTS);
        while self.rows.len() > MAX_EVENTS {
            self.rows.pop_front();
        }
    }

    /// The rows kept, oldest first.
    pub fn rows(&self) -> impl Iterator<Item = &EventRow> {
        self.rows.iter().map(|r| &r.1)
    }

    /// Events since the start or the last clear.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Events per UTC hour of the day.
    pub fn hours(&self) -> &[u32; 24] {
        &self.hours
    }

    /// Forgets the rows, the counts and the total. The table is emptied at the next [`EventLog::publish`].
    pub fn clear(&mut self) {
        self.rows.clear();
        self.fresh = 0;
        self.total = 0;
        self.hours = [0; 24];
        self.hours_changed = true;
        self.cleared = true;
    }

    /// Writes the rows added since the last call into the `events` table and, at the first call and after a change, the counts
    /// per hour into [`HOURS_SERIES`], for a capture that started at `start_utc`.
    pub fn publish(&mut self, p: &mut Published, start_utc: f64) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&SCIENCE_EVENTS);
        }
        let start = self.rows.len().saturating_sub(self.fresh);
        for (serial, r) in self.rows.iter().skip(start) {
            SCIENCE_EVENTS.put(
                p,
                &format!("{serial:06}"),
                &ScienceEventRow {
                    utc: Time(start_utc.floor() + r.signal_time_s.max(0.0).floor()),
                    signal_time_s: r.signal_time_s,
                    r#type: &r.kind,
                    frequency_hz: Hertz(r.frequency_hz),
                    peak_db: r.peak_db,
                    duration_s: Seconds(r.duration_s),
                    aux1: r.aux1,
                    aux2: r.aux2,
                    event_note: &r.note,
                },
            );
        }
        self.fresh = 0;
        if self.hours_id.is_none() {
            self.hours_id = p.results().decl().series_id(HOURS_SERIES.key);
            self.hours_changed = true;
        }
        let changed = std::mem::take(&mut self.hours_changed);
        if let (Some(id), true) = (self.hours_id, changed) {
            write_series(
                p.results_mut(),
                id,
                false,
                self.hours
                    .iter()
                    .enumerate()
                    .map(|(h, &n)| [h as f32, n as f32]),
            );
        }
    }
}

/// The rows of a per-minute trend: row `i` belongs to minute `i` of the capture, two values each.
#[derive(Clone, Debug, Default)]
pub struct TrendLog {
    rows: VecDeque<(u64, [f64; 2])>,
    fresh: usize,
    cleared: bool,
}

impl TrendLog {
    /// Adds the row of minute `minute`.
    pub fn push(&mut self, minute: u64, values: [f64; 2]) {
        self.rows.push_back((minute, values));
        self.fresh = (self.fresh + 1).min(TREND_CAP);
        while self.rows.len() > TREND_CAP {
            self.rows.pop_front();
        }
    }

    /// The rows kept, oldest first: the minute and its two values.
    pub fn rows(&self) -> impl Iterator<Item = &(u64, [f64; 2])> {
        self.rows.iter()
    }

    /// Forgets the rows. The table is emptied at the next [`TrendLog::publish`].
    pub fn clear(&mut self) {
        self.rows.clear();
        self.fresh = 0;
        self.cleared = true;
    }

    /// Writes the rows added since the last call into `table`. `row` makes a row from its time, `start_utc` + (minute + 1)
    /// × 60 s, and its two values.
    pub fn publish<S: Fields<Static = S> + 'static>(
        &mut self,
        p: &mut Published,
        table: &Table<S>,
        start_utc: f64,
        row: impl Fn(Time, [f64; 2]) -> S,
    ) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(table);
        }
        let start = self.rows.len().saturating_sub(self.fresh);
        for (minute, v) in self.rows.iter().skip(start) {
            let utc = Time(start_utc.floor() + (*minute as f64 + 1.0) * 60.0);
            table.put(p, &format!("{minute:05}"), &row(utc, *v));
        }
        self.fresh = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_of_one_event() {
        let r = EventRow {
            signal_time_s: 12.5,
            kind: "whistler".into(),
            frequency_hz: 4000.0,
            peak_db: 7.25,
            duration_s: 1.0,
            aux1: 60.0,
            aux2: 0.0,
            note: "a \"quoted\" note".into(),
        };
        assert!(EVENT_CSV_HEADER.starts_with("utc,signal_time_s,"));
        let line = r.csv(1_700_000_000.0);
        assert!(line.starts_with("2023-11-14T22:13:32Z,12.500,whistler,"), "{line}");
        assert!(line.ends_with("\"a 'quoted' note\""), "{line}");
        let t = trend_csv(1_700_000_000.0, ["events_per_min", "sferics_per_min"], &[[1.0, 2.0]]);
        assert_eq!(t.lines().nth(1).unwrap(), "2023-11-14T22:14:20Z,1,2");
        assert_eq!(per_utc_hour([1_700_000_000.0])[22], 1);
    }
}

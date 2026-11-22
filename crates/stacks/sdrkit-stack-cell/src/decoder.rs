//! The capture search that the cell stacks share. Input samples are gathered into captures. Each capture is searched for every
//! cell it holds, and the cells are merged into the stack's tables. A live radio searches on a helper thread. A recording
//! searches in line, so no capture is skipped and the results do not depend on the machine.

use crate::typed::{FoundCellEvent, SearchFields, SEARCH_REMOVE};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{EventKind, Fields, Hertz, Table};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    Clock, Complex32, ConfigError, EventDef, LockState, Record, SettingError, SettingId, SettingSet, StackConfig,
    StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Helper, Item, Kept, Kind, Outbox, Port,
    RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::{Format as StateFormat, LogLine, LogRing, Shift, Stamp};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;

/// The most lines the event log keeps.
pub const LOG_LINES: usize = 300;

/// Input level is measured over this many seconds of input.
const LEVEL_S: f64 = 0.5;

/// The lock state is written again after this many seconds of signal at the latest, so it follows the signal.
const REPORT_S: f64 = 0.5;

/// The least and the most capture length, seconds.
const CAPTURE_RANGE: (f64, f64) = (0.01, 10.0);

/// The capture length setting of a stack whose default capture is `default_s` seconds.
pub const fn capture_setting(default_s: f64) -> sdrkit_core::Setting<field::F64> {
    sdrkit_core::Setting::new("capture_s", "Capture length", "s", default_s)
        .range(CAPTURE_RANGE.0, CAPTURE_RANGE.1, 0.0)
        .help("Seconds of input searched at a time.")
}

/// The band order of a cell stack: the bands that a lookup takes first where the rasters of bands overlap, numbers apart
/// by spaces or commas. Empty keeps band number order.
pub const BAND_ORDER: sdrkit_core::Setting<field::Text> =
    sdrkit_core::Setting::new("band_order", "Band order", "", "").help(
        "Band numbers taken first where bands overlap (\"1 3 7\"); the other bands follow in band number order.",
    );

/// One capture of input samples passed to a search.
#[derive(Clone, Debug, Default)]
pub struct Capture {
    /// The samples.
    pub samples: Vec<Complex32>,
    /// Input rate, Hz.
    pub rate: f64,
    /// The radio's centre frequency, Hz, or 0 when unknown.
    pub centre_hz: f64,
    /// The offset of the user's dial from 0 Hz in the samples, Hz.
    pub dial_offset_hz: f64,
    /// Signal time just after the last sample, seconds.
    pub end_time: f64,
}

impl Capture {
    /// A capture of `samples` at `rate`, centred on `centre_hz` (0: unknown), with no dial offset, ending at signal time `end_time`.
    pub fn new(samples: Vec<Complex32>, rate: f64, centre_hz: f64, end_time: f64) -> Capture {
        Capture {
            samples,
            rate,
            centre_hz,
            dial_offset_hz: 0.0,
            end_time,
        }
    }
    /// Signal time of the first sample, seconds.
    pub fn start_time(&self) -> f64 {
        self.end_time - self.samples.len() as f64 / self.rate.max(1.0)
    }
    /// Whether the centre frequency is known.
    pub fn centre_known(&self) -> bool {
        self.centre_hz != 0.0
    }
}

/// The wall-clock time of signal times. It is known only when the samples are live.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WallClock {
    at_zero: Option<f64>,
}

impl WallClock {
    /// Starts the clock for a run. If `live`, the wall time of signal time `now_s` is the present time of `clock`.
    pub fn attach(&mut self, live: bool, clock: &Clock, now_s: f64) {
        self.at_zero = live.then(|| clock.now() - now_s);
    }
    /// The moment of signal time `t`, with its wall-clock time when it is known.
    pub fn stamp(&self, t: f64) -> Stamp {
        match self.at_zero {
            Some(w) => Stamp::at(t).with_wall(w + t),
            None => Stamp::at(t),
        }
    }
    /// The wall-clock time of signal time `t`, when it is known.
    pub fn wall(&self, t: f64) -> Option<f64> {
        self.at_zero.map(|w| w + t)
    }
}

/// The event log and counters of a stack, with its tables: what a search stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct Held<T> {
    /// The stack's own tables.
    pub tables: T,
    /// The event log, newest last.
    pub log: LogRing<LogLine>,
    /// Messages decoded.
    pub decoded: u64,
    /// Messages that failed their check.
    pub failed: u64,
}

impl<T: Tables> Shift for Held<T> {
    fn shift_times(&mut self, by_s: f64) {
        self.tables.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl<T: Tables> Tables for Held<T> {
    const FORMAT: StateFormat = T::FORMAT;
    fn clear(&mut self) {
        self.tables.clear();
        self.log.clear();
        self.decoded = 0;
        self.failed = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.tables.adopt(saved.tables);
        self.log.adopt(saved.log);
        self.decoded = saved.decoded;
        self.failed = saved.failed;
    }
}

/// A cell found or identified, as it leaves the graph: an event of the stack's kind.
pub struct CellEvent<M> {
    /// The row key of the cell in the `cells` table.
    pub source: String,
    /// What was found, for people.
    pub text: String,
    stack: PhantomData<fn() -> M>,
}

impl<M> Clone for CellEvent<M> {
    fn clone(&self) -> Self {
        CellEvent {
            source: self.source.clone(),
            text: self.text.clone(),
            stack: PhantomData,
        }
    }
}

impl<M: SearchStack> Item for CellEvent<M> {
    const KIND: &'static str = M::EVENT_KIND;
    fn event_def() -> Option<&'static EventDef> {
        Some(M::EVENT.def())
    }
    fn write_fields(&self, fields: &mut Record) {
        FoundCellEvent {
            source: &self.source,
            text: &self.text,
            family: Family::Other,
        }
        .write(fields);
    }
}

/// What a merge writes besides the tables: log lines, events and the decode counters.
pub struct Log<'a, M> {
    ring: &'a mut LogRing<LogLine>,
    pending: &'a mut Vec<String>,
    events: &'a mut Vec<CellEvent<M>>,
    clock: WallClock,
    prefix: &'static str,
    /// Messages decoded.
    pub decoded: &'a mut u64,
    /// Messages that failed their check.
    pub failed: &'a mut u64,
}

impl<M> Log<'_, M> {
    /// Adds a line at signal time `t`. The results log gets the same text after the stack prefix.
    pub fn line(&mut self, t: f64, text: String) {
        self.pending.push(format!("{}{text}", self.prefix));
        self.ring.push(LogLine {
            at: self.clock.stamp(t),
            text,
        });
    }
    /// Adds a line at signal time `t`, and an event of the cell whose row key is `source` with the same text.
    pub fn event(&mut self, t: f64, source: String, text: String) {
        self.events.push(CellEvent {
            source,
            text: text.clone(),
            stack: PhantomData,
        });
        self.line(t, text);
    }
    /// The moment of signal time `t`, with its wall-clock time when it is known.
    pub fn stamp(&self, t: f64) -> Stamp {
        self.clock.stamp(t)
    }
}

/// The headline of a stack's tables after a merge: the status summary and the strongest recent cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Headline {
    /// The summary line ("3 cells, 2 identified, 1 operator").
    pub summary: String,
    /// Signal time of the newest cell, seconds.
    pub newest_s: Option<f64>,
    /// SNR of the strongest recent cell, dB.
    pub snr_db: Option<f32>,
    /// Carrier offset of the strongest recent cell, Hz.
    pub offset_hz: Option<f64>,
}

/// A search stack: what it searches a capture for and how it keeps what it found.
pub trait SearchStack: Send + 'static {
    /// What one capture held.
    type Found: Send;
    /// The tables kept across captures, retunes, changes of frequency and restarts. Only the `clear` command empties
    /// them.
    type Tables: Tables + Send + 'static;
    /// The settings of the search component.
    type Setting: SettingSet;
    /// The registry name of the search component (`"cell.lte_search"`).
    const NAME: &'static str;
    /// The prefix of its lines in the results log (`"cell: "`).
    const PREFIX: &'static str;
    /// The stack id, whose description in [`crate::typed`] the decoder publishes by.
    const ID: &'static str;
    /// The kind of its events (`"cell.lte_cell"`), the key of [`SearchStack::EVENT`].
    const EVENT_KIND: &'static str;
    /// A cell seen within this many seconds of signal makes the lock state show Locked.
    const RECENT_S: f64;
    /// The table of the cells, whose rows the `remove` command forgets.
    const CELLS: &'static TableDef;
    /// The event of a cell found or identified.
    const EVENT: &'static EventKind<FoundCellEvent<'static>>;
    /// Empty tables.
    fn tables(&self) -> Self::Tables;
    /// Applies setting `s` (id `id`). The value has the declared kind and range already.
    ///
    /// # Errors
    ///
    /// A [`SettingError`] if the stack refuses the value.
    fn apply(&mut self, s: Self::Setting, id: SettingId, value: &Value)
        -> Result<(), SettingError>;
    /// The present values of the settings.
    fn values(&self) -> Values;
    /// The capture length, seconds.
    fn capture_s(&self) -> f64;
    /// The input rate is known. Builds what depends on it.
    ///
    /// # Errors
    ///
    /// A [`ConfigError`] for a rate the stack cannot use.
    fn configure(&mut self, rate: f64) -> Result<(), ConfigError> {
        let _ = rate;
        Ok(())
    }
    /// Searches the whole capture.
    fn search(&mut self, cap: &Capture) -> Self::Found;
    /// Merges what a capture held into the tables.
    fn merge(
        &mut self,
        tables: &mut Self::Tables,
        cap: &Capture,
        found: Self::Found,
        log: &mut Log<Self>,
    ) where
        Self: Sized;
    /// Writes the stack's fields and tables at signal time `now`, and returns the headline.
    fn publish(&self, tables: &Self::Tables, now: f64, p: &mut Published) -> Headline;
    /// Forgets the cell whose row key in [`SearchStack::CELLS`] is `row`. Returns `false` if no cell has that row key.
    fn remove(tables: &mut Self::Tables, row: &str) -> bool;
    /// The host changed the frequency: forgets what `tables` holds of the newest capture only. The default forgets
    /// nothing.
    fn forget_capture(tables: &mut Self::Tables) {
        let _ = tables;
    }
}

/// The key of the cell table of every cellular stack.
pub(crate) const CELLS_KEY: &str = "cells";

sdrkit_core::commands! {
    /// The commands of a [`SearchDecoder`].
    pub enum SearchCommand {
        /// Forgets one cell.
        Remove = SEARCH_REMOVE.command,
    }
}

/// The search decoder of a [`SearchStack`], registered as [`SearchStack::NAME`]. Input: IQ samples. Output `cells`: the events
/// of cells found or identified. The tables and the results hold the rest.
pub struct SearchDecoder<M: SearchStack> {
    stack: M,
    held: Held<M::Tables>,
    pending_log: Vec<String>,
    events: Vec<CellEvent<M>>,
    clock: WallClock,
    len: usize,
    cap: Capture,
    next_index: u64,
    started: bool,
    time_s: f64,
    captures: u64,
    retunes: u64,
    last_centre: f64,
    level_acc: f64,
    level_n: usize,
    level_dbfs: Option<f32>,
    headline: Headline,
    /// The tables or settings changed since the last report.
    changed: bool,
    /// The results log must be emptied at the next report.
    cleared: bool,
    /// Signal time of the last report, seconds.
    reported_s: f64,
    /// Signal time of the last change of frequency, seconds: an older cell tells nothing of the present signal.
    forgot_s: f64,
}

impl<M: SearchStack> SearchDecoder<M> {
    /// The decoder of `stack`.
    pub fn new(stack: M) -> Self {
        let held = Held {
            tables: stack.tables(),
            log: LogRing::new(LOG_LINES),
            decoded: 0,
            failed: 0,
        };
        SearchDecoder {
            stack,
            held,
            pending_log: Vec::new(),
            events: Vec::new(),
            clock: WallClock::default(),
            len: 0,
            cap: Capture::default(),
            next_index: 0,
            started: false,
            time_s: 0.0,
            captures: 0,
            retunes: 0,
            last_centre: 0.0,
            level_acc: 0.0,
            level_n: 0,
            level_dbfs: None,
            headline: Headline::default(),
            changed: true,
            cleared: false,
            reported_s: f64::NEG_INFINITY,
            forgot_s: f64::NEG_INFINITY,
        }
    }
    /// The stack.
    pub fn stack(&self) -> &M {
        &self.stack
    }
    /// The tables, the event log and the counters.
    pub fn held(&self) -> &Held<M::Tables> {
        &self.held
    }
    /// Searches a capture and merges what it held, as the decoder does for a full capture.
    pub fn search_capture(&mut self, cap: &Capture) {
        let found = self.stack.search(cap);
        let mut log = Log {
            ring: &mut self.held.log,
            pending: &mut self.pending_log,
            events: &mut self.events,
            clock: self.clock,
            prefix: M::PREFIX,
            decoded: &mut self.held.decoded,
            failed: &mut self.held.failed,
        };
        self.stack
            .merge(&mut self.held.tables, cap, found, &mut log);
        self.captures += 1;
        self.last_centre = cap.centre_hz;
        self.changed = true;
    }

    fn resize(&mut self) {
        let len = (self.cap.rate * self.stack.capture_s()).round() as usize;
        if len != self.len {
            self.len = len;
            self.cap.samples.clear();
            self.cap.samples.reserve(len);
        }
    }

    fn level(&mut self, x: &[Complex32]) {
        let every = ((self.cap.rate * LEVEL_S) as usize).max(1);
        let mut i = 0;
        while i < x.len() {
            let take = (every - self.level_n).min(x.len() - i);
            self.level_acc += x[i..i + take]
                .iter()
                .map(|s| s.norm_sqr() as f64)
                .sum::<f64>();
            self.level_n += take;
            i += take;
            if self.level_n >= every {
                let p = self.level_acc / self.level_n as f64;
                self.level_dbfs = Some(sdrkit_algo_dsp::meter::power_to_db(p) as f32);
                self.level_acc = 0.0;
                self.level_n = 0;
            }
        }
    }

    fn lock_state(&self) -> LockState {
        if !self.started {
            LockState::Off
        } else if self
            .headline
            .newest_s
            .is_some_and(|t| self.time_s - t <= M::RECENT_S)
        {
            LockState::Locked
        } else {
            LockState::Searching
        }
    }
}

impl<M: SearchStack> Decoder for SearchDecoder<M> {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        M::NAME
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<CellEvent<M>>("cells")]
    }
    fn settings(&self) -> Arc<sdrkit_core::Schema> {
        M::Setting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = M::Setting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        self.stack.apply(s, id, value)?;
        self.changed = true;
        self.resize();
        Ok(())
    }
    fn values(&self) -> Values {
        self.stack.values()
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(crate::typed::shape(M::ID).results_decl()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.stack.configure(input.rate)?;
        self.cap.rate = input.rate;
        self.cap.centre_hz = input.centre_hz;
        self.cap.dial_offset_hz = input.tuned_hz - input.centre_hz;
        self.len = 0;
        self.resize();
        self.level_acc = 0.0;
        self.level_n = 0;
        Ok(vec![Format::new(Kind::typed::<CellEvent<M>>(), 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        // Samples of the old frequency never join a capture of the new one.
        self.cap.samples.clear();
        if (input.centre_hz - self.cap.centre_hz).abs() > 1.0 {
            self.retunes += 1;
            self.changed = true;
        }
        self.cap.centre_hz = input.centre_hz;
        self.cap.dial_offset_hz = input.tuned_hz - input.centre_hz;
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        if x.is_empty() || self.cap.rate <= 0.0 {
            return;
        }
        self.started = true;
        // A gap (input dropped while a live search ran) ends the capture being collected.
        if first != self.next_index {
            self.cap.samples.clear();
        }
        self.next_index = first + x.len() as u64;
        self.time_s = self.next_index as f64 / self.cap.rate;
        self.level(x);
        let mut i = 0;
        while i < x.len() && self.len > 0 {
            let n = (self.len - self.cap.samples.len()).min(x.len() - i);
            self.cap.samples.extend_from_slice(&x[i..i + n]);
            i += n;
            if self.cap.samples.len() >= self.len {
                self.cap.end_time = (first + i as u64) as f64 / self.cap.rate;
                let full = std::mem::take(&mut self.cap);
                self.search_capture(&full);
                self.cap = full;
                self.cap.samples.clear();
                let t = self.cap.end_time;
                out.items::<CellEvent<M>>(0)
                    .extend(self.events.drain(..).map(|value| Timed { time: t, value }));
            }
        }
    }
    /// Writes the tables when a capture was merged or a setting changed, and the lock state at least every half second of signal.
    fn report(&mut self, p: &mut Published) {
        if !self.changed && self.time_s - self.reported_s < REPORT_S {
            return;
        }
        self.reported_s = self.time_s;
        if self.changed {
            self.changed = false;
            if self.cleared {
                self.cleared = false;
                p.results_mut().clear_log();
            }
            for line in self.pending_log.drain(..) {
                p.results_mut().log(line);
            }
            self.headline = self.stack.publish(&self.held.tables, self.time_s, p);
            if self.headline.newest_s.is_some_and(|t| t <= self.forgot_s) {
                self.headline.newest_s = None;
                self.headline.snr_db = None;
                self.headline.offset_hz = None;
            }
        }
        p.put(&SearchFields {
            captures: self.captures as i64,
            retunes: self.retunes as i64,
            decoded: self.held.decoded as i64,
            failed: self.held.failed as i64,
            capture_centre: (self.last_centre != 0.0).then_some(Hertz(self.last_centre)),
            level_dbfs: self.level_dbfs.map(f64::from),
        });
        let lock_state = self.lock_state();
        let head = &self.headline;
        let level = self.level_dbfs;
        let started = self.started;
        p.results_mut().update_status(|s| {
            let summary = if started {
                head.summary.as_str()
            } else {
                "waiting for samples"
            };
            let same = s.lock_state == lock_state
                && s.summary == summary
                && s.snr_db == head.snr_db
                && s.offset_hz == head.offset_hz
                && s.level_dbfs == level;
            if !same {
                s.lock_state = lock_state;
                if s.summary != summary {
                    s.summary.clear();
                    s.summary.push_str(summary);
                }
                s.snr_db = head.snr_db;
                s.offset_hz = head.offset_hz;
                s.level_dbfs = level;
            }
            !same
        });
    }
    /// Returns 0, so [`Decoder::report`] is called after every piece of input and decides itself; a merged capture shows at
    /// once (also the last capture of a recording).
    fn report_interval_s(&self) -> f64 {
        0.0
    }
    fn commands(&self) -> &'static [&'static Command] {
        SearchCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SearchCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SearchCommand::Remove => {
                let row = SEARCH_REMOVE.args::<crate::typed::SearchRemoveArgs>(args)?.row;
                if !M::remove(&mut self.held.tables, row) {
                    return Err(self.results().no_such_row(M::CELLS, row));
                }
                self.changed = true;
            }
        }
        Ok(None)
    }
    fn clear(&mut self) {
        self.held.clear();
        self.pending_log.clear();
        self.events.clear();
        self.captures = 0;
        self.changed = true;
        self.cleared = true;
    }
    /// Keeps the tables, the event log and the counts of messages while the stack keeps the cells; forgets the
    /// capture being collected, the level and what the tables hold of the newest capture.
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(CELLS_KEY) {
            self.clear();
        }
        M::forget_capture(&mut self.held.tables);
        self.cap.samples.clear();
        self.captures = 0;
        self.last_centre = 0.0;
        self.level_acc = 0.0;
        self.level_n = 0;
        self.level_dbfs = None;
        if !kept.field("retunes") {
            self.retunes = 0;
        }
        self.forgot_s = self.time_s;
        self.changed = true;
    }
    fn helper(&self) -> Helper {
        Helper::WhenLive {
            batch: self.len,
            backlog: self.len,
        }
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock.attach(ctx.wall_clock(), &ctx.clock, self.time_s);
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.time_s).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.time_s)?;
        self.pending_log.clear();
        for l in self.held.log.iter() {
            self.pending_log.push(format!("{}{}", M::PREFIX, l.text));
        }
        self.changed = true;
        self.cleared = true;
        Ok(())
    }
}

/// "s" for any count but one.
pub fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// The rows of a table that a stack writes in full at each report: the rows it reports are written, and the rows
/// missing from the report are removed when it finishes.
pub struct RowSync<S: 'static> {
    table: &'static Table<S>,
    written: std::collections::HashSet<String>,
}

impl<S: Fields + 'static> RowSync<S> {
    /// A pass over the rows of `table`.
    pub fn new(table: &'static Table<S>) -> Self {
        RowSync {
            table,
            written: std::collections::HashSet::new(),
        }
    }
    /// Writes the row `key` of the table in `p`.
    pub fn put<R: Fields<Static = S>>(&mut self, p: &mut Published, key: String, row: &R) {
        self.table.put(p, &key, row);
        self.written.insert(key);
    }
    /// Removes the rows of the table that this pass did not write.
    pub fn finish(self, p: &mut Published) {
        let Some(id) = p.results().decl().table_id(self.table.key) else {
            return;
        };
        let stale: Vec<String> = p
            .results()
            .table(id)
            .map(|t| {
                t.rows_newest_first()
                    .filter(|(k, _)| !self.written.contains(*k))
                    .map(|(k, _)| k.to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for k in &stale {
            p.remove_row(self.table.def(), k);
        }
    }
}

/// A list of band numbers as a person types it: numbers apart by spaces or commas.
///
/// # Errors
///
/// The byte position of the first character that is not a digit, a comma or a space, or of a number above 65535.
pub fn parse_band_list(text: &str) -> Result<Vec<u16>, usize> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if c.is_ascii_digit() {
            start.get_or_insert(i);
            continue;
        }
        if !(c == ',' || c.is_whitespace()) {
            return Err(i);
        }
        if let Some(s) = start.take() {
            out.push(text[s..i].parse().map_err(|_| s)?);
        }
    }
    Ok(out)
}

/// A band list setting: the text as typed and the bands it names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BandList {
    /// The text as typed.
    pub text: String,
    /// The bands.
    pub bands: Vec<u16>,
}

impl BandList {
    /// Reads the band list `text` of setting `id`.
    ///
    /// # Errors
    ///
    /// [`SettingError::Malformed`] with the position of the first character that is not part of a band list.
    pub fn read(id: SettingId, text: &str) -> Result<BandList, SettingError> {
        let bands =
            parse_band_list(text).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
        Ok(BandList {
            text: text.to_owned(),
            bands,
        })
    }
}

/// The receiver of a search stack: one search component named `search` built from the description of the stack,
/// every setting of the search component routed to it (the test signal settings stay with the generator), the
/// commands `clear` and `remove`, and the settings of `cfg` applied.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build_search<M: SearchStack>(
    stack: M,
    min_rate_hz: f64,
    min_rate_message: &str,
    cfg: &StackConfig,
) -> Result<StackGraph, GraphError> {
    let keys: Vec<String> = M::Setting::schema()
        .settings()
        .iter()
        .map(|s| s.field.key.to_string())
        .collect();
    let graph = GraphBuilder::new()
        .add("search", DecoderComponent::new(SearchDecoder::new(stack)))
        .input("search")
        .status_from("search")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(M::ID), graph)
        .min_rate(min_rate_hz, min_rate_message)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("search", "remove"));
    for k in &keys {
        g = g.route(k, &[&format!("search.{k}")])?;
    }
    g.with_settings(&cfg.settings).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_lists() {
        assert_eq!(parse_band_list("14 28, 64"), Ok(vec![14, 28, 64]));
        assert_eq!(parse_band_list(""), Ok(vec![]));
        assert_eq!(parse_band_list("3 x"), Err(2));
        assert_eq!(parse_band_list("70000"), Err(0));
    }
}

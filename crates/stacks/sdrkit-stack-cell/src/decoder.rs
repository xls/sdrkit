//! The capture search that the cell stacks share. Input samples are gathered into captures. Each capture is searched for every
//! cell it holds, and the cells are merged into the stack's tables. A live radio searches on a helper thread. A recording
//! searches in line, so no capture is skipped and the results do not depend on the machine.

use crate::typed::{BandSearchFields, FoundCellEvent, SearchFields, SEARCH_REMOVE, SEARCH_START, SEARCH_STAY};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{EventKind, Fields, Hertz, Table};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    Clock, Complex32, ConfigError, EventDef, HostRequest, LockState, Record, SettingError, SettingId, SettingSet,
    StackConfig, StateBlob, StateError, Value, Values,
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

/// Below this input level, dBFS, the status says that the signal is too weak at the radio.
const WEAK_DBFS: f32 = -35.0;

/// The time on each step of a band search: at least `least_s`, and a capture of `capture_s` with the median of the wall
/// times `searches` of the newest capture searches.
fn dwell_s(least_s: f64, capture_s: f64, searches: &[f64]) -> f64 {
    let mut t = searches.to_vec();
    t.sort_by(f64::total_cmp);
    let typical = t.get(t.len() / 2).copied().unwrap_or(0.0);
    least_s.max(capture_s + 1.2 * typical + 0.05)
}

/// The capture searches whose wall times set the time on each step of a band search.
const SEARCH_TIMES: usize = 9;

/// The most samples of a live input that wait while the helper thread searches.
const HELPER_BACKLOG: usize = 262_144;

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

sdrkit_core::choice_enum! {
    /// When `cell` and `nr` search their bands.
    #[derive(Default)]
    pub enum SearchMode {
        /// When the tuned frequency shows no cell for a second: one pass over the bands, then the strongest cell found.
        #[default]
        Auto = ("auto", "When nothing is found"),
        /// Without end, staying on no cell: a survey of the bands.
        Survey = ("survey", "Always (survey)"),
        /// Never: the stack stays on the tuned frequency.
        Off = ("off", "Never"),
    }
}

/// When the stack searches its bands.
pub const BAND_SEARCH: sdrkit_core::Setting<field::Choice<SearchMode>> =
    sdrkit_core::Setting::choice("band_search", "Band search", SearchMode::Auto).help(
        "When the stack steps the radio over the raster of its bands: when the tuned frequency shows no cell (one pass, then \
         the strongest cell found), always (a survey), or never.",
    );
/// The bands a band search covers.
pub const SEARCH_BANDS: sdrkit_core::Setting<field::Text> = sdrkit_core::Setting::new("search_bands", "Bands searched", "", "")
    .help(
        "Band numbers to search, in order (\"28 5 3\"). Empty: the bands of the band order (the bands of the country), else \
         the band of the tuned frequency, else the nearest band.",
    );
/// The known carriers that a band search visits first.
pub const SEARCH_FIRST: sdrkit_core::Setting<field::Text> =
    sdrkit_core::Setting::new("search_first", "Searched first", "MHz", "").help(
        "Frequencies of known carriers, MHz, that a band search visits first, each at its own centre (\"884.45 763\"); a \
         frequency outside the bands searched is left out. In auto the search stays on the first cell it finds. Empty: none.",
    );
/// The least time on each radio centre of a band search.
pub const SEARCH_DWELL: sdrkit_core::Setting<field::F64> = sdrkit_core::Setting::new("search_dwell_s", "Time per step", "s", 0.5)
    .range(0.1, 10.0, 0.0)
    .help("The least time on each radio centre of a band search; longer when one capture takes longer to search.");

/// The band search settings of a stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchConfig {
    /// When the stack searches.
    pub mode: SearchMode,
    /// The bands searched; empty for the bands of the band order, else the band of the tuned frequency.
    pub bands: BandList,
    /// The frequencies visited first.
    pub first: FrequencyList,
    /// The least dwell, seconds.
    pub dwell_s: f64,
}

impl SearchConfig {
    /// The defaults of the settings.
    pub fn new() -> SearchConfig {
        SearchConfig {
            mode: SearchMode::Auto,
            bands: BandList::default(),
            first: FrequencyList::default(),
            dwell_s: 0.5,
        }
    }
    /// The value of the setting `band_search`.
    pub fn mode_value(&self) -> Value {
        Value::Choice(sdrkit_core::Choice::index(self.mode))
    }
}

/// The cells seen without a decoded broadcast channel (only their synchronisation signals): such a cell is listed only
/// when a second capture within [`Sightings::WITHIN_S`] shows it again, so a chance peak of noise or a spur of the radio is
/// never listed.
#[derive(Clone, Debug)]
pub struct Sightings<K> {
    seen: Vec<(K, f64)>,
}

impl<K> Default for Sightings<K> {
    fn default() -> Self {
        Sightings { seen: Vec::new() }
    }
}

impl<K: PartialEq + Copy> Sightings<K> {
    /// The longest time between the two sightings, seconds of signal.
    pub const WITHIN_S: f64 = 10.0;
    /// The most sightings kept.
    const CAP: usize = 64;

    /// Notes that a capture ending at signal time `t` showed `key`. True when an earlier capture showed it within
    /// [`Sightings::WITHIN_S`].
    pub fn again(&mut self, key: K, t: f64) -> bool {
        self.seen.retain(|(_, s)| t - *s <= Self::WITHIN_S && *s <= t);
        let before = self.seen.iter().any(|(k, s)| *k == key && *s < t);
        self.seen.retain(|(k, _)| *k != key);
        if self.seen.len() >= Self::CAP {
            self.seen.remove(0);
        }
        self.seen.push((key, t));
        before
    }
}

/// The samples of the old centre that a radio still sends for a short time after it moved (a network radio has them in its
/// buffers). A cell in them shows at the new centre at the offset it had from the old one. A cell of the first captures after
/// a move with the identity and the offset of a cell of the old centre is such a cell, and is left out.
#[derive(Clone, Debug, Default)]
pub struct MoveGuard {
    centre_hz: f64,
    /// Signal time of the first sample at the present centre, seconds.
    since_s: f64,
    /// The cells of the old centre and of the present one: identity and offset from the centre, Hz.
    old: Vec<(u64, f64)>,
    now: Vec<(u64, f64)>,
}

impl MoveGuard {
    /// Captures that start within this many seconds of signal after a move can hold samples of the old centre.
    pub const LATE_S: f64 = 0.5;
    /// The most that the offset of one cell moves between captures, Hz.
    const SAME_HZ: f64 = 20e3;
    /// The most cells kept for each centre.
    const CAP: usize = 64;

    /// Starts the merge of `cap`: a capture at a new centre makes the cells of the present centre the old ones. A move too
    /// small to tell a cell of the old centre from one of the new keeps the cells as they are.
    pub fn begin(&mut self, cap: &Capture) {
        let moved = (cap.centre_hz - self.centre_hz).abs();
        if moved > 2.0 * Self::SAME_HZ {
            self.old = std::mem::take(&mut self.now);
            self.since_s = cap.start_time();
        }
        self.centre_hz = cap.centre_hz;
    }

    /// Whether cell `id` at `offset_hz` from the centre of `cap` comes from samples of the old centre. A cell that does not
    /// is noted as a cell of the present centre.
    pub fn late(&mut self, cap: &Capture, id: u64, offset_hz: f64) -> bool {
        let same = |c: &(u64, f64)| c.0 == id && (c.1 - offset_hz).abs() <= Self::SAME_HZ;
        if cap.centre_known() && cap.start_time() < self.since_s + Self::LATE_S && self.old.iter().any(same) {
            return true;
        }
        if !self.now.iter().any(same) && self.now.len() < Self::CAP {
            self.now.push((id, offset_hz));
        }
        false
    }
}

/// Why a band search command was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchError {
    /// The stack has had no input yet, so the rate and the frequency are not known.
    NoInput,
    /// No band of the setting `search_bands`, or of the tuned frequency, has a step at the input rate.
    NoSteps,
    /// No cell has been found yet.
    NoCell,
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SearchError::NoInput => "the stack has had no input yet",
            SearchError::NoSteps => {
                "no band of the setting search_bands, or of the tuned frequency, has a step at this sample rate"
            }
            SearchError::NoCell => "no cell has been found yet",
        })
    }
}

impl std::error::Error for SearchError {}

impl From<SearchError> for CommandError {
    fn from(e: SearchError) -> CommandError {
        CommandError::Failed {
            source: sdrkit_core::StageError::new(e),
        }
    }
}

/// A cell of a stack's tables that a band search can stay on.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundCarrier {
    /// The row key in the cells table.
    pub row: String,
    /// The carrier frequency (LTE) or the SSB frequency (NR), Hz.
    pub frequency_hz: f64,
    /// The level, dBFS.
    pub level_dbfs: f64,
    /// The cell as the status names it ("PCI 301").
    pub name: String,
    /// Signal time when the cell was last seen, seconds.
    pub last_s: f64,
    /// The operator of the cell was read.
    pub identified: bool,
}

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
    /// The cells that a capture's result holds. The band search counts captures without one.
    fn found_cells(found: &Self::Found) -> usize {
        let _ = found;
        0
    }
    /// Why the newest capture showed no cell, for people ("no LTE carrier on this frequency"). None when it showed one or
    /// the stack does not say.
    fn quiet_reason(&self) -> Option<String> {
        None
    }
    /// Why the cells of the newest capture are read only in part, for people ("PCI 5 is a 20 MHz carrier: its operator
    /// needs about 18 Msps"). None when nothing is missing for that reason.
    fn partial_reason(&self) -> Option<String> {
        None
    }
    /// The carrier centre and the sample rate, Hz, that would read what the newest capture missed.
    fn rate_wanted(&self) -> Option<(f64, f64)> {
        None
    }
    /// The band search settings. None for a stack without a band search.
    fn band_search(&self) -> Option<&SearchConfig> {
        None
    }
    /// The radio centres of a search of band `band` at `rate`, lowest first.
    fn search_steps(&self, band: u16, rate: f64) -> Vec<f64> {
        let _ = (band, rate);
        Vec::new()
    }
    /// The band order that the host set (the bands of its country), first band first; empty when none.
    fn band_order(&self) -> &[u16] {
        &[]
    }
    /// The bands whose downlink holds `hz`, those of the band order first; else the nearest band.
    fn bands_at(&self, hz: f64) -> Vec<u16> {
        let _ = hz;
        Vec::new()
    }
    /// The name of band `band` in a status line ("band 28", "n78").
    fn band_name(band: u16) -> String {
        format!("band {band}")
    }
    /// The cells of `tables` a band search can stay on.
    fn carriers(tables: &Self::Tables) -> Vec<FoundCarrier> {
        let _ = tables;
        Vec::new()
    }
}

/// A band search that runs: the radio centres in order, the band of each, where the radio is and the passes done.
#[derive(Clone, Debug, PartialEq)]
struct Pass {
    steps: Vec<(f64, u16)>,
    /// The steps that cover the bands, without the visits of found carriers.
    base: Vec<(f64, u16)>,
    at: Option<usize>,
    passes: u32,
    dwell_s: f64,
    /// Signal time of the start of the newest pass, seconds.
    started_s: f64,
    /// Signal time of the newest move of the radio (or of the start), seconds.
    moved_s: f64,
}

/// A search whose radio does not move for this many dwells ends: the host refused or ended the hop.
const STUCK_DWELLS: f64 = 4.0;
/// After a refused search, a search does not start by itself for this many seconds of signal.
const REFUSED_HOLD_S: f64 = 30.0;

/// Whether the samples `x` use so few levels of the radio's converter that a weak signal is lost in its steps: fewer than 16
/// different values of the real part in up to 4096 samples.
/// `v` is the work space, which keeps its room of 4096 values.
fn coarse(x: &[Complex32], v: &mut Vec<u32>) -> bool {
    if x.len() < 1024 {
        return false;
    }
    v.clear();
    v.extend(x.iter().take(4096).map(|s| s.re.to_bits()));
    v.sort_unstable();
    v.dedup();
    v.len() < 16
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

sdrkit_core::commands! {
    /// The commands of a [`SearchDecoder`] of a stack with a band search.
    pub enum BandSearchCommand {
        /// Forgets one cell.
        Remove = SEARCH_REMOVE.command,
        /// Starts a band search.
        Search = SEARCH_START,
        /// Ends a band search on one cell.
        Stay = SEARCH_STAY.command,
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
    /// The band search that runs.
    pass: Option<Pass>,
    /// Requests to the host that wait for the next input.
    requests: Vec<HostRequest>,
    /// Captures in a row without a cell since the last change of frequency.
    quiet: u32,
    /// The frequency where the stack listens, Hz.
    tuned_hz: f64,
    /// The frequency of a cell the stack asked to stay on, Hz.
    staying_hz: Option<f64>,
    /// The wall times of the newest capture searches, seconds, oldest first.
    search_times: Vec<f64>,
    /// The samples use too few levels of the converter.
    coarse: bool,
    /// The work space of the test of the converter levels.
    levels: Vec<u32>,
    /// The samples are live, so the host can move the radio: a band search starts by itself only then.
    live: bool,
    /// Signal time when the host last left a band search without moving the radio, seconds.
    refused_s: Option<f64>,
    /// The cell the band search stayed on, as the status names it.
    stayed: Option<String>,
    /// The carrier centre and the rate the stack asked the host for, Hz.
    asked_rate: Option<(f64, f64)>,
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
            pass: None,
            requests: Vec::new(),
            quiet: 0,
            tuned_hz: 0.0,
            staying_hz: None,
            search_times: Vec::with_capacity(SEARCH_TIMES),
            coarse: false,
            levels: Vec::with_capacity(4096),
            live: false,
            refused_s: None,
            stayed: None,
            asked_rate: None,
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
        let t0 = std::time::Instant::now();
        let found = self.stack.search(cap);
        if self.search_times.len() == SEARCH_TIMES {
            self.search_times.remove(0);
        }
        self.search_times.push(t0.elapsed().as_secs_f64());
        if M::found_cells(&found) > 0 {
            self.quiet = 0;
        } else {
            self.quiet = self.quiet.saturating_add(1);
        }
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
        self.after_capture(cap.rate);
    }

    /// The band search after a capture: starts one when the setting asks for it and the samples are live. A recording
    /// cannot be retuned; the command `search` still asks the host.
    fn after_capture(&mut self, rate: f64) {
        let Some(mode) = self.stack.band_search().map(|c| c.mode) else { return };
        // A carrier wider than the capture: in auto, on a live radio, the stack asks once for the rate and the centre
        // that read its system information.
        if let Some((hz, want)) = self.stack.rate_wanted().filter(|(_, r)| *r > rate) {
            let asked = self.asked_rate.is_some_and(|(h, r)| (h - hz).abs() < 50e3 && r >= want);
            if mode == SearchMode::Auto && self.live && !asked {
                self.asked_rate = Some((hz, want));
                self.pass = None;
                self.staying_hz = Some(hz);
                self.requests.push(HostRequest::Tune {
                    session: String::new(),
                    freq_hz: Some(hz),
                    target: sdrkit_core::TuneTarget::Running,
                    submode: String::new(),
                    rate_hz: Some(want),
                    bandwidth_hz: None,
                    settings: Vec::new(),
                });
                self.changed = true;
                return;
            }
        }
        if let Some(since) = self.pass.as_ref().map(|p| p.started_s) {
            // A cell found in auto: the search ends on the strongest cell of this pass.
            if mode == SearchMode::Auto && self.quiet == 0 && self.stay("", since) {
                return;
            }
        }
        if let Some(p) = &self.pass {
            if self.time_s - p.moved_s > STUCK_DWELLS * p.dwell_s + self.stack.capture_s() {
                // The host refused the hop or ended it: the search ends, and says so.
                self.pass = None;
                self.refused_s = Some(self.time_s);
                self.changed = true;
            }
            return;
        }
        if self.tuned_hz <= 0.0 || !self.live {
            return;
        }
        if self.refused_s.is_some_and(|t| self.time_s - t < REFUSED_HOLD_S) {
            return;
        }
        let quiet_for = (1.0 / self.stack.capture_s().max(0.01)).ceil().max(2.0) as u32;
        let start = match mode {
            SearchMode::Off => false,
            SearchMode::Survey => true,
            SearchMode::Auto => self.quiet >= quiet_for,
        };
        if start {
            self.start_search(rate);
        }
    }

    /// Starts a band search at `rate`: the bands of the setting, else those of the band order, else the band of the tuned
    /// frequency. A search whose bands fit in one capture tunes to its one centre. Returns false when no band holds a step.
    fn start_search(&mut self, rate: f64) -> bool {
        let Some(cfg) = self.stack.band_search() else { return false };
        let bands: Vec<u16> = if !cfg.bands.bands.is_empty() {
            cfg.bands.bands.clone()
        } else if !self.stack.band_order().is_empty() {
            self.stack.band_order().to_vec()
        } else {
            self.stack.bands_at(self.tuned_hz).into_iter().take(1).collect()
        };
        let mut base: Vec<(f64, u16)> = Vec::new();
        for b in bands {
            for c in self.stack.search_steps(b, rate) {
                if !base.iter().any(|(x, _)| (x - c).abs() < 1.0) {
                    base.push((c, b));
                }
            }
        }
        // The known carriers first, each at its own centre: those inside the capture of a step of the bands searched.
        let mut steps: Vec<(f64, u16)> = Vec::new();
        for &hz in &cfg.first.hz {
            let near = base.iter().find(|(c, _)| (c - hz).abs() < rate / 2.0);
            if let Some(&(_, b)) = near {
                if !steps.iter().any(|(x, _)| (x - hz).abs() < 1.0) {
                    steps.push((hz, b));
                }
            }
        }
        base.retain(|(c, _)| !steps.iter().any(|(x, _)| (x - c).abs() < 1.0));
        steps.extend(base);
        let dwell_s = self.dwell_s(cfg.dwell_s);
        self.staying_hz = None;
        self.stayed = None;
        self.quiet = 0;
        self.refused_s = None;
        self.changed = true;
        match steps.len() {
            0 => {
                self.pass = None;
                false
            }
            1 => {
                // One capture covers the band: listen there.
                self.pass = None;
                if (steps[0].0 - self.tuned_hz).abs() > 1.0 {
                    self.staying_hz = Some(steps[0].0);
                    self.requests
                        .push(HostRequest::retune(steps[0].0, "band search: one capture covers the band"));
                }
                true
            }
            _ => {
                self.requests.push(HostRequest::Hop {
                    channels: steps.iter().map(|s| s.0).collect(),
                    dwell_s,
                    stop_on_lock: false,
                });
                self.pass = Some(Pass {
                    base: steps.clone(),
                    steps,
                    at: None,
                    passes: 0,
                    dwell_s,
                    started_s: self.time_s,
                    moved_s: self.time_s,
                });
                true
            }
        }
    }

    /// The time on each step of a band search: at least `least_s`, and the time to collect a capture and search it. The
    /// search time is the median of the newest searches: a capture with a cell to decode takes longer than one of the empty
    /// steps that most of a search visits, and does not set the time of all of them.
    fn dwell_s(&self, least_s: f64) -> f64 {
        dwell_s(least_s, self.stack.capture_s(), &self.search_times)
    }

    /// Ends the band search on the cell of row `row` of the cells table, or, when `row` is empty, on the strongest cell
    /// seen since signal time `since_s`. Returns false when no such cell is known.
    fn stay(&mut self, row: &str, since_s: f64) -> bool {
        let cells = M::carriers(&self.held.tables);
        let pick = if row.is_empty() {
            cells
                .iter()
                .filter(|c| c.last_s >= since_s)
                .max_by(|a, b| a.level_dbfs.total_cmp(&b.level_dbfs))
        } else {
            cells.iter().find(|c| c.row == row)
        };
        let Some(c) = pick else { return false };
        self.pass = None;
        self.quiet = 0;
        self.staying_hz = Some(c.frequency_hz);
        self.stayed = Some(c.name.clone());
        self.requests.push(HostRequest::retune(
            c.frequency_hz,
            &format!("band search: stays on {}", c.name),
        ));
        self.changed = true;
        true
    }

    /// A survey after a pass: the next pass visits first each carrier of this pass whose operator is not read, at its own
    /// centre, so that a capture holds the carrier's centre and its system information can be read. The time on each step
    /// follows the searches of the pass.
    fn visit_found(&mut self, since_s: f64) {
        let dwell_s = self.dwell_s(self.stack.band_search().map_or(0.0, |c| c.dwell_s));
        let Some(pass) = self.pass.as_mut() else { return };
        let band_near = |hz: f64| {
            pass.base
                .iter()
                .min_by(|a, b| (a.0 - hz).abs().total_cmp(&(b.0 - hz).abs()))
                .map_or(0, |s| s.1)
        };
        let mut visits: Vec<(f64, u16)> = Vec::new();
        for c in M::carriers(&self.held.tables) {
            if c.identified || c.last_s < since_s || visits.iter().any(|v| (v.0 - c.frequency_hz).abs() < 50e3) {
                continue;
            }
            visits.push((c.frequency_hz, band_near(c.frequency_hz)));
        }
        let steps: Vec<(f64, u16)> = visits
            .into_iter()
            .chain(pass.base.iter().copied())
            .collect();
        // The time on each step follows the searches of the pass that ended.
        let same_dwell = (dwell_s - pass.dwell_s).abs() <= 0.2 * pass.dwell_s;
        if steps == pass.steps && same_dwell {
            return;
        }
        pass.steps = steps;
        pass.at = None;
        pass.dwell_s = dwell_s;
        self.requests.push(HostRequest::Hop {
            channels: pass.steps.iter().map(|s| s.0).collect(),
            dwell_s: pass.dwell_s,
            stop_on_lock: false,
        });
    }

    /// The band search at a new frequency `tuned`: the step it is at, the end of a pass, or a frequency of someone else,
    /// which ends the search.
    fn search_retuned(&mut self, tuned: f64) {
        self.tuned_hz = tuned;
        self.quiet = 0;
        if self.staying_hz.is_some_and(|h| (h - tuned).abs() <= 1.0) {
            return;
        }
        self.staying_hz = None;
        self.stayed = None;
        let Some(pass) = self.pass.as_mut() else { return };
        let Some(i) = pass.steps.iter().position(|(c, _)| (c - tuned).abs() <= 1.0) else {
            // Someone else tuned the radio: the search ends.
            self.pass = None;
            return;
        };
        // A pass ends at a move back to an earlier step; a second report of the same step is no move.
        if pass.at == Some(i) {
            return;
        }
        let wrapped = pass.at.is_some_and(|a| i < a);
        pass.at = Some(i);
        pass.moved_s = self.time_s;
        if wrapped {
            let since = pass.started_s;
            pass.passes += 1;
            pass.started_s = self.time_s;
            let mode = self.stack.band_search().map(|c| c.mode);
            // Only a cell seen in this pass: an older row of the tables tells nothing of where the radio can hear one now.
            if mode == Some(SearchMode::Auto) {
                self.stay("", since);
            } else if mode == Some(SearchMode::Survey) {
                self.visit_found(since);
            }
        }
    }

    /// The status line: where the band search is, or why nothing shows, before the stack's own summary.
    fn status_text(&self, summary: &str, recent: bool) -> String {
        if let Some(p) = &self.pass {
            let Some(i) = p.at else {
                return format!(
                    "Searching {}: {} steps, waiting for the radio to move; {summary}",
                    M::band_name(p.steps[0].1),
                    p.steps.len()
                );
            };
            let (at, band) = (i + 1, p.steps[i].1);
            let pass = if p.passes > 0 {
                format!(", pass {}", p.passes + 1)
            } else {
                String::new()
            };
            let mhz = format!(" at {:.1} MHz", p.steps[i].0 / 1e6);
            let weak = self.weak_text().map_or(String::new(), |w| format!("; {w}"));
            return format!(
                "Searching {}: step {at} of {}{mhz}{pass}; {summary}{weak}",
                M::band_name(band),
                p.steps.len()
            );
        }
        if recent {
            let partial = self.stack.partial_reason().map_or(String::new(), |p| format!(": {p}"));
            return match &self.stayed {
                Some(c) => format!("Staying on {c}; {summary}{partial}"),
                None => format!("{summary}{partial}"),
            };
        }
        match self.quiet_text() {
            Some(r) => format!("{summary}: {r}"),
            None => summary.to_owned(),
        }
    }

    /// Why no cell shows: a signal too weak for the radio's converter first, then what the stack says of the newest capture.
    fn quiet_text(&self) -> Option<String> {
        if self.refused_s.is_some_and(|t| self.time_s - t < REFUSED_HOLD_S) {
            return Some(
                "the radio did not move for the band search: the frequency is locked, or a step lies outside what the radio \
                 tunes; set search_bands to bands the radio reaches"
                    .to_owned(),
            );
        }
        self.weak_text().or_else(|| self.stack.quiet_reason())
    }

    /// The text of a signal too weak at the radio: samples in a few levels of the converter, or a level below
    /// [`WEAK_DBFS`].
    fn weak_text(&self) -> Option<String> {
        if self.coarse {
            return Some(
                "the signal is too weak at the radio: its samples use only a few levels of the converter; raise the gain, \
                 or use a better antenna"
                    .to_owned(),
            );
        }
        self.level_dbfs.filter(|l| *l < WEAK_DBFS).map(|l| {
            format!(
                "the signal is weak at the radio ({l:.1} dBFS, below {WEAK_DBFS:.0} dBFS): raise the gain, or use a better \
                 antenna"
            )
        })
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
                self.coarse = coarse(&x[i.saturating_sub(take)..i], &mut self.levels);
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
        vec![
            Port::typed::<CellEvent<M>>("cells"),
            Port::typed::<HostRequest>("requests").rate_from(sdrkit_graph::RateFrom::Fixed(0.0)),
        ]
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
        // A new rate gives new search steps: a search that ran starts again.
        let ran = self.pass.take().is_some();
        self.tuned_hz = input.tuned_hz;
        self.quiet = 0;
        if ran {
            self.start_search(input.rate);
        }
        Ok(vec![
            Format::new(Kind::typed::<CellEvent<M>>(), 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
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
        self.search_retuned(input.tuned_hz);
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        if x.is_empty() || self.cap.rate <= 0.0 {
            return;
        }
        self.started = true;
        if !self.requests.is_empty() {
            let t = first as f64 / self.cap.rate;
            out.items::<HostRequest>(1)
                .extend(self.requests.drain(..).map(|value| Timed { time: t, value }));
        }
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
        let recent = lock_state == LockState::Locked;
        let quiet = if recent {
            self.stack.partial_reason()
        } else {
            self.quiet_text()
        };
        if self.stack.band_search().is_some() {
            let band = self.pass.as_ref().map(|p| M::band_name(p.steps[p.at.unwrap_or(0)].1));
            p.put(&BandSearchFields {
                search_band: band.as_deref(),
                search_step: self.pass.as_ref().and_then(|p| p.at).map(|i| i as i64 + 1),
                search_steps: self.pass.as_ref().map(|p| p.steps.len() as i64),
                search_passes: self.pass.as_ref().map_or(0, |p| i64::from(p.passes)),
                quiet_reason: quiet.as_deref(),
            });
        }
        let text = self.status_text(&self.headline.summary, recent);
        let head = &self.headline;
        let level = self.level_dbfs;
        let started = self.started;
        p.results_mut().update_status(|s| {
            let summary = if started {
                text.as_str()
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
        if self.stack.band_search().is_some() {
            BandSearchCommand::COMMANDS
        } else {
            SearchCommand::COMMANDS
        }
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let remove = |d: &mut Self, args: &Values| -> Result<Option<Record>, CommandError> {
            let row = SEARCH_REMOVE.args::<crate::typed::SearchRemoveArgs>(args)?.row;
            if !M::remove(&mut d.held.tables, row) {
                return Err(d.results().no_such_row(M::CELLS, row));
            }
            d.changed = true;
            Ok(None)
        };
        if self.stack.band_search().is_none() {
            return match SearchCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
                SearchCommand::Remove => remove(self, args),
            };
        }
        match BandSearchCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BandSearchCommand::Remove => remove(self, args),
            BandSearchCommand::Search => {
                if self.cap.rate <= 0.0 || self.tuned_hz <= 0.0 {
                    return Err(SearchError::NoInput.into());
                }
                if !self.start_search(self.cap.rate) {
                    return Err(SearchError::NoSteps.into());
                }
                Ok(None)
            }
            BandSearchCommand::Stay => {
                let row = SEARCH_STAY.args::<crate::typed::SearchStayArgs>(args)?.row.to_owned();
                if !self.stay(&row, f64::NEG_INFINITY) {
                    return Err(if row.is_empty() {
                        SearchError::NoCell.into()
                    } else {
                        self.results().no_such_row(M::CELLS, &row)
                    });
                }
                Ok(None)
            }
        }
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
    /// Batches of what has arrived: the decoder collects the capture itself, and while it searches only the newest
    /// [`HELPER_BACKLOG`] samples wait, so the samples that a slow search leaves out cost little copying on the thread of
    /// the input. A batch of what has arrived keeps up with input in pieces of any size.
    fn helper(&self) -> Helper {
        Helper::WhenLive {
            batch: 0,
            backlog: HELPER_BACKLOG,
        }
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock.attach(ctx.wall_clock(), &ctx.clock, self.time_s);
        self.live = ctx.live;
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

/// A list of frequencies in MHz: the text as typed and the frequencies, Hz.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrequencyList {
    /// The text as typed.
    pub text: String,
    /// The frequencies, Hz.
    pub hz: Vec<f64>,
}

impl FrequencyList {
    /// Reads the list `text` of setting `id`: numbers in MHz apart by spaces or commas.
    ///
    /// # Errors
    ///
    /// [`SettingError::Malformed`] with the position of the first word that is not a positive number.
    pub fn read(id: SettingId, text: &str) -> Result<FrequencyList, SettingError> {
        let mut hz = Vec::new();
        for (at, word) in text.split(|c: char| c == ',' || c.is_whitespace()).scan(0, |at, w| {
            let here = *at;
            *at += w.len() + 1;
            Some((here, w))
        }) {
            if word.is_empty() {
                continue;
            }
            match word.parse::<f64>() {
                Ok(mhz) if mhz.is_finite() && mhz > 0.0 => hz.push(mhz * 1e6),
                _ => return Err(SettingError::Malformed { id, at: Some(at) }),
            }
        }
        Ok(FrequencyList {
            text: text.to_owned(),
            hz,
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
    let band_search = stack.band_search().is_some();
    let keys: Vec<String> = M::Setting::schema()
        .settings()
        .iter()
        .map(|s| s.field.key.to_string())
        .collect();
    let graph = GraphBuilder::new()
        .add("search", DecoderComponent::new(SearchDecoder::new(stack)))
        .input("search")
        .output("search.requests")
        .status_from("search")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g = StackGraph::of(crate::typed::shape(M::ID), graph)
        .min_rate(min_rate_hz, min_rate_message)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("remove", sdrkit_graph::CommandTarget::of("search", "remove"));
    if band_search {
        g = g
            .command("search", sdrkit_graph::CommandTarget::of("search", "search"))
            .command("stay", sdrkit_graph::CommandTarget::of("search", "stay"));
    }
    for k in &keys {
        g = g.route(k, &[&format!("search.{k}")])?;
    }
    g.with_settings(&cfg.settings).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_capture_does_not_set_the_time_of_each_step() {
        // Eight searches of empty steps and one of a step with a cell to decode.
        let mut t = vec![0.05; 8];
        t.insert(3, 3.0);
        assert!((dwell_s(0.5, 0.08, &t) - 0.5).abs() < 1e-9);
        // Searches that are slow throughout lengthen it.
        assert!((dwell_s(0.5, 0.08, &[1.0; 9]) - 1.33).abs() < 1e-9);
        assert_eq!(dwell_s(0.5, 0.08, &[]), 0.5);
    }

    #[test]
    fn frequency_lists() {
        let id = SettingId(0);
        assert_eq!(FrequencyList::read(id, "884.45, 763").unwrap().hz, [884.45e6, 763e6]);
        assert!(FrequencyList::read(id, "").unwrap().hz.is_empty());
        assert_eq!(
            FrequencyList::read(id, "763 x").unwrap_err(),
            SettingError::Malformed { id, at: Some(4) }
        );
    }

    #[test]
    fn band_lists() {
        assert_eq!(parse_band_list("14 28, 64"), Ok(vec![14, 28, 64]));
        assert_eq!(parse_band_list(""), Ok(vec![]));
        assert_eq!(parse_band_list("3 x"), Err(2));
        assert_eq!(parse_band_list("70000"), Err(0));
    }
}

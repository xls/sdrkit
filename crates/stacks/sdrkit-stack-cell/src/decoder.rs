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

/// The time on each step of a band search: a first look of `look_s` and its search, whose time is the median of the wall
/// times `searches`, with a margin. A search that takes longer ends while the radio moves, or in the first look of the next
/// step, which the newest samples of the helper thread hold.
fn dwell_s(look_s: f64, searches: &[f64]) -> f64 {
    let mut t = searches.to_vec();
    t.sort_by(f64::total_cmp);
    let typical = t.get(t.len() / 2).copied().unwrap_or(0.0);
    look_s + typical + 0.03
}

/// The capture searches whose wall times stand for a search before a band search has its own.
const SEARCH_TIMES: usize = 9;
/// The searches of first looks whose wall times set the time on each step: few, so that the time follows the cost of each
/// band.
const LOOK_TIMES: usize = 3;
/// A step time this far from the one of the hop that runs starts the hop again with the new time.
const DWELL_CHANGE: f64 = 0.25;

/// The most input, seconds, that waits while the helper thread searches: twice what a bounded search of a capture takes
/// at most (one that reads the system information of two cells), so that a busy machine does not drop input.
const HELPER_BACKLOG_S: f64 = 0.5;

/// While a band search steps, the most samples that waited during a search that are taken: older ones belong to a look
/// that is over, and are left out by design.
const PASS_BACKLOG: usize = 262_144;

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
        /// When the tuned frequency shows no cell whose operator can be read at this rate: after one look without a sync
        /// signal, or after search_dwell_s with one, until such a cell is found, else one pass and the strongest cell found.
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
        "When the stack steps the radio over the raster of its bands: when the tuned frequency shows no cell whose operator \
         can be read at this rate (until it finds one, else one pass and then the strongest cell found), always (a survey), \
         or never.",
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
         frequency outside the bands searched is left out. In auto the search stays on the first cell it finds whose operator \
         can be read at this rate. Empty: none.",
    );
/// The frequency ranges that a band search keeps to.
pub const SEARCH_RANGES: sdrkit_core::Setting<field::Text> =
    sdrkit_core::Setting::new("search_ranges", "Ranges searched", "MHz", "").help(
        "Frequency ranges, MHz, that a band search keeps to in the bands they overlap (\"3400-3800 758-803\"); a band that no \
         range overlaps is searched whole. Empty: every band whole.",
    );
/// The default longest time on one step, seconds: two tries of a system information block that failed.
const DWELL_DEFAULT_S: f64 = 2.0;
/// The longest time on one radio centre of a band search.
pub const SEARCH_DWELL: sdrkit_core::Setting<field::F64> =
    sdrkit_core::Setting::new("search_dwell_s", "Longest time per step", "s", DWELL_DEFAULT_S)
        .range(0.1, 10.0, 0.0)
        .help(
            "The longest time on one radio centre of a band search. A centre whose first look shows a sync signal is searched \
             until its cells are read, or this time has passed.",
        );
/// The first look on each radio centre of a band search, for a stack whose look is `default_s` seconds.
pub const fn look_setting(default_s: f64) -> sdrkit_core::Setting<field::F64> {
    sdrkit_core::Setting::new("search_look_s", "First look", "s", default_s)
        .range(0.02, 1.0, 0.0)
        .help(
            "Seconds of signal searched on each radio centre of a band search. A centre that shows a sync signal is searched \
             longer, up to search_dwell_s.",
        )
}

/// The band search settings of a stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchConfig {
    /// When the stack searches.
    pub mode: SearchMode,
    /// The bands searched; empty for the bands of the band order, else the band of the tuned frequency.
    pub bands: BandList,
    /// The frequencies visited first.
    pub first: FrequencyList,
    /// The ranges searched.
    pub ranges: RangeList,
    /// The longest time on one step, seconds.
    pub dwell_s: f64,
    /// The first look on each step, seconds.
    pub look_s: f64,
}

impl SearchConfig {
    /// The defaults of the settings, with a first look of `look_s` seconds.
    pub fn new(look_s: f64) -> SearchConfig {
        SearchConfig {
            mode: SearchMode::Auto,
            bands: BandList::default(),
            first: FrequencyList::default(),
            ranges: RangeList::default(),
            dwell_s: DWELL_DEFAULT_S,
            look_s,
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
    /// Nothing more of the cell can be read at this centre and rate: its operator was read, or it sends none that this
    /// capture holds.
    pub settled: bool,
    /// Nothing more of the cell can be read at this rate at any centre: a visit at its own centre reads nothing more.
    pub out_of_reach: bool,
    /// The sample rate that reads the operator of the cell with the radio on it, Hz, when the rate of the newest capture
    /// does not. None when that rate does, or when it is not known.
    pub rate_needed_hz: Option<f64>,
}

impl FoundCarrier {
    /// Whether the operator of the cell is read, or is not out of reach of this rate with the radio on the cell.
    pub fn readable(&self) -> bool {
        self.identified || self.rate_needed_hz.is_none()
    }
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
    /// Whether the next searches may follow the cells found with a bounded share of the capture's work, so that a live
    /// input is searched as fast as it arrives. The decoder sets it for live input while no band search steps. The default
    /// ignores it.
    fn bound_search(&mut self, on: bool) {
        let _ = on;
    }
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
    /// Whether a capture's result shows a sync signal over the threshold or a cell: a step of a band search where it shows
    /// is searched longer.
    fn sync_seen(&self, found: &Self::Found) -> bool {
        Self::found_cells(found) > 0
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
    /// The radio centres of a search of band `band` at `rate`, lowest first, without the places that a centre of `taken`
    /// (the steps of the bands before) searches.
    fn search_steps(&self, band: u16, rate: f64, taken: &[f64]) -> Vec<f64> {
        let _ = (band, rate, taken);
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
    /// The time on each step of the hop that runs, seconds.
    dwell_s: f64,
    /// Signal time of the start of the newest pass, seconds.
    started_s: f64,
    /// Signal time of the start of the pass before the newest, seconds; the newest pass's start in the first pass.
    before_s: f64,
    /// Signal time of the newest move of the radio (or of the newest hop asked for), seconds.
    moved_s: f64,
    /// The steps whose first look was searched in this pass.
    searched: Vec<bool>,
    /// The step where the hop that runs starts: the hop visits the steps in turn from there.
    from: usize,
    /// The step searched longer, and the signal time when its first look began, seconds.
    longer: Option<(usize, f64)>,
    /// The steps that visit a carrier found in the pass before at its own centre, before the steps of the bands.
    added: usize,
    /// A hop was asked for since the radio last moved: the next move goes to step `from`.
    fresh: bool,
}

impl Pass {
    /// A pass over `steps` with `dwell_s` on each, from the first step, starting at signal time `now`.
    fn new(steps: Vec<(f64, u16)>, base: Vec<(f64, u16)>, dwell_s: f64, now: f64) -> Pass {
        Pass {
            searched: vec![false; steps.len()],
            steps,
            base,
            at: None,
            passes: 0,
            dwell_s,
            started_s: now,
            before_s: now,
            moved_s: now,
            from: 0,
            longer: None,
            added: 0,
            fresh: true,
        }
    }
    /// The step of the pass, counted from 1: the steps searched in this pass and the one that waits. It grows within a pass
    /// whatever order the radio visits the steps in.
    fn step(&self) -> usize {
        (self.searched.iter().filter(|s| **s).count() + 1).min(self.steps.len())
    }
    /// What the status says of the steps: their count and, when the pass visits carriers found before, how many.
    fn steps_text(&self) -> String {
        match self.added {
            0 => String::new(),
            n => format!(" ({} steps: {n} carrier{} added)", self.steps.len(), plural(n)),
        }
    }
    /// The steps in the order the hop visits them: from `from` on, in turn.
    fn order(&self) -> impl Iterator<Item = usize> + '_ {
        let n = self.steps.len();
        (0..n).map(move |k| (self.from + k) % n)
    }
    /// The first step in the order of the hop whose first look was not searched.
    fn first_waiting(&self) -> Option<usize> {
        self.order().find(|&j| !self.searched[j])
    }
    /// Whether the hop passed a step before step `i` without a searched first look: the search took longer than the step.
    fn skipped_before(&self, i: usize) -> bool {
        self.order()
            .take_while(|&j| j != i)
            .any(|j| !self.searched[j])
    }
}

/// A search whose radio does not move for this many dwells ends: the host refused or ended the hop.
const STUCK_DWELLS: f64 = 4.0;
/// After a refused search, a search does not start by itself for this many seconds of signal.
const REFUSED_HOLD_S: f64 = 30.0;
/// The seconds of signal that the host has to give a rate that the stack asked for, before the stack takes it as refused.
const RATE_WAIT_S: f64 = 1.0;

/// What the tuned frequency is to a band search that starts.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tuned {
    /// No step: it was searched already.
    Skip,
    /// The first step: its cells are seen again in the pass.
    First,
    /// A step only where the bands have one.
    Bands,
}

/// Why the stack stays on cells whose operator this rate cannot read, instead of searching on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hold {
    /// The command `stay` chose the cell.
    Chosen,
    /// A pass in auto found no cell whose operator can be read: the strongest cell found.
    NoneReadable,
}

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
    /// Signal time of the start of the first capture without a listed cell since one was seen or the frequency changed,
    /// seconds.
    quiet_since: Option<f64>,
    /// Signal time of the newest change of frequency, seconds.
    tuned_s: f64,
    /// What the stack says of the newest capture belongs to a frequency or a state that has changed since.
    reason_stale: bool,
    /// The frequency where the stack listens, Hz.
    tuned_hz: f64,
    /// The frequency of a cell the stack asked to stay on, Hz.
    staying_hz: Option<f64>,
    /// The wall times of the newest capture searches, seconds, oldest first.
    search_times: Vec<f64>,
    /// The wall times of the newest searches of first looks of a band search, seconds, oldest first.
    look_times: Vec<f64>,
    /// The first look of the step that the radio is on was searched: the samples wait for the next move.
    looked: bool,
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
    /// Why the stack stays where no cell can be read at this rate. None: in auto such a cell does not end the search.
    hold: Option<Hold>,
    /// The carrier centre and the rate the stack asked the host for, Hz, and the signal time of the request, seconds.
    asked_rate: Option<(f64, f64, f64)>,
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
            quiet_since: None,
            tuned_s: 0.0,
            reason_stale: false,
            tuned_hz: 0.0,
            staying_hz: None,
            search_times: Vec::with_capacity(SEARCH_TIMES),
            look_times: Vec::with_capacity(LOOK_TIMES),
            looked: false,
            coarse: false,
            levels: Vec::with_capacity(4096),
            live: false,
            refused_s: None,
            stayed: None,
            hold: None,
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
        self.stack.bound_search(self.live && self.pass.is_none());
        let t0 = std::time::Instant::now();
        let found = self.stack.search(cap);
        let took_s = t0.elapsed().as_secs_f64();
        if self.search_times.len() == SEARCH_TIMES {
            self.search_times.remove(0);
        }
        self.search_times.push(took_s);
        if M::found_cells(&found) > 0 {
            self.quiet = 0;
        } else {
            self.quiet = self.quiet.saturating_add(1);
        }
        let sync = self.stack.sync_seen(&found);
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
        self.reason_stale = false;
        self.changed = true;
        self.after_capture(cap, took_s, sync);
    }

    /// The band search after a capture: the step of a search that runs, or a search that starts when the setting asks for
    /// it and the samples are live. A recording cannot be retuned; the command `search` still asks the host.
    fn after_capture(&mut self, cap: &Capture, took_s: f64, sync: bool) {
        let rate = cap.rate;
        let Some(mode) = self.stack.band_search().map(|c| c.mode) else { return };
        // A carrier wider than the capture where no cell can be read, outside a band search: in auto, on a live radio, the
        // stack asks once for the rate and the centre that read its system information, and gives the host time for it.
        let wide = self.stack.rate_wanted().filter(|(_, r)| *r > rate);
        if let Some((hz, want)) = wide.filter(|_| mode == SearchMode::Auto && self.live && self.pass.is_none()) {
            if !self.readable_listed() {
                match self.asked_rate.filter(|(h, r, _)| (h - hz).abs() < 50e3 && *r >= want) {
                    None => {
                        self.asked_rate = Some((hz, want, self.time_s));
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
                    Some((_, _, t)) if self.time_s - t < RATE_WAIT_S => return,
                    Some(_) => {}
                }
            }
        }
        if let Some(since) = self.pass.as_ref().map(|p| p.started_s) {
            // A cell found in auto whose operator can be read: the search ends on the strongest such cell of this pass.
            if mode == SearchMode::Auto && self.quiet == 0 && self.stay_readable(since) {
                return;
            }
            self.step_searched(cap, took_s, sync);
            return;
        }
        if self.tuned_hz <= 0.0 || !self.live {
            return;
        }
        if self.refused_s.is_some_and(|t| self.time_s - t < REFUSED_HOLD_S) {
            return;
        }
        match mode {
            SearchMode::Off => {}
            SearchMode::Survey => {
                self.start_search(rate, Tuned::Bands);
            }
            SearchMode::Auto => {
                // The tuned frequency is a step of its own: one look without a sync signal, or search_dwell_s with one
                // while no cell is listed there. A cell whose operator this rate cannot read does not end the search.
                if self.listed() {
                    self.quiet_since = None;
                    if self.hold.is_none() && !self.readable_listed() && !self.start_search(rate, Tuned::First) {
                        // No step to search: the cells here are all there is.
                        self.hold = Some(Hold::NoneReadable);
                    }
                    return;
                }
                let since = *self.quiet_since.get_or_insert(cap.start_time());
                let longest_s = self.stack.band_search().map_or(0.0, |c| c.dwell_s);
                // A frequency the stack moved to for a cell gets the longest time even without a sync signal.
                let wait = !sync && self.staying_hz.is_none();
                if wait || self.time_s - since >= longest_s {
                    self.start_search(rate, Tuned::Skip);
                }
            }
        }
    }

    /// A capture of the step that the radio is on. After a first look with a sync signal the step is searched longer, up
    /// to the setting `search_dwell_s`, until the cells seen there are read, or until a capture shows no sync signal while
    /// no cell was seen there (a peak of noise); else the hop goes on.
    fn step_searched(&mut self, cap: &Capture, took_s: f64, sync: bool) {
        let Some(p) = self.pass.as_ref() else { return };
        let Some(i) = p.at.filter(|&i| (p.steps[i].0 - self.tuned_hz).abs() <= 1.0) else { return };
        let longest_s = self.stack.band_search().map_or(0.0, |c| c.dwell_s);
        match p.longer {
            None => {
                if self.look_times.len() == LOOK_TIMES {
                    self.look_times.remove(0);
                }
                self.look_times.push(took_s);
                let (_, read) = self.cells_seen(cap.start_time());
                let Some(p) = self.pass.as_mut() else { return };
                p.searched[i] = true;
                self.looked = true;
                if sync && !read && longest_s > cap.end_time - cap.start_time() {
                    // The hop is held on the step: the result can come when the radio has moved on, so it moves back.
                    p.longer = Some((i, cap.start_time()));
                    self.looked = false;
                    self.requests.push(HostRequest::retune(
                        p.steps[i].0,
                        "band search: a sync signal, searched longer",
                    ));
                    return;
                }
                self.step_over(i, false);
            }
            Some((e, since)) if e == i => {
                let (seen, read) = self.cells_seen(since);
                if read || (!sync && !seen) || self.time_s - since >= longest_s {
                    self.end_longer();
                }
            }
            Some(_) => {}
        }
    }

    /// Whether cells were seen since signal time `since_s`, and whether each of them is read as far as this centre allows.
    fn cells_seen(&self, since_s: f64) -> (bool, bool) {
        let seen: Vec<FoundCarrier> = M::carriers(&self.held.tables)
            .into_iter()
            .filter(|c| c.last_s >= since_s)
            .collect();
        (!seen.is_empty(), !seen.is_empty() && seen.iter().all(|c| c.settled))
    }

    /// Ends the longer search of a step: the hop goes on from the next step that waits.
    fn end_longer(&mut self) {
        let Some(p) = self.pass.as_mut() else { return };
        let Some((e, _)) = p.longer.take() else { return };
        self.looked = true;
        self.step_over(e, true);
    }

    /// After step `i`: the pass ends when every step was searched. A hop that was held, or whose time on each step no
    /// longer fits the searches, starts again from the next step that waits.
    fn step_over(&mut self, i: usize, held: bool) {
        let want = self.hop_dwell();
        let Some(p) = self.pass.as_mut() else { return };
        let changed = (want - p.dwell_s).abs() > DWELL_CHANGE * p.dwell_s;
        if p.searched.iter().all(|s| *s) {
            let since = p.started_s;
            p.passes += 1;
            p.before_s = since;
            p.started_s = self.time_s;
            p.searched.fill(false);
            p.from = (i + 1) % p.steps.len();
            let mode = self.stack.band_search().map(|c| c.mode);
            // Only a cell seen in this pass: an older row of the tables tells nothing of where the radio can hear one now.
            // A cell whose operator can be read first, else the strongest cell, which then holds the search.
            if mode == Some(SearchMode::Auto) {
                if self.stay_readable(since) {
                    return;
                }
                if self.stay("", since) {
                    self.hold = Some(Hold::NoneReadable);
                    return;
                }
            }
            if mode == Some(SearchMode::Survey) && self.visit_found(since) {
                return;
            }
        }
        if held || changed {
            self.hop_again(want);
        }
    }

    /// Asks for the hop again with `dwell_s` on each step, from the next step that waits.
    fn hop_again(&mut self, dwell_s: f64) {
        let Some(p) = self.pass.as_mut() else { return };
        let Some(j) = p.first_waiting() else { return };
        p.from = j;
        p.dwell_s = dwell_s;
        p.moved_s = self.time_s;
        p.fresh = true;
        let channels = p.steps[j..].iter().chain(&p.steps[..j]).map(|s| s.0).collect();
        self.requests.push(HostRequest::Hop {
            channels,
            dwell_s,
            stop_on_lock: false,
        });
    }

    /// The band search without a move for too long: the host refused the hop or ended it. The search ends, and says so.
    fn check_moved(&mut self) {
        let Some(p) = &self.pass else { return };
        let look_s = self.stack.band_search().map_or(0.0, |c| c.look_s);
        if p.longer.is_none() && self.time_s - p.moved_s > STUCK_DWELLS * p.dwell_s + look_s {
            self.pass = None;
            self.refused_s = Some(self.time_s);
            self.changed = true;
        }
    }

    /// The step searched longer after its time has passed without a capture (the radio did not come back).
    fn check_longer(&mut self) {
        let longest_s = self.stack.band_search().map_or(0.0, |c| c.dwell_s);
        let over = self
            .pass
            .as_ref()
            .and_then(|p| p.longer)
            .is_some_and(|(_, since)| self.time_s - since >= longest_s + self.stack.capture_s());
        if over {
            self.end_longer();
        }
    }

    /// Whether the samples now wait for the radio: the first look of the step was searched, or the radio has not yet come
    /// back to the step searched longer.
    fn samples_wait(&self) -> bool {
        match &self.pass {
            Some(p) => match p.longer {
                None => self.looked,
                Some((e, _)) => p.at != Some(e),
            },
            None => false,
        }
    }

    /// The seconds of input in one capture: the first look while a band search steps, else the capture length.
    fn capture_len_s(&self) -> f64 {
        match (&self.pass, self.stack.band_search()) {
            (Some(p), Some(c)) if p.longer.is_none() => c.look_s,
            _ => self.stack.capture_s(),
        }
    }

    /// Starts a band search at `rate`: the bands of the setting, else those of the band order, else the band of the tuned
    /// frequency. A search whose bands fit in one capture tunes to its one centre. `tuned` says what the tuned frequency
    /// is to the search. Returns false when no band holds a step.
    fn start_search(&mut self, rate: f64, tuned: Tuned) -> bool {
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
            let taken: Vec<f64> = base.iter().map(|s| s.0).collect();
            for c in self.stack.search_steps(b, rate, &taken) {
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
        match tuned {
            Tuned::Skip => steps.retain(|(c, _)| (c - self.tuned_hz).abs() > 1.0),
            Tuned::First if !steps.is_empty() => {
                let hz = self.tuned_hz;
                let band = steps
                    .iter()
                    .min_by(|a, b| (a.0 - hz).abs().total_cmp(&(b.0 - hz).abs()))
                    .map_or(0, |s| s.1);
                steps.retain(|(c, _)| (c - hz).abs() > 1.0);
                steps.insert(0, (hz, band));
            }
            _ => {}
        }
        let dwell_s = self.hop_dwell();
        self.staying_hz = None;
        self.stayed = None;
        self.hold = None;
        self.quiet = 0;
        self.quiet_since = None;
        self.refused_s = None;
        self.looked = false;
        self.changed = true;
        match steps.len() {
            0 => {
                self.pass = None;
                false
            }
            1 => {
                // One capture covers the band: listen there.
                self.pass = None;
                if tuned == Tuned::First {
                    self.hold = Some(Hold::NoneReadable);
                }
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
                let mut p = Pass::new(steps.clone(), steps, dwell_s, self.time_s);
                // The radio is on the first step already: a host reports no move to where it is.
                if (p.steps[0].0 - self.tuned_hz).abs() <= 1.0 {
                    p.at = Some(0);
                    p.fresh = false;
                }
                self.pass = Some(p);
                true
            }
        }
    }

    /// The time on each step of a band search: the first look and its search. The search time is the median of the newest
    /// searches of first looks, so that it follows the cost of the band; before the first one, that of the newest captures.
    fn hop_dwell(&self) -> f64 {
        let look_s = self.stack.band_search().map_or(0.0, |c| c.look_s);
        let times = if self.look_times.is_empty() {
            &self.search_times
        } else {
            &self.look_times
        };
        dwell_s(look_s, times)
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
        self.stay_on(c);
        true
    }

    /// Ends the band search on the strongest cell seen since signal time `since_s` whose operator can be read at this
    /// rate. Returns false when none is known.
    fn stay_readable(&mut self, since_s: f64) -> bool {
        let cells = M::carriers(&self.held.tables);
        let pick = cells
            .iter()
            .filter(|c| c.last_s >= since_s && c.readable())
            .max_by(|a, b| a.level_dbfs.total_cmp(&b.level_dbfs));
        let Some(c) = pick else { return false };
        self.stay_on(c);
        true
    }

    /// Ends the band search on cell `c`: the radio moves to it.
    fn stay_on(&mut self, c: &FoundCarrier) {
        self.pass = None;
        self.hold = None;
        self.quiet = 0;
        self.staying_hz = Some(c.frequency_hz);
        self.stayed = Some(c.name.clone());
        self.requests.push(HostRequest::retune(
            c.frequency_hz,
            &format!("band search: stays on {}", c.name),
        ));
        self.changed = true;
    }

    /// A survey after a pass: the next pass visits first each carrier of this pass whose operator is not read, at its own
    /// centre, so that a capture holds the carrier's centre and its system information can be read. Returns true when the
    /// steps changed and the hop was asked for again.
    fn visit_found(&mut self, since_s: f64) -> bool {
        let dwell_s = self.hop_dwell();
        let Some(pass) = self.pass.as_mut() else { return false };
        let band_near = |hz: f64| {
            pass.base
                .iter()
                .min_by(|a, b| (a.0 - hz).abs().total_cmp(&(b.0 - hz).abs()))
                .map_or(0, |s| s.1)
        };
        let mut visits: Vec<(f64, u16)> = Vec::new();
        for c in M::carriers(&self.held.tables) {
            // A carrier read as far as the rate allows, or one that a step of the bands holds at its centre already, is no
            // visit.
            let near = |v: &(f64, u16)| (v.0 - c.frequency_hz).abs() < 50e3;
            if c.identified || c.out_of_reach || c.last_s < since_s || visits.iter().any(near) || pass.base.iter().any(near) {
                continue;
            }
            visits.push((c.frequency_hz, band_near(c.frequency_hz)));
        }
        let added = visits.len();
        let steps: Vec<(f64, u16)> = visits
            .into_iter()
            .chain(pass.base.iter().copied())
            .collect();
        if steps == pass.steps {
            return false;
        }
        let (passes, started_s, before_s) = (pass.passes, pass.started_s, pass.before_s);
        *pass = Pass::new(steps, std::mem::take(&mut pass.base), dwell_s, started_s);
        pass.passes = passes;
        pass.before_s = before_s;
        pass.added = added;
        pass.moved_s = self.time_s;
        self.requests.push(HostRequest::Hop {
            channels: pass.steps.iter().map(|s| s.0).collect(),
            dwell_s,
            stop_on_lock: false,
        });
        true
    }

    /// The band search at a new frequency `tuned`: the step it is at, or a frequency of someone else, which ends the
    /// search. A hop that passed a step whose search was not done starts again there with more time on each step.
    fn search_retuned(&mut self, tuned: f64) {
        self.tuned_hz = tuned;
        self.quiet = 0;
        self.quiet_since = None;
        self.looked = false;
        if self.staying_hz.is_some_and(|h| (h - tuned).abs() <= 1.0) {
            return;
        }
        self.staying_hz = None;
        self.stayed = None;
        self.hold = None;
        let want = self.hop_dwell();
        let Some(pass) = self.pass.as_mut() else { return };
        let Some(i) = pass.steps.iter().position(|(c, _)| (c - tuned).abs() <= 1.0) else {
            // Someone else tuned the radio: the search ends.
            self.pass = None;
            return;
        };
        // A second report of the same step is no move.
        if pass.at == Some(i) {
            return;
        }
        let skipped = pass.longer.is_none() && pass.at.is_some() && pass.skipped_before(i);
        pass.at = Some(i);
        pass.moved_s = self.time_s;
        pass.fresh = false;
        if skipped {
            let dwell_s = want.max((1.0 + DWELL_CHANGE) * pass.dwell_s);
            self.hop_again(dwell_s);
        }
    }

    /// The status line of band search `p` on step `at` (None: before the radio moves for the first time).
    fn step_text(&self, p: &Pass, at: Option<usize>, summary: &str) -> String {
        let pass = if p.passes > 0 {
            format!(", pass {}", p.passes + 1)
        } else {
            String::new()
        };
        let (step, steps, added) = (p.step(), p.steps.len(), p.steps_text());
        let coarse = self.coarse_text().filter(|_| !self.shown()).map_or(String::new(), |w| format!("; {w}"));
        let unread = self.unread_listed(p).map_or(String::new(), |c| {
            format!(
                "; {} listed; its operator needs about {:.0} Msps: searching on for a cell that can be read",
                c.name,
                c.rate_needed_hz.unwrap_or(0.0) / 1e6
            )
        });
        let Some(i) = at else {
            return format!(
                "Searching {}: step {step} of {steps}, waiting for the radio to move{pass}{added}{unread}; {summary}{coarse}",
                M::band_name(p.steps[0].1),
            );
        };
        let mhz = format!(" at {:.1} MHz", p.steps[i].0 / 1e6);
        format!(
            "Searching {}: step {step} of {steps}{mhz}{pass}{added}{unread}; {summary}{coarse}",
            M::band_name(p.steps[i].1),
        )
    }

    /// In auto, the strongest cell of band search `p` (of this pass, the pass before, or within [`SearchStack::RECENT_S`])
    /// whose operator this rate cannot read.
    fn unread_listed(&self, p: &Pass) -> Option<FoundCarrier> {
        if self.stack.band_search().is_none_or(|c| c.mode != SearchMode::Auto) {
            return None;
        }
        let since = p.before_s.min(self.time_s - M::RECENT_S);
        M::carriers(&self.held.tables)
            .into_iter()
            .filter(|c| c.last_s >= since && !c.readable())
            .max_by(|a, b| a.level_dbfs.total_cmp(&b.level_dbfs))
    }

    /// Whether a cell is listed at this frequency whose operator can be read at this rate.
    fn readable_listed(&self) -> bool {
        let since = self.tuned_s.max(self.time_s - M::RECENT_S);
        M::carriers(&self.held.tables).iter().any(|c| c.last_s >= since && c.readable())
    }

    /// The status line for the time from the next move of the radio to the next report: the step that the hop or the
    /// stack's retune goes to, the cell that the stack moves to, or else the stack's summary.
    fn moving_text(&self, summary: &str) -> String {
        if let Some(p) = &self.pass {
            let n = p.steps.len();
            let to = match (p.longer, p.at) {
                (Some((e, _)), _) => e,
                (None, Some(i)) if !p.fresh => (i + 1) % n,
                (None, _) => p.from % n,
            };
            return self.step_text(p, Some(to), summary);
        }
        match (self.staying_hz, &self.stayed) {
            (Some(_), Some(_)) => self.status_text(summary, true),
            (Some(h), None) if (h - self.tuned_hz).abs() > 1.0 => format!("Tuning to {:.1} MHz; {summary}", h / 1e6),
            _ if summary.is_empty() => "Looking for cells".to_owned(),
            _ => summary.to_owned(),
        }
    }

    /// The status line: where the band search is, or why nothing shows, before the stack's own summary.
    fn status_text(&self, summary: &str, recent: bool) -> String {
        if let Some(p) = &self.pass {
            return self.step_text(p, p.at, summary);
        }
        if recent || self.listed() {
            let partial = self.stack.partial_reason().map_or(String::new(), |p| format!(": {p}"));
            let auto = self.live && self.stack.band_search().is_some_and(|c| c.mode == SearchMode::Auto);
            return match self.staying_on().filter(|_| auto || self.stayed.is_some()) {
                Some(c) if self.hold == Some(Hold::NoneReadable) => format!(
                    "Staying on {c}, the strongest cell found: no cell of the pass can be read at {:.2} Msps; {summary}{partial}",
                    self.cap.rate / 1e6
                ),
                Some(c) if auto => {
                    format!("Staying on {c}, the cell found (band_search survey visits all bands); {summary}{partial}")
                }
                Some(c) => format!("Staying on {c}; {summary}{partial}"),
                None => format!("{summary}{partial}"),
            };
        }
        match self.quiet_text() {
            Some(r) => format!("{summary}: {r}"),
            None => summary.to_owned(),
        }
    }

    /// Why no cell shows: a signal lost in the steps of the radio's converter first, then what the stack says of the newest
    /// capture.
    fn quiet_text(&self) -> Option<String> {
        if self.refused_s.is_some_and(|t| self.time_s - t < REFUSED_HOLD_S) {
            return Some(
                "the radio did not move for the band search: the frequency is locked, or a step lies outside what the radio \
                 tunes; set search_bands to bands the radio reaches"
                    .to_owned(),
            );
        }
        self.coarse_text()
            .or_else(|| self.stack.quiet_reason().filter(|_| !self.reason_stale))
    }

    /// Whether a cell is listed at this frequency: one seen since the newest change of frequency and within
    /// [`SearchStack::RECENT_S`]. A reason why no cell shows does not hold then.
    fn listed(&self) -> bool {
        self.cells_seen(self.tuned_s.max(self.time_s - M::RECENT_S)).0
    }

    /// The cell the stack stays on, as the status names it: the one a band search stayed on, else the strongest cell
    /// listed at this frequency.
    fn staying_on(&self) -> Option<String> {
        self.stayed.clone().or_else(|| {
            M::carriers(&self.held.tables)
                .into_iter()
                .filter(|c| c.last_s >= self.tuned_s)
                .max_by(|a, b| a.level_dbfs.total_cmp(&b.level_dbfs))
                .map(|c| c.name)
        })
    }

    /// Whether cells show: one listed at this frequency, or, while a band search runs, one seen in this pass, the pass
    /// before or within [`SearchStack::RECENT_S`]. A reason why nothing shows does not hold then.
    fn shown(&self) -> bool {
        match &self.pass {
            Some(p) => self.cells_seen(p.before_s.min(self.time_s - M::RECENT_S)).0,
            None => self.listed(),
        }
    }

    /// The text of a signal too weak at the radio: its samples use a few levels of the converter. The input level alone
    /// tells nothing: a radio at its automatic gain decodes cells far below full scale.
    fn coarse_text(&self) -> Option<String> {
        self.coarse.then(|| {
            "the signal is too weak at the radio: its samples use only a few levels of the converter; raise the gain, or \
             use a better antenna"
                .to_owned()
        })
    }

    fn resize(&mut self) {
        let len = (self.cap.rate * self.capture_len_s()).round() as usize;
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
        // At a new rate a cell may be read that could not be before.
        if (input.rate - self.cap.rate).abs() > 0.5 {
            self.hold = None;
        }
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
        self.tuned_s = self.time_s;
        self.quiet = 0;
        self.quiet_since = None;
        self.reason_stale = true;
        if ran {
            self.start_search(input.rate, Tuned::Bands);
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
            self.tuned_s = self.time_s;
            self.reason_stale = true;
        }
        // The framework rewrote the status: the next report writes it again.
        self.changed = true;
        self.cap.centre_hz = input.centre_hz;
        self.cap.dial_offset_hz = input.tuned_hz - input.centre_hz;
        self.search_retuned(input.tuned_hz);
        self.resize();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        if x.is_empty() || self.cap.rate <= 0.0 {
            return;
        }
        let skip = if self.live && self.pass.is_some() {
            x.len().saturating_sub(PASS_BACKLOG)
        } else {
            0
        };
        let (x, first) = (&x[skip..], first + skip as u64);
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
        self.check_longer();
        self.check_moved();
        self.resize();
        if self.samples_wait() {
            return;
        }
        let mut i = 0;
        while i < x.len() && self.len > 0 && !self.samples_wait() {
            let n = (self.len - self.cap.samples.len()).min(x.len() - i);
            self.cap.samples.extend_from_slice(&x[i..i + n]);
            i += n;
            if self.cap.samples.len() >= self.len {
                self.cap.end_time = (first + i as u64) as f64 / self.cap.rate;
                let full = std::mem::take(&mut self.cap);
                self.search_capture(&full);
                self.cap = full;
                self.cap.samples.clear();
                self.resize();
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
            // The framework writes this status at the next move, before the stack reports again.
            let moving = self.moving_text(&self.headline.summary);
            p.results_mut().set_searching_summary(&moving);
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
        let quiet = if recent || self.shown() {
            self.stack.partial_reason()
        } else {
            self.quiet_text()
        };
        if self.stack.band_search().is_some() {
            let band = self.pass.as_ref().map(|p| M::band_name(p.steps[p.at.unwrap_or(0)].1));
            p.put(&BandSearchFields {
                search_band: band.as_deref(),
                search_step: self.pass.as_ref().filter(|p| p.at.is_some()).map(|p| p.step() as i64),
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
                if !self.start_search(self.cap.rate, Tuned::Bands) {
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
                self.hold = Some(Hold::Chosen);
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
        self.reason_stale = true;
        self.changed = true;
    }
    /// Batches of what has arrived: the decoder collects the capture itself, and while it searches the newest
    /// [`HELPER_BACKLOG_S`] of input wait. A batch of what has arrived keeps up with input in pieces of any size.
    fn helper(&self) -> Helper {
        Helper::WhenLive {
            batch: 0,
            backlog: ((self.cap.rate * HELPER_BACKLOG_S) as usize).max(1 << 16),
        }
    }
    /// A band search looks at one short capture on each step and leaves out what arrives meanwhile. Following the cells of
    /// one frequency is no look: there every sample is wanted.
    fn looks(&self) -> bool {
        self.live && self.pass.is_some()
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

/// A list of frequency ranges in MHz, each "low-high": the text as typed and the ranges, Hz.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RangeList {
    /// The text as typed.
    pub text: String,
    /// The ranges, Hz: the low edge, then the high edge.
    pub hz: Vec<(f64, f64)>,
}

impl RangeList {
    /// Reads the list `text` of setting `id`: ranges "low-high" in MHz apart by spaces or commas.
    ///
    /// # Errors
    ///
    /// [`SettingError::Malformed`] with the position of the first word that is not a range of two positive numbers, the
    /// low one first.
    pub fn read(id: SettingId, text: &str) -> Result<RangeList, SettingError> {
        let mut hz = Vec::new();
        let mut at = 0;
        for word in text.split(|c: char| c == ',' || c.is_whitespace()) {
            let here = at;
            at += word.len() + 1;
            if word.is_empty() {
                continue;
            }
            let edges = word.split_once('-').and_then(|(a, b)| Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?)));
            match edges {
                Some((lo, hi)) if lo.is_finite() && hi.is_finite() && lo > 0.0 && lo < hi => hz.push((lo * 1e6, hi * 1e6)),
                _ => return Err(SettingError::Malformed { id, at: Some(here) }),
            }
        }
        Ok(RangeList {
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
    fn the_step_of_a_pass_grows_whatever_order_the_radio_takes() {
        let steps: Vec<(f64, u16)> = (0..4).map(|i| (700e6 + f64::from(i) * 10e6, 28)).collect();
        let mut p = Pass::new(steps.clone(), steps, 0.2, 0.0);
        assert_eq!(p.step(), 1);
        p.searched[3] = true;
        assert_eq!(p.step(), 2);
        // The radio is back on the first step before the last look was searched: the step does not go back.
        p.at = Some(0);
        p.searched[1] = true;
        p.searched[2] = true;
        assert_eq!(p.step(), 4);
        p.searched[0] = true;
        assert_eq!(p.step(), 4);
        assert_eq!(p.steps_text(), "");
        p.added = 2;
        assert_eq!(p.steps_text(), " (4 steps: 2 carriers added)");
    }

    #[test]
    fn the_time_on_a_step_is_the_look_and_its_search() {
        // Two searches of empty looks and one of a look with a cell to decode.
        assert!((dwell_s(0.08, &[0.25, 3.0, 0.2]) - 0.36).abs() < 1e-9);
        assert!((dwell_s(0.05, &[]) - 0.08).abs() < 1e-9);
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
    fn range_lists() {
        let id = SettingId(0);
        assert_eq!(RangeList::read(id, "3400-3800, 758-803").unwrap().hz, [(3400e6, 3800e6), (758e6, 803e6)]);
        assert!(RangeList::read(id, "").unwrap().hz.is_empty());
        assert_eq!(RangeList::read(id, "758-803 900").unwrap_err(), SettingError::Malformed { id, at: Some(8) });
        assert!(RangeList::read(id, "803-758").is_err());
    }

    #[test]
    fn band_lists() {
        assert_eq!(parse_band_list("14 28, 64"), Ok(vec![14, 28, 64]));
        assert_eq!(parse_band_list(""), Ok(vec![]));
        assert_eq!(parse_band_list("3 x"), Err(2));
        assert_eq!(parse_band_list("70000"), Err(0));
    }
}

//! The span-block scan: a range stepped in blocks of the span that a stack watches, each block held for a dwell and judged
//! by a declared result of the stack (for example the count of signals that passed its own gate), passes looped until a
//! total time is over. A find holds the scan; it stays, or goes on after the result has been quiet for a time.

use crate::hits::{Hit, HitLog};
use crate::host::{EndReason, Progress, ScanMachine, ScanState, Step, Target};
use crate::lock::number;
use crate::observe::Observation;
use sdrkit_core::{FieldId, Results, ResultsDecl, ValueKind};
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// What a span or survey scan does after a find.
    pub enum AfterFind {
        /// It stays on the find until the host resumes or stops it.
        Stay = ("stay", "Stay"),
        /// It goes on once the watched result has not passed its test for the quiet time.
        Resume = ("resume", "Resume when quiet"),
    }
}

/// How a block or a candidate is judged, and what follows a find.
#[derive(Clone, Debug, PartialEq)]
pub struct FindConfig {
    /// The stack id. Empty keeps the running stack.
    pub stack: String,
    /// The key of the result field of the stack that is watched.
    pub watch: String,
    /// A find is a watched value over this.
    pub above: f64,
    /// After a retune, the time before results are judged, s.
    pub settle_s: f64,
    /// How long a block or a candidate is held, s.
    pub dwell_s: f64,
    /// What follows a find.
    pub after: AfterFind,
    /// The time the watched value must stay at or under `above` before the scan goes on, s.
    pub quiet_s: f64,
    /// The time after which the scan ends, s. 0 runs until it is stopped.
    pub total_s: f64,
    /// Added to the clock of the scan to give seconds since 1970.
    pub clock_offset_s: f64,
    /// The most hits kept.
    pub hit_cap: usize,
}

/// The number of a result field found by its key, again only when the results declaration changes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Watch {
    key: String,
    decl: Option<Arc<ResultsDecl>>,
    id: Option<FieldId>,
}

impl Watch {
    pub(crate) fn new(key: &str) -> Watch {
        Watch {
            key: key.to_owned(),
            ..Watch::default()
        }
    }

    /// The value of the field in `r`; 0 when the field is absent (the results start empty after a retune).
    pub(crate) fn read(&mut self, r: &Results) -> f64 {
        if !self.decl.as_ref().is_some_and(|d| Arc::ptr_eq(d, r.decl())) {
            self.id = watched_field(r.decl(), &self.key);
            self.decl = Some(Arc::clone(r.decl()));
        }
        self.id
            .and_then(|id| r.fields().value(id))
            .and_then(number)
            .unwrap_or(0.0)
    }
}

/// The result field `key` of `decl` if a scan can watch it: a field whose values read as a number (a number, or a text that
/// starts with one).
pub(crate) fn watched_field(decl: &ResultsDecl, key: &str) -> Option<FieldId> {
    let id = decl.field_id(key)?;
    decl.fields()
        .field(id)
        .is_some_and(|f| watchable(f.kind))
        .then_some(id)
}

/// The keys of the result fields of `decl` that a scan can watch ([`watched_field`]), in the order of the declaration.
pub(crate) fn watched_keys(decl: &ResultsDecl) -> Vec<String> {
    decl.fields()
        .fields()
        .iter()
        .filter(|f| watchable(f.kind))
        .map(|f| f.key.to_string())
        .collect()
}

fn watchable(kind: ValueKind) -> bool {
    matches!(
        kind,
        ValueKind::I64
            | ValueKind::F64
            | ValueKind::Hz
            | ValueKind::Time
            | ValueKind::Duration
            | ValueKind::Text
    )
}

/// The offset of the carrier from the tuned frequency that the stack gives in the status of `results`, Hz; 0 without one.
/// A find is at the tuned frequency plus this offset.
pub(crate) fn carrier_offset(results: Option<&Results>) -> f64 {
    results
        .and_then(|r| r.status().offset_hz)
        .filter(|o| o.is_finite())
        .unwrap_or(0.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Settle { until: f64 },
    Dwell { until: f64 },
    Locked { quiet_since: Option<f64> },
}

/// What a step of a [`Judge`] says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Judged {
    /// Nothing changed.
    Wait,
    /// The watched value passed its test: the scan is locked.
    Found(f64),
    /// The dwell ended without a find, or a lock ended: the scan goes on.
    Next { found: bool },
}

/// The dwell on one block or candidate, its test, and the lock after a find.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Judge {
    phase: Phase,
}

impl Judge {
    /// Starts on a block tuned at `now`.
    pub(crate) fn tuned(now: f64, cfg: &FindConfig) -> Judge {
        Judge {
            phase: Phase::Settle {
                until: now + cfg.settle_s,
            },
        }
    }

    pub(crate) fn locked(&self) -> bool {
        matches!(self.phase, Phase::Locked { .. })
    }

    pub(crate) fn state(&self) -> ScanState {
        match self.phase {
            Phase::Settle { .. } => ScanState::Settle,
            Phase::Dwell { .. } => ScanState::Listen,
            Phase::Locked { .. } => ScanState::Locked,
        }
    }

    /// A step at `now`. `value` is the watched value of new results, `None` without new results.
    pub(crate) fn tick(&mut self, now: f64, value: Option<f64>, cfg: &FindConfig) -> Judged {
        match &mut self.phase {
            Phase::Settle { until } => {
                if now >= *until {
                    self.phase = Phase::Dwell {
                        until: now + cfg.dwell_s,
                    };
                }
                Judged::Wait
            }
            Phase::Dwell { until } => match value {
                Some(v) if v > cfg.above => {
                    self.phase = Phase::Locked { quiet_since: None };
                    Judged::Found(v)
                }
                _ if now >= *until => Judged::Next { found: false },
                _ => Judged::Wait,
            },
            Phase::Locked { quiet_since } => {
                if cfg.after == AfterFind::Stay {
                    return Judged::Wait;
                }
                match value {
                    Some(v) if v > cfg.above => *quiet_since = None,
                    Some(_) => {
                        quiet_since.get_or_insert(now);
                    }
                    None => {}
                }
                match *quiet_since {
                    Some(q) if now - q >= cfg.quiet_s => Judged::Next { found: true },
                    _ => Judged::Wait,
                }
            }
        }
    }
}

/// The settings of a span-block scan.
#[derive(Clone, Debug, PartialEq)]
pub struct SpanConfig {
    /// The low edge of the range, Hz.
    pub start_hz: f64,
    /// The high edge of the range, Hz.
    pub stop_hz: f64,
    /// The span that the stack watches, Hz: the width of a block.
    pub span_hz: f64,
    /// The test and the lock.
    pub find: FindConfig,
}

/// The most blocks of a span-block scan.
pub const MAX_BLOCKS: usize = 100_000;

/// The centres of the blocks of `span_hz` that cover `start_hz` to `stop_hz`, lowest first. Empty for a range or span that
/// is not positive, or more than [`MAX_BLOCKS`] blocks.
pub fn blocks(start_hz: f64, stop_hz: f64, span_hz: f64) -> Vec<f64> {
    let width = stop_hz - start_hz;
    if !(width.is_finite() && span_hz.is_finite() && width > 0.0 && span_hz > 0.0) {
        return Vec::new();
    }
    let n = (width / span_hz).ceil().max(1.0);
    if n > MAX_BLOCKS as f64 {
        return Vec::new();
    }
    // The blocks are spread so that the first and the last end at the edges of the range.
    let step = if n > 1.0 {
        (width - span_hz).max(0.0) / (n - 1.0)
    } else {
        0.0
    };
    let first = if n > 1.0 {
        start_hz + span_hz / 2.0
    } else {
        start_hz + width / 2.0
    };
    (0..n as usize).map(|i| first + step * i as f64).collect()
}

/// A span-block scan.
#[derive(Debug)]
pub struct SpanScanner {
    cfg: SpanConfig,
    blocks: Vec<f64>,
    index: usize,
    pass: u32,
    steps: u64,
    found: usize,
    judge: Option<Judge>,
    watch: Watch,
    hits: HitLog,
    open: Option<u64>,
    start_s: f64,
    paused: bool,
    end: EndReason,
    value: Option<f64>,
}

impl SpanScanner {
    /// A scan with `cfg`.
    pub fn new(cfg: SpanConfig) -> SpanScanner {
        SpanScanner {
            blocks: blocks(cfg.start_hz, cfg.stop_hz, cfg.span_hz),
            index: 0,
            pass: 0,
            steps: 0,
            found: 0,
            judge: None,
            watch: Watch::new(&cfg.find.watch),
            hits: HitLog::new(cfg.find.hit_cap),
            open: None,
            start_s: 0.0,
            paused: false,
            end: EndReason::None,
            value: None,
            cfg,
        }
    }

    /// The centres of the blocks, Hz.
    pub fn blocks(&self) -> &[f64] {
        &self.blocks
    }

    /// The finds.
    pub fn hits(&self) -> &HitLog {
        &self.hits
    }

    /// The last watched value read.
    pub fn watched(&self) -> Option<f64> {
        self.value
    }

    /// The time left, s, if the scan has a total time.
    pub fn time_left(&self, now: f64) -> Option<f64> {
        (self.cfg.find.total_s > 0.0).then(|| (self.start_s + self.cfg.find.total_s - now).max(0.0))
    }

    /// Attaches `text` to the find `seq`, or to the newest find. False if there is no such find.
    pub fn set_text(&mut self, seq: Option<u64>, text: &str) -> bool {
        let seq = seq.or_else(|| self.hits.pushed().checked_sub(1));
        match seq.and_then(|s| self.hits.get_mut(s)) {
            Some(h) => {
                h.text = text.to_owned();
                true
            }
            None => false,
        }
    }

    /// Ends the scan.
    pub fn stop(&mut self, now: f64) -> Step {
        self.close(now);
        self.end = EndReason::Stopped;
        self.judge = None;
        Step::Done
    }

    /// Leaves the block now for the next. `None` once the scan has ended.
    pub fn skip(&mut self, now: f64) -> Option<Step> {
        if self.end != EndReason::None {
            return None;
        }
        self.paused = false;
        Some(self.next(now))
    }

    /// Holds the block that is tuned.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Goes on after a pause or a find that stays: the next block is tuned.
    pub fn resume(&mut self, now: f64) -> Option<Step> {
        let held = self.paused || self.judge.is_some_and(|j| j.locked());
        if self.end != EndReason::None || !held {
            self.paused = false;
            return None;
        }
        self.paused = false;
        Some(self.next(now))
    }

    fn close(&mut self, now: f64) {
        if let Some(seq) = self.open.take() {
            let off = self.cfg.find.clock_offset_s;
            if let Some(h) = self.hits.get_mut(seq) {
                h.end_s = now + off;
            }
        }
    }

    /// The next block, or the end of the scan when the total time is over.
    fn next(&mut self, now: f64) -> Step {
        self.close(now);
        if self.time_over(now) {
            self.end = EndReason::TimeOver;
            self.judge = None;
            return Step::Done;
        }
        self.index += 1;
        if self.index >= self.blocks.len() {
            self.index = 0;
            self.pass += 1;
        }
        self.tune(now)
    }

    fn tune(&mut self, now: f64) -> Step {
        self.steps += 1;
        self.value = None;
        self.judge = Some(Judge::tuned(now, &self.cfg.find));
        Step::Tune
    }

    fn time_over(&self, now: f64) -> bool {
        self.cfg.find.total_s > 0.0 && now - self.start_s >= self.cfg.find.total_s
    }
}

impl ScanMachine for SpanScanner {
    fn start(&mut self, now_s: f64) -> Step {
        self.start_s = now_s;
        if self.blocks.is_empty() {
            self.end = EndReason::NothingToScan;
            return Step::Done;
        }
        self.tune(now_s)
    }

    fn tick(&mut self, now_s: f64, _obs: &Observation, results: Option<&Results>) -> Option<Step> {
        let mut judge = self.judge?;
        let value = results.map(|r| self.watch.read(r));
        if value.is_some() {
            self.value = value;
        }
        if self.paused {
            return None;
        }
        let out = judge.tick(now_s, value, &self.cfg.find);
        self.judge = Some(judge);
        match out {
            Judged::Wait if self.time_over(now_s) && !judge.locked() => Some(self.next(now_s)),
            Judged::Wait => None,
            Judged::Found(_) => {
                self.found += 1;
                let off = self.cfg.find.clock_offset_s;
                self.open = Some(self.hits.pushed());
                self.hits.push(Hit {
                    start_s: now_s + off,
                    end_s: now_s + off,
                    freq_hz: self.blocks[self.index] + carrier_offset(results),
                    stack: self.cfg.find.stack.clone(),
                    name: String::new(),
                    tone: String::new(),
                    peak_snr_db: None,
                    priority: false,
                    memory_id: None,
                    index: self.index,
                    pass: self.pass,
                    text: String::new(),
                });
                None
            }
            Judged::Next { .. } => Some(self.next(now_s)),
        }
    }

    fn target(&self) -> Option<Target<'_>> {
        self.judge?;
        Some(Target {
            freq_hz: *self.blocks.get(self.index)?,
            stack: &self.cfg.find.stack,
            submode: "",
            name: "",
            settings: &[],
            bandwidth_hz: None,
            rate_hz: None,
        })
    }

    fn audio_wanted(&self) -> bool {
        self.paused || self.judge.is_some_and(|j| j.locked())
    }

    fn finished(&self) -> bool {
        self.end != EndReason::None
    }

    fn progress(&self) -> Progress {
        let state = match (self.end, self.judge) {
            (EndReason::None, _) if self.paused => ScanState::Paused,
            (EndReason::None, Some(j)) => j.state(),
            (EndReason::None, None) => ScanState::Settle,
            _ => ScanState::Done,
        };
        Progress {
            state,
            index: self.index,
            count: self.blocks.len(),
            at_priority: false,
            pass: self.pass,
            steps: self.steps,
            found: self.found,
            end: self.end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_cover_the_range_edge_to_edge() {
        let b = blocks(7.0e6, 7.2e6, 48e3);
        assert_eq!(b.len(), 5);
        assert!((b[0] - (7.0e6 + 24e3)).abs() < 1e-6);
        assert!((b[4] - (7.2e6 - 24e3)).abs() < 1e-6);
        assert_eq!(blocks(7.0e6, 7.01e6, 48e3), vec![7.005e6]);
        assert!(blocks(7.0e6, 6.0e6, 48e3).is_empty());
    }

    fn cfg(after: AfterFind) -> FindConfig {
        FindConfig {
            stack: "cw".into(),
            watch: "signals".into(),
            above: 0.0,
            settle_s: 0.5,
            dwell_s: 5.0,
            after,
            quiet_s: 10.0,
            total_s: 0.0,
            clock_offset_s: 0.0,
            hit_cap: 100,
        }
    }

    #[test]
    fn a_judge_locks_on_a_find_and_resumes_after_quiet() {
        let c = cfg(AfterFind::Resume);
        let mut j = Judge::tuned(0.0, &c);
        assert_eq!(j.tick(0.2, Some(3.0), &c), Judged::Wait, "settling");
        assert_eq!(j.tick(0.6, Some(0.0), &c), Judged::Wait);
        assert_eq!(j.tick(1.0, Some(2.0), &c), Judged::Found(2.0));
        assert_eq!(j.tick(2.0, Some(0.0), &c), Judged::Wait);
        assert_eq!(j.tick(5.0, Some(1.0), &c), Judged::Wait, "a signal again");
        assert_eq!(j.tick(6.0, Some(0.0), &c), Judged::Wait);
        assert_eq!(j.tick(15.9, None, &c), Judged::Wait);
        assert_eq!(j.tick(16.0, None, &c), Judged::Next { found: true });
        let s = cfg(AfterFind::Stay);
        let mut j = Judge::tuned(0.0, &s);
        j.tick(0.6, None, &s);
        assert_eq!(j.tick(1.0, Some(1.0), &s), Judged::Found(1.0));
        assert_eq!(j.tick(1000.0, Some(0.0), &s), Judged::Wait);
    }
}

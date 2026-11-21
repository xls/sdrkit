//! The activity scanner: it visits each channel of a list, a band search or the memories, and stops on activity. It holds
//! while the channel is active, waits the resume delay after activity ends, and moves on. A priority channel is checked every
//! few seconds, also while another channel is held. Every stop is a [`Hit`] in the log.

use crate::hits::{Hit, HitLog, DEFAULT_HIT_CAP};
use crate::host::{EndReason, Progress, ScanMachine, ScanState, Step, Target};
use crate::list::Channels;
use crate::observe::{Activity, Observation};
use sdrkit_core::Results;
use std::collections::BTreeSet;

/// How an activity scan behaves.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityConfig {
    /// After a retune, how long before the channel is judged (the radio and the receiver settle), seconds.
    pub settle_s: f64,
    /// How long to listen on a quiet channel, seconds (an entry's own dwell wins).
    pub dwell_s: f64,
    /// The resume delay: how long to stay after activity ends, seconds.
    pub hang_s: f64,
    /// The longest hold before moving on regardless, seconds; `None` holds as long as the channel is active.
    pub max_hold_s: Option<f64>,
    /// What counts as activity.
    pub activity: Activity,
    /// How often the priority channel is checked, seconds.
    pub priority_interval_s: f64,
    /// How long the priority channel is listened to on a check, seconds.
    pub priority_dwell_s: f64,
    /// Passes over the channels before the scan ends; `None` scans until stopped.
    pub passes: Option<u32>,
    /// Added to the clock of the scan to give seconds since 1970 for the hit log.
    pub clock_offset_s: f64,
    /// The most hits the log keeps.
    pub hit_cap: usize,
}

impl Default for ActivityConfig {
    fn default() -> Self {
        ActivityConfig {
            settle_s: 0.15,
            dwell_s: 0.5,
            hang_s: 2.0,
            max_hold_s: None,
            activity: Activity::default(),
            priority_interval_s: 2.0,
            priority_dwell_s: 0.3,
            passes: None,
            clock_offset_s: 0.0,
            hit_cap: DEFAULT_HIT_CAP,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Settle {
        until: f64,
        resume_hold: Option<f64>,
    },
    Listen {
        until: f64,
    },
    Hold {
        since: f64,
    },
    Hang {
        until: f64,
        since: f64,
    },
    Done,
}

impl Phase {
    fn state(&self) -> ScanState {
        match self {
            Phase::Settle { .. } => ScanState::Settle,
            Phase::Listen { .. } => ScanState::Listen,
            Phase::Hold { .. } => ScanState::Hold,
            Phase::Hang { .. } => ScanState::Hang,
            Phase::Done => ScanState::Done,
        }
    }
}

#[derive(Clone, Debug)]
struct OpenHit {
    start_s: f64,
    last_active_s: f64,
    tone: String,
    peak_snr_db: Option<f32>,
}

#[derive(Clone, Debug)]
struct Saved {
    index: usize,
    open: Option<OpenHit>,
    held_since: Option<f64>,
}

/// The activity scanner.
#[derive(Debug)]
pub struct ActivityScanner {
    cfg: ActivityConfig,
    channels: Channels,
    lockouts: BTreeSet<i64>,
    priority: Option<usize>,
    index: usize,
    at_priority: bool,
    saved: Option<Saved>,
    phase: Phase,
    open: Option<OpenHit>,
    last_priority_s: f64,
    hits: HitLog,
    pass: u32,
    steps: u64,
    paused: bool,
    end: EndReason,
}

fn key(hz: f64) -> i64 {
    hz.round() as i64
}

impl ActivityScanner {
    /// A scanner over `channels`. The priority channel is the first list entry marked priority (see
    /// [`ActivityScanner::set_priority`]). The scan passes by list entries marked lockout.
    pub fn new(channels: Channels, cfg: ActivityConfig) -> ActivityScanner {
        let priority = match &channels {
            Channels::List(entries) => entries.iter().position(|e| e.priority && !e.lockout),
            Channels::Band(_) => None,
        };
        let hits = HitLog::new(cfg.hit_cap);
        ActivityScanner {
            cfg,
            channels,
            lockouts: BTreeSet::new(),
            priority,
            index: 0,
            at_priority: false,
            saved: None,
            phase: Phase::Done,
            open: None,
            last_priority_s: 0.0,
            hits,
            pass: 0,
            steps: 0,
            paused: false,
            end: EndReason::None,
        }
    }

    /// The channels, with the hit counts, last-hit times and tones written back to list entries.
    pub fn channels(&self) -> &Channels {
        &self.channels
    }

    /// The hit log.
    pub fn hits(&self) -> &HitLog {
        &self.hits
    }

    /// The configuration.
    pub fn config(&self) -> &ActivityConfig {
        &self.cfg
    }

    /// The channel tuned now.
    pub fn current(&self) -> usize {
        self.index
    }

    /// The scan is holding a stop (sound plays).
    pub fn holding(&self) -> bool {
        matches!(self.phase, Phase::Hold { .. } | Phase::Hang { .. })
    }

    /// The scan is at the priority channel for a check.
    pub fn at_priority(&self) -> bool {
        self.at_priority
    }

    /// Frequencies locked out during this scan (Hz), besides list entries marked lockout.
    pub fn lockouts(&self) -> impl Iterator<Item = f64> + '_ {
        self.lockouts.iter().map(|&k| k as f64)
    }

    /// Adds lockouts (a band search's saved ones).
    pub fn add_lockouts(&mut self, hz: &[f64]) {
        self.lockouts.extend(hz.iter().map(|&h| key(h)));
    }

    /// Lifts a lockout of a frequency (and of the list entries on it).
    pub fn unlock(&mut self, hz: f64) {
        self.lockouts.remove(&key(hz));
        if let Channels::List(entries) = &mut self.channels {
            for e in entries.iter_mut().filter(|e| key(e.freq_hz) == key(hz)) {
                e.lockout = false;
            }
        }
    }

    /// Sets or clears the priority channel (an index of the channels).
    pub fn set_priority(&mut self, index: Option<usize>) {
        self.priority = index.filter(|&i| i < self.channels.len());
    }

    fn locked(&self, i: usize) -> bool {
        self.lockouts.contains(&key(self.channels.freq(i)))
            || self.channels.entry(i).is_some_and(|e| e.lockout)
    }

    fn dwell(&self, i: usize) -> f64 {
        if self.at_priority {
            return self.cfg.priority_dwell_s;
        }
        self.channels
            .entry(i)
            .and_then(|e| e.dwell_s)
            .unwrap_or(self.cfg.dwell_s)
    }

    fn tune(&mut self, now: f64, resume_hold: Option<f64>) -> Step {
        self.steps += 1;
        self.phase = Phase::Settle {
            until: now + self.cfg.settle_s,
            resume_hold,
        };
        Step::Tune
    }

    fn finish(&mut self, now: f64, end: EndReason) -> Step {
        self.end = end;
        self.close_hit(now);
        if let Some(s) = self.saved.take() {
            // Stopped during a priority check: the held channel's stop ends too.
            self.at_priority = false;
            self.index = s.index;
            self.open = s.open;
            self.close_hit(now);
        }
        self.phase = Phase::Done;
        Step::Done
    }

    /// Moves to the next channel that is not locked out, or ends the scan.
    fn next(&mut self, now: f64) -> Step {
        self.close_hit(now);
        let n = self.channels.len();
        for _ in 0..n {
            self.index += 1;
            if self.index >= n {
                self.index = 0;
                self.pass += 1;
                if self.cfg.passes.is_some_and(|p| self.pass >= p) {
                    return self.finish(now, EndReason::PassesDone);
                }
            }
            if !self.locked(self.index) {
                return self.tune(now, None);
            }
        }
        self.finish(now, EndReason::AllLockedOut)
    }

    fn go_priority(&mut self, p: usize, now: f64) -> Step {
        let held_since = match self.phase {
            Phase::Hold { since } | Phase::Hang { since, .. } => Some(since),
            Phase::Settle {
                resume_hold: Some(since),
                ..
            } => Some(since),
            _ => None,
        };
        self.saved = Some(Saved {
            index: self.index,
            open: self.open.take(),
            held_since,
        });
        self.at_priority = true;
        self.index = p;
        self.tune(now, None)
    }

    fn back_from_priority(&mut self, now: f64) -> Step {
        self.close_hit(now);
        self.at_priority = false;
        self.last_priority_s = now;
        match self.saved.take() {
            Some(s) if !self.locked(s.index) => {
                self.index = s.index;
                self.open = s.open;
                self.tune(now, s.held_since)
            }
            Some(s) => {
                self.index = s.index;
                self.open = s.open;
                self.next(now)
            }
            None => self.next(now),
        }
    }

    /// Leaves the current channel: back from a priority check, or on to the next.
    fn leave(&mut self, now: f64) -> Step {
        if self.at_priority {
            self.back_from_priority(now)
        } else {
            self.next(now)
        }
    }

    fn open_hit(&mut self, now: f64, obs: &Observation) {
        if self.open.is_none() {
            self.open = Some(OpenHit {
                start_s: now,
                last_active_s: now,
                tone: String::new(),
                peak_snr_db: None,
            });
        }
        self.update_hit(now, obs);
    }

    fn update_hit(&mut self, now: f64, obs: &Observation) {
        if let Some(h) = &mut self.open {
            h.last_active_s = now;
            if !obs.tone.is_empty() {
                h.tone.clone_from(&obs.tone);
            }
            if let Some(s) = obs.snr() {
                h.peak_snr_db = Some(h.peak_snr_db.map_or(s, |p| p.max(s)));
            }
        }
    }

    fn close_hit(&mut self, _now: f64) {
        let Some(h) = self.open.take() else { return };
        let i = self.index;
        let off = self.cfg.clock_offset_s;
        let hit = Hit {
            start_s: h.start_s + off,
            end_s: h.last_active_s + off,
            freq_hz: self.channels.freq(i),
            stack: self.channels.target(i).stack.to_owned(),
            name: self.channels.name(i),
            tone: h.tone.clone(),
            peak_snr_db: h.peak_snr_db,
            priority: self.priority == Some(i),
            memory_id: self.channels.entry(i).and_then(|e| e.memory_id),
            index: i,
            pass: self.pass,
            text: String::new(),
        };
        if let Some(e) = self.channels.entry_mut(i) {
            e.hits += 1;
            e.last_hit_s = Some(hit.start_s);
            if !hit.tone.is_empty() {
                e.tone.clone_from(&hit.tone);
            }
        }
        self.hits.push(hit);
    }

    /// Skips the current channel and ends its stop.
    pub fn skip(&mut self, now: f64) -> Option<Step> {
        if self.phase == Phase::Done {
            return None;
        }
        self.paused = false;
        Some(self.leave(now))
    }

    /// Locks out the current channel (a list entry, or a band step's frequency) and moves on.
    pub fn lockout_current(&mut self, now: f64) -> Option<Step> {
        if self.phase == Phase::Done {
            return None;
        }
        let i = self.index;
        self.lockouts.insert(key(self.channels.freq(i)));
        if let Some(e) = self.channels.entry_mut(i) {
            e.lockout = true;
        }
        if self.priority == Some(i) {
            self.priority = None;
        }
        self.skip(now)
    }

    /// Stays on the current channel with sound until [`ActivityScanner::resume`].
    pub fn pause(&mut self) {
        if self.phase != Phase::Done {
            self.paused = true;
        }
    }

    /// Continues after a pause: listens to the current channel again, so it holds when still active.
    pub fn resume(&mut self, now: f64) {
        if self.paused {
            self.paused = false;
            self.phase = Phase::Listen {
                until: now + self.dwell(self.index),
            };
        }
    }

    /// Ends the scan.
    pub fn stop(&mut self, now: f64) -> Step {
        self.finish(now, EndReason::Stopped)
    }
}

impl ScanMachine for ActivityScanner {
    fn start(&mut self, now: f64) -> Step {
        self.last_priority_s = now;
        self.end = EndReason::None;
        self.pass = 0;
        let n = self.channels.len();
        match (0..n).find(|&i| !self.locked(i)) {
            Some(i) => {
                self.index = i;
                self.tune(now, None)
            }
            None => self.finish(now, EndReason::NothingToScan),
        }
    }

    fn tick(&mut self, now: f64, obs: &Observation, _results: Option<&Results>) -> Option<Step> {
        if self.phase == Phase::Done || self.paused {
            return None;
        }
        if let Some(p) = self.priority {
            if !self.at_priority
                && p != self.index
                && !self.locked(p)
                && now - self.last_priority_s >= self.cfg.priority_interval_s
            {
                return Some(self.go_priority(p, now));
            }
        }
        let active = self.cfg.activity.active(obs);
        match self.phase {
            Phase::Settle { until, resume_hold } => {
                if now >= until {
                    self.phase = match resume_hold {
                        // Back at a held channel: activity holds it again.
                        Some(since) if active => {
                            self.update_hit(now, obs);
                            Phase::Hold { since }
                        }
                        // The resume delay counts from the last activity heard, so priority checks cannot keep a quiet
                        // channel held.
                        Some(since) => {
                            let last = self.open.as_ref().map_or(now, |h| h.last_active_s);
                            Phase::Hang {
                                until: (last + self.cfg.hang_s).max(now),
                                since,
                            }
                        }
                        None => Phase::Listen {
                            until: now + self.dwell(self.index),
                        },
                    };
                }
                None
            }
            Phase::Listen { until } => {
                if active {
                    self.open_hit(now, obs);
                    self.phase = Phase::Hold { since: now };
                    None
                } else if now >= until {
                    Some(self.leave(now))
                } else {
                    None
                }
            }
            Phase::Hold { since } => {
                if active {
                    self.update_hit(now, obs);
                    if self.cfg.max_hold_s.is_some_and(|m| now - since >= m) {
                        return Some(self.leave(now));
                    }
                } else {
                    self.phase = Phase::Hang {
                        until: now + self.cfg.hang_s,
                        since,
                    };
                }
                None
            }
            Phase::Hang { until, since } => {
                if active {
                    self.update_hit(now, obs);
                    self.phase = Phase::Hold { since };
                    if self.cfg.max_hold_s.is_some_and(|m| now - since >= m) {
                        return Some(self.leave(now));
                    }
                    None
                } else if now >= until || self.cfg.max_hold_s.is_some_and(|m| now - since >= m) {
                    Some(self.leave(now))
                } else {
                    None
                }
            }
            Phase::Done => None,
        }
    }

    fn audio_wanted(&self) -> bool {
        self.paused || self.holding()
    }

    fn finished(&self) -> bool {
        self.phase == Phase::Done
    }

    fn target(&self) -> Option<Target<'_>> {
        (self.phase != Phase::Done && self.index < self.channels.len())
            .then(|| self.channels.target(self.index))
    }

    fn progress(&self) -> Progress {
        Progress {
            state: if self.paused {
                ScanState::Paused
            } else {
                self.phase.state()
            },
            index: self.index,
            count: self.channels.len(),
            at_priority: self.at_priority,
            pass: self.pass,
            steps: self.steps,
            found: self.hits.len(),
            end: self.end,
        }
    }
}

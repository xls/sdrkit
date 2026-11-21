//! The scheduler of timed recordings: a state machine that gets the time from its caller and answers with what to start and
//! stop.
//!
//! A job repeats once, every N seconds, daily, on chosen weekdays or by a cron rule. Each resource runs as many jobs at once
//! as its capacity, and a job of higher priority takes the resource from a lower one. Local time uses a fixed offset from UTC.

use crate::error::CronError;
use crate::format::MediaKind;
use sdrkit_proto_mpegts::time::{civil, weekday};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// How a job repeats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Repeat {
    /// Once, at `start`.
    Once,
    /// Every `seconds`, from `start`.
    Every {
        /// The period.
        seconds: i64,
    },
    /// Every day at the local time of `start`.
    Daily,
    /// On these weekdays (0 Monday to 6 Sunday) at the local time of `start`.
    Weekly {
        /// The weekdays.
        days: Vec<u8>,
    },
    /// By a cron rule (`m h dom mon dow`, or `@hourly`, `@daily`, `@weekly`, `@monthly`), from `start`.
    Cron {
        /// The rule.
        rule: String,
    },
}

/// What to do with a window that began while the scheduler was not ticking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Missed {
    /// Start late within `grace_s` of its start. After that, it is missed.
    #[default]
    RunLate,
    /// Missed.
    Skip,
}

/// What to do when the resource is busy with a job of the same or higher priority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conflict {
    /// Skip this window.
    #[default]
    Skip,
    /// Start when the resource frees, if at least `min_s` of the window is left.
    StartWhenFree,
}

/// What a job records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Target {
    /// Audio, IQ or a transport stream.
    pub kind: MediaKind,
    /// The session to record (its row key in the engine's sessions table).
    pub session: String,
    /// The frequency to tune to first. `None` keeps the frequency.
    pub freq_hz: Option<f64>,
    /// The stack to start first. Empty keeps the running stack.
    pub stack: String,
}

impl Default for Target {
    fn default() -> Self {
        Target {
            kind: MediaKind::Audio,
            session: String::new(),
            freq_hz: None,
            stack: String::new(),
        }
    }
}

/// A timed recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Job {
    /// A stable id.
    pub id: String,
    /// A name (in file names and the history).
    pub name: String,
    /// Off jobs never start.
    pub enabled: bool,
    /// The start of the first window (Unix seconds). For daily and weekly repeats, also the time of day.
    pub start: i64,
    /// The length of a window, seconds.
    pub duration_s: i64,
    /// How it repeats.
    pub repeat: Repeat,
    /// No window starts after this.
    pub until: Option<i64>,
    /// Stop after this many runs.
    pub max_runs: Option<u32>,
    /// Seconds started early.
    pub pad_before_s: i64,
    /// Seconds kept after.
    pub pad_after_s: i64,
    /// Higher wins a busy resource.
    pub priority: i32,
    /// The resource it needs (a radio or tuner id).
    pub resource: String,
    /// Windows begun while not ticking.
    pub on_missed: Missed,
    /// How late a start can be, seconds after the padded start.
    pub grace_s: i64,
    /// A busy resource.
    pub on_conflict: Conflict,
    /// The least of a window worth recording after a late start (seconds).
    pub min_s: i64,
    /// Bytes that must be free to start.
    pub min_free_bytes: u64,
    /// The file name template (without extension).
    pub name_template: String,
    /// What to record.
    pub target: Target,
    /// Runs so far.
    pub runs: u32,
}

impl Default for Job {
    fn default() -> Self {
        Job {
            id: String::new(),
            name: String::new(),
            enabled: true,
            start: 0,
            duration_s: 60,
            repeat: Repeat::Once,
            until: None,
            max_runs: None,
            pad_before_s: 0,
            pad_after_s: 0,
            priority: 0,
            resource: "radio".into(),
            on_missed: Missed::RunLate,
            grace_s: 3600 * 24,
            on_conflict: Conflict::Skip,
            min_s: 10,
            min_free_bytes: 0,
            name_template: "{name}_{date}-{time}".into(),
            target: Target::default(),
            runs: 0,
        }
    }
}

impl Job {
    /// The windows (nominal start, nominal end) that touch `[from, to)` once padded, in start order, before the run-count limit.
    pub fn windows(&self, from: i64, to: i64, utc_offset_s: i32) -> Vec<(i64, i64)> {
        let pre = self.pad_before_s.max(0);
        let post = self.pad_after_s.max(0);
        let d = self.duration_s.max(0);
        let until = self.until.unwrap_or(i64::MAX);
        // Nominal starts in [lo, hi) can touch the range.
        let lo = from - d - post;
        let hi = to.saturating_add(pre);
        let mut out = Vec::new();
        let push = |s: i64, e: i64, out: &mut Vec<(i64, i64)>| {
            if s <= until && s - pre < to && e + post > from {
                out.push((s, e));
            }
        };
        match &self.repeat {
            Repeat::Once => push(self.start, self.start + d, &mut out),
            Repeat::Every { seconds } => periodic(self.start, (*seconds).max(1), lo, hi, |s| {
                push(s, s + d, &mut out)
            }),
            Repeat::Daily => periodic(self.start, 86_400, lo, hi, |s| push(s, s + d, &mut out)),
            Repeat::Weekly { days } => periodic(self.start, 86_400, lo, hi, |s| {
                if days.contains(&(weekday(s + utc_offset_s as i64) as u8)) {
                    push(s, s + d, &mut out)
                }
            }),
            Repeat::Cron { rule } => {
                if let Ok(c) = CronRule::parse(rule) {
                    let mut t = lo.max(self.start) - 1;
                    while let Some(s) = c.next_after(t, utc_offset_s) {
                        if s >= hi {
                            break;
                        }
                        push(s, s + d, &mut out);
                        t = s;
                    }
                }
            }
        }
        out
    }
}

/// Calls `f` with `start + k * period` (k >= 0) in `[lo, hi)`.
fn periodic(start: i64, period: i64, lo: i64, hi: i64, mut f: impl FnMut(i64)) {
    let k0 = if lo > start { (lo - start) / period } else { 0 };
    let mut s = start + k0 * period;
    while s < hi {
        if s >= lo {
            f(s);
        }
        s += period;
    }
}

/// A cron rule: minute, hour, day of month, month, weekday (0 or 7 Sunday), each `*`, values, ranges, steps and lists, with
/// month and weekday names. When both the day of month and the weekday are restricted, either matches (as cron does).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CronRule {
    minutes: u64,
    hours: u64,
    doms: u64,
    months: u64,
    dows: u64,
    dom_any: bool,
    dow_any: bool,
}

/// Reads one field of a cron rule that starts at byte `at`: its bits and whether it is `*`.
fn field(
    text: &str,
    at: usize,
    min: u32,
    max: u32,
    names: &[&str],
) -> Result<(u64, bool), CronError> {
    let mut bits = 0u64;
    let bad = CronError::Field { at };
    let value = |s: &str| -> Result<u32, CronError> {
        if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(s)) {
            return Ok(i as u32 + min);
        }
        s.parse::<u32>().map_err(|_| bad)
    };
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (r, s.parse::<u32>().map_err(|_| bad)?),
            None => (part, 1),
        };
        if step == 0 {
            return Err(bad);
        }
        let (a, b) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (value(a)?, value(b)?)
        } else {
            let v = value(range)?;
            (v, if part.contains('/') { max } else { v })
        };
        if a < min || b > max || a > b {
            return Err(CronError::Range { at });
        }
        let mut v = a;
        while v <= b {
            bits |= 1 << v;
            v += step;
        }
    }
    Ok((bits, text == "*"))
}

impl CronRule {
    /// Reads a rule: five fields, or `@hourly`, `@daily`, `@midnight`, `@weekly`, `@monthly`, `@yearly`, `@annually`.
    ///
    /// # Errors
    ///
    /// [`CronError`] with the byte position of the field that does not read.
    pub fn parse(rule: &str) -> Result<CronRule, CronError> {
        let rule = match rule.trim() {
            "@hourly" => "0 * * * *",
            "@daily" | "@midnight" => "0 0 * * *",
            "@weekly" => "0 0 * * 0",
            "@monthly" => "0 0 1 * *",
            "@yearly" | "@annually" => "0 0 1 1 *",
            _ => rule,
        };
        let mut f: Vec<(usize, &str)> = Vec::with_capacity(5);
        let mut start = None;
        for (i, c) in rule
            .char_indices()
            .chain(std::iter::once((rule.len(), ' ')))
        {
            match (c.is_whitespace(), start) {
                (true, Some(s)) => {
                    f.push((s, &rule[s..i]));
                    start = None;
                }
                (false, None) => start = Some(i),
                _ => {}
            }
        }
        if f.len() != 5 {
            return Err(CronError::Fields);
        }
        const MONTHS: [&str; 12] = [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ];
        const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
        let (minutes, _) = field(f[0].1, f[0].0, 0, 59, &[])?;
        let (hours, _) = field(f[1].1, f[1].0, 0, 23, &[])?;
        let (doms, dom_any) = field(f[2].1, f[2].0, 1, 31, &[])?;
        let (months, _) = field(f[3].1, f[3].0, 1, 12, &MONTHS)?;
        let (mut dows, dow_any) = field(f[4].1, f[4].0, 0, 7, &DAYS)?;
        if dows & (1 << 7) != 0 {
            dows |= 1;
        }
        Ok(CronRule {
            minutes,
            hours,
            doms,
            months,
            dows,
            dom_any,
            dow_any,
        })
    }

    fn day_matches(&self, local_day_start: i64) -> bool {
        let (_, m, d, _, _, _) = civil(local_day_start);
        if self.months & (1 << m) == 0 {
            return false;
        }
        let dow = (weekday(local_day_start) + 1) % 7; // cron: 0 Sunday
        let dom_ok = self.doms & (1 << d) != 0;
        let dow_ok = self.dows & (1 << dow) != 0;
        match (self.dom_any, self.dow_any) {
            (true, true) => true,
            (false, true) => dom_ok,
            (true, false) => dow_ok,
            (false, false) => dom_ok || dow_ok,
        }
    }

    /// The first time (Unix seconds, on a whole minute) after `t` that matches, looking up to five years ahead.
    pub fn next_after(&self, t: i64, utc_offset_s: i32) -> Option<i64> {
        let off = utc_offset_s as i64;
        let local = t + off;
        let mut day = local.div_euclid(86_400);
        for _ in 0..(366 * 5) {
            let ds = day * 86_400;
            if self.day_matches(ds) {
                for h in 0..24 {
                    if self.hours & (1 << h) == 0 {
                        continue;
                    }
                    for m in 0..60 {
                        if self.minutes & (1 << m) == 0 {
                            continue;
                        }
                        let s = ds + h * 3600 + m * 60 - off;
                        if s > t {
                            return Some(s);
                        }
                    }
                }
            }
            day += 1;
        }
        None
    }
}

/// One window of a job.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Occurrence {
    /// The job.
    pub job_id: String,
    /// The nominal start (without padding).
    pub nominal: i64,
    /// The padded start.
    pub start: i64,
    /// The padded end.
    pub end: i64,
    /// The job's priority.
    pub priority: i32,
    /// The resource.
    pub resource: String,
}

/// A run that the service carries out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    /// A number for this run.
    pub id: u64,
    /// The job.
    pub job_id: String,
    /// The job's name.
    pub name: String,
    /// The window's nominal start.
    pub nominal: i64,
    /// When it started.
    pub start: i64,
    /// When it must stop.
    pub end: i64,
    /// The file name (from the template, without folder or extension).
    pub file_name: String,
    /// What to record.
    pub target: Target,
    /// The resource.
    pub resource: String,
    /// The priority.
    pub priority: i32,
}

/// What the service must do.
#[derive(Clone, Debug, PartialEq)]
pub enum SchedAction {
    /// Start recording.
    Start(Run),
    /// Stop a run.
    Stop {
        /// The run.
        run_id: u64,
        /// Why: done, preempted, removed or failed.
        reason: Outcome,
    },
}

sdrkit_core::choice_enum! {
    /// How a window ended.
    #[derive(Serialize, Deserialize)]
    pub enum Outcome {
        /// It runs.
        Running = ("running", "Running"),
        /// It ran to its end.
        Done = ("done", "Done"),
        /// A job of higher priority holds its resource.
        Preempted = ("preempted", "Preempted"),
        /// Its job was removed while it ran.
        Removed = ("removed", "Removed"),
        /// The recording failed.
        Failed = ("failed", "Failed"),
        /// It passed while the scheduler was not ticking.
        Missed = ("missed", "Missed"),
        /// Too little of the window was left to start.
        TooLate = ("too_late", "Too late"),
        /// Too little space was free.
        NoSpace = ("no_space", "No space"),
        /// Its resource was busy.
        Busy = ("busy", "Resource busy"),
    }
}

/// A line of the history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// The run, or 0 for a window that never ran.
    pub run_id: u64,
    /// The job.
    pub job_id: String,
    /// Its name.
    pub name: String,
    /// The padded window.
    pub planned: (i64, i64),
    /// When it actually ran.
    pub ran: Option<(i64, i64)>,
    /// How it ended.
    pub outcome: Outcome,
    /// The file, as reported.
    pub file: String,
    /// Bytes written, as reported.
    pub bytes: u64,
}

/// A window first seen more than this many seconds after its start began while the scheduler was not ticking.
pub const LATE_S: i64 = 5;

/// The scheduler. The service saves it whole.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scheduler {
    /// The jobs.
    pub jobs: Vec<Job>,
    /// Jobs a resource runs at once (1 when not listed).
    pub capacity: BTreeMap<String, u32>,
    /// The local time's offset from UTC (seconds).
    pub utc_offset_s: i32,
    /// Runs in progress.
    pub running: Vec<Run>,
    /// The history, oldest first.
    pub history: VecDeque<RunRecord>,
    /// History lines kept (default 500 when 0).
    pub history_cap: usize,
    /// The last tick (missed windows are counted from it).
    pub last_tick: Option<i64>,
    /// Windows already handled: (job id, nominal start).
    handled: BTreeSet<(String, i64)>,
    /// Windows waiting for a busy resource.
    waiting: BTreeSet<(String, i64)>,
    next_run: u64,
}

impl Scheduler {
    /// An empty scheduler.
    pub fn new() -> Scheduler {
        Scheduler::default()
    }

    /// Sets the offset of local time from UTC, seconds.
    pub fn set_utc_offset(&mut self, s: i32) {
        self.utc_offset_s = s;
    }

    /// Adds a job, or replaces the one with its id.
    ///
    /// # Errors
    ///
    /// [`CronError`] if the cron rule of the job does not parse.
    pub fn upsert(&mut self, mut job: Job) -> Result<(), CronError> {
        if let Repeat::Cron { rule } = &job.repeat {
            CronRule::parse(rule)?;
        }
        if let Repeat::Every { seconds } = &mut job.repeat {
            *seconds = (*seconds).max(1);
        }
        match self.jobs.iter_mut().find(|j| j.id == job.id) {
            Some(j) => *j = job,
            None => self.jobs.push(job),
        }
        Ok(())
    }

    /// Removes a job. Returns the stop actions for its runs in progress.
    pub fn remove(&mut self, id: &str, now: i64) -> Vec<SchedAction> {
        self.jobs.retain(|j| j.id != id);
        let ids: Vec<u64> = self
            .running
            .iter()
            .filter(|r| r.job_id == id)
            .map(|r| r.id)
            .collect();
        ids.into_iter()
            .map(|r| self.stop_run(r, now, Outcome::Removed))
            .collect()
    }

    fn cap(&self, resource: &str) -> usize {
        self.capacity.get(resource).copied().unwrap_or(1).max(1) as usize
    }

    fn occurrences(&self, from: i64, to: i64) -> Vec<Occurrence> {
        let mut v = Vec::new();
        for j in self.jobs.iter().filter(|j| j.enabled) {
            // The run limit counts the windows still to come (handled and running ones are counted in `runs` or gone).
            let left = j.max_runs.map(|m| m.saturating_sub(j.runs) as usize);
            let mut k = 0usize;
            for (s, e) in j.windows(from, to, self.utc_offset_s) {
                let key = (j.id.clone(), s);
                let past = self.handled.contains(&key)
                    || self
                        .running
                        .iter()
                        .any(|r| r.job_id == j.id && r.nominal == s);
                if !past {
                    if left.is_some_and(|l| k >= l) {
                        break;
                    }
                    k += 1;
                }
                v.push(Occurrence {
                    job_id: j.id.clone(),
                    nominal: s,
                    start: s - j.pad_before_s.max(0),
                    end: e + j.pad_after_s.max(0),
                    priority: j.priority,
                    resource: j.resource.clone(),
                });
            }
        }
        v.sort_by(|a, b| a.start.cmp(&b.start).then(b.priority.cmp(&a.priority)));
        v
    }

    /// The windows in `[from, to)`, in start order.
    pub fn upcoming(&self, from: i64, to: i64) -> Vec<Occurrence> {
        self.occurrences(from, to)
    }

    fn record(&mut self, r: RunRecord) {
        self.history.push_back(r);
        let cap = if self.history_cap == 0 {
            500
        } else {
            self.history_cap
        };
        while self.history.len() > cap {
            self.history.pop_front();
        }
    }

    fn not_run(&mut self, o: &Occurrence, outcome: Outcome) {
        let name = self
            .jobs
            .iter()
            .find(|j| j.id == o.job_id)
            .map(|j| j.name.clone())
            .unwrap_or_default();
        self.record(RunRecord {
            run_id: 0,
            job_id: o.job_id.clone(),
            name,
            planned: (o.start, o.end),
            ran: None,
            outcome,
            file: String::new(),
            bytes: 0,
        });
    }

    fn stop_run(&mut self, run_id: u64, now: i64, outcome: Outcome) -> SchedAction {
        self.running.retain(|r| r.id != run_id);
        if let Some(h) = self.history.iter_mut().rev().find(|h| h.run_id == run_id) {
            if let Some((s, _)) = h.ran {
                h.ran = Some((s, now));
            }
            if h.outcome == Outcome::Running {
                h.outcome = outcome;
            }
        }
        SchedAction::Stop {
            run_id,
            reason: outcome,
        }
    }

    /// Advances to `now` (Unix seconds) with `free_bytes` free where recordings go (`None`: unknown). Returns what to stop and
    /// start, stops first.
    pub fn tick(&mut self, now: i64, free_bytes: Option<u64>) -> Vec<SchedAction> {
        let mut acts = Vec::new();
        // Runs at their end.
        let ended: Vec<u64> = self
            .running
            .iter()
            .filter(|r| r.end <= now)
            .map(|r| r.id)
            .collect();
        for id in ended {
            acts.push(self.stop_run(id, now, Outcome::Done));
        }
        let last = match self.last_tick {
            Some(l) if l <= now => l,
            _ => now, // the first tick, or the clock went back
        };
        self.last_tick = Some(now);
        // Windows whose padded start is in (last - 1, now], or that started earlier and are still pending.
        let horizon = self
            .jobs
            .iter()
            .map(|j| {
                j.grace_s.max(0)
                    + j.duration_s.max(0)
                    + j.pad_after_s.max(0)
                    + j.pad_before_s.max(0)
            })
            .max()
            .unwrap_or(0)
            .min(86_400 * 7);
        let occ = self.occurrences(last.min(now - horizon), now + 1);
        // Forget handled windows long past.
        let oldest = now - horizon - 86_400;
        self.handled.retain(|(_, s)| *s >= oldest);
        self.waiting.retain(|(_, s)| *s >= oldest);
        for o in occ {
            if o.start > now || self.handled.contains(&(o.job_id.clone(), o.nominal)) {
                continue;
            }
            if self
                .running
                .iter()
                .any(|r| r.job_id == o.job_id && r.nominal == o.nominal)
            {
                continue;
            }
            let Some(job) = self.jobs.iter().find(|j| j.id == o.job_id).cloned() else {
                continue;
            };
            let key = (o.job_id.clone(), o.nominal);
            if o.end <= now {
                // Passed entirely: missed if it started after the last tick (a window seen before was handled already).
                if o.end > last || o.start >= last {
                    self.not_run(&o, Outcome::Missed);
                }
                self.handled.insert(key);
                continue;
            }
            let late = now - o.start;
            let was_asleep = late > LATE_S && !self.waiting.contains(&key);
            if was_asleep && (job.on_missed == Missed::Skip || late > job.grace_s) {
                self.not_run(&o, Outcome::Missed);
                self.handled.insert(key);
                continue;
            }
            if o.end - now < job.min_s.min(o.end - o.start) {
                self.not_run(&o, Outcome::TooLate);
                self.handled.insert(key);
                continue;
            }
            if let Some(free) = free_bytes {
                if free < job.min_free_bytes {
                    self.not_run(&o, Outcome::NoSpace);
                    self.handled.insert(key);
                    continue;
                }
            }
            if let Some(m) = job.max_runs {
                if job.runs >= m {
                    self.handled.insert(key);
                    continue;
                }
            }
            // The resource.
            let busy: Vec<(u64, i32)> = self
                .running
                .iter()
                .filter(|r| r.resource == o.resource)
                .map(|r| (r.id, r.priority))
                .collect();
            if busy.len() >= self.cap(&o.resource) {
                let lowest = busy.iter().min_by_key(|b| b.1).copied();
                match lowest {
                    Some((rid, p)) if p < o.priority => {
                        acts.push(self.stop_run(rid, now, Outcome::Preempted));
                    }
                    _ => {
                        if job.on_conflict == Conflict::StartWhenFree {
                            self.waiting.insert(key);
                        } else {
                            self.not_run(&o, Outcome::Busy);
                            self.handled.insert(key);
                        }
                        continue;
                    }
                }
            }
            let id = self.next_run + 1;
            self.next_run = id;
            let run = Run {
                id,
                job_id: o.job_id.clone(),
                name: job.name.clone(),
                nominal: o.nominal,
                start: now,
                end: o.end,
                file_name: render_name(&job, now, id, self.utc_offset_s),
                target: job.target.clone(),
                resource: o.resource.clone(),
                priority: o.priority,
            };
            if let Some(j) = self.jobs.iter_mut().find(|j| j.id == o.job_id) {
                j.runs += 1;
            }
            self.record(RunRecord {
                run_id: id,
                job_id: o.job_id.clone(),
                name: job.name.clone(),
                planned: (o.start, o.end),
                ran: Some((now, now)),
                outcome: Outcome::Running,
                file: String::new(),
                bytes: 0,
            });
            self.waiting.remove(&key);
            self.handled.insert(key);
            self.running.push(run.clone());
            acts.push(SchedAction::Start(run));
        }
        acts
    }

    /// Reports on a run: the file, the bytes, and whether it failed. A failure while it runs ends the run.
    pub fn report(&mut self, run_id: u64, now: i64, file: &str, bytes: u64, failed: bool) {
        if failed && self.running.iter().any(|r| r.id == run_id) {
            self.stop_run(run_id, now, Outcome::Failed);
        }
        if let Some(h) = self.history.iter_mut().rev().find(|h| h.run_id == run_id) {
            h.file = file.into();
            h.bytes = bytes;
            if failed {
                h.outcome = Outcome::Failed;
            }
        }
    }

    /// The saved form, JSON text.
    pub(crate) fn to_json(&self) -> Vec<u8> {
        // Plain structs of strings and numbers always serialise.
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Reads the saved form.
    pub(crate) fn from_json(s: &[u8]) -> Result<Scheduler, crate::error::RecordError> {
        serde_json::from_slice(s).map_err(|e| crate::error::RecordError::Saved {
            line: e.line(),
            column: e.column(),
        })
    }
}

/// A file name from a job's template at `now` for run `run`: `{name}`, `{job}`, `{date}` (YYYYMMDD), `{time}` (HHMMSS),
/// `{run}`, `{freq}` (Hz), `{stack}` and `{session}`. Characters that are not safe in a file name become `-`.
pub fn render_name(job: &Job, now: i64, run: u64, utc_offset_s: i32) -> String {
    let (y, mo, d, h, mi, s) = civil(now + utc_offset_s as i64);
    let mut out = String::new();
    let t = &job.name_template;
    let mut rest = t.as_str();
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('}') else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let key = &rest[i + 1..i + j];
        let v = match key {
            "name" => job.name.clone(),
            "job" => job.id.clone(),
            "date" => format!("{y:04}{mo:02}{d:02}"),
            "time" => format!("{h:02}{mi:02}{s:02}"),
            "run" => run.to_string(),
            "freq" => job
                .target
                .freq_hz
                .map_or_else(String::new, |f| format!("{f:.0}")),
            "stack" => job.target.stack.clone(),
            "session" => job.target.session.clone(),
            _ => String::new(),
        };
        out.push_str(&v);
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    let clean: String = out
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() {
        format!("run{run}")
    } else {
        clean
    }
}

#[cfg(test)]
mod scenarios;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_rules() {
        // 2024-01-01 00:00:00 UTC was a Monday.
        let t0 = 1_704_067_200;
        let c = CronRule::parse("30 6 * * 1-5").unwrap();
        assert_eq!(c.next_after(t0, 0), Some(t0 + 6 * 3600 + 1800));
        // Saturday 6 January: next is Monday 8 January.
        let sat = t0 + 5 * 86_400;
        assert_eq!(
            c.next_after(sat, 0),
            Some(t0 + 7 * 86_400 + 6 * 3600 + 1800)
        );
        // A local offset of +1 h moves the UTC time back an hour.
        assert_eq!(c.next_after(t0, 3600), Some(t0 + 5 * 3600 + 1800));
        let every = CronRule::parse("*/15 * * * *").unwrap();
        assert_eq!(every.next_after(t0, 0), Some(t0 + 900));
        let named = CronRule::parse("0 12 1 jan,jul sun").unwrap();
        // The 1st of January (a Monday) matches by day of month, Sunday 7 January by weekday.
        assert_eq!(named.next_after(t0, 0), Some(t0 + 12 * 3600));
        assert_eq!(
            named.next_after(t0 + 13 * 3600, 0),
            Some(t0 + 6 * 86_400 + 12 * 3600)
        );
        assert_eq!(
            CronRule::parse("0 61 * * *"),
            Err(CronError::Range { at: 2 })
        );
        assert_eq!(
            CronRule::parse("0 x * * *"),
            Err(CronError::Field { at: 2 })
        );
        assert_eq!(CronRule::parse("* * *"), Err(CronError::Fields));
        assert_eq!(
            CronRule::parse("@daily").unwrap(),
            CronRule::parse("0 0 * * *").unwrap()
        );
    }

    #[test]
    fn names() {
        let j = Job {
            id: "j1".into(),
            name: "News/Night".into(),
            name_template: "{name}_{freq}_{date}-{time}_{missing}".into(),
            target: Target {
                freq_hz: Some(98_500_000.0),
                ..Target::default()
            },
            ..Default::default()
        };
        assert_eq!(
            render_name(&j, 1_704_067_200, 3, 3600),
            "News-Night_98500000_20240101-010000_"
        );
    }
}

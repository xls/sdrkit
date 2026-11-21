//! The judge of sudden changes of one transmitter's level: it keeps the levels of the last hour and the minute means of the
//! last day, and starts an event when the mean of the last 2 minutes differs from the mean of 2 minutes 10 minutes earlier by
//! the step threshold or more.

use std::collections::VecDeque;

/// The window of levels kept, s.
pub const HOUR_S: f64 = 3600.0;
/// The levels needed before anything is judged, s.
pub const WARM_S: f64 = 900.0;
/// The span of the mean "now" and of the mean "before", s.
pub const MEAN_S: f64 = 120.0;
/// How long before now the mean "before" ends, s.
pub const BACK_S: f64 = 600.0;
/// The lowest baseline at which an event can start, dB.
pub const MIN_BASELINE_DB: f64 = 6.0;
/// The minute means kept (24 h).
pub const MINUTES: usize = 1440;

/// One event of a transmitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidEvent {
    /// The time of the level that started it, s.
    pub start_s: f64,
    /// now − baseline at the start, dB.
    pub rise_db: f64,
    /// The largest now − before (by magnitude) so far, dB.
    pub peak_db: f64,
    /// The time of the level that ended it, s.
    pub end_s: Option<f64>,
}

/// What one level changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Verdict {
    /// An event started with this level.
    pub started: Option<SidEvent>,
    /// An event ended with this level.
    pub ended: Option<SidEvent>,
}

/// The judge of one transmitter. It is fed one level per sample interval `dt`, with the time of the level.
#[derive(Clone, Debug)]
pub struct Judge {
    dt: f64,
    step_db: f64,
    hour: VecDeque<f64>,
    fed: u64,
    minute: Option<(i64, f64, u32)>,
    minutes: VecDeque<[f64; 2]>,
    minutes_made: u64,
    active: Option<SidEvent>,
    now: Option<f64>,
    before: Option<f64>,
    baseline: Option<f64>,
}

/// The mean of `v[a..b]`.
fn mean(v: &VecDeque<f64>, a: usize, b: usize) -> f64 {
    v.range(a..b).sum::<f64>() / (b - a).max(1) as f64
}

impl Judge {
    /// A judge of levels every `dt` seconds with the step threshold `step_db`.
    pub fn new(dt: f64, step_db: f64) -> Self {
        let dt = dt.max(1e-3);
        Judge {
            dt,
            step_db,
            hour: VecDeque::with_capacity((HOUR_S / dt) as usize + 1),
            fed: 0,
            minute: None,
            minutes: VecDeque::with_capacity(MINUTES),
            minutes_made: 0,
            active: None,
            now: None,
            before: None,
            baseline: None,
        }
    }

    /// Changes the step threshold.
    pub fn set_step(&mut self, step_db: f64) {
        self.step_db = step_db;
    }

    /// Forgets the levels of the hour and any event; the 15 minutes before judging start again. The minute means stay.
    pub fn restart(&mut self) {
        self.hour.clear();
        self.fed = 0;
        self.active = None;
        self.now = None;
        self.before = None;
        self.baseline = None;
    }

    fn count(&self, s: f64) -> usize {
        ((s / self.dt).round() as usize).max(1)
    }

    /// Takes the level `level_db` of time `t_s`.
    pub fn push(&mut self, t_s: f64, level_db: f64) -> Verdict {
        let mut v = Verdict::default();
        if !level_db.is_finite() {
            return v;
        }
        self.minute_mean(t_s, level_db);
        let keep = (HOUR_S / self.dt) as usize + 1;
        if self.hour.len() == keep {
            self.hour.pop_front();
        }
        self.hour.push_back(level_db);
        self.fed += 1;
        let (m, back) = (self.count(MEAN_S), self.count(BACK_S));
        let len = self.hour.len();
        if (self.fed as f64) * self.dt < WARM_S || len < m + back {
            return v;
        }
        let now = mean(&self.hour, len - m, len);
        let before = mean(&self.hour, len - back - m, len - back);
        let mut sorted: Vec<f64> = self.hour.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        let baseline = sorted[sorted.len() / 2];
        self.now = Some(now);
        self.before = Some(before);
        self.baseline = Some(baseline);
        let step = now - before;
        if let Some(a) = self.active.as_mut() {
            if step.abs() > a.peak_db.abs() {
                a.peak_db = step;
            }
            if (now - baseline).abs() < 0.5 * self.step_db {
                a.end_s = Some(t_s);
                v.ended = Some(*a);
                self.active = None;
            }
        } else if baseline >= MIN_BASELINE_DB
            && step.abs() >= self.step_db
            && level_db >= super::meter::PRESENT_DB
        {
            let e = SidEvent {
                start_s: t_s,
                rise_db: now - baseline,
                peak_db: step,
                end_s: None,
            };
            self.active = Some(e);
            v.started = Some(e);
        }
        v
    }

    fn minute_mean(&mut self, t_s: f64, level: f64) {
        let m = (t_s / 60.0).floor() as i64;
        match &mut self.minute {
            Some((cur, sum, n)) if *cur == m => {
                *sum += level;
                *n += 1;
            }
            _ => {
                if let Some((cur, sum, n)) = self.minute {
                    if self.minutes.len() == MINUTES {
                        self.minutes.pop_front();
                    }
                    self.minutes
                        .push_back([cur as f64 * 60.0, sum / f64::from(n.max(1))]);
                    self.minutes_made += 1;
                }
                self.minute = Some((m, level, 1));
            }
        }
    }

    /// The minute means of the last day: (start of the minute, s; mean level, dB), oldest first.
    pub fn minutes(&self) -> &VecDeque<[f64; 2]> {
        &self.minutes
    }

    /// The minute means made since the judge was made (it keeps counting when the oldest are dropped).
    pub fn minutes_made(&self) -> u64 {
        self.minutes_made
    }

    /// The event in progress.
    pub fn active(&self) -> Option<&SidEvent> {
        self.active.as_ref()
    }

    /// The mean of the last 2 minutes, once judging has started.
    pub fn now(&self) -> Option<f64> {
        self.now
    }

    /// The mean of the 2 minutes that ended 10 minutes ago, once judging has started.
    pub fn before(&self) -> Option<f64> {
        self.before
    }

    /// The median of the last hour, once judging has started.
    pub fn baseline(&self) -> Option<f64> {
        self.baseline
    }

    /// now − baseline, once judging has started.
    pub fn rise(&self) -> Option<f64> {
        Some(self.now? - self.baseline?)
    }

    /// Whether 15 minutes of levels have been judged.
    pub fn judging(&self) -> bool {
        self.baseline.is_some()
    }
}

//! The counting of sferics as a plain type: impulses over the floor, mains trains left out, strikes and flashes, the rates,
//! the strikes of each minute and the storm trend.

use sdrkit_algo_dsp::impulse::{
    Impulse, ImpulseConfig, ImpulseDetector, PeriodicTrain, TrainConfig,
};
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::Complex32;
use std::collections::VecDeque;

/// An impulse is counted this long after its start, s, so that a mains train is known before its first impulses count.
pub const COMMIT_S: f64 = 0.5;
/// A strike more than this after the strike before starts a new flash, s.
pub const FLASH_GAP_S: f64 = 0.5;
/// The strikes kept in the table.
pub const TABLE_ROWS: usize = 100;
/// The minutes of the histogram.
pub const MINUTES: usize = 60;
/// The span of the storm trend, s: two halves of 5 minutes.
pub const TREND_S: f64 = 600.0;

/// The phase of the storm trend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Fewer than 5 strikes in 10 minutes.
    #[default]
    None,
    /// Few strikes and no clear change.
    Distant,
    /// No clear change.
    Steady,
    /// Stronger or more frequent strikes.
    Approaching,
    /// Many strikes near the strongest yet.
    Overhead,
    /// Weaker or fewer strikes.
    Receding,
}

impl Phase {
    /// The word of the phase.
    pub fn key(self) -> &'static str {
        match self {
            Phase::None => "none",
            Phase::Distant => "distant",
            Phase::Steady => "steady",
            Phase::Approaching => "approaching",
            Phase::Overhead => "overhead",
            Phase::Receding => "receding",
        }
    }

    /// Whether a change to this phase is announced.
    pub fn announced(self) -> bool {
        matches!(self, Phase::Approaching | Phase::Overhead | Phase::Receding)
    }
}

/// The storm trend at one time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trend {
    /// The phase.
    pub phase: Phase,
    /// The median peak of the last 5 minutes minus that of the 5 minutes before, dB; 0 unless both have 4 strikes or more.
    pub change_db: f64,
    /// Strikes per minute in the 5 minutes before the last 5.
    pub rate_a: f64,
    /// Strikes per minute in the last 5 minutes.
    pub rate_b: f64,
}

/// The storm trend at `now` (s) of the strikes `(time s, peak dB)` given oldest first, with `best_db` the highest peak since
/// the trend began. The halves are `(now - 600, now - 300]` and `(now - 300, now]`.
pub fn storm_trend(strikes: impl Iterator<Item = (f64, f64)>, now: f64, best_db: f64) -> Trend {
    let mut a = Vec::new();
    let mut b = Vec::new();
    for (t, p) in strikes {
        if t > now - TREND_S && t <= now - TREND_S / 2.0 {
            a.push(p);
        } else if t > now - TREND_S / 2.0 && t <= now {
            b.push(p);
        }
    }
    let rate_a = a.len() as f64 / 5.0;
    let rate_b = b.len() as f64 / 5.0;
    let both = a.len() >= 4 && b.len() >= 4;
    let mut scratch = Vec::new();
    let median = |v: &[f64], s: &mut Vec<f64>| quantile_f64(v, 0.5, s).unwrap_or(0.0);
    let mb = median(&b, &mut scratch);
    let change_db = if both {
        mb - median(&a, &mut scratch)
    } else {
        0.0
    };
    let (ra, rb, c) = (rate_a, rate_b, change_db);
    let phase = if a.len() + b.len() < 5 {
        Phase::None
    } else if rb >= 12.0 && best_db - mb <= 6.0 {
        Phase::Overhead
    } else if both && ((c >= 2.5 && rb > 0.6 * ra) || (rb >= 1.4 * ra && c > -2.5 && c >= 0.5)) {
        Phase::Approaching
    } else if both && ((c <= -2.5 && rb < 1.4 * ra) || (rb <= 0.6 * ra && c <= -0.5)) {
        Phase::Receding
    } else if both {
        Phase::Steady
    } else if rb < 3.0 {
        Phase::Distant
    } else {
        Phase::Steady
    };
    Trend {
        phase,
        change_db,
        rate_a,
        rate_b,
    }
}

/// One counted strike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    /// Its start, s of signal.
    pub time_s: f64,
    /// Its start in UTC, s since 1970.
    pub utc: f64,
    /// Its peak over the floor, dB.
    pub peak_db: f64,
    /// Its width, µs.
    pub width_us: f64,
    /// The number of its flash.
    pub flash: u64,
}

/// A change of the storm phase that is announced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StormChange {
    /// When, s of signal.
    pub time_s: f64,
    /// The trend then.
    pub trend: Trend,
}

/// The counter of sferics at one input rate.
#[derive(Clone, Debug)]
pub struct Counter {
    rate: f64,
    det: ImpulseDetector,
    train: PeriodicTrain,
    mains_filter: bool,
    queue: VecDeque<(Impulse, bool)>,
    delay: u64,
    tick: u64,
    found: Vec<Impulse>,
    start_utc: f64,
    /// Strikes counted.
    pub strikes: u64,
    /// Flashes counted.
    pub flashes: u64,
    /// Impulses of a mains train left out.
    pub mains: u64,
    /// Impulses too long to be lightning.
    pub too_long: u64,
    recent: VecDeque<(f64, f64)>,
    minutes: VecDeque<(i64, u32)>,
    table: VecDeque<Strike>,
    last: Option<Strike>,
    best_db: f64,
    trend: Trend,
    changes: Vec<StormChange>,
}

impl Counter {
    /// A counter at `rate` (Hz) with the start level `threshold_db` over the floor, the joining time `merge_ms` and the mains
    /// filter on or off.
    ///
    /// # Errors
    ///
    /// [`DspError`] if the detector refuses the lengths of the rate.
    pub fn new(
        rate: f64,
        threshold_db: f64,
        merge_ms: f64,
        mains_filter: bool,
    ) -> Result<Self, DspError> {
        Ok(Counter {
            rate,
            det: ImpulseDetector::new(ImpulseConfig::counter(rate, threshold_db, merge_ms * 1e-3))?,
            train: PeriodicTrain::new(TrainConfig::mains()),
            mains_filter,
            queue: VecDeque::new(),
            delay: (COMMIT_S * rate).round() as u64,
            tick: (rate.round() as u64).max(1),
            found: Vec::new(),
            start_utc: 0.0,
            strikes: 0,
            flashes: 0,
            mains: 0,
            too_long: 0,
            recent: VecDeque::new(),
            minutes: VecDeque::new(),
            table: VecDeque::with_capacity(TABLE_ROWS),
            last: None,
            best_db: f64::NEG_INFINITY,
            trend: Trend::default(),
            changes: Vec::new(),
        })
    }

    /// Sets the start level over the floor, dB (the end level is 3 dB lower).
    pub fn set_threshold(&mut self, threshold_db: f64) {
        self.det.set_levels(threshold_db, threshold_db - 3.0);
    }

    /// Sets the joining time, ms.
    pub fn set_merge_ms(&mut self, merge_ms: f64) {
        self.det
            .set_merge((self.rate * merge_ms * 1e-3).round() as usize);
    }

    /// Turns the mains filter on or off.
    pub fn set_mains_filter(&mut self, on: bool) {
        self.mains_filter = on;
    }

    /// Sets the UTC of sample 0, s since 1970.
    pub fn set_start_utc(&mut self, utc: f64) {
        self.start_utc = utc;
    }

    /// The signal time of the next sample, s.
    pub fn now(&self) -> f64 {
        self.det.position() as f64 / self.rate
    }

    /// The floor (mean noise power), once known.
    pub fn floor(&self) -> Option<f64> {
        self.det.floor()
    }

    /// Whether the floor is valid.
    pub fn floor_valid(&self) -> bool {
        self.det.is_valid()
    }

    /// Whether a mains train is being left out.
    pub fn mains_active(&self) -> bool {
        self.train.is_on()
    }

    /// The newest strike.
    pub fn last(&self) -> Option<&Strike> {
        self.last.as_ref()
    }

    /// The strikes of the table, newest last.
    pub fn table(&self) -> &VecDeque<Strike> {
        &self.table
    }

    /// The storm trend of the last whole second.
    pub fn trend(&self) -> Trend {
        self.trend
    }

    /// Takes the announced changes of the storm phase.
    pub fn take_changes(&mut self) -> Vec<StormChange> {
        std::mem::take(&mut self.changes)
    }

    /// Strikes with a start in the last minute.
    pub fn per_min_1(&self) -> f64 {
        let now = self.now();
        self.recent.iter().filter(|(t, _)| *t > now - 60.0).count() as f64
    }

    /// Strikes per minute over the last 10 minutes (over the time so far when that is shorter, at least one minute).
    pub fn per_min_10(&self) -> f64 {
        let now = self.now();
        let n = self.recent.iter().filter(|(t, _)| *t > now - 600.0).count() as f64;
        n / (now.clamp(60.0, 600.0) / 60.0)
    }

    /// Strikes in each of the last 60 minutes of signal, oldest first; the last is the running minute.
    pub fn per_minute(&self) -> [u32; MINUTES] {
        let cur = (self.now() / 60.0).floor() as i64;
        let mut h = [0u32; MINUTES];
        for &(m, c) in &self.minutes {
            let back = cur - m;
            if (0..MINUTES as i64).contains(&back) {
                h[MINUTES - 1 - back as usize] = c;
            }
        }
        h
    }

    /// Takes `x`, whose first sample has the absolute index `first`.
    pub fn process(&mut self, x: &[Complex32], first: u64) {
        let mut done = 0usize;
        while done < x.len() {
            let at = first + done as u64;
            let to_tick = (self.tick - at % self.tick) as usize;
            let n = to_tick.min(x.len() - done);
            self.run(&x[done..done + n], at);
            done += n;
            let next = at + n as u64;
            if next.is_multiple_of(self.tick) {
                self.evaluate(next as f64 / self.rate);
            }
        }
    }

    /// The input ended: every impulse held is counted now.
    pub fn finish(&mut self) {
        let mut found = std::mem::take(&mut self.found);
        found.clear();
        self.det.flush(&mut found);
        for imp in found.drain(..) {
            self.arrive(imp);
        }
        self.found = found;
        self.commit_due(u64::MAX);
    }

    /// Empties the table, the counters, the histogram and the trend; keeps the floor and the train.
    pub fn clear(&mut self) {
        self.strikes = 0;
        self.flashes = 0;
        self.mains = 0;
        self.too_long = 0;
        self.recent.clear();
        self.minutes.clear();
        self.table.clear();
        self.last = None;
        self.best_db = f64::NEG_INFINITY;
        self.trend = Trend::default();
        self.changes.clear();
    }

    /// As [`Counter::clear`], and the floor and the train are learned again; impulses not yet counted are dropped.
    pub fn reset(&mut self) {
        self.clear();
        self.det.reset();
        self.train.reset();
        self.queue.clear();
    }

    fn run(&mut self, x: &[Complex32], first: u64) {
        let mut found = std::mem::take(&mut self.found);
        found.clear();
        self.det.process(x, first, &mut found);
        for imp in found.drain(..) {
            self.commit_due(imp.given_at);
            self.arrive(imp);
        }
        self.found = found;
        if let Some(last) = (first + x.len() as u64).checked_sub(1) {
            self.commit_due(last);
        }
    }

    fn arrive(&mut self, imp: Impulse) {
        if imp.too_long {
            self.too_long += 1;
            return;
        }
        let hit = self.train.push(imp.start as f64 / self.rate);
        self.queue.push_back((imp, hit));
    }

    /// Counts every impulse whose commit time is at or before sample `n`.
    fn commit_due(&mut self, n: u64) {
        while let Some((imp, _)) = self.queue.front() {
            if imp.start.saturating_add(self.delay) > n {
                break;
            }
            let Some((imp, hit)) = self.queue.pop_front() else {
                break;
            };
            if self.mains_filter && hit && self.train.is_on() {
                self.mains += 1;
            } else {
                self.strike(&imp);
            }
        }
    }

    fn strike(&mut self, imp: &Impulse) {
        let t = imp.start as f64 / self.rate;
        if self.last.is_none_or(|l| t - l.time_s > FLASH_GAP_S) {
            self.flashes += 1;
        }
        self.strikes += 1;
        let s = Strike {
            time_s: t,
            utc: self.start_utc + t,
            peak_db: imp.peak_db(),
            width_us: imp.width() as f64 / self.rate * 1e6,
            flash: self.flashes,
        };
        self.best_db = self.best_db.max(s.peak_db);
        self.recent.push_back((t, s.peak_db));
        while self.recent.front().is_some_and(|(r, _)| t - r > TREND_S) {
            self.recent.pop_front();
        }
        let m = (t / 60.0).floor() as i64;
        match self.minutes.back_mut() {
            Some((bm, c)) if *bm == m => *c += 1,
            _ => self.minutes.push_back((m, 1)),
        }
        while self
            .minutes
            .front()
            .is_some_and(|(bm, _)| m - bm >= MINUTES as i64)
        {
            self.minutes.pop_front();
        }
        if self.table.len() == TABLE_ROWS {
            self.table.pop_front();
        }
        self.table.push_back(s);
        self.last = Some(s);
    }

    fn evaluate(&mut self, now: f64) {
        let t = storm_trend(self.recent.iter().copied(), now, self.best_db);
        if t.phase != self.trend.phase && t.phase.announced() {
            self.changes.push(StormChange {
                time_s: now,
                trend: t,
            });
        }
        self.trend = t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(times: &[(f64, f64)], now: f64, best: f64) -> Trend {
        storm_trend(times.iter().copied(), now, best)
    }

    #[test]
    fn the_phases_of_the_trend() {
        // Fewer than 5 strikes.
        assert_eq!(at(&[(10.0, 20.0); 4], 100.0, 20.0).phase, Phase::None);
        // A few strikes in the last 5 minutes only: distant.
        let few: Vec<_> = (0..6).map(|i| (400.0 + i as f64, 15.0)).collect();
        assert_eq!(at(&few, 600.0, 15.0).phase, Phase::Distant);
        // Equal halves and peaks: steady.
        let even: Vec<_> = (0..20).map(|i| (i as f64 * 30.0 + 1.0, 20.0)).collect();
        let t = at(&even, 600.0, 20.0);
        assert_eq!((t.phase, t.change_db), (Phase::Steady, 0.0));
        // Stronger in the last half: approaching.
        let up: Vec<_> = (0..20)
            .map(|i| (i as f64 * 30.0 + 1.0, if i < 10 { 15.0 } else { 20.0 }))
            .collect();
        let t = at(&up, 600.0, 30.0);
        assert_eq!(t.phase, Phase::Approaching);
        assert!((t.change_db - 5.0).abs() < 1e-9);
        // Weaker: receding.
        let down: Vec<_> = (0..20)
            .map(|i| (i as f64 * 30.0 + 1.0, if i < 10 { 20.0 } else { 15.0 }))
            .collect();
        assert_eq!(at(&down, 600.0, 30.0).phase, Phase::Receding);
        // Many strikes near the best: overhead.
        let many: Vec<_> = (0..70).map(|i| (301.0 + i as f64 * 4.0, 28.0)).collect();
        assert_eq!(at(&many, 600.0, 30.0).phase, Phase::Overhead);
    }
}
